use crate::xdg::{DesktopFile, Icon, IconType};
use nix::unistd::setsid;
use regex::Regex;
use stardust_xr_fusion::{
	client::Client,
	node::{NodeError, NodeType},
	spatial::Spatial,
	startup_settings::StartupSettings,
};
use std::{
	os::unix::process::CommandExt,
	process::{Command, Stdio},
	sync::Arc,
};

#[derive(Debug, Clone)]
pub struct Application {
	desktop_file: DesktopFile,
	startup_settings: Arc<StartupSettings>,
}
impl Application {
	pub fn create(client: &Arc<Client>, desktop_file: DesktopFile) -> Result<Self, NodeError> {
		if desktop_file.no_display {
			return Err(NodeError::DoesNotExist);
		}

		let startup_settings = Arc::new(StartupSettings::create(client)?);
		Ok(Application {
			desktop_file,
			startup_settings,
		})
	}

	pub fn name(&self) -> Option<&str> {
		self.desktop_file.name.as_deref()
	}
	pub fn categories(&self) -> &[String] {
		self.desktop_file.categories.as_slice()
	}

	pub fn icon(&self, preferred_px_size: u16, prefer_3d: bool) -> Option<Icon> {
		let raw_icons = self.desktop_file.get_raw_icons(preferred_px_size);
		let mut icon = raw_icons.iter().max_by_key(|i| i.size).cloned();
		if prefer_3d {
			icon = raw_icons
				.into_iter()
				.find(|i| match i.icon_type {
					IconType::Gltf => true,
					_ => false,
				})
				.or(icon);
		}

		icon.and_then(|i| i.cached_process(preferred_px_size).ok())
	}

	pub fn launch(&self, launch_space: &Spatial) -> Result<(), NodeError> {
		self.startup_settings.set_root(launch_space)?;
		let future_startup_token = self.startup_settings.generate_startup_token()?;
		let future_connection_env = self
			.startup_settings
			.node()
			.client()?
			.get_connection_environment()?;

		let executable = self
			.desktop_file
			.command
			.clone()
			.ok_or(NodeError::DoesNotExist)?;
		tokio::task::spawn(async move {
			let Ok(startup_token) = future_startup_token.await else {return};
			let Ok(connection_env) = future_connection_env.await else {return};
			dbg!(&connection_env);
			for (k, v) in connection_env.into_iter() {
				std::env::set_var(k, v);
			}

			std::env::set_var("STARDUST_STARTUP_TOKEN", startup_token);
			let re = Regex::new(r"%[fFuUdDnNickvm]").unwrap();
			let exec = re.replace_all(&executable, "");
			unsafe {
				Command::new("sh")
					.arg("-c")
					.arg(exec.to_string())
					.stdin(Stdio::null())
					.stdout(Stdio::null())
					.stderr(Stdio::null())
					.pre_exec(|| {
						_ = setsid();
						Ok(())
					})
					.spawn()
					.expect("Failed to start child process");
			}
		});

		Ok(())
	}
}
