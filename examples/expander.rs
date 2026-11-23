slint::slint! {
    import { ExpanderExample } from "ui/expander.slint";
    export { ExpanderExample }
}

fn main() -> Result<(), slint::PlatformError> {
    ExpanderExample::new()?.run()
}
