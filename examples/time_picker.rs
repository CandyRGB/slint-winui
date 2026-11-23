#[path = "../src/behavior/time_picker.rs"]
mod time_picker;

use slint::{Color, ComponentHandle};
use slint_winui::platform::accent::light_accent_text_colors;

slint::slint! {
    import { PickerWheelRow, TimePickerBehavior, TimePickerColumn, TimePickerExample, TimePickerOptions, TimePickerPart, TimePickerState } from "ui/time_picker.slint";
    export { PickerWheelRow, TimePickerBehavior, TimePickerColumn, TimePickerExample, TimePickerOptions, TimePickerPart, TimePickerState }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = TimePickerExample::new()?;
    time_picker::install(ui.global::<TimePickerBehavior>());
    if let Ok(colors) = light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(Color::from_rgb_u8(red, green, blue));
    }
    ui.run()
}
