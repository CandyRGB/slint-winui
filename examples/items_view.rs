#[path = "../src/behavior/items_repeater.rs"]
mod items_repeater;
#[path = "../src/behavior/items_view.rs"]
mod items_view;

#[path = "support/media.rs"]
mod media;

use std::rc::Rc;

use slint::{Color, ComponentHandle, Image, Model, ModelRc, VecModel};

slint::slint! {
    import { GalleryPhoto, ItemsViewExample, ItemsViewBehavior, ItemsViewItem, ItemsViewNavigation, ItemsViewSelectionAction, ItemsViewSelectionMode, ItemsRepeaterBehavior, ItemsRepeaterGeometry, ItemsRepeaterJustification, ItemsRepeaterLayout, ItemsRepeaterLayoutOptions, ItemsRepeaterPlacement, ItemsRepeaterSize, ItemsRepeaterStretch, ItemsRepeaterWindow } from "ui/items_view.slint";
    export { ItemsViewExample, ItemsViewBehavior, ItemsViewItem, ItemsViewNavigation, ItemsViewSelectionAction, ItemsViewSelectionMode, ItemsRepeaterBehavior, ItemsRepeaterGeometry, ItemsRepeaterJustification, ItemsRepeaterLayout, ItemsRepeaterLayoutOptions, ItemsRepeaterPlacement, ItemsRepeaterSize, ItemsRepeaterStretch, ItemsRepeaterWindow }
}

fn load_photos(ui: &ItemsViewExample, samples: &[Image], count: usize) {
    let mut photos = Vec::with_capacity(count);
    let mut sizes = Vec::with_capacity(count);
    let mut items = Vec::with_capacity(count);
    for index in 0..count {
        let image = samples[index % samples.len()].clone();
        let size = image.size();
        sizes.push(ItemsRepeaterSize {
            width: if size.width == 0 {
                136.0 + (index % 5) as f32 * 22.0
            } else {
                size.width as f32
            },
            height: if size.height == 0 {
                160.0
            } else {
                size.height as f32
            },
        });
        photos.push(GalleryPhoto {
            title: format!("Item {}", index + 1).into(),
            likes: (index % 173) as i32,
            image,
        });
        items.push(ItemsViewItem {
            key: format!("photo-{}", index + 1).into(),
            enabled: true,
        });
    }
    let items = ModelRc::new(VecModel::from(items));
    let shrinking = ui.get_items().row_count() > count;
    if shrinking {
        ui.set_items(items.clone());
    }
    ui.set_photo_sizes(ModelRc::new(VecModel::from(sizes)));
    ui.set_photos(ModelRc::new(VecModel::from(photos)));
    if !shrinking {
        ui.set_items(items);
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = ItemsViewExample::new()?;
    items_view::install(ui.global::<ItemsViewBehavior>());
    items_repeater::install(ui.global::<ItemsRepeaterBehavior>());
    if let Ok(colors) = slint_winui::platform::accent::light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(Color::from_rgb_u8(red, green, blue));
    }
    let images = Rc::new(
        (1..=8)
            .map(|index| media::sample_image(&format!("LandscapeImage{index}.jpg")))
            .collect::<Vec<_>>(),
    );
    load_photos(&ui, &images, 40);
    let weak = ui.as_weak();
    ui.on_load_items(move |large| {
        if let Some(ui) = weak.upgrade() {
            load_photos(&ui, &images, if large { 10_000 } else { 40 });
        }
    });
    ui.run()
}
