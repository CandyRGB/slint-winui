#[path = "support/media.rs"]
mod media;

use slint_winui::platform::tool_tip::timing;

slint::slint! {
    import { ToolTipExample } from "ui/tool_tip.slint";
    export { ToolTipExample }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = ToolTipExample::new()?;
    let timing = timing();
    ui.set_system_show_delay(timing.show_delay.as_millis() as i64);
    ui.set_system_show_duration(timing.show_duration.as_millis() as i64);
    ui.set_gallery_photo(media::sample_image("cliff.jpg"));
    ui.run()
}
