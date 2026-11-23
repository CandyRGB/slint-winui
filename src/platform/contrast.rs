use windows::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, COLOR_BTNTEXT, COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT,
    COLOR_WINDOW, COLOR_WINDOWTEXT, GetSysColor, SYS_COLOR_INDEX,
};
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContrastColors {
    pub window: [u8; 3],
    pub window_text: [u8; 3],
    pub button_face: [u8; 3],
    pub button_text: [u8; 3],
    pub disabled_text: [u8; 3],
    pub highlight: [u8; 3],
    pub highlight_text: [u8; 3],
}

/// Reads the currently applied Windows contrast theme.
pub fn current() -> windows::core::Result<Option<ContrastColors>> {
    let mut settings = HIGHCONTRASTW {
        cbSize: size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    // The structure stays alive and writable for the duration of the system call.
    unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            settings.cbSize,
            Some(std::ptr::from_mut(&mut settings).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
        )?;
    }
    Ok(settings
        .dwFlags
        .contains(HCF_HIGHCONTRASTON)
        .then(|| ContrastColors {
            window: system_color(COLOR_WINDOW),
            window_text: system_color(COLOR_WINDOWTEXT),
            button_face: system_color(COLOR_BTNFACE),
            button_text: system_color(COLOR_BTNTEXT),
            disabled_text: system_color(COLOR_GRAYTEXT),
            highlight: system_color(COLOR_HIGHLIGHT),
            highlight_text: system_color(COLOR_HIGHLIGHTTEXT),
        }))
}

fn system_color(index: SYS_COLOR_INDEX) -> [u8; 3] {
    // COLORREF stores red, green, and blue in the three least-significant bytes.
    let [red, green, blue, _] = unsafe { GetSysColor(index) }.to_le_bytes();
    [red, green, blue]
}
