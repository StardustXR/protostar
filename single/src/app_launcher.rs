use protostar::application::Application;
use stardust_xr_asteroids::{Context, CustomElement, ValidState};
use stardust_xr_fusion::{
	Error as NodeError, client::FrameInfo, spatial::{Spatial, SpatialExt as _, SpatialRef, Transform}
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
	type Inner = (Spatial, SpatialRef, bool);
	type Error = NodeError;

	async fn create_inner(
		&self,
		context: &stardust_xr_asteroids::Context,
		info: stardust_xr_asteroids::CreateInnerInfo,
	) -> Result<Self::Inner, Self::Error> {
		let (spatial, spatial_ref) = Spatial::new(
			&context.stardust_client,
			context.stardust_client.root(),
			Transform::IDENTITY,
		)
		.await?;
		spatial.set_relative_transform(info.parent_space, Transform::from_translation([0.0; 3]))?;
		Ok((spatial, spatial_ref, false))
	}

	fn diff(
		&self,
		_old_self: &Self,
		_context: &stardust_xr_asteroids::Context,
		_inner: &mut Self::Inner,
	) {
	}

	fn frame(
		&self,
		context: &Context,
		_info: &FrameInfo,
		state: &mut State,
		inner: &mut Self::Inner,
	) {
		if !inner.2 {
			let _ = self.0.launch(&context.stardust_client, &inner.1);
			(self.1)(state);
			inner.2 = true;
		}
	}
}
impl<State: ValidState> Debug for AppLauncher<State> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("ImperativeSpatial").finish()
	}
}
