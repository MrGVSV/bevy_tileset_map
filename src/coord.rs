use bevy::math::{IVec2, UVec2};
use bevy_ecs_tilemap::TilePos;
use bevy_tileset::tileset::coords::TileCoords;

/// The coordinates of a tile, including the `map_id` and `layer_id`
#[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq)]
#[cfg_attr(
	feature = "serialization",
	derive(serde::Serialize, serde::Deserialize)
)]
pub struct TileCoord {
	#[cfg_attr(feature = "serialization", serde(with = "TilePosRef"))]
	pub pos: TilePos,
	pub map_id: u16,
	pub layer_id: u16,
}

impl TileCoords for TileCoord {
	fn pos(&self) -> IVec2 {
		let pos: UVec2 = self.pos.into();
		pos.as_ivec2()
	}
}

#[cfg(feature = "serialization")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(remote = "TilePos")]
pub(crate) struct TilePosRef(pub u32, pub u32);
