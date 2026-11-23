use slint::{Model, ModelRc, VecModel};

use crate::{BreadcrumbBarBehavior, BreadcrumbLayout, BreadcrumbMeasurement, BreadcrumbRow, Crumb};

const INLINE_PADDING: f32 = 4.0;
const CHEVRON_WIDTH: f32 = 16.0;
const ELLIPSIS_WIDTH: f32 = 40.0;
const OVERFLOW_PADDING: f32 = 32.0;

pub fn install(behavior: BreadcrumbBarBehavior<'_>) {
    behavior.on_measurements(|| ModelRc::new(VecModel::<BreadcrumbMeasurement>::default()));
    behavior.on_measure(|model, index, label, width| {
        let Some(metrics) = model
            .as_any()
            .downcast_ref::<VecModel<BreadcrumbMeasurement>>()
        else {
            return;
        };
        let index = index as usize;
        while metrics.row_count() <= index {
            metrics.push(BreadcrumbMeasurement::default());
        }
        if metrics
            .row_data(index)
            .is_some_and(|entry| entry.label != label || entry.width != width)
        {
            metrics.set_row_data(index, BreadcrumbMeasurement { label, width });
        }
    });
    behavior.on_layout(layout);
    behavior.set_ready(true);
}

fn layout(
    items: ModelRc<Crumb>,
    measurements: ModelRc<BreadcrumbMeasurement>,
    width: f32,
) -> BreadcrumbLayout {
    let mut rows: Vec<_> = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let text_width = measurements
                .row_data(index)
                .filter(|metric| metric.label == item.label)
                .map_or(0.0, |metric| (metric.width / 2.0).ceil() * 2.0);
            let separator = if index + 1 < items.row_count() {
                CHEVRON_WIDTH
            } else {
                0.0
            };
            BreadcrumbRow {
                index: index as i32,
                label: item.label,
                width: text_width + INLINE_PADDING + separator,
                offset: 0.0,
            }
        })
        .collect();
    let Some((current, ancestors)) = rows.split_last_mut() else {
        return BreadcrumbLayout::default();
    };
    let ancestors_width: f32 = ancestors.iter().map(|row| row.width).sum();
    let overflow_button = ELLIPSIS_WIDTH.min(width.max(0.0));
    current.width = current
        .width
        .min((width - ancestors_width.min(overflow_button)).max(0.0));

    let hidden_count =
        if ancestors_width > overflow_button && ancestors_width + current.width > width {
            let mut remaining_width = (width - current.width - overflow_button).max(0.0);
            let mut hidden_count = ancestors.len();
            for row in ancestors.iter().rev() {
                if row.width > remaining_width {
                    break;
                }
                remaining_width -= row.width;
                hidden_count -= 1;
            }
            hidden_count
        } else {
            0
        };

    let mut inline = rows.split_off(hidden_count);
    if hidden_count > 0 {
        inline.insert(
            0,
            BreadcrumbRow {
                index: -1,
                width: overflow_button,
                ..Default::default()
            },
        );
    }
    let mut offset = 0.0;
    for row in &mut inline {
        row.offset = offset;
        offset += row.width;
    }
    let overflow_width = rows.iter().fold(96.0_f32, |width, row| {
        width.max(row.width - INLINE_PADDING - CHEVRON_WIDTH + OVERFLOW_PADDING)
    });
    rows.reverse();
    BreadcrumbLayout {
        inline: ModelRc::new(VecModel::from(inline)),
        overflow: ModelRc::new(VecModel::from(rows)),
        overflow_width,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(width: f32, labels: &[(&str, f32)]) -> BreadcrumbLayout {
        layout(
            ModelRc::new(VecModel::from_iter(labels.iter().map(|(label, _)| Crumb {
                label: (*label).into(),
            }))),
            ModelRc::new(VecModel::from_iter(labels.iter().map(|(label, width)| {
                BreadcrumbMeasurement {
                    label: (*label).into(),
                    width: *width,
                }
            }))),
            width,
        )
    }

    #[test]
    fn overflow_keeps_visible_suffix_and_reports_original_indices() {
        let labels = [
            ("Home", 40.0),
            ("Documents", 100.0),
            ("Images", 40.0),
            ("Current", 46.0),
        ];
        let wide = path(290.0, &labels);
        assert_eq!(
            wide.inline.iter().map(|row| row.index).collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        let narrow = path(150.0, &labels);
        assert_eq!(
            narrow
                .inline
                .iter()
                .map(|row| row.index)
                .collect::<Vec<_>>(),
            [-1, 2, 3]
        );
        assert_eq!(
            narrow
                .overflow
                .iter()
                .map(|row| row.index)
                .collect::<Vec<_>>(),
            [1, 0]
        );
    }

    #[test]
    fn long_current_item_is_constrained_to_its_slot() {
        let labels = [("Root", 80.0), ("Long current", 360.0)];
        let narrow = path(100.0, &labels);
        assert_eq!(narrow.inline.row_data(1).unwrap().width, 60.0);
        assert_eq!(
            path(24.0, &labels[1..]).inline.row_data(0).unwrap().width,
            24.0
        );
        assert_eq!(path(0.0, &[]).inline.row_count(), 0);
    }
}
