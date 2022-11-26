use glam::Quat;
use mint::Vector3;
use stardust_xr_molecules::{
	fusion::{
		client::{Client, LifeCycleHandler, LogicStepInfo},
		drawable::Model,
		fields::SphereField,
		node::NodeType,
		startup_settings::StartupSettings,
	},
	Grabbable,
};
use std::{path::PathBuf, process::Command};
use tween::{QuartInOut, Tweener};

pub struct ProtoStar {
	grabbable: Option<Grabbable>,
	field: SphereField,
	icon: Model,
	icon_shrink: Option<Tweener<QuartInOut<f32, f64>>>,
	size: f32,
	executable_path: PathBuf,
}
impl ProtoStar {
	pub fn new(client: &Client, icon: PathBuf, size: f32, executable_path: PathBuf) -> Self {
		let field = SphereField::builder()
			.spatial_parent(client.get_root())
			.radius(size * 0.5)
			.build()
			.unwrap();
		let grabbable = Grabbable::new(client.get_root(), &field).unwrap();
		field
			.set_spatial_parent(grabbable.content_parent())
			.unwrap();
		let icon = Model::builder()
			.spatial_parent(grabbable.content_parent())
			.resource(&icon)
			.scale(Vector3::from([size; 3]))
			.build()
			.unwrap();
		ProtoStar {
			grabbable: Some(grabbable),
			field,
			icon,
			icon_shrink: None,
			size,
			executable_path,
		}
	}
}
impl LifeCycleHandler for ProtoStar {
	fn logic_step(&mut self, info: LogicStepInfo) {
		if let Some(grabbable) = &mut self.grabbable {
			grabbable.update();
			if grabbable.grab_action().actor_stopped() {
				let startup_settings =
					StartupSettings::create(&self.field.spatial.client().unwrap()).unwrap();
				grabbable
					.content_parent()
					.set_rotation(
						Some(&self.field.client().unwrap().get_root()),
						Quat::IDENTITY,
					)
					.unwrap();
				startup_settings
					.set_root(grabbable.content_parent())
					.unwrap();
				drop(self.grabbable.take());
				self.icon_shrink = Some(Tweener::new(QuartInOut::new(self.size..=0.0, 0.25)));
				let future = startup_settings.generate_desktop_startup_id().unwrap();
				let mut command = Command::new(self.executable_path.clone());
				tokio::task::spawn(async move {
					command.env("DESKTOP_STARTUP_ID", future.await.unwrap());
					command.spawn().unwrap();
					drop(startup_settings);
				});
			}
		}
		if let Some(icon_shrink) = &mut self.icon_shrink {
			if let Some(scale) = icon_shrink.update(info.delta) {
				self.icon
					.set_scale(None, Vector3::from([scale; 3]))
					.unwrap();
			}
		}
	}
}
