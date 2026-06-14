use std::path::Path;
use std::process::Command;

use crate::ssh_util::{ensure_ssh_host, ssh_command_failed, ssh_spawn_error};

pub(crate) fn ssh_control_path(host: &str) -> std::path::PathBuf {
    let safe: String = host
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    std::env::temp_dir().join(format!("mod-manager-ssh-{safe}.sock"))
}

pub(crate) fn ssh_command(host: &str, remote_command: &str) -> Result<std::process::Output, String> {
    ensure_ssh_host(host)?;
    let control_path = format!("ControlPath={}", ssh_control_path(host).display());
    Command::new("ssh")
        .args(ssh_base_args(&control_path))
        .arg(host)
        .arg(remote_command)
        .output()
        .map_err(|error| ssh_spawn_error(host, error))
}

/// Runs a remote command in a background thread without blocking the caller.
/// Used for long-running processes (e.g. Minecraft server) that must keep running after SSH disconnects.
pub(crate) fn ssh_command_background(host: &str, remote_command: String) -> Result<(), String> {
    ensure_ssh_host(host)?;
    let host = host.to_string();
    std::thread::spawn(move || {
        let control_path = format!("ControlPath={}", ssh_control_path(&host).display());
        let _ = Command::new("ssh")
            .args(ssh_base_args(&control_path))
            .arg(&host)
            .arg(&remote_command)
            .status();
    });
    Ok(())
}

fn ssh_base_args(control_path: &str) -> [&str; 10] {
    [
        "-o",
        "BatchMode=yes",
        "-o",
        "ControlMaster=auto",
        "-o",
        control_path,
        "-o",
        "ControlPersist=120",
        "-o",
        "ConnectTimeout=20",
    ]
}

fn scp_remote_target(host: &str, remote_file: &str) -> String {
    let path = remote_file.replace('\\', "/");
    let escaped = path
        .chars()
        .map(|ch| if ch == ' ' { "\\ ".to_string() } else { ch.to_string() })
        .collect::<String>();
    format!("{host}:{escaped}")
}

pub(crate) fn scp_upload(host: &str, local_path: &Path, remote_file: &str) -> Result<(), String> {
    ensure_ssh_host(host)?;
    let remote = scp_remote_target(host, remote_file);
    let control_path = format!("ControlPath={}", ssh_control_path(host).display());
    let output = Command::new("scp")
        .args([
            "-q",
            "-o",
            "BatchMode=yes",
            "-o",
            "ControlMaster=auto",
            "-o",
            control_path.as_str(),
            "-o",
            "ControlPersist=120",
            "-o",
            "ConnectTimeout=120",
        ])
        .arg(local_path)
        .arg(remote)
        .output()
        .map_err(|error| ssh_spawn_error(host, error))?;
    if output.status.success() {
        return Ok(());
    }
    Err(ssh_command_failed(host, &output))
}

pub(crate) fn scp_download(host: &str, remote_file: &str, local_path: &Path) -> Result<(), String> {
    ensure_ssh_host(host)?;
    let remote = scp_remote_target(host, remote_file);
    let control_path = format!("ControlPath={}", ssh_control_path(host).display());
    let output = Command::new("scp")
        .args([
            "-q",
            "-o",
            "BatchMode=yes",
            "-o",
            "ControlMaster=auto",
            "-o",
            control_path.as_str(),
            "-o",
            "ControlPersist=120",
            "-o",
            "ConnectTimeout=120",
        ])
        .arg(&remote)
        .arg(local_path)
        .output()
        .map_err(|error| ssh_spawn_error(host, error))?;
    if output.status.success() {
        return Ok(());
    }
    Err(ssh_command_failed(host, &output))
}
