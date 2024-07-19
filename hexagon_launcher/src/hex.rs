use std::ops::Add;

use crate::{APP_SIZE, PADDING};
use tween::TweenTime;

#[derive(Clone, Copy, Debug, Default)]
pub struct Hex {
	q: isize,
	r: isize,
	s: isize,
}

pub const HEX_CENTER: Hex = Hex { q: 0, r: 0, s: 0 };
pub const HEX_DIRECTION_VECTORS: [Hex; 6] = [
	Hex { q: 1, r: 0, s: -1 },
	Hex { q: 1, r: -1, s: 0 },
	Hex { q: 0, r: -1, s: 1 },
	Hex { q: -1, r: 0, s: 1 },
	Hex { q: -1, r: 1, s: 0 },
	Hex { q: 0, r: 1, s: -1 },
];

impl Hex {
	pub fn new(q: isize, r: isize, s: isize) -> Self {
		Hex { q, r, s }
	}

	pub fn get_coords(&self) -> [f32; 3] {
		let x = 3.0 / 2.0 * (APP_SIZE + PADDING) / 2.0 * (-self.q - self.s).to_f32();
		let y = 3.0_f32.sqrt() * (APP_SIZE + PADDING) / 2.0
			* ((-self.q - self.s).to_f32() / 2.0 + self.s.to_f32());
		[x, y, 0.0]
	}

	pub fn neighbor(self, direction: usize) -> Self {
		self + HEX_DIRECTION_VECTORS[direction]
	}

	pub fn scale(self, factor: isize) -> Self {
		Hex::new(self.q * factor, self.r * factor, self.s * factor)
	}
}
impl Add for Hex {
	type Output = Hex;

	fn add(self, rhs: Self) -> Self::Output {
		Hex::new(self.q + rhs.q, self.r + rhs.r, self.s + rhs.s)
	}
}
