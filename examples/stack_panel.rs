use slint::ComponentHandle;

#[path = "../src/behavior/scroll_snap.rs"]
mod scroll_snap;
#[path = "../src/behavior/stack_panel.rs"]
mod stack_panel;

slint::slint! {
    import {
        StackPanelExample, StackPanelBehavior, StackPanelChild, StackPanelOptions, StackPanelSnapAlignment,
        StackPanelSnapPoints, BorderInsets, ItemsRepeaterGeometry, ItemsRepeaterPlacement, ItemsRepeaterSize
    } from "ui/stack_panel.slint";
    import {
        ScrollSnapAlignment, ScrollSnapBehavior, ScrollSnapIntent, ScrollSnapPoint, ScrollSnapPoints,
        ScrollSnapRange, ScrollSnapRequest
    } from "ui/components/scrolling/scroll_snap.slint";
    export {
        StackPanelExample, StackPanelBehavior, StackPanelChild, StackPanelOptions, StackPanelSnapAlignment,
        StackPanelSnapPoints, BorderInsets, ItemsRepeaterGeometry, ItemsRepeaterPlacement, ItemsRepeaterSize
    }
    export { ScrollSnapAlignment, ScrollSnapBehavior, ScrollSnapIntent, ScrollSnapPoint, ScrollSnapPoints, ScrollSnapRange, ScrollSnapRequest }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = StackPanelExample::new()?;
    stack_panel::install(ui.global::<StackPanelBehavior>());
    scroll_snap::install(ui.global::<ScrollSnapBehavior>());
    ui.run()
}
