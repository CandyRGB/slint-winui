//! Extends a Win32 window's client area into its native caption.

use std::num::NonZeroIsize;

use windows::Win32::Foundation::{
    E_FAIL, E_INVALIDARG, HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DwmDefWindowProc, DwmExtendFrameIntoClientArea,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetClientRect, GetPropW, GetWindowLongPtrW, GetWindowThreadProcessId, HTCAPTION,
    HTCLIENT, HTTOP, HTTOPLEFT, HTTOPRIGHT, IsZoomed, NCCALCSIZE_PARAMS, RemovePropW,
    SM_CXPADDEDBORDER, SM_CYSIZEFRAME, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetPropW, SetWindowPos, WM_ACTIVATE, WM_DPICHANGED, WM_NCCALCSIZE, WM_NCDESTROY,
    WM_NCHITTEST, WM_NCMOUSELEAVE, WS_CAPTION, WS_CHILD, WS_THICKFRAME,
};
use windows::core::{Error, PCWSTR, Result, w};

const SUBCLASS_ID: usize = 1;
const LAYOUT_PROPERTY: PCWSTR = w!("slint-winui.WindowTitleBar.Layout");

/// Title-bar dimensions in logical pixels at 96 DPI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Visible row height, excluding the top resize border clipped when maximized.
    pub height: u16,
    /// Width reserved for application controls along the left side of the row.
    pub left_client_width: u16,
}

impl Layout {
    // Window properties work with both common-controls versions 5 and 6.
    // Store the two dimensions directly. The value is never dereferenced.
    fn store(self, hwnd: HWND) -> Result<()> {
        let data = usize::from(self.height) | (usize::from(self.left_client_width) << 16);
        unsafe { SetPropW(hwnd, LAYOUT_PROPERTY, Some(HANDLE(data as *mut _))) }
    }

    fn load(hwnd: HWND) -> Option<Self> {
        let data = unsafe { GetPropW(hwnd, LAYOUT_PROPERTY) }.0 as usize;
        (data != 0).then_some(Self {
            height: data as u16,
            left_client_width: (data >> 16) as u16,
        })
    }
}

/// Installs the native caption on a live, decorated top-level HWND.
///
/// Call on the window's owning UI thread after creation. Keep the title-bar row
/// transparent so DWM can draw the system buttons over it. The subclass is
/// removed when the window is destroyed.
pub fn attach(window: NonZeroIsize, layout: Layout) -> Result<()> {
    let hwnd = owned_window(window)?;
    // The subclass and the layout property are both managed on the window thread.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        if layout.height == 0
            || style & WS_CHILD.0 != 0
            || style & WS_CAPTION.0 != WS_CAPTION.0
            || Layout::load(hwnd).is_some()
        {
            return Err(Error::from_hresult(E_INVALIDARG));
        }
        layout.store(hwnd)?;
        if !SetWindowSubclass(hwnd, Some(caption_proc), SUBCLASS_ID, 0).as_bool() {
            let _ = RemovePropW(hwnd, LAYOUT_PROPERTY);
            return Err(Error::from_hresult(E_FAIL));
        }
        if let Err(error) = extend_frame(hwnd, layout).and_then(|()| refresh_frame(hwnd)) {
            let _ = RemoveWindowSubclass(hwnd, Some(caption_proc), SUBCLASS_ID);
            let _ = RemovePropW(hwnd, LAYOUT_PROPERTY);
            let _ = DwmExtendFrameIntoClientArea(hwnd, &MARGINS::default());
            let _ = refresh_frame(hwnd);
            return Err(error);
        }
    }
    Ok(())
}

/// Updates the application-controlled width when the title-bar controls change.
pub fn set_left_client_width(window: NonZeroIsize, width: u16) -> Result<()> {
    let hwnd = owned_window(window)?;
    let mut layout = Layout::load(hwnd).ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
    layout.left_client_width = width;
    layout.store(hwnd)
}

/// Logical top inset to add to the content when an attached window is maximized.
pub fn maximized_inset(window: NonZeroIsize) -> f32 {
    let dpi = window_dpi(HWND(window.get() as *mut _));
    resize_border(dpi) as f32 * 96.0 / dpi as f32
}

/// Sets caption colors using Win32 COLORREF values (0x00BBGGRR).
///
/// On Windows versions that lack caption color attributes, the system colors
/// remain in effect.
pub fn set_colors(window: NonZeroIsize, background: u32, foreground: u32) {
    let hwnd = HWND(window.get() as *mut _);
    for (attribute, color) in [
        (DWMWA_CAPTION_COLOR, background),
        (DWMWA_TEXT_COLOR, foreground),
    ] {
        let _ = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                attribute,
                (&raw const color).cast(),
                size_of::<u32>() as u32,
            )
        };
    }
}

fn owned_window(window: NonZeroIsize) -> Result<HWND> {
    let hwnd = HWND(window.get() as *mut _);
    if unsafe { GetWindowThreadProcessId(hwnd, None) } != unsafe { GetCurrentThreadId() } {
        return Err(Error::from_hresult(E_INVALIDARG));
    }
    Ok(hwnd)
}

fn window_dpi(hwnd: HWND) -> u32 {
    match unsafe { GetDpiForWindow(hwnd) } {
        0 => 96,
        dpi => dpi,
    }
}

fn scale(length: u16, dpi: u32) -> i32 {
    ((i64::from(length) * i64::from(dpi) + 48) / 96) as i32
}

fn resize_border(dpi: u32) -> i32 {
    unsafe {
        GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi)
    }
}

unsafe fn extend_frame(hwnd: HWND, layout: Layout) -> Result<()> {
    let dpi = window_dpi(hwnd);
    unsafe {
        DwmExtendFrameIntoClientArea(
            hwnd,
            &MARGINS {
                cyTopHeight: scale(layout.height, dpi) + resize_border(dpi),
                ..MARGINS::default()
            },
        )
    }
}

unsafe fn refresh_frame(hwnd: HWND) -> Result<()> {
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER,
        )
    }
}

unsafe extern "system" fn caption_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _data: usize,
) -> LRESULT {
    // These messages and their pointer arguments come from the window manager.
    unsafe {
        match message {
            WM_NCCALCSIZE if lparam.0 != 0 => {
                let client = if wparam.0 != 0 {
                    &raw mut (*(lparam.0 as *mut NCCALCSIZE_PARAMS)).rgrc[0]
                } else {
                    lparam.0 as *mut RECT
                };
                let window_top = (*client).top;
                DefSubclassProc(hwnd, message, wparam, lparam);
                // Preserve the default side and bottom edges. A top non-client inset
                // interferes with DWM's native caption-button hit testing.
                (*client).top = window_top;
                return LRESULT(0);
            }
            WM_NCHITTEST | WM_NCMOUSELEAVE => {
                let mut result = LRESULT(0);
                if DwmDefWindowProc(hwnd, message, wparam, lparam, &mut result).as_bool() {
                    return result;
                }
                if message == WM_NCHITTEST {
                    let hit = DefSubclassProc(hwnd, message, wparam, lparam);
                    if hit.0 == HTCLIENT as isize || hit.0 == HTCAPTION as isize {
                        return Layout::load(hwnd)
                            .and_then(|layout| hit_client(hwnd, lparam, layout))
                            .unwrap_or(hit);
                    }
                    return hit;
                }
            }
            WM_ACTIVATE => {
                if let Some(layout) = Layout::load(hwnd) {
                    let _ = extend_frame(hwnd, layout);
                }
            }
            WM_DPICHANGED => {
                let result = DefSubclassProc(hwnd, message, wparam, lparam);
                if let Some(layout) = Layout::load(hwnd) {
                    let _ = extend_frame(hwnd, layout);
                }
                return result;
            }
            WM_NCDESTROY => {
                let _ = RemoveWindowSubclass(hwnd, Some(caption_proc), id);
                let _ = RemovePropW(hwnd, LAYOUT_PROPERTY);
            }
            _ => {}
        }
        DefSubclassProc(hwnd, message, wparam, lparam)
    }
}

fn hit_client(hwnd: HWND, lparam: LPARAM, layout: Layout) -> Option<LRESULT> {
    let mut point = POINT {
        x: lparam.0 as i16 as i32,
        y: (lparam.0 >> 16) as i16 as i32,
    };
    let mut client = RECT::default();
    unsafe {
        if !ScreenToClient(hwnd, &mut point).as_bool()
            || GetClientRect(hwnd, &mut client).is_err()
            || point.x < 0
            || point.x >= client.right
            || point.y < 0
        {
            return None;
        }
    }

    let dpi = window_dpi(hwnd);
    let maximized = unsafe { IsZoomed(hwnd) }.as_bool();
    if point.y < scale(4, dpi)
        && !maximized
        && unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_THICKFRAME.0 != 0 }
    {
        let grip = scale(8, dpi);
        let hit = if point.x < grip {
            HTTOPLEFT
        } else if point.x >= client.right - grip {
            HTTOPRIGHT
        } else {
            HTTOP
        };
        return Some(LRESULT(hit as isize));
    }

    let caption_bottom = scale(layout.height, dpi) + if maximized { resize_border(dpi) } else { 0 };
    let hit = if point.y < caption_bottom && point.x >= scale(layout.left_client_width, dpi) {
        HTCAPTION
    } else {
        HTCLIENT
    };
    Some(LRESULT(hit as isize))
}
