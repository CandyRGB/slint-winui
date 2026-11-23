use slint::{Model, ModelRc, SharedString, VecModel};

use crate::{
    TabViewBehavior, TabViewItem, TabViewLayout, TabViewMeasurement, TabViewRow, TabViewWidthMode,
};

const EQUAL_MIN: f32 = 100.0;
const MAX_WIDTH: f32 = 240.0;
const ICON_AND_GAP: f32 = 26.0;
const ICON_WIDTH: f32 = 16.0;
const CLOSE_WIDTH: f32 = 32.0;
const TAB_PADDING: f32 = 16.0;

pub fn install(behavior: TabViewBehavior<'_>) {
    behavior.on_measurements(|| ModelRc::new(VecModel::<TabViewMeasurement>::default()));
    behavior.on_measure(|model, index, label, width| {
        let Some(metrics) = model
            .as_any()
            .downcast_ref::<VecModel<TabViewMeasurement>>()
        else {
            return;
        };
        let index = index as usize;
        while metrics.row_count() <= index {
            metrics.push(TabViewMeasurement::default());
        }
        if metrics
            .row_data(index)
            .is_some_and(|entry| entry.label != label || entry.width != width)
        {
            metrics.set_row_data(index, TabViewMeasurement { label, width });
        }
    });
    behavior.on_layout(layout);
    behavior.on_find_adjacent(find_adjacent);
    behavior.on_drop_index(|rows, origin, x| {
        let insertion = rows
            .iter()
            .position(|row| x < row.offset + row.width / 2.0)
            .unwrap_or_else(|| rows.row_count()) as i32;
        insertion - i32::from(insertion > origin)
    });
    behavior.set_ready(true);
}

fn rounded_even(value: f32) -> f32 {
    (value / 2.0).round() * 2.0
}

fn layout(
    items: ModelRc<TabViewItem>,
    measurements: ModelRc<TabViewMeasurement>,
    requested: SharedString,
    previous: i32,
    width: f32,
    mode: TabViewWidthMode,
) -> TabViewLayout {
    let count = items.row_count();
    let selected = items
        .iter()
        .position(|item| item.id == requested && !item.disabled)
        .or_else(|| {
            (previous.max(0) as usize..count)
                .find(|&i| items.row_data(i).is_some_and(|item| !item.disabled))
        })
        .or_else(|| {
            (0..(previous.max(0) as usize).min(count))
                .rev()
                .find(|&i| items.row_data(i).is_some_and(|item| !item.disabled))
        });
    let equal_total = rounded_even(width).clamp(EQUAL_MIN * count as f32, MAX_WIDTH * count as f32);
    let mut offset = 0.0;
    let rows = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let compact = mode == TabViewWidthMode::Compact && Some(index) != selected;
            let label_width = measurements
                .row_data(index)
                .filter(|measurement| measurement.label == item.label)
                .map_or(0.0, |measurement| measurement.width);
            let icon_width = if item.icon.size().width == 0 {
                0.0
            } else if compact {
                ICON_WIDTH
            } else {
                ICON_AND_GAP
            };
            let close_width = if !item.non_closable { CLOSE_WIDTH } else { 0.0 };
            let width = match mode {
                TabViewWidthMode::Equal => {
                    let step = equal_total / count as f32;
                    rounded_even(step * (index + 1) as f32) - rounded_even(step * index as f32)
                }
                _ => rounded_even(
                    TAB_PADDING
                        + icon_width
                        + close_width
                        + if compact { 0.0 } else { label_width },
                )
                .clamp(32.0, MAX_WIDTH),
            };
            let row = TabViewRow {
                item,
                index: index as i32,
                width,
                offset,
                compact,
            };
            offset += width;
            row
        })
        .collect::<Vec<_>>();
    TabViewLayout {
        selected_id: selected.map_or_else(SharedString::default, |i| rows[i].item.id.clone()),
        selected_index: selected.map_or(-1, |i| i as i32),
        total_width: offset,
        rows: ModelRc::new(VecModel::from(rows)),
    }
}

fn find_adjacent(items: ModelRc<TabViewItem>, start: i32, forward: bool, wrap: bool) -> i32 {
    let count = items.row_count() as i32;
    let step = if forward { 1 } else { -1 };
    for distance in 1..=count {
        let index = start + step * distance;
        if !wrap && !(0..count).contains(&index) {
            break;
        }
        let index = index.rem_euclid(count);
        if items
            .row_data(index as usize)
            .is_some_and(|item| !item.disabled)
        {
            return index;
        }
    }
    -1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> ModelRc<TabViewItem> {
        ModelRc::new(VecModel::from_iter((0..4).map(|i| TabViewItem {
            id: i.to_string().into(),
            label: "Document".into(),
            non_closable: i == 0,
            disabled: i == 2,
            ..Default::default()
        })))
    }

    #[test]
    fn equal_tabs_share_extra_pixels_and_overflow_at_the_minimum() {
        let fits = layout(
            items(),
            ModelRc::default(),
            "1".into(),
            0,
            678.0,
            TabViewWidthMode::Equal,
        );
        assert_eq!(fits.total_width, 678.0);
        assert_eq!(
            fits.rows.iter().map(|row| row.width).collect::<Vec<_>>(),
            [170.0, 170.0, 168.0, 170.0]
        );
        let overflow = layout(
            items(),
            ModelRc::default(),
            "1".into(),
            0,
            260.0,
            TabViewWidthMode::Equal,
        );
        assert_eq!(overflow.total_width, 400.0);
        assert!(overflow.rows.iter().all(|row| row.width == 100.0));
    }

    #[test]
    fn compact_layout_retains_selected_header_and_falls_forward_after_removal() {
        let measured = ModelRc::new(VecModel::from(vec![
            TabViewMeasurement {
                label: "Document".into(),
                width: 58.0
            };
            4
        ]));
        let layout = layout(
            items(),
            measured,
            "removed".into(),
            2,
            700.0,
            TabViewWidthMode::Compact,
        );
        assert_eq!(layout.selected_index, 3);
        assert_eq!(
            layout.rows.iter().map(|row| row.width).collect::<Vec<_>>(),
            [32.0, 48.0, 48.0, 106.0]
        );
        assert!(layout.rows.row_data(0).unwrap().compact);
        assert!(!layout.rows.row_data(3).unwrap().compact);
    }

    #[test]
    fn focus_and_cycling_skip_disabled_items() {
        assert_eq!(find_adjacent(items(), 1, true, false), 3);
        assert_eq!(find_adjacent(items(), 0, false, false), -1);
        assert_eq!(find_adjacent(items(), 0, false, true), 3);
    }
}
