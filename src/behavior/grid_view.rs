use std::collections::HashSet;

use slint::{Model, ModelRc, SharedString, VecModel};

use crate::{GridViewBehavior, GridViewItem, GridViewSelectionMode};

pub fn install(behavior: GridViewBehavior<'_>) {
    behavior.on_index_of(|items, id| {
        items
            .iter()
            .position(|item| item.id == id)
            .map_or(-1, |index| index as i32)
    });
    behavior.on_adjacent(adjacent);
    behavior.on_range(|items, selected, start, end| {
        let mut ids: Vec<_> = selected.iter().collect();
        let mut seen: HashSet<_> = ids.iter().cloned().collect();
        let first = start.min(end).max(0) as usize;
        let last = start.max(end).min(items.row_count() as i32 - 1);
        if last >= 0 {
            for index in first..=last as usize {
                if let Some(item) = items.row_data(index) {
                    if !item.disabled && seen.insert(item.id.clone()) {
                        ids.push(item.id);
                    }
                }
            }
        }
        replace_selection(selected, ids)
    });
    behavior.on_reconcile(|items, selected, mode| {
        if selected.row_count() == 0 {
            return selected;
        }
        if mode == GridViewSelectionMode::None {
            return ModelRc::default();
        }
        let limit = if mode == GridViewSelectionMode::Single {
            1
        } else {
            selected.row_count()
        };
        let mut remaining: HashSet<_> = items.iter().map(|item| item.id).collect();
        let ids = selected
            .iter()
            .filter(|id| remaining.remove(id))
            .take(limit)
            .collect();
        replace_selection(selected, ids)
    });
    behavior.on_mark(|selected, id, marked| {
        let exists = selected.iter().any(|existing| existing == id);
        if exists == marked {
            return selected;
        }
        let mut ids: Vec<_> = selected.iter().filter(|existing| *existing != id).collect();
        if marked {
            ids.push(id);
        }
        ModelRc::new(VecModel::from(ids))
    });
    behavior.on_reorder(reorder);
    behavior.set_ready(true);
}

fn adjacent(items: ModelRc<GridViewItem>, origin: i32, lanes: i32, cross: i32, main: i32) -> i32 {
    let count = items.row_count() as i32;
    let available = |index: i32| {
        items
            .row_data(index as usize)
            .is_some_and(|item| !item.disabled)
    };
    if !(0..count).contains(&origin) {
        return if origin >= count {
            (0..count).rev().find(|&index| available(index))
        } else {
            (0..count).find(|&index| available(index))
        }
        .unwrap_or(-1);
    }

    let lanes = lanes.max(1);
    let line = origin / lanes;
    let lane = origin % lanes;
    if cross != 0 {
        let step = cross.signum();
        let mut candidate = origin + step;
        while (0..count).contains(&candidate) && candidate / lanes == line {
            if available(candidate) {
                return candidate;
            }
            candidate += step;
        }
        return -1;
    }
    let direction = main.signum();
    let last = (count - 1) / lanes;
    let mut destination = (line + main).clamp(0, last);
    if destination == line {
        return -1;
    }
    while (0..=last).contains(&destination) {
        let start = destination * lanes;
        let end = (start + lanes).min(count);
        if let Some(index) = (start..end)
            .filter(|&index| available(index))
            .min_by_key(|&index| (index % lanes - lane).abs())
        {
            return index;
        }
        destination += direction;
    }
    -1
}

fn replace_selection(
    current: ModelRc<SharedString>,
    ids: Vec<SharedString>,
) -> ModelRc<SharedString> {
    if current.iter().eq(ids.iter().cloned()) {
        current
    } else {
        ModelRc::new(VecModel::from(ids))
    }
}

fn reorder(
    items: ModelRc<GridViewItem>,
    ids: ModelRc<SharedString>,
    before: i32,
) -> ModelRc<GridViewItem> {
    let ids: HashSet<_> = ids.iter().collect();
    let before = before.max(0) as usize;
    let mut moved = Vec::new();
    let mut remaining = Vec::new();
    let mut insert_at = 0;
    for (index, item) in items.iter().enumerate() {
        if ids.contains(&item.id) {
            moved.push(item);
        } else {
            if index < before {
                insert_at += 1;
            }
            remaining.push(item);
        }
    }
    remaining.splice(insert_at..insert_at, moved);
    if items
        .iter()
        .map(|item| item.id)
        .eq(remaining.iter().map(|item| item.id.clone()))
    {
        items
    } else {
        ModelRc::new(VecModel::from(remaining))
    }
}
