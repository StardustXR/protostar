mod single;

use clap::Parser;
use color_eyre::{eyre::Result, Report};
use manifest_dir_macros::directory_relative_path;
use protostar::xdg::parse_desktop_file;
use stardust_xr_fusion::{client::Client, root::RootAspect};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use crate::single::Single;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
	// #[clap(short, long)]
	desktop_file: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	tracing_subscriber::fmt()
		.compact()
		.with_env_filter(EnvFilter::from_env("LOG_LEVEL"))
		.init();
	color_eyre::install()?;
	let args = Args::parse();
	let owned_client = Client::connect().await?;
	let client = owned_client.handle();
	let async_loop = owned_client.async_event_loop();
	client
		.get_root()
		.set_base_prefixes(&[directory_relative_path!("../res").to_string()])?;

	let mut protostar = Single::create_from_desktop_file(
		client.get_root(),
		[0.0, 0.0, 0.0],
		parse_desktop_file(args.desktop_file).map_err(Report::msg)?,
	)?;

	let mut owned_client = async_loop.stop().await.unwrap();
	let event_loop = owned_client.sync_event_loop(|handle, _| {
		let Some(event) = handle.get_root().recv_root_event() else {
			return;
		};
		match event {
			stardust_xr_fusion::root::RootEvent::Ping { response } => response.send(Ok(())),
			stardust_xr_fusion::root::RootEvent::Frame { info } => {
				protostar.frame(info);
			}
			stardust_xr_fusion::root::RootEvent::SaveState { response } => {
				response.send(protostar.save_state());
			}
		}
	});

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e?,
	};
	Ok(())
}
