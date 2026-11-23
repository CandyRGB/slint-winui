#[path = "../src/behavior/pivot.rs"]
mod pivot;

use slint::ComponentHandle;

slint::slint! {
    import { PivotBehavior, PivotExample, PivotItem, PivotLayout, PivotMeasurement, PivotHeaderRow } from "ui/pivot.slint";
    export { PivotBehavior, PivotExample, PivotItem, PivotLayout, PivotMeasurement, PivotHeaderRow }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = PivotExample::new()?;
    pivot::install(ui.global::<PivotBehavior>());
    if let Ok(color) = slint_winui::platform::accent::system_accent_color() {
        ui.set_accent(slint::Color::from_rgb_u8(color[0], color[1], color[2]).into());
    }
    ui.run()
}
