use slint::Color;
use slint_winui::platform::accent::light_accent_text_colors;

slint::slint! {
    import { ButtonExample } from "ui/button.slint";
    export { ButtonExample }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = ButtonExample::new()?;
    if let Ok(colors) = light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(Color::from_rgb_u8(red, green, blue));
    }
    ui.run()
}
