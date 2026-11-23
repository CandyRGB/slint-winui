#[path = "../src/behavior/tab_view.rs"]
mod tab_view;

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

slint::slint! {
    import { TabViewBehavior, TabViewExample, TabViewItem, TabViewLayout, TabViewMeasurement, TabViewRow, TabViewWidthMode } from "ui/tab_view.slint";
    export { TabViewBehavior, TabViewExample, TabViewItem, TabViewLayout, TabViewMeasurement, TabViewRow, TabViewWidthMode }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = TabViewExample::new()?;
    tab_view::install(window.global::<TabViewBehavior>());
    window.set_narrow(std::env::args().any(|arg| arg == "--narrow"));
    window.set_slots_visible(std::env::args().any(|arg| arg == "--slots"));

    let documents = Rc::new(RefCell::new(HashMap::<SharedString, SharedString>::new()));
    let copy = documents.clone();
    window.on_load_document(move |id| copy.borrow().get(&id).cloned().unwrap_or_default());
    let copy = documents.clone();
    window.on_save_document(move |id, text| {
        copy.borrow_mut().insert(id, text);
    });

    let weak = window.as_weak();
    let next = Cell::new(1);
    window.on_add(move || {
        if let Some(window) = weak.upgrade() {
            let number = next.get();
            next.set(number + 1);
            let id: SharedString = format!("new-{number}").into();
            let mut tabs: Vec<_> = window.get_tabs().iter().collect();
            tabs.push(TabViewItem {
                id: id.clone(),
                label: format!("New Tab {number}").into(),
                icon: tabs.first().map(|tab| tab.icon.clone()).unwrap_or_default(),
                ..Default::default()
            });
            window.set_tabs(ModelRc::new(VecModel::from(tabs)));
            window.set_selection(id);
            window.set_last_action(format!("Added tab {number}").into());
        }
    });

    let weak = window.as_weak();
    window.on_close_tab(move |id| {
        if let Some(window) = weak.upgrade() {
            documents.borrow_mut().remove(&id);
            let tabs: Vec<_> = window
                .get_tabs()
                .iter()
                .filter(|item| item.id != id)
                .collect();
            window.set_tabs(ModelRc::new(VecModel::from(tabs)));
            window.set_last_action(format!("Closed: {id}").into());
        }
    });

    let weak = window.as_weak();
    window.on_reorder(move |id, destination| {
        if let Some(window) = weak.upgrade() {
            let mut tabs: Vec<_> = window.get_tabs().iter().collect();
            if let Some(index) = tabs.iter().position(|item| item.id == id) {
                let item = tabs.remove(index);
                tabs.insert((destination.max(0) as usize).min(tabs.len()), item);
                window.set_tabs(ModelRc::new(VecModel::from(tabs)));
                window
                    .set_last_action(format!("Moved {id} to position {}", destination + 1).into());
            }
        }
    });
    window.run()
}
