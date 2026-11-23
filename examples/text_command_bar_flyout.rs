#[path = "../src/behavior/command_bar.rs"]
mod command_bar;
#[path = "../src/behavior/password_box.rs"]
mod password_box;

use std::{cell::RefCell, rc::Rc};

slint::slint! {
    import {
        AppBarLabelPosition, CommandBarBehavior, CommandBarEntry, CommandBarItem, CommandBarItemKind,
        CommandBarLayout, CommandBarOverflowButtonVisibility, TextCommandBarFlyoutExample,
        TextEditSnapshot, PasswordBoxBehavior, PasswordEdit, PasswordMask,
    } from "ui/text_command_bar_flyout.slint";
    export {
        AppBarLabelPosition, CommandBarBehavior, CommandBarEntry, CommandBarItem, CommandBarItemKind,
        CommandBarLayout, CommandBarOverflowButtonVisibility, TextCommandBarFlyoutExample,
        TextEditSnapshot, PasswordBoxBehavior, PasswordEdit, PasswordMask,
    }
}

// The demo owns its edit history. The flyout can also use an editor's history directly.
struct History {
    entries: Vec<TextEditSnapshot>,
    current: usize,
}

impl History {
    fn publish_availability(&self, window: &TextCommandBarFlyoutExample) {
        window.set_can_undo(self.current > 0);
        window.set_can_redo(self.current + 1 < self.entries.len());
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = TextCommandBarFlyoutExample::new()?;
    command_bar::install(window.global::<CommandBarBehavior>());
    password_box::install(window.global::<PasswordBoxBehavior>());
    window.on_has_clipboard_text(|| {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.get_text())
            .is_ok_and(|text| !text.is_empty())
    });

    let history = Rc::new(RefCell::new(History {
        entries: vec![window.get_editable_snapshot()],
        current: 0,
    }));
    let window_weak = window.as_weak();
    let edit_history = Rc::clone(&history);
    window.on_record_edit(move |snapshot| {
        let mut history = edit_history.borrow_mut();
        let current = history.current;
        if history.entries[current].text == snapshot.text {
            return;
        }
        history.entries.truncate(current + 1);
        history.entries.push(snapshot);
        history.current += 1;
        if let Some(window) = window_weak.upgrade() {
            history.publish_availability(&window);
        }
    });
    let window_weak = window.as_weak();
    window.on_move_history(move |redo| {
        let mut history = history.borrow_mut();
        history.current = if redo {
            (history.current + 1).min(history.entries.len() - 1)
        } else {
            history.current.saturating_sub(1)
        };
        if let Some(window) = window_weak.upgrade() {
            history.publish_availability(&window);
        }
        history.entries[history.current].clone()
    });
    window.run()
}
