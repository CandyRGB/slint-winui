use std::time::Duration;
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETMESSAGEDURATION, SPI_GETMOUSEHOVERTIME, SYSTEM_PARAMETERS_INFO_ACTION,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};

pub struct ToolTipTiming {
    pub show_delay: Duration,
    pub show_duration: Duration,
}

/// Reads the Windows desktop settings used for initial mouse and keyboard tooltips.
pub fn timing() -> ToolTipTiming {
    ToolTipTiming {
        show_delay: Duration::from_millis(2 * u64::from(parameter(SPI_GETMOUSEHOVERTIME, 400))),
        show_duration: Duration::from_secs(u64::from(parameter(SPI_GETMESSAGEDURATION, 5))),
    }
}

fn parameter(action: SYSTEM_PARAMETERS_INFO_ACTION, fallback: u32) -> u32 {
    let mut value = 0u32;
    // These two SPI read actions write a DWORD to the caller's buffer.
    unsafe {
        SystemParametersInfoW(
            action,
            0,
            Some(std::ptr::from_mut(&mut value).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
        )
    }
    .map_or(fallback, |()| value)
}
