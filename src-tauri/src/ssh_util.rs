use std::process::{Command, Output};

pub(crate) fn short_msg(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max {
        trimmed.to_string()
    } else {
        format!("{}…", trimmed.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}

pub(crate) fn ssh_config_hostname(host: &str) -> Option<String> {
    let output = Command::new("ssh").args(["-G", host]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if parts.next()? != "hostname" {
            continue;
        }
        let hostname = parts.next()?.trim();
        if hostname.is_empty() || hostname.eq_ignore_ascii_case(host) {
            return None;
        }
        return Some(hostname.to_string());
    }
    None
}

pub(crate) fn ensure_ssh_host(host: &str) -> Result<(), String> {
    if ssh_config_hostname(host).is_none() {
        return Err(format!("«{host}» не в ~/.ssh/config."));
    }
    Ok(())
}

pub(crate) fn explain_ssh_error(host: &str, detail: &str) -> String {
    let text = detail.strip_prefix("ssh: ").unwrap_or(detail).trim();
    if text.contains("#< CLIXML") || text.contains("<Objs Version=") {
        return "Служебный вывод PowerShell (не ошибка сервера).".to_string();
    }
    if text.contains("Could not resolve hostname") || text.contains("nodename nor servname") {
        return format!("«{host}» не в ~/.ssh/config.");
    }
    if text.contains("Permission denied") {
        return format!("SSH отказал: «{host}».");
    }
    if text.contains("Connection refused") || text.contains("Operation timed out") {
        return format!("Нет связи с «{host}».");
    }
    if text.starts_with("scp:") {
        return "Не удалось передать файл на сервер (проверьте путь и SSH).".to_string();
    }
    short_msg(text, 48)
}

pub(crate) fn ssh_command_failed(host: &str, output: &Output) -> String {
    let raw_stderr = String::from_utf8_lossy(&output.stderr);
    let raw_stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = crate::server_control::readiness::clean_powershell_stdout(&raw_stderr);
    let stdout = crate::server_control::readiness::clean_powershell_stdout(&raw_stdout);
    let detail = if !stderr.is_empty() { stderr } else { stdout };
    if detail.is_empty() {
        if crate::server_control::readiness::contains_clixml(&raw_stderr)
            || crate::server_control::readiness::contains_clixml(&raw_stdout)
        {
            return crate::server_control::readiness::clixml_noise_message().to_string();
        }
        let code = output.status.code().unwrap_or(-1);
        return format!("Удалённая команда завершилась с кодом {code}.");
    }
    explain_ssh_error(host, &detail)
}

pub(crate) fn ssh_spawn_error(host: &str, error: impl std::fmt::Display) -> String {
    explain_ssh_error(host, &format!("ssh: {error}"))
}

#[cfg(test)]
mod tests {
    use super::explain_ssh_error;

    #[test]
    fn explains_unknown_host() {
        let message = explain_ssh_error(
            "win-test2",
            "ssh: Could not resolve hostname win-test2: nodename nor servname provided, or not known",
        );
        assert_eq!(message, "«win-test2» не в ~/.ssh/config.");
    }
}
