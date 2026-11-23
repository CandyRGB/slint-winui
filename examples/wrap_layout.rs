use slint::ComponentHandle;

#[path = "../src/behavior/wrap_layout.rs"]
mod wrap_layout;

slint::slint! {
    import {
        ItemsRepeaterGeometry, ItemsRepeaterPlacement, ItemsRepeaterSize,
        VariableSizedWrapGridOptions, WrapGridAlignment, WrapLayoutBehavior, WrapLayoutChild,
        WrapLayoutExample, WrapPanelItemsStretch, WrapPanelOptions, WrapPanelPadding
    } from "ui/wrap_layout.slint";
    export {
        ItemsRepeaterGeometry, ItemsRepeaterPlacement, ItemsRepeaterSize,
        VariableSizedWrapGridOptions, WrapGridAlignment, WrapLayoutBehavior, WrapLayoutChild,
        WrapLayoutExample, WrapPanelItemsStretch, WrapPanelOptions, WrapPanelPadding
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = WrapLayoutExample::new()?;
    wrap_layout::install(ui.global::<WrapLayoutBehavior>());
    if let Ok(colors) = slint_winui::platform::accent::light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(slint::Color::from_rgb_u8(red, green, blue).into());
    }
    ui.run()
}
