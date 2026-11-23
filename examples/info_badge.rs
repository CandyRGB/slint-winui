#[path = "../src/behavior/navigation_view.rs"]
mod navigation_view;

slint::slint! {
    import {
        NavigationViewBehavior, NavigationViewDisplayMode, InfoBadgeExample, NavigationViewItem,
        NavigationViewItemKind, NavigationViewLeftLayout, NavigationViewMeasurement, NavigationViewPaneDisplayMode,
        NavigationViewRow, NavigationViewTopContentWidths, NavigationViewTopLayout,
    } from "ui/info_badge.slint";
    export {
        NavigationViewBehavior, NavigationViewDisplayMode, InfoBadgeExample, NavigationViewItem,
        NavigationViewItemKind, NavigationViewLeftLayout, NavigationViewMeasurement, NavigationViewPaneDisplayMode,
        NavigationViewRow, NavigationViewTopContentWidths, NavigationViewTopLayout,
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = InfoBadgeExample::new()?;
    navigation_view::install(window.global::<NavigationViewBehavior>(), &window);
    window.run()
}
