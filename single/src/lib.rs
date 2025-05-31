mod app;
mod app_launcher;

pub use app::App;
use stardust_xr_fusion::values::color::{Rgba, color_space::LinearRgb, rgba_linear};

// Constants from original implementation
pub const APP_SIZE: f32 = 0.06;
pub const PADDING: f32 = 0.005;
pub const MODEL_SCALE: f32 = 0.03;
pub const ACTIVATION_DISTANCE: f32 = 0.05;

pub const DEFAULT_HEX_COLOR: Rgba<f32, LinearRgb> = rgba_linear!(0.211, 0.937, 0.588, 1.0);
pub const BTN_SELECTED_COLOR: Rgba<f32, LinearRgb> = rgba_linear!(0.0, 1.0, 0.0, 1.0);
pub const BTN_COLOR: Rgba<f32, LinearRgb> = rgba_linear!(1.0, 1.0, 0.0, 1.0);
