mod linux;
mod windows;

use super::os::RemoteOs;

pub(crate) trait RemoteServerBackend {
    fn validate_server_root(&self, host: &str, server_root: &str) -> Result<(), String>;
    fn is_running(&self, host: &str, server_root: &str) -> Result<bool, String>;
    fn start(
        &self,
        host: &str,
        server_root: &str,
        start: &crate::server_control::start_config::ServerStartConfig,
    ) -> Result<(), String>;
    fn stop(
        &self,
        host: &str,
        server_root: &str,
        start: &crate::server_control::start_config::ServerStartConfig,
    ) -> Result<(), String>;
}

struct WindowsBackend;
struct LinuxBackend;

pub(crate) fn backend_for(os: RemoteOs) -> Box<dyn RemoteServerBackend> {
    match os {
        RemoteOs::Windows => Box::new(WindowsBackend),
        RemoteOs::Linux => Box::new(LinuxBackend),
    }
}

impl RemoteServerBackend for WindowsBackend {
    fn validate_server_root(&self, host: &str, server_root: &str) -> Result<(), String> {
        windows::validate_server_root(host, server_root)
    }

    fn is_running(&self, host: &str, server_root: &str) -> Result<bool, String> {
        windows::is_running(host, server_root)
    }

    fn start(
        &self,
        host: &str,
        server_root: &str,
        start: &crate::server_control::start_config::ServerStartConfig,
    ) -> Result<(), String> {
        windows::start(host, server_root, start)
    }

    fn stop(
        &self,
        host: &str,
        server_root: &str,
        start: &crate::server_control::start_config::ServerStartConfig,
    ) -> Result<(), String> {
        windows::stop(host, server_root, start)
    }
}

impl RemoteServerBackend for LinuxBackend {
    fn validate_server_root(&self, host: &str, server_root: &str) -> Result<(), String> {
        linux::validate_server_root(host, server_root)
    }

    fn is_running(&self, host: &str, server_root: &str) -> Result<bool, String> {
        linux::is_running(host, server_root)
    }

    fn start(
        &self,
        host: &str,
        server_root: &str,
        start: &crate::server_control::start_config::ServerStartConfig,
    ) -> Result<(), String> {
        linux::start(host, server_root, start)
    }

    fn stop(
        &self,
        host: &str,
        server_root: &str,
        start: &crate::server_control::start_config::ServerStartConfig,
    ) -> Result<(), String> {
        linux::stop(host, server_root, start)
    }
}
