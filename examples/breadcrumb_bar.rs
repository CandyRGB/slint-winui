#[path = "../src/behavior/breadcrumb_bar.rs"]
mod breadcrumb_bar;

use slint::{ComponentHandle, Model, ModelRc, VecModel};

slint::slint! {
    import { BreadcrumbBarExample, BreadcrumbBarBehavior, BreadcrumbLayout, BreadcrumbMeasurement, BreadcrumbRow, Crumb } from "ui/breadcrumb_bar.slint";
    export { BreadcrumbBarExample, BreadcrumbBarBehavior, BreadcrumbLayout, BreadcrumbMeasurement, BreadcrumbRow, Crumb }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = BreadcrumbBarExample::new()?;
    breadcrumb_bar::install(window.global::<BreadcrumbBarBehavior>());
    let weak = window.as_weak();
    window.on_navigate(move |index| {
        if let Some(window) = weak.upgrade() {
            let path = window
                .get_path()
                .iter()
                .take(index as usize + 1)
                .collect::<Vec<_>>();
            window.set_path(ModelRc::new(VecModel::from(path)));
        }
    });
    window.run()
}
