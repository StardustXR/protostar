mod protostar;

use manifest_dir_macros::directory_relative_path;
use protostar::ProtoStar;
use stardust_xr_molecules::fusion::client::Client;
use std::{env::args, path::PathBuf, str::FromStr};

#[tokio::main(flavor = "current_thread")]
async fn main() {
	let (client, event_loop) = Client::connect_with_async_loop().await.unwrap();
	client.set_base_prefixes(&[directory_relative_path!("res")]);

	let _root = client.wrap_root(ProtoStar::new(
		client.clone(),
		PathBuf::from_str(&args().nth(2).unwrap()).unwrap(),
		0.1,
		PathBuf::from_str(&args().nth(1).unwrap()).unwrap(),
	));

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e.unwrap().unwrap(),
	}
}
