#[path = "../src/behavior/date_picker.rs"]
mod date_picker;

use slint::{Color, ComponentHandle};
use slint_winui::platform::{
    accent::light_accent_text_colors,
    calendar::{date_key, today},
};

slint::slint! {
    import { DatePickerBehavior, DatePickerColumn, DatePickerExample, DatePickerOptions, DatePickerPart, DatePickerState, PickerWheelRow } from "ui/date_picker.slint";
    export { DatePickerBehavior, DatePickerColumn, DatePickerExample, DatePickerOptions, DatePickerPart, DatePickerState, PickerWheelRow }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = DatePickerExample::new()?;
    date_picker::install(ui.global::<DatePickerBehavior>());
    ui.set_today(today());
    ui.set_leap_date(date_key(2028, 2, 29).unwrap());
    ui.set_minimum(date_key(2026, 7, 1).unwrap());
    ui.set_maximum(date_key(2028, 7, 1).unwrap());
    if let Ok(colors) = light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(Color::from_rgb_u8(red, green, blue));
    }
    ui.run()
}
