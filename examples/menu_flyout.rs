#[path = "../src/behavior/menu_flyout.rs"]
mod menu_flyout;

slint::slint! {
    import { MenuFlyoutExample, MenuFlyoutBehavior, MenuFlyoutItem, MenuFlyoutItemKind, MenuFlyoutLevel }
        from "ui/menu_flyout.slint";
    export { MenuFlyoutExample, MenuFlyoutBehavior, MenuFlyoutItem, MenuFlyoutItemKind, MenuFlyoutLevel }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = MenuFlyoutExample::new()?;
    menu_flyout::install(window.global::<MenuFlyoutBehavior>());
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    window.set_dark(arguments.iter().any(|arg| arg == "--dark"));
    window.set_compact(arguments.iter().any(|arg| arg == "--compact"));
    window.set_edge(arguments.iter().any(|arg| arg == "--edge"));
    window.set_scoped_shortcuts(arguments.iter().any(|arg| arg == "--scoped-shortcuts"));
    window.run()
}
