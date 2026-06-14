//! The `Settlement` orchestrator — the G2b two-rate loop and the world→econ
//! delivery-escrow seam for one settlement.
//!
//! A `Settlement` owns a [`World`], a per-colonist [`NeedState`] /
//! [`CultureParams`], and an [`econ::Society`], and advances them with
//! [`Settlement::econ_tick`]. Each econ tick runs the documented two-rate order
//! (game-spec §4.3):
//!
//! 1. **FAST** — run the `world` for [`FAST_TICKS_PER_ECON_TICK`] ticks
//!    (movement, harvest node→carry, deposit carry→exchange stockpile). No money
//!    moves. Dead colonists are frozen (idled), so they deliver nothing — their
//!    carried goods stay escrowed, never destroyed.
//! 2. **TRANSFER** — for each delivered exchange unit awaiting credit, *credit
//!    the depositing colonist's econ stock* and then *withdraw it from the world*
//!    (net-zero, conserved, recorded). A unit that cannot be credited stays
//!    world-owned in the exchange stockpile, never destroyed: a live depositor at
//!    its stock ceiling is retried on later ticks, while a **removed** (dead)
//!    depositor is rejected for good (G4a frees it; any such pending unit it left
//!    stays conserved in the stockpile).
//! 3. **NEEDS** — advance each living colonist's [`NeedState`] from the last econ
//!    tick's realized consumption + labor; apply starvation deaths as real removal
//!    (G4a), settling each estate to the commons and idling the dead in the world.
//! 4. **SCALES** — [`regenerate_scale`] for every living colonist, then cancel
//!    now-stale resting quotes (as G1 does).
//! 5. **MARKET** — [`Society::step`], the unchanged econ clearing. Money moves
//!    here only.
//! 6. **READ-BACK** — consumption is read at the top of the next tick's NEEDS.
//! 7. **ASSIGN** — idle gatherers get their next task (harvest → exchange);
//!    handled inline in the fast loop so a gatherer is never idle for a tick.
//!
//! ## The division of labor
//!
//! - **gatherers** harvest FOOD from a node and haul it to the exchange; the
//!   transfer credits the hauled FOOD to their econ stock; they sell it and buy
//!   the warmth good (WOOD) to keep their gold circulating.
//! - **consumers** sit at the exchange; they sell their WOOD endowment and buy
//!   FOOD, consuming it (their need/scale loop drives their bids).
//!
//! Consumers carry the **lower** ids, so their FOOD bids rest in the book first
//! and a gatherer's crossing ask prints at the resting bid — making the realized
//! FOOD price track the buyers' willingness to pay, which climbs when distance
//! starves the supply. That is the distance→price mechanism, sign only.
//!
//! ## Conservation
//!
//! For every physical good the whole-system total — node + carry + exchange
//! stockpile (all `world`) + econ stock — changes per econ tick by **exactly**
//! `+regen − consumed`. Harvest (node→carry), deposit (carry→stockpile), and the
//! transfer (stockpile→econ) are all relocations: net-zero. Node regen is the
//! only source, consumption the only sink. [`Settlement::econ_tick`] checks this
//! every tick and returns it in the [`EconTickReport`]; FOOD is the spatial good
//! (source = its node's regen), WOOD a closed provisioning good (source none,
//! sink consumption) that recirculates gold and keeps the market liquid.
//!
//! Money (GOLD) is a **closed** balance: no settlement path mints or burns it,
//! so the fast loop never moves money and [`Society::step`] only redistributes a
//! conserved total between colonists (the §4.3 rule; the report's gold
//! checkpoints are the proof).

use std::collections::BTreeMap;

use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::bundle::{
    appraise_project_bundle_for_money, ProjectBundleCandidate, ProjectBundleEndowment,
};
use econ::capital::ProjectLineId;
use econ::expect::PriceBelief;
use econ::good::{Gold, GoodId, Horizon, Stock, FOOD, GOLD, NET, WOOD};
use econ::money::{DesignatedMoney, MarketMoneyConfig};
use econ::project::{Recipe, RecipeId, Tick};
use econ::purpose::ProjectPlanId;
use econ::rng::Rng;
use econ::scenario::{MarketScenario, ScenarioName};
use econ::society::Society;

use life::{regenerate_scale, CultureParams, KnownGoods, NeedDynamics, NeedIntake, NeedState};

use world::{AgentStatus, Grid, NodeId, Pos, ResourceNode, Stockpile, StockpileId, Task, World};

use crate::content::ContentSet;

/// Fast `world` ticks per economic tick — the two-rate ratio (game-spec §4.1).
/// A gatherer's round trip to a node costs `2 × distance` fast ticks, so a node
/// far from the exchange completes fewer trips inside this fixed budget and
/// delivers fewer units per econ tick. Holding this fixed while varying distance
/// is exactly the distance→price experiment.
pub const FAST_TICKS_PER_ECON_TICK: u64 = 24;

/// Econ ticks per settlement "year" — the horizon unit the smoke test counts in.
/// A placeholder cadence, not a balance figure.
pub const ECON_TICKS_PER_YEAR: u64 = 12;

/// Upper bound on [`ChainConfig::throughput`], checked at generation. A producer's
/// `throughput` becomes that many unit input wants appended to its value scale every
/// scale regeneration (see [`producer_scale_extension`]), so an unbounded throughput
/// would let a config drive the per-producer scale — and thus the market it iterates
/// — to an arbitrary size (an out-of-memory vector at the extreme). Real mechanism
/// configs use `1`/`2` (the CDA market clears one unit per seller per good per tick),
/// so this generous ceiling rejects only absurd values; it is a sanity bound, not a
/// balance figure.
pub const MAX_CHAIN_THROUGHPUT: u32 = 1_024;

/// A colonist's role in the settlement's minimal division of labor.
///
/// G2b has only [`Gatherer`](Vocation::Gatherer)/[`Consumer`](Vocation::Consumer).
/// G3a adds the two **producer** vocations
/// ([`Miller`](Vocation::Miller)/[`Baker`](Vocation::Baker)) that run the
/// grain→flour→bread chain. In G3a they are *seeded* (hand-placed); G3b adds the
/// [`Unassigned`](Vocation::Unassigned) vocation — a colonist holding latent
/// production capital (a mill or an oven) that has **not** chosen to produce. Each
/// econ tick an unassigned colonist appraises the recipe it could run against the
/// realized price spread and its own value scale, and *adopts* the producer
/// vocation (or reverts to `Unassigned`) accordingly — entrepreneurship from
/// prices, not seeding. A plain settlement has none of the chain vocations, so its
/// config and digest stay byte-identical to G2b.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vocation {
    /// Harvests its node's good (FOOD in G2b, grain in the G3a chain) and hauls
    /// it to the exchange; sells the haul, buys what it needs.
    Gatherer,
    /// Sits at the exchange; sells its provisioning endowment, buys and eats the
    /// staple (FOOD in G2b, bread in the G3a chain).
    Consumer,
    /// Producer: holds a **mill** (durable tool) and, in the production phase,
    /// mills grain it holds into flour, then sells the flour. Seeded in G3a,
    /// **adopted from the spread** in G3b (see [`Vocation::Unassigned`]).
    Miller,
    /// Producer: holds an **oven** (durable tool) and, in the production phase,
    /// bakes flour it holds into bread, eats some, and sells the rest. Seeded in
    /// G3a, **adopted from the spread** in G3b.
    Baker,
    /// G3b: a colonist with **latent** production capital (a mill or an oven) that
    /// has not (yet) chosen to produce. It sits at the exchange and trades like a
    /// consumer, but each tick re-appraises the recipe its tool could run; when
    /// the realized spread pays on its own value scale it adopts
    /// [`Miller`](Vocation::Miller)/[`Baker`](Vocation::Baker), and it reverts here
    /// when the spread collapses. The latent specialty (which recipe) is the
    /// colonist's [`latent`](Colonist::latent) recipe.
    Unassigned,
}

impl Vocation {
    /// A stable serialization tag for [`Settlement::canonical_bytes`]. Consumer
    /// and Gatherer keep the values G2b's `u8::from(== Gatherer)` produced
    /// (`0`/`1`), so every pre-G3a digest is byte-identical; the producers extend
    /// the space with `2`/`3` and the G3b `Unassigned` vocation with `4`.
    fn tag(self) -> u8 {
        match self {
            Vocation::Consumer => 0,
            Vocation::Gatherer => 1,
            Vocation::Miller => 2,
            Vocation::Baker => 3,
            Vocation::Unassigned => 4,
        }
    }
}

/// The endowment of a **resident trader** — a permanent econ agent the `Region`
/// (G2c caravans) adds to a settlement at generation, beyond the colonist roster.
///
/// A resident trader is one half of a caravan's permanent trader *pair* (the
/// other lives in the linked settlement): it is an `econ::Society` agent the
/// settlement does **not** itself manage — it has no [`Vocation`], no
/// [`NeedState`], is never removed, and the settlement's per-econ-tick phases
/// (needs, scales, tasks) skip it entirely. The `Region` owns its value scale and
/// shuttles its wealth as caravan route escrow. Created at generation so no agent
/// is ever added to or removed from a `Society` at runtime (the G4-deferred
/// roster mutation). A plain settlement has none, so every G2b config and golden
/// is byte-identical.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraderEndowment {
    /// Working-capital gold the trader starts with (its initial buying power).
    pub gold: u64,
    /// Initial physical stock, as `(good, qty)` pairs. Every good here is tracked
    /// for whole-system conservation (it joins `self.goods`), so a trader cannot
    /// hold an untracked good. GOLD (money) is rejected: it is not a physical good.
    pub stock: Vec<(GoodId, u32)>,
}

/// A resource node to place: a good, a tile, and its stock/regen/cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeSpec {
    pub good: GoodId,
    pub pos: Pos,
    pub stock: u32,
    pub regen: u32,
    pub cap: u32,
}

/// The G3a **production chain** overlay on a settlement (the seeded
/// grain→flour→bread chain). `None` on a plain G2b/G2c settlement, so every such
/// config and the six econ goldens stay byte-identical by construction; `Some`
/// turns the settlement into a chain economy where **bread is the staple**
/// (`hunger ↔ bread`), grain is the gathered raw good, and the millers/bakers
/// transform it.
///
/// Roles are **seeded** (hand-placed): the gatherers ([`SettlementConfig::gatherers`])
/// harvest the grain node, the [`millers`](ChainConfig::millers) hold mills and
/// the [`bakers`](ChainConfig::bakers) hold ovens, and the
/// [`consumers`](SettlementConfig::consumers) eat bread. No emergence of
/// who-produces-what (that is G3b). The buffers are generous *mechanism* knobs:
/// they bridge the pipeline fill and keep the smoke horizon collapse-free; they
/// pin no magnitude.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainConfig {
    /// The interned chain goods and recipes (built once at generation).
    pub content: ContentSet,
    /// Seeded millers (hold a mill, mill grain → flour). G3a (seeded roles);
    /// `0` for the G3b emergent configs (millers *adopt* from the spread instead).
    pub millers: u16,
    /// Seeded bakers (hold an oven, bake flour → bread). `0` for G3b emergent.
    pub bakers: u16,
    /// G3b: colonists seeded with a **latent mill** that start
    /// [`Unassigned`](Vocation::Unassigned) and adopt [`Miller`](Vocation::Miller)
    /// only when the realized flour−grain spread pays on their own value scale.
    /// `0` for G3a (seeded roles, no emergence).
    pub latent_millers: u16,
    /// G3b: colonists seeded with a **latent oven** that adopt
    /// [`Baker`](Vocation::Baker) from the realized bread−flour spread. `0` for G3a.
    pub latent_bakers: u16,
    /// G3b: the per-operation cost (labor leisure + tool) a recipe's realized
    /// output spread must clear before an unassigned colonist adopts it, so a
    /// yield-3 recipe is not unconditionally worth running. A mechanism knob
    /// (must be ≥ 1), not a magnitude.
    pub operating_cost: u64,
    /// G3b: whether **bread** is the staple (`hunger ↔ bread`, the demand that pulls
    /// the chain) or hunger maps to the gathered node good (`hunger ↔ FOOD`). The
    /// falsification control sets this `false`: with no bread demand the chain's
    /// goods never price, so the same role-choice appraisal forms no roles. G3a and
    /// the emergent config set it `true`.
    pub bread_is_staple: bool,
    /// Per-producer, per-econ-tick cap on recipe applications — a deterministic
    /// throughput bound (nothing is drawn). A producer applies its recipe up to
    /// this many times, limited by the input it holds.
    pub throughput: u32,
    /// Grain a miller is seeded holding (a buffer so milling fires before the
    /// market routes the first grain to it; the market then replenishes it).
    pub miller_grain_buffer: u32,
    /// Flour a baker is seeded holding (a buffer so baking fires from tick 1).
    pub baker_flour_buffer: u32,
    /// G3b: flour a **latent miller** is seeded holding as bootstrap output stock.
    /// A latent miller does not reserve flour (flour is its output, not its input),
    /// so it offers this stock for sale; that is the flour supply the first adopted
    /// baker buys, which gives flour a realized price — the signal a latent miller
    /// then adopts milling on. `0` for G3a (no latent millers).
    pub latent_flour_seed: u32,
    /// Bread every colonist is seeded holding — the staple buffer that bridges
    /// the pipeline fill and keeps hunger bounded over the smoke horizon. In G3b's
    /// emergent config this is the *surplus* a non-consumer carries (so it offers
    /// bread, bootstrapping the bread price the chain forms from).
    pub bread_buffer: u32,
    /// Staple (bread) a **consumer** is seeded holding — kept small in the G3b
    /// emergent config so consumers run short and *buy* bread early, which is what
    /// gives bread a realized price (the demand that pulls the chain into being). In
    /// G3a it equals `bread_buffer` (consumers are not the demand bootstrap there),
    /// so the seeded config is unchanged.
    pub consumer_staple_buffer: u32,
    /// WOOD every colonist is seeded holding — a warmth battery. Warmth never
    /// kills (only hunger does), so this just keeps the warmth need low/bounded.
    pub wood_buffer: u32,
    /// Working gold a producer (miller/baker) starts with — capital to buy its
    /// input while it sells its output.
    pub producer_gold: u64,
}

impl ChainConfig {
    /// The default grain→flour→bread chain content with seeded buffers tuned so a
    /// modest roster runs the chain collapse-free over the smoke horizon.
    pub fn grain_flour_bread() -> Self {
        Self {
            content: ContentSet::grain_flour_bread(),
            // The roster is producer-heavy because the market clears one unit per
            // seller per good per tick: a stage's bread/flour throughput is capped
            // by its seller count, so enough millers/bakers keep the staple
            // flowing to the mouths. Seeded (hand-placed) — no role emergence.
            millers: 3,
            bakers: 5,
            // G3a seeds the producer roles; there is no emergence here, so the
            // latent pool is empty and the role-choice phase is a no-op.
            latent_millers: 0,
            latent_bakers: 0,
            operating_cost: 1,
            bread_is_staple: true,
            throughput: 2,
            miller_grain_buffer: 16,
            baker_flour_buffer: 16,
            // No latent millers in G3a, so no bootstrap flour stock.
            latent_flour_seed: 0,
            // A modest staple buffer: large enough to bridge the pipeline fill,
            // small enough that consumers re-enter the bread market once it
            // drains (so bread realizes a price too), and the chain's surplus
            // keeps hunger bounded over the smoke horizon.
            bread_buffer: 24,
            // G3a consumers carry the same staple buffer as everyone else (the
            // seeded roster does not bootstrap demand from the consumers), so the
            // G3a config and its goldens are unchanged.
            consumer_staple_buffer: 24,
            wood_buffer: 48,
            producer_gold: 24,
        }
    }
}

/// The settlement recipe: geometry (grid, exchange, FOOD nodes), the
/// gatherer/consumer rosters, and the economic knobs. Mechanism knobs, not
/// balance targets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettlementConfig {
    pub width: u16,
    pub height: u16,
    /// Where the exchange stockpile sits; every colonist starts here.
    pub exchange: Pos,
    /// Exchange stockpile capacity — generous, since normal transfers drain it
    /// each econ tick (escrow comes from travel time, not overflow).
    pub exchange_cap: u32,
    /// FOOD nodes the gatherers harvest (assigned round-robin by gatherer index).
    pub nodes: Vec<NodeSpec>,
    pub gatherers: u16,
    pub consumers: u16,
    pub carry_cap: u32,
    pub move_speed: u16,
    pub starting_gold_gatherer: u64,
    pub starting_gold_consumer: u64,
    /// FOOD a gatherer starts with (a buffer to eat while the first hauls land).
    pub gatherer_food_buffer: u32,
    /// WOOD a gatherer starts with (a small warmth buffer).
    pub gatherer_wood_buffer: u32,
    /// FOOD a consumer starts with (a buffer to eat while the market warms up).
    pub consumer_food_buffer: u32,
    /// WOOD a consumer is endowed with — the closed provisioning battery it
    /// sells for gold and burns for warmth.
    pub consumer_wood_endowment: u32,
    /// Gatherers are present-biased (high bps) so they keep selling their haul
    /// to refill a small gold target, circulating gold by buying WOOD.
    pub gatherer_time_preference_base_bps: u16,
    /// Consumers are patient (low bps) so unspent gold accumulates when FOOD is
    /// scarce, lifting their bids — the price's scarcity response.
    pub consumer_time_preference_base_bps: u16,
    pub leisure_weight_base_bps: u16,
    pub dynamics: NeedDynamics,
    /// Permanent **resident traders** (G2c caravans), one econ agent each, added
    /// at generation **before** the colonist roster (taking the **lowest** ids, so
    /// they lead the id-ordered market as the price-setting makers). Empty for
    /// every plain settlement, so the existing configs and the six econ goldens are
    /// byte-identical by construction. The `Region` populates this (one trader per
    /// linked settlement) and manages the agents; see [`TraderEndowment`].
    pub resident_traders: Vec<TraderEndowment>,
    /// The G3a production chain, or `None` for a plain G2b/G2c settlement. `None`
    /// keeps every existing config and the six econ goldens byte-identical (every
    /// chain code path is skipped); `Some` seeds the grain→flour→bread chain (the
    /// node good is grain, the staple is bread, and millers/bakers transform it).
    /// See [`ChainConfig`] and [`SettlementConfig::grain_flour_bread_chain`].
    pub chain: Option<ChainConfig>,
}

impl SettlementConfig {
    /// A viable single-FOOD-node settlement: gatherers haul FOOD from a node a
    /// short distance east of the exchange; consumers sit at the exchange and
    /// trade their WOOD battery for FOOD.
    /// Patient colonists keep offering their surplus so the market clears and the
    /// settlement runs without collapse. Move the node with
    /// [`Self::with_food_node_distance`] for the distance experiment.
    pub fn viable() -> Self {
        let exchange = Pos::new(0, 0);
        Self {
            width: 64,
            height: 1,
            exchange,
            exchange_cap: 1_000_000,
            nodes: vec![NodeSpec {
                good: FOOD,
                pos: Pos::new(4, 0),
                stock: 4_000,
                regen: 16,
                cap: 4_000,
            }],
            // Supply-rich (more sellers than buyers) so the qty-1 book keeps the
            // buyers reliably fed, and the gold loop is **closed**, so it
            // circulates instead of pooling in the sellers — both make the
            // settlement sustain its colonists indefinitely over the smoke-test
            // horizon.
            gatherers: 8,
            consumers: 4,
            carry_cap: 2,
            move_speed: 1,
            starting_gold_gatherer: 4,
            starting_gold_consumer: 12,
            gatherer_food_buffer: 8,
            gatherer_wood_buffer: 6,
            consumer_food_buffer: 8,
            consumer_wood_endowment: 600,
            // Patient on both sides (low bps): sellers keep offering their haul
            // instead of sating a tiny gold target and hoarding, so food keeps
            // reaching the buyers and the settlement sustains.
            gatherer_time_preference_base_bps: 500,
            consumer_time_preference_base_bps: 500,
            leisure_weight_base_bps: 3_000,
            dynamics: NeedDynamics::lab_default(),
            // A plain settlement has no resident traders; the `Region` adds them
            // for caravans (G2c). Empty here keeps every G2b config and the six
            // econ goldens byte-identical.
            resident_traders: Vec::new(),
            // No production chain by default — a plain G2b settlement. The chain
            // is opt-in via `grain_flour_bread_chain`, so `viable`/`price_probe`/
            // `starved_hauler` and every golden stay byte-identical.
            chain: None,
        }
    }

    /// A viable G3a **production-chain** settlement: a grain node a short distance
    /// east of the exchange, grain gatherers hauling grain, seeded millers
    /// (grain → flour) and bakers (flour → bread), and bread consumers. Bread is
    /// the staple (`hunger ↔ bread`); WOOD is the closed warmth battery as in
    /// [`Self::viable`]. The chain operates end-to-end and conserves; the buffers
    /// are sized so it runs collapse-free over the smoke horizon. Mechanism, not
    /// balance.
    pub fn grain_flour_bread_chain() -> Self {
        let chain = ChainConfig::grain_flour_bread();
        let exchange = Pos::new(0, 0);
        Self {
            width: 64,
            height: 1,
            exchange,
            exchange_cap: 1_000_000,
            // The single raw node yields GRAIN (not FOOD): grain is the only good
            // a world node produces in the chain; flour and bread are recipe
            // outputs. Rich + close so grain supply stays loose.
            nodes: vec![NodeSpec {
                good: chain.content.grain(),
                pos: Pos::new(4, 0),
                stock: 8_000,
                regen: 24,
                cap: 8_000,
            }],
            gatherers: 2,
            consumers: 1,
            carry_cap: 2,
            move_speed: 1,
            starting_gold_gatherer: 12,
            starting_gold_consumer: 24,
            // These FOOD-buffer knobs are unused on the chain path (the staple is
            // bread, seeded via `ChainConfig::bread_buffer`); kept at viable()'s
            // values so the config reads consistently.
            gatherer_food_buffer: 0,
            gatherer_wood_buffer: 0,
            consumer_food_buffer: 0,
            consumer_wood_endowment: 0,
            // Patient on both sides so surplus keeps being offered and the chain's
            // intermediate goods keep clearing (the same discipline as viable()).
            gatherer_time_preference_base_bps: 500,
            consumer_time_preference_base_bps: 500,
            leisure_weight_base_bps: 3_000,
            dynamics: NeedDynamics::lab_default(),
            resident_traders: Vec::new(),
            chain: Some(chain),
        }
    }

    /// The G3b **emergent production-chain** settlement: the grain→flour→bread chain
    /// with **no seeded producer roles**. Instead a pool of latent millers (each
    /// holding a mill) and latent bakers (each holding an oven) start
    /// [`Unassigned`](Vocation::Unassigned) and *choose* to produce when the realized
    /// price spread pays on their own value scale (the role-choice appraisal). Bread
    /// is the staple, so consumer demand prices bread; that pulls the chain into
    /// existence bottom-up — a baker adopts on the bread−flour spread and starts
    /// buying flour, which prices flour, which makes a miller adopt on the
    /// flour−grain spread, which prices grain. Generous buffers bridge the pipeline
    /// fill; mechanism, not balance.
    pub fn emergent_chain() -> Self {
        Self::emergent_chain_with_demand(true)
    }

    /// The G3b **no-spread falsification control**: the same emergent world with the
    /// chain's demand removed. Hunger maps to FOOD from seeded buffers instead of
    /// bread (`bread_is_staple = false`), so **no one ever demands bread**; bread and
    /// flour never trade, so they never realize a price, so the *same* role-choice
    /// appraisal — run over the *same* latent pool and grain node every tick — never
    /// sees a spread and **forms no producer roles**, and no flour or bread is ever
    /// produced. Paired with [`Self::emergent_chain`] this isolates the spread as
    /// the cause of the roles: identical machinery and raw input supply, demand the
    /// only causal difference.
    pub fn emergent_chain_control() -> Self {
        Self::emergent_chain_with_demand(false)
    }

    /// Shared builder for the emergent chain and its no-spread control. `bread_demand`
    /// selects the staple (bread, the chain's product → demand pulls the chain; or
    /// FOOD from seeded buffers → bread is never demanded). Both twins keep the same
    /// grain node, so the control removes only the bread demand/spread rather than
    /// the chain's raw input supply.
    fn emergent_chain_with_demand(bread_demand: bool) -> Self {
        let mut chain = ChainConfig::grain_flour_bread();
        // No seeded roles — the producer mix must *emerge* from the spread.
        chain.millers = 0;
        chain.bakers = 0;
        // A latent pool for each stage, so when both spreads exist the chain forms
        // both roles (and when neither does — the control — it forms none).
        chain.latent_millers = 3;
        chain.latent_bakers = 3;
        chain.operating_cost = 1;
        chain.bread_is_staple = bread_demand;
        // One operation per producer per tick, matching the CDA market's one-unit-
        // per-seller-per-tick granularity: an adopted producer buys one input and
        // mills/bakes it each tick, so it keeps spending gold on inputs (its savings
        // want stays unprovisioned, so it does not "retire" the moment it earns) and
        // its input good keeps clearing a price. Producers start with no input buffer
        // — they buy it from the market each tick — except the latent millers, which
        // carry a flour bootstrap stock so the first baker's flour bid finds a seller.
        chain.throughput = 1;
        chain.miller_grain_buffer = 0;
        chain.baker_flour_buffer = 0;
        chain.latent_flour_seed = 12;
        // In the emergent run this is the bread surplus that bootstraps early bread
        // trades. In the no-spread control the same field seeds FOOD instead; keep
        // it ample so the control removes bread demand without turning starvation
        // into a second causal difference.
        chain.bread_buffer = if bread_demand { 24 } else { 80 };
        // Consumers start nearly bread-empty so they buy bread within the first few
        // ticks — that demand is what gives bread a realized price, the spread the
        // first baker adopts on. In the control this seeds FOOD instead, and is
        // intentionally ample: no one needs bread, but the latent pool stays alive
        // while repeatedly declining the absent bread/flour spread.
        chain.consumer_staple_buffer = if bread_demand { 2 } else { 80 };
        chain.wood_buffer = 48;
        // Modest working gold: well below a patient colonist's savings target, so an
        // unprovisioned future-gold want always remains for the appraisal to target
        // (a producer that has already sated its savings would decline new work).
        chain.producer_gold = 12;

        let exchange = Pos::new(0, 0);
        Self {
            width: 64,
            height: 1,
            exchange,
            exchange_cap: 1_000_000,
            nodes: vec![NodeSpec {
                good: chain.content.grain(),
                pos: Pos::new(4, 0),
                stock: 8_000,
                regen: 24,
                cap: 8_000,
            }],
            gatherers: 3,
            // Bread mouths with ample gold: their demand prices bread, the spread
            // that bootstraps the chain in the emergent config. (In the control they
            // eat FOOD, so bread stays unpriced.)
            consumers: 2,
            carry_cap: 2,
            move_speed: 1,
            starting_gold_gatherer: 12,
            starting_gold_consumer: 48,
            gatherer_food_buffer: 0,
            gatherer_wood_buffer: 0,
            consumer_food_buffer: 0,
            consumer_wood_endowment: 0,
            // Patient on both sides so colonists carry a savings want (the
            // entrepreneurial appraisal's target) and keep offering surplus.
            gatherer_time_preference_base_bps: 500,
            consumer_time_preference_base_bps: 500,
            leisure_weight_base_bps: 3_000,
            dynamics: NeedDynamics::lab_default(),
            resident_traders: Vec::new(),
            chain: Some(chain),
        }
    }

    /// A distance→price probe. Two changes from [`Self::viable`] isolate the
    /// supply→price channel for the near/far sign test:
    ///
    /// - enough gatherers for a nearby node to keep supply loose, plus a larger
    ///   initial closed gold balance so scarce far-node supply can lift bids
    ///   without any loop-time money mutation;
    /// - **hunger-resilient** consumers (hunger never reaches the critical
    ///   ceiling) so the market stays demand-heavy and bids up under scarcity
    ///   instead of dying off — the price reflects supply, not a death cascade.
    ///
    /// Both runs use this identical config; only the node distance differs. Sign
    /// only — it pins no magnitude.
    pub fn price_probe() -> Self {
        let mut config = Self::viable();
        config.starting_gold_consumer = 120;
        config.dynamics.hunger_critical = config.dynamics.need_max + 1;
        config
    }

    /// A single gatherer on a very long haul (no market): the node is so far that
    /// the round trip spans many econ ticks, so the gatherer's harvested FOOD
    /// stays locked in **carry** (the world, undeposited) while its small econ
    /// FOOD buffer runs out and it **starves mid-haul**. The escrow-on-death
    /// scenario: when it dies (G4a real removal), its carried goods settle to the
    /// commons (conserved, not destroyed, not transferred to econ).
    pub fn starved_hauler() -> Self {
        let mut config = Self::viable();
        config.width = 320;
        config.gatherers = 1;
        config.consumers = 0;
        config.carry_cap = 4;
        config.gatherer_food_buffer = 2;
        config.gatherer_wood_buffer = 0;
        config.nodes = vec![NodeSpec {
            good: FOOD,
            pos: Pos::new(140, 0),
            stock: 4_000,
            regen: 16,
            cap: 4_000,
        }];
        config
    }

    /// Place the (single) FOOD node `distance` tiles east of the exchange,
    /// holding everything else fixed — the only knob the distance→price test
    /// varies. Panics if there is not exactly one node (the experiment's shape).
    pub fn with_food_node_distance(mut self, distance: u16) -> Self {
        assert_eq!(
            self.nodes.len(),
            1,
            "the distance experiment expects exactly one node"
        );
        let y = self.exchange.y;
        let x = self.exchange.x.saturating_add(distance);
        assert!(x < self.width, "node distance puts the node off the grid");
        self.nodes[0].pos = Pos::new(x, y);
        self
    }

    /// Attach the resident-trader endowments (G2c caravans), replacing any already
    /// set. The `Region` calls this when wiring a settlement into a caravan; a
    /// plain settlement leaves the list empty. Holding everything else fixed.
    pub fn with_resident_traders(mut self, traders: Vec<TraderEndowment>) -> Self {
        self.resident_traders = traders;
        self
    }
}

/// The per-econ-tick conservation + flow receipt. The conservation invariant the
/// G2b DoD pins, for every tracked good:
/// `whole_system_after == whole_system_before + regen − consumed` — the transfer
/// is net-zero and so never appears. The gold checkpoints prove no money moved
/// in the fast loop.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EconTickReport {
    pub econ_tick: u64,
    pub fast_ticks: u64,
    /// Goods created by node regen during the fast loop (the only source).
    pub regen: BTreeMap<GoodId, u64>,
    /// Goods relocated world→econ by the transfer (net-zero for the whole system).
    pub transferred: BTreeMap<GoodId, u64>,
    /// Goods consumed in [`Society::step`] (a sink — eaten).
    pub consumed: BTreeMap<GoodId, u64>,
    /// Goods **produced** by the production phase's recipe applications (G3a) —
    /// the output side of every accounted transformation (e.g. flour, bread).
    pub produced: BTreeMap<GoodId, u64>,
    /// Goods **consumed as a recipe input** by the production phase (G3a) — the
    /// input side of every accounted transformation (e.g. grain milled, flour
    /// baked). Distinct from `consumed` (eaten): an input is *transformed*, not a
    /// final sink. Tools (`required_tool`) are durable and never appear here.
    pub consumed_as_input: BTreeMap<GoodId, u64>,
    /// Whole-system total per good at the start of the econ tick.
    pub whole_system_before: BTreeMap<GoodId, u64>,
    /// Whole-system total per good at the end of the econ tick.
    pub whole_system_after: BTreeMap<GoodId, u64>,
    /// Total money before the fast loop.
    pub total_gold_before_fast: u64,
    /// Total money after the fast loop (must equal `before_fast` — no money in
    /// the fast loop).
    pub total_gold_after_fast: u64,
    /// Total money after [`Society::step`] (a closed balance is conserved).
    pub total_gold_after_step: u64,
    pub deaths: u32,
}

impl EconTickReport {
    pub fn regen_of(&self, good: GoodId) -> u64 {
        self.regen.get(&good).copied().unwrap_or(0)
    }
    pub fn transferred_of(&self, good: GoodId) -> u64 {
        self.transferred.get(&good).copied().unwrap_or(0)
    }
    pub fn consumed_of(&self, good: GoodId) -> u64 {
        self.consumed.get(&good).copied().unwrap_or(0)
    }
    /// Units of `good` produced by recipe applications this tick (G3a).
    pub fn produced_of(&self, good: GoodId) -> u64 {
        self.produced.get(&good).copied().unwrap_or(0)
    }
    /// Units of `good` consumed as a recipe input this tick (G3a).
    pub fn consumed_as_input_of(&self, good: GoodId) -> u64 {
        self.consumed_as_input.get(&good).copied().unwrap_or(0)
    }
    pub fn whole_system_before_of(&self, good: GoodId) -> u64 {
        self.whole_system_before.get(&good).copied().unwrap_or(0)
    }
    pub fn whole_system_after_of(&self, good: GoodId) -> u64 {
        self.whole_system_after.get(&good).copied().unwrap_or(0)
    }

    /// Whether the whole-system ledger balances for every tracked good. This is
    /// the conservation DoD; [`Settlement::econ_tick`] also `debug_assert`s it.
    ///
    /// G2b's invariant was `after == before + regen − consumed` (the transfer
    /// net-zero). G3a **generalizes it across transformations**: a recipe is a
    /// conserved conversion — it consumes an accounted input and produces an
    /// accounted output — so per good X:
    ///
    /// ```text
    /// after(X) == before(X) + regen(X) + produced(X)
    ///                       − consumed_as_input(X) − consumed(X)
    /// ```
    ///
    /// For a plain settlement `produced`/`consumed_as_input` are empty, so this
    /// reduces exactly to the G2b form (every existing test stays green). Tools
    /// are durable — they appear in neither production term, so a recipe that
    /// needs a tool never moves the tool's ledger.
    pub fn conserves(&self) -> bool {
        self.whole_system_before.keys().all(|good| {
            let before = self.whole_system_before_of(*good) as i128;
            let after = self.whole_system_after_of(*good) as i128;
            let regen = self.regen_of(*good) as i128;
            let consumed = self.consumed_of(*good) as i128;
            let produced = self.produced_of(*good) as i128;
            let consumed_as_input = self.consumed_as_input_of(*good) as i128;
            after == before + regen + produced - consumed_as_input - consumed
        })
    }
}

struct Colonist {
    id: AgentId,
    vocation: Vocation,
    /// The node a gatherer harvests (round-robin over the config's nodes).
    node: Option<NodeId>,
    need: NeedState,
    culture: CultureParams,
    critical_streak: u16,
    /// Mirrors real removal (see [`Society::remove_agent`]'s caller contract): set
    /// `false` the tick a colonist dies, checked in every phase so a dead colonist
    /// is never re-scaled, re-credited, re-tasked, or read back. After removal its
    /// id resolves to `None` in the arena; a dead gatherer is idled in the world and
    /// its carry settled to the commons.
    alive: bool,
    /// G3b: the recipe this colonist *could* run with its latent tool, if any.
    /// `Some(Mill)` for a latent miller (holds a mill), `Some(Bake)` for a latent
    /// baker (holds an oven); `None` for a gatherer, consumer, or a **seeded** G3a
    /// producer. The role-choice phase re-appraises this recipe each tick and
    /// toggles [`Vocation::Unassigned`] ↔ the producer vocation from the realized
    /// spread; a `None` colonist is never re-appraised, so the seeded G3a config is
    /// byte-identical (its producers are permanent).
    latent: Option<RecipeId>,
}

/// A settlement of generated colonists driven over a real `world` + `econ`.
pub struct Settlement {
    world: World,
    society: Society,
    colonists: Vec<Colonist>,
    dynamics: NeedDynamics,
    known: KnownGoods,
    exchange: StockpileId,
    carry_cap: u32,
    /// Physical goods tracked for whole-system conservation (node goods ∪ goods
    /// any colonist starts with), `GoodId`-ordered. GOLD (money) is excluded.
    goods: Vec<GoodId>,
    /// Attribution for exchange-stockpile units that were delivered by a
    /// gatherer but have not yet crossed into econ stock. This is not a goods
    /// ledger: the units are counted only in the world stockpile until transfer
    /// succeeds. The map exists solely to retry a clipped credit against the
    /// original depositor once headroom opens.
    pending_deposits: BTreeMap<(AgentId, GoodId), u32>,
    /// The ids of the resident-trader agents (G2c caravans), in generation order
    /// — the agents the settlement does NOT manage (no need/scale/task phase
    /// touches them). The `Region` addresses its caravan trader pair through
    /// these. Empty for a plain settlement.
    trader_ids: Vec<AgentId>,
    /// The numeric id of colonist slot 0 — equal to the resident-trader count,
    /// since traders take the lowest ids. Zero for a plain settlement (colonist
    /// `i` has `AgentId(i)`, exactly as in G2b). Lets [`Settlement::slot_for_id`]
    /// map a colonist id back to its slot in O(1) despite the trader offset.
    colonist_id_base: u64,
    /// The G3a production-chain runtime (content + throughput), or `None` for a
    /// plain settlement. Drives the econ tick's scale-injection and production
    /// phases; `None` skips both, so a plain settlement is byte-identical to G2b.
    chain: Option<ChainRuntime>,
    econ_tick: u64,
    last_report: EconTickReport,
    /// The settlement **commons** (G4a real death): the conserved sink that holds a
    /// dead colonist's settled estate. When a colonist starves, [`Society::remove_agent`]
    /// frees its arena slot and hands back its econ gold + stock, and its world-carried
    /// delivery escrow is drained out of the world — all of it accrues here, nothing
    /// created or destroyed. The commons joins [`Settlement::total_gold`] and
    /// [`Settlement::whole_system_total`] so whole-system conservation holds across the
    /// death. Empty until the first death, so a no-death run is byte-identical to G2b/G3.
    /// G4b will route the estate to heirs/households instead of pooling it here.
    commons_gold: Gold,
    /// The commons' physical-good holdings, `GoodId`-keyed (a subset of
    /// [`Settlement::goods`]). Joins [`Settlement::whole_system_total`].
    commons_stock: BTreeMap<GoodId, u64>,
}

/// The per-settlement production-chain runtime (G3a): the interned content and
/// the per-producer throughput cap. Read-only after generation.
struct ChainRuntime {
    content: ContentSet,
    throughput: u32,
    /// The per-operation cost (labor + tool) the G3b role-choice appraisal charges
    /// against a recipe's realized output spread (see [`ChainConfig::operating_cost`]).
    operating_cost: u64,
}

impl Settlement {
    /// Generate a settlement from `seed` and a [`SettlementConfig`]. All
    /// randomness (per-colonist culture) is drawn here; neither loop draws any.
    /// Deterministic: same `(seed, config)` → byte-identical settlement.
    pub fn generate(seed: u64, config: &SettlementConfig) -> Self {
        assert!(
            config.gatherers == 0 || !config.nodes.is_empty(),
            "a config with gatherers must define at least one resource node to harvest"
        );
        // Money (GOLD) is not a physical good: it never enters `self.goods`, so it
        // is excluded from deposit attribution, the transfer, and the conservation
        // report. A GOLD node would be harvested and deposited by the fast loop yet
        // never transferred or tracked — a silent world-side money leak. Reject it
        // at the seam rather than let the §4.3 "no money in the fast loop" rule and
        // whole-system conservation go blind to it.
        assert!(
            config.nodes.iter().all(|spec| spec.good != GOLD),
            "a resource node cannot harvest the money good (GOLD); money is not a \
             physical good and never crosses the world→econ transfer seam"
        );
        if let Some(chain) = &config.chain {
            assert!(
                chain.operating_cost >= 1,
                "chain operating_cost must be at least 1"
            );
            // A producer's throughput becomes that many input wants on its value scale
            // each regeneration; bound it so a config cannot drive the scale (and the
            // market that iterates it) to an unbounded size. See [`MAX_CHAIN_THROUGHPUT`].
            assert!(
                chain.throughput <= MAX_CHAIN_THROUGHPUT,
                "chain throughput {} exceeds the sanity bound {MAX_CHAIN_THROUGHPUT}",
                chain.throughput
            );
        }
        let dynamics = config.dynamics;
        // The need→good mapping. A plain settlement uses the lab default
        // (hunger ↔ FOOD). The G3a chain and the G3b emergent config make **bread
        // the staple** (hunger ↔ bread) so the chain's final good is what colonists
        // eat to live, and that demand prices bread. The G3b no-spread control sets
        // `bread_is_staple = false`, keeping hunger ↔ FOOD so bread is never demanded
        // (and so never prices, and so no role forms). Warmth stays WOOD, savings GOLD.
        let known = match &config.chain {
            Some(chain) if chain.bread_is_staple => KnownGoods {
                hunger: chain.content.bread(),
                warmth: WOOD,
                savings: GOLD,
            },
            // The control (chain present, bread not the staple) eats seeded FOOD;
            // every plain settlement eats gathered FOOD.
            Some(_) | None => KnownGoods::lab_default(),
        };
        let mut rng = Rng::new(seed);

        // ---- world: grid, exchange stockpile, resource nodes ----
        let grid = Grid::new(config.width, config.height);
        let mut world = World::new(grid);
        let exchange = world
            .add_stockpile(Stockpile::new(config.exchange, config.exchange_cap))
            .expect("exchange lands on a passable tile");
        let mut node_ids = Vec::with_capacity(config.nodes.len());
        for spec in &config.nodes {
            let id = world
                .add_node(ResourceNode::new(
                    spec.pos, spec.good, spec.stock, spec.regen, spec.cap,
                ))
                .expect("node lands on a passable tile");
            node_ids.push(id);
        }

        let consumers = usize::from(config.consumers);
        let gatherers = usize::from(config.gatherers);
        // The seeded producer counts (G3a) and the G3b *latent* producer counts:
        // all zero without a chain, so a plain settlement's population, ids, and
        // digest are byte-identical to G2b. Seeded millers/bakers (G3a) take a fixed
        // producer vocation; the latent pool (G3b) starts `Unassigned` and adopts
        // from the spread. Both bands follow the gatherers in id order.
        let (millers, bakers, latent_millers, latent_bakers) = match &config.chain {
            Some(chain) => (
                usize::from(chain.millers),
                usize::from(chain.bakers),
                usize::from(chain.latent_millers),
                usize::from(chain.latent_bakers),
            ),
            None => (0, 0, 0, 0),
        };
        let population = consumers + gatherers + millers + bakers + latent_millers + latent_bakers;

        // Resident traders (G2c caravans) take the LOWEST ids, *before* the
        // colonists, so they are processed first in the id-ordered market and their
        // resting orders are the **price-setting makers** the rest of the book
        // crosses (a caravan trader leads the book: a seller's cheap ask becomes the
        // realized price, pulling a dear market down toward the cheap one). A trader
        // is otherwise inert at generation — an EMPTY scale posts no orders until
        // the `Region` activates it — and it is not a colonist (no need/scale/task
        // phase touches it). It is given a *parked* world agent at the exchange (so
        // world and econ `AgentId`s stay coincident for the colonists that follow);
        // routes are abstract, so the trader is never tasked and its world agent
        // just idles, carrying nothing. No randomness is drawn for traders — the
        // `Region`, not the settlement, drives them deterministically.
        let num_traders = config.resident_traders.len();
        let mut colonists = Vec::with_capacity(population);
        let mut agents = Vec::with_capacity(num_traders + population);
        let mut trader_ids = Vec::with_capacity(num_traders);
        for (offset, endowment) in config.resident_traders.iter().enumerate() {
            let id = AgentId(offset as u64);
            let placed = world
                .add_agent(config.exchange, config.carry_cap, config.move_speed)
                .expect("trader lands on the exchange tile");
            debug_assert_eq!(placed, id, "world and econ trader ids must coincide");
            agents.push(build_trader_agent(id, endowment));
            trader_ids.push(id);
        }

        // Consumers take the LOWER colonist ids so their FOOD bids rest before the
        // gatherers' asks and set the realized price (the supply-sensitive,
        // buyers-lead book; see the module docs). Gatherers follow. Colonist ids
        // begin at `num_traders` (the trader pair, if any, leads); for a plain
        // settlement `num_traders == 0`, so colonists keep ids 0,1,2,… exactly as
        // in G2b and every existing config and golden is byte-identical. World
        // `AgentId`s match econ `AgentId`s by construction (assigned in this order).
        let colonist_id_base = num_traders as u64;
        for index in 0..population {
            let id = AgentId(colonist_id_base + index as u64);
            // World agent for every colonist (consumers idle at the exchange,
            // gatherers haul); placement at the exchange tile is always passable.
            let placed = world
                .add_agent(config.exchange, config.carry_cap, config.move_speed)
                .expect("colonist lands on the exchange tile");
            debug_assert_eq!(placed, id, "world and econ agent ids must coincide");

            // Vocation by id band: consumers (lowest ids, so their bids lead the
            // book), then gatherers, then the seeded producers (G3a) — millers,
            // then bakers — then the latent pool (G3b) — latent millers, then
            // latent bakers — that start `Unassigned` and adopt from the spread.
            // Producers do not gather (no node) and use the patient consumer
            // time-preference base so they keep offering their output and carry a
            // savings want the entrepreneurial appraisal can target.
            let seeded_end = consumers + gatherers + millers + bakers;
            let (vocation, node, tp_base, latent) = if index < consumers {
                (
                    Vocation::Consumer,
                    None,
                    config.consumer_time_preference_base_bps,
                    None,
                )
            } else if index < consumers + gatherers {
                let node = node_ids[(index - consumers) % node_ids.len()];
                (
                    Vocation::Gatherer,
                    Some(node),
                    config.gatherer_time_preference_base_bps,
                    None,
                )
            } else if index < consumers + gatherers + millers {
                (
                    Vocation::Miller,
                    None,
                    config.consumer_time_preference_base_bps,
                    None,
                )
            } else if index < seeded_end {
                (
                    Vocation::Baker,
                    None,
                    config.consumer_time_preference_base_bps,
                    None,
                )
            } else if index < seeded_end + latent_millers {
                (
                    Vocation::Unassigned,
                    None,
                    config.consumer_time_preference_base_bps,
                    Some(RecipeId::Mill),
                )
            } else {
                (
                    Vocation::Unassigned,
                    None,
                    config.consumer_time_preference_base_bps,
                    Some(RecipeId::Bake),
                )
            };
            let culture = draw_culture(&mut rng, tp_base, config.leisure_weight_base_bps);
            let need = NeedState::rested();
            agents.push(build_agent(
                id, &need, &culture, &known, vocation, latent, config,
            ));
            colonists.push(Colonist {
                id,
                vocation,
                node,
                need,
                culture,
                critical_streak: 0,
                alive: true,
                latent,
            });
        }

        // The goods tracked for conservation: node goods plus anything a colonist
        // or resident trader starts holding (FOOD via nodes/buffers, WOOD via
        // endowments). Money is not a physical good, so it is excluded.
        let mut goods: Vec<GoodId> = Vec::new();
        let push_good = |g: GoodId, goods: &mut Vec<GoodId>| {
            if g != GOLD && !goods.contains(&g) {
                goods.push(g);
            }
        };
        for spec in &config.nodes {
            push_good(spec.good, &mut goods);
        }
        for agent in &agents {
            for g in agent.stock.positive_goods() {
                push_good(g, &mut goods);
            }
        }
        // Every chain good is tracked even if no agent is seeded holding it yet
        // (flour, for instance, only appears once a miller produces it): the
        // production phase mints it into econ stock, and the conservation report
        // and the canonical digest must already account it.
        if let Some(chain) = &config.chain {
            for g in chain.content.goods() {
                push_good(g, &mut goods);
            }
        }
        goods.sort();

        let recipes = config
            .chain
            .as_ref()
            .map(|chain| chain.content.recipes().to_vec())
            .unwrap_or_default();
        let scenario = MarketScenario {
            name: "settlement",
            // A SoundGold M1 (designated-gold spot) scenario, exactly as `Camp`
            // uses (the natural seam): the consumption-log readback and the
            // realized-price accessor live on this path.
            scenario: ScenarioName::MarketBarterishGold,
            seed,
            periods: 0,
            agents,
            recipes,
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        };
        let mut society = Society::from_scenario(scenario);
        society.enable_consumption_log();

        // Build the production-chain runtime and register the content good names
        // so the society's registry resolves them (the viewer reads names through
        // `Society::good_name`). The ids the society interns must equal those the
        // `ContentSet` assigned — both intern over the same lab catalog in the
        // same order — which the assert pins loudly.
        let chain = config.chain.as_ref().map(|chain| {
            for (name, id) in chain.content.good_entries() {
                let interned = society.intern_good(name);
                assert_eq!(
                    interned, id,
                    "content good {name:?} interned to {interned:?} in the society, \
                     not the ContentSet id {id:?}"
                );
            }
            ChainRuntime {
                content: chain.content.clone(),
                throughput: chain.throughput,
                operating_cost: chain.operating_cost,
            }
        });

        Self {
            world,
            society,
            colonists,
            dynamics,
            known,
            exchange,
            carry_cap: config.carry_cap,
            goods,
            pending_deposits: BTreeMap::new(),
            trader_ids,
            colonist_id_base,
            chain,
            econ_tick: 0,
            last_report: EconTickReport::default(),
            commons_gold: Gold::ZERO,
            commons_stock: BTreeMap::new(),
        }
    }

    /// Advance the settlement by one economic tick (the module's documented
    /// phase order). Returns — and stores — the conservation + flow
    /// [`EconTickReport`].
    pub fn econ_tick(&mut self) -> EconTickReport {
        let mut report = EconTickReport {
            econ_tick: self.econ_tick,
            fast_ticks: FAST_TICKS_PER_ECON_TICK,
            ..EconTickReport::default()
        };

        // Snapshot the whole-system totals and the world-only totals before the
        // fast loop. The fast loop only adds goods via regen and only relocates
        // otherwise, so `world_after − world_before` is exactly the regen.
        let world_before: BTreeMap<GoodId, u64> = self
            .goods
            .iter()
            .map(|&g| (g, self.world.total_goods_of(g)))
            .collect();
        for &good in &self.goods {
            report
                .whole_system_before
                .insert(good, self.whole_system_total(good));
        }
        report.total_gold_before_fast = self.total_gold().0;

        // ---- 1. FAST: world ticks; track per-colonist deposits via carry deltas.
        let deposited = self.run_fast_loop();
        report.total_gold_after_fast = self.total_gold().0;
        debug_assert_eq!(
            report.total_gold_before_fast, report.total_gold_after_fast,
            "the fast loop must not move money"
        );
        for &good in &self.goods {
            let after_fast = self.world.total_goods_of(good);
            let before = world_before.get(&good).copied().unwrap_or(0);
            report.regen.insert(good, after_fast - before);
        }

        // ---- 2. TRANSFER: move delivered exchange units into econ stock, net-zero.
        // A unit that cannot be credited remains in the exchange stockpile, still
        // world-owned and counted there — never destroyed. The live case is the only
        // reachable one: a depositor whose stock is momentarily at the `u32` ceiling
        // is *transient* — the attribution is retried each econ tick and the units
        // transfer once consumption opens headroom. A dead depositor never lingers
        // here: G4a's estate settlement drains its stranded pending units to the
        // commons at death and drops the attribution, so `credit_stock`'s rejection
        // of a freed id is a defensive backstop, not a live path.
        self.record_pending_deposits(deposited);
        report.transferred = self.transfer_pending_deposits();

        // ---- 3. NEEDS + real death (G4a): settle each starvation death's estate to
        // the commons, free its arena slot, reconcile the society's caches.
        report.deaths = self.update_needs_and_remove_dead();

        // ---- 4. SCALES.
        self.regenerate_scales();

        // ---- 4b. ROLE-CHOICE (G3b): each living colonist holding latent
        // production capital re-appraises the recipe it could run against the
        // realized price spread it can observe (last tick's prices) and its freshly
        // regenerated value scale, adopting or reverting its producer vocation. If
        // any role changes, regenerate again so this tick's market sees the matching
        // active/latent production wants. The second pass regenerates the whole
        // (small) living roster, not just the changed colonists: a re-regeneration is
        // idempotent for an unchanged colonist (its need state and vocation are
        // identical between the two calls, so it yields the same scale and cancels no
        // quote), so the full pass is byte-identical to a targeted one while keeping
        // the path simple. A no-op for a plain settlement, the seeded G3a config (no
        // latent colonists), and tick 0 (no prices realized yet). Draws no randomness.
        if self.run_role_choice() {
            self.regenerate_scales();
        }

        // ---- 5. MARKET: the unchanged econ clearing; money is redistributed
        // between colonists here. Producers have bought their inputs (a miller a
        // unit of grain, a baker a unit of flour) and sold last tick's output.
        self.society.step();
        report.total_gold_after_step = self.total_gold().0;

        // ---- 6. PRODUCTION (G3a): each living producer applies its recipe to the
        // input it now holds, transforming it into output. A conserved conversion:
        // the input consumed and the output produced are both recorded so the
        // whole-system ledger accounts every transformed unit. Runs after the
        // market (so the input a producer just bought is on hand) and is a no-op
        // for a plain settlement (no chain).
        self.run_production(&mut report);

        // ---- 7. READ-BACK happens at the top of the next tick's NEEDS phase.

        // Conservation receipt: consumed (the eating sink) is this tick's
        // consumption log; the whole-system after-totals (taken AFTER production)
        // must balance against before + regen + produced − consumed_as_input −
        // consumed for every good.
        for &(_, good, qty) in self.society.consumption_log_last_tick() {
            *report.consumed.entry(good).or_insert(0) += u64::from(qty);
        }
        for &good in &self.goods {
            report
                .whole_system_after
                .insert(good, self.whole_system_total(good));
        }
        debug_assert!(
            report.conserves(),
            "whole-system conservation broke at econ tick {}",
            self.econ_tick
        );

        self.econ_tick += 1;
        self.last_report = report.clone();
        report
    }

    /// Run `ticks` economic ticks.
    pub fn run(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.econ_tick();
        }
    }

    // ---- the fast loop --------------------------------------------------

    /// Run [`FAST_TICKS_PER_ECON_TICK`] `world` ticks, keeping idle living
    /// gatherers busy (harvest → exchange), and return the per-colonist,
    /// per-good amounts deposited into the exchange stockpile this interval.
    ///
    /// Deposits are detected as carry **decreases**: a gatherer only ever
    /// deposits at the exchange and harvests at its node, and `world.tick` runs
    /// at most one arrival action per agent per tick, so a per-tick carry drop is
    /// exactly a deposit (the accepted amount — overflow stays carried). Escrow
    /// carried over from a previous interval is part of the opening carry, so it
    /// transfers on the arrival that finally lands it.
    fn run_fast_loop(&mut self) -> BTreeMap<(AgentId, GoodId), u32> {
        let mut deposited: BTreeMap<(AgentId, GoodId), u32> = BTreeMap::new();
        // Opening carry baseline (the current escrow), per living gatherer/good.
        let mut prev_carry: BTreeMap<(AgentId, GoodId), u32> = BTreeMap::new();
        for colonist in &self.colonists {
            if colonist.alive && colonist.vocation == Vocation::Gatherer {
                for &good in &self.goods {
                    prev_carry.insert(
                        (colonist.id, good),
                        self.world.agent_carry(colonist.id, good),
                    );
                }
            }
        }
        // Exchange contents before the interval. Transfer runs *after* this
        // loop, so the only thing that changes exchange contents here is
        // deposits — letting us cross-check our carry-delta attribution against
        // the stockpile's own ledger below (debug only), even when prior clipped
        // deposits are still waiting there.
        #[cfg(debug_assertions)]
        let exchange_before: BTreeMap<GoodId, u32> = self
            .goods
            .iter()
            .map(|&g| (g, self.world.stockpile_get(self.exchange, g)))
            .collect();

        for _ in 0..FAST_TICKS_PER_ECON_TICK {
            self.assign_idle_gatherer_tasks();
            self.world.tick();
            for colonist in &self.colonists {
                if !colonist.alive || colonist.vocation != Vocation::Gatherer {
                    continue;
                }
                for &good in &self.goods {
                    let now = self.world.agent_carry(colonist.id, good);
                    let prev = prev_carry.get(&(colonist.id, good)).copied().unwrap_or(0);
                    if now < prev {
                        *deposited.entry((colonist.id, good)).or_insert(0) += prev - now;
                    }
                    prev_carry.insert((colonist.id, good), now);
                }
            }
        }

        // Defend the deposit-attribution assumption: a carry decrease is taken to
        // be a deposit into the exchange, so the per-good carry drops we summed
        // must equal the exchange stockpile's actual increase over the interval
        // (it is the only stockpile, only living gatherers deposit, and transfer
        // runs after this loop). A future task that drained carry elsewhere would
        // break this equality and trip the check rather than silently misattribute.
        #[cfg(debug_assertions)]
        for &good in &self.goods {
            let increase = self
                .world
                .stockpile_get(self.exchange, good)
                .saturating_sub(exchange_before.get(&good).copied().unwrap_or(0));
            let mut attributed = 0u32;
            for (&(_, g), &q) in &deposited {
                if g == good {
                    attributed += q;
                }
            }
            debug_assert_eq!(
                attributed, increase,
                "carry-delta deposits must equal the exchange increase for {good:?}"
            );
        }

        deposited
    }

    fn record_pending_deposits(&mut self, deposited: BTreeMap<(AgentId, GoodId), u32>) {
        for (key, qty) in deposited {
            if qty == 0 {
                continue;
            }
            let pending = self.pending_deposits.entry(key).or_insert(0);
            *pending = pending
                .checked_add(qty)
                .expect("pending exchange-deposit attribution exceeded stockpile capacity");
        }
        #[cfg(debug_assertions)]
        self.debug_assert_pending_matches_exchange();
    }

    /// Move pending exchange-stockpile units into econ stock when the depositing
    /// colonist can receive them. Credit is attempted before the world withdraw,
    /// so a rejected stale/freed id cannot destroy a unit; the bounded withdraw
    /// then removes exactly the credited units from the exchange.
    ///
    /// A still-live depositor whose stock is momentarily full retries here every
    /// econ tick and transfers once headroom opens. A dead depositor never reaches
    /// this branch: [`Settlement::settle_estate_to_commons`] drains its stranded
    /// pending units to the commons at death and drops the attribution, so no entry
    /// keyed by a freed id lingers to be retried. The [`Society::credit_stock`]
    /// rejection of a freed id (it resolves to `None`) is therefore a pure defensive
    /// backstop — were a pending entry ever to outlive its depositor, the unit would
    /// stay world-owned in the exchange (conserved), never silently destroyed.
    fn transfer_pending_deposits(&mut self) -> BTreeMap<GoodId, u64> {
        let mut transferred = BTreeMap::new();
        let mut remaining = BTreeMap::new();
        let pending = std::mem::take(&mut self.pending_deposits);

        for ((agent, good), qty) in pending {
            if qty == 0 {
                continue;
            }
            let headroom = self
                .society
                .agents
                .get(agent)
                .map_or(0, |a| u32::MAX - a.stock.get(good));
            let available = self.world.stockpile_get(self.exchange, good);
            let take = qty.min(headroom).min(available);
            if take == 0 {
                remaining.insert((agent, good), qty);
                continue;
            }

            if self.society.credit_stock(agent, good, take) {
                let removed = self.world.stockpile_withdraw(self.exchange, good, take);
                assert_eq!(removed, take, "exchange must hold every credited unit");
                if qty > take {
                    remaining.insert((agent, good), qty - take);
                }
                *transferred.entry(good).or_insert(0) += u64::from(removed);
            } else {
                remaining.insert((agent, good), qty);
            }
        }

        self.pending_deposits = remaining;
        #[cfg(debug_assertions)]
        self.debug_assert_pending_matches_exchange();
        transferred
    }

    #[cfg(debug_assertions)]
    fn debug_assert_pending_matches_exchange(&self) {
        for &good in &self.goods {
            let pending = self
                .pending_deposits
                .iter()
                .filter(|((_, g), _)| *g == good)
                .map(|(_, &qty)| qty)
                .sum::<u32>();
            debug_assert_eq!(
                pending,
                self.world.stockpile_get(self.exchange, good),
                "pending transfer attribution must match exchange stock for {good:?}"
            );
        }
    }

    /// Give every idle, living gatherer its next task: deposit if it is carrying
    /// anything, else harvest a full load from its node. Deterministic (id order,
    /// no RNG). Dead gatherers are idled and skipped, so their carry stays frozen.
    fn assign_idle_gatherer_tasks(&mut self) {
        for index in 0..self.colonists.len() {
            let colonist = &self.colonists[index];
            if !colonist.alive || colonist.vocation != Vocation::Gatherer {
                continue;
            }
            let Some(node) = colonist.node else { continue };
            let id = colonist.id;
            if self.world.agent_status(id) != Some(AgentStatus::Idle) {
                continue;
            }
            let task = if self.world.agent_carry_total(id) > 0 {
                Task::GoDeposit(self.exchange)
            } else {
                Task::GoHarvest(node, self.carry_cap)
            };
            self.world.assign_task(id, task);
        }
    }

    // ---- the econ-tick phases ------------------------------------------

    /// NEEDS phase: advance living colonists' needs from the last econ tick's
    /// realized consumption + labor, then apply starvation deaths as **real
    /// removal** (G4a) — settling each dead colonist's estate to the commons,
    /// freeing its arena slot, and idling it in the world. Returns the number of
    /// deaths. Deterministic: deaths are collected in generation order and settled
    /// in that order; nothing is drawn.
    fn update_needs_and_remove_dead(&mut self) -> u32 {
        let mut intakes = vec![NeedIntake::default(); self.colonists.len()];
        for &(agent, good, qty) in self.society.consumption_log_last_tick() {
            let Some(index) = self.slot_for_id(agent) else {
                continue;
            };
            if !self.colonists[index].alive {
                continue;
            }
            if good == self.known.hunger {
                intakes[index].food_consumed = intakes[index].food_consumed.saturating_add(qty);
            } else if good == self.known.warmth {
                intakes[index].wood_consumed = intakes[index].wood_consumed.saturating_add(qty);
            }
        }
        for &(agent, labor) in self.society.labor_used_last_tick() {
            let Some(index) = self.slot_for_id(agent) else {
                continue;
            };
            if self.colonists[index].alive {
                intakes[index].labor_used = intakes[index].labor_used.saturating_add(labor);
            }
        }

        for (index, colonist) in self.colonists.iter_mut().enumerate() {
            if colonist.alive {
                colonist.need.advance(&self.dynamics, intakes[index]);
            }
        }

        let mut deaths = 0;
        // Collect deaths first (immutable read of `dynamics`), then apply.
        let mut dying = Vec::new();
        for colonist in &mut self.colonists {
            if !colonist.alive {
                continue;
            }
            if colonist.need.is_critical(&self.dynamics) {
                colonist.critical_streak = colonist.critical_streak.saturating_add(1);
            } else {
                colonist.critical_streak = 0;
            }
            if colonist.critical_streak >= self.dynamics.death_window {
                colonist.alive = false;
                dying.push(colonist.id);
                deaths += 1;
            }
        }
        for id in dying {
            self.settle_estate_to_commons(id);
        }
        deaths
    }

    /// Settle a starved colonist's estate to the commons and remove it (G4a real
    /// death). The order of operations is the spec's: [`Society::remove_agent`]
    /// settles the estate (gold + econ stock), cancels the colonist's market
    /// presence, frees its arena slot, and reconciles its caches — handing back the
    /// [`econ::society::Estate`]. We route that to the commons, drain the colonist's
    /// world-carried delivery escrow to the commons too, and idle it in the world so
    /// it hauls or deposits nothing more. A conserved transfer end to end: the gold
    /// and goods leave the society and the world for the commons, nothing created or
    /// destroyed (heirs/households are G4b). Deterministic: id-ordered, no RNG.
    fn settle_estate_to_commons(&mut self, id: AgentId) {
        if let Some(estate) = self.society.remove_agent(id) {
            // Econ estate: the dead colonist's gold plus every physical good it held
            // (its stock is a subset of `self.goods`; GOLD is money, not stock).
            self.commons_gold = self.commons_gold.saturating_add(estate.gold);
            for &good in &self.goods {
                let qty = estate.stock.get(good);
                if qty > 0 {
                    *self.commons_stock.entry(good).or_insert(0) += u64::from(qty);
                }
            }
        }
        // World-carried escrow: drain it out of the world into the commons (rather
        // than freezing it in place as the G1 tombstone did).
        for &good in &self.goods {
            let carried = self.world.agent_carry(id, good);
            if carried > 0 {
                let drained = self.world.withdraw_agent_carry(id, good, carried);
                *self.commons_stock.entry(good).or_insert(0) += u64::from(drained);
            }
        }
        // Pending exchange-deposit escrow: units this colonist delivered to the
        // exchange stockpile but never had credited (its attribution still sitting in
        // `pending_deposits`) are part of its estate. Drain them out of the world's
        // exchange into the commons and drop the attribution — a conserved transfer
        // (world exchange → commons) that leaves no entry keyed by the freed id for
        // `transfer_pending_deposits` to retry against forever. The withdraw mirrors
        // the removed attribution unit-for-unit, preserving the pending↔exchange
        // invariant. Empty in the starvation-only death model (the transfer phase
        // credits a still-live depositor before it can die), so this is a defensive
        // settle for any future death that strands a pending deposit.
        let stranded: Vec<(AgentId, GoodId)> = self
            .pending_deposits
            .keys()
            .copied()
            .filter(|(agent, _)| *agent == id)
            .collect();
        for key in stranded {
            let qty = self.pending_deposits.remove(&key).unwrap_or(0);
            if qty == 0 {
                continue;
            }
            let (_, good) = key;
            let drained = self.world.stockpile_withdraw(self.exchange, good, qty);
            debug_assert_eq!(
                drained, qty,
                "the exchange must hold every pending unit attributed to a dead depositor"
            );
            if drained > 0 {
                *self.commons_stock.entry(good).or_insert(0) += u64::from(drained);
            }
        }
        // Idle it in the world so it hauls/deposits nothing more.
        self.world.assign_task(id, Task::Idle);
    }

    /// SCALES phase: regenerate every living colonist's value scale from its need
    /// state, overwriting the econ scale, then cancel now-stale resting quotes.
    ///
    /// For a **seeded producer** (G3a) the regenerated need scale is then extended
    /// with two production wants (see [`producer_scale_extension`]): a top-ranked
    /// tool anchor (so the durable tool is never sold) and an input want (so the
    /// producer buys the good it transforms). These are deterministic and pure;
    /// no RNG is drawn here.
    fn regenerate_scales(&mut self) {
        let mut rewritten = Vec::new();
        for colonist in &self.colonists {
            if !colonist.alive {
                continue;
            }
            let mut scale = regenerate_scale(&colonist.need, &colonist.culture, &self.known);
            if let Some(chain) = &self.chain {
                // A producer's tool/input wants follow its production specialty —
                // its adopted vocation (Miller/Baker, seeded or chosen) or, for a
                // latent G3b colonist, the recipe it could run. A latent producer
                // anchors only its tool (it never sells its capital but posts no
                // input bid), while an **active** producer — seeded G3a or adopted
                // G3b — also bids `throughput` units of its input each tick. The
                // latent/active split keeps a latent producer from autonomously
                // pricing the intermediate good (load-bearing for the control).
                if let Some((tool, input)) =
                    production_specialty(colonist.vocation, colonist.latent, &chain.content)
                {
                    let active = matches!(colonist.vocation, Vocation::Miller | Vocation::Baker);
                    let input_wants = if active { chain.throughput.max(1) } else { 0 };
                    producer_scale_extension(&mut scale, tool, input, input_wants);
                }
            }
            self.society
                .agents
                .get_mut(colonist.id)
                .expect("living colonist resolves in the arena")
                .scale = scale;
            rewritten.push(colonist.id);
        }
        self.society
            .cancel_changed_live_quotes_for_agents(&rewritten);
    }

    /// PRODUCTION phase (G3a): each living producer applies its recipe to the
    /// input it holds, up to the throughput cap, recording the conserved
    /// conversion (input consumed, output produced) into `report`. A no-op
    /// without a chain. Deterministic: id-ordered, no RNG, integer state.
    fn run_production(&mut self, report: &mut EconTickReport) {
        let Some(chain) = &self.chain else {
            return;
        };
        let throughput = chain.throughput;
        let mill_recipe = chain.content.mill_recipe().id;
        let bake_recipe = chain.content.bake_recipe().id;
        // `chain`/`colonists` (immutable) and `society` (mutable) are disjoint
        // fields, so id-ordered iteration here borrows them side by side. The
        // recipe ids are content data; mutation delegates to econ's existing
        // direct-recipe executor through an additive `Society` accessor.
        for slot in 0..self.colonists.len() {
            let colonist = &self.colonists[slot];
            if !colonist.alive {
                continue;
            }
            let recipe_id = match colonist.vocation {
                Vocation::Miller => mill_recipe,
                Vocation::Baker => bake_recipe,
                // A latent (Unassigned) colonist holds a tool but has not adopted
                // production, so it mills/bakes nothing until the spread makes it a
                // Miller/Baker (the role-choice phase sets that before production).
                Vocation::Gatherer | Vocation::Consumer | Vocation::Unassigned => continue,
            };
            for _ in 0..throughput {
                let Some(applied) = self
                    .society
                    .execute_direct_recipe_for_agent_checked(colonist.id, recipe_id)
                else {
                    // Out of input (or missing tool): nothing more to mill/bake.
                    break;
                };
                let (out_good, out_qty) = applied.output;
                *report.produced.entry(out_good).or_insert(0) += u64::from(out_qty);
                if let Some((in_good, in_qty)) = applied.input {
                    *report.consumed_as_input.entry(in_good).or_insert(0) += u64::from(in_qty);
                }
            }
        }
    }

    /// ROLE-CHOICE phase (G3b): each living colonist holding latent production
    /// capital (its [`Colonist::latent`] recipe) re-appraises that recipe against
    /// the realized prices it can observe and its own value scale, adopting the
    /// producer vocation when the spread pays and reverting to
    /// [`Vocation::Unassigned`] when it does not. A no-op without a chain and for
    /// every colonist whose `latent` is `None` (gatherers, consumers, and the
    /// **seeded** G3a producers — so the G3a config and digest are unchanged).
    ///
    /// The decision is **ordinal**: it routes entirely through
    /// [`recipe_adoption_pays`] (econ's M2.5 [`appraise_project_bundle_for_money`]),
    /// which asks whether running the recipe — selling its output at the realized
    /// output price for a future receivable, costing the realized input price plus
    /// the operating cost — newly provisions a future-gold want on the colonist's
    /// *own* scale without breaking a higher want. There is no scalar profit number
    /// and no argmax across colonists: each decides for itself, in id order (the
    /// §pillar-1 "colonists act" rule applied to occupation). Re-running it every
    /// tick is what makes a role sticky while the spread holds and revert when it
    /// collapses. Deterministic: integer state, no RNG, id-ordered.
    fn run_role_choice(&mut self) -> bool {
        let Some(chain) = &self.chain else {
            return false;
        };
        // Pull the content data into owned locals so the `&self.chain` borrow is
        // released before the loop mutates `self.colonists` (disjoint fields, but
        // the borrow checker needs the chain borrow gone first).
        let mill_recipe = chain.content.mill_recipe().clone();
        let bake_recipe = chain.content.bake_recipe().clone();
        let grain = chain.content.grain();
        let flour = chain.content.flour();
        let bread = chain.content.bread();
        let operating_cost = chain.operating_cost;
        let tick = self.society.tick.0;
        let mut changed = false;

        for slot in 0..self.colonists.len() {
            let colonist = &self.colonists[slot];
            if !colonist.alive {
                continue;
            }
            // Only latent colonists re-appraise; a `None` latent (gatherer,
            // consumer, or seeded G3a producer) keeps its vocation untouched.
            let Some(latent) = colonist.latent else {
                continue;
            };
            let (recipe, output_price, input_price, adopted) = match latent {
                RecipeId::Mill => (
                    &mill_recipe,
                    self.society.realized_price(flour),
                    self.society.realized_price(grain),
                    Vocation::Miller,
                ),
                RecipeId::Bake => (
                    &bake_recipe,
                    self.society.realized_price(bread),
                    self.society.realized_price(flour),
                    Vocation::Baker,
                ),
                // No other recipe is a latent specialty (set only at generation).
                _ => continue,
            };
            let id = colonist.id;
            let pays = {
                let agent = self
                    .society
                    .agents
                    .get(id)
                    .expect("living colonist resolves in the arena");
                recipe_adoption_pays(
                    agent,
                    recipe,
                    output_price,
                    input_price,
                    tick,
                    operating_cost,
                )
            };
            let next = if pays { adopted } else { Vocation::Unassigned };
            if self.colonists[slot].vocation != next {
                self.colonists[slot].vocation = next;
                changed = true;
            }
        }
        changed
    }

    fn slot_for_id(&self, id: AgentId) -> Option<usize> {
        // Colonist slot `s` has `AgentId(colonist_id_base + s)` (the resident-trader
        // pair, if any, takes the lower ids), so subtracting the base is its slot —
        // an O(1) hit. A non-colonist id (a trader, below the base) or any mismatch
        // falls back to a search, which returns `None` for a trader. (Traders never
        // appear in the consumption/labor logs this resolves, so the fallback is
        // belt-and-braces, not a hot path.)
        if let Some(guess) = (u64::from(id.index())).checked_sub(self.colonist_id_base) {
            let guess = guess as usize;
            if self.colonists.get(guess).map(|c| c.id) == Some(id) {
                return Some(guess);
            }
        }
        self.colonists.iter().position(|c| c.id == id)
    }

    // ---- accessors ------------------------------------------------------

    /// The whole-system total of `good`: every node, carry, and stockpile
    /// (`world`) plus every agent's econ stock — colonists **and** any resident
    /// traders — plus the settlement **commons** (G4a dead-estate sink). The
    /// conserved quantity. The commons term is zero until the first death, so a
    /// no-death run's totals are byte-identical to G2b/G3.
    pub fn whole_system_total(&self, good: GoodId) -> u64 {
        self.world.total_goods_of(good) + self.econ_stock_total(good) + self.commons_stock_of(good)
    }

    /// Total of `good` held in econ agent stock across all live agents (a freed
    /// dead colonist's stock has settled to the commons), including resident
    /// traders.
    pub fn econ_stock_total(&self, good: GoodId) -> u64 {
        self.society
            .agents
            .iter()
            .map(|a| u64::from(a.stock.get(good)))
            .sum()
    }

    /// Units of `good` held in the settlement commons — the conserved sink for
    /// dead colonists' settled estates (G4a). Zero until the first death.
    pub fn commons_stock_of(&self, good: GoodId) -> u64 {
        self.commons_stock.get(&good).copied().unwrap_or(0)
    }

    /// The gold pooled in the settlement commons — dead colonists' settled gold
    /// (G4a). Zero until the first death.
    pub fn commons_gold(&self) -> Gold {
        self.commons_gold
    }

    /// The goods tracked for whole-system conservation (`GoodId`-ordered).
    pub fn tracked_goods(&self) -> &[GoodId] {
        &self.goods
    }

    /// The G3a production-chain content (interned goods + recipes), or `None` for
    /// a plain settlement. Read-only — the viewer and acceptance tests resolve the
    /// chain's good ids and recipes through it.
    pub fn content(&self) -> Option<&ContentSet> {
        self.chain.as_ref().map(|chain| &chain.content)
    }

    /// The most recent realized spot price for `good` (the last trade), or `None`
    /// if no trade in `good` has cleared.
    pub fn realized_price(&self, good: GoodId) -> Option<Gold> {
        self.society.realized_price(good)
    }

    /// The most recent realized FOOD price — the distance→price observable.
    pub fn realized_food_price(&self) -> Option<Gold> {
        self.realized_price(self.known.hunger)
    }

    /// Total money across the settlement (a closed, conserved balance): live econ
    /// gold plus the settlement **commons** (a dead colonist's settled gold). The
    /// commons term is zero until the first death, so a no-death run's total is
    /// byte-identical to G2b/G3 — and including it keeps gold conserved across a
    /// death, when the dead colonist's gold leaves the society for the commons.
    pub fn total_gold(&self) -> Gold {
        self.society.total_gold().saturating_add(self.commons_gold)
    }

    /// Read-only access to the underlying world (carry/stockpile/node inspection).
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Read-only access to the underlying society (holdings/price assertions).
    pub fn society(&self) -> &Society {
        &self.society
    }

    /// Mutable access to the underlying society — **the `Region`/caravan seam**
    /// (G2c). The `Region` reaches through this to drive its resident-trader pair:
    /// set a trader's value scale (then cancel its stale quotes) and shuttle its
    /// wealth with the additive `econ` transfer accessors
    /// ([`Society::debit_stock`] / [`Society::credit_stock`] /
    /// [`Society::debit_gold`] / [`Society::credit_gold`]). It must touch **only**
    /// the [`Settlement::resident_trader_ids`] agents: the settlement owns every
    /// colonist's scale, liveness, and per-tick phase, and mutating a colonist
    /// here would desynchronize its `alive`/need bookkeeping. Caravan moves run
    /// **between** econ ticks (outside [`Settlement::econ_tick`]), so the
    /// settlement's own per-tick conservation receipt is unaffected.
    pub fn society_mut(&mut self) -> &mut Society {
        &mut self.society
    }

    /// The ids of the resident-trader agents (G2c caravans), in generation order.
    /// Empty for a plain settlement. These are econ-only agents the settlement
    /// does not manage; the `Region` drives them through [`Settlement::society_mut`].
    pub fn resident_trader_ids(&self) -> &[AgentId] {
        &self.trader_ids
    }

    /// The exchange stockpile id.
    pub fn exchange(&self) -> StockpileId {
        self.exchange
    }

    /// The most recent econ tick's report.
    pub fn last_report(&self) -> &EconTickReport {
        &self.last_report
    }

    /// Completed econ ticks.
    pub fn econ_tick_count(&self) -> u64 {
        self.econ_tick
    }

    /// The colonist count (living and dead).
    pub fn population(&self) -> usize {
        self.colonists.len()
    }

    /// The stable id of the colonist at generation `index`.
    pub fn colonist_id(&self, index: usize) -> Option<AgentId> {
        self.colonists.get(index).map(|c| c.id)
    }

    /// The vocation of the colonist at generation `index`.
    pub fn vocation_of(&self, index: usize) -> Option<Vocation> {
        self.colonists.get(index).map(|c| c.vocation)
    }

    /// Whether the colonist at generation `index` is still alive.
    pub fn is_alive(&self, index: usize) -> bool {
        self.colonists.get(index).is_some_and(|c| c.alive)
    }

    /// The current need state of the colonist at generation `index`.
    pub fn need_of(&self, index: usize) -> Option<NeedState> {
        self.colonists.get(index).map(|c| c.need)
    }

    /// Units of `good` the colonist at generation `index` is carrying in the
    /// world (its delivery escrow).
    pub fn carry_of(&self, index: usize, good: GoodId) -> u32 {
        self.colonists
            .get(index)
            .map(|c| self.world.agent_carry(c.id, good))
            .unwrap_or(0)
    }

    /// Living colonists of a vocation.
    pub fn living_count(&self, vocation: Vocation) -> usize {
        self.colonists
            .iter()
            .filter(|c| c.alive && c.vocation == vocation)
            .count()
    }

    /// All colonists of a vocation (living and dead) — the seeded roster count.
    pub fn vocation_count(&self, vocation: Vocation) -> usize {
        self.colonists
            .iter()
            .filter(|c| c.vocation == vocation)
            .count()
    }

    /// Total living colonists.
    pub fn living_total(&self) -> usize {
        self.colonists.iter().filter(|c| c.alive).count()
    }

    /// The highest hunger any living colonist carries — the boundedness probe for
    /// the smoke test (hunger is the need that kills).
    pub fn max_living_hunger(&self) -> u16 {
        self.colonists
            .iter()
            .filter(|c| c.alive)
            .map(|c| c.need.hunger)
            .max()
            .unwrap_or(0)
    }

    // ---- determinism surface -------------------------------------------

    /// A canonical, order-stable byte serialization of the whole settlement —
    /// world, econ holdings, needs, and realized prices. Two settlements are
    /// byte-identical iff these are equal (the determinism tripwire).
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.econ_tick.to_le_bytes());
        out.extend_from_slice(&self.world.canonical_bytes());

        // Config-derived parameters that steer future ticks but are not otherwise
        // captured by the dynamic state below, so two settlements differing only
        // in one of them never digest equal — the tripwire stays honest for
        // non-equivalent configs, not only same-config reruns.
        out.extend_from_slice(&self.carry_cap.to_le_bytes());
        out.extend_from_slice(&self.exchange.0.to_le_bytes());
        push_dynamics_bytes(&mut out, &self.dynamics);
        // The role-choice phase (G3b) acts only on a latent pool; a settlement with
        // none (a plain config or a seeded G3a chain) runs it as a no-op. So the
        // role-choice-only knobs below extend the digest only when a latent pool is
        // present — without one they cannot steer a future tick, and including them
        // would make behaviour-identical states digest differently.
        let has_latent_pool = self
            .colonists
            .iter()
            .any(|colonist| colonist.latent.is_some());
        if let Some(chain) = &self.chain {
            out.extend_from_slice(&chain.throughput.to_le_bytes());
            // The G3b operating cost steers nothing but the role-choice appraisal, so
            // it is part of the future-behaviour identity only when a latent pool can
            // run that appraisal. Without one (a seeded G3a chain) two settlements
            // differing only in it behave identically, so it is omitted — keeping the
            // tripwire's "byte-identical iff future behaviour identical" contract
            // honest rather than splitting equivalent seeded chains apart.
            if has_latent_pool {
                out.extend_from_slice(&chain.operating_cost.to_le_bytes());
            }
            // The staple mapping steers the next needs/scale phase for *any* chain,
            // role-choice or not, so it is included whenever a chain is active. The
            // G3b no-spread control shares the emergent config's physical state but
            // maps hunger to FOOD instead of bread, and that divergence must show.
            out.extend_from_slice(&self.known.hunger.0.to_le_bytes());
            out.extend_from_slice(&self.known.warmth.0.to_le_bytes());
            out.extend_from_slice(&self.known.savings.0.to_le_bytes());
            let entries = chain.content.good_entries();
            out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
            for (name, id) in entries {
                out.extend_from_slice(&id.0.to_le_bytes());
                out.extend_from_slice(&(name.len() as u32).to_le_bytes());
                out.extend_from_slice(name.as_bytes());
            }
            out.extend_from_slice(&(chain.content.recipes().len() as u32).to_le_bytes());
            for recipe in chain.content.recipes() {
                push_recipe_bytes(&mut out, recipe);
            }
        }

        // Delivered exchange-stockpile units that are still awaiting econ credit
        // affect future transfers, so attribution belongs in the canonical state.
        out.extend_from_slice(&(self.pending_deposits.len() as u32).to_le_bytes());
        for (&(agent, good), &qty) in &self.pending_deposits {
            out.extend_from_slice(&agent.0.to_le_bytes());
            out.extend_from_slice(&good.0.to_le_bytes());
            out.extend_from_slice(&qty.to_le_bytes());
        }

        // The settlement commons (G4a dead-estate sink). It never feeds back into
        // stepping, so it is omitted entirely while empty — a no-death run's bytes
        // stay identical to the pre-G4a layout (the test-7 tripwire). Once a death
        // settles an estate here it becomes material public state two otherwise-equal
        // runs can differ in (e.g. a different starting gold leaves a different
        // settled balance), so it joins the digest, distinguishing post-death states
        // the live-agent block alone — which drops the freed colonist — would miss.
        // BTreeMap iteration is key-ordered, so the bytes are deterministic.
        let commons_nonempty =
            self.commons_gold > Gold::ZERO || self.commons_stock.values().any(|&qty| qty > 0);
        if commons_nonempty {
            out.extend_from_slice(&self.commons_gold.0.to_le_bytes());
            out.extend_from_slice(&(self.commons_stock.len() as u32).to_le_bytes());
            for (&good, &qty) in &self.commons_stock {
                out.extend_from_slice(&good.0.to_le_bytes());
                out.extend_from_slice(&qty.to_le_bytes());
            }
        }

        // Econ agent state in id order, over the LIVE arena agents (a dead colonist
        // is freed by G4a real removal, so it drops out here). This includes every
        // mutable public field that can affect later stepping: holdings, labor, full
        // value scales, roles, and adaptive price beliefs.
        out.extend_from_slice(&(self.society.agents.len() as u32).to_le_bytes());
        for agent in self.society.agents.iter() {
            out.extend_from_slice(&agent.id.0.to_le_bytes());
            out.extend_from_slice(&agent.gold.0.to_le_bytes());
            out.extend_from_slice(&agent.labor_capacity.to_le_bytes());
            out.extend_from_slice(&agent.hunger_deficit.to_le_bytes());

            out.extend_from_slice(&(agent.roles.len() as u32).to_le_bytes());
            for &role in &agent.roles {
                push_role_bytes(&mut out, role);
            }

            out.extend_from_slice(&(agent.scale.len() as u32).to_le_bytes());
            for want in &agent.scale {
                push_want_kind_bytes(&mut out, want.kind);
                push_horizon_bytes(&mut out, want.horizon);
                out.extend_from_slice(&want.qty.to_le_bytes());
                out.push(u8::from(want.satisfied));
            }

            // Every physical good an agent can hold is already in `self.goods`
            // (node goods ∪ starting goods; trade only relocates them and no
            // recipe mints a new one here), and `self.goods` is sorted — so
            // serialize against it directly, with no per-agent clone/merge/re-sort.
            // The debug check pins that "complete and sorted" assumption.
            #[cfg(debug_assertions)]
            for good in agent.stock.positive_goods() {
                debug_assert!(
                    good == GOLD || self.goods.contains(&good),
                    "agent holds an untracked good {good:?} the digest would miss"
                );
            }
            out.extend_from_slice(&(self.goods.len() as u32).to_le_bytes());
            for &good in &self.goods {
                out.extend_from_slice(&good.0.to_le_bytes());
                out.extend_from_slice(&agent.stock.get(good).to_le_bytes());
            }

            out.extend_from_slice(&(agent.expect.len() as u32).to_le_bytes());
            for belief in &agent.expect {
                out.extend_from_slice(&belief.expected.0.to_le_bytes());
                out.extend_from_slice(&belief.step.0.to_le_bytes());
                out.extend_from_slice(&belief.last_seen.to_le_bytes());
            }
        }

        // Colonist need/liveness state in generation order.
        out.extend_from_slice(&(self.colonists.len() as u32).to_le_bytes());
        for colonist in &self.colonists {
            out.extend_from_slice(&colonist.id.0.to_le_bytes());
            out.push(u8::from(colonist.alive));
            // The vocation tag (Consumer=0, Gatherer=1 — exactly G2b's
            // `u8::from(== Gatherer)` — plus Miller=2, Baker=3, and the G3b
            // Unassigned=4). Pre-G3a settlements only ever emit 0/1, so every
            // G2b/G2c digest is byte-identical; the producers extend the space.
            out.push(colonist.vocation.tag());
            out.extend_from_slice(&colonist.need.hunger.to_le_bytes());
            out.extend_from_slice(&colonist.need.warmth.to_le_bytes());
            out.extend_from_slice(&colonist.need.rest.to_le_bytes());
            out.extend_from_slice(&colonist.critical_streak.to_le_bytes());
            // Culture drives the next scale regeneration and the node the next
            // harvest target, so both belong in the future-behavior identity.
            out.extend_from_slice(&colonist.culture.time_preference_bps.to_le_bytes());
            out.extend_from_slice(&colonist.culture.leisure_weight_bps.to_le_bytes());
            match colonist.node {
                Some(node) => {
                    out.push(1);
                    out.extend_from_slice(&node.0.to_le_bytes());
                }
                None => out.push(0),
            }
            if has_latent_pool {
                // The latent specialty (G3b) steers each tick's role-choice
                // re-appraisal, so it is part of the future-behavior identity. This
                // block is omitted entirely when no latent pool exists, preserving
                // the pre-G3b canonical layout for plain and seeded-only configs.
                match colonist.latent {
                    Some(recipe) => {
                        out.push(1);
                        push_recipe_id_bytes(&mut out, recipe);
                    }
                    None => out.push(0),
                }
            }
        }

        // Realized prices for the tracked goods.
        for &good in &self.goods {
            out.extend_from_slice(&good.0.to_le_bytes());
            match self.realized_price(good) {
                Some(price) => {
                    out.push(1);
                    out.extend_from_slice(&price.0.to_le_bytes());
                }
                None => out.push(0),
            }
        }
        out
    }

    /// A 64-bit FNV-1a digest of [`Settlement::canonical_bytes`] — a compact
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

/// Draw a colonist's culture from the world-generation `Rng` only: time
/// preference within a small band above the vocation's base, and a leisure
/// weight in a fixed band. Mirrors `life::Camp::draw_culture` so the same
/// determinism discipline holds.
fn draw_culture(rng: &mut Rng, time_preference_base: u16, leisure_base: u16) -> CultureParams {
    let span = u16::try_from(rng.next_u64() % 500).unwrap_or(0);
    let time_preference_bps = time_preference_base.saturating_add(span);
    let leisure_weight_bps =
        leisure_base.saturating_add(u16::try_from(rng.next_u64() % 1_001).unwrap_or(0));
    CultureParams::new(time_preference_bps, leisure_weight_bps)
}

fn build_agent(
    id: AgentId,
    need: &NeedState,
    culture: &CultureParams,
    known: &KnownGoods,
    vocation: Vocation,
    latent: Option<RecipeId>,
    config: &SettlementConfig,
) -> Agent {
    let mut stock = Stock::new(NET.0);
    let gold = match &config.chain {
        // ---- Chain endowments. The staple everyone eats is bread (G3a / the G3b
        // emergent config) or seeded FOOD (the no-spread control, where bread
        // demand is absent); WOOD is the warmth battery. Producers — seeded (G3a) or
        // latent (G3b, starting `Unassigned`) — also hold their durable tool and an
        // input buffer so production can fire before the market routes the first
        // input, plus a flour bootstrap stock for a latent miller so the first
        // adopted baker has flour to buy (the chain prices itself bottom-up).
        Some(chain) => {
            let staple = if chain.bread_is_staple {
                chain.content.bread()
            } else {
                FOOD
            };
            // Consumers carry a smaller staple buffer (so they buy early, pricing
            // the staple); everyone else carries the surplus buffer.
            let staple_buffer = match vocation {
                Vocation::Consumer => chain.consumer_staple_buffer,
                _ => chain.bread_buffer,
            };
            stock.add(staple, staple_buffer);
            stock.add(WOOD, chain.wood_buffer);
            match vocation {
                Vocation::Consumer => config.starting_gold_consumer,
                Vocation::Gatherer => config.starting_gold_gatherer,
                Vocation::Miller => {
                    stock.add(chain.content.mill(), 1);
                    stock.add(chain.content.grain(), chain.miller_grain_buffer);
                    chain.producer_gold
                }
                Vocation::Baker => {
                    stock.add(chain.content.oven(), 1);
                    stock.add(chain.content.flour(), chain.baker_flour_buffer);
                    chain.producer_gold
                }
                // A latent producer (G3b) holds the tool + input it would run with,
                // ready to mill/bake the moment its appraisal adopts the vocation. A
                // latent miller also holds a flour stock to sell, so the first
                // adopted baker's flour bid finds a seller and flour realizes a price
                // (which is what then lets a latent miller see the milling spread).
                Vocation::Unassigned => {
                    match latent {
                        Some(RecipeId::Mill) => {
                            stock.add(chain.content.mill(), 1);
                            stock.add(chain.content.grain(), chain.miller_grain_buffer);
                            stock.add(chain.content.flour(), chain.latent_flour_seed);
                        }
                        Some(RecipeId::Bake) => {
                            stock.add(chain.content.oven(), 1);
                            stock.add(chain.content.flour(), chain.baker_flour_buffer);
                        }
                        _ => {}
                    }
                    chain.producer_gold
                }
            }
        }
        // ---- G2b endowments (unchanged; chain vocations never occur without a chain).
        None => {
            let (gold, food, wood) = match vocation {
                Vocation::Gatherer => (
                    config.starting_gold_gatherer,
                    config.gatherer_food_buffer,
                    config.gatherer_wood_buffer,
                ),
                Vocation::Consumer => (
                    config.starting_gold_consumer,
                    config.consumer_food_buffer,
                    config.consumer_wood_endowment,
                ),
                Vocation::Miller | Vocation::Baker | Vocation::Unassigned => {
                    unreachable!("chain vocations require a production chain config")
                }
            };
            stock.add(FOOD, food);
            stock.add(WOOD, wood);
            gold
        }
    };
    Agent {
        id,
        scale: regenerate_scale(need, culture, known),
        stock,
        gold: Gold(gold),
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: belief_vec(),
    }
}

/// The G3b **ordinal role-choice appraisal**: would `agent` adopt the vocation that
/// runs `recipe`, given the realized prices it can observe?
///
/// This is entrepreneurship the praxeology-honest way — it reuses econ's M2.5
/// [`appraise_project_bundle_for_money`] (the same machinery the lab's planner uses
/// to appraise a borrow-build-sell project) rather than computing a scalar profit.
/// It frames running the recipe once as a project bundle:
///
/// - **expected revenue** = the realized `output_price` × the recipe's output yield
///   — the gold the produced good would sell for. If the output has *no* realized
///   price (`output_price` is `None`), the colonist cannot observe a sale and
///   declines: a good with no market has no spread. This is the gate the no-spread
///   control trips — remove the demand that prices the output and no role forms.
/// - **present advance** (the cost) = the realized `input_price` × the input qty
///   (the grain/flour it would *acquire*, valued at `0` until that good prices) plus
///   `operating_cost` (the labor-leisure + tool cost a yield-multiplying recipe must
///   still clear, so a 3× yield is not free).
///
/// The input is *acquired* (bought via the market), not required on hand — the
/// decision is whether the spread pays, so a producer adopts and then buys its
/// input each tick, and reverts when the spread (output price minus input+operating
/// cost) no longer clears, not merely when it momentarily runs dry. Roles track the
/// spread.
///
/// `appraise_project_bundle_for_money` then returns `Some` iff that revenue−cost
/// spread newly provisions a future-gold (savings) want on the agent's own value
/// scale without breaking a higher-ranked want — a strictly ordinal test, decided
/// on the agent's scale, never by a profit threshold. `true` here means *adopt*.
///
/// Pure and deterministic (no RNG, integer state); the role-choice phase calls it
/// once per latent colonist per tick, and the acceptance suite calls it directly to
/// pin the adopt/decline boundary (test 4) and the spread-collapse reversion (test 5).
pub fn recipe_adoption_pays(
    agent: &Agent,
    recipe: &Recipe,
    output_price: Option<Gold>,
    input_price: Option<Gold>,
    tick: u64,
    operating_cost: u64,
) -> bool {
    assert!(operating_cost >= 1, "operating_cost must be at least 1");
    // No observable sale price for the output → no spread to appraise → decline.
    let Some(output_price) = output_price else {
        return false;
    };
    // The input is what the producer must acquire to run the recipe. The reused G3a
    // `Recipe` carries at most one input (`input_good: Option<(GoodId, u32)>`), so the
    // appraisal weighs a single input cost basis — the chain recipes (Mill, Bake) each
    // have exactly one. An input-less recipe (`None`) is NOT special-cased away: its
    // input qty is simply zero, so the appraisal reduces to the output spread against
    // the operating cost alone. Mill/Bake always carry an input, so their appraisal is
    // byte-identical; this only generalizes an input-less recipe rather than declining
    // it outright.
    let input_qty = recipe.input_good.map_or(0, |(_input_good, qty)| qty);

    let expected_revenue = output_price.0.saturating_mul(u64::from(recipe.output_qty));
    let input_cost = input_price
        .map_or(0, |price| price.0)
        .saturating_mul(u64::from(input_qty));
    // The operating cost is required to be ≥ 1 by config, so the present advance
    // is never zero and a flat output price cannot clear it on yield alone.
    let present_advance = input_cost.saturating_add(operating_cost);

    // The future-gold want the project must provision sits at the agent's own
    // savings horizon; target the soonest such horizon so the want qualifies
    // (`later >= loan_horizon`). No savings want → nothing to provision → decline.
    let Some(loan_horizon) = soonest_savings_horizon(&agent.scale) else {
        return false;
    };
    // `econ` rejects `candidate.owner == AgentId(0)` as an invalid project-candidate
    // sentinel (bundle.rs), so the first colonist (id 0) needs a non-zero label to
    // appraise the same ordinal bundle as everyone else. Using `AgentId(1)` is safe
    // even when a real `AgentId(1)` exists: the owner id is stamped ONLY onto the two
    // hypothetical contracts the appraisal builds in-memory for this one call (the
    // imagined receivable/payable in `bundle_accepts_due`), and the provisioning math
    // those feed reads only their `(due_tick, remaining_due)` amounts, never the
    // borrower id and never a global claim registry (agio.rs). This wrapper passes the
    // real agent's own `receivables`/`payables` as empty (`&[]`), so no other agent's
    // claims are in scope to collide with. The owner is a per-call label, not a key.
    let appraisal_owner = if agent.id == AgentId(0) {
        AgentId(1)
    } else {
        agent.id
    };
    let candidate = ProjectBundleCandidate {
        owner: appraisal_owner,
        line: ProjectLineId(0),
        present_advance: Gold(present_advance),
        expected_revenue: Gold(expected_revenue),
        input_cost_basis: Gold(input_cost),
        required_labor: recipe.labor,
        // Production + sale resolve in the near term; the loan (the imagined
        // working-capital advance) is repaid by the savings horizon.
        project_period: 1,
        loan_horizon,
        // The input is *acquired* (its cost is in `present_advance`), not required on
        // hand — an empty bundle so the decision is the spread, not current stock.
        input_goods: Vec::new(),
    };
    let endowment = ProjectBundleEndowment {
        scale: &agent.scale,
        stock: &agent.stock,
        gold: agent.gold,
        receivables: &[],
        payables: &[],
        tick: Tick(tick),
    };
    appraise_project_bundle_for_money(&endowment, &candidate, ProjectPlanId(0), GOLD).is_some()
}

/// The soonest `Later` horizon at which `scale` holds a savings (GOLD) want — the
/// loan horizon the role-choice appraisal targets so that want qualifies as the
/// future-gold want the project bundle must newly provision. `None` if the colonist
/// has no savings want (a present-biased colonist that never appraises a vocation).
///
/// Only `Horizon::Later` wants are considered, and that is the appraisal's own
/// requirement, not an incidental coupling to how scales are generated:
/// `appraise_project_bundle_for_money` can ONLY ever provision a future-money want at
/// `Horizon::Later(later)` with `later >= loan_horizon` (bundle.rs). A `Now`/`Next`
/// GOLD want is immediate liquidity, never the future provisioning a project bundle
/// targets — so even if a scale ever carried one, this appraisal could not satisfy it,
/// and targeting it would only produce a guaranteed decline. Filtering to `Later` is
/// therefore correct by construction.
fn soonest_savings_horizon(scale: &[Want]) -> Option<u32> {
    scale
        .iter()
        .filter_map(|want| match (want.kind, want.horizon) {
            (WantKind::Good(GOLD), Horizon::Later(later)) => Some(u32::from(later)),
            _ => None,
        })
        .min()
}

/// The `(tool, input_good)` a chain vocation produces with, if any: a Miller (or a
/// latent miller) runs the mill (grain → flour); a Baker (or latent baker) the oven
/// (flour → bread). `None` for a gatherer/consumer. This keys
/// [`producer_scale_extension`] so a latent G3b producer reserves its capital just
/// like a seeded/adopted one — the only difference between latent and active is
/// whether [`Settlement::run_production`] runs its recipe.
fn production_specialty(
    vocation: Vocation,
    latent: Option<RecipeId>,
    content: &ContentSet,
) -> Option<(GoodId, GoodId)> {
    let recipe = match vocation {
        Vocation::Miller => Some(RecipeId::Mill),
        Vocation::Baker => Some(RecipeId::Bake),
        Vocation::Unassigned => latent,
        Vocation::Gatherer | Vocation::Consumer => None,
    }?;
    match recipe {
        RecipeId::Mill => Some((content.mill(), content.grain())),
        RecipeId::Bake => Some((content.oven(), content.flour())),
        _ => None,
    }
}

/// Extend a producer's regenerated need scale with its production wants. Pure and
/// deterministic; applied to a seeded producer (G3a), an adopted G3b producer, and
/// a latent G3b producer alike (keyed by [`production_specialty`]) — but the input
/// wants are gated by `input_wants`, which distinguishes the two G3b states.
///
/// - a **tool anchor** (always): a top-ranked `Next` want for the durable tool the
///   producer holds (a mill / an oven). Because the producer holds the tool, the
///   want is always provisioned (it posts no bid), and a sale would un-provision
///   a want ranked above any gold it could gain — so the producer never sells its
///   capital, whether it is actively producing or merely latent. Tools stay durable.
/// - **input wants** (`input_wants` of them, `0` for a latent producer): unit `Next`
///   wants for the good the producer transforms (grain for a miller, flour for a
///   baker), placed *below* every current survival-good want (eat and warm first),
///   then before the lower remainder of the regenerated scale. If a patient,
///   low-need colonist ranks a savings want above a current bread/wood unit, that
///   generated priority is preserved rather than letting recipe inputs jump ahead of
///   survival goods. Unit wants so each is providable by one market buy. `Next` (not
///   `Now`) so the input is reserved for the recipe, never eaten.
///
/// Only an **active** producer (one that has adopted the vocation and will run the
/// recipe this tick) bids for input, so it gets `input_wants = throughput`. A
/// **latent** producer (`Unassigned`) gets `input_wants = 0`: it holds its tool but
/// posts no input bid, so it creates no autonomous demand for the intermediate good.
/// That is load-bearing for the no-spread control — without it, latent producers
/// would price the intermediate good among themselves and roles would form with no
/// downstream demand, defeating the falsification.
fn producer_scale_extension(
    scale: &mut Vec<Want>,
    tool: GoodId,
    input_good: GoodId,
    input_wants: u32,
) {
    // Input wants sit after every present good want (bread/wood in the chain).
    // Savings can legitimately interleave above low-urgency present wants for a
    // patient colonist; using the first `Later` slot would put recipe inputs
    // ahead of those survival goods.
    let insert_at = scale
        .iter()
        .rposition(|want| {
            matches!(want.kind, WantKind::Good(_)) && matches!(want.horizon, Horizon::Now)
        })
        .map(|position| position + 1)
        .or_else(|| {
            scale
                .iter()
                .position(|want| matches!(want.horizon, Horizon::Later(_)))
        })
        .unwrap_or(scale.len());
    let input_wants = input_wants as usize;
    let mut base = std::mem::take(scale);
    scale.reserve(base.len() + input_wants + 1);

    // Tool anchor at the very top.
    scale.push(Want {
        kind: WantKind::Good(tool),
        horizon: Horizon::Next,
        qty: 1,
        satisfied: false,
    });
    scale.extend(base.drain(..insert_at));
    for _ in 0..input_wants {
        scale.push(Want {
            kind: WantKind::Good(input_good),
            horizon: Horizon::Next,
            qty: 1,
            satisfied: false,
        });
    }
    scale.extend(base);
}

/// Build a resident-trader agent (G2c caravans) from its endowment: working gold,
/// an initial physical stock, an **empty** value scale (so it posts no orders
/// until the `Region` activates it), and the [`Role::Trader`]. Draws no
/// randomness — traders are `Region`-driven, not culture-generated.
fn build_trader_agent(id: AgentId, endowment: &TraderEndowment) -> Agent {
    let mut stock = Stock::new(NET.0);
    for &(good, qty) in &endowment.stock {
        assert!(
            good != GOLD,
            "a resident trader cannot be endowed with the money good (GOLD); \
             pass working capital via TraderEndowment::gold instead"
        );
        stock.add(good, qty);
    }
    Agent {
        id,
        scale: Vec::new(),
        stock,
        gold: Gold(endowment.gold),
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Trader],
        expect: belief_vec(),
    }
}

fn push_dynamics_bytes(out: &mut Vec<u8>, d: &NeedDynamics) {
    out.extend_from_slice(&d.need_max.to_le_bytes());
    out.extend_from_slice(&d.hunger_deplete.to_le_bytes());
    out.extend_from_slice(&d.warmth_deplete.to_le_bytes());
    out.extend_from_slice(&d.hunger_per_food.to_le_bytes());
    out.extend_from_slice(&d.warmth_per_wood.to_le_bytes());
    out.extend_from_slice(&d.rest_per_labor.to_le_bytes());
    out.extend_from_slice(&d.rest_recover.to_le_bytes());
    out.extend_from_slice(&d.hunger_critical.to_le_bytes());
    out.extend_from_slice(&d.death_window.to_le_bytes());
}

fn push_role_bytes(out: &mut Vec<u8>, role: Role) {
    out.push(match role {
        Role::Household => 0,
        Role::Producer => 1,
        Role::Trader => 2,
        Role::Capitalist => 3,
        Role::Worker => 4,
        Role::Consumer => 5,
    });
}

fn push_want_kind_bytes(out: &mut Vec<u8>, kind: WantKind) {
    match kind {
        WantKind::Good(good) => {
            out.push(0);
            out.extend_from_slice(&good.0.to_le_bytes());
        }
        WantKind::Leisure => out.push(1),
    }
}

fn push_horizon_bytes(out: &mut Vec<u8>, horizon: Horizon) {
    match horizon {
        Horizon::Now => out.push(0),
        Horizon::Next => out.push(1),
        Horizon::Later(ticks) => {
            out.push(2);
            out.push(ticks);
        }
    }
}

fn push_recipe_bytes(out: &mut Vec<u8>, recipe: &Recipe) {
    push_recipe_id_bytes(out, recipe.id);
    out.extend_from_slice(&(recipe.name.len() as u32).to_le_bytes());
    out.extend_from_slice(recipe.name.as_bytes());
    out.extend_from_slice(&recipe.labor.to_le_bytes());
    match recipe.input_good {
        Some((good, qty)) => {
            out.push(1);
            out.extend_from_slice(&good.0.to_le_bytes());
            out.extend_from_slice(&qty.to_le_bytes());
        }
        None => out.push(0),
    }
    match recipe.required_tool {
        Some(good) => {
            out.push(1);
            out.extend_from_slice(&good.0.to_le_bytes());
        }
        None => out.push(0),
    }
    out.extend_from_slice(&recipe.output_good.0.to_le_bytes());
    out.extend_from_slice(&recipe.output_qty.to_le_bytes());
    out.push(u8::from(recipe.enabled));
}

fn push_recipe_id_bytes(out: &mut Vec<u8>, id: RecipeId) {
    out.push(match id {
        RecipeId::GatherFood => 0,
        RecipeId::CutWood => 1,
        RecipeId::FishWithNet => 2,
        RecipeId::Mill => 3,
        RecipeId::Bake => 4,
    });
}

fn belief_vec() -> Vec<PriceBelief> {
    let slots = usize::from(NET.0) + 1;
    vec![PriceBelief::new(Gold(2), Gold(1)); slots]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_places_one_world_agent_per_colonist_at_the_exchange() {
        let config = SettlementConfig::viable();
        let s = Settlement::generate(1, &config);
        assert_eq!(
            s.population(),
            usize::from(config.consumers) + usize::from(config.gatherers)
        );
        // Consumers take the lower ids, gatherers the higher.
        for index in 0..s.population() {
            let expected = if index < usize::from(config.consumers) {
                Vocation::Consumer
            } else {
                Vocation::Gatherer
            };
            assert_eq!(s.vocation_of(index), Some(expected));
            assert_eq!(s.colonist_id(index), Some(AgentId(index as u64)));
        }
        // Everyone starts on the exchange tile.
        for index in 0..s.population() {
            let id = s.colonist_id(index).unwrap();
            assert_eq!(s.world().agent_pos(id), Some(config.exchange));
        }
    }

    #[test]
    fn tracked_goods_are_food_and_wood_only() {
        let s = Settlement::generate(1, &SettlementConfig::viable());
        assert_eq!(s.tracked_goods(), &[FOOD, WOOD]);
    }

    #[test]
    fn resident_traders_take_the_lowest_ids_and_start_idle() {
        let config = SettlementConfig::viable().with_resident_traders(vec![TraderEndowment {
            gold: 500,
            stock: Vec::new(),
        }]);
        let s = Settlement::generate(1, &config);
        let population = usize::from(config.consumers) + usize::from(config.gatherers);

        // The trader takes id 0 (a price-setting maker, processed first) and is NOT
        // a colonist; colonists shift up to ids 1..=population.
        assert_eq!(s.population(), population, "traders are not colonists");
        assert_eq!(s.resident_trader_ids(), &[AgentId(0)]);
        assert_eq!(
            s.colonist_id(0),
            Some(AgentId(1)),
            "colonists shift up by one"
        );

        // It is a real econ agent: present in the arena with its endowment, an
        // empty (idle) scale, the Trader role, and a parked world agent at the
        // exchange (so world/econ ids stay coincident for the colonists).
        let trader = s
            .society()
            .agents
            .get(AgentId(0))
            .expect("trader resolves in the arena");
        assert_eq!(trader.gold.0, 500);
        assert!(trader.scale.is_empty(), "a fresh trader posts no orders");
        assert_eq!(trader.roles, vec![Role::Trader]);
        assert_eq!(
            s.world().agent_pos(AgentId(0)),
            Some(config.exchange),
            "a trader parks at the exchange, never tasked"
        );
    }

    #[test]
    fn no_resident_traders_is_byte_identical_to_a_plain_settlement() {
        // The additive field must not move a trader-less settlement's digest — the
        // G2b determinism tripwire and the econ goldens depend on this.
        let plain = Settlement::generate(7, &SettlementConfig::viable());
        let explicit_empty = Settlement::generate(
            7,
            &SettlementConfig::viable().with_resident_traders(Vec::new()),
        );
        assert_eq!(plain.digest(), explicit_empty.digest());
    }

    #[test]
    fn settle_estate_drains_a_stranded_pending_deposit_to_the_commons() {
        // A gatherer can deliver units to the exchange whose econ credit is still
        // pending when it dies. Estate settlement must drain that stranded escrow to
        // the commons (a conserved world-exchange → commons transfer) and drop the
        // attribution — never orphan the units in the exchange or leak the entry.
        // Drive the deposit phase WITHOUT the transfer to strand a pending entry,
        // then settle the depositor directly and check the drain.
        let mut s = Settlement::generate(1, &SettlementConfig::viable());

        // Accumulate a real pending deposit (deposit phase only — no transfer, so it
        // is never credited and stays attributed in `pending_deposits`).
        for _ in 0..8 {
            let deposited = s.run_fast_loop();
            s.record_pending_deposits(deposited);
            if !s.pending_deposits.is_empty() {
                break;
            }
        }
        let &(depositor, good) = s
            .pending_deposits
            .keys()
            .next()
            .expect("a gatherer must have a stranded pending deposit");
        let pending_qty = s.pending_deposits[&(depositor, good)];
        assert!(pending_qty > 0, "the stranded pending deposit is non-empty");

        // Mark the depositor dead (mirroring the real caller) and snapshot the
        // conserved totals + the exchange contents before settling.
        let index = s
            .colonists
            .iter()
            .position(|c| c.id == depositor)
            .expect("the depositor is a colonist");
        s.colonists[index].alive = false;
        let goods = s.goods.clone();
        let before: Vec<u64> = goods.iter().map(|&g| s.whole_system_total(g)).collect();
        let exchange_before = s.world.stockpile_get(s.exchange, good);
        let commons_before = s.commons_stock_of(good);

        s.settle_estate_to_commons(depositor);

        // The attribution is gone, exactly the stranded units left the exchange for
        // the commons, and every good's whole-system total is unchanged.
        assert!(
            s.pending_deposits.keys().all(|(a, _)| *a != depositor),
            "the dead depositor's pending attribution must be drained"
        );
        assert_eq!(
            s.world.stockpile_get(s.exchange, good),
            exchange_before - pending_qty,
            "exactly the stranded pending units leave the exchange"
        );
        assert!(
            s.commons_stock_of(good) >= commons_before + u64::from(pending_qty),
            "the stranded pending units settle to the commons"
        );
        for (i, &g) in goods.iter().enumerate() {
            assert_eq!(
                s.whole_system_total(g),
                before[i],
                "estate settlement broke whole-system conservation"
            );
        }
    }

    #[test]
    fn canonical_bytes_capture_a_nonempty_commons() {
        // The commons is omitted from the canonical bytes while empty — so a no-death
        // run matches the pre-G4a layout (the test-7 tripwire) — but joins the digest
        // once a death settles an estate, so two states that differ only in their
        // settled commons no longer collide.
        let config = SettlementConfig::viable();
        let baseline = Settlement::generate(1, &config);
        let empty_len = baseline.canonical_bytes().len();

        // An empty commons adds nothing: a clone with an untouched commons is byte-
        // identical (the inertness the no-death goldens depend on).
        let mut settled_gold = Settlement::generate(1, &config);
        assert_eq!(
            settled_gold.canonical_bytes(),
            baseline.canonical_bytes(),
            "an empty commons must not perturb the canonical bytes"
        );

        // Settling gold to the commons changes the bytes and lengthens them.
        settled_gold.commons_gold = Gold(7);
        let with_gold = settled_gold.canonical_bytes();
        assert!(
            with_gold.len() > empty_len,
            "a non-empty commons extends the digest"
        );
        assert_ne!(with_gold, baseline.canonical_bytes());

        // Two commons that differ only in their settled balance digest differently —
        // the post-death collision the digest would otherwise miss is closed.
        let mut more_gold = Settlement::generate(1, &config);
        more_gold.commons_gold = Gold(8);
        assert_ne!(
            settled_gold.digest(),
            more_gold.digest(),
            "distinct settled commons balances must not digest equal"
        );

        // Commons stock alone (a settled estate of goods, no gold) registers too.
        let mut settled_stock = Settlement::generate(1, &config);
        settled_stock.commons_stock.insert(FOOD, 3);
        assert_ne!(
            settled_stock.canonical_bytes(),
            baseline.canonical_bytes(),
            "settled commons stock must enter the canonical bytes"
        );
    }

    #[test]
    #[should_panic(expected = "cannot be endowed with the money good")]
    fn resident_trader_rejects_gold_stock() {
        let config = SettlementConfig::viable().with_resident_traders(vec![TraderEndowment {
            gold: 0,
            stock: vec![(GOLD, 10)],
        }]);
        let _ = Settlement::generate(1, &config);
    }

    #[test]
    #[should_panic(expected = "cannot harvest the money good")]
    fn generate_rejects_a_money_good_resource_node() {
        // GOLD is excluded from `self.goods`, so a GOLD node would be harvested
        // and deposited by the fast loop yet never transferred or conserved — a
        // silent world-side money leak. `generate` must reject it at the seam.
        let mut config = SettlementConfig::viable();
        config.nodes[0].good = GOLD;
        let _ = Settlement::generate(1, &config);
    }

    #[test]
    #[should_panic(expected = "must define at least one resource node")]
    fn generate_rejects_gatherers_without_nodes() {
        let mut config = SettlementConfig::viable();
        config.nodes.clear();
        let _ = Settlement::generate(1, &config);
    }

    #[test]
    fn canonical_bytes_include_value_scale_contents() {
        let config = SettlementConfig::viable();
        let a = Settlement::generate(1, &config);
        let mut b = Settlement::generate(1, &config);

        let agent = b
            .society
            .agents
            .get_mut(AgentId(0))
            .expect("generated consumer resolves");
        assert!(
            !agent.scale.is_empty(),
            "generated agents have value scales"
        );
        agent.scale[0].qty = agent.scale[0].qty.saturating_add(1);

        assert_ne!(a.canonical_bytes(), b.canonical_bytes());
        assert_ne!(a.digest(), b.digest());
    }

    #[test]
    fn first_econ_tick_transfers_some_food_and_conserves() {
        let config = SettlementConfig::viable().with_food_node_distance(3);
        let mut s = Settlement::generate(1, &config);
        let report = s.econ_tick();
        // A near node delivers FOOD within the first interval.
        assert!(
            report.transferred_of(FOOD) > 0,
            "no FOOD reached the market"
        );
        // No WOOD is ever hauled (it never enters the world).
        assert_eq!(report.transferred_of(WOOD), 0);
        assert_eq!(s.world().total_goods_of(WOOD), 0);
        assert!(report.conserves(), "first tick broke conservation");
    }

    #[test]
    fn emergent_config_seeds_a_latent_pool_not_seeded_roles() {
        // G3b: the emergent config hand-places NO producer; instead it seeds a pool
        // of `Unassigned` colonists carrying a latent recipe (and the tool for it),
        // following the gatherers/consumers in id order.
        let config = SettlementConfig::emergent_chain();
        let s = Settlement::generate(1, &config);
        let content = s.content().expect("emergent config has chain content");

        let (mut latent_millers, mut latent_bakers) = (0, 0);
        for colonist in &s.colonists {
            match colonist.latent {
                Some(RecipeId::Mill) => {
                    assert_eq!(colonist.vocation, Vocation::Unassigned);
                    // A latent miller holds its mill (latent capital) — never seeded
                    // as an active producer.
                    let stock = &s.society.agents.get(colonist.id).unwrap().stock;
                    assert_eq!(stock.get(content.mill()), 1, "latent miller holds a mill");
                    latent_millers += 1;
                }
                Some(RecipeId::Bake) => {
                    assert_eq!(colonist.vocation, Vocation::Unassigned);
                    let stock = &s.society.agents.get(colonist.id).unwrap().stock;
                    assert_eq!(stock.get(content.oven()), 1, "latent baker holds an oven");
                    latent_bakers += 1;
                }
                Some(_) => panic!("only the chain recipes are latent specialties"),
                None => assert_ne!(
                    colonist.vocation,
                    Vocation::Unassigned,
                    "a non-latent colonist is never Unassigned"
                ),
            }
        }
        assert!(
            latent_millers > 0 && latent_bakers > 0,
            "both latent stages seeded"
        );
        // No producer role is hand-placed at generation.
        assert_eq!(s.vocation_count(Vocation::Miller), 0);
        assert_eq!(s.vocation_count(Vocation::Baker), 0);
    }

    #[test]
    fn canonical_bytes_include_operating_cost_and_latent() {
        // Two emergent configs differing only in the operating cost must digest
        // differently — it steers the role-choice appraisal, so it is part of the
        // settlement's future-behaviour identity (the determinism tripwire stays
        // honest for non-equivalent chain configs).
        let base = SettlementConfig::emergent_chain();
        let mut dearer = SettlementConfig::emergent_chain();
        dearer.chain.as_mut().expect("chain").operating_cost += 1;
        let base = Settlement::generate(7, &base);
        let dearer = Settlement::generate(7, &dearer);
        assert_ne!(
            base.canonical_bytes(),
            dearer.canonical_bytes(),
            "operating cost must be part of the chain config identity"
        );
    }

    #[test]
    fn seeded_chain_digest_ignores_unused_operating_cost() {
        // A seeded G3a chain has no latent pool, so role-choice is a no-op and the
        // operating cost can never steer a future tick. Two such chains differing
        // only in it behave identically, so they must digest identically — the
        // determinism tripwire's "byte-identical iff future behaviour identical"
        // contract. (Contrast `canonical_bytes_include_operating_cost_and_latent`,
        // where a latent pool makes the same knob load-bearing.)
        let base = SettlementConfig::grain_flour_bread_chain();
        assert_eq!(
            base.chain.as_ref().expect("chain").latent_millers,
            0,
            "the seeded G3a chain must have no latent pool for this contract"
        );
        let mut dearer = SettlementConfig::grain_flour_bread_chain();
        dearer.chain.as_mut().expect("chain").operating_cost += 1;
        let base = Settlement::generate(7, &base);
        let dearer = Settlement::generate(7, &dearer);
        assert_eq!(
            base.canonical_bytes(),
            dearer.canonical_bytes(),
            "an operating cost no latent pool can read must not split the digest"
        );
    }

    #[test]
    fn canonical_bytes_include_staple_mapping() {
        // Same physical generated state, different need→good mapping: future scale
        // regeneration will diverge, so the canonical bytes must diverge too.
        let config = SettlementConfig::emergent_chain();
        let a = Settlement::generate(7, &config);
        let mut b = Settlement::generate(7, &config);
        b.known.hunger = FOOD;

        assert_ne!(
            a.canonical_bytes(),
            b.canonical_bytes(),
            "the staple mapping must be part of the chain config identity"
        );
    }

    #[test]
    #[should_panic(expected = "operating_cost must be at least 1")]
    fn generate_rejects_zero_chain_operating_cost() {
        let mut config = SettlementConfig::emergent_chain();
        config.chain.as_mut().expect("chain").operating_cost = 0;
        let _ = Settlement::generate(7, &config);
    }

    #[test]
    #[should_panic(expected = "exceeds the sanity bound")]
    fn generate_rejects_absurd_chain_throughput() {
        // An unbounded throughput would let a config append arbitrarily many input
        // wants to every producer's value scale (an OOM at the extreme); generation
        // rejects it at the seam, like a zero operating cost.
        let mut config = SettlementConfig::emergent_chain();
        config.chain.as_mut().expect("chain").throughput = MAX_CHAIN_THROUGHPUT + 1;
        let _ = Settlement::generate(7, &config);
    }

    #[test]
    fn role_choice_uses_fresh_scales_and_refreshes_changed_roles() {
        let mut s = Settlement::generate(2_026, &SettlementConfig::emergent_chain());

        let mut miller_slot = None;
        for _ in 0..12 {
            s.econ_tick();
            miller_slot =
                (0..s.population()).find(|&index| s.vocation_of(index) == Some(Vocation::Miller));
            if miller_slot.is_some() {
                break;
            }
        }
        let miller_slot = miller_slot.expect("milling emerged");
        let miller_id = s.colonist_id(miller_slot).expect("miller id");
        let content = s.content().expect("chain").clone();

        // Poison the live econ scale. If role-choice reads the stale scale before
        // SCALES, the miller sees no future savings want and incorrectly reverts.
        s.society
            .agents
            .get_mut(miller_id)
            .expect("miller resolves")
            .scale
            .clear();

        s.econ_tick();

        assert_eq!(
            s.vocation_of(miller_slot),
            Some(Vocation::Miller),
            "role-choice used the stale pre-regeneration scale"
        );
        let scale = &s
            .society
            .agents
            .get(miller_id)
            .expect("miller resolves")
            .scale;
        assert!(
            scale
                .iter()
                .any(|want| want.kind == WantKind::Good(content.grain())),
            "the post-adoption scale must be refreshed with active input wants"
        );
    }

    #[test]
    fn latent_producer_anchors_its_tool_but_posts_no_input_bid() {
        // A latent (Unassigned) producer reserves only its tool — it never bids for
        // its recipe input, so it creates no autonomous demand for the intermediate
        // good (the property the no-spread control relies on). An adopted producer
        // does bid for input.
        let content = ContentSet::grain_flour_bread();
        let mut latent = vec![Want {
            kind: WantKind::Good(content.bread()),
            horizon: Horizon::Now,
            qty: 1,
            satisfied: false,
        }];
        producer_scale_extension(&mut latent, content.mill(), content.grain(), 0);
        assert!(
            !latent
                .iter()
                .any(|w| w.kind == WantKind::Good(content.grain())),
            "a latent producer must not post an input want"
        );
        assert!(
            latent
                .iter()
                .any(|w| w.kind == WantKind::Good(content.mill())),
            "a latent producer still anchors its tool (never sells its capital)"
        );

        let mut active = vec![Want {
            kind: WantKind::Good(content.bread()),
            horizon: Horizon::Now,
            qty: 1,
            satisfied: false,
        }];
        producer_scale_extension(&mut active, content.mill(), content.grain(), 2);
        assert_eq!(
            active
                .iter()
                .filter(|w| w.kind == WantKind::Good(content.grain()))
                .count(),
            2,
            "an active producer bids throughput units of its input"
        );
    }

    #[test]
    fn recipe_adoption_pays_appraises_an_input_less_recipe() {
        // The reused G3a `Recipe` carries at most one input; an input-less recipe
        // (`input_good: None`) is NOT special-cased away — its input cost is zero, so
        // the appraisal reduces to the output spread against the operating cost alone.
        // The chain recipes (Mill, Bake) always carry an input, so this only
        // generalizes the input-less case rather than declining it outright.
        let content = ContentSet::grain_flour_bread();
        let free_recipe = Recipe {
            id: RecipeId::GatherFood,
            name: "Forage",
            labor: 1,
            input_good: None,
            required_tool: None,
            output_good: content.bread(),
            output_qty: 2,
            enabled: true,
        };
        let mut patient = Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(GOLD),
                horizon: Horizon::Later(4),
                qty: 1,
                satisfied: false,
            }],
            stock: Stock::new(NET.0),
            gold: Gold(0),
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Household],
            expect: Vec::new(),
        };
        // An observable output price with an unprovisioned savings want and no input
        // cost still appraises (the input-less recipe is weighed, not auto-declined).
        assert!(
            recipe_adoption_pays(&patient, &free_recipe, Some(Gold(5)), None, 0, 1),
            "an input-less recipe with an output spread must still be appraised"
        );
        // Still ordinal: a gold-sated colonist declines the same spread.
        patient.gold = Gold(100);
        assert!(
            !recipe_adoption_pays(&patient, &free_recipe, Some(Gold(5)), None, 0, 1),
            "a sated colonist declines even an input-less spread (ordinal, not scalar)"
        );
    }
}
