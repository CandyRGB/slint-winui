#[path = "support/media.rs"]
mod media;

use slint::{ComponentHandle, ModelRc, VecModel};

#[path = "../src/behavior/scroll_anchor.rs"]
mod scroll_anchor;
#[path = "../src/behavior/scroll_snap.rs"]
mod scroll_snap;

slint::slint! {
    import { ScrollPhoto, ScrollPhotoStrip, ScrollViewExample } from "ui/scroll_view.slint";
    import { ScrollAnchorBehavior, ScrollAnchorCandidate, ScrollAnchorLayout, ScrollAnchorRequest, ScrollAnchorResult }
        from "ui/components/scrolling/scroll_anchor.slint";
    import {
        ScrollSnapAlignment, ScrollSnapBehavior, ScrollSnapIntent, ScrollSnapPoint, ScrollSnapPoints,
        ScrollSnapRange, ScrollSnapRequest
    } from "ui/components/scrolling/scroll_snap.slint";
    export { ScrollPhoto, ScrollPhotoStrip, ScrollViewExample }
    export { ScrollAnchorBehavior, ScrollAnchorCandidate, ScrollAnchorLayout, ScrollAnchorRequest, ScrollAnchorResult }
    export { ScrollSnapAlignment, ScrollSnapBehavior, ScrollSnapIntent, ScrollSnapPoint, ScrollSnapPoints, ScrollSnapRange, ScrollSnapRequest }
}

fn load_strip<'a>(files: impl Iterator<Item = (&'a str, String)>) -> ScrollPhotoStrip {
    let photos = files
        .map(|(name, file)| {
            let image = media::sample_image(&file);
            let size = image.size();
            let height = if size.width > 0 {
                (200.0 * size.height as f32 / size.width as f32).round() * 2.0
            } else {
                240.0
            };
            ScrollPhoto {
                name: name.into(),
                image,
                height,
            }
        })
        .collect::<Vec<_>>();
    ScrollPhotoStrip {
        height: photos.iter().map(|photo| photo.height).sum(),
        photos: ModelRc::new(VecModel::from(photos)),
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = ScrollViewExample::new()?;
    scroll_anchor::install(window.global::<ScrollAnchorBehavior>());
    scroll_snap::install(window.global::<ScrollSnapBehavior>());
    if std::env::var("SLINT_BACKEND").is_ok_and(|backend| backend.contains("software")) {
        window.set_transform_content(false);
    }
    window.set_cliff(media::sample_image("cliff.jpg"));
    let landscape = [
        "leaves", "carousel", "bicycles", "pond", "marina", "beach", "rampart", "mountain",
    ];
    let velocity = ["grapes", "rainier", "sunset", "treetops", "valley", "cliff"];
    window.set_animation_strip(load_strip(
        landscape
            .into_iter()
            .enumerate()
            .map(|(index, name)| (name, format!("LandscapeImage{}.jpg", index + 1))),
    ));
    window.set_velocity_strip(load_strip(
        velocity
            .into_iter()
            .map(|name| (name, format!("{name}.jpg"))),
    ));
    window.run()
}
