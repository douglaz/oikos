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

/// G6b research goods. `KNOWLEDGE` is the research recipe's output — but it is an
/// **accumulator, not a tradeable good**: `sim` drains it into a per-settlement
/// counter every tick, so it never enters the goods-conservation ledger. `PASTRY`
/// is the tier-2 (higher-order) good the gated recipe produces. `LIBRARY` and
/// `ATELIER` are the durable tools that gate research and the tier-2 recipe (held,
/// never consumed) — the same capital-gate pattern as `MILL`/`OVEN`.
pub const KNOWLEDGE: &str = "knowledge";
pub const PASTRY: &str = "pastry";
pub const LIBRARY: &str = "library";
pub const ATELIER: &str = "atelier";

/// S12 own-labor subsistence: the low-grade subsistence good a hungry colonist
/// **forages** from its own labor (booked `produced`, eaten at home, ranked below
/// bread). A conserved good (tracked), wired as `KnownGoods::subsistence` on the
/// gated `own_labor_subsistence` path. `None` on every other content set, so they
/// stay byte-identical.
pub const FORAGE: &str = "forage";

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

/// Grain a scholar consumes per research application — a **conserved** good input
/// (accounted as research consumption, like ordinary consumption). G6b test 4 is
/// the tripwire: research's good inputs conserve even though its Knowledge output
/// does not.
pub const GRAIN_PER_RESEARCH: u32 = 1;
/// Knowledge produced per research application — the accumulator's increment. Not a
/// conserved good (drained into the per-settlement counter), so it has no
/// mass-balance relationship to the grain consumed.
pub const KNOWLEDGE_PER_RESEARCH: u32 = 1;
/// Flour consumed per tier-2 (confect) application — a conserved good input.
pub const FLOUR_PER_CONFECT: u32 = 1;
/// Pastry produced per confect application — the tier-2 good's yield.
pub const PASTRY_PER_CONFECT: u32 = 2;

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
    /// G6b research goods, `Some` only for a [`Self::research_tiers`] set (`None`
    /// for the plain G3a chain, so that set is byte-identical). `knowledge` is the
    /// accumulator (interned so the research recipe can name it, but **kept out of
    /// [`Self::goods`]** — it is never conserved); `pastry` is the tier-2 good;
    /// `library`/`atelier` are the durable research/tier-2 tools.
    knowledge: Option<GoodId>,
    pastry: Option<GoodId>,
    library: Option<GoodId>,
    atelier: Option<GoodId>,
    /// S12: the foraged subsistence good, `Some` only for a [`Self::with_forage`]
    /// set (the gated `own_labor_subsistence` path). `None` everywhere else, so the
    /// G3a/G6b sets are byte-identical. It has no recipe (it is produced directly
    /// from labor by the settlement's forage phase, not transformed) but IS a
    /// conserved, tracked good (it joins [`Self::goods`] and [`Self::good_entries`]).
    forage: Option<GoodId>,
    /// `[mill, bake]` for the plain chain, or `[mill, bake, research, confect]` for
    /// a research-tiers set, in chain order. The `confect` (tier-2) recipe starts
    /// `enabled: false` and is flipped by the `sim` unlock.
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
            knowledge: None,
            pastry: None,
            library: None,
            atelier: None,
            forage: None,
            recipes,
        }
    }

    /// The G6b **research-tiers** chain: the grain→flour→bread chain (unchanged)
    /// plus the research recipe (grain + labor, gated by a held `library`, →
    /// Knowledge) and the tier-2 `confect` recipe (flour + labor, gated by a held
    /// `atelier`, → pastry) which starts **`enabled: false`** until the Knowledge
    /// unlock flips it. Interns the four extra goods over the G3a chain (so
    /// `knowledge = base+5 … atelier = base+8`); `knowledge` is interned (the
    /// research recipe names it) but is the lone good kept OUT of [`Self::goods`] —
    /// it is the accumulator, never conserved.
    pub fn research_tiers() -> Self {
        let mut registry = GoodRegistry::lab_default();
        let grain = registry.intern(GRAIN);
        let flour = registry.intern(FLOUR);
        let bread = registry.intern(BREAD);
        let mill = registry.intern(MILL);
        let oven = registry.intern(OVEN);
        let knowledge = registry.intern(KNOWLEDGE);
        let pastry = registry.intern(PASTRY);
        let library = registry.intern(LIBRARY);
        let atelier = registry.intern(ATELIER);

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
            Recipe {
                id: RecipeId::Research,
                name: "Research",
                labor: 1,
                input_good: Some((grain, GRAIN_PER_RESEARCH)),
                required_tool: Some(library),
                output_good: knowledge,
                output_qty: KNOWLEDGE_PER_RESEARCH,
                enabled: true,
            },
            Recipe {
                id: RecipeId::Confect,
                name: "Confect",
                labor: 1,
                input_good: Some((flour, FLOUR_PER_CONFECT)),
                required_tool: Some(atelier),
                output_good: pastry,
                output_qty: PASTRY_PER_CONFECT,
                // Tier-gated: disabled until Knowledge crosses the unlock threshold,
                // then flipped `true` for the settlement (reusing `Recipe.enabled`).
                enabled: false,
            },
        ];

        Self {
            registry,
            grain,
            flour,
            bread,
            mill,
            oven,
            knowledge: Some(knowledge),
            pastry: Some(pastry),
            library: Some(library),
            atelier: Some(atelier),
            forage: None,
            recipes,
        }
    }

    /// S12: intern the [`FORAGE`] subsistence good onto this content set, returning the
    /// extended set. Interns over the same registry so FORAGE takes the next free id
    /// *after* every existing content good (it never shifts a grain/flour/bread/research
    /// id), keeping the gated `own_labor_subsistence` path additive. No recipe is added
    /// — FORAGE is produced directly from labor by the settlement's forage phase.
    pub fn with_forage(mut self) -> Self {
        let forage = self.registry.intern(FORAGE);
        self.forage = Some(forage);
        self
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

    /// The G6b **Knowledge** accumulator good — the research recipe's output —, or
    /// `None` for the plain G3a chain. It is interned (so the recipe names it) but is
    /// NOT a conserved good: `sim` drains every produced unit into a per-settlement
    /// counter, so it never appears in [`Self::goods`] or the conservation ledger.
    pub fn knowledge(&self) -> Option<GoodId> {
        self.knowledge
    }

    /// The G6b tier-2 (higher-order) good `pastry`, produced by the gated recipe once
    /// unlocked, or `None` for the plain chain. A conserved good (tracked).
    pub fn pastry(&self) -> Option<GoodId> {
        self.pastry
    }

    /// The durable `library` tool that gates the research recipe, or `None`.
    pub fn library(&self) -> Option<GoodId> {
        self.library
    }

    /// The durable `atelier` tool that gates the tier-2 recipe, or `None`.
    pub fn atelier(&self) -> Option<GoodId> {
        self.atelier
    }

    /// S12: the foraged subsistence good, or `None` for a content set without
    /// [`Self::with_forage`]. A conserved, tracked good with no recipe.
    pub fn forage(&self) -> Option<GoodId> {
        self.forage
    }

    /// Whether this is a research-tiers content set (the research/tier-2 recipes and
    /// the Knowledge accumulator are present).
    pub fn has_research(&self) -> bool {
        self.knowledge.is_some()
    }

    /// The research recipe (grain + labor + library → Knowledge), or `None` for the
    /// plain chain. Resolved by [`RecipeId`], like [`Self::mill_recipe`].
    pub fn research_recipe(&self) -> Option<&Recipe> {
        self.recipes
            .iter()
            .find(|recipe| recipe.id == RecipeId::Research)
    }

    /// The tier-2 recipe (flour + labor + atelier → pastry), or `None`. Starts
    /// `enabled: false`; the `sim` unlock flips it `true`. Resolved by [`RecipeId`].
    pub fn tier2_recipe(&self) -> Option<&Recipe> {
        self.recipes
            .iter()
            .find(|recipe| recipe.id == RecipeId::Confect)
    }

    /// The tier-2 recipe's [`RecipeId`], or `None` for the plain chain — the handle
    /// the settlement flips on unlock.
    pub fn tier2_recipe_id(&self) -> Option<RecipeId> {
        self.tier2_recipe().map(|recipe| recipe.id)
    }

    /// Set the `enabled` flag of the recipe with `id` (the tier-2 unlock keeps the
    /// content's own recipe copy consistent with the society's live one). A no-op if
    /// no recipe matches.
    pub fn set_recipe_enabled(&mut self, id: RecipeId, enabled: bool) {
        if let Some(recipe) = self.recipes.iter_mut().find(|recipe| recipe.id == id) {
            recipe.enabled = enabled;
        }
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

    /// Every **conserved** chain good (grain, flour, bread, mill, oven, plus the
    /// G6b pastry/library/atelier when present), in id order. These are the goods a
    /// chain settlement must track for whole-system conservation. The G6b
    /// **Knowledge** accumulator is deliberately EXCLUDED: it is monotonic, never
    /// traded or consumed, so it lives outside the goods-conservation ledger (its own
    /// reported `knowledge_produced` line). Tools (mill/oven/library/atelier) are
    /// durable and never move, but tracking them keeps "every chain good is
    /// accounted" total.
    pub fn goods(&self) -> Vec<GoodId> {
        let mut goods = vec![self.grain, self.flour, self.bread, self.mill, self.oven];
        // The research-tiers extras, minus Knowledge (the non-conserved accumulator).
        goods.extend(self.pastry);
        goods.extend(self.library);
        goods.extend(self.atelier);
        // S12: the foraged subsistence good is conserved and tracked (so the forage
        // phase that mints it into econ stock is accounted by the digest and the
        // whole-system ledger). `None` (omitted) on every non-forage set.
        goods.extend(self.forage);
        goods
    }

    /// The `(name, id)` of every interned content good, in interning (id) order — the
    /// catalog a driver replays into [`econ::society::Society::intern_good`] so
    /// the engine resolves the content names. The ids must match what `Society`
    /// returns, which holds because both intern over the same lab catalog in the
    /// same order (a `Settlement` asserts it at generation). Unlike [`Self::goods`]
    /// this DOES include the G6b Knowledge good (the research recipe names it, so the
    /// society must intern it) — interning is name resolution, not conservation
    /// tracking.
    pub fn good_entries(&self) -> Vec<(&'static str, GoodId)> {
        let mut entries = vec![
            (GRAIN, self.grain),
            (FLOUR, self.flour),
            (BREAD, self.bread),
            (MILL, self.mill),
            (OVEN, self.oven),
        ];
        if let Some(knowledge) = self.knowledge {
            entries.push((KNOWLEDGE, knowledge));
        }
        if let Some(pastry) = self.pastry {
            entries.push((PASTRY, pastry));
        }
        if let Some(library) = self.library {
            entries.push((LIBRARY, library));
        }
        if let Some(atelier) = self.atelier {
            entries.push((ATELIER, atelier));
        }
        if let Some(forage) = self.forage {
            entries.push((FORAGE, forage));
        }
        entries
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
        assert_eq!(ContentSet::research_tiers(), ContentSet::research_tiers());
    }

    #[test]
    fn plain_chain_has_no_research() {
        let content = ContentSet::grain_flour_bread();
        assert!(!content.has_research());
        assert_eq!(content.knowledge(), None);
        assert_eq!(content.research_recipe(), None);
        assert_eq!(content.tier2_recipe(), None);
        assert_eq!(content.tier2_recipe_id(), None);
        // The plain chain's good catalog is exactly the five G3a goods (byte-identical
        // serialization surface) and excludes any research good.
        assert_eq!(content.good_entries().len(), 5);
        assert_eq!(content.goods().len(), 5);
    }

    #[test]
    fn research_tiers_interns_extra_goods_after_the_chain() {
        let content = ContentSet::research_tiers();
        let base = u16::try_from(GoodRegistry::lab_default().len())
            .expect("the lab catalog fits in GoodId width");
        // The chain goods keep their ids; the research goods follow contiguously.
        assert_eq!(content.grain().0, base);
        assert_eq!(content.oven().0, base + 4);
        assert_eq!(content.knowledge(), Some(GoodId(base + 5)));
        assert_eq!(content.pastry(), Some(GoodId(base + 6)));
        assert_eq!(content.library(), Some(GoodId(base + 7)));
        assert_eq!(content.atelier(), Some(GoodId(base + 8)));
    }

    #[test]
    fn knowledge_is_interned_but_not_a_conserved_good() {
        let content = ContentSet::research_tiers();
        let knowledge = content.knowledge().expect("research content has knowledge");
        // good_entries (name resolution) includes Knowledge; goods (the conservation
        // ledger) does NOT — the accumulator lives outside the goods invariant.
        assert!(content
            .good_entries()
            .iter()
            .any(|&(_, id)| id == knowledge));
        assert!(
            !content.goods().contains(&knowledge),
            "Knowledge must be excluded from the conserved goods ledger"
        );
        // The conserved goods are the chain five plus pastry + the two tools.
        assert_eq!(content.goods().len(), 8);
        assert!(content.goods().contains(&content.pastry().unwrap()));
        assert!(content.goods().contains(&content.library().unwrap()));
        assert!(content.goods().contains(&content.atelier().unwrap()));
    }

    #[test]
    fn research_recipe_makes_knowledge_and_tier2_starts_gated() {
        let content = ContentSet::research_tiers();
        let research = content.research_recipe().expect("has a research recipe");
        assert_eq!(
            research.input_good,
            Some((content.grain(), GRAIN_PER_RESEARCH))
        );
        assert_eq!(research.required_tool, content.library());
        assert_eq!(research.output_good, content.knowledge().unwrap());
        assert!(research.enabled, "research itself is never gated");

        let confect = content.tier2_recipe().expect("has a tier-2 recipe");
        assert_eq!(
            confect.input_good,
            Some((content.flour(), FLOUR_PER_CONFECT))
        );
        assert_eq!(confect.required_tool, content.atelier());
        assert_eq!(confect.output_good, content.pastry().unwrap());
        assert!(
            !confect.enabled,
            "the tier-2 recipe must start disabled (unlocked by research)"
        );
    }

    #[test]
    fn with_forage_interns_after_the_chain_and_tracks_the_good() {
        // S12: FORAGE interns after every existing content good (never shifting a
        // chain id), is a conserved/tracked good (joins `goods` AND `good_entries`),
        // and carries no recipe. The base set without it stays byte-identical.
        let base = ContentSet::grain_flour_bread();
        let forage_set = ContentSet::grain_flour_bread().with_forage();
        assert_eq!(base.forage(), None);
        assert_eq!(base.goods().len(), 5, "the plain chain is unchanged");

        let forage = forage_set.forage().expect("with_forage interns the good");
        assert_eq!(
            forage.0,
            base.oven().0 + 1,
            "FORAGE takes the next id after the chain goods"
        );
        assert!(forage_set.goods().contains(&forage), "FORAGE is tracked");
        assert!(forage_set
            .good_entries()
            .iter()
            .any(|&(name, id)| name == FORAGE && id == forage));
        assert_eq!(forage_set.recipes().len(), base.recipes().len());
        // Research-tiers + forage: FORAGE follows the research goods too.
        let research_forage = ContentSet::research_tiers().with_forage();
        assert_eq!(
            research_forage.forage().map(|g| g.0),
            Some(research_forage.atelier().unwrap().0 + 1)
        );
    }

    #[test]
    fn set_recipe_enabled_flips_the_tier2_gate() {
        let mut content = ContentSet::research_tiers();
        assert!(!content.tier2_recipe().unwrap().enabled);
        content.set_recipe_enabled(RecipeId::Confect, true);
        assert!(content.tier2_recipe().unwrap().enabled);
    }
}
