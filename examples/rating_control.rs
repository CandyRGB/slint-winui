slint::slint! {
    import { RatingControlExample } from "ui/rating_control.slint";
    export { RatingControlExample }
}

fn main() -> Result<(), slint::PlatformError> {
    RatingControlExample::new()?.run()
}
