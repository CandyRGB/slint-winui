#[path = "../src/behavior/items_repeater.rs"]
mod items_repeater;

use std::{cell::Cell, rc::Rc};

use slint::{ComponentHandle, Model, ModelExt, ModelRc, VecModel};

slint::slint! {
    import { ItemsRepeaterBehavior, ItemsRepeaterExample, ItemsRepeaterGeometry, ItemsRepeaterJustification, ItemsRepeaterLayout, ItemsRepeaterLayoutOptions, ItemsRepeaterPlacement, ItemsRepeaterSize, ItemsRepeaterStretch, ItemsRepeaterWindow, SampleItem } from "ui/items_repeater.slint";
    export { ItemsRepeaterBehavior, ItemsRepeaterExample, ItemsRepeaterGeometry, ItemsRepeaterJustification, ItemsRepeaterLayout, ItemsRepeaterLayoutOptions, ItemsRepeaterPlacement, ItemsRepeaterSize, ItemsRepeaterStretch, ItemsRepeaterWindow, SampleItem }
}

fn set_layout_data(ui: &ItemsRepeaterExample, count: usize) {
    let mut sizes = Vec::with_capacity(count);
    let mut placements = Vec::with_capacity(count);
    for index in 0..count {
        let width = 96.0 + (index % 3) as f32 * 16.0;
        let height = 64.0 + (index % 2) as f32 * 8.0;
        sizes.push(ItemsRepeaterSize { width, height });
        placements.push(ItemsRepeaterPlacement {
            index: index as i32,
            x: (index % 3) as f32 * 136.0,
            y: (index / 3) as f32 * 96.0 + (index % 3) as f32 * 8.0,
            width,
            height,
            ..Default::default()
        });
    }
    ui.set_sizes(ModelRc::new(VecModel::from(sizes)));
    ui.set_custom_placements(ModelRc::new(VecModel::from(placements)));
}

fn item(index: usize, length: f32) -> SampleItem {
    SampleItem {
        label: format!("Item {index}").into(),
        length,
    }
}

fn set_sample(ui: &ItemsRepeaterExample, items: ModelRc<SampleItem>) {
    set_layout_data(ui, items.row_count());
    ui.set_keys(ModelRc::new(items.clone().map(|item| item.label)));
    ui.set_items(items);
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = ItemsRepeaterExample::new()?;
    items_repeater::install(ui.global::<ItemsRepeaterBehavior>());
    if let Ok([red, green, blue]) = slint_winui::platform::accent::system_accent_color() {
        ui.set_accent(slint::Color::from_rgb_u8(red, green, blue));
    }
    let bars = Rc::new(VecModel::from(vec![
        item(1, 300.0),
        item(2, 26.0),
        item(3, 176.0),
    ]));
    set_sample(&ui, ModelRc::from(bars.clone()));

    let weak = ui.as_weak();
    let model = bars.clone();
    let next_index = Cell::new(4);
    ui.on_add_item(move || {
        let index = next_index.get();
        next_index.set(index + 1);
        model.push(item(index, (24 + (index * 72) % 400) as f32));
        if let Some(ui) = weak.upgrade() {
            set_layout_data(&ui, model.row_count());
        }
    });
    let weak = ui.as_weak();
    let model = bars.clone();
    ui.on_remove_item(move || {
        if model.row_count() != 0 {
            model.remove(0);
        }
        if let Some(ui) = weak.upgrade() {
            set_layout_data(&ui, model.row_count());
        }
    });
    let weak = ui.as_weak();
    ui.on_load_sample(move |large| {
        if let Some(ui) = weak.upgrade() {
            let model = if large {
                ModelRc::new(VecModel::from(
                    (1..=10_000)
                        .map(|index| item(index, 0.0))
                        .collect::<Vec<_>>(),
                ))
            } else {
                ModelRc::from(bars.clone())
            };
            set_sample(&ui, model);
        }
    });
    ui.run()
}
