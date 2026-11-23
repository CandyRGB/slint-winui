#[path = "../src/behavior/command_bar.rs"]
mod command_bar;
#[path = "../src/behavior/password_box.rs"]
mod password_box;

use slint::Color;
use slint_winui::platform::accent::light_accent_text_colors;

slint::slint! {
    import { PasswordBoxExample, PasswordBoxBehavior, PasswordMask, PasswordEdit, AppBarLabelPosition,
        CommandBarBehavior, CommandBarEntry, CommandBarItem, CommandBarItemKind, CommandBarLayout, CommandBarOverflowButtonVisibility }
        from "ui/password_box.slint";
    export { PasswordBoxExample, PasswordBoxBehavior, PasswordMask, PasswordEdit, AppBarLabelPosition,
        CommandBarBehavior, CommandBarEntry, CommandBarItem, CommandBarItemKind, CommandBarLayout, CommandBarOverflowButtonVisibility }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = PasswordBoxExample::new()?;
    password_box::install(ui.global::<PasswordBoxBehavior>());
    command_bar::install(ui.global::<CommandBarBehavior>());
    if let Ok(colors) = light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(Color::from_rgb_u8(red, green, blue));
    }
    ui.on_clipboard_contains_text(|| {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.get_text())
            .is_ok_and(|text| !text.is_empty())
    });
    ui.run()
}
