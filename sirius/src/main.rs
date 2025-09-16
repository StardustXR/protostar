use stardust_xr_asteroids::{
	client, elements::{Button, Grabbable, Model, ModelPart, PointerMode, Spatial}, ClientState, CustomElement, Element, Identifiable as _, Migrate, Reify, Transformable
};
use clap::Parser;
use glam::Quat;
use mint::{Quaternion, Vector3};
use protostar::xdg::DesktopFile;
use serde::{Deserialize, Serialize};
use single::{App, BTN_COLOR, BTN_SELECTED_COLOR};
use stardust_xr_fusion::{
	drawable::MaterialParameter, fields::Shape, project_local_resources, spatial::Transform,
};
use std::path::PathBuf;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};
use walkdir::WalkDir;

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

	client::run::<Sirius>(&[&project_local_resources!("../res")]).await
}

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
	/// Directory to scan for desktop files
	apps_directory: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sirius {
	visible: bool,
	pos: Vector3<f32>,
	rot: Quaternion<f32>,
	#[serde(skip)]
	apps: Vec<App>,
}

impl Default for Sirius {
	fn default() -> Self {
		Self {
			visible: false,
			pos: [0.0; 3].into(),
			rot: Quat::IDENTITY.into(),
			apps: Vec::new(),
		}
	}
}

impl Migrate for Sirius {
	type Old = Self;
}

impl ClientState for Sirius {
	const APP_ID: &'static str = "org.protostar.sirius";

	fn initial_state_update(&mut self) {
		let args = Args::parse();
		if !args.apps_directory.is_dir() {
			panic!(
				"{} is not a directory",
				args.apps_directory.to_string_lossy()
			)
		}

		let walkdir = WalkDir::new(args.apps_directory.canonicalize().unwrap());

		self.apps = walkdir
			.into_iter()
			.filter_map(|path| path.ok())
			.map(|entry| entry.into_path())
			.filter(|path| {
				path.is_file()
					&& path.extension().is_some()
					&& path.extension().unwrap() == "desktop"
			})
			.filter_map(|path| App::new(DesktopFile::parse(path).ok()?).ok())
			.collect();
	}
}
impl Reify for Sirius {
	fn reify(&self) -> impl Element<Self> {
		Grabbable::new(
			Shape::Box([0.1; 3].into()),
			self.pos,
			self.rot,
			|state: &mut Self, pos, rot| {
				state.pos = pos;
				state.rot = rot;
			},
		)
		.pointer_mode(PointerMode::Align)
		.zoneable(false)
		.build()
		.child(
			Button::new(|state: &mut Sirius| {
				state.visible = !state.visible;
			})
			.pos([0.0, 0.0, 0.005])
			.size([0.1; 2])
			.build(),
		)
		.child(
			Model::namespaced("protostar", "button")
				.transform(Transform::identity())
				.part(ModelPart::new("?????").mat_param(
					"color",
					MaterialParameter::Color(if self.visible {
						BTN_SELECTED_COLOR
					} else {
						BTN_COLOR
					}),
				))
				.build(),
		)
		.children(
			self.visible
				.then(|| {
					self.apps.iter().enumerate().map(|(pos, app)| {
						let mut starpos = (pos as f32 + 1.0) / 10.0;
						match starpos % 0.2 == 0.0 {
							true => starpos = -starpos / 2.0,
							false => starpos = (starpos - 0.1) / 2.0,
						}

						Spatial::default()
							.pos([starpos, 0.1, 0.0])
							.build()
							.identify(&app.app.name())
							.child(
								app.reify_substate(move |state: &mut Sirius| {
									state.apps.get_mut(pos)
								}),
							)
					})
				})
				.into_iter()
				.flatten(),
		)
	}
}
