mod hex;

use glam::Quat;
use hex::Hex;
use mint::{Quaternion, Vector3};
use protostar::xdg::{DesktopFile, get_desktop_files};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use single::{APP_SIZE, App, BTN_COLOR, BTN_SELECTED_COLOR, MODEL_SCALE};
use stardust_xr_asteroids::{
	ClientState, CustomElement, Element, Migrate, Reify, Transformable, client,
	elements::{Button, Grabbable, Model, ModelPart, PointerMode, Spatial},
};
use stardust_xr_fusion::{
	drawable::MaterialParameter,
	fields::{CylinderShape, Shape},
	project_local_resources,
	spatial::Transform,
};
use std::f32::consts::{FRAC_PI_2, PI};
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main(flavor = "current_thread")]
async fn main() {
	color_eyre::install().unwrap();

	let registry = tracing_subscriber::registry();
	#[cfg(feature = "tracy")]
	let registry = registry.with({
		use tracing_subscriber::Layer;
		tracing_tracy::TracyLayer::new(tracing_tracy::DefaultConfig::default())
			.with_filter(tracing::level_filters::LevelFilter::DEBUG)
	});
	let log_layer = tracing_subscriber::fmt::Layer::new()
		.with_thread_names(true)
		.with_ansi(true)
		.with_line_number(true)
		.with_filter(EnvFilter::from_default_env());
	registry.with(log_layer).init();

	client::run::<HexagonLauncher>(&[&project_local_resources!("../res")]).await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HexagonLauncher {
	/// if the hexagon launcher is expanded
	open: bool,
	pos: Vector3<f32>,
	rot: Quaternion<f32>,
	#[serde(skip)]
	/// position in the vector is mapped to hex coordinates
	apps: Vec<App>,
}

impl Default for HexagonLauncher {
	fn default() -> Self {
		Self {
			open: false,
			pos: [0.0; 3].into(),
			rot: Quat::IDENTITY.into(),
			apps: Vec::new(),
		}
	}
}
impl Migrate for HexagonLauncher {
	type Old = Self;
}

impl ClientState for HexagonLauncher {
	const APP_ID: &'static str = "org.protostar.hexagon_launcher";

	fn initial_state_update(&mut self) {
		// Load desktop files
		self.apps = get_desktop_files()
			.filter_map(|d| DesktopFile::parse(d).ok())
			.filter(|d| !d.no_display)
			.filter_map(|d| App::new(d).ok())
			.collect();

		self.apps.par_iter().for_each(|app| {
			app.load_icon();
		});

		// Sort by name
		self.apps
			.sort_by_key(|app| app.app.name().unwrap_or_default().to_string());
	}
}
impl Reify for HexagonLauncher {
	#[tracing::instrument(skip_all)]
	fn reify(&self) -> impl Element<Self> {
		// Build UI based on current state
		Grabbable::new(
			Shape::Cylinder(CylinderShape {
				radius: APP_SIZE / 2.0,
				length: 0.01,
			}),
			self.pos,
			self.rot,
			|state: &mut Self, pos, rot| {
				state.pos = pos;
				state.rot = rot;
			},
		)
		.field_transform(Transform::from_rotation(Quat::from_rotation_x(FRAC_PI_2)))
		.pointer_mode(PointerMode::Align)
		.zoneable(false)
		.build()
		.child(
			Button::new(|state: &mut HexagonLauncher| {
				state.open = !state.open;
			})
			.pos([0.0, 0.0, 0.005])
			.size([APP_SIZE / 2.0; 2])
			.build(),
		)
		.child(
			Model::namespaced("protostar", "hexagon/hexagon")
				.transform(Transform::from_rotation_scale(
					Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
					[MODEL_SCALE; 3],
				))
				.part(ModelPart::new("Hex").mat_param(
					"color",
					MaterialParameter::Color(if self.open {
						BTN_SELECTED_COLOR
					} else {
						BTN_COLOR
					}),
				))
				.build(),
		)
		.children(
			self.open
				.then(|| {
					self.apps.iter().enumerate().map(|(i, app)| {
						Spatial::default()
							.pos(Hex::spiral(i + 1).get_coords())
							.build()
							.child(app.reify_substate(move |state: &mut HexagonLauncher| {
								state.apps.get_mut(i)
							}))
					})
				})
				.into_iter()
				.flatten(),
		)
	}
}
