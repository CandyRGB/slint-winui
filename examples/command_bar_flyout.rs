#[path = "../src/behavior/command_bar.rs"]
mod command_bar;
#[path = "../src/behavior/menu_flyout.rs"]
mod menu_flyout;

slint::slint! {
    import {
        AppBarLabelPosition, CommandBarBehavior, CommandBarEntry, CommandBarFlyoutExample, CommandBarItem,
        CommandBarItemKind, CommandBarLayout, CommandBarOverflowButtonVisibility,
        MenuFlyoutBehavior, MenuFlyoutItem, MenuFlyoutItemKind, MenuFlyoutLevel,
    } from "ui/command_bar_flyout.slint";
    export {
        AppBarLabelPosition, CommandBarBehavior, CommandBarEntry, CommandBarFlyoutExample, CommandBarItem,
        CommandBarItemKind, CommandBarLayout, CommandBarOverflowButtonVisibility,
        MenuFlyoutBehavior, MenuFlyoutItem, MenuFlyoutItemKind, MenuFlyoutLevel,
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = CommandBarFlyoutExample::new()?;
    command_bar::install(window.global::<CommandBarBehavior>());
    menu_flyout::install(window.global::<MenuFlyoutBehavior>());
    let args: Vec<_> = std::env::args().skip(1).collect();
    window.set_edge(args.iter().any(|argument| argument == "--edge"));
    window.set_always_expanded(args.iter().any(|argument| argument == "--always-expanded"));
    window.set_secondary_only(args.iter().any(|argument| argument == "--secondary-only"));
    window.set_primary_only(args.iter().any(|argument| argument == "--primary-only"));
    window.set_pointer_dismiss(args.iter().any(|argument| argument == "--pointer-dismiss"));
    window.run()
}
