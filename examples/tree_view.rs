#[path = "../src/behavior/tree_view.rs"]
mod tree_view;

use slint::{ComponentHandle, Model, ModelRc, VecModel};

slint::slint! {
    import { TreeViewExample, TreeViewBehavior, TreeViewItem, TreeViewRow, TreeViewSelectionMode, TreeViewCheckState, TreeViewDropMode } from "ui/tree_view.slint";
    export { TreeViewExample, TreeViewBehavior, TreeViewItem, TreeViewRow, TreeViewSelectionMode, TreeViewCheckState, TreeViewDropMode }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = TreeViewExample::new()?;
    tree_view::install(window.global::<TreeViewBehavior>());
    let weak = window.as_weak();
    window.on_load_children(move |id| {
        if let Some(window) = weak.upgrade() {
            let mut nodes: Vec<_> = window.get_nodes().iter().collect();
            if let Some(item) = nodes
                .iter_mut()
                .find(|item| item.id == id && item.has_unrealized_children)
            {
                item.has_unrealized_children = false;
                nodes.push(TreeViewItem {
                    id: "package".into(),
                    parent_id: id,
                    label: "Package".into(),
                    ..Default::default()
                });
                window.set_nodes(ModelRc::new(VecModel::from(nodes)));
            }
        }
    });
    let weak = window.as_weak();
    window.on_load_large_tree(move || {
        if let Some(window) = weak.upgrade() {
            let nodes: Vec<_> = (1..=10_000)
                .map(|i| TreeViewItem {
                    id: i.to_string().into(),
                    label: format!("Row {i}").into(),
                    ..Default::default()
                })
                .collect();
            window.set_nodes(ModelRc::new(VecModel::from(nodes)));
        }
    });
    window.run()
}
