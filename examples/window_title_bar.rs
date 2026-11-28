slint::slint! {
    import { WindowTitleBarExample } from "ui/window_title_bar.slint";
    export { WindowTitleBarExample }
}

#[cfg(all(windows, feature = "windows-platform"))]
fn install_title_bar(window: &WindowTitleBarExample) -> Result<(), slint::EventLoopError> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::winit_030::{EventResult, winit::event::WindowEvent};
    use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
    use slint_winui::platform::title_bar::{self, Layout};

    let weak = window.as_weak();
    slint::spawn_local(async move {
        let Some(window) = weak.upgrade() else { return };
        if window.window().winit_window().await.is_err() {
            return;
        }
        let handle = window.window().window_handle();
        let Ok(handle) = handle.window_handle() else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let hwnd = handle.hwnd;
        let layout = Layout {
            height: window.get_caption_height() as u16,
            left_client_width: window.get_caption_controls_width() as u16,
        };
        if let Err(error) = title_bar::attach(hwnd, layout) {
            eprintln!("Unable to extend the Windows title bar: {error}");
            return;
        }

        window.set_maximized_caption_inset(title_bar::maximized_inset(hwnd));
        let update_colors = move |window: &WindowTitleBarExample| {
            let color_ref = |c: slint::Color| u32::from_le_bytes([c.red(), c.green(), c.blue(), 0]);
            title_bar::set_colors(
                hwnd,
                color_ref(window.get_caption_background()),
                color_ref(window.get_caption_foreground()),
            );
        };
        update_colors(&window);
        let weak = window.as_weak();
        window.on_caption_colors_changed(move || {
            if let Some(window) = weak.upgrade() {
                update_colors(&window);
            }
        });
        window.set_caption_extended(true);

        let weak = window.as_weak();
        window.on_caption_controls_changed(move || {
            if let Some(window) = weak.upgrade() {
                let _ = title_bar::set_left_client_width(
                    hwnd,
                    window.get_caption_controls_width() as u16,
                );
            }
        });

        let weak = window.as_weak();
        window.window().on_winit_window_event(move |_, event| {
            if matches!(event, WindowEvent::ScaleFactorChanged { .. })
                && let Some(window) = weak.upgrade()
            {
                window.set_maximized_caption_inset(title_bar::maximized_inset(hwnd));
            }
            EventResult::Propagate
        });
    })?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let window = WindowTitleBarExample::new()?;
    #[cfg(all(windows, feature = "windows-platform"))]
    install_title_bar(&window)?;
    window.run()?;
    Ok(())
}
