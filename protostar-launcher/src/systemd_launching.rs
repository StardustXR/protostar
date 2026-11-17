use std::path::PathBuf;

use tracing::error;
use which::which;
use zbus::zvariant::{OwnedValue, Type, Value};
use zbus_systemd::systemd1::ManagerProxy;
use zvariant::Array;

#[derive(Type, Value, OwnedValue)]
struct Exec {
	path: String,
	args: Vec<String>,
	ignore_failure: bool,
}

// TODO: double fork and move the pid into a transient .scope in app.slice instead, so the env is
// inherited for real and the env filtering below can go
pub async fn launch_systemd(
	manager: &ManagerProxy<'_>,
	exec: impl IntoIterator<Item = &str>,
	connection_env: impl IntoIterator<Item = (String, String)>,
) {
	fn p(prop: &str, val: &str) -> (String, zbus::zvariant::OwnedValue) {
		(
			prop.to_string(),
			zbus::zvariant::Value::Str(zbus::zvariant::Str::from(val.to_string()))
				.try_into_owned()
				.unwrap(),
		)
	}
	let mut args = exec.into_iter().map(|v| v.to_string()).collect::<Vec<_>>();
	let mut path = PathBuf::from(args[0].clone());
	if path.is_relative()
		&& let Ok(bin_path) = which(&path)
	{
		path = bin_path.clone();
	}
	args[0] = path.to_string_lossy().to_string();
	// TODO: use ExitType=cgroup if version is 250 or higher
	let mut properties = vec![
		p("Type", "exec"),
		p("Slice", "app.slice"),
		(
			"ExecStart".to_string(),
			Array::from(vec![Exec {
				path: args[0].clone(),
				args,
				ignore_failure: false,
			}])
			.try_into()
			.unwrap(),
		),
	];
	let version = manager
		.version()
		.await
		.ok()
		.map(|v| {
			v.chars()
				.take_while(|c| c.is_ascii_digit())
				.collect::<String>()
		})
		.and_then(|v| v.parse::<u32>().ok());
	// the unit would otherwise get the user manager's env, so forward ours like a fork would
	let env_vars = std::env::vars_os()
		.filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
		.filter(|(k, v)| systemd_accepts(k, v) && !inherited_only(k))
		.chain(connection_env)
		.map(|(k, v)| format!("{k}={v}"))
		.collect::<Vec<_>>();
	properties.push((
		"Environment".to_string(),
		Array::from(env_vars).try_into().unwrap(),
	));
	if version.is_some_and(|v| v >= 250) {
		properties.push(p("ExitType", "cgroup"));
	}
	let unit = format!("protostar-app-{}.service", rand::random_range(0..99999));
	if let Err(e) = manager
		.start_transient_unit(unit.clone(), "fail".to_string(), properties, vec![])
		.await
	{
		error!("failed to start {unit} for {path:?}: {e}");
	}
}

/// one bad entry makes systemd reject the whole block, e.g. bash's exported `BASH_FUNC_foo%%`
fn systemd_accepts(k: &str, v: &str) -> bool {
	k.chars().next().is_some_and(|c| !c.is_ascii_digit())
		&& k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
		&& !v.chars().any(|c| c.is_control() && c != '\t' && c != '\n')
}

/// vars that describe the launcher's own process or unit, not the session
fn inherited_only(k: &str) -> bool {
	matches!(
		k,
		"INVOCATION_ID"
			| "JOURNAL_STREAM"
			| "SYSTEMD_EXEC_PID"
			| "MANAGERPID"
			| "MAINPID"
			| "NOTIFY_SOCKET"
			| "WATCHDOG_PID"
			| "WATCHDOG_USEC"
			| "LISTEN_PID"
			| "LISTEN_FDS"
			| "LISTEN_FDNAMES"
			| "PWD" | "OLDPWD"
			| "SHLVL" | "_"
			| "STARDUST_STARTUP_TOKEN"
	) || k.starts_with("MONITOR_")
}
