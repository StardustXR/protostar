use std::{borrow::Cow, path::PathBuf};

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

pub async fn launch_systemd(
	manager: &ManagerProxy<'_>,
	exec: Cow<'_, str>,
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
	let mut args = exec.split(' ').map(|v| v.to_string()).collect::<Vec<_>>();
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
	let env_vars = connection_env
		.into_iter()
		.map(|(k, v)| format!("{k}={v}"))
		.collect::<Vec<_>>();
	properties.push((
		"Environment".to_string(),
		Array::from(env_vars).try_into().unwrap(),
	));
	if version.is_some_and(|v| v >= 250) {
		properties.push(p("ExitType", "cgroup"));
	}
	manager
		.start_transient_unit(
			format!("protostar-app-{}.service", rand::random_range(0..99999)),
			"fail".to_string(),
			properties,
			vec![],
		)
		.await
		.unwrap();
}
