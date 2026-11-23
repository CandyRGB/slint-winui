use slint::{
    ComponentHandle,
    platform::{Key, WindowEvent},
};

slint::slint! {
    import { SplitViewBehavior, SplitViewExample } from "ui/split_view.slint";
    export { SplitViewBehavior, SplitViewExample }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = SplitViewExample::new()?;
    let weak = ui.as_weak();
    ui.global::<SplitViewBehavior>()
        .on_traverse_focus(move |forward| {
            if let Some(ui) = weak.upgrade() {
                let key = if forward { Key::Tab } else { Key::Backtab };
                ui.window()
                    .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
            }
        });
    ui.run()
}
