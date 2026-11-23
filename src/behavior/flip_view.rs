use slint::Model;

use crate::FlipViewBehavior;

pub fn install(behavior: FlipViewBehavior<'_>) {
    behavior.on_index_of(|items, id| {
        items
            .iter()
            .position(|item| item.id == id)
            .map_or(-1, |index| index as i32)
    });
}
