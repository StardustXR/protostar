use protostar::application::Application;
use stardust_xr_asteroids::{Context, CustomElement, ValidState};
use stardust_xr_fusion::{
	node::{NodeError, NodeType},
	root::FrameInfo,
	spatial::{Spatial, SpatialAspect, SpatialRef, Transform},
};
use std::fmt::Debug;

pub struct AppLauncher<State: ValidState>(Application, Box<dyn Fn(&mut State) + Send + Sync>);
impl<State: ValidState> AppLauncher<State> {
	pub fn new(app: &Application) -> Self {
		AppLauncher(app.clone(), Box::new(|_| {}))
	}
	pub fn done<F: Fn(&mut State) + Send + Sync + 'static>(mut self, f: F) -> Self {
		self.1 = Box::new(f);
		self
	}
}
impl<State: ValidState> CustomElement<State> for AppLauncher<State> {
	type Inner = (Spatial, bool);
	type Resource = ();
	type Error = NodeError;

	fn create_inner(
		&self,
		_asteroids_context: &stardust_xr_asteroids::Context,
		info: stardust_xr_asteroids::CreateInnerInfo,
		_resource: &mut Self::Resource,
	) -> Result<Self::Inner, Self::Error> {
		let spatial = Spatial::create(
			info.parent_space.client().get_root(),
			Transform::identity(),
			false,
		)?;
		spatial.set_relative_transform(info.parent_space, Transform::from_translation([0.0; 3]))?;
		Ok((spatial, false))
	}

	fn diff(&self, _old_self: &Self, _inner: &mut Self::Inner, _resource: &mut Self::Resource) {}

	fn frame(
		&self,
		_context: &Context,
		_info: &FrameInfo,
		state: &mut State,
		inner: &mut Self::Inner,
	) {
		if !inner.1 {
			let _ = self.0.launch(&inner.0);
			(self.1)(state);
			inner.1 = true;
		}
	}

	fn spatial_aspect(&self, inner: &Self::Inner) -> SpatialRef {
		inner.0.clone().as_spatial_ref()
	}
}
impl<State: ValidState> Debug for AppLauncher<State> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("ImperativeSpatial").finish()
	}
}
