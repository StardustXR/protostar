use asteroids::elements::{
	Grabbable, Lines, Model, ModelPart, PointerMode, Text, line_from_points,
};
use asteroids::{CustomElement, Element, Reify, Transformable};
use glam::{Quat, Vec3};
use mint::{Quaternion, Vector3};
use protostar::application::Application;
use protostar::xdg::{DesktopFile, Icon, IconType};
use serde::{Deserialize, Serialize};
use stardust_xr_fusion::drawable::{TextBounds, TextFit};
use stardust_xr_fusion::node::NodeError;
use stardust_xr_fusion::values::ResourceID;
use stardust_xr_fusion::{
	drawable::{MaterialParameter, XAlign, YAlign},
	fields::{CylinderShape, Shape},
	spatial::Transform,
};
use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::app_launcher::AppLauncher;
use crate::{ACTIVATION_DISTANCE, APP_SIZE, DEFAULT_HEX_COLOR, MODEL_SCALE};

#[derive(Debug, Serialize, Deserialize)]
pub struct App {
	pub app: Application,
	#[serde(skip)]
	icon: OnceLock<Icon>,
	pos: Vector3<f32>,
	rot: Quaternion<f32>,
	#[serde(skip)]
	launched: AtomicBool,
}
impl App {
	pub fn new(desktop_entry: DesktopFile) -> Result<Self, NodeError> {
		let app = Application::create(desktop_entry)?;
		Ok(App {
			app,
			icon: OnceLock::default(),
			pos: [0.0; 3].into(),
			rot: Quat::IDENTITY.into(),
			launched: AtomicBool::new(false),
		})
	}

	pub fn load_icon(&self) {
		if self.icon.get().is_none()
			&& let Some(icon) = self
				.app
				.icon(64, true)
				.and_then(|i| i.cached_process(64).ok())
		{
			let _ = self.icon.set(icon);
		}
	}

	// Helper functions for creating app components
	fn create_model(&self) -> impl Element<Self> {
		match self.icon.get().as_ref().map(|i| (i.icon_type.clone(), i)) {
			Some((IconType::Gltf, icon)) => Model::direct(icon.path.clone())
				.unwrap()
				.transform(Transform::from_rotation_scale(
					Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
					[MODEL_SCALE; 3],
				))
				.build(),
			other => {
				let model = Model::namespaced("protostar", "hexagon/hexagon")
					.transform(Transform::from_rotation_scale(
						Quat::from_rotation_x(PI / 2.0) * Quat::from_rotation_y(PI),
						[APP_SIZE / 2.0; 3],
					))
					.part(
						ModelPart::new("Hex")
							.mat_param("color", MaterialParameter::Color(DEFAULT_HEX_COLOR)),
					);

				match other {
					Some((IconType::Png, icon)) => model.part(ModelPart::new("Icon").mat_param(
						"diffuse",
						MaterialParameter::Texture(ResourceID::Direct(icon.path.clone())),
					)),
					_ => model,
				}
				.build()
			}
		}
	}
}
impl Reify for App {
	#[tracing::instrument(skip_all)]
	fn reify(&self) -> impl Element<Self> {
		// The field shape for the grabbable
		let field_shape = Shape::Cylinder(CylinderShape {
			radius: APP_SIZE / 2.0,
			length: 0.01,
		});

		let converted = Vec3::from(self.pos);
		let length = converted.length();
		let direction = converted.normalize_or_zero();

		Lines::new([line_from_points(vec![
			Vec3::from([0.0; 3]),
			(length < ACTIVATION_DISTANCE) as u32 as f32
				* direction * length.clamp(0.0, ACTIVATION_DISTANCE),
		])])
		.build()
		.child(
			Grabbable::new(
				field_shape,
				self.pos,
				self.rot,
				move |state: &mut Self, pos, rot| {
					state.pos = pos;
					state.rot = rot;
				},
			)
			.grab_stop({
				move |state: &mut Self| {
					let pos_vec = Vec3::from(state.pos);
					if pos_vec.length() > ACTIVATION_DISTANCE {
						// state.app.launch(launch_space)
						state.launched.store(true, Ordering::Relaxed);
					} else {
						state.pos = [0.0; 3].into();
						state.rot = Quat::IDENTITY.into();
					}
				}
			})
			.field_transform(Transform::from_rotation(Quat::from_rotation_x(FRAC_PI_2)))
			.pointer_mode(PointerMode::Align)
			.max_distance(0.05)
			.build()
			.child(self.create_model())
			.children(self.launched.load(Ordering::Relaxed).then(|| {
				AppLauncher::new(&self.app)
					.done(|state: &mut Self| {
						state.launched.store(false, Ordering::Relaxed);
						state.pos = [0.0; 3].into();
						state.rot = Quat::IDENTITY.into();
					})
					.build()
			}))
			.child(
				Text::new(self.app.name().unwrap_or_default())
					.character_height(0.005)
					.bounds(TextBounds {
						bounds: [0.05, 0.05].into(),
						fit: TextFit::Wrap,
						anchor_align_x: XAlign::Center,
						anchor_align_y: YAlign::Center,
					})
					.text_align_x(XAlign::Center)
					.text_align_y(YAlign::Center)
					.pos([0.0, -APP_SIZE * 0.35, 0.001])
					.build(),
			),
		)
	}
}
