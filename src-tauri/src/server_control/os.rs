use crate::ssh_exec::ssh_command;
use crate::ssh_util::ssh_command_failed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RemoteOs {
    Windows,
    Linux,
}

impl RemoteOs {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
        }
    }
}

pub(crate) fn resolve_remote_os(host: &str, setting: &str) -> Result<RemoteOs, String> {
    match setting.trim().to_ascii_lowercase().as_str() {
        "windows" | "win" => Ok(RemoteOs::Windows),
        "linux" => Ok(RemoteOs::Linux),
        "auto" | "" => detect_remote_os(host),
        _ => Err(format!(
            "Неизвестная ОС сервера «{setting}». Используйте auto, windows или linux."
        )),
    }
}

fn detect_remote_os(host: &str) -> Result<RemoteOs, String> {
    let output = ssh_command(
        host,
        "cmd /c ver 2>nul || (uname -s 2>/dev/null || echo unknown)",
    )?;
    if !output.status.success() {
        return Err(ssh_command_failed(host, &output));
    }
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .to_ascii_lowercase();
    if text.contains("windows") || text.contains("microsoft") {
        Ok(RemoteOs::Windows)
    } else if text.contains("linux") {
        Ok(RemoteOs::Linux)
    } else {
        Ok(RemoteOs::Windows)
    }
}
