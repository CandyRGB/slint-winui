use slint::Color;
use slint_winui::platform::accent::light_accent_text_colors;

slint::slint! {
    import { ToggleButtonExample } from "ui/toggle_button.slint";
    export { ToggleButtonExample }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = ToggleButtonExample::new()?;
    if let Ok(colors) = light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(Color::from_rgb_u8(red, green, blue));
    }
    ui.run()
}
