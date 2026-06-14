use super::{cleanup, neoforged, ssh};

pub(crate) fn apply_server(
    http: &reqwest::blocking::Client,
    host: &str,
    server_root: &str,
    target_version: &str,
) -> Result<Vec<String>, String> {
    let remote_root = server_root.trim_end_matches('/').replace('\\', "/");
    let installer_name = format!("neoforge-{target_version}-installer.jar");
    let remote_installer = format!("{remote_root}/{installer_name}");

    let temp_dir = std::env::temp_dir().join(format!(
        "mod-manager-neoforge-{target_version}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).map_err(|error| error.to_string())?;
    let local_installer = temp_dir.join(&installer_name);

    let url = neoforged::installer_url(target_version);
    neoforged::download_http(http, &url, &local_installer)?;
    ssh::scp_upload(host, &local_installer, &remote_installer)?;

    let win_root = remote_root.replace('/', "\\");
    let win_installer = remote_installer.replace('/', "\\");
    let install_cmd = format!(
        "cd /d \"{win_root}\" && java -jar \"{win_installer}\" --installServer"
    );
    let output = ssh::ssh_command(host, &install_cmd)?;
    let _ = std::fs::remove_dir_all(&temp_dir);

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}").trim().to_string();
        return Err(if combined.is_empty() {
            "Installer на сервере завершился с ошибкой.".to_string()
        } else if combined.chars().count() > 240 {
            format!("{}…", combined.chars().take(240).collect::<String>())
        } else {
            combined
        });
    }

    cleanup::remove_server_installers(host, server_root)
}
