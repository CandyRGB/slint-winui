use slint::ComponentHandle;

#[path = "../src/behavior/grid.rs"]
mod grid;

slint::slint! {
    import {
        GridExample, GridBehavior, GridChild, GridDefinition, GridLayoutResult, GridLengthKind,
        GridMeasureRequest, GridOptions, GridTrackPlacement, LayoutGeometry, LayoutPlacement,
        LayoutSize
    } from "ui/grid.slint";
    export {
        GridExample, GridBehavior, GridChild, GridDefinition, GridLayoutResult, GridLengthKind,
        GridMeasureRequest, GridOptions, GridTrackPlacement, LayoutGeometry, LayoutPlacement,
        LayoutSize
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = GridExample::new()?;
    grid::install(ui.global::<GridBehavior>(), |request| request.preferred);
    ui.run()
}
