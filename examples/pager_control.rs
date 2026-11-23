#[path = "../src/behavior/combo_box.rs"]
mod combo_box;
#[path = "../src/behavior/number_box.rs"]
mod number_box;
#[path = "../src/behavior/pager_control.rs"]
mod pager_control;

use slint::{
    ComponentHandle,
    platform::{Key, WindowEvent},
};

slint::slint! {
    import { PagerControlExample, PagerControlBehavior, ComboBoxBehavior, ComboBoxItem, NumberBoxMath, NumberBoxParseResult, NumberBoxValue } from "ui/pager_control.slint";
    export { PagerControlExample, PagerControlBehavior, ComboBoxBehavior, ComboBoxItem, NumberBoxMath, NumberBoxParseResult, NumberBoxValue }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = PagerControlExample::new()?;
    combo_box::install(ui.global::<ComboBoxBehavior>());
    number_box::install(ui.global::<NumberBoxMath>());
    pager_control::install(ui.global::<PagerControlBehavior>());
    let weak = ui.as_weak();
    ui.global::<PagerControlBehavior>()
        .on_traverse_focus(move |forward| {
            if let Some(ui) = weak.upgrade() {
                let key = if forward { Key::Tab } else { Key::Backtab };
                ui.window()
                    .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
            }
        });
    if let Ok(colors) = slint_winui::platform::accent::light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(slint::Color::from_rgb_u8(red, green, blue).into());
    }
    ui.run()
}
