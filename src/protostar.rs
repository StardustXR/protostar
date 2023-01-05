use glam::Quat;
use mint::Vector3;
use nix::unistd::{execv, fork};
use stardust_xr_molecules::{
	fusion::{
		client::{Client, LifeCycleHandler, LogicStepInfo},
		core::values::Transform,
		drawable::Model,
		fields::SphereField,
		node::NodeType,
		resource::NamespacedResource,
		startup_settings::StartupSettings,
	},
	GrabData, Grabbable,
};
use std::{env::args, ffi::CString, path::PathBuf, sync::Arc};
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
	pub fn new(client: Arc<Client>, size: f32, executable_path: PathBuf) -> Self {
		let field =
			SphereField::create(client.get_root(), Vector3::from([0.0; 3]), size * 0.5).unwrap();
		let grabbable = Grabbable::new(
			client.get_root(),
			Transform::default(),
			&field,
			GrabData { max_distance: 0.05 },
		)
		.unwrap();
		field
			.set_spatial_parent(grabbable.content_parent())
			.unwrap();
		let icon = Model::create(
			grabbable.content_parent(),
			Transform::from_scale([size; 3]),
			&NamespacedResource::new("protostar", "default_icon"),
		)
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

		if let Some(icon_shrink) = &mut self.icon_shrink {
			if let Some(scale) = icon_shrink.update(info.delta) {
				self.icon
					.set_scale(None, Vector3::from([scale; 3]))
					.unwrap();
			} else {
				self.client.stop_loop();
			}
		} else if self.grabbable.grab_action().actor_stopped() {
			let startup_settings = StartupSettings::create(&self.field.client().unwrap()).unwrap();
			self.icon
				.set_spatial_parent_in_place(self.client.get_root())
				.unwrap();
			self.grabbable
				.content_parent()
				.set_rotation(
					Some(&self.field.client().unwrap().get_root()),
					Quat::IDENTITY,
				)
				.unwrap();
			startup_settings
				.set_root(self.grabbable.content_parent())
				.unwrap();
			self.icon_shrink = Some(Tweener::new(QuartInOut::new(self.size..=0.0, 0.25)));
			let future = startup_settings.generate_startup_token().unwrap();
			let executable = self.executable_path.clone();
			tokio::task::spawn(async move {
				std::env::set_var("STARDUST_STARTUP_TOKEN", future.await.unwrap());
				if unsafe { fork() }.unwrap().is_parent() {
					let executable = ustr(executable.to_str().unwrap());
					let args = args()
						.skip(1)
						.map(|arg| CString::new(arg))
						.collect::<Result<Vec<_>, _>>()
						.unwrap();
					execv::<CString>(executable.as_cstr(), args.as_slice()).unwrap();
				}
			});
		}
	}
}
