//! The `Region` — multiple settlements that trade (game milestone G2c).
//!
//! G2c completes the revised G2 by making **several settlement economies exist
//! and trade**. The game-spec frames this as "extract settlement-scoped services
//! from the `Society` monolith"; we reach the same *end* — independent settlement
//! economies that trade — by **composition, not internal surgery**. A [`Region`]
//! holds a `Vec<Settlement>`, each one **unchanged from G2b** (its own `World` +
//! `Society` + colonists + conservation receipt), linked by an abstract
//! inter-settlement [`Route`] (a transit-tick count), with a caravan carrying
//! one good from where it is cheap to where it is dear. Because no `Settlement`
//! and no `Society` internal behaviour changes, the six econ conformance goldens
//! and every G1/G2a/G2b/G2d test stay byte-identical (the unchanged workspace
//! `cargo test` is the proof).
//!
//! ## The caravan as a permanent trader pair (the load-bearing design)
//!
//! Runtime agent-roster mutation (`AgentArena` free/cache reconciliation) is
//! G4-deferred, so a caravan must **never** add or remove an agent from a
//! `Society`. Instead a caravan is a **pair of permanent resident trader agents**
//! — one per linked settlement, created at *generation* (see
//! [`crate::settlement::TraderEndowment`]) — and the `Region` shuttles their
//! **wealth** between them as route escrow, never the agents themselves. Each
//! settlement's agent count is therefore constant for the whole run.
//!
//! One arbitrage cycle (good `X` dearer in B than A; `trader_a` resides in A,
//! `trader_b` in B):
//!
//! 1. **BuyA** — `trader_a` is an active bidder in A's market (it holds gold and
//!    a standing want for `X`); it buys `X` at A's price through the *unchanged*
//!    [`Settlement::econ_tick`] / `Society::step` clearing — no special accessor.
//! 2. **DepartA→B** — the `Region` withdraws the bought `X` from `trader_a` into
//!    **route escrow** (`Society::debit_stock`); `trader_a`'s gold stays home
//!    (it is idled — an empty scale posts nothing — so the gold-poor seller it
//!    feeds keeps a strong sell motivation).
//! 3. **Transit** — each econ tick decrements the route's remaining transit; the
//!    escrow holdings are conserved (counted in the `Region` roll-up, at no
//!    settlement). G2c has no loss/risk: escrow in transit is never destroyed and
//!    is retained intact if a leg never completes.
//! 4. **ArriveB** — the `Region` deposits the escrowed `X` into `trader_b`
//!    (`Society::credit_stock`) and activates it to sell.
//! 5. **SellB** — `trader_b` asks `X` into B's market; it clears at B's price,
//!    accumulating gold.
//! 6. **DepartB→A / ArriveA** — the `Region` escrows `trader_b`'s proceeds (and
//!    any unsold `X`) back along the route and deposits them into `trader_a`
//!    (`Society::credit_gold` / `credit_stock`); the cycle repeats.
//!
//! While a trader is not the active side, its scale is empty, so it places no
//! orders (idle). Every transfer is **net-zero** across the `[Σ societies ∪
//! escrow]` ledger — the additive `econ` accessors MOVE value, never mint or burn
//! it — so the `Region` roll-up is an exact ledger: for each good and for gold,
//! the regional total changes only by accounted node regen (the source) and
//! consumption (the sink). [`Region::econ_tick`] checks this every tick.
//!
//! ## Direction, and what this deliberately is not
//!
//! G2c is **two settlements, one traded good, one caravan**. The buy→sell
//! direction is fixed by construction: settlement 0 (A) is configured cheaper
//! (a nearer FOOD node) and settlement 1 (B) dearer (a farther node), so the
//! realized-price differential's *sign* is persistent; the caravan still gates a
//! fresh haul on the live differential (a `Hold` phase) so it never hauls the
//! wrong way after a convergence overshoot. There is no loss/risk, no
//! roads-as-projects, no `>2` settlements, and no multi-good / dynamic multi-hop
//! routing (all later). Determinism is inherited: integer state, the econ `Rng`
//! consumed only at generation, nothing drawn in the loops, settlement- and
//! id-ordered iteration, `BTreeMap`/`Vec` only — same `(seed, RegionConfig)` →
//! byte-identical run ([`Region::digest`]).

use std::collections::BTreeMap;

use econ::agent::{AgentId, Want, WantKind};
use econ::good::{Gold, GoodId, Horizon, Stock, FOOD, GOLD, WOOD};
use econ::project::{
    advance_project_by, build_road_template, complete_project_if_ready, start_project, Project,
    ProjectId, ProjectState, ProjectTemplate, Tick,
};

use crate::settlement::{Settlement, SettlementConfig, TraderEndowment};

/// An abstract inter-settlement **route**: a transit-tick count, NOT intra-
/// settlement grid movement. The number of econ ticks a caravan spends in transit
/// on each leg. "near"/"far" route lengths give the convergence contrast and the
/// future road idea; G2c uses one route between the two settlements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    /// Econ ticks a caravan is in transit on a leg (each way). Zero means a leg
    /// completes the same tick it departs.
    pub transit_ticks: u32,
}

/// A G7 **road** to build on the region's one route (game milestone G7).
///
/// A road is **public works built from community labor** — the §5.9 funding ladder
/// before state taxation exists (G8): colonists contribute labor to the road
/// project each econ tick, reusing the G3 project-labor lifecycle
/// ([`econ::project`]). When the contributed labor meets [`Self::labor_cost`] the
/// route's [`Route::transit_ticks`] is cut to [`Self::transit_after`], so completed
/// caravans cycle faster and the realized-price gap converges faster than the
/// no-road control. The build is a **conserved expenditure**: it consumes the
/// (optional) community materials it declares and creates **no** good — it merely
/// changes the abstract route's transit cost. The road is **one-way** (once built it
/// stays) and deterministic (no randomness; the completion tick is a pure function
/// of seed + config). Scope is ONE road on the ONE route — no networks, no
/// grid-pathable roads, no state-funded works (all deferred; see
/// `docs/engine-divergence.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoadPlan {
    /// Total labor units the community must contribute before the road is built.
    pub labor_cost: u32,
    /// Labor each living colonist contributes to the public works per econ tick —
    /// the community's daily public-works effort. Gated on a living population, so
    /// an emptied colony stalls the road (it is community labor, never a timer).
    pub labor_per_colonist: u32,
    /// The route's [`Route::transit_ticks`] AFTER the road completes (the built-road
    /// transit). Must be strictly below the unbuilt route's, so completed caravans
    /// cycle faster — the convergence acceleration the milestone proves.
    pub transit_after: u32,
    /// The conserved good the build consumes as community **materials** (e.g. WOOD).
    /// Drawn incrementally from the region road fund as labor is contributed, and
    /// accounted as a conserved input (never re-created).
    pub material: GoodId,
    /// Units of [`Self::material`] consumed per labor unit. `0` is a labor-only road
    /// (no good cost) — the materials are then optional, exactly as the spec allows.
    pub material_per_labor: u32,
}

/// How a region of two settlements is generated: the two settlement recipes, the
/// linking route, the single traded good, the caravan's trader working capital,
/// the dwell lengths, and whether the caravan runs at all (the no-caravan control
/// is `caravans_enabled = false`). Mechanism knobs, not balance targets.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionConfig {
    /// Settlement A (index 0, the buyer/cheaper side) recipe.
    pub settlement_a: SettlementConfig,
    /// Settlement B (index 1, the seller/dearer side) recipe.
    pub settlement_b: SettlementConfig,
    /// The abstract route linking A and B.
    pub route: Route,
    /// The single good the caravan trades. Must be a good both settlements
    /// actively **trade** — one with an order book in each settlement's market
    /// (a built-in market good like FOOD/WOOD/NET, or one some agent holds or
    /// wants at generation) and tracked for conservation in both. FOOD is the
    /// canonical choice. Never GOLD (money is not hauled); a node-only tracked
    /// good with no order book is rejected at [`Region::generate`] (it would
    /// conserve but never trade).
    pub good: GoodId,
    /// Working-capital gold the buyer trader (`trader_a`) starts with. The seller
    /// trader (`trader_b`) starts gold-poor (zero) so it keeps a strong incentive
    /// to sell the good it is handed.
    pub trader_gold: u64,
    /// Econ ticks the caravan dwells buying at A before departing.
    pub buy_ticks: u32,
    /// Econ ticks the caravan dwells selling at B before departing.
    pub sell_ticks: u32,
    /// Whether the caravan operates. `false` is the **no-caravan control** (the
    /// falsification twin): the trader pair still exists (agent counts match) but
    /// stays idle, so the settlements never interact and the price gap is kept.
    pub caravans_enabled: bool,
    /// The G7 **road** to build on this route, or `None` for a region with no road
    /// (the **no-road control**). A road is community-labor public works that cuts
    /// the route's transit cost on completion, so caravans cycle faster and the
    /// price gap converges faster than the control. `None` leaves every G2c config
    /// byte-identical (the road is purely additive region state).
    pub road: Option<RoadPlan>,
}

impl RegionConfig {
    /// The canonical two-settlement, one-caravan region for the price-convergence
    /// experiment. Settlement A is cheap and supply-rich (a near FOOD node, more
    /// gatherers than consumers); settlement B is dear and supply-poor (a far node,
    /// a small population that starts with no FOOD buffer, so it is scarce — and so
    /// dear — from the first ticks). The caravan buys FOOD cheap at A and sells it
    /// at B, relieving B's scarcity so its realized FOOD price falls toward A's;
    /// the no-caravan control keeps the gap. Both settlements use the G2b
    /// hunger-resilient [`SettlementConfig::price_probe`] colonists (so the dear
    /// market stays demand-heavy and bids up under scarcity instead of dying off).
    /// Mechanism knobs, not balance targets; the proof is sign only.
    pub fn two_settlements() -> Self {
        let mut settlement_a = SettlementConfig::price_probe();
        settlement_a.gatherers = 8;
        settlement_a.consumers = 4;
        let settlement_a = settlement_a.with_food_node_distance(6);

        let mut settlement_b = SettlementConfig::price_probe();
        settlement_b.gatherers = 4;
        settlement_b.consumers = 2;
        // No starting FOOD buffer, so B's consumers are hungry — and so the market
        // is dear — from the opening ticks, giving a positive gap for the caravan
        // to narrow (rather than merely prevent from opening).
        settlement_b.consumer_food_buffer = 0;
        settlement_b.gatherer_food_buffer = 0;
        let settlement_b = settlement_b.with_food_node_distance(18);

        Self {
            settlement_a,
            settlement_b,
            route: Route { transit_ticks: 1 },
            good: FOOD,
            trader_gold: 600,
            buy_ticks: 6,
            sell_ticks: 10,
            caravans_enabled: true,
            road: None,
        }
    }

    /// The no-caravan **control** twin of [`Self::two_settlements`] — identical
    /// settlements and trader pair (so agent counts match), but the caravan is
    /// disabled, so the settlements never interact and the price gap is kept. The
    /// falsification twin that proves the caravan is what closes the gap.
    pub fn two_settlements_control() -> Self {
        Self {
            caravans_enabled: false,
            ..Self::two_settlements()
        }
    }

    /// The G7 **roads** region: the two-settlement caravan region on a **longer**
    /// route (so the road's transit cut bites), plus a road public-works project the
    /// community builds from labor. Once enough labor is contributed the route's
    /// transit cost is cut (`20 → 8`, a defined amount well below the unbuilt route),
    /// so caravans cycle faster and the FOOD price gap converges faster than the
    /// [`Self::roads_control`] twin (same region, no road). The caravan is enabled in
    /// BOTH (the control is the no-**road** twin, not the no-caravan one), so the road
    /// — not the caravan, which G2c already had — is the only difference, and it is the
    /// cause of the faster convergence.
    ///
    /// The build consumes a modest WOOD material per labor unit (a conserved
    /// expenditure, accounted) and creates no good. Mechanism knobs, not balance
    /// targets; the proof is sign only (the gap is tighter with the road).
    pub fn roads() -> Self {
        Self {
            route: Route { transit_ticks: 20 },
            road: Some(RoadPlan {
                // ~18 living colonists each contribute one labor unit per econ tick,
                // so the road completes early (tick 9) and most of the horizon runs on
                // the cut transit. A pure function of seed + config (deterministic).
                labor_cost: 180,
                labor_per_colonist: 1,
                transit_after: 8,
                material: WOOD,
                material_per_labor: 1,
            }),
            ..Self::two_settlements()
        }
    }

    /// The G7 **no-road control** twin of [`Self::roads`]: the SAME region and
    /// caravan on the SAME longer route, but with **no road** — the route's transit
    /// cost is never cut, so caravans stay slow and the price gap converges slower /
    /// to a wider final gap. Paired with [`Self::roads`], it is the proof that the
    /// road (not time or the caravan alone) is what accelerates convergence: if both
    /// converged identically the road would not be cutting transit.
    pub fn roads_control() -> Self {
        Self {
            road: None,
            ..Self::roads()
        }
    }

    /// Number of distinct `Want{Good(GOLD), Later}` units the seller's value scale
    /// carries — its standing money demand. Sized well above the gold a seller
    /// ever holds in one sell phase (it arrives gold-poor and only accrues a
    /// cycle's proceeds before they are swept home), so the seller keeps an
    /// unmet money want and goes on asking the good cheaply. If it were ever too
    /// small the seller would simply stop early and carry the remainder home —
    /// conserved, never lost.
    const SELL_LADDER_UNITS: u16 = 256;
    /// Number of `Want{Good(X), Next}` units the buyer's scale carries — its
    /// standing demand for the traded good. Bounds how much the buyer accumulates
    /// in one buy phase (it buys at most one unit per econ tick); a few times the
    /// buy dwell is ample.
    const BUY_LADDER_UNITS: u16 = 64;
}

/// The per-econ-tick **region-wide** conservation + flow receipt. Sums each
/// settlement's own [`crate::EconTickReport`] and adds the in-transit route
/// escrow, so the invariant it pins is regional: for every tracked good the total
/// over all settlements **plus** route escrow changes by exactly `+regen +
/// endowment + produced − consumed_as_input − consumed`; for gold the regional
/// total is unchanged (gold is a closed regional balance). Every caravan transfer
/// is net-zero and so never appears. The `endowment` term is nonzero only with a
/// demography settlement (G4b) and the two production terms only when a composed
/// settlement runs the G3a chain; for a plain region all three stay empty and the
/// invariant reduces to the G2c form `+regen − consumed`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegionTickReport {
    pub econ_tick: u64,
    /// Goods created by node regen across all settlements (the only raw source).
    pub regen: BTreeMap<GoodId, u64>,
    /// Goods minted by the G4b per-member **provision** (the household hearth)
    /// across all settlements — a conserved source like `regen`. Empty for a
    /// region with no demography-enabled settlement, so the invariant is
    /// unchanged there.
    pub endowment: BTreeMap<GoodId, u64>,
    /// Goods **produced** by recipe applications across all settlements (G3a) —
    /// the output side of every accounted transformation. Empty for a plain
    /// (non-chain) region.
    pub produced: BTreeMap<GoodId, u64>,
    /// Goods **consumed as a recipe input** by recipe applications across all
    /// settlements (G3a) — the input side of every accounted transformation.
    /// Distinct from `consumed` (eaten). Empty for a plain (non-chain) region.
    pub consumed_as_input: BTreeMap<GoodId, u64>,
    /// Goods consumed across all settlements (the only final sink — eaten).
    pub consumed: BTreeMap<GoodId, u64>,
    /// Regional total per good (Σ settlements + route escrow) before this tick.
    pub before: BTreeMap<GoodId, u64>,
    /// Regional total per good (Σ settlements + route escrow) after this tick.
    pub after: BTreeMap<GoodId, u64>,
    /// In-transit route escrow per good at the end of this tick.
    pub escrow: BTreeMap<GoodId, u64>,
    /// Regional gold (Σ settlements + escrow) before / after — must be equal.
    pub gold_before: u64,
    pub gold_after: u64,
    /// In-transit route escrow gold at the end of this tick.
    pub escrow_gold: u64,
    /// G7: labor the community contributed to the road public-works project this
    /// tick — a conserved **expenditure** reported on its OWN line, NOT a good (labor
    /// is abstract in this engine, as in G3/G6b, so it is deliberately outside the
    /// goods-conservation identity; the road's conserved good *materials* ARE
    /// accounted, in [`Self::consumed_as_input`]). Zero on a region with no road, on
    /// every tick after the road completes, and whenever no labor is contributed.
    pub road_labor: u64,
}

impl RegionTickReport {
    pub fn regen_of(&self, good: GoodId) -> u64 {
        self.regen.get(&good).copied().unwrap_or(0)
    }
    /// Units of `good` provisioned region-wide by the G4b household hearth this
    /// tick — a source. Zero for a region with no demography settlement.
    pub fn endowment_of(&self, good: GoodId) -> u64 {
        self.endowment.get(&good).copied().unwrap_or(0)
    }
    /// Units of `good` produced by recipe applications region-wide this tick (G3a).
    pub fn produced_of(&self, good: GoodId) -> u64 {
        self.produced.get(&good).copied().unwrap_or(0)
    }
    /// Units of `good` consumed as a recipe input region-wide this tick (G3a).
    pub fn consumed_as_input_of(&self, good: GoodId) -> u64 {
        self.consumed_as_input.get(&good).copied().unwrap_or(0)
    }
    pub fn consumed_of(&self, good: GoodId) -> u64 {
        self.consumed.get(&good).copied().unwrap_or(0)
    }
    pub fn before_of(&self, good: GoodId) -> u64 {
        self.before.get(&good).copied().unwrap_or(0)
    }
    pub fn after_of(&self, good: GoodId) -> u64 {
        self.after.get(&good).copied().unwrap_or(0)
    }
    pub fn escrow_of(&self, good: GoodId) -> u64 {
        self.escrow.get(&good).copied().unwrap_or(0)
    }

    /// Whether the regional ledger balances: for every tracked good `after ==
    /// before + regen + endowment + produced − consumed_as_input − consumed`, and
    /// gold is unchanged. This is the G2c conservation DoD, **generalized across
    /// G3a transformations** exactly as [`crate::EconTickReport::conserves`] is: a
    /// recipe is a conserved conversion, so its output and input are accounted on
    /// the producing/consuming sides; the G4b household provision is a source like
    /// `regen`. For a plain region the `endowment` and production terms are empty
    /// and this reduces to the G2c form `after == before + regen − consumed`.
    /// [`Region::econ_tick`] also `debug_assert`s it.
    pub fn conserves(&self) -> bool {
        let goods_ok = self.before.keys().all(|good| {
            let before = self.before_of(*good) as i128;
            let after = self.after_of(*good) as i128;
            let regen = self.regen_of(*good) as i128;
            let endowment = self.endowment_of(*good) as i128;
            let produced = self.produced_of(*good) as i128;
            let consumed_as_input = self.consumed_as_input_of(*good) as i128;
            let consumed = self.consumed_of(*good) as i128;
            after == before + regen + endowment + produced - consumed_as_input - consumed
        });
        goods_ok && self.gold_after == self.gold_before
    }
}

/// One arbitrage cycle's phase. The caravan advances one phase boundary at a time,
/// dwelling `buy_ticks` / `sell_ticks` at the endpoints and `route.transit_ticks`
/// on each leg. Integer state only; no randomness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// `trader_a` actively buys the good at settlement A.
    BuyA,
    /// In transit A→B: the bought good rides in route escrow.
    ToB,
    /// `trader_b` actively sells the good at settlement B.
    SellB,
    /// In transit B→A: the proceeds (and any unsold good) ride in route escrow.
    ToA,
    /// Parked at A: the live price differential is unfavourable (A is no longer
    /// cheaper than B), so the caravan waits rather than haul the wrong way.
    Hold,
}

/// The caravan operator: the permanent trader pair, the phase machine, and the
/// in-transit route escrow. `trader_a` resides in settlement 0 (the buyer side),
/// `trader_b` in settlement 1 (the seller side).
struct Caravan {
    trader_a: AgentId,
    trader_b: AgentId,
    phase: Phase,
    /// Econ ticks remaining in the current dwell/transit phase.
    counter: u32,
    /// Good held in route escrow (mid-transit, at no settlement).
    escrow_good: u32,
    /// Gold held in route escrow (mid-transit, at no settlement).
    escrow_gold: u64,
}

/// The G7 **road** public-works project on the region's route: the reused
/// project-labor lifecycle (an [`econ::project::Project`] + its template), a road
/// **fund** of conserved community materials, and the route transit the build cuts
/// to on completion. The `Region` runs it as a parallel to the caravan operator —
/// community labor in, a transit cut out — never touching the settlements'
/// economies, so the road config and its no-road control share byte-identical
/// settlements until the cut fires.
struct RoadProject {
    /// The labor-only public-works template (no good output). The labor cost is
    /// `template.required_labor`.
    template: ProjectTemplate,
    /// The reused project state: accumulated labor and the Forming→Complete lifecycle.
    project: Project,
    /// Conserved community materials the build draws down (so they stay in the
    /// region-wide total until spent, and their consumption is accounted). Holds only
    /// [`Self::material`]; never gold.
    fund: Stock,
    /// The conserved good consumed as materials, and the amount per labor unit.
    material: GoodId,
    material_per_labor: u32,
    /// Labor each living colonist contributes to the road per econ tick.
    labor_per_colonist: u32,
    /// The route transit the road cuts to on completion.
    transit_after: u32,
    /// The econ tick the road completed (its transit cut fired), or `None` while
    /// still building — the one-way completion stamp.
    completed_at: Option<u64>,
}

/// A region of two composed settlements linked by one caravan over one route.
pub struct Region {
    settlements: Vec<Settlement>,
    caravan: Caravan,
    route: Route,
    good: GoodId,
    caravans_enabled: bool,
    /// The G7 road public works on the route, or `None` for a no-road region.
    road: Option<RoadProject>,
    buy_ticks: u32,
    sell_ticks: u32,
    /// The goods tracked for region-wide conservation (union of the settlements'
    /// tracked goods), `GoodId`-ordered. GOLD is excluded (money, tracked apart).
    goods: Vec<GoodId>,
    econ_tick: u64,
    last_report: RegionTickReport,
}

impl Region {
    /// Generate a region from `seed` and a [`RegionConfig`]. Each settlement is
    /// generated with one resident trader (the caravan's pair); all randomness is
    /// drawn inside the settlements' `generate` (per-colonist culture) — the
    /// region loop draws none. Deterministic: same `(seed, RegionConfig)` →
    /// byte-identical region.
    pub fn generate(seed: u64, config: &RegionConfig) -> Self {
        assert_ne!(
            config.good, GOLD,
            "the caravan cannot trade the money good (GOLD)"
        );
        // One resident trader per settlement (the permanent pair). The buyer side
        // (A) carries the working capital; the seller side (B) starts gold-poor so
        // it keeps a standing money demand once it is handed the good to sell.
        let config_a = config
            .settlement_a
            .clone()
            .with_resident_traders(vec![TraderEndowment {
                gold: config.trader_gold,
                stock: Vec::new(),
            }]);
        let config_b = config
            .settlement_b
            .clone()
            .with_resident_traders(vec![TraderEndowment {
                gold: 0,
                stock: Vec::new(),
            }]);

        // Distinct sub-seeds so the two settlements are independently generated
        // (same base config would otherwise produce identical colonist cultures).
        let mut settlement_a = Settlement::generate(seed, &config_a);
        let settlement_b = Settlement::generate(seed ^ 0x9E37_79B9_7F4A_7C15, &config_b);

        let trader_a = settlement_a
            .resident_trader_ids()
            .first()
            .copied()
            .expect("settlement A was generated with one resident trader");
        let trader_b = settlement_b
            .resident_trader_ids()
            .first()
            .copied()
            .expect("settlement B was generated with one resident trader");

        assert!(
            settlement_a.tracked_goods().contains(&config.good)
                && settlement_b.tracked_goods().contains(&config.good),
            "both settlements must track the traded good for region-wide conservation"
        );
        // The good must also be actively tradeable — have an order book — in BOTH
        // settlements, else the caravan would set up its trader scales but post no
        // orders, so the region would conserve yet never trade (a silent no-op). A
        // book exists only for the built-in market goods (FOOD/WOOD/NET) or a good
        // some agent holds or wants at generation; a node-only tracked good (e.g.
        // SALT) has none. Reject it loudly rather than run a dead region.
        assert!(
            settlement_a.society().market_goods().contains(&config.good)
                && settlement_b.society().market_goods().contains(&config.good),
            "the traded good {:?} must have an order book in both settlements' \
             markets (a built-in market good like FOOD/WOOD/NET, or one held or \
             wanted by some agent at generation); a node-only good would conserve \
             but never trade",
            config.good
        );

        // The tracked-goods union (GoodId order) for the conservation roll-up.
        let mut goods: Vec<GoodId> = settlement_a.tracked_goods().to_vec();
        for &g in settlement_b.tracked_goods() {
            if !goods.contains(&g) {
                goods.push(g);
            }
        }
        // G7: the road's material is conserved community stock held in the road fund,
        // so it joins the region ledger even if a settlement does not otherwise track
        // it — its draw-down during the build is then snapshotted and accounted.
        if let Some(plan) = &config.road {
            if plan.material_per_labor > 0 && !goods.contains(&plan.material) {
                goods.push(plan.material);
            }
        }
        goods.sort();

        // G7: build the road public-works project (the reused project-labor lifecycle
        // plus its fund of conserved community materials). `None` for a no-road region
        // — every G2c region is byte-identical there.
        let road = config
            .road
            .as_ref()
            .map(|plan| build_road(plan, &config.route));

        // The caravan opens by buying at A (A is configured the cheaper side), so
        // its buyer scale is active on tick 0. At generation no trade has cleared,
        // so the favourability gate is vacuously true and the caravan opens in
        // `BuyA`; it is applied anyway so a configuration whose realized
        // differential is already inverted (A dearer than B) parks in `Hold`
        // instead of opening with a wrong-way haul. The control (caravans
        // disabled) keeps the same `BuyA`/`buy_ticks` opening — it never steps, so
        // its serialized state is unchanged.
        let opens_buying = config.caravans_enabled
            && haul_is_favourable(
                settlement_a.realized_price(config.good),
                settlement_b.realized_price(config.good),
            );
        let (phase, counter) = if config.caravans_enabled && !opens_buying {
            (Phase::Hold, 0)
        } else {
            (Phase::BuyA, config.buy_ticks)
        };
        if opens_buying {
            set_trader_scale(&mut settlement_a, trader_a, buy_scale(config.good));
        }

        let caravan = Caravan {
            trader_a,
            trader_b,
            phase,
            counter,
            escrow_good: 0,
            escrow_gold: 0,
        };

        Self {
            settlements: vec![settlement_a, settlement_b],
            caravan,
            route: config.route,
            good: config.good,
            caravans_enabled: config.caravans_enabled,
            road,
            buy_ticks: config.buy_ticks,
            sell_ticks: config.sell_ticks,
            goods,
            econ_tick: 0,
            last_report: RegionTickReport::default(),
        }
    }

    /// Advance the region by one economic tick: advance each settlement's econ
    /// tick (the unchanged G2b loop), run the caravan step (decide / escrow-move /
    /// (de)activate traders) using the additive transfer accessors, then roll up
    /// the region-wide conservation report. Returns — and stores — the report.
    pub fn econ_tick(&mut self) -> RegionTickReport {
        let mut report = RegionTickReport {
            econ_tick: self.econ_tick,
            ..RegionTickReport::default()
        };

        // ---- before: regional totals (Σ settlements + route escrow).
        for &good in &self.goods {
            report.before.insert(good, self.regional_total(good));
        }
        report.gold_before = self.regional_gold();

        // ---- advance each settlement (unchanged G2b), summing the conservation
        // flows. `produced`/`consumed_as_input` are nonzero only for a composed
        // G3a chain settlement; for a plain region they stay empty and the
        // regional invariant reduces to the G2c `+regen − consumed` form.
        for settlement in &mut self.settlements {
            let tick_report = settlement.econ_tick();
            for (&good, &qty) in &tick_report.regen {
                *report.regen.entry(good).or_insert(0) += qty;
            }
            for (&good, &qty) in &tick_report.endowment {
                *report.endowment.entry(good).or_insert(0) += qty;
            }
            for (&good, &qty) in &tick_report.produced {
                *report.produced.entry(good).or_insert(0) += qty;
            }
            for (&good, &qty) in &tick_report.consumed_as_input {
                *report.consumed_as_input.entry(good).or_insert(0) += qty;
            }
            for (&good, &qty) in &tick_report.consumed {
                *report.consumed.entry(good).or_insert(0) += qty;
            }
        }

        // ---- caravan step (net-zero moves; only runs when enabled).
        if self.caravans_enabled {
            self.caravan_step();
        }

        // ---- road public works (G7): the community contributes labor to the road
        // until its cost is met; on completion the route's transit cost is cut. The
        // build consumes conserved materials from the road fund (accounted in
        // `consumed_as_input`) and creates no good. A no-op for a region with no road
        // and after the road completes (one-way). Runs after the caravan, so this
        // tick's haul used the old transit and the cut applies from the next departure.
        self.road_step(&mut report);

        // ---- after: regional totals + escrow snapshot.
        for &good in &self.goods {
            report.after.insert(good, self.regional_total(good));
        }
        report.gold_after = self.regional_gold();
        report
            .escrow
            .insert(self.good, u64::from(self.caravan.escrow_good));
        report.escrow_gold = self.caravan.escrow_gold;

        debug_assert!(
            report.conserves(),
            "region-wide conservation broke at econ tick {}",
            self.econ_tick
        );

        self.econ_tick += 1;
        self.last_report = report.clone();
        report
    }

    /// Run `ticks` region econ ticks.
    pub fn run(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.econ_tick();
        }
    }

    // ---- the caravan operator ------------------------------------------

    /// One caravan step, run after both settlements have advanced this tick. It
    /// progresses the phase machine: counting down dwells/transits, moving wealth
    /// between a trader and the route escrow at each boundary (always net-zero),
    /// and (de)activating trader scales for the next tick. The buyer's gold stays
    /// home through the A→B leg so the seller it feeds arrives gold-poor and keeps
    /// a strong sell motivation; the proceeds ride back on the B→A leg.
    fn caravan_step(&mut self) {
        let trader_a = self.caravan.trader_a;
        let trader_b = self.caravan.trader_b;
        let good = self.good;

        match self.caravan.phase {
            Phase::BuyA => {
                self.caravan.counter = self.caravan.counter.saturating_sub(1);
                if self.caravan.counter == 0 {
                    // The buy dwell discovers A's price; gate the departure on the
                    // live differential so the caravan never hauls the wrong way
                    // even if the buy phase revealed A is no longer the cheaper
                    // side. Idle the trader either way (an empty scale posts
                    // nothing, and its gold stays home for the A→B leg).
                    idle_trader(&mut self.settlements[0], trader_a);
                    if self.favourable_to_haul() {
                        // Depart A→B: escrow the bought good.
                        let moved = withdraw_all_stock(&mut self.settlements[0], trader_a, good);
                        self.caravan.escrow_good += moved;
                        self.caravan.phase = Phase::ToB;
                        self.caravan.counter = self.route.transit_ticks;
                        // A zero-length transit arrives the same tick.
                        self.maybe_complete_transit();
                    } else {
                        // Unfavourable: park at A rather than haul wrong. The bought
                        // good stays with the idled trader (conserved at A, never
                        // hauled wrong) until the differential is favourable again.
                        self.caravan.phase = Phase::Hold;
                    }
                }
            }
            Phase::ToB => {
                self.advance_transit();
            }
            Phase::SellB => {
                self.caravan.counter = self.caravan.counter.saturating_sub(1);
                if self.caravan.counter == 0 {
                    // Depart B→A: escrow the proceeds (gold) and any unsold good.
                    idle_trader(&mut self.settlements[1], trader_b);
                    let unsold = withdraw_all_stock(&mut self.settlements[1], trader_b, good);
                    let proceeds = withdraw_all_gold(&mut self.settlements[1], trader_b);
                    self.caravan.escrow_good += unsold;
                    self.caravan.escrow_gold += proceeds;
                    self.caravan.phase = Phase::ToA;
                    self.caravan.counter = self.route.transit_ticks;
                    self.maybe_complete_transit();
                }
            }
            Phase::ToA => {
                self.advance_transit();
            }
            Phase::Hold => {
                // Parked at A because the differential is unfavourable; re-check.
                if self.favourable_to_haul() {
                    set_trader_scale(&mut self.settlements[0], trader_a, buy_scale(good));
                    self.caravan.phase = Phase::BuyA;
                    self.caravan.counter = self.buy_ticks;
                }
            }
        }
    }

    /// Decrement the current transit and, when it reaches zero, complete the leg.
    fn advance_transit(&mut self) {
        debug_assert!(matches!(self.caravan.phase, Phase::ToB | Phase::ToA));
        self.caravan.counter = self.caravan.counter.saturating_sub(1);
        self.maybe_complete_transit();
    }

    /// If a transit phase has run out its counter, deposit the escrow into the
    /// destination trader and start the next dwell. Also handles a zero-length
    /// transit (arrival the same tick it departs).
    fn maybe_complete_transit(&mut self) {
        if self.caravan.counter != 0 {
            return;
        }
        let trader_a = self.caravan.trader_a;
        let trader_b = self.caravan.trader_b;
        let good = self.good;
        match self.caravan.phase {
            Phase::ToB => {
                // Arrive at B: hand the good to the seller and activate it.
                let amount = self.caravan.escrow_good;
                self.caravan.escrow_good = 0;
                if amount > 0 {
                    let ok = self.settlements[1]
                        .society_mut()
                        .credit_stock(trader_b, good, amount);
                    assert!(
                        ok,
                        "the resident seller trader must accept the escrowed good"
                    );
                }
                set_trader_scale(&mut self.settlements[1], trader_b, sell_scale());
                self.caravan.phase = Phase::SellB;
                self.caravan.counter = self.sell_ticks;
            }
            Phase::ToA => {
                // Arrive at A: return proceeds (and any unsold good) to the buyer.
                let amount = self.caravan.escrow_good;
                self.caravan.escrow_good = 0;
                if amount > 0 {
                    let ok = self.settlements[0]
                        .society_mut()
                        .credit_stock(trader_a, good, amount);
                    assert!(
                        ok,
                        "the resident buyer trader must accept the returned good"
                    );
                }
                let gold = self.caravan.escrow_gold;
                self.caravan.escrow_gold = 0;
                if gold > 0 {
                    let ok = self.settlements[0]
                        .society_mut()
                        .credit_gold(trader_a, Gold(gold));
                    assert!(
                        ok,
                        "the resident buyer trader must accept the returned gold"
                    );
                }
                // Start a fresh cycle only if the differential is still favourable.
                if self.favourable_to_haul() {
                    set_trader_scale(&mut self.settlements[0], trader_a, buy_scale(good));
                    self.caravan.phase = Phase::BuyA;
                    self.caravan.counter = self.buy_ticks;
                } else {
                    self.caravan.phase = Phase::Hold;
                }
            }
            _ => {}
        }
    }

    /// Whether the live realized-price differential favours the configured
    /// A→B haul: A is no greater than B (or a price is not yet known, in which
    /// case the caravan proceeds to get the market going). Buy-low/sell-high.
    fn favourable_to_haul(&self) -> bool {
        haul_is_favourable(
            self.settlements[0].realized_price(self.good),
            self.settlements[1].realized_price(self.good),
        )
    }

    // ---- the road operator (G7 public works) ---------------------------

    /// One road step, run after the caravan step this tick. The community contributes
    /// labor to the road project (gated on a living population — it is community
    /// labor, not a timer), the build draws down its conserved materials from the road
    /// fund (accounted as a conserved input), and on completion the route's transit
    /// cost is cut to the built-road transit. **One-way**: once `Complete`, the
    /// project is never advanced again, so the transit cut never flaps. A no-op for a
    /// region with no road. Deterministic — integer state, nothing drawn.
    fn road_step(&mut self, report: &mut RegionTickReport) {
        if self.road.is_none() {
            return;
        }
        // Community labor available this tick: every living colonist across the
        // settlements contributes (resident traders are not colonists, so they do
        // not). Computed before the road borrow (disjoint fields, but the borrow
        // checker needs the immutable settlement read released first).
        let living: u64 = self
            .settlements
            .iter()
            .map(|s| s.living_total() as u64)
            .sum();
        let tick = self.econ_tick;
        let mut completed_transit: Option<u32> = None;
        {
            let road = self.road.as_mut().expect("road is Some (checked above)");
            // One-way: a built (or abandoned) road never builds again.
            if road.project.state != ProjectState::Forming {
                return;
            }
            // Community labor only: with no living colonists nothing is contributed.
            if living == 0 {
                return;
            }
            let want = living.saturating_mul(u64::from(road.labor_per_colonist));
            let remaining = u64::from(
                road.template
                    .required_labor
                    .saturating_sub(road.project.labor_advanced),
            );
            let mut labor = want.min(remaining);
            // Cap by the materials on hand — the build cannot outrun its conserved
            // inputs (a labor-only road, `material_per_labor == 0`, is never capped).
            if road.material_per_labor > 0 {
                let affordable =
                    u64::from(road.fund.get(road.material)) / u64::from(road.material_per_labor);
                labor = labor.min(affordable);
            }
            if labor == 0 {
                return;
            }
            let labor = u32::try_from(labor).expect("per-tick road labor fits in u32");
            // Consume the conserved community materials (a conserved expenditure,
            // accounted as a recipe-style input; the build creates no good).
            if road.material_per_labor > 0 {
                let used = labor
                    .checked_mul(road.material_per_labor)
                    .expect("validated road material use fits in u32");
                let ok = road.fund.remove(road.material, used);
                debug_assert!(ok, "the road fund holds the materials it consumes");
                *report.consumed_as_input.entry(road.material).or_insert(0) += u64::from(used);
            }
            // Contribute the whole tick's pooled community labor through the reused
            // project-labor lifecycle in ONE bulk step. Equivalent to looping the unit
            // advance `labor` times (the completion tick and digest are unchanged), but
            // O(1) — a large accepted config (e.g. a huge `labor_per_colonist`) cannot
            // spin the per-unit loop for billions of iterations within a single tick.
            advance_project_by(&mut road.project, labor);
            report.road_labor = u64::from(labor);
            // On completion, stamp the tick and cut the route transit.
            if complete_project_if_ready(&mut road.project, &road.template, &mut road.fund) {
                road.completed_at = Some(tick);
                completed_transit = Some(road.transit_after);
            }
        }
        if let Some(transit) = completed_transit {
            self.route.transit_ticks = transit;
        }
    }

    // ---- conservation roll-up helpers ----------------------------------

    /// The regional total of `good`: Σ over settlements of their whole-system
    /// total, plus any units in route escrow, plus any held in the road fund. The
    /// conserved regional quantity.
    fn regional_total(&self, good: GoodId) -> u64 {
        let in_settlements: u64 = self
            .settlements
            .iter()
            .map(|s| s.whole_system_total(good))
            .sum();
        let in_escrow = if good == self.good {
            u64::from(self.caravan.escrow_good)
        } else {
            0
        };
        let in_road_fund = self
            .road
            .as_ref()
            .map_or(0, |road| u64::from(road.fund.get(good)));
        in_settlements + in_escrow + in_road_fund
    }

    /// The regional gold total: Σ over settlements of their (closed) gold balance,
    /// plus any gold in route escrow.
    fn regional_gold(&self) -> u64 {
        let in_settlements: u64 = self.settlements.iter().map(|s| s.total_gold().0).sum();
        in_settlements + self.caravan.escrow_gold
    }

    // ---- accessors -----------------------------------------------------

    /// The realized spot price for `good` at the settlement with index
    /// `settlement` (0 = A, 1 = B), or `None` if no trade in that good has
    /// cleared there yet.
    pub fn realized_price(&self, settlement: usize, good: GoodId) -> Option<Gold> {
        self.settlements.get(settlement)?.realized_price(good)
    }

    /// The realized-price gap `|price_A(good) − price_B(good)|`, or `None` if
    /// either settlement has not yet cleared a trade in `good`. The convergence
    /// observable — sign only, no magnitude is pinned.
    pub fn price_gap(&self, good: GoodId) -> Option<u64> {
        let a = self.realized_price(0, good)?.0;
        let b = self.realized_price(1, good)?.0;
        Some(a.abs_diff(b))
    }

    /// The traded good.
    pub fn traded_good(&self) -> GoodId {
        self.good
    }

    /// Whether the caravan operates (the no-caravan control has this `false`).
    pub fn caravans_enabled(&self) -> bool {
        self.caravans_enabled
    }

    /// Units of the traded good currently in route escrow (mid-transit).
    pub fn escrow_good(&self) -> u32 {
        self.caravan.escrow_good
    }

    /// Gold currently in route escrow (mid-transit).
    pub fn escrow_gold(&self) -> u64 {
        self.caravan.escrow_gold
    }

    // ---- G7 road / public-works surface --------------------------------

    /// Whether this region has a road public-works project (built or building). The
    /// no-road control has `false`.
    pub fn has_road(&self) -> bool {
        self.road.is_some()
    }

    /// The route's CURRENT transit cost (econ ticks per leg). The road's effect cuts
    /// this on completion; the convergence observable surfaces it alongside the gap.
    pub fn route_transit_ticks(&self) -> u32 {
        self.route.transit_ticks
    }

    /// Total labor the road requires before it is built, or `None` for a no-road
    /// region — the denominator of the build-progress surface.
    pub fn road_labor_cost(&self) -> Option<u32> {
        self.road.as_ref().map(|road| road.template.required_labor)
    }

    /// Labor the community has contributed to the road so far (clamped to the cost),
    /// or `None` for a no-road region — the build-progress numerator.
    pub fn road_labor_advanced(&self) -> Option<u32> {
        self.road.as_ref().map(|road| road.project.labor_advanced)
    }

    /// Whether the road is built (its transit cut has fired). `false` while building
    /// and for a no-road region.
    pub fn road_complete(&self) -> bool {
        self.road
            .as_ref()
            .is_some_and(|road| road.project.state == ProjectState::Complete)
    }

    /// The econ tick the road completed (its transit cut fired), or `None` if it has
    /// not (yet) — and for a no-road region. The one-way completion stamp.
    pub fn road_completed_at(&self) -> Option<u64> {
        self.road.as_ref().and_then(|road| road.completed_at)
    }

    /// Conserved community materials of `good` remaining in the road fund (drawn down
    /// as the road is built). `0` for a no-road region or an untracked good.
    pub fn road_fund_of(&self, good: GoodId) -> u64 {
        self.road
            .as_ref()
            .map_or(0, |road| u64::from(road.fund.get(good)))
    }

    /// Read-only access to a settlement by index.
    pub fn settlement(&self, index: usize) -> Option<&Settlement> {
        self.settlements.get(index)
    }

    /// The settlements, in index order (0 = A, 1 = B).
    pub fn settlements(&self) -> &[Settlement] {
        &self.settlements
    }

    /// The number of settlements (two in G2c).
    pub fn settlement_count(&self) -> usize {
        self.settlements.len()
    }

    /// The goods tracked for region-wide conservation (`GoodId`-ordered).
    pub fn tracked_goods(&self) -> &[GoodId] {
        &self.goods
    }

    /// The most recent region econ tick's report.
    pub fn last_report(&self) -> &RegionTickReport {
        &self.last_report
    }

    /// Completed region econ ticks.
    pub fn econ_tick_count(&self) -> u64 {
        self.econ_tick
    }

    /// The resident-trader id at settlement `index` (its caravan trader), if any.
    pub fn trader_id(&self, index: usize) -> Option<AgentId> {
        match index {
            0 => Some(self.caravan.trader_a),
            1 => Some(self.caravan.trader_b),
            _ => None,
        }
    }

    // ---- determinism surface -------------------------------------------

    /// A canonical, order-stable byte serialization of the whole region — each
    /// settlement's canonical bytes plus the caravan phase, counter, and route
    /// escrow. Two regions are byte-identical iff these are equal (the determinism
    /// tripwire).
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.econ_tick.to_le_bytes());
        out.extend_from_slice(&(self.settlements.len() as u32).to_le_bytes());
        for settlement in &self.settlements {
            let bytes = settlement.canonical_bytes();
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(&bytes);
        }
        // Config-derived parameters that steer future ticks but are not otherwise
        // captured by the dynamic state below, so non-equivalent fresh regions
        // never digest equal merely because they have not diverged yet.
        out.extend_from_slice(&self.good.0.to_le_bytes());
        out.extend_from_slice(&self.route.transit_ticks.to_le_bytes());
        out.extend_from_slice(&self.buy_ticks.to_le_bytes());
        out.extend_from_slice(&self.sell_ticks.to_le_bytes());
        out.push(u8::from(self.caravans_enabled));

        // The caravan operator state that steers future ticks.
        out.push(match self.caravan.phase {
            Phase::BuyA => 0,
            Phase::ToB => 1,
            Phase::SellB => 2,
            Phase::ToA => 3,
            Phase::Hold => 4,
        });
        out.extend_from_slice(&self.caravan.counter.to_le_bytes());
        out.extend_from_slice(&self.caravan.escrow_good.to_le_bytes());
        out.extend_from_slice(&self.caravan.escrow_gold.to_le_bytes());

        // G7 road public-works state that steers future ticks. The transit cut itself
        // already shows through `route.transit_ticks` above; this captures the build
        // progress, the one-way completion, and the fund draw-down so a building road
        // never digests equal to its no-road control. `None` emits nothing: every G2c
        // no-road region keeps its pre-G7 serialization byte-for-byte.
        if let Some(road) = &self.road {
            out.push(1);
            out.extend_from_slice(&road.project.labor_advanced.to_le_bytes());
            out.extend_from_slice(&road.template.required_labor.to_le_bytes());
            out.push(match road.project.state {
                ProjectState::Forming => 0,
                ProjectState::Complete => 1,
                ProjectState::Abandoned => 2,
            });
            out.extend_from_slice(&road.material.0.to_le_bytes());
            out.extend_from_slice(&road.material_per_labor.to_le_bytes());
            out.extend_from_slice(&road.labor_per_colonist.to_le_bytes());
            out.extend_from_slice(&road.transit_after.to_le_bytes());
            out.extend_from_slice(&road.fund.get(road.material).to_le_bytes());
            // Completion tick: `u64::MAX` sentinel while still building.
            out.extend_from_slice(&road.completed_at.unwrap_or(u64::MAX).to_le_bytes());
        }
        out
    }

    /// A 64-bit FNV-1a digest of [`Region::canonical_bytes`] — a compact
    /// cross-run determinism check.
    pub fn digest(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in self.canonical_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }
}

/// Build the G7 [`RoadProject`] from a [`RoadPlan`] and the unbuilt [`Route`]: the
/// reused project-labor lifecycle (an `econ` [`Project`] over a labor-only
/// [`ProjectTemplate`]) plus a fund pre-stocked with the conserved community
/// materials the build will draw down. Deterministic; draws no randomness.
fn build_road(plan: &RoadPlan, route: &Route) -> RoadProject {
    assert!(
        plan.labor_cost > 0,
        "a road needs a positive labor cost to be built from"
    );
    assert!(
        plan.transit_after < route.transit_ticks,
        "a road must REDUCE the route transit (transit_after {} is not below the \
         unbuilt route transit {})",
        plan.transit_after,
        route.transit_ticks
    );
    assert_ne!(
        plan.material, GOLD,
        "a road material cannot be GOLD; money is not a physical road input"
    );
    // The fund holds exactly the materials the whole build consumes (a labor-only
    // road, `material_per_labor == 0`, funds nothing). Sized to the material id.
    let mut fund = Stock::new(plan.material.0);
    if plan.material_per_labor > 0 {
        let total = plan
            .labor_cost
            .checked_mul(plan.material_per_labor)
            .expect("road material total must fit in u32");
        fund.add(plan.material, total);
    }
    let template = build_road_template(plan.material, plan.labor_cost);
    let project = start_project(&template, &mut fund, ProjectId(0), Tick(0))
        .expect("a road template commits no inputs at start, so start_project succeeds");
    RoadProject {
        template,
        project,
        fund,
        material: plan.material,
        material_per_labor: plan.material_per_labor,
        labor_per_colonist: plan.labor_per_colonist,
        transit_after: plan.transit_after,
        completed_at: None,
    }
}

/// Set a resident trader's value scale (the `Region`/caravan seam) and release any
/// now-stale resting quotes so its reservations match the new scale. Only ever
/// called on a resident-trader id, never a colonist.
fn set_trader_scale(settlement: &mut Settlement, trader: AgentId, scale: Vec<Want>) {
    let society = settlement.society_mut();
    if let Some(agent) = society.agents.get_mut(trader) {
        agent.scale = scale;
    }
    society.cancel_changed_live_quotes_for_agent(trader);
}

/// Idle a resident trader: an empty scale posts no orders, and cancelling its
/// quotes releases every reservation so its wealth can be withdrawn safely.
fn idle_trader(settlement: &mut Settlement, trader: AgentId) {
    set_trader_scale(settlement, trader, Vec::new());
}

/// Withdraw the trader's entire holding of `good` into the caller (route escrow),
/// returning the amount moved. Net-zero against the escrow credit the caller does.
///
/// G2c hauls **exactly one** good, so a trader only ever bids for `good` (or asks
/// GOLD) and never holds another tracked good. The single-good route escrow has no
/// slot for any other good; were a future multi-good engine to leave one in a
/// trader's inventory, this withdrawal would silently strand it (still conserved at
/// the settlement, but never hauled). The debug-assert pins that invariant so the
/// stranding fails loudly here rather than passing silently — generalizing escrow
/// to many goods is explicitly later work, not G2c.
fn withdraw_all_stock(settlement: &mut Settlement, trader: AgentId, good: GoodId) -> u32 {
    debug_assert!(
        settlement
            .society()
            .agents
            .get(trader)
            .map(|a| settlement
                .tracked_goods()
                .iter()
                .all(|&g| g == good || a.stock.get(g) == 0))
            .unwrap_or(true),
        "a G2c caravan trader held a tracked good other than the single traded good \
         {good:?}; the single-good route escrow would strand it"
    );
    let qty = settlement
        .society()
        .agents
        .get(trader)
        .map_or(0, |a| a.stock.get(good));
    if qty > 0 {
        let ok = settlement.society_mut().debit_stock(trader, good, qty);
        assert!(ok, "an idled resident trader's stock must debit cleanly");
    }
    qty
}

/// Withdraw the trader's entire gold into the caller (route escrow), returning the
/// amount moved. Net-zero against the escrow credit the caller does.
fn withdraw_all_gold(settlement: &mut Settlement, trader: AgentId) -> u64 {
    let amount = settlement
        .society()
        .agents
        .get(trader)
        .map_or(Gold::ZERO, |a| a.gold);
    if amount.0 > 0 {
        let ok = settlement.society_mut().debit_gold(trader, amount);
        assert!(ok, "an idled resident trader's gold must debit cleanly");
    }
    amount.0
}

/// Whether the configured A→B haul is favourable given the two realized prices:
/// A (the buyer side) must be no dearer than B (the seller side). Unknown prices
/// (no trade has cleared in that settlement yet) count as favourable, so the
/// caravan opens its market rather than stall before any price exists. The shared
/// buy-low/sell-high gate used at generation and at every fresh-haul boundary.
fn haul_is_favourable(price_a: Option<Gold>, price_b: Option<Gold>) -> bool {
    match (price_a, price_b) {
        (Some(a), Some(b)) => a <= b,
        _ => true,
    }
}

/// The buyer's value scale: a ladder of `Want{Good(good), Next}` units. `Next`
/// (not `Now`) makes the trader **acquire and hold** the good rather than consume
/// it — it bids for the good while it holds fewer units than the ladder is long,
/// and the held units are reserved (never eaten), so the caravan can carry them.
fn buy_scale(good: GoodId) -> Vec<Want> {
    (0..RegionConfig::BUY_LADDER_UNITS)
        .map(|_| Want {
            kind: WantKind::Good(good),
            horizon: Horizon::Next,
            qty: 1,
            satisfied: false,
        })
        .collect()
}

/// The seller's value scale: a ladder of `Want{Good(GOLD), Later}` units — a
/// standing money demand, exactly the mechanism that makes a gatherer sell its
/// surplus. With unmet money wants and the good in stock the trader asks the good
/// for gold (cheaply, since one coin satisfies the next want), so it sells what
/// the caravan handed it.
fn sell_scale() -> Vec<Want> {
    (0..RegionConfig::SELL_LADDER_UNITS)
        .map(|_| Want {
            kind: WantKind::Good(GOLD),
            horizon: Horizon::Later(4),
            qty: 1,
            satisfied: false,
        })
        .collect()
}
