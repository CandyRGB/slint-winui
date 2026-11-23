#[path = "../src/behavior/combo_box.rs"]
mod combo_box;

use slint::{Color, ComponentHandle};
use slint_winui::platform::accent::light_accent_text_colors;

slint::slint! {
    import { ComboBoxBehavior, ComboBoxExample, ComboBoxItem } from "ui/combo_box.slint";
    export { ComboBoxBehavior, ComboBoxExample, ComboBoxItem }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = ComboBoxExample::new()?;
    combo_box::install(ui.global::<ComboBoxBehavior>());
    if let Ok(colors) = light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(Color::from_rgb_u8(red, green, blue));
    }
    ui.run()
}
