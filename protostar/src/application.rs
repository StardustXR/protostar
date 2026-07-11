use crate::xdg::{DesktopFile, Icon, IconType};
use regex::Regex;
use serde::{Deserialize, Serialize};
use stardust_xr_fusion::{
	Error as NodeError,
	client::{Client, ClientHandler},
	spatial::SpatialRef,
	types::ResourceLoadError,
};
use std::{borrow::Cow, collections::HashMap, env};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Application {
	desktop_file: DesktopFile,
}
impl Application {
	pub fn create(desktop_file: DesktopFile) -> Result<Self, NodeError> {
		if desktop_file.no_display {
			return Err(ResourceLoadError::NotFound.into());
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

	pub fn launch(
		&self,
		client: &Client<impl ClientHandler>,
		launch_space: &SpatialRef,
	) -> Result<(), NodeError> {
		let launch_space = launch_space.clone();

		let executable = self
			.desktop_file
			.command
			.clone()
			.ok_or(ResourceLoadError::NotFound)?;
		let server_interface = client.server().clone();
		tokio::task::spawn(async move {
			let Ok(Ok(startup_token)) = server_interface
				.generate_startup_token(launch_space.clone())
				.await
			else {
				return;
			};
			// Strip/ignore field codes https://specifications.freedesktop.org/desktop-entry-spec/latest/ar01s07.html
			let re = Regex::new(r"%[fFuUdDnNickvm]").unwrap();
			let exec: Cow<'_, str> = re.replace_all(&executable, "");

			let mut connection_env = HashMap::new();
			connection_env.insert("STARDUST_STARTUP_TOKEN".to_string(), startup_token);
			if let Ok(v) = env::var("WAYLAND_DISPLAY") {
				connection_env.insert("WAYLAND_DISPLAY".to_string(), v);
			}
			if let Ok(v) = env::var("DISPLAY") {
				connection_env.insert("DISPLAY".to_string(), v);
			}
			if let Ok(v) = env::var("XDG_CURRENT_DESKTOP") {
				connection_env.insert("XDG_CURRENT_DESKTOP".to_string(), v);
			}
			protostar_launcher::launch(exec, connection_env).await
		});

		Ok(())
	}
}
