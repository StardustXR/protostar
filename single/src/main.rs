use asteroids::{ClientState, CustomElement, Element, Migrate, Reify, client, elements::Spatial};
use clap::Parser;
use protostar::xdg::DesktopFile;
use serde::{Deserialize, Serialize};
use single::App;
use stardust_xr_fusion::project_local_resources;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
	// #[clap(short, long)]
	desktop_file: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
	tracing_subscriber::fmt()
		.compact()
		.with_env_filter(EnvFilter::from_env("LOG_LEVEL"))
		.init();
	client::run::<Single>(&[&project_local_resources!("../res")]).await
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Single {
	app: Option<App>,
}
impl Migrate for Single {
	type Old = Self;
}
impl ClientState for Single {
	const APP_ID: &'static str = "org.stardustxr.protostar.single";

	fn initial_state_update(&mut self) {
		let desktop_file_path = Args::parse().desktop_file;

		let app = App::new(DesktopFile::parse(desktop_file_path).unwrap()).unwrap();
		app.load_icon();
		self.app.replace(app);
	}
}
impl Reify for Single {
	#[tracing::instrument(skip_all)]
	fn reify(&self) -> impl Element<Self> {
		Spatial::default().build().maybe_child(
			self.app
				.as_ref()
				.map(|app| app.reify_substate(|state: &mut Self| state.app.as_mut())),
		)
	}
}
