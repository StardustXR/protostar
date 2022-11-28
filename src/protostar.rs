use glam::Quat;
use mint::Vector3;
use nix::unistd::{execv, fork};
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
use std::{ffi::CString, path::PathBuf, sync::Arc};
use tween::{QuartInOut, Tweener};
use ustr::ustr;

pub struct ProtoStar {
	client: Arc<Client>,
	grabbable: Grabbable,
	field: SphereField,
	icon: Model,
	icon_shrink: Option<Tweener<QuartInOut<f32, f64>>>,
	size: f32,
	executable_path: PathBuf,
}
impl ProtoStar {
	pub fn new(client: Arc<Client>, icon: PathBuf, size: f32, executable_path: PathBuf) -> Self {
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
			client,
			grabbable,
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
		self.grabbable.update();
		if self.grabbable.grab_action().actor_stopped() {
			let startup_settings =
				StartupSettings::create(&self.field.spatial.client().unwrap()).unwrap();
			self.grabbable
				.content_parent()
				.set_rotation(
					Some(&self.field.client().unwrap().get_root()),
					Quat::IDENTITY,
				)
				.unwrap();
			self.icon
				.set_spatial_parent_in_place(self.client.get_root())
				.unwrap();
			startup_settings
				.set_root(self.grabbable.content_parent())
				.unwrap();
			self.icon_shrink = Some(Tweener::new(QuartInOut::new(self.size..=0.0, 0.25)));
			let future = startup_settings.generate_desktop_startup_id().unwrap();
			let executable = self.executable_path.clone();
			tokio::task::spawn(async move {
				std::env::set_var("DESKTOP_STARTUP_ID", future.await.unwrap());
				if unsafe { fork() }.unwrap().is_parent() {
					let executable = ustr(executable.to_str().unwrap());
					execv::<CString>(executable.as_cstr(), &[]).unwrap();
				}
			});
		}
		if let Some(icon_shrink) = &mut self.icon_shrink {
			if let Some(scale) = icon_shrink.update(info.delta) {
				self.icon
					.set_scale(None, Vector3::from([scale; 3]))
					.unwrap();
			} else {
				self.client.stop_loop();
			}
		}
	}
}
