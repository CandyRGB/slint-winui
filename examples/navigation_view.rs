#[path = "../src/behavior/navigation_view.rs"]
mod navigation_view;

slint::slint! {
    import {
        NavigationViewBehavior, NavigationViewDisplayMode, NavigationViewExample, NavigationViewItem,
        NavigationViewItemKind, NavigationViewLeftLayout, NavigationViewMeasurement, NavigationViewPaneDisplayMode,
        NavigationViewRow, NavigationViewTopContentWidths, NavigationViewTopLayout,
    } from "ui/navigation_view.slint";
    export {
        NavigationViewBehavior, NavigationViewDisplayMode, NavigationViewExample, NavigationViewItem,
        NavigationViewItemKind, NavigationViewLeftLayout, NavigationViewMeasurement, NavigationViewPaneDisplayMode,
        NavigationViewRow, NavigationViewTopContentWidths, NavigationViewTopLayout,
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = NavigationViewExample::new()?;
    navigation_view::install(window.global::<NavigationViewBehavior>(), &window);
    if std::env::args().any(|argument| argument == "--top") {
        window.set_mode(NavigationViewPaneDisplayMode::Top);
    }
    if std::env::args().any(|argument| argument == "--deep-menu") {
        window.set_deep_menu(true);
    }
    if std::env::args().any(|argument| argument == "--search") {
        window.set_with_search(true);
    }
    if std::env::args().any(|argument| argument == "--extended-title-bar") {
        window.set_title_bar_height(32.0);
    }
    if std::env::args().any(|argument| argument == "--slots") {
        window.set_with_slots(true);
    }
    window.run()
}
