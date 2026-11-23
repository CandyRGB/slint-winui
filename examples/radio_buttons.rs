#[path = "../src/behavior/radio_buttons.rs"]
mod radio_buttons;

use slint::{Color, ComponentHandle, Model, ModelRc, VecModel};
use slint_winui::platform::accent::light_accent_text_colors;

slint::slint! {
    import { RadioButtonsBehavior, RadioButtonsExample, RadioButtonsItem } from "ui/radio_buttons.slint";
    export { RadioButtonsBehavior, RadioButtonsExample, RadioButtonsItem }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = RadioButtonsExample::new()?;
    radio_buttons::install(ui.global::<RadioButtonsBehavior>());
    if let Ok(colors) = light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(Color::from_rgb_u8(red, green, blue));
    }

    let weak = ui.as_weak();
    ui.on_move_selection_to_end(move || {
        if let Some(ui) = weak.upgrade() {
            let mut items: Vec<_> = ui.get_items().iter().collect();
            if let Some(index) = items
                .iter()
                .position(|item| item.id == ui.get_selected_id())
            {
                let item = items.remove(index);
                items.push(item);
                ui.set_items(ModelRc::new(VecModel::from(items)));
            }
        }
    });
    let weak = ui.as_weak();
    ui.on_remove_selected(move || {
        if let Some(ui) = weak.upgrade() {
            let items: Vec<_> = ui
                .get_items()
                .iter()
                .filter(|item| item.id != ui.get_selected_id())
                .collect();
            ui.set_items(ModelRc::new(VecModel::from(items)));
        }
    });

    ui.run()
}
