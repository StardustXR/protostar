use color::rgba_linear;
use color_eyre::eyre::Result;
use glam::{Quat, Vec3};
use mint::Vector3;
use protostar::{
	application::Application,
	xdg::{DesktopFile, Icon, IconType},
};
use stardust_xr_fusion::{
	client::{ClientState, FrameInfo, RootHandler},
	core::values::ResourceID,
	drawable::{
		MaterialParameter, Model, ModelPartAspect, Text, TextBounds, TextFit, TextStyle, XAlign,
		YAlign,
	},
	fields::BoxField,
	node::NodeType,
	spatial::{Spatial, SpatialAspect, Transform},
};
use stardust_xr_molecules::{Grabbable, GrabbableSettings};
use std::f32::consts::PI;
use tween::{QuartInOut, Tweener};

const MODEL_SCALE: f32 = 0.05;
const ACTIVATION_DISTANCE: f32 = 0.5;

fn model_from_icon(parent: &Spatial, icon: &Icon) -> Result<Model> {
	match &icon.icon_type {
		IconType::Png => {
			let t = Transform::from_rotation_scale(
				Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
				[MODEL_SCALE; 3],
			);

			let model = Model::create(
				parent,
				t,
				&ResourceID::new_namespaced("protostar", "hexagon/hexagon"),
			)?;
			model.model_part("Hex")?.set_material_parameter(
				"color",
				MaterialParameter::Color(rgba_linear!(0.0, 1.0, 1.0, 1.0)),
			)?;
			model.model_part("Icon")?.set_material_parameter(
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
	}
}

pub struct Single {
	application: Application,
	root: Spatial,
	position: Vector3<f32>,
	grabbable: Grabbable,
	_field: BoxField,
	icon: Model,
	label: Option<Text>,
	grabbable_shrink: Option<Tweener<f32, f64, QuartInOut>>,
	grabbable_grow: Option<Tweener<f32, f64, QuartInOut>>,
	grabbable_move: Option<Tweener<f32, f64, QuartInOut>>,
	currently_shown: bool,
}

impl Single {
	pub fn create_from_desktop_file(
		parent: &impl SpatialAspect,
		position: impl Into<Vector3<f32>>,
		desktop_file: DesktopFile,
	) -> Result<Self> {
		let root = Spatial::create(parent, Transform::identity(), false)?;
		let position = position.into();
		let field = BoxField::create(&root, Transform::identity(), [MODEL_SCALE * 2.0; 3])?;
		let application = Application::create(desktop_file)?;
		let icon = application.icon(128, false);
		let grabbable = Grabbable::create(
			&root,
			Transform::from_translation(position),
			&field,
			GrabbableSettings {
				max_distance: 0.01,
				..Default::default()
			},
		)?;
		grabbable.content_parent().set_spatial_parent(&root)?;
		field.set_spatial_parent(grabbable.content_parent())?;
		let icon = icon
			.map(|i| model_from_icon(grabbable.content_parent(), &i))
			.unwrap_or_else(|| {
				Ok(Model::create(
					grabbable.content_parent(),
					Transform::from_scale([MODEL_SCALE; 3]),
					&ResourceID::new_namespaced("protostar", "default_icon"),
				)?)
			})?;

		let label_style = TextStyle {
			character_height: MODEL_SCALE * 4.0,
			bounds: Some(TextBounds {
				bounds: [1.0; 2].into(),
				fit: TextFit::Wrap,
				anchor_align_x: XAlign::Center,
				anchor_align_y: YAlign::Center,
			}),
			text_align_x: XAlign::Center,
			text_align_y: YAlign::Center,
			..Default::default()
		};
		let label = application.name().and_then(|name| {
			Text::create(
				&icon,
				Transform::from_translation_rotation(
					[0.0, 0.1, -(MODEL_SCALE * 8.0)],
					Quat::from_rotation_x(PI * 0.5),
				),
				name,
				label_style,
			)
			.ok()
		});
		Ok(Single {
			root,
			position,
			grabbable,
			_field: field,
			label,
			application,
			icon,
			grabbable_shrink: None,
			grabbable_grow: None,
			grabbable_move: None,
			currently_shown: true,
		})
	}
	pub fn content_parent(&self) -> &Spatial {
		self.grabbable.content_parent()
	}
}
impl RootHandler for Single {
	fn frame(&mut self, info: FrameInfo) {
		let _ = self.grabbable.update(&info);

		if let Some(grabbable_move) = &mut self.grabbable_move {
			if !grabbable_move.is_finished() {
				let scale = grabbable_move.move_by(info.delta);
				self.grabbable
					.content_parent()
					.set_relative_transform(
						&self.root,
						Transform::from_translation([
							self.position.x * scale,
							self.position.y * scale,
							self.position.z * scale,
						]),
					)
					.unwrap();
			} else {
				if grabbable_move.final_value() == 0.0001 {
					self.icon.set_enabled(false).unwrap();
					if let Some(label) = self.label.as_ref() {
						label.set_enabled(false).unwrap()
					}
				}
				self.grabbable_move = None;
			}
		}
		if let Some(grabbable_shrink) = &mut self.grabbable_shrink {
			if !grabbable_shrink.is_finished() {
				let scale = grabbable_shrink.move_by(info.delta);
				self.grabbable
					.content_parent()
					.set_relative_transform(&self.root, Transform::from_scale([scale; 3]))
					.unwrap();
			} else {
				self.grabbable
					.content_parent()
					.set_spatial_parent(&self.root)
					.unwrap();
				if self.currently_shown {
					self.grabbable_grow = Some(Tweener::quart_in_out(0.0001, 1.0, 0.25));
					self.grabbable.cancel_angular_velocity();
					self.grabbable.cancel_linear_velocity();
				}
				self.grabbable_shrink = None;
				self.grabbable
					.content_parent()
					.set_relative_transform(&self.root, Transform::from_translation(self.position))
					.unwrap();
				self.grabbable
					.content_parent()
					.set_relative_transform(&self.root, Transform::from_rotation(Quat::default()))
					.unwrap();
				self.icon
					.set_local_transform(Transform::from_rotation(
						Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
					))
					.unwrap();
			}
		} else if let Some(grabbable_grow) = &mut self.grabbable_grow {
			if !grabbable_grow.is_finished() {
				let scale = grabbable_grow.move_by(info.delta);
				self.grabbable
					.content_parent()
					.set_relative_transform(&self.root, Transform::from_scale([scale; 3]))
					.unwrap();
			} else {
				self.grabbable
					.content_parent()
					.set_spatial_parent(&self.root)
					.unwrap();
				self.grabbable_grow = None;
			}
		} else if self.grabbable.grab_action().actor_stopped() {
			self.grabbable_shrink = Some(Tweener::quart_in_out(MODEL_SCALE, 0.0001, 0.25));

			let application = self.application.clone();
			let space = self.content_parent().alias();
			let root = self.root.alias();

			//TODO: split the executable string for the args
			tokio::task::spawn(async move {
				let distance_vector = space
					.get_transform(&root)
					.await
					.unwrap()
					.translation
					.unwrap();
				let distance = Vec3::from(distance_vector).length_squared();

				if distance > ACTIVATION_DISTANCE {
					let _ = application.launch(&space);
				}
			});
		}
	}

	fn save_state(&mut self) -> ClientState {
		ClientState::from_root(self.content_parent())
	}
}
