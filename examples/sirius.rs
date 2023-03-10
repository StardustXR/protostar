#![allow(dead_code)]

use color_eyre::eyre::Result;
use glam::Vec3;
use manifest_dir_macros::directory_relative_path;
use protostar::protostar::ProtoStar;
use stardust_xr_fusion::{
	client::{Client, FrameInfo, RootHandler},
	core::values::Transform,
	drawable::{MaterialParameter, Model, ResourceID},
	node::NodeError, input::{InputData, InputDataType}, spatial::Spatial, fields::BoxField,
};
use stardust_xr_molecules::{touch_plane::TouchPlane, Grabbable, GrabData};



#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
	color_eyre::install()?;
	let (client, event_loop) = Client::connect_with_async_loop().await?;
	client.set_base_prefixes(&[directory_relative_path!("res")]);

	let _wrapped_root = client.wrap_root(Sirius::new(&client)?)?;

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e??,
	}
	Ok(())
}

struct Star {
    cli: ProtoStar
}

impl Star {
    fn new(parent: &Spatial, name: Option<&str>,path: &str) -> Option<Self> {
        let cli = ProtoStar::new_raw(parent, Vec3::default(), name, None, path.to_string()).unwrap();
        Some(Star {
            cli,
        })
    }
}

impl RootHandler for Star {
    fn frame(&mut self, info: FrameInfo) {
        self.cli.frame(info);
    }
}

struct Sirius {
	touch_plane: TouchPlane,
	model: Model,
    root: Spatial,
    clients: Vec<Star>,
    visibility: bool,
    grabbable: Grabbable,

}
impl Sirius {
	fn new(client: &Client) -> Result<Self, NodeError> {
        let client_list: Vec<(Option<&str>, &str)> = Vec::from([
            (Some("Magnetar"), "$HOME/repos/stardust/telescope/repos/magnetar/target/release/magnetar"),
            (Some("Atmosphere"), "$HOME/repos/stardust/telescope/repos/atmosphere/target/release/atmosphere"),
            (Some("Manifold"), "$HOME/repos/stardust/telescope/repos/manifold/target/release/manifold"),
        ]);
        let root = Spatial::create(client.get_root(), Transform::default(), false).unwrap();
        
        let field = BoxField::create(&root, Transform::default(), Vec3::from([0.1;3])).unwrap();
        let grabbable = Grabbable::new(&root, Transform::default(), &field, GrabData::default())?;
		let touch_plane = TouchPlane::new(grabbable.content_parent(), Transform::default(), [0.1; 2], 0.03)?;
        let mut clients = Vec::new();
        for clientkv in client_list {
            clients.push(Star::new(grabbable.content_parent(), clientkv.0, clientkv.1).unwrap());
        }
		let model = Model::create(
            grabbable.content_parent(),
			Transform::default(),
			&ResourceID::new_namespaced("protostar", "button"),
		)?;
        field.set_spatial_parent(grabbable.content_parent())?;
        let visibility = false;

		Ok(Sirius { touch_plane, model , root, clients, visibility, grabbable})
	}

//    fn left_hand(input_data: &InputData, _: &()) -> bool {
//       match &input_data.input {
//            InputDataType::Hand(h) => !h.right,
//            _ => false,
//        }
//    }
}
impl RootHandler for Sirius {
	fn frame(&mut self, info: FrameInfo) {

        for app in &mut self.clients {
            app.frame(info);
        }
        
        self.grabbable.update(&info);
        self.touch_plane.update();
		if self.touch_plane.touch_started() {
			println!("Touch started");
            self.visibility = !self.visibility;
            match self.visibility {
                true => for star in self.clients.iter().enumerate() {
                    let mut starpos = (star.0 as f32 +1.0)/10.0;
                    match starpos % 0.2 == 0.0 {
                        true => starpos = -starpos/2.0,
                        false => starpos = (starpos - 0.1)/2.0,
                    }
                    println!("{}", starpos);
                    star.1.cli.content_parent().set_position(Some(&self.grabbable.content_parent()), Vec3::from([starpos,0.1,0.0])).ok();
                },
                false => for star in &self.clients {
                    star.cli.content_parent().set_position(Some(&self.grabbable.content_parent()), Vec3::from([0.0,0.0,0.0])).ok();
                },

            }
			let color = [0.0, 1.0, 0.0, 1.0];
			self.model
				.set_material_parameter(0, "color", MaterialParameter::Color(color))
				.unwrap();
			self.model
				.set_material_parameter(
					0,
					"emission_factor",
					MaterialParameter::Color(color.map(|c| c * 0.75)),
				)
				.unwrap();
		}

		if self.touch_plane.touch_stopped() {
			println!("Touch ended");
			let color = [1.0, 0.0, 0.0, 1.0];
			self.model
				.set_material_parameter(0, "color", MaterialParameter::Color(color))
				.unwrap();
			self.model
				.set_material_parameter(
					0,
					"emission_factor",
					MaterialParameter::Color(color.map(|c| c * 0.5)),
				)
				.unwrap();
		}
	}
}

fn position(data: &InputData) -> Vec3 {
    match &data.input {
        InputDataType::Hand(h) => h.palm.position.into(),
        InputDataType::Pointer(w) => w.deepest_point.into(),
        InputDataType::Tip(t) => t.origin.into(),
    }

    
}
