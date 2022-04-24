//! # bevy_tileset_map
//!
//! > An extension to bevy_ecs_tilemap, allowing for configurable tilesets, auto tiling, and more
//! > using the bevy_tileset crate
//!
//! This crate integrates both [bevy_ecs_tilemap](https://github.com/StarArawn/bevy_ecs_tilemap) and
//! [bevy_tileset](https://github.com/MrGVSV/bevy_tileset) in order to provide an easy-to-use API
//! that utilizes the power of bevy_ecs_tilemap with the configurability of bevy_tileset.
//!
//! ## Usage
//!
//! Add the plugins to your Bevy app:
//!
//! ```
//! use bevy::prelude::*;
//! use bevy_tileset_map::prelude::{TilesetPlugin, TilesetMapPlugin};
//! use bevy_ecs_tilemap::prelude::TilemapPlugin;
//!
//! fn main() {
//!   App::new()
//!     // ...
//!     // bevy_ecs_tilemap
//!     .add_plugin(TilemapPlugin)
//!     // bevy_tileset
//!     .add_plugin(TilesetPlugin::default())
//!     // bevy_tileset_map
//!     .add_plugin(TilesetMapPlugin)
//!     // ...
//!     .run();
//! }
//! ```
//!
//! And add a system to place tiles:
//!
//! ```
//! use bevy_tileset_map::prelude::Tileplacer;
//! fn place_tile(mut placer: Tileplacer, /* ... */) {
//! #   let tile_id = bevy_tileset_map::prelude::TileId::new(0, 0);
//! #   let tile_pos = bevy_ecs_tilemap::TilePos(0, 0);
//! #   let map_id = 0u16;
//! #   let layer_id = 0u16;
//!   // ...
//!   placer.place(
//!     tile_id,
//!     tile_pos,
//!     map_id,
//!     layer_id
//!   );
//! }
//! ```
//!
//! ## Crate Features
//!
//! * __`default`__ - No features automatically enabled
//! * __`variants`__ - Enables usage of Variant tiles
//! * __`auto-tile`__ - Enables usage of Auto tiles
//! * __`serialization`__ - Enables tilemap serialization
//!

pub use bevy_tileset as tileset;

#[cfg(feature = "auto-tile")]
pub(crate) mod auto;
mod coord;
mod placement;
mod plugin;
#[cfg(feature = "serialization")]
mod serialization;

pub mod prelude {
	pub use bevy_tileset::prelude::*;

	#[cfg(feature = "auto-tile")]
	pub use super::auto::RemoveAutoTileEvent;
	pub use super::coord::TileCoord;
	pub use super::placement::*;
	pub use super::plugin::{TilesetMapLabel, TilesetMapPlugin, TilesetMapStage};
	#[cfg(feature = "serialization")]
	pub use super::serialization::*;
}
