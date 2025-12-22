// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use fontique::Collection;

use fontique::CollectionOptions;
use fontique::SourceCache;
use fontique::SourceCacheOptions;

/// A font database/cache (wrapper around a Fontique [`Collection`] and [`SourceCache`]).
///
/// This type is designed to be a global resource with only one per-application (or per-thread).
#[derive(Default, Clone)]
pub struct FontContext {
    pub collection: Collection,
    pub source_cache: SourceCache,
}

impl FontContext {
    /// Create a new `FontContext`, discovering system fonts if available.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new **shared** `FontContext`, discovering system fonts if available.
    pub fn shared() -> Self {
        Self {
            collection: Collection::new(CollectionOptions {
                shared: true,
                system_fonts: true,
            }),
            source_cache: SourceCache::new(SourceCacheOptions { shared: true }),
        }
    }

    pub fn sync_shared(&mut self) {
        self.collection.sync_shared()
    }

    pub fn make_shared(&mut self) {
        self.collection.make_shared();
        self.source_cache.make_shared();
    }
}
