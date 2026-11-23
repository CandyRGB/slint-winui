use slint::{Model, ModelRc, SharedString, VecModel};

use crate::{PivotBehavior, PivotHeaderRow, PivotItem, PivotLayout, PivotMeasurement};

pub fn install(behavior: PivotBehavior<'_>) {
    behavior.on_measurements(|| ModelRc::new(VecModel::<PivotMeasurement>::default()));
    behavior.on_measure(|cache, index, label, width| {
        let Some(cache) = cache.as_any().downcast_ref::<VecModel<PivotMeasurement>>() else {
            return;
        };
        let index = index as usize;
        while cache.row_count() <= index {
            cache.push(PivotMeasurement::default());
        }
        if cache
            .row_data(index)
            .is_some_and(|entry| entry.label != label || entry.width != width)
        {
            cache.set_row_data(index, PivotMeasurement { label, width });
        }
    });
    behavior.on_layout(layout);
    behavior.on_next_enabled(next_enabled);
    behavior.set_ready(true);
}

fn next_enabled(items: ModelRc<PivotItem>, start: i32, forward: bool) -> i32 {
    let count = items.row_count() as i32;
    let result = if forward {
        ((start + 1).clamp(0, count)..count).find(|&index| {
            items
                .row_data(index as usize)
                .is_some_and(|item| !item.disabled)
        })
    } else {
        (0..start.clamp(0, count)).rev().find(|&index| {
            items
                .row_data(index as usize)
                .is_some_and(|item| !item.disabled)
        })
    };
    result.unwrap_or(-1)
}

fn layout(
    items: ModelRc<PivotItem>,
    measurements: ModelRc<PivotMeasurement>,
    requested: SharedString,
    previous: i32,
    locked: bool,
) -> PivotLayout {
    let selected = items
        .iter()
        .position(|item| !item.disabled && item.id == requested)
        .map(|index| index as i32)
        .unwrap_or_else(|| {
            let next = next_enabled(items.clone(), previous.max(0) - 1, true);
            if next >= 0 {
                next
            } else {
                next_enabled(items.clone(), previous, false)
            }
        });
    let mut total_width = 0.0;
    let rows = items
        .iter()
        .enumerate()
        .filter(|(index, _)| !locked || *index as i32 == selected)
        .map(|(index, item)| {
            let measured = measurements
                .row_data(index)
                .filter(|measurement| measurement.label == item.label)
                .map_or(0.0, |measurement| measurement.width);
            let width = ((measured + 24.0) / 2.0).round() * 2.0;
            let offset = total_width;
            total_width += width;
            PivotHeaderRow {
                item,
                index: index as i32,
                offset,
                width,
            }
        })
        .collect::<Vec<_>>();
    let selected_row = if locked && selected >= 0 { 0 } else { selected };
    let active = rows.get(selected_row as usize);
    PivotLayout {
        selected_id: active.map_or_else(SharedString::default, |row| row.item.id.clone()),
        selected_index: selected,
        selected_offset: active.map_or(0.0, |row| row.offset),
        selected_width: active.map_or(0.0, |row| row.width),
        total_width,
        rows: ModelRc::new(VecModel::from(rows)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_follows_identity_and_skips_unavailable_headers_without_wrapping() {
        let items = ModelRc::new(VecModel::from(vec![
            PivotItem {
                id: "third".into(),
                ..Default::default()
            },
            PivotItem {
                id: "second".into(),
                disabled: true,
                ..Default::default()
            },
            PivotItem {
                id: "first".into(),
                ..Default::default()
            },
        ]));
        let selected = layout(items.clone(), ModelRc::default(), "third".into(), 2, false);
        assert_eq!(selected.selected_index, 0);
        assert_eq!(next_enabled(items.clone(), 0, true), 2);
        assert_eq!(next_enabled(items.clone(), 2, true), -1);
        let fallback = layout(items, ModelRc::default(), "deleted".into(), 1, true);
        assert_eq!(fallback.selected_index, 2);
        assert_eq!(fallback.rows.row_count(), 1);
        assert_eq!(fallback.selected_offset, 0.0);
    }
}
