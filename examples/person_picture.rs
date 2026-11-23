#[path = "../src/behavior/person_picture.rs"]
mod person_picture;

use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer};

slint::slint! {
    import { PersonPictureBehavior, PersonPictureExample } from "ui/person_picture.slint";
    export { PersonPictureBehavior, PersonPictureExample }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = PersonPictureExample::new()?;
    person_picture::install(window.global::<PersonPictureBehavior>());
    // The landscape fixture makes an incorrect square crop or stretched photo visible.
    window.set_profile_photo(sample(360, 220, false));
    window.set_small_photo(sample(96, 60, true));
    window.set_badge_photo(sample(64, 88, true));
    window.run()
}

fn sample(width: u32, height: u32, warm: bool) -> Image {
    let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
    for (index, pixel) in pixels.make_mut_slice().iter_mut().enumerate() {
        let x = (index % width as usize) as f32 / width as f32;
        let y = (index / width as usize) as f32 / height as f32;
        let sky = if warm { (232, 124, 79) } else { (83, 162, 217) };
        let ridge = 0.63 - (x * 11.0).sin() * 0.10 - (x * 23.0).sin() * 0.04;
        let color = if y > ridge {
            if y > ridge + 0.16 {
                (29, 78, 79)
            } else {
                (72, 128, 125)
            }
        } else if ((x - 0.58) * width as f32 / height as f32).hypot(y - 0.38) < 0.12 {
            (255, 232, 168)
        } else {
            sky
        };
        *pixel = Rgba8Pixel {
            r: color.0,
            g: color.1,
            b: color.2,
            a: 255,
        };
    }
    Image::from_rgba8(pixels)
}
