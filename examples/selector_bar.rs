#[path = "../src/behavior/selector_bar.rs"]
mod selector_bar;

use slint::ComponentHandle;

slint::slint! {
    import { SelectorBarBehavior, SelectorBarExample, SelectorBarItem } from "ui/selector_bar.slint";
    export { SelectorBarBehavior, SelectorBarExample, SelectorBarItem }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = SelectorBarExample::new()?;
    selector_bar::install(ui.global::<SelectorBarBehavior>());
    if let Ok(colors) = slint_winui::platform::accent::light_accent_text_colors() {
        // WinUI's light AccentFillColorDefault uses the AccentDark1 swatch.
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(slint::Color::from_rgb_u8(red, green, blue).into());
    }
    ui.run()
}
