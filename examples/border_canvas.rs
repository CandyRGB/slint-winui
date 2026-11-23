use slint::ComponentHandle;

slint::slint! {
    import { BorderCanvasExample } from "ui/border_canvas.slint";
    export { BorderCanvasExample }
}

fn main() -> Result<(), slint::PlatformError> {
    BorderCanvasExample::new()?.run()
}
