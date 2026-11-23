use std::fmt::Write as _;

use slint::{Model, ModelRc, SharedString, VecModel, language::Orientation};

use crate::{
    BorderInsets, ItemsRepeaterGeometry, ItemsRepeaterPlacement, StackPanelBehavior,
    StackPanelChild, StackPanelOptions, StackPanelSnapAlignment, StackPanelSnapPoints,
};

const MAX_IRREGULAR_SNAP_POINTS: usize = u8::MAX as usize - 1;

pub fn install(behavior: StackPanelBehavior<'_>) {
    behavior.on_layout(layout);
    behavior.on_snap_points(snap_points);
    behavior.on_snap_key(snap_key);
}

#[derive(Clone, Copy)]
struct Axes(Orientation);

impl Axes {
    fn split<T>(self, x: T, y: T) -> (T, T) {
        match self.0 {
            Orientation::Horizontal => (x, y),
            Orientation::Vertical => (y, x),
        }
    }

    fn margins(self, margin: &BorderInsets) -> (f32, f32) {
        let (near, _) = self.split(margin.left, margin.top);
        let (far, _) = self.split(margin.right, margin.bottom);
        (finite(near), finite(far))
    }

    fn dimensions(self, item: &StackPanelChild) -> (f32, f32) {
        if item.collapsed {
            return (0.0, 0.0);
        }
        self.split(
            finite(item.size.width).max(0.0),
            finite(item.size.height).max(0.0),
        )
    }
}

fn finite(value: f32) -> f32 {
    if value.is_finite() && value != 0.0 {
        value
    } else {
        0.0
    }
}

fn children(model: &ModelRc<StackPanelChild>) -> impl Iterator<Item = StackPanelChild> + '_ {
    model.model_tracker().track_row_count_changes();
    model.iter().enumerate().map(|(index, child)| {
        model.model_tracker().track_row_data_changes(index);
        child
    })
}

fn layout(model: ModelRc<StackPanelChild>, options: StackPanelOptions) -> ItemsRepeaterGeometry {
    let axes = Axes(options.orientation);
    let (available_main, available_cross) =
        axes.split(options.available.width, options.available.height);
    let mut position: f32 = 0.0;
    let mut preferred_cross: f32 = 0.0;
    let mut cross = finite(available_cross).max(0.0);
    let spacing = finite(options.spacing);
    let mut has_visible_child = false;
    let mut slots = Vec::with_capacity(model.row_count());

    for (index, child) in children(&model).enumerate() {
        let mut slot = ItemsRepeaterPlacement {
            index: index as i32,
            ..Default::default()
        };
        if !child.collapsed {
            let (main, child_cross) = axes.dimensions(&child);
            preferred_cross = preferred_cross.max(child_cross);
            // The cross-axis arrange rectangle can grow as later children are encountered.
            cross = cross.max(child_cross);
            if has_visible_child {
                position += spacing;
            }
            (slot.x, slot.y) = axes.split(position, 0.0);
            (slot.width, slot.height) = axes.split(main, cross);
            position += main;
            has_visible_child = true;
        }
        slots.push(slot);
    }
    let (width, height) = axes.split(position.max(0.0), preferred_cross);
    if options.rtl {
        let arranged_width = if options.orientation == Orientation::Horizontal {
            available_main
        } else {
            available_cross
        };
        let mirror = if arranged_width >= 0.0 && arranged_width.is_finite() {
            arranged_width
        } else {
            width
        };
        for slot in &mut slots {
            if slot.width > 0.0 || slot.height > 0.0 {
                slot.x = mirror - slot.x - slot.width;
            }
        }
    }
    ItemsRepeaterGeometry {
        placements: ModelRc::new(VecModel::from(slots)),
        width,
        height,
    }
}

fn snap_points(
    model: ModelRc<StackPanelChild>,
    orientation: Orientation,
    regular: bool,
    margin: BorderInsets,
    alignment: StackPanelSnapAlignment,
) -> StackPanelSnapPoints {
    let axes = Axes(orientation);
    let (near, far) = axes.margins(&margin);
    let mut sizes = children(&model).map(|child| axes.dimensions(&child).0);
    let mut result = StackPanelSnapPoints {
        available: true,
        regular,
        ..Default::default()
    };
    if regular {
        if let Some(first) = sizes.next() {
            result.interval = first;
            result.offset = match alignment {
                StackPanelSnapAlignment::Near => near,
                StackPanelSnapAlignment::Center => near + first / 2.0,
                StackPanelSnapAlignment::Far => far,
            };
        }
        return result;
    }

    let mut preceding = 0.0;
    let positions: Vec<_> = sizes
        .take(MAX_IRREGULAR_SNAP_POINTS)
        .enumerate()
        .map(|(index, size)| {
            let point = match alignment {
                StackPanelSnapAlignment::Near if index == 0 => 0.0,
                StackPanelSnapAlignment::Near => preceding + near,
                StackPanelSnapAlignment::Center => preceding + size / 2.0 + near,
                StackPanelSnapAlignment::Far => preceding + size + near,
            };
            preceding += size;
            point
        })
        .collect();
    result.positions = ModelRc::new(VecModel::from(positions));
    result
}

fn snap_key(
    model: ModelRc<StackPanelChild>,
    orientation: Orientation,
    regular: bool,
    margin: BorderInsets,
) -> SharedString {
    let axes = Axes(orientation);
    let (near, far) = axes.margins(&margin);
    let mut key = format!("{orientation:?}:{regular}:{near}:{far}");
    let count = if regular {
        1
    } else {
        MAX_IRREGULAR_SNAP_POINTS
    };
    for child in children(&model).take(count) {
        let _ = write!(key, ":{}", axes.dimensions(&child).0);
    }
    key.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ItemsRepeaterSize;
    use Orientation::{Horizontal, Vertical};
    use StackPanelSnapAlignment::{Center, Far, Near};

    fn child(width: f32, height: f32, collapsed: bool) -> StackPanelChild {
        StackPanelChild {
            size: ItemsRepeaterSize { width, height },
            collapsed,
        }
    }

    fn model(children: Vec<StackPanelChild>) -> ModelRc<StackPanelChild> {
        ModelRc::new(VecModel::from(children))
    }

    #[test]
    fn folded_children_skip_spacing_and_cross_space_can_grow_as_items_arrange() {
        let items = model(vec![
            child(40.0, 42.0, false),
            child(120.0, 60.0, true),
            child(74.0, 52.0, false),
            child(36.0, 48.0, false),
        ]);
        let geometry = layout(
            items,
            StackPanelOptions {
                orientation: Vertical,
                available: ItemsRepeaterSize {
                    width: 64.0,
                    height: 80.0,
                },
                spacing: 8.0,
                rtl: true,
            },
        );
        assert_eq!((geometry.width, geometry.height), (74.0, 158.0));
        let slots: Vec<_> = geometry
            .placements
            .iter()
            .map(|p| (p.index, p.x, p.y, p.width, p.height))
            .collect();
        assert_eq!(
            slots,
            [
                (0, 0.0, 0.0, 64.0, 42.0),
                (1, 0.0, 0.0, 0.0, 0.0),
                (2, -10.0, 50.0, 74.0, 52.0),
                (3, -10.0, 110.0, 74.0, 48.0),
            ]
        );
    }

    #[test]
    fn irregular_points_keep_collapsed_indices_and_ignore_spacing() {
        let items = model(vec![
            child(40.0, 56.0, false),
            child(40.0, 60.0, true),
            child(40.0, 48.0, false),
        ]);
        let margin = BorderInsets {
            top: 8.0,
            bottom: 12.0,
            ..Default::default()
        };
        for (alignment, expected) in [
            (Near, [0.0, 64.0, 64.0]),
            (Center, [36.0, 64.0, 88.0]),
            (Far, [64.0, 64.0, 112.0]),
        ] {
            let points = snap_points(items.clone(), Vertical, false, margin.clone(), alignment);
            assert_eq!(points.positions.iter().collect::<Vec<_>>(), expected);
        }
    }

    #[test]
    fn regular_points_use_first_child_and_the_matching_panel_margin() {
        let items = model(vec![child(44.0, 56.0, false), child(52.0, 60.0, false)]);
        let margin = BorderInsets {
            left: 4.0,
            right: 10.0,
            ..Default::default()
        };
        for (alignment, expected) in [(Near, 4.0), (Center, 26.0), (Far, 10.0)] {
            let points = snap_points(items.clone(), Horizontal, true, margin.clone(), alignment);
            assert!(points.regular);
            assert_eq!((points.offset, points.interval), (expected, 44.0));
        }
        let collapsed_first = model(vec![child(44.0, 56.0, true), child(52.0, 60.0, false)]);
        assert_eq!(
            snap_points(collapsed_first, Horizontal, true, margin, Near).interval,
            0.0
        );
    }

    #[test]
    fn irregular_native_limit_preserves_the_first_254_entries() {
        let items = model(vec![child(40.0, 40.0, false); 256]);
        let points = snap_points(items, Vertical, false, BorderInsets::default(), Near);
        assert_eq!(points.positions.row_count(), 254);
        assert_eq!(points.positions.row_data(253), Some(10_120.0));
    }
}
