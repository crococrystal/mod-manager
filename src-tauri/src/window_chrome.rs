use tauri::{WebviewWindow, WindowEvent};

pub fn attach(window: &WebviewWindow) {
    let _ = apply(window, true);

    let window = window.clone();
    let _ = window.clone().on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. }
        ) {
            #[cfg(target_os = "macos")]
            invalidate(&window);
        }
    });
}

pub fn apply(window: &WebviewWindow, enable: bool) -> tauri::Result<()> {
    window.set_shadow(enable)?;
    #[cfg(target_os = "macos")]
    if enable {
        invalidate(window);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn invalidate(window: &WebviewWindow) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    if let Ok(ptr) = window.ns_window() {
        unsafe {
            let obj = ptr as *mut AnyObject;
            let _: () = msg_send![obj, invalidateShadow];
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn invalidate(_window: &WebviewWindow) {}
