#[path = "support/media.rs"]
mod media;

use slint::ComponentHandle;

slint::slint! {
    import { ViewboxExample, ViewboxLayout, ViewboxStretch, ViewboxStretchDirection, ViewboxScale, ViewboxAlignment } from "ui/viewbox.slint";
    export { ViewboxExample, ViewboxLayout, ViewboxStretch, ViewboxStretchDirection, ViewboxScale, ViewboxAlignment }
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = ViewboxExample::new()?;
    if std::env::var("SLINT_BACKEND").is_ok_and(|backend| backend.contains("software")) {
        ui.set_transform_content(false);
    }
    ui.set_slices(media::sample_image("Slices.png"));
    ui.run()
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use slint::{
        PlatformError,
        platform::{
            Platform, WindowAdapter, set_platform,
            software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
        },
    };

    use super::*;

    struct HeadlessPlatform;

    impl Platform for HeadlessPlatform {
        fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
            Ok(MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer))
        }
    }

    #[test]
    fn stretch_direction_unbounded_axes_and_alignment_match_viewbox_rules() {
        set_platform(Box::new(HeadlessPlatform)).unwrap();
        let ui = ViewboxExample::new().unwrap();
        let viewbox = ui.global::<ViewboxLayout>();
        use ViewboxStretch::{Fill, None, Uniform, UniformToFill};
        use ViewboxStretchDirection::{Both, DownOnly, UpOnly};
        let scale = |w, h, stretch, direction| {
            let scale = viewbox.invoke_scale(w, h, 100.0, 50.0, stretch, direction);
            (scale.x, scale.y)
        };

        assert_eq!(scale(50.0, 100.0, None, Both), (1.0, 1.0));
        assert_eq!(scale(50.0, 100.0, Fill, Both), (0.5, 2.0));
        assert_eq!(scale(50.0, 100.0, Uniform, Both), (0.5, 0.5));
        assert_eq!(scale(50.0, 100.0, UniformToFill, Both), (2.0, 2.0));
        assert_eq!(scale(50.0, 100.0, Fill, UpOnly), (1.0, 2.0));
        assert_eq!(scale(50.0, 100.0, Fill, DownOnly), (0.5, 1.0));
        assert_eq!(scale(50.0, -1.0, Fill, Both), (0.5, 0.5));
        assert_eq!(scale(-1.0, 100.0, Fill, Both), (2.0, 2.0));
        assert_eq!(scale(-1.0, -1.0, UniformToFill, Both), (1.0, 1.0));
        let empty = viewbox.invoke_scale(100.0, 100.0, 0.0, 50.0, Fill, Both);
        assert_eq!((empty.x, empty.y), (0.0, 2.0));

        use ViewboxAlignment::{Center, End, Stretch};
        assert_eq!(viewbox.invoke_offset(50.0, 100.0, Stretch), 0.0);
        assert_eq!(viewbox.invoke_offset(50.0, 100.0, Center), -24.0);
        assert_eq!(viewbox.invoke_offset(50.0, 100.0, End), -50.0);
        assert_eq!(viewbox.invoke_offset(50.0, 40.0, Stretch), 6.0);
    }
}
