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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::atomic::AtomicU64;
use tokio::time::Duration;

static REIFY_COUNT: AtomicUsize = AtomicUsize::new(0);
static REIFY_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static APP_REIFY_COUNT: AtomicUsize = AtomicUsize::new(0);

use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main(flavor = "current_thread")]
async fn main() {
	color_eyre::install().unwrap();

	// spawn a background logger that prints reify calls per second
	tokio::spawn(async {
		loop {
			tokio::time::sleep(Duration::from_secs(1)).await;
			let v = REIFY_COUNT.swap(0, Ordering::Relaxed);
			let total_ns = REIFY_TOTAL_NS.swap(0, Ordering::Relaxed);
			let app_hits = APP_REIFY_COUNT.swap(0, Ordering::Relaxed);
			let avg_ns = if v > 0 { total_ns / (v as u64) } else { 0 };
			tracing::info!(
				reify_per_sec = v,
				avg_reify_ms = (avg_ns as f64) / 1_000_000.0,
				app_reify_hits = app_hits,
				"hexagon reify stats"
			);
		}
	});

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
		// measure reify latency and count
		let start = std::time::Instant::now();

		// Build UI based on current state
		let elem = Grabbable::new(
             Shape::Cylinder(CylinderShape {
                 radius: APP_SIZE / 2.0,
                 length: 0.01,
             }),
             self.pos,
             self.rot,
             |state: &mut Self, pos, rot| {
                 // only update if changed enough to avoid constant reify
                 let dx = (state.pos.x - pos.x).abs();
                 let dy = (state.pos.y - pos.y).abs();
                 let dz = (state.pos.z - pos.z).abs();
                 if dx > 0.0005 || dy > 0.0005 || dz > 0.0005 {
                     tracing::trace!(?pos, "updating grab position");
                     state.pos = pos;
                 }
                 // rotation updates can also be debounced if noisy
                 state.rot = rot;
             },
         )
         .field_transform(Transform::from_rotation(Quat::from_rotation_x(FRAC_PI_2)))
         .pointer_mode(PointerMode::Align)
         .reparentable(true)
         .build()
         .child(
             Button::new(|state: &mut HexagonLauncher| {
                 state.open = !state.open;
                 tracing::debug!(open = state.open, "toggled hexagon open");
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
                                 // log & count access to per-app substate
                                 APP_REIFY_COUNT.fetch_add(1, Ordering::Relaxed);
                                 tracing::trace!(index = i, "accessing app substate");
                                 state.apps.get_mut(i)
                             }))
                     })
                 })
                 .into_iter()
                 .flatten(),
         )
		;

         let elapsed = start.elapsed().as_nanos() as u64;
         REIFY_TOTAL_NS.fetch_add(elapsed, Ordering::Relaxed);
         REIFY_COUNT.fetch_add(1, Ordering::Relaxed);
		elem
	}
}
