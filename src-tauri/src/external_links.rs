use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime, Url};
use tauri_plugin_opener::OpenerExt;

fn is_app_url(url: &Url) -> bool {
    match url.scheme() {
        "tauri" | "asset" | "file" | "data" => true,
        "http" | "https" => url.host_str().is_some_and(|host| {
            host == "localhost" || host == "127.0.0.1" || host.ends_with(".localhost")
        }),
        _ => false,
    }
}

pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("external-links-guard")
        .on_navigation(|webview, url| {
            if is_app_url(url) {
                return true;
            }

            if matches!(url.scheme(), "http" | "https" | "mailto" | "tel") {
                let target = url.to_string();
                let handle = webview.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    let _ = handle.opener().open_url(target, None::<&str>);
                });
                return false;
            }

            false
        })
        .build()
}
