#[cfg(feature = "systemd_launching")]
mod systemd_launching;
use std::{
	borrow::Cow,
	os::unix::process::CommandExt as _,
	process::{Command, Stdio, exit},
};

use nix::unistd::{ForkResult, setsid};

pub async fn launch(
	exec: Cow<'_, str>,
	connection_env: impl IntoIterator<Item = (String, String)> + Send + Sync + 'static,
) {
	#[cfg(feature = "systemd_launching")]
	{
		let conn = zbus::connection::Connection::session().await.ok();
		let systemd_proxy = if let Some(conn) = conn.as_ref() {
			zbus_systemd::systemd1::ManagerProxy::new(conn).await.ok()
		} else {
			None
		};
		if let Some(systemd) = systemd_proxy {
			crate::systemd_launching::launch_systemd(&systemd, exec, connection_env).await;
		} else {
			double_fork_launch(exec, connection_env);
		}
	};
	#[cfg(not(feature = "systemd_launching"))]
	{
		double_fork_launch(exec, connection_env);
	}
}
fn double_fork_launch(
	exec: Cow<'_, str>,
	connection_env: impl IntoIterator<Item = (String, String)>,
) {
	unsafe {
		if let ForkResult::Child = nix::unistd::fork().expect("fork died???? how?????") {
			let _ = Command::new("sh")
				.arg("-c")
				.arg(exec.to_string())
				.stdin(Stdio::null())
				.stdout(Stdio::null())
				.stderr(Stdio::null())
				.envs(connection_env)
				.pre_exec(|| {
					_ = setsid();
					Ok(())
				})
				.spawn()
				.expect("Failed to start child process");
			exit(0);
		}
	}
}
