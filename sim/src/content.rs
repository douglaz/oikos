//! The G3a **content layer**: a code-level [`ContentSet`] that defines the
//! grain → flour → bread production chain.
//!
//! Per the game-spec, content (goods and recipes) is *data*, interned into the
//! engine rather than hard-coded as `GoodId` constants. G3a realises that seam
//! at the code level: [`ContentSet::grain_flour_bread`] interns the chain's five
//! goods through an [`econ::registry::GoodRegistry`] and builds the two
//! single-input [`Recipe`]s that chain them — flour is the *output* of the mill
//! recipe and the *input* of the bake recipe, the signature production chain.
//! A TOML file loader is deferred (game-spec G3-later); the `ContentSet` API is
//! the forward-compatible shape that loader will populate.
//!
//! The set is built once at generation from a registry seeded with the lab
//! catalog ([`GoodRegistry::lab_default`]), so the chain goods take the ids
//! *after* the seven lab goods (`grain = 7 … oven = 11`) and never collide with
//! `GOLD`/`FOOD`/`WOOD`/`NET`, which the spatial economy still uses. No
//! randomness is drawn here; the same call always yields the same ids and
//! recipes.
//!
//! Tools (mill, oven) are **durable capital gates** modelled by a recipe's
//! `required_tool`: a producer must hold the tool to run the recipe, but the
//! recipe never consumes it. Grain is the only raw good — gathered from a world
//! resource node, exactly as FOOD is in G2b — while flour and bread exist only as
//! recipe outputs.

use econ::good::GoodId;
use econ::project::{Recipe, RecipeId};
use econ::registry::GoodRegistry;

/// The interned name of each chain good, in the order [`ContentSet`] interns
/// them. The position is *not* the id (the lab catalog precedes them); the id is
/// resolved through the registry and exposed by the accessors below.
pub const GRAIN: &str = "grain";
pub const FLOUR: &str = "flour";
pub const BREAD: &str = "bread";
pub const MILL: &str = "mill";
pub const OVEN: &str = "oven";

/// Grain consumed per mill application (the conserved conversion's input ratio).
pub const GRAIN_PER_MILL: u32 = 1;
/// Flour produced per mill application — the mill's yield. A recipe is a conserved
/// *conversion*, so the input and output goods each keep their own ledger; the
/// ratio is the accounted conversion, not a mass-balance constraint between two
/// different goods. A yield above one gives the chain enough throughput to feed
/// the settlement past the market's one-unit-per-tick acquisition granularity.
pub const FLOUR_PER_MILL: u32 = 3;
/// Flour consumed per bake application.
pub const FLOUR_PER_BAKE: u32 = 1;
/// Bread produced per bake application — the oven's yield (see [`FLOUR_PER_MILL`]).
pub const BREAD_PER_BAKE: u32 = 3;

/// A code-level content definition: the interned chain goods plus the recipes
/// that transform them. Built once at generation and then read-only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentSet {
    registry: GoodRegistry,
    grain: GoodId,
    flour: GoodId,
    bread: GoodId,
    mill: GoodId,
    oven: GoodId,
    /// `[mill, bake]` — the two single-input chain recipes, in chain order.
    recipes: Vec<Recipe>,
}

impl ContentSet {
    /// The G3a chain: grain → flour → bread, with mill/oven tool gates.
    ///
    /// Interns the five goods over the lab catalog (so ids are `grain = 7 …
    /// oven = 11`) and builds the mill and bake recipes. `mill`: grain + labor,
    /// gated by a held `mill`, → flour. `bake`: flour + labor, gated by a held
    /// `oven`, → bread. Single-input recipes that chain through flour.
    pub fn grain_flour_bread() -> Self {
        let mut registry = GoodRegistry::lab_default();
        let grain = registry.intern(GRAIN);
        let flour = registry.intern(FLOUR);
        let bread = registry.intern(BREAD);
        let mill = registry.intern(MILL);
        let oven = registry.intern(OVEN);

        let recipes = vec![
            Recipe {
                id: RecipeId::Mill,
                name: "Mill",
                labor: 1,
                input_good: Some((grain, GRAIN_PER_MILL)),
                required_tool: Some(mill),
                output_good: flour,
                output_qty: FLOUR_PER_MILL,
                enabled: true,
            },
            Recipe {
                id: RecipeId::Bake,
                name: "Bake",
                labor: 1,
                input_good: Some((flour, FLOUR_PER_BAKE)),
                required_tool: Some(oven),
                output_good: bread,
                output_qty: BREAD_PER_BAKE,
                enabled: true,
            },
        ];

        Self {
            registry,
            grain,
            flour,
            bread,
            mill,
            oven,
            recipes,
        }
    }

    /// The raw, gathered good (the only chain good a world node produces).
    pub fn grain(&self) -> GoodId {
        self.grain
    }

    /// The intermediate good: the mill recipe's output and the bake recipe's input.
    pub fn flour(&self) -> GoodId {
        self.flour
    }

    /// The final, consumed good — the staple the chain feeds the settlement.
    pub fn bread(&self) -> GoodId {
        self.bread
    }

    /// The durable mill tool that gates the mill recipe.
    pub fn mill(&self) -> GoodId {
        self.mill
    }

    /// The durable oven tool that gates the bake recipe.
    pub fn oven(&self) -> GoodId {
        self.oven
    }

    /// The mill recipe (grain + labor + mill → flour). Resolved by [`RecipeId`]
    /// rather than by position, so it stays correct independent of construction
    /// order — the forward-compatible TOML loader may populate `recipes` in any
    /// order without silently re-pointing this accessor.
    pub fn mill_recipe(&self) -> &Recipe {
        self.recipe(RecipeId::Mill)
    }

    /// The bake recipe (flour + labor + oven → bread). Resolved by [`RecipeId`]
    /// (see [`Self::mill_recipe`]).
    pub fn bake_recipe(&self) -> &Recipe {
        self.recipe(RecipeId::Bake)
    }

    /// The chain recipe with `id`. Panics only on a malformed `ContentSet` whose
    /// recipe set is missing a chain recipe — impossible for the sealed
    /// [`Self::grain_flour_bread`] constructor, and a loud failure rather than a
    /// silent mis-point for any future loader.
    fn recipe(&self, id: RecipeId) -> &Recipe {
        self.recipes
            .iter()
            .find(|recipe| recipe.id == id)
            .expect("the content set is missing a chain recipe")
    }

    /// Both chain recipes, in chain order `[mill, bake]`.
    pub fn recipes(&self) -> &[Recipe] {
        &self.recipes
    }

    /// Every chain good (grain, flour, bread, mill, oven), in id order. These are
    /// the goods a chain settlement must track for whole-system conservation.
    pub fn goods(&self) -> Vec<GoodId> {
        vec![self.grain, self.flour, self.bread, self.mill, self.oven]
    }

    /// The `(name, id)` of every chain good, in interning (id) order — the
    /// catalog a driver replays into [`econ::society::Society::intern_good`] so
    /// the engine resolves the content names. The ids must match what `Society`
    /// returns, which holds because both intern over the same lab catalog in the
    /// same order (a `Settlement` asserts it at generation).
    pub fn good_entries(&self) -> [(&'static str, GoodId); 5] {
        [
            (GRAIN, self.grain),
            (FLOUR, self.flour),
            (BREAD, self.bread),
            (MILL, self.mill),
            (OVEN, self.oven),
        ]
    }

    /// The content's own good registry (names ↔ ids), for inspection.
    pub fn registry(&self) -> &GoodRegistry {
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use econ::good::{FOOD, GOLD, NET, WOOD};

    #[test]
    fn chain_goods_intern_after_the_lab_catalog() {
        let content = ContentSet::grain_flour_bread();
        // The lab goods keep their ids; the chain goods follow contiguously in
        // interning order, never colliding with GOLD/FOOD/WOOD/NET (the goods the
        // spatial economy still uses). The contract is "after the lab catalog",
        // whatever its size — so the base is derived from the catalog, not the
        // literal `7` (today's seven-good catalog yields grain = 7 … oven = 11).
        let base = u16::try_from(GoodRegistry::lab_default().len())
            .expect("the lab catalog fits in GoodId width");
        assert_eq!(content.grain().0, base);
        assert_eq!(content.flour().0, base + 1);
        assert_eq!(content.bread().0, base + 2);
        assert_eq!(content.mill().0, base + 3);
        assert_eq!(content.oven().0, base + 4);
        for lab in [GOLD, FOOD, WOOD, NET] {
            assert!(content.goods().iter().all(|&chain_good| chain_good != lab));
        }
    }

    #[test]
    fn flour_is_the_output_of_one_recipe_and_the_input_of_the_next() {
        let content = ContentSet::grain_flour_bread();
        let mill = content.mill_recipe();
        let bake = content.bake_recipe();

        // The signature chain: the mill produces flour; the bake consumes it.
        assert_eq!(mill.output_good, content.flour());
        assert_eq!(bake.input_good, Some((content.flour(), FLOUR_PER_BAKE)));

        // Single-input recipes that chain through flour.
        assert_eq!(mill.input_good, Some((content.grain(), GRAIN_PER_MILL)));
        assert_eq!(bake.output_good, content.bread());

        // Tools are capital gates, held but not consumed.
        assert_eq!(mill.required_tool, Some(content.mill()));
        assert_eq!(bake.required_tool, Some(content.oven()));
    }

    #[test]
    fn goods_and_entries_are_in_id_order_and_named() {
        let content = ContentSet::grain_flour_bread();
        assert_eq!(
            content.goods(),
            vec![
                content.grain(),
                content.flour(),
                content.bread(),
                content.mill(),
                content.oven(),
            ]
        );
        let entries = content.good_entries();
        assert_eq!(entries[0], (GRAIN, content.grain()));
        assert_eq!(entries[4], (OVEN, content.oven()));
        assert_eq!(content.registry().name(content.bread()), BREAD);
    }

    #[test]
    fn construction_is_deterministic() {
        // No randomness: the same call yields identical ids and recipes.
        assert_eq!(
            ContentSet::grain_flour_bread(),
            ContentSet::grain_flour_bread()
        );
    }
}
