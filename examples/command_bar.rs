#[path = "../src/behavior/command_bar.rs"]
mod command_bar;
#[path = "../src/behavior/menu_flyout.rs"]
mod menu_flyout;

slint::slint! {
    import { AppBarButtonsExample } from "ui/app_bar_button.slint";
    import {
        AppBarLabelPosition, CommandBarBehavior, CommandBarEntry, CommandBarExample, CommandBarItem,
        CommandBarItemKind, CommandBarLayout, CommandBarOverflowButtonVisibility,
        MenuFlyoutBehavior, MenuFlyoutItem, MenuFlyoutItemKind, MenuFlyoutLevel,
    } from "ui/command_bar.slint";
    export {
        AppBarButtonsExample,
        AppBarLabelPosition, CommandBarBehavior, CommandBarEntry, CommandBarExample, CommandBarItem,
        CommandBarItemKind, CommandBarLayout, CommandBarOverflowButtonVisibility,
        MenuFlyoutBehavior, MenuFlyoutItem, MenuFlyoutItemKind, MenuFlyoutLevel,
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|argument| argument == "--app-bar-buttons") {
        return AppBarButtonsExample::new()?.run();
    }
    let window = CommandBarExample::new()?;
    command_bar::install(window.global::<CommandBarBehavior>());
    menu_flyout::install(window.global::<MenuFlyoutBehavior>());
    window.set_narrow(args.iter().any(|argument| argument == "--narrow"));
    window.set_bottom(args.iter().any(|argument| argument == "--bottom"));
    window.set_ordered(args.iter().any(|argument| argument == "--ordered"));
    window.run()
}
