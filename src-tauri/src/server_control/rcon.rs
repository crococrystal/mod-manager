use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::process::{Child, Command};
use std::time::Duration;

use crate::server_control::os::RemoteOs;
use crate::ssh_exec::{ssh_command, ssh_control_path};
use crate::ssh_util::{ensure_ssh_host, ssh_command_failed, ssh_config_hostname};

const SERVERDATA_AUTH: i32 = 3;
const SERVERDATA_EXECCOMMAND: i32 = 2;
const SERVERDATA_RESPONSE_VALUE: i32 = 0;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const IO_TIMEOUT: Duration = Duration::from_secs(12);
const MAX_RESPONSE_PACKETS: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RconConfig {
    pub enabled: bool,
    pub port: u16,
    pub password: String,
}

#[derive(Clone, Debug)]
pub(crate) struct RconCheckDetails {
    pub port: u16,
    pub connect_host: String,
    pub ssh_alias: String,
    pub via_tunnel: bool,
    pub properties_path: String,
    pub message: String,
    pub detail: String,
}

struct SshTunnel {
    child: Child,
    local_port: u16,
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct RconConnection {
    stream: TcpStream,
    #[allow(dead_code)]
    tunnel: Option<SshTunnel>,
    connect_label: String,
    via_tunnel: bool,
}

pub(crate) fn parse_server_properties(content: &str) -> RconConfig {
    let mut enabled = false;
    let mut port = 25575_u16;
    let mut password = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "enable-rcon" => enabled = value.trim().eq_ignore_ascii_case("true"),
            "rcon.port" => {
                if let Ok(parsed) = value.trim().parse::<u16>() {
                    port = parsed;
                }
            }
            "rcon.password" => password = unescape_property_value(value.trim()),
            _ => {}
        }
    }

    RconConfig {
        enabled,
        port,
        password,
    }
}

fn unescape_property_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some(':') => out.push(':'),
                Some('=') => out.push('='),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

pub(crate) fn server_properties_remote_path(server_root: &str) -> String {
    let base = server_root
        .trim()
        .trim_end_matches('/')
        .trim_end_matches('\\')
        .replace('\\', "/");
    format!("{base}/server.properties")
}

fn powershell_literal(path: &str) -> String {
    path.replace('\'', "''")
}

pub(crate) fn read_server_properties(
    host: &str,
    server_root: &str,
    os: RemoteOs,
) -> Result<String, String> {
    let path = server_properties_remote_path(server_root);
    let output = match os {
        RemoteOs::Windows => {
            let win_path = path.replace('/', "\\");
            let cmd = format!(
                "powershell -NoProfile -NonInteractive -Command \"Get-Content -LiteralPath '{}' -Raw -ErrorAction Stop\"",
                powershell_literal(&win_path)
            );
            ssh_command(host, &cmd)?
        }
        RemoteOs::Linux => {
            let escaped = path.replace('\'', "'\\''");
            ssh_command(host, &format!("cat '{escaped}'"))?
        }
    };
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        if detail.contains("not found") || detail.contains("Cannot find path") {
            return Err(format!(
                "server.properties не найден: {path}"
            ));
        }
        return Err(ssh_command_failed(host, &output));
    }
    let content = String::from_utf8_lossy(&output.stdout).to_string();
    if content.trim().is_empty() {
        return Err(format!("server.properties пустой: {path}"));
    }
    Ok(content)
}

pub(crate) fn load_rcon_config(
    host: &str,
    server_root: &str,
    os: RemoteOs,
) -> Result<RconConfig, String> {
    let content = read_server_properties(host, server_root, os)?;
    Ok(parse_server_properties(&content))
}

pub(crate) fn resolve_rcon_host(ssh_alias: &str) -> String {
    ssh_config_hostname(ssh_alias).unwrap_or_else(|| ssh_alias.to_string())
}

fn validate_config(config: &RconConfig) -> Result<(), String> {
    if !config.enabled {
        return Err("RCON отключён в server.properties (enable-rcon=false).".to_string());
    }
    if config.password.is_empty() {
        return Err("В server.properties не задан rcon.password.".to_string());
    }
    Ok(())
}

fn config_detail(config: &RconConfig, path: &str, ssh_alias: &str, connect_host: &str) -> String {
    format!(
        "Файл: {path}. enable-rcon={}, rcon.port={}, пароль задан. Подключение: {connect_host} (SSH: {ssh_alias}). IP в server.properties не нужен.",
        config.enabled, config.port
    )
}

fn map_io_error(error: std::io::Error, context: &str) -> String {
    match error.kind() {
        ErrorKind::WouldBlock | ErrorKind::TimedOut => format!(
            "Таймаут RCON ({context}). Порт не отвечает как RCON — перезапустите сервер после enable-rcon=true или проверьте rcon.port."
        ),
        ErrorKind::ConnectionRefused => format!(
            "RCON недоступен ({context}). Проверьте enable-rcon, порт и фаервол Windows."
        ),
        ErrorKind::ConnectionReset => format!("RCON разорвал соединение ({context})."),
        _ => format!("Ошибка RCON ({context}): {error}"),
    }
}

fn configure_stream(stream: &TcpStream) -> Result<(), String> {
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .map_err(|error| map_io_error(error, "настройка сокета"))?;
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .map_err(|error| map_io_error(error, "настройка сокета"))?;
    let _ = stream.set_nodelay(true);
    Ok(())
}

fn connect_tcp(host: &str, port: u16) -> Result<TcpStream, String> {
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("Не удалось разрешить адрес {host}:{port}: {error}"))?;
    let mut last_error = String::new();
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
            Ok(stream) => {
                configure_stream(&stream)?;
                return Ok(stream);
            }
            Err(error) => last_error = error.to_string(),
        }
    }
    Err(format!(
        "Не удалось подключиться к {host}:{port}. {last_error}"
    ))
}

fn pick_local_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("Не удалось открыть локальный порт: {error}"))
        .map(|listener| listener.local_addr().map(|addr| addr.port()).unwrap_or(0))
}

fn open_ssh_tunnel(ssh_alias: &str, remote_port: u16) -> Result<SshTunnel, String> {
    ensure_ssh_host(ssh_alias)?;
    let local_port = pick_local_port()?;
    if local_port == 0 {
        return Err("Не удалось выбрать локальный порт для SSH-туннеля.".to_string());
    }
    let forward = format!("127.0.0.1:{local_port}:127.0.0.1:{remote_port}");
    let control_path = format!("ControlPath={}", ssh_control_path(ssh_alias).display());
    let child = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ControlMaster=auto",
            "-o",
            control_path.as_str(),
            "-o",
            "ControlPersist=120",
            "-o",
            "ConnectTimeout=15",
            "-N",
            "-L",
            &forward,
            ssh_alias,
        ])
        .spawn()
        .map_err(|error| format!("SSH-туннель: {error}"))?;
    std::thread::sleep(Duration::from_millis(500));
    Ok(SshTunnel { child, local_port })
}

fn is_rcon_port_listening(host: &str, port: u16, os: RemoteOs) -> Result<bool, String> {
    let output = match os {
        RemoteOs::Windows => {
            let cmd = format!(
                "powershell -NoProfile -NonInteractive -Command \"if (Get-NetTCPConnection -LocalPort {port} -State Listen -ErrorAction SilentlyContinue) {{ exit 0 }} else {{ exit 1 }}\""
            );
            ssh_command(host, &cmd)?
        }
        RemoteOs::Linux => {
            let cmd = format!(
                "ss -ltn 2>/dev/null | grep ':{port} ' || netstat -ltn 2>/dev/null | grep ':{port} '"
            );
            ssh_command(host, &cmd)?
        }
    };
    Ok(output.status.success())
}

fn port_not_listening_message(port: u16) -> String {
    format!(
        "Порт {port} не слушается на сервере. В server.properties нужны enable-rcon=true и rcon.password, затем полный перезапуск сервера (stop и start)."
    )
}

fn open_tunnel_connection(ssh_alias: &str, port: u16) -> Result<RconConnection, String> {
    let tunnel = open_ssh_tunnel(ssh_alias, port)?;
    let local_port = tunnel.local_port;
    let stream = connect_tcp("127.0.0.1", local_port).map_err(|error| {
        format!("SSH-туннель к {ssh_alias}:{port} не открылся: {error}")
    })?;
    Ok(RconConnection {
        stream,
        tunnel: Some(tunnel),
        connect_label: format!("127.0.0.1:{local_port} → {ssh_alias}:{port}"),
        via_tunnel: true,
    })
}

fn open_direct_connection(host: &str, port: u16) -> Result<RconConnection, String> {
    let stream = connect_tcp(host, port)?;
    Ok(RconConnection {
        stream,
        tunnel: None,
        connect_label: format!("{host}:{port}"),
        via_tunnel: false,
    })
}

fn connect_rcon(ssh_alias: &str, port: u16, password: &str, os: RemoteOs) -> Result<RconConnection, String> {
    if !is_rcon_port_listening(ssh_alias, port, os)? {
        return Err(port_not_listening_message(port));
    }

    let direct_host = resolve_rcon_host(ssh_alias);
    let mut errors: Vec<String> = Vec::new();

    match open_tunnel_connection(ssh_alias, port) {
        Ok(mut conn) => {
            match authenticate(&mut conn.stream, password) {
                Ok(()) => return Ok(conn),
                Err(error) => errors.push(format!("SSH-туннель: {error}")),
            }
        }
        Err(error) => errors.push(format!("SSH-туннель: {error}")),
    }

    match open_direct_connection(&direct_host, port) {
        Ok(mut conn) => {
            match authenticate(&mut conn.stream, password) {
                Ok(()) => return Ok(conn),
                Err(error) => errors.push(format!("{direct_host}:{port}: {error}")),
            }
        }
        Err(error) => errors.push(format!("{direct_host}:{port}: {error}")),
    }

    Err(if errors.is_empty() {
        port_not_listening_message(port)
    } else {
        errors.join(" ")
    })
}

fn write_packet(stream: &mut TcpStream, id: i32, packet_type: i32, body: &str) -> Result<(), String> {
    let body_bytes = body.as_bytes();
    let length = (4 + 4 + body_bytes.len() + 2) as i32;
    stream
        .write_all(&length.to_le_bytes())
        .map_err(|error| map_io_error(error, "отправка"))?;
    stream
        .write_all(&id.to_le_bytes())
        .map_err(|error| map_io_error(error, "отправка"))?;
    stream
        .write_all(&packet_type.to_le_bytes())
        .map_err(|error| map_io_error(error, "отправка"))?;
    stream
        .write_all(body_bytes)
        .map_err(|error| map_io_error(error, "отправка"))?;
    stream
        .write_all(&[0, 0])
        .map_err(|error| map_io_error(error, "отправка"))?;
    Ok(())
}

fn read_packet(stream: &mut TcpStream) -> Result<(i32, i32, String), String> {
    let mut length_buf = [0_u8; 4];
    stream
        .read_exact(&mut length_buf)
        .map_err(|error| map_io_error(error, "чтение ответа"))?;
    let length = i32::from_le_bytes(length_buf);
    if !(10..=4096).contains(&length) {
        return Err("RCON: некорректный ответ сервера.".to_string());
    }
    let mut rest = vec![0_u8; length as usize];
    stream
        .read_exact(&mut rest)
        .map_err(|error| map_io_error(error, "чтение ответа"))?;
    let id = i32::from_le_bytes(rest[0..4].try_into().expect("packet id"));
    let packet_type = i32::from_le_bytes(rest[4..8].try_into().expect("packet type"));
    let body_end = rest.len().saturating_sub(2);
    let body = if body_end > 8 {
        String::from_utf8_lossy(&rest[8..body_end]).to_string()
    } else {
        String::new()
    };
    Ok((id, packet_type, body))
}

fn authenticate(stream: &mut TcpStream, password: &str) -> Result<(), String> {
    write_packet(stream, 1, SERVERDATA_AUTH, password)?;
    let (id, _, _) = read_packet(stream)?;
    if id == -1 {
        return Err("Неверный пароль RCON.".to_string());
    }
    // Некоторые серверы шлют второй пустой пакет после auth.
    let _ = stream.set_read_timeout(Some(Duration::from_millis(400)));
    let _ = read_packet(stream);
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    Ok(())
}

fn exec_on_connection(connection: &mut RconConnection, command: &str) -> Result<String, String> {
    write_packet(
        &mut connection.stream,
        2,
        SERVERDATA_EXECCOMMAND,
        command.trim(),
    )?;

    let mut output = String::new();
    for _ in 0..MAX_RESPONSE_PACKETS {
        let (_, packet_type, body) = read_packet(&mut connection.stream)?;
        if packet_type != SERVERDATA_RESPONSE_VALUE {
            continue;
        }
        if body.is_empty() {
            break;
        }
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&body);
    }
    Ok(output)
}

pub(crate) fn test_rcon(
    ssh_alias: &str,
    server_root: &str,
    os: RemoteOs,
) -> Result<RconCheckDetails, String> {
    let path = server_properties_remote_path(server_root);
    let config = load_rcon_config(ssh_alias, server_root, os)?;
    validate_config(&config)?;
    let connect_host = resolve_rcon_host(ssh_alias);
    let detail = config_detail(&config, &path, ssh_alias, &connect_host);

    let connection = connect_rcon(ssh_alias, config.port, &config.password, os)?;

    let tunnel_note = if connection.via_tunnel {
        " Через SSH-туннель (порт доступен только на сервере)."
    } else {
        ""
    };

    Ok(RconCheckDetails {
        port: config.port,
        connect_host: connection.connect_label.clone(),
        ssh_alias: ssh_alias.to_string(),
        via_tunnel: connection.via_tunnel,
        properties_path: path,
        detail,
        message: format!(
            "RCON доступен ({}){}",
            connection.connect_label, tunnel_note
        ),
    })
}

pub(crate) fn send_rcon_command(
    ssh_alias: &str,
    server_root: &str,
    os: RemoteOs,
    command: &str,
) -> Result<String, String> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err("Введите команду.".to_string());
    }
    let config = load_rcon_config(ssh_alias, server_root, os)?;
    validate_config(&config)?;
    let mut connection = connect_rcon(ssh_alias, config.port, &config.password, os)?;
    exec_on_connection(&mut connection, trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rcon_settings_from_properties() {
        let content = "# comment\nenable-rcon=true\nrcon.port=25580\nrcon.password=secret\\:pass\n";
        let config = parse_server_properties(content);
        assert!(config.enabled);
        assert_eq!(config.port, 25580);
        assert_eq!(config.password, "secret:pass");
    }

    #[test]
    fn defaults_rcon_disabled() {
        let config = parse_server_properties("max-players=20\n");
        assert!(!config.enabled);
        assert_eq!(config.port, 25575);
        assert!(config.password.is_empty());
    }
}
