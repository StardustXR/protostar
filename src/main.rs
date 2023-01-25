use clap::Parser;
use color_eyre::{
	eyre::{bail, Result},
	Report,
};
use manifest_dir_macros::directory_relative_path;
use protostar::{protostar::ProtoStar, xdg::parse_desktop_file};
use stardust_xr_molecules::fusion::client::Client;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
	#[clap(short, long, default_value_t = 0.1)]
	size: f32,
	#[clap(
		short,
		long,
		conflicts_with = "command",
		required_unless_present = "command",
		conflicts_with = "icon",
		required_unless_present = "icon"
	)]
	desktop_file: Option<PathBuf>,
	#[clap(short, long, conflicts_with = "desktop_file", requires = "command")]
	icon: Option<PathBuf>,
	#[clap(short, long, conflicts_with = "desktop_file", requires = "icon")]
	command: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	color_eyre::install()?;
	let args = Args::parse();
	let (client, event_loop) = Client::connect_with_async_loop().await?;
	client.set_base_prefixes(&[directory_relative_path!("res")]);

	let protostar = if let Some(desktop_file) = args.desktop_file {
		ProtoStar::create_from_desktop_file(
			client.get_root(),
			parse_desktop_file(desktop_file).map_err(|e| Report::msg(e))?,
		)?
	} else if let Some(command) = args.command {
		ProtoStar::new_raw(client.get_root(), None, command)?
	} else {
		bail!("No command or desktop file, nothing to launch.");
	};

	let _root = client.wrap_root(protostar);

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e??,
	};
	Ok(())
}
