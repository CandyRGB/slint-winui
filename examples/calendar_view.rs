#[path = "../src/behavior/calendar_view.rs"]
mod calendar_view;

use slint::{Color, ComponentHandle};
use slint_winui::platform::{accent::light_accent_text_colors, calendar::today};

slint::slint! {
    import { CalendarDateText, CalendarViewBehavior, CalendarViewCell, CalendarViewDayName, CalendarViewDisplayMode, CalendarViewExample, CalendarViewOptions, CalendarViewPage, CalendarViewSelectionChange, CalendarViewSelectionMode } from "ui/calendar_view.slint";
    export { CalendarDateText, CalendarViewBehavior, CalendarViewCell, CalendarViewDayName, CalendarViewDisplayMode, CalendarViewExample, CalendarViewOptions, CalendarViewPage, CalendarViewSelectionChange, CalendarViewSelectionMode }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = CalendarViewExample::new()?;
    calendar_view::install(ui.global::<CalendarViewBehavior>());
    let date = today();
    ui.set_today(date);
    ui.set_demo_min(date - 4);
    ui.set_demo_max(date + 4);
    ui.set_demo_blackout(date - 1);
    if let Ok(colors) = light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(Color::from_rgb_u8(red, green, blue));
        let [red, green, blue] = colors.primary;
        ui.set_selection_ink(Color::from_rgb_u8(red, green, blue));
    }
    ui.run()
}
