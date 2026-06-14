use crate::server_control::start_config::ServerStartConfig;
use crate::ssh_exec::ssh_command;
use crate::ssh_util::ssh_command_failed;

fn shell_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(crate) fn validate_server_root(host: &str, server_root: &str) -> Result<(), String> {
    let root = shell_single_quoted(server_root);
    let cmd = format!("test -d {root}");
    let output = ssh_command(host, &cmd)?;
    if output.status.success() {
        Ok(())
    } else {
        Err("Папка сервера не найдена на удалённой машине.".to_string())
    }
}

pub(crate) fn is_running(host: &str, server_root: &str) -> Result<bool, String> {
    let root = shell_single_quoted(server_root);
    let cmd = format!(
        "pgrep -af java 2>/dev/null | grep -F {root} >/dev/null && echo running || true"
    );
    let output = ssh_command(host, &cmd)?;
    if !output.status.success() {
        return Err(ssh_command_failed(host, &output));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .eq_ignore_ascii_case("running"))
}

pub(crate) fn start(host: &str, server_root: &str, start: &ServerStartConfig) -> Result<(), String> {
    start_script(host, server_root, start)
}

fn start_script(host: &str, server_root: &str, start: &ServerStartConfig) -> Result<(), String> {
    let launch = start.launch_command();
    let root = shell_single_quoted(server_root);
    let script_path = shell_single_quoted(&launch.script);
    let extra_args = launch
        .extra_args
        .iter()
        .map(|arg| shell_single_quoted(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let cmd = format!(
        "cd {root} && test -f {script_path} || {{ echo MM_SCRIPT_NOT_FOUND >&2; exit 1; }} && \
         mkdir -p logs && nohup bash {script_path} {extra_args} >> logs/server-control.log 2>&1 &"
    );
    let output = ssh_command(host, &cmd)?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr);
        if detail.contains("MM_SCRIPT_NOT_FOUND") {
            return Err("Скрипт запуска не найден в корне сервера.".to_string());
        }
        Err(ssh_command_failed(host, &output))
    }
}

pub(crate) fn stop(host: &str, server_root: &str, start: &ServerStartConfig) -> Result<(), String> {
    let root = shell_single_quoted(server_root);
    let script = start.launch_script.trim();
    let script_filter = if script.is_empty() {
        String::new()
    } else {
        format!(" | grep -F {script}", script = shell_single_quoted(script))
    };
    let cmd = format!(
        "root={root}
for sig in TERM KILL; do
  pids=$(pgrep -af java 2>/dev/null | grep -F \"$root\" | awk '{{print $1}}')
  if [ -n \"$pids\" ]; then kill -$sig $pids 2>/dev/null || true; fi
  pids=$(pgrep -af bash 2>/dev/null | grep -F \"$root\"{script_filter} | awk '{{print $1}}')
  if [ -n \"$pids\" ]; then kill -$sig $pids 2>/dev/null || true; fi
  sleep 0.4
done
true"
    );
    let output = ssh_command(host, &cmd)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ssh_command_failed(host, &output))
    }
}
