#[derive(Clone, Debug)]
pub(crate) struct ServerStartConfig {
    pub launch_script: String,
}

#[derive(Clone, Debug)]
pub(crate) struct LaunchCommand {
    pub script: String,
    pub extra_args: Vec<String>,
}

impl ServerStartConfig {
    pub(crate) fn from_settings(launch_script: &str) -> Self {
        Self {
            launch_script: launch_script.trim().to_string(),
        }
    }

    pub(crate) fn validate_for_start(&self) -> Result<(), String> {
        if self.launch_script.trim().is_empty() {
            return Err("Укажите скрипт запуска в корне сервера.".to_string());
        }
        Ok(())
    }

    pub(crate) fn launch_command(&self) -> LaunchCommand {
        let script = self.launch_script.trim().to_string();
        let name = script
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(script.as_str())
            .to_ascii_lowercase();
        let extra_args = if name == "run.bat" || name == "run.sh" {
            vec!["nogui".to_string()]
        } else {
            Vec::new()
        };
        LaunchCommand { script, extra_args }
    }
}
