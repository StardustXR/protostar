use color_eyre::eyre::Result;
use glam::Quat;
use manifest_dir_macros::directory_relative_path;
use mint::Vector3;
use protostar::{
	protostar::ProtoStar,
	xdg::{get_desktop_files, parse_desktop_file, DesktopFile},
};
use stardust_xr_fusion::{
	client::{Client, FrameInfo, RootHandler},
	core::values::Transform,
	drawable::{MaterialParameter, Model, ResourceID},
	fields::BoxField,
	node::NodeError,
	spatial::Spatial,
};
use stardust_xr_molecules::{touch_plane::TouchPlane, GrabData, Grabbable};
use std::f32::consts::PI;
use tween::TweenTime;

const APP_SIZE: f32 = 0.06;
const PADDING: f32 = 0.005;

#[derive(Clone)]
struct Hex {
	q: isize,
	r: isize,
	s: isize,
}

const HEX_CENTER: Hex = Hex { q: 0, r: 0, s: 0 };
const HEX_DIRECTION_VECTORS: [Hex; 6] = [
	Hex { q: 1, r: 0, s: -1 },
	Hex { q: 1, r: -1, s: 0 },
	Hex { q: 0, r: -1, s: 1 },
	Hex { q: -1, r: 0, s: 1 },
	Hex { q: -1, r: 1, s: 0 },
	Hex { q: 0, r: 1, s: -1 },
];

impl Hex {
	fn new(q: isize, r: isize, s: isize) -> Self {
		Hex { q, r, s }
	}

	fn get_coords(&self) -> [f32; 3] {
		let x = 3.0 / 2.0 * (APP_SIZE + PADDING) / 2.0 * (-self.q - self.s).to_f32();
		let y = 3.0_f32.sqrt() * (APP_SIZE + PADDING) / 2.0
			* ((-self.q - self.s).to_f32() / 2.0 + self.s.to_f32());
		[x, y, 0.0]
	}

	fn add(self, vec: &Hex) -> Self {
		Hex::new(self.q + vec.q, self.r + vec.r, self.s + vec.s)
	}

	fn neighbor(self, direction: usize) -> Self {
		self.add(&HEX_DIRECTION_VECTORS[direction])
	}

	fn scale(self, factor: isize) -> Self {
		Hex::new(self.q * factor, self.r * factor, self.s * factor)
	}
}

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
			.into_iter()
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
						App::new(
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
			let color = [0.0, 1.0, 0.0, 1.0];
			self.button
				.model
				.set_material_parameter(1, "color", MaterialParameter::Color(color))
				.unwrap();
			for app in &mut self.apps {
				app.protostar.toggle();
			}
		} else if self.button.touch_plane.touch_stopped() {
			let color = [0.0, 0.0, 1.0, 1.0];
			self.button
				.model
				.set_material_parameter(1, "color", MaterialParameter::Color(color))
				.unwrap();
		}
		for app in &mut self.apps {
			app.frame(info);
		}
	}
}
struct App {
	_desktop_file: DesktopFile,
	protostar: ProtoStar,
}

impl App {
	fn new(
		parent: &Spatial,
		position: impl Into<Vector3<f32>>,
		desktop_file: DesktopFile,
	) -> Option<Self> {
		let position = position.into();
		let protostar =
			ProtoStar::create_from_desktop_file(parent, position, desktop_file.clone()).ok()?;
		Some(App {
			_desktop_file: desktop_file,
			protostar,
		})
	}
}
impl RootHandler for App {
	fn frame(&mut self, info: FrameInfo) {
		self.protostar.frame(info);
	}
}

struct Button {
	touch_plane: TouchPlane,
	grabbable: Grabbable,
	model: Model,
}
impl Button {
	fn new(client: &Client) -> Result<Self, NodeError> {
		let field = BoxField::create(client.get_root(), Transform::default(), [APP_SIZE; 3])?;
		let grabbable = Grabbable::new(
			client.get_root(),
			Transform::default(),
			&field,
			GrabData {
				max_distance: 0.01,
				..Default::default()
			},
		)?;
		field.set_spatial_parent(grabbable.content_parent())?;
		let touch_plane = TouchPlane::new(
			grabbable.content_parent(),
			Transform::default(),
			[(APP_SIZE + PADDING) / 2.0; 2],
			(APP_SIZE + PADDING) / 2.0,
		)?;

		let model = Model::create(
			grabbable.content_parent(),
			Transform::from_rotation_scale(
				Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
				[0.03, 0.03, 0.03],
			),
			&ResourceID::new_namespaced("protostar", "hexagon/hexagon"),
		)?;
		model.set_material_parameter(1, "color", MaterialParameter::Color([0.0, 0.0, 1.0, 1.0]))?;
		Ok(Button {
			touch_plane,
			grabbable,
			model,
		})
	}
}
impl RootHandler for Button {
	fn frame(&mut self, info: FrameInfo) {
		self.touch_plane.update();
		self.grabbable.update(&info);
	}
}
