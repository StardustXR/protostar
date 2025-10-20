use crate::xdg::{DesktopFile, Icon, IconType};
use nix::{libc::setsid, unistd::ForkResult};
use regex::Regex;
use serde::{Deserialize, Serialize};
use stardust_xr_fusion::{
	node::{NodeError, NodeResult},
	root::{ClientState, RootAspect},
	spatial::SpatialRefAspect,
};
use std::{
	os::unix::process::CommandExt,
	process::{Command, Stdio, exit},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Application {
	desktop_file: DesktopFile,
}
impl Application {
	pub fn create(desktop_file: DesktopFile) -> Result<Self, NodeError> {
		if desktop_file.no_display {
			return Err(NodeError::DoesNotExist);
		}

		Ok(Application { desktop_file })
	}

	pub fn name(&self) -> Option<&str> {
		self.desktop_file.name.as_deref()
	}
	pub fn categories(&self) -> &[String] {
		self.desktop_file.categories.as_slice()
	}

	pub fn icon(&self, preferred_px_size: u16, prefer_3d: bool) -> Option<Icon> {
		let raw_icons = self.desktop_file.get_icon(preferred_px_size);
		let mut icon = raw_icons.iter().max_by_key(|i| i.size).cloned();
		if prefer_3d {
			icon = raw_icons
				.into_iter()
				.find(|i| i.icon_type == IconType::Gltf)
				.or(icon);
		}

		icon.and_then(|i| i.cached_process(preferred_px_size).ok())
	}

	pub fn launch<T: SpatialRefAspect + Clone>(&self, launch_space: &T) -> NodeResult<()> {
		let client = launch_space.client().clone();
		let launch_space = launch_space.clone();

		let executable = self
			.desktop_file
			.command
			.clone()
			.ok_or(NodeError::DoesNotExist)?;
		tokio::task::spawn(async move {
			let Ok(startup_token) = client
				.get_root()
				.generate_state_token(ClientState::from_root(&launch_space).unwrap())
				.await
			else {
				return;
			};

			let Ok(connection_env) = client.get_root().get_connection_environment().await else {
				return;
			};
			for (k, v) in connection_env.into_iter() {
				// this should be fine, probably?
				unsafe {
					std::env::set_var(k, v);
				}
			}

			// this should be fine, probably?
			unsafe {
				std::env::set_var("STARDUST_STARTUP_TOKEN", startup_token);
			}

			// Strip/ignore field codes https://specifications.freedesktop.org/desktop-entry-spec/latest/ar01s07.html
			let re = Regex::new(r"%[fFuUdDnNickvm]").unwrap();
			let exec: std::borrow::Cow<'_, str> = re.replace_all(&executable, "");

			unsafe {
				if let ForkResult::Child = nix::unistd::fork().expect("fork died???? how?????") {
					let _ = Command::new("sh")
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
					exit(0);
				}
			}
		});

		Ok(())
	}
}
