#[path = "../src/behavior/items_repeater.rs"]
mod items_repeater;
#[path = "../src/behavior/items_view.rs"]
mod items_view;
#[path = "../src/behavior/list_view.rs"]
mod list_view;

use slint::{Color, ComponentHandle, ModelRc, VecModel};

slint::slint! {
    import { ListViewExample, ListViewBehavior, ListViewEntry, ListViewHeaderPosition, ItemsViewBehavior, ItemsViewItem, ItemsViewNavigation, ItemsViewSelectionAction, ItemsViewSelectionMode, ItemsRepeaterBehavior, ItemsRepeaterGeometry, ItemsRepeaterJustification, ItemsRepeaterLayout, ItemsRepeaterLayoutOptions, ItemsRepeaterPlacement, ItemsRepeaterSize, ItemsRepeaterStretch, ItemsRepeaterWindow } from "ui/list_view.slint";
    export { ListViewExample, ListViewBehavior, ListViewEntry, ListViewHeaderPosition, ItemsViewBehavior, ItemsViewItem, ItemsViewNavigation, ItemsViewSelectionAction, ItemsViewSelectionMode, ItemsRepeaterBehavior, ItemsRepeaterGeometry, ItemsRepeaterJustification, ItemsRepeaterLayout, ItemsRepeaterLayoutOptions, ItemsRepeaterPlacement, ItemsRepeaterSize, ItemsRepeaterStretch, ItemsRepeaterWindow }
}

fn contacts(grouped: bool, large: bool) -> ModelRc<ListViewEntry> {
    let names = [
        ("Kendall", "Collins", "Fabrikam"),
        ("Henry", "Ross", "Contoso"),
        ("Vance", "DeLeon", "Northwind"),
        ("Victoria", "Burke", "Adventure Works"),
        ("Amber", "Rodriguez", "Fabrikam"),
        ("Amari", "Rivera", "Contoso"),
        ("Jessie", "Irwin", "Northwind"),
        ("Quinn", "Campbell", "Adventure Works"),
    ];
    let count = if large { 10_000 } else { 48 };
    let mut entries = (0..count)
        .map(|index| {
            let (first, last, company) = names[index % names.len()];
            let label = if index < names.len() {
                format!("{first} {last}")
            } else {
                format!("{first} {last} {}", index + 1)
            };
            ListViewEntry {
                key: format!("contact-{}", index + 1).into(),
                label: label.into(),
                detail: company.into(),
                group: if grouped {
                    last[..1].into()
                } else {
                    Default::default()
                },
                enabled: index != 7,
                height: 0.0,
            }
        })
        .collect::<Vec<_>>();
    if grouped {
        entries.sort_by(|left, right| left.group.cmp(&right.group));
    }
    ModelRc::new(VecModel::from(entries))
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = ListViewExample::new()?;
    items_repeater::install(ui.global::<ItemsRepeaterBehavior>());
    items_view::install(ui.global::<ItemsViewBehavior>());
    list_view::install(ui.global::<ListViewBehavior>());
    if let Ok(colors) = slint_winui::platform::accent::light_accent_text_colors() {
        let [r, g, b] = colors.tertiary;
        ui.set_accent(Color::from_rgb_u8(r, g, b));
    }
    ui.set_contacts(contacts(false, false));
    let weak = ui.as_weak();
    ui.on_load_contacts(move |grouped, large| {
        if let Some(ui) = weak.upgrade() {
            ui.set_contacts(contacts(grouped, large));
        }
    });
    ui.run()
}
