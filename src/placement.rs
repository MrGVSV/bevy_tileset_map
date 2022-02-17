//! Tools for placing and removing tiles

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use bevy_tileset::prelude::*;
use thiserror::Error;

/// Errors related to the placement of tiles
#[derive(Error, Debug)]
pub enum TilePlacementError {
	/// A tile already exists at the given coordinate
	///
	/// The meaning of this changes a bit depending on the context. For example, it could
	/// mean that an unexpected tile was found or that _any_ tile was found.
	#[error("Attempted to place tile {new:?} but found existing tile {existing:?} (@ {pos:?})")]
	TileAlreadyExists {
		/// The ID of the new tile to be placed
		new: TileId,
		/// The ID of the existing tile
		existing: Option<TileId>,
		/// The desired/occupied tile coordinate
		pos: TilePos,
	},
	/// The tileset does not exist or is invalid
	///
	/// Contains the ID of the tileset in question
	#[error("Invalid tileset {0:?}")]
	InvalidTileset(TilesetId),
	/// The tile does not exist or is invalid
	///
	/// This can happen when a given [`TileId`] doesn't exist within a tileset
	///
	/// Contains the ID of the tile in question
	#[error("Invalid tile {0:?}")]
	InvalidTile(TileId),
	/// A catch-all for errors generated by `bevy_ecs_tilemap`
	///
	/// Contains the generated error
	#[error("Tilemap error: {0:?}")]
	MapError(MapTileError),
}

/// An enum denoting how a tile was placed or removed
///
/// This allows you to respond to the results the placement, such as handling cleanup
/// or performing a secondary action.
///
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PlacedTile {
	/// A tile was added
	Added {
		/// The replaced tile
		old_tile: Option<(Entity, Option<TileId>)>,
		/// The placed tile
		new_tile: (Entity, TileId),
	},
	/// A tile was removed
	Removed {
		/// The removed tile
		old_tile: Option<(Entity, Option<TileId>)>,
	},
}

/// A result type alias for tile placement
pub type TilePlacementResult = Result<PlacedTile, TilePlacementError>;

/// A helper system param used to place tiles
///
/// All methods automatically account for the tile's [`TileType`] and respects Auto Tiles,
/// allowing for a much simpler user experience.
///
/// Additionally, tilesets are automatically derived from the given [`TileId`]s. This works for
/// any [`Tileset`] registered in `Assets<Tileset>`.
///
/// # Examples
///
/// ```
/// # use bevy::prelude::Res;
/// # use bevy_ecs_tilemap::TilePos;
/// # use bevy_tileset_map::prelude::{TileId, TilePlacer};
/// struct CurrentTile(TileId);
///
/// fn tile_placement_system(mut placer: TilePlacer, tile: Res<CurrentTile>) {
///   placer.place(
///     tile.0,
///     TilePos(0, 0),
///     0u16,
///     0u16,
///   ).unwrap();
/// }
/// ```
#[derive(SystemParam)]
pub struct TilePlacer<'w, 's> {
	map_query: MapQuery<'w, 's>,
	tilesets: Tilesets<'w, 's>,
	commands: Commands<'w, 's>,
	/// Query used to get info about a tile
	#[cfg(not(feature = "auto-tile"))]
	#[allow(dead_code)]
	query: Query<'w, 's, (&'static Tile, Option<&'static GPUAnimated>)>,
	/// Query used to get info about a tile
	#[cfg(feature = "auto-tile")]
	#[allow(dead_code)]
	query: Query<
		'w,
		's,
		(
			&'static Tile,
			Option<&'static GPUAnimated>,
			Option<&'static bevy_tileset::auto::AutoTileId>,
		),
	>,
	/// Query used to get and send data for the [`RemoveAutoTileEvent`] event
	#[cfg(feature = "auto-tile")]
	#[allow(dead_code)]
	auto_query: Query<
		'w,
		's,
		(
			&'static TilePos,
			&'static TileParent,
			&'static bevy_tileset::auto::AutoTileId,
		),
		With<Tile>,
	>,
	#[cfg(feature = "auto-tile")]
	#[allow(dead_code)]
	event_writer: EventWriter<'w, 's, crate::auto::RemoveAutoTileEvent>,
}

impl<'w, 's> TilePlacer<'w, 's> {
	/// Place a tile
	///
	/// This will remove and overwrite any tile beneath it, whether it matches this one or not.
	///
	/// # Arguments
	///
	/// * `tile_id`: The full ID of the tile to place
	/// * `pos`: The tile position
	/// * `map_id`: The tile map
	/// * `layer_id`: The layer within the tile map
	///
	pub fn place<Id: Into<TileId>, Pos: Into<TilePos> + Clone, MId: MapId>(
		&mut self,
		tile_id: Id,
		pos: Pos,
		map_id: MId,
		layer_id: u16,
	) -> TilePlacementResult {
		self.place_unchecked(tile_id, pos, map_id, layer_id)
	}

	/// Place a tile only if the coordinate is not already occupied
	///
	/// # Arguments
	///
	/// * `tile_id`: The full ID of the tile to place
	/// * `pos`: The tile position
	/// * `map_id`: The tile map
	/// * `layer_id`: The layer within the tile map
	///
	/// # Errors
	///
	/// While this method can return any kind of error from [`TilePlacementError`], the one
	/// to look out for is [`TilePlacementError::TileAlreadyExists`] as it denotes that the
	/// given coordinate is already occupied and that the tile could not be placed.
	///
	/// You can use this to respond appropriately on a failed placement.
	///
	pub fn try_place<Id: Into<TileId>, Pos: Into<TilePos> + Clone, MId: MapId>(
		&mut self,
		tile_id: Id,
		pos: Pos,
		map_id: MId,
		layer_id: u16,
	) -> TilePlacementResult {
		let id = tile_id.into();
		let pos = pos.into();

		if let Some(existing) = self.get_existing(id, pos, map_id, layer_id) {
			// Tile already exists -> don't place
			return Err(TilePlacementError::TileAlreadyExists {
				new: id,
				existing: existing.id,
				pos,
			});
		}

		self.place_unchecked(id, pos, map_id, layer_id)
	}

	/// Places a tile if the coordinate is not already occupied or if the existing tile does not
	/// match this one
	///
	/// # Arguments
	///
	/// * `tile_id`: The full ID of the tile to place
	/// * `pos`: The tile position
	/// * `map_id`: The tile map
	/// * `layer_id`: The layer within the tile map
	///
	/// # Errors
	///
	/// While this method can return any kind of error from [`TilePlacementError`], the one
	/// to look out for is [`TilePlacementError::TileAlreadyExists`] as it denotes that the
	/// given coordinate is already occupied by a matching tile and could not be replaced.
	///
	pub fn replace<Id: Into<TileId>, Pos: Into<TilePos> + Clone, MId: MapId>(
		&mut self,
		tile_id: Id,
		pos: Pos,
		map_id: MId,
		layer_id: u16,
	) -> TilePlacementResult {
		let id = tile_id.into();
		let pos = pos.into();

		if let Some(existing) = self.get_existing(id, pos, map_id, layer_id) {
			// Check that the existing tile is of a different type
			if let Some(existing_id) = existing.id {
				if existing_id.eq_tile_group(&id) {
					return Err(TilePlacementError::TileAlreadyExists {
						new: id,
						existing: existing.id,
						pos,
					});
				}
			}
		}

		self.place_unchecked(id, pos, map_id, layer_id)
	}

	/// Places a tile if the coordinate is not already occupied, otherwise, if the existing tile matches
	/// this one, remove it
	///
	/// # Arguments
	///
	/// * `tile_id`: The full ID of the tile to place
	/// * `pos`: The tile position
	/// * `map_id`: The tile map
	/// * `layer_id`: The layer within the tile map
	///
	/// # Errors
	///
	/// While this method can return any kind of error from [`TilePlacementError`], the one
	/// to look out for is [`TilePlacementError::TileAlreadyExists`] as it denotes that the
	/// given coordinate is already occupied by a _non_-matching tile and could not be placed.
	///
	pub fn toggle_matching<Id: Into<TileId>, Pos: Into<TilePos> + Clone, MId: MapId>(
		&mut self,
		tile_id: Id,
		pos: Pos,
		map_id: MId,
		layer_id: u16,
	) -> TilePlacementResult {
		let id = tile_id.into();
		let pos = pos.into();

		if let Some(existing) = self.get_existing(id, pos, map_id, layer_id) {
			// Remove the existing tile if it matches
			if let Some(existing_id) = existing.id {
				if existing_id.eq_tile_group(&id) {
					self.remove(pos, map_id, layer_id)?;
					return Ok(PlacedTile::Removed {
						old_tile: Some((existing.entity, Some(existing_id))),
					});
				}
			}

			// Tile exists but did not match -> don't remove
			return Err(TilePlacementError::TileAlreadyExists {
				new: id,
				existing: existing.id,
				pos,
			});
		}

		self.place_unchecked(id, pos, map_id, layer_id)
	}

	/// Places a tile if the coordinate is not already occupied, otherwise removes the existing tile
	///
	/// # Arguments
	///
	/// * `tile_id`: The full ID of the tile to place
	/// * `pos`: The tile position
	/// * `map_id`: The tile map
	/// * `layer_id`: The layer within the tile map
	///
	pub fn toggle<Id: Into<TileId>, Pos: Into<TilePos> + Clone, MId: MapId>(
		&mut self,
		tile_id: Id,
		pos: Pos,
		map_id: MId,
		layer_id: u16,
	) -> TilePlacementResult {
		let id = tile_id.into();
		let pos = pos.into();

		if let Some(existing) = self.get_existing(id, pos, map_id, layer_id) {
			self.remove(pos, map_id, layer_id)?;
			return Ok(PlacedTile::Removed {
				old_tile: Some((existing.entity, existing.id)),
			});
		}

		self.place_unchecked(id, pos, map_id, layer_id)
	}

	/// Removes the tile at the given coordinate
	///
	/// This method is preferred over handling the removal manually as it will also account for
	/// any Auto Tiles and handle their removal accordingly.
	///
	/// # Arguments
	///
	/// * `pos`: The tile position
	/// * `map_id`: The tile map
	/// * `layer_id`: The layer within the tile map
	///
	pub fn remove<Pos: Into<TilePos>, MId: MapId>(
		&mut self,
		pos: Pos,
		map_id: MId,
		layer_id: u16,
	) -> Result<(), TilePlacementError> {
		let pos = pos.into();

		#[cfg(feature = "auto-tile")]
		{
			// Get the current tile entity
			let entity = self
				.map_query
				.get_tile_entity(pos, map_id, layer_id)
				.map_err(|err| TilePlacementError::MapError(err))?;

			// Attempt to remove the auto tile
			self.try_remove_auto_tile(entity);
		}

		// Despawn the tile and notify the chunk
		self.map_query
			.despawn_tile(&mut self.commands, pos, map_id, layer_id)
			.map_err(|err| TilePlacementError::MapError(err))?;
		self.map_query.notify_chunk_for_tile(pos, map_id, layer_id);
		Ok(())
	}

	/// Adds a tile to the given `LayerBuilder`
	///
	/// This is used to initialize the tilemap layer _before_ it becomes accessible via queries
	///
	/// # Arguments
	///
	/// * `tile_id`: The full ID of the tile to place
	/// * `pos`: The tile position
	/// * `layer_builder`: The layer builder
	///
	pub fn add_to_layer<TId: Into<TileId>, Pos: Into<TilePos>>(
		&mut self,
		tile_id: TId,
		pos: Pos,
		layer_builder: &mut LayerBuilder<TileBundle>,
	) -> TilePlacementResult {
		let id = tile_id.into();
		let pos = pos.into();
		let tileset_id = self.get_tileset_id(&id)?;
		let tile_index = self.get_tile_index(&id)?;

		let entity = match tile_index {
			TileIndex::Standard(index) => {
				layer_builder
					.set_tile(
						pos,
						Tile {
							texture_index: index as u16,
							..Default::default()
						}
						.into(),
					)
					.map_err(|err| TilePlacementError::MapError(err))?;
				layer_builder
					.get_tile_entity(&mut self.commands, pos)
					.map_err(|err| TilePlacementError::MapError(err))?
			},
			TileIndex::Animated(start, end, speed) => {
				layer_builder
					.set_tile(
						pos,
						Tile {
							texture_index: start as u16,
							..Default::default()
						}
						.into(),
					)
					.map_err(|err| TilePlacementError::MapError(err))?;
				let entity = layer_builder
					.get_tile_entity(&mut self.commands, pos)
					.map_err(|err| TilePlacementError::MapError(err))?;
				self.commands.entity(entity).insert(GPUAnimated::new(
					start as u32,
					end as u32,
					speed,
				));
				entity
			},
		};

		self.commands
			.entity(entity)
			.insert(TilesetParent(tileset_id));

		#[cfg(feature = "auto-tile")]
		self.apply_auto_tile(&id, &tileset_id, entity);

		Ok(PlacedTile::Added {
			old_tile: None,
			new_tile: (entity, id),
		})
	}

	/// Updates an entity to match the given tile
	///
	/// This is useful for when you need to update a specific entity rather than replacing it.
	/// If you don't care or need to maintain the same entity, you're better off using the
	/// [`place`](Self::place) method.
	///
	/// # Arguments
	///
	/// * `tile_id`: The full ID of the tile to place
	/// * `entity`: The tile entity
	///
	pub fn update<TId: Into<TileId>>(
		&mut self,
		tile_id: TId,
		entity: Entity,
	) -> Result<(), TilePlacementError> {
		let id = tile_id.into();
		let tileset_id = self.get_tileset_id(&id)?;
		let tile_index = self.get_tile_index(&id)?;

		match tile_index {
			TileIndex::Standard(index) => {
				self.commands
					.entity(entity)
					.insert(Tile {
						texture_index: index as u16,
						..Default::default()
					})
					.remove::<GPUAnimated>();
			},
			TileIndex::Animated(start, end, speed) => {
				self.commands
					.entity(entity)
					.insert(Tile {
						texture_index: start as u16,
						..Default::default()
					})
					.insert(GPUAnimated::new(start as u32, end as u32, speed));
			},
		}

		self.commands
			.entity(entity)
			.insert(TilesetParent(tileset_id));

		#[cfg(feature = "auto-tile")]
		self.apply_auto_tile(&id, &tileset_id, entity);

		Ok(())
	}

	/// The main placement method
	///
	/// Handles the actual placement of a tile, without checking if it should or shouldn't
	/// be placed.
	fn place_unchecked<Id: Into<TileId>, Pos: Into<TilePos>, MId: MapId>(
		&mut self,
		tile_id: Id,
		pos: Pos,
		map_id: MId,
		layer_id: u16,
	) -> TilePlacementResult {
		let id = tile_id.into();
		let pos = pos.into();
		let tileset_id = self.get_tileset_id(&id)?;
		let tile_index = self.get_tile_index(&id)?;

		let old_tile = if let Some(existing) = self.get_existing(id, pos, map_id, layer_id) {
			// Remove existing
			self.remove(pos, map_id, layer_id)?;
			Some((existing.entity, existing.id))
		} else {
			None
		};

		let index = *tile_index.base_index();

		// Set the tile
		let entity = self
			.map_query
			.set_tile(
				&mut self.commands,
				pos,
				Tile {
					texture_index: index as u16,
					..Default::default()
				},
				map_id,
				layer_id,
			)
			.map_err(|err| TilePlacementError::MapError(err))?;

		// Handle index specifics
		match tile_index {
			TileIndex::Standard(..) => {
				// Remove any `GPUAnimated` component
				self.commands.entity(entity).remove::<GPUAnimated>().id()
			},
			TileIndex::Animated(start, end, speed) => {
				// Add the `GPUAnimated` component
				self.commands
					.entity(entity)
					.insert(GPUAnimated::new(start as u32, end as u32, speed))
					.id()
			},
		};

		// Insert the reference to the tileset this tile belongs to
		self.commands
			.entity(entity)
			.insert(TilesetParent(tileset_id));

		#[cfg(feature = "auto-tile")]
		self.apply_auto_tile(&id, &tileset_id, entity);

		self.map_query.notify_chunk_for_tile(pos, map_id, layer_id);

		Ok(PlacedTile::Added {
			old_tile,
			new_tile: (entity, id),
		})
	}

	/// Attempts to add/remove an Auto Tile for the given tile
	#[cfg(feature = "auto-tile")]
	fn apply_auto_tile(&mut self, id: &TileId, tileset_id: &TilesetId, entity: Entity) {
		let id = id.into();
		let is_auto = self
			.get_tile_data(id)
			.ok()
			.and_then(|data| Some(data.is_auto()))
			.unwrap_or_default();

		let mut cmds = self.commands.entity(entity);
		if is_auto {
			cmds.insert(bevy_tileset::auto::AutoTileId {
				group_id: id.group_id,
				tileset_id: *tileset_id,
			});
		} else {
			cmds.remove::<bevy_tileset::auto::AutoTileId>();
			self.try_remove_auto_tile(entity);
		}
	}

	/// Attempts to handle the removal of an Auto Tile
	///
	/// Returns true if the tile was successfully removed
	#[cfg(feature = "auto-tile")]
	fn try_remove_auto_tile(&mut self, entity: Entity) -> bool {
		// Create the remove event
		let event = if let Ok((pos, parent, auto)) = self.auto_query.get(entity) {
			Some(crate::auto::RemoveAutoTileEvent {
				entity,
				pos: *pos,
				parent: *parent,
				auto_id: *auto,
			})
		} else {
			None
		};

		// Send the remove event (separated due to mutability rules)
		if let Some(event) = event {
			self.event_writer.send(event);
			true
		} else {
			false
		}
	}

	/// Tries to get the existing tile for a given tile coordinate
	fn get_existing<Pos: Into<TilePos>, MId: MapId>(
		&mut self,
		tile_id: TileId,
		pos: Pos,
		map_id: MId,
		layer_id: u16,
	) -> Option<ExistingTile> {
		let mut tile =
			if let Ok(entity) = self.map_query.get_tile_entity(pos.into(), map_id, layer_id) {
				if let Ok(results) = self.query.get(entity) {
					let tex_idx = results.0.texture_index as usize;
					let mut existing = ExistingTile::new(entity, None, tex_idx);
					existing.is_animated = results.1.is_some();
					#[cfg(feature = "auto-tile")]
					{
						existing.is_auto = results.2.is_some();
					}
					Some(existing)
				} else {
					None
				}
			} else {
				None
			};

		if let Some(ref mut tile) = tile {
			if let Some(tileset) = self.get_tileset(&tile_id).ok() {
				let tex_idx = tile.texture_index;
				tile.id = tileset.get_tile_id(&tex_idx).cloned();
			}
		}

		tile
	}

	/// Get the tileset belonging to the given `TileId`
	fn get_tileset(&self, tile_id: &TileId) -> Result<&Tileset, TilePlacementError> {
		let tileset = self
			.tilesets
			.get_by_id(&tile_id.tileset_id)
			.ok_or_else(|| TilePlacementError::InvalidTileset(tile_id.tileset_id))?;
		Ok(tileset)
	}

	/// Get the ID of the tileset belonging to the given `TileId`
	fn get_tileset_id(&self, tile_id: &TileId) -> Result<TilesetId, TilePlacementError> {
		let tileset = self.get_tileset(tile_id)?;
		Ok(tileset.id().clone())
	}

	/// Get the `TileIndex` matching the given `TileId`
	fn get_tile_index(&self, tile_id: &TileId) -> Result<TileIndex, TilePlacementError> {
		let tileset = self.get_tileset(&tile_id)?;
		let (tile_index, _) = tileset
			.select_tile_by_id(tile_id)
			.ok_or_else(|| TilePlacementError::InvalidTile(*tile_id))?;
		Ok(tile_index)
	}

	/// Get the `TileData` matching the given `TileId`
	fn get_tile_data(&self, tile_id: &TileId) -> Result<&TileData, TilePlacementError> {
		let tileset = self.get_tileset(&tile_id)?;
		let (_, tile_data) = tileset
			.select_tile_by_id(tile_id)
			.ok_or_else(|| TilePlacementError::InvalidTile(*tile_id))?;
		Ok(tile_data)
	}
}

/// Helper struct for passing information regarding existing tiles
struct ExistingTile {
	entity: Entity,
	id: Option<TileId>,
	texture_index: usize,
	is_animated: bool,
	#[cfg(feature = "auto-tile")]
	is_auto: bool,
}

impl ExistingTile {
	fn new(entity: Entity, id: Option<TileId>, texture_index: usize) -> Self {
		Self {
			entity,
			id,
			texture_index,
			is_animated: false,
			#[cfg(feature = "auto-tile")]
			is_auto: false,
		}
	}
}
