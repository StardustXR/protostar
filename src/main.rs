use clap::Parser;
use color_eyre::{eyre::Result, Report};
use manifest_dir_macros::directory_relative_path;
use protostar::{protostar::ProtoStar, xdg::parse_desktop_file};
use stardust_xr_fusion::client::Client;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
	// #[clap(short, long)]
	desktop_file: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	color_eyre::install()?;
	let args = Args::parse();
	let (client, event_loop) = Client::connect_with_async_loop().await?;
	client.set_base_prefixes(&[directory_relative_path!("res")]);

	let protostar = ProtoStar::create_from_desktop_file(
		client.get_root(),
		[0.0, 0.0, 0.0],
		parse_desktop_file(args.desktop_file).map_err(Report::msg)?,
	)?;

	let _root = client.wrap_root(protostar);

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e??,
	};
	Ok(())
}
