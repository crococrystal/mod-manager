use std::process::Output;

use base64::{engine::general_purpose::STANDARD, Engine};

use crate::server_control::readiness::{self, DONE_MARKER};
use crate::server_control::start_config::ServerStartConfig;
use crate::ssh_exec::{ssh_command, ssh_command_background};
use crate::ssh_util::ssh_command_failed;

fn powershell_literal(path: &str) -> String {
    path.replace('\'', "''")
}

fn encode_powershell(script: &str) -> String {
    let mut bytes = Vec::with_capacity(script.len() * 2);
    for unit in script.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    STANDARD.encode(bytes)
}

fn wrap_powershell(script: &str) -> String {
    format!(
        "$ProgressPreference='SilentlyContinue';$WarningPreference='SilentlyContinue';$ErrorActionPreference='Stop';{script}"
    )
}

fn run_powershell(host: &str, script: &str) -> Result<Output, String> {
    let encoded = encode_powershell(&wrap_powershell(script));
    ssh_command(
        host,
        &format!("powershell -NoProfile -NonInteractive -EncodedCommand {encoded}"),
    )
}

fn explain_remote_error(detail: &str) -> Option<String> {
    let text = detail.trim();
    if text.contains("CLIXML") || text.contains("<Objs Version=") {
        return Some(
            "Ошибка PowerShell на сервере. Проверьте SSH и доступ к logs/latest.log.".to_string(),
        );
    }
    if text.contains("MM_STOP_FAILED") {
        return Some("Не удалось остановить сервер.".to_string());
    }
    if text.contains("MM_LOG_READ_FAILED") {
        return Some("Не удалось прочитать logs/latest.log.".to_string());
    }
    if text.contains("MM_SCRIPT_NOT_FOUND") {
        return Some("Скрипт запуска не найден в корне сервера.".to_string());
    }
    if text.contains("MM_ROOT_MISSING") {
        return Some("Папка сервера не найдена на удалённой машине.".to_string());
    }
    if text.contains("MM_ROOT_NOT_DIR") {
        return Some("Путь должен указывать на папку.".to_string());
    }
    if text.contains("MM_NO_NEOFORGE") {
        return Some("Не найден NeoForge в libraries/net/neoforged/neoforge.".to_string());
    }
    if text.contains("MM_SCRIPT_START_FAILED") {
        return Some("Не удалось запустить скрипт на сервере.".to_string());
    }
    if detail.contains("$root") || detail.contains("ParserError") || detail.contains("At line:") {
        return Some("Ошибка удалённого скрипта.".to_string());
    }
    None
}

fn remote_command_failed(host: &str, output: &Output) -> String {
    let raw_stderr = String::from_utf8_lossy(&output.stderr);
    let raw_stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = readiness::clean_powershell_stdout(&raw_stderr);
    let stdout = readiness::clean_powershell_stdout(&raw_stdout);
    let detail = if !stderr.is_empty() { stderr } else { stdout };
    if let Some(message) = explain_remote_error(&detail) {
        return message;
    }
    if detail.is_empty()
        && (readiness::contains_clixml(&raw_stderr) || readiness::contains_clixml(&raw_stdout))
    {
        return readiness::clixml_noise_message().to_string();
    }
    if !detail.is_empty() {
        return crate::ssh_util::explain_ssh_error(host, &detail);
    }
    ssh_command_failed(host, &output)
}

fn assign_root(root: &str) -> String {
    format!("$root = '{}'.Replace('/', '\\').TrimEnd('\\')", powershell_literal(root))
}

fn neo_marker_block() -> &'static str {
    "$neoVer = Get-ChildItem -LiteralPath (Join-Path $root 'libraries\\net\\neoforged\\neoforge') -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending |
        Select-Object -First 1 -ExpandProperty Name
    if (-not $neoVer) { throw 'MM_NO_NEOFORGE' }
    $marker = ('libraries/net/neoforged/neoforge/' + $neoVer + '/win_args.txt').ToLower()"
}

fn cmd_quoted_path(path: &str) -> String {
    format!(
        "\"{}\"",
        path.replace('/', "\\").trim_end_matches('\\').replace('"', "")
    )
}

fn build_start_command(server_root: &str, start: &ServerStartConfig) -> String {
    let launch = start.launch_command();
    let root = cmd_quoted_path(server_root);
    let script_leaf = launch
        .script
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(launch.script.as_str());
    let mut run = if script_leaf.to_ascii_lowercase().ends_with(".bat") {
        format!("call {script_leaf}")
    } else {
        script_leaf.to_string()
    };
    if !launch.extra_args.is_empty() {
        run.push(' ');
        run.push_str(&launch.extra_args.join(" "));
    }
    // Script existence is checked separately; do not chain "if not exist ... & cd && run"
    // — that pattern fails over Windows OpenSSH and never starts java.
    format!("cd /d {root} && {run}")
}

fn start_script(host: &str, server_root: &str, start: &ServerStartConfig) -> Result<(), String> {
    let launch = start.launch_command();
    let script_leaf = launch
        .script
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(launch.script.as_str());
    let script_path = cmd_quoted_path(&format!(
        "{}/{}",
        server_root.trim_end_matches('/').trim_end_matches('\\'),
        script_leaf
    ));
    let check = format!("if not exist {script_path} (echo MM_SCRIPT_NOT_FOUND>&2 & exit /b 1)");
    let output = ssh_command(host, &check)?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        if detail.contains("MM_SCRIPT_NOT_FOUND") {
            return Err("Скрипт запуска не найден в корне сервера.".to_string());
        }
        return Err(remote_command_failed(host, &output));
    }

    let remote = build_start_command(server_root, start);
    ssh_command_background(host, remote)
}

pub(crate) fn validate_server_root(host: &str, server_root: &str) -> Result<(), String> {
    let script = format!(
        "{assign}
        if (-not (Test-Path -LiteralPath $root)) {{ throw 'MM_ROOT_MISSING' }}
        if (-not (Test-Path -LiteralPath $root -PathType Container)) {{ throw 'MM_ROOT_NOT_DIR' }}",
        assign = assign_root(server_root)
    );
    let output = run_powershell(host, &script)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(remote_command_failed(host, &output))
    }
}

pub(crate) fn is_ready(host: &str, server_root: &str) -> Result<bool, String> {
    let script = format!(
        "{assign}
        $log = Join-Path $root 'logs\\latest.log'
        if (-not (Test-Path -LiteralPath $log)) {{ exit 0 }}
        $lastDone = 0
        $lastBoot = 0
        $lineNo = 0
        try {{
            $stream = [System.IO.File]::Open($log, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
            $reader = New-Object System.IO.StreamReader($stream)
            try {{
                while ($null -ne ($line = $reader.ReadLine())) {{
                    $lineNo++
                    if ($line.Contains('{done_marker}')) {{ $lastDone = $lineNo }}
                    $lower = $line.ToLowerInvariant()
                    if ($lower.Contains('modlauncher running') -or $lower.Contains('starting minecraft server on') -or $lower.Contains('preparing spawn area')) {{
                        $lastBoot = $lineNo
                    }}
                }}
            }} finally {{
                $reader.Dispose()
                $stream.Dispose()
            }}
        }} catch {{
            Write-Error 'MM_LOG_READ_FAILED'
            exit 1
        }}
        if ($lastDone -gt 0 -and ($lastBoot -eq 0 -or $lastDone -gt $lastBoot)) {{ Write-Output 'ready' }}",
        assign = assign_root(server_root),
        done_marker = DONE_MARKER
    );
    let output = run_powershell(host, &script)?;
    if !output.status.success() {
        return Err(remote_command_failed(host, &output));
    }
    Ok(readiness::stdout_indicates_ready(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

pub(crate) fn is_running(host: &str, server_root: &str) -> Result<bool, String> {
    let script = format!(
        "{assign}
        {neo}
        $rootKey = $root.Replace('\\', '/').ToLower()
        Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
            Where-Object {{
                if ($_.Name -ne 'java.exe' -or -not $_.CommandLine) {{ return $false }}
                $cmdNorm = $_.CommandLine.Replace('\\', '/').ToLower()
                $cmdNorm.Contains($marker) -or $cmdNorm.Contains($rootKey)
            }} |
            Select-Object -First 1 |
            ForEach-Object {{ Write-Output 'running' }}",
        assign = assign_root(server_root),
        neo = neo_marker_block()
    );
    let output = run_powershell(host, &script)?;
    if !output.status.success() {
        return Err(remote_command_failed(host, &output));
    }
    Ok(
        readiness::clean_powershell_stdout(&String::from_utf8_lossy(&output.stdout))
            .eq_ignore_ascii_case("running"),
    )
}

pub(crate) fn start(host: &str, server_root: &str, start: &ServerStartConfig) -> Result<(), String> {
    start_script(host, server_root, start)
}

pub(crate) fn stop(host: &str, server_root: &str, start: &ServerStartConfig) -> Result<(), String> {
    let script_key = start
        .launch_script
        .trim()
        .to_ascii_lowercase()
        .replace('\\', "/");
    let script_match = if script_key.is_empty() {
        String::new()
    } else {
        format!(
            "$scriptKey = '{}'
        ",
            powershell_literal(&script_key)
        )
    };
    let script_predicate = if script_key.is_empty() {
        "($false)".to_string()
    } else {
        "$cmdNorm.Contains($scriptKey)".to_string()
    };
    let script = format!(
        "{assign}
        {neo}
        {script_match}$rootKey = $root.Replace('\\', '/').ToLower()
        $all = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue)
        $javaProcs = @($all | Where-Object {{
            if ($_.Name -ne 'java.exe' -or -not $_.CommandLine) {{ return $false }}
            $cmdNorm = $_.CommandLine.Replace('\\', '/').ToLower()
            $cmdNorm.Contains($marker) -or $cmdNorm.Contains($rootKey)
        }})
        $cmdProcs = @($all | Where-Object {{
            if ($_.Name -ne 'cmd.exe' -or -not $_.CommandLine) {{ return $false }}
            $cmdNorm = $_.CommandLine.Replace('\\', '/').ToLower()
            $cmdNorm.Contains($rootKey) -or {script_predicate}
        }})
        $targets = @($javaProcs + $cmdProcs)
        if (-not $targets) {{ exit 0 }}
        foreach ($proc in $targets) {{
            try {{
                & taskkill /F /T /PID $proc.ProcessId 2>$null | Out-Null
            }} catch {{}}
            try {{
                Stop-Process -Id $proc.ProcessId -Force -ErrorAction SilentlyContinue
            }} catch {{}}
        }}
        $still = @(Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object {{
            if ($_.Name -ne 'java.exe' -and $_.Name -ne 'cmd.exe') {{ return $false }}
            if (-not $_.CommandLine) {{ return $false }}
            $cmdNorm = $_.CommandLine.Replace('\\', '/').ToLower()
            ($_.Name -eq 'java.exe' -and ($cmdNorm.Contains($marker) -or $cmdNorm.Contains($rootKey))) -or
            ($_.Name -eq 'cmd.exe' -and ($cmdNorm.Contains($rootKey) -or {script_predicate}))
        }})
        if ($still.Count -gt 0) {{ throw 'MM_STOP_FAILED' }}",
        assign = assign_root(server_root),
        neo = neo_marker_block(),
        script_match = script_match,
        script_predicate = script_predicate,
    );
    let output = run_powershell(host, &script)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(remote_command_failed(host, &output))
    }
}
