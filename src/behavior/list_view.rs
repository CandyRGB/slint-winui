use std::collections::HashSet;

use slint::{Model, ModelExt, ModelRc, SharedString, VecModel};

use crate::{
    ItemsRepeaterPlacement, ItemsRepeaterSize, ItemsViewItem, ListViewBehavior, ListViewEntry,
    ListViewHeaderPosition,
};

pub fn install(behavior: ListViewBehavior<'_>) {
    behavior.on_source(|entries| {
        ModelRc::new(entries.map(|entry| ItemsViewItem {
            key: entry.key,
            enabled: entry.enabled,
        }))
    });
    behavior.on_sizes(sizes);
    behavior.on_groups(|entries| {
        ModelRc::new(VecModel::from(
            group_starts(&entries)
                .map(|(index, _)| index as i32)
                .collect::<Vec<_>>(),
        ))
    });
    behavior.on_sticky_header(sticky_header);
    behavior.on_insertion(insertion);
    behavior.on_reorder(reorder);
}

fn read_rows(model: &ModelRc<ListViewEntry>) -> impl Iterator<Item = ListViewEntry> + '_ {
    model.model_tracker().track_row_count_changes();
    model.iter().enumerate().map(|(index, row)| {
        model.model_tracker().track_row_data_changes(index);
        row
    })
}

fn group_starts(
    entries: &ModelRc<ListViewEntry>,
) -> impl Iterator<Item = (usize, SharedString)> + '_ {
    let mut previous = None;
    read_rows(entries)
        .enumerate()
        .filter_map(move |(index, row)| {
            let start = previous.as_ref() != Some(&row.group);
            previous = Some(row.group.clone());
            start.then_some((index, row.group))
        })
}

fn sizes(
    entries: ModelRc<ListViewEntry>,
    width: f32,
    row_height: f32,
    header_height: f32,
) -> ModelRc<ItemsRepeaterSize> {
    let mut previous = SharedString::default();
    let sizes = read_rows(&entries)
        .map(|row| {
            let header = if row.group != previous && !row.group.is_empty() {
                header_height
            } else {
                0.0
            };
            previous = row.group;
            let height = if row.height > 0.0 {
                row.height.max(40.0)
            } else {
                row_height
            };
            ItemsRepeaterSize {
                width,
                height: height + header,
            }
        })
        .collect::<Vec<_>>();
    ModelRc::new(VecModel::from(sizes))
}

fn sticky_header(
    placements: ModelRc<ItemsRepeaterPlacement>,
    groups: ModelRc<i32>,
    offset: f32,
    height: f32,
) -> ListViewHeaderPosition {
    let position = |i| {
        groups
            .row_data(i)
            .and_then(|index| placements.row_data(index as usize))
    };
    let next = partition(groups.row_count(), |i| {
        position(i).is_some_and(|rect| rect.y < offset)
    });
    let Some(active) = next.checked_sub(1).and_then(position) else {
        return ListViewHeaderPosition { index: -1, y: 0.0 };
    };
    let y = position(next).map_or(offset, |rect| offset.min(rect.y - height));
    ListViewHeaderPosition {
        index: active.index,
        y,
    }
}

fn insertion(placements: ModelRc<ItemsRepeaterPlacement>, y: f32) -> i32 {
    partition(placements.row_count(), |i| {
        placements
            .row_data(i)
            .is_some_and(|rect| rect.y + rect.height / 2.0 <= y)
    }) as i32
}

fn partition(count: usize, precedes: impl Fn(usize) -> bool) -> usize {
    let (mut first, mut end) = (0, count);
    while first < end {
        let middle = first + (end - first) / 2;
        if precedes(middle) {
            first = middle + 1;
        } else {
            end = middle;
        }
    }
    first
}

fn reorder(
    entries: ModelRc<ListViewEntry>,
    keys: ModelRc<SharedString>,
    boundary: i32,
) -> ModelRc<ListViewEntry> {
    let Ok(mut boundary) = usize::try_from(boundary) else {
        return ModelRc::default();
    };
    let selected = keys.iter().collect::<HashSet<_>>();
    let mut moved = Vec::new();
    let mut remaining = Vec::new();
    if boundary > entries.row_count() || selected.is_empty() {
        return ModelRc::default();
    }
    let insertion = boundary;
    for (index, entry) in entries.iter().enumerate() {
        if !entry.group.is_empty() {
            return ModelRc::default();
        }
        if selected.contains(&entry.key) {
            moved.push(entry);
            if index < insertion {
                boundary -= 1;
            }
        } else {
            remaining.push(entry);
        }
    }
    remaining.splice(boundary..boundary, moved);
    if remaining
        .iter()
        .zip(entries.iter())
        .all(|(left, right)| left.key == right.key)
    {
        return ModelRc::default();
    }
    ModelRc::new(VecModel::from(remaining))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reorder_preserves_relative_order_and_uses_pre_move_boundary() {
        let entries = ModelRc::new(VecModel::from(
            ["a", "b", "c", "d", "e"]
                .map(|key| ListViewEntry {
                    key: key.into(),
                    ..Default::default()
                })
                .to_vec(),
        ));
        let keys = ModelRc::new(VecModel::from(vec!["d".into(), "b".into()]));
        let result = reorder(entries, keys, 5);
        let order = result
            .iter()
            .map(|row| row.key.to_string())
            .collect::<Vec<_>>();
        assert_eq!(order, ["a", "c", "e", "b", "d"]);
    }

    #[test]
    fn upcoming_group_pushes_the_sticky_header_out() {
        let placements = ModelRc::new(VecModel::from(
            [0.0, 90.0]
                .into_iter()
                .enumerate()
                .map(|(index, y)| ItemsRepeaterPlacement {
                    index: index as i32,
                    y,
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        ));
        let groups = ModelRc::new(VecModel::from(vec![0, 1]));
        let pushed = sticky_header(placements, groups, 70.0, 42.0);
        assert_eq!(pushed.index, 0);
        assert_eq!(pushed.y, 48.0);
    }
}
