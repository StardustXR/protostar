use clap::{self, Parser};
use color_eyre::eyre::Result;
use glam::{Quat, Vec3};
use manifest_dir_macros::directory_relative_path;
use mint::Vector3;
use protostar::{
	application::Application,
	xdg::{parse_desktop_file, DesktopFile, Icon, IconType},
};
use stardust_xr_fusion::{
	client::{Client, FrameInfo, RootHandler},
	core::values::Transform,
	drawable::{Alignment, Bounds, MaterialParameter, Model, ResourceID, Text, TextFit, TextStyle},
	fields::BoxField,
	input::{InputData, InputDataType},
	node::NodeError,
	node::NodeType,
	spatial::Spatial,
};
use stardust_xr_molecules::{touch_plane::TouchPlane, GrabData, Grabbable};
use std::{f32::consts::PI, path::PathBuf};

use tween::{QuartInOut, Tweener};
use walkdir::WalkDir;

const APP_SIZE: f32 = 0.06;
const ACTIVATION_DISTANCE: f32 = 0.5;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
	/// Directory to scan for desktop files
	apps_directory: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	color_eyre::install()?;

	let args = Args::parse();
	if !args.apps_directory.is_dir() {
		panic!(
			"{} is not a direcotry",
			args.apps_directory.to_string_lossy()
		)
	}

	let (client, event_loop) = Client::connect_with_async_loop().await?;
	client.set_base_prefixes(&[directory_relative_path!("res")]);

	let _wrapped_root = client.wrap_root(Sirius::new(&client, args)?)?;

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e??,
	}
	Ok(())
}

struct Sirius {
	touch_plane: TouchPlane,
	model: Model,
	root: Spatial,
	clients: Vec<App>,
	visibility: bool,
	grabbable: Grabbable,
}
impl Sirius {
	fn new(client: &Client, args: Args) -> Result<Self, NodeError> {
		let mut client_list: Vec<(Option<&str>, String)> = Vec::new();

		let root = Spatial::create(client.get_root(), Transform::default(), false).unwrap();

		let field = BoxField::create(&root, Transform::default(), [0.1; 3]).unwrap();
		let grabbable =
			Grabbable::create(&root, Transform::default(), &field, GrabData::default())?;
		let touch_plane = TouchPlane::create(
			grabbable.content_parent(),
			Transform::default(),
			[0.1; 2],
			0.03,
			1.0..0.0,
			1.0..0.0,
		)?;

		let walkdir = WalkDir::new(args.apps_directory.canonicalize().unwrap());

		let mut clients: Vec<App> = walkdir
			.into_iter()
			.filter_map(|path| path.ok())
			.map(|entry| entry.into_path())
			.filter(|path| {
				path.is_file()
					&& path.extension().is_some()
					&& path.extension().unwrap() == "desktop"
			})
			.filter_map(|path| {
				App::create_from_desktop_file(
					grabbable.content_parent(),
					[0.0; 3],
					parse_desktop_file(path).ok()?,
				)
				.ok()
			})
			.collect();

		let model = Model::create(
			grabbable.content_parent(),
			Transform::default(),
			&ResourceID::new_namespaced("protostar", "button"),
		)?;
		field.set_spatial_parent(grabbable.content_parent())?;
		let visibility = false;

		Ok(Sirius {
			touch_plane,
			model,
			root,
			clients,
			visibility,
			grabbable,
		})
	}

	//    fn left_hand(input_data: &InputData, _: &()) -> bool {
	//       match &input_data.input {
	//            InputDataType::Hand(h) => !h.right,
	//            _ => false,
	//        }
	//    }
}
impl RootHandler for Sirius {
	fn frame(&mut self, info: FrameInfo) {
		for app in &mut self.clients {
			app.frame(info);
		}

		self.grabbable.update(&info).unwrap();
		self.touch_plane.update();
		if self.touch_plane.touch_started() {
			println!("Touch started");
			self.visibility = !self.visibility;
			match self.visibility {
				true => {
					for (pos, star) in self.clients.iter().enumerate() {
						let mut starpos = (pos as f32 + 1.0) / 10.0;
						match starpos % 0.2 == 0.0 {
							true => starpos = -starpos / 2.0,
							false => starpos = (starpos - 0.1) / 2.0,
						}
						println!("{}", starpos);
						star.content_parent()
							.set_position(
								Some(&self.grabbable.content_parent()),
								[starpos, 0.1, 0.0],
							)
							.ok();
					}
				}
				false => {
					for star in &self.clients {
						star.content_parent()
							.set_position(Some(&self.grabbable.content_parent()), [0.0, 0.0, 0.0])
							.ok();
					}
				}
			}
			let color = [0.0, 1.0, 0.0, 1.0];
			self.model
				.model_part("?????")
				.unwrap()
				.set_material_parameter("color", MaterialParameter::Color(color))
				.unwrap();
			self.model
				.model_part("?????")
				.unwrap()
				.set_material_parameter(
					"emission_factor",
					MaterialParameter::Color(color.map(|c| c * 0.75)),
				)
				.unwrap();
		}

		if self.touch_plane.touch_stopped() {
			println!("Touch ended");
			let color = [1.0, 0.0, 0.0, 1.0];
			self.model
				.model_part("?????")
				.unwrap()
				.set_material_parameter("color", MaterialParameter::Color(color))
				.unwrap();
			self.model
				.model_part("?????")
				.unwrap()
				.set_material_parameter(
					"emission_factor",
					MaterialParameter::Color(color.map(|c| c * 0.5)),
				)
				.unwrap();
		}
	}
}

fn position(data: &InputData) -> Vec3 {
	match &data.input {
		InputDataType::Hand(h) => h.palm.position.into(),
		InputDataType::Pointer(w) => w.deepest_point.into(),
		InputDataType::Tip(t) => t.origin.into(),
	}
}

fn model_from_icon(parent: &Spatial, icon: &Icon) -> Result<Model> {
	return match &icon.icon_type {
		IconType::Png => {
			let t = Transform::from_rotation_scale(
				Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
				[APP_SIZE * 0.5; 3],
			);

			let model = Model::create(
				parent,
				t,
				&ResourceID::new_namespaced("protostar", "hexagon/hexagon"),
			)?;
			model
				.model_part("Hex")?
				.set_material_parameter("color", MaterialParameter::Color([0.0, 1.0, 1.0, 1.0]))?;
			model.model_part("Icon")?.set_material_parameter(
				"diffuse",
				MaterialParameter::Texture(ResourceID::Direct(icon.path.clone())),
			)?;
			Ok(model)
		}
		IconType::Gltf => Ok(Model::create(
			parent,
			Transform::from_scale([0.05; 3]),
			&ResourceID::new_direct(icon.path.clone())?,
		)?),
		_ => panic!("Invalid Icon Type"),
	};
}

pub struct App {
	application: Application,
	parent: Spatial,
	position: Vector3<f32>,
	grabbable: Grabbable,
	_field: BoxField,
	icon: Model,
	label: Option<Text>,
	grabbable_shrink: Option<Tweener<f32, f64, QuartInOut>>,
	grabbable_grow: Option<Tweener<f32, f64, QuartInOut>>,
	grabbable_move: Option<Tweener<f32, f64, QuartInOut>>,
	currently_shown: bool,
}
impl App {
	pub fn create_from_desktop_file(
		parent: &Spatial,
		position: impl Into<Vector3<f32>>,
		desktop_file: DesktopFile,
	) -> Result<Self> {
		let position = position.into();
		let field = BoxField::create(parent, Transform::default(), [APP_SIZE; 3])?;
		let application = Application::create(&parent.client()?, desktop_file)?;
		let icon = application.icon(128, false);
		let grabbable = Grabbable::create(
			parent,
			Transform::from_position(position),
			&field,
			GrabData {
				max_distance: 0.01,
				frame_cancel_threshold: 50,
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
					Transform::from_rotation_scale(
						Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
						[APP_SIZE * 0.5; 3],
					),
					&ResourceID::new_namespaced("protostar", "hexagon/hexagon"),
				)?)
			})?;

		let label_style = TextStyle {
			character_height: APP_SIZE * 2.0,
			bounds: Some(Bounds {
				bounds: [1.0; 2].into(),
				fit: TextFit::Wrap,
				bounds_align: Alignment::XCenter | Alignment::YCenter,
			}),
			text_align: Alignment::Center.into(),
			..Default::default()
		};
		let label = application.name().and_then(|name| {
			Text::create(
				&icon,
				Transform::from_position_rotation(
					[0.0, 0.1, -(APP_SIZE * 4.0)],
					Quat::from_rotation_x(PI * 0.5),
				),
				name,
				label_style,
			)
			.ok()
		});
		Ok(App {
			parent: parent.alias(),
			position,
			grabbable,
			_field: field,
			label,
			application,
			icon,
			grabbable_shrink: None,
			grabbable_grow: None,
			grabbable_move: None,
			currently_shown: true,
		})
	}
	pub fn content_parent(&self) -> &Spatial {
		self.grabbable.content_parent()
	}
	pub fn toggle(&mut self) {
		self.grabbable.set_enabled(!self.currently_shown).unwrap();
		if self.currently_shown {
			self.grabbable_move = Some(Tweener::quart_in_out(1.0, 0.0001, 0.25)); //TODO make the scale a parameter
		} else {
			self.icon.set_enabled(true).unwrap();
			self.label.as_ref().map(|l| l.set_enabled(true).unwrap());
			self.grabbable_move = Some(Tweener::quart_in_out(0.0001, 1.0, 0.25));
		}
		self.currently_shown = !self.currently_shown;
	}
}
impl RootHandler for App {
	fn frame(&mut self, info: FrameInfo) {
		let _ = self.grabbable.update(&info);

		if let Some(grabbable_move) = &mut self.grabbable_move {
			if !grabbable_move.is_finished() {
				let scale = grabbable_move.move_by(info.delta);
				self.grabbable
					.content_parent()
					.set_position(
						Some(&self.parent),
						[
							self.position.x * scale,
							self.position.y * scale,
							self.position.z * scale,
						],
					)
					.unwrap();
			} else {
				if grabbable_move.final_value() == 0.0001 {
					self.icon.set_enabled(false).unwrap();
					self.label.as_ref().map(|l| l.set_enabled(false).unwrap());
				}
				self.grabbable_move = None;
			}
		}
		if let Some(grabbable_shrink) = &mut self.grabbable_shrink {
			if !grabbable_shrink.is_finished() {
				let scale = grabbable_shrink.move_by(info.delta);
				self.grabbable
					.content_parent()
					.set_scale(Some(&self.parent), Vector3::from([scale; 3]))
					.unwrap();
			} else {
				self.grabbable
					.content_parent()
					.set_spatial_parent(&self.parent)
					.unwrap();
				if self.currently_shown {
					self.grabbable_grow = Some(Tweener::quart_in_out(0.0001, 1.0, 0.25));
					self.grabbable.cancel_angular_velocity();
					self.grabbable.cancel_linear_velocity();
				}
				self.grabbable_shrink = None;
				self.grabbable
					.content_parent()
					.set_position(Some(&self.parent), self.position)
					.unwrap();
				self.grabbable
					.content_parent()
					.set_rotation(Some(&self.parent), Quat::default())
					.unwrap();
				self.icon
					.set_rotation(
						None,
						Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
					)
					.unwrap();
			}
		} else if let Some(grabbable_grow) = &mut self.grabbable_grow {
			if !grabbable_grow.is_finished() {
				let scale = grabbable_grow.move_by(info.delta);
				self.grabbable
					.content_parent()
					.set_scale(Some(&self.parent), Vector3::from([scale; 3]))
					.unwrap();
			} else {
				self.grabbable
					.content_parent()
					.set_spatial_parent(&self.parent)
					.unwrap();
				self.grabbable_grow = None;
			}
		} else if self.grabbable.valid() && self.grabbable.grab_action().actor_stopped() {
			self.grabbable_shrink = Some(Tweener::quart_in_out(APP_SIZE * 0.5, 0.0001, 0.25));
			let Ok(distance_future) = self.grabbable
				.content_parent()
				.get_position_rotation_scale(&self.parent)
				 else {return};

			let application = self.application.clone();
			let space = self.content_parent().alias();

			//TODO: split the executable string for the args
			tokio::task::spawn(async move {
				let distance_vector = distance_future.await.ok().unwrap().0;
				let distance = ((distance_vector.x.powi(2) + distance_vector.y.powi(2)).sqrt()
					+ distance_vector.z.powi(2))
				.sqrt();
				if dbg!(distance) > ACTIVATION_DISTANCE {
					let _ = application.launch(&space);
				}
			});
		}
	}
}
