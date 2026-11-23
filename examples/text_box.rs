#[path = "../src/behavior/command_bar.rs"]
mod command_bar;
#[path = "../src/behavior/text_edit_limit.rs"]
mod text_edit_limit;

use slint::Color;
use slint_winui::platform::accent::light_accent_text_colors;

slint::slint! {
    import { TextBoxExample, TextBoxBehavior, TextBoxEdit, AppBarLabelPosition, CommandBarBehavior,
        CommandBarEntry, CommandBarItem, CommandBarItemKind, CommandBarLayout, CommandBarOverflowButtonVisibility }
        from "ui/text_box.slint";
    export { TextBoxExample, TextBoxBehavior, TextBoxEdit, AppBarLabelPosition, CommandBarBehavior,
        CommandBarEntry, CommandBarItem, CommandBarItemKind, CommandBarLayout, CommandBarOverflowButtonVisibility }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = TextBoxExample::new()?;
    ui.global::<TextBoxBehavior>()
        .on_limit_edit(|previous, draft, cursor, maximum| {
            let (text, cursor) = text_edit_limit::limit_edit(&previous, draft, cursor, maximum);
            TextBoxEdit { text, cursor }
        });
    command_bar::install(ui.global::<CommandBarBehavior>());
    if let Ok(colors) = light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(Color::from_rgb_u8(red, green, blue));
    }
    ui.on_has_clipboard_text(|| {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.get_text())
            .is_ok_and(|text| !text.is_empty())
    });
    ui.run()
}
