use slint::{Image, Rgba8Pixel, SharedPixelBuffer, SharedString};

use crate::PersonPictureBehavior;

pub fn install(behavior: PersonPictureBehavior<'_>) {
    behavior.on_initials_from_name(|name| initials_from_name(&name).into());
    behavior.on_initials_from_parts(|first, last| {
        let mut initials = String::new();
        if uses_initials(&first) {
            append_initial(&mut initials, &first);
            append_initial(&mut initials, &last);
        }
        SharedString::from(initials)
    });
    behavior.on_round_photo(round_photo);
    behavior.set_ready(true);
}

fn initials_from_name(name: &str) -> String {
    if !uses_initials(name) {
        return String::new();
    }
    let mut words = without_trailing_note(name)
        .split(' ')
        .filter(|word| !word.is_empty());
    let mut initials = String::new();
    if let Some(first) = words.next() {
        append_initial(&mut initials, first);
        if let Some(last) = words.next_back() {
            append_initial(&mut initials, last);
        }
    }
    initials
}

fn without_trailing_note(name: &str) -> &str {
    let Some(open) = (match name.chars().next_back() {
        Some(')') => Some('('),
        Some(']') => Some('['),
        Some('}') => Some('{'),
        _ => None,
    }) else {
        return name;
    };
    name.rsplit_once(open).map_or(name, |(prefix, _)| prefix)
}

fn uses_initials(name: &str) -> bool {
    let mut standard = false;
    for ch in name
        .chars()
        .take(3)
        .take_while(|&ch| ch != '\0' && ch != '\u{feff}')
    {
        match ch as u32 {
            0x0250..=0x02af
            | 0x0370..=0x03ff
            | 0x0530..=0x06ff
            | 0x0750..=0x077f
            | 0x08a0..=0x0b7f
            | 0x0b80..=0x0bff
            | 0x0c00..=0x0dff
            | 0x0e00..=0x0eff
            | 0x2e80..=0x2eff
            | 0x3000..=0x303f
            | 0x31c0..=0x33ff
            | 0x3400..=0x4dbf
            | 0x4e00..=0x9fff
            | 0xa8e0..=0xa8ff
            | 0xf900..=0xfaff
            | 0xfb50..=0xfdff
            | 0xfe30..=0xfe4f
            | 0xfe70..=0xfefe
            | 0x20000..=0x2a6df
            | 0x2a700..=0x2b81f
            | 0x2f800..=0x2fa1f => return false,
            0x0001..=0x024f
            | 0x0300..=0x036f
            | 0x0400..=0x052f
            | 0x1e00..=0x1eff
            | 0x2c60..=0x2c7f
            | 0xa720..=0xa7ff
            | 0xab30..=0xab6f => standard = true,
            _ => {}
        }
    }
    standard
}

fn append_initial(output: &mut String, word: &str) {
    let omitted = |ch| matches!(ch, '!'..='/' | ':'..='@' | '{'..='~');
    let trimmed = word.trim_start_matches(omitted);
    let word = if trimmed.is_empty() { word } else { trimmed };
    let mut chars = word.chars();
    if let Some(base) = chars.next() {
        let mut uppercase = base.to_uppercase();
        let letter = uppercase.next().unwrap_or(base);
        output.push(if uppercase.next().is_some() {
            base
        } else {
            letter
        });
        output.extend(chars.take_while(|ch| matches!(ch, '\u{0300}'..='\u{036f}')));
    }
}

// Slint's software renderer clips children to a rectangle even when its corners
// are rounded. Preparing a transparent photo also keeps the crop independent of the renderer.
fn round_photo(image: Image) -> Image {
    let Some(source) = image.to_rgba8() else {
        return Image::default();
    };
    let side = source.width().min(source.height());
    if side == 0 {
        return Image::default();
    }

    let left = (source.width() - side) / 2;
    let top = (source.height() - side) / 2;
    let diameter = side.min(1024);
    let scale = f64::from(side) / f64::from(diameter);
    let radius = f64::from(diameter) / 2.0;
    let source_width = source.width() as usize;
    let source_pixels = source.as_slice();
    let mut result = SharedPixelBuffer::<Rgba8Pixel>::new(diameter, diameter);
    for (index, dest) in result.make_mut_slice().iter_mut().enumerate() {
        let x = (index % diameter as usize) as f64 + 0.5;
        let y = (index / diameter as usize) as f64 + 0.5;
        let coverage = (radius + 0.5 - (x - radius).hypot(y - radius)).clamp(0.0, 1.0);
        if coverage > 0.0 {
            let sx = left + (x * scale) as u32;
            let sy = top + (y * scale) as u32;
            let mut pixel = source_pixels[sy as usize * source_width + sx as usize];
            pixel.a = (f64::from(pixel.a) * coverage).round() as u8;
            *dest = pixel;
        }
    }
    Image::from_rgba8(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_first_and_last_words_and_preserves_combining_marks() {
        assert_eq!(
            initials_from_name("e\u{0301}mile de Saint [Staff]"),
            "E\u{0301}S"
        );
        assert_eq!(initials_from_name("Luma"), "L");
        assert_eq!(initials_from_name("Иван Петров"), "ИП");
        assert_eq!(initials_from_name("Jean -Benoît"), "JB");
        assert_eq!(initials_from_name("张小明"), "");
        assert_eq!(initials_from_name(""), "");
    }

    #[test]
    fn round_photo_crops_the_center_and_clears_the_corners() {
        let mut input = SharedPixelBuffer::<Rgba8Pixel>::new(20, 12);
        for (index, pixel) in input.make_mut_slice().iter_mut().enumerate() {
            *pixel = Rgba8Pixel {
                r: (index % 20) as u8,
                g: 0,
                b: 0,
                a: 255,
            };
        }
        let cropped = round_photo(Image::from_rgba8(input)).to_rgba8().unwrap();
        assert_eq!((cropped.width(), cropped.height()), (12, 12));
        assert_eq!(cropped.as_slice()[0].a, 0);
        assert_eq!(cropped.as_slice()[6 * 12 + 6].r, 10);
        assert_eq!(cropped.as_slice()[6 * 12 + 6].a, 255);
    }
}
