use std::collections::{HashMap, HashSet};

use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, Model, ModelExt, ModelRc, SharedString, VecModel};

use crate::{
    NavigationViewBehavior, NavigationViewItem, NavigationViewItemKind, NavigationViewLeftLayout,
    NavigationViewMeasurement, NavigationViewRow, NavigationViewTopContentWidths,
    NavigationViewTopLayout,
};

const OVERFLOW_ID: &str = "$overflow";
const TOP_OVERFLOW_WIDTH: f32 = 40.0;

pub fn install<T: ComponentHandle + 'static>(behavior: NavigationViewBehavior<'_>, owner: &T) {
    let owner = owner.as_weak();
    behavior.on_traverse_focus(move |forward| {
        if let Some(owner) = owner.upgrade() {
            let key = if forward { Key::Tab } else { Key::Backtab };
            owner
                .window()
                .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
        }
    });
    behavior.on_measurements(|| ModelRc::new(VecModel::<NavigationViewMeasurement>::default()));
    behavior.on_measure(|model, id, width| {
        let Some(metrics) = model
            .as_any()
            .downcast_ref::<VecModel<NavigationViewMeasurement>>()
        else {
            return;
        };
        if id.is_empty() {
            return;
        }
        let previous = metrics.iter().position(|entry| entry.id == id);
        let entry = NavigationViewMeasurement { id, width };
        if let Some(index) = previous {
            if metrics
                .row_data(index)
                .is_some_and(|current| current.width != width)
            {
                metrics.set_row_data(index, entry);
            }
        } else {
            metrics.push(entry);
        }
    });
    behavior.on_left_rows(left_rows);
    behavior.on_top_rows(top_rows);
    behavior.on_flyout_rows(flyout_rows);
    behavior.on_next(|rows, id, forward| {
        let count = rows.row_count();
        let index = find(&rows, &id);
        let range = if forward {
            (index + 1) as usize..count
        } else {
            0..if index < 0 { count } else { index as usize }
        };
        if forward {
            range.into_iter().find(|&index| focusable(&rows, index))
        } else {
            range.rev().find(|&index| focusable(&rows, index))
        }
        .map_or(-1, |index| index as i32)
    });
    behavior.on_find(|rows, id| find(&rows, &id));
    behavior.on_index(|rows, id| {
        let Ok(index) = usize::try_from(find(&rows, &id)) else {
            return -1;
        };
        rows.row_data_tracked(index)
            .map_or(-1, |row| row.accessible_index)
    });
    behavior.on_indicator(|rows| {
        rows.iter()
            .find(|row| row.shows_indicator)
            .map_or_else(SharedString::default, |row| row.item.id)
    });
    behavior.on_last(|rows| {
        rows.row_count()
            .checked_sub(1)
            .and_then(|index| rows.row_data_tracked(index))
            .map_or(0.0, |row| row.offset + row.extent + 4.0)
    });
    behavior.on_toggle(|ids, id| {
        let mut expanded = tracked(&ids);
        if let Some(index) = expanded.iter().position(|entry| *entry == id) {
            expanded.remove(index);
        } else {
            expanded.push(id);
        }
        model(expanded)
    });
    behavior.set_ready(true);
}

fn tracked<T: Clone + 'static>(model: &ModelRc<T>) -> Vec<T> {
    model.model_tracker().track_row_count_changes();
    (0..model.row_count())
        .filter_map(|index| model.row_data_tracked(index))
        .collect()
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

fn is_item(item: &NavigationViewItem) -> bool {
    item.kind == NavigationViewItemKind::Item
}

fn focusable(rows: &ModelRc<NavigationViewRow>, index: usize) -> bool {
    rows.row_data_tracked(index)
        .is_some_and(|row| is_item(&row.item) && !row.item.disabled)
}

fn find(rows: &ModelRc<NavigationViewRow>, id: &str) -> i32 {
    if id.is_empty() {
        return -1;
    }
    (0..rows.row_count())
        .position(|index| {
            rows.row_data_tracked(index).is_some_and(|row| {
                row.item.id.as_str() == id && is_item(&row.item) && !row.item.disabled
            })
        })
        .map_or(-1, |index| index as i32)
}

struct Items {
    all: HashMap<SharedString, NavigationViewItem>,
    children: HashMap<SharedString, Vec<SharedString>>,
    primary: Vec<SharedString>,
    footer: Vec<SharedString>,
}

impl Items {
    fn new(
        menu: &ModelRc<NavigationViewItem>,
        footer: &ModelRc<NavigationViewItem>,
        settings: NavigationViewItem,
    ) -> Self {
        let mut items = Self {
            all: HashMap::new(),
            children: HashMap::new(),
            primary: Vec::new(),
            footer: Vec::new(),
        };
        for (group, in_footer) in [
            (tracked(menu), false),
            (tracked(footer), true),
            (vec![settings], true),
        ] {
            for item in group {
                let id = item.id.clone();
                if id.is_empty() || id.as_str() == OVERFLOW_ID || items.all.contains_key(&id) {
                    continue;
                }
                if !item.parent_id.is_empty() {
                    items
                        .children
                        .entry(item.parent_id.clone())
                        .or_default()
                        .push(id.clone());
                } else if in_footer {
                    items.footer.push(id.clone());
                } else {
                    items.primary.push(id.clone());
                }
                items.all.insert(id, item);
            }
        }
        items
    }

    fn children(&self, id: &SharedString) -> &[SharedString] {
        self.children.get(id).map_or(&[], Vec::as_slice)
    }

    fn visible(
        &self,
        roots: &[SharedString],
        expanded: &HashSet<SharedString>,
        compact: bool,
    ) -> Vec<NavigationViewRow> {
        let mut rows = Vec::new();
        let mut pending: Vec<_> = roots.iter().rev().map(|id| (id, 0)).collect();
        while let Some((id, depth)) = pending.pop() {
            let item = &self.all[id];
            if compact && item.kind == NavigationViewItemKind::Header {
                continue;
            }
            let children = self.children(id);
            let opened = !compact && expanded.contains(id) && is_item(item) && !children.is_empty();
            rows.push(NavigationViewRow {
                item: item.clone(),
                depth,
                has_children: is_item(item) && !children.is_empty(),
                expanded: opened,
                ..Default::default()
            });
            if opened {
                pending.extend(children.iter().rev().map(|child| (child, depth + 1)));
            }
        }
        rows
    }

    fn nearest(
        &self,
        id: &SharedString,
        visible: impl Fn(&SharedString) -> bool,
    ) -> Option<SharedString> {
        let mut current = id;
        for _ in 0..self.all.len() {
            let item = self.all.get(current)?;
            if visible(current) {
                return Some(current.clone());
            }
            current = &item.parent_id;
        }
        None
    }

    fn mark(&self, rows: &mut [NavigationViewRow], id: &SharedString) {
        let indicator = self.nearest(id, |id| rows.iter().any(|row| row.item.id == *id));
        let mut accessible_index = 0;
        for row in rows {
            row.selected = is_item(&row.item) && row.item.id == *id;
            row.shows_indicator = is_item(&row.item) && indicator.as_ref() == Some(&row.item.id);
            if is_item(&row.item) {
                row.accessible_index = accessible_index;
                accessible_index += 1;
            } else {
                row.accessible_index = -1;
            }
        }
    }
}

fn layout_vertical(rows: &mut [NavigationViewRow], start: f32) -> f32 {
    let mut offset = start;
    for row in rows {
        row.offset = offset;
        row.extent = match row.item.kind {
            NavigationViewItemKind::Item => 36.0,
            NavigationViewItemKind::Header => 32.0,
            NavigationViewItemKind::Separator => 8.0,
        };
        offset += row.extent + 4.0;
    }
    offset
}

fn left_rows(
    menu: ModelRc<NavigationViewItem>,
    footer: ModelRc<NavigationViewItem>,
    settings: NavigationViewItem,
    expanded: ModelRc<SharedString>,
    selected: SharedString,
    height: f32,
    compact: bool,
) -> NavigationViewLeftLayout {
    let items = Items::new(&menu, &footer, settings);
    let expanded = tracked(&expanded).into_iter().collect();
    let mut rows = items.visible(&items.primary, &expanded, compact);
    let mut trailing = items.visible(&items.footer, &expanded, compact);
    let menu_extent = layout_vertical(&mut rows, 0.0);
    let footer_extent = layout_vertical(&mut trailing, 0.0);
    let mut available = (height.max(0.0) / 2.0).floor() * 2.0;
    let separator =
        menu_extent > 0.0 && footer_extent > 0.0 && menu_extent + footer_extent > available;
    if separator {
        available = (available - 4.0).max(0.0);
    }
    let half = (available / 4.0).floor() * 2.0;
    let footer_height = if separator && footer_extent > half {
        available - menu_extent.min(half)
    } else {
        footer_extent.min(available)
    };
    items.mark(&mut rows, &selected);
    items.mark(&mut trailing, &selected);
    NavigationViewLeftLayout {
        menu: model(rows),
        footer: model(trailing),
        menu_height: available - footer_height,
        footer_height,
        separator,
    }
}

fn top_rows(
    menu: ModelRc<NavigationViewItem>,
    footer: ModelRc<NavigationViewItem>,
    settings: NavigationViewItem,
    metrics: ModelRc<NavigationViewMeasurement>,
    selected: SharedString,
    available: f32,
    content: NavigationViewTopContentWidths,
) -> NavigationViewTopLayout {
    let items = Items::new(&menu, &footer, settings);
    let available = (available.max(0.0) / 2.0).floor() * 2.0;
    let metrics: HashMap<_, _> = tracked(&metrics)
        .into_iter()
        .map(|entry| (entry.id, entry.width))
        .collect();
    let expanded = HashSet::new();
    let mut primary = items.visible(&items.primary, &expanded, true);
    let mut trailing = items.visible(&items.footer, &expanded, true);
    for row in primary.iter_mut().chain(&mut trailing) {
        row.extent = if row.item.kind == NavigationViewItemKind::Separator {
            12.0
        } else if row.item.id.as_str() == "$settings" {
            40.0
        } else {
            let width = metrics.get(&row.item.id).copied().unwrap_or(48.0);
            width + if row.has_children { 20.0 } else { 0.0 }
        };
    }
    let selected_root = items.nearest(&selected, |id| primary.iter().any(|row| row.item.id == *id));
    let protected = primary
        .iter()
        .position(|row| Some(&row.item.id) == selected_root.as_ref())
        .or_else(|| {
            primary
                .iter()
                .position(|row| is_item(&row.item) && !row.item.disabled)
        })
        .or_else(|| primary.iter().position(|row| is_item(&row.item)));
    let primary_width: f32 = primary.iter().map(|row| row.extent).sum();
    let trailing_width: f32 = trailing.iter().map(|row| row.extent).sum();
    let navigation_width = (available - content.custom - content.search - content.footer).max(0.0);
    let overflow_width = if primary_width + trailing_width > navigation_width {
        TOP_OVERFLOW_WIDTH.min(navigation_width)
    } else {
        0.0
    };
    let mut remaining = navigation_width - overflow_width;
    let mut overflow = Vec::new();
    let mut rows = Vec::new();
    let mut top_footer = Vec::new();
    if let Some(index) = protected {
        primary[index].extent = primary[index].extent.min(remaining.max(0.0));
        remaining -= primary[index].extent;
    }
    for row in trailing.into_iter().rev() {
        if row.extent <= remaining {
            remaining -= row.extent;
            top_footer.push(row);
        } else {
            overflow.push(row);
        }
    }
    top_footer.reverse();
    overflow.reverse();
    let mut hidden_primary = Vec::new();
    let mut offset = 0.0;
    for (index, mut row) in primary.into_iter().enumerate() {
        if Some(index) == protected || row.extent <= remaining {
            if Some(index) != protected {
                remaining -= row.extent;
            }
            if row.extent > 0.0 {
                row.offset = offset;
                offset += row.extent;
                rows.push(row);
            } else {
                hidden_primary.push(row);
            }
        } else {
            hidden_primary.push(row);
        }
    }
    hidden_primary.extend(overflow);
    if !hidden_primary.is_empty() {
        rows.push(NavigationViewRow {
            item: NavigationViewItem {
                id: OVERFLOW_ID.into(),
                label: "More".into(),
                suppress_selection: true,
                ..Default::default()
            },
            offset,
            extent: overflow_width,
            ..Default::default()
        });
        offset += overflow_width;
    }
    let mut end_offset = available - top_footer.iter().map(|row| row.extent).sum::<f32>();
    let footer_offset = end_offset - content.footer;
    let search_offset = footer_offset - content.search;
    for mut row in top_footer {
        row.offset = end_offset;
        end_offset += row.extent;
        rows.push(row);
    }
    items.mark(&mut rows, &selected);
    NavigationViewTopLayout {
        rows: model(rows),
        overflow: model(hidden_primary),
        primary_end: offset,
        search_offset,
        footer_offset,
    }
}

fn flyout_rows(
    menu: ModelRc<NavigationViewItem>,
    footer: ModelRc<NavigationViewItem>,
    overflow: ModelRc<NavigationViewRow>,
    expanded: ModelRc<SharedString>,
    owner: SharedString,
    selected: SharedString,
) -> ModelRc<NavigationViewRow> {
    if owner.is_empty() {
        return ModelRc::default();
    }
    let overflow = if owner.as_str() == OVERFLOW_ID {
        tracked(&overflow)
    } else {
        Vec::new()
    };
    let mut items = Items::new(&menu, &footer, NavigationViewItem::default());
    for row in &overflow {
        items
            .all
            .entry(row.item.id.clone())
            .or_insert_with(|| row.item.clone());
    }
    let overflow_ids: Vec<_> = overflow.into_iter().map(|row| row.item.id).collect();
    let roots = if owner.as_str() == OVERFLOW_ID {
        &overflow_ids
    } else {
        items.children(&owner)
    };
    let expanded = tracked(&expanded).into_iter().collect();
    let mut rows = items.visible(roots, &expanded, false);
    layout_vertical(&mut rows, 0.0);
    items.mark(&mut rows, &selected);
    model(rows)
}
