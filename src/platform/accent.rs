use windows::UI::ViewManagement::{UIColorType, UISettings};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LightAccentTextColors {
    pub primary: [u8; 3],
    pub secondary: [u8; 3],
    pub tertiary: [u8; 3],
}

pub fn system_accent_color() -> windows::core::Result<[u8; 3]> {
    let color = UISettings::new()?.GetColorValue(UIColorType::Accent)?;
    Ok([color.R, color.G, color.B])
}

pub fn light_accent_text_colors() -> windows::core::Result<LightAccentTextColors> {
    let settings = UISettings::new()?;
    let color = |kind| {
        settings
            .GetColorValue(kind)
            .map(|color| [color.R, color.G, color.B])
    };

    Ok(LightAccentTextColors {
        primary: color(UIColorType::AccentDark2)?,
        secondary: color(UIColorType::AccentDark3)?,
        tertiary: color(UIColorType::AccentDark1)?,
    })
}
