use tauri::{WebviewWindow, WindowEvent};

pub fn attach(window: &WebviewWindow) {
    let _ = apply(window, true);

    let window = window.clone();
    let _ = window.clone().on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. }
        ) {
            invalidate(&window);

            // DWM иногда сбрасывает кастомные атрибуты после WM_DPICHANGED / theme switch —
            // переприменяем их после геометрических событий.
            #[cfg(windows)]
            apply_windows_dwm_chrome(&window);
        }
    });
}

pub fn apply(window: &WebviewWindow, enable: bool) -> tauri::Result<()> {
    window.set_shadow(enable)?;
    #[cfg(target_os = "macos")]
    if enable {
        invalidate(window);
    }
    #[cfg(windows)]
    apply_windows_dwm_chrome(window);
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

/// Отключаем системную рамку DWM и стандартное скругление углов на Windows 11+.
/// Кастомное скругление и тень рисует фронт через CSS, и системные атрибуты не должны
/// конфликтовать с ним.
///
/// На Windows 10 и более старых билдах эти атрибуты не поддерживаются — `DwmSetWindowAttribute`
/// вернёт ошибку, которую мы безопасно игнорируем.
#[cfg(windows)]
pub fn apply_windows_dwm_chrome(window: &WebviewWindow) {
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
    };

    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    let hwnd: HWND = hwnd.0 as HWND;

    unsafe {
        // DWMWA_COLOR_NONE = 0xFFFFFFFE — DWM не рисует системную рамку поверх borderless-окна.
        let no_border: u32 = 0xFFFF_FFFE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            &no_border as *const u32 as *const c_void,
            std::mem::size_of::<u32>() as u32,
        );

        // DWMWCP_DONOTROUND = 1 — система не скругляет углы, чтобы они совпадали с CSS border-radius.
        let preference: u32 = DWMWCP_DONOTROUND as u32;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &preference as *const u32 as *const c_void,
            std::mem::size_of::<u32>() as u32,
        );
    }
}
