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
	drawable::{Alignment, Bounds, Text, TextFit, TextStyle},
	spatial::Spatial,
};
use tween::TweenTime;

const APP_SIZE: f32 = 0.065;
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
		Hex { q: q, r: r, s: s }
	}

	fn get_coords(&self) -> [f32; 3] {
		let x = 3.0 / 2.0 * APP_SIZE / 2.0 * (-self.q - self.s).to_f32();
		let y =
			3.0_f32.sqrt() * APP_SIZE / 2.0 * ((-self.q - self.s).to_f32() / 2.0 + self.s.to_f32());
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
}
impl AppHexGrid {
	fn new(client: &Client) -> Self {
		let mut desktop_files: Vec<DesktopFile> = get_desktop_files()
			.into_iter()
			.filter_map(|d| parse_desktop_file(d).ok())
			.filter(|d| !d.no_display)
			.collect();

		desktop_files.sort_by_key(|d| d.clone().name.unwrap());

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
							client.get_root(),
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
		AppHexGrid { apps }
	}
}
impl RootHandler for AppHexGrid {
	fn frame(&mut self, info: FrameInfo) {
		for app in &mut self.apps {
			app.frame(info);
		}
	}
}
struct App {
	_text: Text,
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
		let style = TextStyle {
			character_height: APP_SIZE * 0.1,
			bounds: Some(Bounds {
				bounds: [APP_SIZE; 2].into(),
				fit: TextFit::Wrap,
				bounds_align: Alignment::XCenter | Alignment::YCenter,
			}),
			text_align: Alignment::XCenter | Alignment::YCenter,
			..Default::default()
		};
		let protostar = ProtoStar::create_from_desktop_file(parent, position, desktop_file.clone()).ok()?;
		let text = Text::create(
			protostar.content_parent(),
			Transform::from_position_rotation([0.0, 0.0, 0.004], Quat::from_rotation_y(3.14)),
			desktop_file.name.as_deref().unwrap_or("Unknown"),
			style,
		)
		.unwrap();
		Some(App {
			_text: text,
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
