//! Bind an opted-in Slint title bar to the Windows caption's client area.

#[cfg(windows)]
use std::num::NonZeroIsize;

#[cfg(windows)]
use crate::WindowTitleBarBehavior;
#[cfg(windows)]
use slint_winui::platform::title_bar::{self, Layout};

/// Attach on the HWND's UI thread. Enable `sync-native-hit-test` on the
/// WindowTitleBar that owns the interactive area of this window.
#[cfg(windows)]
pub fn attach(
    behavior: WindowTitleBarBehavior<'_>,
    hwnd: NonZeroIsize,
    height: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    title_bar::attach(
        hwnd,
        Layout {
            height,
            left_client_width: behavior.get_interactive_width() as u16,
        },
    )?;
    behavior.on_interactive_width_changed(move |width| {
        let _ = title_bar::set_left_client_width(hwnd, width as u16);
    });
    Ok(())
}
