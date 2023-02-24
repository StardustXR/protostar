use std::path::PathBuf;

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
	spatial::Spatial, drawable::{Text, TextStyle, Bounds, TextFit, Alignment}, core::values::Transform,
};
use tween::TweenTime;

const APP_LIMIT: usize = 18;
const APP_SIZE: f32 = 0.055;

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


		let mut circles = 1;
		let mut current_num = 6;
		let mut target = 6;
		desktop_files.sort_by_key(|d| d.clone().name.unwrap());
		dbg!(&desktop_files);
		let apps :Vec<App>= desktop_files
			.into_iter()
			.enumerate()
			.filter(|(i,_)| *i < APP_LIMIT)
			.filter_map(|(i,d)|{
				let angle = ((3.14*2.0)/current_num.to_f32())*(i%current_num).to_f32();

				let x = (target-i)%circles; // this gives 0,3,2,1 but I need 0,1,2,3
				dbg!(x);
				let m = circles.to_f32()*APP_SIZE - (x.to_f32()*0.5*(circles-1).to_f32()*(APP_SIZE/2.0));
				
				let position = [angle.sin()*m,angle.cos()*m,0.0];

				if (i+1) == target {
					circles += 1;
					current_num += 6;
					target = target + current_num;
				}

				return App::new(client.get_root(),position,d);
			})
			.collect();
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
