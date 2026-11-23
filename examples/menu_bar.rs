#[path = "../src/behavior/menu_flyout.rs"]
mod menu_flyout;

slint::slint! {
    import { MenuBarExample, MenuFlyoutBehavior, MenuFlyoutItem, MenuFlyoutItemKind, MenuFlyoutLevel }
        from "ui/menu_bar.slint";
    export { MenuBarExample, MenuFlyoutBehavior, MenuFlyoutItem, MenuFlyoutItemKind, MenuFlyoutLevel }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = MenuBarExample::new()?;
    menu_flyout::install(ui.global::<MenuFlyoutBehavior>());
    ui.run()
}
