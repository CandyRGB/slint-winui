slint::slint! {
    import { RepeatButtonExample } from "ui/repeat_button.slint";
    export { RepeatButtonExample }
}

fn main() -> Result<(), slint::PlatformError> {
    RepeatButtonExample::new()?.run()
}
