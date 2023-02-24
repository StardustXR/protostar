use color_eyre::eyre::Result;
use glam::Quat;
use manifest_dir_macros::directory_relative_path;
use mint::Vector3;
use protostar::{
	protostar::ProtoStar,
	xdg::{get_desktop_files, parse_desktop_file, DesktopFile},
};
use stardust_xr_molecules::fusion::{
	client::{Client, FrameInfo, RootHandler},
	core::values::Transform,
	drawable::{Alignment, Bounds, Text, TextFit, TextStyle},
	spatial::Spatial,
};

const APP_LIMIT: usize = 300;
const APP_SIZE: f32 = 0.05;
const GRID_PADDING: f32 = 0.01;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	color_eyre::install().unwrap();
	tracing_subscriber::fmt()
		.with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
		.pretty()
		.init();
	let (client, event_loop) = Client::connect_with_async_loop().await?;
	client.set_base_prefixes(&[directory_relative_path!("res")]);

	let _root = client.wrap_root(AppGrid::new(&client))?;

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e??,
	};
	Ok(())
}

struct AppGrid {
	apps: Vec<App>,
	//style: TextStyle,
}
impl AppGrid {
	fn new(client: &Client) -> Self {
		let apps = get_desktop_files()
			.into_iter()
			.filter_map(|d| parse_desktop_file(d).ok())
			.filter(|d| !d.no_display)
			.enumerate()
			.filter(|(i, _)| *i <= APP_LIMIT)
			.filter_map(|(i, a)| {
				App::new(
					client.get_root(),
					[
						(i % 10) as f32 * (APP_SIZE + GRID_PADDING),
						(i / 10) as f32 * (APP_SIZE + GRID_PADDING),
						0.0,
					],
					a,
					//style,
				)
			})
			.collect::<Vec<_>>();
		AppGrid { apps }
	}
}
impl RootHandler for AppGrid {
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
		//style: TextStyle,
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
		let protostar = ProtoStar::create_from_desktop_file(parent, desktop_file.clone()).ok()?;
		let text = Text::create(
			protostar.content_parent(),
			Transform::from_position_rotation(
				[0.0, 0.0, APP_SIZE / 2.0],
				Quat::from_rotation_y(3.14),
			),
			desktop_file.name.as_deref().unwrap_or("Unknown"),
			style,
		)
		.unwrap();
		protostar
			.content_parent()
			.set_position(None, position)
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
