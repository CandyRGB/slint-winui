slint::slint! {
    import { ProgressRingExample } from "ui/progress_ring.slint";
    export { ProgressRingExample }
}

fn main() -> Result<(), slint::PlatformError> {
    ProgressRingExample::new()?.run()
}
