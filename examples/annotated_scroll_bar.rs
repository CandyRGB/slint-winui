#[path = "../src/behavior/annotated_scroll_bar.rs"]
mod annotated_scroll_bar;

use slint::ComponentHandle;

slint::slint! {
    import { AnnotatedScrollBarBehavior, AnnotatedScrollBarExample, AnnotatedScrollBarLabel, AnnotatedScrollBarMarker, AnnotatedScrollBarScrollingKind } from "ui/annotated_scroll_bar.slint";
    export { AnnotatedScrollBarBehavior, AnnotatedScrollBarExample, AnnotatedScrollBarLabel, AnnotatedScrollBarMarker, AnnotatedScrollBarScrollingKind }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AnnotatedScrollBarExample::new()?;
    annotated_scroll_bar::install(ui.global::<AnnotatedScrollBarBehavior>());
    if let Ok(colors) = slint_winui::platform::accent::light_accent_text_colors() {
        let [red, green, blue] = colors.tertiary;
        ui.set_accent(slint::Color::from_rgb_u8(red, green, blue).into());
    }
    ui.run()
}
