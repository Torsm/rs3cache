use core::ops::{Range, RangeInclusive};
use std::{
    collections::{hash_map, BTreeMap, HashMap},
    iter::Zip,
};

use itertools::{iproduct, Product};
use ndarray::{iter::LanesIter, s, Axis, Dim};

use crate::{
    cache::{
        arc::Archive,
        index::{self, CacheIndex},
        indextype::IndexType,
    },
    definitions::{
        locations::Location,
        mapsquares::{GroupMapSquare, MapSquare},
        tiles::{Tile, TileArray},
    },
    utils::{
        error::{CacheError, CacheResult},
        rangeclamp::RangeClamp,
    },
};

/// Iterates over all [`MapSquare`]s in arbitrary order.

pub struct MapSquareIterator {
    inner: index::IntoIter,
}

impl MapSquareIterator {
    /// Constructor for MapSquareIterator.
    pub fn new() -> CacheResult<MapSquareIterator> {
        let inner = CacheIndex::new(IndexType::MAPSV2)?.into_iter();
        Ok(MapSquareIterator { inner })
    }
}

impl Iterator for MapSquareIterator {
    type Item = MapSquare;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(MapSquare::from_archive)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

/// Iterates over [`GroupMapSquare`] in arbitrary order.

pub struct GroupMapSquareIterator {
    inner: index::IntoIterGrouped,
}

impl GroupMapSquareIterator {
    /// Constructor for [`GroupMapSquareIterator`].
    pub fn new(dx: RangeInclusive<i32>, dy: RangeInclusive<i32>) -> CacheResult<GroupMapSquareIterator> {
        let inner = CacheIndex::new(IndexType::MAPSV2)?.grouped(dx, dy).into_iter();
        Ok(GroupMapSquareIterator { inner })
    }

    /// Constructor for [`GroupMapSquareIterator`], but limited to the [`MapSquare`]s in `coordinates`.
    pub fn new_only(dx: RangeInclusive<i32>, dy: RangeInclusive<i32>, coordinates: Vec<(u8, u8)>) -> CacheResult<GroupMapSquareIterator> {
        let archive_ids = coordinates.into_iter().map(|(i, j)| (i as u32) | (j as u32) << 7).collect::<Vec<u32>>();
        let inner = CacheIndex::new(IndexType::MAPSV2)?.retain(archive_ids).grouped(dx, dy).into_iter();
        Ok(GroupMapSquareIterator { inner })
    }
}

impl Iterator for GroupMapSquareIterator {
    type Item = GroupMapSquare;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|group| {
            let (core_i, core_j) = ((group.core_id() & 0x7F) as u8, (group.core_id() >> 7) as u8);
            let mapsquares = group
                .into_iter()
                .map(MapSquare::from_archive)
                .map(|sq| ((sq.i, sq.j), sq))
                .collect::<HashMap<_, _>>();
            GroupMapSquare { core_i, core_j, mapsquares }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for GroupMapSquareIterator {}