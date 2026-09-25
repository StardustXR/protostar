mod hex;

use glam::{Mat4, Quat};
use hex::Hex;
use protostar::xdg::{DesktopFile, get_desktop_files};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use single::{APP_SIZE, App, BTN_COLOR, BTN_SELECTED_COLOR, MODEL_SCALE};
use stardust_xr_asteroids::{
	ClientState, Context, CustomElement, Element, Entity, Migrate, Reify, Tasker, Transformable,
	client,
	components::{Containable, Derezzable, Grabbable, PointerMode, Poseable},
	elements::{Button, Model, ModelPart, Spatial},
};
use stardust_xr_fusion::{
	drawable::MaterialParameter,
	fields::Shape,
	project_local_resources,
	spatial::Transform,
	types::{
		Posef,
		color::{Deg, Hsv, ToHsv, ToRgba, color_space::Srgb},
	},
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

	client::run::<HexagonLauncher>(&[&project_local_resources!("../res")])
		.await
		.unwrap()
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct HexagonLauncher {
	/// if the hexagon launcher is expanded
	open: bool,
	pose: Posef,
	#[serde(skip)]
	apps: Vec<App>,
	/// where each app sits, same order as `apps`
	#[serde(skip)]
	hexes: Vec<Hex>,
}

impl Migrate for HexagonLauncher {
	type Old = Self;
}

impl ClientState for HexagonLauncher {
	const APP_ID: &'static str = "org.protostar.hexagon_launcher";

	fn initial_state_update(&mut self) {
		let current_desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
		// Load desktop files
		self.apps = get_desktop_files()
			.filter_map(|d| DesktopFile::parse(d).ok())
			.filter(|d| !d.no_display)
			.filter(|d| d.only_show_in.is_empty() || d.only_show_in.contains(&current_desktop))
			.filter_map(|d| App::new(d).ok())
			.collect();

		self.apps.par_iter().for_each(|app| {
			app.load_icon();
		});

		// five triangles out from the center split the colorful icons into runs of similar hues,
		// the sixth is the black one for everything grey, alphabetical going outwards in each
		let hue = |app: &App| app.color().map_or(f32::INFINITY, |c| c.to_hsv::<f32>().h.0);
		self.apps.sort_by(|a, b| hue(a).total_cmp(&hue(b)));
		let colorful = self.apps.iter().filter(|a| a.color().is_some()).count();
		let per_wedge = colorful.div_ceil(5).max(1);
		let wedges: Vec<usize> = (0..self.apps.len())
			.map(|i| if i < colorful { i / per_wedge } else { 5 })
			.collect();
		let mut start = 0;
		for side in 0..6 {
			let len = wedges[start..].iter().take_while(|&&w| w == side).count();
			let section = &mut self.apps[start..start + len];
			section.sort_by_key(|app| app.app.name().unwrap_or_default().to_lowercase());
			// one shared color per section, the middle of the hue range it covers, so the
			// triangles read as clean areas instead of muddy averages
			let hues: Vec<f32> = section
				.iter()
				.filter_map(App::color)
				.map(|c| c.to_hsv::<f32>().h.0)
				.collect();
			let lo = hues.iter().copied().reduce(f32::min);
			let hi = hues.iter().copied().reduce(f32::max);
			// muted rather than fully saturated, so the icons stay readable on top
			let tint = lo.zip(hi).map(|(lo, hi)| {
				Hsv::<f32, Srgb>::new(Deg((lo + hi) / 2.0), 0.65, 0.8)
					.to_rgba::<f32>()
					.to_linear()
			});
			for app in section {
				app.set_tint(tint);
			}
			self.hexes.extend((0..len).map(|n| Hex::wedge(side, n)));
			start += len;
		}
	}
}
impl Reify for HexagonLauncher {
	#[tracing::instrument(skip_all)]
	fn reify(&self, context: &Context, tasks: impl Tasker<Self>) -> impl Element<Self> {
		let field_shape = Shape::Transform {
			shape: Box::new(Shape::Cylinder {
				radius: APP_SIZE / 2.0,
				length: 0.005,
			}),
			transform: Mat4::from_rotation_x(FRAC_PI_2).into(),
		};
		// Build UI based on current state
		Entity::new(field_shape)
			.pos(self.pose.position)
			.rot(self.pose.orientation)
			.component(
				Grabbable::new(|state: &mut Self, pose| {
					state.pose = pose;
				})
				.pointer_mode(PointerMode::Align),
			)
			.component(Containable::default())
			.component(Poseable::new(|state: &mut Self, pose| {
				state.pose = pose;
			}))
			.component(Derezzable::new({
				let context = context.clone();
				move |_| context.stop()
			}))
			// .component(Tappable::new(Vec3::Z, |state: &mut Self| {
			// 	state.open = !state.open
			// }))
			.build()
			.child(
				Button::new(|state: &mut HexagonLauncher| {
					state.open = !state.open;
				})
				.pos([0.0, 0.0, 0.006])
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
						MaterialParameter::Color {
							value: if self.open {
								BTN_SELECTED_COLOR
							} else {
								BTN_COLOR
							},
						},
					))
					.build(),
			)
			.children(
				self.open
					.then(|| {
						self.apps.iter().enumerate().map(|(i, app)| {
							Spatial::default()
								.pos(self.hexes[i].get_coords())
								.build()
								.child(app.reify_substate(
									context,
									tasks.clone(),
									move |state: &mut HexagonLauncher| state.apps.get_mut(i),
								))
						})
					})
					.into_iter()
					.flatten(),
			)
	}
}
