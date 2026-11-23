use slint::Model;

use crate::ComboBoxBehavior;

pub fn install(behavior: ComboBoxBehavior<'_>) {
    behavior.on_next(|items, start, direction| {
        let count = items.row_count() as i32;
        if direction == 0 {
            return -1;
        }
        let direction = direction.signum();
        let mut index = start.clamp(-1, count) + direction;
        while (0..count).contains(&index) {
            if items
                .row_data(index as usize)
                .is_some_and(|item| !item.disabled)
            {
                return index;
            }
            index += direction;
        }
        -1
    });

    behavior.on_find(|items, query, start, exact| {
        let count = items.row_count();
        if count == 0 || query.is_empty() {
            return -1;
        }
        let first = start.saturating_add(1).max(0) as usize % count;
        let query = query.to_lowercase();
        (0..count)
            .map(|offset| (first + offset) % count)
            .find(|&index| {
                items.row_data(index).is_some_and(|item| {
                    if item.disabled {
                        return false;
                    }
                    let label = item.text.trim_start_matches(' ').to_lowercase();
                    if exact {
                        label == query
                    } else {
                        label.starts_with(&query)
                    }
                })
            })
            .map_or(-1, |index| index as i32)
    });
}
