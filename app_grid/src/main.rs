use color_eyre::eyre::Result;
use glam::{Quat, Vec3};
use manifest_dir_macros::directory_relative_path;
use protostar::{
	application::Application,
	xdg::{get_desktop_files, parse_desktop_file, DesktopFile, Icon, IconType},
};
use stardust_xr_fusion::{
	client::Client,
	core::values::{color::rgba_linear, ResourceID, Vector3},
	drawable::{
		MaterialParameter, Model, ModelPartAspect, Text, TextBounds, TextFit, TextStyle, XAlign,
		YAlign,
	},
	fields::{Field, Shape},
	root::{ClientState, FrameInfo, RootAspect},
	spatial::{Spatial, SpatialAspect, SpatialRefAspect, Transform},
	ClientHandle,
};
use stardust_xr_molecules::{FrameSensitive, Grabbable, GrabbableSettings, UIElement};
use std::f32::consts::PI;

const APP_LIMIT: usize = 300;
const APP_SIZE: f32 = 0.05;
const GRID_PADDING: f32 = 0.01;
const ACTIVATION_DISTANCE: f32 = 0.5;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	color_eyre::install().unwrap();
	tracing_subscriber::fmt()
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.pretty()
		.init();
	let owned_client = Client::connect().await?;
	let client = owned_client.handle();
	let async_loop = owned_client.async_event_loop();
	client
		.get_root()
		.set_base_prefixes(&[directory_relative_path!("../res").to_string()])
		.unwrap();

	let mut grid = AppGrid::new(&client);
	let mut owned_client = async_loop.stop().await.unwrap();
	let event_loop = owned_client.sync_event_loop(|handle, _| {
		let Some(event) = handle.get_root().recv_root_event() else {
			return;
		};
		match event {
			stardust_xr_fusion::root::RootEvent::Ping { response } => response.send(Ok(())),
			stardust_xr_fusion::root::RootEvent::Frame { info } => {
				grid.frame(info);
			}
			stardust_xr_fusion::root::RootEvent::SaveState { response } => {
				response.send(grid.save_state());
			}
		}
	});

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e?,
	};
	Ok(())
}

struct AppGrid {
	apps: Vec<App>,
	//style: TextStyle,
}
impl AppGrid {
	fn new(client: &ClientHandle) -> Self {
		let apps = get_desktop_files()
			.filter_map(|d| parse_desktop_file(d).ok())
			.filter(|d| !d.no_display)
			.enumerate()
			.filter(|(i, _)| *i <= APP_LIMIT)
			.filter_map(|(i, a)| {
				App::create_from_desktop_file(
					client.get_root(),
					[
						(i % 10) as f32 * (APP_SIZE + GRID_PADDING),
						(i / 10) as f32 * (APP_SIZE + GRID_PADDING),
						0.0,
					],
					a,
					//style,
				)
				.ok()
			})
			.collect::<Vec<_>>();
		AppGrid { apps }
	}
}
impl AppGrid {
	fn frame(&mut self, info: FrameInfo) {
		for app in &mut self.apps {
			app.frame(&info);
		}
	}
	fn save_state(&mut self) -> Result<ClientState> {
		Ok(ClientState::default())
	}
}

fn model_from_icon(parent: &Spatial, icon: &Icon) -> Result<Model> {
	match &icon.icon_type {
		IconType::Png => {
			// let t = Transform::from_rotation_scale(
			// 	Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
			// 	[1.0; 3],
			// );

			let model = Model::create(
				parent,
				Transform::from_rotation(Quat::from_rotation_y(PI)),
				&ResourceID::new_namespaced("protostar", "cartridge"),
			)?;
			model.part("Cartridge")?.set_material_parameter(
				"color",
				MaterialParameter::Color(rgba_linear!(0.0, 1.0, 1.0, 1.0)),
			)?;
			model.part("Icon")?.set_material_parameter(
				"diffuse",
				MaterialParameter::Texture(ResourceID::Direct(icon.path.clone())),
			)?;
			Ok(model)
		}
		IconType::Gltf => Ok(Model::create(
			parent,
			Transform::none(),
			&ResourceID::new_direct(icon.path.clone())?,
		)?),
		_ => panic!("Invalid Icon Type"),
	}
}

pub struct App {
	root: Spatial,
	application: Application,
	grabbable: Grabbable,
	_field: Field,
	_icon: Model,
	_label: Option<Text>,
}
impl App {
	pub fn create_from_desktop_file(
		parent: &impl SpatialRefAspect,
		position: impl Into<Vector3<f32>>,
		desktop_file: DesktopFile,
	) -> Result<Self> {
		let root = Spatial::create(parent, Transform::from_translation(position), false)?;
		let field = Field::create(&root, Transform::none(), Shape::Box([APP_SIZE; 3].into()))?;
		let application = Application::create(desktop_file)?;
		let icon = application.icon(128, true);
		let grabbable = Grabbable::create(
			&root,
			Transform::identity(),
			&field,
			GrabbableSettings {
				max_distance: 0.01,
				..Default::default()
			},
		)?;
		grabbable.content_parent().set_spatial_parent(parent)?;
		field.set_spatial_parent(grabbable.content_parent())?;
		let icon = icon
			.map(|i| model_from_icon(grabbable.content_parent(), &i))
			.unwrap_or_else(|| {
				Ok(Model::create(
					grabbable.content_parent(),
					Transform::from_rotation(Quat::from_rotation_y(PI)),
					&ResourceID::new_namespaced("protostar", "cartridge"),
				)?)
			})?;

		let label_style = TextStyle {
			character_height: 0.005,
			bounds: Some(TextBounds {
				bounds: [0.047013, 0.01].into(),
				fit: TextFit::Wrap,
				anchor_align_x: XAlign::Center,
				anchor_align_y: YAlign::Center,
			}),
			text_align_x: XAlign::Center,
			text_align_y: YAlign::Center,
			..Default::default()
		};
		let label = application.name().and_then(|name| {
			Text::create(
				&icon.part("Label").ok()?,
				Transform::none(),
				name,
				label_style,
			)
			.ok()
		});
		Ok(App {
			root,
			grabbable,
			_field: field,
			_label: label,
			application,
			_icon: icon,
		})
	}
	pub fn content_parent(&self) -> &Spatial {
		self.grabbable.content_parent()
	}

	// fn bring_back(&self) {
	// 	self.grabbable
	// 		.content_parent()
	// 		.set_transform(Some(&self.root), Transform::identity())
	// 		.unwrap();
	// }

	fn frame(&mut self, info: &FrameInfo) {
		if !self.grabbable.handle_events() {
			return;
		}
		self.grabbable.frame(info);

		if self.grabbable.grab_action().actor_stopped() {
			self.grabbable.cancel_angular_velocity();
			self.grabbable.cancel_linear_velocity();

			// if !self.grabbable.valid() {
			// 	self.bring_back();
			// 	return;
			// }

			let application = self.application.clone();
			let space = self.content_parent().clone();
			let root = self.root.clone();

			tokio::task::spawn(async move {
				let Ok(transform) = space.get_transform(&root).await else {
					space
						.set_relative_transform(&root, Transform::identity())
						.unwrap();
					return;
				};
				let distance = Vec3::from(transform.translation.unwrap()).length_squared();

				if distance > ACTIVATION_DISTANCE.powi(2) {
					let _ = application.launch(&space);
				}

				space
					.set_relative_transform(&root, Transform::identity())
					.unwrap();
			});
		}
	}
}
