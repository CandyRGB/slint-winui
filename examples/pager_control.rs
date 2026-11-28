#[path = "../src/behavior/combo_box.rs"]
mod combo_box;
#[path = "../src/behavior/number_box.rs"]
mod number_box;
#[path = "../src/behavior/pager_control.rs"]
mod pager_control;

slint::slint! {
    import { PagerControlExample, PagerControlBehavior, ComboBoxBehavior, ComboBoxItem, NumberBoxMath, NumberBoxParseResult, NumberBoxValue } from "ui/pager_control.slint";
    export { PagerControlExample, PagerControlBehavior, ComboBoxBehavior, ComboBoxItem, NumberBoxMath, NumberBoxParseResult, NumberBoxValue }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = PagerControlExample::new()?;
    pager_control::install_for(&ui);
    if let Ok(colors) = slint_winui::platform::accent::light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(slint::Color::from_rgb_u8(red, green, blue).into());
    }
    ui.run()
}
