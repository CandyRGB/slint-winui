use slint::{Model, ModelRc, VecModel, language::Orientation};

use crate::{
    ItemsRepeaterGeometry, ItemsRepeaterPlacement, VariableSizedWrapGridOptions, WrapGridAlignment,
    WrapLayoutBehavior, WrapLayoutChild, WrapPanelItemsStretch, WrapPanelOptions,
};

pub fn install(behavior: WrapLayoutBehavior<'_>) {
    behavior.on_wrap_panel(wrap_panel);
    behavior.on_variable_grid(variable_grid);
}

#[derive(Clone, Copy)]
struct Axes {
    horizontal: bool,
}

impl Axes {
    fn split<T>(self, x: T, y: T) -> (T, T) {
        if self.horizontal { (x, y) } else { (y, x) }
    }

    fn placement(
        self,
        index: usize,
        along: f32,
        across: f32,
        length: f32,
        breadth: f32,
    ) -> ItemsRepeaterPlacement {
        let (x, y) = self.split(along, across);
        let (width, height) = self.split(length, breadth);
        ItemsRepeaterPlacement {
            index: index as i32,
            x,
            y,
            width,
            height,
            ..Default::default()
        }
    }
}

fn children(model: ModelRc<WrapLayoutChild>) -> Vec<WrapLayoutChild> {
    model.model_tracker().track_row_count_changes();
    model
        .iter()
        .enumerate()
        .map(|(index, item)| {
            model.model_tracker().track_row_data_changes(index);
            item
        })
        .collect()
}

fn empty_placements(count: usize) -> Vec<ItemsRepeaterPlacement> {
    (0..count)
        .map(|index| ItemsRepeaterPlacement {
            index: index as i32,
            ..Default::default()
        })
        .collect()
}

fn geometry(
    mut placements: Vec<ItemsRepeaterPlacement>,
    width: f32,
    height: f32,
    mirror: Option<f32>,
) -> ItemsRepeaterGeometry {
    if let Some(origin) = mirror {
        for item in &mut placements {
            if item.width > 0.0 || item.height > 0.0 {
                item.x = origin - item.x - item.width;
            }
        }
    }
    ItemsRepeaterGeometry {
        placements: ModelRc::new(VecModel::from(placements)),
        width,
        height,
    }
}

fn limit(size: f32) -> Option<f32> {
    (size.is_finite() && size >= 0.0).then_some(size)
}

fn length(size: f32) -> f32 {
    limit(size).unwrap_or(0.0)
}

fn finite(size: f32) -> f32 {
    if size.is_finite() { size } else { 0.0 }
}

fn even(size: f32) -> f32 {
    (size / 2.0 + 0.5).floor() * 2.0
}

fn wrap_panel(model: ModelRc<WrapLayoutChild>, options: WrapPanelOptions) -> ItemsRepeaterGeometry {
    let items = children(model);
    let mut placements = empty_placements(items.len());
    let axes = Axes {
        horizontal: options.orientation == Orientation::Horizontal,
    };
    let (available, _) = axes.split(
        limit(options.available.width),
        limit(options.available.height),
    );
    let (leading, start) = axes.split(finite(options.padding.left), finite(options.padding.top));
    let (trailing, end) = axes.split(
        finite(options.padding.right),
        finite(options.padding.bottom),
    );
    let item_gap = finite(options.item_spacing);
    let line_gap = finite(options.line_spacing);
    let mut along = leading;
    let mut across = start;
    let mut row_height: f32 = 0.0;
    let mut used_along: f32 = 0.0;
    let mut row = Vec::new();

    let place_row = |placements: &mut [ItemsRepeaterPlacement], row: &[(usize, f32, f32)], y, h| {
        for &(index, x, w) in row {
            placements[index] = axes.placement(index, x, y, w, h);
        }
    };
    for (index, item) in items.iter().enumerate().filter(|(_, item)| !item.collapsed) {
        let (mut item_length, breadth) =
            axes.split(length(item.size.width), length(item.size.height));
        if available.is_some_and(|available| along + item_length + trailing > available) {
            place_row(&mut placements, &row, across, row_height);
            // WinUI also accounts for the leading empty line if the first child is oversized.
            across += row_height + line_gap;
            row_height = 0.0;
            along = leading;
            row.clear();
        }
        if index + 1 == items.len() && options.stretch == WrapPanelItemsStretch::Last {
            if let Some(available) = available {
                item_length = (available - along).max(0.0);
            }
        }
        row.push((index, along, item_length));
        along += item_length + item_gap;
        used_along = used_along.max(along);
        row_height = row_height.max(breadth);
    }
    if row.is_empty() {
        let (width, height) = axes.split((leading + trailing).max(0.0), (start + end).max(0.0));
        return geometry(placements, width, height, None);
    }
    place_row(&mut placements, &row, across, row_height);
    let (width, height) = axes.split(
        (used_along + trailing).max(0.0),
        (across + row_height + end).max(0.0),
    );
    let mirror = options
        .rtl
        .then(|| limit(options.available.width).unwrap_or(width));
    geometry(placements, width, height, mirror)
}

#[derive(Clone, Copy)]
struct GridSlot {
    lane: usize,
    line: usize,
    lanes: usize,
    lines: usize,
}

fn spans(child: &WrapLayoutChild, axes: Axes) -> (usize, usize) {
    axes.split(
        child.column_span.max(1) as usize,
        child.row_span.max(1) as usize,
    )
}

fn cell(fixed: f32, desired: f32) -> f32 {
    if let Some(fixed) = limit(fixed).filter(|size| *size > 0.0) {
        fixed
    } else {
        even(length(desired)).max(2.0)
    }
}

fn track_limit(available: Option<f32>, cell: f32, tiles: usize) -> usize {
    available
        .map_or(tiles, |available| (available / cell).floor() as usize)
        .clamp(1, tiles.max(1))
}

fn place_grid(
    items: &[WrapLayoutChild],
    axes: Axes,
    lane_limit: usize,
    line_limit: usize,
) -> Vec<GridSlot> {
    let mut slots = Vec::with_capacity(items.len());
    let mut occupied: Vec<GridSlot> = Vec::new();
    let (mut lane, mut line) = (0, 0);
    for item in items {
        let (span_lanes, span_lines) = spans(item, axes);
        let lanes = span_lanes.min(lane_limit);
        let mut starting_lane = lane;
        loop {
            if line >= line_limit {
                return slots;
            }
            if lane > lane_limit - lanes {
                // Once a full line was searched, an earlier retry cannot succeed before a tile ends.
                line = if starting_lane == 0 {
                    occupied
                        .iter()
                        .map(|slot| slot.line + slot.lines)
                        .min()
                        .unwrap_or(line + 1)
                } else {
                    line + 1
                };
                occupied.retain(|slot| slot.line + slot.lines > line);
                lane = 0;
                starting_lane = 0;
                continue;
            }
            if let Some(blocked_until) = occupied
                .iter()
                .filter(|slot| lane < slot.lane + slot.lanes && slot.lane < lane + lanes)
                .map(|slot| slot.lane + slot.lanes)
                .max()
            {
                lane = blocked_until;
                continue;
            }
            let slot = GridSlot {
                lane,
                line,
                lanes,
                lines: span_lines.min(line_limit - line),
            };
            slots.push(slot);
            occupied.push(slot);
            lane += lanes;
            if lane >= lane_limit {
                lane = 0;
                line += 1;
                occupied.retain(|slot| slot.line + slot.lines > line);
            }
            break;
        }
    }
    slots
}

fn align(
    alignment: WrapGridAlignment,
    available: Option<f32>,
    used: f32,
    tracks: usize,
) -> (f32, f32) {
    let spare = available.map_or(0.0, |space| (space - used).max(0.0));
    match alignment {
        WrapGridAlignment::Start => (0.0, 0.0),
        WrapGridAlignment::Center => (spare / 2.0, 0.0),
        WrapGridAlignment::End => (spare, 0.0),
        WrapGridAlignment::Stretch => (0.0, spare / (tracks + 1) as f32),
    }
}

fn variable_grid(
    model: ModelRc<WrapLayoutChild>,
    options: VariableSizedWrapGridOptions,
) -> ItemsRepeaterGeometry {
    let items = children(model);
    let Some(first) = items.first() else {
        return ItemsRepeaterGeometry::default();
    };
    let axes = Axes {
        horizontal: options.orientation == Orientation::Horizontal,
    };
    let (available_lanes, available_lines) = axes.split(
        limit(options.available.width),
        limit(options.available.height),
    );
    let desired = if first.collapsed {
        Default::default()
    } else {
        first.size.clone()
    };
    let (lane_size, line_size) = axes.split(
        cell(options.cell.width, desired.width),
        cell(options.cell.height, desired.height),
    );
    let (total_lanes, total_lines) =
        items
            .iter()
            .fold((0_usize, 0_usize), |(lanes, lines), item| {
                let (a, b) = spans(item, axes);
                (lanes.saturating_add(a), lines.saturating_add(b))
            });
    let mut lane_limit = track_limit(available_lanes, lane_size, total_lanes);
    if options.maximum_rows_or_columns > 0 {
        lane_limit = lane_limit.min(options.maximum_rows_or_columns as usize);
    }
    let line_limit = track_limit(available_lines, line_size, total_lines);
    let slots = place_grid(&items, axes, lane_limit, line_limit);
    let used_lanes = slots
        .iter()
        .map(|slot| slot.lane + slot.lanes)
        .max()
        .unwrap_or(0);
    let used_lines = slots
        .iter()
        .map(|slot| slot.line + slot.lines)
        .max()
        .unwrap_or(0);
    let lane_extent = used_lanes as f32 * lane_size;
    let line_extent = used_lines as f32 * line_size;
    let (cell_width, cell_height) = axes.split(lane_size, line_size);
    // The native VariableSizedWrapGrid calculates alignment with the physical axes
    // and assigns justification offsets from the source sequence during arrange.
    let (left, x_gap) = align(
        options.horizontal_alignment,
        limit(options.available.width),
        cell_width * used_lines as f32,
        used_lines,
    );
    let (top, y_gap) = align(
        options.vertical_alignment,
        limit(options.available.height),
        cell_height * used_lanes as f32,
        used_lanes,
    );
    let (lane_start, line_start) = axes.split(left, top);
    let (lane_gap, line_gap) = axes.split(x_gap, y_gap);
    let mut placements = empty_placements(items.len());
    for (index, slot) in slots.into_iter().enumerate() {
        let (requested_lanes, _) = spans(&items[index], axes);
        let mut length = slot.lanes as f32 * lane_size;
        if slot.lane == 0 && requested_lanes > slot.lanes {
            let desired = requested_lanes as f32 * lane_size;
            length = length.max(available_lanes.map_or(desired, |space| desired.min(space)));
        }
        placements[index] = axes.placement(
            index,
            slot.lane as f32 * lane_size
                + even(lane_start + (index % used_lanes + 1) as f32 * lane_gap),
            slot.line as f32 * line_size
                + even(line_start + (index / used_lanes + 1) as f32 * line_gap),
            length,
            slot.lines as f32 * line_size,
        );
    }
    let (width, height) = axes.split(
        lane_extent + even(2.0 * lane_start + (used_lanes + 1) as f32 * lane_gap),
        line_extent + even(2.0 * line_start + (used_lines + 1) as f32 * line_gap),
    );
    let mirror = options
        .rtl
        .then(|| limit(options.available.width).unwrap_or(width));
    geometry(placements, width, height, mirror)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ItemsRepeaterSize, WrapPanelPadding};

    fn child(width: f32, height: f32, row_span: i32, column_span: i32) -> WrapLayoutChild {
        WrapLayoutChild {
            size: ItemsRepeaterSize { width, height },
            row_span,
            column_span,
            collapsed: false,
        }
    }

    fn rectangles(geometry: &ItemsRepeaterGeometry) -> Vec<(f32, f32, f32, f32)> {
        geometry
            .placements
            .iter()
            .map(|slot| (slot.x, slot.y, slot.width, slot.height))
            .collect()
    }

    #[test]
    fn stretch_uses_the_last_source_item_and_each_row_shares_its_cross_size() {
        let options = WrapPanelOptions {
            available: ItemsRepeaterSize {
                width: 100.0,
                height: 140.0,
            },
            orientation: Orientation::Horizontal,
            padding: WrapPanelPadding {
                left: 6.0,
                top: 8.0,
                right: 10.0,
                bottom: 12.0,
            },
            item_spacing: 4.0,
            line_spacing: 6.0,
            stretch: WrapPanelItemsStretch::Last,
            ..Default::default()
        };
        let items = vec![
            child(36.0, 20.0, 1, 1),
            child(42.0, 34.0, 1, 1),
            child(20.0, 24.0, 1, 1),
            child(24.0, 16.0, 1, 1),
        ];
        let model = ModelRc::new(VecModel::from(items.clone()));
        assert_eq!(
            rectangles(&wrap_panel(model, options.clone())),
            [
                (6.0, 8.0, 36.0, 34.0),
                (46.0, 8.0, 42.0, 34.0),
                (6.0, 48.0, 20.0, 24.0),
                (30.0, 48.0, 70.0, 24.0),
            ]
        );

        let mut items = items;
        items[3].collapsed = true;
        let model = ModelRc::new(VecModel::from(items));
        assert_eq!(
            rectangles(&wrap_panel(model, options)),
            [
                (6.0, 8.0, 36.0, 34.0),
                (46.0, 8.0, 42.0, 34.0),
                (6.0, 48.0, 20.0, 24.0),
                (0.0, 0.0, 0.0, 0.0),
            ]
        );
    }

    #[test]
    fn spans_follow_gallery_order_in_both_orientations() {
        let model = ModelRc::new(VecModel::from(vec![
            child(44.0, 44.0, 1, 1),
            child(44.0, 80.0, 2, 1),
            child(80.0, 44.0, 1, 2),
            child(80.0, 80.0, 2, 2),
        ]));
        let mut options = VariableSizedWrapGridOptions {
            available: ItemsRepeaterSize {
                width: 400.0,
                height: -1.0,
            },
            orientation: Orientation::Vertical,
            cell: ItemsRepeaterSize {
                width: 44.0,
                height: 44.0,
            },
            maximum_rows_or_columns: 3,
            ..Default::default()
        };
        assert_eq!(
            rectangles(&variable_grid(model.clone(), options.clone())),
            [
                (0.0, 0.0, 44.0, 44.0),
                (0.0, 44.0, 44.0, 88.0),
                (44.0, 0.0, 88.0, 44.0),
                (44.0, 44.0, 88.0, 88.0),
            ]
        );
        options.orientation = Orientation::Horizontal;
        assert_eq!(
            rectangles(&variable_grid(model, options)),
            [
                (0.0, 0.0, 44.0, 44.0),
                (44.0, 0.0, 44.0, 88.0),
                (0.0, 88.0, 88.0, 44.0),
                (0.0, 132.0, 88.0, 88.0),
            ]
        );
    }

    #[test]
    fn spans_clip_at_the_growth_limit_without_reusing_hidden_source_indices() {
        let model = ModelRc::new(VecModel::from(vec![
            child(24.0, 20.0, 0, 1),
            child(24.0, 20.0, 5, 3),
            child(24.0, 20.0, 1, 1),
        ]));
        let options = VariableSizedWrapGridOptions {
            available: ItemsRepeaterSize {
                width: 72.0,
                height: 60.0,
            },
            orientation: Orientation::Horizontal,
            cell: ItemsRepeaterSize {
                width: -1.0,
                height: -1.0,
            },
            maximum_rows_or_columns: 2,
            ..Default::default()
        };
        let geometry = variable_grid(model, options);
        assert_eq!(
            rectangles(&geometry),
            [
                (0.0, 0.0, 24.0, 20.0),
                (0.0, 20.0, 72.0, 40.0),
                (0.0, 0.0, 0.0, 0.0),
            ]
        );
        assert_eq!(geometry.placements.row_data(2).unwrap().index, 2);
    }
}
