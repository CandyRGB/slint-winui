use std::path::PathBuf;

use slint::{Image, Rgb8Pixel, SharedPixelBuffer};

pub fn sample_image(name: &str) -> Image {
    let directory = std::env::var_os("SLINT_WINUI_SAMPLE_MEDIA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("demo-assets"));
    if let Ok(image) = Image::load_from_path(&directory.join(name)) {
        return image;
    }
    landscape(name)
}

fn landscape(name: &str) -> Image {
    let mut seed = 0_u32;
    for byte in name.bytes() {
        seed = seed.wrapping_mul(31).wrapping_add(u32::from(byte));
    }
    let width = 960;
    let height = if name == "cliff.jpg" { 608 } else { 640 };
    let mut image = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
    let sun_horizontal = 0.2 + 0.6 * (seed % 67) as f32 / 66.0;
    let tint = (seed % 7) as u8 * 7;
    for (index, pixel) in image.make_mut_slice().iter_mut().enumerate() {
        let x = (index as u32 % width) as f32 / width as f32;
        let y = (index as u32 / width) as f32 / height as f32;
        let ridge = 0.56 + 0.09 * (13.0 * x + seed as f32 % 6.0).sin() + 0.035 * (27.0 * x).sin();
        let foreground = 0.73 + 0.105 * (8.0 * x + seed as f32 % 4.0).sin();
        let sun = (x - sun_horizontal).powi(2) + (y - 0.30).powi(2) < 0.0038;
        let (r, g, b) = if sun {
            (252, 233, 170)
        } else if y > foreground {
            (21 + tint, 86 + tint, 108 + tint / 2)
        } else if y > ridge {
            (47 + tint, 118 + tint, 136)
        } else {
            let gradient = (y * 130.0) as u8;
            (126 + gradient, 179 + gradient / 2, 212 + tint / 2)
        };
        *pixel = Rgb8Pixel::new(r, g, b);
    }
    Image::from_rgb8(image)
}
