#[path = "../src/behavior/menu_flyout.rs"]
mod menu_flyout;

slint::slint! {
    import { FlyoutButtonsExample, MenuFlyoutBehavior, MenuFlyoutItem, MenuFlyoutItemKind, MenuFlyoutLevel }
        from "ui/flyout_buttons.slint";
    export { FlyoutButtonsExample, MenuFlyoutBehavior, MenuFlyoutItem, MenuFlyoutItemKind, MenuFlyoutLevel }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = FlyoutButtonsExample::new()?;
    menu_flyout::install(window.global::<MenuFlyoutBehavior>());
    window.set_dark(std::env::args().any(|argument| argument == "--dark"));
    window.run()
}
