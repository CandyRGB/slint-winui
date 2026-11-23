use std::collections::{HashMap, VecDeque};

use slint::{Model, ModelRc, SharedString, VecModel};

use crate::{
    ItemsRepeaterGeometry, ItemsRepeaterPlacement, ItemsRepeaterSize, RelativePanelAlignment,
    RelativePanelAxis, RelativePanelBehavior, RelativePanelChild, RelativePanelIssue,
    RelativePanelLayout,
};

pub fn install(behavior: RelativePanelBehavior<'_>) {
    behavior.on_layout(layout);
}

type Index = usize;
type LayoutError = (RelativePanelIssue, SharedString);

#[derive(Clone, Copy, PartialEq)]
enum Anchor {
    Panel,
    Aligned(Index),
    Adjacent(Index),
    Center(Index),
    Free,
}

#[derive(Clone, Copy)]
struct Axis {
    start: Anchor,
    end: Anchor,
    center_panel: bool,
}

impl Axis {
    fn center(self) -> Option<Index> {
        match (self.start, self.end) {
            (Anchor::Center(index), Anchor::Center(_)) => Some(index),
            _ => None,
        }
    }

    fn centered_in_panel(self) -> bool {
        self.center_panel && self.start == Anchor::Free && self.end == Anchor::Free
    }
}

#[derive(Clone, Copy, Default)]
struct Span {
    start: f32,
    size: f32,
}

impl Span {
    fn end(self) -> f32 {
        self.start + self.size
    }

    fn center(self) -> f32 {
        self.start + self.size / 2.0
    }
}

fn resolve_axis(
    input: &RelativePanelAxis,
    names: &HashMap<&str, Index>,
    dependencies: &mut Vec<Index>,
) -> Result<Axis, LayoutError> {
    let mut resolve = |name: &SharedString| match name.as_str() {
        "" => Ok(None),
        key => {
            let index = names
                .get(key)
                .copied()
                .ok_or_else(|| (RelativePanelIssue::MissingTarget, name.clone()))?;
            dependencies.push(index);
            Ok(Some(index))
        }
    };
    // Even a reference shadowed by higher-priority alignment participates in cycle detection.
    let before = resolve(&input.before)?;
    let after = resolve(&input.after)?;
    let start = resolve(&input.align_start_with)?;
    let end = resolve(&input.align_end_with)?;
    let center = resolve(&input.align_center_with)?;
    let anchor = |panel, aligned: Option<Index>, adjacent: Option<Index>| {
        if panel {
            Anchor::Panel
        } else {
            aligned
                .map(Anchor::Aligned)
                .or_else(|| center.map(Anchor::Center))
                .or_else(|| adjacent.map(Anchor::Adjacent))
                .unwrap_or(Anchor::Free)
        }
    };
    Ok(Axis {
        start: anchor(input.align_start_with_panel, start, after),
        end: anchor(input.align_end_with_panel, end, before),
        center_panel: input.align_center_with_panel,
    })
}

fn arrange_axis(axis: Axis, resolved: &[Span], limit: f32, desired: f32) -> (Span, f32) {
    let mut start = match axis.start {
        Anchor::Aligned(index) => resolved[index].start,
        Anchor::Adjacent(index) => resolved[index].end(),
        _ => 0.0,
    };
    let mut end = match axis.end {
        Anchor::Aligned(index) => resolved[index].end(),
        Anchor::Adjacent(index) => resolved[index].start,
        _ => limit,
    };
    if let Some(center) = axis.center() {
        let center = resolved[center].center();
        let half = center.min(limit - center);
        start = center - half;
        end = center + half;
    }
    let available = end - start;
    let size = available.min(desired);
    let slot = match (axis.start, axis.end) {
        (
            Anchor::Panel | Anchor::Aligned(_) | Anchor::Adjacent(_),
            Anchor::Panel | Anchor::Aligned(_) | Anchor::Adjacent(_),
        ) => Span {
            start,
            size: available,
        },
        (_, Anchor::Panel | Anchor::Aligned(_) | Anchor::Adjacent(_)) => Span {
            start: end - size,
            size,
        },
        _ if axis.center().is_some() || axis.centered_in_panel() => Span {
            start: (start + end - size) / 2.0,
            size,
        },
        _ => Span { start, size },
    };
    (slot, available.max(0.0))
}

enum MeasureStep {
    Visit(Index, f32, bool),
    Cap(usize, f32, bool),
    Balance(f32),
}

fn desired_axis(axes: &[Axis], sizes: &[f32]) -> f32 {
    let mut leaves = vec![true; axes.len()];
    for axis in axes {
        for anchor in [axis.start, axis.end] {
            if let Anchor::Aligned(index) | Anchor::Adjacent(index) = anchor {
                leaves[index] = false;
            }
        }
        if let Some(index) = axis.center() {
            leaves[index] = false;
        }
    }
    let mut desired: f32 = 0.0;
    // Walk outward from each leaf. A panel edge can cap the bounds of the chain.
    let mut stack = Vec::new();
    for (leaf, _) in leaves.iter().enumerate().filter(|(_, is_leaf)| **is_leaf) {
        let mut bounds = [0.0_f32; 2];
        let mut capped = [false; 2];
        stack.push(MeasureStep::Visit(leaf, 0.0, true));
        while let Some(step) = stack.pop() {
            match step {
                MeasureStep::Visit(index, initial, positive) => {
                    let (side, other, direction) =
                        if positive { (1, 0, 1.0) } else { (0, 1, -1.0) };
                    let edge = initial + sizes[index] * direction;
                    bounds[side] = extreme(side, bounds[side], edge);
                    let axis = axes[index];
                    let (leading, trailing) = if positive {
                        (axis.start, axis.end)
                    } else {
                        (axis.end, axis.start)
                    };
                    if let Some(neighbor) = axis.center() {
                        let opposite = edge - direction * (sizes[index] + sizes[neighbor]) / 2.0;
                        bounds[other] = extreme(other, bounds[other], opposite);
                        stack.push(MeasureStep::Visit(neighbor, opposite, positive));
                    } else if axis.centered_in_panel() {
                        stack.push(MeasureStep::Balance((initial + edge) / 2.0));
                    }
                    match trailing {
                        Anchor::Panel => stack.push(MeasureStep::Cap(other, initial, true)),
                        Anchor::Aligned(neighbor) => {
                            stack.push(MeasureStep::Visit(neighbor, initial, positive))
                        }
                        Anchor::Adjacent(neighbor) => {
                            stack.push(MeasureStep::Visit(neighbor, initial, !positive))
                        }
                        _ => {}
                    }
                    match leading {
                        Anchor::Panel => stack.push(MeasureStep::Cap(side, edge, false)),
                        Anchor::Aligned(neighbor) if leading != trailing => {
                            stack.push(MeasureStep::Visit(neighbor, edge, !positive))
                        }
                        Anchor::Adjacent(neighbor) => {
                            stack.push(MeasureStep::Visit(neighbor, edge, positive))
                        }
                        _ => {}
                    }
                }
                MeasureStep::Cap(side, edge, extend) => {
                    if !capped[side] {
                        bounds[side] = edge;
                        capped[side] = true;
                    } else if extend {
                        bounds[side] = extreme(side, bounds[side], edge);
                    }
                }
                MeasureStep::Balance(center) => {
                    bounds = [0.0, 2.0 * (bounds[1] - center).max(center - bounds[0])];
                }
            }
        }
        desired = desired.max(bounds[1] - bounds[0]);
    }
    desired
}

fn extreme(side: usize, first: f32, second: f32) -> f32 {
    if side == 0 {
        first.min(second)
    } else {
        first.max(second)
    }
}

fn axis_layout(
    axes: &[Axis],
    order: &[Index],
    preferred: &[f32],
    measure_limit: f32,
    final_limit: f32,
) -> (f32, Vec<Span>) {
    let mut measured = preferred.to_vec();
    let mut slots = vec![Span::default(); axes.len()];
    if measure_limit.is_finite() && measure_limit >= 0.0 {
        for &index in order {
            let (slot, available) =
                arrange_axis(axes[index], &slots, measure_limit, measured[index]);
            measured[index] = measured[index].min(available);
            slots[index] = slot;
        }
    }
    let desired = desired_axis(axes, &measured);
    let final_limit = if final_limit >= 0.0 && final_limit.is_finite() {
        final_limit
    } else {
        desired
    };
    for &index in order {
        slots[index] = arrange_axis(axes[index], &slots, final_limit, measured[index]).0;
    }
    (desired, slots)
}

fn graph(items: &[RelativePanelChild]) -> Result<(Vec<Axis>, Vec<Axis>, Vec<Index>), LayoutError> {
    let count = items.len();
    let mut names = HashMap::with_capacity(count);
    for (index, item) in items.iter().enumerate() {
        if !item.id.is_empty() && names.insert(item.id.as_str(), index).is_some() {
            return Err((RelativePanelIssue::DuplicateId, item.id.clone()));
        }
    }
    let mut horizontal = Vec::with_capacity(count);
    let mut vertical = Vec::with_capacity(count);
    let mut consumers = vec![Vec::new(); count];
    let mut pending = vec![0; count];
    let mut ready = VecDeque::new();
    let mut dependencies = Vec::new();
    for (index, item) in items.iter().enumerate() {
        horizontal.push(resolve_axis(&item.horizontal, &names, &mut dependencies)?);
        vertical.push(resolve_axis(&item.vertical, &names, &mut dependencies)?);
        pending[index] = dependencies.len();
        if dependencies.is_empty() {
            ready.push_back(index);
        }
        for dependency in dependencies.drain(..) {
            consumers[dependency].push(index);
        }
    }
    let mut order = Vec::with_capacity(count);
    while let Some(index) = ready.pop_front() {
        order.push(index);
        for &consumer in &consumers[index] {
            pending[consumer] -= 1;
            if pending[consumer] == 0 {
                ready.push_back(consumer);
            }
        }
    }
    if let Some(index) = pending.iter().position(|&degree| degree > 0) {
        return Err((
            RelativePanelIssue::CircularDependency,
            items[index].id.clone(),
        ));
    }
    Ok((horizontal, vertical, order))
}

fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn even(value: f32) -> f32 {
    (value / 2.0 + 0.5).floor() * 2.0
}

fn child_span(
    slot: Span,
    preferred: f32,
    start_gap: f32,
    end_gap: f32,
    alignment: RelativePanelAlignment,
    fixed: bool,
) -> Span {
    if slot.size <= 0.0 {
        return Span {
            start: even(slot.start.max(0.0)),
            size: 0.0,
        };
    }
    let available = (slot.size - start_gap - end_gap).max(0.0);
    let size = match (alignment, fixed) {
        (RelativePanelAlignment::Stretch, false) => available,
        (_, true) => preferred.max(0.0),
        _ => preferred.max(0.0).min(available),
    };
    let remaining = available - size;
    let offset = match alignment {
        RelativePanelAlignment::Center | RelativePanelAlignment::Stretch => remaining / 2.0,
        RelativePanelAlignment::Start => 0.0,
        RelativePanelAlignment::End => remaining,
    };
    Span {
        start: even(slot.start.max(0.0) + start_gap + offset),
        size: even(size),
    }
}

fn layout(
    model: ModelRc<RelativePanelChild>,
    measure: ItemsRepeaterSize,
    arrange: ItemsRepeaterSize,
) -> RelativePanelLayout {
    model.model_tracker().track_row_count_changes();
    let items: Vec<_> = model
        .iter()
        .enumerate()
        .map(|(index, item)| {
            model.model_tracker().track_row_data_changes(index);
            item
        })
        .collect();
    let (horizontal, vertical, order) = match graph(&items) {
        Ok(result) => result,
        Err((issue, issue_target)) => {
            return RelativePanelLayout {
                issue,
                issue_target,
                ..Default::default()
            };
        }
    };
    let desired: Vec<_> = items
        .iter()
        .map(|item| {
            if item.collapsed {
                (0.0, 0.0)
            } else {
                (
                    (finite(item.size.width)
                        + finite(item.margin.left)
                        + finite(item.margin.right))
                    .max(0.0),
                    (finite(item.size.height)
                        + finite(item.margin.top)
                        + finite(item.margin.bottom))
                    .max(0.0),
                )
            }
        })
        .collect();
    let (widths, heights): (Vec<_>, Vec<_>) = desired.into_iter().unzip();
    let (width, x) = axis_layout(&horizontal, &order, &widths, measure.width, arrange.width);
    let (height, y) = axis_layout(&vertical, &order, &heights, measure.height, arrange.height);
    let placements: Vec<_> = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let mut placement = ItemsRepeaterPlacement {
                index: index as i32,
                key: item.id.clone(),
                ..Default::default()
            };
            if !item.collapsed {
                let x = child_span(
                    x[index],
                    finite(item.size.width),
                    finite(item.margin.left),
                    finite(item.margin.right),
                    item.horizontal_alignment,
                    item.fixed_width,
                );
                let y = child_span(
                    y[index],
                    finite(item.size.height),
                    finite(item.margin.top),
                    finite(item.margin.bottom),
                    item.vertical_alignment,
                    item.fixed_height,
                );
                placement.x = x.start;
                placement.y = y.start;
                placement.width = x.size;
                placement.height = y.size;
            }
            placement
        })
        .collect();
    RelativePanelLayout {
        geometry: ItemsRepeaterGeometry {
            width: even(width),
            height: even(height),
            placements: ModelRc::new(VecModel::from(placements)),
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, width: f32, height: f32) -> RelativePanelChild {
        RelativePanelChild {
            id: id.into(),
            size: ItemsRepeaterSize { width, height },
            ..Default::default()
        }
    }

    fn arrange(items: &[RelativePanelChild], width: f32, height: f32) -> RelativePanelLayout {
        layout(
            ModelRc::new(VecModel::from(items.to_vec())),
            ItemsRepeaterSize {
                width: 2000.0,
                height: 2000.0,
            },
            ItemsRepeaterSize { width, height },
        )
    }

    #[test]
    fn panel_edge_then_sibling_edge_then_sibling_center_then_adjacency_then_panel_center() {
        let mut items = [
            item("first", 100.0, 100.0),
            item("middle", 100.0, 100.0),
            item("last", 50.0, 100.0),
        ];
        items[0].horizontal.align_start_with_panel = true;
        items[1].horizontal.after = "first".into();
        items[2].horizontal = RelativePanelAxis {
            align_start_with_panel: true,
            align_center_with_panel: true,
            align_start_with: "middle".into(),
            align_center_with: "middle".into(),
            after: "middle".into(),
            ..Default::default()
        };
        for (stage, (desired, x)) in [
            (200.0, 0.0),
            (200.0, 100.0),
            (200.0, 126.0),
            (250.0, 200.0),
            (200.0, 76.0),
        ]
        .into_iter()
        .enumerate()
        {
            let result = arrange(&items, -1.0, -1.0);
            assert_eq!(result.geometry.width, desired, "stage {stage}");
            let slot = result.geometry.placements.row_data(2).unwrap();
            assert_eq!((slot.x, slot.width), (x, 50.0), "stage {stage}");
            match stage {
                0 => items[2].horizontal.align_start_with_panel = false,
                1 => items[2].horizontal.align_start_with = "".into(),
                2 => items[2].horizontal.align_center_with = "".into(),
                3 => items[2].horizontal.after = "".into(),
                _ => {}
            }
        }
    }

    #[test]
    fn desired_size_includes_a_chain_on_the_left_of_a_centered_sibling() {
        let mut items = [
            item("center", 100.0, 100.0),
            item("left", 100.0, 100.0),
            item("below", 300.0, 100.0),
        ];
        items[0].horizontal.align_center_with_panel = true;
        items[1].horizontal.before = "center".into();
        items[2].horizontal.align_start_with = "left".into();
        items[2].vertical.after = "left".into();
        let result = arrange(&items, -1.0, -1.0);
        assert_eq!(
            (result.geometry.width, result.geometry.height),
            (300.0, 200.0)
        );
        let slots: Vec<_> = result
            .geometry
            .placements
            .iter()
            .map(|p| (p.x, p.y, p.width))
            .collect();
        assert_eq!(
            slots,
            [(100.0, 0.0, 100.0), (0.0, 0.0, 100.0), (0.0, 100.0, 300.0)]
        );
    }

    #[test]
    fn invalid_cross_axis_references_are_rejected_even_when_shadowed() {
        let mut items = [item("first", 60.0, 30.0), item("second", 60.0, 30.0)];
        items[0].horizontal.align_start_with_panel = true;
        items[0].horizontal.after = "second".into();
        items[1].vertical.before = "first".into();
        assert!(matches!(
            arrange(&items, 300.0, 100.0).issue,
            RelativePanelIssue::CircularDependency
        ));
        items[0].horizontal.after = "missing".into();
        let result = arrange(&items, 300.0, 100.0);
        assert!(matches!(result.issue, RelativePanelIssue::MissingTarget));
        assert_eq!(result.issue_target.as_str(), "missing");
        assert_eq!(result.geometry.placements.row_count(), 0);
        items[1].id = "first".into();
        assert!(matches!(
            arrange(&items, 300.0, 100.0).issue,
            RelativePanelIssue::DuplicateId
        ));
    }

    #[test]
    fn contradicting_edges_return_an_empty_slot() {
        let mut items = [item("anchor", 100.0, 100.0), item("child", 100.0, 100.0)];
        items[1].horizontal.after = "anchor".into();
        items[1].horizontal.before = "anchor".into();
        items[1].vertical.before = "anchor".into();
        items[1].vertical.after = "anchor".into();
        items[1].fixed_width = true;
        items[1].fixed_height = true;
        let result = arrange(&items, 200.0, 100.0);
        let slot = result.geometry.placements.row_data(1).unwrap();
        assert_eq!((slot.width, slot.height), (0.0, 0.0));
    }
}
