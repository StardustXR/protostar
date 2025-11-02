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
use stardust_xr_fusion::values::ResourceID;
use std::path::PathBuf;
use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::atomic::AtomicU64;
use tokio::time::Duration;

static REIFY_COUNT: AtomicUsize = AtomicUsize::new(0);
static REIFY_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static APP_REIFY_COUNT: AtomicUsize = AtomicUsize::new(0);
static VISIBLE_LIMIT: AtomicUsize = AtomicUsize::new(0);
const VISIBLE_STEP: usize = 12;

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
    #[serde(skip)]
    /// cached world coordinates for each app hex
    positions: Vec<[f32; 3]>,
	#[serde(skip)]
	/// lightweight immutable snapshots for fast per-frame reify
	snapshots: Vec<Snapshot>,
}

#[derive(Debug, Clone)]
struct Snapshot {
	name: String,
	cached_texture: Option<ResourceID>,
	cached_gltf: Option<PathBuf>,
}

impl Default for HexagonLauncher {
	fn default() -> Self {
		Self {
			open: false,
			pos: [0.0; 3].into(),
			rot: Quat::IDENTITY.into(),
			apps: Vec::new(),
			positions: Vec::new(),
			snapshots: Vec::new(),
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

		// Sort by name
		self.apps
			.sort_by_key(|app| app.app.name().unwrap_or_default().to_string());

		// precompute coordinates for each app to avoid recomputing per-reify
		self.positions = (0..self.apps.len())
			.map(|i| Hex::spiral(i + 1).get_coords())
			.collect();

		// Preload icons/resources off the reify path so create_model() is cheap later.
		// Use rayon to parallelize filesystem/processing work.
		self.apps
			.par_iter()
			.for_each(|app| {
				// idempotent: App::load_icon uses OnceLock internally
				app.load_icon();
			});

		// Warm / prebuild heavy Model resources in parallel so the renderer
		// doesn't pay parsing/creation cost during reify.
		//
		// We do this after load_icon above so cached_gltf / cached_texture are populated.
		self.apps.par_iter().for_each(|app| {
			// GLTF path warm: call Model::direct once (parser/cache warm-up)
			if let Some(gltf_path) = app.cached_gltf.get() {
				if let Ok(builder) = Model::direct(gltf_path.to_string_lossy().to_string()) {
					let _ = CustomElement::<HexagonLauncher>::build(builder);
                }
             } else if let Some(tex) = app.cached_texture.get() {
                 // Raster icon path warm: build the lightweight namespaced model
                 // with the cached texture so material/texture creation happens now.
                let builder = Model::namespaced("protostar", "hexagon/hexagon")
                    .part(ModelPart::new("Hex").mat_param(
                        "color",
                        MaterialParameter::Color(crate::BTN_COLOR),
                    ))
                    .part(ModelPart::new("Icon").mat_param(
                        "diffuse",
                        MaterialParameter::Texture(tex.clone()),
                    ));
                let _ = CustomElement::<HexagonLauncher>::build(builder);
             }
         });

		// build immutable lightweight snapshots used during reify
		self.snapshots = self
			.apps
			.iter()
			.map(|a| Snapshot {
-					name: a.app.name().unwrap_or_default(),
+					name: a.app.name().unwrap_or_default().to_string(),
                     cached_texture: a.cached_texture.get().cloned(),
                     cached_gltf: a.cached_gltf.get().cloned(),
                 })
                 .collect();
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
         // limit how many children we build per-frame to avoid reify explosion;
         // increase if performance is acceptable, or implement a pager/virtualization.
         .children({
             // read configured maximum (fall back to all apps)
             let env_max = std::env::var("HEX_MAX_VISIBLE")
                 .ok()
                 .and_then(|s| s.parse::<usize>().ok());
             let configured_max = env_max.unwrap_or(self.apps.len());
             // desired target: if open -> min(configured_max, apps.len()) else 0
             let desired = if self.open {
                 std::cmp::min(configured_max, self.apps.len())
             } else {
                 0
             };
             // nudge the global visible limit toward desired to spread creation cost
             let current = VISIBLE_LIMIT.load(Ordering::Relaxed);
             if desired == 0 {
                 // closing -> quickly collapse
                 if current != 0 {
                     VISIBLE_LIMIT.store(0, Ordering::Relaxed);
                 }
             } else if current < desired {
                 let add = (desired - current).min(VISIBLE_STEP);
                 VISIBLE_LIMIT.fetch_add(add, Ordering::Relaxed);
             } else if current > desired {
                 // clamp down if configured max reduced
                 VISIBLE_LIMIT.store(desired, Ordering::Relaxed);
             }

             let take_n = std::cmp::min(VISIBLE_LIMIT.load(Ordering::Relaxed), self.apps.len());
             tracing::debug!(total_apps = self.apps.len(), configured_max, visible = take_n, desired, "building visible app children");

             self.open
                 .then(|| {
                     self.apps
                         .iter()
                         .enumerate()
                         .take(take_n)
                         .map(|(i, _app)| {
                             // use snapshot instead of reify_substate (cheap, immutable)
                             let snap = self.snapshots[i].clone();
                             let pos = self.positions[i];
                             // build spatial + cheap model from snapshot (no per-app state access)
                             let mut spatial = Spatial::default().pos(pos).build();
+
+                            // attach model from snapshot (gltf preferred, else namespaced + texture)
+                            if let Some(gltf) = snap.cached_gltf {
+                                if let Ok(builder) = Model::direct(gltf.to_string_lossy().to_string()) {
+                                    spatial = spatial.child(builder.transform(Transform::from_rotation_scale(
+                                        Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
+                                        [MODEL_SCALE; 3],
+                                    )).build());
+                                }
+                            } else {
+                                let mut mb = Model::namespaced("protostar", "hexagon/hexagon")
+                                    .transform(Transform::from_rotation_scale(
+                                        Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
+                                        [MODEL_SCALE; 3],
+                                    ))
+                                    .part(ModelPart::new("Hex").mat_param(
+                                        "color",
+                                        MaterialParameter::Color(if self.open {
+                                            BTN_SELECTED_COLOR
+                                        } else {
+                                            BTN_COLOR
+                                        }),
+                                    ));
+                                if let Some(tex) = snap.cached_texture {
+                                    mb = mb.part(ModelPart::new("Icon").mat_param(
+                                        "diffuse",
+                                        MaterialParameter::Texture(tex),
+                                    ));
+                                }
+                                spatial = spatial.child(mb.build());
+                            }
+
+                            // attach a Button that mutates real state when used (captures index)
+                            spatial.child(
+                                Button::new(move |state: &mut HexagonLauncher| {
+                                    // example: toggle open / or launch the app via state.apps[i]
+                                    // keep mutation here, but we avoid doing this per-frame.
+                                    // if you need to launch: state.apps[i].launch(...);
+                                    tracing::debug!(index = i, "app button pressed");
+                                })
+                                .pos([0.0, 0.0, 0.0])
+                                .size([0.01; 2])
+                                .build(),
+                            )
                         })
                 })
                 .into_iter()
                 .flatten()
         })
		;

         let elapsed = start.elapsed().as_nanos() as u64;
         REIFY_TOTAL_NS.fetch_add(elapsed, Ordering::Relaxed);
         REIFY_COUNT.fetch_add(1, Ordering::Relaxed);
		elem
	}
}
