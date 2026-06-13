//! Dynamic good registry — goods as data, not constants.
//!
//! G0b migration (game-spec §11): the lab hard-codes its goods as `GoodId`
//! constants (`good.rs`). The registry interns goods by name into `GoodId`
//! values so a future `content/` layer (G3) can define them, while
//! [`GoodRegistry::lab_default`] reproduces the lab's exact set in the exact id
//! order — so `GoodId` values, `Stock`/belief slot counts, and names are
//! bit-for-bit today's. The legacy constants and [`crate::good::good_name`]
//! survive as lab-compat surface (see `docs/engine-divergence.md`).
//!
//! Pure std, no `HashMap`: interning is a linear scan over a `Vec<String>`
//! whose index is the `GoodId`. Catalogs are tiny (the lab has seven goods).

use crate::good::{GoodId, LAB_GOOD_NAMES};

/// A name → `GoodId` catalog. The vector index *is* the `GoodId.0`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GoodRegistry {
    names: Vec<String>,
}

impl GoodRegistry {
    /// An empty registry. The first interned good gets `GoodId(0)`.
    pub fn new() -> Self {
        Self { names: Vec::new() }
    }

    /// The exact lab good set in the exact id order
    /// (`gold, food, wood, net, salt, cloth, ore` → `GoodId(0..=6)`).
    ///
    /// Interned from [`LAB_GOOD_NAMES`], the same table `good_name` shims over,
    /// so the registry, the constants, and `good_name` agree by construction.
    pub fn lab_default() -> Self {
        let mut registry = Self::new();
        for name in LAB_GOOD_NAMES {
            registry.intern(name);
        }
        registry
    }

    /// Convert a catalog position into its `GoodId`. Every position comes from
    /// `names`, whose length [`GoodRegistry::intern`] bounds to `u16::MAX` at
    /// insert, so this conversion never truncates — the `expect` documents that
    /// invariant explicitly instead of a silent `as`-cast.
    fn good_id_at(index: usize) -> GoodId {
        GoodId(u16::try_from(index).expect("GoodRegistry catalog index exceeds GoodId width"))
    }

    /// Intern a good by name. An already-known name returns its existing id;
    /// a new name extends the catalog with the next id.
    pub fn intern(&mut self, name: &str) -> GoodId {
        if let Some(index) = self.names.iter().position(|existing| existing == name) {
            return Self::good_id_at(index);
        }
        // Hard-error (release builds too) rather than silently truncate into a
        // colliding `GoodId`: `intern` is the G3 `content/` layer's entry point.
        let index = u16::try_from(self.names.len())
            .expect("GoodRegistry exceeded GoodId's u16 catalog width");
        let id = GoodId(index);
        self.names.push(name.to_string());
        id
    }

    /// The id of an already-interned name, or `None` if it is unknown.
    pub fn id_of(&self, name: &str) -> Option<GoodId> {
        self.names
            .iter()
            .position(|existing| existing == name)
            .map(Self::good_id_at)
    }

    /// The name of a good. Unknown ids resolve to `"unknown"`, matching the
    /// legacy [`crate::good::good_name`] shim.
    pub fn name(&self, good: GoodId) -> &str {
        self.names
            .get(usize::from(good.0))
            .map(String::as_str)
            .unwrap_or("unknown")
    }

    /// The number of interned goods — the `Stock`/belief slot-count source.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether the catalog is empty.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::GoodRegistry;
    use crate::good::{good_name, Stock, CLOTH, FOOD, GOLD, NET, ORE, SALT, WOOD};

    #[test]
    fn lab_default_matches_legacy_constants_one_by_one() {
        let registry = GoodRegistry::lab_default();

        // Exact id order: every constant resolves to its name through the
        // registry, and the name matches the legacy `good_name` shim.
        for good in [GOLD, FOOD, WOOD, NET, SALT, CLOTH, ORE] {
            assert_eq!(registry.name(good), good_name(good));
            assert_eq!(registry.id_of(good_name(good)), Some(good));
        }

        assert_eq!(registry.name(GOLD), "gold");
        assert_eq!(registry.name(ORE), "ore");
        assert_eq!(registry.len(), 7);
        assert!(!registry.is_empty());
    }

    #[test]
    fn intern_is_stable_and_extends() {
        let mut registry = GoodRegistry::lab_default();
        let before = registry.len();

        // Existing names return the same id; interning is idempotent.
        assert_eq!(registry.intern("gold"), GOLD);
        assert_eq!(registry.intern("ore"), ORE);
        assert_eq!(registry.len(), before);

        // A new name extends with the next id.
        let timber = registry.intern("timber");
        assert_eq!(timber.0 as usize, before);
        assert_eq!(registry.len(), before + 1);
        assert_eq!(registry.intern("timber"), timber);
    }

    #[test]
    fn len_drives_stock_sizing_equal_to_constant_derived_size() {
        let registry = GoodRegistry::lab_default();

        // A stock sized from the registry holds the same slots as one sized
        // from the legacy max-constant-id derivation (ORE = 6 → 7 slots).
        let from_registry = Stock::new((registry.len() - 1) as u16);
        let from_constants = Stock::new(ORE.0);
        assert_eq!(from_registry, from_constants);
        assert!(from_registry.can_remove(ORE, 0));
    }

    #[test]
    fn empty_registry_interns_from_zero() {
        let mut registry = GoodRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.intern("gold").0, 0);
        assert_eq!(registry.intern("food").0, 1);
        assert_eq!(registry.name(GOLD), "gold");
        assert_eq!(registry.name(crate::good::GoodId(99)), "unknown");
    }
}
