pub mod app;
pub mod hex;

use app::App;
use color_eyre::eyre::Result;
use glam::Quat;
use hex::{HEX_CENTER, HEX_DIRECTION_VECTORS};
use manifest_dir_macros::directory_relative_path;
use protostar::xdg::{get_desktop_files, parse_desktop_file, DesktopFile};
use serde::{Deserialize, Serialize};
use stardust_xr_fusion::{
	client::{Client, ClientState, FrameInfo, RootHandler},
	core::{
		schemas::flex::flexbuffers,
		values::{
			color::{color_space::LinearRgb, rgba_linear, Rgba},
			ResourceID,
		},
	},
	drawable::{MaterialParameter, Model, ModelPartAspect},
	node::{NodeError, NodeType},
	spatial::{Spatial, SpatialAspect, Transform},
};
use stardust_xr_molecules::{
	button::{Button, ButtonSettings},
	Grabbable, GrabbableSettings, PointerMode,
};
use std::{f32::consts::PI, time::Duration};

const APP_SIZE: f32 = 0.06;
const PADDING: f32 = 0.005;
const MODEL_SCALE: f32 = 0.03;
const ACTIVATION_DISTANCE: f32 = 0.05;

const DEFAULT_HEX_COLOR: Rgba<f32, LinearRgb> = rgba_linear!(0.211, 0.937, 0.588, 1.0);
const BTN_SELECTED_COLOR: Rgba<f32, LinearRgb> = rgba_linear!(0.0, 1.0, 0.0, 1.0);
const BTN_COLOR: Rgba<f32, LinearRgb> = rgba_linear!(1.0, 1.0, 0.0, 1.0);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	color_eyre::install().unwrap();
	tracing_subscriber::fmt()
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.pretty()
		.init();
	let (client, event_loop) = Client::connect_with_async_loop().await?;
	client.set_base_prefixes(&[directory_relative_path!("../res")]);

	let _root = client.wrap_root(AppHexGrid::new(&client).await)?;

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e??,
	}
	Ok(())
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
	unfurled: bool,
}

struct AppHexGrid {
	movable_root: Spatial,
	apps: Vec<App>,
	button: CenterButton,
	state: State,
}
impl AppHexGrid {
	async fn new(client: &Client) -> Self {
		let state = flexbuffers::from_slice(&client.state().data).unwrap_or_default();

		let movable_root =
			Spatial::create(client.get_root(), Transform::identity(), false).unwrap();

		let button = CenterButton::new(client, &client.state()).unwrap();
		tokio::time::sleep(Duration::from_millis(10)).await; // give it a bit of time to send the messages properly

		let mut desktop_files: Vec<DesktopFile> = get_desktop_files()
			.filter_map(|d| parse_desktop_file(d).ok())
			.filter(|d| !d.no_display)
			.collect();

		desktop_files.sort_by_key(|d| d.clone().name.unwrap_or_default());

		let mut apps = Vec::new();
		let mut radius = 1;
		while !desktop_files.is_empty() {
			let mut hex = HEX_CENTER.add(&HEX_DIRECTION_VECTORS[4].clone().scale(radius));
			for i in 0..6 {
				if desktop_files.is_empty() {
					break;
				};
				for _ in 0..radius {
					if desktop_files.is_empty() {
						break;
					};
					apps.push(
						App::create_from_desktop_file(
							button.grabbable.content_parent(),
							hex.get_coords(),
							desktop_files.pop().unwrap(),
							&state,
						)
						.unwrap(),
					);
					hex = hex.neighbor(i);
				}
			}
			radius += 1;
		}
		AppHexGrid {
			movable_root,
			apps,
			button,
			state,
		}
	}
}
impl RootHandler for AppHexGrid {
	fn frame(&mut self, info: FrameInfo) {
		self.button.frame(info);
		if self.button.button.pressed() {
			self.button
				.model
				.model_part("Hex")
				.unwrap()
				.set_material_parameter("color", MaterialParameter::Color(BTN_SELECTED_COLOR))
				.unwrap();
			self.state.unfurled = !self.state.unfurled;
			for app in &mut self.apps {
				app.apply_state(&self.state);
			}
		} else if self.button.button.released() {
			self.button
				.model
				.model_part("Hex")
				.unwrap()
				.set_material_parameter("color", MaterialParameter::Color(BTN_COLOR))
				.unwrap();
		}
		for app in &mut self.apps {
			app.frame(info, &self.state);
		}
	}

	fn save_state(&mut self) -> ClientState {
		self.movable_root
			.set_relative_transform(
				self.button.grabbable.content_parent(),
				Transform::from_translation([0.0; 3]),
			)
			.unwrap();
		ClientState {
			data: flexbuffers::to_vec(&self.state).unwrap(),
			root: self.movable_root.alias(),
			spatial_anchors: [(
				"content_parent".to_string(),
				self.button.grabbable.content_parent().alias(),
			)]
			.into_iter()
			.collect(),
		}
	}
}

struct CenterButton {
	button: Button,
	grabbable: Grabbable,
	model: Model,
}
impl CenterButton {
	fn new(client: &Client, state: &ClientState) -> Result<Self, NodeError> {
		// (APP_SIZE + PADDING) / 2.0,
		let button = Button::create(
			client.get_root(),
			Transform::identity(),
			[(APP_SIZE + PADDING) / 2.0; 2],
			ButtonSettings {
				visuals: None,
				..Default::default()
			},
		)?;
		let grabbable = Grabbable::create(
			client.get_root(),
			Transform::none(),
			&button.touch_plane().field(),
			GrabbableSettings {
				max_distance: 0.025,
				pointer_mode: PointerMode::Align,
				magnet: false,
				..Default::default()
			},
		)?;
		button
			.touch_plane()
			.root()
			.set_spatial_parent(grabbable.content_parent())?;

		let model = Model::create(
			grabbable.content_parent(),
			Transform::from_rotation_scale(
				Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
				[MODEL_SCALE; 3],
			),
			&ResourceID::new_namespaced("protostar", "hexagon/hexagon"),
		)?;
		model
			.model_part("Hex")?
			.set_material_parameter("color", MaterialParameter::Color(BTN_COLOR))?;
		if let Some(content_parent) = state.spatial_anchors.get("content_parent") {
			grabbable
				.content_parent()
				.set_relative_transform(content_parent, Transform::identity())?;
		}
		Ok(CenterButton {
			button,
			grabbable,
			model,
		})
	}

	fn frame(&mut self, info: FrameInfo) {
		let _ = self.grabbable.update(&info);
		self.button.update();
	}
}
