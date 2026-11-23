#[path = "../src/behavior/grid_view.rs"]
mod grid_view;

#[path = "support/media.rs"]
mod media;

use std::rc::Rc;

use slint::{Color, ComponentHandle, ModelRc, VecModel};
use slint_winui::platform::accent::light_accent_text_colors;

slint::slint! {
    import { GridViewBehavior, GridViewExample, GridViewItem, GridViewSelectionMode } from "ui/grid_view.slint";
    export { GridViewBehavior, GridViewExample, GridViewItem, GridViewSelectionMode }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = GridViewExample::new()?;
    grid_view::install(ui.global::<GridViewBehavior>());
    if let Ok(colors) = light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(Color::from_rgb_u8(red, green, blue));
    }
    let photos = Rc::new(
        (1..=8)
            .map(|index| GridViewItem {
                id: format!("photo-{index}").into(),
                title: format!("Item {index}").into(),
                description: format!("Landscape {index}").into(),
                image: media::sample_image(&format!("LandscapeImage{index}.jpg")),
                disabled: index == 8,
            })
            .collect::<Vec<_>>(),
    );
    ui.set_items(ModelRc::new(VecModel::from(photos.as_ref().clone())));

    let weak = ui.as_weak();
    ui.on_load_sample(move |large| {
        if let Some(ui) = weak.upgrade() {
            let items = if large {
                (1..=10_000)
                    .map(|index| GridViewItem {
                        id: format!("tile-{index}").into(),
                        title: format!("Tile {index}").into(),
                        ..Default::default()
                    })
                    .collect()
            } else {
                photos.as_ref().clone()
            };
            ui.set_items(ModelRc::new(VecModel::from(items)));
        }
    });
    ui.run()
}
