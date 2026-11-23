use std::path::PathBuf;

use slint::{Color, ComponentHandle, Image, Model, ModelRc, VecModel};
use slint_winui::platform::accent::light_accent_text_colors;

slint::slint! {
    import { SwipeControlExample } from "ui/swipe_control.slint";
    export { SwipeControlExample }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = SwipeControlExample::new()?;
    if let Ok(colors) = light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(Color::from_rgb_u8(red, green, blue));
    }
    if std::env::args().any(|arg| arg == "--scroll-list") {
        ui.set_initial_rows(ModelRc::new(VecModel::from_iter(1..=12)));
    }
    let coffee = std::env::var_os("SLINT_WINUI_SAMPLE_MEDIA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("demo-assets"))
        .join("CoffeeCup.png");
    if let Ok(icon) = Image::load_from_path(&coffee) {
        ui.set_coffee_icon(icon);
    }
    let weak = ui.as_weak();
    ui.on_remove_row(move |removed| {
        if let Some(ui) = weak.upgrade() {
            let rows = ui.get_rows();
            ui.set_rows(ModelRc::new(VecModel::from_iter(
                rows.iter().filter(|&row| row != removed),
            )));
        }
    });
    ui.run()
}
