use slint::{Model, ModelRc, SharedString};

use crate::{SelectorBarBehavior, SelectorBarItem};

pub fn install(behavior: SelectorBarBehavior<'_>) {
    behavior.on_index_of(index_of);
    behavior.on_navigate(navigate);
    behavior.set_ready(true);
}

fn index_of(items: ModelRc<SelectorBarItem>, id: SharedString, focusable: bool) -> i32 {
    if id.is_empty() {
        return -1;
    }
    items
        .iter()
        .position(|item| item.id == id && (!focusable || !item.disabled))
        .map_or(-1, |index| index as i32)
}

fn navigate(items: ModelRc<SelectorBarItem>, from: i32, forward: bool) -> i32 {
    let count = items.row_count() as i32;
    let is_focusable = |index: &i32| {
        items
            .row_data(*index as usize)
            .is_some_and(|item| !item.disabled)
    };
    let target = if forward {
        ((from + 1).clamp(0, count)..count).find(is_focusable)
    } else {
        (0..from.clamp(0, count)).rev().find(is_focusable)
    };
    target.unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::VecModel;

    #[test]
    fn selection_and_keyboard_availability_are_independent() {
        let items = ModelRc::new(VecModel::from(vec![
            SelectorBarItem {
                id: "first".into(),
                ..Default::default()
            },
            SelectorBarItem {
                id: "unavailable".into(),
                disabled: true,
                ..Default::default()
            },
            SelectorBarItem {
                id: "last".into(),
                ..Default::default()
            },
        ]));
        assert_eq!(index_of(items.clone(), "unavailable".into(), false), 1);
        assert_eq!(index_of(items.clone(), "unavailable".into(), true), -1);
        assert_eq!(index_of(items.clone(), "removed".into(), false), -1);
        assert_eq!(navigate(items.clone(), 0, true), 2);
        assert_eq!(navigate(items.clone(), 2, false), 0);
        assert_eq!(navigate(items, 2, true), -1);
    }
}
