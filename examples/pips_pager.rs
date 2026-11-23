slint::slint! {
    import { PipsPagerExample } from "ui/pips_pager.slint";
    export { PipsPagerExample }
}

fn main() -> Result<(), slint::PlatformError> {
    PipsPagerExample::new()?.run()
}
