use slint::ComponentHandle;

#[path = "../src/behavior/relative_panel.rs"]
mod relative_panel;

slint::slint! {
    import {
        LayoutGeometry, LayoutPlacement, LayoutSize,
        RelativePanelAlignment, RelativePanelAxis, RelativePanelBehavior, RelativePanelChild,
        RelativePanelExample, RelativePanelInsets, RelativePanelIssue, RelativePanelLayout
    } from "ui/relative_panel.slint";
    export {
        LayoutGeometry, LayoutPlacement, LayoutSize,
        RelativePanelAlignment, RelativePanelAxis, RelativePanelBehavior, RelativePanelChild,
        RelativePanelExample, RelativePanelInsets, RelativePanelIssue, RelativePanelLayout
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = RelativePanelExample::new()?;
    relative_panel::install(ui.global::<RelativePanelBehavior>());
    ui.run()
}
