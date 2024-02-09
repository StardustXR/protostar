pub mod app;
pub mod hex;

use app::App;
use color::{color_space::LinearRgb, rgba_linear, Rgba};
use color_eyre::eyre::Result;
use glam::Quat;
use hex::{HEX_CENTER, HEX_DIRECTION_VECTORS};
use manifest_dir_macros::directory_relative_path;
use protostar::xdg::{get_desktop_files, parse_desktop_file, DesktopFile};
use stardust_xr_fusion::{
	client::{Client, ClientState, FrameInfo, RootHandler},
	core::values::ResourceID,
	drawable::{MaterialParameter, Model, ModelPartAspect},
	fields::BoxField,
	node::NodeError,
	spatial::{SpatialAspect, Transform},
};
use stardust_xr_molecules::{touch_plane::TouchPlane, Grabbable, GrabbableSettings, PointerMode};
use std::f32::consts::PI;

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
	client.set_base_prefixes(&[directory_relative_path!("res")]);

	let _root = client.wrap_root(AppHexGrid::new(&client))?;

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e??,
	};
	Ok(())
}

struct AppHexGrid {
	apps: Vec<App>,
	button: Button,
}
impl AppHexGrid {
	fn new(client: &Client) -> Self {
		let button = Button::new(client).unwrap();
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
						)
						.unwrap(),
					);
					hex = hex.neighbor(i);
				}
			}
			radius += 1;
		}
		AppHexGrid { apps, button }
	}
}
impl RootHandler for AppHexGrid {
	fn frame(&mut self, info: FrameInfo) {
		self.button.frame(info);
		if self.button.touch_plane.touch_started() {
			self.button
				.model
				.model_part("Hex")
				.unwrap()
				.set_material_parameter("color", MaterialParameter::Color(BTN_SELECTED_COLOR))
				.unwrap();
			for app in &mut self.apps {
				app.toggle();
			}
		} else if self.button.touch_plane.touch_stopped() {
			self.button
				.model
				.model_part("Hex")
				.unwrap()
				.set_material_parameter("color", MaterialParameter::Color(BTN_COLOR))
				.unwrap();
		}
		for app in &mut self.apps {
			app.frame(info);
		}
	}

	fn save_state(&mut self) -> ClientState {
		ClientState::default()
	}
}

struct Button {
	touch_plane: TouchPlane,
	grabbable: Grabbable,
	model: Model,
}
impl Button {
	fn new(client: &Client) -> Result<Self, NodeError> {
		let field = BoxField::create(client.get_root(), Transform::identity(), [APP_SIZE; 3])?;
		let grabbable = Grabbable::create(
			client.get_root(),
			Transform::none(),
			&field,
			GrabbableSettings {
				max_distance: 0.01,
				pointer_mode: PointerMode::Align,
				magnet: false,
				..Default::default()
			},
		)?;
		field.set_spatial_parent(grabbable.content_parent())?;
		let touch_plane = TouchPlane::create(
			grabbable.content_parent(),
			Transform::identity(),
			[(APP_SIZE + PADDING) / 2.0; 2],
			(APP_SIZE + PADDING) / 2.0,
			0.0..1.0,
			0.0..1.0,
		)?;

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
		Ok(Button {
			touch_plane,
			grabbable,
			model,
		})
	}

	fn frame(&mut self, info: FrameInfo) {
		let _ = self.grabbable.update(&info);
		if self.grabbable.grab_action().actor_started() {
			let _ = self.touch_plane.set_enabled(false);
		}
		if self.grabbable.grab_action().actor_stopped() {
			let _ = self.touch_plane.set_enabled(true);
		}
		self.touch_plane.update();
	}
}
