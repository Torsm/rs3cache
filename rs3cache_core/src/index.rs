//! The interface between [rs3cache](crate) and the cache database.

#![allow(unused_imports)] // varies based on mock config flags

/// This contains the game-specific implementations.
#[cfg_attr(feature = "rs3", path = "index/rs3.rs")]
#[cfg_attr(feature = "osrs", path = "index/osrs.rs")]
#[cfg_attr(feature = "legacy", path = "index/legacy.rs")]
mod index_impl;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    env::{self, VarError},
    fs::{self, File},
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    marker::PhantomData,
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use bytes::{Buf, Bytes};
use fstrings::{f, format_args_f};
pub use index_impl::*;
use itertools::iproduct;
use path_macro::path;

#[cfg(feature = "osrs")]
use crate::xtea::Xtea;
use crate::{
    arc::Archive,
    buf::BufExtra,
    decoder,
    error::{CacheError, CacheResult},
    indextype::IndexType,
    meta::{IndexMetadata, Metadata},
};

mod states {
    use std::ops::RangeInclusive;

    /// Initial state of [`CacheIndex`](super::CacheIndex).
    pub struct Initial {}

    pub struct Truncated {
        pub feed: Vec<u32>,
    }

    /// Trait that describes the current index state. Cannot be implemented.
    pub trait IndexState {}
    impl IndexState for Initial {}
    impl IndexState for Truncated {}
}

pub use states::{IndexState, Initial, Truncated};

/// Container of [`Archive`]s.
pub struct CacheIndex<S: IndexState> {
    index_id: u32,
    metadatas: IndexMetadata,
    state: S,
    path: PathBuf,

    #[cfg(feature = "rs3")]
    connection: sqlite::Connection,

    #[cfg(any(feature = "osrs", feature = "legacy"))]
    file: Box<[u8]>,

    #[cfg(feature = "osrs")]
    xteas: Option<HashMap<u32, Xtea>>,
}

// methods valid in any state
impl<S> CacheIndex<S>
where
    S: IndexState,
{
    /// The [index id](crate::indextype::IndexType) of `self`,
    /// corresponding to the `raw/js5-{index_id}.jcache` file being held.
    #[inline(always)]
    pub fn index_id(&self) -> u32 {
        self.index_id
    }

    /// Returns a view over the [`IndexMetadata`] of `self`.
    #[inline(always)]
    pub fn metadatas(&self) -> &IndexMetadata {
        &(self.metadatas)
    }

    /// Get an [`Archive`] from `self`.
    ///
    /// # Errors
    ///
    /// Raises [`ArchiveNotFoundError`](CacheError::ArchiveNotFoundError) if `archive_id` is not in `self`.
    pub fn archive(&self, archive_id: u32) -> CacheResult<Archive> {
        let metadata = self
            .metadatas()
            .get(&archive_id)
            .ok_or_else(|| CacheError::ArchiveNotFoundError(self.index_id(), archive_id))?;
        let data = self.get_file(metadata)?;

        Ok(Archive::deserialize(metadata, data))
    }
}

impl CacheIndex<Initial> {
    /// Retain only those archives that are in `ids`.
    /// Advances `self` to the `Truncated` state.
    ///
    /// # Panics
    ///
    /// Panics if any of `ids` is not in `self`.
    pub fn retain(self, ids: Vec<u32>) -> CacheIndex<Truncated> {
        let all_ids = self.metadatas().keys().copied().collect::<BTreeSet<_>>();

        if let Some(missing_id) = ids.iter().find(|id| !all_ids.contains(id)) {
            panic!("Attempted to retain missing archive id {},", missing_id)
        }
        let Self {
            path,
            #[cfg(feature = "rs3")]
            connection,
            #[cfg(any(feature = "osrs", feature = "legacy"))]
            file,
            index_id,
            metadatas,
            #[cfg(feature = "osrs")]
            xteas,
            ..
        } = self;

        CacheIndex {
            path,
            #[cfg(feature = "rs3")]
            connection,
            #[cfg(any(feature = "osrs", feature = "legacy"))]
            file,
            index_id,
            metadatas,
            #[cfg(feature = "osrs")]
            xteas,
            state: Truncated { feed: ids },
        }
    }
}

impl IntoIterator for CacheIndex<Initial> {
    type Item = CacheResult<Archive>;

    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        let feed = self.metadatas().keys().copied().collect::<Vec<u32>>().into_iter();

        IntoIter { index: self, feed }
    }
}

impl IntoIterator for CacheIndex<Truncated> {
    type Item = CacheResult<Archive>;

    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        let Self {
            path,
            #[cfg(feature = "rs3")]
            connection,
            #[cfg(any(feature = "osrs", feature = "legacy"))]
            file,
            index_id,
            metadatas,
            #[cfg(feature = "osrs")]
            xteas,
            state,
        } = self;

        let index = CacheIndex {
            path,
            #[cfg(feature = "rs3")]
            connection,
            #[cfg(any(feature = "osrs", feature = "legacy"))]
            file,
            index_id,
            metadatas,
            #[cfg(feature = "osrs")]
            xteas,
            state: Initial {},
        };

        IntoIter {
            index,
            feed: state.feed.into_iter(),
        }
    }
}

/// Iterator over all [`Archive`]s of `self`. Yields in arbitrary order.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct IntoIter {
    pub(crate) index: CacheIndex<Initial>,
    feed: std::vec::IntoIter<u32>,
}

impl IntoIter {
    /// Returns a view of the [`IndexMetadata`] of the contained [`CacheIndex`].
    pub fn metadatas(&self) -> &IndexMetadata {
        self.index.metadatas()
    }
}

impl Iterator for IntoIter {
    type Item = CacheResult<Archive>;

    fn next(&mut self) -> Option<Self::Item> {
        self.feed.next().map(|archive_id| self.index.archive(archive_id))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.feed.size_hint()
    }
}