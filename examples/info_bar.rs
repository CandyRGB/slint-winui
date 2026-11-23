use slint::{Color, ComponentHandle};
use slint_winui::platform::accent::light_accent_text_colors;

slint::slint! {
    import { InfoBarExample, HyperlinkButtonTheme, HyperlinkButtonAccentColors } from "ui/info_bar.slint";
    export { InfoBarExample, HyperlinkButtonTheme, HyperlinkButtonAccentColors }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = InfoBarExample::new()?;
    if let Ok(colors) = light_accent_text_colors() {
        let to_color = |[r, g, b]: [u8; 3]| Color::from_rgb_u8(r, g, b);
        ui.global::<HyperlinkButtonTheme>()
            .set_light_accent_text(HyperlinkButtonAccentColors {
                primary: to_color(colors.primary),
                secondary: to_color(colors.secondary),
                tertiary: to_color(colors.tertiary),
            });
    }
    ui.run()
}
