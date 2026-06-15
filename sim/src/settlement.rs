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
//!    moves. Dead colonists are removed from the spatial world after their carried
//!    goods settle, so they deliver nothing and no escrow is destroyed.
//! 2. **TRANSFER** — for each delivered exchange unit awaiting credit, *credit
//!    the depositing colonist's econ stock* and then *withdraw it from the world*
//!    (net-zero, conserved, recorded). A unit that cannot be credited stays
//!    world-owned in the exchange stockpile, never destroyed: a live depositor at
//!    its stock ceiling is retried on later ticks, while a **removed** (dead)
//!    depositor is rejected for good (G4a frees it; any such pending unit it left
//!    stays conserved in the stockpile).
//! 3. **NEEDS** — advance each living colonist's [`NeedState`] from the last econ
//!    tick's realized consumption + labor; apply starvation deaths as real removal
//!    (G4a), settling each estate to the commons and removing the dead from the world.
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
use econ::bank::{Bank, BankPolicy};
use econ::bundle::{
    appraise_project_bundle_for_money, ProjectBundleCandidate, ProjectBundleEndowment,
};
use econ::cantillon::CantillonReceipt;
use econ::capital::ProjectLineId;
use econ::expect::PriceBelief;
use econ::good::{Gold, GoodId, Horizon, Stock, FOOD, GOLD, NET, SALT, WOOD};
use econ::ledger::BankId;
use econ::menger::MengerianEmergence;
use econ::money::{
    DesignatedMoney, MarketMoneyConfig, MengerianConfig, PublicSpotTender, Regime, ReserveRatioBps,
};
use econ::project::{Recipe, RecipeId, Tick};
use econ::purpose::{CreditSource, ProjectPlanId};
use econ::rng::Rng;
use econ::scenario::{EventKind, MarketScenario, ScenarioName};
use econ::society::Society;

use life::{regenerate_scale, CultureParams, KnownGoods, NeedDynamics, NeedIntake, NeedState};

use world::{AgentStatus, Grid, NodeId, Pos, ResourceNode, Stockpile, StockpileId, Task, World};

use crate::content::ContentSet;
use crate::demography::{child_seed, founder_seed, DemographyConfig, HouseholdSpec};

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

/// The id of the (single) bank a G8b settlement charters. Settlements run at most
/// one bank, so a fixed id keeps the bank phase, the canonical bytes, and the
/// accessors unambiguous.
const BANK_ID: BankId = BankId(1);

const G8B_FRACTIONAL_BANK: BankConfig = BankConfig {
    name: "fractional bank",
    reserve_ratio_bps: ReserveRatioBps(2_000),
    deposit_per_tick: Gold(2),
};

const G8B_FULL_RESERVE_BANK: BankConfig = BankConfig {
    name: "full-reserve bank",
    reserve_ratio_bps: ReserveRatioBps::FULL,
    deposit_per_tick: Gold(2),
};

fn is_supported_g8b_bank_charter(bank: BankConfig) -> bool {
    bank == G8B_FRACTIONAL_BANK || bank == G8B_FULL_RESERVE_BANK
}

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
    /// G6b: a **scholar** holds a `library` (durable tool) and, in the research
    /// phase, turns grain it holds into **Knowledge** (the research recipe). Knowledge
    /// is an accumulator, not a tradeable good — the settlement drains the recipe's
    /// output into a per-settlement counter, never into circulation. Seeded (like the
    /// G3a producers); the emergence of the scholar role is deferred (G6b scope).
    Scholar,
    /// G6b: a **confectioner** holds an `atelier` (durable tool) and, once the
    /// settlement's Knowledge unlocks tier 2, runs the tier-2 (gated) recipe — flour
    /// it holds into **pastry**, the higher-order good impossible before the unlock.
    /// Before the unlock the recipe is `enabled: false`, so it produces nothing even
    /// while holding its inputs (the tier gate). Seeded.
    Confectioner,
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
            // G6b extends the space; pre-G6b configs never emit 5/6, so their
            // digests stay byte-identical.
            Vocation::Scholar => 5,
            Vocation::Confectioner => 6,
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
    /// WOOD a **consumer** is seeded holding. In G3a/G3b it equals `wood_buffer`
    /// (consumers are warmth-batteried like everyone else), so those configs are
    /// byte-identical. The G5b frontier keeps it small so consumers run WOOD-short and
    /// *buy* WOOD with the SALT medium — making the SALT-rich consumers the buyers of
    /// BOTH barter counterparts (bread and WOOD), the saleability hub that lets SALT
    /// monetize (the same goods-poor/medium-rich consumer that drives `barter_camp`).
    pub consumer_wood_buffer: u32,
    /// Working gold a producer (miller/baker) starts with — capital to buy its
    /// input while it sells its output.
    pub producer_gold: u64,
    /// G6b: seeded **scholars** (hold a `library`, run grain → Knowledge). `0` for a
    /// non-research chain (the G3a/G3b/G5b chains), so those configs are
    /// byte-identical. The `no-scholars` control sets this `0`: Knowledge never
    /// accumulates, so tier 2 never unlocks (the falsification twin).
    pub scholars: u16,
    /// G6b: seeded **confectioners** (hold an `atelier`, run the tier-2 recipe flour →
    /// pastry once unlocked). Present in BOTH the research config and its control, so
    /// the control proves a would-be producer holding its inputs still produces
    /// nothing while the tier is gated. `0` for a non-research chain.
    pub confectioners: u16,
    /// G6b: the Knowledge counter a settlement must accumulate to unlock tier 2.
    /// `0` means "no tech tiers" (a non-research chain never unlocks). The research
    /// config sets a positive threshold; deterministic — the unlock tick is a pure
    /// function of seed + config.
    pub tier2_threshold: u64,
    /// G6b: grain a scholar is seeded holding — its research input buffer (and the
    /// size of its grain reservation, so it neither dumps the buffer nor starves the
    /// chain's millers of grain). `0` for a non-research chain.
    pub scholar_grain_buffer: u32,
    /// G6b: flour a confectioner is seeded holding — its tier-2 input buffer (held
    /// from tick 0 so the control's "would-be producer holds its inputs" claim is
    /// real, yet produces nothing while gated). `0` for a non-research chain.
    pub confectioner_flour_buffer: u32,
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
            // G3a/G3b consumers carry the same warmth battery as everyone else, so
            // those configs stay byte-identical; the G5b frontier overrides it.
            consumer_wood_buffer: 48,
            producer_gold: 24,
            // No research/tiers by default — the G3a/G3b/G5b chains carry no scholars
            // or confectioners and a zero threshold (no unlock), so every existing
            // config and its digest is byte-identical. The G6b `research` config opts
            // in via `ChainConfig::research_tiers`.
            scholars: 0,
            confectioners: 0,
            tier2_threshold: 0,
            scholar_grain_buffer: 0,
            confectioner_flour_buffer: 0,
        }
    }

    /// The G6b **research-tiers** chain: the seeded grain→flour→bread chain plus
    /// seeded scholars (grain → Knowledge) and a confectioner that runs the
    /// tier-2 recipe (flour → pastry) ONCE Knowledge crosses [`Self::tier2_threshold`].
    /// Built on the [`ContentSet::research_tiers`] content (so it carries the research
    /// and gated tier-2 recipes). Pass `scholars = 0` (via
    /// [`SettlementConfig::research_control`]) for the falsification control.
    pub fn research_tiers() -> Self {
        Self {
            content: ContentSet::research_tiers(),
            // Enough seeded millers/bakers to keep bread flowing while scholars and a
            // confectioner run alongside. Seeded roles (no emergence — G6b scope).
            millers: 3,
            bakers: 5,
            latent_millers: 0,
            latent_bakers: 0,
            operating_cost: 1,
            bread_is_staple: true,
            throughput: 2,
            miller_grain_buffer: 16,
            baker_flour_buffer: 16,
            latent_flour_seed: 0,
            // Generous staple/warmth/gold buffers (mechanism knobs, not balance): the
            // research config adds scholars and a confectioner that BUY inputs and sell
            // nothing tradeable (Knowledge and the seeded pastry never circulate), so
            // they are gold/bread sinks. Large buffers bridge the smoke horizon so the
            // chain stays collapse-free while the tech progression is demonstrated.
            bread_buffer: 80,
            consumer_staple_buffer: 80,
            wood_buffer: 80,
            consumer_wood_buffer: 80,
            producer_gold: 64,
            // Two scholars accumulate Knowledge from labor; one confectioner stands
            // ready to run the tier-2 recipe the moment it unlocks. The threshold is a
            // mechanism knob (not a magnitude): low enough to unlock well inside the
            // smoke horizon, high enough that the unlock is several ticks of real
            // research, not tick 0.
            scholars: 2,
            confectioners: 1,
            tier2_threshold: 20,
            // Modest input buffers (also the per-tick bid ceiling): large enough that
            // research runs from seeded stock through the unlock and tier-2 production
            // has flour on hand, small enough that the scholars do not hoard grain and
            // starve the millers (the chain stays collapse-free over the smoke horizon).
            scholar_grain_buffer: 12,
            confectioner_flour_buffer: 24,
        }
    }
}

/// The G5a **barter-start** overlay: instead of a designated-GOLD market the
/// settlement runs econ's V2 emergence machinery (`MarketMoneyConfig::Emergent`),
/// so a money good must **emerge** from realized spatial barter rather than being
/// assumed. `None` keeps a settlement on the designated-GOLD M1 market — every
/// pre-G5a config and the six econ goldens stay byte-identical (every emergent
/// code path is skipped). `Some` makes colonists barter goods-for-goods at the
/// exchange (driven by econ's reused `BarterBook`/`SaleabilityTracker`) until the
/// Mengerian `winner` rule promotes a money good, after which the existing G2b
/// money market clears trade.
///
/// G5a adds NO emergence rule: [`BarterConfig::menger`] is the lab's adopted M20
/// envelope reused unchanged, and the promotion decision runs inside econ's
/// `step_v2`/`MengerianEmergence::winner`. The only spatial wiring is that the
/// bartered stock is sourced from gather/haul and the durable **medium** the
/// colonists demand ([`BarterConfig::medium_good`]) is the candidate the
/// most-saleable good emerges from.
///
/// The medium is demanded via a config-specific value-scale extension (a
/// `Horizon::Next` "hold the medium" want added on top of the need-driven scale,
/// the same way the G3a/G3b chain adds producer tool/input wants) — not via the
/// need model, which is unchanged. The savings good (`known.savings`) is the
/// emergent medium too, so the post-promotion money market clears those
/// store-of-value wants through GOLD exactly like G2b.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarterConfig {
    /// The Mengerian emergence envelope — the candidate goods and the adopted M20
    /// promotion thresholds. Reused from econ unchanged (no sim-local rule); the
    /// thresholds equal [`MengerianConfig::default`], only `candidate_goods` is
    /// the camp's tradeable set.
    pub menger: MengerianConfig,
    /// The durable medium colonists demand (a `Horizon::Next` "hold the medium"
    /// want on every scale). Its universal, persistent demand — traded against
    /// both the FOOD a gatherer sells and the WOOD a consumer sells — makes it the
    /// good accepted against the most counterparts, the most saleable, so it is
    /// the good that emerges. Never a gathered node good (the world would
    /// regenerate the money good, breaking the conserved promotion).
    pub medium_good: GoodId,
    /// How many `Horizon::Next` units of the medium each colonist wants to hold —
    /// the demand intensity that drives the barter for it.
    pub medium_want_qty: u32,
    /// Units of the medium each gatherer is endowed with at generation. The
    /// curated G5a camp leaves this at zero; gatherers earn the medium by selling
    /// their hauled FOOD/WOOD.
    pub gatherer_medium_endowment: u32,
    /// Units of the medium each consumer is endowed with at generation — the
    /// circulating medium's bulk supply. It changes hands as colonists barter
    /// surplus FOOD/WOOD for it, accumulating the acceptances the saleability rule
    /// reads. Zero in the control: no medium to monetize.
    pub consumer_medium_endowment: u32,
}

/// The G8b **bank charter** overlay (deposits + fiduciary credit), requiring the
/// M3 ledger (`m3 = true`) and no demography overlay until demand-claim estate
/// routing exists. `None` keeps the settlement bank-free exactly as G8a.
/// `Some` charters one econ [`Bank`] into the society and runs the bank phase each
/// econ tick: colonists **deposit** M3 specie (specie → the bank's reserves, and
/// the depositor receives demand claims they spend) and the bank **lends fiduciary
/// credit** — demand claims beyond its reserves, up to its
/// [`Bank::fiduciary_lend_capacity`] for the regime, credited to borrowers who
/// spend them into the economy. The reuse is total: deposit and lend route through
/// econ's existing M3 ledger / bank balance-sheet paths unchanged; G8b only wires
/// the sim's deposit/lend actions into them. `Copy`, so the runtime can hold a
/// detached copy without borrowing the config.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BankConfig {
    /// The bank's display name (the viewer's balance-sheet banner reads it).
    pub name: &'static str,
    /// The bank's reserve ratio in basis points. A **fractional** value (e.g.
    /// `2_000` = 20%) lets the bank lend fiduciary credit beyond its reserves;
    /// [`ReserveRatioBps::FULL`] (`10_000` = 100%) is the **control** — a
    /// full-reserve bank's [`Bank::fiduciary_lend_capacity`] is zero, so it lends
    /// no fiduciary while its deposits still circulate as claims. This single knob
    /// is the milestone's falsification twin (`fractional` vs `full-reserve`).
    pub reserve_ratio_bps: ReserveRatioBps,
    /// Specie a depositing colonist moves into the bank per econ tick (capped by
    /// the colonist's actual specie). Each deposit moves specie → the bank's
    /// reserves and credits the depositor an equal demand claim; the depositor's
    /// spendable total is unchanged (specie became a claim), so claims circulate as
    /// money in its place. A modest value drains specie gradually, so specie,
    /// claims, reserves, and fiduciary are all nonzero through the run.
    pub deposit_per_tick: Gold,
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
    /// The G4b **demography** overlay (births, aging, households, inheritance), or
    /// `None` for a pre-G4b settlement. `None` keeps every existing config and the
    /// six econ goldens byte-identical (every demography code path is skipped and no
    /// colonist is added or removed at runtime by a no-demography run); `Some` seeds
    /// households of non-spatial householders that age, die of old age (via the G4a
    /// removal path), and reproduce — children inheriting their parents' mutated
    /// [`CultureParams`]. See [`DemographyConfig`] and
    /// [`SettlementConfig::lineages`].
    pub demography: Option<DemographyConfig>,
    /// The G5a **barter-start** overlay (emergent money), or `None` for a
    /// designated-GOLD settlement. `None` keeps every existing config and the six
    /// econ goldens byte-identical (every emergent code path is skipped); `Some`
    /// runs the V2 barter/saleability/promotion machinery until a money good
    /// emerges, then the existing G2b money market. Mutually exclusive with
    /// `chain`/`demography` (the G5a slice is a plain gatherer/consumer camp; the
    /// composition with production/demography is G5b). See [`BarterConfig`] and
    /// [`SettlementConfig::barter_camp`].
    pub barter: Option<BarterConfig>,
    /// The G8a **M3 ledger-money** flag. `false` (the default for every pre-G8a
    /// config) keeps the settlement on the closed-GOLD M1 spot market exactly as
    /// before, so every existing config and the six econ goldens are byte-identical
    /// by construction. `true` builds the society on econ's M3 `MoneySystem` (specie
    /// is the money; **no banks, no fiat, no claims** — those are G8b/G8c), so every
    /// money flow (spot trades, the world→econ settlement, wage/birth/estate
    /// transfers) is a ledger move rather than an `Agent.gold` mutation. Economically
    /// equivalent to the M1 settlement (M3 specie with no banks/fiat *is* M1, only
    /// ledger-accounted); mutually exclusive with `barter` (which runs the V2
    /// emergent-money path). See [`SettlementConfig::m3_settlement`].
    pub m3: bool,
    /// The G8b **bank charter** overlay (deposits + fiduciary credit), or `None`
    /// for a bank-free settlement. `None` keeps every pre-G8b config (and the six
    /// econ goldens) byte-identical by construction — the bank phase is skipped
    /// entirely. `Some` requires the M3 ledger (`m3 = true`) and is rejected with a
    /// demography overlay until demand-claim estate routing exists; G8b ships only the
    /// curated `bank`/`full-reserve` controls. The charter adds one econ [`Bank`] in
    /// the society, running deposits and fiduciary lending through the existing M3
    /// ledger / bank paths each econ tick. A depositor that reaches the
    /// starvation-death window (the colony is viable only over a bounded horizon) has
    /// its deposit *withdrawn* on death — claims redeemed for specie, settled as the
    /// G8a specie estate (see [`Settlement::liquidate_bank_deposit_on_death`]) — with
    /// no econ change. See [`BankConfig`] and [`SettlementConfig::bank`] /
    /// [`SettlementConfig::bank_full_reserve`].
    pub bank: Option<BankConfig>,
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
            // No demography by default (G4b is opt-in via `lineages`), so every
            // existing config and golden is byte-identical.
            demography: None,
            // No barter overlay by default — a designated-GOLD G2b settlement.
            // Emergent money is opt-in via `barter_camp`, so every golden stays
            // byte-identical.
            barter: None,
            // Closed-GOLD M1 by default; the M3 ledger settlement is opt-in via
            // `m3_settlement`, so every golden stays byte-identical.
            m3: false,
            bank: None,
        }
    }

    /// The G8a **M3 ledger-money** settlement: the exact [`Self::viable`] economy run
    /// on econ's M3 `MoneySystem` instead of closed-GOLD `Agent.gold`. The money is M3
    /// **specie** — there are NO banks, NO fiat, NO demand claims (those are G8b/G8c) —
    /// so every money flow (the ledger-settled spot market, the world→econ settlement,
    /// and any wage/birth/estate transfer) is a ledger move that conserves the M3
    /// ledger total. Because specie with no banks/fiat behaves economically exactly
    /// like the M1 gold did, this settlement produces the **same trades, prices, and
    /// provisioning** as [`Self::viable`] — it is M1, only ledger-accounted. That
    /// equivalence is the proof the G8a wiring is correct (`g8a_m3_money` test 3).
    pub fn m3_settlement() -> Self {
        Self {
            m3: true,
            ..Self::viable()
        }
    }

    /// The G8b **fractional-reserve bank** settlement: the [`Self::m3_settlement`]
    /// economy with one chartered bank that takes deposits and lends **fiduciary
    /// credit**. Colonists deposit M3 specie (specie → the bank's reserves, claims
    /// to the depositor), and the bank lends demand claims **beyond** its reserves
    /// up to its [`Bank::fiduciary_lend_capacity`] for the regime — credited to
    /// borrowers who spend them. At a 20% reserve ratio the bank lends roughly four
    /// claims of fiduciary per claim of reserve, so claims, reserves, and fiduciary
    /// are all nonzero while specie stays conserved. Paired with
    /// [`Self::bank_full_reserve`] this is the milestone's mechanism + falsification
    /// twin: only the reserve ratio differs. Built on `m3_settlement`, so the
    /// underlying spot market is byte-identical to G8a — the bank is purely additive.
    pub fn bank() -> Self {
        Self {
            bank: Some(G8B_FRACTIONAL_BANK),
            ..Self::m3_settlement()
        }
    }

    /// The G8b **100%-reserve control** — the falsification twin of [`Self::bank`].
    /// Identical in every way except the reserve ratio: a [`ReserveRatioBps::FULL`]
    /// bank's [`Bank::fiduciary_lend_capacity`] is zero, so it lends **no** fiduciary
    /// credit (`fiduciary_issued == 0`) even though its deposits still circulate as
    /// claims. Paired with `bank`, it isolates credit creation to the fractional
    /// reserve: same deposits, same regime, same economy — only the reserve ratio
    /// changes, and the fiduciary vanishes. This is the lab's
    /// `hundred_pct_reserve_lends_no_fiduciary` invariant, in the spatial sim.
    pub fn bank_full_reserve() -> Self {
        Self {
            bank: Some(G8B_FULL_RESERVE_BANK),
            ..Self::m3_settlement()
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
            demography: None,
            barter: None,
            m3: false,
            bank: None,
        }
    }

    /// The G6b **research** settlement: the seeded grain→flour→bread chain plus
    /// seeded scholars who accumulate **Knowledge** from labor and a confectioner who
    /// runs the **tier-2** (gated) recipe — flour → pastry — only after Knowledge
    /// crosses the unlock threshold. Designated-GOLD market (no barter), so the proof
    /// is purely about research-driven tech progression: Knowledge accrues, tier 2
    /// unlocks at a definite tick, and the higher-order good (pastry) appears that was
    /// impossible before. Paired with [`Self::research_control`] (the same world with
    /// the scholars removed), this is the milestone's mechanism + falsification twin.
    pub fn research() -> Self {
        Self::research_with_scholars(true)
    }

    /// The G6b **no-scholars control**: the same research settlement with the scholars
    /// removed (`scholars = 0`). With no scholar labor, Knowledge never accumulates,
    /// so the tier-2 recipe stays disabled and pastry is never produced — even though
    /// the confectioner is present and holds its flour input throughout. Paired with
    /// [`Self::research`] this isolates the cause: identical world and producers, the
    /// scholars (and so the research) the only difference. If the tier unlocked here,
    /// the gate would be reading time (or anything other than research).
    pub fn research_control() -> Self {
        Self::research_with_scholars(false)
    }

    /// Shared builder for the research settlement and its control. `with_scholars`
    /// toggles the scholar count: present (the research config, Knowledge accrues and
    /// tier 2 unlocks) or absent (the control, no Knowledge, no unlock). Everything
    /// else — the chain, the confectioner, the grain node, the rosters — is identical,
    /// so the pair is a clean falsification twin.
    fn research_with_scholars(with_scholars: bool) -> Self {
        let mut chain = ChainConfig::research_tiers();
        if !with_scholars {
            chain.scholars = 0;
        }
        let exchange = Pos::new(0, 0);
        Self {
            width: 64,
            height: 1,
            exchange,
            exchange_cap: 1_000_000,
            // A single rich, close grain node: grain feeds the millers AND the
            // scholars' research, so keep supply loose (more gatherers + regen than the
            // plain chain, since research adds a second class of grain consumer).
            nodes: vec![NodeSpec {
                good: chain.content.grain(),
                pos: Pos::new(4, 0),
                stock: 16_000,
                regen: 80,
                cap: 16_000,
            }],
            gatherers: 5,
            consumers: 1,
            carry_cap: 2,
            move_speed: 1,
            starting_gold_gatherer: 12,
            starting_gold_consumer: 64,
            gatherer_food_buffer: 0,
            gatherer_wood_buffer: 0,
            consumer_food_buffer: 0,
            consumer_wood_endowment: 0,
            gatherer_time_preference_base_bps: 500,
            consumer_time_preference_base_bps: 500,
            leisure_weight_base_bps: 3_000,
            dynamics: NeedDynamics::lab_default(),
            resident_traders: Vec::new(),
            chain: Some(chain),
            demography: None,
            barter: None,
            m3: false,
            bank: None,
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
            demography: None,
            barter: None,
            m3: false,
            bank: None,
        }
    }

    /// The G5a **barter camp** (emergent money): a plain gatherer/consumer camp
    /// that starts in **barter** — no designated money — and lets a money good
    /// *emerge* from realized spatial trade. Gatherers haul FOOD from a node;
    /// consumers hold WOOD and a stock of the durable **SALT** medium. Everyone
    /// is patient, so the store-of-value (savings) want each colonist carries is
    /// strong — and because `known.savings` is SALT, that demand is what makes
    /// SALT the most-saleable good. As they barter surplus FOOD/WOOD for SALT to
    /// provision the future, SALT accumulates acceptances across many agents and
    /// counterpart goods until econ's reused Mengerian `winner` rule promotes it;
    /// from the next tick trade is SALT-money-priced (the existing G2b market).
    ///
    /// Paired with [`Self::barter_camp_control`] (the same camp with the SALT
    /// medium's supply removed) this is the milestone's mechanism + falsification
    /// twin: SALT emerges here, nothing emerges there. G5a adds no emergence rule
    /// — the envelope and the decision are econ's, reused unchanged.
    pub fn barter_camp() -> Self {
        Self::barter_camp_with_medium(true)
    }

    /// The G5a **no-surplus/symmetric control**: the same barter camp with the
    /// circulating SALT medium's **supply removed** (no colonist is endowed with
    /// SALT). The store-of-value want still names SALT, so the *same* emergence
    /// machinery runs over the *same* FOOD/WOOD barter every tick — but with no
    /// SALT in the economy the only swaps that clear are FOOD-for-WOOD, which are
    /// perfectly reciprocal (each trade counts one FOOD acceptance and one WOOD
    /// acceptance), so no good ever leads by the promotion margin and **nothing
    /// monetizes**. The settlement stays in barter. Paired with
    /// [`Self::barter_camp`] this isolates the cause: identical machinery and FOOD
    /// supply, the saleable medium's presence the only difference. If both
    /// monetized, the wiring would be reading something other than realized
    /// spatial barter.
    pub fn barter_camp_control() -> Self {
        Self::barter_camp_with_medium(false)
    }

    /// Shared builder for the barter camp and its control. `has_medium` toggles
    /// the SALT endowment: present (the camp, SALT circulates and emerges) or
    /// absent (the control, no medium, nothing leads). Everything else — the FOOD
    /// node, the rosters, the patient cultures, the reused M20 emergence envelope
    /// — is identical, so the pair is a clean falsification twin. Starting gold is
    /// zero on both sides (econ's V2 path requires zero initial money balances;
    /// the money good has not emerged yet).
    fn barter_camp_with_medium(has_medium: bool) -> Self {
        let exchange = Pos::new(0, 0);
        // The circulating medium's initial supply. Consumers hold the bulk; they
        // spend it down buying FOOD/WOOD from gatherers, so it changes hands and
        // earns the acceptances the saleability tracker reads. Zero on both sides
        // in the control.
        let (gatherer_salt, consumer_salt) = if has_medium { (0, 80) } else { (0, 0) };
        Self {
            width: 64,
            height: 1,
            exchange,
            exchange_cap: 1_000_000,
            // TWO close, rich gathered goods: a FOOD node and a WOOD node. The
            // gatherers split round-robin (half haul FOOD, half WOOD), so FOOD and
            // WOOD each have specialist sellers — and the durable SALT medium,
            // held by the consumers, is the good both kinds of haul trade against,
            // the common counterpart that makes it the most saleable. SALT is NOT
            // gathered (it is the endowed medium), so the world never regenerates
            // the money good and the promotion conversion is clean.
            nodes: vec![
                NodeSpec {
                    good: FOOD,
                    pos: Pos::new(2, 0),
                    stock: 8_000,
                    regen: 64,
                    cap: 8_000,
                },
                NodeSpec {
                    good: WOOD,
                    pos: Pos::new(3, 0),
                    stock: 8_000,
                    regen: 64,
                    cap: 8_000,
                },
            ],
            // Eight gatherers split round-robin over the two nodes (four haul
            // FOOD, four haul WOOD) and four medium-holding consumers — the roster
            // that makes the medium the common counterpart both kinds of haul trade
            // against.
            gatherers: 8,
            consumers: 4,
            carry_cap: 6,
            move_speed: 1,
            // Barter start: no money is designated, so colonists hold no gold (the
            // econ V2 path requires zero initial money balances).
            starting_gold_gatherer: 0,
            starting_gold_consumer: 0,
            // Tight survival buffers so a specialist gatherer (hauling only one of
            // FOOD/WOOD) must TRADE for the good it does not produce — the strong
            // gains-from-trade that drive a thick barter book. The consumers carry
            // almost no WOOD, so they buy WOOD (as well as FOOD) with the medium:
            // it is the consumers demanding BOTH gathered goods through the medium
            // that makes the medium the most-saleable hub, not merely the FOOD
            // side. Each buffer bridges only the haul warmup.
            gatherer_food_buffer: 6,
            gatherer_wood_buffer: 6,
            consumer_food_buffer: 4,
            consumer_wood_endowment: 1,
            // Patient on both sides (a low time preference) so colonists keep
            // offering their surplus rather than hoarding it — the sustained supply
            // the medium circulates against.
            gatherer_time_preference_base_bps: 400,
            consumer_time_preference_base_bps: 400,
            leisure_weight_base_bps: 3_000,
            // Hunger-resilient (like `price_probe`): hunger never reaches the
            // critical ceiling, so the camp does not die off mid-emergence. The
            // milestone is the MONEY-EMERGENCE mechanism, not a survival race —
            // decoupling the two keeps the proof about the saleability dynamics
            // (the same discipline the distance→price probe uses).
            dynamics: {
                let mut d = NeedDynamics::lab_default();
                d.hunger_critical = d.need_max + 1;
                d
            },
            resident_traders: Vec::new(),
            chain: None,
            demography: None,
            m3: false,
            barter: Some(BarterConfig {
                menger: MengerianConfig {
                    candidate_goods: vec![FOOD, WOOD, SALT],
                    ..MengerianConfig::default()
                },
                medium_good: SALT,
                medium_want_qty: 6,
                gatherer_medium_endowment: gatherer_salt,
                consumer_medium_endowment: consumer_salt,
            }),
            bank: None,
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

    /// The G4b **two-lineage demography** settlement: a non-spatial colony of two
    /// households — a patient one and a present-biased one — whose members age, die
    /// of old age (via the G4a removal path), and reproduce, children inheriting their
    /// parents' mutated [`CultureParams`]. There are no gatherers or nodes: each
    /// household feeds its members a renewable FOOD provision (so deaths are old age,
    /// not starvation) and the patient household also gets a WOOD surplus it sells —
    /// gold flows from the present-biased buyers to the patient savers, so the patient
    /// lineage out-accumulates the other (sign only; the selection demonstration). See
    /// [`DemographyConfig::lineages`].
    pub fn lineages() -> Self {
        let mut config = Self::viable();
        // Non-spatial: no gatherers and no nodes (the households' provisions feed the
        // colony directly). A tiny grid holds just the exchange tile every colonist
        // nominally sits on.
        config.width = 4;
        config.height = 1;
        config.gatherers = 0;
        config.consumers = 0;
        config.nodes = Vec::new();
        config.demography = Some(DemographyConfig::lineages());
        config
    }

    /// The G5b **frontier** — emergence composed with the full stack in ONE
    /// settlement: a barter camp where **money emerges**, then **producer roles**
    /// take up milling/baking from the resulting price spreads, while **births and
    /// deaths** run demographic selection — all conserving and deterministic.
    ///
    /// It composes three reused mechanisms unchanged:
    /// - **G5a money emergence** — colonists barter goods-for-goods for a durable
    ///   SALT medium until the lab's Mengerian `winner` rule promotes it; from the
    ///   next tick trade is money-priced. Bread (the chain staple, from buffers and
    ///   the household hearth) and WOOD (warmth, gathered) are the two counterpart
    ///   goods the universal SALT demand trades against — the saleability hub that
    ///   makes SALT the money good (the spatial analogue of `barter_camp`'s FOOD/WOOD).
    /// - **G3b production roles** — a latent miller/baker pool starts `Unassigned`
    ///   and *adopts* its vocation only when the realized **money** spread pays, which
    ///   exists only post-promotion: a division of labor follows the medium of
    ///   exchange (role-choice is gated on the money phase).
    /// - **G4b demography** — two households (a patient and a present-biased lineage)
    ///   whose non-spatial members are provisioned the **bread staple** + WOOD, age,
    ///   die of old age (via the G4a removal path), and reproduce, children inheriting
    ///   their parents' mutated culture.
    ///
    /// Every gold source is **zero** before promotion (the econ V2 path requires zero
    /// initial money balances; `generate` asserts it), and hunger is resilient (it
    /// never reaches the critical ceiling) so the camp survives the emergence window
    /// and the only deaths are old age. The buffers are generous *mechanism* knobs that
    /// bridge the barter window and the chain's pipeline fill — sign/conservation only,
    /// no magnitude pinned. The promotion-rejection list (nodes ∪ recipe outputs ∪ the
    /// demography hearth) vetoes every renewable good, so SALT — the one durable,
    /// non-renewable candidate — is what monetizes (or nothing does).
    pub fn frontier() -> Self {
        // The emergent grain→flour→bread chain (no seeded roles — millers/bakers
        // *adopt* from the post-promotion spread). Zero producer gold: a barter-start
        // settlement holds no money before promotion.
        let mut chain = ChainConfig::grain_flour_bread();
        chain.millers = 0;
        chain.bakers = 0;
        chain.latent_millers = 3;
        chain.latent_bakers = 3;
        chain.operating_cost = 1;
        chain.bread_is_staple = true;
        chain.throughput = 1;
        chain.miller_grain_buffer = 0;
        chain.baker_flour_buffer = 0;
        // A latent miller's flour bootstrap stock, so the first adopted baker's flour
        // bid finds a seller and flour realizes a price (the chain prices bottom-up).
        chain.latent_flour_seed = 12;
        // A generous bread surplus bridges the whole barter window (the chain produces
        // no bread until roles adopt post-promotion) and gives every non-consumer bread
        // to offer — the bread side of the SALT saleability hub.
        chain.bread_buffer = 64;
        // Consumers start nearly bread-empty so they *buy* bread (with SALT) from the
        // first ticks — that demand is the bread side of the barter hub and, after
        // promotion, the realized bread price the first baker adopts on.
        chain.consumer_staple_buffer = 2;
        chain.wood_buffer = 48;
        // Consumers are also WOOD-poor: with almost no WOOD of their own they pay the
        // SALT medium (not their own WOOD) for both bread AND WOOD, so the SALT-rich
        // consumers are the buyers of both barter counterparts — the saleability hub
        // (exactly `barter_camp`'s goods-poor/medium-rich consumer) that lets SALT win.
        chain.consumer_wood_buffer = 1;
        // Barter start: no money before promotion.
        chain.producer_gold = 0;

        // Two lineages, food-secure (so deaths are old age) with a fast aging cadence
        // so deaths fall inside a modest horizon; the patient lineage gets a WOOD
        // surplus it sells (selection sign). All gold sources are zero (barter start).
        let demography = DemographyConfig {
            households: vec![
                HouseholdSpec {
                    founders: 2,
                    time_preference_base_bps: 500,
                    food_provision: 3,
                    wood_provision: 3,
                    starting_gold: 0,
                    starting_food: 8,
                    starting_wood: 6,
                },
                HouseholdSpec {
                    founders: 2,
                    time_preference_base_bps: 9_400,
                    food_provision: 3,
                    wood_provision: 0,
                    starting_gold: 0,
                    starting_food: 8,
                    starting_wood: 6,
                },
            ],
            ticks_per_year: 6,
            old_age_onset_years: 3,
            lifespan_span_years: 3,
            birth_interval: 4,
            birth_hunger_ceiling: 12,
            max_household_size: 5,
            child_food_endowment: 4,
            // Barter start: a newborn inherits no money before promotion.
            child_gold_endowment: 0,
            mutation_delta_bps: 200,
        };

        let exchange = Pos::new(0, 0);
        let bread = chain.content.bread();
        Self {
            width: 64,
            height: 1,
            exchange,
            exchange_cap: 1_000_000,
            // Two close, rich gathered goods: grain (the chain's raw input) and WOOD
            // (warmth). The gatherers split round-robin. WOOD is one barter counterpart
            // for SALT (everyone warms with it); grain feeds the chain post-promotion.
            // Neither is the money good — both are in the promotion-rejection list.
            nodes: vec![
                NodeSpec {
                    good: chain.content.grain(),
                    pos: Pos::new(2, 0),
                    stock: 8_000,
                    regen: 64,
                    cap: 8_000,
                },
                NodeSpec {
                    good: WOOD,
                    pos: Pos::new(3, 0),
                    stock: 8_000,
                    regen: 64,
                    cap: 8_000,
                },
            ],
            gatherers: 8,
            consumers: 4,
            carry_cap: 6,
            move_speed: 1,
            // Barter start: no money is designated, so colonists hold no gold.
            starting_gold_gatherer: 0,
            starting_gold_consumer: 0,
            // These FOOD-buffer knobs are unused on the chain path (the staple is
            // bread, seeded via `ChainConfig`); kept at zero for a consistent read.
            gatherer_food_buffer: 0,
            gatherer_wood_buffer: 0,
            consumer_food_buffer: 0,
            consumer_wood_endowment: 0,
            // Patient on both sides (a low time preference) so colonists keep offering
            // their surplus rather than hoarding it — the sustained supply the medium
            // circulates against, and the savings want the role-choice appraisal targets.
            gatherer_time_preference_base_bps: 400,
            consumer_time_preference_base_bps: 400,
            leisure_weight_base_bps: 3_000,
            // Hunger-resilient (like `barter_camp`): hunger never reaches the critical
            // ceiling, so the camp survives the emergence window and the only deaths are
            // old age (the demographic selection signal), not a mid-emergence die-off.
            dynamics: {
                let mut d = NeedDynamics::lab_default();
                d.hunger_critical = d.need_max + 1;
                d
            },
            resident_traders: Vec::new(),
            chain: Some(chain),
            demography: Some(demography),
            m3: false,
            barter: Some(BarterConfig {
                menger: MengerianConfig {
                    // The candidate set the saleability tracker watches. SALT is the
                    // durable medium; bread and WOOD are the renewable counterparts it
                    // trades against (both vetoed by the rejection list, so if either
                    // ever led it could not commit). SALT is the universal hub, so it
                    // is what actually monetizes.
                    candidate_goods: vec![WOOD, bread, SALT],
                    ..MengerianConfig::default()
                },
                medium_good: SALT,
                medium_want_qty: 6,
                gatherer_medium_endowment: 0,
                consumer_medium_endowment: 80,
            }),
            bank: None,
        }
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
    /// Goods created by node regen during the fast loop (a source).
    pub regen: BTreeMap<GoodId, u64>,
    /// Goods minted by the G4b per-member **provision** (the household hearth) — a
    /// conserved source, like `regen`, delivered directly into econ stock. Empty for
    /// a non-demography settlement, so the conservation identity is unchanged there.
    pub endowment: BTreeMap<GoodId, u64>,
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
    /// Units of a good **converted to money** by a G5a emergence **promotion**
    /// this tick — the econ-stock of the winning good that the lab's conserved
    /// promotion turned into `Gold` units 1-for-1. The good→money side of the
    /// phase transition: it leaves the physical ledger (a sink for that good) and
    /// reappears as gold (the gold checkpoints account it). Empty on every tick
    /// but the single promotion tick, and on every non-emergent settlement, so the
    /// conservation identity is unchanged elsewhere.
    pub promoted: BTreeMap<GoodId, u64>,
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
    /// Deaths this tick — starvation (any config) plus old age (G4b).
    pub deaths: u32,
    /// Births this tick (G4b). Zero for a non-demography settlement.
    pub births: u32,
    /// G6b: **Knowledge** produced by scholar labor this tick — the accumulator's
    /// increment, reported on its OWN non-conserved line (NOT in the goods ledger).
    /// Knowledge is monotonic, never traded or consumed, so it is deliberately
    /// excluded from [`Self::conserves`]; the conserved good *inputs* to research
    /// (e.g. grain) ARE accounted in `consumed_as_input`. Zero for a non-research
    /// settlement.
    pub knowledge_produced: u64,
}

impl EconTickReport {
    pub fn regen_of(&self, good: GoodId) -> u64 {
        self.regen.get(&good).copied().unwrap_or(0)
    }
    /// Units of `good` provisioned (G4b household hearth) this tick — a source.
    pub fn endowment_of(&self, good: GoodId) -> u64 {
        self.endowment.get(&good).copied().unwrap_or(0)
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
    /// Units of `good` converted to money by a G5a promotion this tick (a sink
    /// for the physical good, matched 1-for-1 by the gold the promotion mints).
    pub fn promoted_of(&self, good: GoodId) -> u64 {
        self.promoted.get(&good).copied().unwrap_or(0)
    }
    /// G6b: Knowledge produced this tick — the non-conserved accumulator line. It is
    /// NOT part of the goods-conservation identity (see [`Self::conserves`]).
    pub fn knowledge_produced(&self) -> u64 {
        self.knowledge_produced
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
    /// after(X) == before(X) + regen(X) + endowment(X) + produced(X)
    ///                       − consumed_as_input(X) − consumed(X) − promoted(X)
    /// ```
    ///
    /// For a plain settlement `endowment`/`produced`/`consumed_as_input`/`promoted`
    /// are empty, so this reduces exactly to the G2b form (every existing test stays
    /// green). `endowment` is the G4b household provision (a source); `promoted` is
    /// the G5a good→money conversion (a sink for the promoted good, matched 1-for-1
    /// by the gold the promotion mints — accounted in the gold checkpoints). Births
    /// and deaths move goods *within* the whole system (parent→child,
    /// dead→heir/commons) so they cancel in `before`/`after` and need no term here.
    /// Tools are durable — they appear in neither production term, so a recipe that
    /// needs a tool never moves the tool's ledger.
    ///
    /// G6b: **Knowledge** is deliberately absent from this identity. It is not a good
    /// (not in [`Settlement::tracked_goods`], so not a key of `whole_system_before`),
    /// it is monotonic and never traded or consumed, and `sim` reports it on its own
    /// non-conserved line ([`Self::knowledge_produced`]). The conserved good *inputs*
    /// to research (e.g. grain) DO appear here, in `consumed_as_input` — so research
    /// consumption is accounted exactly like ordinary consumption.
    pub fn conserves(&self) -> bool {
        self.whole_system_before.keys().all(|good| {
            let before = self.whole_system_before_of(*good) as i128;
            let after = self.whole_system_after_of(*good) as i128;
            let regen = self.regen_of(*good) as i128;
            let endowment = self.endowment_of(*good) as i128;
            let consumed = self.consumed_of(*good) as i128;
            let produced = self.produced_of(*good) as i128;
            let consumed_as_input = self.consumed_as_input_of(*good) as i128;
            let promoted = self.promoted_of(*good) as i128;
            after == before + regen + endowment + produced - consumed_as_input - consumed - promoted
        })
    }
}

/// Where a dead colonist's estate was routed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EstateDestination {
    /// The estate settled to the settlement commons.
    Commons,
    /// The estate settled to a living member of the dead colonist's household.
    Household { household: usize, heir: AgentId },
}

/// Single-pass lineage dashboard stats for one household.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineageStats {
    pub living: usize,
    pub gold: u64,
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
    /// id resolves to `None` in the arena; a dead gatherer's carry settles to the
    /// commons and its spatial world agent is removed after that drain.
    alive: bool,
    /// G3b: the recipe this colonist *could* run with its latent tool, if any.
    /// `Some(Mill)` for a latent miller (holds a mill), `Some(Bake)` for a latent
    /// baker (holds an oven); `None` for a gatherer, consumer, or a **seeded** G3a
    /// producer. The role-choice phase re-appraises this recipe each tick and
    /// toggles [`Vocation::Unassigned`] ↔ the producer vocation from the realized
    /// spread; a `None` colonist is never re-appraised, so the seeded G3a config is
    /// byte-identical (its producers are permanent).
    latent: Option<RecipeId>,
    /// G4b demography: the household (lineage) this colonist belongs to, indexing
    /// [`Settlement::households`], or `None` for a non-demography colonist
    /// (gatherer/consumer/producer in a pre-G4b config). Drives the per-member
    /// provision, the birth roster, and estate routing to heirs.
    household: Option<usize>,
    /// G4b: age in **econ ticks** since birth (founders seeded with a staggered
    /// starting age). Advanced once per econ tick for a living demography colonist;
    /// `0` and untouched for a non-demography colonist.
    age: u64,
    /// G4b: the colonist's deterministic old-age lifespan in econ ticks — it dies of
    /// old age (via the G4a removal path) once `age >= lifespan`. `None` (no old-age
    /// mortality) for a non-demography colonist.
    lifespan: Option<u64>,
    /// G4b: a stable per-colonist seed — the deterministic source of its lifespan and
    /// (as a parent) its children's mutation and seeds. No loop-time `Rng` draws from
    /// it. `0` for a non-demography colonist.
    seed: u64,
    /// The settlement destination recorded once this colonist dies and its estate
    /// is collected. `None` while alive.
    estate_destination: Option<EstateDestination>,
}

/// A settlement of generated colonists driven over a real `world` + `econ`.
pub struct Settlement {
    world: World,
    society: Society,
    colonists: Vec<Colonist>,
    /// Live colonist slots in colonist-insertion order. Dead historical entries stay
    /// addressable for tests/viewer, but hot tick phases iterate this compact roster.
    live_colonist_slots: Vec<usize>,
    /// Stable id -> colonist slot, including dead historical colonists. This avoids a
    /// history-length search when a reused numeric id appears in the econ logs.
    colonist_slot_by_id: BTreeMap<AgentId, usize>,
    dynamics: NeedDynamics,
    known: KnownGoods,
    exchange: StockpileId,
    carry_cap: u32,
    /// Physical goods tracked for whole-system conservation (node goods ∪ goods
    /// any colonist starts with), `GoodId`-ordered. GOLD (money) is excluded.
    goods: Vec<GoodId>,
    /// The commodity-money **promotion rejection list**: goods a settlement's own
    /// substrate keeps regenerating, so econ's `winner` rule must not be allowed to
    /// commit one as money. `GoodId`-ordered. A promotion to one of these is
    /// unsupported because future minting would create physical units of the money
    /// good *after* econ has removed it from the money-priced market, breaking the
    /// conserved promotion. It covers every **renewable** source a settlement runs:
    ///
    /// - the spatial **resource nodes** (the (non-GOLD) node goods) — the world
    ///   regenerates them (the G5a slice's only renewable source);
    /// - the production-chain **recipe outputs** (flour, bread) — a producer keeps
    ///   minting them every tick (G3a/G3b);
    /// - the G4b **demography** provision goods (the hunger staple + WOOD) — the
    ///   renewable household hearth keeps minting them.
    ///
    /// The G5b frontier composes all three, so the list finally bites: the durable
    /// emergent **medium** (e.g. SALT) is the only candidate that is none of these, so
    /// it is the only good the camp can monetize. A designated-money settlement never
    /// consults this list (it runs `step`, not `step_rejecting_v2_money_goods`).
    money_rejection_goods: Vec<GoodId>,
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
    /// The G4b **demography** overlay config, or `None` for a pre-G4b settlement
    /// (every demography phase is then skipped, so the run is byte-identical to
    /// G3/G4a). Read each tick to drive provisions, aging/mortality, and births.
    demography: Option<DemographyConfig>,
    /// Per-household runtime state (the birth cadence), index-parallel to
    /// `demography.households`. Empty for a non-demography settlement.
    households: Vec<HouseholdRuntime>,
    /// The colony's monotonic **birth sequence** counter — the stable, unique number
    /// per birth that seeds the child's deterministic culture mutation and its own
    /// seed (no loop-time `Rng`). Never decreases; reused arena slots get fresh
    /// children, so a sequence number is never reissued.
    birth_seq: u64,
    /// Lifetime birth count (the viewer/acceptance surface).
    births_total: u64,
    /// Lifetime old-age death count (distinct from starvation deaths).
    old_age_deaths_total: u64,
    /// The G5a barter-start overlay config, retained because its knobs steer future
    /// ticks even before they leave a runtime trace. Non-emergent settlements keep
    /// this `None`, so their canonical state layout is unchanged.
    barter: Option<BarterConfig>,
    /// The G5a emergent **medium** demand `(good, want_qty)`, or `None` for a
    /// non-emergent settlement. While the settlement is still in barter, each
    /// colonist's freshly regenerated value scale is extended with `want_qty`
    /// `Horizon::Next` wants for `good` (the demand that drives barter for the
    /// medium). Dropped once a money good has emerged — the post-promotion scale
    /// is pure need-driven (the money market clears in GOLD like G2b).
    barter_medium: Option<(GoodId, u32)>,
    /// G6b: the settlement's accumulated **Knowledge** — produced by scholar labor,
    /// monotonic, never traded or consumed. It is OUTSIDE the goods-conservation
    /// ledger (it is not a good, not in [`Settlement::goods`]); the per-tick
    /// [`EconTickReport::knowledge_produced`] reports the increment on its own
    /// non-conserved line. `0` and untouched for a non-research settlement.
    knowledge: u64,
    /// G6b: the econ tick at which Knowledge first crossed the tier-2 threshold and
    /// the gated recipe was enabled, or `None` if it has not (yet) unlocked. The
    /// unlock is **one-way** — once set, never cleared, so the tier never flaps.
    tier2_unlocked_at: Option<u64>,
    /// The G8b **bank charter** overlay config, or `None` for a bank-free settlement
    /// (every bank phase is then skipped, so the run is byte-identical to G8a). When
    /// `Some`, one econ [`Bank`] (id [`BANK_ID`]) is chartered in `society.banks` and
    /// [`Settlement::run_bank_phase`] runs deposits + fiduciary lending each econ
    /// tick. Held as a detached `Copy` of the config so the bank phase needs no
    /// borrow of the original config.
    bank: Option<BankConfig>,
}

/// Per-household birth-cadence runtime (G4b), index-parallel to a
/// [`DemographyConfig`]'s households. The household's *membership* lives on the
/// colonists (`Colonist::household`); only the cadence needs mutable runtime state.
struct HouseholdRuntime {
    /// The econ tick of this household's most recent birth, or `None` if it has not
    /// birthed yet — the birth-interval gate reads it.
    last_birth_tick: Option<u64>,
}

/// The per-settlement production-chain runtime (G3a): the interned content and
/// the per-producer throughput cap. Read-only after generation.
struct ChainRuntime {
    content: ContentSet,
    throughput: u32,
    /// The per-operation cost (labor + tool) the G3b role-choice appraisal charges
    /// against a recipe's realized output spread (see [`ChainConfig::operating_cost`]).
    operating_cost: u64,
    /// G6b: the Knowledge threshold that unlocks tier 2. `0` (no tech tiers) for a
    /// non-research chain — the unlock check is then a no-op.
    tier2_threshold: u64,
    /// G6b: the tier-2 recipe id to flip `enabled` on unlock, or `None` for a
    /// non-research chain.
    tier2_recipe_id: Option<RecipeId>,
    /// G6b: the grain a scholar holds/reserves (its research input buffer + bid
    /// ceiling). `0` for a non-research chain.
    scholar_grain_buffer: u32,
    /// G6b: the flour a confectioner holds/reserves (its tier-2 input buffer + bid
    /// ceiling). `0` for a non-research chain.
    confectioner_flour_buffer: u32,
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
        let dynamics = config.dynamics;
        // The need→good mapping. A plain settlement uses the lab default
        // (hunger ↔ FOOD). The G3a chain and the G3b emergent config make **bread
        // the staple** (hunger ↔ bread) so the chain's final good is what colonists
        // eat to live, and that demand prices bread. The G3b no-spread control sets
        // `bread_is_staple = false`, keeping hunger ↔ FOOD so bread is never demanded
        // (and so never prices, and so no role forms). Warmth stays WOOD.
        let known = match (&config.chain, &config.barter) {
            // G5b **frontier**: a bread-staple chain composed with the barter-start
            // medium. Hunger ↔ bread (the chain's demand pulls the chain into being),
            // warmth WOOD, and savings is the **emergent medium** (e.g. SALT) — the
            // good that monetizes. Post-promotion the money market provisions that
            // store-of-value want with the emerged money exactly like the plain barter
            // camp, and the role-choice appraisal targets that same future-money want
            // (threaded with the current money good, not assumed to be GOLD).
            (Some(chain), Some(barter)) if chain.bread_is_staple => KnownGoods {
                hunger: chain.content.bread(),
                warmth: WOOD,
                savings: barter.medium_good,
            },
            (Some(chain), _) if chain.bread_is_staple => KnownGoods {
                hunger: chain.content.bread(),
                warmth: WOOD,
                savings: GOLD,
            },
            // The G5a barter camp (no chain) eats gathered FOOD, warms with WOOD,
            // and **saves the emergent medium** (e.g. SALT). Saving the good that
            // becomes money is what the lab's emergence scenarios do, and it is
            // load-bearing for the money phase: the promotion converts the medium
            // stock to gold while leaving the medium's place on every scale, so the
            // money market provisions those store-of-value wants with gold and
            // colonists trade FOOD/WOOD for money exactly like a designated-money
            // settlement. (Pre-promotion the medium is also demanded as a NEAR want
            // via a separate scale extension; that is what drives the barter for
            // it — a `Later` savings want alone never barters.)
            (None, Some(barter)) => KnownGoods {
                hunger: FOOD,
                warmth: WOOD,
                savings: barter.medium_good,
            },
            // A barter-start chain whose bread is NOT the staple (hunger stays FOOD,
            // the no-spread control's shape) still circulates and is endowed the
            // emergent medium: `build_agent` always adds `barter.medium_good` under a
            // barter overlay and the post-promotion market runs `step_rejecting_v2_*`,
            // so the savings want MUST be that medium too. Falling through to
            // `lab_default` (savings GOLD) would save GOLD while the agent holds and
            // the market clears a non-GOLD medium, and `run_role_choice`'s
            // `soonest_savings_horizon(money_good)` would then find no matching want and
            // never adopt a role. No shipped config reaches this arm today (the
            // frontier is bread-staple; the no-spread control has no barter), but every
            // barter-start chain must keep its savings coherent with its medium.
            (Some(_), Some(barter)) => KnownGoods {
                hunger: FOOD,
                warmth: WOOD,
                savings: barter.medium_good,
            },
            // The control (chain present, bread not the staple) eats seeded FOOD;
            // every plain settlement eats gathered FOOD, warms with WOOD, saves GOLD.
            (Some(_), None) | (None, None) => KnownGoods::lab_default(),
        };
        // The G5a barter overlay was the MECHANISM slice: a plain gatherer/consumer
        // camp. G5b **composes** it with production (a chain) and demography (the
        // `frontier` config), so that mutual-exclusion is lifted. What still holds is
        // that the emergent medium must be **non-renewable**: a good the settlement's
        // own substrate keeps minting (a gathered node good, a recipe output, or a
        // demography-provisioned staple) cannot be the money good, because future
        // minting would create physical units of it *after* econ removed it from the
        // money-priced market, breaking the conserved promotion. The promotion
        // rejection list (`money_rejection_goods`) enforces that at the step boundary;
        // these asserts reject the unsupportable medium loudly at generation.
        if let Some(barter) = &config.barter {
            assert!(
                config
                    .nodes
                    .iter()
                    .all(|spec| spec.good != barter.medium_good),
                "the emergent medium must not be a gathered node good (the world would \
                 regenerate the money good, breaking the conserved promotion)"
            );
            // A chain's goods (the gathered raw, the recipe outputs, the durable tools)
            // are all renewable or capital — none can be the money good. Reject a medium
            // that names one rather than ship a config whose chain would re-mint the
            // money good after promotion.
            assert!(
                config
                    .chain
                    .as_ref()
                    .is_none_or(|chain| !chain.content.goods().contains(&barter.medium_good)),
                "the emergent medium must not be a production-chain good (a recipe output \
                 or raw input the chain keeps producing, breaking the conserved promotion)"
            );
            // The demography household hearth provisions the hunger staple and WOOD every
            // tick — both renewable sources. The medium must be neither, or the promotion
            // would convert a stock the provision keeps refilling.
            assert!(
                config.demography.is_none()
                    || (barter.medium_good != WOOD && barter.medium_good != known.hunger),
                "the emergent medium must not be a demography-provisioned good (the \
                 household hearth would keep minting the money good after promotion)"
            );
            // The emergent medium is a PHYSICAL good that circulates as barter stock
            // before promotion, so it must not be GOLD: GOLD is the money ledger, not
            // a physical good — it never enters `self.goods`, the deposit attribution,
            // the transfer, or the conservation report. A GOLD medium endowment would
            // mint stock the digest and whole-system ledger never track (a silent
            // money leak), and the promotion's good→money conversion is meaningless
            // when the "good" is already money. Reject it at the seam.
            assert!(
                barter.medium_good != GOLD,
                "the emergent medium cannot be GOLD; GOLD is the money ledger, not a \
                 physical good, so an endowed GOLD medium would create untracked stock \
                 the conservation report and digest never see"
            );
            assert!(
                config.starting_gold_gatherer == 0 && config.starting_gold_consumer == 0,
                "a barter-start camp holds no money before promotion (econ's V2 path \
                 requires zero initial money balances)"
            );
            // The G5b frontier composes the camp with a production chain and demography,
            // each of which has its OWN gold endowment knob. The V2 promotion converts
            // each agent's medium stock to gold and refuses to commit if ANY agent
            // already holds gold (`NonZeroMoneyBalance`), so every gold source — the
            // producers' working capital, the household founders' starting gold, and the
            // newborn gift — must also be zero before promotion. Reject a composed config
            // that seeds money loudly here rather than silently never-promote.
            assert!(
                config
                    .chain
                    .as_ref()
                    .is_none_or(|chain| chain.producer_gold == 0),
                "a barter-start frontier holds no money before promotion: a chain's \
                 producer_gold must be 0 under a barter overlay"
            );
            assert!(
                config.demography.as_ref().is_none_or(|demo| {
                    demo.child_gold_endowment == 0
                        && demo.households.iter().all(|h| h.starting_gold == 0)
                }),
                "a barter-start frontier holds no money before promotion: demography \
                 starting_gold and child_gold_endowment must be 0 under a barter overlay"
            );
        }
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
        // The G4b demography overlay provisions the **hunger staple** as the household
        // hearth (`deliver_demography_provisions`, the birth food gate, and the newborn
        // endowment all use [`KnownGoods::hunger`]). A plain/`lineages` settlement maps
        // hunger ↔ FOOD, so it provisions FOOD exactly as G4b did (byte-identical); the
        // G5b frontier maps hunger ↔ bread, so the same path provisions bread — members
        // are always fed the good they eat, so the pre-G5b "non-FOOD staple starves the
        // household" hazard cannot arise and needs no generation guard.
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
        // G6b seeded scholars + confectioners: both zero without a research chain, so
        // every pre-G6b config's population, ids, and digest are byte-identical. They
        // follow the latent pool in id order (the highest colonist ids).
        let (scholars, confectioners) = match &config.chain {
            Some(chain) => (
                usize::from(chain.scholars),
                usize::from(chain.confectioners),
            ),
            None => (0, 0),
        };
        let population = consumers
            + gatherers
            + millers
            + bakers
            + latent_millers
            + latent_bakers
            + scholars
            + confectioners;

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
            let latent_end = seeded_end + latent_millers + latent_bakers;
            let scholar_end = latent_end + scholars;
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
            } else if index < latent_end {
                (
                    Vocation::Unassigned,
                    None,
                    config.consumer_time_preference_base_bps,
                    Some(RecipeId::Bake),
                )
            } else if index < scholar_end {
                // G6b: a seeded scholar — patient (so it carries a savings want and
                // keeps offering nothing it needs), holding a library + grain buffer.
                (
                    Vocation::Scholar,
                    None,
                    config.consumer_time_preference_base_bps,
                    None,
                )
            } else {
                // G6b: a seeded confectioner — holds an atelier + flour buffer, runs
                // the tier-2 recipe once unlocked.
                (
                    Vocation::Confectioner,
                    None,
                    config.consumer_time_preference_base_bps,
                    None,
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
                // Pre-G4b colonists carry no demography state (no household, no
                // aging, no old-age mortality), so a no-demography settlement is
                // byte-identical to G3/G4a.
                household: None,
                age: 0,
                lifespan: None,
                seed: 0,
                estate_destination: None,
            });
        }

        // ---- G4b demography founders: the non-spatial household members ----
        // When a demography overlay is present, its households' founders follow the
        // normal colonist roster in id order (a non-demography settlement adds none,
        // so it is byte-identical). A founder is a NON-SPATIAL householder: it has
        // an econ agent but **no world agent** — it never hauls, so the fast loop,
        // the deposit transfer, and the world↔econ id coincidence the gatherers rely
        // on are untouched. Its stable seed (hashed from the world seed + its global
        // founder index — no extra `Rng` draw) fixes its staggered starting age and
        // its deterministic old-age lifespan; its culture is drawn from the
        // household's time-preference base (the heritable ordinal bias).
        let mut households: Vec<HouseholdRuntime> = Vec::new();
        if let Some(demo) = &config.demography {
            let mut founder_index = 0usize;
            for (household_index, spec) in demo.households.iter().enumerate() {
                households.push(HouseholdRuntime {
                    last_birth_tick: None,
                });
                for _ in 0..spec.founders {
                    let id = AgentId(colonist_id_base + colonists.len() as u64);
                    let seed = founder_seed(seed, founder_index);
                    founder_index += 1;
                    let culture = draw_culture(
                        &mut rng,
                        spec.time_preference_base_bps,
                        config.leisure_weight_base_bps,
                    );
                    let need = NeedState::rested();
                    agents.push(build_demography_agent(id, &need, &culture, &known, spec));
                    colonists.push(Colonist {
                        id,
                        vocation: Vocation::Consumer,
                        node: None,
                        need,
                        culture,
                        critical_streak: 0,
                        alive: true,
                        latent: None,
                        household: Some(household_index),
                        age: demo.founder_start_age_ticks(seed),
                        lifespan: Some(demo.lifespan_ticks(seed)),
                        seed,
                        estate_destination: None,
                    });
                }
            }
        }

        // The promotion rejection list (see the `money_rejection_goods` field doc):
        // every renewable source the settlement runs, so econ's `winner` rule can
        // never commit a good the substrate keeps minting. The G5a slice had only the
        // spatial nodes; the G5b frontier adds the chain's recipe outputs and the
        // demography hearth, so the list finally bites and the durable medium (e.g.
        // SALT) is the only candidate left that the camp can monetize.
        let mut money_rejection_goods: Vec<GoodId> = Vec::new();
        let reject = |good: GoodId, list: &mut Vec<GoodId>| {
            if good != GOLD && !list.contains(&good) {
                list.push(good);
            }
        };
        // The spatial resource nodes (the world regenerates them).
        for spec in &config.nodes {
            reject(spec.good, &mut money_rejection_goods);
        }
        // The production-chain recipe outputs (a producer keeps minting them). Tools
        // are durable capital, never an emergent-money candidate, but rejecting them
        // too is harmless and keeps the list "no chain good can be money".
        if let Some(chain) = &config.chain {
            for good in chain.content.goods() {
                reject(good, &mut money_rejection_goods);
            }
        }
        // The demography household hearth (the renewable provision): the hunger staple
        // and WOOD. Empty without a demography overlay.
        if config.demography.is_some() {
            reject(known.hunger, &mut money_rejection_goods);
            reject(WOOD, &mut money_rejection_goods);
        }
        money_rejection_goods.sort();

        // The goods tracked for conservation: node goods plus anything a colonist
        // or resident trader starts holding (FOOD via nodes/buffers, WOOD via
        // endowments). Money is not a physical good, so it is excluded.
        let mut goods: Vec<GoodId> = Vec::new();
        let push_good = |g: GoodId, goods: &mut Vec<GoodId>| {
            if g != GOLD && !goods.contains(&g) {
                goods.push(g);
            }
        };
        // A demography settlement trades the hunger staple and WOOD (warmth) even if
        // a household starts a buffer at zero, and the per-member provision mints both
        // into econ stock — so both join the conservation ledger up front. The staple
        // is FOOD on a plain `lineages` colony and bread on the G5b frontier; both are
        // tracked here through [`KnownGoods::hunger`].
        if config.demography.is_some() {
            push_good(known.hunger, &mut goods);
            push_good(WOOD, &mut goods);
        }
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
        // The market regime. A plain/chain/demography settlement runs the
        // designated-GOLD M1 spot market (`Camp`'s natural seam: the
        // consumption-log readback and the realized-price accessor live on this
        // path). The G5a barter camp instead runs econ's V2 emergence machinery
        // (`MengerSaltMoney` → `ScenarioKind::MarketV2` + `Emergent`): `step_v2`
        // clears barter and feeds the SaleabilityTracker until the reused
        // Mengerian `winner` rule promotes a money good, after which the same
        // V2 money phase clears the money-priced market. Both log consumption
        // (the additive V2 logging G5a wired into econ) and realize prices.
        //
        // G8a adds the M3 ledger-money settlement: `EmergedGoldSoundControl` is the
        // pure-specie M3 scenario (`ScenarioKind::MarketM3`, SoundGold regime, no banks,
        // no issuers, no project lines, default specie tenders), so the society builds a
        // `MoneySystem` whose only active machinery is the ledger-settled spot market —
        // economically the same designated-GOLD market as M1, only ledger-accounted. The
        // money good is still GOLD (the specie). M3 is mutually exclusive with the barter
        // overlay (which runs the V2 emergent-money path).
        assert!(
            !(config.m3 && config.barter.is_some()),
            "an M3 ledger settlement is mutually exclusive with the barter (V2 emergent-money) overlay"
        );
        // G8b: a chartered bank takes deposits and lends fiduciary on the M3 ledger,
        // so it requires the M3 `MoneySystem` (there is no bank without ledger money).
        assert!(
            config.bank.is_none() || config.m3,
            "a chartered bank (G8b) requires the M3 ledger settlement (m3 = true)"
        );
        // The demography guard is layered intentionally BEFORE the layout-equality
        // guard below. `SettlementConfig::m3_settlement()` already has `demography:
        // None`, so the stricter layout check would reject a banked+demography config
        // regardless — but this earlier assert fires first to emit the *specific*
        // "cannot run with demography" message (the `bank_rejects_demography_until_
        // claim_estates_exist` test pins that wording). Keep both: this one names the
        // demography cause precisely (old-age/heir settlement of claims is unhandled —
        // the deposit-withdrawal-on-death below only covers the starvation path); the
        // layout check below scopes G8b to its two shipped bank controls.
        assert!(
            config.bank.is_none() || config.demography.is_none(),
            "a chartered bank (G8b) cannot run with demography until demand-claim \
             estate routing exists"
        );
        if let Some(bank_cfg) = config.bank {
            let mut bank_free_config = config.clone();
            bank_free_config.bank = None;
            assert!(
                bank_free_config == SettlementConfig::m3_settlement(),
                "a chartered bank (G8b) is limited to the curated M3 settlement layout \
                 (the shipped bank/full-reserve controls) until G8c finance"
            );
            assert!(
                is_supported_g8b_bank_charter(bank_cfg),
                "a chartered bank (G8b) is limited to the shipped bank/full-reserve \
                 charters until G8c finance"
            );
        }
        let (scenario_name, money) = match (&config.barter, config.m3) {
            (Some(barter), _) => (
                ScenarioName::MengerSaltMoney,
                MarketMoneyConfig::Emergent(barter.menger.clone()),
            ),
            (None, true) => (
                ScenarioName::EmergedGoldSoundControl,
                MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
            ),
            (None, false) => (
                ScenarioName::MarketBarterishGold,
                MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
            ),
        };
        let scenario = MarketScenario {
            name: "settlement",
            scenario: scenario_name,
            seed,
            periods: 0,
            agents,
            recipes,
            events: Vec::new(),
            money,
        };
        let mut society = Society::from_scenario(scenario);
        society.enable_consumption_log();

        // G8b: charter the bank. The bank is a *settlement* entity (config-chartered
        // here; the player-`Command` charter is G8c/UI), so the sim adds it after the
        // econ society is built rather than through a new econ scenario — the spot
        // market stays byte-identical to G8a. Two game-only wirings, both reusing the
        // existing M3 machinery unchanged:
        //
        // 1. the regime is moved to `FractionalConvertible` (econ's existing command
        //    surface, `apply_command(SetRegime)`) so the bank may issue fiduciary
        //    against fractional reserves — this is the bank's fixed operating regime,
        //    not the G8c regime *ladder* (which transitions regimes over time to drive
        //    the boom/bust cycle); and
        // 2. one econ `Bank` is pushed into `society.banks` with zero reserves — the
        //    deposit phase builds them. The ledger's `bank_reserves` is likewise zero
        //    at construction, so `sum(bank.reserves) == bank_reserves` holds and the
        //    money invariant reconciles from tick zero.
        //
        // The deposit/lend amounts run through the existing M3 ledger / bank paths in
        // `run_bank_phase`; no bank logic is added to econ. A `full-reserve` charter
        // is the falsification twin — its `fiduciary_lend_capacity` is zero, so the
        // same phase lends nothing.
        if let Some(bank_cfg) = config.bank {
            // `SetRegime` always applies (it only sets the field); the M3 society is
            // built `SoundGold`, which forbids fiduciary, so this is the one charter-
            // time move that lets a fractional bank lend at all.
            let result = society.apply_command(EventKind::SetRegime(Regime::FractionalConvertible));
            assert!(
                result.is_applied(),
                "setting the G8b bank operating regime must apply"
            );
            society.banks.push(Bank {
                id: BANK_ID,
                name: bank_cfg.name,
                reserves: Gold::ZERO,
                demand_deposits: Gold::ZERO,
                time_deposits: Gold::ZERO,
                loans_outstanding: Gold::ZERO,
                fiduciary_issued: Gold::ZERO,
                reserve_ratio_bps: bank_cfg.reserve_ratio_bps,
                convertible: true,
                policy: BankPolicy {
                    // Generous per-tick cap: the binding limit on lending is the
                    // reserve ratio (via `convertible_deposit_capacity`), not this.
                    max_new_fiduciary_per_tick: Gold(1_000_000_000),
                    // The one-unit loan policy must be nonzero for
                    // `fiduciary_lend_capacity` to be positive at all; the actual
                    // amount is gated by the reserve ratio.
                    loan_present: Gold(1),
                    loan_horizon: 7,
                    loan_future_due: Gold(1),
                    enabled: true,
                },
            });
        }

        // G8a resolves the G4b deferral: M3 (ledger-money) demography now settles. A
        // funded M3 colonist's death drains its ledger specie into the estate via
        // `Society::remove_agent` (`can_remove_agent` no longer refuses a funded specie
        // balance), the heir credit re-credits that specie through the ledger, and a
        // birth endowment is a conserved within-ledger `transfer_gold`. So demography
        // runs on either money regime; the G4b pre-G8a assert that forbade M3 demography
        // is retired (banks/fiat — not specie — remain G8b/c, and a fiat/claims balance
        // is still refused upstream).

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
                tier2_threshold: chain.tier2_threshold,
                tier2_recipe_id: chain.content.tier2_recipe_id(),
                scholar_grain_buffer: chain.scholar_grain_buffer,
                confectioner_flour_buffer: chain.confectioner_flour_buffer,
            }
        });

        let live_colonist_slots: Vec<usize> = (0..colonists.len()).collect();
        let colonist_slot_by_id: BTreeMap<AgentId, usize> = colonists
            .iter()
            .enumerate()
            .map(|(slot, colonist)| (colonist.id, slot))
            .collect();

        Self {
            world,
            society,
            colonists,
            live_colonist_slots,
            colonist_slot_by_id,
            dynamics,
            known,
            exchange,
            carry_cap: config.carry_cap,
            goods,
            money_rejection_goods,
            pending_deposits: BTreeMap::new(),
            trader_ids,
            chain,
            econ_tick: 0,
            last_report: EconTickReport::default(),
            commons_gold: Gold::ZERO,
            commons_stock: BTreeMap::new(),
            demography: config.demography.clone(),
            households,
            birth_seq: 0,
            births_total: 0,
            old_age_deaths_total: 0,
            barter: config.barter.clone(),
            // The medium-demand scale extension runs only when a medium is
            // actually supplied (the camp). The control endows none, so its
            // colonists carry no medium want — they barter FOOD-for-WOOD only, the
            // symmetric trade structure that cannot monetize. This is what makes
            // the pair a clean falsification twin: the medium (its demand AND its
            // supply) is the only difference.
            barter_medium: config.barter.as_ref().and_then(|barter| {
                let supplied =
                    barter.gatherer_medium_endowment > 0 || barter.consumer_medium_endowment > 0;
                supplied.then_some((barter.medium_good, barter.medium_want_qty))
            }),
            // G6b: Knowledge starts at zero and tier 2 starts locked. A non-research
            // settlement never touches either (no scholar runs, the threshold is 0),
            // so its digest is byte-identical.
            knowledge: 0,
            tier2_unlocked_at: None,
            // G8b: the chartered-bank config (or `None`). A detached copy — the bank
            // entity itself lives in `society.banks`; this drives `run_bank_phase`.
            bank: config.bank,
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
        // the household heir (G4b) or the commons (G4a fallback), free its arena
        // slot, reconcile the society's caches.
        report.deaths = self.update_needs_and_remove_dead();

        // ---- 3b. AGING + OLD-AGE DEATH (G4b): advance each living householder's age
        // and remove any that reach their deterministic lifespan, routing the estate
        // to a household heir (commons if the lineage is extinct). Reuses G4a's
        // removal path; a no-op without a demography overlay. Deterministic — the
        // lifespan is a function of the stable seed, nothing is drawn.
        report.deaths += self.age_and_remove_elderly();

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

        // ---- 4c. PROVISION (G4b): deliver each living householder its household's
        // renewable hunger-staple/WOOD hearth into econ stock, recorded as a source
        // (`report.endowment`). Mirrors `life::Camp`'s harvest delivery — after the
        // scale regeneration (the stock add does not change the scale, so no resting
        // quote goes stale), before the market clears. A no-op without a demography
        // overlay.
        self.deliver_demography_provisions(&mut report);

        // ---- 4d. BANK (G8b): colonists deposit M3 specie into the chartered bank
        // (specie → reserves, claims to the depositor) and the bank lends fiduciary
        // credit (demand claims beyond its reserves, reserve-ratio-gated) to
        // borrowers. Both route through econ's existing M3 ledger / bank balance-sheet
        // paths; the freshly-issued claims are spendable in this tick's market under
        // the default par-all tender, so they circulate as money immediately. A no-op
        // without a bank charter, so every pre-G8b run is byte-identical. The amount
        // returned is sim-direct fiduciary issuance, copied into the M3 record after
        // econ writes that tick's row so exported M3 metrics surface the credit.
        let bank_credit_issued = self.run_bank_phase();

        // ---- 5. MARKET: the econ clearing; money is redistributed between
        // colonists here. Producers have bought their inputs (a miller a unit of
        // grain, a baker a unit of flour) and sold last tick's output. For a G5a
        // barter camp this runs econ's `step_v2`: pre-promotion it clears barter
        // (goods-for-goods relocations, conserved) and feeds the SaleabilityTracker
        // from the realized spatial barter; on the tick the reused Mengerian
        // `winner` rule promotes, the winning good's econ stock is converted to
        // gold 1-for-1 (the lab's conserved promotion); thereafter it is the G2b
        // money-priced market. The promotion is a good→money conversion, so the
        // gold society mints equals the physical units it removed — recorded in
        // `report.promoted` so the whole-system ledger balances across the phase
        // transition (and the gold checkpoints account the minted gold).
        let money_good_before = self.society.current_money_good();
        let society_gold_before = self.society.total_gold();
        if self.barter.is_some() {
            self.society
                .step_rejecting_v2_money_goods(&self.money_rejection_goods);
        } else {
            self.society.step();
        }
        if bank_credit_issued > Gold::ZERO {
            // `checked_add` onto whatever econ's own loan market booked this tick. In G8b
            // the curated bank configs do not activate the debt cycle; when G8c
            // broadens finance both paths could issue in the same tick, and adding (not
            // overwriting) keeps the column a single credit total then too.
            let record = self
                .society
                .m3_records
                .last_mut()
                .expect("a banked M3 settlement writes an M3 record each econ tick");
            record.bank_credit_issued = record
                .bank_credit_issued
                .checked_add(bank_credit_issued)
                .expect("bank credit issued cannot overflow the M3 record");
        }
        report.total_gold_after_step = self.total_gold().0;
        if money_good_before.is_none() {
            if let Some(emerged) = self.society.current_money_good() {
                let minted = self
                    .society
                    .total_gold()
                    .0
                    .saturating_sub(society_gold_before.0);
                report.promoted.insert(emerged, minted);
            }
        }

        // ---- 6. PRODUCTION (G3a): each living producer applies its recipe to the
        // input it now holds, transforming it into output. A conserved conversion:
        // the input consumed and the output produced are both recorded so the
        // whole-system ledger accounts every transformed unit. Runs after the
        // market (so the input a producer just bought is on hand) and is a no-op
        // for a plain settlement (no chain).
        self.run_production(&mut report);

        // ---- 6b. BIRTHS (G4b): each food-secure household under its size cap and
        // past its birth interval bears one child — a new colonist with an inherited,
        // mutated culture and a conserved endowment transferred from a parent, added
        // via `Society::add_agent` so it participates from the NEXT econ tick. Runs
        // after the market so the newborn does not trade the tick it is born, and
        // before the after-snapshot so its (transferred-in) holdings balance the
        // parent's debit. A no-op without a demography overlay; draws no randomness.
        report.births = self.run_births();

        // ---- 7. READ-BACK happens at the top of the next tick's NEEDS phase.

        // Conservation receipt: consumed (the eating sink) is this tick's
        // consumption log; the whole-system after-totals (taken AFTER production and
        // births) must balance against before + regen + endowment + produced −
        // consumed_as_input − consumed − promoted for every good (births/deaths move
        // goods within the whole system, so they need no term).
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

    // ---- the G8b bank phase ---------------------------------------------

    /// The G8b bank phase: **deposits** then **fiduciary lending**, both routed
    /// through econ's existing M3 ledger / bank balance-sheet paths — no bank logic
    /// is added to econ. A no-op without a chartered bank, so every pre-G8b run is
    /// byte-identical.
    ///
    /// **Deposit.** Each living consumer moves `min(deposit_per_tick, its specie)`
    /// of M3 specie into the bank. [`MoneySystem::issue_demand_claim`] with
    /// `backed_by_reserves == amount` debits the depositor's specie, credits the
    /// ledger's bank reserves, and gives the depositor an equal demand claim;
    /// [`Bank::credit_reserves`] and `demand_deposits` mirror the move on the bank's
    /// balance sheet (so `sum(bank.reserves) == ledger bank_reserves` stays true).
    /// The depositor's spendable total is unchanged — specie became a claim — so the
    /// claim circulates as money in the specie's place.
    ///
    /// **Lend fiduciary.** The bank lends up to econ's
    /// [`Bank::fiduciary_lend_capacity`] for the regime, capped by a sim-side
    /// depositor-death redemption buffer, and split across the living gatherers
    /// (deterministically; the remainder lands on the lowest-id borrowers).
    /// `issue_demand_claim` with `backed_by_reserves == 0` issues claims **beyond**
    /// reserves — the ledger tracks them as `fiduciary = demand_claims −
    /// bank_reserves` — and [`Bank::record_fiduciary_loan`] books the loan. A
    /// 100%-reserve bank's capacity is zero, so the control lends nothing while its
    /// deposits still circulate. The buffer is game-only: it preserves enough excess
    /// reserves that a future depositor death can redeem the protected claims without
    /// taking the bank below its configured reserve ratio.
    ///
    /// Returns the fiduciary credit issued in this sim-side phase so the current M3
    /// record can expose it through econ's existing `bank_credit_issued` column.
    ///
    /// Deterministic: integer amounts, slot-ordered rosters, nothing drawn.
    fn run_bank_phase(&mut self) -> Gold {
        let Some(bank_cfg) = self.bank else {
            return Gold::ZERO;
        };
        let regime = self.society.regime();
        let Some(bank_pos) = self
            .society
            .banks
            .iter()
            .position(|bank| bank.id == BANK_ID)
        else {
            return Gold::ZERO;
        };

        // Disjoint borrows: the live roster (read) and the society's ledger + bank
        // balance sheet (mutated). Borrowing the roster in place lets the deposit/lend
        // loops walk it in slot order — depositors are the living consumers, borrowers
        // the living gatherers — without collecting either into a fresh `Vec` each tick.
        let live_slots = &self.live_colonist_slots;
        let colonists = &self.colonists;
        let society = &mut self.society;
        let tick = society.tick;
        let Some(money_system) = society.money_system.as_mut() else {
            return Gold::ZERO;
        };
        let bank = &mut society.banks[bank_pos];
        let mut bank_credit_receipts = Vec::new();

        // ---- Deposit: each living consumer moves specie -> reserves + a demand claim.
        for &slot in live_slots {
            let colonist = &colonists[slot];
            if colonist.vocation != Vocation::Consumer {
                continue;
            }
            let depositor = colonist.id;
            let specie = money_system
                .balance_snapshot(depositor)
                .map(|balance| balance.public_specie)
                .unwrap_or(Gold::ZERO);
            let amount = bank_cfg.deposit_per_tick.min(specie);
            if amount == Gold::ZERO {
                continue;
            }
            money_system
                .issue_demand_claim(BANK_ID, depositor, amount, amount)
                .expect("a deposit bounded by the depositor's specie must succeed");
            bank.credit_reserves(amount)
                .expect("crediting bank reserves cannot overflow for a bounded deposit");
            bank.demand_deposits = bank
                .demand_deposits
                .checked_add(amount)
                .expect("bank demand deposits cannot overflow for a bounded deposit");
        }

        let protected_depositor_claims = live_slots
            .iter()
            .filter(|&&slot| colonists[slot].vocation == Vocation::Consumer)
            .map(|&slot| money_system.demand_claim_on(colonists[slot].id, BANK_ID))
            .try_fold(Gold::ZERO, Gold::checked_add)
            .expect("bounded G8b depositor claims cannot overflow");

        // ---- Lend fiduciary: the reserve-gated capacity that still leaves room for a
        // future depositor-death redemption, split evenly across the living gatherers in
        // slot order (the remainder lands on the lowest-slot borrowers). Zero for a
        // 100%-reserve bank (the control).
        let capacity = Self::fiduciary_lend_capacity_preserving_redemption(
            bank,
            regime,
            Gold::ZERO,
            protected_depositor_claims,
        );
        let borrower_count = live_slots
            .iter()
            .filter(|&&slot| colonists[slot].vocation == Vocation::Gatherer)
            .count() as u64;
        let mut issued_this_tick = Gold::ZERO;
        if capacity > Gold::ZERO && borrower_count > 0 {
            let base = capacity.0 / borrower_count;
            let extra = capacity.0 % borrower_count;
            let mut borrower_index: u64 = 0;
            for &slot in live_slots {
                let colonist = &colonists[slot];
                if colonist.vocation != Vocation::Gatherer {
                    continue;
                }
                let share = base + u64::from(borrower_index < extra);
                borrower_index += 1;
                if share == 0 {
                    continue;
                }
                let amount = Gold(share);
                // Defensive backstop, never fires for the even split above: the shares
                // sum to exactly the pre-computed `capacity` (base*borrowers + extra),
                // so each `amount` is within the remaining `capacity - issued_this_tick`.
                // The check re-derives the live capacity from the *mutated* balance sheet
                // (booking a fiduciary loan grows `demand_deposits`, shrinking the
                // convertible deposit capacity unit-for-unit) so the bank's reserve-gated
                // per-tick cap can never be breached even if the split logic later changes.
                if Self::fiduciary_lend_capacity_preserving_redemption(
                    bank,
                    regime,
                    issued_this_tick,
                    protected_depositor_claims,
                ) < amount
                {
                    break;
                }
                money_system
                    .issue_demand_claim(BANK_ID, colonist.id, amount, Gold::ZERO)
                    .expect("a fiduciary issue within capacity must succeed");
                bank.record_fiduciary_loan(regime, amount)
                    .expect("recording a fiduciary loan within capacity must succeed");
                bank_credit_receipts.push(CantillonReceipt {
                    tick,
                    agent: colonist.id,
                    amount,
                    source: CreditSource::BankFiduciary(BANK_ID),
                });
                issued_this_tick = issued_this_tick
                    .checked_add(amount)
                    .expect("prechecked fiduciary issuance cannot overflow");
            }
        }

        // Reconcile the agents' spendable-money caches to the mutated ledger so the
        // market this tick reads the new specie/claims and the money invariant holds.
        money_system.reconcile_agent_cache(society.agents.as_mut_slice());
        society.cantillon_receipts.extend(bank_credit_receipts);
        issued_this_tick
    }

    fn fiduciary_lend_capacity_preserving_redemption(
        bank: &Bank,
        regime: Regime,
        issued_this_tick: Gold,
        protected_redemption: Gold,
    ) -> Gold {
        let capacity = bank.fiduciary_lend_capacity_after_tick_issuance(regime, issued_this_tick);
        if capacity == Gold::ZERO || protected_redemption == Gold::ZERO {
            return capacity;
        }
        let ratio = u128::from(bank.reserve_ratio_bps.0);
        if ratio == 0 || !bank.convertible || regime == Regime::SuspendedConvertibility {
            return capacity;
        }
        if protected_redemption > bank.reserves {
            return Gold::ZERO;
        }

        let reserves_after_redemption =
            u128::from(bank.reserves.saturating_sub(protected_redemption).0);
        let deposits_after_redemption =
            u128::from(bank.demand_deposits.saturating_sub(protected_redemption).0);
        let max_deposits_after_redemption =
            reserves_after_redemption.saturating_mul(10_000) / ratio;
        if max_deposits_after_redemption <= deposits_after_redemption {
            return Gold::ZERO;
        }

        let protected_capacity = max_deposits_after_redemption - deposits_after_redemption;
        capacity.min(Gold(u64::try_from(protected_capacity).unwrap_or(u64::MAX)))
    }

    /// Liquidate a dying colonist's bank **deposit** so a banked death settles through
    /// the **unchanged** G8a specie estate. The underlying viable economy is viable
    /// only over a bounded horizon: its consumers eventually starve once their finite
    /// WOOD income is exhausted (true with or without a bank — even the bank-free
    /// `viable`/`m3_settlement` colony loses its consumers at long tick counts), so a
    /// depositing colonist can reach the starvation-death window still holding the
    /// demand claims its deposits created. G8b settles that with **no econ change and
    /// no claim-estate routing** (both G8c): the deposit is *withdrawn* — the dying
    /// colonist's demand claims are redeemed for specie through econ's existing
    /// [`MoneySystem::redeem_demand_claim_for_specie`] path (the bank pays specie out
    /// of its reserves, the mirror image of the deposit), after which the colonist
    /// holds only specie and [`Society::can_remove_agent`] accepts it for the G8a
    /// specie estate — exactly as a bank-free starvation death settles.
    ///
    /// The withdrawal is capped by the bank's reserves; the bank phase leaves enough
    /// reserve-ratio headroom for the shipped charters that a protected depositor claim
    /// can be withdrawn without putting the bank below its configured reserve ratio. It
    /// conserves both the M3 ledger and the bank balance sheet — reserves and demand
    /// deposits each fall by the redeemed amount and the fiduciary is untouched — so the
    /// reconcile gate (and amendment A1's `sum(bank.demand_deposits) == demand_claims`
    /// check) stays green across the death. A no-op without a bank (every pre-G8b death
    /// path is byte-identical) or when the dying colonist holds no claim. Deterministic:
    /// integer amounts, no RNG.
    fn liquidate_bank_deposit_on_death(&mut self, id: AgentId) {
        if self.bank.is_none() {
            return;
        }
        let society = &mut self.society;
        let Some(bank_pos) = society.banks.iter().position(|bank| bank.id == BANK_ID) else {
            return;
        };
        let Some(money_system) = society.money_system.as_mut() else {
            return;
        };
        let bank = &mut society.banks[bank_pos];
        // The bank can only honor a redemption out of its reserves; cap the withdrawal
        // there. For the shipped configs the lending phase preserves enough headroom for
        // the protected depositor claims, so the cap never bites — any residual would be
        // refused by `can_remove_agent` and caught by the death-window assert below.
        let amount = money_system.demand_claim_on(id, BANK_ID).min(bank.reserves);
        if amount == Gold::ZERO {
            return;
        }
        money_system
            .redeem_demand_claim_for_specie(id, BANK_ID, amount)
            .expect("redeeming a dying depositor's claim bounded by bank reserves must succeed");
        bank.debit_reserves(amount)
            .expect("debiting reserves for a reserve-bounded redemption cannot underflow");
        bank.retire_demand_deposit(amount)
            .expect("retiring the redeemed deposit cannot underflow the bank's demand deposits");
        // Reconcile the spendable-money caches so the withdrawn specie lands in the
        // dying colonist's `gold` cache, which the G8a `remove_agent` drains into the
        // estate.
        money_system.reconcile_agent_cache(society.agents.as_mut_slice());
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
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            if colonist.vocation == Vocation::Gatherer {
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
            for &slot in &self.live_colonist_slots {
                let colonist = &self.colonists[slot];
                if colonist.vocation != Vocation::Gatherer {
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
    /// no RNG). Dead gatherers have already had their carry settled and their world
    /// agents removed, so this loop never sees them.
    fn assign_idle_gatherer_tasks(&mut self) {
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            if colonist.vocation != Vocation::Gatherer {
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
    /// freeing its arena slot, and removing it from the world. Returns the number of
    /// deaths. Deterministic: deaths are collected in generation order and settled
    /// in that order; nothing is drawn.
    fn update_needs_and_remove_dead(&mut self) -> u32 {
        let live_slots = self.live_colonist_slots.clone();
        let mut intakes = vec![NeedIntake::default(); live_slots.len()];
        for &(agent, good, qty) in self.society.consumption_log_last_tick() {
            let Some(index) = self.slot_for_id(agent) else {
                continue;
            };
            let Ok(intake_index) = live_slots.binary_search(&index) else {
                continue;
            };
            if good == self.known.hunger {
                intakes[intake_index].food_consumed =
                    intakes[intake_index].food_consumed.saturating_add(qty);
            } else if good == self.known.warmth {
                intakes[intake_index].wood_consumed =
                    intakes[intake_index].wood_consumed.saturating_add(qty);
            }
        }
        for &(agent, labor) in self.society.labor_used_last_tick() {
            let Some(index) = self.slot_for_id(agent) else {
                continue;
            };
            let Ok(intake_index) = live_slots.binary_search(&index) else {
                continue;
            };
            intakes[intake_index].labor_used =
                intakes[intake_index].labor_used.saturating_add(labor);
        }

        for (intake_index, &slot) in live_slots.iter().enumerate() {
            self.colonists[slot]
                .need
                .advance(&self.dynamics, intakes[intake_index]);
        }

        // Collect deaths first (immutable read of `dynamics`), then apply.
        let mut dying = Vec::new();
        for &slot in &live_slots {
            let colonist = &mut self.colonists[slot];
            if colonist.need.is_critical(&self.dynamics) {
                colonist.critical_streak = colonist.critical_streak.saturating_add(1);
            } else {
                colonist.critical_streak = 0;
            }
            if colonist.critical_streak >= self.dynamics.death_window {
                dying.push(colonist.id);
            }
        }
        // Settle each dying colonist's bank deposit before removal: redeem its demand
        // claims for specie (the deposit's mirror image) so it holds only specie and
        // settles through the unchanged G8a specie estate. A no-op without a bank, so
        // every pre-G8b death path is byte-identical. The underlying economy is viable
        // only over a bounded horizon — its consumers eventually starve once their
        // finite WOOD income runs out (with or without a bank) — so a depositing
        // colonist can reach the death window still holding claims; this withdraws them
        // with no econ change and no claim-estate routing (G8c). See
        // [`Self::liquidate_bank_deposit_on_death`].
        for &id in &dying {
            self.liquidate_bank_deposit_on_death(id);
        }
        // Every colonist that reached the starvation death window must now be settle-able.
        // A balance still holding demand claims or fiat has no conserved estate route yet
        // (claim/fiat estates land with the G8c tax/regime work); the deposit-withdrawal
        // above clears the only claim a shipped config produces, so this stays a fail-loud
        // backstop for any future claim/fiat holder the withdrawal cannot cover (e.g. a
        // claim beyond the bank's reserves), rather than silently dropping it from the
        // dying list and leaving an alive-but-permanently-critical colonist that never
        // settles. It is an assertion pass, not a filter — the `dying` set is unchanged
        // when every member is settle-able (every shipped case).
        for &id in &dying {
            assert!(
                self.society.can_remove_agent(id),
                "colonist {id:?} reached the starvation death window but cannot be \
                 settled (still holds demand claims or fiat the deposit-withdrawal \
                 could not cover, with no estate route until G8c); the dying -> \
                 settle path must stay complete for every shipped config"
            );
        }
        for &id in &dying {
            if let Some(slot) = self.slot_for_id(id) {
                self.mark_colonist_dead(slot);
            }
        }
        let mut deaths = 0;
        for id in dying {
            deaths += u32::from(self.settle_death(id));
        }
        deaths
    }

    /// Route a dead colonist's estate (G4a removal + G4b inheritance). A demography
    /// settlement routes to the household **heirs** (the commons only if the lineage
    /// is extinct); every pre-G4b settlement routes to the commons exactly as G4a.
    /// The dispatch keeps the no-demography path structurally unchanged, so the G4a
    /// suite and the conformance goldens are byte-identical.
    fn settle_death(&mut self, id: AgentId) -> bool {
        if self.demography.is_some() {
            self.settle_estate_to_heirs(id)
        } else {
            self.settle_estate_to_commons(id)
        }
    }

    /// Remove `id` from the running settlement and collect its full estate — econ
    /// gold + stock (via [`Society::remove_agent`]), world-carried delivery escrow,
    /// and any stranded exchange-deposit escrow — returning the gold and a per-good
    /// map, and removing its world agent. The estate is collected but NOT yet routed;
    /// the caller settles it to the commons (G4a) or the household heirs (G4b). The
    /// order is the spec's (settle → cancel → free → reconcile, inside
    /// `remove_agent`; then drain world/exchange escrow), so wherever the estate goes
    /// the whole-system total is conserved. Deterministic: id-ordered, no RNG.
    fn collect_estate(&mut self, id: AgentId) -> Option<(Gold, BTreeMap<GoodId, u64>)> {
        let estate = self.society.remove_agent(id)?;
        let gold = estate.gold;
        let mut stock: BTreeMap<GoodId, u64> = BTreeMap::new();
        // Econ estate: the dead colonist's gold plus every physical good it held
        // (its stock is a subset of `self.goods`; GOLD is money, not stock).
        for &good in &self.goods {
            let qty = estate.stock.get(good);
            if qty > 0 {
                *stock.entry(good).or_insert(0) += u64::from(qty);
            }
        }
        // World-carried escrow: drain it out of the world (rather than freezing it in
        // place as the G1 tombstone did). A non-spatial householder (G4b) carries
        // nothing, so this is a no-op for it.
        for &good in &self.goods {
            let carried = self.world.agent_carry(id, good);
            if carried > 0 {
                let drained = self.world.withdraw_agent_carry(id, good, carried);
                *stock.entry(good).or_insert(0) += u64::from(drained);
            }
        }
        // Pending exchange-deposit escrow: units this colonist delivered to the
        // exchange stockpile but never had credited (its attribution still sitting in
        // `pending_deposits`) are part of its estate. Drain them out of the world's
        // exchange and drop the attribution — a conserved transfer that leaves no
        // entry keyed by the freed id for `transfer_pending_deposits` to retry against
        // forever. The withdraw mirrors the removed attribution unit-for-unit,
        // preserving the pending↔exchange invariant. Empty in the starvation/old-age
        // death models (the transfer credits a still-live depositor before it can
        // die; a householder never deposits), so this is a defensive settle.
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
                *stock.entry(good).or_insert(0) += u64::from(drained);
            }
        }
        // Remove its spatial body after draining carry so future world ticks do not
        // scan historical deaths. Non-spatial G4b householders were never in the
        // world, so this is a no-op for them.
        if let Some(remaining_carry) = self.world.remove_agent(id) {
            // The loop above drains every good in `self.goods`; this sweeps any residual
            // into the estate rather than dropping it in release builds (the assert pins
            // the invariant in debug). Conservation is enforced, never assumed.
            debug_assert!(
                remaining_carry.values().all(|&qty| qty == 0),
                "estate collection must drain carry before removing a world agent"
            );
            for (good, qty) in remaining_carry {
                if qty > 0 {
                    *stock.entry(good).or_insert(0) += u64::from(qty);
                }
            }
        }
        Some((gold, stock))
    }

    /// Settle a dead colonist's estate to the **commons** (G4a). A conserved transfer
    /// end to end: the gold and goods leave the society and the world for the commons,
    /// nothing created or destroyed. Deterministic: id-ordered, no RNG.
    fn settle_estate_to_commons(&mut self, id: AgentId) -> bool {
        if !self.society.can_remove_agent(id) {
            return false;
        }
        if let Some(slot) = self.slot_for_id(id) {
            self.mark_colonist_dead(slot);
        }
        let Some((gold, stock)) = self.collect_estate(id) else {
            return false;
        };
        self.commons_gold = self.commons_gold.saturating_add(gold);
        for (good, qty) in stock {
            if qty > 0 {
                *self.commons_stock.entry(good).or_insert(0) += qty;
            }
        }
        self.record_estate_destination(id, EstateDestination::Commons);
        true
    }

    /// Settle a dead colonist's estate to the household **heirs** (G4b inheritance):
    /// credit the whole estate to a living member of the same household (the first
    /// surviving heir in colonist-insertion order), falling back to the **commons** if the lineage is extinct (no
    /// living member remains). Crediting a live heir is a conserved transfer *within*
    /// the society (the dead's holdings move to a survivor), and the commons fallback
    /// is the same conserved transfer G4a used — so whole-system conservation holds
    /// either way. Any unplaceable remainder (an heir at the `u32`/`u64` ceiling — never
    /// reached with these small quantities) routes to the commons rather than vanish.
    fn settle_estate_to_heirs(&mut self, id: AgentId) -> bool {
        if !self.society.can_remove_agent(id) {
            return false;
        }
        if let Some(slot) = self.slot_for_id(id) {
            self.mark_colonist_dead(slot);
        }
        let Some((gold, stock)) = self.collect_estate(id) else {
            return false;
        };
        let destination = self.heir_for(id).map(|heir| EstateDestination::Household {
            household: self.colonist_household(id).unwrap_or_default(),
            heir,
        });
        match destination {
            Some(EstateDestination::Household { heir, .. }) => {
                if !self.credit_estate_gold_to_heir(heir, gold) {
                    // Defensive: an overflow at the heir, stale heir id, or future
                    // ledger-money estate routes the gold to the commons.
                    self.commons_gold = self.commons_gold.saturating_add(gold);
                }
                for (good, qty) in stock {
                    if qty == 0 {
                        continue;
                    }
                    // Clamp the credit to the heir's remaining headroom so the
                    // saturating `Stock::add` can never silently drop goods: any amount
                    // the heir cannot hold (its stock would pass `u32::MAX`) routes to
                    // the commons instead of vanishing — the same clamp the provision
                    // path uses. Unreached with these small quantities, but conservation
                    // is load-bearing, so it is enforced here, never assumed.
                    let held = self
                        .society
                        .agents
                        .get(heir)
                        .map_or(0, |agent| agent.stock.get(good));
                    let headroom = u64::from(u32::MAX - held);
                    let credited = u32::try_from(qty.min(headroom)).unwrap_or(0);
                    let placed = if credited > 0 && self.society.credit_stock(heir, good, credited)
                    {
                        u64::from(credited)
                    } else {
                        0
                    };
                    if qty > placed {
                        *self.commons_stock.entry(good).or_insert(0) += qty - placed;
                    }
                }
            }
            Some(EstateDestination::Commons) | None => {
                self.commons_gold = self.commons_gold.saturating_add(gold);
                for (good, qty) in stock {
                    if qty > 0 {
                        *self.commons_stock.entry(good).or_insert(0) += qty;
                    }
                }
            }
        }
        self.record_estate_destination(id, destination.unwrap_or(EstateDestination::Commons));
        true
    }

    /// Credit already-collected estate gold to a live heir, on either money regime.
    /// [`Society::remove_agent`] has already removed the dead colonist's money from
    /// this same society — its `Agent.gold` in closed-GOLD M1, or its public specie
    /// drained out of the ledger in M3 (G8a) — so restoring it to a live household
    /// heir is a conserved in-settlement estate move. [`Society::credit_estate_gold`]
    /// handles every regime: it adds to `Agent.gold` in closed-GOLD M1 and in
    /// post-promotion emergent money, and re-credits ledger specie (returning
    /// `commodity_base` to its pre-death total) in M3. Returns `false` only on an
    /// overflow or stale heir, in which case the gold routes to the commons instead.
    fn credit_estate_gold_to_heir(&mut self, heir: AgentId, gold: Gold) -> bool {
        self.society.credit_estate_gold(heir, gold)
    }

    fn colonist_household(&self, id: AgentId) -> Option<usize> {
        self.slot_for_id(id)
            .and_then(|slot| self.colonists[slot].household)
    }

    fn record_estate_destination(&mut self, id: AgentId, destination: EstateDestination) {
        if let Some(slot) = self.slot_for_id(id) {
            self.colonists[slot].estate_destination = Some(destination);
        }
    }

    /// The heir for a dead colonist's estate (G4b): the first **living** member of
    /// the dead colonist's household, in colonist-insertion order, that still resolves as a live econ agent, or
    /// `None` if the lineage is extinct (or the colonist has no household — a pre-G4b
    /// colonist, which therefore settles to the commons). The dead colonist is already
    /// marked `alive = false` before settlement, so it is never its own heir.
    fn heir_for(&self, dead_id: AgentId) -> Option<AgentId> {
        let household = self
            .slot_for_id(dead_id)
            .and_then(|s| self.colonists[s].household)?;
        // Scan only the compact live roster: the dead colonist is marked dead — and so
        // already off `live_colonist_slots` — before settlement, so it is never its own
        // heir, and co-dying members (marked before any are settled) are excluded too.
        // `live_colonist_slots` is kept in slot order, so this yields the first
        // surviving household member in colonist-insertion order, the same colonist the
        // historical scan picked, without walking the full historical roster.
        self.live_colonist_slots
            .iter()
            .map(|&slot| &self.colonists[slot])
            .filter(|c| c.household == Some(household))
            .map(|c| c.id)
            .find(|&heir| self.society.agents.get(heir).is_some())
    }

    /// AGING + OLD-AGE DEATH (G4b): advance each living householder's age by one econ
    /// tick and remove any that reach their deterministic `lifespan` via the G4a
    /// removal path, settling the estate to a household heir. Returns the old-age
    /// death count. A no-op without a demography overlay. Deterministic: ages and
    /// deaths are taken in slot order, the lifespan is a pure function of the
    /// colonist's seed, nothing is drawn.
    fn age_and_remove_elderly(&mut self) -> u32 {
        if self.demography.is_none() {
            return 0;
        }
        let mut dying = Vec::new();
        let live_slots = self.live_colonist_slots.clone();
        for &slot in &live_slots {
            let colonist = &mut self.colonists[slot];
            let Some(lifespan) = colonist.lifespan else {
                continue;
            };
            colonist.age = colonist.age.saturating_add(1);
            if colonist.age >= lifespan {
                dying.push(colonist.id);
            }
        }
        let dying: Vec<_> = dying
            .into_iter()
            .filter(|&id| self.society.can_remove_agent(id))
            .collect();
        for &id in &dying {
            if let Some(slot) = self.slot_for_id(id) {
                self.mark_colonist_dead(slot);
            }
        }
        let mut deaths = 0;
        for id in dying {
            deaths += u32::from(self.settle_estate_to_heirs(id));
        }
        self.old_age_deaths_total = self.old_age_deaths_total.saturating_add(u64::from(deaths));
        deaths
    }

    /// PROVISION phase (G4b): deliver each living householder its household's
    /// renewable staple/WOOD hearth into econ stock, recording the total as a conserved
    /// source in `report.endowment`. A no-op without a demography overlay.
    /// Deterministic: slot order, no RNG. The provision keeps members fed (so deaths
    /// are old age, not starvation) and supplies the wood-surplus household its
    /// tradeable surplus. The staple is the settlement's hunger good
    /// ([`KnownGoods::hunger`]) — FOOD on a `lineages` colony, bread on the G5b
    /// frontier — so members are always provisioned the very good they eat.
    fn deliver_demography_provisions(&mut self, report: &mut EconTickReport) {
        let Some(demo) = self.demography.clone() else {
            return;
        };
        let staple = self.known.hunger;
        // Collect (id, household) first so the colonists borrow is released before the
        // society is mutated.
        let members: Vec<(AgentId, usize)> = self
            .live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                let colonist = &self.colonists[slot];
                colonist.household.map(|h| (colonist.id, h))
            })
            .collect();
        for (id, h) in members {
            let spec = &demo.households[h];
            self.deliver_demography_provision_unit(id, staple, spec.food_provision, report);
            self.deliver_demography_provision_unit(id, WOOD, spec.wood_provision, report);
        }
    }

    fn deliver_demography_provision_unit(
        &mut self,
        id: AgentId,
        good: GoodId,
        provision: u32,
        report: &mut EconTickReport,
    ) {
        if provision == 0 {
            return;
        }
        let Some(held) = self
            .society
            .agents
            .get(id)
            .map(|agent| agent.stock.get(good))
        else {
            return;
        };
        let credited = provision.min(u32::MAX - held);
        if credited > 0 && self.society.credit_stock(id, good, credited) {
            *report.endowment.entry(good).or_insert(0) += u64::from(credited);
        }
    }

    /// BIRTHS phase (G4b): each food-secure household, under its size cap and past its
    /// birth interval, bears one child. The newborn inherits its chosen parent's
    /// **mutated** culture (deterministic — a hash of the parent's culture and the
    /// colony's monotonic birth sequence, no `Rng`), is endowed by **conserved
    /// transfers** from that parent (a FOOD buffer it must hold plus a best-effort
    /// gold gift), and joins the society via [`Society::add_agent`] so it
    /// participates from the next econ tick. Returns the birth count. A no-op without
    /// a demography overlay.
    ///
    /// The birth is a **threshold rule**, not an optimizer: a household reproduces
    /// when it clears the need-security margin and can feed a child — the heritable
    /// ordinal patience bias does its selection work through the market
    /// (`regenerate_scale`), not a fitness function. The gold gift is best-effort
    /// (clamped to the parent's unreserved balance), so a gold-poor lineage still reproduces;
    /// poverty shapes a lineage's wealth, never its survival.
    fn run_births(&mut self) -> u32 {
        let Some(demo) = self.demography.clone() else {
            return 0;
        };
        let mut births = 0u32;
        for h in 0..demo.households.len() {
            let next_eligible = self.households[h]
                .last_birth_tick
                .map_or(demo.birth_interval, |t| t + demo.birth_interval);
            if self.econ_tick < next_eligible {
                continue;
            }

            // The household's living members (slots), in slot order.
            let member_slots: Vec<usize> = self
                .live_colonist_slots
                .iter()
                .copied()
                .filter(|&slot| self.colonists[slot].household == Some(h))
                .collect();
            if member_slots.is_empty() || member_slots.len() >= usize::from(demo.max_household_size)
            {
                continue; // extinct (cannot reproduce) or at the size cap (blowup bound)
            }

            // Need-security gate: every living member's hunger at or below the ceiling.
            if !member_slots
                .iter()
                .all(|&slot| self.colonists[slot].need.hunger <= demo.birth_hunger_ceiling)
            {
                continue;
            }

            // Choose the parent: a member that can endow the child's staple buffer,
            // preferring the wealthiest (most gold), ties broken to the lowest slot —
            // a fully deterministic choice. None can endow → skip (poverty of the
            // staple, which the provision makes rare). The staple is the hunger good
            // ([`KnownGoods::hunger`]): FOOD on `lineages`, bread on the frontier.
            let staple = self.known.hunger;
            let parent_slot = member_slots
                .iter()
                .copied()
                .filter(|&slot| {
                    let pid = self.colonists[slot].id;
                    self.society.agents.get(pid).is_some_and(|_| {
                        self.society.free_stock_after_all_reserves(pid, staple)
                            >= demo.child_food_endowment
                    })
                })
                .max_by_key(|&slot| {
                    let pid = self.colonists[slot].id;
                    let gold = self.society.free_gold_after_all_reserves(pid).0;
                    (gold, std::cmp::Reverse(slot))
                });
            let Some(parent_slot) = parent_slot else {
                continue;
            };

            let parent_id = self.colonists[parent_slot].id;
            let parent_culture = self.colonists[parent_slot].culture;
            let parent_seed = self.colonists[parent_slot].seed;

            // The endowment: conserved TRANSFERS from the parent — the staple buffer
            // (required, already verified free after reservations) plus a best-effort
            // gold gift clamped to the parent's unreserved balance.
            if !self
                .society
                .debit_stock(parent_id, staple, demo.child_food_endowment)
            {
                continue; // guarded above; defensive
            }
            let parent_gold = self.society.free_gold_after_all_reserves(parent_id).0;
            let gold_endow = demo.child_gold_endowment.min(parent_gold);

            // The child: inherited+mutated culture, a deterministic lifespan from its
            // own seed, the transferred endowment, and a fresh arena slot via add_agent.
            let birth_seq = self.birth_seq;
            self.birth_seq = self.birth_seq.saturating_add(1);
            let child_culture = parent_culture.inherit(birth_seq, demo.mutation_delta_bps);
            let cseed = child_seed(parent_seed, birth_seq);
            let lifespan = demo.lifespan_ticks(cseed);
            let need = NeedState::rested();
            let child_agent = build_newborn_agent(
                &need,
                &child_culture,
                &self.known,
                0,
                demo.child_food_endowment,
            );
            let child_id = self.society.add_agent(child_agent);
            if gold_endow > 0 {
                let transferred = self
                    .society
                    .transfer_gold(parent_id, child_id, Gold(gold_endow));
                debug_assert!(transferred, "the parent's gold gift must transfer");
            }

            self.colonists.push(Colonist {
                id: child_id,
                vocation: Vocation::Consumer,
                node: None,
                need,
                culture: child_culture,
                critical_streak: 0,
                alive: true,
                latent: None,
                household: Some(h),
                age: 0,
                lifespan: Some(lifespan),
                seed: cseed,
                estate_destination: None,
            });
            let child_slot = self.colonists.len() - 1;
            self.live_colonist_slots.push(child_slot);
            self.colonist_slot_by_id.insert(child_id, child_slot);
            self.households[h].last_birth_tick = Some(self.econ_tick);
            self.births_total = self.births_total.saturating_add(1);
            births += 1;
        }
        births
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
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
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
                    let input_wants = match colonist.vocation {
                        // Active producers (G3a seeded, G3b adopted) bid `throughput`
                        // units of input each tick.
                        Vocation::Miller | Vocation::Baker => chain.throughput.max(1),
                        // G6b: a scholar/confectioner reserves (and tops up) its FULL
                        // input buffer, so research / tier-2 production runs from seeded
                        // stock and the buffer is neither dumped nor eaten.
                        Vocation::Scholar => chain.scholar_grain_buffer.max(1),
                        Vocation::Confectioner => chain.confectioner_flour_buffer.max(1),
                        // A latent producer (Unassigned) posts NO input bid — load-bearing
                        // for the G3b control (it must not price the intermediate good).
                        _ => 0,
                    };
                    producer_scale_extension(&mut scale, tool, input, input_wants);
                }
            }
            // G5a: while still in barter (no money good has emerged), extend the
            // need scale with a near "hold the medium" want so every colonist
            // barters surplus FOOD/WOOD for the durable medium. Its universal
            // demand, traded against both FOOD and WOOD, is what makes the medium
            // the most-saleable good — the saleability differential the camp
            // monetizes on. Dropped once a money good has emerged: the
            // post-promotion scale is pure need-driven and the money market clears
            // in GOLD exactly like G2b. Pure/deterministic; draws no randomness.
            if let Some((medium, qty)) = self.barter_medium {
                if self.society.current_money_good().is_none() {
                    medium_scale_extension(&mut scale, medium, qty);
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
        // G6b content recipes (`None` for a plain G3a/G3b/G5b chain).
        let research_recipe = chain.content.research_recipe().map(|recipe| recipe.id);
        let confect_recipe = chain.content.tier2_recipe().map(|recipe| recipe.id);
        // `chain`/`colonists` (immutable) and `society` (mutable) are disjoint
        // fields, so id-ordered iteration here borrows them side by side. The
        // recipe ids are content data; mutation delegates to econ's existing
        // direct-recipe executor through an additive `Society` accessor.
        for &slot in &self.live_colonist_slots {
            let id = self.colonists[slot].id;
            let (recipe_id, is_research) = match self.colonists[slot].vocation {
                Vocation::Miller => (mill_recipe, false),
                Vocation::Baker => (bake_recipe, false),
                // G6b: a scholar runs research → Knowledge (drained to the counter); a
                // confectioner runs the tier-2 recipe → pastry. Skip if the content
                // carries no such recipe (a non-research chain).
                Vocation::Scholar => match research_recipe {
                    Some(recipe) => (recipe, true),
                    None => continue,
                },
                Vocation::Confectioner => match confect_recipe {
                    Some(recipe) => (recipe, false),
                    None => continue,
                },
                // A latent (Unassigned) colonist holds a tool but has not adopted
                // production, so it mills/bakes nothing until the spread makes it a
                // Miller/Baker (the role-choice phase sets that before production).
                Vocation::Gatherer | Vocation::Consumer | Vocation::Unassigned => continue,
            };
            for _ in 0..throughput {
                // The tier gate: `execute_direct_recipe_for_agent_checked` returns
                // `None` for a DISABLED recipe (the executor honors `Recipe.enabled`),
                // so a confectioner produces nothing while tier 2 is locked even while
                // holding its flour input — the G6b tier-gate test.
                let Some(applied) = self
                    .society
                    .execute_direct_recipe_for_agent_checked(id, recipe_id)
                else {
                    // Out of input, missing tool, or a gated recipe: nothing more.
                    break;
                };
                let (out_good, out_qty) = applied.output;
                if is_research {
                    // G6b: Knowledge is an ACCUMULATOR, not a tradeable good. Drain the
                    // produced units straight back out of the scholar's econ stock (so
                    // they never enter circulation, the digest, or the goods-conservation
                    // ledger) and add them to the per-settlement counter — reported on
                    // its own non-conserved line.
                    let drained = self.society.debit_stock(id, out_good, out_qty);
                    debug_assert!(drained, "the scholar holds the Knowledge it just produced");
                    let amount = u64::from(out_qty);
                    report.knowledge_produced = report.knowledge_produced.saturating_add(amount);
                    self.knowledge = self.knowledge.saturating_add(amount);
                } else {
                    *report.produced.entry(out_good).or_insert(0) += u64::from(out_qty);
                }
                // Conserved good INPUTS to any recipe — research included — are accounted
                // exactly like consumption (the conservation ledger sees every consumed
                // unit). Tools are durable and never appear here.
                if let Some((in_good, in_qty)) = applied.input {
                    *report.consumed_as_input.entry(in_good).or_insert(0) += u64::from(in_qty);
                }
            }
        }

        // G6b: having added this tick's Knowledge, check the tier-2 unlock. After the
        // research phase so the just-produced Knowledge counts toward the threshold.
        self.maybe_unlock_tier_two();
    }

    /// G6b TIER-2 UNLOCK: if accumulated Knowledge has crossed the threshold, enable
    /// the tier-2 (gated) recipe for this settlement and stamp the unlock tick. The
    /// unlock is **per-settlement, deterministic, and one-way** — once unlocked it is
    /// never re-checked, so the tier cannot flap. A no-op for a non-research chain (a
    /// zero threshold / no tier-2 recipe) and once already unlocked.
    fn maybe_unlock_tier_two(&mut self) {
        if self.tier2_unlocked_at.is_some() {
            return;
        }
        let Some(chain) = &self.chain else {
            return;
        };
        let Some(recipe_id) = chain.tier2_recipe_id else {
            return;
        };
        let threshold = chain.tier2_threshold;
        // A zero threshold means "no tech tiers" — the tier never unlocks from time or
        // anything else; only accumulated research crosses a positive threshold.
        if threshold == 0 || self.knowledge < threshold {
            return;
        }
        self.tier2_unlocked_at = Some(self.econ_tick);
        // Flip the gate on the society's LIVE recipe set (what the executor runs) and
        // keep the content's own copy consistent (what the digest and viewer read).
        self.society.set_recipe_enabled(recipe_id, true);
        if let Some(chain) = self.chain.as_mut() {
            chain.content.set_recipe_enabled(recipe_id, true);
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
    /// **G5b gating — role-choice follows money.** The appraisal weighs a recipe's
    /// realized *money* spread, which exists only once a money good is priced. On a
    /// designated-money settlement (G3a/G3b) that holds from tick 0 (the money good is
    /// GOLD), so this is unchanged. On a G5b barter-start frontier there is no money
    /// good — and so no money spread — until promotion, so role-choice is **gated on
    /// the post-promotion money phase**: pre-promotion (barter) no producer role is
    /// ever adopted, and a division of labor emerges only AFTER a medium of exchange
    /// does (the load-bearing economic ordering; the spread is also `None` during
    /// barter, but the gate makes the ordering explicit rather than incidental).
    ///
    /// The decision is **ordinal**: it routes entirely through
    /// [`recipe_adoption_pays_for_money`] (econ's M2.5
    /// [`appraise_project_bundle_for_money`]), which asks whether running the recipe —
    /// selling its output at the realized output price for a future receivable, costing
    /// the realized input price plus the operating cost — newly provisions a
    /// future-**money** want on the colonist's *own* scale without breaking a higher
    /// want. The money good is the settlement's *current* one (GOLD when designated,
    /// the emerged medium post-promotion), so the appraisal and the market agree on
    /// what "money" is. There is no scalar profit number and no argmax across
    /// colonists: each decides for itself, in id order (the §pillar-1 "colonists act"
    /// rule applied to occupation). Re-running it every tick is what makes a role
    /// sticky while the spread holds and revert when it collapses. Deterministic:
    /// integer state, no RNG, id-ordered.
    fn run_role_choice(&mut self) -> bool {
        let Some(chain) = &self.chain else {
            return false;
        };
        // Gate on the money phase: a producer appraises a realized money spread, which
        // exists only once a money good is priced. Designated-money settlements always
        // pass here (current_money_good is GOLD from tick 0, so G3a/G3b are unchanged);
        // a barter-start frontier stays in the no-role barter phase until promotion.
        let Some(money_good) = self.current_money_good() else {
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

        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
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
                recipe_adoption_pays_for_money(
                    agent,
                    recipe,
                    output_price,
                    input_price,
                    tick,
                    operating_cost,
                    money_good,
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
        self.colonist_slot_by_id.get(&id).copied()
    }

    fn mark_colonist_dead(&mut self, slot: usize) {
        if !self.colonists[slot].alive {
            return;
        }
        self.colonists[slot].alive = false;
        if let Ok(index) = self.live_colonist_slots.binary_search(&slot) {
            self.live_colonist_slots.remove(index);
        }
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

    // ---- G6b research / tech-tier surface --------------------------------

    /// Whether this settlement runs the G6b research overlay (its content carries the
    /// research + tier-2 recipes and the Knowledge accumulator).
    pub fn is_research(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(|chain| chain.content.has_research())
    }

    /// The settlement's accumulated **Knowledge** — produced by scholar labor,
    /// monotonic, never traded or consumed (outside the goods-conservation ledger).
    /// `0` for a non-research settlement.
    pub fn knowledge(&self) -> u64 {
        self.knowledge
    }

    /// The Knowledge threshold that unlocks tier 2, or `0` (no tech tiers) for a
    /// non-research settlement.
    pub fn tier2_threshold(&self) -> u64 {
        self.chain.as_ref().map_or(0, |chain| chain.tier2_threshold)
    }

    /// The current tech tier: `2` once the Knowledge unlock has fired, else `1`. A
    /// non-research settlement is always tier `1`.
    pub fn current_tier(&self) -> u8 {
        if self.tier2_unlocked_at.is_some() {
            2
        } else {
            1
        }
    }

    /// The econ tick at which tier 2 unlocked (the gated recipe was enabled), or
    /// `None` if it has not (yet) unlocked. Once `Some`, never cleared — the unlock is
    /// one-way.
    pub fn tier2_unlocked_at(&self) -> Option<u64> {
        self.tier2_unlocked_at
    }

    /// Whether the tier-2 (gated) recipe is currently enabled in the live society —
    /// the gate the production phase honors. `false` before the unlock (and for a
    /// non-research settlement), `true` after.
    pub fn tier2_recipe_enabled(&self) -> bool {
        let Some(chain) = &self.chain else {
            return false;
        };
        chain
            .content
            .tier2_recipe()
            .is_some_and(|recipe| recipe.enabled)
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

    // ---- G5a emergent-money surface --------------------------------------

    /// Whether this settlement runs the G5a barter-start emergence overlay (vs the
    /// designated-GOLD market). `true` even after promotion — it describes the
    /// regime, not the current phase.
    pub fn is_emergent(&self) -> bool {
        self.society.emergence().is_some()
    }

    /// The current money good. For a designated-GOLD settlement this is always
    /// GOLD; for a G5a barter camp it is `None` while the settlement is still in
    /// barter and `Some(good)` once a money good has emerged.
    pub fn current_money_good(&self) -> Option<GoodId> {
        self.society.current_money_good()
    }

    /// Whether a G5a barter camp is still in the **barter phase** (no money good
    /// has emerged yet). Always `false` for a designated-money settlement (its
    /// money is assumed from tick 0).
    pub fn in_barter_phase(&self) -> bool {
        self.is_emergent() && self.current_money_good().is_none()
    }

    /// The econ tick at which a money good was promoted from realized spatial
    /// barter, or `None` if none has (still in barter, or not an emergent camp).
    pub fn promoted_at_tick(&self) -> Option<u64> {
        self.society.money_promoted_at_tick()
    }

    /// The current provisional saleability leader — the good the barter book is
    /// routing indirect offers through as it converges on a money good. `None`
    /// before any good leads, or for a non-emergent settlement.
    pub fn saleability_leader(&self) -> Option<GoodId> {
        self.society.saleability_provisional_leader()
    }

    /// The realized acceptance share (basis points) of `good` in the running
    /// saleability tally, or `None` for a non-emergent settlement. Read-only
    /// surfacing of the lab's tracker for the viewer.
    pub fn saleability_bps(&self, good: GoodId) -> Option<u16> {
        self.society
            .emergence()
            .and_then(|e| e.saleability_bps(good))
    }

    /// Total realized barter trades over the run so far (the emergent camp's
    /// goods-for-goods volume). Zero for a designated-money settlement.
    pub fn barter_trade_count(&self) -> usize {
        self.society.barter_trades.len()
    }

    /// The adopted Mengerian envelope this camp drives, or `None` for a
    /// designated-money settlement. G5a's test 6 asserts the spatial camp routes
    /// through this reused econ config, not a sim-local reimplementation.
    pub fn mengerian_config(&self) -> Option<&MengerianConfig> {
        self.society.emergence().map(|e| e.config())
    }

    /// Total money across the settlement (a closed, conserved balance): live econ
    /// gold plus the settlement **commons** (a dead colonist's settled gold). The
    /// commons term is zero until the first death, so a no-death run's total is
    /// byte-identical to G2b/G3 — and including it keeps gold conserved across a
    /// death, when the dead colonist's gold leaves the society for the commons.
    pub fn total_gold(&self) -> Gold {
        self.society.total_gold().saturating_add(self.commons_gold)
    }

    /// Whether this settlement runs on the M3 ledger-money [`econ::ledger::MoneySystem`]
    /// (G8a) rather than closed-GOLD `Agent.gold` M1. `false` for every pre-G8a config.
    pub fn is_m3(&self) -> bool {
        self.society.money_system.is_some()
    }

    /// The M3 money composition (G8a), or `None` on the closed-GOLD M1 path. The
    /// snapshot's `public_specie` is the circulating money; for a G8a settlement
    /// `public_fiat`, `demand_claims`, `bank_reserves`, `fiduciary`, and `time_deposits`
    /// are all zero — there are no banks and no fiat (those are G8b/G8c). The viewer
    /// surfaces this composition; `g8a_m3_money` test 6 pins the all-specie shape.
    pub fn money_composition(&self) -> Option<econ::ledger::MoneyStock> {
        self.society
            .money_system
            .as_ref()
            .map(|money_system| money_system.snapshot())
    }

    // ---- G8b banks & credit surface --------------------------------------

    /// Whether this settlement runs the G8b **bank charter** overlay (a chartered
    /// bank taking deposits and lending fiduciary credit). `false` for every pre-G8b
    /// config (including the bank-free G8a M3 settlement).
    pub fn is_banked(&self) -> bool {
        self.bank.is_some()
    }

    /// The chartered bank's balance sheet (reserves, demand deposits, fiduciary
    /// issued, reserve ratio, name), or `None` for a bank-free settlement. A
    /// read-only view of the reused econ [`Bank`] the sim charters; the viewer's
    /// balance-sheet banner and the G8b acceptance tests read it. The single bank's
    /// `reserves` equals the M3 ledger's `bank_reserves`, and its `fiduciary_issued`
    /// equals the ledger's `fiduciary` — `g8b_banks` pins both.
    pub fn bank(&self) -> Option<&Bank> {
        self.bank.as_ref()?;
        let mut matches = self.society.banks.iter().filter(|bank| bank.id == BANK_ID);
        let bank = matches.next();
        debug_assert!(
            matches.next().is_none(),
            "a G8b settlement charters at most one bank with the reserved bank id"
        );
        bank
    }

    /// The total demand claims the chartered bank's depositors and borrowers hold
    /// against it (the M3 ledger's circulating bank-claim money), or `Gold::ZERO`
    /// without a bank. Equals [`money_composition`](Self::money_composition)'s
    /// `demand_claims`; surfaced for the viewer and tests as the "claims circulate as
    /// money" measure.
    pub fn demand_claims_outstanding(&self) -> Gold {
        self.money_composition()
            .map(|composition| composition.demand_claims)
            .unwrap_or(Gold::ZERO)
    }

    /// The demand claim `agent` holds against the chartered bank, or `Gold::ZERO`
    /// (no bank, no money system, or no claim). The depositor-holds-a-claim and
    /// borrower-holds-the-fiduciary observables the G8b tests assert on.
    pub fn demand_claim_of(&self, agent: AgentId) -> Gold {
        let Some(money_system) = self.society.money_system.as_ref() else {
            return Gold::ZERO;
        };
        money_system.demand_claim_on(agent, BANK_ID)
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
        self.live_colonist_slots
            .iter()
            .filter(|&&slot| self.colonists[slot].vocation == vocation)
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
        self.live_colonist_slots.len()
    }

    // ---- G4b demography surface ----------------------------------------

    /// Whether this settlement runs the G4b demography overlay.
    pub fn is_demographic(&self) -> bool {
        self.demography.is_some()
    }

    /// The number of seeded households (lineages); `0` without a demography overlay.
    pub fn household_count(&self) -> usize {
        self.households.len()
    }

    /// Lifetime births so far (G4b).
    pub fn births_total(&self) -> u64 {
        self.births_total
    }

    /// Lifetime old-age deaths so far (G4b) — distinct from starvation deaths.
    pub fn old_age_deaths_total(&self) -> u64 {
        self.old_age_deaths_total
    }

    /// The household (lineage) the colonist at generation `index` belongs to, or
    /// `None` for a non-demography colonist.
    pub fn household_of(&self, index: usize) -> Option<usize> {
        self.colonists.get(index).and_then(|c| c.household)
    }

    /// The age (econ ticks) of the colonist at generation `index`, or `None`.
    pub fn age_of(&self, index: usize) -> Option<u64> {
        self.colonists.get(index).map(|c| c.age)
    }

    /// The deterministic old-age lifespan (econ ticks) of the colonist at generation
    /// `index`, or `None` (no demography / no old-age mortality).
    pub fn lifespan_of(&self, index: usize) -> Option<u64> {
        self.colonists.get(index).and_then(|c| c.lifespan)
    }

    /// The culture (the heritable [`CultureParams`]) of the colonist at generation
    /// `index`, or `None`.
    pub fn culture_of(&self, index: usize) -> Option<CultureParams> {
        self.colonists.get(index).map(|c| c.culture)
    }

    /// The destination a dead colonist's estate settled to, or `None` while alive.
    pub fn estate_destination_of(&self, index: usize) -> Option<EstateDestination> {
        self.colonists.get(index).and_then(|c| c.estate_destination)
    }

    /// Living count and accumulated gold for every household, computed in one pass
    /// over the live roster.
    pub fn lineage_stats(&self) -> Vec<LineageStats> {
        let mut stats = vec![LineageStats::default(); self.households.len()];
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            let Some(household) = colonist.household else {
                continue;
            };
            let Some(lineage) = stats.get_mut(household) else {
                continue;
            };
            lineage.living += 1;
            if let Some(agent) = self.society.agents.get(colonist.id) {
                lineage.gold = lineage.gold.saturating_add(agent.gold.0);
            }
        }
        stats
    }

    /// Living members of household (lineage) `household`.
    pub fn lineage_living_count(&self, household: usize) -> usize {
        self.lineage_stats()
            .get(household)
            .map_or(0, |stats| stats.living)
    }

    /// The lineage's **accumulated gold** — the sum of its living members' econ gold
    /// balances (G4b). Estates route to heirs, so a lineage's gold stays within it
    /// across deaths; this is the wealth the patient/present-biased comparison reads.
    pub fn lineage_gold(&self, household: usize) -> u64 {
        self.lineage_stats()
            .get(household)
            .map_or(0, |stats| stats.gold)
    }

    /// The lineage's total holdings of `good` across its living members (G4b) — used
    /// for the per-lineage wealth surfacing.
    pub fn lineage_stock(&self, household: usize, good: GoodId) -> u64 {
        self.live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                let colonist = &self.colonists[slot];
                if colonist.household == Some(household) {
                    self.society.agents.get(colonist.id)
                } else {
                    None
                }
            })
            .map(|a| u64::from(a.stock.get(good)))
            .sum()
    }

    /// The highest hunger any living colonist carries — the boundedness probe for
    /// the smoke test (hunger is the need that kills).
    pub fn max_living_hunger(&self) -> u16 {
        self.live_colonist_slots
            .iter()
            .map(|&slot| self.colonists[slot].need.hunger)
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
            // G6b research/tech-tier dynamic state. Gated on a research chain, so every
            // pre-G6b chain config (no research recipes) is byte-identical. The
            // tier-2 threshold steers when future ticks unlock, and the Knowledge
            // counter plus unlock tick are independent state two
            // otherwise-equal runs can differ in, so all three belong in the
            // "byte-identical iff future behaviour identical" identity — the tick the
            // tier unlocks is part of the determinism contract (G6b test 1). (The
            // tier-2 recipe's `enabled` flip is already captured by the recipe bytes
            // above, since the unlock keeps `content` consistent with the society.)
            if chain.content.has_research() {
                out.extend_from_slice(&chain.tier2_threshold.to_le_bytes());
                out.extend_from_slice(&self.knowledge.to_le_bytes());
                match self.tier2_unlocked_at {
                    Some(tick) => {
                        out.push(1);
                        out.extend_from_slice(&tick.to_le_bytes());
                    }
                    None => out.push(0),
                }
            }
        }

        // The G5a emergent-money config + runtime. The config fields steer future
        // barter ticks even before they show up in holdings or tracker outputs
        // (`medium_want_qty`, endowments, and the Mengerian thresholds/candidates),
        // while the runtime fields capture the phase switch (the promoted good +
        // tick) and the FULL Mengerian emergence state — the saleability tracker's
        // accumulated per-candidate acceptances/acceptor-sets/counterpart-sets and
        // the promotion-timing latch. All of that steers the future promotion
        // decision, so it belongs in the "byte-identical iff future behaviour
        // identical" identity (the provisional leader the old layout captured is a
        // derived projection of it). Omitted entirely for non-emergent settlements,
        // so every G2b/G3/G4 canonical layout stays byte-identical.
        if let Some(barter) = &self.barter {
            push_barter_config_bytes(&mut out, barter);
            out.extend_from_slice(&self.known.savings.0.to_le_bytes());
            push_option_good_bytes(&mut out, self.current_money_good());
            match self.promoted_at_tick() {
                Some(tick) => {
                    out.push(1);
                    out.extend_from_slice(&tick.to_le_bytes());
                }
                None => out.push(0),
            }
            // A barter overlay always runs econ's Emergent money state (the two are
            // wired together in `generate`), so the emergence object is present
            // through every phase — `expect` documents that invariant rather than
            // silently dropping the runtime bytes if it were ever violated.
            let emergence = self
                .society
                .emergence()
                .expect("a barter-overlay settlement runs econ's Emergent money state");
            push_emergence_runtime_bytes(&mut out, emergence);
        }

        // The G8a M3 ledger-money runtime. Omitted entirely for pre-G8a settlements
        // so their canonical layout stays byte-identical; present for M3 so a
        // ledger-backed settlement never collides with the M1 state whose Agent.gold
        // cache happens to match at generation, and so future ledger composition
        // changes are part of the determinism surface.
        if let Some(money_system) = &self.society.money_system {
            out.push(1);
            push_money_system_bytes(&mut out, money_system);
        }

        // The G8b chartered-bank state. Omitted entirely for a bank-free settlement so
        // the pre-G8b canonical layout is byte-identical; present once a bank is
        // chartered so deposits and fiduciary lending — and every config/regime field
        // that steers the *next* bank phase — are part of the determinism surface. The
        // ledger block above already carries the system-level reserves/fiduciary; the
        // fields below are otherwise zero/default at generation, so two banked configs
        // that only diverge on tick one would collide without them.
        if let Some(bank_cfg) = &self.bank {
            // The deposit cadence steers how much specie each future tick moves into
            // reserves (and thus the whole claims/fiduciary trajectory); it lives only
            // in the config, so without it two banked configs differing only in it
            // collide at generation while diverging the next tick.
            out.extend_from_slice(&bank_cfg.deposit_per_tick.0.to_le_bytes());
            // The money regime gates `fiduciary_lend_capacity` (only
            // `FractionalConvertible` / `SuspendedConvertibility` permit fiduciary) and
            // the public spot tender decides whether the issued claims circulate — both
            // steer every future bank phase, so a divergence in either must show in the
            // digest (the G8c regime ladder will move these over time).
            out.push(regime_tag(self.society.regime()));
            out.push(public_spot_tender_tag(self.society.public_spot_tender));
            // Every chartered bank's full balance sheet AND lending policy, in `banks`
            // order (not just `BANK_ID`), so two runs differing in any bank field are
            // distinguishable even if a future settlement charters more than one.
            out.extend_from_slice(&(self.society.banks.len() as u32).to_le_bytes());
            for bank in &self.society.banks {
                push_bank_bytes(&mut out, bank);
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

        // The G4b demography runtime (the birth cadence + lifetime counters). It is
        // omitted entirely without a demography overlay, so a pre-G4b settlement's
        // bytes are unchanged; when present it steers future births, so it is part of
        // the future-behaviour identity. The per-household block is index-ordered
        // (deterministic). The per-colonist demography fields (household, age,
        // lifespan, seed) are appended in the colonist loop below, also gated.
        let is_demographic = self.demography.is_some();
        if let Some(demo) = &self.demography {
            push_demography_config_bytes(&mut out, demo);
            out.extend_from_slice(&self.birth_seq.to_le_bytes());
            out.extend_from_slice(&self.births_total.to_le_bytes());
            out.extend_from_slice(&self.old_age_deaths_total.to_le_bytes());
            out.extend_from_slice(&(self.households.len() as u32).to_le_bytes());
            for household in &self.households {
                match household.last_birth_tick {
                    Some(tick) => {
                        out.push(1);
                        out.extend_from_slice(&tick.to_le_bytes());
                    }
                    None => out.push(0),
                }
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
        let has_estate_destinations = self
            .colonists
            .iter()
            .any(|colonist| colonist.estate_destination.is_some());
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
            if is_demographic {
                // The G4b demography fields steer aging, old-age mortality, the birth
                // roster, and culture inheritance, so they are part of the
                // future-behavior identity. Gated on a demography overlay, so the
                // pre-G4b canonical layout for every other config is unchanged.
                match colonist.household {
                    Some(h) => {
                        out.push(1);
                        out.extend_from_slice(&(h as u32).to_le_bytes());
                    }
                    None => out.push(0),
                }
                out.extend_from_slice(&colonist.age.to_le_bytes());
                match colonist.lifespan {
                    Some(life) => {
                        out.push(1);
                        out.extend_from_slice(&life.to_le_bytes());
                    }
                    None => out.push(0),
                }
                out.extend_from_slice(&colonist.seed.to_le_bytes());
            }
            if has_estate_destinations {
                match colonist.estate_destination {
                    Some(EstateDestination::Commons) => out.push(1),
                    Some(EstateDestination::Household { household, heir }) => {
                        out.push(2);
                        out.extend_from_slice(&(household as u32).to_le_bytes());
                        out.extend_from_slice(&heir.0.to_le_bytes());
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
            // the staple) and — on the frontier — a smaller WOOD buffer (so they buy
            // WOOD with the medium); everyone else carries the surplus buffers. In
            // G3a/G3b both consumer buffers equal the surplus, so those configs are
            // byte-identical.
            let (staple_buffer, wood) = match vocation {
                Vocation::Consumer => (chain.consumer_staple_buffer, chain.consumer_wood_buffer),
                _ => (chain.bread_buffer, chain.wood_buffer),
            };
            stock.add(staple, staple_buffer);
            stock.add(WOOD, wood);
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
                // G6b: a scholar holds a `library` (durable) and a grain buffer it
                // researches into Knowledge. A confectioner holds an `atelier` and a
                // flour buffer it confects into pastry once tier 2 unlocks. Both reserve
                // their input via the scale extension (see `regenerate_scales`), so the
                // buffer is neither dumped nor eaten.
                Vocation::Scholar => {
                    stock.add(
                        chain
                            .content
                            .library()
                            .expect("a scholar requires research-tiers content (a library tool)"),
                        1,
                    );
                    stock.add(chain.content.grain(), chain.scholar_grain_buffer);
                    chain.producer_gold
                }
                Vocation::Confectioner => {
                    stock.add(
                        chain.content.atelier().expect(
                            "a confectioner requires research-tiers content (an atelier tool)",
                        ),
                        1,
                    );
                    stock.add(chain.content.flour(), chain.confectioner_flour_buffer);
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
                Vocation::Miller
                | Vocation::Baker
                | Vocation::Unassigned
                | Vocation::Scholar
                | Vocation::Confectioner => {
                    unreachable!("chain vocations require a production chain config")
                }
            };
            stock.add(FOOD, food);
            stock.add(WOOD, wood);
            gold
        }
    };
    // G5a/G5b: endow the emergent **medium** so it has an initial supply to circulate.
    // Gatherers earn most of it by selling their haul, so they hold a small seed;
    // consumers hold the bulk and spend it down. This is shared by the plain barter
    // camp (G5a, `None` chain) and the G5b frontier (a chain *and* a barter overlay) —
    // a chain colonist demands and barters for the medium exactly like a camp colonist,
    // so the endowment must land on the chain path too (it did not in the G5a-only
    // code, which only reached the `None` branch). Zero in the no-medium control and
    // for producers (they earn the medium by selling surplus, never a seed).
    if let Some(barter) = &config.barter {
        let medium = match vocation {
            Vocation::Gatherer => barter.gatherer_medium_endowment,
            Vocation::Consumer => barter.consumer_medium_endowment,
            _ => 0,
        };
        stock.add(barter.medium_good, medium);
    }
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

/// Build a G4b household member's econ agent (a founder or a newborn): a
/// non-spatial householder endowed from its household's `spec` (gold + a staple/WOOD
/// buffer), with a value scale generated from its need state and (inherited)
/// culture. The staple buffer (`spec.starting_food`) is held in the hunger good
/// ([`KnownGoods::hunger`]) — FOOD on a `lineages` colony, bread on the frontier —
/// so the founder starts with a buffer of the good it eats. Like every other colonist
/// it is a `Household`-role agent with neutral price beliefs; it has no labor capacity
/// and no world agent (it never hauls).
fn build_demography_agent(
    id: AgentId,
    need: &NeedState,
    culture: &CultureParams,
    known: &KnownGoods,
    spec: &crate::demography::HouseholdSpec,
) -> Agent {
    let mut stock = Stock::new(NET.0);
    stock.add(known.hunger, spec.starting_food);
    stock.add(WOOD, spec.starting_wood);
    Agent {
        id,
        scale: regenerate_scale(need, culture, known),
        stock,
        gold: Gold(spec.starting_gold),
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: belief_vec(),
    }
}

/// Build a newborn householder's econ agent (G4b): a non-spatial `Household`-role
/// agent endowed only with the **conserved transfer** its parent gave it (a staple
/// buffer plus, on closed-GOLD M1, any gold gift already represented in `gold`),
/// its value scale generated from a newborn-rested need state and its
/// inherited+mutated culture. The `food` buffer is held in the hunger good
/// ([`KnownGoods::hunger`]) — FOOD on `lineages`, bread on the frontier — the good
/// the newborn eats. Its `id` is overwritten by [`Society::add_agent`].
/// It carries no wood — the household provision supplies that from its first tick.
/// M3 callers install the newborn with zero ledger money and move any gold gift
/// afterward through [`Society::transfer_gold`], so this mints nothing.
fn build_newborn_agent(
    need: &NeedState,
    culture: &CultureParams,
    known: &KnownGoods,
    gold: u64,
    food: u32,
) -> Agent {
    let mut stock = Stock::new(NET.0);
    stock.add(known.hunger, food);
    Agent {
        id: AgentId(0), // overwritten by the arena on insert
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
/// spread newly provisions a future-money (savings) want on the agent's own value
/// scale without breaking a higher-ranked want — a strictly ordinal test, decided
/// on the agent's scale, never by a profit threshold. `true` here means *adopt*.
///
/// This wrapper appraises against **GOLD** as the money good — the designated money a
/// G3a/G3b chain runs on. The G5b frontier's money good is the *emergent* medium
/// (e.g. SALT), so its role-choice phase calls [`recipe_adoption_pays_for_money`]
/// directly with the settlement's current money good (so the appraisal and the market
/// agree on what "money" — and the future savings want — is).
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
    recipe_adoption_pays_for_money(
        agent,
        recipe,
        output_price,
        input_price,
        tick,
        operating_cost,
        GOLD,
    )
}

/// [`recipe_adoption_pays`] generalized over the money good — the role-choice
/// appraisal weighed against `money_good` instead of assuming GOLD. A designated-money
/// chain (G3a/G3b) passes `GOLD` (via the wrapper); the G5b frontier passes its
/// *current* emergent money good (e.g. SALT) so the future-money savings want the
/// appraisal must provision is the same good the post-promotion market clears in. The
/// `output_price`/`input_price` are realized money prices either way (`Gold`-valued),
/// so only the identity of the future want changes, not the spread arithmetic.
pub fn recipe_adoption_pays_for_money(
    agent: &Agent,
    recipe: &Recipe,
    output_price: Option<Gold>,
    input_price: Option<Gold>,
    tick: u64,
    operating_cost: u64,
    money_good: GoodId,
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

    // The future-money want the project must provision sits at the agent's own
    // savings horizon; target the soonest such horizon so the want qualifies
    // (`later >= loan_horizon`). No savings want → nothing to provision → decline.
    let Some(loan_horizon) = soonest_savings_horizon(&agent.scale, money_good) else {
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
    appraise_project_bundle_for_money(&endowment, &candidate, ProjectPlanId(0), money_good)
        .is_some()
}

/// The soonest `Later` horizon at which `scale` holds a savings want for `money_good`
/// — the loan horizon the role-choice appraisal targets so that want qualifies as the
/// future-money want the project bundle must newly provision. `None` if the colonist
/// has no such savings want (a present-biased colonist that never appraises a
/// vocation). `money_good` is GOLD on a designated-money chain and the emergent medium
/// (e.g. SALT) on the G5b frontier, matching the good the colonist's scale actually
/// saves in ([`KnownGoods::savings`]).
///
/// Only `Horizon::Later` wants are considered, and that is the appraisal's own
/// requirement, not an incidental coupling to how scales are generated:
/// `appraise_project_bundle_for_money` can ONLY ever provision a future-money want at
/// `Horizon::Later(later)` with `later >= loan_horizon` (bundle.rs). A `Now`/`Next`
/// money want is immediate liquidity, never the future provisioning a project bundle
/// targets — so even if a scale ever carried one, this appraisal could not satisfy it,
/// and targeting it would only produce a guaranteed decline. Filtering to `Later` is
/// therefore correct by construction.
fn soonest_savings_horizon(scale: &[Want], money_good: GoodId) -> Option<u32> {
    scale
        .iter()
        .filter_map(|want| match (want.kind, want.horizon) {
            (WantKind::Good(good), Horizon::Later(later)) if good == money_good => {
                Some(u32::from(later))
            }
            _ => None,
        })
        .min()
}

/// The `(tool, input_good)` a chain vocation produces with, if any: a Miller (or a
/// latent miller) runs the mill (grain → flour); a Baker (or latent baker) the oven
/// (flour → bread); a G6b Scholar the library (grain → Knowledge); a Confectioner the
/// atelier (flour → pastry). `None` for a gatherer/consumer. This keys
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
        // G6b: a scholar runs research (grain → Knowledge); a confectioner the tier-2
        // recipe (flour → pastry). Both reserve their tool + input like a chain producer.
        Vocation::Scholar => Some(RecipeId::Research),
        Vocation::Confectioner => Some(RecipeId::Confect),
        Vocation::Unassigned => latent,
        Vocation::Gatherer | Vocation::Consumer => None,
    }?;
    match recipe {
        RecipeId::Mill => Some((content.mill(), content.grain())),
        RecipeId::Bake => Some((content.oven(), content.flour())),
        RecipeId::Research => Some((content.library()?, content.grain())),
        RecipeId::Confect => Some((content.atelier()?, content.flour())),
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
/// The scale slot where a `Horizon::Next` "input" want block belongs: just after
/// the last present (`Horizon::Now`) good want and before the first future
/// (`Horizon::Later`) savings want. Both the chain's producer-input wants
/// ([`producer_scale_extension`]) and the G5a medium wants
/// ([`medium_scale_extension`]) sit here — survival goods first, then the input
/// block, then the pure-savings ladder. Savings can legitimately interleave above
/// low-urgency present wants for a patient colonist, so the first `Later` slot
/// alone would put the block ahead of those survival goods; anchoring after the
/// last `Now` good keeps it below them.
fn scale_input_insert_position(scale: &[Want]) -> usize {
    scale
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
        .unwrap_or(scale.len())
}

fn producer_scale_extension(
    scale: &mut Vec<Want>,
    tool: GoodId,
    input_good: GoodId,
    input_wants: u32,
) {
    // Input wants sit after every present good want (bread/wood in the chain); the
    // tool anchor, added below, sits separately at the very top.
    let insert_at = scale_input_insert_position(scale);
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

/// G5a: extend a colonist's need scale with `qty` `Horizon::Next` "hold the
/// medium" wants for `medium`, the demand that drives barter for the emergent
/// medium. The wants are inserted **just below** the present consumption block —
/// the same slot the chain places its producer **input** wants (the durable tool
/// anchor a chain adds separately at the very top has no analogue here): a
/// colonist provisions its Now hunger/warmth first (survival), then barters its
/// surplus for the medium. That sustained, universal demand — traded against both
/// the FOOD a FOOD-gatherer sells and the WOOD a WOOD-gatherer sells — is what
/// makes the medium the good accepted against the most counterparts, the most
/// saleable. Pure and deterministic; no RNG.
fn medium_scale_extension(scale: &mut Vec<Want>, medium: GoodId, qty: u32) {
    if qty == 0 {
        return;
    }
    // Insert after the last present (Now) good want, before the future (Later)
    // savings ladder — survival first, then the medium, then pure savings.
    let insert_at = scale_input_insert_position(scale);
    let medium_wants = (0..qty).map(|_| Want {
        kind: WantKind::Good(medium),
        horizon: Horizon::Next,
        qty: 1,
        satisfied: false,
    });
    let tail = scale.split_off(insert_at);
    scale.extend(medium_wants);
    scale.extend(tail);
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

fn push_barter_config_bytes(out: &mut Vec<u8>, barter: &BarterConfig) {
    push_mengerian_config_bytes(out, &barter.menger);
    out.extend_from_slice(&barter.medium_good.0.to_le_bytes());
    out.extend_from_slice(&barter.medium_want_qty.to_le_bytes());
    out.extend_from_slice(&barter.gatherer_medium_endowment.to_le_bytes());
    out.extend_from_slice(&barter.consumer_medium_endowment.to_le_bytes());
}

fn push_money_system_bytes(out: &mut Vec<u8>, money_system: &econ::ledger::MoneySystem) {
    out.extend_from_slice(&money_system.base.commodity_base.0.to_le_bytes());
    out.extend_from_slice(&money_system.base.fiat_base.0.to_le_bytes());
    out.extend_from_slice(&money_system.base.issuer_gold_vault.0.to_le_bytes());
    out.extend_from_slice(&money_system.base.issuer_fiat_unissued.0.to_le_bytes());
    out.extend_from_slice(&money_system.base.bank_reserves.0.to_le_bytes());
    out.extend_from_slice(&money_system.base.bank_fiat_reserves.0.to_le_bytes());
    out.extend_from_slice(&money_system.claims.demand_claims.0.to_le_bytes());
    out.extend_from_slice(&money_system.claims.fiduciary.0.to_le_bytes());
    out.extend_from_slice(&money_system.claims.time_deposits.0.to_le_bytes());
    out.extend_from_slice(&(money_system.balances.len() as u32).to_le_bytes());
    for balance in &money_system.balances {
        out.extend_from_slice(&balance.agent.0.to_le_bytes());
        out.extend_from_slice(&balance.public_specie.0.to_le_bytes());
        out.extend_from_slice(&balance.public_fiat.0.to_le_bytes());
        out.extend_from_slice(&(balance.demand_claims.len() as u32).to_le_bytes());
        for (bank, claim) in &balance.demand_claims {
            out.extend_from_slice(&bank.0.to_le_bytes());
            out.extend_from_slice(&claim.0.to_le_bytes());
        }
    }
}

/// Serialize the G8b chartered-bank balance sheet into the canonical digest. The
/// ledger block already carries the bank's reserves/fiduciary at the system level;
/// this adds the bank-owned fields (demand_deposits, loans_outstanding, the reserve
/// ratio, convertibility) so two runs that differ only in the bank's balance sheet
/// are distinguishable, plus the lending **policy** — which steers each tick's
/// `fiduciary_lend_capacity` (the per-tick cap, the one-unit loan template, the
/// enabled flag) yet is zero/default-free at generation, so two configs differing
/// only in it would otherwise collide before the first loan.
fn push_bank_bytes(out: &mut Vec<u8>, bank: &Bank) {
    out.extend_from_slice(&bank.id.0.to_le_bytes());
    out.extend_from_slice(&bank.reserves.0.to_le_bytes());
    out.extend_from_slice(&bank.demand_deposits.0.to_le_bytes());
    out.extend_from_slice(&bank.time_deposits.0.to_le_bytes());
    out.extend_from_slice(&bank.loans_outstanding.0.to_le_bytes());
    out.extend_from_slice(&bank.fiduciary_issued.0.to_le_bytes());
    out.extend_from_slice(&bank.reserve_ratio_bps.0.to_le_bytes());
    out.push(u8::from(bank.convertible));
    out.extend_from_slice(&bank.policy.max_new_fiduciary_per_tick.0.to_le_bytes());
    out.extend_from_slice(&bank.policy.loan_present.0.to_le_bytes());
    out.push(bank.policy.loan_horizon);
    out.extend_from_slice(&bank.policy.loan_future_due.0.to_le_bytes());
    out.push(u8::from(bank.policy.enabled));
}

/// A stable 1-byte tag for the society's money [`Regime`], for the canonical digest.
/// The regime gates fiduciary lending, so a banked run encodes it (see
/// [`Settlement::canonical_bytes`]); the explicit match keeps the encoding pinned even
/// if the enum gains variants.
fn regime_tag(regime: Regime) -> u8 {
    match regime {
        Regime::SoundGold => 0,
        Regime::FractionalConvertible => 1,
        Regime::SuspendedConvertibility => 2,
        Regime::Fiat => 3,
    }
}

/// A stable 1-byte tag for the [`PublicSpotTender`] policy, for the canonical digest.
/// The tender decides whether bank claims circulate in the spot market, so a banked run
/// encodes it; the explicit match pins the encoding against future enum additions.
fn public_spot_tender_tag(tender: PublicSpotTender) -> u8 {
    match tender {
        PublicSpotTender::ParAll => 0,
        PublicSpotTender::SpecieOnly => 1,
        PublicSpotTender::FiatAndSpecie => 2,
        PublicSpotTender::BankClaimsAndSpecie => 3,
    }
}

/// Serialize an `Option<GoodId>` into the canonical digest: a present/absent tag
/// byte followed by the good id when present. Keeps the optional-good encoding
/// uniform across the emergent-money blocks.
fn push_option_good_bytes(out: &mut Vec<u8>, good: Option<GoodId>) {
    match good {
        Some(good) => {
            out.push(1);
            out.extend_from_slice(&good.0.to_le_bytes());
        }
        None => out.push(0),
    }
}

/// Serialize the FULL Mengerian emergence runtime into the canonical digest: the
/// promotion-timing latch (the stable winner and how many consecutive ticks it
/// has led) and the saleability tracker's accumulated per-candidate state (the
/// running acceptance count plus the DISTINCT acceptor agents and counterpart
/// goods each candidate has been traded against). All of it steers the future
/// promotion decision — two barter states agreeing on holdings and the current
/// leader but differing in a stability counter or an acceptor set promote on
/// different future ticks — so it is part of the "byte-identical iff future
/// behaviour identical" identity. The member lists (not just their counts) are
/// serialized because a later acceptance only advances the eligibility counts
/// when its acceptor/counterpart is new. The tracker freezes once a good has
/// promoted (it stops observing), but is still serialized so the post-promotion
/// bytes stay a faithful function of the run. Candidate order is the tracker's
/// stored sorted order, so the bytes are deterministic.
fn push_emergence_runtime_bytes(out: &mut Vec<u8>, emergence: &MengerianEmergence) {
    push_option_good_bytes(out, emergence.stable_winner());
    out.extend_from_slice(&emergence.stable_winner_ticks().to_le_bytes());
    let tracker = emergence.tracker();
    out.extend_from_slice(&tracker.total_acceptances().to_le_bytes());
    let candidates = tracker.candidate_saleability();
    out.extend_from_slice(&(candidates.len() as u32).to_le_bytes());
    for candidate in candidates {
        out.extend_from_slice(&candidate.good.0.to_le_bytes());
        out.extend_from_slice(&candidate.acceptances.to_le_bytes());
        out.extend_from_slice(&(candidate.acceptor_agents.len() as u32).to_le_bytes());
        for agent in candidate.acceptor_agents {
            out.extend_from_slice(&agent.0.to_le_bytes());
        }
        out.extend_from_slice(&(candidate.counterpart_goods.len() as u32).to_le_bytes());
        for good in candidate.counterpart_goods {
            out.extend_from_slice(&good.0.to_le_bytes());
        }
    }
}

fn push_mengerian_config_bytes(out: &mut Vec<u8>, menger: &MengerianConfig) {
    out.extend_from_slice(&(menger.candidate_goods.len() as u32).to_le_bytes());
    for good in &menger.candidate_goods {
        out.extend_from_slice(&good.0.to_le_bytes());
    }
    out.extend_from_slice(&menger.min_total_acceptances.to_le_bytes());
    out.extend_from_slice(&menger.promotion_threshold_bps.to_le_bytes());
    out.extend_from_slice(&menger.lead_margin_bps.to_le_bytes());
    out.extend_from_slice(&menger.min_acceptor_agents.to_le_bytes());
    out.extend_from_slice(&menger.min_counterpart_goods.to_le_bytes());
    out.extend_from_slice(&menger.stability_ticks.to_le_bytes());
    out.extend_from_slice(&menger.indirect_min_acceptance_share_bps.to_le_bytes());
}

fn push_demography_config_bytes(out: &mut Vec<u8>, demo: &DemographyConfig) {
    out.extend_from_slice(&(demo.households.len() as u32).to_le_bytes());
    for household in &demo.households {
        out.extend_from_slice(&household.founders.to_le_bytes());
        out.extend_from_slice(&household.time_preference_base_bps.to_le_bytes());
        out.extend_from_slice(&household.food_provision.to_le_bytes());
        out.extend_from_slice(&household.wood_provision.to_le_bytes());
        out.extend_from_slice(&household.starting_gold.to_le_bytes());
        out.extend_from_slice(&household.starting_food.to_le_bytes());
        out.extend_from_slice(&household.starting_wood.to_le_bytes());
    }
    out.extend_from_slice(&demo.ticks_per_year.to_le_bytes());
    out.extend_from_slice(&demo.old_age_onset_years.to_le_bytes());
    out.extend_from_slice(&demo.lifespan_span_years.to_le_bytes());
    out.extend_from_slice(&demo.birth_interval.to_le_bytes());
    out.extend_from_slice(&demo.birth_hunger_ceiling.to_le_bytes());
    out.extend_from_slice(&demo.max_household_size.to_le_bytes());
    out.extend_from_slice(&demo.child_food_endowment.to_le_bytes());
    out.extend_from_slice(&demo.child_gold_endowment.to_le_bytes());
    out.extend_from_slice(&demo.mutation_delta_bps.to_le_bytes());
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
        // G6b content recipes; pre-G6b configs never serialize these, so existing
        // digests are byte-identical.
        RecipeId::Research => 5,
        RecipeId::Confect => 6,
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
    fn medium_scale_extension_inserts_near_wants_below_survival() {
        // A scale with a present (Now) survival want and a future (Later) savings
        // want; the medium wants must land between them (survival first, then the
        // medium, then savings) and be `Horizon::Next` good wants for the medium.
        let mut scale = vec![
            Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Now,
                qty: 1,
                satisfied: false,
            },
            Want {
                kind: WantKind::Good(SALT),
                horizon: Horizon::Later(4),
                qty: 1,
                satisfied: false,
            },
        ];
        medium_scale_extension(&mut scale, WOOD, 2);
        assert_eq!(scale.len(), 4, "two medium wants were added");
        // Survival (the Now want) stays first.
        assert!(matches!(scale[0].horizon, Horizon::Now));
        // The two medium wants follow, before the Later savings want.
        assert_eq!(scale[1].kind, WantKind::Good(WOOD));
        assert_eq!(scale[1].horizon, Horizon::Next);
        assert_eq!(scale[2].kind, WantKind::Good(WOOD));
        assert_eq!(scale[2].horizon, Horizon::Next);
        assert!(matches!(scale[3].horizon, Horizon::Later(_)));

        // Zero qty is a no-op.
        let mut empty = scale.clone();
        let before = empty.clone();
        medium_scale_extension(&mut empty, WOOD, 0);
        assert_eq!(empty, before);
    }

    #[test]
    fn report_conserves_accounts_the_promotion_sink() {
        // A tick that converts 5 units of SALT to money (a promotion): the physical
        // ledger drops by exactly the promoted units, and `conserves` accepts it
        // only when the `promoted` term balances the drop.
        let mut report = EconTickReport::default();
        report.whole_system_before.insert(SALT, 5);
        report.whole_system_after.insert(SALT, 0);
        report.promoted.insert(SALT, 5);
        assert!(
            report.conserves(),
            "the promotion sink must balance the drop"
        );

        // Without the promoted term the same drop is a conservation violation.
        report.promoted.clear();
        assert!(
            !report.conserves(),
            "an unaccounted physical drop must fail conservation"
        );
    }

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
    fn bank_phase_respects_tight_fiduciary_tick_cap() {
        let mut s = Settlement::generate(7, &SettlementConfig::bank());
        s.run_bank_phase();
        let borrower = s
            .live_colonist_slots
            .iter()
            .find_map(|&slot| {
                (s.colonists[slot].vocation == Vocation::Gatherer).then_some(s.colonists[slot].id)
            })
            .expect("banked settlement has a gatherer borrower");
        let depositors = s
            .live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                (s.colonists[slot].vocation == Vocation::Consumer).then_some(s.colonists[slot].id)
            })
            .collect::<Vec<_>>();
        {
            let money_system = s
                .society
                .money_system
                .as_mut()
                .expect("banked settlement runs on the M3 ledger");
            for depositor in depositors {
                let claim = money_system.demand_claim_on(depositor, BANK_ID);
                if claim > Gold::ZERO {
                    money_system
                        .transfer_spendable(depositor, borrower, claim)
                        .expect("test claim transfer is funded by the depositor's demand claim");
                }
            }
            money_system.reconcile_agent_cache(s.society.agents.as_mut_slice());
        }
        let before = s
            .bank()
            .expect("banked settlement charters a bank")
            .fiduciary_issued;
        s.society
            .banks
            .iter_mut()
            .find(|bank| bank.id == BANK_ID)
            .expect("banked settlement charters a bank")
            .policy
            .max_new_fiduciary_per_tick = Gold(3);

        s.run_bank_phase();

        let bank = s.bank().expect("banked settlement charters a bank");
        assert_eq!(
            bank.fiduciary_issued
                .checked_sub(before)
                .expect("fiduciary issuance is monotone"),
            Gold(3),
            "direct G8b lending must stop at the bank's per-tick fiduciary cap"
        );
    }

    #[test]
    fn demography_provisions_report_only_credited_headroom() {
        let mut config = SettlementConfig::lineages();
        config.demography = Some(DemographyConfig {
            households: vec![crate::demography::HouseholdSpec {
                founders: 1,
                time_preference_base_bps: 500,
                food_provision: 7,
                wood_provision: 7,
                starting_gold: 0,
                starting_food: u32::MAX,
                starting_wood: u32::MAX - 1,
            }],
            birth_interval: 100,
            ..DemographyConfig::lineages()
        });
        let mut s = Settlement::generate(1, &config);
        let id = s.colonist_id(0).unwrap();
        let mut report = EconTickReport::default();

        s.deliver_demography_provisions(&mut report);

        let agent = s.society.agents.get(id).unwrap();
        assert_eq!(agent.stock.get(FOOD), u32::MAX);
        assert_eq!(agent.stock.get(WOOD), u32::MAX);
        assert_eq!(
            report.endowment_of(FOOD),
            0,
            "saturated FOOD stock must not report uncredited provision"
        );
        assert_eq!(
            report.endowment_of(WOOD),
            1,
            "only WOOD headroom should be reported as provisioned"
        );
    }

    #[test]
    fn estate_to_heir_overflow_routes_remainder_to_commons() {
        // A death's estate that would push a living heir's stock past `u32::MAX` must
        // not silently saturate-and-drop the overflow: the heir takes only its headroom
        // and the uncreditable remainder routes to the commons, so whole-system
        // conservation holds even at the ceiling. (The saturating `Stock::add` would
        // otherwise vanish the overflow — this pins the headroom clamp.)
        let mut config = SettlementConfig::lineages();
        config.demography = Some(DemographyConfig {
            households: vec![crate::demography::HouseholdSpec {
                founders: 2,
                time_preference_base_bps: 500,
                food_provision: 0,
                wood_provision: 0,
                starting_gold: 0,
                starting_food: u32::MAX - 1,
                starting_wood: 0,
            }],
            ..DemographyConfig::lineages()
        });
        // Settle directly post-generate (no tick, no provision, no consumption), so each
        // founder holds exactly `starting_food` and the heir's headroom is a single unit.
        let mut s = Settlement::generate(1, &config);
        let deceased = s.colonist_id(0).unwrap();
        let heir = s.colonist_id(1).unwrap();
        assert_eq!(
            s.society.agents.get(heir).unwrap().stock.get(FOOD),
            u32::MAX - 1
        );

        let before = s.whole_system_total(FOOD);

        // Mirror the real caller: mark the dying member dead, then settle to heirs.
        let slot = s.slot_for_id(deceased).unwrap();
        s.mark_colonist_dead(slot);
        s.settle_estate_to_heirs(deceased);

        // The heir saturates at the ceiling, the remainder (the deceased's stock minus
        // the heir's single unit of headroom) lands in the commons, and total FOOD is
        // unchanged — nothing minted, nothing lost.
        assert_eq!(
            s.society.agents.get(heir).unwrap().stock.get(FOOD),
            u32::MAX
        );
        assert_eq!(s.commons_stock_of(FOOD), u64::from(u32::MAX - 2));
        assert_eq!(
            s.whole_system_total(FOOD),
            before,
            "estate overflow to the commons must conserve total FOOD"
        );
    }

    #[test]
    fn frontier_estate_gold_inherits_after_emergent_promotion() {
        // After G5a promotion the frontier's money balances live in `Agent.gold` even
        // though the money regime is still `Emergent(SALT)`. The public econ
        // `credit_gold` half-move correctly rejects that regime, but household
        // inheritance must still route an already-collected estate to the heir instead
        // of diverting it to the commons.
        let mut s = Settlement::generate(2_026, &SettlementConfig::frontier());

        let mut victim_slot = None;
        for tick in 0..120 {
            let report = s.econ_tick();
            assert!(report.conserves(), "frontier ledger broke at tick {tick}");
            if s.current_money_good() != Some(SALT) {
                continue;
            }
            victim_slot = s.live_colonist_slots.iter().copied().find(|&slot| {
                let colonist = &s.colonists[slot];
                let Some(household) = colonist.household else {
                    return false;
                };
                let has_gold = s
                    .society
                    .agents
                    .get(colonist.id)
                    .is_some_and(|agent| agent.gold > Gold::ZERO);
                let has_heir = s
                    .live_colonist_slots
                    .iter()
                    .any(|&other| other != slot && s.colonists[other].household == Some(household));
                has_gold && has_heir
            });
            if victim_slot.is_some() {
                break;
            }
        }

        let slot = victim_slot.expect("a promoted frontier household member holds money");
        let victim = s.colonists[slot].id;
        let household = s.colonists[slot].household.expect("household member");
        let estate_gold = s.society.agents.get(victim).expect("live victim").gold;
        assert!(
            estate_gold > Gold::ZERO,
            "the estate must exercise gold routing"
        );
        assert_eq!(
            s.current_money_good(),
            Some(SALT),
            "the test must run in the post-promotion emergent-money phase"
        );

        s.mark_colonist_dead(slot);
        let heir = s.heir_for(victim).expect("same-household heir");
        let heir_gold_before = s.society.agents.get(heir).expect("live heir").gold;
        let total_gold_before = s.total_gold();
        let commons_gold_before = s.commons_gold();

        assert!(
            !s.society.credit_gold(heir, estate_gold),
            "the external gold accessor must still reject emergent-money societies"
        );
        assert!(s.settle_estate_to_heirs(victim));

        let heir_gold_after = s.society.agents.get(heir).expect("live heir").gold;
        assert_eq!(
            heir_gold_after,
            heir_gold_before
                .checked_add(estate_gold)
                .expect("small frontier estate fits"),
            "the heir must inherit the post-promotion money balance"
        );
        assert_eq!(
            s.commons_gold(),
            commons_gold_before,
            "household-routed money must not be diverted to commons"
        );
        assert_eq!(
            s.total_gold(),
            total_gold_before,
            "estate settlement must conserve total money"
        );
        assert_eq!(
            s.estate_destination_of(slot),
            Some(EstateDestination::Household { household, heir })
        );
    }

    #[test]
    fn birth_gold_endowment_uses_only_unreserved_parent_balance() {
        let mut config = SettlementConfig::lineages();
        config.demography = Some(DemographyConfig {
            households: vec![crate::demography::HouseholdSpec {
                founders: 1,
                time_preference_base_bps: 500,
                food_provision: 0,
                wood_provision: 0,
                starting_gold: 5,
                starting_food: 8,
                starting_wood: 0,
            }],
            birth_interval: 0,
            max_household_size: 2,
            child_food_endowment: 4,
            child_gold_endowment: 5,
            ..DemographyConfig::lineages()
        });
        let mut s = Settlement::generate(1, &config);
        let parent = s.colonist_id(0).unwrap();
        let bid = econ::market::Order {
            agent: parent,
            side: econ::market::OrderSide::Bid,
            good: FOOD,
            limit: Gold(1),
            qty: 4,
            seq: 1,
            expires_tick: 10,
        };
        assert!(s
            .society
            .reservations
            .reserve_order(&s.society.agents, &bid));
        assert_eq!(s.society.reservations.reserved_gold(parent), Gold(4));

        assert_eq!(s.run_births(), 1);

        let child = s.colonist_id(1).unwrap();
        assert_eq!(
            s.society.agents.get(child).unwrap().gold,
            Gold(1),
            "the newborn gets only the parent's unreserved gold"
        );
        let parent_agent = s.society.agents.get(parent).unwrap();
        assert_eq!(parent_agent.gold, Gold(4));
        assert!(
            s.society.reservations.reserved_gold(parent) <= parent_agent.gold,
            "birth must not leave reserved gold above the parent's balance"
        );
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
    fn canonical_bytes_include_m3_ledger_money_runtime() {
        // M3 starts with the same public money quantities as the M1 viable economy,
        // but its future stepping is ledger-backed. The canonical state must encode
        // that regime and the ledger rows, or generation-time M1/M3 twins collide.
        let m1 = Settlement::generate(7, &SettlementConfig::viable());
        let m3 = Settlement::generate(7, &SettlementConfig::m3_settlement());

        assert!(
            !m1.is_m3() && m3.is_m3(),
            "the twins must differ only by money regime"
        );
        assert_ne!(
            m1.canonical_bytes(),
            m3.canonical_bytes(),
            "M1 and M3 settlements must not serialize identically"
        );
        assert_ne!(
            m1.digest(),
            m3.digest(),
            "M1 and M3 settlements must not digest identically"
        );

        let mut expected = vec![1];
        push_money_system_bytes(
            &mut expected,
            m3.society.money_system.as_ref().expect("M3 money system"),
        );
        let bytes = m3.canonical_bytes();
        assert!(
            bytes
                .windows(expected.len())
                .any(|window| window == expected.as_slice()),
            "the M3 ledger snapshot is missing from canonical bytes"
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
    #[should_panic(expected = "emergent medium cannot be GOLD")]
    fn generate_rejects_gold_emergent_medium() {
        // GOLD is the money ledger, not a physical good: it never enters
        // `self.goods`, the deposit attribution, the transfer, or the conservation
        // report. A GOLD medium with a positive endowment would mint stock the
        // digest and whole-system ledger never track — `generate` rejects it at the
        // seam rather than ship a silent money leak.
        let mut config = SettlementConfig::barter_camp();
        let barter = config.barter.as_mut().expect("barter overlay");
        barter.medium_good = GOLD;
        barter.menger.candidate_goods = vec![FOOD, WOOD, GOLD];
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
    fn demography_provisions_the_hunger_staple_not_just_food() {
        // G5b generalizes the G4b household hearth to provision the settlement's hunger
        // staple ([`KnownGoods::hunger`]). On a `lineages` colony that is FOOD (byte-
        // identical to G4b); composed with a `bread_is_staple` chain it becomes bread,
        // so householders are endowed and provisioned in the very good they eat. The
        // composition the pre-G5b FOOD-only guard used to reject is now supported.
        let mut config = SettlementConfig::lineages();
        let mut chain = ChainConfig::grain_flour_bread();
        // No spatial producers — just the demography colony plus the chain's staple
        // mapping (bread), so the test isolates the provision good.
        chain.millers = 0;
        chain.bakers = 0;
        config.chain = Some(chain);
        let mut s = Settlement::generate(1, &config);
        let bread = s.content().expect("chain content").bread();

        // A founder starts with its staple buffer in bread, never FOOD.
        let founder = s.colonist_id(0).expect("a founder");
        let stock = &s
            .society
            .agents
            .get(founder)
            .expect("founder resolves")
            .stock;
        assert!(
            stock.get(bread) > 0,
            "the founder holds a bread staple buffer"
        );
        assert_eq!(stock.get(FOOD), 0, "FOOD is no longer the household staple");

        // The provision phase mints bread (the staple), recorded as a conserved source.
        let mut report = EconTickReport::default();
        s.deliver_demography_provisions(&mut report);
        assert!(
            report.endowment_of(bread) > 0,
            "the staple bread is provisioned"
        );
        assert_eq!(
            report.endowment_of(FOOD),
            0,
            "FOOD is no longer the provisioned staple"
        );
    }

    #[test]
    fn barter_chain_without_bread_staple_saves_the_medium() {
        // A barter overlay composed with a chain whose bread is NOT the staple is a
        // coherent (if unshipped) camp: hunger stays FOOD, yet the emergent medium is
        // still endowed and circulated (`build_agent` always adds `medium_good` under a
        // barter overlay; the post-promotion market runs `step_rejecting_v2_*`). The
        // savings want must therefore name the medium, not the lab-default GOLD —
        // otherwise colonists would save GOLD while the market clears SALT, and
        // `run_role_choice`'s `soonest_savings_horizon(money_good)` would find no
        // matching want and never adopt a role. Guards the generation arm.
        let mut config = SettlementConfig::frontier();
        config
            .chain
            .as_mut()
            .expect("frontier ships a chain")
            .bread_is_staple = false;
        let s = Settlement::generate(7, &config);

        assert_eq!(
            s.known.savings, SALT,
            "a barter-start chain saves the emergent medium even when bread is not staple"
        );
        assert_eq!(
            s.known.hunger, FOOD,
            "with bread not the staple, hunger stays FOOD"
        );

        // The retargeted savings want is exactly what role-choice looks for: at least
        // one (patient) colonist carries a future `Good(SALT)` savings want, and no
        // colonist saves GOLD (the lab-default fallthrough the fix removes).
        let mut saw_salt_savings = false;
        for index in 0..s.population() {
            let id = s.colonist_id(index).expect("colonist id");
            let scale = &s
                .society
                .agents
                .get(id)
                .expect("living colonist resolves")
                .scale;
            for want in scale {
                if let WantKind::Good(good) = want.kind {
                    assert_ne!(good, GOLD, "no colonist saves GOLD under a barter overlay");
                    if good == SALT && matches!(want.horizon, Horizon::Later(_)) {
                        saw_salt_savings = true;
                    }
                }
            }
        }
        assert!(
            saw_salt_savings,
            "a patient colonist carries a future SALT savings want the appraisal can target"
        );
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
    fn canonical_bytes_include_barter_config() {
        // Same generated physical state, different barter overlay: future scale
        // regeneration / promotion checks will diverge, so emergent configs must not
        // collide in the determinism digest before the first tick.
        let base = SettlementConfig::barter_camp();

        let mut stronger_medium_want = SettlementConfig::barter_camp();
        stronger_medium_want
            .barter
            .as_mut()
            .expect("barter overlay")
            .medium_want_qty += 1;

        let mut stricter_promotion = SettlementConfig::barter_camp();
        stricter_promotion
            .barter
            .as_mut()
            .expect("barter overlay")
            .menger
            .min_total_acceptances += 1;

        let base = Settlement::generate(7, &base);
        let stronger_medium_want = Settlement::generate(7, &stronger_medium_want);
        let stricter_promotion = Settlement::generate(7, &stricter_promotion);

        assert_ne!(
            base.canonical_bytes(),
            stronger_medium_want.canonical_bytes(),
            "medium_want_qty must be part of the barter config identity"
        );
        assert_ne!(
            base.canonical_bytes(),
            stricter_promotion.canonical_bytes(),
            "Mengerian thresholds must be part of the barter config identity"
        );
    }

    #[test]
    fn canonical_bytes_include_emergence_runtime() {
        // A barter camp run into the barter phase accumulates saleability state (the
        // per-candidate acceptance counts plus the DISTINCT acceptor/counterpart
        // members and the stability latch) that steers the FUTURE promotion tick.
        // That state must ride in the canonical digest — otherwise two barter states
        // with equal holdings but different tracker progress would collide and then
        // promote on different ticks. Reconstruct the runtime bytes from econ's
        // accessors and assert they appear verbatim in the digest input.
        let mut s = Settlement::generate(2_026, &SettlementConfig::barter_camp());
        // Advance into barter but stop before promotion so the tracker is live.
        for _ in 0..3 {
            s.econ_tick();
        }
        assert!(
            s.in_barter_phase(),
            "the run must still be bartering so the tracker is live"
        );
        let emergence = s
            .society
            .emergence()
            .expect("a barter camp runs econ's emergence");
        assert!(
            emergence.tracker().total_acceptances() > 0,
            "the test is vacuous — no barter was observed"
        );

        let mut expected = Vec::new();
        push_emergence_runtime_bytes(&mut expected, emergence);
        let bytes = s.canonical_bytes();
        assert!(
            bytes
                .windows(expected.len())
                .any(|window| window == expected.as_slice()),
            "the accumulated emergence runtime is missing from the canonical bytes"
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
