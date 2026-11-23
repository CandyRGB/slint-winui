use std::collections::{HashMap, HashSet};

use slint::{Model, ModelExt, ModelRc, SharedString, VecModel};

use crate::{
    TreeViewBehavior, TreeViewCheckState, TreeViewDropMode, TreeViewItem, TreeViewRow,
    TreeViewSelectionMode,
};

pub fn install(behavior: TreeViewBehavior<'_>) {
    behavior.on_layout(layout);
    behavior.on_window(|rows, first, end| {
        let range = first.max(0) as usize..(end.max(0) as usize).min(rows.row_count());
        model(
            range
                .filter_map(|index| rows.row_data_tracked(index))
                .collect(),
        )
    });
    behavior.on_find(|rows, id| {
        rows.iter()
            .position(|row| row.item.id == id && !row.item.disabled)
            .map_or(-1, |index| index as i32)
    });
    behavior.on_next(|rows, from, forward| {
        let len = rows.row_count() as i32;
        let candidate = if forward {
            (from.saturating_add(1).max(0)..len).find(|&i| {
                rows.row_data(i as usize)
                    .is_some_and(|row| !row.item.disabled)
            })
        } else {
            (0..from.max(0).min(len)).rev().find(|&i| {
                rows.row_data(i as usize)
                    .is_some_and(|row| !row.item.disabled)
            })
        };
        candidate.unwrap_or(-1)
    });
    behavior.on_set_expanded(|ids, id, expanded| {
        let mut ids = tracked(&ids);
        ids.retain(|other| *other != id);
        if expanded {
            ids.push(id);
        }
        model(ids)
    });
    behavior.on_set_selected(set_selected);
    behavior.on_reconcile_selection(|items, selected| {
        Tree::new(&items).checked_ids(&tracked(&selected).into_iter().collect())
    });
    behavior.on_can_move(|items, moving, target, mode| {
        let tree = Tree::new(&items);
        tree.move_plan(&moving, &target, mode).is_some()
    });
    behavior.on_move_items(move_items);
    behavior.set_ready(true);
}

fn model<T: Clone + 'static>(values: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(values))
}

fn tracked<T: Clone + 'static>(model: &ModelRc<T>) -> Vec<T> {
    model.model_tracker().track_row_count_changes();
    (0..model.row_count())
        .filter_map(|index| model.row_data_tracked(index))
        .collect()
}

struct Tree {
    items: Vec<TreeViewItem>,
    by_id: HashMap<SharedString, usize>,
    roots: Vec<usize>,
    children: Vec<Vec<usize>>,
    order: Vec<usize>,
}

struct MovePlan {
    moving: Vec<usize>,
    target: usize,
    parent: SharedString,
}

impl Tree {
    fn new(model: &ModelRc<TreeViewItem>) -> Self {
        let mut items = Vec::new();
        let mut by_id = HashMap::new();
        for item in tracked(model) {
            if !item.id.is_empty() && !by_id.contains_key(&item.id) {
                by_id.insert(item.id.clone(), items.len());
                items.push(item);
            }
        }
        let mut roots = Vec::new();
        let mut children = vec![Vec::new(); items.len()];
        for (index, item) in items.iter().enumerate() {
            if item.parent_id.is_empty() {
                roots.push(index);
            } else if let Some(&parent) = by_id.get(&item.parent_id) {
                children[parent].push(index);
            }
        }

        let mut order = Vec::new();
        let mut pending = roots.clone();
        pending.reverse();
        while let Some(index) = pending.pop() {
            order.push(index);
            pending.extend(children[index].iter().rev());
        }
        Self {
            items,
            by_id,
            roots,
            children,
            order,
        }
    }

    fn selection(&self, selected: &HashSet<SharedString>) -> Vec<TreeViewCheckState> {
        use TreeViewCheckState::*;
        let mut states = vec![Unchecked; self.items.len()];
        for &index in self.order.iter().rev() {
            let item = &self.items[index];
            if item.disabled {
                continue;
            }
            let mut children = self.children[index]
                .iter()
                .filter(|&&child| !self.items[child].disabled)
                .map(|&child| states[child])
                .peekable();
            // An unrealized branch retains its own selection intent until loaded.
            if children.peek().is_none() || item.has_unrealized_children {
                if selected.contains(&item.id) {
                    states[index] = Checked;
                } else if children.any(|state| state != Unchecked) {
                    states[index] = Partial;
                }
            } else {
                let (checked, unchecked) = children.fold((false, false), |(yes, no), state| {
                    (yes || state != Unchecked, no || state != Checked)
                });
                states[index] = match (checked, unchecked) {
                    (true, true) => Partial,
                    (true, false) => Checked,
                    _ => Unchecked,
                };
            }
        }
        states
    }

    fn checked_ids(&self, selected: &HashSet<SharedString>) -> ModelRc<SharedString> {
        let states = self.selection(selected);
        model(
            self.order
                .iter()
                .filter(|&&index| states[index] == TreeViewCheckState::Checked)
                .map(|&index| self.items[index].id.clone())
                .collect(),
        )
    }

    fn ancestors(&self, mut index: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::from_fn(move || {
            index = *self.by_id.get(&self.items[index].parent_id)?;
            Some(index)
        })
        .take(self.items.len())
    }

    fn move_plan(
        &self,
        moving: &ModelRc<SharedString>,
        target: &SharedString,
        mode: TreeViewDropMode,
    ) -> Option<MovePlan> {
        let target = *self.by_id.get(target)?;
        if self.items[target].disabled || !self.order.contains(&target) {
            return None;
        }
        if mode == TreeViewDropMode::Inside
            && !self.items[target].can_have_children
            && self.children[target].is_empty()
            && !self.items[target].has_unrealized_children
        {
            return None;
        }
        let requested: HashSet<_> = moving
            .iter()
            .filter_map(|id| self.by_id.get(&id).copied())
            .collect();
        let moving: Vec<_> = self
            .order
            .iter()
            .copied()
            .filter(|&index| {
                !self.items[index].disabled
                    && requested.contains(&index)
                    && !self
                        .ancestors(index)
                        .any(|parent| requested.contains(&parent))
            })
            .collect();
        if moving.is_empty()
            || moving.contains(&target)
            || self.ancestors(target).any(|index| moving.contains(&index))
        {
            return None;
        }
        let parent = if mode == TreeViewDropMode::Inside {
            self.items[target].id.clone()
        } else {
            self.items[target].parent_id.clone()
        };
        Some(MovePlan {
            moving,
            target,
            parent,
        })
    }
}

fn layout(
    items: ModelRc<TreeViewItem>,
    expanded: ModelRc<SharedString>,
    selected: ModelRc<SharedString>,
    mode: TreeViewSelectionMode,
) -> ModelRc<TreeViewRow> {
    let tree = Tree::new(&items);
    let expanded: HashSet<_> = tracked(&expanded).into_iter().collect();
    let selected = tracked(&selected);
    let states = tree.selection(&selected.iter().cloned().collect());
    let mut rows = Vec::new();
    let mut pending: Vec<_> = tree
        .roots
        .iter()
        .enumerate()
        .rev()
        .map(|(position, _)| (tree.roots.as_slice(), position, 0))
        .collect();
    while let Some((group, position, depth)) = pending.pop() {
        let node = group[position];
        let item = &tree.items[node];
        let children = &tree.children[node];
        let expandable = !children.is_empty() || item.has_unrealized_children;
        let is_expanded = expandable && expanded.contains(&item.id);
        let check = match mode {
            TreeViewSelectionMode::Multiple => states[node],
            TreeViewSelectionMode::Single
                if !item.disabled && selected.first() == Some(&item.id) =>
            {
                TreeViewCheckState::Checked
            }
            _ => TreeViewCheckState::Unchecked,
        };
        rows.push(TreeViewRow {
            item: item.clone(),
            depth,
            index: rows.len() as i32,
            expandable,
            expanded: is_expanded,
            check,
            previous_sibling: position
                .checked_sub(1)
                .map_or_else(SharedString::default, |i| tree.items[group[i]].id.clone()),
            next_sibling: group
                .get(position + 1)
                .map_or_else(SharedString::default, |&i| tree.items[i].id.clone()),
            first_child: children
                .iter()
                .find(|&&i| !tree.items[i].disabled)
                .map_or_else(SharedString::default, |&i| tree.items[i].id.clone()),
        });
        if is_expanded {
            pending.extend(
                children
                    .iter()
                    .enumerate()
                    .rev()
                    .map(|(i, _)| (children.as_slice(), i, depth + 1)),
            );
        }
    }
    model(rows)
}

fn set_selected(
    items: ModelRc<TreeViewItem>,
    current: ModelRc<SharedString>,
    id: SharedString,
    checked: bool,
    mode: TreeViewSelectionMode,
) -> ModelRc<SharedString> {
    if mode == TreeViewSelectionMode::None {
        return current;
    }
    let tree = Tree::new(&items);
    let Some(&index) = tree.by_id.get(&id) else {
        return current;
    };
    if tree.items[index].disabled || !tree.order.contains(&index) {
        return current;
    }
    if mode == TreeViewSelectionMode::Single {
        return if checked {
            model(vec![id])
        } else if current.row_data(0).as_ref() == Some(&id) {
            ModelRc::default()
        } else {
            current
        };
    }
    let mut selected: HashSet<_> = tracked(&current).into_iter().collect();
    let mut pending = vec![index];
    while let Some(index) = pending.pop() {
        if tree.items[index].disabled {
            continue;
        }
        let id = &tree.items[index].id;
        if checked {
            selected.insert(id.clone());
        } else {
            selected.remove(id);
        }
        pending.extend(&tree.children[index]);
    }
    if !checked {
        for parent in tree.ancestors(index) {
            selected.remove(&tree.items[parent].id);
        }
    }
    tree.checked_ids(&selected)
}

fn move_items(
    items: ModelRc<TreeViewItem>,
    moving: ModelRc<SharedString>,
    target: SharedString,
    mode: TreeViewDropMode,
) -> ModelRc<TreeViewItem> {
    let tree = Tree::new(&items);
    let Some(plan) = tree.move_plan(&moving, &target, mode) else {
        return items;
    };
    let mut result = tracked(&items);
    let moving_ids: HashSet<_> = plan.moving.iter().map(|&i| &tree.items[i].id).collect();
    result.retain(|item| !moving_ids.contains(&item.id));
    let target_id = &tree.items[plan.target].id;
    let target_index = result
        .iter()
        .position(|item| &item.id == target_id)
        .unwrap_or(result.len());
    let insertion = match mode {
        TreeViewDropMode::Before => target_index,
        TreeViewDropMode::After => target_index + 1,
        TreeViewDropMode::Inside => result
            .iter()
            .rposition(|item| &item.parent_id == target_id)
            .map_or(result.len(), |i| i + 1),
    };
    result.splice(
        insertion..insertion,
        plan.moving.into_iter().map(|index| TreeViewItem {
            parent_id: plan.parent.clone(),
            ..tree.items[index].clone()
        }),
    );
    model(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> ModelRc<TreeViewItem> {
        model(
            [
                ("root", ""),
                ("a", "root"),
                ("b", "root"),
                ("leaf", "a"),
                ("second", ""),
            ]
            .into_iter()
            .map(|(id, parent_id)| TreeViewItem {
                id: id.into(),
                parent_id: parent_id.into(),
                can_have_children: true,
                ..Default::default()
            })
            .collect(),
        )
    }

    #[test]
    fn collapsed_rows_keep_selection_and_show_partial_ancestors() {
        let items = fixture();
        let chosen = set_selected(
            items.clone(),
            ModelRc::default(),
            "root".into(),
            true,
            TreeViewSelectionMode::Multiple,
        );
        assert_eq!(chosen.row_count(), 4);
        let chosen = set_selected(
            items.clone(),
            chosen,
            "leaf".into(),
            false,
            TreeViewSelectionMode::Multiple,
        );
        let view = layout(
            items,
            ModelRc::default(),
            chosen,
            TreeViewSelectionMode::Multiple,
        );
        assert_eq!(view.row_count(), 2);
        assert_eq!(view.row_data(0).unwrap().check, TreeViewCheckState::Partial);
    }

    #[test]
    fn moving_child_and_parent_only_moves_topmost_node_and_rejects_cycles() {
        let items = fixture();
        let moving = model(vec!["a".into(), "leaf".into()]);
        let rejected = move_items(
            items.clone(),
            moving.clone(),
            "leaf".into(),
            TreeViewDropMode::Inside,
        );
        assert_eq!(rejected, items);
        let moved = move_items(items, moving, "second".into(), TreeViewDropMode::Inside);
        let moved = Tree::new(&moved);
        assert_eq!(moved.items[moved.by_id["a"]].parent_id.as_str(), "second");
        assert_eq!(moved.items[moved.by_id["leaf"]].parent_id.as_str(), "a");
    }
}
