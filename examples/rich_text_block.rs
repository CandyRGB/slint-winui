#[path = "../src/behavior/command_bar.rs"]
mod command_bar;
#[path = "../src/behavior/rich_text_block.rs"]
mod rich_text_block;

use slint::{Color, ComponentHandle};
use slint_winui::platform::accent::light_accent_text_colors;

slint::slint! {
    import { RichTextBlockExample, RichTextAlignment, RichTextBehavior, RichTextFill, RichTextFormat,
        RichTextFrame, RichTextHighlight, RichTextHit, RichTextImage, RichTextLayout, RichTextLink,
        RichTextLocation, RichTextNavigation, RichTextParagraph, RichTextRect, RichTextRun,
        AppBarLabelPosition, CommandBarBehavior, CommandBarEntry, CommandBarItem, CommandBarItemKind,
        CommandBarLayout, CommandBarOverflowButtonVisibility } from "ui/rich_text_block.slint";
    export { RichTextBlockExample, RichTextAlignment, RichTextBehavior, RichTextFill, RichTextFormat,
        RichTextFrame, RichTextHighlight, RichTextHit, RichTextImage, RichTextLayout, RichTextLink,
        RichTextLocation, RichTextNavigation, RichTextParagraph, RichTextRect, RichTextRun,
        AppBarLabelPosition, CommandBarBehavior, CommandBarEntry, CommandBarItem, CommandBarItemKind,
        CommandBarLayout, CommandBarOverflowButtonVisibility }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = RichTextBlockExample::new()?;
    rich_text_block::install(ui.global::<RichTextBehavior>())?;
    command_bar::install(ui.global::<CommandBarBehavior>());
    if let Ok(colors) = light_accent_text_colors() {
        let [red, green, blue] = colors.primary;
        ui.set_link_accent(Color::from_rgb_u8(red, green, blue));
    }
    ui.on_copy_text(|value| {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(value.to_string());
        }
    });
    ui.show()?;
    ui.set_text_scale(ui.window().scale_factor());
    slint::run_event_loop()?;
    Ok(())
}
