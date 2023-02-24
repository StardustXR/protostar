use crate::xdg::{DesktopFile, Icon, IconType};
use color_eyre::eyre::{eyre, Result};
use glam::Quat;
use mint::Vector3;
use fork::{daemon, Fork, setsid};
use std::process::{Command,Stdio};
use std::os::unix::process::CommandExt;
use stardust_xr_molecules::{
	fusion::{
		client::{Client, FrameInfo, RootHandler},
		core::values::Transform,
		drawable::{MaterialParameter, Model, ResourceID},
		fields::BoxField,
		node::NodeType,
		spatial::Spatial,
		startup_settings::StartupSettings,
	},
	GrabData, Grabbable,
};
use stardust_xr_molecules::{GrabData, Grabbable};
use std::{f32::consts::PI, ffi::CStr, sync::Arc};
use tween::{QuartInOut, Tweener};
use ustr::ustr;
use nix::unistd::fork;

fn model_from_icon(parent: &Spatial, icon: &Icon) -> Result<Model> {
	
	return match &icon.icon_type {
		IconType::Png => {
			let t = Transform::from_rotation_scale(Quat::from_rotation_x(PI/2.0)*Quat::from_rotation_y(PI),[0.03,0.03,0.03]);

			let model = Model::create(
				parent,
				t,
				&ResourceID::new_namespaced("protostar", "hexagon/hexagon"),
			)?;
			model.set_material_parameter(
				1,
				"color",
				MaterialParameter::Color([0.0,1.0,1.0,1.0]),
			)?;
			model.set_material_parameter(
				0,
				"diffuse",
				MaterialParameter::Texture(ResourceID::Direct(icon.path.clone())),
			)?;
			Ok(model)
		}
		IconType::Gltf => Ok(Model::create(
			parent,
			Transform::from_scale([0.05; 3]),
			&ResourceID::new_direct(icon.path.clone())?,
		)?),
		_ => panic!("Invalid Icon Type"),
	};
}

pub struct ProtoStar {
	client: Arc<Client>,
	grabbable: Grabbable,
	field: BoxField,
	icon: Model,
	icon_shrink: Option<Tweener<f32, f64, QuartInOut>>,
	icon_grow: Option<Tweener<f32, f64, QuartInOut>>,
	execute_command: String,
}
impl ProtoStar {
	pub fn create_from_desktop_file(parent: &Spatial, desktop_file: DesktopFile) -> Result<Self> {
		// dbg!(&desktop_file);
		let raw_icons = desktop_file.get_raw_icons();
		let mut icon = raw_icons
			.clone()
			.into_iter()
			.find(|i| match i.icon_type {
				IconType::Gltf => true,
				_ => false,
			})
			.or(
			raw_icons
				.into_iter()
				.max_by_key(|i| i.size)
			);

		match icon{
			Some(i) => {
				icon = match i.cached_process(128) {
					Ok(i) => Some(i),
					_ => None,
			}},
			None => {},
		}

		Self::new_raw(
			parent,
			icon,
			desktop_file.command.ok_or_else(|| eyre!("No command"))?,
		)
	}
	pub fn new_raw(parent: &Spatial, icon: Option<Icon>, execute_command: String) -> Result<Self> {
		let field = BoxField::create(
			parent,
			Transform::default(),
			match icon.as_ref() {
				Some(_) => [0.05, 0.0665, 0.005],
				_ => [0.05; 3],
			},
		)?;
		let grabbable = Grabbable::new(
			parent,
			Transform::default(),
			&field,
			GrabData {
				max_distance: 0.025,
				..Default::default()
			},
		)?;
		field.set_spatial_parent(grabbable.content_parent())?;
		let icon = icon
			.map(|i| model_from_icon(grabbable.content_parent(), &i))
			.unwrap_or_else(|| {
				Ok(Model::create(
					grabbable.content_parent(),
					Transform::from_rotation_scale(Quat::from_xyzw(0.0,0.707,0.707,0.0),[0.03,0.03,0.03]),
					&ResourceID::new_namespaced("protostar", "hexagon/hexagon"),
				)?)
			})?;
		Ok(ProtoStar {
			client: parent.client()?,
			grabbable,
			field,
			icon,
			icon_shrink: None,
			icon_grow: None,
			execute_command,
		})
	}
	pub fn content_parent(&self) -> &Spatial {
		self.grabbable.content_parent()
	}
}
impl RootHandler for ProtoStar {
	fn frame(&mut self, info: FrameInfo) {
		self.grabbable.update(&info);

		if let Some(icon_shrink) = &mut self.icon_shrink {
			if !icon_shrink.is_finished() {
				let scale = icon_shrink.move_by(info.delta);
				self.icon
					.set_scale(None, Vector3::from([scale; 3]))
					.unwrap();
			} 
		if let Some(icon_grow) = &mut self.icon_shrink {
			if !icon_grow.is_finished(){
				let scale = icon_grow.move_by(info.delta);
				self.icon
					.set_scale(None, Vector3::from([scale; 3]))
					.unwrap();
			}
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
			self.icon_shrink = Some(Tweener::quart_in_out(0.03, 0.0, 0.25)); //TODO make the scale a parameter
			let future = startup_settings.generate_startup_token().unwrap();
			let executable = dbg!(self.execute_command.clone());
			//TODO: split the executable string for  the args
			tokio::task::spawn(async move {
				std::env::set_var("STARDUST_STARTUP_TOKEN", future.await.unwrap());
				unsafe {
					Command::new(executable)
					.stdin(Stdio::null())
					.stdout(Stdio::null())
					.stderr(Stdio::null())
					.pre_exec(|| {
						setsid();
						Ok(())
					})
					.spawn()
					.expect("Failed to start child process")
				}
			});
			self.icon_grow = Some(Tweener::quart_in_out(0.00, 0.03, 0.25)); //TODO make the scale a parameter
			dbg!("reached here");
		}
	}
}
