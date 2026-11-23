#[path = "../src/behavior/teaching_tip.rs"]
mod teaching_tip;

use slint::ComponentHandle;

slint::slint! {
    import { TeachingTipExample, TeachingTipLayout, TeachingTipGeometry, TeachingTipSide, TeachingTipPlacementInput, TeachingTipPlacement, FlyoutRect } from "ui/teaching_tip.slint";
    export { TeachingTipExample, TeachingTipLayout, TeachingTipGeometry, TeachingTipSide, TeachingTipPlacementInput, TeachingTipPlacement, FlyoutRect }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = TeachingTipExample::new()?;
    teaching_tip::install(window.global::<TeachingTipLayout>());
    window.set_edge(std::env::args().any(|arg| arg == "--edge"));
    window.run()
}
