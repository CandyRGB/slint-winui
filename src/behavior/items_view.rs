use std::collections::HashSet;

use slint::{Model, ModelRc, SharedString, VecModel};

use crate::{
    ItemsRepeaterPlacement, ItemsViewBehavior, ItemsViewItem, ItemsViewNavigation,
    ItemsViewSelectionAction, ItemsViewSelectionMode,
};

pub fn install(behavior: ItemsViewBehavior<'_>) {
    behavior.on_keys(|items| {
        items.model_tracker().track_row_count_changes();
        let keys = items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                items.model_tracker().track_row_data_changes(index);
                item.key
            })
            .collect::<Vec<_>>();
        ModelRc::new(VecModel::from(keys))
    });
    behavior.on_index_of(|keys, key| {
        if key.is_empty() {
            return -1;
        }
        keys.model_tracker().track_row_count_changes();
        keys.iter()
            .position(|candidate| candidate == key)
            .map_or(-1, |index| index as i32)
    });
    behavior.on_same_selection(|left, right| left.iter().eq(right.iter()));
    behavior.on_selection(selection);
    behavior.on_target(navigation_target);
}

fn selection(
    items: ModelRc<ItemsViewItem>,
    old: ModelRc<SharedString>,
    mode: ItemsViewSelectionMode,
    action: ItemsViewSelectionAction,
    index: i32,
    anchor: i32,
    ctrl: bool,
    shift: bool,
) -> ModelRc<SharedString> {
    use ItemsViewSelectionAction::*;
    use ItemsViewSelectionMode::*;

    let mut selected: HashSet<_> = old.iter().collect();
    let target = usize::try_from(index).ok().and_then(|i| items.row_data(i));
    if let Some(item) = target.filter(|item| !item.key.is_empty()) {
        match action {
            Select => {
                if mode == Single {
                    selected.clear();
                }
                selected.insert(item.key);
            }
            Deselect => {
                selected.remove(&item.key);
            }
            Interact | Navigate | ListInteract | ListNavigate if item.enabled => {
                let focused = matches!(action, Navigate | ListNavigate);
                let list_view = matches!(action, ListInteract | ListNavigate);
                let was_selected = selected.contains(&item.key);
                if mode == Single && (!focused || !ctrl) {
                    selected.clear();
                    if !ctrl || !was_selected {
                        selected.insert(item.key);
                    }
                } else if (mode == Multiple || mode == Extended) && shift && anchor >= 0 {
                    let mut set_range = true;
                    if mode == Multiple && !list_view {
                        set_range = items
                            .row_data(anchor as usize)
                            .is_some_and(|item| selected.contains(&item.key));
                    } else if mode == Extended && (!ctrl || !focused && !list_view) {
                        selected.clear();
                    }
                    if focused || list_view || mode == Extended || set_range != was_selected {
                        for row in index.min(anchor)..=index.max(anchor) {
                            if let Some(item) =
                                items.row_data(row as usize).filter(|item| item.enabled)
                            {
                                if set_range {
                                    selected.insert(item.key);
                                } else {
                                    selected.remove(&item.key);
                                }
                            }
                        }
                    }
                } else if !focused && (mode == Multiple || mode == Extended && ctrl) {
                    if was_selected {
                        selected.remove(&item.key);
                    } else {
                        selected.insert(item.key);
                    }
                } else if mode == Extended
                    && (!focused && (list_view || !was_selected) || focused && !ctrl && !shift)
                {
                    selected.clear();
                    selected.insert(item.key);
                }
            }
            _ => {}
        }
    }

    let mut keys = items
        .iter()
        .filter_map(|item| {
            let keep = match action {
                All => true,
                Invert => !selected.contains(&item.key),
                _ => selected.contains(&item.key),
            };
            (keep && !item.key.is_empty()).then_some(item.key)
        })
        .collect::<Vec<_>>();
    if mode == Single {
        keys.truncate(1);
    }
    ModelRc::new(VecModel::from(keys))
}

fn navigation_target(
    items: ModelRc<ItemsViewItem>,
    placements: ModelRc<ItemsRepeaterPlacement>,
    current: i32,
    direction: ItemsViewNavigation,
    x: f32,
    y: f32,
) -> i32 {
    use ItemsViewNavigation::*;

    let candidates = || {
        placements.iter().filter(|rect| {
            rect.width > 0.0
                && rect.height > 0.0
                && (direction == ViewportPoint
                    || items
                        .row_data(rect.index as usize)
                        .is_some_and(|item| item.enabled))
        })
    };
    if direction == ViewportPoint {
        let distance = |rect: &ItemsRepeaterPlacement| {
            let dx = x - x.clamp(rect.x, rect.x + rect.width);
            let dy = y - y.clamp(rect.y, rect.y + rect.height);
            dx * dx + dy * dy
        };
        return candidates()
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
            .map_or(-1, |rect| rect.index);
    }
    if direction == Last {
        return candidates().last().map_or(-1, |rect| rect.index);
    }
    let origin = usize::try_from(current)
        .ok()
        .and_then(|index| placements.row_data(index));
    if direction == First || origin.is_none() {
        return candidates().next().map_or(-1, |rect| rect.index);
    }
    let origin = origin.unwrap();
    let vertical = matches!(direction, Up | Down | PageUp | PageDown);
    let sign = if matches!(direction, Up | Left | PageUp) {
        -1.0
    } else {
        1.0
    };
    let axes = |rect: &ItemsRepeaterPlacement| {
        if vertical {
            (rect.y + rect.height / 2.0, rect.x, rect.width)
        } else {
            (rect.x + rect.width / 2.0, rect.y, rect.height)
        }
    };
    let (along, across, span) = axes(&origin);
    let page = if matches!(direction, PageUp | PageDown) {
        y
    } else {
        0.0
    };
    candidates()
        .filter_map(|rect| {
            let (position, cross, breadth) = axes(&rect);
            let delta = (position - along) * sign;
            if delta <= 0.0 {
                return None;
            }
            let overlap = cross < across + span && across < cross + breadth;
            let cross_distance = (cross + breadth / 2.0 - across - span / 2.0).abs();
            Some((!overlap, (delta - page).abs(), cross_distance, rect.index))
        })
        .min_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.total_cmp(&b.1))
                .then(a.2.total_cmp(&b.2))
        })
        .map_or(-1, |score| score.3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_view_plain_extended_click_replaces_existing_selection() {
        let items = ModelRc::new(VecModel::from(
            ["a", "b", "c"]
                .map(|key| ItemsViewItem {
                    key: key.into(),
                    enabled: true,
                })
                .to_vec(),
        ));
        let old = ModelRc::new(VecModel::from(vec!["a".into(), "c".into()]));
        let selected = selection(
            items,
            old,
            ItemsViewSelectionMode::Extended,
            ItemsViewSelectionAction::ListInteract,
            2,
            0,
            false,
            false,
        );
        assert_eq!(selected.row_count(), 1);
        assert_eq!(selected.row_data(0).as_deref(), Some("c"));
    }

    #[test]
    fn multiple_shift_extends_the_anchors_current_state() {
        let items = ModelRc::new(VecModel::from(
            ["a", "b", "c", "d"]
                .into_iter()
                .map(|key| ItemsViewItem {
                    key: key.into(),
                    enabled: key != "b",
                })
                .collect::<Vec<_>>(),
        ));
        let old = ModelRc::new(VecModel::from(vec![SharedString::from("a")]));
        let marked = selection(
            items.clone(),
            old,
            ItemsViewSelectionMode::Multiple,
            ItemsViewSelectionAction::Navigate,
            3,
            0,
            false,
            true,
        );
        assert_eq!(
            marked.iter().map(|key| key.to_string()).collect::<Vec<_>>(),
            ["a", "c", "d"]
        );
        let toggled = selection(
            items.clone(),
            marked,
            ItemsViewSelectionMode::Multiple,
            ItemsViewSelectionAction::Interact,
            0,
            0,
            false,
            false,
        );
        let unmarked = selection(
            items,
            toggled,
            ItemsViewSelectionMode::Multiple,
            ItemsViewSelectionAction::Navigate,
            3,
            0,
            false,
            true,
        );
        assert_eq!(unmarked.row_count(), 0);
    }

    #[test]
    fn spatial_navigation_skips_disabled_items_and_uses_physical_coordinates() {
        let items = ModelRc::new(VecModel::from(
            ["right", "center", "left"]
                .into_iter()
                .map(|key| ItemsViewItem {
                    key: key.into(),
                    enabled: key != "center",
                })
                .collect::<Vec<_>>(),
        ));
        let placements = ModelRc::new(VecModel::from(
            [200.0, 100.0, 0.0]
                .into_iter()
                .enumerate()
                .map(|(index, x)| ItemsRepeaterPlacement {
                    index: index as i32,
                    x,
                    width: 96.0,
                    height: 80.0,
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        ));
        assert_eq!(
            navigation_target(
                items.clone(),
                placements.clone(),
                0,
                ItemsViewNavigation::Left,
                0.0,
                0.0
            ),
            2
        );
        assert_eq!(
            navigation_target(
                items,
                placements,
                -1,
                ItemsViewNavigation::ViewportPoint,
                150.0,
                20.0
            ),
            1
        );
    }
}
