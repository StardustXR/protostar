mod desktop_file;
mod protostar;

use manifest_dir_macros::directory_relative_path;
use protostar::ProtoStar;
use stardust_xr_molecules::fusion::client::Client;
use std::{
	env::{args, current_dir},
	path::Path,
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
	let (client, event_loop) = Client::connect_with_async_loop().await.unwrap();
	client.set_base_prefixes(&[directory_relative_path!("res")]);

	let _root = client.wrap_root(ProtoStar::new(
		client.clone(),
		0.1,
		current_dir()
			.unwrap()
			.join(Path::new(&args().nth(1).unwrap())),
	));

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e.unwrap().unwrap(),
	}
}
