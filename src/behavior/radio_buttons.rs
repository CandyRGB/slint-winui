use slint::{Model, ModelRc};

use crate::{RadioButtonsBehavior, RadioButtonsItem};

pub fn install(behavior: RadioButtonsBehavior<'_>) {
    behavior.on_index_of(|items, id| {
        items
            .iter()
            .position(|item| item.id == id)
            .map_or(-1, |index| index as i32)
    });
    behavior.on_next_enabled(|items, index, direction| {
        let count = items.row_count() as i32;
        let mut candidate = index + direction;
        while (0..count).contains(&candidate) {
            if is_enabled(&items, candidate) {
                return candidate;
            }
            candidate += direction;
        }
        -1
    });
    behavior.on_across_columns(|items, index, columns, direction| {
        let count = items.row_count() as i32;
        if !(0..count).contains(&index) || !(2..=count).contains(&columns) {
            return -1;
        }

        let rows = count / columns;
        let longer_columns = count % columns;
        let first_short_column = longer_columns * (rows + 1);
        let position = |index| {
            if index < first_short_column {
                (index / (rows + 1), index % (rows + 1))
            } else {
                let short_index = index - first_short_column;
                (longer_columns + short_index / rows, short_index % rows)
            }
        };
        let (column, row) = position(index);
        (0..count)
            .filter_map(|candidate| {
                let (candidate_column, candidate_row) = position(candidate);
                let distance = (candidate_column - column) * direction;
                (distance > 0 && is_enabled(&items, candidate)).then_some((
                    distance,
                    (candidate_row - row).abs(),
                    candidate,
                ))
            })
            .min()
            .map_or(-1, |(_, _, candidate)| candidate)
    });
}

fn is_enabled(items: &ModelRc<RadioButtonsItem>, index: i32) -> bool {
    items
        .row_data(index as usize)
        .is_some_and(|item| !item.disabled)
}
