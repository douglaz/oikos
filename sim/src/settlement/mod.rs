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

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use econ::agent::{Agent, AgentId, AskOutcome, Role, Want, WantKind};
use econ::agio::{provisioning_bitmap_for_money, TemporalEndowment};
use econ::bank::{Bank, BankPolicy};
use econ::barter::{BarterReason, BarterTrade};
use econ::bundle::{
    appraise_project_bundle_for_money, ProjectBundleCandidate, ProjectBundleEndowment,
};
use econ::cantillon::{CantillonReceipt, CantillonRoute, CantillonSector};
use econ::capital::{
    M2Project, M2ProjectState, ProjectFundingPlan, ProjectLineId, ProjectOutputLot,
};
use econ::expect::PriceBelief;
use econ::good::{Gold, GoodId, Horizon, Stock, FOOD, GOLD, NET, SALT, WOOD};
use econ::ledger::BankId;
use econ::market::OrderSide;
use econ::marketability::{GoodMarketability, MarketabilityAcceptance, MarketabilityConfig};
use econ::menger::MengerianEmergence;
use econ::money::{
    BankRepaymentTender, DesignatedMoney, IssuerRepaymentTender, LaborWageTender,
    MarketMoneyConfig, MengerianConfig, PublicDebtTender, PublicSpotTender, Regime,
    ReserveRatioBps, TaxReceivability,
};
use econ::project::{
    advance_project, build_cultivation_tool_template, build_mill_template, build_oven_template,
    complete_project_if_ready, start_project, Project, ProjectId, ProjectTemplate,
    ProjectTemplateId, Recipe, RecipeId, Tick,
};
use econ::purpose::{CreditLender, CreditSource, DebtPurpose, ProjectPlanId};
use econ::rng::Rng;
use econ::scenario::{
    builtin_market_scenario, Event, EventKind, MarketScenario, RedemptionRoute, ScenarioName,
};
use econ::shadow::run_credit_disabled_shadow;
use econ::society::{
    AllocationExecutionStatus, AllocationRecord, QuoteOutcome, Society, TracedWant,
};
use econ::timemarket::{DebtContract, DebtId, DebtState};

use life::{
    deterministic_mix64, max_savings_ladder_horizon, regenerate_scale,
    regenerate_scale_for_capital, CultureParams, KnownGoods, NeedDynamics, NeedIntake, NeedState,
    FORECAST_BIAS_NEUTRAL_BPS,
};

use world::{AgentStatus, Grid, NodeId, Pos, ResourceNode, Stockpile, StockpileId, Task, World};

use crate::content::ContentSet;
use crate::demography::{child_seed, founder_seed, DemographyConfig, HouseholdSpec};

/// Fast `world` ticks per economic tick — the two-rate ratio (game-spec §4.1).
/// A gatherer's round trip to a node costs `2 × distance` fast ticks, so a node
/// far from the exchange completes fewer trips inside this fixed budget and
/// delivers fewer units per econ tick. Holding this fixed while varying distance
/// is exactly the distance→price experiment.
pub const FAST_TICKS_PER_ECON_TICK: u64 = 24;

/// S15: per-econ-tick own-labor budget for the cultivation phase. Sim colonists keep
/// `econ` direct-labor capacity at zero so they do not run content recipes through the
/// generic market path; this external budget is the time envelope the own-use phase
/// spends through the checked direct-recipe seam.
///
/// The budget is a deliberately generous *produce ceiling*, not a per-tick rate target.
/// At [`CULTIVATE_LABOR`] = 2 it allows up to 24 loaves/cultivator/econ-tick versus a
/// [`ChainConfig::cultivate_consume`] of 4 eaten — a ~6× gap whose surplus accrues in
/// stock as the child-endowment reserve (the broadened birth-food rule endows newborns
/// from a parent's cultivated bread). In practice it is GRAIN-flow-bound, never
/// labor-bound: the phase stops as soon as the colonist's hauled grain runs out, so the
/// real per-tick output tracks the grain the cultivator could carry, well under the
/// ceiling. The headroom only ensures a bumper haul is fully converted in one tick.
const OWN_USE_CULTIVATION_LABOR_BUDGET: u32 = 48;

/// C1R — pinned headline output share, swept by the acceptance suite.
pub const SHARE_TENANCY_BPS_DEFAULT: u16 = 5_000;
/// C1R — pinned headline contract term in econ ticks, swept by the acceptance suite.
pub const SHARE_TENANCY_TERM_DEFAULT: u16 = 12;
/// C3R.a — keep mortal producer seed derivation in a disjoint founder-index band.
const MORTAL_CHAIN_PRODUCER_SEED_OFFSET: usize = 1_000_000;
/// C3R.b — dedicated one-producer households for the mortal mill/bake seed band.
const MORTAL_PRODUCER_HOUSEHOLDS: usize = 6;
/// C3R.b — one producer plus one child-heir slot per producer household.
const MORTAL_PRODUCER_HOUSE_CAP_DEFAULT: u8 = 2;

/// S22b — pinned **cultivation-skill** magnitudes (the bounded accumulate/decay scalar that
/// raises a skilled cultivator's per-trip grain haul). Modest, house-style, NOT tuned to a
/// target: a small per-tick gain, a slow decay (`DECAY < GAIN`, so sustained cultivation builds
/// a durable advantage idleness erodes only gradually), a bounded cap, and a ≤2× haul ceiling.
/// They are the [`ChainConfig`] defaults; the controls/sweep override the chain fields, never
/// these constants. Skill is born at 0 (earned, not inherited).
const SKILL_GAIN: u16 = 50;
const SKILL_DECAY: u16 = 5;
const SKILL_CAP: u16 = 1_000;
/// S22b: the full-skill grain-haul multiplier (the harvest-room ceiling). At skill `SKILL_CAP`
/// a cultivator's per-trip haul is `SKILL_HAUL_CEILING × carry_cap`; `2` is the shipped ≤2×
/// bound (`carry_cap + carry_cap·skill/cap`). `1` is the no-op (skill has no productivity effect —
/// the cap-zero control), and a larger value (e.g. `4`) is the exaggerated-cap SENSITIVITY probe.
const SKILL_HAUL_CEILING: u32 = 2;

/// S22c — pinned **profit-driven-retention** magnitudes (the post-money stay-decision that lets a
/// cultivating agent remain cultivating past the normal hunger exit when its realized
/// cultivation-sale return clears its outside option). House-style, NOT tuned to a target:
///
/// * [`RETURN_WINDOW`] — the rolling window (econ ticks) over which a colonist's realized
///   cultivation-sale vs non-cultivation-sale proceeds are accumulated as the return signal.
/// * [`RETENTION_MARGIN_BPS`] — the basis-point margin the cultivation rate must clear the outside
///   rate by; `0` is the shipped slice (cultivating must earn *at least* the outside option),
///   swept as sensitivity.
/// * [`RETENTION_MATERIAL_FLOOR`] — a small floor on the windowed cultivation proceeds so a single
///   dust sale cannot lock an agent in (the signal must be a real, recurring realized gain).
///
/// All three are the pinned defaults; the controls/sweep override them, never these constants.
const RETURN_WINDOW: u64 = 48;
const RETENTION_MARGIN_BPS: u64 = 0;
const RETENTION_MATERIAL_FLOOR: u64 = 2;

/// S22d — pinned **durable-cultivation-capital** magnitudes (the sunk-cost, owned, role-specific
/// tool that raises only its owner's grain-haul ceiling while it cultivates). House-style, NOT
/// tuned to manufacture a cohort; the controls/sweep override the chain fields, never these
/// constants:
///
/// * [`TOOL_BUILD_PATIENCE`] — the realized-cultivation-output tenure (consecutive output ticks,
///   `Colonist::cultivation_tenure`) a cultivator must sustain before it invests in a tool. A
///   distinct counter from `cultivate_pressure` (a hunger-ENTRY streak): tenure credits ONLY on a
///   tick of realized cultivation output and RESETS otherwise, so only a sustained PRODUCING
///   cultivator builds (not one merely pressured to enter).
/// * [`CULTIVATION_TOOL_HAUL_CEILING`] — the OWNER's per-trip grain-haul ceiling (×`carry_cap`)
///   while cultivating, routed through the same conserved-node haul lever as S22b skill. `> 1` so a
///   tool-owner strictly out-hauls a non-owner (whose ceiling is `1×carry_cap`, the S22c no-tool
///   return); `1` is the no-boost control. The WOOD/labor build cost reuses the existing
///   [`ChainConfig::tool_build_wood`]/[`ChainConfig::tool_build_labor`] (the producer-capital
///   chain is OFF on every cultivation scenario, so the fields are free to drive the plow build).
const TOOL_BUILD_PATIENCE: u16 = 12;
const CULTIVATION_TOOL_HAUL_CEILING: u32 = 3;

/// S22e — the EXPANDED lineage roster for the endowed-capital scenarios. The base cultivation
/// colony has only 2 lineage households, so the `PERSIST_COHORT` (4) owner-lineage success floor is
/// unreachable there; the headline + all matched controls run on this many lineage households,
/// proportionally expanding the WOOD-poor cultivator/woodcutter/consumer demand side so money +
/// mortality still hold. A multiple of the base household count (2) so the demand side scales by an
/// exact integer factor. The shipped `ENDOWED_TOOL_COUNT_DEFAULT` minority leaves owner-share
/// head-room.
const ENDOWED_ROSTER_HOUSEHOLDS: usize = 8;

/// S22e — the headline endowment count: how many of the [`ENDOWED_ROSTER_HOUSEHOLDS`] lineage
/// households start with a plow. A MINORITY edge (`2 × count ≤ roster`) that is also `≥`
/// `PERSIST_COHORT` (4) so an owner-lineage cohort is reachable yet ownership stays a minority. The
/// sweep raises it toward universal to show the `UniversalOwnership` boundary.
const ENDOWED_TOOL_COUNT_DEFAULT: u16 = 4;

/// S22f — the shipped voluntary fixed-term cultivation commitment **term** (the binding length, in
/// econ ticks): drawn from the existing [`RETURN_WINDOW`] scale so it is not a fitted figure. The
/// headline/success bar requires `commitment_term < ceil(PERSIST_FRACTION × FINAL_WINDOW)` (the
/// test asserts it) so a single term mathematically cannot span the persistence window — persistence
/// must come from RENEWALS from fresh post-expiry signals, not one long binding. The robustness
/// sweep drives `{12, 24, 48, 96}` around this default; `2 × RETURN_WINDOW` is the over-long end.
const COMMITMENT_TERM_DEFAULT: u16 = RETURN_WINDOW as u16;

/// S22f — the shipped voluntary-commitment **entry floor**: the floor on a candidate's windowed
/// realized cultivation proceeds below which its entry signal does not clear (so a single dust sale
/// cannot trigger an opt-in). Reuses the S22c material-floor logic / magnitude
/// ([`RETENTION_MATERIAL_FLOOR`]); the `unprofitable_offer` control raises it to `u64::MAX` for zero
/// uptake.
const COMMITMENT_ENTRY_FLOOR_DEFAULT: u64 = RETENTION_MATERIAL_FLOOR;

/// S22f — the `fiat_pin` CONTROL's default forced-commit count: a small deterministic minority of
/// the expanded roster. The control is a bounded forced re-pin used to falsify voluntary uptake, not
/// the headline; tests may override `commitment_fiat_pin` when they need a different forced count.
const COMMITMENT_FIAT_PIN_DEFAULT: u16 = 6;

const COMMITMENT_SEED_SHARE_BPS_DEFAULT: u16 = 1_500;
const COMMITMENT_NORM_IMITATION_PERIOD_DEFAULT: u64 = 24;
const COMMITMENT_NORM_IMITATION_WINDOW_DEFAULT: u64 = 48;
const COMMITMENT_NORM_IMITATION_MARGIN_BPS_DEFAULT: u64 = 1_500;
const COMMITMENT_NORM_IMITATION_RADIUS_DEFAULT: u16 = 1;
const COMMITMENT_NORM_IMITATION_MAX_MODELS_DEFAULT: u16 = 8;
const COMMITMENT_NORM_FOOD_WINDOW_TARGET_DEFAULT: u64 = COMMITMENT_NORM_IMITATION_WINDOW_DEFAULT;
const COMMITMENT_NORM_SCORE_BPS: u64 = 10_000;
const COMMITMENT_NORM_ALIVE_WEIGHT: u64 = 2;
const COMMITMENT_NORM_HUNGER_WEIGHT: u64 = 1;
const COMMITMENT_NORM_FOOD_WEIGHT: u64 = 1;
const COMMITMENT_NORM_SALT_WEIGHT: u64 = 1;
const COMMITMENT_NORM_GROUP_MIN_SIZE: usize = 3;
const COMMITMENT_NORM_ADOPTER_SHARE_GAP_BPS: u64 = 1_000;
const COMMITMENT_NORM_SEED_CLUSTER: bool = true;
pub const ABANDONABLE_NORM_ADOPTER_SHARE_MIN: f64 = 0.15;
pub const ABANDONABLE_NORM_ADOPTER_SHARE_MAX: f64 = 0.6;
pub const ABANDONABLE_NORM_MIN_ABANDONMENTS: u64 = 8;
pub const ABANDONABLE_NORM_CORE_MARGIN: usize = 4;
pub const ABANDONABLE_NORM_CHURN_FLIP_RATE: f64 = 0.5;
pub const ABANDONABLE_NORM_CHURN_SHARE_VAR: f64 = 0.01;

/// S23a — the shipped private-land idle-forfeiture clock. A plot reverts only after this many
/// consecutive fast ticks with no owner engagement (no harvest task, no pending carried grain from
/// that plot, and no harvest event this tick).
const LAND_IDLE_LIMIT_DEFAULT: u16 = 12;
const LAND_TOTAL_PLOTS_DEFAULT: u16 = 48;
const LAND_GOOD_PLOTS_DEFAULT: u16 = 4;
const LAND_MARGINAL_PLOTS_DEFAULT: u16 = LAND_TOTAL_PLOTS_DEFAULT - LAND_GOOD_PLOTS_DEFAULT;
const LAND_MARKET_TOTAL_PLOTS_DEFAULT: u16 = 28;
const LAND_GOOD_REGEN: u32 = 64;
const LAND_GOOD_CAP: u32 = 8_000;
const LAND_MARGINAL_REGEN_DEFAULT: u32 = 12;
const LAND_MARGINAL_CAP: u32 = 1_000;
const LAND_GOOD_START_X: u16 = 2;
const LAND_MARGINAL_START_X: u32 = 12;
const LAND_MARGINAL_SPACING: u32 = 6;
const LAND_GOOD_TO_MARGINAL_GAP: u32 = 4;
const LAND_LAYOUT_MIN_WIDTH: u16 = 64;
const LAND_LAYOUT_MARGIN: u32 = 10;
const LAND_CARRYING_COST_DEFAULT: u64 = 1;
const LAND_PRICE_CAP_FACTOR_DEFAULT: u64 = 1;
pub const RIVAL_COMMONS_BASELINE_FINAL_WINDOW_TICKS: u64 = 400;
pub const RIVAL_COMMONS_BASELINE_EMERGENCY_DRAW: u64 = 12_768;
pub const RIVAL_COMMONS_K_TICKS: u64 = 3;
pub const RIVAL_COMMONS_PHI_ABUNDANT_BPS: u32 = 12_500;
pub const RIVAL_COMMONS_PHI_MARGINAL_BPS: u32 = 5_000;
pub const RIVAL_COMMONS_PHI_SCARCE_BPS: u32 = 2_500;
const RIVAL_COMMONS_BPS_DENOM: u64 = 10_000;
pub const LAND_CARRYING_PERIOD: u64 = 12;
pub const LAND_RENT_WINDOW: u64 = 100;

pub fn rival_subsistence_commons_regen_for_phi(phi_bps: u32) -> u64 {
    let numerator = u128::from(RIVAL_COMMONS_BASELINE_EMERGENCY_DRAW) * u128::from(phi_bps);
    let denominator =
        u128::from(RIVAL_COMMONS_BASELINE_FINAL_WINDOW_TICKS * RIVAL_COMMONS_BPS_DENOM);
    ((numerator + denominator / 2) / denominator) as u64
}
pub const LAND_MIN_RENT_HISTORY: usize = 8;
pub const LAND_SALE_HISTORY_WEIGHT_BPS: u64 = 5_000;
pub const LAND_SALE_HISTORY_K: usize = 3;
pub const LAND_LIST_IDLE: u16 = 12;
pub const LAND_FORECLOSE_DISCOUNT_BPS: u64 = 2_000;
pub const LAND_PRICE_MIN: u64 = 1;

/// S23a viability floors (§2a `VIABLE_MARGINAL`): a grain plot counts as viable, homesteadable
/// land only when its `regen`/`cap` clear these floors (a plot that yields ~nothing does not
/// count). Re-exported so the acceptance suite shares the same numbers and they cannot drift.
/// Deliberately floor-based — an unowned good plot is *also* viable land, so this does NOT pin
/// to the marginal-cap literal (which a future cap sweep would otherwise break).
pub const LAND_VIABLE_REGEN_FLOOR: u32 = 8;
pub const LAND_VIABLE_CAP_FLOOR: u32 = 256;

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

/// S7.2: how many econ ticks back the capital-build appraisal looks for a real trade
/// of a tool's output good before it trusts that good's realized price. A build is only
/// appraised while its output is ACTUALLY clearing within this window, so a stale price
/// (frozen because the good stopped trading) cannot drive endless building — the
/// demand-anchored brake. Fixed (not a config knob): it bounds the recency scan and
/// gates only the gated S7.2 phase, so it never steers a pre-S7 run.
const CAPITAL_BUILD_RECENCY: u64 = 8;

/// S7.2: how many idle tools of a kind the capital-build phase tolerates before it
/// stops adding more of that kind — the slack over the active-producer count that
/// absorbs the emergent chain's adoption churn while keeping built capital bounded
/// near the producers that actually run it (the structural overinvestment guard).
/// Fixed (not a config knob): it gates only the gated S7.2 phase.
const CAPITAL_IDLE_SLACK: u64 = 1;

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

/// The econ id of the (single) issuer the G8c-1 credit cycle routes through — the
/// state that prints fiat / extends fiat-credit under the [`Regime::Fiat`] rung.
/// The lab's `EmergedGoldFiatCreditExpansion` scenario seeds exactly this issuer.
const ISSUER_ID: econ::ledger::IssuerId = econ::ledger::IssuerId(1);

/// Which G8c-1 **finance demonstration** a [`SettlementConfig::cycle`] runs.
///
/// This is the climax slice (the Austrian business cycle in the colony game): the
/// settlement routes the **regime ladder + fiat issuance** into econ's *unchanged*
/// ABCT machinery and runs the **credit-disabled shadow** replay to measure the
/// natural-rate gap. The two kinds are a falsification twin — same agents, same
/// roundabout project line, the *only* difference is whether credit expands:
///
/// - [`CreditCycle`](CycleKind::CreditCycle): the regime descends to
///   [`Regime::Fiat`], the issuer extends fiat-credit, the market rate falls below
///   the shadow natural rate (a measured **gap**), capitalists over-invest in the
///   long roundabout project (the **boom**), credit **stops**, the rate reasserts,
///   the malinvested project is abandoned and capital is consumed (the **bust**).
/// - [`SoundMoney`](CycleKind::SoundMoney): the **control** — `SoundGold`, no fiat,
///   no credit expansion — so the gap stays ≈ 0, no boom forms, nothing is
///   abandoned, and no capital is consumed. The proof the cycle is *credit*-driven,
///   not an artifact of the production/spatial dynamics.
///
/// Reuses econ's `Regime` ladder, `SetRegime`/`SetIssuerPolicy`/`StopIssuerCredit`
/// events, the boom/bust/abandonment/capital-consumption records, and the
/// `run_credit_disabled_shadow` counterfactual — all UNCHANGED. G8c-1 only routes
/// the sim's policy into them and reads the measured signals back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CycleKind {
    /// Fiat-credit expansion → boom → stop → bust → capital consumed.
    CreditCycle,
    /// The sound-money control: `SoundGold`, no fiat, no credit, no cycle.
    SoundMoney,
}

/// The G8c-1 **credit-cycle** overlay: the settlement runs the Austrian
/// business-cycle demonstration (or its sound-money control) on econ's unchanged
/// ABCT/regime/shadow machinery instead of a spatial colony.
///
/// A `cycle` settlement is a **finance** settlement: it has no gatherers/consumers
/// and no spatial production. Its [`Society`] is built from econ's credit-ladder
/// scenario (the lab's `EmergedGoldFiatCreditExpansion`), so the issuer, the
/// roundabout project line, the regime ladder, and the credit-ladder agents are all
/// reused unchanged; each econ tick the sim steps the society (the cycle runs
/// endogenously) and reads the boom/bust/gap/capital-consumed signals back from the
/// M3 records, with the credit-disabled shadow supplying the natural-rate baseline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CycleConfig {
    /// Which demonstration: the credit cycle or its sound-money control.
    pub kind: CycleKind,
    /// The G8c-2 **tender policy** layered onto the cycle's settlement surfaces. The
    /// *headline* knob is [`TenderPolicy::wage`]: under
    /// [`LaborWageTender::FiatAndSpecie`] (or [`ParAll`](LaborWageTender::ParAll)) the
    /// fiat-credit employers can pay fiat wages, so the fiat credit reaches workers and
    /// the boom→bust transmits; under [`LaborWageTender::SpecieOnly`] fiat wages are
    /// **refused**, the credit never enters the real economy, and the same issuance is
    /// inert (no boom, no bust). [`TenderPolicy::default`] equals econ's defaults, so a
    /// default cycle emits no tender events and is byte-identical to the G8c-1 cycle.
    pub tender: TenderPolicy,
}

/// The G8c-2 **tender policy** — the set of media-acceptance levers, one per
/// settlement surface, routed through econ's *unchanged* tender machinery (each
/// surface settles through its policy's
/// [`accepted_media`](econ::money::PublicSpotTender::accepted_media): a refused
/// medium cannot settle there even if held; the active medium does). Tender gates
/// **composition** (which medium settles a surface), never **totals** (no money is
/// created or destroyed by the policy). Set by config here; the player-`Command`
/// route is G9.
///
/// [`Default`] equals econ's per-surface defaults
/// ([`ParAll`](PublicSpotTender::ParAll) for spot/wage/debt/bank-repayment,
/// [`FiatOnly`](IssuerRepaymentTender::FiatOnly) for issuer-repayment); a config that
/// leaves a knob at its default emits **no** `SetXTender` event for it, so a
/// default-tender finance settlement is byte-identical to its G8c-1 form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TenderPolicy {
    /// Which media settle the **public spot** market (M11). Active on the spot bench.
    pub spot: PublicSpotTender,
    /// Which media settle **labor wages** (M17). The cycle's transmission valve.
    pub wage: LaborWageTender,
    /// Which media discharge **public debt** (M12). Active on the debt bench.
    pub debt: PublicDebtTender,
    /// Which media repay a **bank loan** (M15).
    pub bank_repayment: BankRepaymentTender,
    /// Which media the **issuer** accepts to repay fiat credit (M16).
    pub issuer_repayment: IssuerRepaymentTender,
}

impl Default for TenderPolicy {
    fn default() -> Self {
        Self {
            spot: PublicSpotTender::ParAll,
            wage: LaborWageTender::ParAll,
            debt: PublicDebtTender::ParAll,
            bank_repayment: BankRepaymentTender::ParAll,
            issuer_repayment: IssuerRepaymentTender::FiatOnly,
        }
    }
}

impl TenderPolicy {
    /// The `SetXTender` events this policy layers onto a finance scenario — **one per
    /// knob that differs from econ's default**, all at `Tick(0)` (set before any
    /// surface settles), in a fixed surface order (spot, debt, bank-repayment,
    /// issuer-repayment, wage). A knob left at its default emits nothing, so a
    /// default `TenderPolicy` contributes zero events and the scenario is byte-identical
    /// to the policy-free G8c-1 form. Reuses econ's `EventKind::SetXTender` unchanged.
    fn tender_events(self) -> Vec<Event> {
        let default = Self::default();
        let mut events = Vec::new();
        if self.spot != default.spot {
            events.push(Event {
                tick: Tick(0),
                kind: EventKind::SetPublicSpotTender(self.spot),
            });
        }
        if self.debt != default.debt {
            events.push(Event {
                tick: Tick(0),
                kind: EventKind::SetPublicDebtTender(self.debt),
            });
        }
        if self.bank_repayment != default.bank_repayment {
            events.push(Event {
                tick: Tick(0),
                kind: EventKind::SetBankRepaymentTender(self.bank_repayment),
            });
        }
        if self.issuer_repayment != default.issuer_repayment {
            events.push(Event {
                tick: Tick(0),
                kind: EventKind::SetIssuerRepaymentTender(self.issuer_repayment),
            });
        }
        if self.wage != default.wage {
            events.push(Event {
                tick: Tick(0),
                kind: EventKind::SetLaborWageTender(self.wage),
            });
        }
        events
    }
}

/// Which settlement surface a [`TenderBench`] demonstrates — the G8c-2 tender
/// **bench** wires the spot, debt, and repayment tenders as the same config-lever
/// mechanism the wage×cycle headline uses, on econ's unchanged M11-M16 scenarios. A
/// bench is a **finance** settlement with no spatial colony: econ sets up the held
/// medium for the demonstrated surface, and that surface's tender decides whether it
/// may settle there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchSurface {
    /// The **public spot** market (M11): under `FiatAndSpecie` the held fiat settles
    /// goods trades; under `SpecieOnly` it is refused and specie settles instead.
    Spot,
    /// **Public debt** discharge (M12): under `FiatAndSpecie` the seeded commodity
    /// debt is paid in fiat; under `SpecieOnly` fiat is refused and it is paid in
    /// specie. (The spot tender stays `SpecieOnly` so the debtor must hold its fiat
    /// for the debt surface — the lab's M12 construction, reused.)
    Debt,
    /// **Bank-loan repayment** (M15): under `BankClaimsAndSpecie` the borrower's
    /// unredeemable claim repays and retires bank credit; under `SpecieOnly` that
    /// claim is refused and the debt defaults.
    BankRepayment,
    /// **Issuer-credit repayment** (M16): under `FiatOnly` the returned fiat retires
    /// issuer credit; under `FiatRefused` the fiat is refused and the credit overhang
    /// remains.
    IssuerRepayment,
}

/// The G8c-2 **tender bench** overlay: a finance settlement that runs econ's
/// unchanged tender scenarios (M11-M16) to demonstrate one tender surface's
/// refusal-vs-acceptance. Paired refusal/acceptance benches show the policy gates
/// **composition** (which medium settles), never **totals**. Reuses the same
/// `SetXTender` lever as the cycle; only the base scenario (which exercises that
/// surface) differs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TenderBench {
    /// Which surface this bench demonstrates.
    pub surface: BenchSurface,
    /// The tender policy in force; the bench reads the knob for its surface.
    pub tender: TenderPolicy,
}

/// The fiat-holding agent the G8c-3 counter-lever levies: a fiat-credit **capitalist**
/// in the wage-refusal cycle. Under specie-only wages it holds the issuer's fiat
/// **idle** (it cannot pay specie-only wages), so a `FiatOnly` tax compels that idle
/// fiat back to the issuer through the fiscal channel — fiat circulates via tax where
/// the labor market refused it (the chartalist headline). The lab's
/// `EmergedGoldFiatCreditExpansion` seeds it (a `project_cluster` capitalist).
const TAX_FIAT_HOLDER: AgentId = AgentId(200);

/// The specie-holding agent the same levy targets: an `organic_credit_pair` **trader**
/// that holds specie (and no fiat) throughout the cycle. Under a `SpecieOnly` tax it
/// remits specie (`tax_receipts_specie > 0`); under a `FiatOnly` tax it holds no fiat
/// and the levy is unmet-by-rule (a default, not a leak). Taxing both a fiat-holder and
/// a specie-holder with the *same* levy in both configs is what isolates the compelled
/// fiat demand to the **receivability** policy: the two configs differ only in the
/// `SetTaxReceivability` value, and the gate alone decides which medium settles.
const TAX_SPECIE_HOLDER: AgentId = AgentId(100);

/// The levy each counter-lever target owes (well within each holder's balance, so the
/// receivable medium settles in full and the non-receivable one defaults — sign only).
const TAX_LEVY_AMOUNT: Gold = Gold(1);

/// The tick the counter-lever levy comes due. Chosen inside the cycle's
/// fiat-outstanding window (after the regime reaches `Fiat` and the capitalists hold
/// the borrowed fiat, before the loans unwind) and before any loan repayment retires
/// credit — so the tax settlement is purely fiscal at its due tick (test 4).
const TAX_DUE_TICK: Tick = Tick(8);

/// The G8c-3 **tax overlay** — the state's levy + receivability layered on the finance
/// (wage-refusal cycle) settlement, routed through econ's *unchanged* M21 machinery
/// (the `SetTaxReceivability` / `LevyTax` events, `apply_levy_tax`, `settle_due_debts_m3`
/// gated by [`TaxReceivability`], and the issuer tax accounts). The chartalist
/// counter-lever to G8c-2: under specie-only wages (fiat credit inert, no private fiat
/// demand) a **fiat-receivable** tax compels fiat demand through the **fiscal** channel.
///
/// A tax is a zero-principal [`DebtContract`] owed to the single state issuer; the
/// payables view pulls the agent's labor to cover the **amount**, and the receivability
/// gate decides which media may remit at settlement (the declared Known Seam — media
/// enter only at settlement; this overlay engineers no media-aware planning). Tax is
/// **fiscal, not credit**: receipts move into the issuer's tax accounts and never touch
/// `credit_retired` / `fiat_credit_outstanding`. Set by config here; the
/// player-`Command` route is G9. Single-issuer only (econ's M21): the levy carries no
/// issuer id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaxPolicy {
    /// Which media discharge the tax — the chartalist gate. `FiatOnly` compels fiat
    /// demand; `SpecieOnly` (the control) compels none; `FiatAndSpecie` accepts either.
    pub receivability: TaxReceivability,
    /// The levies the state raises, each a zero-principal liability owed to the single
    /// issuer. The counter-lever twin levies the *same* set in both configs, so only the
    /// receivability differs.
    pub levies: Vec<TaxLevy>,
}

/// One state levy — a single `LevyTax` event (econ's M21, single-issuer, no issuer id).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaxLevy {
    /// The taxed agent (the liability's borrower).
    pub agent: AgentId,
    /// The amount owed (the zero-principal debt's `due`).
    pub amount: Gold,
    /// The tick the levy comes due (when `settle_due_debts_m3` discharges or defaults it).
    pub due_tick: Tick,
}

impl TaxPolicy {
    /// The G8c-3 **counter-lever** levy: tax a fiat-holding capitalist
    /// ([`TAX_FIAT_HOLDER`]) and a specie-holding trader ([`TAX_SPECIE_HOLDER`]) the
    /// same fixed amount at the same due tick. Paired with [`TaxReceivability::FiatOnly`]
    /// (the headline) the fiat-holder remits fiat (`tax_receipts_fiat > 0`) and the
    /// specie-holder defaults; with [`TaxReceivability::SpecieOnly`] (the control) the
    /// specie-holder remits specie (`tax_receipts_specie > 0`, `tax_receipts_fiat == 0`)
    /// and the fiat-holder defaults. Because the levy set is identical, the *only*
    /// difference between the twin is the receivability — so the compelled fiat demand
    /// is isolated to the gate, not the levy or the spatial dynamics.
    fn counter_lever(receivability: TaxReceivability) -> Self {
        Self {
            receivability,
            levies: vec![
                TaxLevy {
                    agent: TAX_FIAT_HOLDER,
                    amount: TAX_LEVY_AMOUNT,
                    due_tick: TAX_DUE_TICK,
                },
                TaxLevy {
                    agent: TAX_SPECIE_HOLDER,
                    amount: TAX_LEVY_AMOUNT,
                    due_tick: TAX_DUE_TICK,
                },
            ],
        }
    }

    /// Layer this overlay's M21 events onto a finance scenario: the
    /// [`EventKind::SetTaxReceivability`] (always emitted, so the counter-lever twin
    /// differs in exactly the receivability byte and the active policy is set
    /// explicitly) and one [`EventKind::LevyTax`] per levy — all at `Tick(0)`, before
    /// any settlement. Reuses econ's unchanged tax events; the sim only authors the
    /// timeline.
    fn apply_to(&self, scenario: &mut MarketScenario) {
        scenario.events.push(Event {
            tick: Tick(0),
            kind: EventKind::SetTaxReceivability(self.receivability),
        });
        for levy in &self.levies {
            scenario.events.push(Event {
                tick: Tick(0),
                kind: EventKind::LevyTax {
                    agent: levy.agent,
                    amount: levy.amount,
                    due_tick: levy.due_tick,
                },
            });
        }
    }

    /// The total levied across this overlay's levies (`taxes_levied` should match it
    /// once every levy event has fired) — the canonical/viewer headline magnitude.
    fn total_levied(&self) -> Gold {
        self.levies
            .iter()
            .fold(Gold::ZERO, |sum, levy| sum.saturating_add(levy.amount))
    }
}

/// Build the econ [`MarketScenario`] a [`CycleConfig`] runs. Both kinds share the
/// lab's `EmergedGoldFiatCreditExpansion` agents, roundabout project line, and
/// issuer (so they are a true falsification twin); they differ only in the **regime
/// ladder + issuer policy** events:
///
/// - [`CycleKind::CreditCycle`] walks the full ladder
///   `SoundGold → FractionalConvertible → SuspendedConvertibility → Fiat` and keeps
///   the lab's `SetIssuerPolicy` (credit on) + `StopIssuerCredit` (the boom→stop),
///   so the cycle fires exactly as the lab proved.
/// - [`CycleKind::SoundMoney`] drops every regime/issuer event, so the society stays
///   `SoundGold` with the issuer's default (disabled) policy — no fiat, no credit.
///
/// Nothing here adds ABCT/regime/shadow logic to econ; it only authors the policy
/// timeline (`SetRegime`/`SetIssuerPolicy`/`StopIssuerCredit`) the sim routes in.
///
/// G8c-2 layers the [`TenderPolicy`]'s non-default `SetXTender` events on top (at
/// `Tick(0)`, before any surface settles). A default policy adds nothing, so a
/// default cycle is byte-identical to the G8c-1 form; the wage knob is the headline
/// (`SpecieOnly` wages refuse the fiat the cycle would otherwise transmit, rendering
/// the same issuance inert).
fn cycle_scenario(kind: CycleKind, tender: TenderPolicy) -> MarketScenario {
    let base = builtin_market_scenario(ScenarioName::EmergedGoldFiatCreditExpansion);
    let mut scenario = match kind {
        CycleKind::CreditCycle => {
            // Walk the regime ladder over time —
            // `SoundGold → FractionalConvertible → SuspendedConvertibility → Fiat` —
            // so the descent is a visible, measured sequence (not an instantaneous
            // jump). The lab's credit machinery is reused verbatim, only re-timed so
            // the issuer is enabled the tick the regime reaches Fiat and stops a fixed
            // window later — preserving the proven boom→stop→bust shape, shifted by the
            // 2-tick ladder descent (`LADDER_SHIFT`).
            const LADDER_SHIFT: u64 = 2;
            let mut events = vec![
                Event {
                    tick: Tick(0),
                    kind: EventKind::SetRegime(Regime::FractionalConvertible),
                },
                Event {
                    tick: Tick(1),
                    kind: EventKind::SetRegime(Regime::SuspendedConvertibility),
                },
            ];
            // Re-time the lab's own events past the ladder descent: the regime reaches
            // Fiat and the issuer's credit policy turns on at `LADDER_SHIFT`, and the
            // stop slides by the same amount (keeping the lab's credit window length).
            for event in &base.events {
                events.push(Event {
                    tick: Tick(event.tick.0 + LADDER_SHIFT),
                    kind: event.kind.clone(),
                });
            }
            MarketScenario {
                periods: base.periods + LADDER_SHIFT,
                events,
                ..base
            }
        }
        CycleKind::SoundMoney => {
            // The control: strip every regime-descent / issuer-credit / fiat-print
            // event, leaving a SoundGold specie economy with the same agents and the
            // same (now never-funded) roundabout project line.
            MarketScenario {
                events: Vec::new(),
                ..base
            }
        }
    };
    // G8c-2: layer the tender policy's non-default `SetXTender` events. A default
    // policy adds nothing (so the G8c-1 cycle/control bytes are unchanged); the wage
    // knob is the headline transmission valve.
    scenario.events.extend(tender.tender_events());
    scenario
}

/// Build the econ [`MarketScenario`] a [`TenderBench`] runs — econ's *unchanged*
/// M11-M16 tender scenarios, with the bench's surface tender swapped in for the lab's
/// baked default. Because each base scenario exercises the chosen surface with the
/// refused/accepted medium already held, swapping the surface tender flips settlement
/// composition: spot/debt benches leave totals fixed, while repayment benches route
/// through econ's normal credit-retirement accounting. Reuses `builtin_market_scenario`
/// + `SetXTender` unchanged; the sim only authors the lever.
fn tender_bench_scenario(bench: TenderBench) -> MarketScenario {
    match bench.surface {
        BenchSurface::Spot => {
            // The M11 base (fiat displacement + a spot tender). Replace its baked spot
            // tender with the configured one so the bench is the config lever, not a
            // fixed scenario.
            let mut scenario = builtin_market_scenario(ScenarioName::EmergedGoldFiatLegalTender);
            scenario
                .events
                .retain(|event| !matches!(event.kind, EventKind::SetPublicSpotTender(_)));
            scenario.events.push(Event {
                tick: Tick(0),
                kind: EventKind::SetPublicSpotTender(bench.tender.spot),
            });
            scenario
        }
        BenchSurface::Debt => {
            // The M12 base (fiat displacement + spot SpecieOnly + a seeded commodity
            // debt + a debt tender). Replace the baked debt tender with the configured
            // one; the spot-SpecieOnly and the SeedCommodityDebt events stay (they set
            // up the surface), so the debtor must hold its fiat for the debt surface.
            let mut scenario =
                builtin_market_scenario(ScenarioName::EmergedGoldFiatDebtLegalTender);
            scenario
                .events
                .retain(|event| !matches!(event.kind, EventKind::SetPublicDebtTender(_)));
            scenario.events.push(Event {
                tick: Tick(0),
                kind: EventKind::SetPublicDebtTender(bench.tender.debt),
            });
            scenario
        }
        BenchSurface::BankRepayment => {
            // The M15 base: a suspended bank borrower holds an unredeemable bank claim
            // and a bank-loan repayment comes due. Replace the baked bank-repayment
            // tender with the configured one; the due-tick event stays to exercise the
            // repayment surface.
            let mut scenario =
                builtin_market_scenario(ScenarioName::EmergedGoldBankLoanRepaymentClaimTender);
            scenario
                .events
                .retain(|event| !matches!(event.kind, EventKind::SetBankRepaymentTender(_)));
            scenario.events.push(Event {
                tick: Tick(4),
                kind: EventKind::SetBankRepaymentTender(bench.tender.bank_repayment),
            });
            scenario
        }
        BenchSurface::IssuerRepayment => {
            // The M16 base: an issuer-credit borrower holds fiat and a fiat-credit
            // repayment comes due. Replace the baked issuer-repayment tender with the
            // configured one; later reset events from the refusal proof are stripped so
            // the bench's active policy remains the configured one for viewer readback.
            let mut scenario =
                builtin_market_scenario(ScenarioName::EmergedGoldIssuerRepaymentFiatTender);
            scenario
                .events
                .retain(|event| !matches!(event.kind, EventKind::SetIssuerRepaymentTender(_)));
            scenario.events.push(Event {
                tick: Tick(13),
                kind: EventKind::SetIssuerRepaymentTender(bench.tender.issuer_repayment),
            });
            scenario
        }
    }
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
    /// S19 cycle role A: holds its cycle tool, consumes Z, and produces X.
    CycleA,
    /// S19 cycle role B: holds its cycle tool, consumes X, and produces Y.
    CycleB,
    /// S19 cycle role C: holds its cycle tool, consumes Y, and produces Z.
    CycleC,
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
            // S19 extends the space; pre-S19 configs never emit 7/8/9.
            Vocation::CycleA => 7,
            Vocation::CycleB => 8,
            Vocation::CycleC => 9,
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
/// S14 — a **capped FORAGE commons**: the parameters of the real
/// [`world::ResourceNode`] the forage path harvests when this mode is on. The S12
/// own-labor path created the FORAGE node as a `0/0/0` marker and credited a fixed
/// [`ChainConfig::forage_yield`] per completed forage task (independent of forager
/// count); the commons replaces that with a depleting node so per-capita yield
/// **falls** as the foraging population grows — the carrying capacity the
/// endogenous population plateau presses on. Routed through the existing GoHarvest
/// haul cycle (harvest → carry → deposit → transfer), so node regen stays the only
/// source and conservation is untouched. `None` on [`ChainConfig::forage_commons`]
/// for every existing config (the S12 fixed-credit path is byte-identical).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForageCommons {
    /// Initial FORAGE stock at generation (clamped to `cap` by `ResourceNode::new`).
    pub stock: u32,
    /// FORAGE units the commons regenerates per **fast** world tick (the only
    /// source of FORAGE on this path), capped at `cap`.
    pub regen: u32,
    /// The commons' stock ceiling — the standing larder a burst of foragers can draw
    /// down before regen alone bounds the per-tick flow.
    pub cap: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum InheritanceRegime {
    #[default]
    Impartible,
    Partible,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WageLaborMode {
    #[default]
    Voluntary,
    FiatWage,
    SubsidisedWage,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShareTenancyMode {
    #[default]
    Voluntary,
    ForcedShare,
    LineageWorker,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BirthStockSavingMode {
    #[default]
    Off,
    Motive,
    SufficiencyControl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BirthStockInjectionRecord {
    pub tick: u64,
    pub household: usize,
    pub birth_succeeded: bool,
}

#[derive(Clone, Debug, PartialEq)]
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
    /// EXPERIMENTAL (subsistence floor): when `true` *and* `bread_is_staple`,
    /// raw **grain** becomes a directly-edible subsistence food ranked just
    /// below bread (see [`KnownGoods::subsistence`]). Colonists prefer bread but
    /// eat raw grain to survive when the chain stalls, so the grain→flour→bread
    /// chain is optional specialization on top of a subsistence base rather than
    /// the sole food source. `false` (every existing config) leaves hunger
    /// satisfied only by the staple, byte-identical.
    pub subsistence_on_grain: bool,
    /// S12 — OWN-LABOR SUBSISTENCE (default `false`, byte-identical when off). When
    /// `true` the settlement retires the **hunger-good mints** (the producer staple
    /// floor in [`Self::producer_subsistence`] and the demographic `food_provision`)
    /// and replaces them with a labor-produced survival **floor**: a hungry,
    /// unprovisioned, spatial non-lineage colonist with spare labor is sent to
    /// **forage** the FORAGE node ([`Task::GoForage`]) instead of harvesting WOOD,
    /// and after it completes that task the settlement credits it
    /// [`Self::forage_yield`] units of the FORAGE subsistence good into its OWN econ
    /// stock — booked `report.produced` (own labor), NOT `report.endowment` (a mint).
    /// FORAGE is wired as
    /// `KnownGoods::subsistence` (read back as hunger relief) and ranked BELOW bread,
    /// so bread stays the superior market good that monetizes SALT. The WOOD/warmth
    /// provision stays an endowment (hunger-only scope). Requires the content to
    /// carry a forage good ([`ContentSet::with_forage`]).
    pub own_labor_subsistence: bool,
    /// S12: the FORAGE units a colonist produces from its own labor after completing
    /// a forage task — the survival-floor knob. The S12 sweep varies it to test
    /// whether a bounded-hunger / money-emergence band exists. `0` (with
    /// [`Self::own_labor_subsistence`] on) retires the mint but produces no floor —
    /// the no-forage control. Inert unless `own_labor_subsistence` is on.
    pub forage_yield: u32,
    /// S12: the hunger at/above which an eligible colonist starts foraging, and (paired
    /// with [`Self::forage_hunger_out`]) the hysteresis exit below which it stops — so a
    /// gatherer does not thrash between foraging and selling WOOD every tick. Consulted
    /// only while `own_labor_subsistence` is on.
    pub forage_hunger_in: u16,
    /// S12: the hunger below which a foraging colonist stops foraging and reverts to its
    /// home role (resuming WOOD gathering / idling). Must sit strictly below
    /// [`Self::forage_hunger_in`]. Consulted only while `own_labor_subsistence` is on.
    pub forage_hunger_out: u16,
    /// S14 — the **capped FORAGE commons** (default `None`, byte-identical when off).
    /// When `Some` *and* [`Self::own_labor_subsistence`] is active, the FORAGE node is
    /// created as a real depleting [`world::ResourceNode`] with these `stock/regen/cap`
    /// and foragers harvest it through the GoHarvest haul cycle (deposit → transfer),
    /// instead of S12's `0/0/0` marker + fixed [`Self::forage_yield`] credit — so
    /// per-capita yield falls with the foraging population (the carrying capacity).
    /// `None` keeps the S12 fixed-credit path exactly, so every existing config stays
    /// byte-identical.
    pub forage_commons: Option<ForageCommons>,
    /// S15 — **own-use cultivation** (default `false`, byte-identical when off). When
    /// `true` *and* the content set carries the no-tool [`econ::project::RecipeId::Cultivate`]
    /// recipe (via [`crate::content::ContentSet::with_cultivate`]) on the active
    /// own-labor/forage-commons path, a colonist *still hungry after foraging* (the
    /// second hysteresis tier below) is steered to GoHarvest the abundant grain node and
    /// cultivate bread by own labor — booked `produced`/`consumed_as_input`, eaten at
    /// home through the consumption readback (never traded). The escape valve that lets
    /// the colony **intensify** past the forage-only plateau. Off keeps the S14 path
    /// exactly, so every existing config is byte-identical.
    pub own_use_cultivation: bool,
    /// S15: the hunger at/above which a (still-hungry) forager escalates to cultivation,
    /// and (paired with [`Self::cultivate_hunger_out`]) the hysteresis exit below which
    /// it reverts to plain foraging. A **second tier** above [`Self::forage_hunger_in`]
    /// (so cultivation is the fallback forage could not relieve), and below the
    /// birth-hunger ceiling (so cultivating pulls hunger back under the preventive
    /// check). Consulted only while `own_use_cultivation` is active.
    pub cultivate_hunger_in: u16,
    /// S15: the hunger below which a cultivating colonist drops back to foraging. Must
    /// sit strictly below [`Self::cultivate_hunger_in`]. Consulted only while
    /// `own_use_cultivation` is active.
    pub cultivate_hunger_out: u16,
    /// S15: the bread a cultivating colonist eats from its OWN freshly-cultivated stock
    /// per econ tick, through the consumption readback seam (so hunger actually falls).
    /// The own-use subsistence draw; the remainder of a tick's cultivated bread stays in
    /// stock to endow children (the broadened birth-food rule). Consulted only while
    /// `own_use_cultivation` is active.
    pub cultivate_consume: u32,
    /// S15: the **patience** — how many CONSECUTIVE econ ticks a colonist's hunger must
    /// stay at/above [`Self::cultivate_hunger_in`] before it escalates to cultivation. A
    /// transient forage-haul hunger spike resets the streak, so cultivation fires only on
    /// SUSTAINED hunger (genuine forage scarcity), never when forage is merely catching
    /// up — this is what makes "no cultivation without scarcity" hold. Consulted only
    /// while `own_use_cultivation` is active.
    pub cultivate_patience: u16,
    /// S16 — **money from PRODUCED bread** (default `false`, byte-identical when off).
    /// When `true` *and* the own-use cultivation path is active, two gated behaviors turn
    /// on. (1) The **buy/sell split**: forage/cultivation eligibility is scoped to
    /// **lineage** spatial household members (`household.is_some() && spatial_active`), so
    /// non-lineage colonists — the seeded SALT-holding consumers S16 needs as the buy
    /// side — no longer self-forage/cultivate (off the flag eligibility is the S13/S14
    /// `household.is_none() || spatial_active`, so consumers would feed themselves and
    /// never buy). (2) The **produced-bread provenance ledger**: a per-agent stock-origin
    /// balance (produced vs minted) is maintained and bread→medium trades are attributed
    /// to produced vs minted (the proof that money emerges against produced, not minted,
    /// bread). The cultivated **surplus** itself needs no new offer code — it stays free
    /// in stock (the S15 own-use consume is reserve-aware) and the existing S9
    /// direct/indirect barter offers it for the cultivator's normal unsatisfied wants (no
    /// special medium want — Base Fact 7). Off keeps the S15 path exactly, so every
    /// existing config and its goldens are byte-identical.
    pub cultivation_sells_surplus: bool,
    /// S18 — **money from a produced MULTI-GOOD economy** (default `false`, byte-identical
    /// when off). When `true` *and* the money-from-produced-bread path is active, the
    /// non-lineage `Gatherer`s (the woodcutter role) are routed to the **WOOD node** at
    /// generation instead of round-robin over `config.nodes` — so with both a grain node
    /// (the cultivators' input) and a WOOD node present, grain never draws the woodcutters
    /// off into a third surplus and WOOD becomes a clean, market-supplied second produced
    /// good. The flag also turns on the runtime-only multi-good instrumentation (the WOOD
    /// source bound + the pending-indirect-SALT round-trip ledger), which is diagnostic and
    /// NOT digested. Off keeps the S16 path exactly, so every existing config and its
    /// goldens are byte-identical. Composes strictly on `cultivation_sells_surplus`.
    pub multigood_money: bool,
    /// S21f — **household production-for-barter activation seam** (default `false`,
    /// byte-identical when off). The S15 own-use cultivation path requires the
    /// own-labor/forage substrate (`own_labor_subsistence && content.forage().is_some()`)
    /// in addition to `own_use_cultivation` + the `Cultivate` recipe, so it cannot run on
    /// the open-survival money base (which interns NO forage good). When this flag is
    /// `true` *and* `own_use_cultivation` + the `Cultivate` recipe are present, cultivation
    /// becomes an **alternative substrate to forage**: the cultivation steering phase
    /// ([`Settlement::run_own_labor_subsistence`]) runs with NO forage good interned (so it
    /// does not pollute the value scale with a phantom `known.subsistence`), eligible
    /// **lineage** spatial members escalate to cultivation directly from sustained hunger
    /// (the `cultivate_hunger_in`/`cultivate_patience` hysteresis, no forage tier beneath
    /// it), and the own-use cultivation phase + the multi-good money routing engage exactly
    /// as on the forage path. The `Cultivate` recipe stays POST-market, so the cultivated
    /// surplus sells **cross-tick** (cultivate tick `t` → surplus persists in stock →
    /// barter tick `t+1` posts the sell lane). Canonicalized **ON-only** (it changes
    /// production), mirroring the S16/S18/S21d gates: a flag-off config keeps its exact
    /// prior byte layout. Composes with `own_use_cultivation` (which it does not replace).
    pub household_barter_cultivation: bool,
    /// S22a — **endogenous cultivation entry** (default `false`, byte-identical when off).
    /// Through S21 the food-producing class is pinned: the S16 buy/sell split restricts
    /// household-barter cultivation eligibility to the spatial **lineage**
    /// (`household.is_some() && spatial_active`), so the non-lineage roles (the SALT-rich
    /// buyers + the woodcutters) never cultivate regardless of hunger. When this flag is
    /// `true` *and* the money-from-produced-bread path is active
    /// ([`Self::cultivation_sells_surplus`]), the cultivation eligibility set is **relaxed**:
    /// ANY spatial colonist (lineage or not) becomes eligible to enter cultivation through the
    /// SAME existing S15/S21f pressure/patience hysteresis — the food-producing class can then
    /// self-form from sustained hunger rather than assigned identity. The relaxation is ONLY
    /// the household/spatial membership predicate; the `Consumer|Gatherer|Unassigned` vocation
    /// filter is **preserved** (an active Miller/Baker is still excluded), no `Vocation` is
    /// mutated (a steering-flag-only change via the existing `cultivating` flag), and the
    /// Miller/Baker money gate is untouched. The opportunity cost stays structural (a
    /// cultivating tick cannot also gather WOOD / use the world-task slot — market buying in the
    /// econ step is unaffected). Canonicalized **ON-only** (it changes
    /// who produces), mirroring the S16/S18/S21d/h gates: a flag-off config keeps its exact
    /// prior byte layout. Composes on `cultivation_sells_surplus` (the path whose eligibility
    /// branch it overrides).
    pub endogenous_cultivation_entry: bool,
    /// S22b — **bounded cultivation skill** (default `false`, byte-identical when off). When
    /// `true` *and* the endogenous-cultivation-entry path is active
    /// ([`Self::endogenous_cultivation_entry`]), each colonist carries a bounded, earned-not-
    /// inherited [`Colonist::cultivation_skill`] scalar that ACCUMULATES on a tick of realized
    /// cultivation output (grain actually harvested AND converted to bread) and DECAYS on any
    /// tick without it. Skill raises ONLY the per-trip **grain-haul capacity** of a cultivating
    /// agent's grain `GoHarvest` (`haul = carry_cap + carry_cap·(ceiling−1)·skill/skill_cap`,
    /// capped at `skill_haul_ceiling × carry_cap`), routed through the gated
    /// [`world::Task::GoHarvestWithRoom`] per-trip room override — a faster draw on the conserved
    /// grain node, NEVER a higher bread-per-grain ratio, so conservation is untouched by
    /// construction (the 1:1 recipe and all produced/consumed_as_input accounting are unchanged).
    /// The lever tests whether mild accumulated productivity turns S22a's fluid self-provisioning
    /// into a stable occupational split. Canonicalized **ON-only** (digest tag 8 + the skill
    /// parameters + the per-colonist skill state), mirroring the S16/S18/S21/S22a gates: a
    /// flag-off config keeps its exact prior byte layout. Composes on `endogenous_cultivation_entry`.
    pub cultivation_skill: bool,
    /// S22c — **profit-driven cultivation retention** (default `false`, byte-identical when off).
    /// When `true` *and* the endogenous-cultivation-entry path is active
    /// ([`Self::endogenous_cultivation_entry`]; orthogonal to `cultivation_skill`, works with it on
    /// or off), the cultivation *exit* branch is profit-modulated: a currently-cultivating agent
    /// remains cultivating past the normal hunger exit when, **only after money exists**
    /// (`current_money_good() == Some(SALT)` — the hard anti-circularity gate), its realized
    /// cultivation-sale return over a rolling [`RETURN_WINDOW`] clears both a small material floor
    /// ([`RETENTION_MATERIAL_FLOOR`]) and its outside option (its own realized non-cultivation sale
    /// rate, or the colony reference non-cultivating-seller rate, plus [`RETENTION_MARGIN_BPS`]).
    /// Entry stays hunger/pressure-gated (S22a/b unchanged) — only the exit is profit-modulated.
    /// The per-agent rolling-return accumulators STEER the next `cultivating` flag, so they are
    /// FUTURE-BEHAVIOUR state serialized into the digest **ON-only** (the `cultivation_skill`
    /// discipline), NOT runtime-only; the counterfactual-flip count / proceeds distributions are
    /// runtime-only diagnostics. Canonicalized **ON-only** (digest tag 9), mirroring the
    /// S16/S18/S21/S22a/b gates: a flag-off config keeps its exact prior byte layout. Composes on
    /// `endogenous_cultivation_entry`.
    pub profit_driven_retention: bool,
    /// S22c: the rolling-return window length in econ ticks (default [`RETURN_WINDOW`]). The
    /// per-agent realized cultivation-sale / non-cultivation-sale proceeds are accumulated over the
    /// trailing `return_window` ticks. Consulted only while `profit_driven_retention` is active;
    /// swept as the window sensitivity axis.
    pub return_window: u64,
    /// S22c: the basis-point margin the cultivation per-sale-tick rate must clear the outside rate
    /// by (default [`RETENTION_MARGIN_BPS`] = 0 ⇒ cultivating must earn *at least* the outside
    /// option). Consulted only while `profit_driven_retention` is active; swept as sensitivity.
    pub retention_margin_bps: u64,
    /// S22c: the floor on windowed cultivation proceeds below which the stay is inert (default
    /// [`RETENTION_MATERIAL_FLOOR`]) — so a single dust sale cannot lock an agent in. `0` is the
    /// permissive control (any realized cultivation sale qualifies). Consulted only while
    /// `profit_driven_retention` is active.
    pub retention_material_floor: u64,
    /// S22b: the per-tick skill GAIN credited on a realized-cultivation-output tick (default
    /// [`SKILL_GAIN`]). Consulted only while `cultivation_skill` is active.
    pub skill_gain: u16,
    /// S22b: the per-tick skill DECAY applied on a tick without realized cultivation output
    /// (default [`SKILL_DECAY`]; `< skill_gain`). `0` is the no-decay control (skill ratchets
    /// monotonically). Consulted only while `cultivation_skill` is active.
    pub skill_decay: u16,
    /// S22b: the skill ceiling (default [`SKILL_CAP`]). Skill saturates here; `0` makes the
    /// productivity effect a no-op (the cap-zero control). Consulted only while
    /// `cultivation_skill` is active.
    pub skill_cap: u16,
    /// S22b: the full-skill grain-haul multiplier / harvest-room ceiling (default
    /// [`SKILL_HAUL_CEILING`] = 2 ⇒ ≤2× `carry_cap` at full skill). `1` is the no-op (cap-zero)
    /// control; a larger value (e.g. `4`) is the exaggerated-cap SENSITIVITY probe. Consulted
    /// only while `cultivation_skill` is active.
    pub skill_haul_ceiling: u32,
    /// S22d — **durable role-specific cultivation capital** (default `false`, byte-identical when
    /// off). When `true` *and* the profit-driven-retention path is active
    /// ([`Self::profit_driven_retention`], which itself requires the S22a endogenous-entry path),
    /// a sustained-producing cultivator may invest a SUNK cost — [`Self::tool_build_wood`] WOOD +
    /// [`Self::tool_build_labor`] labor (the existing producer-capital build knobs, free because
    /// the producer chain is off on every cultivation scenario) — into a durable, OWNED,
    /// role-specific cultivation tool (the [`content::CULTIVATION_TOOL`] good, built by a
    /// dedicated [`econ::project::ProjectTemplateId::BuildCultivationTool`] project in a SEPARATE
    /// gated [`Settlement::run_cultivation_capital_formation`] phase — never reusing the
    /// money-gated mill/oven machinery, so it can build PRE-money). The tool gates no recipe (the
    /// no-tool `Cultivate` recipe is unchanged); it raises ONLY its owner's grain-HAUL ceiling
    /// ([`Self::cultivation_tool_haul_ceiling`] × `carry_cap`) **while it cultivates** (asset
    /// specificity), a conservation-safe faster draw on the conserved grain node (never the
    /// bread-per-grain ratio). The owner's higher realized cultivation return then flows through
    /// the UNMODIFIED S22c profit-stay exit — no stay flag is added, no exit branch edited — so
    /// any stickiness arises from durable OWNERSHIP, not raw productivity. Canonicalized
    /// **ON-only** (digest tag 10 + the build params + the in-flight builds + the per-colonist
    /// cultivation tenure), mirroring the S16/S18/S21/S22a/b/c gates: a flag-off config keeps its
    /// exact prior byte layout. Composes on `profit_driven_retention`.
    pub durable_cultivation_tool: bool,
    /// S22d: the realized-cultivation-output tenure (consecutive output ticks) a cultivator must
    /// sustain before it invests in a tool (default [`TOOL_BUILD_PATIENCE`]). Credited only on a
    /// tick of realized cultivation output, reset otherwise — distinct from `cultivate_pressure`
    /// (a hunger-entry streak). Consulted only while `durable_cultivation_tool` is active.
    pub tool_build_patience: u16,
    /// S22d: the OWNER's per-trip grain-haul ceiling (×`carry_cap`) while cultivating (default
    /// [`CULTIVATION_TOOL_HAUL_CEILING`]). `> 1` ⇒ a tool-owner strictly out-hauls a non-owner
    /// (whose ceiling is `1×carry_cap`); `1` is the no-boost control. Consulted only while
    /// `durable_cultivation_tool` is active.
    pub cultivation_tool_haul_ceiling: u32,
    /// S22d — the **non-durable / rented-tool CONTROL** (default `false`). When `true` (and the
    /// durable-cultivation-capital path is active), a built plow is CONSUMED (booked
    /// `consumed_as_input`, a real sink) after the one cultivation opportunity it boosts — so it
    /// leaves NO persistent stock and the agent must re-build (re-pay the sunk WOOD) each time to
    /// get the boost again. Same per-use owner-only productivity as the durable tool, but NO
    /// durable ownership — it isolates *durability* specifically: if it still produces stickiness,
    /// the stickiness was not from persistence. Consulted only while `durable_cultivation_tool` is
    /// active. The durable headline keeps it `false`.
    pub cultivation_tool_non_durable: bool,
    /// S22e — **endowed + inherited cultivation capital** gate (default `false`,
    /// byte-identical when off). When `true` AND the durable-cultivation-capital path is active
    /// (it composes strictly on [`Self::durable_cultivation_tool`]), a MINORITY of lineage
    /// households are seeded with one durable cultivation tool (the [`content::CULTIVATION_TOOL`]
    /// plow) at generation — a conservation-safe INITIAL endowment, no earning required (counted by
    /// [`Settlement::endowed_cultivation_tools_total`], included in the initial whole-system
    /// conservation baseline). The genuinely new estate primitive is the plow-routing SWITCH
    /// [`Self::cultivation_tool_inheritance`]: plows already inherit to the household heir via
    /// `settle_estate_to_heirs`, so the lever toggles whether they keep that heir path (true) or
    /// are FORCED to the commons (false, the falsifying control). Everything else reuses S22d
    /// unchanged (the owner-exclusive haul boost; the unmodified S22c profit-stay as the only
    /// retention). Canonicalized **ON-only** when it can steer behavior (digest tag 11 +
    /// [`Self::endowed_tool_count`] + the inheritance switch + the granted household ids), mirroring
    /// the S16/S18/S21/S22a–d gates: a flag-off config keeps its exact prior byte layout, and the
    /// zero-endowment/inheritance-on control stays identical to the expanded S22d base. Composes on
    /// `durable_cultivation_tool`.
    pub endowed_cultivation_capital: bool,
    /// S22e: how many lineage households are seeded with a plow at generation — a MINORITY (the
    /// shipped headline keeps owner-share a minority; the sweep raises it to universal to show the
    /// `UniversalOwnership` boundary). The endowed households are selected by a deterministic hash
    /// of `(seed, household_id)` over the eligible lineage set, sorted by hash (NOT lowest-ids).
    /// Consulted only while `endowed_cultivation_capital` is active. `0` ⇒ no endowment (the
    /// no-endowment control: tools must be earned, reducing to S22d).
    pub endowed_tool_count: u16,
    /// S22e: the plow estate-routing switch (default `true` under the gate). `true` ⇒ plows follow
    /// the existing heir path (lineage heir; commons fallback); `false` ⇒ plows are FORCED to the
    /// commons even when the rest of the estate goes to the heir (the no-inheritance control that
    /// isolates whether inheritance is load-bearing). A pure conserved transfer either way, never a
    /// mint. Consulted only while `endowed_cultivation_capital` is active.
    pub cultivation_tool_inheritance: bool,
    /// S22f — **voluntary fixed-term cultivation commitment** gate (default `false`, byte-identical
    /// when off). When `true` AND the S22c profit-driven-retention path is active (it composes
    /// strictly on [`Self::profit_driven_retention`]; the entry signal IS the S22c realized return,
    /// inert pre-money via the same `current_money_good() == Some(SALT)` gate), an eligible
    /// **uncommitted** agent whose own realized cultivation-return signal clears
    /// [`Self::commitment_entry_floor`] vs its outside option may **voluntarily opt in** to a
    /// cultivator commitment of [`Self::commitment_term`] econ ticks. While the term runs the agent
    /// CULTIVATES — the normal hunger/profit cultivation *exit* cannot turn it off (the ONE new exit
    /// behavior in the arc) — and the term decrements once per econ tick; at expiry the agent
    /// returns to the normal S22a/S22c fluid logic and re-decides from FRESH realized returns,
    /// re-committing (a tracked renewal) only if the signal still clears. Commitment overrides the
    /// EXIT, not vocation eligibility: a committed agent that dies or becomes an active specialized
    /// producer (leaves the S22a-eligible set) has its commitment cleared deterministically (no
    /// orphaned binding on a non-cultivator). Pure steering state (no goods — cultivation effects
    /// flow through the existing grain/bread accounting, the conservation guard). Canonicalized
    /// **ON-only** (digest tag 12 + [`Self::commitment_term`] + [`Self::commitment_entry_floor`] +
    /// [`Self::commitment_fiat_pin`] + the per-colonist commitment state), mirroring the
    /// S16/S18/S21/S22a–e gates: a flag-off config keeps its exact prior byte layout. Composes on
    /// `profit_driven_retention`.
    pub voluntary_cultivation_commitment: bool,
    /// S22f: the binding length in econ ticks (default [`COMMITMENT_TERM_DEFAULT`]). For the
    /// headline/success bar it MUST satisfy `commitment_term < ceil(PERSIST_FRACTION × FINAL_WINDOW)`
    /// so a single term cannot span the persistence window (persistence must come from renewals). The
    /// `nonbinding_term` control sets it to `1` (a one-tick "commitment" reproduces S22c marginal
    /// retention); the robustness sweep drives `{12, 24, 48, 96}`. Consulted only while
    /// `voluntary_cultivation_commitment` is active.
    pub commitment_term: u16,
    /// S22f: the floor on a candidate's windowed realized cultivation proceeds below which its entry
    /// signal does not clear (default [`COMMITMENT_ENTRY_FLOOR_DEFAULT`]; reuses the S22c
    /// material-floor logic). The `unprofitable_offer` control sets it to `u64::MAX` so NO agent's
    /// signal ever clears (zero uptake → `CommitmentUnchosen`, proving uptake is voluntary/
    /// signal-gated). Consulted only while `voluntary_cultivation_commitment` is active.
    pub commitment_entry_floor: u64,
    /// S22f — the **fiat-pin CONTROL** count (default `0`). When `> 0` (and the commitment gate is
    /// active), the voluntary signal-gated entry is BYPASSED and the first `commitment_fiat_pin`
    /// eligible agents (deterministic slot order) are FORCE-committed from the first post-money tick
    /// and re-pinned on expiry — a forced re-pin of a producer class, not a voluntary institution. It
    /// must classify `RePinScaffold` and never count as headline success: the forced commits record
    /// NO signal-gated uptake, leave NO below-floor non-committer set, and earn NO fresh-signal
    /// renewals, so the voluntary headline (which has all three) is distinguishable even when both
    /// show low churn. `0` (the headline + every other config) leaves entry purely voluntary.
    pub commitment_fiat_pin: u16,
    /// S24a — endogenous spread of the S22f commitment norm (default `false`, byte-identical when
    /// off). When `true` AND the S22f commitment path is active, commitment ENTRY is available only
    /// to agents carrying `adopts_commitment_norm`; a deterministic minority starts with that bit,
    /// and non-adopters can copy it from locally observed agents with better generic outcomes.
    pub commitment_norm_spread: bool,
    /// S24b — abandonable commitment-norm adoption (default `false`, byte-identical when off).
    /// When `true` AND S24a is active, the imitation step becomes bidirectional: every agent copies
    /// the better-off observed neighbour's norm bit, so adoption can be dropped as well as gained.
    pub abandonable_norm: bool,
    /// S24c — group-payoff imitation (default `false`, byte-identical when off). When `true` AND
    /// S24b is active, the abandonable imitation step scores local groups by generic welfare and
    /// copies toward the adopter-share gradient of the welfare-selected group.
    pub group_payoff_imitation: bool,
    /// S25a — fixed deme-level commitment-norm culture (default `None`, byte-identical when off).
    /// When set, S22f commitment entry is gated by `adopts_commitment_norm`, founders and newborns
    /// receive that bit from a deterministic hash against this prevalence, and S24 imitation/spread
    /// machinery remains off. The option itself is the inherited deme culture selected by the
    /// multi-deme harness; no per-agent imitation rule reads it.
    pub fixed_commitment_norm_prevalence: Option<f64>,
    pub commitment_seed_share_bps: u16,
    pub imitation_period: u64,
    pub imitation_window: u64,
    pub imitation_margin_bps: u64,
    pub imitation_radius: u16,
    pub imitation_max_models: u16,
    pub food_window_target: u64,
    pub no_imitation: bool,
    pub random_imitation: bool,
    pub salt_in_score: bool,
    /// S23a — **private land tenure** (default `false`, byte-identical when off). When `true` AND
    /// the S22a endogenous-cultivation-entry path is active, grain nodes become a predeclared finite
    /// set of heterogeneous land plots. A plot is claimed by the first successful homesteading
    /// harvest, harvests are owner-exclusive under [`Self::harvest_gate`], and an owner loses the
    /// plot after [`Self::land_idle_limit`] consecutive fast ticks with no engagement. The grain
    /// stock itself remains conserved; ownership is metadata over resource nodes.
    pub private_land_tenure: bool,
    /// S23a: consecutive unengaged fast ticks before an owned plot reverts to unowned. Consulted
    /// only while private land tenure is active and [`Self::forfeit_on_idle`] is true.
    pub land_idle_limit: u16,
    /// S23a: whether harvest access is owner-exclusive. `false` is the non-excludable-deed control:
    /// ownership can be recorded, but non-owners are not blocked or rerouted.
    pub harvest_gate: bool,
    /// S23a: whether idle ownership reverts to unowned. `false` is the no-forfeit control.
    pub forfeit_on_idle: bool,
    /// S23a: whether a reverted plot is reserved for its prior owner at no spatial cost. `true` is
    /// the free-reclaim control.
    pub reclaim_reserved_for_prior_owner: bool,
    /// S23a: count of near, high-yield good plots in the predeclared layout.
    pub land_good_plots: u16,
    /// S23a: count of farther marginal plots in the predeclared layout.
    pub land_marginal_plots: u16,
    /// S23a: per-tick regeneration for marginal plots. The viability floor is intentionally below
    /// the default so the robustness sweep can expose the hard-barrier boundary.
    pub land_marginal_regen: u32,
    /// S23c — secure private land tenure (default `false`, byte-identical when off). When
    /// active on the S22a private-land substrate, title does not lapse from idle use, harvest is
    /// owner-only, and ownership turns over through death inheritance.
    pub secure_land_tenure: bool,
    /// S23c: whether secure-title inheritance keeps each plot atomic (`Impartible`) or splits
    /// fractional beneficial interests on the same resource node (`Partible`).
    pub inheritance_regime: InheritanceRegime,
    /// S23b — **post-money alienable land market** (default `false`, byte-identical when off).
    /// When `true` AND private land tenure is active, idle forfeiture is disabled from tick 0 and
    /// the market institution activates only after SALT is the money good. Owned plots can be
    /// listed, bought, sold, and charged a conserved carrying cost; title remains metadata over the
    /// finite plot registry.
    pub land_market: bool,
    /// S23d — mortal-landowner demography base (default `false`, byte-identical when off).
    /// When active on the secure-land substrate, only mortal reproducing lineage household
    /// actors may claim homesteaded plots or receive secure-title fallback inheritance.
    pub mortal_landowner_demography: bool,
    /// S23e — finite rival subsistence commons (default `false`, byte-identical when off).
    /// When active on the S23d mortal-landowner base, the S21h emergency survival step draws
    /// residual non-lineage hunger from a finite regenerating, non-excludable rival pool instead
    /// of minting unlimited own-labor bread. The pool is distinct from the death-estate commons:
    /// it has its own stock/cap/regen telemetry and acquisition channel, and only the active
    /// ON path is canonicalized.
    pub rival_subsistence_commons: bool,
    /// S23e: scarcity scalar in basis points. The shipped sweep uses 12_500 (abundant), 5_000
    /// (marginal), and 2_500 (scarce). Runtime regen is `round(phi * D0)`, where D0 is the
    /// measured S23d flag-off emergency throughput pinned by
    /// [`RIVAL_COMMONS_BASELINE_EMERGENCY_DRAW`] / [`RIVAL_COMMONS_BASELINE_FINAL_WINDOW_TICKS`].
    pub rival_subsistence_commons_phi_bps: u32,
    /// C1 — wage labor on the S23e rival-commons mortal-landowner base. Default-off and
    /// inert unless it composes on S23e and SALT has promoted.
    pub wage_labor: bool,
    /// C1 acceptance controls. Ignored while `wage_labor` is off.
    pub wage_labor_mode: WageLaborMode,
    /// C1R — voluntary output-share tenancy over the S23e rival-commons mortal-landowner
    /// base. Default-off and inert unless it composes on S23e; unlike C1 wages it has no
    /// money/promotion gate.
    pub share_tenancy: bool,
    /// C1R acceptance/scaffold mode. Ignored while `share_tenancy` is off.
    pub share_tenancy_mode: ShareTenancyMode,
    /// P1.5 — forward-provisioning worker gate. Orthogonal to the C1R acceptance mode and inert
    /// unless share tenancy itself composes on the S23e substrate.
    pub share_forward_provisioning: bool,
    /// P1.6 — owner-death succession for live share contracts. Inert unless share tenancy
    /// itself composes on the S23e substrate.
    pub share_contract_succession: bool,
    /// C1N — fixed in-kind bread wage on C1R's share-tenancy substrate. The owner advances
    /// bread up front out of self-produced stock and receives 100% of the contract product.
    /// Inert unless share tenancy is active.
    pub in_kind_wage: bool,
    /// C3R.a — mortal production-chain producers, with no role/capital succession.
    /// Active only when paired with demography and a chain. The flag makes the seeded
    /// latent mill/bake producers lifespan-only mortals and closes producer formation to
    /// mortal agents; it does not add households, inheritance, or new goods flows.
    pub mortal_chain_producers: bool,
    /// C3R.b — bounded producer households for the mortal chain producers. Active only
    /// on top of C3R.a (`mortal_chain_producers`) with demography and a chain; when
    /// active, the six seeded producer subjects are assigned to six dedicated
    /// reproducing households and the existing estate/S7 seams can carry tools to heirs.
    pub mortal_producer_inheritance: bool,
    /// C3R.b matched control switch. Defaults to `true` so the heritable base uses the
    /// existing estate heir route; tests flip it to `false` to force mill/oven tools to
    /// commons while leaving the producer households unchanged.
    pub mortal_producer_tool_inheritance: bool,
    /// C3R.b per-producer-house birth cap. Serialized only under tag 28 and applied
    /// only to the dedicated producer households, never to the lineage cap.
    pub producer_house_cap: u8,
    /// C3R.c — earned provisioning for mortal producer households. Active only on
    /// top of C3R.b inheritance. The producer-house food mints can be retired by
    /// config, and this flag lets active producers transfer conserved GOLD from
    /// their tracked bread-sale proceeds to hungry same-household members before
    /// the market. The member then bids through the normal order machinery.
    pub earned_provisioning: bool,
    /// C3R.c stock-provisioning control. Active only on top of C3R.b inheritance:
    /// active producers feed hungry same-household members from their own bread
    /// stock by conserved stock transfer, with no GOLD transfer and no market bid.
    /// This is a control cell, not a tuning parameter.
    pub producer_stock_provisioning_control: bool,
    /// C3R.d — future-bread saving motive for producer households. The bool is
    /// serialized explicitly under tag 31; the mode keeps the headline and the
    /// conserved sufficiency control mutually exclusive.
    pub birth_stock_saving: bool,
    pub birth_stock_saving_mode: BirthStockSavingMode,
    /// C3R.e-obs (impl-66) — the PURE-OBSERVATION allocation-contest instrumentation.
    /// Gated `saving_allocation_obs && birth_stock_saving_active()` (the motive must be on
    /// to observe its bids). When active it enables the econ allocation trace and runs the
    /// per-tick §2 loss-reason join; it changes NO behavior, so its ONLY digest footprint
    /// is the ON-only tag-32 emission (`[32, 1]`). Off (and thus digest-absent) for every
    /// existing config.
    pub saving_allocation_obs: bool,
    /// DH.b-obs (impl-70) — the PURE-OBSERVATION birth-gate-stock diagnostic. Configured by
    /// `demography.is_some() && birth_gate_obs` (a `Society`-owned staple-stock event tape plus
    /// the `Settlement` join); it changes NO behavior, so its ONLY digest footprint is the
    /// configured-only tag-35 emission (`[35, 1]`), UNAFFECTED by the closure force-disable (the
    /// tape WRITES additionally gate on `closure_active()`). Off (and thus digest-absent) for
    /// every existing config.
    pub birth_gate_obs: bool,
    /// C1R worker share in basis points. Pinned/swept, never searched.
    pub share_bps: u16,
    /// C1R fixed contract term in econ ticks. Pinned/swept, never searched.
    pub share_term: u16,
    /// S23b: SALT carrying cost per held plot every [`LAND_CARRYING_PERIOD`] econ ticks. Paid into
    /// the settlement land-fee sink, never redistributed in this slice.
    pub land_carrying_cost: u64,
    /// S23b: capitalization slope from realized/quality-prior rent into fundamental land price.
    /// `0` is the free-rebuy control.
    pub land_price_cap_factor: u64,
    /// S21d.0 — **retire the food mints** (the open-survival probe; default `false`,
    /// byte-identical when off). When `true`, the two staple-food mint sites are skipped
    /// **independent of `own_labor_subsistence`/forage**: the demographic `food_provision`
    /// hearth ([`Settlement::deliver_demography_provisions`]) and the producer staple floor
    /// ([`Settlement::run_producer_subsistence`]) no longer mint the hunger staple, so the
    /// food-mint endowment term is zero and every agent — producers included — must acquire
    /// its food on the MARKET (or by its own production). Unlike the S12 forage hack
    /// (`own_labor_subsistence=true + with_forage() + forage_yield=0`), this interns NO
    /// FORAGE good, so it pollutes no scale/spoilage/market trace with a phantom subsistence
    /// good. WOOD/warmth provision is unaffected (warmth is out of scope for this probe).
    /// Canonicalized **ON-only** (it changes the recurring staple-mint behaviour), mirroring
    /// the S20/S21 menger gates: a flag-off config keeps its exact prior byte layout.
    pub retire_food_mints: bool,
    /// S21d.1 — **acquisition-channel ledger** gate (runtime-only diagnostic; default
    /// `false`). When `true` *and* a chain carries a bread good, the sim maintains a
    /// per-agent FIFO ledger classifying each tracked-food (bread) unit by acquisition
    /// channel ([`AcquisitionLedger`]) — `bought`/`seeded-minted`/`self-produced`/`foraged`
    /// — so the probe can prove survivors eat MARKET-acquired food after warm-up. Pure
    /// read-only accounting: it mutates only its own ledger, steers no future tick, and is
    /// deliberately EXCLUDED from `canonical_bytes` (like `starvation_deaths_total`), so it
    /// shifts no digest and every existing golden is byte-identical whether on or off.
    pub acquisition_ledger: bool,
    /// EXPERIMENTAL (capital-advance probe): when `true` *and* money has emerged,
    /// each econ tick a conserved working-capital advance moves real money from
    /// the richest saver to any cashless active chain producer (Miller/Baker), so
    /// it can buy inputs ahead of selling output. The causal test of the
    /// producer-working-capital thesis — a funded transfer of real money (NOT
    /// fiduciary credit), with no repayment modeled yet (the first test is
    /// causal). `false` (every existing config) skips the phase, byte-identical.
    pub capital_advance: bool,
    /// EXPERIMENTAL (inventory carrying cost): per-econ-tick **spoilage** rate, in
    /// basis points, applied to every colonist's holdings of the perishable chain
    /// foods (grain, flour, bread). `0` (every existing config) means no spoilage,
    /// byte-identical. A positive rate means a satiated agent cannot hoard its way
    /// out of the market permanently — its food stock decays, hunger returns, so
    /// it must keep acquiring (buying or producing), and raw grain must be sold
    /// before it rots. Codex's primary fix for the distribution-seizure halt: the
    /// counter-pressure that forces a satiated hoard back into circulation.
    pub perishable_decay_bps: u16,
    /// EXPERIMENTAL (in-kind subsistence advance): when `true` *and* money has
    /// emerged, each econ tick (before the market) a hungry active chain producer
    /// (Miller/Baker) is advanced **staple food in kind** from the richest
    /// food-holder. The live order-book trace proved a funded-but-hungry producer
    /// posts no input bid because its money is reserved for its own unmet bread
    /// want; feeding it provisions that want, frees its money, and it bids for
    /// grain. Conserved (food moves holder→producer, then is eaten). It also
    /// recirculates the satiated consumers' idle bread hoard to the producers.
    /// `false` (every existing config) skips the phase, byte-identical.
    pub subsistence_advance: bool,
    /// EXPERIMENTAL (in-kind INPUT advance): when `true` *and* money has emerged,
    /// each econ tick (before production) a capitalist (the richest money-holder)
    /// **buys each active producer's recipe input in kind** from a seller — grain
    /// for a miller, flour for a baker — paying the seller real money and placing
    /// the input directly in the producer's hands. This bypasses the value-scale
    /// gate entirely: production no longer depends on a producer out-ranking its
    /// own consumption/savings to *bid* for inputs (Experiment 9–10's residual
    /// blocker). It also recirculates the capitalist's idle money to the input
    /// sellers (gatherers). Conserved (money cap→seller, input seller→producer).
    /// `false` (every existing config) skips the phase, byte-identical.
    pub input_advance: bool,
    /// EXPERIMENTAL (recurring owner-operator motive): when `true`, a latent
    /// producer also adopts/keeps its role whenever the recipe is simply
    /// **profitable** at realized prices (expected revenue > input + operating
    /// cost), not only when it newly provisions a one-off future-money savings
    /// want. The faithful self-employment fix for the satiation wall: a real
    /// artisan keeps producing because consumption **recurs** (it must keep
    /// earning to keep eating), so it does not permanently retire the moment its
    /// savings ladder fills. No firms, no value-scale surgery — role adoption
    /// keyed to ongoing profitability. `false` (every existing config) keeps the
    /// savings-want-only rule, byte-identical.
    pub recurring_motive: bool,
    /// EXPERIMENTAL (project-aware input bids — the endogenous fix): when `true`
    /// *and* money has emerged, each econ tick (before production) an active
    /// producer buys ONE unit of its recipe input through a real market trade,
    /// using its OWN money, at a price **imputed** from the output it will
    /// produce (Menger: a higher-order good's value derives from the final good —
    /// the producer's reservation is the output's realized value minus the
    /// operating cost), matched against the cheapest *willing* seller (a gatherer
    /// holding grain / a miller holding flour, at that seller's own ask). Unlike
    /// the curated `input_advance` (a planner *places* inputs), here the producer
    /// pays for its own input from its own purse — the input acquired by market
    /// trade, not handed over. Conserved; voluntary on both sides. `false` (every
    /// existing config) skips it, byte-identical.
    pub project_input_bids: bool,
    /// LOCAL producer subsistence floor (S5 — the household/subsistence base): the
    /// units of staple food each chain producer (Miller/Baker, and the latent pool
    /// that will adopt) draws each tick from its OWN renewable household hearth —
    /// minted fresh, exactly like the demography [`deliver_demography_provisions`]
    /// provision, and NOT taken from any other agent. This is the **local**
    /// household allocation the endogenous milestone is allowed to keep (a
    /// producer's kitchen garden / its lineage's hearth), as distinct from the
    /// **global** `subsistence_advance` redistribution (richest food-holder → every
    /// producer) it turns OFF. It keeps a producer fed so its money frees to bid
    /// for recipe inputs rather than reserve for its own hunger — the subsistence
    /// base the specialization sits on top of. Conserved: a source recorded in
    /// `report.endowment`, eaten in the consume phase like any provision. `0`
    /// (every existing config) mints nothing, so the run is byte-identical.
    pub producer_subsistence: u32,
    /// PRODUCTIVE RE-ENTRY (S6 — provisioning at scale): when `true` *and* raw
    /// grain is edible (`subsistence_on_grain`), a gated `econ_tick` phase
    /// (`run_productive_reentry`, before the market) lets a hungry **spatial
    /// non-lineage** colonist adopt edible-grain gathering on its own value scale:
    /// an idle [`Consumer`](Vocation::Consumer) (no node, produces nothing) and a
    /// [`Gatherer`](Vocation::Gatherer) mis-allocated to a non-edible (WOOD) node
    /// each become a grain [`Gatherer`](Vocation::Gatherer) once hunger reaches
    /// [`Self::reentry_hunger_in`], so the permanently-stranded underclass can feed
    /// itself instead of starving forever (`hunger_critical = need_max + 1` keeps it
    /// alive); conversely a fed re-entrant reverts to its home role once its hunger
    /// falls below [`Self::reentry_hunger_out`] (the hysteresis). It never touches
    /// lineage members (hearth-fed) or the latent/seeded **tooled** chain producers
    /// (the S7 path); it mints nothing — gathering is the existing conserved
    /// node-regen source. `false` (every existing config) skips the phase,
    /// byte-identical.
    pub productive_reentry: bool,
    /// S6 re-entry HYSTERESIS, entry threshold: a non-lineage colonist re-enters
    /// edible-grain gathering once its hunger reaches this level. Only consulted when
    /// [`Self::productive_reentry`] is set.
    pub reentry_hunger_in: u16,
    /// S6 re-entry HYSTERESIS, exit threshold (`< reentry_hunger_in`): a fed
    /// re-entrant reverts to its **home** role — a WOOD gatherer resumes WOOD
    /// gathering, an idle consumer goes idle — only once its hunger falls below this
    /// level. The gap `reentry_hunger_in − reentry_hunger_out` is the hysteresis band:
    /// inside it a colonist holds its current node, so re-entry does not thrash
    /// node-to-node every tick, and fed colonists return to WOOD gathering, keeping
    /// the WOOD supply alive. Only consulted when [`Self::productive_reentry`] is set.
    pub reentry_hunger_out: u16,
    /// PRODUCIBLE CAPITAL — tool-acquisition eligibility (S7.1): when `true`, a
    /// colonist that **holds** the required tool (a mill / an oven) is admitted to
    /// the role-choice adoption appraisal even when its seeded
    /// [`latent`](Colonist::latent) is `None`, and its regenerated scale carries the
    /// durable tool's anchor so it never posts the just-acquired capital as surplus
    /// (the phase-order trap). The existing spread appraisal then decides whether it
    /// adopts Miller/Baker — so a colonist that builds (S7.2) or is handed a tool can
    /// become a producer, lifting the chain's hard cap at the seeded tool count.
    /// `false` (every existing config) keeps the seeded-`latent`-only gate, so the
    /// pre-S7 role-choice path and the conformance goldens are byte-identical.
    pub tool_acquisition_eligibility: bool,
    /// PRODUCIBLE CAPITAL — the per-builder BuildMill/BuildOven phase (S7.2): when
    /// `true` *and* money has emerged, each econ tick (before role-choice) a fed,
    /// non-latent colonist with saved WOOD that **appraises** building a mill/oven will
    /// pay commits its OWN WOOD via a project, advances it with its own labor over
    /// several ticks, and on completion credits the tool to its own stock (booked
    /// `consumed_as_input` at the start, `produced` at completion — conserved). The new
    /// tool then makes the builder eligible (S7.1) to adopt and produce, lifting the
    /// chain's hard cap at the seeded tool count. Requires
    /// [`Self::tool_acquisition_eligibility`] (a built tool is useless if holding it
    /// does not make the builder eligible). `false` (every existing config) skips the
    /// phase, so the conformance goldens (`produced_of(mill) == 0`) are byte-identical.
    pub producible_capital: bool,
    /// S10 (per-agent intertemporal capital choice / originary interest): when `true`
    /// *and* [`Self::producible_capital`] is on, the capital-formation phase replaces
    /// S7's settlement-level build planner (one global stage choice by capacity
    /// bottleneck + a scalar `margin × capital_payback_cycles` test + first-eligible-fed
    /// builder assignment) with a **per-colonist ordinal** decision: each eligible
    /// colonist runs [`appraise_capital_tool_bundle_for_money`] on its OWN value scale —
    /// committing present WOOD + forgone leisure against the tool's recipe-margin receipt
    /// stream provisioning one of its own future-money savings wants — and each colonist
    /// its own appraisal accepts starts its own build (no global stage choice, no
    /// first-eligible assignment, no single-in-flight gate). Capital formation then tracks
    /// each colonist's `time_preference_bps` ordinally (the savings ladder deepens with
    /// patience — [`life::savings_ladder_depth`]), with NO cardinal discount and
    /// `capital_payback_cycles` left inert. The per-builder substrate
    /// (`start_project`/`advance_project`/`complete_project_if_ready`) is reused unchanged.
    /// `false` (every existing config) keeps the S7 heuristic, so the conformance goldens
    /// are byte-identical.
    pub per_agent_capital: bool,
    /// S11 (entrepreneurial uncertainty + profit/loss selection): when `true` *and*
    /// money has emerged, every entrepreneurial appraisal weighs its OUTPUT-revenue
    /// estimate against a **per-agent fallible forecast** instead of the shared last
    /// realized price — `forecast = grounded_base × culture.forecast_bias_bps / 10_000`,
    /// where the grounded base is the agent's own adaptive [`PriceBelief`] (once it has
    /// observed the good) else the public realized price. The role-choice adopt, the
    /// per-agent capital build, and the project input-bid all read this forecast for the
    /// output price; INPUT/build costs stay at observed prices (one clean lever —
    /// output optimism). The market still clears at the REAL price (forecasts move no
    /// goods), so an over-optimist sinks WOOD/inputs into capital that underperforms and
    /// bears the loss through CAPITAL accumulation — selection without mortality. With
    /// the default `forecast_bias_bps == 10_000` (×1.0) the forecast equals the grounded
    /// base, and the whole path is gated on this flag, so `false` (every existing config)
    /// is byte-identical to the pre-S11 stream. Heterogeneity comes from the heritable
    /// [`CultureParams::forecast_bias_bps`].
    pub entrepreneurial_forecasts: bool,
    /// C3R.h (L2): value a recipe's input in `run_role_choice` at the minimum
    /// non-self holder reservation ask instead of the stale last-trade realized price.
    /// This is a fresh, deterministic appraisal proxy, not a guaranteed clearing price.
    /// With no such ask the candidate declines rather than treating the input as free.
    /// Default-off so existing runs keep the prior realized-price path byte-for-byte.
    pub stale_input_price_fix: bool,
    /// S7.2: the amortization horizon for the build appraisal — a durable tool is
    /// multi-period capital, so a colonist builds only when its expected per-run margin
    /// over this many cycles repays the build cost: `expected_margin_per_run × N >
    /// WOOD_build_cost + labor_opportunity_cost + first input`. Larger `N` makes the
    /// colony build more readily (a longer payback window); `0`/`1` makes a durable
    /// tool repay in ~one cycle, near the one-shot adoption test. Consulted only when
    /// [`Self::producible_capital`] is on.
    pub capital_payback_cycles: u32,
    /// S7.2: the saved WOOD a single mill/oven build consumes (`input_goods = [(WOOD,
    /// n)]`). Its market value (`wood_price × n`) is the dominant build cost the
    /// appraisal charges, so a scarce/dear WOOD supply is the faithful brake on
    /// over-building. Consulted only when [`Self::producible_capital`] is on.
    pub tool_build_wood: u32,
    /// S7.2: the labor a single mill/oven build requires (the project's
    /// `required_labor`) — the builder advances it one unit per tick from its own
    /// labor, so the tool completes this many ticks after it starts. Its opportunity
    /// cost (`labor × operating_cost`) is charged in the legacy build appraisal.
    /// In per-agent mode the dated receipt stream starts after this gestation, so
    /// values at or beyond the deepest savings horizon leave no appraisable future
    /// receipt. Consulted only when [`Self::producible_capital`] is on.
    pub tool_build_labor: u32,
    /// S7.2: the highest hunger at which a colonist will START a build — a fed colonist
    /// with surplus invests in capital, a hungry one gathers/feeds first (hunger above
    /// building on its own value scale). An in-flight build keeps advancing regardless
    /// (its labor is background; abandoning would forfeit committed WOOD). Consulted
    /// only when [`Self::producible_capital`] is on.
    pub capital_build_hunger_max: u16,
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
    /// S21e: one-time finite surplus bread added only to the diagnosed seller class.
    /// `0` for every existing config, so default/golden runs remain byte-identical;
    /// a nonzero value is canonicalized because it changes future holdings.
    pub seeded_surplus_bread: u32,
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
    /// S19: seeded cycle role A producers (Z → X). `0` for every non-cycle chain.
    pub cycle_a_producers: u16,
    /// S19: seeded cycle role B producers (X → Y). `0` for every non-cycle chain.
    pub cycle_b_producers: u16,
    /// S19: seeded cycle role C producers (Y → Z). `0` for every non-cycle chain.
    pub cycle_c_producers: u16,
    /// S19: Z units each role A producer starts holding and reserves as recipe input.
    pub cycle_a_input_buffer: u32,
    /// S19: X units each role B producer starts holding and reserves as recipe input.
    pub cycle_b_input_buffer: u32,
    /// S19: Y units each role C producer starts holding and reserves as recipe input.
    pub cycle_c_input_buffer: u32,
    /// S21h.0: a finite **consumed-only** starting bread cushion for the NON-LINEAGE
    /// woodcutters (`Vocation::Gatherer`, built via [`build_agent`] — lineage members use
    /// `build_demography_agent`). A dedicated field rather than the shared `bread_buffer`
    /// (which also seeds the chain's other non-consumer roles) so raising it can never
    /// re-seed lineage/seller bread and break the `SelfProduced`/sold-for-SALT provenance.
    /// `0` for every existing config, so the gatherers carry no bread and the run is
    /// byte-identical (canonicalized ON-only). The buyers' analogous cushion reuses the
    /// already-wired `consumer_staple_buffer`.
    pub gatherer_food_cushion: u32,
    /// S21h.1: the gated EMERGENCY self-provisioning trigger — a non-lineage
    /// `Consumer`/`Gatherer` becomes eligible for the near-critical own-labor bread floor
    /// ([`Settlement::run_emergency_self_provision`]) **only when its hunger reaches this
    /// threshold**, and the floor pulls its projected hunger down to just below it
    /// (`threshold - 1`), so the role survives WITHOUT being satiated out of the bread
    /// market (demand-preserving). `0` (off) for every existing config, so the phase is
    /// inert and the run is byte-identical (canonicalized ON-only). When on it is
    /// validated above the lineage `cultivate_hunger_in` trigger and strictly below
    /// `hunger_critical`, so it fires within the alive-but-lethal-pressure window.
    pub emergency_hunger_threshold: u16,
    /// C3R.e (impl-67): the A1 ignition gate — the econ tick at which the one-shot conserved
    /// birth-stock injection fires ONCE (donors restricted to NON-producer households, every
    /// moved staple unit origin-flagged), a dedicated latch independent of the SufficiencyControl
    /// mode. `None` for every existing config, so no ignition fires and the run is byte-identical
    /// (tag 33, ON-only).
    pub birth_stock_ignition_at: Option<u64>,
    /// C3R.e (impl-67): the A2 additive endowment — extra starting staple credited to each
    /// producer-house subject at generation ([`build_agent`]), then split out of the tick-0
    /// bootstrap sweep as origin-flagged lots (so the additive stock is exhaustion-tracked). `0`
    /// for every existing config, so no endowment is added and the run is byte-identical (tag 33).
    pub producer_house_starting_staple: u32,
    /// C3R.e (impl-67): the B support-withdrawal gate — while `econ_tick < until` the producer-house
    /// `food_provision` hearth AND the `producer_subsistence` cushion's STAPLE leg are delivered; at
    /// or after `until` both are withdrawn. Its mere PRESENCE (`Some`) also disables the cushion's
    /// WOOD leg for the ENTIRE run (constant across eras — the bread-only ledger cannot origin-track
    /// subsidized WOOD). `None` for every existing config (support never gated off, WOOD leg intact),
    /// so the run is byte-identical (tag 33, ON-only).
    pub producer_support_until_tick: Option<u64>,
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
            subsistence_on_grain: false,
            // Own-labor subsistence off by default (S12): the food mints stay, no FORAGE
            // is produced, and the forage knobs are inert — so every existing config and
            // its goldens are byte-identical.
            own_labor_subsistence: false,
            forage_yield: 0,
            forage_commons: None,
            forage_hunger_in: 8,
            forage_hunger_out: 4,
            // Own-use cultivation off by default (S15): no Cultivate recipe, the steering
            // and phase are inert, and these knobs are unused — so every existing config
            // and its goldens are byte-identical.
            own_use_cultivation: false,
            cultivate_hunger_in: 10,
            cultivate_hunger_out: 6,
            cultivate_consume: 0,
            cultivate_patience: 4,
            // S16 off by default: no buy/sell split, no provenance ledger — byte-identical.
            cultivation_sells_surplus: false,
            // S18 off by default: no woodcutter routing, no multi-good instrumentation —
            // byte-identical.
            multigood_money: false,
            // S21f off by default: no household-barter cultivation seam, so cultivation
            // still requires the forage substrate and every existing config is byte-identical.
            household_barter_cultivation: false,
            // S22a off by default: cultivation eligibility stays pinned to the lineage, so
            // every existing config is byte-identical (canonicalized ON-only).
            endogenous_cultivation_entry: false,
            // S22b off by default: no per-agent cultivation skill, so the grain-haul lever is
            // inert and every existing config is byte-identical (canonicalized ON-only). The
            // magnitudes are the pinned house-style defaults, consulted only when the gate is on.
            cultivation_skill: false,
            // S22c off by default: the cultivation exit stays pure hunger/pressure (no profit-stay),
            // so every existing config is byte-identical (canonicalized ON-only). The window/margin/
            // floor are the pinned house-style defaults, consulted only when the gate is on.
            profit_driven_retention: false,
            return_window: RETURN_WINDOW,
            retention_margin_bps: RETENTION_MARGIN_BPS,
            retention_material_floor: RETENTION_MATERIAL_FLOOR,
            skill_gain: SKILL_GAIN,
            skill_decay: SKILL_DECAY,
            skill_cap: SKILL_CAP,
            skill_haul_ceiling: SKILL_HAUL_CEILING,
            durable_cultivation_tool: false,
            tool_build_patience: TOOL_BUILD_PATIENCE,
            cultivation_tool_haul_ceiling: CULTIVATION_TOOL_HAUL_CEILING,
            cultivation_tool_non_durable: false,
            // S22e off by default: no endowed/inherited cultivation capital, so every existing
            // config and its goldens are byte-identical (canonicalized ON-only). The inheritance
            // switch defaults to `true` so that, when the gate IS turned on, plows keep the
            // existing heir routing unless a control flips it off.
            endowed_cultivation_capital: false,
            endowed_tool_count: 0,
            cultivation_tool_inheritance: true,
            // S22f off by default: no voluntary cultivation commitment, so every existing config and
            // its goldens are byte-identical (canonicalized ON-only). The term/floor defaults are
            // consulted only once the gate is on (and composed on S22c profit-driven retention).
            voluntary_cultivation_commitment: false,
            commitment_term: COMMITMENT_TERM_DEFAULT,
            commitment_entry_floor: COMMITMENT_ENTRY_FLOOR_DEFAULT,
            commitment_fiat_pin: 0,
            commitment_norm_spread: false,
            abandonable_norm: false,
            group_payoff_imitation: false,
            fixed_commitment_norm_prevalence: None,
            commitment_seed_share_bps: COMMITMENT_SEED_SHARE_BPS_DEFAULT,
            imitation_period: COMMITMENT_NORM_IMITATION_PERIOD_DEFAULT,
            imitation_window: COMMITMENT_NORM_IMITATION_WINDOW_DEFAULT,
            imitation_margin_bps: COMMITMENT_NORM_IMITATION_MARGIN_BPS_DEFAULT,
            imitation_radius: COMMITMENT_NORM_IMITATION_RADIUS_DEFAULT,
            imitation_max_models: COMMITMENT_NORM_IMITATION_MAX_MODELS_DEFAULT,
            food_window_target: COMMITMENT_NORM_FOOD_WINDOW_TARGET_DEFAULT,
            no_imitation: false,
            random_imitation: false,
            salt_in_score: false,
            private_land_tenure: false,
            land_idle_limit: LAND_IDLE_LIMIT_DEFAULT,
            harvest_gate: true,
            forfeit_on_idle: true,
            reclaim_reserved_for_prior_owner: false,
            land_good_plots: LAND_GOOD_PLOTS_DEFAULT,
            land_marginal_plots: LAND_MARGINAL_PLOTS_DEFAULT,
            land_marginal_regen: LAND_MARGINAL_REGEN_DEFAULT,
            secure_land_tenure: false,
            inheritance_regime: InheritanceRegime::Impartible,
            land_market: false,
            mortal_landowner_demography: false,
            rival_subsistence_commons: false,
            rival_subsistence_commons_phi_bps: 0,
            wage_labor: false,
            wage_labor_mode: WageLaborMode::Voluntary,
            share_tenancy: false,
            share_tenancy_mode: ShareTenancyMode::Voluntary,
            share_forward_provisioning: false,
            share_contract_succession: false,
            in_kind_wage: false,
            mortal_chain_producers: false,
            mortal_producer_inheritance: false,
            mortal_producer_tool_inheritance: true,
            producer_house_cap: MORTAL_PRODUCER_HOUSE_CAP_DEFAULT,
            earned_provisioning: false,
            producer_stock_provisioning_control: false,
            birth_stock_saving: false,
            birth_stock_saving_mode: BirthStockSavingMode::Off,
            saving_allocation_obs: false,
            birth_gate_obs: false,
            share_bps: SHARE_TENANCY_BPS_DEFAULT,
            share_term: SHARE_TENANCY_TERM_DEFAULT,
            land_carrying_cost: LAND_CARRYING_COST_DEFAULT,
            land_price_cap_factor: LAND_PRICE_CAP_FACTOR_DEFAULT,
            // S21d.0 off by default: the food mints stay, so every existing config and its
            // goldens are byte-identical (canonicalized ON-only).
            retire_food_mints: false,
            // S21d.1 off by default: no acquisition-channel ledger (runtime-only diagnostic,
            // never digested), so every existing config is byte-identical whether on or off.
            acquisition_ledger: false,
            capital_advance: false,
            perishable_decay_bps: 0,
            subsistence_advance: false,
            input_advance: false,
            recurring_motive: false,
            project_input_bids: false,
            producer_subsistence: 0,
            // Re-entry off by default (S6): the gated phase is inert, so every existing
            // config and its goldens are byte-identical. The hysteresis thresholds are
            // consulted only when the flag is on.
            productive_reentry: false,
            reentry_hunger_in: 8,
            reentry_hunger_out: 4,
            // Producible capital off by default (S7): the relaxed tool-acquisition
            // gate and the per-builder build phase are inert, so every existing config
            // and its goldens are byte-identical (no tool is ever built, and a
            // non-latent tool-holder never appears without it). The build knobs are
            // consulted only when the phase is on.
            tool_acquisition_eligibility: false,
            producible_capital: false,
            per_agent_capital: false,
            entrepreneurial_forecasts: false,
            stale_input_price_fix: false,
            capital_payback_cycles: 8,
            tool_build_wood: 6,
            tool_build_labor: 4,
            capital_build_hunger_max: 4,
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
            seeded_surplus_bread: 0,
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
            cycle_a_producers: 0,
            cycle_b_producers: 0,
            cycle_c_producers: 0,
            cycle_a_input_buffer: 0,
            cycle_b_input_buffer: 0,
            cycle_c_input_buffer: 0,
            // S21h off by default: no demand-side survival bridge — the gatherers carry no
            // bread cushion and the emergency self-provisioning phase is inert, so every
            // existing config and its goldens are byte-identical (both canonicalized ON-only).
            gatherer_food_cushion: 0,
            emergency_hunger_threshold: 0,
            // C3R.e off by default (tag 33, ON-only): no ignition, no additive endowment, no
            // support-withdrawal gate — every existing config stays byte-identical.
            birth_stock_ignition_at: None,
            producer_house_starting_staple: 0,
            producer_support_until_tick: None,
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
            subsistence_on_grain: false,
            own_labor_subsistence: false,
            forage_yield: 0,
            forage_commons: None,
            forage_hunger_in: 8,
            forage_hunger_out: 4,
            // Own-use cultivation off by default (S15): no Cultivate recipe, the steering
            // and phase are inert, and these knobs are unused — so every existing config
            // and its goldens are byte-identical.
            own_use_cultivation: false,
            cultivate_hunger_in: 10,
            cultivate_hunger_out: 6,
            cultivate_consume: 0,
            cultivate_patience: 4,
            // S16 off by default: no buy/sell split, no provenance ledger — byte-identical.
            cultivation_sells_surplus: false,
            // S18 off by default: no woodcutter routing, no multi-good instrumentation —
            // byte-identical.
            multigood_money: false,
            // S21f off by default: no household-barter cultivation seam, so cultivation
            // still requires the forage substrate and every existing config is byte-identical.
            household_barter_cultivation: false,
            // S22a off by default: cultivation eligibility stays pinned to the lineage, so
            // every existing config is byte-identical (canonicalized ON-only).
            endogenous_cultivation_entry: false,
            // S22b off by default: no per-agent cultivation skill, so the grain-haul lever is
            // inert and every existing config is byte-identical (canonicalized ON-only). The
            // magnitudes are the pinned house-style defaults, consulted only when the gate is on.
            cultivation_skill: false,
            // S22c off by default: the cultivation exit stays pure hunger/pressure (no profit-stay),
            // so every existing config is byte-identical (canonicalized ON-only). The window/margin/
            // floor are the pinned house-style defaults, consulted only when the gate is on.
            profit_driven_retention: false,
            return_window: RETURN_WINDOW,
            retention_margin_bps: RETENTION_MARGIN_BPS,
            retention_material_floor: RETENTION_MATERIAL_FLOOR,
            skill_gain: SKILL_GAIN,
            skill_decay: SKILL_DECAY,
            skill_cap: SKILL_CAP,
            skill_haul_ceiling: SKILL_HAUL_CEILING,
            durable_cultivation_tool: false,
            tool_build_patience: TOOL_BUILD_PATIENCE,
            cultivation_tool_haul_ceiling: CULTIVATION_TOOL_HAUL_CEILING,
            cultivation_tool_non_durable: false,
            // S22e off by default: no endowed/inherited cultivation capital, so every existing
            // config and its goldens are byte-identical (canonicalized ON-only). The inheritance
            // switch defaults to `true` so that, when the gate IS turned on, plows keep the
            // existing heir routing unless a control flips it off.
            endowed_cultivation_capital: false,
            endowed_tool_count: 0,
            cultivation_tool_inheritance: true,
            // S22f off by default: no voluntary cultivation commitment, so every existing config and
            // its goldens are byte-identical (canonicalized ON-only). The term/floor defaults are
            // consulted only once the gate is on (and composed on S22c profit-driven retention).
            voluntary_cultivation_commitment: false,
            commitment_term: COMMITMENT_TERM_DEFAULT,
            commitment_entry_floor: COMMITMENT_ENTRY_FLOOR_DEFAULT,
            commitment_fiat_pin: 0,
            commitment_norm_spread: false,
            abandonable_norm: false,
            group_payoff_imitation: false,
            fixed_commitment_norm_prevalence: None,
            commitment_seed_share_bps: COMMITMENT_SEED_SHARE_BPS_DEFAULT,
            imitation_period: COMMITMENT_NORM_IMITATION_PERIOD_DEFAULT,
            imitation_window: COMMITMENT_NORM_IMITATION_WINDOW_DEFAULT,
            imitation_margin_bps: COMMITMENT_NORM_IMITATION_MARGIN_BPS_DEFAULT,
            imitation_radius: COMMITMENT_NORM_IMITATION_RADIUS_DEFAULT,
            imitation_max_models: COMMITMENT_NORM_IMITATION_MAX_MODELS_DEFAULT,
            food_window_target: COMMITMENT_NORM_FOOD_WINDOW_TARGET_DEFAULT,
            no_imitation: false,
            random_imitation: false,
            salt_in_score: false,
            private_land_tenure: false,
            land_idle_limit: LAND_IDLE_LIMIT_DEFAULT,
            harvest_gate: true,
            forfeit_on_idle: true,
            reclaim_reserved_for_prior_owner: false,
            land_good_plots: LAND_GOOD_PLOTS_DEFAULT,
            land_marginal_plots: LAND_MARGINAL_PLOTS_DEFAULT,
            land_marginal_regen: LAND_MARGINAL_REGEN_DEFAULT,
            secure_land_tenure: false,
            inheritance_regime: InheritanceRegime::Impartible,
            land_market: false,
            mortal_landowner_demography: false,
            rival_subsistence_commons: false,
            rival_subsistence_commons_phi_bps: 0,
            wage_labor: false,
            wage_labor_mode: WageLaborMode::Voluntary,
            share_tenancy: false,
            share_tenancy_mode: ShareTenancyMode::Voluntary,
            share_forward_provisioning: false,
            share_contract_succession: false,
            in_kind_wage: false,
            mortal_chain_producers: false,
            mortal_producer_inheritance: false,
            mortal_producer_tool_inheritance: true,
            producer_house_cap: MORTAL_PRODUCER_HOUSE_CAP_DEFAULT,
            earned_provisioning: false,
            producer_stock_provisioning_control: false,
            birth_stock_saving: false,
            birth_stock_saving_mode: BirthStockSavingMode::Off,
            saving_allocation_obs: false,
            birth_gate_obs: false,
            share_bps: SHARE_TENANCY_BPS_DEFAULT,
            share_term: SHARE_TENANCY_TERM_DEFAULT,
            land_carrying_cost: LAND_CARRYING_COST_DEFAULT,
            land_price_cap_factor: LAND_PRICE_CAP_FACTOR_DEFAULT,
            // S21d.0 off by default: the food mints stay, so every existing config and its
            // goldens are byte-identical (canonicalized ON-only).
            retire_food_mints: false,
            // S21d.1 off by default: no acquisition-channel ledger (runtime-only diagnostic,
            // never digested), so every existing config is byte-identical whether on or off.
            acquisition_ledger: false,
            capital_advance: false,
            perishable_decay_bps: 0,
            subsistence_advance: false,
            input_advance: false,
            recurring_motive: false,
            project_input_bids: false,
            producer_subsistence: 0,
            productive_reentry: false,
            reentry_hunger_in: 8,
            reentry_hunger_out: 4,
            tool_acquisition_eligibility: false,
            producible_capital: false,
            per_agent_capital: false,
            entrepreneurial_forecasts: false,
            stale_input_price_fix: false,
            capital_payback_cycles: 8,
            tool_build_wood: 6,
            tool_build_labor: 4,
            capital_build_hunger_max: 4,
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
            seeded_surplus_bread: 0,
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
            cycle_a_producers: 0,
            cycle_b_producers: 0,
            cycle_c_producers: 0,
            cycle_a_input_buffer: 0,
            cycle_b_input_buffer: 0,
            cycle_c_input_buffer: 0,
            // S21h off by default (see `grain_flour_bread`).
            gatherer_food_cushion: 0,
            emergency_hunger_threshold: 0,
            // C3R.e off by default (see `grain_flour_bread`).
            birth_stock_ignition_at: None,
            producer_house_starting_staple: 0,
            producer_support_until_tick: None,
        }
    }

    /// S19: the canonical imperfect-double-coincidence 3-good production cycle.
    /// Role A consumes Z and produces X; B consumes X and produces Y; C consumes Y
    /// and produces Z. The produced goods are wanted only as producer inputs, via
    /// `Horizon::Next` producer-input wants; there is no consumption taste for X/Y/Z.
    pub fn three_good_cycle() -> Self {
        Self {
            content: ContentSet::three_good_cycle(),
            millers: 0,
            bakers: 0,
            latent_millers: 0,
            latent_bakers: 0,
            operating_cost: 1,
            bread_is_staple: false,
            subsistence_on_grain: false,
            own_labor_subsistence: false,
            forage_yield: 0,
            forage_commons: None,
            forage_hunger_in: 8,
            forage_hunger_out: 4,
            own_use_cultivation: false,
            cultivate_hunger_in: 10,
            cultivate_hunger_out: 6,
            cultivate_consume: 0,
            cultivate_patience: 4,
            cultivation_sells_surplus: false,
            multigood_money: false,
            // S21f off by default: no household-barter cultivation seam, so cultivation
            // still requires the forage substrate and every existing config is byte-identical.
            household_barter_cultivation: false,
            // S22a off by default: cultivation eligibility stays pinned to the lineage, so
            // every existing config is byte-identical (canonicalized ON-only).
            endogenous_cultivation_entry: false,
            // S22b off by default: no per-agent cultivation skill, so the grain-haul lever is
            // inert and every existing config is byte-identical (canonicalized ON-only). The
            // magnitudes are the pinned house-style defaults, consulted only when the gate is on.
            cultivation_skill: false,
            // S22c off by default: the cultivation exit stays pure hunger/pressure (no profit-stay),
            // so every existing config is byte-identical (canonicalized ON-only). The window/margin/
            // floor are the pinned house-style defaults, consulted only when the gate is on.
            profit_driven_retention: false,
            return_window: RETURN_WINDOW,
            retention_margin_bps: RETENTION_MARGIN_BPS,
            retention_material_floor: RETENTION_MATERIAL_FLOOR,
            skill_gain: SKILL_GAIN,
            skill_decay: SKILL_DECAY,
            skill_cap: SKILL_CAP,
            skill_haul_ceiling: SKILL_HAUL_CEILING,
            durable_cultivation_tool: false,
            tool_build_patience: TOOL_BUILD_PATIENCE,
            cultivation_tool_haul_ceiling: CULTIVATION_TOOL_HAUL_CEILING,
            cultivation_tool_non_durable: false,
            // S22e off by default: no endowed/inherited cultivation capital, so every existing
            // config and its goldens are byte-identical (canonicalized ON-only). The inheritance
            // switch defaults to `true` so that, when the gate IS turned on, plows keep the
            // existing heir routing unless a control flips it off.
            endowed_cultivation_capital: false,
            endowed_tool_count: 0,
            cultivation_tool_inheritance: true,
            // S22f off by default: no voluntary cultivation commitment, so every existing config and
            // its goldens are byte-identical (canonicalized ON-only). The term/floor defaults are
            // consulted only once the gate is on (and composed on S22c profit-driven retention).
            voluntary_cultivation_commitment: false,
            commitment_term: COMMITMENT_TERM_DEFAULT,
            commitment_entry_floor: COMMITMENT_ENTRY_FLOOR_DEFAULT,
            commitment_fiat_pin: 0,
            commitment_norm_spread: false,
            abandonable_norm: false,
            group_payoff_imitation: false,
            fixed_commitment_norm_prevalence: None,
            commitment_seed_share_bps: COMMITMENT_SEED_SHARE_BPS_DEFAULT,
            imitation_period: COMMITMENT_NORM_IMITATION_PERIOD_DEFAULT,
            imitation_window: COMMITMENT_NORM_IMITATION_WINDOW_DEFAULT,
            imitation_margin_bps: COMMITMENT_NORM_IMITATION_MARGIN_BPS_DEFAULT,
            imitation_radius: COMMITMENT_NORM_IMITATION_RADIUS_DEFAULT,
            imitation_max_models: COMMITMENT_NORM_IMITATION_MAX_MODELS_DEFAULT,
            food_window_target: COMMITMENT_NORM_FOOD_WINDOW_TARGET_DEFAULT,
            no_imitation: false,
            random_imitation: false,
            salt_in_score: false,
            private_land_tenure: false,
            land_idle_limit: LAND_IDLE_LIMIT_DEFAULT,
            harvest_gate: true,
            forfeit_on_idle: true,
            reclaim_reserved_for_prior_owner: false,
            land_good_plots: LAND_GOOD_PLOTS_DEFAULT,
            land_marginal_plots: LAND_MARGINAL_PLOTS_DEFAULT,
            land_marginal_regen: LAND_MARGINAL_REGEN_DEFAULT,
            secure_land_tenure: false,
            inheritance_regime: InheritanceRegime::Impartible,
            land_market: false,
            mortal_landowner_demography: false,
            rival_subsistence_commons: false,
            rival_subsistence_commons_phi_bps: 0,
            wage_labor: false,
            wage_labor_mode: WageLaborMode::Voluntary,
            share_tenancy: false,
            share_tenancy_mode: ShareTenancyMode::Voluntary,
            share_forward_provisioning: false,
            share_contract_succession: false,
            in_kind_wage: false,
            mortal_chain_producers: false,
            mortal_producer_inheritance: false,
            mortal_producer_tool_inheritance: true,
            producer_house_cap: MORTAL_PRODUCER_HOUSE_CAP_DEFAULT,
            earned_provisioning: false,
            producer_stock_provisioning_control: false,
            birth_stock_saving: false,
            birth_stock_saving_mode: BirthStockSavingMode::Off,
            saving_allocation_obs: false,
            birth_gate_obs: false,
            share_bps: SHARE_TENANCY_BPS_DEFAULT,
            share_term: SHARE_TENANCY_TERM_DEFAULT,
            land_carrying_cost: LAND_CARRYING_COST_DEFAULT,
            land_price_cap_factor: LAND_PRICE_CAP_FACTOR_DEFAULT,
            // S21d.0 off by default: the food mints stay, so every existing config and its
            // goldens are byte-identical (canonicalized ON-only).
            retire_food_mints: false,
            // S21d.1 off by default: no acquisition-channel ledger (runtime-only diagnostic,
            // never digested), so every existing config is byte-identical whether on or off.
            acquisition_ledger: false,
            capital_advance: false,
            perishable_decay_bps: 0,
            subsistence_advance: false,
            input_advance: false,
            recurring_motive: false,
            project_input_bids: false,
            // The S19 scenario sets a positive floor. The constructor stays usable
            // for controls that deliberately force survival back on-market.
            producer_subsistence: 0,
            productive_reentry: false,
            reentry_hunger_in: 8,
            reentry_hunger_out: 4,
            tool_acquisition_eligibility: false,
            producible_capital: false,
            per_agent_capital: false,
            entrepreneurial_forecasts: false,
            stale_input_price_fix: false,
            capital_payback_cycles: 8,
            tool_build_wood: 6,
            tool_build_labor: 4,
            capital_build_hunger_max: 4,
            throughput: 1,
            miller_grain_buffer: 0,
            baker_flour_buffer: 0,
            latent_flour_seed: 0,
            bread_buffer: 0,
            seeded_surplus_bread: 0,
            consumer_staple_buffer: 0,
            wood_buffer: 0,
            consumer_wood_buffer: 0,
            producer_gold: 0,
            scholars: 0,
            confectioners: 0,
            tier2_threshold: 0,
            scholar_grain_buffer: 0,
            confectioner_flour_buffer: 0,
            cycle_a_producers: 3,
            cycle_b_producers: 3,
            cycle_c_producers: 3,
            cycle_a_input_buffer: 6,
            cycle_b_input_buffer: 6,
            cycle_c_input_buffer: 6,
            // S21h off by default (see `grain_flour_bread`).
            gatherer_food_cushion: 0,
            emergency_hunger_threshold: 0,
            // C3R.e off by default (see `grain_flour_bread`).
            birth_stock_ignition_at: None,
            producer_house_starting_staple: 0,
            producer_support_until_tick: None,
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
    /// S19: units of the medium commodity seeded to each 3-good-cycle producer.
    /// This is a neutral stock endowment, not a designated-money balance and not a
    /// medium want. It lets the agents that carry derived producer-input demand bid
    /// SALT for X/Y/Z before any money good exists. Zero for every pre-S19 config.
    pub cycle_producer_medium_endowment: u32,
    /// S9 — the **heterogeneous real direct use** of the medium good (SALT). How
    /// many fixed `Good(medium)/Now` consumption wants the SELECTED subset of
    /// colonists carries each pre-promotion tick (injected like
    /// [`medium_scale_extension`], but representing CONSUMPTION, not a "hold the
    /// medium" savings demand). This is the Mengerian regression-theorem seed: a
    /// commodity with a real non-monetary use that some actors directly want, which
    /// lets SALT accrue saleability from real direct trades BEFORE it is money — the
    /// replacement for the circular universal medium want. The want is consumed into
    /// the `consumed` bucket and is active only while no money good has emerged.
    /// Default `0`: off — every pre-S9 scenario keeps its medium-want path unchanged
    /// and is byte-identical. Paired with [`Self::salt_direct_use_period`], which
    /// makes the demand HETEROGENEOUS (a universal direct want would suppress
    /// indirect acceptance — `generate_indirect_barter_offers` skips an agent that
    /// directly wants the leader).
    pub salt_direct_use_qty: u32,
    /// S9 — the heterogeneity selector for [`Self::salt_direct_use_qty`]: a colonist
    /// directly wants SALT this tick iff its stable id index is `0 mod period`. A
    /// `period` of 2 gives every other colonist the direct want, 3 one in three, and
    /// so on; `1` would be universal (deliberately avoidable — universality is the
    /// trap Base Fact 6 warns against). The non-selected colonists never carry the
    /// direct want, so they remain eligible to accept SALT INDIRECTLY (the breadth
    /// the strong-bar gate requires). Default `0`: off (no colonist is selected even
    /// if a qty were set), keeping every pre-S9 scenario byte-identical.
    pub salt_direct_use_period: u16,
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
#[derive(Clone, Debug, PartialEq)]
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
    /// S11: the colony's base entrepreneurial **forecast bias** (bps), around which
    /// each colonist's heritable [`CultureParams::forecast_bias_bps`] is jittered
    /// deterministically at generation. Neutral [`FORECAST_BIAS_NEUTRAL_BPS`] (×1.0)
    /// for every config: with `entrepreneurial_forecasts` off the bias is never read
    /// or serialized, so a non-neutral base only matters for an entrepreneurial colony.
    pub forecast_bias_base_bps: u16,
    /// S11: the half-width (bps) of the deterministic per-colonist jitter band around
    /// [`Self::forecast_bias_base_bps`] at generation — a colonist's drawn bias lands in
    /// `base ± forecast_bias_jitter_bps` (then clamped to `5_000..=20_000`). The flagship
    /// keeps a wide band so optimists and accurate forecasters coexist (the selection
    /// substrate); a controlled microtest sets `0` for a UNIFORM colony (every colonist at
    /// the base). Consumes no `Rng` either way, so a flag-off run is byte-identical.
    pub forecast_bias_jitter_bps: u16,
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
    /// The G8c-1 **credit-cycle** overlay (the Austrian business cycle + regime
    /// ladder + fiat), or `None` for every pre-G8c-1 config — which keeps them all
    /// byte-identical by construction (the finance path is skipped entirely). When
    /// `Some`, the settlement is a **finance** settlement: it has no spatial colony
    /// (gatherers/consumers/chain), its [`Society`] is built from econ's unchanged
    /// credit-ladder scenario, and each econ tick steps that society so the cycle
    /// (or its sound-money control) runs endogenously. Mutually exclusive with every
    /// spatial overlay (`chain`/`demography`/`barter`/`bank`) and requires the M3
    /// ledger. See [`CycleConfig`], [`SettlementConfig::credit_cycle`], and
    /// [`SettlementConfig::sound_money`].
    pub cycle: Option<CycleConfig>,
    /// The G8c-2 **tender bench** overlay (one of the spot / debt / bank-repayment /
    /// issuer-repayment tender surfaces, M11/M12/M15/M16), or `None` for every config
    /// that is not a bench — which keeps them all byte-identical by construction (the
    /// bench path is skipped entirely). When `Some`, the settlement is a **finance**
    /// settlement (like the cycle): no spatial colony, its [`Society`] built from the
    /// unchanged econ scenario that exercises that surface (M11/M12 from the
    /// fiat-displacement scenarios, M15 from the suspended bank-loan-repayment scenario,
    /// M16 from the fiat-credit issuer-repayment scenario) with the surface's tender
    /// swapped in. Mutually exclusive with every spatial overlay and with [`Self::cycle`],
    /// and requires the M3 ledger. See [`TenderBench`], [`BenchSurface`], and the
    /// [`spot_tender_bench`](SettlementConfig::spot_tender_bench) /
    /// [`debt_tender_bench`](SettlementConfig::debt_tender_bench) /
    /// [`bank_repayment_tender_bench`](SettlementConfig::bank_repayment_tender_bench) /
    /// [`issuer_repayment_tender_bench`](SettlementConfig::issuer_repayment_tender_bench)
    /// constructors.
    pub tender_bench: Option<TenderBench>,
    /// The G8c-3 **tax overlay** (the state's levy + receivability), or `None` for every
    /// config that levies no tax — which keeps them all byte-identical by construction
    /// (the canonical tax block is omitted entirely). When `Some`, the settlement is the
    /// finance (credit-cycle) settlement with econ's unchanged M21 tax machinery routed
    /// in: the [`SetTaxReceivability`](EventKind::SetTaxReceivability) /
    /// [`LevyTax`](EventKind::LevyTax) events are layered onto the cycle scenario, and
    /// each econ tick steps the society so the levy seeds and settles endogenously.
    /// Requires the credit cycle ([`Self::cycle`] `Some`) and is mutually exclusive with
    /// a tender bench. See [`TaxPolicy`], [`SettlementConfig::tax_in_fiat`], and
    /// [`SettlementConfig::tax_in_specie`].
    pub tax: Option<TaxPolicy>,
    /// DH.a (impl-68) — the **closed-circulation marker**. `false` for every config except
    /// [`SettlementConfig::frontier_closed_circulation`], so every existing config and golden is
    /// byte-identical by construction (default-false; digest tag 34 is emitted ON-only). When
    /// `true` the runtime activates the whole-population gold/physical provenance ledger and the
    /// closure-observation pass — pure observation that alters no settlement (proven by the DH.a
    /// inertness test). See [`mod@closure`].
    pub closed_circulation: bool,
}

pub mod birth_gate;

pub mod burden;

pub mod closure;

mod in_kind_wage;

mod share_tenancy;

mod wage_labor;

/// Chain-runtime gate predicates (extracted from mod.rs — pure code motion).
mod gates;
mod rival_commons;
use gates::*;
/// Commitment-norm machinery (extracted from mod.rs — pure code motion).
mod commitment_norm;
use commitment_norm::*;
/// Land-market subsystem (extracted from mod.rs — pure code motion).
mod land_market;
use land_market::*;
/// Money/provenance accounting types (extracted from mod.rs — pure code motion).
mod accounting;
use accounting::*;
/// Determinism/digest surface (extracted from mod.rs — pure code motion).
mod digest;
#[cfg(test)]
use digest::*;
/// Demography/lifecycle machinery (extracted from mod.rs — pure code motion).
mod demography;
/// Settlement world/economy generator (extracted from mod.rs — pure code motion).
mod generation;
/// Econ-tick phase implementations (extracted from mod.rs — pure code motion).
mod phases;
/// Frontier scenario preset constructors (extracted from mod.rs — pure code motion).
mod scenarios;

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
            forecast_bias_base_bps: FORECAST_BIAS_NEUTRAL_BPS,
            forecast_bias_jitter_bps: FORECAST_BIAS_GEN_JITTER_DEFAULT,
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
            cycle: None,
            tender_bench: None,
            tax: None,
            closed_circulation: false,
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

    /// The G8c-1 **credit cycle** — the Austrian business cycle in the colony game.
    /// A finance settlement (no spatial colony) whose [`Society`] runs econ's
    /// unchanged credit-ladder scenario: the regime descends the ladder to
    /// [`Regime::Fiat`], the issuer extends fiat-credit, the market rate falls below
    /// the credit-disabled shadow natural rate (a measured **gap**), capitalists
    /// over-invest in the long roundabout project (the **boom**), credit **stops**,
    /// the rate reasserts, and the malinvested project is abandoned — consuming
    /// capital (the **bust**). Every signal is MEASURED from the M3 records + the
    /// shadow replay, never set. Paired with [`Self::sound_money`] it is the
    /// milestone's headline + falsification twin. See [`CycleKind::CreditCycle`].
    pub fn credit_cycle() -> Self {
        Self {
            m3: true,
            cycle: Some(CycleConfig {
                kind: CycleKind::CreditCycle,
                // Default (econ-default) tender: the fiat-credit employers pay fiat
                // wages under `ParAll`, so the cycle transmits exactly as G8c-1. A
                // default policy emits no tender events, keeping these bytes identical.
                tender: TenderPolicy::default(),
            }),
            ..Self::finance_base()
        }
    }

    /// The G8c-1 **sound-money control** — the falsification twin of
    /// [`Self::credit_cycle`]. Same agents and the same roundabout project line, but
    /// the regime stays [`Regime::SoundGold`] and the issuer never extends credit, so
    /// there is no fiat and no credit expansion: the gap stays ≈ 0, no boom forms,
    /// nothing is abandoned, and no capital is consumed. The proof the cycle is
    /// *credit*-driven, not an artifact of the production/spatial dynamics — if the
    /// control busts, the cycle is not coming from credit. See
    /// [`CycleKind::SoundMoney`].
    pub fn sound_money() -> Self {
        Self {
            m3: true,
            cycle: Some(CycleConfig {
                kind: CycleKind::SoundMoney,
                tender: TenderPolicy::default(),
            }),
            ..Self::finance_base()
        }
    }

    /// The G8c-2 **wage-tender cycle** — the headline. The G8c-1 credit cycle with
    /// fiat wages as **legal tender** ([`LaborWageTender::FiatAndSpecie`]): the
    /// fiat-credit employers can pay fiat wages, so the fiat credit reaches workers,
    /// demand follows, and the boom→stop→bust transmits exactly as the cycle proved.
    /// Paired with [`Self::wage_refusal_cycle`] it is the milestone's falsification
    /// twin: the *only* difference is whether fiat wages are accepted, isolating the
    /// wage surface as the transmission valve from credit to the structure of
    /// production (the lab's M17 result, now in the spatial cycle).
    pub fn wage_tender_cycle() -> Self {
        Self {
            m3: true,
            cycle: Some(CycleConfig {
                kind: CycleKind::CreditCycle,
                tender: TenderPolicy {
                    wage: LaborWageTender::FiatAndSpecie,
                    ..TenderPolicy::default()
                },
            }),
            ..Self::finance_base()
        }
    }

    /// The G8c-2 **wage-refusal cycle** — the control. The same credit cycle and the
    /// same fiat-credit issuance, but wages are **specie-only**
    /// ([`LaborWageTender::SpecieOnly`]): the fiat-credit employers cannot pay fiat
    /// wages, the credit never enters the real economy, and the same issuance is
    /// **inert** — no boom, no bust, no capital consumed. Paired with
    /// [`Self::wage_tender_cycle`] this is the proof the wage surface is the
    /// transmission valve: if the cycle fired here, the wage gate would not be routing
    /// settlement. (The printed fiat still round-trips back to the issuer — conserved,
    /// never leaked.)
    pub fn wage_refusal_cycle() -> Self {
        Self {
            m3: true,
            cycle: Some(CycleConfig {
                kind: CycleKind::CreditCycle,
                tender: TenderPolicy {
                    wage: LaborWageTender::SpecieOnly,
                    ..TenderPolicy::default()
                },
            }),
            ..Self::finance_base()
        }
    }

    /// The G8c-3 **tax-in-fiat** headline — the state's chartalist counter-lever to
    /// G8c-2. The wage-refusal cycle (specie-only wages → fiat credit inert, no private
    /// fiat demand) with a **fiat-receivable** state tax ([`TaxReceivability::FiatOnly`]):
    /// the fiat-credit capitalist holding idle fiat must remit it to the state, so fiat
    /// circulates through the **fiscal** channel (`tax_receipts_fiat > 0`) even though
    /// the **labor** channel refused it — the state compels what the market would not.
    /// Paired with [`Self::tax_in_specie`] it is the milestone's falsification twin: the
    /// *only* difference is the receivability, so the compelled fiat demand is isolated
    /// to the gate (not the levy, which is identical, or the spatial dynamics). Routes
    /// econ's unchanged M21 tax machinery; adds no tax logic to econ.
    pub fn tax_in_fiat() -> Self {
        Self {
            tax: Some(TaxPolicy::counter_lever(TaxReceivability::FiatOnly)),
            ..Self::wage_refusal_cycle()
        }
    }

    /// The G8c-3 **tax-in-specie** control — the falsification twin of
    /// [`Self::tax_in_fiat`]. The same wage-refusal cycle and the *same* levy set, but a
    /// **specie-receivable** state tax ([`TaxReceivability::SpecieOnly`]): the
    /// specie-holding trader remits specie (`tax_receipts_specie > 0`) and **no** fiat
    /// is compelled (`tax_receipts_fiat == 0`). Paired with the headline this proves the
    /// compelled fiat demand comes from the **receivability** policy: if the control
    /// showed fiat receipts, the gate would not be routing settlement. The fiat-holder's
    /// levy is unmet-by-rule under specie receivability (a default, conserved — not a
    /// leak), exactly as the headline's specie-holder defaults under fiat receivability.
    pub fn tax_in_specie() -> Self {
        Self {
            tax: Some(TaxPolicy::counter_lever(TaxReceivability::SpecieOnly)),
            ..Self::wage_refusal_cycle()
        }
    }

    /// The G8c-2 **spot-tender bench** — the public spot market (M11) wired as the
    /// same config lever. A finance settlement on econ's fiat-displacement scenario:
    /// the issuer prints fiat the first receivers then try to spend on goods.
    /// `FiatAndSpecie` lets the held fiat settle those spot trades; `SpecieOnly`
    /// refuses it and specie settles instead — composition changes, the printed fiat
    /// and the specie base do not. Pass the spot tender to demonstrate either side.
    pub fn spot_tender_bench(spot: PublicSpotTender) -> Self {
        Self {
            m3: true,
            tender_bench: Some(TenderBench {
                surface: BenchSurface::Spot,
                tender: TenderPolicy {
                    spot,
                    ..TenderPolicy::default()
                },
            }),
            ..Self::finance_base()
        }
    }

    /// The G8c-2 **debt-tender bench** — public debt discharge (M12) wired as the same
    /// config lever. A finance settlement on econ's fiat-displacement scenario with a
    /// seeded commodity debt (spot stays specie-only, so the debtor must hold its fiat
    /// for the debt surface). `FiatAndSpecie` lets the debt be paid in fiat;
    /// `SpecieOnly` refuses it and it is paid in specie — composition changes, totals
    /// do not. Pass the debt tender to demonstrate either side.
    pub fn debt_tender_bench(debt: PublicDebtTender) -> Self {
        Self {
            m3: true,
            tender_bench: Some(TenderBench {
                surface: BenchSurface::Debt,
                tender: TenderPolicy {
                    debt,
                    ..TenderPolicy::default()
                },
            }),
            ..Self::finance_base()
        }
    }

    /// The G8c-2 **bank-repayment tender bench** — bank-loan repayment (M15) wired as
    /// the same config lever. A finance settlement on econ's suspended-redemption
    /// repayment proof: the borrower holds a bank claim. `BankClaimsAndSpecie` lets
    /// that claim repay and retire bank credit; `SpecieOnly` refuses it, so the debt
    /// defaults with the claim still held. Pass the bank-repayment tender to
    /// demonstrate either side.
    pub fn bank_repayment_tender_bench(bank_repayment: BankRepaymentTender) -> Self {
        Self {
            m3: true,
            tender_bench: Some(TenderBench {
                surface: BenchSurface::BankRepayment,
                tender: TenderPolicy {
                    bank_repayment,
                    ..TenderPolicy::default()
                },
            }),
            ..Self::finance_base()
        }
    }

    /// The G8c-2 **issuer-repayment tender bench** — issuer-credit repayment (M16)
    /// wired as the same config lever. A finance settlement on econ's fiat-credit
    /// repayment proof: the borrower holds fiat from issuer credit. `FiatOnly` retires
    /// the returned fiat; `FiatRefused` refuses it, so the debt defaults and the credit
    /// overhang remains. Pass the issuer-repayment tender to demonstrate either side.
    pub fn issuer_repayment_tender_bench(issuer_repayment: IssuerRepaymentTender) -> Self {
        Self {
            m3: true,
            tender_bench: Some(TenderBench {
                surface: BenchSurface::IssuerRepayment,
                tender: TenderPolicy {
                    issuer_repayment,
                    ..TenderPolicy::default()
                },
            }),
            ..Self::finance_base()
        }
    }

    /// The shared spatial shell of a finance (`cycle`) settlement: an empty colony
    /// (no gatherers/consumers/nodes/chain/demography/barter). The cycle runs in the
    /// econ society the finance branch of [`Settlement::generate`] installs, so the
    /// spatial knobs are inert; this just gives the config a valid, colony-free base.
    fn finance_base() -> Self {
        Self {
            gatherers: 0,
            consumers: 0,
            nodes: Vec::new(),
            ..Self::viable()
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
            forecast_bias_base_bps: FORECAST_BIAS_NEUTRAL_BPS,
            forecast_bias_jitter_bps: FORECAST_BIAS_GEN_JITTER_DEFAULT,
            dynamics: NeedDynamics::lab_default(),
            resident_traders: Vec::new(),
            chain: Some(chain),
            demography: None,
            barter: None,
            m3: false,
            bank: None,
            cycle: None,
            tender_bench: None,
            tax: None,
            closed_circulation: false,
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
            forecast_bias_base_bps: FORECAST_BIAS_NEUTRAL_BPS,
            forecast_bias_jitter_bps: FORECAST_BIAS_GEN_JITTER_DEFAULT,
            dynamics: NeedDynamics::lab_default(),
            resident_traders: Vec::new(),
            chain: Some(chain),
            demography: None,
            barter: None,
            m3: false,
            bank: None,
            cycle: None,
            tender_bench: None,
            tax: None,
            closed_circulation: false,
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
            forecast_bias_base_bps: FORECAST_BIAS_NEUTRAL_BPS,
            forecast_bias_jitter_bps: FORECAST_BIAS_GEN_JITTER_DEFAULT,
            dynamics: NeedDynamics::lab_default(),
            resident_traders: Vec::new(),
            chain: Some(chain),
            demography: None,
            barter: None,
            m3: false,
            bank: None,
            cycle: None,
            tender_bench: None,
            tax: None,
            closed_circulation: false,
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
            forecast_bias_base_bps: FORECAST_BIAS_NEUTRAL_BPS,
            forecast_bias_jitter_bps: FORECAST_BIAS_GEN_JITTER_DEFAULT,
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
                cycle_producer_medium_endowment: 0,
                salt_direct_use_qty: 0,
                salt_direct_use_period: 0,
            }),
            bank: None,
            cycle: None,
            tender_bench: None,
            tax: None,
            closed_circulation: false,
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
            // S13 off by default: the shipped frontier keeps econ-only lineages, so the
            // frontier golden is byte-identical. `frontier_spatial_households` flips it.
            spatial_households: false,
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
            forecast_bias_base_bps: FORECAST_BIAS_NEUTRAL_BPS,
            forecast_bias_jitter_bps: FORECAST_BIAS_GEN_JITTER_DEFAULT,
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
                cycle_producer_medium_endowment: 0,
                salt_direct_use_qty: 0,
                salt_direct_use_period: 0,
            }),
            bank: None,
            cycle: None,
            tender_bench: None,
            tax: None,
            closed_circulation: false,
        }
    }

    /// S22e — expand the lineage roster + the matched demand side PROPORTIONALLY to
    /// [`ENDOWED_ROSTER_HOUSEHOLDS`] households. The 2-household cultivation base cannot host a
    /// `PERSIST_COHORT` (4) owner-lineage cohort, so the endowed-capital headline + every matched
    /// control run on this expanded base. The expansion preserves the colony's economic structure
    /// so money + mortality still hold:
    /// - the lineage households are replicated round-robin from the base specs (so the
    ///   time-preference spread + the WOOD-poor / `starting_food = 0` cold-start character carry
    ///   over), reaching `ENDOWED_ROSTER_HOUSEHOLDS`;
    /// - the non-lineage woodcutters (`gatherers`) and SALT-rich buyers (`consumers`) scale by the
    ///   SAME integer factor (roster ÷ base households), preserving the cultivator:woodcutter:consumer
    ///   ratio and the S21h survival floor (a per-agent threshold, so it scales automatically);
    /// - the grain commons and the WOOD node regen/stock/cap scale by that factor, so per-capita
    ///   food + warmth supply (and thus the cultivated-bread flow) is preserved.
    ///
    /// Purely a roster/supply rescale: it touches no gate flag, so applied to the gate-off S22d base
    /// it yields the EXPANDED S22d base (the matched churn baseline + the precondition colony), and
    /// the headline simply flips the endowment gate on top of it.
    fn expanded_endowment_roster(mut self) -> Self {
        let target = ENDOWED_ROSTER_HOUSEHOLDS;
        let demo = self
            .demography
            .as_mut()
            .expect("the cultivation base carries a demography overlay");
        let base = demo.households.clone();
        let base_len = base.len();
        assert!(
            base_len > 0 && target.is_multiple_of(base_len) && target >= base_len,
            "the expanded roster must be a whole multiple of the base household count"
        );
        let factor = (target / base_len) as u16;
        demo.households = (0..target).map(|i| base[i % base_len].clone()).collect();
        // Scale the matched non-lineage demand/supply side by the same factor (ratios preserved).
        self.gatherers = self.gatherers.saturating_mul(factor);
        self.consumers = self.consumers.saturating_mul(factor);
        // Scale the grain commons + the WOOD node so per-cultivator food + warmth supply is held
        // constant under the larger roster (the grain commons regen sets the cultivated-bread flow).
        let factor32 = u32::from(factor);
        let grain = self
            .chain
            .as_ref()
            .expect("the cultivation base carries a chain")
            .content
            .grain();
        for node in self.nodes.iter_mut() {
            if node.good == grain || node.good == WOOD {
                node.stock = node.stock.saturating_mul(factor32);
                node.regen = node.regen.saturating_mul(factor32);
                node.cap = node.cap.saturating_mul(factor32);
            }
        }
        self
    }

    fn with_private_land_layout(&self) -> Self {
        let mut cfg = self.clone();
        cfg.apply_private_land_layout();
        cfg
    }

    fn apply_private_land_layout(&mut self) {
        let Some(chain) = self.chain.as_ref() else {
            return;
        };
        let grain = chain.content.grain();
        self.width = private_land_layout_width(chain.land_good_plots, chain.land_marginal_plots)
            .expect("private land layout exceeds the maximum grid width");
        self.height = 1;
        self.exchange = Pos::new(0, 0);

        let mut used_x = BTreeSet::new();
        used_x.insert(0u16);
        let width = self.width;
        let place_x = |preferred: u16, used_x: &mut BTreeSet<u16>| -> u16 {
            if preferred < width && !used_x.contains(&preferred) {
                used_x.insert(preferred);
                return preferred;
            }
            for x in 1..width {
                if !used_x.contains(&x) {
                    used_x.insert(x);
                    return x;
                }
            }
            panic!("private land layout requires fewer occupied x positions than grid tiles");
        };

        // Good plots occupy the near band; marginal plots are generated outward at fixed spacing.
        // The grid width above is derived from the farthest generated plot plus a margin, so large
        // land-count cells remain actual open-entry tests rather than strip-cap artifacts.
        let mut nodes = Vec::new();
        for i in 0..chain.land_good_plots {
            let preferred = u16::try_from(u32::from(LAND_GOOD_START_X) + u32::from(i))
                .expect("private land good plot exceeds the maximum grid width");
            let x = place_x(preferred, &mut used_x);
            nodes.push(NodeSpec {
                good: grain,
                pos: Pos::new(x, 0),
                stock: LAND_GOOD_CAP,
                regen: LAND_GOOD_REGEN,
                cap: LAND_GOOD_CAP,
            });
        }
        for i in 0..chain.land_marginal_plots {
            let preferred = private_land_marginal_x(chain.land_good_plots, i)
                .expect("private land marginal plot exceeds the maximum grid width");
            let x = place_x(preferred, &mut used_x);
            nodes.push(NodeSpec {
                good: grain,
                pos: Pos::new(x, 0),
                stock: LAND_MARGINAL_CAP,
                regen: chain.land_marginal_regen,
                cap: LAND_MARGINAL_CAP,
            });
        }

        for original in self.nodes.iter().copied().filter(|node| node.good != grain) {
            let x = if original.pos.y == 0
                && original.pos.x < width
                && !used_x.contains(&original.pos.x)
            {
                used_x.insert(original.pos.x);
                original.pos.x
            } else {
                place_x(original.pos.x.min(width.saturating_sub(1)), &mut used_x)
            };
            nodes.push(NodeSpec {
                pos: Pos::new(x, 0),
                ..original
            });
        }
        self.nodes = nodes;
    }

    /// S22f — turn the voluntary fixed-term cultivation commitment gate on with the shipped term +
    /// entry floor (the term defaults to [`COMMITMENT_TERM_DEFAULT`], well below
    /// `ceil(PERSIST_FRACTION × FINAL_WINDOW)` so a single term cannot span the persistence window).
    /// A pure gate flip — it adds no capital and changes no other lever, so the headline's stickiness
    /// (if any) is the commitment institution alone.
    fn with_voluntary_commitment(mut self) -> Self {
        if let Some(chain) = self.chain.as_mut() {
            chain.voluntary_cultivation_commitment = true;
            chain.commitment_term = COMMITMENT_TERM_DEFAULT;
            chain.commitment_entry_floor = COMMITMENT_ENTRY_FLOOR_DEFAULT;
        }
        self
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
    /// Goods **spoiled** this tick (EXPERIMENT): perishable holdings decayed out
    /// of existence by the inventory carrying-cost / spoilage phase — a real
    /// sink, like `consumed`. Empty unless spoilage is enabled
    /// ([`ChainConfig::perishable_decay_bps`]), so the conservation identity is
    /// unchanged on every other settlement.
    pub spoiled: BTreeMap<GoodId, u64>,
    /// S23e: bread regenerated into the finite rival subsistence commons this tick. This is a
    /// named source for the whole-system identity, distinct from node regen and from the death
    /// estate commons.
    pub subsistence_commons_regen: BTreeMap<GoodId, u64>,
    /// S23e: bread drawn out of the finite rival subsistence commons and immediately eaten by
    /// eligible residual-hunger agents. This is a pool sink / agent transfer diagnostic; the
    /// whole-system goods identity treats it as internal movement because the pool itself is
    /// included in [`Settlement::whole_system_total`].
    pub subsistence_commons_draw: BTreeMap<GoodId, u64>,
    pub subsistence_commons_stock_before: u64,
    pub subsistence_commons_stock_after: u64,
    pub subsistence_commons_cap: u64,
    pub subsistence_commons_phi_bps: u32,
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct FastLoopReport {
    deposited: BTreeMap<(AgentId, GoodId), u32>,
    foraged: BTreeSet<AgentId>,
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
    /// Units of `good` spoiled this tick (a sink). Zero unless spoilage is enabled.
    pub fn spoiled_of(&self, good: GoodId) -> u64 {
        self.spoiled.get(&good).copied().unwrap_or(0)
    }
    pub fn subsistence_commons_regen_of(&self, good: GoodId) -> u64 {
        self.subsistence_commons_regen
            .get(&good)
            .copied()
            .unwrap_or(0)
    }
    pub fn subsistence_commons_draw_of(&self, good: GoodId) -> u64 {
        self.subsistence_commons_draw
            .get(&good)
            .copied()
            .unwrap_or(0)
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
            let spoiled = self.spoiled_of(*good) as i128;
            let subsistence_commons_regen = self.subsistence_commons_regen_of(*good) as i128;
            after
                == before + regen + endowment + produced + subsistence_commons_regen
                    - consumed_as_input
                    - consumed
                    - promoted
                    - spoiled
        })
    }

    pub fn subsistence_commons_conserves(&self) -> bool {
        self.subsistence_commons_stock_after
            == self
                .subsistence_commons_stock_before
                .saturating_add(self.subsistence_commons_regen.values().copied().sum())
                .saturating_sub(self.subsistence_commons_draw.values().copied().sum())
            && self.subsistence_commons_stock_after <= self.subsistence_commons_cap
    }

    pub fn money_conserves(&self) -> bool {
        let promoted: u64 = self.promoted.values().copied().sum();
        self.total_gold_after_fast == self.total_gold_before_fast
            && self.total_gold_after_step == self.total_gold_before_fast.saturating_add(promoted)
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

/// Diagnostic: the live money BID/ASK intent for one good from one vocation,
/// reconstructed from each living colonist's reservation price (the order it
/// *would* post). `best_bid`/`best_ask` are `None` when no colonist of that
/// vocation would post that side. A bid and ask **cross** (would trade) when
/// `best_bid >= best_ask`. Used to localize the input-market halt: do millers
/// bid for grain, do gatherers ask for grain, and do they cross?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderStat {
    pub vocation: Vocation,
    pub bidders: usize,
    pub best_bid: Option<u64>,
    pub askers: usize,
    pub best_ask: Option<u64>,
}

/// S8.0 emergence probe: one money candidate's accumulated barter saleability —
/// the acceptances it has won and the breadth of distinct acceptors and
/// counterpart goods behind them (the volume + breadth econ's Mengerian promotion
/// rule reads). A read-only view of the saleability tracker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateAcceptances {
    pub good: GoodId,
    pub acceptances: u64,
    pub acceptor_agents: usize,
    pub counterpart_goods: usize,
    /// S9: of `acceptances`, the count taken INDIRECTLY (`IndirectFor`), plus the
    /// distinct indirect acceptor agents and distinct indirect target goods behind
    /// them — the real indirect-exchange breadth the strong-bar gate reads.
    pub indirect_acceptances: u64,
    pub indirect_acceptor_agents: usize,
    pub indirect_target_goods: usize,
}

/// S19 emergence probe: direct vs indirect acceptances by candidate good. This is
/// derived from [`CandidateAcceptances`] (`direct = total - indirect`) so it reads the
/// same saleability tracker without adding runtime state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirectIndirectAcceptances {
    pub good: GoodId,
    pub total: u64,
    pub direct: u64,
    pub indirect: u64,
}

/// S21d.1: a read-only snapshot of the acquisition-channel ledger split across the four
/// channels — the units of tracked food (bread) consumed/held/credited by how they reached
/// the agent. The open-survival bar reads `consumed`: after warm-up, `bought` ≫
/// `seeded_minted` + `foraged`. `held`/`credited` surface seed depletion (the seeded channel
/// falling toward zero while bought rises).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcquisitionChannels {
    pub bought: u64,
    pub seeded_minted: u64,
    pub self_produced: u64,
    pub foraged: u64,
    pub commons: u64,
}

/// S22a: a read-only snapshot of the produced bread→SALT volume split by the PRODUCER's class
/// recorded at PRODUCTION time (spatial lineage vs non-lineage entrant), plus the distinct
/// producers per class. The entrant-class provenance the S22a classifier reads: whether the
/// food-producing class that monetized SALT formed from the pinned lineage, self-formed from
/// non-lineage agents under hunger, or both. See
/// [`Settlement::bread_for_salt_by_entrant_class`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntrantClassSale {
    /// Cumulative produced bread→SALT volume produced by spatial LINEAGE members (whole-run).
    pub lineage_volume: u64,
    /// Cumulative produced bread→SALT volume produced by NON-lineage entrants (whole-run).
    pub nonlineage_volume: u64,
    /// The same lineage split accumulated only over PRE-promotion ticks (frozen at the
    /// promotion tick) — the causality probe for whether lineage production drove a promotion.
    pub pre_promotion_lineage_volume: u64,
    /// The same non-lineage split over PRE-promotion ticks (frozen at the promotion tick).
    pub pre_promotion_nonlineage_volume: u64,
    /// Distinct LINEAGE producers whose `SelfProduced` bread reached a bread→SALT sale.
    pub lineage_sellers: usize,
    /// Distinct NON-lineage entrants whose `SelfProduced` bread reached a bread→SALT sale — the
    /// SUCCESS criterion "≥2 non-lineage entrants sell `SelfProduced` bread".
    pub nonlineage_sellers: usize,
}

impl AcquisitionChannels {
    fn from_array(channels: [u64; FoodChannel::COUNT]) -> Self {
        Self {
            bought: channels[FoodChannel::Bought.index()],
            seeded_minted: channels[FoodChannel::SeededMinted.index()],
            self_produced: channels[FoodChannel::SelfProduced.index()],
            foraged: channels[FoodChannel::Foraged.index()],
            commons: channels[FoodChannel::Commons.index()],
        }
    }

    /// Total units across all acquisition channels.
    pub fn total(&self) -> u64 {
        self.bought + self.seeded_minted + self.self_produced + self.foraged + self.commons
    }
}

/// S21d.2a: a read-only snapshot of the cross-tick bootstrap microtrace — the buy → eat → bid
/// sequence over the run, and WHY a fed producer with a positive project-input ceiling does not
/// get a real order-book bid. The classification reads it to localize the Exp-9 gate: a Phase B
/// deadlock shows `bids_posted_after_recent_buy = 0` with the blocks split into `cashless` (no
/// money earned) vs `reserved` (money present but unavailable to this bid).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BootstrapTraceSummary {
    pub food_buys: u64,
    pub food_eats: u64,
    pub bid_attempts: u64,
    pub bids_posted: u64,
    pub bids_posted_after_recent_buy: u64,
    pub bids_blocked_cashless: u64,
    pub bids_blocked_reserved: u64,
    pub first_bootstrap_bid_tick: Option<u64>,
}

/// C3R.j (impl-75): how the census candidate came by its oven. Measured, not
/// assumed — this base has `producible_capital`, so an oven may be self-built, and a
/// self-builder mislabeled "seeded" would misfire the identity axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolProvenance {
    /// A seeded latent Baker (`Colonist::latent == Some(RecipeId::Bake)`).
    SeededLatent,
    /// The oven arrived by producer estate settlement (`producer_tool_inheritors`).
    Inherited,
    /// Holds an oven, is neither seeded nor a recorded inheritor, and carries the
    /// existing `acquired_tool` marker set on self-build completion. That marker is
    /// good-AGNOSTIC (set on any capital build, `mod.rs:9176`), so this reads "holds
    /// an oven and has self-built some tool", not "self-built THIS oven".
    SelfBuilt,
    /// None of the seeded, recorded-inheritance, or self-build evidence applies.
    Other,
}

/// One non-self colonist in the flour re-ignition census.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlourCensusColonist {
    pub id: AgentId,
    pub vocation: Vocation,
    pub flour_held: u32,
    /// The executable flour ask.
    pub reservation_ask: Option<Gold>,
    /// C3R.j: WHICH exit the ask rule took — the decomposition of the `None` above.
    /// `reservation_ask` is this value's `Some/None` projection, by construction.
    pub ask_outcome: AskOutcome,
}

/// One living Miller in the flour re-ignition census.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlourCensusMiller {
    pub id: AgentId,
    pub gold: u64,
    pub grain_held: u32,
    pub flour_held: u32,
}

/// Read-only state captured at an armed `InputPriceAbsent` Bake decline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlourCensusRow {
    pub decline_tick: u64,
    /// `mortal_producer_old_age_deaths` at the decline.
    pub deaths_before_decline: u64,
    pub candidate_id: AgentId,
    pub candidate_own_flour: u32,
    /// C3R.j identity axis. `None` for a non-colonist appraiser id.
    pub candidate_vocation: Option<Vocation>,
    pub candidate_holds_oven: bool,
    pub candidate_provenance: ToolProvenance,
    /// C3R.j: raw membership in `producer_tool_inheritors` for the oven, INDEPENDENT of
    /// [`ToolProvenance`]'s precedence. A seeded latent Baker that also inherited an oven
    /// reports `SeededLatent`, so the enum alone would silently mask an heir on the
    /// identity axis — the axis reads this too.
    pub candidate_recorded_inheritor: bool,
    /// Every other living colonist, in live-roster order.
    pub colonists: Vec<FlourCensusColonist>,
    pub commons_flour: u64,
    pub millers: Vec<FlourCensusMiller>,
    pub bootstrap_trace_active: bool,
    /// Zero defaults carry no evidence unless `bootstrap_trace_active`.
    pub bootstrap: BootstrapTraceSummary,
}

/// S21e.0: runtime-only provenance row for a bread seller observed in the mints-on
/// control or seeded-surplus diagnostic. This is a read-only trace of realized
/// settlement-level barter, not canonical state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BreadSellerProvenance {
    pub tick: u64,
    pub seller: AgentId,
    pub buyer: AgentId,
    pub seller_vocation: Option<Vocation>,
    pub buyer_vocation: Option<Vocation>,
    pub seller_household: Option<usize>,
    pub buyer_household: Option<usize>,
    pub bread_good: GoodId,
    pub received_good: GoodId,
    pub qty: u32,
    pub reason: BarterReason,
}

/// S21e.1: runtime-only summary for the finite seeded-surplus non-vacuity and
/// exhaustion gates. The fields are derived from the actual barter-preservation
/// helper and settlement-level offer/trade traces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeededSurplusTraceSummary {
    pub max_pre_promotion_seeded_sellers: usize,
    pub first_non_vacuous_tick: Option<u64>,
    pub cleared_bread_salt_indirect_for_wood: u64,
    pub live_bread_salt_indirect_for_wood_ticks: u64,
    pub seeded_offerable_surplus_exhausted_tick: Option<u64>,
}

/// S8.0 emergence probe: a chain producer's role for the working-capital trace —
/// an ACTIVE producer already running its recipe, or a LATENT one (an
/// `Unassigned` colonist holding the tool, waiting on money to adopt). Tension B
/// asks whether a latent producer holds free gold the tick after promotion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProducerRole {
    Miller,
    Baker,
    LatentMiller,
    LatentBaker,
}

impl ProducerRole {
    /// `true` for the latent (pre-adoption) roles — the producers Tension B turns
    /// on (they must hold free gold the tick after promotion to win an input bid).
    pub fn is_latent(self) -> bool {
        matches!(self, ProducerRole::LatentMiller | ProducerRole::LatentBaker)
    }
}

/// S8.0 emergence probe: one chain producer's working capital, the heart of
/// Tension B — the barter MEDIUM (SALT) it holds pre-promotion, its GOLD, and its
/// FREE (non-reserved) gold available to fund an input bid. Read right after the
/// promotion tick, a latent producer's `gold` is exactly its converted-SALT
/// capital (it held no gold before promotion) and `free_gold` is what it can put
/// behind its input bid before the chain freezes (the S8.2 cutover metric).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProducerCash {
    pub role: ProducerRole,
    pub medium: u64,
    pub gold: u64,
    pub free_gold: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommitmentNormCopyDriver {
    Alive,
    HungerRelief,
    FoodConsumed,
    SaltStock,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommitmentNormCopyRow {
    pub tick: u64,
    pub copier: u64,
    pub model: u64,
    pub copied_norm_bit: bool,
    pub driver: CommitmentNormCopyDriver,
    pub copier_score_bps: u64,
    pub model_score_bps: u64,
    pub positive_pre_copy_advantage: bool,
    pub adopter_share_gap_bps: i64,
    pub group_imitation: bool,
    pub aligned_group_adoption_pre_core: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommitmentNormFlipRow {
    pub tick: u64,
    pub agent: u64,
    pub from: bool,
    pub to: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CommitmentNormObservation {
    tick: u64,
    hunger: u16,
    food_consumed: u32,
    salt_stock: u64,
    at_market: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CommitmentNormScore {
    alive_bps: u64,
    hunger_bps: u64,
    food_bps: u64,
    salt_bps: u64,
    total_bps: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CommitmentNormGroupCandidate {
    center_id: AgentId,
    score: CommitmentNormScore,
    adopter_share_bps: u64,
}

struct Colonist {
    id: AgentId,
    vocation: Vocation,
    /// The node a gatherer harvests (round-robin over the config's nodes).
    node: Option<NodeId>,
    /// S6 re-entry hysteresis: the colonist's **home** vocation+node, captured at
    /// generation. Re-entry temporarily overrides `vocation`/`node` to send a hungry
    /// non-lineage colonist to the edible grain node; once it is fed again it reverts
    /// to this home role (a WOOD gatherer resumes WOOD, an idle consumer goes idle).
    /// Immutable after generation. Because it decides the revert *target*, it is part of
    /// the future-behaviour identity: [`Settlement::canonical_bytes`] serializes it
    /// whenever re-entry can run (gated on [`ChainConfig::productive_reentry`]), so two
    /// states with identical current `vocation`/`node` but different homes digest apart.
    /// A re-entry-OFF run never reads it and never serializes it, keeping the pre-S6
    /// per-colonist layout byte-identical.
    home_vocation: Vocation,
    home_node: Option<NodeId>,
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
    /// S23c: the stable parent id for a newborn, if this colonist was born inside the
    /// simulation. Founders and pre-demography colonists have no parent. Read only by the
    /// secure-tenure universal-heir order and serialized only when that mode is active.
    parent: Option<AgentId>,
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
    /// S7 observability (read-only, NOT serialized): set `true` the tick a colonist
    /// that was NOT seeded latent (`latent == None`) comes to hold a **produced**
    /// chain tool it built itself (S7.2). It lets a test tie a produced tool to a
    /// formerly-non-latent adopter (acceptance test 6) without inferring it from
    /// holdings. Purely diagnostic — no phase reads it, so it steers no future tick
    /// and is deliberately absent from [`Settlement::canonical_bytes`] (a derived
    /// marker, not behaviour state). Deterministic: set on a deterministic build
    /// completion.
    acquired_tool: bool,
    /// S12 own-labor subsistence: `true` while this colonist is foraging the FORAGE
    /// node for its own-consumption subsistence floor. Set by `run_own_labor_subsistence`
    /// (a hungry, eligible, unprovisioned colonist with spare labor) and cleared once it
    /// is fed (the hysteresis). It STEERS the next fast loop: a spatial forager is
    /// assigned [`Task::GoForage`] instead of harvesting WOOD (the structural
    /// opportunity cost). Part of the future-behaviour identity whenever the
    /// `own_labor_subsistence` phase can run; serialized only under that gate, so a
    /// flag-off colonist block stays byte-identical to pre-S12. Always `false` for a
    /// non-own-labor settlement.
    foraging: bool,
    /// S15 own-use cultivation: `true` while this colonist (still hungry after foraging)
    /// is cultivating bread by own labor — it STEERS the next fast loop to GoHarvest the
    /// grain node (instead of foraging/WOOD, the structural opportunity cost) so the
    /// cultivation phase can convert that grain to bread. **Mutually exclusive with
    /// [`Self::foraging`]** (one world task per econ tick — never both). Set/cleared by
    /// `run_own_labor_subsistence`'s second tier; part of the future-behaviour identity
    /// whenever the cultivation phase can run, serialized only under that gate so a
    /// flag-off colonist block stays byte-identical. Always `false` off the cultivation
    /// path.
    cultivating: bool,
    /// S15: the cultivation **pressure** — the count of CONSECUTIVE econ ticks this
    /// colonist's hunger has stayed at/above `cultivate_hunger_in`. It resets the moment
    /// hunger drops below that threshold (forage caught up), and once it reaches
    /// `cultivate_patience` the colonist escalates to cultivation. The sustained-hunger
    /// gate that keeps a transient forage-haul spike from triggering cultivation (so the
    /// escape valve fires only under real scarcity). Steers the cultivation decision, so
    /// it is part of the future-behaviour identity; serialized only on the active
    /// cultivation path, so a flag-off colonist block stays byte-identical. `0` off the
    /// cultivation path.
    cultivate_pressure: u16,
    /// S15: `true` after this colonist has actually entered the cultivation steering
    /// path and may still have own-use grain stock to drain after the `cultivating`
    /// steering flag clears. This preserves delayed harvest deposits without letting
    /// unrelated grain holders run the own-use recipe. Serialized only on the active
    /// cultivation path because it steers future production.
    cultivation_stock_pending: bool,
    /// S22b own-use cultivation **skill** — a bounded, earned-not-inherited scalar (born `0`)
    /// that ACCUMULATES by [`ChainConfig::skill_gain`] on a tick of realized cultivation output
    /// (grain actually harvested AND converted to bread) and DECAYS by
    /// [`ChainConfig::skill_decay`] on any tick without it, saturating at
    /// [`ChainConfig::skill_cap`]. It STEERS the next grain trip's per-trip haul capacity (the
    /// [`world::Task::GoHarvestWithRoom`] override), so it is part of the future-behaviour
    /// identity whenever the cultivation-skill phase can run; serialized only under that gate
    /// (mirroring the `cultivating`/`cultivate_pressure` block), so a flag-off colonist block
    /// stays byte-identical. `0` off the cultivation-skill path.
    cultivation_skill: u16,
    /// S22c own-use cultivation **return window** — a rolling [`RETURN_WINDOW`]-tick FIFO of this
    /// colonist's realized post-money cultivation-sale vs non-cultivation-sale SALT proceeds (one
    /// [`ReturnTick`] per tick it realized SOME sale). It STEERS the cultivation *exit*: when
    /// [`Settlement::profit_stay_active`] reads a clearing realized return it keeps the colonist
    /// cultivating past the normal hunger exit, so it is part of the future-behaviour identity
    /// whenever the profit-driven-retention phase can run; serialized only under that gate
    /// (mirroring the `cultivation_skill` block), so a flag-off colonist block stays
    /// byte-identical. Empty off the profit-driven-retention path (and pre-money, where there are
    /// no SALT sales to credit).
    cultivation_return_window: VecDeque<ReturnTick>,
    /// S22d own-use cultivation **tenure** — a streak of consecutive econ ticks on which this
    /// colonist realized cultivation output (grain harvested AND converted to bread), credited by
    /// [`Settlement::run_cultivation_capital_formation`] and RESET on any tick without realized
    /// output. Distinct from `cultivate_pressure` (a hunger-ENTRY streak that resets once eating
    /// works): tenure measures sustained PRODUCING cultivation. It STEERS the tool-build decision
    /// (a cultivator invests once tenure ≥ [`ChainConfig::tool_build_patience`]), so it is part of
    /// the future-behaviour identity whenever the durable-cultivation-capital phase can run;
    /// serialized only under that gate (nested in the cultivation block, mirroring the
    /// `cultivation_skill` block), so a flag-off colonist block stays byte-identical. `0` off the
    /// durable-cultivation-capital path.
    cultivation_tenure: u16,
    /// S22f own-use cultivation **commitment remaining** — the econ ticks left in this colonist's
    /// current voluntary commitment term (`0` = uncommitted). Set to [`ChainConfig::commitment_term`]
    /// on a voluntary opt-in (its realized cultivation-return signal cleared
    /// [`ChainConfig::commitment_entry_floor`] vs its outside option, post-money), decremented once
    /// per econ tick, and reset to `0` deterministically on expiry or when the agent leaves the
    /// S22a-eligible set (death / becoming an active specialized producer). While `> 0` it STEERS the
    /// cultivation EXIT (the binding overrides the normal hunger/profit exit), so it is part of the
    /// future-behaviour identity whenever the commitment phase can run; serialized only under that
    /// gate (nested in the cultivation block, mirroring the `cultivation_tenure` block), so a flag-off
    /// colonist block stays byte-identical. `0` off the voluntary-commitment path.
    commitment_remaining: u16,
    /// S22f own-use cultivation **commitment renewals** — the count of times this colonist re-committed
    /// from a FRESH post-expiry signal (the first opt-in is not a renewal). It does not steer the
    /// cultivate decision directly, but is part of the commitment future-behaviour identity (the S22c/
    /// S22e discipline digests the steering state ON-only) and is read by the success bar (every
    /// persistent committed id needs ≥1 renewal so persistence is RE-CHOSEN, not one mega-term).
    /// Serialized only under the gate (nested in the cultivation block), so a flag-off colonist block
    /// stays byte-identical. `0` off the voluntary-commitment path / for a never-renewed committer.
    commitment_renewals: u16,
    adopts_commitment_norm: bool,
    next_norm_bit: Option<bool>,
    commitment_norm_seed_adopter: bool,
    commitment_norm_observations: VecDeque<CommitmentNormObservation>,
    /// S23a private land tenure: the grain plot this colonist is currently hauling from. Set by the
    /// post-world-tick harvest event detector, kept through the GoDeposit trip and transfer retry,
    /// and cleared only after both carried grain and pending grain transfer are gone. It steers idle
    /// forfeiture, so it is serialized ON-only with the private-land gate.
    carried_grain_source: Option<NodeId>,
    /// C1R: the live share contract id that sourced the currently carried/pending grain.
    /// This is deliberately a term id, not just a node id, so an expired carry deposited
    /// after a same-plot renewal remains worker-owned instead of being attached to the
    /// new term.
    carried_share_contract_id: Option<u64>,
    /// C1N: the live in-kind wage contract id that sourced the currently carried/pending grain.
    /// Kept separate from C1R so a same-node wage/share transition cannot attach old carry to
    /// the wrong contract form.
    carried_in_kind_contract_id: Option<u64>,
}

// ============================================================================
// C3R.e-obs (impl-66): the allocation-contest instrumentation. PURE OBSERVATION —
// every type below is runtime-only diagnostic, NEVER serialized. The §2 join assigns
// exactly one outcome to each unfilled saving quote-opportunity by pinned precedence.
// ============================================================================

/// One saving quote-opportunity's outcome under the §2 pinned precedence (first match
/// wins). `Filled` is tracked by the caller; this enum covers the UNFILLED cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SavingLossOutcome {
    /// No live bid this tick (reservation None / gold clamp / over-reserve / post fail).
    NoBidPosted,
    /// Every staple ask in the window was the agent's own (≥1 self ask, no other).
    SelfAskOnly,
    /// A bid was live but no non-self staple ask existed anywhere in the window.
    NoExecutableAskInWindow,
    /// Non-self asks existed but every one's limit exceeded the saving bid's limit.
    AllAsksAboveLimit,
    /// A compatible non-self ask existed but its unit went to another buyer.
    CompetitiveLoss {
        basis: PriorityBasis,
        winner_intent: WinnerIntent,
    },
    /// Overflow/invalid-settlement rejection or any unreconciled case — never silently
    /// absorbed.
    ExecutionResidual,
}

/// How a `CompetitiveLoss` was decided (the two orthogonal sub-dimensions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PriorityBasis {
    /// Consumed BEFORE the saving bid entered the book (quote-order artifact).
    PreEntryOrder,
    /// Consumed while the bid was live, by a strictly higher-limit winner (price contest).
    HigherLimit,
    /// Consumed while live, equal limits, arrival order decided.
    EqualLimitEarlierSeq,
    /// Consumed after the saving bid was cancelled/exited intra-pass.
    PostExitConsumption,
}

/// The winner's first-unprovided want at its quote time — interpretive color only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum WinnerIntent {
    HungryNow,
    SavingNext,
    ProducerInputNext,
    Other,
}

// The five §2 diagnosis families (OfferScarcity / AllocationPriority / Microstructure /
// GoldBind / Residual) are exposed as the per-family COUNT accessors on
// `SavingAllocationObs` below (`offer_scarcity`, `allocation_priority`,
// `microstructure_loss`, `gold_bind`, `residual`); the acceptance suite forms shares and
// the >1/2 majority diagnosis from them. They partition `unfilled()` by construction.

/// C3R.e-obs (§5): one money-priced spot tick's supply-side telemetry — the two spec
/// series (`offerable_bread_series` + `posted_asks_series`) joined by tick. It lets a
/// reader of the OfferScarcity family separate genuine offer scarcity (no offerable staple
/// anywhere) from quote-generation failure (offerable staple is held but few/no asks are
/// posted), which select DIFFERENT levers. Runtime-only diagnostic; NEVER serialized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavingSupplyTick {
    pub tick: u64,
    /// `offerable_bread_series` by seller class: potential sellers who COULD post a
    /// ≥1-unit staple ask this tick — the pure `reservation_ask_for_money(staple, 1,
    /// money_good)` predicate the live ask path uses (agent.rs:443), evaluated
    /// counterfactually on the PRE-market state (ADVISORY per §3.2a, since the live path
    /// evaluates an available_agent with other reserves removed). Split by whether the
    /// seller is a member of this tick's C3R.d attribution snapshot.
    pub offerable_sellers_member: u64,
    pub offerable_sellers_other: u64,
    /// `posted_asks_series`: distinct staple asks actually live in the window (the
    /// post-cancellation pass-start book snapshot plus asks freshly posted during the pass,
    /// deduped by seq) — trace-AUTHORITATIVE.
    pub posted_asks: u64,
}

/// Runtime-only per-run accumulator for the §2 loss decomposition. NEVER serialized.
#[derive(Clone, Debug, Default)]
pub struct SavingAllocationObs {
    /// C3R.g runtime-only role-choice telemetry. Kept in this existing defaulted,
    /// non-digested diagnostics store so settlement construction stays unchanged.
    role_choice_diag: RoleChoiceDiag,
    /// C3R.h cut-2 runtime-only Baker-class round-trip telemetry.
    baker_round_trip: BakerRoundTrip,
    /// Opportunities that Filled (a staple bought under the attribution predicate).
    pub filled: u64,
    pub no_bid_posted: u64,
    pub self_ask_only: u64,
    pub no_executable_ask_in_window: u64,
    pub all_asks_above_limit: u64,
    /// CompetitiveLoss cells keyed by (priority basis, winner intent).
    competitive_loss: BTreeMap<(PriorityBasis, WinnerIntent), u64>,
    pub execution_residual: u64,
    /// Ticks with ≥1 eligible member but NO money-priced spot pass (excluded from the
    /// opportunity domain; reported separately per §2).
    pub no_spot_pass_ticks: u64,
    /// Opportunities whose staple bid QuoteAttempt was absent from the trace (logged, not
    /// silently absorbed — folded into ExecutionResidual for totality).
    pub drops: u64,
    /// Staple physical-stock reconciliation (§3.2c) — aggregate over the run.
    pub phys_produced: u64,
    pub phys_consumed: u64,
    pub phys_net_delta: i64,
    /// The post-reconciliation residual (produced − consumed vs the measured net delta):
    /// the `WithinPhaseAmbiguous` term, reported never guessed.
    pub phys_within_phase_ambiguous: i64,
    /// Physical-stock and reservation deltas at the pinned settlement phase seams.
    /// Reservations are diagnostic color only and never folded into physical stock.
    pub death_phase: SavingStockPhaseObs,
    pub pre_market_phase: SavingStockPhaseObs,
    pub market_phase: SavingStockPhaseObs,
    pub production_own_use_phase: SavingStockPhaseObs,
    pub birth_phase: SavingStockPhaseObs,
    pub end_of_tick_phase: SavingStockPhaseObs,
    /// §5 supply-side series — per money-priced spot tick, the offerable staple supply
    /// (`offerable_bread_series`, by seller class) joined with the posted asks
    /// (`posted_asks_series`). Disambiguates the OfferScarcity family (true scarcity vs
    /// quote-generation failure); read by the suite, printed never asserted. Runtime-only,
    /// NEVER serialized.
    pub supply_series: Vec<SavingSupplyTick>,
}

/// Aggregate physical-stock attribution for one pinned settlement phase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SavingStockPhaseObs {
    pub physical_delta: i64,
    pub reservation_delta: i64,
    pub attributed_delta: i64,
    pub within_phase_ambiguous: i64,
}

impl SavingStockPhaseObs {
    fn record(&mut self, physical_delta: i64, reservation_delta: i64, attributed_delta: i64) {
        self.physical_delta += physical_delta;
        self.reservation_delta += reservation_delta;
        self.attributed_delta += attributed_delta;
        self.within_phase_ambiguous += physical_delta - attributed_delta;
    }
}

#[derive(Clone, Copy, Debug)]
struct SavingStockSnapshot {
    physical: u64,
    reserved: u64,
}

#[derive(Clone, Copy, Debug)]
struct SavingStockTick {
    staple: GoodId,
    previous: SavingStockSnapshot,
    produced: u64,
    endowment: u64,
    consumed_as_input: u64,
    market_consumed: u64,
}

#[derive(Clone, Copy)]
enum SavingStockPhase {
    Death,
    PreMarket,
    Market,
    ProductionOwnUse,
    Birth,
    EndOfTick,
}

impl SavingAllocationObs {
    fn record(&mut self, outcome: SavingLossOutcome) {
        match outcome {
            SavingLossOutcome::NoBidPosted => self.no_bid_posted += 1,
            SavingLossOutcome::SelfAskOnly => self.self_ask_only += 1,
            SavingLossOutcome::NoExecutableAskInWindow => self.no_executable_ask_in_window += 1,
            SavingLossOutcome::AllAsksAboveLimit => self.all_asks_above_limit += 1,
            SavingLossOutcome::CompetitiveLoss {
                basis,
                winner_intent,
            } => {
                *self
                    .competitive_loss
                    .entry((basis, winner_intent))
                    .or_insert(0) += 1;
            }
            SavingLossOutcome::ExecutionResidual => self.execution_residual += 1,
        }
    }

    /// The count of UNFILLED opportunities (the loss-decomposition denominator).
    pub fn unfilled(&self) -> u64 {
        self.no_bid_posted
            + self.self_ask_only
            + self.no_executable_ask_in_window
            + self.all_asks_above_limit
            + self.competitive_loss_total()
            + self.execution_residual
    }

    pub fn competitive_loss_total(&self) -> u64 {
        self.competitive_loss.values().sum()
    }

    /// CompetitiveLoss units whose basis is `HigherLimit` (the AllocationPriority family).
    pub fn allocation_priority(&self) -> u64 {
        self.competitive_loss
            .iter()
            .filter(|((basis, _), _)| *basis == PriorityBasis::HigherLimit)
            .map(|(_, count)| *count)
            .sum()
    }

    /// CompetitiveLoss units whose basis is EqualLimitEarlierSeq or PreEntryOrder.
    pub fn microstructure_loss(&self) -> u64 {
        self.competitive_loss
            .iter()
            .filter(|((basis, _), _)| {
                matches!(
                    basis,
                    PriorityBasis::EqualLimitEarlierSeq | PriorityBasis::PreEntryOrder
                )
            })
            .map(|(_, count)| *count)
            .sum()
    }

    /// CompetitiveLoss units routed to Residual (PostExitConsumption).
    pub fn post_exit_loss(&self) -> u64 {
        self.competitive_loss
            .iter()
            .filter(|((basis, _), _)| *basis == PriorityBasis::PostExitConsumption)
            .map(|(_, count)| *count)
            .sum()
    }

    pub fn offer_scarcity(&self) -> u64 {
        self.no_executable_ask_in_window + self.all_asks_above_limit
    }

    pub fn gold_bind(&self) -> u64 {
        self.no_bid_posted
    }

    pub fn residual(&self) -> u64 {
        self.execution_residual + self.self_ask_only + self.post_exit_loss()
    }

    /// The (priority_basis × winner_intent) CompetitiveLoss matrix, sorted for a stable
    /// print. Interpretive color; never a rule input.
    pub fn competitive_loss_matrix(&self) -> Vec<(PriorityBasis, WinnerIntent, u64)> {
        let mut rows: Vec<_> = self
            .competitive_loss
            .iter()
            .map(|((basis, intent), count)| (*basis, *intent, *count))
            .collect();
        rows.sort_by_key(|row| (row.0, row.1));
        rows
    }

    /// §5 supply-side run totals over `supply_series`: the money-priced spot ticks
    /// observed, the summed offerable-seller counts (member, other), the summed posted
    /// staple asks, and the count of ticks where offerable staple supply existed but NO ask
    /// was posted — the direct quote-generation-failure signal that keeps the OfferScarcity
    /// family from being read as genuine scarcity. Interpretive; never a rule input.
    pub fn supply_totals(&self) -> SavingSupplyTotals {
        let mut totals = SavingSupplyTotals {
            spot_ticks: self.supply_series.len() as u64,
            ..SavingSupplyTotals::default()
        };
        for row in &self.supply_series {
            totals.offerable_sellers_member += row.offerable_sellers_member;
            totals.offerable_sellers_other += row.offerable_sellers_other;
            totals.posted_asks += row.posted_asks;
            if row.offerable_sellers_member + row.offerable_sellers_other > 0
                && row.posted_asks == 0
            {
                totals.offerable_but_no_ask_ticks += 1;
            }
        }
        totals
    }
}

/// §5 supply-side run totals derived from [`SavingAllocationObs::supply_series`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SavingSupplyTotals {
    pub spot_ticks: u64,
    pub offerable_sellers_member: u64,
    pub offerable_sellers_other: u64,
    pub posted_asks: u64,
    /// Money-priced spot ticks with ≥1 offerable seller but zero posted staple asks.
    pub offerable_but_no_ask_ticks: u64,
}

/// C3R.e-obs (§5 `posted_asks_series`): the count of distinct staple asks live in the
/// opportunity window — the post-cancellation pass-start book snapshot plus asks freshly
/// posted during the pass, deduped by seq (a carried ask appears in both). Trace-
/// authoritative; counts every seller's staple ask (self-asks included — this is the
/// realized market ask supply, not a per-opportunity view).
fn count_window_staple_asks(records: &[AllocationRecord], staple: GoodId) -> u64 {
    let mut seqs: BTreeSet<u64> = BTreeSet::new();
    for record in records {
        match record {
            AllocationRecord::BookSnapshot {
                side: OrderSide::Ask,
                good,
                seq,
                ..
            } if *good == staple => {
                seqs.insert(*seq);
            }
            AllocationRecord::QuoteAttempt {
                good,
                side: OrderSide::Ask,
                outcome: QuoteOutcome::Posted { seq, .. },
                ..
            } if *good == staple => {
                seqs.insert(*seq);
            }
            _ => {}
        }
    }
    seqs.len() as u64
}

/// The §2 deterministic join for ONE saving quote-opportunity (a member's staple BID that
/// did NOT fill). Pure over the drained trace + intent map: the classifier reads no engine
/// state, so the whole join lives in this side-effect-free function (Risk #1). Returns the
/// single §2 outcome by pinned precedence.
fn classify_saving_opportunity(
    records: &[AllocationRecord],
    intent: &BTreeMap<u64, TracedWant>,
    staple: GoodId,
    member: AgentId,
    input_goods: &BTreeSet<GoodId>,
) -> SavingLossOutcome {
    // The quote loop must account for this member/good even when no order results.
    if !member_has_staple_bid_attempt(records, staple, member) {
        // No staple bid attempt recorded — unreconciled; folded into the residual, logged.
        return SavingLossOutcome::ExecutionResidual;
    }
    let bid_intervals = saving_bid_intervals(records, staple, member);
    if bid_intervals.is_empty() {
        return SavingLossOutcome::NoBidPosted;
    }

    // Every staple ask in the window: pass-start snapshot asks + asks posted during the
    // pass (carried asks are already in the snapshot; dedupe by seq).
    let mut asks: Vec<AskInWindow> = Vec::new();
    for record in records {
        match record {
            AllocationRecord::BookSnapshot {
                side: OrderSide::Ask,
                agent,
                good,
                limit,
                seq,
                ..
            } if *good == staple => asks.push(AskInWindow {
                agent: *agent,
                limit: *limit,
                seq: *seq,
            }),
            AllocationRecord::QuoteAttempt {
                agent,
                good,
                side: OrderSide::Ask,
                outcome: QuoteOutcome::Posted { seq, limit, .. },
                ..
            } if *good == staple => asks.push(AskInWindow {
                agent: *agent,
                limit: *limit,
                seq: *seq,
            }),
            _ => {}
        }
    }
    asks.sort_by_key(|ask| ask.seq);
    asks.dedup_by_key(|ask| ask.seq);
    let has_self_ask = asks.iter().any(|ask| ask.agent == member);
    asks.retain(|ask| ask.agent != member);

    if asks.is_empty() {
        // SelfAskOnly PRECEDES NoExecutable (round-2): ≥1 ask existed but all were self.
        return if has_self_ask {
            SavingLossOutcome::SelfAskOnly
        } else {
            SavingLossOutcome::NoExecutableAskInWindow
        };
    }

    let compatible: Vec<&AskInWindow> = asks
        .iter()
        .filter(|ask| {
            bid_intervals
                .iter()
                .any(|interval| ask.limit <= interval.limit)
        })
        .collect();
    if compatible.is_empty() {
        return SavingLossOutcome::AllAsksAboveLimit;
    }

    // For each compatible ask, find its consuming execution and classify the loss.
    let mut lost: Vec<(Gold, u64, SavingLossOutcome)> = Vec::new();
    for ask in &compatible {
        let compatible_bids: Vec<&BidInterval> = bid_intervals
            .iter()
            .filter(|interval| ask.limit <= interval.limit)
            .collect();
        let consumed = records
            .iter()
            .enumerate()
            .find_map(|(index, record)| match record {
                AllocationRecord::Execution {
                    incoming_seq,
                    resting_seq,
                    incoming_side,
                    good,
                    bid_limit,
                    status: AllocationExecutionStatus::Succeeded,
                    ..
                } if *good == staple
                    && ((*incoming_side == OrderSide::Bid && *resting_seq == ask.seq)
                        || (*incoming_side == OrderSide::Ask && *incoming_seq == ask.seq)) =>
                {
                    Some((
                        index,
                        *incoming_seq,
                        *resting_seq,
                        *incoming_side,
                        *bid_limit,
                    ))
                }
                _ => None,
            });
        let Some((exec_idx, incoming_seq, resting_seq, incoming_side, winner_limit)) = consumed
        else {
            // Compatible ask never consumed (still resting / a rejection skip): not a
            // loss to another buyer. Left for the residual reconciliation below.
            continue;
        };
        let winner_seq = match incoming_side {
            OrderSide::Bid => incoming_seq,
            OrderSide::Ask => resting_seq,
        };
        let rejected_saving_cross = records[..exec_idx].iter().any(|record| match record {
            AllocationRecord::Execution {
                incoming_seq,
                resting_seq,
                incoming_side,
                good,
                status: AllocationExecutionStatus::Rejected,
                ..
            } if *good == staple => match incoming_side {
                OrderSide::Bid => {
                    *resting_seq == ask.seq
                        && compatible_bids
                            .iter()
                            .any(|interval| interval.seq == *incoming_seq)
                }
                OrderSide::Ask => {
                    *incoming_seq == ask.seq
                        && compatible_bids
                            .iter()
                            .any(|interval| interval.seq == *resting_seq)
                }
            },
            _ => false,
        });
        let live = compatible_bids.iter().find(|interval| {
            interval.entered_at <= exec_idx && interval.exited_at.is_none_or(|exit| exec_idx < exit)
        });
        let basis = if rejected_saving_cross {
            // This saving bid already reached the ask and failed settlement. A later
            // successful consumer does not turn that execution failure into a price-
            // or queue-priority loss.
            None
        } else if let Some(interval) = live {
            if winner_limit > interval.limit {
                Some(PriorityBasis::HigherLimit)
            } else if winner_limit == interval.limit && winner_seq < interval.seq {
                // Equal limits AND the winner arrived first: arrival order alone
                // decided the loss (Microstructure).
                Some(PriorityBasis::EqualLimitEarlierSeq)
            } else {
                // Either a lower-limit winner (cannot beat a live higher-limit
                // saving bid) or an equal-limit winner whose seq is LATER than the
                // saving bid's — the saving bid held queue priority and should have
                // won, so the loss came from a settlement rejection, not arrival
                // order. Route to ExecutionResidual rather than mislabel it
                // EqualLimitEarlierSeq (which would spuriously inflate the
                // Microstructure family, the trap-narrowing diagnosis).
                None
            }
        } else if compatible_bids
            .iter()
            .any(|interval| interval.exited_at.is_some_and(|exit| exit < exec_idx))
        {
            Some(PriorityBasis::PostExitConsumption)
        } else if compatible_bids
            .iter()
            .any(|interval| exec_idx < interval.entered_at)
        {
            Some(PriorityBasis::PreEntryOrder)
        } else {
            None
        };
        let outcome = match basis {
            Some(basis) => SavingLossOutcome::CompetitiveLoss {
                basis,
                winner_intent: winner_intent_from(intent, winner_seq, staple, input_goods),
            },
            None => SavingLossOutcome::ExecutionResidual,
        };
        lost.push((ask.limit, ask.seq, outcome));
    }

    if lost.is_empty() {
        // Compatible asks existed but none were lost to another buyer — unreconciled.
        return SavingLossOutcome::ExecutionResidual;
    }
    // PAYLOAD RULE: the first compatible LOST unit in (limit, seq) order — deterministic.
    lost.sort_by_key(|entry| (entry.0, entry.1));
    lost[0].2
}

#[derive(Clone, Copy)]
struct AskInWindow {
    agent: AgentId,
    limit: Gold,
    seq: u64,
}

#[derive(Clone, Copy)]
struct BidInterval {
    seq: u64,
    limit: Gold,
    entered_at: usize,
    exited_at: Option<usize>,
}

fn saving_bid_intervals(
    records: &[AllocationRecord],
    staple: GoodId,
    member: AgentId,
) -> Vec<BidInterval> {
    let mut intervals = Vec::new();
    for (index, record) in records.iter().enumerate() {
        let bid = match record {
            AllocationRecord::BookSnapshot {
                side: OrderSide::Bid,
                agent,
                good,
                limit,
                seq,
                ..
            } if *agent == member && *good == staple => Some((*seq, *limit, 0)),
            AllocationRecord::QuoteAttempt {
                agent,
                good,
                side: OrderSide::Bid,
                outcome: QuoteOutcome::Posted { seq, limit, .. },
                ..
            } if *agent == member && *good == staple => Some((
                *seq,
                *limit,
                first_incoming_index(records, *seq)
                    .unwrap_or(index)
                    .min(index),
            )),
            _ => None,
        };
        let Some((seq, limit, entered_at)) = bid else {
            continue;
        };
        let exited_at = records
            .iter()
            .enumerate()
            .find_map(|(exit_index, exit)| match exit {
                AllocationRecord::QuoteExit {
                    agent,
                    good,
                    side: OrderSide::Bid,
                    seq: exit_seq,
                    ..
                } if *agent == member
                    && *good == staple
                    && *exit_seq == seq
                    && exit_index > entered_at =>
                {
                    Some(exit_index)
                }
                _ => None,
            });
        intervals.push(BidInterval {
            seq,
            limit,
            entered_at,
            exited_at,
        });
    }
    intervals.sort_by_key(|interval| (interval.entered_at, interval.seq));
    intervals.dedup_by_key(|interval| interval.seq);
    intervals
}

fn first_incoming_index(records: &[AllocationRecord], seq: u64) -> Option<usize> {
    records
        .iter()
        .enumerate()
        .find_map(|(index, record)| match record {
            AllocationRecord::Execution { incoming_seq, .. } if *incoming_seq == seq => Some(index),
            _ => None,
        })
}

/// Whether the member had ANY staple bid QuoteAttempt this pass (else it is a drop).
fn member_has_staple_bid_attempt(
    records: &[AllocationRecord],
    staple: GoodId,
    member: AgentId,
) -> bool {
    records.iter().any(|record| {
        matches!(
            record,
            AllocationRecord::QuoteAttempt {
                agent,
                good,
                side: OrderSide::Bid,
                ..
            } if *agent == member && *good == staple
        )
    })
}

/// Map the winner's captured first-unprovided want to a `WinnerIntent` by the pinned
/// precedence Now-hunger > Next-saving > Next-input > Other.
fn winner_intent_from(
    intent: &BTreeMap<u64, TracedWant>,
    seq: u64,
    staple: GoodId,
    input_goods: &BTreeSet<GoodId>,
) -> WinnerIntent {
    match intent.get(&seq) {
        Some(want) => match (want.kind, want.horizon) {
            (WantKind::Good(good), Horizon::Now) if good == staple => WinnerIntent::HungryNow,
            (WantKind::Good(good), Horizon::Next) if good == staple => WinnerIntent::SavingNext,
            (WantKind::Good(good), Horizon::Next) if input_goods.contains(&good) => {
                WinnerIntent::ProducerInputNext
            }
            _ => WinnerIntent::Other,
        },
        None => WinnerIntent::Other,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RoleChoiceReason {
    PriceAbsent,
    /// C3R.h (L2, `stale_input_price_fix` only): [`Settlement::fresh_input_ask`] yielded
    /// no fresh input price, so the candidate declines rather than appraise a free input.
    /// Two distinct causes share this bucket: no OTHER living colonist holds the recipe's
    /// input with a reservation ask (a supply fact), OR the input good IS the current
    /// money good, which `reservation_ask_for_money` declines to price unconditionally
    /// (`econ/src/agent.rs:449`) — so every holder is skipped regardless of supply. No
    /// chain config promotes a recipe input to money today, but nothing in `econ/menger`
    /// forbids it; read this bucket as "no money-denominated input price", not "no stock".
    /// Distinct from [`Self::PriceAbsent`] (an absent OUTPUT price) so the measurement can
    /// tell "the fix flipped the margin" apart from "the fix starved the appraisal".
    /// Never observed with the flag off.
    InputPriceAbsent,
    MarginNonpositive,
    OrdinalDecline,
    Accepts,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoleChoiceHistogram {
    pub attempts: u64,
    pub price_absent: u64,
    /// C3R.h (L2): declines for want of a fresh input price. Always `0` unless
    /// [`ChainConfig::stale_input_price_fix`] is on, so the pre-C3R.h partition
    /// `attempts == price_absent + margin_nonpositive + ordinal_decline + accepts`
    /// still holds exactly for every flag-off run.
    pub input_price_absent: u64,
    pub margin_nonpositive: u64,
    pub ordinal_decline: u64,
    pub accepts: u64,
}

impl RoleChoiceHistogram {
    fn observe(&mut self, reason: RoleChoiceReason) {
        self.attempts = self.attempts.saturating_add(1);
        let bucket = match reason {
            RoleChoiceReason::PriceAbsent => &mut self.price_absent,
            RoleChoiceReason::InputPriceAbsent => &mut self.input_price_absent,
            RoleChoiceReason::MarginNonpositive => &mut self.margin_nonpositive,
            RoleChoiceReason::OrdinalDecline => &mut self.ordinal_decline,
            RoleChoiceReason::Accepts => &mut self.accepts,
        };
        *bucket = bucket.saturating_add(1);
    }
}

/// Per-run Baker-class cash and bread-flow telemetry. Always present, zero until an
/// event, runtime-only, and excluded from `canonical_bytes`. Trade attribution uses
/// each agent's current vocation at the once-per-tick observation point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BakerRoundTrip {
    /// Gold paid by Baker BUYERS for the recipe input (flour): Σ price×qty.
    pub flour_gold_spent: u64,
    /// Gold received by Baker SELLERS for bread: Σ price×qty.
    pub bread_gold_earned: u64,
    /// Σ qty of those Baker-sold bread units.
    pub bread_units_sold: u64,
    /// Bread booked by the Baker-only production phase over the run.
    pub bread_units_produced: u64,
}

/// Runtime-only C3R.g counters; excluded from `canonical_bytes`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoleChoiceDiag {
    pub mill: RoleChoiceHistogram,
    pub bake: RoleChoiceHistogram,
    pub baker_first_econ_tick: Option<u64>,
    pub baker_last_econ_tick: Option<u64>,
}

impl RoleChoiceDiag {
    fn observe(&mut self, recipe: RecipeId, reason: RoleChoiceReason) {
        match recipe {
            RecipeId::Mill => self.mill.observe(reason),
            RecipeId::Bake => self.bake.observe(reason),
            _ => {}
        }
    }

    fn observe_baker_hold(&mut self, econ_tick: u64) {
        self.baker_first_econ_tick.get_or_insert(econ_tick);
        self.baker_last_econ_tick = Some(econ_tick);
    }
}

/// A settlement of generated colonists driven over a real `world` + `econ`.
pub struct Settlement {
    generation_seed: u64,
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
    /// S12/S14: the dedicated FORAGE node id — the `GoForage`/`GoHarvest` target the
    /// own-labor path resolves. Captured at generation, where the node is created
    /// OUTSIDE `config.nodes`, so the commons mode always harvests THIS node even when a
    /// config ALSO defines a `NodeSpec` for the FORAGE good (which a resolve-by-good
    /// lookup would otherwise find first, depleting the wrong node). `None` when
    /// own-labor subsistence is off, so no other run depends on it. Not serialized — a
    /// positional world locator, like [`Settlement::exchange`].
    forage_node_id: Option<NodeId>,
    carry_cap: u32,
    /// The move speed every colonist world agent is generated with (mirrors
    /// `config.move_speed`). Stored so a mid-run newborn's world agent (S13 spatial
    /// households) is placed with the same speed as the founders/roster.
    move_speed: u16,
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
    /// EXPERIMENTAL (capital-advance-with-repayment): outstanding revolving
    /// working-capital loans, `borrower -> (lender, owed)`. Each enabled tick a
    /// cashless producer borrows up to a working-capital floor from the richest
    /// saver (before the market) and repays from its sales (after the market),
    /// so it stays cash-light and its future-money want stays unmet (the
    /// incentive to keep producing survives, unlike an unrepaid gift). Empty and
    /// unused unless [`ChainConfig::capital_advance`] is set.
    capital_loans: BTreeMap<AgentId, (AgentId, Gold)>,
    /// S7.2: in-flight per-builder capital projects (BuildMill/BuildOven). Each holds a
    /// builder's own committed WOOD + advancing labor; on completion the tool credits
    /// the builder and the entry is removed. Empty unless
    /// [`ChainConfig::producible_capital`] is on, so every other run is byte-identical.
    capital_builds: Vec<CapitalBuild>,
    /// S7.2: a monotonic id source for capital projects (distinct per build). Only
    /// advanced by the capital-formation phase.
    next_capital_project_id: u32,
    /// S7 observability (read-only, NOT serialized): the whole-system count of tools
    /// built via S7.2 over the run — lets a test assert new capital entered the chain
    /// (acceptance tests 4/6). Diagnostic only; no phase reads it.
    tools_built: u64,
    /// C3R.a runtime-only telemetry: old-age deaths among mortal chain-producer
    /// subjects (active or latent mill/bake/tool-holder). Diagnostic only; not
    /// serialized and never consulted by production or inheritance paths.
    mortal_producer_old_age_deaths: u64,
    /// C3R.a runtime-only telemetry: mortal S7 role re-adoptions after a producer
    /// old-age death. Diagnostic only; not serialized.
    role_readoptions: u64,
    /// C3R.a runtime-only telemetry: fresh mill/oven units completed by mortal
    /// builders. Diagnostic only; not serialized.
    mortal_capital_builds: u64,
    /// C3R.b runtime-only telemetry: mill/oven units from a dead producer's estate
    /// that were credited to a living heir. Counts mill and oven units.
    producer_tool_inheritances: u64,
    /// C3R.b runtime-only telemetry: producer deaths whose mill/oven tools fell to
    /// commons because no living household heir existed.
    heirless_producer_deaths: u64,
    /// C3R.b runtime-only telemetry: inherited-tool holders that later adopted
    /// Miller/Baker through the ordinary S7 tool-holder role-choice path.
    heir_tool_adoptions: u64,
    /// C3R.b v2 runtime-only telemetry: food hearth units actually credited to
    /// dedicated producer-house members.
    producer_house_hearth_food_minted: u64,
    /// C3R.b v2 runtime-only telemetry: food hearth units actually credited to
    /// every other demography household member.
    non_producer_hearth_food_minted: u64,
    /// C3R.b v2 runtime-only telemetry: births in the dedicated producer houses.
    producer_house_births: u64,
    /// C3R.b v2 runtime-only telemetry: deaths among producer-house members.
    producer_house_deaths: u64,
    /// C3R.b v2 runtime-only telemetry: live producer-house members sampled once
    /// per economic tick.
    producer_house_person_ticks: u64,
    /// C3R.b v2 runtime-only telemetry: producer-role recipe appraisals that did
    /// not pay.
    producer_recipe_pay_rejections: u64,
    /// C3R.b v2 runtime-only telemetry: producer capital-build opportunities that
    /// did not clear the build appraisal/eligibility path.
    producer_build_rejections: u64,
    /// C3R.b v2 runtime-only telemetry: role-choice switches into producer roles
    /// blocked by the ordinary switch-readiness guard.
    producer_adoption_rejections: u64,
    /// C3R.b runtime-only attribution: `(heir, inherited_tool)` pairs created by
    /// producer estate settlement, used only to make `heir_tool_adoptions` narrow.
    producer_tool_inheritors: BTreeSet<(AgentId, GoodId)>,
    /// S10 observability (read-only, NOT serialized): the per-agent build decisions the
    /// LAST per-agent capital-formation phase recorded — one per eligible candidate, with
    /// accept/reject, the target savings want rank, and the decline reason. Cleared and
    /// refilled each tick the per-agent phase runs; empty in the S7 heuristic path. Lets a
    /// test prove the build is a per-colonist ordinal decision (an earlier-eligible
    /// colonist declined on its own scale while a later one accepted). Steers no future
    /// tick, so it is deliberately absent from [`Settlement::canonical_bytes`].
    last_capital_decisions: Vec<CapitalDecision>,
    /// S8.0 emergence probe (read-only, NOT serialized): the highest hunger any
    /// living colonist reached on a pre-promotion econ tick, and the count of
    /// pre-promotion ticks at least one colonist sat at/over the critical-hunger
    /// ceiling — the starvation pressure the emergence window must survive (Tension
    /// A). Frozen once a money good promotes. Deterministic; no phase reads them, so
    /// they steer no future tick and are deliberately absent from `canonical_bytes`.
    peak_pre_promotion_hunger: u16,
    critical_ticks_pre_promotion: u64,
    /// S16: the produced-bread provenance ledger (see [`BreadProvenance`]). Maintained only
    /// while [`Self::cultivation_sells_surplus_active`] holds; an empty default ledger
    /// otherwise, so every pre-S16 config keeps its identity and is byte-identical.
    bread_provenance: BreadProvenance,
    /// S18: the runtime-only multi-good money instrumentation (see [`MultigoodMoney`]).
    /// Maintained only while [`Self::multigood_money_active`] holds; an empty default
    /// otherwise. NOT in `canonical_bytes`, so it shifts no digest (byte-identical goldens).
    multigood: MultigoodMoney,
    /// S21d.1: the runtime-only acquisition-channel ledger (see [`AcquisitionLedger`]).
    /// Maintained only while [`Self::acquisition_ledger_active`] holds; an empty default
    /// otherwise. NOT in `canonical_bytes`, so it shifts no digest (byte-identical goldens).
    acquisition: AcquisitionLedger,
    /// C3R.c runtime-only earned-provisioning ledger. It observes C3R.b spot bread
    /// sales, classifies external revenue by buyer class, and tracks producer-house
    /// GOLD provenance buckets for earned-first provisioning. It is deliberately
    /// absent from `canonical_bytes`; the only future-behavior switch is the chain
    /// flag serialized under tag 29.
    earned_provisioning: EarnedProvisioningLedger,
    /// C3R.d runtime-only saving/control diagnostics. None steer a future tick;
    /// tag 31 and the live value scales carry the behavioral identity.
    birth_stock_wants_emitted: u64,
    birth_stock_attributable_purchases: u64,
    /// Producer-house members observed below the child-food target. This keeps
    /// seeded/newborn endowments from masquerading as stock accumulated by the
    /// saving motive; a member counts as reaching the target only after crossing
    /// it from below.
    birth_stock_below_target_agents: BTreeSet<AgentId>,
    birth_stock_reached_agents: BTreeSet<AgentId>,
    birth_stock_held_max: u32,
    birth_stock_held_at_death: u32,
    birth_stock_eligible_opportunities: u64,
    birth_stock_injections_completed: u64,
    birth_stock_source_shortfalls: u64,
    /// C3R.e (impl-67): the A1 ignition dose — the total staple quantity the one-shot injection
    /// moved at `birth_stock_ignition_at` (`< 24 = 6 × child_food_endowment → an under-dose →
    /// IgnitionShortfall`). `0` off the A1 path. Runtime-only, never digested.
    ignition_injected_qty: u64,
    /// C3R.e debt repair: the one-shot ignition's per-household gate decomposition (runtime-only,
    /// never serialized) — which gate blocked each producer household at the pinned shot.
    ignition_gate_blocked_interval: u64,
    ignition_gate_extinct: u64,
    ignition_gate_blocked_cap: u64,
    ignition_gate_blocked_hunger: u64,
    ignition_gate_suppressed_at_target: u64,
    ignition_gate_donor_shortfall: u64,
    /// C3R.e (impl-67): cumulative producer-house birth funding, split by the acquisition channel
    /// the drawn (parent→child) endowment lots carried — criterion iii reads market funding
    /// (`Bought`/`SelfProduced`) against non-market (`SeededMinted`/`Foraged`/`Commons`). `0` off
    /// the acquisition-ledger path. Runtime-only, never digested.
    producer_birth_funded_by_channel: [u64; FoodChannel::COUNT],
    /// C3R.e (impl-67): cumulative producer-house birth funding drawn from INTERVENTION-ORIGIN
    /// lots — criterion iii requires this to stay flat within an eligible window (a birth paid for
    /// with subsidy residue is not market-funded). `0` off the path. Runtime-only, never digested.
    producer_birth_funded_intervention: u64,
    birth_stock_injection_records: Vec<BirthStockInjectionRecord>,
    birth_stock_births_by_household: Vec<u64>,
    /// C3R.e-obs (impl-66 repair): the C3R.d attribution snapshot captured at the most
    /// recent pre-market seam while allocation observation is active. Runtime-only, read-only
    /// telemetry for independently recounting eligible opportunities; no decision path reads
    /// it and it is NEVER serialized.
    last_birth_stock_attribution_snapshot: BTreeSet<AgentId>,
    /// C3R.e-obs (impl-66): the runtime-only §2 loss-decomposition accumulator. NEVER
    /// serialized; populated only when `saving_allocation_obs_active()`.
    saving_allocation_obs: SavingAllocationObs,
    /// DH.b-obs (impl-70): the runtime-only birth-gate opportunity/recount accumulator. NEVER
    /// serialized; populated only when `birth_gate_obs_active()`.
    birth_gate_obs: birth_gate::BirthGateObs,
    /// Current tick's runtime-only §3.2c stock seam cursor.
    saving_obs_stock_tick: Option<SavingStockTick>,
    /// C3R.e-obs (§3.2a/§5): this tick's PRE-market offerable-seller counts (member,
    /// other), captured before `society.step()` and committed to `supply_series` by the §2
    /// join iff the tick is a money-priced spot pass. Runtime-only; NEVER serialized.
    saving_obs_pending_offerable: Option<(u64, u64)>,
    /// S21d.2a: the runtime-only cross-tick bootstrap microtrace (see [`BootstrapTrace`]).
    /// Maintained only while [`Self::acquisition_ledger_active`] holds; an empty default
    /// otherwise. NOT in `canonical_bytes`, so it shifts no digest (byte-identical goldens).
    bootstrap_trace: BootstrapTrace,
    /// Default-off, non-digested flour re-ignition census.
    flour_census: Option<FlourCensusRow>,
    flour_census_armed: bool,
    /// S21e.0: runtime-only bread seller provenance rows, used to pin the actual
    /// seller class in the mints-on positive control. Diagnostic, not canonical.
    bread_seller_trace: Vec<BreadSellerProvenance>,
    /// S21e.1: runtime-only finite seeded-surplus non-vacuity/exhaustion trace.
    /// Diagnostic, not canonical.
    seeded_surplus_trace: SeededSurplusTrace,
    /// S21h.0: runtime-only cumulative count of `SeededMinted` bread units transferred out
    /// of a seller (via [`AcquisitionLedger::transfer_as_bought`]) in a bread→SALT trade —
    /// pre-promotion barter where the counterparty good is SALT, or post-promotion spot
    /// sales once SALT is the money good. The hard demand-bridge invariant: this stays 0 on
    /// every cushion cell, so SALT can only monetize on the lineage's `SelfProduced` bread,
    /// never on cushion (`SeededMinted`) bread. Diagnostic, NOT digested. Independent of the
    /// `seeded_surplus_enabled` gate (the cushion is a buffer, not `seeded_surplus_bread`).
    seeded_minted_bread_sold_for_salt: u64,
    /// S21h.1: runtime-only cumulative emergency-provisioned bread units (produced == eaten
    /// in [`Self::run_emergency_self_provision`]) — the non-lineage roles' own-labor survival
    /// floor. The demand-preservation test reads it to confirm the floor is the SURVIVAL
    /// minimum, not the bulk of the demand side's food (post-promotion their food is
    /// materially BOUGHT, with the emergency floor a small tail). `0` off the seam; not digested.
    emergency_bread_provisioned: u64,
    /// S23e: finite rival subsistence commons state. This pool is a distinct accounting object
    /// from the G4a death-estate `commons_stock`: it regenerates bread as a named source and is
    /// drawn down by the S21h residual-hunger set as a rival outside option.
    subsistence_commons_stock: u64,
    subsistence_commons_cap: u64,
    subsistence_commons_regen: u64,
    subsistence_commons_phi_bps: u32,
    subsistence_commons_drawn_total: u64,
    subsistence_commons_unmet_total: u64,
    subsistence_commons_depleted_ticks: u64,
    subsistence_commons_shortfall_ticks: u64,
    subsistence_commons_eligible_need_total: u64,
    /// C1: conserved wage escrow holder and live contracts. `escrow_gold` joins
    /// [`Self::total_gold`]; each record either releases to the worker or refunds to the employer.
    wage_escrow_gold: Gold,
    wage_escrows: Vec<WageEscrow>,
    next_wage_contract_id: u64,
    /// C1: provenance tag on already-conserved owner money. Credited only from realized output
    /// sales, capped by spendable gold, discarded on death.
    wage_retained_earnings: BTreeMap<AgentId, Gold>,
    /// C1: FIFO wage-derived money attribution for non-owner purchases.
    wage_proceeds_buckets: BTreeMap<AgentId, VecDeque<WageProceedsLot>>,
    wage_workers_ever: BTreeSet<AgentId>,
    wage_employers_ever: BTreeSet<AgentId>,
    wage_hires_total: u64,
    wage_hires_post_promotion: u64,
    wage_below_ask_not_hired: u64,
    wage_endowment_funded_wages: Gold,
    wage_financed_output_buys: Gold,
    wage_nonowner_output_buys: Gold,
    wage_circular_loop_turnovers: u64,
    /// C1R: live output-share contracts over at-cap owned plots. No money/escrow: the
    /// worker's realized own-use output is split in kind at conversion.
    share_contracts: Vec<ShareContract>,
    next_share_contract_id: u64,
    share_workers_ever: BTreeSet<AgentId>,
    share_owners_ever: BTreeSet<AgentId>,
    share_contracts_total: u64,
    share_voluntary_contracts_total: u64,
    share_forced_contracts_total: u64,
    share_renewals_total: u64,
    share_worker_bread_income: u64,
    share_owner_bread_income: u64,
    share_worker_declined: u64,
    share_worker_unmatched: u64,
    share_forward_only_eligibility: u64,
    share_renewal_hints_total: u64,
    share_renewal_fed_out: u64,
    share_renewal_base_ineligible: u64,
    share_renewal_owner_not_candidate: u64,
    share_renewal_bread_declined: u64,
    share_renewal_matched_elsewhere: u64,
    share_owner_candidates_total: u64,
    share_owner_no_atcap_plot: u64,
    share_stock_opportunity_refusal: u64,
    share_reservation_collision: u64,
    share_stock_drawdown: u64,
    share_unattributed_share_deposit: u64,
    share_owner_grain_settled: u64,
    share_successions_total: u64,
    share_succession_heir_declined: u64,
    share_succession_worker_re_declined: u64,
    share_post_succession_renewals: u64,
    share_succeeded_live_ids: BTreeSet<u64>,
    /// C1N: live fixed-bread-wage contracts over at-cap owned plots. The wage advance is
    /// paid up front; the worker's realized own-use output is transferred 100% to the employer.
    in_kind_contracts: Vec<InKindWageContract>,
    next_in_kind_contract_id: u64,
    in_kind_workers_ever: BTreeSet<AgentId>,
    in_kind_employers_ever: BTreeSet<AgentId>,
    in_kind_hires_total: u64,
    in_kind_worker_advance_bread: u64,
    in_kind_employer_bread_income: u64,
    in_kind_expected_output_total: u64,
    in_kind_worker_declined: u64,
    in_kind_worker_unmatched: u64,
    in_kind_owner_candidates_total: u64,
    in_kind_owner_no_atcap_plot: u64,
    in_kind_owner_insufficient_fund: u64,
    in_kind_productivity_declined: u64,
    in_kind_reservation_collision: u64,
    in_kind_stock_drawdown: u64,
    in_kind_unattributed_deposit: u64,
    in_kind_employer_grain_settled: u64,
    in_kind_endowment_funded_hires: u64,
    in_kind_term_starvations: u64,
    /// S23e runtime-only owner/surplus telemetry used by the scarcity classifier.
    ever_landowner_ids: BTreeSet<AgentId>,
    owner_first_claim_tick: BTreeMap<AgentId, u64>,
    owner_age_at_first_claim: BTreeMap<AgentId, u64>,
    owner_tenure_before_death: Vec<(AgentId, u64)>,
    owner_bread_consumed: BTreeMap<AgentId, u64>,
    owner_surplus_sold_before_death: BTreeMap<AgentId, u64>,
    owner_inventory_at_death: Vec<(AgentId, u64)>,
    inherited_stock_to_heirs: u64,
    buyer_purchases_by_owner_age_cohort: BTreeMap<u64, u64>,
    owner_seller_attributed_bought: u64,
    /// S22b: per-econ-tick scratch — the set of agent ids that realized cultivation output
    /// (harvested grain AND converted it to bread, output > 0) in the current tick's
    /// [`Self::run_own_use_cultivation`]. Drained by [`Self::run_cultivation_skill`] to credit
    /// skill GAIN to exactly those agents (every other living colonist decays). Cleared and
    /// refilled each tick; only maintained on the active cultivation-skill path. Runtime-only,
    /// NOT digested (the per-agent skill it drives IS digested, in the colonist roster).
    cultivation_skill_producers: BTreeSet<AgentId>,
    /// S22b: per-agent CUMULATIVE grain hauled (the cultivation input good deposited to econ)
    /// over the run — the monopolization/grain-share probe + the non-vacuity grain measure.
    /// Maintained only on the active cultivation-skill path. Runtime-only, NOT digested.
    cultivation_grain_harvested: BTreeMap<AgentId, u64>,
    /// S22b: per-agent CUMULATIVE bread produced by own-use cultivation over the run — the
    /// non-vacuity bread measure (a skilled cultivator must produce strictly more). Maintained
    /// only on the active cultivation-skill path. Runtime-only, NOT digested.
    cultivation_bread_produced: BTreeMap<AgentId, u64>,
    /// S22c: per-econ-tick scratch — each seller's realized post-money SALT proceeds from selling
    /// its OWN cultivated bread surplus this tick (`price × own-produced qty`, attributed via
    /// `produced_lots` where `lot.producer == seller`). Cleared at the start of
    /// [`Self::run_bread_provenance_market`] and filled as each post-promotion bread→SALT spot
    /// sale is attributed; drained by [`Self::update_cultivation_returns`] into the per-colonist
    /// [`Colonist::cultivation_return_window`]. Maintained only on the active
    /// profit-driven-retention path. Runtime scratch, NOT digested (the window it feeds IS).
    cultivation_proceeds_scratch: BTreeMap<AgentId, u64>,
    /// S22c (runtime-only diagnostic, NOT digested): the agent ids RETAINED-BY-PROFIT this tick —
    /// agents whose cultivation `cultivate_now` was true ONLY because [`Self::profit_stay_active`]
    /// fired (they were past the hunger exit, no input in flight, not pressure-escalating, so the
    /// flag-off path would have EXITED them). The counterfactual exit-flip set. Cleared/refilled
    /// each tick in [`Self::run_own_labor_subsistence`]; empty off the profit-driven-retention path.
    profit_retained_ids: BTreeSet<AgentId>,
    /// S22c (runtime-only diagnostic): the union of [`Self::profit_retained_ids`] over the whole
    /// run — distinct agents ever retained-by-profit. Not digested.
    profit_retained_ever: BTreeSet<AgentId>,
    /// S22f (runtime-only diagnostic, NOT digested): distinct agent ids that ever held a commitment
    /// over the run (voluntary or fiat) — the committed cohort. Doubles as the "has ever committed"
    /// tracker the voluntary entry seam reads to distinguish a FIRST opt-in from a RENEWAL. Empty off
    /// the voluntary-commitment path.
    commitment_committed_ever: BTreeSet<AgentId>,
    /// S22f (runtime-only diagnostic, NOT digested): per VOLUNTARY committer, the `(econ tick, signal
    /// value)` at its FIRST signal-gated opt-in — the proof each entry is traceable to that agent's
    /// own cleared S22c return (the signal value is its windowed realized cultivation proceeds at
    /// uptake). Fiat-pinned commits are deliberately absent (they have no signal). Empty off the path.
    commitment_uptake: BTreeMap<AgentId, (u64, u64)>,
    /// S22f (runtime-only diagnostic, NOT digested): agent ids FORCE-committed by the `fiat_pin`
    /// control (bypassing the voluntary signal). Empty for the voluntary headline + every other
    /// config; non-empty only under `commitment_fiat_pin > 0`, where it marks the run as a re-pin.
    commitment_fiat_ever: BTreeSet<AgentId>,
    /// S22f (runtime-only diagnostic, NOT digested): eligible UNCOMMITTED agent ids whose entry
    /// signal was evaluated post-money and fell BELOW the entry floor (windowed cultivation proceeds
    /// `< commitment_entry_floor`) — the below-floor non-committers that prove the signal
    /// DISCRIMINATES (entry is a real decision, not a universal auto-yes). Empty off the path.
    commitment_below_floor_ever: BTreeSet<AgentId>,
    /// S22f (runtime-only diagnostic, NOT digested): agent ids cultivating THIS tick ONLY because the
    /// commitment binding overrode the exit (the normal S22a/S22c rule would have exited them).
    /// Cleared/refilled each tick in [`Self::run_own_labor_subsistence`]; empty off the path.
    commitment_exit_override_ids: BTreeSet<AgentId>,
    /// S22f (runtime-only diagnostic, NOT digested): the union of [`Self::commitment_exit_override_ids`]
    /// over the run — distinct agents whose commitment ever bound a tick the flag-off run would have
    /// exited (the real exit-override the mandatory non-vacuity test reads). Empty off the path.
    commitment_exit_override_ever: BTreeSet<AgentId>,
    commitment_norm_copy_events: Vec<CommitmentNormCopyRow>,
    commitment_norm_flip_events: Vec<CommitmentNormFlipRow>,
    commitment_norm_adoptions: u64,
    commitment_norm_abandonments: u64,
    commitment_norm_imitation_adopters: BTreeSet<AgentId>,
    commitment_norm_group_covariance_sum: i128,
    commitment_norm_group_covariance_count: u64,
    /// S22d: in-flight per-builder durable-cultivation-tool projects (a
    /// [`ProjectTemplateId::BuildCultivationTool`] each). Each holds a cultivating builder's own
    /// committed WOOD + advancing labor; on completion the plow credits the builder's stock and
    /// the entry is removed. SEPARATE from `capital_builds` (the money-gated mill/oven phase) so
    /// the cultivation tool can build PRE-money. Steers future ticks (which builder, how much
    /// labor advanced) ⇒ digested ON-only under the gate. Empty unless
    /// [`ChainConfig::durable_cultivation_tool`] is active, so every other run is byte-identical.
    cultivation_tool_builds: Vec<CapitalBuild>,
    /// S22d: a monotonic id source for cultivation-tool projects, distinct from
    /// `next_capital_project_id`. Only advanced by the cultivation-capital phase. Digested ON-only.
    next_cultivation_tool_project_id: u32,
    /// S22d: per-econ-tick scratch — the set of agent ids that realized cultivation output this
    /// tick (the tenure-credit set), filled in [`Self::run_own_use_cultivation`] and drained by
    /// [`Self::run_cultivation_capital_formation`] to credit/reset the per-colonist tenure. Cleared
    /// and refilled each tick; only maintained on the active durable-cultivation-capital path.
    /// Runtime scratch, NOT digested (the tenure it drives IS, in the colonist roster).
    cultivation_tool_producers: BTreeSet<AgentId>,
    /// S22d observability (read-only, NOT digested): the whole-system count of durable cultivation
    /// tools built over the run, the cumulative WOOD consumed building them (the measured SUNK
    /// cost), and the cumulative tools DESTROYED — `0` for the durable headline, positive only on
    /// the non-durable/rented control (each built plow consumed after one cultivation opportunity).
    /// The tool-stock accounting invariant is `built − destroyed == stock_total`. No phase reads them.
    cultivation_tools_built: u64,
    cultivation_tool_wood_consumed: u64,
    cultivation_tools_destroyed: u64,
    /// S22e: the number of durable cultivation tools (plows) GRANTED to lineage households at
    /// generation (the conservation-safe INITIAL endowment). `0` off the path. Part of the
    /// tool-stock invariant `endowed + built − destroyed == stock_total`; included in the initial
    /// whole-system conservation baseline by virtue of being placed in agent stock at generation
    /// (before the first `econ_tick`). Read-only, NOT digested (the granted plows live in the
    /// already-serialized agent stock).
    endowed_cultivation_tools_total: u64,
    /// S22e: the lineage household indices actually granted an endowment at generation after the
    /// deterministic hash selection succeeds. Digested ON-only under the behavior-active gate; also
    /// exposed as a runtime diagnostic. Empty off the path.
    endowed_households: Vec<usize>,
    /// S22e: the founding-member agent ids that were granted an endowed plow (one per selected
    /// household). Runtime diagnostic only (the granted plows are serialized in agent stock). Empty
    /// off the path.
    endowed_member_ids: Vec<AgentId>,
    /// S22e observability (read-only, NOT digested): the cumulative count of plow units that passed
    /// to a LIVING household heir via `settle_estate_to_heirs` (a real inheritance transfer), and
    /// the set of heir ids that received one. The non-vacuity test reads these to confirm a real
    /// post-founder-death transfer occurred. `0`/empty when the gate forces plows to the commons or
    /// no plow-holder dies with a living heir.
    cultivation_tool_inherited_total: u64,
    cultivation_tool_inheritor_ids: BTreeSet<AgentId>,
    /// S23a: finite private-land registry, keyed by grain resource-node id. Ownership is metadata:
    /// the node stock remains in the world conservation ledger. The registry steers harvest gating
    /// and idle forfeiture, so it is serialized ON-only under private land tenure.
    land_plots: BTreeMap<NodeId, LandPlotRecord>,
    /// S23c runtime diagnostics: title transfers caused by death inheritance, including
    /// partible co-heir transfers and heirless reversions. Future behavior is in `land_plots`;
    /// this log is for the verdict trace.
    secure_land_inheritance_events: Vec<SecureLandInheritanceRow>,
    /// S23d runtime diagnostics: distinct secure-title owners that died of old age while
    /// holding land, and the subset that had at least one live secure heir at death. They are
    /// measurement-only and deliberately excluded from canonical bytes.
    secure_land_owner_old_age_deaths_total: u64,
    secure_land_inherit_eligible_owner_deaths_total: u64,
    /// S23c runtime diagnostic: count of partible inheritance shares that fell below the
    /// viability floor and were recorded as stranded capacity.
    secure_land_stranded_shares_total: u64,
    /// S23a runtime-only diagnostics used by the acceptance classifier and trace surface.
    land_claims_total: u64,
    land_idle_losses_total: u64,
    /// Every rerouted harvest task (owner-gate denials AND single-targeter stampede losers) — a
    /// throughput diagnostic only.
    land_harvest_denials_total: u64,
    /// The subset of the above that is a reroute off a plot already HELD by another live owner (or
    /// reserved for one), rather than a single-targeter stampede loser on unowned land. NOTE: this
    /// is ~always 0 by design — §3.5(b)'s reservation + §3.7 targeting (non-owners only ever target
    /// *unowned* plots) resolve all contention while a plot is still unowned, so an agent never ends
    /// up holding a task pointed at an owned-by-other plot. Kept as a diagnostic; the load-bearing
    /// gate proof is [`Self::land_nonowner_harvest_of_owned_total`].
    land_owner_gate_denials_total: u64,
    /// Harvest events where a non-owner pulled grain from a plot HELD by another. Under the headline
    /// (`harvest_gate` on) this is 0 — the pre-`world.tick` validation reroutes non-owners off owned
    /// plots, so harvest is owner-exclusive. Under the `non_excludable_deed` control it is > 0. The
    /// pair (headline == 0, control > 0) is the §4 non-vacuity proof that ownership actually gates
    /// harvest, not merely records title.
    land_nonowner_harvest_of_owned_total: u64,
    land_reclaims_by_other_total: u64,
    land_marginal_nonowner_claims_total: u64,
    land_lapsed_reentry_worse_total: u64,
    land_plot_harvest_totals: BTreeMap<NodeId, u64>,
    land_lapsed_losses: BTreeMap<AgentId, LandPlotQuality>,
    land_lost_prior_owners: BTreeMap<NodeId, (AgentId, LandLossCause)>,
    land_market_plots: BTreeMap<NodeId, LandMarketPlotState>,
    land_market_yield_this_tick: BTreeMap<NodeId, u32>,
    land_market_sales: Vec<LandSaleRecord>,
    land_market_trade_count: u64,
    land_market_pre_promotion_trade_count: u64,
    land_market_carrying_paid_total: u64,
    land_market_pre_promotion_charges: u64,
    land_market_foreclosure_listings_total: u64,
    land_market_priced_out_total: u64,
    land_market_lapsed_priced_out_total: u64,
    land_market_ask_bid_gap_sum: u64,
    land_market_ask_bid_gap_count: u64,
    land_market_title_history: BTreeMap<AgentId, LandTitleHistory>,
    land_fee_pool_salt: Gold,
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
    /// TEST-ONLY fault injection: gold minted into the commons inside the births
    /// phase each tick (a deliberate conservation violation). Exists solely to prove
    /// the per-tick money identity spans the whole tick — a post-market mint must
    /// trip [`EconTickReport::money_conserves`]. Always zero except in the tripwire
    /// unit test; absent entirely from non-test builds.
    #[cfg(test)]
    pub(crate) test_fault_mint_birth_gold: u64,
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
    /// S17 — lifetime **starvation** (positive-check) death count, accumulated from the
    /// death count [`Self::update_needs_and_remove_dead`] returns. Distinct from
    /// `old_age_deaths_total` so the two Malthusian checks are attributable. It is a
    /// **runtime-only diagnostic**: NOT serialized into `canonical_bytes` (the deaths
    /// themselves live in the colonist liveness/estate state the digest already pins, and
    /// existing configs — `g4a_death`, `starved_hauler` — already carry live starvation,
    /// so digesting this counter would break their goldens). The asymmetry with
    /// `old_age_deaths_total` (which IS digested) is intentional — it avoids golden churn.
    starvation_deaths_total: u64,
    /// S14 — **birth-block diagnostics**: lifetime counts of *why* a household that
    /// was checked did NOT birth this tick, so the endogenous-plateau finding is
    /// interpretable (does the population stall at the carrying capacity via the
    /// hunger ceiling — the preventive check — or fail to grow for a demographic
    /// reason?). Each is incremented at the matching `continue` in [`Self::run_births`]:
    /// the household was past its interval but at the size cap
    /// ([`Self::birth_block_size_cap`]), or had a member over the hunger ceiling
    /// ([`Self::birth_block_hunger_ceiling`]), or no member could endow the child's
    /// food ([`Self::birth_block_endowment`]), or it was still inside its birth
    /// interval ([`Self::birth_block_interval`]). Counted for any demography config, but
    /// serialized into `canonical_bytes` only on the forage-commons path (gated), so
    /// every existing demography golden is byte-identical.
    birth_block_interval: u64,
    birth_block_size_cap: u64,
    birth_block_hunger_ceiling: u64,
    birth_block_endowment: u64,
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
    /// S9 — the emergent medium's **heterogeneous real direct use**
    /// `(good, qty, period)`, or `None` when off. While the settlement is still in
    /// barter, the SELECTED subset of colonists (stable id index `0 mod period`) has
    /// its scale extended with `qty` fixed `Good(good)/Now` CONSUMPTION wants — the
    /// real non-monetary demand that seeds SALT's pre-monetary saleability without a
    /// circular "want it as money" demand. Heterogeneous by construction (`period`),
    /// so the non-selected colonists stay free to accept SALT indirectly. Consumed
    /// into the `consumed` bucket; dropped once a money good has emerged (SALT then
    /// delists to money). `None` for every pre-S9 scenario, so their scale
    /// generation — and canonical layout — is unchanged.
    salt_direct_use: Option<(GoodId, u32, u16)>,
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
    /// The G8c-1 **credit-cycle** runtime, or `None` for every non-finance
    /// settlement (the cycle path is then skipped entirely, so the run is
    /// byte-identical to G8b). When `Some`, the settlement is a finance settlement:
    /// `society` runs econ's credit-ladder scenario, every spatial phase is a no-op
    /// (the colonist roster is empty), and each econ tick simply steps the society
    /// so the cycle runs endogenously. Holds the base [`MarketScenario`] so the
    /// credit-disabled **shadow** can be replayed for the natural-rate baseline.
    cycle: Option<CycleRuntime>,
    /// Cached, read-only shadow replay metrics for the current finance run length.
    /// This is not canonical state: it is a pure function of `cycle.scenario` and the
    /// live M3 records, kept only so repeated viewer/test reads do not rerun the same
    /// credit-disabled replay.
    shadow_cycle_cache: RefCell<Option<ShadowCycleMetrics>>,
    /// The G8c-2 **tender bench** runtime, or `None` for every non-bench settlement
    /// (the bench path is then skipped, so the run is byte-identical to G8c-1). When
    /// `Some`, the settlement is a finance settlement (like the cycle) whose `society`
    /// runs the unchanged econ scenario for its surface (the M11/M12 fiat-displacement,
    /// M15 bank-loan-repayment, or M16 issuer-repayment scenario) to demonstrate that
    /// surface's refusal-vs-acceptance. Mutually exclusive with [`Self::cycle`]. Retains
    /// the surface + base scenario for the canonical-bytes determinism surface.
    bench: Option<BenchRuntime>,
    /// The G8c-3 **tax overlay** runtime, or `None` for every settlement that levies no
    /// tax (the canonical tax block is then omitted, so non-tax runs are byte-identical).
    /// When `Some`, the finance (cycle) settlement carries the state's levy +
    /// receivability: the M21 events are in `society` (and in `cycle.scenario`), and this
    /// retains the configured receivability + levied total for readback and the canonical
    /// fingerprint. The live receivability, receipts, and defaults are read from the
    /// society / its issuer.
    tax: Option<TaxRuntime>,
    /// DH.a (impl-68): the closed-circulation marker, copied from
    /// [`SettlementConfig::closed_circulation`]. `false` for every non-closed settlement, so the
    /// closure observation and provenance ledger are inert and every existing run is byte-identical.
    /// Deliberately absent from [`Settlement::canonical_bytes`] except via the ON-only digest tag
    /// 34 (see [`closure`]).
    closed_circulation: bool,
    /// DH.a (impl-68): the whole-population gold/physical provenance ledger + raw event tape,
    /// maintained only when `closed_circulation` is on. Pure observation — runtime-only, never
    /// serialized in `canonical_bytes`, and provably behaviour-inert (the DH.a inertness test).
    closure: closure::ClosureLedger,
    /// DH.b (impl-69): the reproductive-burden audit telemetry (succession events, per-birth
    /// funding records, settled-trade records). Maintained only when [`Self::closure_active`]
    /// holds — the same predicate the DH.a force-disable control flips — so it is inert on every
    /// non-closed run. Runtime-only, never serialized in `canonical_bytes`.
    burden: burden::BurdenTelemetry,
}

/// The G8c-3 tax-overlay runtime (held on a finance [`Settlement`]). Reused econ M21
/// machinery does the work (seeds the zero-principal liability, settles it under the
/// receivability gate, books the issuer tax accounts); this retains only what the sim
/// surfaces and pins: the configured receivability and the total levied. The live
/// outcomes (receipts, defaults, the active receivability after the Tick(0) event fires)
/// are read from the society and its single issuer.
struct TaxRuntime {
    /// The receivability the overlay set — the chartalist gate. Read back live from the
    /// society for the active policy; retained here for the canonical fingerprint and to
    /// mark the settlement as a tax settlement.
    receivability: TaxReceivability,
    /// The total the overlay levies (matches the issuer's `taxes_levied` once every levy
    /// event has fired) — the viewer/headline magnitude.
    levied: Gold,
}

/// The G8c-1 credit-cycle runtime (held on a finance [`Settlement`]). Reused econ
/// machinery does all the work; this only retains what the sim needs to **measure**
/// the cycle: the kind (cycle vs control) and the base scenario the shadow replays.
struct CycleRuntime {
    /// Which demonstration this settlement runs.
    kind: CycleKind,
    /// The econ scenario the society was built from — cloned and run credit-disabled
    /// to get the shadow natural-rate baseline (`gap = shadow − market`). Cloned per
    /// shadow query (a read-only replay), so the live run is never perturbed.
    scenario: MarketScenario,
}

/// The G8c-2 tender-bench runtime (held on a finance [`Settlement`]). Reused econ
/// machinery enforces the tender; this only retains what the sim needs to **measure**
/// the surface's settlement: which surface, and the base scenario (for the canonical
/// determinism surface + the run length).
struct BenchRuntime {
    /// Which surface this bench demonstrates.
    surface: BenchSurface,
    /// The unchanged econ scenario the society was built from for this surface (the
    /// M11/M12 fiat-displacement, M15 bank-loan-repayment, or M16 issuer-repayment
    /// scenario), with the surface's tender swapped in. Retained so the canonical bytes
    /// distinguish the benches from one another and carry the policy that steers
    /// settlement.
    scenario: MarketScenario,
}

/// Derived metrics from the credit-disabled shadow replay at a specific live run
/// length. All fields are MEASURED from the shadow + live M3 records; this is a
/// cache of observations, not ABCT state.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ShadowCycleMetrics {
    ticks: usize,
    natural_rate_bps: Vec<Option<i64>>,
    gap_bps: Vec<Option<i64>>,
    max_gap_bps: i64,
    structure_rose_above_shadow: bool,
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
    /// S21e: finite one-time surplus size configured for the seeded-surplus probe.
    /// Runtime-only helpers use this as the on/off gate; it is serialized ON-only
    /// separately from diagnostics.
    seeded_surplus_bread: u32,
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
    /// EXPERIMENTAL: enable the conserved capital-advance phase (see
    /// [`ChainConfig::capital_advance`]). `false` for every existing config.
    capital_advance: bool,
    /// EXPERIMENTAL: per-tick spoilage rate (bps) on perishable chain foods (see
    /// [`ChainConfig::perishable_decay_bps`]). `0` for every existing config.
    perishable_decay_bps: u16,
    /// EXPERIMENTAL: enable the in-kind subsistence advance to hungry producers
    /// (see [`ChainConfig::subsistence_advance`]). `false` for every existing config.
    subsistence_advance: bool,
    /// EXPERIMENTAL: enable the in-kind input advance to producers (see
    /// [`ChainConfig::input_advance`]). `false` for every existing config.
    input_advance: bool,
    /// EXPERIMENTAL: recurring owner-operator role-adoption motive (see
    /// [`ChainConfig::recurring_motive`]). `false` for every existing config.
    recurring_motive: bool,
    /// EXPERIMENTAL: project-aware (imputed) input bids — the producer buys its
    /// own input by market trade (see [`ChainConfig::project_input_bids`]).
    /// `false` for every existing config.
    project_input_bids: bool,
    /// S5: the local producer-subsistence floor — the staple a producer's own
    /// household hearth mints fresh each tick (see
    /// [`ChainConfig::producer_subsistence`]). `0` for every existing config.
    producer_subsistence: u32,
    /// S12: the own-labor subsistence gate + forage knobs (see
    /// [`ChainConfig::own_labor_subsistence`]). `false`/`0` for every existing config,
    /// so the food mints stay, no FORAGE is produced, and the run is byte-identical.
    own_labor_subsistence: bool,
    forage_yield: u32,
    forage_hunger_in: u16,
    forage_hunger_out: u16,
    /// S14: the capped FORAGE-commons parameters (see [`ChainConfig::forage_commons`]).
    /// `None` for every existing config, so the FORAGE node stays a `0/0/0` marker and
    /// the S12 fixed-credit path is byte-identical.
    forage_commons: Option<ForageCommons>,
    /// S15: the own-use cultivation gate + its hysteresis tier + the per-tick own-use
    /// bread draw (see [`ChainConfig::own_use_cultivation`]). `false`/`0` for every
    /// existing config, so the cultivation steering/phase are inert and byte-identical.
    own_use_cultivation: bool,
    cultivate_hunger_in: u16,
    cultivate_hunger_out: u16,
    cultivate_consume: u32,
    cultivate_patience: u16,
    /// S16: the money-from-produced-bread gate (see [`ChainConfig::cultivation_sells_surplus`]).
    /// `false` for every existing config, so the buy/sell split and the provenance ledger
    /// are inert and the run is byte-identical.
    cultivation_sells_surplus: bool,
    /// S18: the money-from-a-multi-good-economy gate (see [`ChainConfig::multigood_money`]).
    /// `false` for every existing config, so the woodcutter routing and the runtime-only
    /// instrumentation are inert and the run is byte-identical.
    multigood_money: bool,
    /// S21f: the household-barter cultivation activation seam (see
    /// [`ChainConfig::household_barter_cultivation`]). `false` for every existing config, so
    /// cultivation still requires the forage substrate and the run is byte-identical.
    household_barter_cultivation: bool,
    /// S22a: the endogenous cultivation-entry gate (see
    /// [`ChainConfig::endogenous_cultivation_entry`]). `false` for every existing config, so
    /// cultivation eligibility stays pinned to the lineage and the run is byte-identical.
    endogenous_cultivation_entry: bool,
    /// S22b: the bounded cultivation-skill gate + its magnitudes (see
    /// [`ChainConfig::cultivation_skill`]). `false`/defaults for every existing config, so the
    /// grain-haul lever is inert and the run is byte-identical.
    cultivation_skill: bool,
    /// S22c: the profit-driven-retention gate (see [`ChainConfig::profit_driven_retention`]).
    /// `false` for every existing config, so the cultivation exit stays pure hunger/pressure and
    /// the run is byte-identical.
    profit_driven_retention: bool,
    /// S22c: the rolling-return window / outside-margin / material floor (see the matching
    /// [`ChainConfig`] fields). Consulted only while `profit_driven_retention` is active.
    return_window: u64,
    retention_margin_bps: u64,
    retention_material_floor: u64,
    skill_gain: u16,
    skill_decay: u16,
    skill_cap: u16,
    skill_haul_ceiling: u32,
    /// S22d: the durable-cultivation-capital gate + its build/boost knobs (see
    /// [`ChainConfig::durable_cultivation_tool`]). `false`/defaults for every existing config, so
    /// the cultivation-capital phase is inert, no tool is ever built, and the run is byte-identical.
    durable_cultivation_tool: bool,
    tool_build_patience: u16,
    cultivation_tool_haul_ceiling: u32,
    cultivation_tool_non_durable: bool,
    /// S22e: the endowed + inherited cultivation-capital gate + its knobs (see
    /// [`ChainConfig::endowed_cultivation_capital`]). `false`/defaults for every existing config,
    /// so no household is endowed, plows keep the existing heir routing, and the run is
    /// byte-identical.
    endowed_cultivation_capital: bool,
    endowed_tool_count: u16,
    cultivation_tool_inheritance: bool,
    /// S22f: the voluntary fixed-term cultivation commitment gate + its knobs (see
    /// [`ChainConfig::voluntary_cultivation_commitment`]). `false`/defaults for every existing
    /// config, so the commitment entry/binding/expiry seam and the ON-only digest surface never
    /// engage and the run is byte-identical.
    voluntary_cultivation_commitment: bool,
    commitment_term: u16,
    commitment_entry_floor: u64,
    commitment_fiat_pin: u16,
    commitment_norm_spread: bool,
    abandonable_norm: bool,
    group_payoff_imitation: bool,
    fixed_commitment_norm_prevalence: Option<f64>,
    commitment_seed_share_bps: u16,
    imitation_period: u64,
    imitation_window: u64,
    imitation_margin_bps: u64,
    imitation_radius: u16,
    imitation_max_models: u16,
    food_window_target: u64,
    no_imitation: bool,
    random_imitation: bool,
    salt_in_score: bool,
    /// S23a: private land tenure gate + knobs (see [`ChainConfig`]). All inert unless the gate
    /// composes on S22a endogenous cultivation entry.
    private_land_tenure: bool,
    land_idle_limit: u16,
    harvest_gate: bool,
    forfeit_on_idle: bool,
    reclaim_reserved_for_prior_owner: bool,
    land_good_plots: u16,
    land_marginal_plots: u16,
    land_marginal_regen: u32,
    secure_land_tenure: bool,
    inheritance_regime: InheritanceRegime,
    land_market: bool,
    mortal_landowner_demography: bool,
    rival_subsistence_commons: bool,
    rival_subsistence_commons_phi_bps: u32,
    wage_labor: bool,
    wage_labor_mode: WageLaborMode,
    share_tenancy: bool,
    share_tenancy_mode: ShareTenancyMode,
    share_forward_provisioning: bool,
    share_contract_succession: bool,
    in_kind_wage: bool,
    mortal_chain_producers: bool,
    mortal_producer_inheritance: bool,
    mortal_producer_tool_inheritance: bool,
    producer_house_cap: u8,
    earned_provisioning: bool,
    producer_stock_provisioning_control: bool,
    birth_stock_saving: bool,
    birth_stock_saving_mode: BirthStockSavingMode,
    saving_allocation_obs: bool,
    birth_gate_obs: bool,
    share_bps: u16,
    share_term: u16,
    land_carrying_cost: u64,
    land_price_cap_factor: u64,
    /// S21h.0: the non-lineage woodcutters' consumed-only bread cushion (see
    /// [`ChainConfig::gatherer_food_cushion`]). `0` for every existing config; canonicalized
    /// ON-only (its differing gatherer starting stock already splits the digest).
    gatherer_food_cushion: u32,
    /// S21h.1: the emergency self-provisioning trigger (see
    /// [`ChainConfig::emergency_hunger_threshold`]). `0` (off) for every existing config, so
    /// the [`Settlement::run_emergency_self_provision`] phase is inert and byte-identical.
    emergency_hunger_threshold: u16,
    /// C3R.e (impl-67): the A1 ignition gate (see [`ChainConfig::birth_stock_ignition_at`]).
    /// `None` for every existing config; canonicalized ON-only under tag 33.
    birth_stock_ignition_at: Option<u64>,
    /// C3R.e (impl-67): the A2 additive endowment (see
    /// [`ChainConfig::producer_house_starting_staple`]). `0` for every existing config; tag 33.
    producer_house_starting_staple: u32,
    /// C3R.e (impl-67): the B support-withdrawal gate (see
    /// [`ChainConfig::producer_support_until_tick`]). `None` for every existing config; tag 33.
    producer_support_until_tick: Option<u64>,
    /// S21d.0: retire the food mints (see [`ChainConfig::retire_food_mints`]). `false` for every
    /// existing config, so the demographic + producer staple mints fire and the run is
    /// byte-identical (the flag is canonicalized ON-only).
    retire_food_mints: bool,
    /// S21d.1: the acquisition-channel ledger gate (see [`ChainConfig::acquisition_ledger`]).
    /// `false` for every existing config; runtime-only diagnostic, never digested.
    acquisition_ledger: bool,
    /// S6: the productive-re-entry phase gate + hysteresis thresholds (see
    /// [`ChainConfig::productive_reentry`]). `false`/unused for every existing config.
    productive_reentry: bool,
    reentry_hunger_in: u16,
    reentry_hunger_out: u16,
    /// S7.1: the relaxed tool-acquisition eligibility gate (see
    /// [`ChainConfig::tool_acquisition_eligibility`]). `false` for every existing
    /// config, so the seeded-`latent`-only role-choice path is byte-identical.
    tool_acquisition_eligibility: bool,
    /// S7.2: the per-builder capital-formation phase gate + its appraisal knobs (see
    /// [`ChainConfig::producible_capital`]). `false`/unused for every existing config,
    /// so no tool is ever built and the goldens are byte-identical.
    producible_capital: bool,
    /// S10: the per-agent intertemporal capital decision replaces S7's settlement-level
    /// build planner (see [`ChainConfig::per_agent_capital`]). `false` for every existing
    /// config, so the S7 heuristic is byte-identical.
    per_agent_capital: bool,
    /// S11: per-agent fallible OUTPUT-price forecasts feed the entrepreneurial
    /// appraisals (see [`ChainConfig::entrepreneurial_forecasts`]). `false` for every
    /// existing config, so the appraisals read the raw realized price and the run is
    /// byte-identical.
    entrepreneurial_forecasts: bool,
    /// C3R.h (L2): the role-choice appraisal values a recipe's input at a fresh, non-self
    /// reservation ask instead of the stale realized price (see
    /// [`ChainConfig::stale_input_price_fix`]). `false` for every existing config, so the
    /// appraisal reads `realized_price` and the run is byte-identical.
    stale_input_price_fix: bool,
    capital_payback_cycles: u32,
    tool_build_wood: u32,
    tool_build_labor: u32,
    capital_build_hunger_max: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LandShare {
    regen: u32,
    cap: u32,
    available: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SecureLandInheritanceRow {
    pub tick: u64,
    pub deceased: u64,
    pub heir: Option<u64>,
    pub plot: u32,
    pub regime: InheritanceRegime,
    pub pre_regen: u32,
    pub pre_cap: u32,
    pub post_regen: u32,
    pub post_cap: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MortalLandownerOwnerRow {
    pub owner: u64,
    pub lifespan: Option<u64>,
    pub household: Option<usize>,
    pub lineage_id: Option<usize>,
    pub reproduction_eligible: bool,
    pub in_birth_kinship_graph: bool,
    pub born_in_sim: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RivalSubsistenceCommonsState {
    pub stock: u64,
    pub cap: u64,
    pub regen: u64,
    pub phi_bps: u32,
    pub drawn_total: u64,
    pub unmet_total: u64,
    pub depleted_ticks: u64,
    pub shortfall_ticks: u64,
    pub eligible_need_total: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OwnerSurplusTelemetry {
    pub owner_age_at_first_claim: Vec<(u64, u64)>,
    pub owner_tenure_before_death: Vec<(u64, u64)>,
    pub owner_surplus_produced_minus_consumed: Vec<(u64, i64)>,
    pub owner_surplus_sold_before_death: Vec<(u64, u64)>,
    pub owner_inventory_at_death: Vec<(u64, u64)>,
    pub inherited_stock_to_heirs: u64,
    pub buyer_purchases_by_owner_age_cohort: Vec<(u64, u64)>,
    pub owner_seller_attributed_bought: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WageLaborStats {
    pub escrow_gold: Gold,
    pub open_escrows: usize,
    pub retained_earnings_total: Gold,
    pub wage_proceeds_bucket_total: Gold,
    pub hires_total: u64,
    pub hires_post_promotion: u64,
    pub distinct_workers: usize,
    pub distinct_employers: usize,
    pub below_ask_not_hired: u64,
    pub endowment_funded_wages: Gold,
    pub wage_financed_output_buys: Gold,
    pub nonowner_output_buys: Gold,
    pub circular_loop_turnovers: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShareTenancyStats {
    pub open_contracts: usize,
    pub contracts_total: u64,
    pub voluntary_contracts_total: u64,
    pub forced_contracts_total: u64,
    pub renewals_total: u64,
    pub distinct_workers: usize,
    pub distinct_owners: usize,
    pub worker_bread_income: u64,
    pub owner_bread_income: u64,
    pub worker_declined: u64,
    /// Eligible, gate-passing workers left over when no owner candidate remained to
    /// evaluate — NOT a bread-acceptance decline (spec-review P2: `worker_declined` means
    /// the ordinal acceptance itself failed, nothing else).
    pub worker_unmatched: u64,
    /// Worker-ticks admitted by the term forecast while the legacy instantaneous gate would have
    /// said the commons covered the worker. Runtime-only diagnostic; never canonicalized.
    pub forward_only_eligibility: u64,
    pub renewal_hints_total: u64,
    pub renewal_fed_out: u64,
    pub renewal_base_ineligible: u64,
    pub renewal_owner_not_candidate: u64,
    pub renewal_bread_declined: u64,
    pub renewal_matched_elsewhere: u64,
    /// Count of at-cap owner plot candidates offered to the share matcher over the run.
    pub owner_candidates_total: u64,
    pub owner_no_atcap_plot: u64,
    pub stock_opportunity_refusal: u64,
    pub reservation_collision: u64,
    pub share_stock_drawdown: u64,
    pub unattributed_share_deposit: u64,
    /// Un-converted contract-sourced grain the owner received in kind at dissolution
    /// (the term-boundary settle that closes the final-haul leak).
    pub owner_grain_settled: u64,
    pub successions_total: u64,
    pub heir_declined: u64,
    pub worker_re_declined: u64,
    pub post_succession_renewals: u64,
    pub final_open_succeeded: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InKindWageStats {
    pub open_contracts: usize,
    pub hires_total: u64,
    pub distinct_workers: usize,
    pub distinct_employers: usize,
    pub worker_advance_bread: u64,
    pub employer_bread_income: u64,
    pub expected_output_total: u64,
    pub worker_declined: u64,
    pub worker_unmatched: u64,
    pub owner_candidates_total: u64,
    pub owner_no_atcap_plot: u64,
    pub owner_insufficient_fund: u64,
    pub productivity_declined: u64,
    pub reservation_collision: u64,
    pub stock_drawdown: u64,
    pub unattributed_deposit: u64,
    pub employer_grain_settled: u64,
    pub endowment_funded_hires: u64,
    pub term_starvations: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EarnedProvisioningStats {
    pub from_immortal_consumers: Gold,
    pub from_gatherers: Gold,
    pub from_lineage: Gold,
    pub from_other_producer_households: Gold,
    pub external_earned_revenue: Gold,
    pub genuine_external_revenue: Gold,
    pub external_bread_trades: u64,
    pub genuine_external_bread_trades: u64,
    pub intra_household_sales: Gold,
    pub intra_household_bread_trades: u64,
    /// C3R.d disclosure: cross-household NON-bread sale proceeds credited `Earned` to a
    /// producer-house seller, split by buyer class so producer-to-producer (e.g. flour)
    /// recirculation stays visible to the accounting-loop reading instead of silently
    /// inflating "earned" funds. Runtime-only.
    pub non_bread_external_earned: Gold,
    pub non_bread_producer_class_earned: Gold,
    pub endowment_funded_provisioning: Gold,
    pub provisioning_transfers: u64,
    pub provisioning_gold: Gold,
    pub members_fed_by_purchase: u64,
    pub member_starvations: u64,
    pub funded_but_unfilled: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EarnedBuyerClass {
    ImmortalConsumer,
    Gatherer,
    Lineage,
    OtherProducerHousehold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EarnedGoldSource {
    Earned,
    Endowed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EarnedGoldLot {
    source: EarnedGoldSource,
    amount: Gold,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct EarnedProvisioningLedger {
    stats: EarnedProvisioningStats,
    per_seller_external: BTreeMap<AgentId, Gold>,
    buckets: BTreeMap<AgentId, EarnedGoldBuckets>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ShareContract {
    id: u64,
    owner: AgentId,
    worker: AgentId,
    node: NodeId,
    share_bps: u16,
    term: u16,
    opened_tick: u64,
    renewals: u16,
    cap_at_start: u32,
    /// Contract-sourced grain that has reached the worker's econ stock and has not yet
    /// been split or settled. This keeps worker-owned grain from being charged to a
    /// renewed/current contract merely because the worker converts it while a contract is live.
    grain_in_stock: u32,
    /// Split residue in bps-units of bread (0..10_000): carries the sub-unit worker credit
    /// across output batches so the cumulative worker payout is exactly
    /// `floor(cumulative_output · share_bps / 10_000)` — the identical floor the acceptance
    /// evaluator uses. `Cultivate` books ONE loaf per application, so flooring each batch
    /// independently would zero the worker's share at any `share_bps < 10_000` (review P1).
    /// Digested: it steers every future split.
    split_remainder_bps: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InKindWageContract {
    id: u64,
    employer: AgentId,
    worker: AgentId,
    node: NodeId,
    wage_bread: u32,
    term: u16,
    opened_tick: u64,
    grain_in_stock: u32,
    split_remainder_bps: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WageEscrow {
    id: u64,
    employer: AgentId,
    worker: AgentId,
    amount: Gold,
    wage: Gold,
    retained_funded: Gold,
    endowment_funded: Gold,
    qty: u32,
    opened_tick: u64,
    release_tick: u64,
    recipe: RecipeId,
    output_good: GoodId,
    output_qty: u32,
    input: Option<(GoodId, u32)>,
    delivered: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WageProceedsLot {
    amount: Gold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WageWorkerQuote {
    worker: AgentId,
    ask: Gold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WageHireCandidate {
    employer: AgentId,
    max_total_wage: Gold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ShareOwnerCandidate {
    owner: AgentId,
    node: NodeId,
    cap_at_start: u32,
}

type RenewalHintKey = (AgentId, AgentId, NodeId);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ShareRenewalHint {
    renewals: u16,
    from_succeeded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingShareSuccession {
    contract: ShareContract,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenewalFate {
    FedOut,
    BaseIneligible,
    OwnerNotCandidate,
    BreadDeclined,
    MatchedElsewhere,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TermNeedMember {
    agent: AgentId,
    hunger: u16,
    held_free_bread: u64,
    held_total_bread: u64,
}

pub type SecureLandShareRow = (u32, u64, u32, u32, u32);
type ImpartibleLandTransfer = (NodeId, AgentId, Option<AgentId>, (u32, u32));
type PartibleLandTransfer = (NodeId, AgentId, Vec<AgentId>, (u32, u32), u32, bool);

#[derive(Clone, Debug, PartialEq, Eq)]
struct LandPlotRecord {
    owner: Option<AgentId>,
    idle_counter: u16,
    reserved_for: Option<AgentId>,
    shares: BTreeMap<AgentId, LandShare>,
    stranded_regen: u32,
    stranded_cap: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LandPlotQuality {
    regen: u32,
    cap: u32,
    distance: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LandListingKind {
    Idle,
    Foreclosure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LandListing {
    ask: u64,
    kind: LandListingKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LandYieldTick {
    tick: u64,
    qty: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LandMarketPlotState {
    price: u64,
    listing: Option<LandListing>,
    last_sale_price: Option<u64>,
    last_sale_tick: Option<u64>,
    yield_history: VecDeque<LandYieldTick>,
}

impl LandMarketPlotState {
    fn new(price: u64) -> Self {
        Self {
            price,
            listing: None,
            last_sale_price: None,
            last_sale_tick: None,
            yield_history: VecDeque::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LandTitleSource {
    OriginalClaim,
    Inherited,
    Bought,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LandTitleHistory {
    ever_owned: bool,
    ever_sold: bool,
    current: Option<LandTitleSource>,
    ever_bought: bool,
    retained_through_priced_out: bool,
    foreclosed_out: bool,
    last_carrying_paid_tick: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LandSaleRecord {
    tick: u64,
    node: NodeId,
    buyer: AgentId,
    seller: AgentId,
    price: u64,
    rent: u64,
    good_plot: bool,
    foreclosure: bool,
}

pub type LandMarketSaleRow = (u64, u32, u64, u64, u64, u64, bool, bool);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WorkedLandEvent {
    agent: AgentId,
    node: NodeId,
    moved: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SubsistenceCommonsRequest {
    agent: AgentId,
    hunger: u16,
    need: u32,
}

/// S23a: why a plot last reverted to unowned. Kept on the runtime-only `land_lost_prior_owners`
/// trail so the non-vacuity reclaim counter can demand the spec's exact mechanic — a plot
/// *idle-lost* and then re-claimed by a different agent — rather than conflating it with a plot
/// vacated by a heirless death.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LandLossCause {
    Idle,
    Death,
}

/// S7.2: one in-flight per-builder capital project (a BuildMill / BuildOven). The
/// builder commits its OWN WOOD via [`start_project`] (booked `consumed_as_input` at
/// the start tick), advances the project with its own labor one unit per econ tick,
/// and on completion the tool credits its own stock (booked `produced`). A finished
/// or abandoned/dead-builder entry is dropped from the settlement's `capital_builds`,
/// so only `Forming` projects are ever stored. The full [`ProjectTemplate`] is kept
/// so the reused project lifecycle (`advance_project`/`complete_project_if_ready`) can
/// run against it each tick.
struct CapitalBuild {
    /// The builder's stable id — the agent whose WOOD/labor funds the build and whose
    /// stock the completed tool credits.
    builder: AgentId,
    /// The builder's colonist slot at start, for the acquired-tool marker.
    slot: usize,
    template: ProjectTemplate,
    project: Project,
}

/// S22a (runtime-only diagnostic): a produced-bread lot tagged with the class of the agent
/// that PRODUCED it (cultivated/baked the grain). Carried alongside the flat `produced`
/// balance so a bread→SALT sale can be attributed to who PRODUCED the bread (the entrant), not
/// who SOLD it — the seller's `cultivating` flag may already be false at trade time, and a
/// produced loaf may transfer (inheritance / resale) before it sells. Never digested.
/// S22c — one rolling-window econ tick's realized sale proceeds for one colonist: the
/// post-money SALT proceeds it realized from selling its OWN cultivated bread surplus
/// (`cultivation_proceeds`, attributed via `produced_lots` where `lot.producer == seller`) and
/// from any NON-cultivation sale (`outside_proceeds`, e.g. WOOD→SALT) — the realized outside
/// option. Only ticks on which the colonist realized SOME sale are stored; entries older than
/// [`RETURN_WINDOW`] are pruned. Both feed [`Settlement::profit_stay_active`], so the window is
/// FUTURE-BEHAVIOUR state digested ON-only (with the colonist roster), NOT runtime-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReturnTick {
    /// The econ tick this row was realized on.
    tick: u64,
    /// Post-money SALT proceeds (`price × own-produced qty`) from selling the colonist's OWN
    /// cultivated bread surplus this tick.
    cultivation_proceeds: u64,
    /// Post-money SALT proceeds from the colonist's NON-cultivation sales (e.g. WOOD→SALT) this
    /// tick — the realized outside option.
    outside_proceeds: u64,
}

/// S22f: the outcome of evaluating an eligible uncommitted agent's voluntary-commitment entry signal
/// against the SAME rolling cultivation-proceeds / outside-option data S22c uses. Reuses the data +
/// the entry floor, NOT the `profit_stay_active` exit helper (which is phrased for an already-
/// cultivating agent around the `cultivate_now` branch), to avoid phase/order ambiguity (Codex P2 #5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommitmentEntrySignal {
    /// The signal cleared: windowed realized cultivation proceeds ≥ entry floor over ≥1 sale tick AND
    /// the cultivation per-sale-tick rate ≥ outside rate + margin. Carries the signal value (windowed
    /// cultivation proceeds) recorded at uptake.
    Clears(u64),
    /// Below the entry floor (no / insufficient realized cultivation proceeds) — a below-floor
    /// non-committer that proves the signal DISCRIMINATES.
    BelowFloor,
    /// Above the floor but the cultivation rate did not beat the outside option — a real decision that
    /// went the other way (NOT a below-floor case, so it is not counted as discrimination evidence).
    AboveFloorLoses,
}

/// S22c: sum a colonist's rolling return window over the last `return_window` ticks ending at
/// `tick`, returning `(cult_sum, cult_sale_ticks, outside_sum, outside_sale_ticks)` — its windowed
/// realized cultivation-sale and non-cultivation-sale proceeds and the count of distinct ticks each
/// was realized on. A read-time window filter (so an entry the previous tick's prune left one step
/// stale is excluded) keeps the rate exactly the trailing `return_window` completed ticks.
fn window_return_sums(
    window: &VecDeque<ReturnTick>,
    tick: u64,
    return_window: u64,
) -> (u64, u64, u64, u64) {
    let mut cult_sum = 0u64;
    let mut cult_ticks = 0u64;
    let mut out_sum = 0u64;
    let mut out_ticks = 0u64;
    if return_window == 0 {
        return (0, 0, 0, 0);
    }
    for entry in window {
        if tick.saturating_sub(entry.tick) > return_window {
            continue;
        }
        if entry.cultivation_proceeds > 0 {
            cult_sum = cult_sum.saturating_add(entry.cultivation_proceeds);
            cult_ticks += 1;
        }
        if entry.outside_proceeds > 0 {
            out_sum = out_sum.saturating_add(entry.outside_proceeds);
            out_ticks += 1;
        }
    }
    (cult_sum, cult_ticks, out_sum, out_ticks)
}

/// One bread trade this tick, normalized to (seller-of-bread, buyer, what-the-buyer-gave, qty,
/// per-unit spot price). `spot_price` is `Some` for a spot-tape trade (so the S22c post-promotion
/// bread→SALT proceeds can be credited to the producing seller) and `None` for a barter trade.
#[derive(Clone, Copy, Debug)]
struct BreadTradeRow {
    seller: AgentId,
    buyer: AgentId,
    other_good: Option<GoodId>,
    qty: u64,
    spot_price: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProducedLot {
    /// The producing agent at production time.
    producer: AgentId,
    /// Whether the producer was a spatial **lineage** member (`household.is_some()`) at
    /// production time. Class is a stable per-agent property, so production-time and sale-time
    /// reads agree.
    lineage: bool,
    qty: u64,
}

/// S18: fold one side of a barter trade into the round-trip ledger. A side that ACCEPTS the
/// medium `IndirectFor{target}` credits pending; a side that SPENDS the medium for a good it
/// had earmarked decrements it (the means role completing). A free function so it borrows
/// only the ledger, not the whole settlement.
fn observe_round_trip_side(
    ledger: &mut MultigoodMoney,
    agent: AgentId,
    gives: GoodId,
    receives: GoodId,
    reason: BarterReason,
    medium: GoodId,
    qty: u64,
) {
    if receives == medium {
        if let BarterReason::IndirectFor { target } = reason {
            ledger.credit_indirect(agent, target, qty);
        }
    } else if gives == medium {
        ledger.spend_on_target(agent, receives, qty);
    }
}

/// S21d.1: one FIFO lot of tracked-food held by an agent — `qty` units that all entered via the
/// same `channel`. Lots are stored oldest-at-front so a debit draws the earliest acquisition first.
///
/// C3R.e (impl-67): `intervention` is an ORTHOGONAL origin flag — `true` for a unit that entered
/// through a finite intervention (A1's moved loaves, A2's endowment split, B's support mints). It
/// is preserved through EVERY transfer, INCLUDING the market-sale retag to `Bought` (a sale changes
/// the `channel`, never the origin flag), so it is resale-proof: an ignition loaf that leaves the
/// cohort and returns as `Bought` still reads as intervention-origin. Estate-to-COMMONS (a terminal,
/// economically inaccessible sink) consumes the flag. It rides the un-digested ledger, so it is
/// digest-free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FoodLot {
    channel: FoodChannel,
    qty: u64,
    intervention: bool,
    /// DH.b (impl-69): the **purchase identity** — the settled spot-trade id whose credit created
    /// this `Bought` lot. `None` off the closed marker (so every existing ledger run coalesces
    /// byte-identically) and for every non-`Bought` construction channel. Lifecycle (R2-7):
    /// splitting, inheritance, and birth transfer PRESERVE it; a resale OVERWRITES it with the
    /// new trade id; coalescing requires equality.
    identity: Option<u64>,
    /// DH.b (impl-69): the orthogonal **ultimate-construction-endowment taint** — set at
    /// construction for `SeededMinted` lots (the closed base's construction stock) and NEVER
    /// cleared; preserved through every transfer INCLUDING the market-sale retag. `false` off
    /// the closed marker. Coalescing requires equality.
    taint: bool,
}

/// One prepared project-input bid whose real order-book fate is classified after
/// `society.step()`, once the spot tender/reservation gate has actually run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BootstrapBidAttempt {
    producer: AgentId,
    input: GoodId,
    gross_money: u64,
}

/// S21d.2a: the **cross-tick bootstrap microtrace** — runtime-only instrumentation that localizes
/// the Exp-9 gate (`reservation_bid_for_money` → `allocated_money_before_rank`, `agent.rs:357/889`).
/// The phase order is load-bearing: input-bid overrides are prepared BEFORE `society.step()`
/// (`set_project_input_bid_overrides`), while the market consume eats INSIDE the step — so food a
/// producer buys in tick `t` cannot free its money to bid the same tick; it is eaten and the
/// hunger readback lands the NEXT tick, and only then can the bid post (if money remains). This
/// trace records, per active chain producer, the **buy → eat → bid** sequence and, crucially, WHY
/// a fed producer with a positive project-input ceiling does not get a real order-book bid: it is
/// `cashless` (no money at all) or its money is `reserved` (present but allocated to higher-ranked
/// wants/orders — the Exp-9 gate biting).
///
/// NOT serialized into `canonical_bytes` (diagnostic, like the acquisition ledger). Maintained only
/// while [`Settlement::acquisition_ledger_active`] holds (the probe enables both together).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct BootstrapTrace {
    /// Per active producer: the last econ tick it BOUGHT tracked food (the buy leg).
    last_food_buy_tick: BTreeMap<AgentId, u64>,
    /// Per active producer: the last econ tick it ATE tracked food (the eat leg, hunger relief).
    last_food_eat_tick: BTreeMap<AgentId, u64>,
    /// Cumulative producer-ticks a producer BOUGHT tracked food.
    food_buys: u64,
    /// Cumulative producer-ticks a producer ATE tracked food.
    food_eats: u64,
    /// Cumulative input-bid attempts reaching the reservation decision (an eligible producer with
    /// an imputable output price and unsold-output headroom — the point the Exp-9 gate adjudicates).
    bid_attempts: u64,
    /// Of those, input bids actually POSTED (the gate passed — money was free to reserve).
    bids_posted: u64,
    /// Of the posts, those by a producer that bought tracked food on the previous tick and ate
    /// tracked food on this tick before the bid gate — the **buy → eat → bid** bootstrap leg
    /// completing end to end.
    bids_posted_after_recent_buy: u64,
    /// Of the blocks, the producer held NO free money at all (cashless — never earned/kept money).
    bids_blocked_cashless: u64,
    /// Of the blocks, the producer held free money but the gate RESERVED it for a higher-ranked
    /// want (its own hunger) — the canonical Exp-9 deadlock the long-horizon-death arc localized.
    bids_blocked_reserved: u64,
    /// The first econ tick a recently-fed producer posted an input bid (the bootstrap firing), or
    /// `None` if it never did across the run (a Phase B deadlock).
    first_bootstrap_bid_tick: Option<u64>,
    /// Prepared attempts awaiting post-market classification. Drained every econ tick.
    pending_bid_attempts: Vec<BootstrapBidAttempt>,
}

impl BootstrapTrace {
    /// Record a producer buying tracked food at `tick`.
    fn observe_food_buy(&mut self, producer: AgentId, tick: u64) {
        self.last_food_buy_tick.insert(producer, tick);
        self.food_buys += 1;
    }

    /// Record a producer eating tracked food at `tick`.
    fn observe_food_eat(&mut self, producer: AgentId, tick: u64) {
        self.last_food_eat_tick.insert(producer, tick);
        self.food_eats += 1;
    }

    /// True only for the advertised cross-tick bootstrap leg: buy tracked food on tick `t`, eat
    /// tracked food at the start of `t + 1`, then let the input bid gate run later in `t + 1`.
    fn bought_then_ate_on_tick(
        &self,
        producer: AgentId,
        tick: u64,
        ate_food_this_tick: bool,
    ) -> bool {
        ate_food_this_tick
            && self
                .last_food_buy_tick
                .get(&producer)
                .is_some_and(|&buy_tick| buy_tick + 1 == tick)
    }

    /// Record an economically valid input-bid attempt before `society.step()`. The prepared
    /// override is classified after the step so `posted` means it reached the real spot order book
    /// or filled there, after tender/reservation checks.
    fn prepare_bid_attempt(&mut self, producer: AgentId, input: GoodId, gross_money: u64) {
        self.pending_bid_attempts.push(BootstrapBidAttempt {
            producer,
            input,
            gross_money,
        });
    }

    /// Record a post-market input-bid decision at `tick`: `posted` = the real order-book gate
    /// passed (the bid is live or filled); otherwise `gross_money` distinguishes a cashless block
    /// (no money at all) from a reserved block (money present but unavailable to this bid).
    /// `bought_then_ate` flags the buy → eat → bid leg.
    fn observe_bid_decision(
        &mut self,
        tick: u64,
        posted: bool,
        gross_money: u64,
        bought_then_ate: bool,
    ) {
        self.bid_attempts += 1;
        if posted {
            self.bids_posted += 1;
            if bought_then_ate {
                self.bids_posted_after_recent_buy += 1;
                if self.first_bootstrap_bid_tick.is_none() {
                    self.first_bootstrap_bid_tick = Some(tick);
                }
            }
        } else if gross_money == 0 {
            self.bids_blocked_cashless += 1;
        } else {
            self.bids_blocked_reserved += 1;
        }
    }
}

/// S21e.1: mutable runtime state behind [`SeededSurplusTraceSummary`]. Diagnostic
/// only, never serialized.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct SeededSurplusTrace {
    max_pre_promotion_seeded_sellers: usize,
    first_non_vacuous_tick: Option<u64>,
    cleared_bread_salt_indirect_for_wood: u64,
    live_bread_salt_indirect_for_wood_ticks: u64,
    seeded_offerable_surplus_exhausted_tick: Option<u64>,
}

impl Settlement {
    fn init_private_land_tenure(&mut self) {
        if !self.private_land_tenure_active() {
            return;
        }
        let Some(grain) = self.chain.as_ref().map(|chain| chain.content.grain()) else {
            return;
        };
        self.land_plots.clear();
        self.land_market_plots.clear();
        for idx in 0..self.world.node_count() {
            let node_id = NodeId(idx as u32);
            if self
                .world
                .node(node_id)
                .is_some_and(|node| node.good == grain)
            {
                self.land_plots.insert(
                    node_id,
                    LandPlotRecord {
                        owner: None,
                        idle_counter: 0,
                        reserved_for: None,
                        shares: BTreeMap::new(),
                        stranded_regen: 0,
                        stranded_cap: 0,
                    },
                );
                if self.land_market_active() {
                    let price = self.land_market_initial_price(node_id);
                    self.land_market_plots
                        .insert(node_id, LandMarketPlotState::new(price));
                }
            }
        }
    }

    fn init_commitment_norm_seed(&mut self, seed: u64) {
        if let Some(prevalence) = self.fixed_commitment_norm_prevalence() {
            for colonist in &mut self.colonists {
                let seeded = fixed_commitment_norm_seeded(seed, colonist.id, prevalence);
                colonist.adopts_commitment_norm = seeded;
                colonist.commitment_norm_seed_adopter = seeded;
            }
            return;
        }
        if !self.commitment_norm_spread_active() {
            return;
        }
        let share_bps = self
            .chain
            .as_ref()
            .map_or(0, |chain| chain.commitment_seed_share_bps)
            .min(COMMITMENT_NORM_SCORE_BPS as u16);
        if share_bps == 0 {
            return;
        }
        if self.group_payoff_imitation_active() && COMMITMENT_NORM_SEED_CLUSTER {
            self.init_commitment_norm_cluster_seed(seed, share_bps);
            return;
        }
        for colonist in &mut self.colonists {
            let seeded = commitment_norm_seeded(seed, colonist.id, share_bps);
            colonist.adopts_commitment_norm = seeded;
            colonist.commitment_norm_seed_adopter = seeded;
        }
    }

    fn init_commitment_norm_cluster_seed(&mut self, seed: u64, share_bps: u16) {
        let target = (self.colonists.len().saturating_mul(usize::from(share_bps))
            / usize::try_from(COMMITMENT_NORM_SCORE_BPS).expect("score bps fits usize"))
        .max(1)
        .min(self.colonists.len());
        let center = commitment_norm_seed_cluster_center(
            seed,
            self.world.grid().width(),
            self.world.grid().height(),
        );
        let exchange_pos = self
            .world
            .stockpile(self.exchange)
            .map(|stockpile| stockpile.pos)
            .unwrap_or(Pos::new(0, 0));
        let mut ranked: Vec<(u32, u64, usize)> = self
            .colonists
            .iter()
            .enumerate()
            .map(|(slot, colonist)| {
                let pos = self.commitment_norm_seed_anchor_pos(colonist, exchange_pos);
                (pos.manhattan(center), colonist.id.0, slot)
            })
            .collect();
        ranked.sort_unstable_by_key(|&(distance, id, _)| (distance, id));
        for &(_, _, slot) in ranked.iter().take(target) {
            self.colonists[slot].adopts_commitment_norm = true;
            self.colonists[slot].commitment_norm_seed_adopter = true;
        }
    }

    /// S22e: endow a MINORITY of lineage households with one durable cultivation tool (the plow) at
    /// generation. A no-op unless [`Self::endowed_cultivation_capital_active`] holds (the flag is on
    /// AND the durable-cultivation-capital path is active), so every existing config is
    /// byte-identical. The endowment is restricted to LINEAGE households — the only agents
    /// inheritance can operate on (`heir_for` needs a `household`), the disclosed S22e limitation.
    ///
    /// Selection is a DETERMINISTIC hash of `(seed, household_id)` over the eligible household set,
    /// ranked by hash (NOT lowest-ids, which could privilege a special roster segment), taking the
    /// first `endowed_tool_count`. Each selected household's FOUNDING member (its first founder, in
    /// colonist-insertion order) is granted one plow into agent stock. The grant lands BEFORE the
    /// first `econ_tick`, so it is part of the tick-0 whole-system baseline — a conservation-safe
    /// INITIAL endowment, no earning required, never a mint. Records the endowed-tool count (the
    /// §3.5 invariant term), the granted household indices (digested ON-only + a diagnostic), and
    /// the granted member ids (a diagnostic).
    fn apply_endowed_cultivation_capital(&mut self, seed: u64, config: &SettlementConfig) {
        debug_assert_eq!(
            self.econ_tick, 0,
            "S22e endowments must be applied before the initial conservation baseline"
        );
        if !self.endowed_cultivation_capital_active() {
            return;
        }
        let Some(tool) = self.cultivation_tool_good() else {
            return;
        };
        let count = self
            .chain
            .as_ref()
            .map_or(0, |c| usize::from(c.endowed_tool_count));
        let household_count = config
            .demography
            .as_ref()
            .map_or(0, |demo| demo.households.len());
        if count == 0 || household_count == 0 {
            return;
        }
        // Rank every lineage household by the deterministic endowment hash and take the first
        // `count` (clamped to the roster). Re-sort the winners by index so the digested selection +
        // the grant order are roster-stable.
        let mut ranked: Vec<usize> = (0..household_count).collect();
        ranked.sort_by_key(|&h| (endowment_hash(seed, h), h));
        ranked.truncate(count.min(household_count));
        ranked.sort_unstable();
        let mut granted_households = Vec::new();
        for &household in &ranked {
            // The household's founding member: its first founder in colonist-insertion order.
            let Some(founder_id) = self
                .colonists
                .iter()
                .find(|c| c.household == Some(household))
                .map(|c| c.id)
            else {
                continue;
            };
            // A conserved, id-addressed stock credit (the same primitive the estate path uses); it
            // returns false only for a missing agent, in which case nothing is granted or counted.
            if self.society.credit_stock(founder_id, tool, 1) {
                self.endowed_cultivation_tools_total =
                    self.endowed_cultivation_tools_total.saturating_add(1);
                self.endowed_member_ids.push(founder_id);
                granted_households.push(household);
            }
        }
        self.endowed_households = granted_households;
    }

    /// Advance the settlement by one economic tick (the module's documented
    /// phase order). Returns — and stores — the conservation + flow
    /// [`EconTickReport`].
    pub fn econ_tick(&mut self) -> EconTickReport {
        self.shadow_cycle_cache.get_mut().take();
        // S8.0 emergence probe: whether the colony has not yet promoted a money good
        // as of the start of this tick (the promotion itself happens in the market
        // step below). The pre-promotion hunger pressure is accumulated end-of-tick
        // while this holds, so it freezes on the promotion tick (inclusive).
        let was_pre_promotion = self.promoted_at_tick().is_none();
        // ---- 0. ACQUISITION LEDGER (S21d.1): one-time bootstrap sweep of the generated seed
        // bread into the SeededMinted channel, before any death/provision/market moves it, so
        // the runtime-only ledger starts in lockstep with held stock. A no-op off the gated
        // path (and after the first active tick).
        self.maybe_init_acquisition_ledger();
        // DH.a (impl-68): reset the closure per-tick aggregate (pure observation).
        if self.closure_active() {
            self.closure_begin_tick();
        }
        let mut report = EconTickReport {
            econ_tick: self.econ_tick,
            fast_ticks: FAST_TICKS_PER_ECON_TICK,
            ..EconTickReport::default()
        };
        self.record_producer_house_person_ticks();

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
        let fast = self.run_fast_loop();
        self.land_market_finalize_rent_tick();
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
        // S22b: accumulate per-agent grain hauled (the cultivation input good deposited this
        // tick) — the monopolization / grain-share probe + the non-vacuity grain measure.
        // Runtime-only (never digested), so extending the gate touches no golden: maintained on
        // the active cultivation-skill path OR (S22c) the profit-driven-retention path, whose
        // skill-OFF headline still needs the per-agent grain-share monopolization probe.
        if self.cultivation_skill_active()
            || self.profit_driven_retention_active()
            || self.private_land_tenure_active()
        {
            if let Some(grain) = self.cultivation_input_good() {
                for (&(id, good), &qty) in &fast.deposited {
                    if good == grain {
                        *self.cultivation_grain_harvested.entry(id).or_insert(0) += u64::from(qty);
                    }
                }
            }
        }
        self.record_pending_deposits(fast.deposited);
        report.transferred = self.transfer_pending_deposits();
        // DH.a: the world→econ gathered-node deposits (own_produced credits).
        if self.closure_active() {
            self.closure_phase(closure::ClosurePhase::Gather);
        }
        self.refresh_private_land_carried_sources();
        // S18: accumulate the WOOD relocated node→econ this tick — the provenance bound for
        // traded WOOD (with every WOOD buffer + the mint zeroed, all WOOD enters here). A
        // no-op off the multi-good path; the counter is runtime-only (not digested).
        if self.multigood_money_active() {
            let gathered = report.transferred.get(&WOOD).copied().unwrap_or(0);
            self.multigood.wood_gathered = self.multigood.wood_gathered.saturating_add(gathered);
        }

        // Wage labor released by the normal phase is recorded after the market step because
        // `Society::step` clears the tick-local labor log at entry. A due escrow can also be
        // released inside death settlement below, so the scratch log has to exist before deaths.
        let mut wage_labor_used = Vec::new();
        // C3R.e-obs §3.2c: start/pre-death physical-stock and reservation seam.
        self.saving_obs_begin_stock_tick();

        // ---- 3. NEEDS + real death (G4a): settle each starvation death's estate to
        // the household heir (G4b) or the commons (G4a fallback), free its arena
        // slot, reconcile the society's caches.
        report.deaths = self.update_needs_and_remove_dead(&mut report, &mut wage_labor_used);

        // ---- 3b. AGING + OLD-AGE DEATH (G4b): advance each living householder's age
        // and remove any that reach their deterministic lifespan, routing the estate
        // to a household heir (commons if the lineage is extinct). Reuses G4a's
        // removal path; a no-op without a demography overlay. Deterministic — the
        // lifespan is a function of the stable seed, nothing is drawn.
        report.deaths += self.age_and_remove_elderly(&mut report, &mut wage_labor_used);
        self.saving_obs_capture_death_phase();
        // DH.a: route the estates of agents that died this tick (heir / commons drain).
        if self.closure_active() {
            self.closure_observe_estates();
        }

        // DH.b-obs: open the staple-stock observation window at WindowStart — RIGHT AFTER
        // death/estate (so estate-to-heir credits fold into the baseline) and BEFORE
        // `regenerate_scales` (whose quote cancellations emit AskChange). Observation stays live
        // through `regenerate_scales` and `society.step()`, drained/disabled right after.
        self.birth_gate_obs_begin_window();

        // ---- 4. SCALES.
        self.regenerate_scales();

        // ---- 4a-bis. CAPITAL FORMATION (S7.2): each fed, non-latent colonist with
        // saved WOOD that appraises building a mill/oven will pay commits its own WOOD
        // (a conserved project), advances any in-flight build with its own labor, and
        // on completion credits the tool to its own stock. After the scale regeneration
        // (so the appraisal reads this tick's needs) and BEFORE role-choice (so a tool
        // completed this tick is adopted the same tick). If any tool completes,
        // regenerate the scales immediately so the fresh tool-holder carries its
        // tool-anchor into the market step (the phase-order trap — it must not post the
        // just-built capital as surplus). A no-op unless `producible_capital` is on, so
        // every other run is byte-identical.
        let mut capital_labor_used = Vec::new();
        if self.run_capital_formation(&mut report, &mut capital_labor_used) {
            self.regenerate_scales();
        }
        // DH.a: WOOD → mill/oven tool completions (own_produced tools, endowed input debits).
        if self.closure_active() {
            self.closure_phase(closure::ClosurePhase::Capital);
        }

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

        // ---- 4b-bis. PRODUCTIVE RE-ENTRY (S6): a hungry spatial non-lineage
        // colonist adopts edible-grain gathering on its own value scale (an idle
        // consumer / a WOOD-mis-allocated gatherer becomes a grain gatherer), and a
        // fed re-entrant reverts to its home role (the hysteresis). After role-choice
        // (so this tick's vocations are settled) and before the market. The flip is
        // between two untooled spatial roles whose econ scale is identical, so it
        // perturbs no resting quote and needs no scale regeneration — it only steers
        // the NEXT fast loop's `assign_idle_gatherer_tasks`. Mints nothing; a no-op
        // unless `ChainConfig::productive_reentry` is set, so every other run is
        // byte-identical.
        self.run_productive_reentry();

        // ---- 4b'. CAPITAL ADVANCE (EXPERIMENT): a conserved working-capital
        // advance — move real money from the richest saver to any cashless active
        // chain producer so it can buy inputs ahead of selling output. After
        // role-choice (producers are in role) and before the market clears (the
        // advanced cash funds this tick's input bids). A no-op unless enabled and
        // money has emerged, so every other run is byte-identical.
        self.run_capital_advance();

        // ---- 4b''. IN-KIND SUBSISTENCE ADVANCE (EXPERIMENT): feed hungry active
        // producers staple food in kind (from the richest holder) so their bread
        // want is provisioned and their money frees to bid for inputs. After the
        // capital advance (so funded producers are also fed), before the market.
        // Conserved; a no-op unless enabled.
        self.run_subsistence_advance();

        // ---- 4c. PROVISION (G4b): deliver each living householder its household's
        // renewable hunger-staple/WOOD hearth into econ stock, recorded as a source
        // (`report.endowment`). Mirrors `life::Camp`'s harvest delivery — after the
        // scale regeneration (the stock add does not change the scale, so no resting
        // quote goes stale), before the market clears. A no-op without a demography
        // overlay.
        self.deliver_demography_provisions(&mut report);

        // ---- 4c'. LOCAL PRODUCER SUBSISTENCE (S5): top each chain producer (and
        // the latent pool) up to a small staple floor from its OWN renewable
        // household hearth — minted fresh, like the demography provision, not taken
        // from any other agent. The local household/subsistence base the
        // specialization sits on, so a producer's money frees for input bids; a
        // no-op unless enabled, so every other run is byte-identical.
        self.run_producer_subsistence(&mut report);

        // ---- 4c'-C3R.c. EARNED / STOCK PRODUCER-HOUSE PROVISIONING: after scales
        // and the existing pre-market provisions, before the market. The earned
        // headline moves conserved GOLD from active producers to same-household
        // members with unprovided Now bread wants; the control moves the producer's
        // own bread stock instead. Both are no-ops off their gates.
        let earned_funded_bid_members = self.run_earned_provisioning_transfers();
        self.run_producer_stock_provisioning_control();
        // DH.a: the (retired-in-NoIgnition) hearth + producer-subsistence deliveries — endowed
        // support credits (B arms only; a no-op in NoIgnition where all provisions are 0).
        if self.closure_active() {
            self.closure_phase(closure::ClosurePhase::Support);
        }

        // ---- 4c''. OWN-LABOR SUBSISTENCE (S12): with the food mints retired, give each
        // hungry, eligible, unprovisioned colonist with spare labor a labor-produced
        // FORAGE floor — credited to its OWN stock as `report.produced` (own labor), not
        // a mint, and eaten this tick (FORAGE is `known.subsistence`, ranked below
        // bread). Sets the `foraging` flag that steers the next fast loop to forage
        // instead of harvesting WOOD (the structural opportunity cost). After the
        // (retired) hearth phases, before the market, so the floor is on hand to eat. A
        // no-op unless the gated own-labor path is active, so every other run is
        // byte-identical.
        self.run_own_labor_subsistence(&fast.foraged, &mut report);
        // DH.a: labor-produced own-consumption subsistence (own_produced + own consumption).
        if self.closure_active() {
            self.closure_phase(closure::ClosurePhase::OwnUse);
        }

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

        // ---- 4e. PROJECT-AWARE INPUT BID OVERRIDES (S2, the endogenous fix): for
        // each active producer, set a gated econ spot-bid override for its recipe
        // input at the reservation IMPUTED from the project-bundle appraisal (the
        // same machinery role-choice uses to adopt, not the recipe-blind scale
        // bid). The override is consumed by this tick's `society.step()`: the bid
        // enters the real order book, reserves the producer's own gold, and fills
        // against a willing seller (a gatherer's grain, a miller's flour) — the
        // input acquired by market trade, not placed. A no-op (no overrides set)
        // unless [`ChainConfig::project_input_bids`] is on, so every other run is
        // byte-identical.
        self.set_project_input_bid_overrides();

        // ---- 4f. IN-KIND WAGE (C1N): before the share match, expire due bread-wage
        // contracts and match owner-funded fixed-advance hires over the same at-cap plots.
        // Newly opened contracts reserve their plot, so the C1R share phase below cannot
        // double-contract it in the same tick.
        self.run_in_kind_wage_phase();

        // ---- 4g. SHARE TENANCY (C1R): before the goods market, expire due contracts and
        // match new no-advance output-share contracts. Newly opened contracts steer the next
        // fast loop; live contracts from prior ticks have already hauled in this tick's fast
        // loop and split in the own-use cultivation phase below.
        self.run_share_tenancy_phase();

        // ---- 4h. WAGE LABOR (C1): after all pre-market bids are prepared but before the
        // goods market, release any due wage escrows from prior matches (worker labor produces
        // the owner's bread, wage escrow pays the worker), then match new owner-funded hires.
        // The phase is post-promotion-only and default-off, so existing runs never see it.
        // The worker's delivered wage-labor is accumulated here and recorded AFTER the market
        // step, mirroring `capital_labor_used`: every `Society::step` variant clears
        // `tick_labor_used` at entry, so recording the labor now would erase it before the
        // needs readback and let the worker spend a full labor budget again this same tick.
        self.run_wage_labor_phase(&mut report, &mut wage_labor_used);

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
        let birth_stock_attribution_snapshot = self.birth_stock_attribution_snapshot();
        if self.saving_allocation_obs_active() {
            self.last_birth_stock_attribution_snapshot = birth_stock_attribution_snapshot.clone();
        }
        // C3R.e-obs: close the post-death/pre-market provisioning seam.
        self.saving_obs_capture_pre_market(&report);
        // C3R.e-obs (§3.2a/§5): capture the PRE-market offerable staple supply here — the
        // live ask path evaluates this same state inside the step below. Committed to the
        // supply series by the join iff this becomes a money-priced spot pass.
        self.saving_obs_capture_offerable_supply(&birth_stock_attribution_snapshot);
        let money_good_before = self.society.current_money_good();
        let society_gold_before = self.society.total_gold();
        // S16: the produced-bread provenance ledger reads THIS tick's market trades as
        // the suffixes past here. Pre-promotion bread moves on the retained barter log;
        // post-promotion bread moves on the spot tape.
        let provenance_barter_trades_start = self.society.barter_trades.len();
        let provenance_spot_trades_start = self.society.trades.len();
        let pre_market_seeded_sellers = if was_pre_promotion {
            self.seeded_offerable_wood_seller_count()
        } else {
            0
        };
        if self.barter.is_some() {
            self.society
                .step_rejecting_v2_money_goods(&self.money_rejection_goods);
        } else {
            self.society.step();
        }
        // DH.b-obs: drain the Society staple-stock tape (Consumption/SettledTrade/AskChange) into
        // the joined event tape and disable recording immediately after the step. Production and
        // the gate/BirthDebit events are appended by the settlement below.
        self.birth_gate_obs_drain_after_step();
        self.finalize_bootstrap_bid_attempts(provenance_spot_trades_start);
        for (agent, labor) in capital_labor_used {
            self.society.record_external_labor_used(agent, labor);
        }
        // Wage labor delivered in this tick's escrow release (§4.5): recorded after the step
        // cleared `tick_labor_used`, so the worker's wage-labor reduces the labor budget the
        // needs readback and next tick see (a no-op off the wage path — the vec stays empty).
        for (agent, labor) in wage_labor_used {
            if let Some(worker) = self.society.agents.get_mut(agent) {
                worker.labor_capacity = worker.labor_capacity.max(labor);
            }
            self.society.record_external_labor_used(agent, labor);
        }
        // ---- 5b. CAPITAL-ADVANCE REPAYMENT (EXPERIMENT): borrowers repay their
        // revolving working-capital loans from this tick's sales, staying
        // cash-light so the future-money want stays unmet. Conserved; no-op
        // unless the capital-advance experiment is enabled and loans are open.
        self.run_capital_repayment();
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
        let society_gold_after_market = self.society.total_gold();
        if money_good_before.is_none() {
            if let Some(emerged) = self.society.current_money_good() {
                let minted = society_gold_after_market
                    .0
                    .saturating_sub(society_gold_before.0);
                report.promoted.insert(emerged, minted);
            }
        }
        self.record_bread_seller_provenance(provenance_barter_trades_start);
        self.observe_seeded_surplus_probe_tick(
            provenance_barter_trades_start,
            pre_market_seeded_sellers,
            was_pre_promotion,
        );

        // ---- 5b-bis. PROVENANCE: MARKET (S16): sink this tick's market-consume bread and
        // attribute the bread→medium trades produced-vs-minted, in the within-step order
        // (consume eats before the barter clears). The cursor marks the consumed-log prefix
        // already sinked, so the own-use consume below is not re-counted. No-op off the path.
        let provenance_consume_cursor = self.run_bread_provenance_market(
            provenance_barter_trades_start,
            provenance_spot_trades_start,
            was_pre_promotion,
        );

        // ---- 5b-bis-S22c. CULTIVATION-RETURN WINDOW (S22c): fold this tick's realized
        // cultivation-sale proceeds (filled by the provenance market pass) and outside (non-bread)
        // spot-sale proceeds into each colonist's rolling return window — the future-behaviour
        // signal the profit-stay exit reads next tick. A no-op off the profit-driven-retention
        // path, so every other run is byte-identical.
        self.update_cultivation_returns(provenance_spot_trades_start);

        // ---- 5b-bis'. ACQUISITION LEDGER: MARKET (S21d.1): debit this tick's market-consume
        // bread FIFO (by the channel it arrived through) and transfer the bread trades
        // seller→buyer as `Bought`, in the same within-step order as the provenance pass. The
        // cursor marks the consumed-log prefix already debited so the own-use consume below is
        // not re-counted. No-op off the path.
        let acquisition_consume_cursor = self
            .run_acquisition_market(provenance_barter_trades_start, provenance_spot_trades_start);
        self.run_wage_labor_market_attribution(provenance_spot_trades_start);
        self.finalize_earned_provisioning_market(
            provenance_spot_trades_start,
            &earned_funded_bid_members,
        );
        self.run_earned_provisioning_market_attribution(provenance_spot_trades_start);
        self.record_birth_stock_attributable_purchases(
            provenance_spot_trades_start,
            &birth_stock_attribution_snapshot,
        );
        // C3R.e-obs (impl-66): the §2 loss-decomposition join over THIS tick's drained
        // allocation trace. Runs right after the market so the trace batch is still fresh
        // and the intent map is not yet pruned. A no-op unless the obs is active.
        self.observe_saving_allocation(
            provenance_spot_trades_start,
            &birth_stock_attribution_snapshot,
        );
        self.saving_obs_capture_post_market(&report);

        // ---- 5b-bis''. LAND MARKET (S23b): after ordinary food/SALT trades, and only once
        // SALT has promoted, charge carrying costs and run the deterministic pairwise land sweep.
        // This preserves the bootstrap and lets the budget-hysteresis test observe agents that spent
        // SALT on food before trying to re-buy land.
        self.run_land_market();
        // NOTE: `total_gold_after_step` is deliberately NOT snapshotted here. It is taken
        // at true end-of-tick (after births, birth-stock injections, ignition, and
        // spoilage) so the per-tick money identity covers the post-market phases too —
        // a mint/burn bug in the birth-endowment or ignition path cannot re-baseline
        // itself away each tick.
        // DH.a: observe the settled market batch in its real order — authoritative consumption log
        // first, then the tick's spot trades (both physical legs + the gold sale-split). Reconciles
        // buckets post-market-batch.
        if self.closure_active() {
            self.closure_observe_market(provenance_spot_trades_start);
            // DH.b (impl-69, R4-1): validate this tick's purchase-credit-seam facts against the
            // settled-trade records the observation above just recorded. Mismatches are seam
            // violations the integration suite hard-fails on.
            self.burden_validate_purchase_credits();
        }

        // ---- 5b-ter. MULTI-GOOD MONEY INSTRUMENTATION (S18): trace this tick's barter
        // trades for the WOOD↔medium leg (the WOOD provenance bound) and the
        // pending-indirect-SALT round-trip ledger. The round-trip ledger also reads this
        // tick's spot trades, so a means role that completes only POST-promotion (the medium
        // accepted in barter, spent on the target as money) still counts. Runtime-only (not
        // digested), a no-op for a settlement with no barter medium.
        self.run_multigood_instrumentation(
            provenance_barter_trades_start,
            provenance_spot_trades_start,
            was_pre_promotion,
        );

        // ---- 5c. IN-KIND INPUT ADVANCE (EXPERIMENT): a capitalist buys each
        // active producer's recipe input in kind from the richest holder and
        // places it in the producer's hands — before production, so the producer
        // mills/bakes it this tick. Bypasses the value-scale bid gate. Conserved;
        // a no-op unless enabled.
        self.run_input_advance();

        // ---- 6. PRODUCTION (G3a): each living producer applies its recipe to the
        // input it now holds, transforming it into output. A conserved conversion:
        // the input consumed and the output produced are both recorded so the
        // whole-system ledger accounts every transformed unit. Runs after the
        // market (so the input a producer just bought is on hand) and is a no-op
        // for a plain settlement (no chain).
        let bread_produced_before = self
            .chain
            .as_ref()
            .map_or(0, |chain| report.produced_of(chain.content.bread()));
        self.run_production(&mut report);
        // Capture the Baker-only phase delta before own-use cultivation widens the report.
        let baker_bread_produced = self.chain.as_ref().map_or(0, |chain| {
            report
                .produced_of(chain.content.bread())
                .saturating_sub(bread_produced_before)
        });
        // DH.a: miller/baker recipe production (input debit, own_produced output).
        if self.closure_active() {
            self.closure_phase(closure::ClosurePhase::Production);
        }

        // ---- 6a-bis. OWN-USE CULTIVATION (S15): each cultivating colonist converts the
        // grain it hauled this tick into bread by its own labor (booked
        // produced/consumed_as_input, never minted) and eats part of it through the
        // consumption-readback seam so its hunger falls — the rest stays in stock to
        // endow children. After the market (so the bread is minted post-clearing and
        // never offered for sale — own-use) and BEFORE births (so a parent's
        // just-cultivated bread can endow this tick's newborn). A no-op off the gated
        // cultivation path, so every other run is byte-identical.
        self.run_own_use_cultivation(&mut report);

        // ---- 6a-bis-skill. CULTIVATION SKILL (S22b): advance each colonist's bounded
        // cultivation skill from this tick's realized cultivation output — gain for the agents
        // that actually harvested grain AND converted it to bread, decay for everyone else.
        // After the own-use cultivation phase (so its producer set is filled) and before births
        // (a newborn is added later, born at skill 0). A no-op off the gated skill path, so
        // every other run is byte-identical.
        self.run_cultivation_skill();

        // ---- 6a-bis-capital. CULTIVATION CAPITAL FORMATION (S22d): credit each cultivator's
        // realized-output tenure, advance/complete in-flight durable-cultivation-tool builds, and
        // start a new build for each sustained-producing cultivator that can afford the sunk
        // WOOD+labor cost. SEPARATE from the money-gated mill/oven capital phase (it can build
        // PRE-money). After the own-use cultivation + skill phases (so the producer set is filled)
        // and before births. A no-op off the gated path, so every other run is byte-identical.
        self.run_cultivation_capital_formation(&mut report);

        // ---- 6a-bis'. EMERGENCY SELF-PROVISIONING (S21h.1): the demand-side survival
        // bridge — each hungry non-lineage Consumer/Gatherer that reached the emergency
        // threshold produces (own labor, no input) and immediately eats just enough bread to
        // pull its hunger one notch below the threshold, so it survives WITHOUT being
        // satiated out of the bread market (demand-preserving). Conserved (produced ==
        // eaten, never offered), credited SelfProduced. After the own-use cultivation phase
        // and BEFORE the own-use consume passes (so its consume-log tail is sinked by them).
        // A no-op off the gated seam, so every other run is byte-identical.
        self.run_emergency_self_provision(&mut report);
        // DH.a: own-use cultivation + the emergency subsistence floor (own_produced + consumption).
        if self.closure_active() {
            self.closure_phase(closure::ClosurePhase::OwnUse);
        }

        // ---- 6a-ter. PROVENANCE: OWN-USE CONSUME (S16): sink the cultivators' own-use bread
        // consume (the consumed-log tail past the market pass), produced-first. No-op off path.
        self.run_bread_provenance_own_use(provenance_consume_cursor);

        // ---- 6a-quater. ACQUISITION LEDGER: OWN-USE CONSUME (S21d.1): debit the cultivators'
        // own-use bread consume (the consumed-log tail past the market pass) FIFO. No-op off path
        // (and inert in the probe — cultivation is off).
        self.run_acquisition_own_use(acquisition_consume_cursor);
        self.saving_obs_capture_post_production(&report);

        // ---- 6b. BIRTHS (G4b): each food-secure household under its size cap and
        // past its birth interval bears one child — a new colonist with an inherited,
        // mutated culture and a conserved endowment transferred from a parent, added
        // via `Society::add_agent` so it participates from the NEXT econ tick. Runs
        // after the market so the newborn does not trade the tick it is born, and
        // before the after-snapshot so its (transferred-in) holdings balance the
        // parent's debit. A no-op without a demography overlay; draws no randomness.
        let birth_stock_injections = self.run_birth_stock_sufficiency_control();
        // C3R.e (impl-67): the A1 one-shot ignition fires here (before the births), independent of
        // the recurring SufficiencyControl gate above — a no-op unless `birth_stock_ignition_at`
        // matches this tick.
        self.run_birth_stock_ignition();
        self.observe_birth_stock_holdings();
        // DH.b-obs: the independent denominator recount snapshot — a pre-`run_births` capture of
        // every producer household's gate inputs, from which the recount replays the strata
        // WITHOUT reading the observer or any `birth_block_*` counter (§4a).
        self.capture_birth_gate_recount();
        report.births = self.run_births();
        self.record_birth_stock_control_results(&birth_stock_injections);
        self.saving_obs_capture_post_birth();

        // ---- 7. READ-BACK happens at the top of the next tick's NEEDS phase.

        // Conservation receipt: consumed (the eating sink) is this tick's
        // consumption log; the whole-system after-totals (taken AFTER production and
        // births) must balance against before + regen + endowment + produced −
        // consumed_as_input − consumed − promoted for every good (births/deaths move
        // goods within the whole system, so they need no term).
        let owner_bread = self.provenance_bread_good();
        let consumed_log: Vec<(AgentId, GoodId, u32)> =
            self.society.consumption_log_last_tick().to_vec();
        for (agent, good, qty) in consumed_log {
            *report.consumed.entry(good).or_insert(0) += u64::from(qty);
            if Some(good) == owner_bread && self.current_or_ever_landowner(agent) {
                *self.owner_bread_consumed.entry(agent).or_insert(0) += u64::from(qty);
            }
        }
        // ---- 7. SPOILAGE (EXPERIMENT): decay perishable food holdings, a real
        // sink recorded in `report.spoiled`. After all production/consumption so
        // it decays end-of-tick holdings, before the whole-system snapshot so the
        // conservation identity accounts it. A no-op unless enabled.
        self.run_spoilage(&mut report);
        // DH.a: per-agent perishable decay (a recorded physical sink).
        if self.closure_active() {
            self.closure_phase(closure::ClosurePhase::Spoilage);
        }
        // Close the post-birth/end-of-tick seam after the only later physical sink.
        self.saving_obs_finish_stock_tick(&report);
        // ---- 7b. PROVENANCE FINALIZE (S16): record the first produced-surplus tick and
        // assert the provenance ledger conserves. A no-op off the path.
        self.finalize_bread_provenance();
        // ---- 7b'. ACQUISITION LEDGER FINALIZE (S21d.1): assert the channel ledger still equals
        // the tracked food actually held (every inflow credited, every outflow debited). No-op
        // off the path.
        self.finalize_acquisition_ledger();
        self.update_seeded_offerable_surplus_exhaustion();
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
        // §5 money/escrow invariant, spanning the WHOLE tick: total gold (agents +
        // commons + land-fee pool + wage escrow) at end-of-tick — after deaths/estates,
        // the market, births, birth-stock injections, ignition, and spoilage — equals
        // the pre-fast total plus only what the promotion channel minted. Every other
        // phase may only transfer gold, never create or destroy it. `report.conserves()`
        // is per-good and does NOT cover money, so this is the escrow-inclusive money
        // check. The assert is scoped to `wage_labor_active()` (the closed-gold C1
        // configs the acceptance suite exercises) rather than unconditional, since
        // ledger-money (M3) regimes are not a closed-gold total.
        report.total_gold_after_step = self.total_gold().0;
        debug_assert!(
            !self.wage_labor_active() || report.money_conserves(),
            "wage-labor money/escrow invariant: gold + escrow must conserve across the \
             whole tick except at the promotion channel"
        );

        // S8.0 emergence probe: while the colony is still pre-promotion, track the
        // hunger trough the emergence window must survive (Tension A) — the peak
        // hunger reached and the count of ticks at/over the critical ceiling. Pure
        // read-back of end-of-tick need state; not serialized, so determinism and the
        // goldens are untouched.
        if was_pre_promotion {
            let critical = self.dynamics.hunger_critical;
            let mut at_critical = false;
            for &slot in &self.live_colonist_slots {
                let hunger = self.colonists[slot].need.hunger;
                self.peak_pre_promotion_hunger = self.peak_pre_promotion_hunger.max(hunger);
                at_critical |= hunger >= critical;
            }
            if at_critical {
                self.critical_ticks_pre_promotion += 1;
            }
        }

        // DH.a: finalize the closure per-tick aggregate (CC0 living, CC3 boundary sinks), reconcile
        // end-of-tick, and push it. Pure observation — before the tick counter advances.
        if self.closure_active() {
            self.closure_finalize_tick();
        }
        self.record_commitment_norm_observations();
        // Pure observation after every phase that can move goods or gold this tick.
        self.observe_baker_round_trip(provenance_spot_trades_start, baker_bread_produced);
        self.econ_tick += 1;
        self.last_report = report.clone();
        report
    }

    /// Fold this tick's tape suffix and Baker production into non-digested telemetry.
    /// Vocations are read here so later ticks cannot relabel settled cash flows.
    fn observe_baker_round_trip(&mut self, spot_trades_start: usize, bread_produced: u64) {
        let Some(chain) = self.chain.as_ref() else {
            return;
        };
        let flour = chain.content.flour();
        let bread = chain.content.bread();
        let mut flour_gold_spent = 0u64;
        let mut bread_gold_earned = 0u64;
        let mut bread_units_sold = 0u64;
        for trade in &self.society.trades[spot_trades_start..] {
            let value = trade.price.0.saturating_mul(u64::from(trade.qty));
            if trade.good == flour && self.vocation_of_id(trade.buyer) == Some(Vocation::Baker) {
                flour_gold_spent = flour_gold_spent.saturating_add(value);
            }
            if trade.good == bread && self.vocation_of_id(trade.seller) == Some(Vocation::Baker) {
                bread_gold_earned = bread_gold_earned.saturating_add(value);
                bread_units_sold = bread_units_sold.saturating_add(u64::from(trade.qty));
            }
        }
        let acc = &mut self.saving_allocation_obs.baker_round_trip;
        acc.flour_gold_spent = acc.flour_gold_spent.saturating_add(flour_gold_spent);
        acc.bread_gold_earned = acc.bread_gold_earned.saturating_add(bread_gold_earned);
        acc.bread_units_sold = acc.bread_units_sold.saturating_add(bread_units_sold);
        acc.bread_units_produced = acc.bread_units_produced.saturating_add(bread_produced);
    }

    /// Run `ticks` economic ticks.
    pub fn run(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.econ_tick();
        }
    }

    // ---- the G8b bank phase ---------------------------------------------

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
        let share_input = self.cultivation_input_good();

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
                // DH.a (P1-1): the world→econ deposit seam — a settled gathered-node deposit is
                // own-produced. Emit at the real credit, not a phase diff.
                self.closure_emit(closure::ClosureEventKind::GatherDeposit {
                    agent,
                    good,
                    qty: removed,
                });
                if Some(good) == share_input {
                    self.credit_share_contract_grain(agent, removed);
                    self.credit_in_kind_contract_grain(agent, removed);
                }
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

    fn refresh_private_land_carried_sources(&mut self) {
        if !self.private_land_tenure_active() {
            return;
        }
        let Some(grain) = self.chain.as_ref().map(|chain| chain.content.grain()) else {
            return;
        };
        for &slot in &self.live_colonist_slots {
            let id = self.colonists[slot].id;
            if self.colonists[slot].carried_grain_source.is_none() {
                self.colonists[slot].carried_share_contract_id = None;
                self.colonists[slot].carried_in_kind_contract_id = None;
                continue;
            }
            let carry_empty = self.world.agent_carry(id, grain) == 0;
            let pending_empty = self
                .pending_deposits
                .get(&(id, grain))
                .copied()
                .unwrap_or(0)
                == 0;
            if carry_empty && pending_empty {
                self.colonists[slot].carried_grain_source = None;
                self.colonists[slot].carried_share_contract_id = None;
                self.colonists[slot].carried_in_kind_contract_id = None;
            }
        }
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
    ///
    /// S12: a colonist marked `foraging` (a hungry, eligible forager — possibly a
    /// Gatherer, an idle Consumer, or a latent producer) is instead sent to deposit any
    /// carry it still holds and then [`Task::GoForage`] the FORAGE node — occupying its
    /// one world-task slot foraging *instead of* harvesting WOOD (the structural
    /// opportunity cost). Only spatial colonists reach the body (a non-spatial founder
    /// has no Idle world agent), and the foraging branch is taken only on the gated
    /// own-labor path (where `forage_node` resolves), so every other run is unchanged.
    /// S14: whether a colonist's exchange deposits are attributed to it in the fast
    /// loop. A `Gatherer` deposits harvested WOOD/grain (the pre-S14 set); a colonist
    /// marked `foraging` (a Consumer/Unassigned/lineage forager, on the commons path)
    /// deposits harvested FORAGE — both must be attributed so the unit transfers to
    /// econ. One predicate, used for both the opening-carry snapshot and the carry-delta
    /// attribution, so a Gatherer that also forages is counted once. Off the own-labor
    /// path no colonist is `foraging`, so this is exactly the pre-S14 Gatherer-only set.
    fn carry_is_forage_attributed(colonist: &Colonist) -> bool {
        // S15: a cultivating colonist deposits harvested GRAIN through the same haul
        // cycle, so it joins the attributed set too (else its grain would carry/deposit
        // but never transfer to econ). Off the cultivation path no colonist is
        // `cultivating`, so this reduces to the S14 forager/Gatherer set (byte-identical).
        colonist.foraging || colonist.cultivating || colonist.vocation == Vocation::Gatherer
    }

    fn assign_idle_gatherer_tasks(&mut self) {
        let forage_node = self.forage_node();
        // S15: a cultivating colonist HARVESTS the grain node (the abundant resource the
        // cultivation phase converts to bread), then deposits — the same haul cycle as
        // foraging, but on grain. Resolved only on the active cultivation path, so every
        // other run never touches it (byte-identical).
        let cultivation_active = self.own_use_cultivation_active();
        let grain_node = if cultivation_active {
            self.grain_node()
        } else {
            None
        };
        // S22b: when bounded cultivation skill is active, a cultivating agent's grain trip uses a
        // PER-TRIP haul capacity scaled by its skill (`GoHarvestWithRoom`, room = want = haul),
        // never the agent's permanent `carry_cap`. Resolved only on the active skill path, so
        // every pre-S22b run keeps the plain `GoHarvest(grain, carry_cap)` (byte-identical).
        let (skill_active, skill_cap, skill_ceiling) =
            self.chain.as_ref().map_or((false, 0u16, 0u32), |chain| {
                (
                    self.cultivation_skill_active(),
                    chain.skill_cap,
                    chain.skill_haul_ceiling,
                )
            });
        // S22d: when durable cultivation capital is active, a tool-OWNING cultivator draws up to
        // `cultivation_tool_haul_ceiling × carry_cap` per grain trip (the owner-EXCLUSIVE boost),
        // routed through the same `GoHarvestWithRoom` per-trip room override as S22b skill — a
        // faster draw on the conserved grain node, never a higher bread-per-grain ratio. Resolved
        // only on the active path, so every off-path run keeps its prior grain-haul behaviour.
        let (tool_active, tool_ceiling) = self.chain.as_ref().map_or((false, 0u32), |chain| {
            (
                self.durable_cultivation_tool_active(),
                chain.cultivation_tool_haul_ceiling,
            )
        });
        let tool_good = if tool_active {
            self.cultivation_tool_good()
        } else {
            None
        };
        // S14: in the capped-commons mode a forager HARVESTS the FORAGE node (depleting
        // it, so per-capita yield falls), then deposits — the real haul cycle. In the
        // S12 marker mode it forages (relocating nothing) and is credited a fixed yield.
        let commons = self.forage_commons_active();
        let land_active = self.private_land_tenure_active();
        let no_reserved_land = BTreeSet::new();
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            let id = colonist.id;
            if self.wage_worker_has_open_escrow(id) {
                continue;
            }
            if self.world.agent_status(id) != Some(AgentStatus::Idle) {
                continue;
            }
            if let Some(contract) = self.share_contract_for_worker(id) {
                let task = self.share_contract_task(id, contract.node);
                self.world.assign_task(id, task);
                continue;
            }
            if let Some(contract) = self.in_kind_contract_for_worker(id) {
                let task = self.in_kind_contract_task(id, contract.node);
                self.world.assign_task(id, task);
                continue;
            }
            // S15: cultivating takes the world-task slot (it is mutually exclusive with
            // `foraging`). Deposit any carry first (incl. FORAGE from a just-ended forage
            // spell), then GoHarvest the grain node so the cultivation phase has grain.
            if colonist.cultivating {
                // S23a: under private land tenure the harvest target may resolve to `None` (the
                // owned plot is depleted/reverted and no claimable plot has stock this tick). Deposit
                // any carry FIRST, before the target lookup, so a `None` target can never strand the
                // carry — which would also pin `carried_grain_source` and keep an idle plot alive.
                // Gated on `land_active`, so every commons config keeps its byte-identical behaviour.
                if land_active && self.world.agent_carry_total(id) > 0 {
                    self.world.assign_task(id, Task::GoDeposit(self.exchange));
                    continue;
                }
                let grain_node = if land_active {
                    self.private_land_target_for_agent(id, &no_reserved_land)
                } else {
                    grain_node
                };
                if let Some(grain_node) = grain_node {
                    let task = if self.world.agent_carry_total(id) > 0 {
                        Task::GoDeposit(self.exchange)
                    } else {
                        // The base per-trip haul: the S22b skilled haul when skill is active, else
                        // the plain `carry_cap` (the S22c no-tool return). At skill 0 (or ceiling
                        // ≤ 1) `cultivation_haul` returns exactly `carry_cap`.
                        let base_haul = if skill_active {
                            cultivation_haul(
                                self.carry_cap,
                                colonist.cultivation_skill,
                                skill_cap,
                                skill_ceiling,
                            )
                        } else {
                            self.carry_cap
                        };
                        // S22d: a tool-OWNING cultivator (holds the plow) draws up to
                        // `owner_ceiling × carry_cap` — strictly more than a non-owner's
                        // `carry_cap` — ONLY while cultivating (asset specificity). Bounded by
                        // `node.stock`; the 1:1 grain→bread recipe is unchanged (a faster
                        // conserved-node draw, never a mint).
                        let owns_tool = tool_good.is_some_and(|tool| {
                            self.society
                                .agents
                                .get(id)
                                .is_some_and(|agent| agent.stock.get(tool) > 0)
                        });
                        let haul = if owns_tool {
                            base_haul.max(self.carry_cap.saturating_mul(tool_ceiling))
                        } else {
                            base_haul
                        };
                        let haul = self.private_land_harvest_room_for(id, grain_node, haul);
                        // `GoHarvestWithRoom(grain, carry_cap, carry_cap)` is behaviour-identical
                        // to `GoHarvest(grain, carry_cap)` (both room-capped at carry_cap), only
                        // the task tag differs — so a non-skilled non-owner keeps the exact S22c
                        // `GoHarvest` task and stays byte-identical.
                        if haul == 0 {
                            Task::Idle
                        } else if skill_active || owns_tool || haul != self.carry_cap {
                            Task::GoHarvestWithRoom(grain_node, haul, haul)
                        } else {
                            Task::GoHarvest(grain_node, self.carry_cap)
                        }
                    };
                    self.world.assign_task(id, task);
                    continue;
                }
            }
            if colonist.foraging {
                if let Some(forage_node) = forage_node {
                    let task = if self.world.agent_carry_total(id) > 0 {
                        Task::GoDeposit(self.exchange)
                    } else if commons {
                        Task::GoHarvest(forage_node, self.carry_cap)
                    } else {
                        Task::GoForage(forage_node, self.carry_cap)
                    };
                    self.world.assign_task(id, task);
                    continue;
                }
            }
            if colonist.vocation != Vocation::Gatherer {
                continue;
            }
            let Some(node) = colonist.node else { continue };
            let task = if self.world.agent_carry_total(id) > 0 {
                Task::GoDeposit(self.exchange)
            } else {
                Task::GoHarvest(node, self.carry_cap)
            };
            self.world.assign_task(id, task);
        }
    }

    fn land_quality_prior_rent(quality: LandPlotQuality) -> u64 {
        let cap_weight = (u64::from(quality.cap) / 1_000).max(1);
        let distance = u64::from(quality.distance).saturating_add(1);
        u64::from(quality.regen)
            .saturating_mul(cap_weight)
            .checked_div(distance)
            .unwrap_or(0)
            .max(1)
    }

    // ---- the econ-tick phases ------------------------------------------

    fn colonist_household(&self, id: AgentId) -> Option<usize> {
        self.slot_for_id(id)
            .and_then(|slot| self.colonists[slot].household)
    }

    fn stock_of_id(&self, agent: AgentId, good: GoodId) -> u64 {
        self.society
            .agents
            .get(agent)
            .map_or(0, |held| u64::from(held.stock.get(good)))
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
        let birth_stock_saving = self.birth_stock_saving_active();
        let child_food_endowment = self
            .demography
            .as_ref()
            .map_or(0, |demo| demo.child_food_endowment);
        let birth_stock_max_household_size = self
            .demography
            .as_ref()
            .map_or(0, |demo| demo.max_household_size);
        let mut birth_stock_household_sizes = vec![0; self.households.len()];
        if birth_stock_saving {
            for &slot in &self.live_colonist_slots {
                if let Some(household) = self.colonists[slot].household {
                    birth_stock_household_sizes[household] += 1;
                }
            }
        }
        // S10: in the per-agent-capital path the savings ladder spans MULTIPLE future
        // horizons (depth set by each colonist's own time preference), so a built tool's
        // late-due receipts can provision a patient colonist's deep savings want. Gated,
        // so every off-path scale keeps the single Later(4) ladder and stays byte-identical.
        let deep_savings = self
            .chain
            .as_ref()
            .is_some_and(|chain| chain.per_agent_capital);
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            let mut scale = if deep_savings {
                regenerate_scale_for_capital(&colonist.need, &colonist.culture, &self.known)
            } else {
                regenerate_scale(&colonist.need, &colonist.culture, &self.known)
            };
            let selected_direct_use = self.salt_direct_use.and_then(|(good, qty, period)| {
                (self.society.current_money_good().is_none()
                    && period > 0
                    && colonist.id.index().is_multiple_of(u32::from(period)))
                .then_some((good, qty))
            });
            let cycle_direct_use_before_input = selected_direct_use.is_some()
                && matches!(
                    colonist.vocation,
                    Vocation::CycleA | Vocation::CycleB | Vocation::CycleC
                );
            if colonist.household.is_some_and(|household| {
                birth_stock_saving
                    && self.is_producer_household(household)
                    && birth_stock_household_sizes[household]
                        < self.birth_cap_for_household(household, birth_stock_max_household_size)
            }) {
                medium_scale_extension(&mut scale, self.known.hunger, child_food_endowment);
                self.birth_stock_wants_emitted = self
                    .birth_stock_wants_emitted
                    .saturating_add(u64::from(child_food_endowment));
            }
            if let Some(chain) = &self.chain {
                // A producer's tool/input wants follow its production specialty —
                // its adopted vocation (Miller/Baker, seeded or chosen) or, for a
                // latent G3b colonist, the recipe it could run. A latent producer
                // anchors only its tool (it never sells its capital but posts no
                // input bid), while an **active** producer — seeded G3a or adopted
                // G3b — also bids `throughput` units of its input each tick. The
                // latent/active split keeps a latent producer from autonomously
                // pricing the intermediate good (load-bearing for the control).
                let mut specialty =
                    production_specialty(colonist.vocation, colonist.latent, &chain.content);
                // S7.1: a non-producer that has ACQUIRED a chain tool (it holds a
                // mill/oven but is not seeded latent and has not yet adopted) gets the
                // tool anchor too — so the durable capital it just built/was handed is
                // never posted as surplus and sold on the market step before role-choice
                // adopts it (the phase-order trap). It anchors ONLY the tool (no input
                // bid); the input wants come once it adopts and `production_specialty`
                // keys off its Miller/Baker vocation. Gated on the eligibility flag, so
                // every pre-S7 scale is byte-identical.
                let mut anchor_only = false;
                if specialty.is_none() && chain.tool_acquisition_eligibility {
                    if let Some(agent) = self.society.agents.get(colonist.id) {
                        if agent.stock.get(chain.content.mill()) > 0 {
                            specialty = Some((chain.content.mill(), chain.content.grain()));
                            anchor_only = true;
                        } else if agent.stock.get(chain.content.oven()) > 0 {
                            specialty = Some((chain.content.oven(), chain.content.flour()));
                            anchor_only = true;
                        }
                    }
                }
                if let Some((tool, input)) = specialty {
                    let input_wants = if anchor_only {
                        // An acquired-but-not-adopted tool-holder posts NO input bid (it
                        // is not yet producing) — just the anchor that protects the tool.
                        0
                    } else {
                        match colonist.vocation {
                            // Active producers (G3a seeded, G3b adopted) bid `throughput`
                            // units of input each tick. S2: when the project-aware
                            // override drives input acquisition, suppress this generic
                            // (recipe-blind, low-ranked) input want so the producer posts
                            // no DUPLICATE bid — the override is the sole input bid.
                            Vocation::Miller
                            | Vocation::Baker
                            | Vocation::CycleA
                            | Vocation::CycleB
                            | Vocation::CycleC
                                if chain.project_input_bids
                                    && self.society.current_money_good().is_some() =>
                            {
                                0
                            }
                            Vocation::Miller
                            | Vocation::Baker
                            | Vocation::CycleA
                            | Vocation::CycleB
                            | Vocation::CycleC => chain.throughput.max(1),
                            // G6b: a scholar/confectioner reserves (and tops up) its FULL
                            // input buffer, so research / tier-2 production runs from seeded
                            // stock and the buffer is neither dumped nor eaten.
                            Vocation::Scholar => chain.scholar_grain_buffer.max(1),
                            Vocation::Confectioner => chain.confectioner_flour_buffer.max(1),
                            // A latent producer (Unassigned) posts NO input bid —
                            // load-bearing for the G3b control (it must not price the
                            // intermediate good).
                            _ => 0,
                        }
                    };
                    if let (true, Some((direct_good, direct_qty))) =
                        (cycle_direct_use_before_input, selected_direct_use)
                    {
                        cycle_producer_scale_extension(
                            &mut scale,
                            tool,
                            input,
                            input_wants,
                            direct_good,
                            direct_qty,
                        );
                    } else {
                        producer_scale_extension(&mut scale, tool, input, input_wants);
                    }
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
            // S9: the medium's HETEROGENEOUS real direct use. A SELECTED subset of
            // colonists (stable id index `0 mod period`) carries `qty` fixed
            // `Good(SALT)/Now` CONSUMPTION wants — the real non-monetary demand that
            // lets SALT accrue saleability from direct trades before it is money (the
            // Mengerian regression-theorem seed, replacing the circular medium want).
            // The non-selected colonists never carry it, so they stay eligible to
            // accept SALT INDIRECTLY (the breadth the strong-bar gate needs). Active
            // only pre-promotion (SALT delists to money on promotion); consumed into
            // the `consumed` bucket by the existing `Horizon::Now` consume arm. Pure
            // and deterministic; draws no randomness.
            if let Some((good, qty)) = selected_direct_use {
                if !cycle_direct_use_before_input {
                    direct_use_scale_extension(&mut scale, good, qty);
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

    /// S15 own-use readback seam: debit `qty` of `good` from `agent`'s OWN stock and
    /// record the consumption in the readback log, so the `sim` need readback advances
    /// the agent's hunger from it next tick (a debit alone would conserve but never feed
    /// — Base Fact 8). `report.consumed` is booked by the end-of-tick log aggregation
    /// (the same path the market consume uses), so this only debits + logs. Called after
    /// the market step (which clears the log at its start), so the entry survives into
    /// the next tick's needs phase. A no-op for `qty == 0` or a debit that cannot clear.
    fn consume_own_use_stock(&mut self, id: AgentId, good: GoodId, qty: u32) {
        if qty == 0 {
            return;
        }
        if self.society.debit_stock(id, good, qty) {
            self.society.record_own_use_consumption(id, good, qty);
        }
    }

    /// Credit `qty` units of a labor-PRODUCED good into an agent's own econ stock,
    /// booking it to `report.produced` (the produced side of the conservation
    /// identity) — the [`Self::deliver_demography_provision_unit`] analogue for own
    /// labor rather than a hearth mint. A no-op for `qty == 0`, a freed id, or a stock
    /// already at the `u32` ceiling (the unit is simply not created — never destroyed).
    fn credit_produced(
        &mut self,
        id: AgentId,
        good: GoodId,
        qty: u32,
        report: &mut EconTickReport,
    ) {
        if qty == 0 {
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
        let credited = qty.min(u32::MAX - held);
        if credited > 0 && self.society.credit_stock(id, good, credited) {
            *report.produced.entry(good).or_insert(0) += u64::from(credited);
        }
    }

    /// PROJECT-AWARE INPUT BID phase (S2 — the endogenous fix, see
    /// [`ChainConfig::project_input_bids`]). Before the market clears, set a gated
    /// econ spot-bid override (S1) for each active producer's recipe input, so the
    /// producer bids for its input on the REAL order book during `society.step()`
    /// — with its own money, at the reservation IMPUTED from the project-bundle
    /// appraisal. The bid reserves the producer's gold and fills against a willing
    /// seller (a gatherer's grain, a miller's flour) at the seller's own ask,
    /// recording a real `Trade` — the input acquired by market trade, not placed
    /// by a planner (`run_input_advance`).
    ///
    /// The reservation reuses [`recipe_adoption_pays_for_money`] /
    /// [`appraise_project_bundle_for_money`] — the SAME machinery role-choice
    /// adopts on — not the scalar [`recipe_is_profitable`]: it is the highest input
    /// price at which running the recipe-as-project still provisions the producer's
    /// savings want on its current endowment (see [`imputed_input_reservation`]).
    ///
    /// Output-price source for the imputation: the good's LAST REALIZED trade price
    /// (`Society::realized_price`), the same observed signal role-choice adopts on
    /// — not a hypothetical live bid. No realized output price yet → no basis to
    /// impute → no override (the cold-start buffers seed the first realized prices,
    /// S4). The generic low-ranked input want is suppressed for these producers in
    /// `regenerate_scales`, so the override is the SOLE input bid (no duplicate).
    ///
    /// Deterministic: id-ordered, integer, nothing drawn. A no-op (sets no
    /// override) unless enabled and money has emerged, so every other run is
    /// byte-identical.
    fn set_project_input_bid_overrides(&mut self) {
        let (
            mill_recipe,
            bake_recipe,
            cycle_a_recipe,
            cycle_b_recipe,
            cycle_c_recipe,
            grain,
            flour,
            bread,
            cycle_x,
            cycle_y,
            cycle_z,
            operating_cost,
            recurring,
            subsistence,
            entrepreneurial,
        ) = match self.chain.as_ref() {
            Some(chain) if chain.project_input_bids => (
                chain.content.mill_recipe().clone(),
                chain.content.bake_recipe().clone(),
                chain.content.cycle_a_recipe().cloned(),
                chain.content.cycle_b_recipe().cloned(),
                chain.content.cycle_c_recipe().cloned(),
                chain.content.grain(),
                chain.content.flour(),
                chain.content.bread(),
                chain.content.cycle_x(),
                chain.content.cycle_y(),
                chain.content.cycle_z(),
                chain.operating_cost,
                chain.recurring_motive,
                chain.producer_subsistence,
                // S11: the producer imputes its input reservation against its own fallible
                // OUTPUT forecast. The posted bid limit is still anchored to the observed
                // input price below, so a resting optimistic bid cannot become a new
                // forecast-inflated input price.
                chain.entrepreneurial_forecasts,
            ),
            _ => return,
        };
        let Some(money) = self.society.current_money_good() else {
            return;
        };
        let staple = self.known.hunger;
        let tick = self.society.tick.0;
        // S21d.2a: the bootstrap microtrace records prepared input-bid attempts here, keyed to
        // the econ tick so it lines up with the buy/eat legs the acquisition market pass records.
        // The attempts are classified after `society.step()`, when the real spot-order
        // tender/reservation gate has either posted/filled the bid or blocked it.
        let trace_active = self.acquisition_ledger_active();
        let live_len = self.live_colonist_slots.len();
        for live_index in 0..live_len {
            let slot = self.live_colonist_slots[live_index];
            let (producer_id, vocation) = {
                let colonist = &self.colonists[slot];
                (colonist.id, colonist.vocation)
            };
            let (recipe, input, output) = match vocation {
                Vocation::Miller => (&mill_recipe, grain, flour),
                Vocation::Baker => (&bake_recipe, flour, bread),
                Vocation::CycleA => {
                    let (Some(recipe), Some(input), Some(output)) =
                        (cycle_a_recipe.as_ref(), cycle_z, cycle_x)
                    else {
                        continue;
                    };
                    (recipe, input, output)
                }
                Vocation::CycleB => {
                    let (Some(recipe), Some(input), Some(output)) =
                        (cycle_b_recipe.as_ref(), cycle_x, cycle_y)
                    else {
                        continue;
                    };
                    (recipe, input, output)
                }
                Vocation::CycleC => {
                    let (Some(recipe), Some(input), Some(output)) =
                        (cycle_c_recipe.as_ref(), cycle_y, cycle_z)
                    else {
                        continue;
                    };
                    (recipe, input, output)
                }
                _ => continue,
            };
            // The output's last realized price is the imputation basis (S4 seeds it). S11:
            // under entrepreneurial forecasts the producer imputes against its own grounded
            // fallible forecast of that output price instead (input cost stays observed).
            let realized_output = self.society.realized_price(output);
            let forecast_bias = self.colonists[slot].culture.forecast_bias_bps;
            let Some(agent) = self.society.agents.get(producer_id) else {
                continue;
            };
            let out_price = if entrepreneurial {
                forecast_output_price(agent, output, realized_output, forecast_bias)
            } else {
                realized_output
            };
            let Some(out_price) = out_price else {
                continue;
            };
            // Demand-responsive restock (S3 working-capital discipline): do not buy
            // more input while last cycle's output is still UNSOLD. A producer
            // clears its output before restocking, so its input demand tracks its
            // actual sales — it never drains working capital over-producing into a
            // saturated market (the recurring-motive failure mode). The subsistence
            // floor is excluded when the output IS the staple the producer's own
            // hearth feeds it (a baker's bread), so its food is not mistaken for
            // unsold inventory. One full batch (`output_qty`) of slack absorbs the
            // one-tick lag between buying input and selling the output it becomes.
            let output_floor = if output == staple { subsistence } else { 0 };
            if agent.stock.get(output) >= recipe.output_qty.saturating_add(output_floor) {
                continue;
            }
            let Some(reservation) = imputed_input_reservation(
                agent,
                recipe,
                out_price,
                tick,
                operating_cost,
                recurring,
                money,
            ) else {
                continue;
            };
            // The reservation carries the producer's max willingness to buy input. With
            // forecasts on, the posted limit is capped at the observed input price when
            // one exists: the forecast can make the producer willing to buy, but cannot
            // by itself raise the realized input quote if its bid rests first.
            let limit = project_input_bid_limit(
                reservation,
                self.society.realized_price(input),
                entrepreneurial,
            );
            if trace_active {
                self.bootstrap_trace
                    .prepare_bid_attempt(producer_id, input, agent.gold.0);
            }
            self.society
                .set_bid_override(producer_id, input, reservation, limit);
        }
    }

    fn share_worker_instantaneous_outside_option_fails(
        &self,
        worker: AgentId,
        bread: GoodId,
        mode: ShareTenancyMode,
    ) -> bool {
        if mode == ShareTenancyMode::LineageWorker {
            let threshold = self
                .chain
                .as_ref()
                .map_or(0, |chain| chain.emergency_hunger_threshold);
            if threshold == 0 {
                return false;
            }
            return self
                .slot_for_id(worker)
                .is_some_and(|slot| self.colonists[slot].need.hunger >= threshold);
        }
        !self.forecast_commons_sufficiency(worker, bread)
    }

    fn renewal_fate_outside_pool(
        &self,
        worker: AgentId,
        bread: GoodId,
        mode: ShareTenancyMode,
    ) -> RenewalFate {
        if self.share_worker_base_eligible(worker, mode) {
            debug_assert!(
                !self.share_worker_outside_option_fails(worker, bread, mode),
                "base-eligible renewal worker outside the pool should have passed its outside option"
            );
            RenewalFate::FedOut
        } else {
            RenewalFate::BaseIneligible
        }
    }

    fn classify_renewal_fates_without_workers(
        &self,
        renewal_fates: &mut BTreeMap<RenewalHintKey, Option<RenewalFate>>,
        bread: GoodId,
    ) {
        let mode = self.share_tenancy_mode();
        for (&(worker, _, _), fate) in renewal_fates.iter_mut() {
            *fate = Some(self.renewal_fate_outside_pool(worker, bread, mode));
        }
    }

    fn classify_renewal_fates_without_owners(
        &self,
        renewal_fates: &mut BTreeMap<RenewalHintKey, Option<RenewalFate>>,
        workers: &[AgentId],
        bread: GoodId,
    ) {
        let mode = self.share_tenancy_mode();
        for (&(worker, _, _), fate) in renewal_fates.iter_mut() {
            *fate = Some(if workers.binary_search(&worker).is_ok() {
                RenewalFate::OwnerNotCandidate
            } else {
                self.renewal_fate_outside_pool(worker, bread, mode)
            });
        }
    }

    fn set_renewal_fate(
        renewal_fates: &mut BTreeMap<RenewalHintKey, Option<RenewalFate>>,
        key: RenewalHintKey,
        fate: RenewalFate,
    ) {
        if let Some(slot) = renewal_fates.get_mut(&key) {
            *slot = Some(fate);
        }
    }

    fn finalize_renewal_fates(
        &mut self,
        renewal_fates: BTreeMap<RenewalHintKey, Option<RenewalFate>>,
        hint_count: u64,
        same_plot_renewed: u64,
    ) {
        if hint_count == 0 {
            return;
        }
        let mut fate_total = 0u64;
        self.share_renewal_hints_total = self.share_renewal_hints_total.saturating_add(hint_count);
        for fate in renewal_fates.into_values() {
            debug_assert!(fate.is_some(), "renewal fate left pending at finalization");
            let Some(fate) = fate else {
                self.share_renewal_base_ineligible =
                    self.share_renewal_base_ineligible.saturating_add(1);
                fate_total = fate_total.saturating_add(1);
                continue;
            };
            fate_total = fate_total.saturating_add(1);
            match fate {
                RenewalFate::FedOut => {
                    self.share_renewal_fed_out = self.share_renewal_fed_out.saturating_add(1)
                }
                RenewalFate::BaseIneligible => {
                    self.share_renewal_base_ineligible =
                        self.share_renewal_base_ineligible.saturating_add(1)
                }
                RenewalFate::OwnerNotCandidate => {
                    self.share_renewal_owner_not_candidate =
                        self.share_renewal_owner_not_candidate.saturating_add(1)
                }
                RenewalFate::BreadDeclined => {
                    self.share_renewal_bread_declined =
                        self.share_renewal_bread_declined.saturating_add(1)
                }
                RenewalFate::MatchedElsewhere => {
                    self.share_renewal_matched_elsewhere =
                        self.share_renewal_matched_elsewhere.saturating_add(1)
                }
            }
        }
        debug_assert_eq!(
            fate_total.saturating_add(same_plot_renewed),
            hint_count,
            "renewal fates must sum to hints minus same-plot renewals"
        );
        let cumulative_fates = self
            .share_renewal_fed_out
            .saturating_add(self.share_renewal_base_ineligible)
            .saturating_add(self.share_renewal_owner_not_candidate)
            .saturating_add(self.share_renewal_bread_declined)
            .saturating_add(self.share_renewal_matched_elsewhere);
        debug_assert_eq!(
            cumulative_fates,
            self.share_renewal_hints_total
                .saturating_sub(self.share_renewals_total),
            "cumulative renewal fate counts must stay internally consistent"
        );
    }

    fn share_forward_leisure_guard(&self, worker: AgentId, bread: GoodId) -> bool {
        let Some(slot) = self.slot_for_id(worker) else {
            return false;
        };
        let Some(agent) = self.society.agents.get(worker) else {
            return false;
        };
        let threshold = self
            .chain
            .as_ref()
            .map_or(0, |chain| chain.emergency_hunger_threshold);
        if threshold == 0 {
            return false;
        }
        let before_endowment = TemporalEndowment {
            stock: &agent.stock,
            gold: agent.gold,
            receivables: &[],
            payables: &[],
            tick: Tick(self.econ_tick),
        };
        let before = provisioning_bitmap_for_money(&agent.scale, &before_endowment, GOLD);
        let leisure_rank = agent.scale.iter().enumerate().find_map(|(index, want)| {
            (want.kind == WantKind::Leisure
                && matches!(want.horizon, Horizon::Now)
                && !before.get(index).copied().unwrap_or(false))
            .then_some(index)
        });
        let Some(leisure_rank) = leisure_rank else {
            return true;
        };
        let mut need = self.colonists[slot].need;
        need.hunger = threshold.min(self.dynamics.need_max);
        let deep_savings = self
            .chain
            .as_ref()
            .is_some_and(|chain| chain.per_agent_capital);
        let scale = if deep_savings {
            regenerate_scale_for_capital(&need, &self.colonists[slot].culture, &self.known)
        } else {
            regenerate_scale(&need, &self.colonists[slot].culture, &self.known)
        };
        scale
            .iter()
            .position(|want| want.kind == WantKind::Good(bread) && want.horizon == Horizon::Now)
            .is_some_and(|rank| rank < leisure_rank)
    }

    fn worker_labor_ask_for_salt(&self, worker: AgentId, labor_qty: u32) -> Option<Gold> {
        let mut agent = self.society.agents.get(worker)?.clone();
        agent.labor_capacity = self.wage_worker_available_labor(worker, labor_qty)?;
        agent.reservation_labor_ask_for_money(labor_qty, SALT)
    }

    fn colonist_marked_dead(&self, id: AgentId) -> bool {
        self.slot_for_id(id)
            .is_some_and(|slot| !self.colonists[slot].alive)
    }

    /// S21d.2a: classify the project-input bid attempts prepared before the market after the
    /// market has run. A bid counts as posted if it either remains live in the spot book or filled
    /// as an input purchase on this tick's trade suffix; otherwise it was blocked by the real
    /// tender/reservation gate (`cashless` vs `reserved` split by the pre-step gross-money read).
    fn finalize_bootstrap_bid_attempts(&mut self, spot_trades_start: usize) {
        if !self.acquisition_ledger_active() {
            self.bootstrap_trace.pending_bid_attempts.clear();
            return;
        }
        let attempts = std::mem::take(&mut self.bootstrap_trace.pending_bid_attempts);
        let tick = self.econ_tick;
        let food = self.acquisition_food_good();
        for attempt in attempts {
            let filled = self.society.trades[spot_trades_start..]
                .iter()
                .any(|trade| {
                    trade.buyer == attempt.producer && trade.good == attempt.input && trade.qty > 0
                });
            // The project-input override is the producer's only input-bid source in this path; a
            // live bid for `(producer, input)` therefore means this prepared attempt posted.
            let posted = filled
                || self
                    .society
                    .has_live_spot_bid(attempt.producer, attempt.input);
            let bought_then_ate = food.is_some_and(|food| {
                let ate_food_this_tick = self.society.consumption_log_last_tick().iter().any(
                    |&(agent, eaten_good, qty)| {
                        agent == attempt.producer && eaten_good == food && qty > 0
                    },
                );
                self.bootstrap_trace.bought_then_ate_on_tick(
                    attempt.producer,
                    tick,
                    ate_food_this_tick,
                )
            });
            self.bootstrap_trace.observe_bid_decision(
                tick,
                posted,
                attempt.gross_money,
                bought_then_ate,
            );
        }
    }

    /// Conserved money move usable in BOTH the designated-GOLD / M3 regimes (via
    /// [`Society::transfer_gold`]) and the EMERGENT regime (where `transfer_gold`
    /// refuses because the money lives in `Agent.gold` under a non-`Designated`
    /// regime, so move the field directly). Returns whether it moved. The caller
    /// must cap `amount` at the sender's free (unreserved) gold; the direct path
    /// re-checks that and never over-commits a reservation. Experiment-only; the
    /// direct path assumes the emergent no-`money_system` regime.
    fn move_money_conserved(&mut self, from: AgentId, to: AgentId, amount: Gold) -> bool {
        if amount == Gold::ZERO {
            return false;
        }
        if self.society.transfer_gold(from, to, amount) {
            return true;
        }
        if self.society.free_gold_after_all_reserves(from) < amount {
            return false;
        }
        let Some(sender) = self.society.agents.get_mut(from) else {
            return false;
        };
        sender.gold = sender.gold.saturating_sub(amount);
        if let Some(recipient) = self.society.agents.get_mut(to) {
            recipient.gold = recipient.gold.saturating_add(amount);
        }
        true
    }

    fn reentry_revert_ready(&self, id: AgentId) -> bool {
        self.world.agent_status(id) == Some(AgentStatus::Idle)
            && self.world.agent_carry_total(id) == 0
    }

    fn role_choice_switch_ready(&self, id: AgentId, current: Vocation, next: Vocation) -> bool {
        if current != Vocation::Gatherer || !matches!(next, Vocation::Miller | Vocation::Baker) {
            return true;
        }
        self.world.agent_status(id) == Some(AgentStatus::Idle)
            && self.world.agent_carry_total(id) == 0
    }

    /// S10: START new builds by PER-AGENT ordinal appraisal (the `per_agent_capital`
    /// path). Iterates live colonists in slot order (iteration order ONLY, never
    /// selection); each eligible colonist (the S7 fed/idle/holds-WOOD/holds-no-tool
    /// filter) runs [`appraise_capital_tool_bundle_for_money`] on its OWN scale for each
    /// tool it could build, and any it accepts starts its own build from its OWN WOOD +
    /// labor via the reused per-builder substrate ([`start_project`]) — no global stage
    /// choice, no first-eligible assignment, no single-in-flight gate. Records one
    /// per-candidate decision in `last_capital_decisions` (the diagnostic a test reads to
    /// prove an earlier-eligible colonist declined on its own scale while a later one
    /// accepted). Returns `true` if any build completed this tick. Deterministic:
    /// slot-ordered, integer state.
    fn start_per_agent_builds(
        &mut self,
        report: &mut EconTickReport,
        labor_used: &mut Vec<(AgentId, u32)>,
        params: &PerAgentBuildParams<'_>,
    ) -> bool {
        self.last_capital_decisions.clear();
        let Some(money_good) = self.current_money_good() else {
            return false;
        };
        // S11: when entrepreneurial forecasts are on, each colonist appraises the OUTPUT
        // price as its own grounded fallible forecast (re-derived per colonist below), so
        // the build is a bet on its own price expectation. The demand-gating signals stay
        // observed (a build still requires real current demand), and the INPUT price stays
        // observed — only the output-revenue estimate is forecast.
        let entrepreneurial = self.entrepreneurial_can_run();
        let mortal_only = self.mortal_chain_producers_active();
        // The two tool candidates with their RECENT realized recipe prices, ordered by net
        // margin DESC so each colonist prefers the more rewarding roundabout investment
        // (Menger's imputation — a per-agent choice, not a global stage choice); ties by tool
        // id. Output prices are demand signals, so stale last-ever prices are unavailable to
        // the appraisal just as in the legacy builder.
        let raw_flour_price = self.society.realized_price(params.flour);
        let grain_price = self.society.realized_price(params.grain);
        let raw_bread_price = self.society.realized_price(params.bread);
        let held_mills = self.live_colonist_holder_count(params.mill_good);
        let held_ovens = self.live_colonist_holder_count(params.oven_good);
        let bread_signal = self.good_traded_within(params.bread, CAPITAL_BUILD_RECENCY)
            || (held_ovens == 0 && raw_bread_price.is_some());
        let flour_signal = self.good_traded_within(params.flour, CAPITAL_BUILD_RECENCY)
            || (held_mills == 0 && raw_flour_price.is_some());
        let bread_price = bread_signal.then_some(raw_bread_price).flatten();
        let flour_price = (bread_signal && flour_signal)
            .then_some(raw_flour_price)
            .flatten();
        let mut tool_candidates = [
            ToolCandidate {
                tool: params.oven_good,
                recipe: params.bake_recipe,
                template_id: ProjectTemplateId::BuildOven,
                output_price: bread_price,
                input_price: flour_price,
            },
            ToolCandidate {
                tool: params.mill_good,
                recipe: params.mill_recipe,
                template_id: ProjectTemplateId::BuildMill,
                output_price: flour_price,
                input_price: grain_price,
            },
        ];
        tool_candidates.sort_by(|a, b| {
            recipe_net_margin(b, params.operating_cost)
                .cmp(&recipe_net_margin(a, params.operating_cost))
                .then(a.tool.0.cmp(&b.tool.0))
        });

        let mut built = false;
        for idx in 0..self.live_colonist_slots.len() {
            let slot = self.live_colonist_slots[idx];
            let colonist = &self.colonists[slot];
            // The S7 eligibility filter (unchanged): a fed, non-latent colonist in a
            // survival/idle role with no in-flight build of its own.
            if (mortal_only && colonist.lifespan.is_none())
                || colonist.latent.is_some()
                || !matches!(
                    colonist.vocation,
                    Vocation::Gatherer | Vocation::Consumer | Vocation::Unassigned
                )
                || colonist.need.hunger > params.hunger_max
            {
                continue;
            }
            let id = colonist.id;
            if self.capital_builds.iter().any(|build| build.builder == id) {
                continue;
            }
            // S11: the colonist's heritable forecast bias (×1.0 = neutral). A Copy value, so
            // no borrow is held into the appraisal below.
            let forecast_bias = self.colonists[slot].culture.forecast_bias_bps;
            // Holds no chain tool and enough saved WOOD to fund the build itself, then
            // appraises each tool on its own scale — all under one immutable agent borrow.
            // The candidate array used for the appraisal is the SAME one indexed for the
            // build template, so a per-agent forecast re-sort cannot desync them.
            let appraised: Option<([ToolCandidate<'_>; 2], CapitalDecision, Option<usize>)> = {
                let Some(agent) = self.society.agents.get(id) else {
                    continue;
                };
                if agent.stock.get(params.mill_good) != 0
                    || agent.stock.get(params.oven_good) != 0
                    || u64::from(agent.stock.get(WOOD)) < u64::from(params.wood_qty)
                {
                    None
                } else {
                    // The candidates this colonist appraises: under entrepreneurial forecasts
                    // the OUTPUT price is the colonist's grounded fallible forecast of the
                    // (still demand-gated) base price, re-sorted by its OWN per-agent margin
                    // so an optimist prefers what IT thinks pays; the INPUT price stays the
                    // observed gated price. Off the flag this is exactly the global
                    // `tool_candidates` (byte-identical).
                    let candidates = if entrepreneurial {
                        let forecast = |good: GoodId, gated: Option<Gold>| -> Option<Gold> {
                            gated.and_then(|_| {
                                forecast_output_price(agent, good, gated, forecast_bias)
                            })
                        };
                        let mut c = [
                            ToolCandidate {
                                tool: params.oven_good,
                                recipe: params.bake_recipe,
                                template_id: ProjectTemplateId::BuildOven,
                                output_price: forecast(params.bread, bread_price),
                                input_price: flour_price,
                            },
                            ToolCandidate {
                                tool: params.mill_good,
                                recipe: params.mill_recipe,
                                template_id: ProjectTemplateId::BuildMill,
                                output_price: forecast(params.flour, flour_price),
                                input_price: grain_price,
                            },
                        ];
                        c.sort_by(|a, b| {
                            recipe_net_margin(b, params.operating_cost)
                                .cmp(&recipe_net_margin(a, params.operating_cost))
                                .then(a.tool.0.cmp(&b.tool.0))
                        });
                        c
                    } else {
                        tool_candidates
                    };
                    let (decision, chosen) = appraise_capital_for_colonist(
                        agent,
                        slot,
                        &candidates,
                        params.wood_qty,
                        params.build_labor,
                        params.tick,
                        params.operating_cost,
                        money_good,
                    );
                    Some((candidates, decision, chosen))
                }
            };
            let Some((candidates, decision, chosen)) = appraised else {
                continue;
            };
            self.last_capital_decisions.push(decision);
            let Some(chosen_index) = chosen else {
                if self.mortal_producer_inheritance_active() {
                    self.producer_build_rejections =
                        self.producer_build_rejections.saturating_add(1);
                }
                continue;
            };
            let candidate = candidates[chosen_index];

            // Commit the build via the reused per-builder substrate: the builder's OWN
            // WOOD up front (booked consumed_as_input) + one labor advance, exactly the S7
            // path — only the DECISION above is per-colonist, not the mechanics.
            let template = match candidate.template_id {
                ProjectTemplateId::BuildOven => {
                    build_oven_template(candidate.tool, params.wood_qty, params.build_labor)
                }
                _ => build_mill_template(candidate.tool, params.wood_qty, params.build_labor),
            };
            let pid = ProjectId(self.next_capital_project_id);
            let started = match self.society.agents.get_mut(id) {
                Some(agent) => start_project(&template, &mut agent.stock, pid, Tick(params.tick)),
                None => None,
            };
            let Some(mut project) = started else {
                continue;
            };
            self.closure_emit(closure::ClosureEventKind::CapitalFormation {
                agent: id,
                input: WOOD,
                input_qty: params.wood_qty,
                tool: project.output_good,
                tool_qty: 0,
            });
            *report.consumed_as_input.entry(WOOD).or_insert(0) += u64::from(params.wood_qty);
            if project.labor_advanced < template.required_labor && advance_project(&mut project) {
                labor_used.push((id, 1));
            }
            self.next_capital_project_id = self.next_capital_project_id.wrapping_add(1);
            let completed = match self.society.agents.get_mut(id) {
                Some(agent) => complete_project_if_ready(&mut project, &template, &mut agent.stock),
                None => false,
            };
            if completed {
                let qty = project.output_qty;
                self.closure_emit(closure::ClosureEventKind::CapitalFormation {
                    agent: id,
                    input: WOOD,
                    input_qty: 0,
                    tool: project.output_good,
                    tool_qty: qty,
                });
                *report.produced.entry(project.output_good).or_insert(0) += u64::from(qty);
                self.tools_built = self.tools_built.saturating_add(u64::from(qty));
                self.record_mortal_capital_build_completion(slot, qty);
                self.colonists[slot].acquired_tool = true;
                built = true;
            } else {
                self.capital_builds.push(CapitalBuild {
                    builder: id,
                    slot,
                    template,
                    project,
                });
            }
            // NO break: each colonist its own appraisal accepts starts its own build (no
            // single-in-flight gate). The slot order is iteration order, not selection.
        }

        built
    }

    fn productive_reentry_can_run(&self) -> bool {
        self.chain.as_ref().is_some_and(|chain| {
            let grain = chain.content.grain();
            // Mirror every runtime guard in `run_productive_reentry` exactly: the gate
            // is on, raw grain is the edible subsistence fallback, AND a grain-yielding
            // node exists. Without the node the phase returns before mutating, so a
            // config meeting the first two but lacking a grain node is inert and must
            // not serialize its thresholds/home (else two inert configs digest apart).
            chain.productive_reentry
                && self.known.subsistence == Some(grain)
                && self.node_for_good(grain).is_some()
        })
    }

    /// S7.1: whether the relaxed tool-acquisition eligibility gate is active for this
    /// settlement — the role-choice digest block (operating cost + per-colonist latent)
    /// must widen to fire on this too, since role-choice now acts on a tool-holder even
    /// with no seeded latent pool. A pure function of the chain flag, so a config with
    /// it off serializes exactly the pre-S7 stream.
    fn tool_acquisition_can_run(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(|chain| chain.tool_acquisition_eligibility)
    }

    fn mortal_chain_producers_active(&self) -> bool {
        self.demography.is_some()
            && self
                .chain
                .as_ref()
                .is_some_and(chain_runtime_mortal_chain_producers_active)
    }

    fn mortal_producer_inheritance_active(&self) -> bool {
        self.demography.is_some()
            && self
                .chain
                .as_ref()
                .is_some_and(chain_runtime_mortal_producer_inheritance_active)
    }

    fn mortal_producer_tool_inheritance_active(&self) -> bool {
        self.mortal_producer_inheritance_active()
            && self
                .chain
                .as_ref()
                .is_some_and(|chain| chain.mortal_producer_tool_inheritance)
    }

    fn earned_provisioning_active(&self) -> bool {
        self.demography.is_some()
            && self
                .chain
                .as_ref()
                .is_some_and(chain_runtime_earned_provisioning_active)
    }

    /// C3R.e (impl-67): true when ANY of the three ignition/withdrawal knobs is set — the sole
    /// gate on the tag-33 digest record (ON-only, so every existing config stays byte-identical).
    fn ignition_withdrawal_active(&self) -> bool {
        self.chain.as_ref().is_some_and(|chain| {
            chain.birth_stock_ignition_at.is_some()
                || chain.producer_house_starting_staple > 0
                || chain.producer_support_until_tick.is_some()
        })
    }

    /// C3R.e (impl-67): the B support gate — `true` while the producer-house food hearth and the
    /// `producer_subsistence` cushion's STAPLE leg are still delivered. `None`
    /// (`producer_support_until_tick`, every non-B config) → always on, so the gate is inert and
    /// the run is byte-identical; `Some(until)` → on while `econ_tick < until`, withdrawn at/after.
    fn producer_support_active(&self) -> bool {
        self.chain.as_ref().is_none_or(|chain| {
            chain
                .producer_support_until_tick
                .is_none_or(|until| self.econ_tick < until)
        })
    }

    /// C3R.e (impl-67): true for a B cell — the support-withdrawal gate is configured
    /// (`producer_support_until_tick.is_some()`). Every producer-house support mint (the food
    /// hearth AND the cushion's STAPLE leg) is then ORIGIN-FLAGGED, so the withdrawn support
    /// inventory is exhaustion-tracked (criterion ii): a window right after withdrawal cannot
    /// pass on residual pre-withdrawal support bread. `None` (every non-B config) → false, so no
    /// support mint is flagged and the ledger is behaviour-identical.
    fn producer_support_configured(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(|chain| chain.producer_support_until_tick.is_some())
    }

    /// C3R.e (impl-67): the cushion's WOOD leg is disabled for the ENTIRE run of any B cell
    /// (marked by `producer_support_until_tick` being set — the bread-only ledger cannot
    /// origin-track subsidized WOOD). `None` (every non-B config) leaves the WOOD leg intact.
    fn producer_cushion_wood_disabled(&self) -> bool {
        self.producer_support_configured()
    }

    fn birth_stock_saving_active(&self) -> bool {
        self.demography.is_some()
            && self
                .chain
                .as_ref()
                .is_some_and(chain_runtime_birth_stock_saving_active)
    }

    fn birth_stock_control_active(&self) -> bool {
        self.demography.is_some()
            && self
                .chain
                .as_ref()
                .is_some_and(chain_runtime_birth_stock_control_active)
    }

    fn saving_allocation_obs_active(&self) -> bool {
        self.demography.is_some()
            && self
                .chain
                .as_ref()
                .is_some_and(chain_runtime_saving_allocation_obs_active)
    }

    /// DH.b-obs (impl-70): the CONFIGURED predicate — demography present AND the flag set. Drives
    /// the tag-35 serialization AND construction-time tape allocation, and is UNAFFECTED by the
    /// closure force-disable, so the two force-disable twins carry the identical tag (§4b).
    fn birth_gate_obs_configured(&self) -> bool {
        self.demography.is_some()
            && self
                .chain
                .as_ref()
                .is_some_and(|chain| chain.birth_gate_obs)
    }

    /// DH.b-obs (impl-70): the ACTIVE predicate — configured AND the closure observation is live.
    /// Drives event WRITES only; under the closure force-disable this goes false (the tape records
    /// nothing) while the tag byte is unchanged (§2).
    fn birth_gate_obs_active(&self) -> bool {
        self.birth_gate_obs_configured() && self.closure_active()
    }

    fn producer_stock_provisioning_control_active(&self) -> bool {
        self.demography.is_some()
            && self
                .chain
                .as_ref()
                .is_some_and(chain_runtime_producer_stock_provisioning_control_active)
    }

    fn earned_provisioning_ledger_active(&self) -> bool {
        self.mortal_producer_inheritance_active()
            && self.current_money_good() == Some(GOLD)
            && self.provenance_bread_good().is_some()
    }

    fn producer_household_start(&self) -> Option<usize> {
        self.mortal_producer_inheritance_active()
            .then(|| {
                self.households
                    .len()
                    .checked_sub(MORTAL_PRODUCER_HOUSEHOLDS)
            })
            .flatten()
    }

    fn is_producer_household(&self, household: usize) -> bool {
        self.producer_household_start().is_some_and(|start| {
            household >= start && household < start + MORTAL_PRODUCER_HOUSEHOLDS
        })
    }

    /// C3R.e (impl-67): whether `id` is a producer-house SUBJECT (a seeded/latent producer) — the
    /// key for the A2 bootstrap-sweep endowment split. Mirrors [`is_producer_subject_vocation`],
    /// keyed by agent id through the colonist arena. `false` for an unknown or dead-slot id.
    fn is_producer_subject_id(&self, id: AgentId) -> bool {
        self.colonist_slot_by_id
            .get(&id)
            .map(|&slot| &self.colonists[slot])
            .is_some_and(|colonist| {
                is_producer_subject_vocation(colonist.vocation, colonist.latent)
            })
    }

    fn record_producer_house_person_ticks(&mut self) {
        let Some(start) = self.producer_household_start() else {
            return;
        };
        let live = self
            .live_colonist_slots
            .iter()
            .filter(|&&slot| {
                self.colonists[slot].household.is_some_and(|household| {
                    household >= start && household < start + MORTAL_PRODUCER_HOUSEHOLDS
                })
            })
            .count() as u64;
        self.producer_house_person_ticks = self.producer_house_person_ticks.saturating_add(live);
    }

    fn record_producer_house_death(&mut self, id: AgentId) {
        if self
            .colonist_household(id)
            .is_some_and(|household| self.is_producer_household(household))
        {
            if let (true, Some(staple)) = (
                self.birth_stock_saving_active() || self.birth_stock_control_active(),
                self.provenance_bread_good(),
            ) {
                // Mirror the holding telemetry gate: only stock the member
                // accumulated from below the target counts as saved birth stock,
                // so a founder that dies still holding its seeded endowment is
                // not attributed to the saving motive.
                if self.birth_stock_below_target_agents.contains(&id)
                    || self.birth_stock_reached_agents.contains(&id)
                {
                    let held = self.society.free_stock_after_all_reserves(id, staple);
                    self.birth_stock_held_at_death = self.birth_stock_held_at_death.max(held);
                }
            }
            self.producer_house_deaths = self.producer_house_deaths.saturating_add(1);
        }
    }

    fn init_earned_provisioning_buckets(&mut self) {
        if !self.earned_provisioning_ledger_active() {
            return;
        }
        let ids: Vec<AgentId> = self
            .live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                let colonist = &self.colonists[slot];
                colonist
                    .household
                    .is_some_and(|household| self.is_producer_household(household))
                    .then_some(colonist.id)
            })
            .collect();
        for id in ids {
            let gold = self
                .society
                .agents
                .get(id)
                .map_or(Gold::ZERO, |agent| agent.gold);
            self.credit_earned_provisioning_lot(
                id,
                EarnedGoldLot {
                    source: EarnedGoldSource::Endowed,
                    amount: gold,
                },
            );
        }
    }

    fn credit_earned_provisioning_lot(&mut self, agent: AgentId, lot: EarnedGoldLot) {
        if lot.amount == Gold::ZERO || !self.earned_provisioning_ledger_active() {
            return;
        }
        self.earned_provisioning
            .buckets
            .entry(agent)
            .or_default()
            .credit(lot);
    }

    fn credit_earned_provisioning_lots(
        &mut self,
        agent: AgentId,
        lots: impl IntoIterator<Item = EarnedGoldLot>,
    ) {
        for lot in lots {
            self.credit_earned_provisioning_lot(agent, lot);
        }
    }

    fn debit_earned_provisioning_gold(
        &mut self,
        agent: AgentId,
        amount: Gold,
    ) -> (Gold, Gold, Gold) {
        if amount == Gold::ZERO || !self.earned_provisioning_ledger_active() {
            return (Gold::ZERO, Gold::ZERO, amount);
        }
        let Some(mut buckets) = self.earned_provisioning.buckets.remove(&agent) else {
            return (Gold::ZERO, Gold::ZERO, amount);
        };
        let debited = buckets.debit(amount);
        if !buckets.is_empty() {
            self.earned_provisioning.buckets.insert(agent, buckets);
        }
        debited
    }

    fn debit_earned_provisioning_lots(
        &mut self,
        agent: AgentId,
        amount: Gold,
    ) -> (Vec<EarnedGoldLot>, Gold) {
        if amount == Gold::ZERO || !self.earned_provisioning_ledger_active() {
            return (Vec::new(), amount);
        }
        let Some(mut buckets) = self.earned_provisioning.buckets.remove(&agent) else {
            return (Vec::new(), amount);
        };
        let debited = buckets.debit_lots(amount);
        if !buckets.is_empty() {
            self.earned_provisioning.buckets.insert(agent, buckets);
        }
        debited
    }

    fn earned_provisioning_transfer_gold_provenance(
        &mut self,
        from: AgentId,
        to: AgentId,
        amount: Gold,
    ) -> (Gold, Gold, Gold) {
        let (lots, untracked) = self.debit_earned_provisioning_lots(from, amount);
        let mut earned = Gold::ZERO;
        let mut endowed = Gold::ZERO;
        for lot in &lots {
            match lot.source {
                EarnedGoldSource::Earned => earned = earned.saturating_add(lot.amount),
                EarnedGoldSource::Endowed => endowed = endowed.saturating_add(lot.amount),
            }
        }
        self.credit_earned_provisioning_lots(to, lots);
        self.credit_earned_provisioning_lot(
            to,
            EarnedGoldLot {
                source: EarnedGoldSource::Endowed,
                amount: untracked,
            },
        );
        (earned, endowed, untracked)
    }

    fn producer_house_producers(&self) -> Vec<(AgentId, usize)> {
        let mut producers: Vec<_> = self
            .live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                let colonist = &self.colonists[slot];
                let household = colonist.household?;
                (self.is_producer_household(household)
                    && matches!(colonist.vocation, Vocation::Miller | Vocation::Baker))
                .then_some((colonist.id, household))
            })
            .collect();
        producers.sort_unstable_by_key(|&(id, household)| (id.0, household));
        producers
    }

    fn producer_house_members(&self, household: usize) -> Vec<AgentId> {
        let mut members: Vec<_> = self
            .live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                let colonist = &self.colonists[slot];
                (colonist.household == Some(household)).then_some(colonist.id)
            })
            .collect();
        members.sort_unstable_by_key(|id| id.0);
        members
    }

    fn free_agent_gold(&self, agent: AgentId) -> Gold {
        let free = self.society.free_gold_after_all_reserves(agent);
        let held = self
            .society
            .agents
            .get(agent)
            .map_or(Gold::ZERO, |agent| agent.gold);
        free.min(held)
    }

    fn has_unprovided_now_bread_want(&self, member: AgentId, bread: GoodId) -> bool {
        let Some(agent) = self.society.agents.get(member) else {
            return false;
        };
        let mut available = agent.stock.get(bread);
        for want in &agent.scale {
            if want.kind != WantKind::Good(bread) || matches!(want.horizon, Horizon::Later(_)) {
                continue;
            }
            let provided = available.min(want.qty);
            if provided == want.qty {
                available = available.saturating_sub(provided);
                continue;
            }
            if matches!(want.horizon, Horizon::Now) && !want.satisfied {
                return true;
            }
            available = 0;
        }
        false
    }

    fn birth_stock_attribution_snapshot(&self) -> BTreeSet<AgentId> {
        if !self.birth_stock_saving_active() {
            return BTreeSet::new();
        }
        let Some(staple) = self.provenance_bread_good() else {
            return BTreeSet::new();
        };
        self.live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                let colonist = &self.colonists[slot];
                let household = colonist.household?;
                if !self.is_producer_household(household)
                    || self.has_unprovided_now_bread_want(colonist.id, staple)
                {
                    return None;
                }
                self.society
                    .agents
                    .get(colonist.id)
                    .is_some_and(|agent| {
                        agent.scale.iter().any(|want| {
                            want.kind == WantKind::Good(staple)
                                && matches!(want.horizon, Horizon::Next)
                        })
                    })
                    .then_some(colonist.id)
            })
            .collect()
    }

    fn record_birth_stock_attributable_purchases(
        &mut self,
        spot_trades_start: usize,
        snapshot: &BTreeSet<AgentId>,
    ) {
        if snapshot.is_empty() {
            return;
        }
        let Some(staple) = self.provenance_bread_good() else {
            return;
        };
        let purchased = self.society.trades[spot_trades_start..]
            .iter()
            .filter(|trade| trade.good == staple && snapshot.contains(&trade.buyer))
            .map(|trade| u64::from(trade.qty))
            .sum::<u64>();
        self.birth_stock_attributable_purchases = self
            .birth_stock_attributable_purchases
            .saturating_add(purchased);
    }

    /// C3R.e-obs (impl-66): the per-tick §2 loss-decomposition join. Runs after the market
    /// step, draining this tick's allocation-trace batch and assigning EXACTLY ONE outcome
    /// to each unfilled saving quote-opportunity (a snapshot member's staple bid). A no-op
    /// unless the obs is active. Reads only the trace + intent map — never re-enables or
    /// drains the (behavior-driving) consumption log.
    fn observe_saving_allocation(
        &mut self,
        spot_trades_start: usize,
        snapshot: &BTreeSet<AgentId>,
    ) {
        if !self.saving_allocation_obs_active() {
            return;
        }
        let trace = self.society.take_allocation_trace();
        let Some(staple) = self.provenance_bread_good() else {
            return;
        };
        // No money-priced spot pass this tick (barter / pre-promotion): the trace is empty.
        // Eligible members had no opportunity — excluded from the domain, counted separately.
        if trace.is_empty() {
            if !snapshot.is_empty() {
                self.saving_allocation_obs.no_spot_pass_ticks += 1;
            }
            return;
        }
        if snapshot.is_empty() {
            return;
        }
        let input_goods = self.saving_obs_input_goods();
        // Filled uses the SAME attribution predicate as the denominator (bought ≥1 staple).
        let mut filled_members: BTreeSet<AgentId> = BTreeSet::new();
        for trade in &self.society.trades[spot_trades_start..] {
            if trade.good == staple && snapshot.contains(&trade.buyer) {
                filled_members.insert(trade.buyer);
            }
        }
        let intent = self.society.allocation_intent();
        let mut filled = 0u64;
        let mut outcomes: Vec<(AgentId, SavingLossOutcome)> = Vec::new();
        for &member in snapshot {
            if filled_members.contains(&member) {
                filled += 1;
                continue;
            }
            outcomes.push((
                member,
                classify_saving_opportunity(&trace, intent, staple, member, &input_goods),
            ));
        }
        // The `intent` borrow of `self.society` ends here; now mutate the accumulator.
        self.saving_allocation_obs.filled += filled;
        for (member, outcome) in outcomes {
            if !member_has_staple_bid_attempt(&trace, staple, member) {
                // A genuine drop (no staple bid attempt recorded) — logged for §6 honesty,
                // NOT silently absorbed. It is still counted as ExecutionResidual so the
                // totality invariant (one outcome per unfilled opportunity) holds.
                self.saving_allocation_obs.drops += 1;
            }
            self.saving_allocation_obs.record(outcome);
        }
        // §5 supply-side series: this IS a money-priced spot pass (the trace was non-empty
        // and the snapshot non-empty), so record the PRE-market offerable supply captured
        // before the step against this tick's trace-authoritative posted-ask count. Lets a
        // reader tell genuine scarcity from quote-generation failure within OfferScarcity.
        let posted_asks = count_window_staple_asks(&trace, staple);
        let (offerable_sellers_member, offerable_sellers_other) =
            self.saving_obs_pending_offerable.take().unwrap_or((0, 0));
        self.saving_allocation_obs
            .supply_series
            .push(SavingSupplyTick {
                tick: self.econ_tick,
                offerable_sellers_member,
                offerable_sellers_other,
                posted_asks,
            });
    }

    /// C3R.e-obs (§3.2c): begin the corrected stock-phase timeline immediately before
    /// deaths. Physical `agent.stock` and reservations are captured independently.
    fn saving_obs_begin_stock_tick(&mut self) {
        if !self.saving_allocation_obs_active() {
            return;
        }
        let Some(staple) = self.provenance_bread_good() else {
            return;
        };
        self.saving_obs_stock_tick = Some(SavingStockTick {
            staple,
            previous: self.saving_stock_snapshot(staple),
            produced: 0,
            endowment: 0,
            consumed_as_input: 0,
            market_consumed: 0,
        });
    }

    fn saving_obs_capture_death_phase(&mut self) {
        let Some(tick) = self.saving_obs_stock_tick else {
            return;
        };
        let current = self.saving_stock_snapshot(tick.staple);
        // This seam contains only death/estate handling, so its measured physical move
        // is itself the named death-estate cause (including transfers to commons).
        let attributed = current.physical as i64 - tick.previous.physical as i64;
        self.saving_obs_record_stock_phase(SavingStockPhase::Death, current, attributed);
    }

    fn saving_obs_capture_pre_market(&mut self, report: &EconTickReport) {
        let Some(tick) = self.saving_obs_stock_tick else {
            return;
        };
        let produced = report.produced.get(&tick.staple).copied().unwrap_or(0);
        let endowment = report.endowment.get(&tick.staple).copied().unwrap_or(0);
        let consumed_as_input = report
            .consumed_as_input
            .get(&tick.staple)
            .copied()
            .unwrap_or(0);
        let attributed = produced as i64 - tick.produced as i64 + endowment as i64
            - tick.endowment as i64
            - (consumed_as_input as i64 - tick.consumed_as_input as i64);
        let current = self.saving_stock_snapshot(tick.staple);
        self.saving_obs_record_stock_phase(SavingStockPhase::PreMarket, current, attributed);
        if let Some(tick) = self.saving_obs_stock_tick.as_mut() {
            tick.produced = produced;
            tick.endowment = endowment;
            tick.consumed_as_input = consumed_as_input;
        }
    }

    /// C3R.e-obs (§3.2a/§5): capture this tick's offerable staple supply from the PRE-market
    /// state the live ask path evaluates (post-death, before `society.step()`). Sums the
    /// pure `reservation_ask_for_money(staple, 1, money_good)` counterfactual over every
    /// potential seller, split by attribution-snapshot membership. Advisory (the live path
    /// removes other reserves and restores the agent's own quote — §3.2a); the trace's
    /// posted asks stay authoritative. Pure/read-only: no state moves, so this shifts no
    /// digest. Stashed for the §2 join, which commits it only on money-priced spot passes.
    fn saving_obs_capture_offerable_supply(&mut self, snapshot: &BTreeSet<AgentId>) {
        if !self.saving_allocation_obs_active() {
            return;
        }
        let Some(staple) = self.provenance_bread_good() else {
            return;
        };
        let Some(money_good) = self.society.current_money_good() else {
            // No money good yet (pre-promotion barter): not a money-priced spot pass, so the
            // join discards this. Record zero offerable to keep the scratch fresh.
            self.saving_obs_pending_offerable = Some((0, 0));
            return;
        };
        let mut member = 0u64;
        let mut other = 0u64;
        for agent in self.society.agents.iter() {
            if agent
                .reservation_ask_for_money(staple, 1, money_good)
                .is_some()
            {
                if snapshot.contains(&agent.id) {
                    member += 1;
                } else {
                    other += 1;
                }
            }
        }
        self.saving_obs_pending_offerable = Some((member, other));
    }

    fn saving_obs_capture_post_market(&mut self, report: &EconTickReport) {
        let Some(tick) = self.saving_obs_stock_tick else {
            return;
        };
        let consumed = self.saving_obs_consumed_stock(tick.staple);
        let promoted = report.promoted.get(&tick.staple).copied().unwrap_or(0);
        let current = self.saving_stock_snapshot(tick.staple);
        self.saving_obs_record_stock_phase(
            SavingStockPhase::Market,
            current,
            -(consumed as i64) - promoted as i64,
        );
        if let Some(tick) = self.saving_obs_stock_tick.as_mut() {
            tick.market_consumed = consumed;
        }
    }

    fn saving_obs_capture_post_production(&mut self, report: &EconTickReport) {
        let Some(tick) = self.saving_obs_stock_tick else {
            return;
        };
        let produced = report.produced.get(&tick.staple).copied().unwrap_or(0);
        let consumed_as_input = report
            .consumed_as_input
            .get(&tick.staple)
            .copied()
            .unwrap_or(0);
        let consumed = self.saving_obs_consumed_stock(tick.staple);
        let attributed = produced as i64
            - tick.produced as i64
            - (consumed_as_input as i64 - tick.consumed_as_input as i64)
            - (consumed as i64 - tick.market_consumed as i64);
        let current = self.saving_stock_snapshot(tick.staple);
        self.saving_obs_record_stock_phase(SavingStockPhase::ProductionOwnUse, current, attributed);
        if let Some(tick) = self.saving_obs_stock_tick.as_mut() {
            tick.produced = produced;
            tick.consumed_as_input = consumed_as_input;
        }
    }

    fn saving_obs_capture_post_birth(&mut self) {
        let Some(tick) = self.saving_obs_stock_tick else {
            return;
        };
        // Birth stock is a parent debit/newborn credit; its named net physical cause is 0.
        let current = self.saving_stock_snapshot(tick.staple);
        self.saving_obs_record_stock_phase(SavingStockPhase::Birth, current, 0);
    }

    fn saving_obs_finish_stock_tick(&mut self, report: &EconTickReport) {
        let Some(tick) = self.saving_obs_stock_tick else {
            return;
        };
        let spoiled = report.spoiled.get(&tick.staple).copied().unwrap_or(0);
        let current = self.saving_stock_snapshot(tick.staple);
        self.saving_obs_record_stock_phase(SavingStockPhase::EndOfTick, current, -(spoiled as i64));
        let consumed = self.saving_obs_consumed_stock(tick.staple);
        let obs = &mut self.saving_allocation_obs;
        obs.phys_produced = obs.phys_produced.saturating_add(tick.produced);
        obs.phys_consumed = obs.phys_consumed.saturating_add(consumed);
        self.saving_obs_stock_tick = None;
    }

    fn saving_obs_record_stock_phase(
        &mut self,
        phase: SavingStockPhase,
        current: SavingStockSnapshot,
        attributed_delta: i64,
    ) {
        let tick = self
            .saving_obs_stock_tick
            .as_mut()
            .expect("stock observation phase requires a started tick");
        let physical_delta = current.physical as i64 - tick.previous.physical as i64;
        let reservation_delta = current.reserved as i64 - tick.previous.reserved as i64;
        tick.previous = current;
        let obs = &mut self.saving_allocation_obs;
        let target = match phase {
            SavingStockPhase::Death => &mut obs.death_phase,
            SavingStockPhase::PreMarket => &mut obs.pre_market_phase,
            SavingStockPhase::Market => &mut obs.market_phase,
            SavingStockPhase::ProductionOwnUse => &mut obs.production_own_use_phase,
            SavingStockPhase::Birth => &mut obs.birth_phase,
            SavingStockPhase::EndOfTick => &mut obs.end_of_tick_phase,
        };
        target.record(physical_delta, reservation_delta, attributed_delta);
        obs.phys_net_delta += physical_delta;
        obs.phys_within_phase_ambiguous += physical_delta - attributed_delta;
    }

    fn saving_stock_snapshot(&self, staple: GoodId) -> SavingStockSnapshot {
        let mut physical = 0u64;
        let mut reserved = 0u64;
        for agent in self.society.agents.iter() {
            let held = agent.stock.get(staple);
            physical = physical.saturating_add(u64::from(held));
            let free = self.society.free_stock_after_all_reserves(agent.id, staple);
            reserved = reserved.saturating_add(u64::from(held.saturating_sub(free)));
        }
        SavingStockSnapshot { physical, reserved }
    }

    fn saving_obs_consumed_stock(&self, staple: GoodId) -> u64 {
        self.society
            .consumption_log_last_tick()
            .iter()
            .filter(|(_, good, _)| *good == staple)
            .map(|&(_, _, qty)| u64::from(qty))
            .sum()
    }

    /// The producer recipe INPUT goods (grain, flour) used to tag a `ProducerInputNext`
    /// winner intent. Empty without a chain.
    fn saving_obs_input_goods(&self) -> BTreeSet<GoodId> {
        let mut goods = BTreeSet::new();
        if let Some(chain) = self.chain.as_ref() {
            goods.insert(chain.content.grain());
            goods.insert(chain.content.flour());
        }
        goods
    }

    /// The runtime-only C3R.e-obs loss-decomposition accumulator for this run (one seed).
    /// NEVER serialized; read by the acceptance suite to print family shares + diagnosis.
    pub fn saving_allocation_obs_report(&self) -> &SavingAllocationObs {
        &self.saving_allocation_obs
    }

    /// C3R.e-obs (impl-66 repair): the C3R.d birth-stock attribution snapshot captured at the
    /// most recent pre-market seam inside [`Self::econ_tick`] while allocation observation is
    /// active. Runtime-only, read-only, NEVER serialized and NEVER read by a decision path.
    /// The acceptance suite reads this after each tick so deaths and pre-market provisioning
    /// cannot make a before/after recomputation drift from the opportunity domain the market
    /// actually saw.
    pub fn birth_stock_attribution_members(&self) -> &BTreeSet<AgentId> {
        &self.last_birth_stock_attribution_snapshot
    }

    fn init_birth_stock_reach_baseline(&mut self) {
        if !self.birth_stock_saving_active() && !self.birth_stock_control_active() {
            return;
        }
        let (Some(staple), Some(demo)) = (self.provenance_bread_good(), self.demography.as_ref())
        else {
            return;
        };
        let target = demo.child_food_endowment;
        self.birth_stock_below_target_agents = self
            .live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                let colonist = &self.colonists[slot];
                colonist
                    .household
                    .is_some_and(|household| self.is_producer_household(household))
                    .then_some(colonist.id)
            })
            .filter(|&agent| self.society.free_stock_after_all_reserves(agent, staple) < target)
            .collect();
    }

    fn observe_birth_stock_holdings(&mut self) {
        if !self.birth_stock_saving_active() && !self.birth_stock_control_active() {
            return;
        }
        let (Some(staple), Some(demo)) = (self.provenance_bread_good(), self.demography.as_ref())
        else {
            return;
        };
        let target = demo.child_food_endowment;
        for slot_index in 0..self.live_colonist_slots.len() {
            let slot = self.live_colonist_slots[slot_index];
            let colonist = &self.colonists[slot];
            if !colonist
                .household
                .is_some_and(|household| self.is_producer_household(household))
            {
                continue;
            }
            let agent = colonist.id;
            let held = self.society.free_stock_after_all_reserves(agent, staple);
            if held < target {
                self.birth_stock_below_target_agents.insert(agent);
            } else if self.birth_stock_below_target_agents.remove(&agent) {
                self.birth_stock_reached_agents.insert(agent);
            }
            // Only stock accumulated from below the target counts as saved birth
            // stock: seeded founders (start above the target) and newborn
            // endowments (start at the target) never enter the below-target
            // population, so their endowments cannot inflate the holding telemetry.
            if self.birth_stock_below_target_agents.contains(&agent)
                || self.birth_stock_reached_agents.contains(&agent)
            {
                self.birth_stock_held_max = self.birth_stock_held_max.max(held);
            }
        }
    }

    fn record_birth_stock_control_results(&mut self, injected: &[usize]) {
        for &household in injected {
            self.birth_stock_injection_records
                .push(BirthStockInjectionRecord {
                    tick: self.econ_tick,
                    household,
                    birth_succeeded: self.households[household].last_birth_tick
                        == Some(self.econ_tick),
                });
        }
    }

    fn finalize_earned_provisioning_market(
        &mut self,
        spot_trades_start: usize,
        funded_bid_members: &[AgentId],
    ) {
        if !self.earned_provisioning_ledger_active() {
            return;
        }
        let Some(bread) = self.provenance_bread_good() else {
            return;
        };
        let mut filled: BTreeSet<AgentId> = BTreeSet::new();
        for trade in &self.society.trades[spot_trades_start..] {
            if trade.good == bread {
                if self
                    .colonist_household(trade.buyer)
                    .is_some_and(|household| self.is_producer_household(household))
                {
                    self.earned_provisioning.stats.members_fed_by_purchase = self
                        .earned_provisioning
                        .stats
                        .members_fed_by_purchase
                        .saturating_add(1);
                }
                filled.insert(trade.buyer);
            }
        }
        for &member in funded_bid_members {
            if !filled.contains(&member) {
                self.earned_provisioning.stats.funded_but_unfilled = self
                    .earned_provisioning
                    .stats
                    .funded_but_unfilled
                    .saturating_add(1);
            }
        }
    }

    fn earned_buyer_class(
        &self,
        buyer: AgentId,
        buyer_household: Option<usize>,
    ) -> EarnedBuyerClass {
        if let Some(household) = buyer_household {
            if self.is_producer_household(household) {
                return EarnedBuyerClass::OtherProducerHousehold;
            }
            return EarnedBuyerClass::Lineage;
        }
        match self
            .slot_for_id(buyer)
            .map(|slot| self.colonists[slot].vocation)
        {
            Some(Vocation::Gatherer) => EarnedBuyerClass::Gatherer,
            Some(Vocation::Consumer) | None => EarnedBuyerClass::ImmortalConsumer,
            Some(_) => EarnedBuyerClass::OtherProducerHousehold,
        }
    }

    /// S7.2: whether the per-builder capital-formation phase is active — its appraisal
    /// knobs and the in-flight build state below it in the digest serialize only when
    /// it can run (gated on the chain flag), so a producible-capital-OFF config
    /// serializes exactly the pre-S7 stream.
    fn producible_capital_can_run(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(|chain| chain.producible_capital)
    }

    /// S10: whether the per-agent intertemporal build decision replaces the S7 heuristic —
    /// gated on both flags (per-agent mode requires the producible-capital substrate). A
    /// pure function of the chain flags. When this holds, `capital_payback_cycles` is
    /// behaviour-INERT (no uniform payback is charged), so `canonical_bytes` serializes
    /// the `per_agent_capital` flag in its place rather than the inert knob (avoiding false
    /// digest splits); when it is off the legacy stream — `capital_payback_cycles`, no flag
    /// — is byte-identical to pre-S10.
    fn per_agent_capital_can_run(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(|chain| chain.producible_capital && chain.per_agent_capital)
    }

    /// S11: whether per-agent fallible OUTPUT-price forecasts steer the entrepreneurial
    /// appraisals — gated on the chain flag. When this holds, the role-choice adopt, the
    /// per-agent capital build, and the project input-bid weigh each agent's
    /// `forecast_output_price` instead of the raw realized price, and `canonical_bytes`
    /// serializes the per-colonist `forecast_bias_bps` + the per-belief `observed` flag
    /// (the new state that steers future ticks). A pure function of the chain flag; with
    /// it off every appraisal reads the raw realized price and the stream is byte-identical
    /// to pre-S11.
    fn entrepreneurial_can_run(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(|chain| chain.entrepreneurial_forecasts)
    }

    /// C3R.h (L2): whether the fresh-input-price branch can steer role choice. Mirror
    /// [`Self::run_role_choice`]'s candidate gate — the LIVE roster's latent specialties
    /// (`phases.rs`, `for &slot in &self.live_colonist_slots`), not `self.colonists`,
    /// which retains dead historical entries — so an inert flag does not split
    /// behavior-equivalent digests once the last latent candidate dies.
    fn stale_input_price_fix_can_run(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(|chain| chain.stale_input_price_fix)
            && (self.tool_acquisition_can_run()
                || self
                    .live_colonist_slots
                    .iter()
                    .any(|&slot| self.colonists[slot].latent.is_some()))
    }

    /// Minimum PER-UNIT reservation ask for the input good among other living colonist
    /// holders. This is an optimistic appraisal lower bound, not a live or matched quote.
    /// It reads only serialized agent state and iterates in deterministic roster order.
    ///
    /// Deliberately the RESERVATION, not the executable quote. `Society::ensure_ask` posts
    /// `belief.shade_ask(reservation)` off a reservation-netted `available_agent` snapshot
    /// (`econ/src/society.rs:3313,3343-3344,3599`). That snapshot can produce a different
    /// reservation than the raw agent used here, while `shade_ask` only floors the quote at
    /// its OWN reservation (`econ/src/expect.rs:40-42`); a holder whose stock is already
    /// reserved may not be able to post at all. This proxy therefore has no ordering
    /// guarantee against a live ask. A candidate can adopt against a price the book would
    /// not currently honor; the measurement reports that as bread output and baker survival
    /// rather than suppressing it. It stays a reservation on purpose: the netting reads
    /// `Society::reservations` / `live_quotes`, which `canonical_bytes` does NOT serialize,
    /// so steering role choice by them would make this DIGESTED flag depend on undigested
    /// order-book state — the determinism contract this lever is built on.
    ///
    /// Deliberately queried for ONE unit: `reservation_ask_for_money` prices the whole
    /// `qty` bundle (the gain enters gold once, `econ/src/agent.rs:449-488`), while both
    /// consumers multiply the returned price by the recipe's `input_qty`
    /// ([`recipe_adoption_pays_for_money`], [`recipe_is_profitable`]) — as they must for
    /// the flag-off substitute, the per-unit `realized_price`. Asking for the bundle here
    /// would hand those consumers an already-`qty`-sized price to multiply by `input_qty`
    /// a second time — a double-count that is at least quadratic in `input_qty`, since
    /// `reservation_ask_for_money` is superadditive in `qty` (each further unit is given
    /// up from a higher-ranked want). Both current chain recipes request one input unit
    /// (`content.rs:80,88`), so the two readings coincide today; this keeps them
    /// coincident under a future yield-ratio change.
    ///
    /// Scope is the LIVE COLONIST roster, matching the phase that calls it. Resident
    /// traders can hold tracked stock and quote in the same book, but only `region.rs`
    /// seeds any (every `SettlementConfig` constructor leaves `resident_traders` empty),
    /// and no region config runs a chain with this flag. The omission is safe in the one
    /// direction that matters: dropping a holder can only leave the minimum higher or
    /// absent, never cheaper than a real ask, so it cannot manufacture an adoption.
    ///
    /// Returns `None` when no other living colonist holds the good — and also when the
    /// input good IS the current money good, which `reservation_ask_for_money` prices as
    /// `None` unconditionally (`econ/src/agent.rs:449`). Both are declines, never a free
    /// input; see [`RoleChoiceReason::InputPriceAbsent`].
    ///
    /// Cost: one full roster scan per candidate per recipe per tick, so role choice is
    /// O(live colonists²) while the flag is on. Immaterial at the roster sizes this
    /// default-off research lever is measured at; revisit before any default-on promotion.
    fn fresh_input_ask(
        &self,
        appraiser: AgentId,
        input_good: GoodId,
        money_good: GoodId,
    ) -> Option<Gold> {
        let mut best: Option<Gold> = None;
        for &slot in &self.live_colonist_slots {
            let id = self.colonists[slot].id;
            if id == appraiser {
                continue;
            }
            let Some(agent) = self.society.agents.get(id) else {
                continue;
            };
            // `reservation_ask_for_money` already returns `None` for a non-holder (it
            // requires `stock.can_remove(good, qty)`), so this is the "other living
            // colonists that currently hold the input good" filter.
            let Some(ask) = agent.reservation_ask_for_money(input_good, 1, money_good) else {
                continue;
            };
            best = Some(best.map_or(ask, |best: Gold| best.min(ask)));
        }
        best
    }

    /// S12: whether the own-labor subsistence path is active — the food mints are
    /// retired and a hungry, eligible colonist forages the FORAGE floor. Gated on the
    /// chain flag (and, for a real path, a forage good in the content). When this holds,
    /// `canonical_bytes` serializes the forage knobs + the per-colonist `foraging` state;
    /// with it off the stream is byte-identical to pre-S12.
    fn own_labor_subsistence_can_run(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_own_labor_subsistence_can_run)
    }

    /// S14: whether the **capped FORAGE commons** is active — own-labor subsistence can
    /// run AND a [`ForageCommons`] is configured. When this holds the FORAGE node is a
    /// real depleting [`world::ResourceNode`], foragers harvest it through the GoHarvest
    /// haul cycle (so per-capita yield falls with the foraging population), the fixed
    /// [`ChainConfig::forage_yield`] credit is retired, the deposit attribution covers
    /// foragers, and births endow children from FORAGE (the [`Self::birth_food`]
    /// selector). Off (every existing config), the S12 fixed-credit path is unchanged.
    fn forage_commons_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_forage_commons_active)
    }

    /// S15: whether the **own-use cultivation** path can run this tick — the
    /// `own_use_cultivation` flag is on, the content set carries the no-tool `Cultivate`
    /// recipe, AND the own-labor/forage path it composes on is active (so the foraging
    /// eligibility + the FORAGE node it escalates from exist). When this holds, a
    /// still-hungry forager is steered to GoHarvest the grain node and cultivate bread
    /// (the [`Self::run_own_use_cultivation`] phase), births broaden to endow from any
    /// edible food, and the cultivation steering state + config knobs enter the digest.
    /// Off (every existing config), the S14 path is unchanged and byte-identical.
    fn own_use_cultivation_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_own_use_cultivation_active)
    }

    /// S21f: whether the **household-barter cultivation seam** is active this tick — the flag
    /// is on AND the S15 own-use cultivation knobs (the gate + the `Cultivate` recipe) are
    /// present. When this holds, the cultivation steering phase
    /// ([`Self::run_own_labor_subsistence`]) runs WITHOUT the own-labor/forage substrate (no
    /// forage good interned), so eligible lineage members escalate to cultivation directly
    /// from sustained hunger. Off (every existing config), cultivation still requires the
    /// forage substrate and the run is byte-identical. Note that whenever this holds,
    /// [`Self::own_use_cultivation_active`] also holds (the seam is one of its substrates).
    fn household_barter_cultivation_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_household_barter_cultivation_active)
    }

    /// S16: whether the **money-from-produced-bread** path is active this tick — the
    /// `cultivation_sells_surplus` flag is on AND own-use cultivation can run. When this
    /// holds, two gated behaviors engage: the buy/sell split (only lineage spatial members
    /// forage/cultivate, so seeded SALT consumers stay the buy side) and the produced-bread
    /// provenance ledger (the stock-origin attribution of bread→medium trades). Off (every
    /// existing config), both are inert and the run is byte-identical.
    fn cultivation_sells_surplus_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_cultivation_sells_surplus_active)
    }

    /// S22a: whether **endogenous cultivation entry** is active this tick — the
    /// `endogenous_cultivation_entry` flag is on AND the money-from-produced-bread path is
    /// active. When this holds, the own-labor subsistence cultivation eligibility set is
    /// relaxed from the spatial lineage to ANY spatial colonist (the
    /// `Consumer|Gatherer|Unassigned` vocation filter is preserved), so the food-producing
    /// class can self-form from sustained hunger via the existing pressure/patience
    /// hysteresis. Off (every existing config), eligibility stays pinned to the lineage and
    /// the run is byte-identical. Mirrors [`Self::emergency_self_provision_active`].
    fn endogenous_cultivation_entry_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_endogenous_cultivation_entry_active)
    }

    fn secure_partible_land_active(&self) -> bool {
        self.chain.as_ref().is_some_and(|chain| {
            chain_runtime_secure_land_tenure_active(chain)
                && chain.inheritance_regime == InheritanceRegime::Partible
        })
    }

    fn mortal_landowner_demography_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_mortal_landowner_demography_active)
    }

    fn share_forward_provisioning_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_share_forward_provisioning_active)
            && self.provenance_bread_good().is_some()
    }

    fn share_contract_succession_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_share_contract_succession_active)
            && self.provenance_bread_good().is_some()
    }

    /// S22b: whether **bounded cultivation skill** is active this tick — the
    /// `cultivation_skill` flag is on AND the S22a endogenous-cultivation-entry path is active
    /// (it composes strictly on it). When this holds, the per-agent skill scalar accumulates/
    /// decays each econ tick, a skilled cultivator's grain trip uses the
    /// [`world::Task::GoHarvestWithRoom`] per-trip haul override, and the skill state + parameters
    /// enter the digest (ON-only). Off (every existing config) it is `false`, so the grain-haul
    /// lever is inert, no skill bytes are serialized, and the run is byte-identical.
    fn cultivation_skill_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_cultivation_skill_active)
    }

    /// S22c: whether **profit-driven cultivation retention** is active this tick — the
    /// `profit_driven_retention` flag is on AND the S22a endogenous-cultivation-entry path is
    /// active (it composes strictly on it; orthogonal to `cultivation_skill`). When this holds,
    /// the per-agent rolling cultivation-sale/outside-sale return window is maintained (and
    /// digested ON-only), and the cultivation *exit* branch consults [`Self::profit_stay_active`].
    /// Off (every existing config) it is `false`, so the window is never created/consulted, the
    /// exit stays pure hunger/pressure, and the run is byte-identical.
    fn profit_driven_retention_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_profit_driven_retention_active)
    }

    /// S22d: whether **durable role-specific cultivation capital** is active this tick — the
    /// `durable_cultivation_tool` flag is on, the S22c profit-driven-retention path is active
    /// (it composes strictly on it), and the content set carries the cultivation-tool good. When
    /// this holds, a sustained-producing cultivator may build the durable cultivation tool from a
    /// sunk WOOD+labor cost
    /// ([`Self::run_cultivation_capital_formation`]), a tool-OWNER's grain trip uses the higher
    /// owner haul ceiling while it cultivates, the per-agent cultivation tenure is credited, and
    /// the tenure + build params + in-flight builds enter the digest (ON-only). Off (every
    /// existing config) it is `false`, so all of it is inert and the run is byte-identical.
    fn durable_cultivation_tool_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_durable_cultivation_tool_active)
    }

    /// S22f: whether the **voluntary fixed-term cultivation commitment** path is active this tick —
    /// the `voluntary_cultivation_commitment` flag is on AND the S22c profit-driven-retention path is
    /// active (which itself requires S22a entry + the money-from-produced-bread path). When this
    /// holds, the per-agent commitment state is maintained + digested ON-only, the voluntary entry
    /// seam (gated post-money) can opt eligible agents in, and the cultivation exit is overridden for
    /// the bound term. Off (every existing config) it is `false`, so all of it is inert and the run
    /// is byte-identical. (Pre-money inertness is enforced at the entry seam via `current_money_good`.)
    fn voluntary_cultivation_commitment_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_voluntary_cultivation_commitment_active)
    }

    fn abandonable_norm_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_abandonable_norm_active)
    }

    fn group_payoff_imitation_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_group_payoff_imitation_active)
    }

    /// S22f: the configured commitment binding length (econ ticks), falling back to the pinned
    /// default for a settlement with no chain. Consulted only while the commitment path is active.
    fn commitment_term(&self) -> u16 {
        self.chain
            .as_ref()
            .map_or(COMMITMENT_TERM_DEFAULT, |c| c.commitment_term)
    }

    /// S22f: the configured commitment entry floor (windowed realized cultivation proceeds a
    /// candidate must clear), falling back to the pinned default for a settlement with no chain.
    /// Consulted only while the commitment path is active.
    fn commitment_entry_floor(&self) -> u64 {
        self.chain
            .as_ref()
            .map_or(COMMITMENT_ENTRY_FLOOR_DEFAULT, |c| c.commitment_entry_floor)
    }

    /// S22f: the configured fiat-pin count (forced commitments bypassing the voluntary signal); `0`
    /// (the headline + every other config) leaves entry purely voluntary.
    fn commitment_fiat_pin(&self) -> u16 {
        self.chain.as_ref().map_or(0, |c| c.commitment_fiat_pin)
    }

    /// S22d: whether the **non-durable / rented-tool control** is active this tick — the
    /// `cultivation_tool_non_durable` flag is on AND the durable-cultivation-capital path is active.
    /// When this holds, a built plow is consumed (booked `consumed_as_input`) after the one
    /// cultivation opportunity it boosts, so no persistent stock accrues. Off (the durable headline
    /// + every existing config) it is `false`, so a built plow persists.
    fn cultivation_tool_non_durable_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(|c| c.cultivation_tool_non_durable)
            && self.durable_cultivation_tool_active()
    }

    /// S22e: whether the **endowed + inherited cultivation-capital** path is active this tick — the
    /// `endowed_cultivation_capital` flag is on AND the durable-cultivation-capital path is active
    /// (which already requires S22a entry + S22c profit-stay + the plow content good). When this
    /// holds, a minority of lineage households were endowed a plow at generation and the
    /// plow-routing switch governs estate inheritance. Off (every existing config) it is `false`, so
    /// no household is endowed and the estate routing is byte-identical.
    fn endowed_cultivation_capital_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(|c| c.endowed_cultivation_capital)
            && self.durable_cultivation_tool_active()
    }

    /// S22e canonical marker gate. The zero-endowment / inheritance-on configuration is behavior-
    /// identical to the expanded S22d base: it grants no plows, and plows that are later built keep
    /// the existing heir route. Omit tag 11 for that inert combination while still marking either
    /// real endowment (`endowed_tool_count > 0`) or the no-inheritance estate-routing switch.
    fn endowed_cultivation_capital_digest_active(&self) -> bool {
        self.endowed_cultivation_capital_active()
            && self
                .chain
                .as_ref()
                .is_some_and(|c| c.endowed_tool_count > 0 || !c.cultivation_tool_inheritance)
    }

    /// S22e: whether plows keep the existing heir routing on death (`true`, the default under the
    /// gate) versus being FORCED to the commons (`false`, the no-inheritance control). Only meaningful
    /// while [`Self::endowed_cultivation_capital_active`] holds; defaults to the inheriting behavior
    /// off the path so a non-S22e estate settlement is untouched.
    fn cultivation_tool_inheritance_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_none_or(|c| c.cultivation_tool_inheritance)
    }

    /// S22d: the durable cultivation tool good (the plow), when a cultivation-capital content set
    /// carries one. `None` off the path / for a settlement with no chain — so the owner-haul
    /// boost, the build, and the owner diagnostics are all inert.
    fn cultivation_tool_good(&self) -> Option<GoodId> {
        self.chain
            .as_ref()
            .and_then(|c| c.content.cultivation_tool())
    }

    /// S22c: the configured profit-driven-retention parameters `(return_window, margin_bps,
    /// material_floor)`, falling back to the pinned defaults for a settlement with no chain. The
    /// controls/sweep override the chain fields, never the constants.
    fn retention_params(&self) -> (u64, u64, u64) {
        self.chain.as_ref().map_or(
            (
                RETURN_WINDOW,
                RETENTION_MARGIN_BPS,
                RETENTION_MATERIAL_FLOOR,
            ),
            |c| {
                (
                    c.return_window,
                    c.retention_margin_bps,
                    c.retention_material_floor,
                )
            },
        )
    }

    /// S22c: the colony **reference outside rate** — the pooled realized non-cultivation sale
    /// proceeds and sale-tick count over every live colonist that was NOT cultivating at the START
    /// of this econ tick (`cultivating_at_pass_start`, snapshotted before the stay loop mutates the
    /// flag, so the pooled cohort is roster-order-independent) and has realized at least one outside
    /// sale in its window (`Σ outside_sum / Σ outside_ticks` as a rational `(sum, ticks)`). `None`
    /// when no such reference seller exists (the outside option is then reported as *weak* and falls
    /// back to 0 in [`Self::profit_stay_active`]). The fallback a continuous cultivator with no
    /// recent outside ticks of its own compares against (§8.2).
    fn colony_reference_outside_rate(
        &self,
        return_window: u64,
        cultivating_at_pass_start: &BTreeSet<AgentId>,
    ) -> Option<(u64, u64)> {
        let tick = self.econ_tick;
        let mut sum = 0u64;
        let mut ticks = 0u64;
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            if cultivating_at_pass_start.contains(&colonist.id) {
                continue;
            }
            let (_c_sum, _c_ticks, o_sum, o_ticks) =
                window_return_sums(&colonist.cultivation_return_window, tick, return_window);
            if o_ticks > 0 {
                sum = sum.saturating_add(o_sum);
                ticks = ticks.saturating_add(o_ticks);
            }
        }
        (ticks > 0).then_some((sum, ticks))
    }

    /// S22c: the post-money cultivation **stay** decision — whether a currently-cultivating
    /// colonist `id` should remain cultivating past the normal hunger exit because its realized
    /// cultivation return clears its outside option. Returns true iff ALL of:
    /// * profit-driven retention is active AND `current_money_good() == Some(SALT)` (the hard
    ///   anti-circularity gate — inert pre-money; the signal itself is post-money realized
    ///   sale proceeds), AND
    /// * its windowed realized cultivation-sale proceeds clear the material floor
    ///   ([`RETENTION_MATERIAL_FLOOR`]) over ≥1 sale tick (it is realizing cultivation sales — not
    ///   one dust sale, not vacuous), AND
    /// * its cultivation per-sale-tick rate ≥ its outside rate + [`RETENTION_MARGIN_BPS`], where the
    ///   outside rate is its OWN realized non-cultivation rate when it has recent outside ticks,
    ///   else the colony reference rate, else 0 (the outside option reported as *weak*).
    ///
    /// All comparisons are exact integer cross-multiplications (no float, no RNG). Read-only; the
    /// cultivation exit branch consults it. `false` for an unknown id / off the path / pre-money.
    fn profit_stay_active(
        &self,
        id: AgentId,
        cultivating_at_pass_start: &BTreeSet<AgentId>,
    ) -> bool {
        if !self.profit_driven_retention_active() || self.society.current_money_good() != Some(SALT)
        {
            return false;
        }
        let Some(&slot) = self.colonist_slot_by_id.get(&id) else {
            return false;
        };
        let (return_window, margin_bps, material_floor) = self.retention_params();
        let tick = self.econ_tick;
        let (cult_sum, cult_ticks, out_sum, out_ticks) = window_return_sums(
            &self.colonists[slot].cultivation_return_window,
            tick,
            return_window,
        );
        // Material floor: a real, recurring realized cultivation gain (not one dust sale; not
        // vacuous). Without it a single tiny sale could lock an agent in.
        if cult_ticks == 0 || cult_sum < material_floor {
            return false;
        }
        // Outside option: the agent's OWN realized non-cultivation rate, else the colony reference,
        // else 0 (weak). Kept as `(sum, ticks)` rationals so the rate comparison stays integer.
        let (o_sum, o_ticks) = if out_ticks > 0 {
            (out_sum, out_ticks)
        } else {
            self.colony_reference_outside_rate(return_window, cultivating_at_pass_start)
                .unwrap_or((0, 1))
        };
        // cult_sum/cult_ticks ≥ (o_sum/o_ticks) · (10000 + margin)/10000, cross-multiplied in u128.
        let lhs = u128::from(cult_sum) * u128::from(o_ticks) * 10_000;
        let rhs = u128::from(o_sum) * u128::from(cult_ticks) * (10_000 + u128::from(margin_bps));
        lhs >= rhs
    }

    /// S22f: the voluntary-commitment **entry** signal for an eligible uncommitted agent `id` — a
    /// REUSABLE predicate over the SAME rolling cultivation-sale / outside-option data S22c maintains
    /// (the per-colonist `cultivation_return_window` + the colony reference outside rate) and the
    /// SAME floor logic, but read with [`Self::commitment_entry_floor`] instead of the retention
    /// material floor and phrased for an opt-in DECISION rather than the exit. It does NOT call
    /// [`Self::profit_stay_active`] (which is phrased for an already-cultivating agent around the
    /// `cultivate_now` branch) — that avoids phase/order ambiguity (Codex P2 #5). Returns
    /// [`CommitmentEntrySignal::Clears`] with the windowed cultivation proceeds (the recorded signal
    /// value) iff the proceeds clear the entry floor over ≥1 sale tick AND the cultivation rate ≥
    /// outside rate + margin; [`CommitmentEntrySignal::BelowFloor`] when the proceeds are below the
    /// floor (the discriminating below-floor case); else [`CommitmentEntrySignal::AboveFloorLoses`].
    /// Read-only; the money/active gate is the caller's. All comparisons are exact integer
    /// cross-multiplications (no float, no RNG).
    fn commitment_entry_signal_clears(
        &self,
        id: AgentId,
        cultivating_at_pass_start: &BTreeSet<AgentId>,
    ) -> CommitmentEntrySignal {
        let Some(&slot) = self.colonist_slot_by_id.get(&id) else {
            return CommitmentEntrySignal::BelowFloor;
        };
        let (return_window, margin_bps, _material_floor) = self.retention_params();
        let entry_floor = self.commitment_entry_floor();
        let tick = self.econ_tick;
        let (cult_sum, cult_ticks, out_sum, out_ticks) = window_return_sums(
            &self.colonists[slot].cultivation_return_window,
            tick,
            return_window,
        );
        // Entry floor: a real, recurring realized cultivation gain (not one dust sale; not vacuous).
        // Below it ⇒ the discriminating below-floor non-committer.
        if cult_ticks == 0 || cult_sum < entry_floor {
            return CommitmentEntrySignal::BelowFloor;
        }
        // Outside option: the agent's OWN realized non-cultivation rate, else the colony reference,
        // else 0 (weak). Kept as `(sum, ticks)` rationals so the rate comparison stays integer.
        let (o_sum, o_ticks) = if out_ticks > 0 {
            (out_sum, out_ticks)
        } else {
            self.colony_reference_outside_rate(return_window, cultivating_at_pass_start)
                .unwrap_or((0, 1))
        };
        let lhs = u128::from(cult_sum) * u128::from(o_ticks) * 10_000;
        let rhs = u128::from(o_sum) * u128::from(cult_ticks) * (10_000 + u128::from(margin_bps));
        if lhs >= rhs {
            CommitmentEntrySignal::Clears(cult_sum)
        } else {
            CommitmentEntrySignal::AboveFloorLoses
        }
    }

    fn stage_or_apply_commitment_norm_bit(&mut self, slot: usize, bit: bool) -> bool {
        if !self
            .colonists
            .get(slot)
            .is_some_and(|colonist| colonist.alive)
        {
            return false;
        }
        if self.colonists[slot].commitment_remaining > 0 {
            let current = self.colonists[slot].adopts_commitment_norm;
            if bit == current {
                self.colonists[slot].next_norm_bit = None;
                return false;
            }
            if self.colonists[slot].next_norm_bit == Some(bit) {
                return false;
            }
            self.colonists[slot].next_norm_bit = Some(bit);
            true
        } else {
            self.colonists[slot].next_norm_bit = None;
            self.apply_commitment_norm_bit(slot, bit)
        }
    }

    fn apply_staged_commitment_norm_bit_if_unbound(&mut self, slot: usize) {
        if !self.abandonable_norm_active() || self.colonists[slot].commitment_remaining > 0 {
            return;
        }
        if let Some(bit) = self.colonists[slot].next_norm_bit.take() {
            self.apply_commitment_norm_bit(slot, bit);
        }
    }

    fn apply_commitment_norm_bit(&mut self, slot: usize, bit: bool) -> bool {
        let from = self.colonists[slot].adopts_commitment_norm;
        if from == bit {
            return false;
        }
        let id = self.colonists[slot].id;
        self.colonists[slot].adopts_commitment_norm = bit;
        self.commitment_norm_flip_events
            .push(CommitmentNormFlipRow {
                tick: self.econ_tick,
                agent: id.0,
                from,
                to: bit,
            });
        if bit {
            self.commitment_norm_adoptions = self.commitment_norm_adoptions.saturating_add(1);
            self.commitment_norm_imitation_adopters.insert(id);
        } else {
            self.commitment_norm_abandonments = self.commitment_norm_abandonments.saturating_add(1);
        }
        true
    }

    fn record_commitment_norm_group_covariance(&mut self, groups: &[CommitmentNormGroupCandidate]) {
        if groups.len() < 2 {
            return;
        }
        let n = groups.len() as i128;
        let sum_score: i128 = groups
            .iter()
            .map(|group| i128::from(group.score.total_bps))
            .sum();
        let sum_share: i128 = groups
            .iter()
            .map(|group| i128::from(group.adopter_share_bps))
            .sum();
        let sum_product: i128 = groups
            .iter()
            .map(|group| i128::from(group.score.total_bps) * i128::from(group.adopter_share_bps))
            .sum();
        let covariance = (sum_product * n - sum_score * sum_share) / (n * n);
        self.commitment_norm_group_covariance_sum = self
            .commitment_norm_group_covariance_sum
            .saturating_add(covariance);
        self.commitment_norm_group_covariance_count = self
            .commitment_norm_group_covariance_count
            .saturating_add(1);
    }

    fn live_committed_count(&self) -> usize {
        self.live_colonist_slots
            .iter()
            .filter(|&&slot| {
                self.colonists
                    .get(slot)
                    .is_some_and(|colonist| colonist.alive && colonist.commitment_remaining > 0)
            })
            .count()
    }

    fn record_commitment_norm_observations(&mut self) {
        if !self.commitment_norm_spread_active() {
            return;
        }
        let window = self
            .chain
            .as_ref()
            .map_or(0, |chain| chain.imitation_window);
        let Ok(window_len) = usize::try_from(window) else {
            return;
        };
        if window_len == 0 {
            return;
        }
        let mut food_goods = BTreeSet::new();
        food_goods.insert(self.known.hunger);
        if let Some(subsistence) = self.known.subsistence {
            food_goods.insert(subsistence);
        }
        let mut consumed_by_agent: BTreeMap<AgentId, u32> = BTreeMap::new();
        for &(agent, good, qty) in self.society.consumption_log_last_tick() {
            if food_goods.contains(&good) {
                let entry = consumed_by_agent.entry(agent).or_insert(0);
                *entry = entry.saturating_add(qty);
            }
        }
        let exchange_pos = self
            .world
            .stockpile(self.exchange)
            .map(|stockpile| stockpile.pos);
        let tick = self.econ_tick;
        for colonist in &mut self.colonists {
            let salt_stock = self
                .society
                .agents
                .get(colonist.id)
                .map(|agent| u64::from(agent.stock.get(SALT)))
                .unwrap_or(0)
                .saturating_add(self.society.free_gold_after_all_reserves(colonist.id).0);
            let at_market =
                exchange_pos.is_some_and(|pos| self.world.agent_pos(colonist.id) == Some(pos));
            colonist
                .commitment_norm_observations
                .push_back(CommitmentNormObservation {
                    tick,
                    hunger: colonist.need.hunger,
                    food_consumed: consumed_by_agent.get(&colonist.id).copied().unwrap_or(0),
                    salt_stock,
                    at_market,
                });
            while colonist.commitment_norm_observations.len() > window_len {
                colonist.commitment_norm_observations.pop_front();
            }
        }
    }

    /// S18: whether the **money-from-a-multi-good-economy** path is active this tick — the
    /// `multigood_money` flag is on AND the S16 money-from-produced-bread path is active.
    /// When this holds, the woodcutter→WOOD-node routing has fired at generation and the
    /// runtime-only multi-good instrumentation (the WOOD source bound + the
    /// pending-indirect-SALT round-trip ledger) is maintained. Off (every existing config),
    /// all of it is inert and the run is byte-identical.
    fn multigood_money_active(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(chain_runtime_multigood_money_active)
    }

    /// S16: whether the produced-bread provenance ledger is maintained this tick — exactly
    /// the money-from-produced-bread path. Off (every existing config), the ledger stays the
    /// empty default, so no hook fires and the run is byte-identical.
    fn bread_provenance_active(&self) -> bool {
        self.cultivation_sells_surplus_active()
    }

    /// S21d.0: whether the food mints are retired this tick — the open-survival probe gate.
    /// When this holds, the demographic `food_provision` hearth and the producer staple
    /// floor skip minting the hunger staple (WOOD/warmth provision is unaffected), so the
    /// food-mint endowment term is zero and every agent must buy or produce its food.
    /// Gated purely on the chain flag (independent of `own_labor_subsistence`/forage). Off
    /// (every existing config), the mints fire as before and the run is byte-identical.
    fn retire_food_mints(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(|chain| chain.retire_food_mints)
    }

    /// S21d.1: whether the acquisition-channel ledger is maintained this tick — the chain
    /// flag is on AND the chain carries a bread good to track. Off (every existing config),
    /// the ledger stays the empty default, no hook fires, and the run is byte-identical
    /// (the ledger is never digested regardless).
    ///
    /// DH.b (impl-69): the ledger ALSO runs under `closure_active()` — the birth-funding
    /// telemetry needs the lot lifecycle on the exact closed base without touching the cell
    /// config (§5.6b pins the (q=4, Off) cell byte-identical to
    /// `frontier_closed_circulation()`, so the chain flag cannot be set there). Gating through
    /// `closure_active()` keeps DH.a's force-disable control governing it, and the ledger is
    /// pure observation, so the DH.a inertness comparison still holds.
    fn acquisition_ledger_active(&self) -> bool {
        (self
            .chain
            .as_ref()
            .is_some_and(|chain| chain.acquisition_ledger)
            || self.closure_active())
            && self.provenance_bread_good().is_some()
    }

    /// S16: the bread good, when a chain carries one (the good the ledger tracks).
    fn provenance_bread_good(&self) -> Option<GoodId> {
        self.content().map(ContentSet::bread)
    }

    /// S22c: fold this tick's realized cultivation-sale proceeds (the scratch
    /// [`Self::run_bread_provenance_market`] filled) and this tick's NON-cultivation (outside)
    /// spot-sale proceeds into each live colonist's rolling [`Colonist::cultivation_return_window`],
    /// pruning entries older than [`RETURN_WINDOW`]. Runs right after the provenance market pass (so
    /// the cultivation scratch is filled) and reads the post-promotion spot trades for the
    /// outside-option side. Only ticks on which a colonist realized SOME sale store a row, so a
    /// continuous cultivator with no outside ticks falls back to the colony reference in
    /// [`Self::profit_stay_active`]. Deterministic (slot order, integer proceeds, no RNG). A no-op
    /// off the profit-driven-retention path, so every other run is byte-identical.
    fn update_cultivation_returns(&mut self, spot_trades_start: usize) {
        if !self.profit_driven_retention_active() {
            return;
        }
        let (return_window, _margin, _floor) = self.retention_params();
        // S22c: a zero-length window can never feed `profit_stay_active` (the read-time filter in
        // `window_return_sums` returns zeros), so storing a row would only leave an inert last-sale
        // tail in the digested window. Skip storing entirely so a zero-window run keeps an empty,
        // clean window (the digest never depends on an inert row, and the window never stays full).
        if return_window == 0 {
            return;
        }
        let Some(bread) = self.provenance_bread_good() else {
            return;
        };
        let tick = self.econ_tick;
        // The outside option: realized SALT proceeds from each seller's NON-cultivation (non-bread)
        // spot sales this tick — only once SALT is money (pre-money there are no spot SALT sales).
        let mut outside: BTreeMap<AgentId, u64> = BTreeMap::new();
        if self.society.current_money_good() == Some(SALT) {
            for trade in &self.society.trades[spot_trades_start..] {
                if trade.good != bread {
                    *outside.entry(trade.seller).or_insert(0) +=
                        trade.price.0.saturating_mul(u64::from(trade.qty));
                }
            }
        }
        // Keep entries within the last `return_window` ticks (drop tick < tick - return_window + 1).
        let cutoff = tick.saturating_sub(return_window.saturating_sub(1));
        let live = self.live_colonist_slots.clone();
        for slot in live {
            let id = self.colonists[slot].id;
            let cult = self
                .cultivation_proceeds_scratch
                .get(&id)
                .copied()
                .unwrap_or(0);
            let out = outside.get(&id).copied().unwrap_or(0);
            let window = &mut self.colonists[slot].cultivation_return_window;
            while window.front().is_some_and(|e| e.tick < cutoff) {
                window.pop_front();
            }
            if cult > 0 || out > 0 {
                window.push_back(ReturnTick {
                    tick,
                    cultivation_proceeds: cult,
                    outside_proceeds: out,
                });
            }
        }
    }

    /// S16: end-of-tick provenance bookkeeping — record the first produced-surplus tick and
    /// assert the ledger conserves. A no-op off the path.
    fn finalize_bread_provenance(&mut self) {
        if !self.bread_provenance_active() {
            return;
        }
        if self.bread_provenance.first_produced_surplus_tick.is_none()
            && self.bread_provenance.total_held() > 0
        {
            self.bread_provenance.first_produced_surplus_tick = Some(self.econ_tick);
        }
        debug_assert!(
            self.bread_provenance_conserves(),
            "bread provenance ledger broke at econ tick {}",
            self.econ_tick
        );
    }

    /// S21d.1: the tracked food good for the acquisition-channel ledger — the chain's bread
    /// (the hunger staple in the open-survival probe). `None` for a settlement with no chain.
    fn acquisition_food_good(&self) -> Option<GoodId> {
        self.provenance_bread_good()
    }

    fn seeded_surplus_enabled(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(|chain| chain.seeded_surplus_bread > 0)
    }

    fn seller_can_offer_bread_for_salt_indirect_wood(&self, agent: &Agent, bread: GoodId) -> bool {
        let Some(barter) = self.barter.as_ref() else {
            return false;
        };
        agent.would_accept_indirect_barter_swap_with_stock(
            &agent.stock,
            bread,
            SALT,
            WOOD,
            1,
            MarketabilityAcceptance {
                durability_aware_acceptance: barter.menger.durability_aware_acceptance,
                config: &barter.menger.marketability,
            },
        )
    }

    fn seeded_surplus_agent_class(&self, agent: AgentId) -> bool {
        let Some(vocation) = self.vocation_of_id(agent) else {
            return false;
        };
        seeded_surplus_seller_class(vocation, self.colonist_household(agent))
    }

    /// S21e.1: count pinned seeded sellers whose SeededMinted bread is still an
    /// actual sellable unit under the same indirect-barter preservation rule the
    /// offer generator uses. This is intentionally not a provisioning approximation.
    fn seeded_offerable_wood_seller_count(&self) -> usize {
        if !self.seeded_surplus_enabled() || !self.acquisition_ledger_active() {
            return 0;
        }
        let Some(bread) = self.acquisition_food_good() else {
            return 0;
        };
        self.live_colonist_slots
            .iter()
            .filter(|&&slot| {
                let colonist = &self.colonists[slot];
                if !seeded_surplus_seller_class(colonist.vocation, colonist.household) {
                    return false;
                }
                if self
                    .acquisition
                    .held_by_agent_channel(colonist.id, FoodChannel::SeededMinted)
                    == 0
                {
                    return false;
                }
                self.society.agents.get(colonist.id).is_some_and(|agent| {
                    agent.stock.get(bread) > 0
                        && self.seller_can_offer_bread_for_salt_indirect_wood(agent, bread)
                })
            })
            .count()
    }

    /// S21e.1: total seeded-origin bread that is currently **offerable** — held above
    /// the holder's protected hunger (bread Now/Next) allocation, so giving a unit
    /// passes the real barter preservation rule (`barter_swap_acceptable` /
    /// `preserved_near_allocations_above_target`).
    ///
    /// This is the spec's exhaustion quantity, and it is deliberately
    /// **target-independent**: "offerable surplus" is a property of the *bread*
    /// (removable above the hunger floor), not of whether the holder happens to want
    /// WOOD this tick. The WOOD-want coupling belongs to the seller-count gate
    /// ([`Self::seeded_offerable_wood_seller_count`]); reusing it here would latch
    /// exhaustion on the first transient tick every seller is momentarily
    /// WOOD-satisfied. The quantity itself can move with the holder's hunger floor as
    /// well as with seed drains, so the latch below means "the first tick the seeded
    /// lots no longer provide a removable market scaffold under the actual barter
    /// preservation rule." Counting every seeded-bread holder — not just the pinned
    /// sellers — makes "no holder has offerable seeded surplus" the honest exhaustion
    /// test.
    fn seeded_offerable_surplus_units(&self) -> u64 {
        if !self.seeded_surplus_enabled() || !self.acquisition_ledger_active() {
            return 0;
        }
        let Some(bread) = self.acquisition_food_good() else {
            return 0;
        };
        self.live_colonist_slots
            .iter()
            .map(|&slot| {
                let id = self.colonists[slot].id;
                let seeded = self
                    .acquisition
                    .held_by_agent_channel(id, FoodChannel::SeededMinted);
                if seeded == 0 {
                    return 0;
                }
                let Some(agent) = self.society.agents.get(id) else {
                    return 0;
                };
                let held = u64::from(agent.stock.get(bread));
                let reserved = u64::from(agent.stock_reserved_for_near_wants_barter(bread));
                // Offerable bread (above the hunger floor) that can be drawn from a
                // seeded lot: the seeded-origin share of the removable surplus.
                held.saturating_sub(reserved).min(seeded)
            })
            .sum()
    }

    fn current_tick_acquisition_after_market_consumption(
        &self,
        bread: GoodId,
    ) -> AcquisitionLedger {
        let mut acquisition = self.acquisition.clone();
        for &(agent, good, qty) in self.society.consumption_log_last_tick() {
            if good == bread {
                acquisition.consume(agent, u64::from(qty));
            }
        }
        acquisition
    }

    fn bread_trade_parties(
        trade: &BarterTrade,
        bread: GoodId,
    ) -> Option<(AgentId, AgentId, GoodId, u32, BarterReason)> {
        if trade.a_gives == bread {
            Some((trade.a, trade.b, trade.b_gives, trade.qty, trade.a_reason))
        } else if trade.b_gives == bread {
            Some((trade.b, trade.a, trade.a_gives, trade.qty, trade.b_reason))
        } else {
            None
        }
    }

    fn bread_salt_indirect_for_wood_live_from_seeded_seller(
        &self,
        bread: GoodId,
        acquisition: &AcquisitionLedger,
    ) -> bool {
        self.society.live_barter_offers().iter().any(|offer| {
            offer.give_good == bread
                && offer.receive_good == SALT
                && matches!(offer.reason, BarterReason::IndirectFor { target } if target == WOOD)
                && self.seeded_surplus_agent_class(offer.agent)
                && acquisition.held_by_agent_channel(offer.agent, FoodChannel::SeededMinted) > 0
                && self.society.agents.get(offer.agent).is_some_and(|agent| {
                    self.seller_can_offer_bread_for_salt_indirect_wood(agent, bread)
                })
        })
    }

    fn seeded_bread_salt_indirect_for_wood_cleared_since(
        &self,
        bread: GoodId,
        barter_trades_start: usize,
        acquisition: &mut AcquisitionLedger,
    ) -> u64 {
        let mut cleared = 0u64;
        for trade in &self.society.barter_trades[barter_trades_start..] {
            let Some((seller, buyer, received_good, qty, reason)) =
                Self::bread_trade_parties(trade, bread)
            else {
                continue;
            };
            let drawn = acquisition.transfer_as_bought(seller, buyer, u64::from(qty));
            if received_good == SALT
                && matches!(reason, BarterReason::IndirectFor { target } if target == WOOD)
                && self.seeded_surplus_agent_class(seller)
            {
                cleared = cleared.saturating_add(drawn[FoodChannel::SeededMinted.index()]);
            }
        }
        cleared
    }

    fn observe_seeded_surplus_probe_tick(
        &mut self,
        barter_trades_start: usize,
        pre_market_seeded_sellers: usize,
        was_pre_promotion: bool,
    ) {
        if !was_pre_promotion || !self.seeded_surplus_enabled() {
            return;
        }
        let Some(bread) = self.acquisition_food_good() else {
            return;
        };
        self.seeded_surplus_trace.max_pre_promotion_seeded_sellers = self
            .seeded_surplus_trace
            .max_pre_promotion_seeded_sellers
            .max(pre_market_seeded_sellers);
        let mut acquisition = self.current_tick_acquisition_after_market_consumption(bread);
        let cleared = self.seeded_bread_salt_indirect_for_wood_cleared_since(
            bread,
            barter_trades_start,
            &mut acquisition,
        );
        self.seeded_surplus_trace
            .cleared_bread_salt_indirect_for_wood = self
            .seeded_surplus_trace
            .cleared_bread_salt_indirect_for_wood
            .saturating_add(cleared);
        let live = self.bread_salt_indirect_for_wood_live_from_seeded_seller(bread, &acquisition);
        if live {
            self.seeded_surplus_trace
                .live_bread_salt_indirect_for_wood_ticks = self
                .seeded_surplus_trace
                .live_bread_salt_indirect_for_wood_ticks
                .saturating_add(1);
        }
        if (cleared > 0 || live) && self.seeded_surplus_trace.first_non_vacuous_tick.is_none() {
            self.seeded_surplus_trace.first_non_vacuous_tick = Some(self.econ_tick);
        }
    }

    fn record_bread_seller_provenance(&mut self, barter_trades_start: usize) {
        if !self.acquisition_ledger_active() {
            return;
        }
        let Some(bread) = self.acquisition_food_good() else {
            return;
        };
        let mut rows = Vec::new();
        for trade in &self.society.barter_trades[barter_trades_start..] {
            let row = if trade.a_gives == bread {
                Some((
                    trade.tick,
                    trade.a,
                    trade.b,
                    trade.b_gives,
                    trade.qty,
                    trade.a_reason,
                ))
            } else if trade.b_gives == bread {
                Some((
                    trade.tick,
                    trade.b,
                    trade.a,
                    trade.a_gives,
                    trade.qty,
                    trade.b_reason,
                ))
            } else {
                None
            };
            let Some((tick, seller, buyer, received_good, qty, reason)) = row else {
                continue;
            };
            rows.push(BreadSellerProvenance {
                tick,
                seller,
                buyer,
                seller_vocation: self.vocation_of_id(seller),
                buyer_vocation: self.vocation_of_id(buyer),
                seller_household: self.colonist_household(seller),
                buyer_household: self.colonist_household(buyer),
                bread_good: bread,
                received_good,
                qty,
                reason,
            });
        }
        self.bread_seller_trace.extend(rows);
    }

    fn update_seeded_offerable_surplus_exhaustion(&mut self) {
        if !self.seeded_surplus_enabled()
            || !self.acquisition_ledger_active()
            || self
                .seeded_surplus_trace
                .seeded_offerable_surplus_exhausted_tick
                .is_some()
        {
            return;
        }
        // Honest finite-seed exhaustion: the first tick at which no holder has
        // seeded-origin bread above its protected hunger allocation (offerable surplus
        // gone, only the eat-only floor left). Target-independent and computed through
        // the barter-preservation helper, so it does not treat a temporary lack of
        // WOOD demand as exhaustion.
        if self.seeded_offerable_surplus_units() == 0 {
            self.seeded_surplus_trace
                .seeded_offerable_surplus_exhausted_tick = Some(self.econ_tick);
        }
    }

    /// S21d.2a: the active chain producers (Miller/Baker and the 3-good cycle roles) — the agents
    /// `set_project_input_bid_overrides` adjudicates and whose buy → eat → bid sequence the
    /// bootstrap microtrace follows.
    fn active_producer_ids(&self) -> BTreeSet<AgentId> {
        self.live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                let colonist = &self.colonists[slot];
                matches!(
                    colonist.vocation,
                    Vocation::Miller
                        | Vocation::Baker
                        | Vocation::CycleA
                        | Vocation::CycleB
                        | Vocation::CycleC
                )
                .then_some(colonist.id)
            })
            .collect()
    }

    /// S21d.1: total tracked-food held across all society agents — the conservation right-hand
    /// side the ledger's `total_held()` must match each active tick.
    fn acquisition_food_held(&self, food: GoodId) -> u64 {
        self.society
            .agents
            .iter()
            .map(|agent| u64::from(agent.stock.get(food)))
            .sum()
    }

    /// S21d.1: one-time bootstrap — sweep the generated SEED bread into the `SeededMinted`
    /// channel before any death/provision/market moves it, so the ledger starts in lockstep
    /// with held stock. Idempotent (latched) and a no-op off the gated path.
    fn maybe_init_acquisition_ledger(&mut self) {
        if !self.acquisition_ledger_active() || self.acquisition.initialized {
            return;
        }
        let Some(food) = self.acquisition_food_good() else {
            return;
        };
        // DH.b (impl-69): latch the burden provenance extension (purchase identity +
        // construction taint) with the same predicate the DH.a force-disable control flips,
        // BEFORE the seed sweep below so the swept `SeededMinted` lots carry construction taint.
        self.acquisition.burden_provenance = self.closure_active();
        let endow = self
            .chain
            .as_ref()
            .map_or(0, |chain| chain.producer_house_starting_staple);
        let seed: Vec<(AgentId, u64)> = self
            .society
            .agents
            .iter()
            .map(|agent| (agent.id, u64::from(agent.stock.get(food))))
            .filter(|&(_, qty)| qty > 0)
            .collect();
        for (id, qty) in seed {
            // C3R.e (impl-67): the A2 split — for a producer subject, credit exactly the additive
            // endowment (`producer_house_starting_staple`, clamped to its holding) as
            // INTERVENTION-origin lots at the FRONT (oldest, so the head start is spent first and
            // exhaustion is honestly measured), and the remainder as plain `SeededMinted`. Every
            // non-producer's whole holding — and every non-A2 config (`endow == 0`) — stays plain
            // `SeededMinted`, so the sweep is byte-behaviour-identical off A2.
            let flagged = if endow > 0 && self.is_producer_subject_id(id) {
                u64::from(endow).min(qty)
            } else {
                0
            };
            if flagged > 0 {
                self.acquisition
                    .credit_intervention(id, FoodChannel::SeededMinted, flagged);
            }
            let plain = qty - flagged;
            if plain > 0 {
                self.acquisition
                    .credit(id, FoodChannel::SeededMinted, plain);
            }
        }
        self.acquisition.initialized = true;
    }

    /// S21d.1: end-of-tick conservation assert — `total_held()` must equal the tracked food
    /// actually held across all society agents, proving every inflow credited and every outflow
    /// debited the channel ledger. A no-op off the path.
    fn finalize_acquisition_ledger(&mut self) {
        if !self.acquisition_ledger_active() {
            return;
        }
        let Some(food) = self.acquisition_food_good() else {
            return;
        };
        debug_assert_eq!(
            self.acquisition.total_held(),
            self.acquisition_food_held(food),
            "acquisition-channel ledger broke conservation at econ tick {}",
            self.econ_tick
        );
    }

    /// S16: the provenance conservation receipt. (1) Global: produced bread credited equals
    /// produced bread sunk plus produced bread still held — production and sinks are the only
    /// terms; transfers move produced units between agents without changing the total.
    /// (2) Per living agent: produced-origin bread held never exceeds the agent's bread
    /// stock (so the residual other-origin pool is non-negative).
    fn bread_provenance_conserves(&self) -> bool {
        let bp = &self.bread_provenance;
        if bp.produced_credited != bp.produced_sunk + bp.total_held() {
            return false;
        }
        // S22a: the class-tagged produced lots mirror the flat `produced` balance exactly (the
        // entrant-class split is unsound otherwise) — same agents, same per-agent totals.
        let lot_total = |id: AgentId| -> u64 {
            bp.produced_lots
                .get(&id)
                .map_or(0, |q| q.iter().map(|lot| lot.qty).sum())
        };
        if bp.produced_lots.len() != bp.produced.len()
            || !bp
                .produced
                .iter()
                .all(|(&id, &produced)| lot_total(id) == produced)
        {
            return false;
        }
        let Some(bread) = self.provenance_bread_good() else {
            return bp.produced.is_empty();
        };
        bp.produced.iter().all(|(&id, &produced)| {
            let held = self
                .society
                .agents
                .get(id)
                .map_or(0, |agent| u64::from(agent.stock.get(bread)));
            produced <= held
        })
    }

    fn cultivation_input_good(&self) -> Option<GoodId> {
        self.chain
            .as_ref()?
            .content
            .cultivate_recipe()?
            .input_good
            .map(|(good, _)| good)
    }

    /// S15: whether a cultivator's grain haul is still IN FLIGHT, so the steering latch
    /// holds `cultivating` until the grain reaches econ stock — the colonist is CARRYING
    /// the input good (the return-with-grain leg) or it has a PENDING DEPOSIT of it (the
    /// just-landed leg). The empty-carry walk-out leg is deliberately NOT a latch leg:
    /// under scarcity (the only regime that escalates to cultivation) hunger climbs back
    /// over `cultivate_hunger_in`, so a colonist that cleared the flag mid-walk
    /// re-escalates and deposits its harvested grain on the next cultivation spell — the
    /// carry is held (conserved, attributed by the [`Self::run_fast_loop`] carry-snapshot),
    /// never lost. Latching the walk leg too would instead hold the colonist cultivating
    /// CONTINUOUSLY, which shifts the carrying-capacity equilibrium.
    fn cultivation_input_in_flight(&self, id: AgentId, input: GoodId) -> bool {
        self.world.agent_carry(id, input) > 0
            || self
                .pending_deposits
                .get(&(id, input))
                .copied()
                .unwrap_or(0)
                > 0
    }

    fn cultivation_input_in_stock(&self, id: AgentId, input: GoodId) -> bool {
        self.society
            .agents
            .get(id)
            .is_some_and(|agent| agent.stock.get(input) > 0)
    }

    /// S14: the good a **birth** endows — the parent-endowment gate, the parent debit,
    /// the newborn's initial buffer, and (at generation) the founder seed. On the
    /// forage-commons path it is the FORAGE subsistence good (the colony's *actual*
    /// food on this path), so births stall on **FORAGE** scarcity (the preventive
    /// check) rather than a bread shortage; off the path it is the hunger staple
    /// (`known.hunger`) exactly as before, so every existing config is byte-identical.
    /// This selects the endowment good ONLY — `known.hunger` is left untouched, so
    /// consumption / the chain / sales still thread the staple unchanged.
    fn birth_food(&self) -> GoodId {
        birth_food_good(self.forage_commons_active(), &self.known)
    }

    /// S15: the child-food goods a birth may endow from, in PREFERENCE order. Off the
    /// cultivation path it is exactly `[birth_food()]` — the single S14 selector (FORAGE
    /// on the commons path, the hunger staple otherwise) — so every existing run is
    /// byte-identical. On the cultivation path it BROADENS to any edible food the parent
    /// holds — bread (`known.hunger`) first, then forage (`known.subsistence`) — so a
    /// cultivator's own bread can endow children. Without this a fed-by-cultivation
    /// colony would still stall births on a FORAGE shortage (the cultivators gather grain,
    /// not forage), and the intensified plateau could not rise (Base Fact 7).
    fn birth_food_options<'a>(&self, buf: &'a mut [GoodId; 2]) -> &'a [GoodId] {
        if self.own_use_cultivation_active() {
            buf[0] = self.known.hunger;
            let mut len = 1;
            if let Some(sub) = self.known.subsistence {
                if sub != self.known.hunger {
                    buf[1] = sub;
                    len = 2;
                }
            }
            &buf[..len]
        } else {
            // The single S14 selector — kept a stack write (no allocation) on the shared
            // off-cultivation `run_births` hot path.
            buf[0] = self.birth_food();
            &buf[..1]
        }
    }

    /// S15: the first child-food good (in [`Self::birth_food_options`] preference order)
    /// that `parent` holds at least `need` free units of, or `None`. Generalises the S14
    /// single-good endowment gate; with one option it is exactly the S14 check, so the
    /// off-cultivation path is byte-identical.
    fn parent_birth_food(&self, parent: AgentId, foods: &[GoodId], need: u32) -> Option<GoodId> {
        foods.iter().copied().find(|&good| {
            self.society.agents.get(parent).is_some()
                && self.society.free_stock_after_all_reserves(parent, good) >= need
        })
    }

    /// S13: whether spatial households are active — every lineage member (founders +
    /// newborns) is given a world agent at its exact econ id and is eligible to be
    /// assigned forage/gather/haul tasks. Gated on the demography overlay's flag, so
    /// every pre-S13 config (the flag default-off) keeps econ-only lineages and a
    /// byte-identical stream.
    fn spatial_households_active(&self) -> bool {
        self.demography
            .as_ref()
            .is_some_and(|demo| demo.spatial_households)
    }

    /// S12/S14: the FORAGE node (the `GoForage`/`GoHarvest` target / forage location),
    /// or `None` when own-labor subsistence is off. Returns the dedicated node captured
    /// at generation (`forage_node_id`), NOT a resolve-by-good lookup: the commons node
    /// is created outside `config.nodes`, so a by-good search would pick an earlier
    /// config FORAGE node (if any) and deplete the wrong stock. For every existing
    /// own-labor scenario (no config FORAGE node) this is the same node the by-good
    /// lookup returned, so the stream is byte-identical.
    fn forage_node(&self) -> Option<NodeId> {
        self.forage_node_id
    }

    /// S7.2: whether `good` has cleared a real trade within the last `window` econ ticks
    /// — a recency guard so the build appraisal never trusts a STALE realized price (a
    /// price frozen because the good stopped clearing). Without it a mill would be built
    /// forever on a flour price frozen high after bakers stopped buying flour: the build
    /// must be backed by ACTUAL current demand for the tool's output, not a ghost price.
    /// Scans the trade tape backward only over the window, so it is bounded by recent
    /// trade volume, not the whole (growing) tape. Reads only existing canonical state
    /// (the trades), so it adds no digest surface.
    fn good_traded_within(&self, good: GoodId, window: u64) -> bool {
        let floor = self.society.tick.0.saturating_sub(window);
        for trade in self.society.trades.iter().rev() {
            if trade.tick < floor {
                break;
            }
            if trade.good == good {
                return true;
            }
        }
        false
    }

    /// The resource node whose harvested good is `good`, in node-id order (the
    /// settlement's nodes are added in config order, so this is stable). `None` if no
    /// node yields `good`. Read-only; used by the S6 re-entry phase to resolve the
    /// edible grain node and by the `grain_node` accessor.
    fn node_for_good(&self, good: GoodId) -> Option<NodeId> {
        (0..self.world.node_count())
            .map(|i| NodeId(i as u32))
            .find(|&id| self.world.node(id).map(|node| node.good) == Some(good))
    }

    fn slot_for_id(&self, id: AgentId) -> Option<usize> {
        self.colonist_slot_by_id.get(&id).copied()
    }

    fn mark_colonist_dead(&mut self, slot: usize) {
        if !self.colonists[slot].alive {
            return;
        }
        self.colonists[slot].alive = false;
        // S22f §3.5b: a commitment overrides the cultivation EXIT, never survival — drop a dead
        // committer's binding so no orphaned commitment lingers in the digest on a non-living agent.
        // `0` for every non-commitment colonist, so this is byte-identical off the path.
        self.colonists[slot].commitment_remaining = 0;
        self.colonists[slot].next_norm_bit = None;
        self.colonists[slot].carried_grain_source = None;
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
        self.world.total_goods_of(good)
            + self.econ_stock_total(good)
            + self.commons_stock_of(good)
            + self.subsistence_commons_stock_of(good)
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

    /// Count of LIVE colonists each holding **at least one** `good` — the usable
    /// capital CAPACITY of `good`, not a raw unit sum. A colonist has one vocation and
    /// one producer throughput, so extra units concentrated in a single holder (e.g. an
    /// inherited or transferred estate that stacks two ovens on one heir) add no
    /// capacity; counting holders, not units, keeps the bottleneck and idle-tool guards
    /// honest under such concentration. Unlike [`Self::whole_system_total`], this
    /// intentionally excludes the world, resident traders, and the commons; S7
    /// capital-capacity decisions need wieldable tools, not every conserved unit.
    fn live_colonist_holder_count(&self, good: GoodId) -> u64 {
        self.live_colonist_slots
            .iter()
            .filter_map(|&slot| self.society.agents.get(self.colonists[slot].id))
            .filter(|agent| agent.stock.get(good) > 0)
            .count() as u64
    }

    fn mortal_chain_producer_subject(&self, slot: usize) -> bool {
        self.colonists.get(slot).is_some_and(|colonist| {
            matches!(colonist.vocation, Vocation::Miller | Vocation::Baker)
                || matches!(colonist.latent, Some(RecipeId::Mill) | Some(RecipeId::Bake))
                || colonist.acquired_tool
        })
    }

    fn record_mortal_capital_build_completion(&mut self, slot: usize, qty: u32) {
        if self.mortal_chain_producers_active()
            && self
                .colonists
                .get(slot)
                .is_some_and(|colonist| colonist.lifespan.is_some())
        {
            self.mortal_capital_builds = self.mortal_capital_builds.saturating_add(u64::from(qty));
        }
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

    /// The directly-edible **subsistence** good (`known.subsistence`) — the FORAGE/raw-grain
    /// staple fallback, or `None` when none is wired. The S21d open-survival probe asserts this
    /// is `None` (the explicit `retire_food_mints` flag interns NO forage good, unlike the S12
    /// forage hack). Read-only.
    pub fn subsistence_good(&self) -> Option<GoodId> {
        self.known.subsistence
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

    /// The current single provisional saleability leader as it converges on a
    /// money good. `None` before any good leads, or for a non-emergent settlement.
    ///
    /// Note: under `two_layer_saleability` the barter book routes indirect offers
    /// through the full `provisional_media_candidates` set, which is gated only by
    /// the direct-use floor; this leader is the more strictly gated single winner
    /// (it also requires the medium-share / lead-margin discipline), so the two can
    /// diverge during bootstrap.
    pub fn saleability_leader(&self) -> Option<GoodId> {
        self.society.saleability_provisional_leader()
    }

    /// The realized **total** acceptance share (basis points) of `good` in the
    /// running saleability tally — direct and medium acceptances conflated. `None`
    /// for a non-emergent settlement. Read-only surfacing of the lab's tracker for
    /// the viewer. Under `two_layer_saleability`, prefer [`Self::medium_saleability_bps`]
    /// for the non-conflated medium share the leadership race actually reads.
    pub fn saleability_bps(&self, good: GoodId) -> Option<u16> {
        self.society
            .emergence()
            .and_then(|e| e.saleability_bps(good))
    }

    /// The realized **medium** (re-trade) saleability share (basis points) of
    /// `good` — `indirect_acceptances / total_indirect_acceptances`, the
    /// non-conflated metric two-layer leadership ranks on. `None` for a
    /// non-emergent settlement. Read-only surfacing of the lab's tracker.
    pub fn medium_saleability_bps(&self, good: GoodId) -> Option<u16> {
        self.society
            .emergence()
            .and_then(|e| e.medium_share_bps(good))
    }

    /// Total realized barter trades over the run so far (the emergent camp's
    /// goods-for-goods volume). Zero for a designated-money settlement.
    pub fn barter_trade_count(&self) -> usize {
        self.society.barter_trades.len()
    }

    /// S15: the total units of `good` that have changed hands across the whole run — the
    /// realized market trades plus the spatial barter trades. The acceptance suite reads
    /// it to prove cultivated bread is **own-use** (never bartered/sold): on the
    /// cultivation path the bread is produced and eaten after the market clears, so its
    /// trade volume stays zero. Read-only over the conserved trade tapes.
    pub fn trade_volume_of(&self, good: GoodId) -> u64 {
        let market: u64 = self
            .society
            .trades
            .iter()
            .filter(|trade| trade.good == good)
            .map(|trade| u64::from(trade.qty))
            .sum();
        let barter: u64 = self
            .society
            .barter_trades
            .iter()
            .filter(|trade| trade.a_gives == good || trade.b_gives == good)
            .map(|trade| u64::from(trade.qty))
            .sum();
        market + barter
    }

    /// S18 (read-only): the DISTINCT agents who have OFFERED `good` as a barter surplus (gave
    /// it in a cleared barter trade) over the whole run. The role-separation probe: with clean
    /// role separation each agent offers only ONE surplus good, so the giver-sets of two
    /// surplus goods (bread, WOOD) are disjoint and the lowest-good-id preemption cannot fire.
    /// Reads only over the conserved barter tape.
    pub fn barter_givers_of(&self, good: GoodId) -> Vec<AgentId> {
        let mut givers: Vec<AgentId> = Vec::new();
        for trade in &self.society.barter_trades {
            if trade.a_gives == good && !givers.contains(&trade.a) {
                givers.push(trade.a);
            }
            if trade.b_gives == good && !givers.contains(&trade.b) {
                givers.push(trade.b);
            }
        }
        givers.sort();
        givers
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
        self.society
            .total_gold()
            .saturating_add(self.commons_gold)
            .saturating_add(self.land_fee_pool_salt)
            .saturating_add(self.wage_escrow_gold)
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

    // ---- G8c-1 credit-cycle surface --------------------------------------

    /// Whether this settlement runs the G8c-1 **credit-cycle** (finance) overlay (the
    /// Austrian business cycle or its sound-money control). `false` for every spatial
    /// settlement.
    pub fn is_cycle(&self) -> bool {
        self.cycle.is_some()
    }

    /// Which finance demonstration this settlement runs ([`CycleKind`]), or `None`
    /// for a spatial settlement.
    pub fn cycle_kind(&self) -> Option<CycleKind> {
        self.cycle.as_ref().map(|cycle| cycle.kind)
    }

    /// The current money **regime** — the rung of the ladder (`SoundGold →
    /// FractionalConvertible → SuspendedConvertibility → Fiat`), reused from econ.
    pub fn regime(&self) -> Regime {
        self.society.regime()
    }

    /// A stable lowercase label for the current regime rung (the viewer renders it).
    pub fn regime_label(&self) -> &'static str {
        match self.society.regime() {
            Regime::SoundGold => "sound-gold",
            Regime::FractionalConvertible => "fractional",
            Regime::SuspendedConvertibility => "suspended",
            Regime::Fiat => "fiat",
        }
    }

    /// The state issuer's cumulative **fiat issued** (`Gold::ZERO` without an issuer).
    pub fn fiat_issued(&self) -> Gold {
        self.issuer()
            .map_or(Gold::ZERO, |issuer| issuer.fiat_issued)
    }

    /// The state issuer's cumulative **fiat retired** (repaid credit principal).
    pub fn fiat_retired(&self) -> Gold {
        self.issuer()
            .map_or(Gold::ZERO, |issuer| issuer.fiat_retired)
    }

    /// The **fiat base** = issued − retired — the outstanding fiat the issuer has put
    /// into circulation (`Gold::ZERO` without an issuer). Conserved by rule: a default
    /// changes the money stock by retiring/booking, never by a leak (G8c-1 test 5).
    pub fn fiat_base(&self) -> Gold {
        self.fiat_issued().saturating_sub(self.fiat_retired())
    }

    /// The state issuer (id [`ISSUER_ID`]) the cycle routes through, or `None`.
    fn issuer(&self) -> Option<&econ::issuer::Issuer> {
        self.society
            .issuers
            .iter()
            .find(|issuer| issuer.id == ISSUER_ID)
    }

    /// The lifetime count of **boom** project starts (capitalists over-investing in
    /// the long roundabout project under cheap credit). Summed from the M3 records.
    pub fn boom_projects_started(&self) -> u32 {
        self.society.m3_records.iter().fold(0u32, |sum, record| {
            sum.saturating_add(record.boom_projects_started)
        })
    }

    /// The lifetime count of **bust** project abandonments (the malinvested projects
    /// abandoned when credit stops and the rate reasserts). Summed from the M3 records.
    pub fn bust_abandoned_projects(&self) -> u32 {
        self.society.m3_records.iter().fold(0u32, |sum, record| {
            sum.saturating_add(record.bust_abandoned_projects)
        })
    }

    /// The total **capital consumed** by the bust — the labor + non-salvaged input
    /// goods embodied in the abandoned projects (reusing econ's M2/M3 capital-
    /// consumption accounting). Read from the **latest** M3 record's counters, which
    /// are running cumulative lifetime totals (each abandonment `saturating_add`s into
    /// them), so the run-to-date consumption is the final record's value — NOT a sum
    /// across records (that would re-count the same lifetime total every tick). Mirrors
    /// the lab's `final_capital_*_consumed` sweep accounting. `0` if no project was
    /// abandoned (the sound-money control).
    pub fn capital_consumed(&self) -> u64 {
        self.society.m3_records.last().map_or(0u64, |record| {
            u64::from(record.m2.capital_labor_consumed)
                + u64::from(record.m2.capital_goods_consumed)
        })
    }

    /// The peak **structure length** (×100 ticks) the production structure reached
    /// over the run — the roundabout structure lengthening in the boom. `0` if no
    /// project ever formed.
    pub fn peak_structure_length_x100(&self) -> u64 {
        self.society
            .m3_records
            .iter()
            .map(|record| record.m2.structure_length_ticks_x100)
            .max()
            .unwrap_or(0)
    }

    /// Whether institutionally-created **credit** (bank fiduciary or issuer
    /// fiat-credit) has ever circulated over the run — the Credit-era signal.
    pub fn credit_ever_circulated(&self) -> bool {
        self.society.m3_records.iter().any(|record| {
            record.bank_credit_issued > Gold::ZERO || record.fiat_credit_issued > Gold::ZERO
        })
    }

    /// Whether **state fiat** has ever circulated as money over the run — `true` once
    /// any M3 record carried outstanding `public_fiat`. The Modern-era (marginal-medium)
    /// signal: a monotonic "ever" measure mirroring
    /// [`credit_ever_circulated`](Self::credit_ever_circulated), so the climax rung is
    /// earned once and never silently regresses when the bust defaults outstanding fiat
    /// back toward zero. (Contrast [`fiat_base`](Self::fiat_base), the *current*
    /// outstanding base the viewer banner surfaces.)
    pub fn fiat_ever_circulated(&self) -> bool {
        self.society
            .m3_records
            .iter()
            .any(|record| record.public_fiat > Gold::ZERO)
    }

    /// The number of realized **loan trades** (the time/credit market), summed over
    /// the run — a measure of reciprocal credit exchange.
    pub fn loan_trade_count(&self) -> usize {
        self.society.loan_trades.len()
    }

    /// The number of realized **spot trades** (the goods market) — reciprocal goods
    /// exchange.
    pub fn spot_trade_count(&self) -> usize {
        self.society.trades.len()
    }

    /// The credit-disabled **shadow** natural-rate series (bps per tick), replayed
    /// from the retained scenario. Read-only — clones the scenario and runs it
    /// credit-disabled, never perturbing the live run. `None` for a non-finance
    /// settlement.
    pub fn shadow_natural_rate_bps(&self) -> Option<Vec<Option<i64>>> {
        Some(self.shadow_cycle_metrics()?.natural_rate_bps)
    }

    /// The per-tick **shadow gap** = shadow natural rate − market rate (bps). The cycle
    /// opens a **positive** gap during the boom (cheap credit pulls the market rate
    /// below the credit-disabled natural rate); the sound-money control's gap stays
    /// ≈ 0. `None` entries are ticks with no measured rate on one side. `None` for a
    /// non-finance settlement. MEASURED, never set (lab doctrine).
    pub fn shadow_gap_bps(&self) -> Option<Vec<Option<i64>>> {
        Some(self.shadow_cycle_metrics()?.gap_bps)
    }

    /// The largest positive shadow gap over the run (`0` if the gap never opened) —
    /// the sign-only boom signal: `> 0` for the credit cycle, `0` for the control.
    pub fn max_shadow_gap_bps(&self) -> i64 {
        self.shadow_cycle_metrics()
            .map_or(0, |metrics| metrics.max_gap_bps)
    }

    /// Whether the boom's roundabout structure ever rose **above** the credit-disabled
    /// shadow baseline — the measured boom (over-investment in longer production).
    /// `false` for a non-finance settlement or the sound-money control.
    pub fn structure_rose_above_shadow(&self) -> bool {
        self.shadow_cycle_metrics()
            .is_some_and(|metrics| metrics.structure_rose_above_shadow)
    }

    /// Whether the credit cycle actually **fired** — a boom started *and* the bust
    /// abandoned a malinvested project. The headline outcome: `true` under fiat-legal
    /// wages (the credit transmits), `false` under specie-only wages (the same credit
    /// is inert). `false` for the sound-money control and every non-cycle settlement.
    pub fn cycle_fired(&self) -> bool {
        self.boom_projects_started() > 0 && self.bust_abandoned_projects() > 0
    }

    // ---- G8c-2 tender-policy surface -------------------------------------

    /// Whether this settlement runs the G8c-2 **tender bench** overlay (a finance
    /// settlement demonstrating one tender surface's refusal-vs-acceptance). `false`
    /// for the cycle and every spatial settlement.
    pub fn is_tender_bench(&self) -> bool {
        self.bench.is_some()
    }

    /// Which tender surface the bench demonstrates ([`BenchSurface`]), or `None` for a
    /// non-bench settlement.
    pub fn bench_surface(&self) -> Option<BenchSurface> {
        self.bench.as_ref().map(|bench| bench.surface)
    }

    /// The **active** public-spot tender — which media may settle the spot market
    /// (reused from econ's society state; the viewer surfaces it). Reads the live
    /// policy, so a config that set it, or a scenario event that changed it, both show.
    pub fn public_spot_tender(&self) -> PublicSpotTender {
        self.society.public_spot_tender
    }

    /// The **active** labor-wage tender — which media may pay wages. The cycle's
    /// transmission valve: `SpecieOnly` refuses the fiat the cycle would transmit.
    pub fn labor_wage_tender(&self) -> LaborWageTender {
        self.society.labor_wage_tender
    }

    /// The **active** public-debt tender — which media may discharge public debt.
    pub fn public_debt_tender(&self) -> PublicDebtTender {
        self.society.public_debt_tender
    }

    /// The **active** bank-repayment tender — which media may repay a bank loan.
    pub fn bank_repayment_tender(&self) -> BankRepaymentTender {
        self.society.bank_repayment_tender
    }

    /// The **active** issuer-repayment tender — which media the issuer accepts to
    /// repay fiat credit.
    pub fn issuer_repayment_tender(&self) -> IssuerRepaymentTender {
        self.society.issuer_repayment_tender
    }

    /// Total **fiat** that settled the **spot** market over the run (summed from
    /// econ's spot-payment audit). Positive when the spot tender accepts fiat and the
    /// buyers hold it; `Gold::ZERO` when the spot tender refuses fiat (specie settles
    /// instead). The spot surface's composition signal.
    pub fn spot_fiat_settled(&self) -> Gold {
        self.society
            .payment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| sum.saturating_add(row.public_fiat))
    }

    /// Total **specie** that settled the **spot** market over the run (summed from the
    /// spot-payment audit).
    pub fn spot_specie_settled(&self) -> Gold {
        self.society
            .payment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| sum.saturating_add(row.public_specie))
    }

    /// Total **fiat** that discharged **public debt** over the run (summed from econ's
    /// debt-payment audit). Positive when the debt tender accepts fiat and the debtor
    /// holds it; `Gold::ZERO` when the debt tender refuses fiat (specie settles).
    pub fn debt_fiat_settled(&self) -> Gold {
        self.society
            .debt_payment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| sum.saturating_add(row.public_fiat))
    }

    /// Total **specie** that discharged **public debt** over the run (summed from the
    /// debt-payment audit).
    pub fn debt_specie_settled(&self) -> Gold {
        self.society
            .debt_payment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| sum.saturating_add(row.public_specie))
    }

    /// Total **fiat** that repaid **bank loans** over the run (summed from econ's
    /// bank-repayment audit). Usually zero in the M15 claim-tender bench; exposed so
    /// the repayment surface has the same read-only composition signal as spot/debt.
    pub fn bank_repayment_fiat_settled(&self) -> Gold {
        self.society
            .bank_repayment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| sum.saturating_add(row.public_fiat))
    }

    /// Total **bank claims** accepted in **bank-loan repayment** over the run. Positive
    /// when the bank-repayment tender accepts claims; zero when that medium is refused.
    pub fn bank_repayment_claims_settled(&self) -> Gold {
        self.society
            .bank_repayment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| sum.saturating_add(row.demand_claims))
    }

    /// Total **specie** that repaid **bank loans** over the run.
    pub fn bank_repayment_specie_settled(&self) -> Gold {
        self.society
            .bank_repayment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| sum.saturating_add(row.public_specie))
    }

    /// Total bank credit retired by bank-loan repayment over the run.
    pub fn bank_repayment_credit_retired(&self) -> Gold {
        self.society
            .bank_repayment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| {
                sum.saturating_add(row.credit_retired)
            })
    }

    /// Total **fiat** that repaid **issuer credit** over the run. Positive when the
    /// issuer-repayment tender accepts fiat; zero under `FiatRefused`.
    pub fn issuer_repayment_fiat_settled(&self) -> Gold {
        self.society
            .issuer_repayment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| sum.saturating_add(row.public_fiat))
    }

    /// Total **specie** that repaid **issuer credit** over the run. Econ's issuer
    /// repayment surface never accepts specie for credit retirement, so this remains
    /// zero; it is exposed for viewer symmetry with the other surfaces.
    pub fn issuer_repayment_specie_settled(&self) -> Gold {
        self.society
            .issuer_repayment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| sum.saturating_add(row.public_specie))
    }

    /// Total issuer credit retired by issuer-credit repayment over the run.
    pub fn issuer_repayment_credit_retired(&self) -> Gold {
        self.society
            .issuer_repayment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| {
                sum.saturating_add(row.credit_retired)
            })
    }

    /// The settlement's **total broad money** (the TMS: specie + fiat + claims), or
    /// `Gold::ZERO` without an M3 ledger. Tender gates *composition* (which medium
    /// settles a surface); the spot/debt displacement twins hold this total equal
    /// while the per-surface fiat/specie split flips (G8c-2 tests 4, 6).
    pub fn total_broad_money(&self) -> Gold {
        self.money_composition()
            .map(|composition| composition.tms())
            .unwrap_or(Gold::ZERO)
    }

    /// Whether this settlement levies the G8c-3 **tax overlay** (the state's levy +
    /// receivability on the credit cycle). `false` for a plain cycle, every tender bench,
    /// and every spatial settlement — none of which the viewer's tax banner surfaces.
    pub fn is_tax(&self) -> bool {
        self.tax.is_some()
    }

    /// The G8c-3 **configured** tax receivability — the chartalist gate this overlay set,
    /// or `None` for a settlement that levies no tax. This is the policy the config
    /// chose; the *active* receivability ([`Self::tax_receivability`]) reads it back live
    /// from the society once the `Tick(0)` event has fired.
    pub fn configured_tax_receivability(&self) -> Option<TaxReceivability> {
        self.tax.as_ref().map(|tax| tax.receivability)
    }

    /// The **active** tax receivability — which media discharge the tax (reused from
    /// econ's society state; the viewer surfaces it). Reads the live policy, so a config
    /// that set it, or the `Tick(0)` `SetTaxReceivability` event that fired it, both show.
    /// econ's default is [`TaxReceivability::SpecieOnly`] until an event sets otherwise.
    pub fn tax_receivability(&self) -> TaxReceivability {
        self.society.tax_receivability
    }

    /// Total tax **levied** over the run — the sum of the single state issuer's
    /// `taxes_levied` (the zero-principal liabilities the state raised). `Gold::ZERO`
    /// without an issuer. Matches the overlay's configured total once every levy fired.
    pub fn taxes_levied(&self) -> Gold {
        self.sum_issuer_tax(|issuer| issuer.taxes_levied)
    }

    /// Total tax settled in **fiat** over the run (the issuer's `tax_receipts_fiat`). The
    /// chartalist headline signal: positive under a fiat-receivable tax (the fiscal
    /// channel circulates fiat the labor market refused), `Gold::ZERO` under a
    /// specie-receivable tax (no compelled fiat demand).
    pub fn tax_receipts_fiat(&self) -> Gold {
        self.sum_issuer_tax(|issuer| issuer.tax_receipts_fiat)
    }

    /// Total tax settled in **specie** over the run (the issuer's `tax_receipts_specie`).
    /// Positive under a specie-receivable tax; `Gold::ZERO` when only fiat discharges.
    pub fn tax_receipts_specie(&self) -> Gold {
        self.sum_issuer_tax(|issuer| issuer.tax_receipts_specie)
    }

    /// Total tax **defaulted** over the run (the issuer's `taxes_defaulted`) — a levy
    /// unmet **by rule** (the holder lacks the receivable medium), conserved, never a
    /// leak. In the counter-lever twin the non-receivable holder always defaults.
    pub fn taxes_defaulted(&self) -> Gold {
        self.sum_issuer_tax(|issuer| issuer.taxes_defaulted)
    }

    /// Sum a per-issuer tax counter across the society's issuers. econ's M21 is
    /// single-issuer, so this is the lone state issuer's counter; the fold is the
    /// defensive form (and `Gold::ZERO` for an issuer-free settlement).
    fn sum_issuer_tax(&self, field: impl Fn(&econ::issuer::Issuer) -> Gold) -> Gold {
        self.society
            .issuers
            .iter()
            .fold(Gold::ZERO, |sum, issuer| sum.saturating_add(field(issuer)))
    }

    fn shadow_cycle_metrics(&self) -> Option<ShadowCycleMetrics> {
        let cycle = self.cycle.as_ref()?;

        let ticks = self.society.m3_records.len();
        let cached = {
            let cache = self.shadow_cycle_cache.borrow();
            cache
                .as_ref()
                .filter(|metrics| metrics.ticks == ticks)
                .cloned()
        };
        if cached.is_some() {
            return cached;
        }

        let mut scenario = cycle.scenario.clone();
        scenario.periods = ticks as u64;
        let shadow = run_credit_disabled_shadow(&scenario);

        let mut gap_bps = Vec::with_capacity(ticks);
        let mut max_gap_bps = 0;
        let mut structure_rose_above_shadow = false;
        for (index, record) in self.society.m3_records.iter().enumerate() {
            let gap = match (
                shadow.natural_rate_bps.get(index).copied().flatten(),
                record.m2.market_rate_bps,
            ) {
                (Some(shadow_rate), Some(market_rate)) => Some(shadow_rate - market_rate),
                _ => None,
            };
            if let Some(gap) = gap.filter(|&gap| gap > max_gap_bps) {
                max_gap_bps = gap;
            }
            if record.m2.structure_length_ticks_x100
                > shadow
                    .structure_length_ticks_x100
                    .get(index)
                    .copied()
                    .unwrap_or(0)
            {
                structure_rose_above_shadow = true;
            }
            gap_bps.push(gap);
        }

        let metrics = ShadowCycleMetrics {
            ticks,
            natural_rate_bps: shadow.natural_rate_bps,
            gap_bps,
            max_gap_bps,
            structure_rose_above_shadow,
        };
        *self.shadow_cycle_cache.borrow_mut() = Some(metrics.clone());
        Some(metrics)
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
        self.shadow_cycle_cache.get_mut().take();
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

    /// The current vocation of the colonist with stable id `id` (living or dead),
    /// or `None` for a non-colonist id (e.g. a resident trader). Lets a test map a
    /// `Society::trades` buyer/seller back to a role — the S5 acceptance metric
    /// that an input was bought by an active Miller/Baker from a different seller.
    pub fn vocation_of_id(&self, id: AgentId) -> Option<Vocation> {
        self.colonist_slot_by_id
            .get(&id)
            .map(|&slot| self.colonists[slot].vocation)
    }

    /// S22a (internal): whether the colonist with stable id `id` is a spatial **lineage**
    /// member (`household.is_some()`) — the producer-class tag recorded at production time for
    /// the bread-provenance entrant-class split. Class is a stable per-agent property (a seeded
    /// non-lineage role never gains a household, a lineage member never loses one), so recording
    /// it at production time and reading it back at sale time agree. `false` for an unknown id.
    fn is_lineage_agent(&self, id: AgentId) -> bool {
        self.colonist_slot_by_id
            .get(&id)
            .is_some_and(|&slot| self.colonists[slot].household.is_some())
    }

    /// Whether `id` is a mortal reproducing-lineage actor for the S23d owner-identity invariant
    /// (§2.1). In this model reproduction flows through households — there is no separate fertility
    /// or adulthood concept — so live lineage-household membership *is* the reproductive-actor
    /// signal. The `lineage_id`/`reproduction_eligible`/`in_birth_kinship_graph` fields on
    /// [`MortalLandownerOwnerRow`] are therefore all derived from this one domain-grounded signal by
    /// design; they are not independent predicates. That is the correct signal for the §2.1
    /// disjoint-population test: it separates the immortal roster (`household = None`,
    /// `lifespan = None`) and any mortal shell outside a household from the reproducing lineage.
    /// It is deliberately **not** age-gated — a newborn heir owner is a legitimate member of the
    /// reproducing lineage, not a disjoint shell, so gating on adulthood here would misclassify it
    /// as residue.
    fn mortal_landowner_reproductive_actor(&self, id: AgentId) -> bool {
        let Some(&slot) = self.colonist_slot_by_id.get(&id) else {
            return false;
        };
        let colonist = &self.colonists[slot];
        colonist.alive
            && self.demography.is_some()
            && colonist.lifespan.is_some()
            && colonist.household.is_some()
            && self.society.agents.get(id).is_some()
    }

    fn mortal_landowner_claim_eligible(&self, id: AgentId) -> bool {
        !self.mortal_landowner_demography_active() || self.mortal_landowner_reproductive_actor(id)
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

    /// Units of `good` the colonist at generation `index` holds in **econ stock**
    /// (its deposited, market-tradeable balance — distinct from [`Self::carry_of`],
    /// which is the world delivery escrow that oscillates as the colonist harvests
    /// and deposits). The S6 re-entry acceptance metric reads this to prove a flipped
    /// colonist is *actually accumulating* the grain it gathers, not merely relabeled.
    pub fn stock_of(&self, index: usize, good: GoodId) -> u64 {
        self.colonists
            .get(index)
            .and_then(|c| self.society.agents.get(c.id))
            .map(|agent| u64::from(agent.stock.get(good)))
            .unwrap_or(0)
    }

    /// The resource node the colonist at generation `index` is currently assigned to
    /// harvest, or `None` (a non-gatherer / idle consumer has no node). The S6
    /// acceptance suite reads this to prove a re-entered colonist is on the **edible
    /// grain node** (`node_of(i) == grain_node()`), not merely flipped to `Gatherer`.
    pub fn node_of(&self, index: usize) -> Option<NodeId> {
        self.colonists.get(index).and_then(|c| c.node)
    }

    /// The resource node that yields the chain's raw grain (the edible subsistence
    /// good), or `None` (no chain / no grain node). The fixed target the S6 re-entry
    /// phase sends hungry colonists to; the acceptance suite compares
    /// [`Self::node_of`] against it.
    pub fn grain_node(&self) -> Option<NodeId> {
        let grain = self.chain.as_ref()?.content.grain();
        self.node_for_good(grain)
    }

    /// S18: the resource node that yields WOOD (the woodcutters' gathered good), or `None`
    /// (no WOOD node). The acceptance suite compares [`Self::node_of`] against it to prove
    /// the woodcutters are pinned to the WOOD node, not split onto grain.
    pub fn wood_node(&self) -> Option<NodeId> {
        self.node_for_good(WOOD)
    }

    /// S12: the FORAGE node — the [`Task::GoForage`] target — or `None` when own-labor
    /// subsistence is off. The acceptance suite compares a forager's world task against
    /// `Task::GoForage(forage_node, _)`.
    pub fn forage_node_id(&self) -> Option<NodeId> {
        self.forage_node()
    }

    /// S12: the FORAGE subsistence good, or `None` (no chain / no forage good).
    pub fn forage_good(&self) -> Option<GoodId> {
        self.chain.as_ref()?.content.forage()
    }

    /// S12: whether the colonist at generation `index` is currently foraging the FORAGE
    /// floor (the own-labor path). Always `false` for a non-own-labor settlement.
    pub fn is_foraging(&self, index: usize) -> bool {
        self.colonists.get(index).is_some_and(|c| c.foraging)
    }

    /// S15: whether the colonist at generation `index` is currently cultivating (hauling
    /// grain to make bread by own labor). Always `false` off the cultivation path.
    /// Mutually exclusive with [`Self::is_foraging`] — never both true in one econ tick.
    pub fn is_cultivating(&self, index: usize) -> bool {
        self.colonists.get(index).is_some_and(|c| c.cultivating)
    }

    /// S22a: cumulative tracked food (bread) the colonist at generation `index` ever acquired
    /// through the `Bought` channel — a market purchase. `0` for an agent that never bought, and
    /// `0` off the acquisition-ledger path. The rolling material-buyer diagnostic reads it to
    /// count non-cultivating buyers that actually transact (a genuine division-of-labor split),
    /// not just non-cultivators that are alive. Runtime-only; not digested.
    pub fn bought_food_of(&self, index: usize) -> u64 {
        self.colonists
            .get(index)
            .map_or(0, |c| self.acquisition.bought_credited_of(c.id))
    }

    /// P1.5 (read-only): cumulative tracked food (bread) the colonist at generation `index`
    /// acquired through the finite rival-commons channel. Runtime-only; not digested.
    pub fn commons_food_of(&self, index: usize) -> u64 {
        self.colonists
            .get(index)
            .map_or(0, |c| self.acquisition.commons_credited_of(c.id))
    }

    /// S23c (read-only): cumulative tracked food (bread) the colonist at `index` has CONSUMED
    /// (eaten) across every acquisition channel over the run — the per-agent food-intake basis a
    /// tenure-tier metric divides by. `0` off the acquisition-ledger path. Runtime-only; not
    /// digested.
    pub fn consumed_food_of(&self, index: usize) -> u64 {
        self.colonists
            .get(index)
            .map_or(0, |c| self.acquisition.consumed_food_of_agent(c.id))
    }

    /// S22b: the bounded cultivation **skill** of the colonist at generation `index` (born `0`,
    /// saturating at [`ChainConfig::skill_cap`]). `0` off the cultivation-skill path. The
    /// skill-distribution diagnostic (max / mean / count above a maturity threshold) reads it.
    /// Runtime-readable; the underlying field IS digested (ON-only, with the colonist roster).
    pub fn cultivation_skill_of(&self, index: usize) -> u16 {
        self.colonists.get(index).map_or(0, |c| c.cultivation_skill)
    }

    /// S22b: cumulative grain the colonist at generation `index` hauled (the cultivation input
    /// good deposited to econ) over the run — the monopolization / grain-share probe + the
    /// non-vacuity grain measure. `0` off the cultivation-skill path. Runtime-only; not digested.
    pub fn cultivation_grain_harvested_of(&self, index: usize) -> u64 {
        self.colonists.get(index).map_or(0, |c| {
            self.cultivation_grain_harvested
                .get(&c.id)
                .copied()
                .unwrap_or(0)
        })
    }

    /// S22b: cumulative bread the colonist at generation `index` produced by own-use cultivation
    /// over the run — the non-vacuity bread measure (a high-skill cultivator must produce
    /// strictly more). `0` off the cultivation-skill path. Runtime-only; not digested.
    pub fn cultivation_bread_produced_of(&self, index: usize) -> u64 {
        self.colonists.get(index).map_or(0, |c| {
            self.cultivation_bread_produced
                .get(&c.id)
                .copied()
                .unwrap_or(0)
        })
    }

    pub fn mortal_landowner_demography_on(&self) -> bool {
        self.mortal_landowner_demography_active()
    }

    pub fn owner_surplus_telemetry(&self) -> OwnerSurplusTelemetry {
        let mut produced_minus_consumed = Vec::new();
        for owner in &self.ever_landowner_ids {
            let produced = self
                .cultivation_bread_produced
                .get(owner)
                .copied()
                .unwrap_or(0);
            let consumed = self.owner_bread_consumed.get(owner).copied().unwrap_or(0);
            produced_minus_consumed.push((
                owner.0,
                i64::try_from(produced).unwrap_or(i64::MAX)
                    - i64::try_from(consumed).unwrap_or(i64::MAX),
            ));
        }
        produced_minus_consumed.sort_by_key(|&(owner, _)| owner);

        OwnerSurplusTelemetry {
            owner_age_at_first_claim: self
                .owner_age_at_first_claim
                .iter()
                .map(|(&owner, &age)| (owner.0, age))
                .collect(),
            owner_tenure_before_death: self
                .owner_tenure_before_death
                .iter()
                .map(|&(owner, tenure)| (owner.0, tenure))
                .collect(),
            owner_surplus_produced_minus_consumed: produced_minus_consumed,
            owner_surplus_sold_before_death: self
                .owner_surplus_sold_before_death
                .iter()
                .map(|(&owner, &qty)| (owner.0, qty))
                .collect(),
            owner_inventory_at_death: self
                .owner_inventory_at_death
                .iter()
                .map(|&(owner, qty)| (owner.0, qty))
                .collect(),
            inherited_stock_to_heirs: self.inherited_stock_to_heirs,
            buyer_purchases_by_owner_age_cohort: self
                .buyer_purchases_by_owner_age_cohort
                .iter()
                .map(|(&cohort, &qty)| (cohort, qty))
                .collect(),
            owner_seller_attributed_bought: self.owner_seller_attributed_bought,
        }
    }

    /// DH.a (impl-68): the wage-labor escrow pool (CC3 reads it at every window boundary — it must
    /// stay 0 on the closed regime, which runs no wage labor). Runtime-only.
    pub fn wage_escrow_gold(&self) -> u64 {
        self.wage_escrow_gold.0
    }

    /// S22c: the colonist at generation `index`'s windowed realized cultivation-sale proceeds (the
    /// own-cultivated bread→SALT SALT proceeds over the trailing [`RETURN_WINDOW`] ticks) — the
    /// per-agent return signal `profit_stay_active` reads. `0` off the profit-driven-retention path
    /// (the window is empty) and pre-money. Runtime-readable; the underlying window IS digested
    /// (ON-only, with the colonist roster). The non-vacuity test reads it to prove the signal
    /// VARIES across agents (not a single agent firing once).
    pub fn recent_cultivation_proceeds_of(&self, index: usize) -> u64 {
        let (return_window, _m, _f) = self.retention_params();
        self.colonists.get(index).map_or(0, |c| {
            window_return_sums(&c.cultivation_return_window, self.econ_tick, return_window).0
        })
    }

    /// S22c: the colonist at generation `index`'s windowed realized non-cultivation (outside) sale
    /// proceeds over the trailing [`RETURN_WINDOW`] ticks — its realized outside option. `0` off the
    /// path / pre-money / for a continuous cultivator with no outside sales (which then falls back
    /// to the colony reference in the rule). Runtime-readable; the window IS digested ON-only.
    pub fn recent_outside_proceeds_of(&self, index: usize) -> u64 {
        let (return_window, _m, _f) = self.retention_params();
        self.colonists.get(index).map_or(0, |c| {
            window_return_sums(&c.cultivation_return_window, self.econ_tick, return_window).2
        })
    }

    /// S22c (runtime-only diagnostic): whether the colonist at generation `index` is RETAINED-BY-
    /// PROFIT this tick — cultivating ONLY because the profit-stay term fired (past the hunger exit,
    /// no input in flight, not pressure-escalating), so the flag-off path would have EXITED it. The
    /// counterfactual exit-flip marker. `false` off the path.
    pub fn is_profit_retained(&self, index: usize) -> bool {
        self.colonists
            .get(index)
            .is_some_and(|c| self.profit_retained_ids.contains(&c.id))
    }

    /// S22c (runtime-only diagnostic): the count of agents retained-by-profit THIS tick — the
    /// counterfactual exit-flip count. `0` off the path.
    pub fn profit_retained_now(&self) -> usize {
        self.profit_retained_ids.len()
    }

    /// S22c (runtime-only diagnostic): distinct agents ever retained-by-profit over the run. `0`
    /// off the path. The non-vacuity test reads it to confirm the flip is not a one-agent fluke.
    pub fn profit_retained_ever_count(&self) -> usize {
        self.profit_retained_ever.len()
    }

    /// S22f: the colonist at generation `index`'s remaining voluntary-commitment term (econ ticks
    /// left; `0` = uncommitted). Runtime-readable; the underlying field IS digested (ON-only, with the
    /// colonist roster). `0` off the voluntary-commitment path.
    pub fn commitment_remaining_of(&self, index: usize) -> u16 {
        self.colonists
            .get(index)
            .map_or(0, |c| c.commitment_remaining)
    }

    /// S22f: whether the colonist at generation `index` is currently committed (`commitment_remaining
    /// > 0`). `false` off the path.
    pub fn is_committed(&self, index: usize) -> bool {
        self.commitment_remaining_of(index) > 0
    }

    /// S22f: the colonist at generation `index`'s count of renewals from FRESH post-expiry signals
    /// (the first opt-in is not a renewal). Runtime-readable; the underlying field IS digested
    /// (ON-only). `0` off the path / for a never-renewed committer.
    pub fn commitment_renewals_of(&self, index: usize) -> u16 {
        self.colonists
            .get(index)
            .map_or(0, |c| c.commitment_renewals)
    }

    /// S22f (runtime-only diagnostic): the distinct agent ids that EVER held a commitment over the run
    /// (voluntary or fiat) — the committed cohort. Empty off the path.
    pub fn commitment_committed_ids(&self) -> Vec<u64> {
        self.commitment_committed_ever
            .iter()
            .map(|id| id.0)
            .collect()
    }

    /// S22f (runtime-only diagnostic): per VOLUNTARY committer, the `(econ tick, signal value)` at its
    /// first signal-gated opt-in — each entry traceable to that agent's own cleared S22c return. Keyed
    /// by `AgentId.0`. Empty off the path / under the fiat-pin control (forced commits have no signal).
    pub fn commitment_uptake(&self) -> BTreeMap<u64, (u64, u64)> {
        self.commitment_uptake
            .iter()
            .map(|(id, &v)| (id.0, v))
            .collect()
    }

    /// S22f (runtime-only diagnostic): the agent ids force-committed by the `fiat_pin` control
    /// (bypassing the voluntary signal). Empty for the voluntary headline + every other config.
    pub fn commitment_fiat_ids(&self) -> Vec<u64> {
        self.commitment_fiat_ever.iter().map(|id| id.0).collect()
    }

    /// S22f (runtime-only diagnostic): the eligible UNCOMMITTED agent ids whose entry signal fell
    /// below the entry floor post-money — the below-floor non-committers that prove the signal
    /// DISCRIMINATES (entry is a real decision). Empty off the path / under the fiat-pin control.
    pub fn commitment_below_floor_ids(&self) -> Vec<u64> {
        self.commitment_below_floor_ever
            .iter()
            .map(|id| id.0)
            .collect()
    }

    /// S22f (runtime-only diagnostic): the distinct agent ids whose commitment ever BOUND a tick the
    /// matched flag-off run would have exited (a real exit-override) — the mandatory non-vacuity test
    /// reads it. Empty off the path.
    pub fn commitment_exit_overridden_ids(&self) -> Vec<u64> {
        self.commitment_exit_override_ever
            .iter()
            .map(|id| id.0)
            .collect()
    }

    /// S22f: the configured commitment binding length (econ ticks) — the headline hard guard
    /// (`commitment_term < ceil(PERSIST_FRACTION × FINAL_WINDOW)`) reads it. The pinned default for a
    /// non-chain settlement.
    pub fn commitment_term_config(&self) -> u16 {
        self.commitment_term()
    }

    /// S22f: whether the voluntary fixed-term cultivation commitment path is active for this
    /// settlement (the flag on + the S22c composition). The non-vacuity/precondition test reads it.
    pub fn voluntary_cultivation_commitment_on(&self) -> bool {
        self.voluntary_cultivation_commitment_active()
    }

    pub fn abandonable_norm_on(&self) -> bool {
        self.abandonable_norm_active()
    }

    pub fn group_payoff_imitation_on(&self) -> bool {
        self.group_payoff_imitation_active()
    }

    pub fn flip_commitment_identity_for_fitness_guard(&mut self) {
        let term = self.commitment_term().max(1);
        for colonist in &mut self.colonists {
            colonist.adopts_commitment_norm = !colonist.adopts_commitment_norm;
            colonist.next_norm_bit = colonist.next_norm_bit.map(|bit| !bit);
            colonist.commitment_remaining = if colonist.commitment_remaining == 0 {
                term
            } else {
                0
            };
        }
        if let Some(chain) = self.chain.as_mut() {
            if let Some(prevalence) = chain.fixed_commitment_norm_prevalence {
                chain.fixed_commitment_norm_prevalence = Some(1.0 - prevalence);
            }
        }
    }

    pub fn adopts_commitment_norm_of(&self, index: usize) -> bool {
        self.colonists
            .get(index)
            .is_some_and(|c| c.adopts_commitment_norm)
    }

    pub fn pending_commitment_norm_bit_of(&self, index: usize) -> Option<bool> {
        self.colonists.get(index).and_then(|c| c.next_norm_bit)
    }

    /// S22c: the chain's `cultivate_hunger_out` threshold — the hysteresis exit below which a
    /// cultivating agent normally leaves (the non-vacuity test reads it to confirm a retained agent
    /// is genuinely past the exit). `0` for a non-chain settlement.
    pub fn cultivate_hunger_out(&self) -> u16 {
        self.chain.as_ref().map_or(0, |c| c.cultivate_hunger_out)
    }

    /// S22c (test-only control): empty EVERY colonist's cultivation-return window. The
    /// ZERO-RETURNS control re-applies it before each econ tick — the cultivate decision reads an
    /// empty window, so `profit_stay_active` never fires (the material floor is never cleared) — so
    /// stickiness must DISAPPEAR even with the rule ON, proving the *signal* (not the rule's mere
    /// presence) drives any retention. A no-op off the profit-driven-retention path.
    pub fn clear_cultivation_return_windows(&mut self) {
        for colonist in &mut self.colonists {
            colonist.cultivation_return_window.clear();
        }
    }

    /// S22b (test-only lever control): pin EVERY living colonist's cultivation skill to `skill`
    /// (clamped to [`ChainConfig::skill_cap`]). The mandatory non-vacuity test re-applies it
    /// before each econ tick — the per-trip haul reads skill at task-assignment time, the
    /// end-of-tick [`Self::run_cultivation_skill`] then overwrites it — so a CAP-pinned run and a
    /// 0-pinned run differ ONLY by the grain-haul capacity, isolating the lever's effect. A no-op
    /// off the cultivation-skill path (the skill is never read there).
    pub fn set_all_cultivation_skill(&mut self, skill: u16) {
        let cap = self.chain.as_ref().map_or(u16::MAX, |c| c.skill_cap);
        let skill = skill.min(cap);
        for &slot in &self.live_colonist_slots {
            self.colonists[slot].cultivation_skill = skill;
        }
    }

    /// S22b (test-only lever control): pin the cultivation skill of the colonist at generation
    /// `index` to `skill` (clamped to [`ChainConfig::skill_cap`]). Lets a controlled non-vacuity
    /// micro-harness drive ONE designated cultivator to `SKILL_CAP` while holding the rest of the
    /// colony fixed, isolating that agent's per-opportunity grain/bread productivity.
    pub fn set_cultivation_skill_for_test(&mut self, index: usize, skill: u16) {
        let cap = self.chain.as_ref().map_or(u16::MAX, |c| c.skill_cap);
        if let Some(colonist) = self.colonists.get_mut(index) {
            colonist.cultivation_skill = skill.min(cap);
        }
    }

    /// S22d: the durable cultivation tool good (the plow), or `None` off the
    /// durable-cultivation-capital path. The owner diagnostics read OWNERSHIP through this good's
    /// stock (NOT the generic `acquired_tool` bool, which is the mill/oven marker).
    pub fn cultivation_tool_good_id(&self) -> Option<GoodId> {
        self.cultivation_tool_good()
    }

    /// S22d (runtime-only diagnostic): whether the colonist at generation `index` OWNS a durable
    /// cultivation tool (holds ≥1 plow in its stock). The owner-exclusive boost + the owner-cohort
    /// overlap read this, NOT `acquired_tool`. `false` off the path / for a non-chain settlement.
    pub fn owns_cultivation_tool(&self, index: usize) -> bool {
        let Some(tool) = self.cultivation_tool_good() else {
            return false;
        };
        self.colonists.get(index).is_some_and(|c| {
            self.society
                .agents
                .get(c.id)
                .is_some_and(|agent| agent.stock.get(tool) > 0)
        })
    }

    /// S22d (runtime-only diagnostic): the count of LIVING colonists that own a durable cultivation
    /// tool right now. `0` off the path.
    pub fn cultivation_tool_owner_count(&self) -> usize {
        (0..self.population())
            .filter(|&i| self.is_alive(i) && self.owns_cultivation_tool(i))
            .count()
    }

    /// S22d (runtime-only diagnostic): the per-colonist realized-cultivation-output TENURE streak
    /// (the build-eligibility counter; consecutive output ticks, reset otherwise). Runtime-readable;
    /// the underlying counter IS digested (ON-only, with the colonist roster). `0` off the path.
    pub fn cultivation_tenure_of(&self, index: usize) -> u16 {
        self.colonists
            .get(index)
            .map_or(0, |c| c.cultivation_tenure)
    }

    /// S22d observability: the whole-system count of durable cultivation tools BUILT over the run
    /// (the cumulative `produced` plows; never decreases — durable, no decay). `0` until the first
    /// build. Runtime-only; not digested.
    pub fn cultivation_tools_built(&self) -> u64 {
        self.cultivation_tools_built
    }

    /// S22d observability: the cumulative WOOD permanently consumed building cultivation tools over
    /// the run — the measured SUNK cost. `0` until the first build. Runtime-only; not digested.
    pub fn cultivation_tool_wood_consumed(&self) -> u64 {
        self.cultivation_tool_wood_consumed
    }

    /// S22d observability: the cumulative durable cultivation tools DESTROYED over the run — `0` for
    /// the durable headline, positive only on the non-durable/rented control (each plow consumed
    /// after one cultivation opportunity). The tool-stock accounting invariant is
    /// `cultivation_tools_built() − cultivation_tools_destroyed() == cultivation_tool_stock_total()`.
    /// Runtime-only; not digested.
    pub fn cultivation_tools_destroyed(&self) -> u64 {
        self.cultivation_tools_destroyed
    }

    /// S22d observability: the whole-system total of the cultivation-tool good right now (live-agent
    /// stock + commons/estate + world). The tool-stock accounting invariant pins
    /// `cultivation_tools_built() − cultivation_tools_destroyed() == cultivation_tool_stock_total()`
    /// (the durable headline destroys none, so it reduces to `built == stock_total`; the build's
    /// completion deposits immediately so there is no completed-but-undeposited in-flight). `0` off
    /// the path. Runtime-only; not digested.
    pub fn cultivation_tool_stock_total(&self) -> u64 {
        self.cultivation_tool_good()
            .map_or(0, |tool| self.whole_system_total(tool))
    }

    /// S22d: the number of in-flight per-builder cultivation-tool projects right now. Read-only
    /// diagnostic for tests (a build is dropped on completion or builder death).
    pub fn active_cultivation_tool_builds(&self) -> usize {
        self.cultivation_tool_builds.len()
    }

    /// S22e observability: the count of durable cultivation tools (plows) GRANTED to lineage
    /// households at generation (the conservation-safe INITIAL endowment). `0` off the path. The
    /// §3.5 tool-stock invariant is `endowed + built − destroyed == stock_total`, asserting
    /// non-negativity first. Runtime-only; not digested.
    pub fn endowed_cultivation_tools_total(&self) -> u64 {
        self.endowed_cultivation_tools_total
    }

    /// S22e (runtime diagnostic): the lineage household indices actually granted an endowment at
    /// generation after deterministic hash selection. Empty off the path.
    pub fn endowed_household_indices(&self) -> &[usize] {
        &self.endowed_households
    }

    /// S22e (runtime diagnostic): whether the colonist at generation `index` is an ENDOWED founding
    /// member (was granted a plow at generation). `false` off the path / for a non-endowed agent.
    pub fn is_endowed_member(&self, index: usize) -> bool {
        self.colonists
            .get(index)
            .is_some_and(|c| self.endowed_member_ids.contains(&c.id))
    }

    /// S22e observability: the cumulative count of plow units that passed to a LIVING household heir
    /// via the estate settlement (a real inheritance transfer). `0` when the no-inheritance switch
    /// forces plows to the commons, or when no plow-holder dies leaving a living heir. Runtime-only;
    /// not digested.
    pub fn cultivation_tool_inherited_total(&self) -> u64 {
        self.cultivation_tool_inherited_total
    }

    /// S22e (runtime diagnostic): the set of heir agent ids (as `u64`) that received ≥1 plow via a
    /// real inheritance transfer, in id order. The non-vacuity test reads these to confirm a
    /// post-founder-death transfer to a living heir occurred. Empty when none did.
    pub fn cultivation_tool_inheritor_ids(&self) -> Vec<u64> {
        self.cultivation_tool_inheritor_ids
            .iter()
            .map(|id| id.0)
            .collect()
    }

    /// S22d (test-only matched-condition harness): grant the colonist at generation `index` one
    /// durable cultivation tool by adding it to the colonist's agent stock. Conservation-safe ONLY
    /// when called BEFORE the first `econ_tick` (the granted unit is then part of the tick-0
    /// whole-system baseline, so `before == after` each tick and `conserves()` holds) — the
    /// non-vacuity micro-harness compares a granted OWNER vs an ungranted no-tool cultivator under
    /// matched forced cultivation. A no-op off the durable-cultivation-capital path.
    pub fn grant_cultivation_tool_for_test(&mut self, index: usize) {
        let Some(tool) = self.cultivation_tool_good() else {
            return;
        };
        if let Some(colonist) = self.colonists.get(index) {
            let id = colonist.id;
            if let Some(agent) = self.society.agents.get_mut(id) {
                agent.stock.add(tool, 1);
            }
        }
    }

    /// S22d (test-only matched-condition harness): pin the colonist at generation `index`'s
    /// `cultivating` flag to `cultivating`. Re-applied before each `econ_tick` so the next fast
    /// loop assigns the grain harvest to the designated agent regardless of its hunger — isolating
    /// the owner-haul boost from the hunger-gated exit (the only difference between the matched
    /// owner / no-tool runs is then the per-trip haul). Conservation-neutral (a steering flag).
    pub fn set_cultivating_for_test(&mut self, index: usize, cultivating: bool) {
        if let Some(colonist) = self.colonists.get_mut(index) {
            colonist.cultivating = cultivating;
        }
    }

    /// S15: the chain's bread good (the cultivation output / hunger staple), or `None`
    /// for a non-chain settlement. The acceptance suite reads cultivated-bread
    /// production through it.
    pub fn bread_good(&self) -> Option<GoodId> {
        Some(self.chain.as_ref()?.content.bread())
    }

    /// S7.1: whether the colonist at generation `index` is ELIGIBLE to adopt a chain
    /// vocation in the role-choice appraisal — either it is seeded `latent`, or (when
    /// [`ChainConfig::tool_acquisition_eligibility`] is on) it now HOLDS the required
    /// tool (a built or handed mill/oven). A read-only mirror of the relaxed
    /// `run_role_choice` entry gate, so a test can confirm a tool-holder became
    /// eligible without the gate being a relabel. Always `false` for a non-chain
    /// settlement, and — with the flag off — exactly "is seeded latent".
    pub fn is_tool_acquisition_eligible(&self, index: usize) -> bool {
        let Some(colonist) = self.colonists.get(index) else {
            return false;
        };
        if self.mortal_chain_producers_active() && colonist.lifespan.is_none() {
            return false;
        }
        if colonist.latent.is_some() {
            return true;
        }
        let Some(chain) = &self.chain else {
            return false;
        };
        if !chain.tool_acquisition_eligibility {
            return false;
        }
        self.society.agents.get(colonist.id).is_some_and(|agent| {
            agent.stock.get(chain.content.mill()) > 0 || agent.stock.get(chain.content.oven()) > 0
        })
    }

    /// S7 observability: whether the colonist at generation `index` built a chain tool
    /// it did not start latent with (the `acquired_tool` marker — acceptance test 6).
    /// Read-only; `false` for every colonist until S7.2 capital formation completes a
    /// build for it.
    pub fn acquired_tool_of(&self, index: usize) -> bool {
        self.colonists.get(index).is_some_and(|c| c.acquired_tool)
    }

    /// S7.2 observability: the whole-system count of tools built via the per-builder
    /// capital-formation phase over the run (acceptance tests 4/6). `0` until a build
    /// completes; never decreases (tools are durable). Read-only.
    pub fn tools_built(&self) -> u64 {
        self.tools_built
    }

    /// C3R.a runtime-only telemetry: old-age deaths among mortal chain-producer
    /// subjects. Excluded from canonical bytes.
    pub fn mortal_producer_old_age_deaths(&self) -> u64 {
        self.mortal_producer_old_age_deaths
    }

    /// C3R.a reservoir guard: live active producer vocations that have no lifespan.
    /// Under `mortal_chain_producers`, this must stay zero.
    pub fn immortal_producer_count(&self) -> usize {
        self.live_colonist_slots
            .iter()
            .filter(|&&slot| {
                let colonist = &self.colonists[slot];
                colonist.lifespan.is_none()
                    && matches!(colonist.vocation, Vocation::Miller | Vocation::Baker)
            })
            .count()
    }

    /// C3R.a runtime-only telemetry: mortal agents adopting a chain producer role
    /// after at least one mortal producer old-age death.
    pub fn role_readoptions(&self) -> u64 {
        self.role_readoptions
    }

    /// C3R.a runtime-only telemetry: fresh mill/oven units completed by mortal
    /// builders, excluding the pre-seeded latent tools.
    pub fn mortal_capital_builds(&self) -> u64 {
        self.mortal_capital_builds
    }

    /// C3R.b runtime-only telemetry: mill/oven units from dead producer estates that
    /// reached a living heir. Counts both mill and oven tools; excluded from
    /// canonical bytes.
    pub fn producer_tool_inheritances(&self) -> u64 {
        self.producer_tool_inheritances
    }

    /// C3R.b runtime-only telemetry: producer deaths whose mill/oven tools went to
    /// commons because no live household heir existed.
    pub fn heirless_producer_deaths(&self) -> u64 {
        self.heirless_producer_deaths
    }

    /// C3R.b runtime-only telemetry: inherited-tool holders that adopted Miller/Baker
    /// through the ordinary S7 role-choice path while still holding that tool.
    pub fn heir_tool_adoptions(&self) -> u64 {
        self.heir_tool_adoptions
    }

    /// C3R.b v2 runtime-only telemetry: producer-house-scoped food hearth units.
    pub fn producer_house_hearth_food_minted(&self) -> u64 {
        self.producer_house_hearth_food_minted
    }

    /// C3R.b v2 runtime-only telemetry: non-producer-house food hearth units.
    pub fn non_producer_hearth_food_minted(&self) -> u64 {
        self.non_producer_hearth_food_minted
    }

    /// C3R.b v2 runtime-only telemetry: births in dedicated producer households.
    pub fn producer_house_births(&self) -> u64 {
        self.producer_house_births
    }

    /// C3R.b v2 runtime-only telemetry: deaths among producer-house members.
    pub fn producer_house_deaths(&self) -> u64 {
        self.producer_house_deaths
    }

    /// C3R.b v2 runtime-only telemetry: live producer-house member-ticks.
    pub fn producer_house_person_ticks(&self) -> u64 {
        self.producer_house_person_ticks
    }

    /// C3R.b v2 runtime-only telemetry: role recipe appraisals that did not pay.
    pub fn producer_recipe_pay_rejections(&self) -> u64 {
        self.producer_recipe_pay_rejections
    }

    /// C3R.b v2 runtime-only telemetry: capital build appraisals/eligibility misses.
    pub fn producer_build_rejections(&self) -> u64 {
        self.producer_build_rejections
    }

    /// C3R.b v2 runtime-only telemetry: producer-role adoption switch rejections.
    pub fn producer_adoption_rejections(&self) -> u64 {
        self.producer_adoption_rejections
    }

    /// C3R.g (impl-72) Stage-1 diagnostic: the per-run mill/bake role-choice reason
    /// histogram + Baker-hold econ-ticks. Runtime-only; excluded from `canonical_bytes`.
    pub fn role_choice_diag(&self) -> RoleChoiceDiag {
        self.saving_allocation_obs.role_choice_diag
    }

    /// Test-support (C3R.g digest tripwire): perturb the role-choice diagnostic so an
    /// integration test can prove it is excluded from [`Self::canonical_bytes`]. Pure
    /// telemetry mutation — it must never shift the byte stream.
    pub fn debug_perturb_role_choice_diag(&mut self) {
        let diag = &mut self.saving_allocation_obs.role_choice_diag;
        diag.bake.observe(RoleChoiceReason::Accepts);
        diag.baker_last_econ_tick = Some(diag.baker_last_econ_tick.unwrap_or(0).saturating_add(1));
    }

    /// Arm the default-off flour census for its next Bake input-price absence.
    pub fn debug_arm_flour_census(&mut self) {
        self.flour_census_armed = true;
    }

    /// Take the captured row without re-arming.
    pub fn debug_take_flour_census(&mut self) -> Option<FlourCensusRow> {
        self.flour_census.take()
    }

    /// C3R.j: build the census row for `appraiser` at the CURRENT tick, unconditionally.
    /// The armed capture fires only on a Bake `InputPriceAbsent` decline, so row-absence
    /// there is ambiguous between "an ask appeared", "a different rejection", and "no Bake
    /// candidate was appraised at all" — persistence sampling reads this for the per-tick
    /// holder texture (does the wall recur every tick, or oscillate?) instead of inferring
    /// it from absence. `resolves` itself stays pinned to the Bake `accepts` histogram, so
    /// this reports the shape of the window, not the stop rule. `&self`-pure and, like the
    /// armed capture, outside [`Self::canonical_bytes`]: it arms nothing and consumes nothing.
    pub fn debug_flour_census_row_now(&self, appraiser: AgentId) -> Option<FlourCensusRow> {
        let chain = self.chain.as_ref()?;
        let flour = chain.content.flour();
        let grain = chain.content.grain();
        let money_good = self.current_money_good()?;
        Some(self.build_flour_census_row(appraiser, flour, grain, money_good, self.society.tick.0))
    }

    /// C3R.j: how `id` came by an oven, from state the settlement already records —
    /// no new persistent field. Seeded specialty outranks recorded inheritance, which
    /// outranks the existing self-build marker. That precedence LOSES information when
    /// both hold, so `recorded_inheritor` is reported on the row unshadowed alongside.
    fn oven_provenance(
        &self,
        id: AgentId,
        holds_oven: bool,
        recorded_inheritor: bool,
    ) -> ToolProvenance {
        let Some(&slot) = self.colonist_slot_by_id.get(&id) else {
            return ToolProvenance::Other;
        };
        let colonist = &self.colonists[slot];
        if colonist.latent == Some(RecipeId::Bake) {
            return ToolProvenance::SeededLatent;
        }
        if recorded_inheritor {
            return ToolProvenance::Inherited;
        }
        if holds_oven && colonist.acquired_tool {
            ToolProvenance::SelfBuilt
        } else {
            ToolProvenance::Other
        }
    }

    /// Build the row using read-only stock, reservation, commons, and trace access.
    fn build_flour_census_row(
        &self,
        appraiser: AgentId,
        flour: GoodId,
        grain: GoodId,
        money_good: GoodId,
        decline_tick: u64,
    ) -> FlourCensusRow {
        let candidate_own_flour = self
            .society
            .agents
            .get(appraiser)
            .map_or(0, |agent| agent.stock.get(flour));
        // C3R.j identity axis: the oven good comes from the chain content the capture
        // site already resolved, so the builder's signature is unchanged.
        let oven = self.chain.as_ref().map(|chain| chain.content.oven());
        let candidate_holds_oven = oven.is_some_and(|oven| {
            self.society
                .agents
                .get(appraiser)
                .is_some_and(|agent| agent.stock.get(oven) > 0)
        });
        let candidate_recorded_inheritor =
            oven.is_some_and(|oven| self.producer_tool_inheritors.contains(&(appraiser, oven)));
        let mut colonists = Vec::new();
        let mut millers = Vec::new();
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            let id = colonist.id;
            if id == appraiser {
                continue;
            }
            let Some(agent) = self.society.agents.get(id) else {
                continue;
            };
            let flour_held = agent.stock.get(flour);
            // Both come from ONE body (`reservation_ask_for_money` is the `Some/None`
            // projection, `econ/src/agent.rs:487`), so they cannot disagree today; the
            // row carries both so a consumer sees the executable value beside its reason,
            // and the test's equality check is a tripwire against a future re-implementation.
            let reservation_ask = agent.reservation_ask_for_money(flour, 1, money_good);
            let ask_outcome = agent.reservation_ask_outcome(flour, 1, money_good);
            colonists.push(FlourCensusColonist {
                id,
                vocation: colonist.vocation,
                flour_held,
                reservation_ask,
                ask_outcome,
            });
            if colonist.vocation == Vocation::Miller {
                millers.push(FlourCensusMiller {
                    id,
                    gold: agent.gold.0,
                    grain_held: agent.stock.get(grain),
                    flour_held,
                });
            }
        }
        FlourCensusRow {
            decline_tick,
            deaths_before_decline: self.mortal_producer_old_age_deaths,
            candidate_id: appraiser,
            candidate_own_flour,
            candidate_vocation: self.vocation_of_id(appraiser),
            candidate_holds_oven,
            candidate_provenance: self.oven_provenance(
                appraiser,
                candidate_holds_oven,
                candidate_recorded_inheritor,
            ),
            candidate_recorded_inheritor,
            colonists,
            commons_flour: self.commons_stock_of(flour),
            millers,
            bootstrap_trace_active: self.acquisition_ledger_active(),
            bootstrap: self.bootstrap_trace_summary(),
        }
    }

    /// Runtime-only per-run Baker-class round-trip counters.
    pub fn baker_round_trip(&self) -> BakerRoundTrip {
        self.saving_allocation_obs.baker_round_trip
    }

    /// Test-support digest tripwire for the runtime-only accumulator. Perturbs EVERY
    /// field, so the `canonical_bytes` exclusion check covers all of them, not just the
    /// first.
    pub fn debug_perturb_baker_round_trip(&mut self) {
        let acc = &mut self.saving_allocation_obs.baker_round_trip;
        acc.flour_gold_spent = acc.flour_gold_spent.saturating_add(1);
        acc.bread_gold_earned = acc.bread_gold_earned.saturating_add(1);
        acc.bread_units_sold = acc.bread_units_sold.saturating_add(1);
        acc.bread_units_produced = acc.bread_units_produced.saturating_add(1);
    }

    /// C3R.a population-artifact guard: live mortal agents that can currently build
    /// or adopt a chain producer role under the same mortality gate.
    pub fn mortal_builder_adopter_pool(&self) -> usize {
        if !self.mortal_chain_producers_active() {
            return 0;
        }
        let Some(chain) = &self.chain else {
            return 0;
        };
        let mill = chain.content.mill();
        let oven = chain.content.oven();
        let tool_eligibility = chain.tool_acquisition_eligibility;
        let producible_capital = chain.producible_capital;
        let wood_qty = chain.tool_build_wood;
        let hunger_max = chain.capital_build_hunger_max;
        self.live_colonist_slots
            .iter()
            .filter(|&&slot| {
                let colonist = &self.colonists[slot];
                if colonist.lifespan.is_none() {
                    return false;
                }
                if matches!(colonist.latent, Some(RecipeId::Mill) | Some(RecipeId::Bake)) {
                    return true;
                }
                let Some(agent) = self.society.agents.get(colonist.id) else {
                    return false;
                };
                if tool_eligibility && (agent.stock.get(mill) > 0 || agent.stock.get(oven) > 0) {
                    return true;
                }
                producible_capital
                    && colonist.latent.is_none()
                    && matches!(
                        colonist.vocation,
                        Vocation::Gatherer | Vocation::Consumer | Vocation::Unassigned
                    )
                    && colonist.need.hunger <= hunger_max
                    && !self
                        .capital_builds
                        .iter()
                        .any(|build| build.builder == colonist.id)
                    && agent.stock.get(mill) == 0
                    && agent.stock.get(oven) == 0
                    && agent.stock.get(WOOD) >= wood_qty
            })
            .count()
    }

    /// S7.2: the number of in-flight per-builder capital projects right now (a build is
    /// dropped on completion or builder death). Read-only diagnostic for tests.
    pub fn active_capital_builds(&self) -> usize {
        self.capital_builds.len()
    }

    /// S10 observability: the per-agent build decisions the LAST per-agent
    /// capital-formation phase recorded this tick — one [`CapitalDecision`] per eligible
    /// candidate, in slot order, with accept/reject, the target savings want rank, and the
    /// decline reason. Empty on the S7 heuristic path and on any tick the phase did not
    /// run. Lets a test prove the build is a per-colonist ordinal decision (an
    /// earlier-eligible colonist declined on its OWN scale while a later one accepted).
    pub fn last_capital_decisions(&self) -> &[CapitalDecision] {
        &self.last_capital_decisions
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

    /// S17 — lifetime **starvation** (positive-check) deaths so far, distinct from
    /// `old_age_deaths_total` so the two Malthusian checks are attributable. A
    /// runtime-only diagnostic (NOT in `canonical_bytes`): it shifts no digest, so every
    /// existing golden — including the live-starvation `g4a_death` / `starved_hauler`
    /// configs — is byte-identical. Read-only.
    pub fn starvation_deaths_total(&self) -> u64 {
        self.starvation_deaths_total
    }

    /// S14 — birth-block diagnostics: lifetime count of births skipped because the
    /// household was still inside its birth interval. Read-only.
    pub fn birth_block_interval(&self) -> u64 {
        self.birth_block_interval
    }

    /// S14 — birth-block diagnostics: lifetime count of births skipped because the
    /// household was at the `max_household_size` cap (the artificial knob / blowup
    /// bound). On the forage-commons path this should NOT be the binding stall. Read-only.
    pub fn birth_block_size_cap(&self) -> u64 {
        self.birth_block_size_cap
    }

    /// S14 — birth-block diagnostics: lifetime count of births skipped because a member
    /// was over the birth-hunger ceiling — the **preventive check**. On the
    /// forage-commons path a genuine carrying-capacity plateau shows up here (forage
    /// scarcity raises hunger and stalls births). Read-only.
    pub fn birth_block_hunger_ceiling(&self) -> u64 {
        self.birth_block_hunger_ceiling
    }

    /// S14 — birth-block diagnostics: lifetime count of births skipped because no member
    /// held the child's food endowment. The P1a tripwire: a non-trivial count on the
    /// forage-commons path would mean births stall on the *endowment*, not the hunger
    /// ceiling (the plateau would be endowment-bound, not forage-bound). Read-only.
    pub fn birth_block_endowment(&self) -> u64 {
        self.birth_block_endowment
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

    /// S23c (read-only): the parent (owning progenitor) of the colonist at generation `index`,
    /// or `None` for a founder / pre-demography colonist. Lets the heir-order probe recompute the
    /// deterministic universal-heir selection (child-of-deceased before same-household kin).
    pub fn parent_of(&self, index: usize) -> Option<AgentId> {
        self.colonists.get(index).and_then(|c| c.parent)
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

    /// S11: the heritable forecast bias of the colonist at generation `index` (bps;
    /// `10_000` = ×1.0 neutral), or `None` for a non-colonist index. Read-only.
    pub fn forecast_bias_of(&self, index: usize) -> Option<u16> {
        self.colonists
            .get(index)
            .map(|c| c.culture.forecast_bias_bps)
    }

    /// S11: the colonist's live **grounded fallible forecast** of `good`'s OUTPUT price —
    /// the entrepreneurial estimate the appraisals weigh: its belief `expected` once it has
    /// observed the good, else the public realized price, each × its forecast bias; `None`
    /// if it has neither an observed belief nor a realized price to ground on. The forecast
    /// a test reads to show it can be WRONG (materially differs from the realized price).
    pub fn forecast_price_for_good(&self, index: usize, good: GoodId) -> Option<Gold> {
        let colonist = self.colonists.get(index)?;
        let agent = self.society.agents.get(colonist.id)?;
        forecast_output_price(
            agent,
            good,
            self.realized_price(good),
            colonist.culture.forecast_bias_bps,
        )
    }

    /// S11: the colonist's adaptive belief level for `good` (its `PriceBelief.expected`),
    /// or `None` for a non-colonist index. A test samples it across ticks to show beliefs
    /// adapt toward realized (`observe()` is live) — forecasting under uncertainty, not
    /// clairvoyance.
    pub fn belief_expected_of(&self, index: usize, good: GoodId) -> Option<Gold> {
        let colonist = self.colonists.get(index)?;
        let agent = self.society.agents.get(colonist.id)?;
        Some(agent_belief(agent, good).expected)
    }

    /// S11: whether the colonist has actually OBSERVED `good` (its belief was updated by a
    /// trade/quote) — the grounding switch the forecast reads, distinct from a tick-0
    /// `last_seen`. `false` for a non-colonist index.
    pub fn belief_observed_of(&self, index: usize, good: GoodId) -> bool {
        self.colonists
            .get(index)
            .and_then(|c| self.society.agents.get(c.id))
            .is_some_and(|agent| agent_belief(agent, good).observed)
    }

    /// S11: total money proceeds the colonist has REALIZED from selling on the market —
    /// `Σ price × qty` over every trade where it was the seller, across the whole run. The
    /// realized side of profit/loss: an over-optimist that adopted/built on an inflated
    /// forecast earns the real (lower) proceeds. Read-only over the trade tape.
    pub fn realized_proceeds_of(&self, index: usize) -> u128 {
        let Some(colonist) = self.colonists.get(index) else {
            return 0;
        };
        let id = colonist.id;
        self.society
            .trades
            .iter()
            .filter(|t| t.seller == id)
            .map(|t| u128::from(t.price.0) * u128::from(t.qty))
            .sum()
    }

    /// S11: the colonist's NET-WORTH balance sheet —
    /// `gold + WOOD × realized_wood_price + tools × V`, where `V` is the tool's realized
    /// LIQUIDATION price if tools ever trade ELSE ZERO. Tools do not trade today, so an
    /// idle/unproductive tool adds nothing (a *productive* tool's worth already shows up as
    /// the gold it earned), so an optimist cannot hide a sunk-WOOD loss inside idle tools.
    /// The capital-selection metric: a wrong forecast ends a colonist STRICTLY LOWER here.
    /// `None` for a non-colonist / freed index.
    pub fn agent_capital(&self, index: usize) -> Option<u128> {
        let colonist = self.colonists.get(index)?;
        let agent = self.society.agents.get(colonist.id)?;
        let wood_price = self.realized_price(WOOD).map_or(0, |g| g.0);
        let mut capital = u128::from(agent.gold.0);
        capital += u128::from(agent.stock.get(WOOD)) * u128::from(wood_price);
        if let Some(chain) = &self.chain {
            for tool in [chain.content.mill(), chain.content.oven()] {
                let units = u128::from(agent.stock.get(tool));
                if units == 0 {
                    continue;
                }
                // V = the tool's realized liquidation price (0 — tools never trade today).
                let liquidation = self.realized_price(tool).map_or(0, |g| g.0);
                capital += units * u128::from(liquidation);
            }
        }
        Some(capital)
    }

    /// S11: the colony-wide total of [`Self::agent_capital`] over LIVING colonists — the
    /// aggregate capital-selection metric (an optimist colony ends strictly lower than an
    /// accurate one when the opportunity is negative-NPV at the real price).
    pub fn total_agent_capital(&self) -> u128 {
        self.live_colonist_slots
            .iter()
            .filter_map(|&slot| self.agent_capital(slot))
            .sum()
    }

    /// S11.3 (a settlement-level chain shock): disable or re-enable the BAKE stage
    /// (flour → bread) on the society's LIVE recipe set, keeping the chain's own content
    /// copy consistent — the same dual-flip [`Settlement::maybe_unlock_tier_two`] uses for
    /// the tier-2 gate. A time-boxed disable over `[A, B)` demonstrably perturbs the
    /// grain→flour→bread chain (bread output dips, since no oven can fire), letting a test
    /// drive a real shock → discoordination → recovery rather than an econ `EventKind` that
    /// may never reach the sim-side chain. Returns whether the chain carries a bake recipe
    /// (`false` for a non-chain settlement). It mutates the recipe `enabled` flag — which
    /// the digest already serializes — so it is real, conserved state (no goods are created
    /// or destroyed; production simply stops while disabled); no existing phase calls it, so
    /// every run that does not is byte-identical.
    pub fn set_bake_stage_enabled(&mut self, enabled: bool) -> bool {
        let recipe_id = match self.chain.as_ref() {
            Some(chain) => chain.content.bake_recipe().id,
            None => return false,
        };
        let found = self.society.set_recipe_enabled(recipe_id, enabled);
        if let Some(chain) = self.chain.as_mut() {
            chain.content.set_recipe_enabled(recipe_id, enabled);
        }
        found
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

    /// S21d.1 (read-only): tracked food (bread) CONSUMED so far, split by acquisition
    /// channel — the open-survival bar. After warm-up, `bought` must dominate while
    /// `seeded_minted` + `foraged` flatten near zero. Empty default off the gated path.
    pub fn acquisition_consumed_by_channel(&self) -> AcquisitionChannels {
        AcquisitionChannels::from_array(self.acquisition.consumed_by_channel)
    }

    /// C3R.c runtime-only ledger for earned producer-house provisioning. It is
    /// intentionally excluded from `canonical_bytes`; tag 29 serializes only the
    /// behavior switch, while this reports the measured revenue classes and
    /// provisioning outcomes.
    pub fn earned_provisioning_stats(&self) -> EarnedProvisioningStats {
        self.earned_provisioning.stats
    }

    pub fn birth_stock_wants_emitted(&self) -> u64 {
        self.birth_stock_wants_emitted
    }

    pub fn birth_stock_attributable_purchases(&self) -> u64 {
        self.birth_stock_attributable_purchases
    }

    pub fn birth_stock_reached_four_count(&self) -> usize {
        self.birth_stock_reached_agents.len()
    }

    pub fn birth_stock_held_max(&self) -> u32 {
        self.birth_stock_held_max
    }

    pub fn birth_stock_held_at_death(&self) -> u32 {
        self.birth_stock_held_at_death
    }

    pub fn birth_stock_eligible_opportunities(&self) -> u64 {
        self.birth_stock_eligible_opportunities
    }

    pub fn birth_stock_injections_completed(&self) -> u64 {
        self.birth_stock_injections_completed
    }

    pub fn birth_stock_source_shortfalls(&self) -> u64 {
        self.birth_stock_source_shortfalls
    }

    /// C3R.e (impl-67) (read-only): the A1 ignition dose — the total staple quantity the one-shot
    /// injection moved at `birth_stock_ignition_at`. `< 24` (= 6 × `child_food_endowment`) is an
    /// under-dose → `IgnitionShortfall`. `0` off the A1 path.
    pub fn ignition_injected_qty(&self) -> u64 {
        self.ignition_injected_qty
    }

    /// C3R.e debt repair: the one-shot ignition's gate decomposition
    /// (interval, extinct, cap, hunger, at-target, donor-shortfall). Runtime-only.
    pub fn ignition_gate_decomposition(&self) -> [u64; 6] {
        [
            self.ignition_gate_blocked_interval,
            self.ignition_gate_extinct,
            self.ignition_gate_blocked_cap,
            self.ignition_gate_blocked_hunger,
            self.ignition_gate_suppressed_at_target,
            self.ignition_gate_donor_shortfall,
        ]
    }

    /// C3R.e (impl-67) (read-only): cumulative producer-house birth funding split by the drawn
    /// endowment lots' acquisition channel. Window-diffed, criterion iii reads market funding
    /// (`bought + self_produced`) against non-market (`seeded_minted + foraged + commons`).
    pub fn producer_birth_funded_by_channel(&self) -> AcquisitionChannels {
        AcquisitionChannels::from_array(self.producer_birth_funded_by_channel)
    }

    /// C3R.e (impl-67) (read-only): cumulative producer-house birth funding drawn from
    /// INTERVENTION-ORIGIN lots. Window-diffed, criterion iii requires this flat (a birth paid
    /// for with subsidy residue is not market-funded). `0` off the path.
    pub fn producer_birth_funded_intervention(&self) -> u64 {
        self.producer_birth_funded_intervention
    }

    pub fn birth_stock_injection_records(&self) -> &[BirthStockInjectionRecord] {
        &self.birth_stock_injection_records
    }

    pub fn births_by_household(&self) -> &[u64] {
        &self.birth_stock_births_by_household
    }

    /// S21d.1 (read-only): tracked food (bread) currently HELD across living agents, split by
    /// channel — the live channel mix and the seed-depletion read (`seeded_minted → 0`).
    pub fn acquisition_held_by_channel(&self) -> AcquisitionChannels {
        AcquisitionChannels::from_array(self.acquisition.held_by_channel())
    }

    /// C3R.e (impl-67) (read-only): the GLOBAL intervention-origin exhaustion read — total
    /// intervention-flagged tracked food held across every living agent and channel. Criterion ii
    /// (`ResidualExhausted`) is exactly `== 0`. Resale-proof (the origin flag survives the market
    /// retag). `0` off the acquisition-ledger path.
    pub fn acquisition_intervention_held(&self) -> u64 {
        self.acquisition.intervention_held()
    }

    /// C3R.e (impl-67) (read-only): the PRODUCER-COHORT intervention-origin held total — the
    /// intervention-flagged tracked food held by living producer-household members. A subset of
    /// [`Self::acquisition_intervention_held`], surfaced beside it for cohort-level diagnosis.
    pub fn producer_intervention_held(&self) -> u64 {
        let cohort: BTreeSet<AgentId> = self
            .colonists
            .iter()
            .filter(|colonist| {
                colonist.alive
                    && colonist
                        .household
                        .is_some_and(|household| self.is_producer_household(household))
            })
            .map(|colonist| colonist.id)
            .collect();
        self.acquisition.intervention_held_by(&cohort)
    }

    /// S21h.1 (read-only): tracked food (bread) currently HELD by the living non-lineage
    /// demand-side roles, split by channel. In the emergency-provision scenario, these roles
    /// never cultivate; any `SelfProduced` units held here would therefore be emergency bread
    /// that was not immediately consumed.
    pub fn non_lineage_acquisition_held_by_channel(&self) -> AcquisitionChannels {
        let mut held = [0u64; FoodChannel::COUNT];
        for colonist in &self.colonists {
            if !colonist.alive || colonist.household.is_some() {
                continue;
            }
            let by_channel = self.acquisition.held_by_agent(colonist.id);
            for channel in FoodChannel::ALL {
                held[channel.index()] += by_channel[channel.index()];
            }
        }
        AcquisitionChannels::from_array(held)
    }

    /// S21d.1 (read-only): tracked food (bread) ever CREDITED (entered stock) per channel — the
    /// inflow totals (e.g. how much seeded/minted food ever entered vs how much bought).
    pub fn acquisition_credited_by_channel(&self) -> AcquisitionChannels {
        AcquisitionChannels::from_array(self.acquisition.credited_by_channel)
    }

    /// S21h.0 (read-only): cumulative `SeededMinted` bread units SOLD FOR SALT over the run —
    /// the share of any bread→SALT sale (pre-promotion barter for SALT, or post-promotion
    /// spot sale once SALT is money) drawn FIFO from the seller's seeded/cushion lots. The
    /// hard demand-bridge invariant: this must stay `0` on every cushion cell, so SALT can
    /// monetize ONLY on the lineage's `SelfProduced` bread, never on cushion (`SeededMinted`)
    /// bread (else the cell is a seeded-supply result, not a demand-survival one). `0` off the
    /// acquisition-ledger path. Runtime-only; not digested.
    pub fn seeded_minted_bread_sold_for_salt(&self) -> u64 {
        self.seeded_minted_bread_sold_for_salt
    }

    /// S21h.1 (read-only): cumulative emergency-provisioned bread (produced == immediately
    /// eaten) over the run — the non-lineage roles' own-labor survival floor. The
    /// demand-preservation test compares it against bought food: the floor must be the
    /// SURVIVAL minimum, not the bulk of the demand side's diet (post-promotion their food is
    /// materially bought). `0` off the emergency seam. Runtime-only; not digested.
    pub fn emergency_bread_provisioned(&self) -> u64 {
        self.emergency_bread_provisioned
    }

    /// S21h (read-only): the number of LIVING NON-LINEAGE roles (`household_of` is `None` —
    /// the SALT-rich buyers + the woodcutters) whose freshly regenerated value scale carries
    /// at least one present (`Horizon::Now`, `qty > 0`) bread want — the **demand-survival
    /// probe**. The food-want ladder is rebuilt each tick from current hunger, so a role
    /// satiated to hunger 0 emits ZERO `Now` bread wants: a non-zero count means the demand
    /// side both survives AND still demands bread (the S21h window), while `0` means the
    /// demand side is dead or sated out of the market. Reads the live scales only; not
    /// digested. `bread` is the chain's bread good ([`Self::bread_good`]).
    pub fn living_non_lineage_with_bread_now_wants(&self, bread: GoodId) -> usize {
        (0..self.colonists.len())
            .filter(|&index| {
                let colonist = &self.colonists[index];
                if !colonist.alive || colonist.household.is_some() {
                    return false;
                }
                self.society.agents.get(colonist.id).is_some_and(|agent| {
                    agent.scale.iter().any(|want| {
                        want.kind == WantKind::Good(bread)
                            && matches!(want.horizon, Horizon::Now)
                            && want.qty > 0
                    })
                })
            })
            .count()
    }

    /// S21d.2a (read-only): the cross-tick bootstrap microtrace summary — the buy → eat → bid
    /// sequence and the post-market block breakdown that localizes the Exp-9 gate. Empty default
    /// off the path.
    pub fn bootstrap_trace_summary(&self) -> BootstrapTraceSummary {
        BootstrapTraceSummary {
            food_buys: self.bootstrap_trace.food_buys,
            food_eats: self.bootstrap_trace.food_eats,
            bid_attempts: self.bootstrap_trace.bid_attempts,
            bids_posted: self.bootstrap_trace.bids_posted,
            bids_posted_after_recent_buy: self.bootstrap_trace.bids_posted_after_recent_buy,
            bids_blocked_cashless: self.bootstrap_trace.bids_blocked_cashless,
            bids_blocked_reserved: self.bootstrap_trace.bids_blocked_reserved,
            first_bootstrap_bid_tick: self.bootstrap_trace.first_bootstrap_bid_tick,
        }
    }

    /// S21e.0 (read-only): realized bread-seller provenance rows from barter
    /// trades. Runtime-only diagnostic, excluded from canonical bytes.
    pub fn bread_seller_provenance(&self) -> &[BreadSellerProvenance] {
        &self.bread_seller_trace
    }

    /// S21e.1 (read-only): finite seeded-surplus non-vacuity and exhaustion trace.
    /// Runtime-only diagnostic, excluded from canonical bytes.
    pub fn seeded_surplus_trace_summary(&self) -> SeededSurplusTraceSummary {
        SeededSurplusTraceSummary {
            max_pre_promotion_seeded_sellers: self
                .seeded_surplus_trace
                .max_pre_promotion_seeded_sellers,
            first_non_vacuous_tick: self.seeded_surplus_trace.first_non_vacuous_tick,
            cleared_bread_salt_indirect_for_wood: self
                .seeded_surplus_trace
                .cleared_bread_salt_indirect_for_wood,
            live_bread_salt_indirect_for_wood_ticks: self
                .seeded_surplus_trace
                .live_bread_salt_indirect_for_wood_ticks,
            seeded_offerable_surplus_exhausted_tick: self
                .seeded_surplus_trace
                .seeded_offerable_surplus_exhausted_tick,
        }
    }

    /// Diagnostic (game-only, read-only): total `gold` held by living colonists
    /// grouped by [`Vocation`] — the probe for the producer-working-capital
    /// hypothesis (do the chain producers cash-starve while the saving
    /// households accumulate, halting the chain for lack of working capital?).
    /// Returns `(vocation, total_gold)` pairs in living-roster encounter order;
    /// reads only, so the econ goldens are untouched.
    pub fn gold_by_vocation(&self) -> Vec<(Vocation, u64)> {
        let mut out: Vec<(Vocation, u64)> = Vec::new();
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            let Some(agent) = self.society.agents.get(colonist.id) else {
                continue;
            };
            let gold = agent.gold.0;
            if let Some(entry) = out.iter_mut().find(|(voc, _)| *voc == colonist.vocation) {
                entry.1 = entry.1.saturating_add(gold);
            } else {
                out.push((colonist.vocation, gold));
            }
        }
        out
    }

    /// Diagnostic (game-only, read-only): living colonists' holdings of `good`
    /// grouped by [`Vocation`] — the market-gate probe for the downstream halt
    /// (does raw grain pile up with the gatherers/savers who won't sell it while
    /// the millers hold none, i.e. supply withdrawal, vs. consumers sitting on
    /// bread they don't bid for?). Returns `(vocation, total_qty)` pairs.
    pub fn stock_by_vocation(&self, good: GoodId) -> Vec<(Vocation, u64)> {
        let mut out: Vec<(Vocation, u64)> = Vec::new();
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            let Some(agent) = self.society.agents.get(colonist.id) else {
                continue;
            };
            let qty = u64::from(agent.stock.get(good));
            if let Some(entry) = out.iter_mut().find(|(voc, _)| *voc == colonist.vocation) {
                entry.1 = entry.1.saturating_add(qty);
            } else {
                out.push((colonist.vocation, qty));
            }
        }
        out
    }

    /// Diagnostic (read-only): the live money BID/ASK intent for `good` grouped by
    /// [`Vocation`] — for each living colonist, the single-unit order it *would*
    /// post is reconstructed from `Agent::reservation_bid_for_money` /
    /// `reservation_ask_for_money` (the same pure functions the market uses). This
    /// is the live order-book probe the stock/gold trace could not give: it
    /// distinguishes "miller posts no grain bid" (producer-side gate) from
    /// "miller bids but no gatherer asks" (seller-side gate) from "both post but
    /// don't cross" (price/spread gate). Empty before money emerges.
    pub fn order_stats_by_vocation(&self, good: GoodId) -> Vec<OrderStat> {
        let Some(money) = self.society.current_money_good() else {
            return Vec::new();
        };
        let mut out: Vec<OrderStat> = Vec::new();
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            let Some(agent) = self.society.agents.get(colonist.id) else {
                continue;
            };
            let bid = agent.reservation_bid_for_money(good, 1, money).map(|g| g.0);
            let ask = agent.reservation_ask_for_money(good, 1, money).map(|g| g.0);
            let stat = match out.iter_mut().find(|s| s.vocation == colonist.vocation) {
                Some(stat) => stat,
                None => {
                    out.push(OrderStat {
                        vocation: colonist.vocation,
                        bidders: 0,
                        best_bid: None,
                        askers: 0,
                        best_ask: None,
                    });
                    out.last_mut().expect("just pushed")
                }
            };
            if let Some(bid) = bid {
                stat.bidders += 1;
                stat.best_bid = Some(stat.best_bid.map_or(bid, |b| b.max(bid)));
            }
            if let Some(ask) = ask {
                stat.askers += 1;
                stat.best_ask = Some(stat.best_ask.map_or(ask, |a| a.min(ask)));
            }
        }
        out
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

    /// S8.0 emergence probe (read-only): the barter saleability accumulated by each
    /// money candidate — its acceptances and the count of distinct acceptors and
    /// counterpart goods behind them. This is the breadth+volume econ's Mengerian
    /// promotion rule reads; empty for a non-emergent (designated-money) settlement.
    /// Reads only, so the goldens are untouched.
    pub fn emergence_acceptances(&self) -> Vec<CandidateAcceptances> {
        let Some(emergence) = self.society.emergence() else {
            return Vec::new();
        };
        emergence
            .tracker()
            .candidate_saleability()
            .map(|c| CandidateAcceptances {
                good: c.good,
                acceptances: c.acceptances,
                acceptor_agents: c.acceptor_agents.len(),
                counterpart_goods: c.counterpart_goods.len(),
                indirect_acceptances: c.indirect_acceptances,
                indirect_acceptor_agents: c.indirect_acceptor_agents.len(),
                indirect_target_goods: c.indirect_target_goods.len(),
            })
            .collect()
    }

    /// S19 (read-only): by-good acceptance split derived from the existing emergence
    /// tracker. `direct` is the ordinary direct-want saleability count and `indirect`
    /// is the `IndirectFor{target}` count the strong-bar gate uses.
    pub fn direct_indirect_acceptances(&self) -> Vec<DirectIndirectAcceptances> {
        self.emergence_acceptances()
            .into_iter()
            .map(|c| DirectIndirectAcceptances {
                good: c.good,
                total: c.acceptances,
                direct: c.acceptances.saturating_sub(c.indirect_acceptances),
                indirect: c.indirect_acceptances,
            })
            .collect()
    }

    /// S18 (read-only): the DISTINCT indirect target goods `good` has accrued as a
    /// saleability candidate — the `IndirectFor{target}` MEMBERSHIP the strong-bar gate
    /// counts (`min_indirect_target_goods`) but [`Self::emergence_acceptances`] collapses to
    /// a count. Surfaces the `&[GoodId]` membership (`menger.rs`) so the DoD can assert the
    /// two-sided produced breadth `indirect_target_goods(SALT) ⊇ {bread, WOOD}` — bread
    /// sellers `IndirectFor{WOOD}` AND woodcutters `IndirectFor{bread}`. Sorted; empty for a
    /// non-emergent settlement or an untracked good. Reads only.
    pub fn indirect_target_goods(&self, good: GoodId) -> Vec<GoodId> {
        let Some(emergence) = self.society.emergence() else {
            return Vec::new();
        };
        emergence
            .tracker()
            .candidate_saleability()
            .find(|candidate| candidate.good == good)
            .map(|candidate| candidate.indirect_target_goods.to_vec())
            .unwrap_or_default()
    }

    /// S16 causality probe (read-only): whether the chain **bread** is among the **medium**
    /// candidate's saleability counterpart goods — i.e. bread is materially traded against
    /// the medium in the saleability the promotion gate reads, not incidental. Distinguishes
    /// the third outcome (the medium promotes on WOOD/forage breadth with bread incidental)
    /// from a real produced-bread channel. `false` for a settlement with no medium/chain.
    /// Reads only.
    pub fn bread_in_medium_saleability(&self) -> bool {
        let (Some((medium, _)), Some(content), Some(emergence)) =
            (self.barter_medium, self.content(), self.society.emergence())
        else {
            return false;
        };
        let bread = content.bread();
        emergence
            .tracker()
            .candidate_saleability()
            .find(|candidate| candidate.good == medium)
            .is_some_and(|candidate| candidate.counterpart_goods.contains(&bread))
    }

    /// S8.0 emergence probe (read-only): the working capital of every living chain
    /// producer — active (Miller/Baker) and latent (an `Unassigned` colonist holding
    /// a mill/oven, distinguished into latent Miller/Baker by its latent recipe). For
    /// each: the barter MEDIUM it holds (SALT, its pre-promotion working capital), its
    /// GOLD, and its FREE (non-reserved) gold. This is the Tension-B probe: read right
    /// after the promotion tick, a latent producer's gold is its converted-SALT
    /// capital and `free_gold` is what it can put behind its input bid before the
    /// chain freezes. Empty for a settlement with no production chain. Reads only.
    pub fn producer_cash(&self) -> Vec<ProducerCash> {
        let medium_good = self.barter_medium.map(|(good, _)| good);
        let mut out = Vec::new();
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            let role = match (colonist.vocation, colonist.latent) {
                (Vocation::Miller, _) => ProducerRole::Miller,
                (Vocation::Baker, _) => ProducerRole::Baker,
                (Vocation::Unassigned, Some(RecipeId::Mill)) => ProducerRole::LatentMiller,
                (Vocation::Unassigned, Some(RecipeId::Bake)) => ProducerRole::LatentBaker,
                _ => continue,
            };
            let Some(agent) = self.society.agents.get(colonist.id) else {
                continue;
            };
            let medium = medium_good
                .map(|good| u64::from(agent.stock.get(good)))
                .unwrap_or(0);
            out.push(ProducerCash {
                role,
                medium,
                gold: agent.gold.0,
                free_gold: self.society.free_gold_after_all_reserves(colonist.id).0,
            });
        }
        out
    }

    /// S8.0/S16 emergence probe (read-only): the cumulative units of the chain staple
    /// (bread) exchanged against the barter MEDIUM (SALT) over the run — the
    /// bread-for-SALT leg of indirect exchange that can monetize SALT. Pre-promotion
    /// volume is derived from the retained barter-trade log (which survives the
    /// promotion barter-book wipe); if the configured medium later promotes, post-
    /// promotion bread spot sales are included too because the money phase represents
    /// those as bread-for-current-money trades. `0` for a settlement with no chain or
    /// no barter medium. Reads only.
    pub fn bread_for_salt_volume(&self) -> u64 {
        let (Some((medium, _)), Some(content)) = (self.barter_medium, self.content()) else {
            return 0;
        };
        let staple = content.bread();
        let barter_volume: u64 = self
            .society
            .barter_trades
            .iter()
            .filter(|trade| {
                (trade.a_gives == staple && trade.b_gives == medium)
                    || (trade.a_gives == medium && trade.b_gives == staple)
            })
            .map(|trade| u64::from(trade.qty))
            .sum();
        let spot_volume: u64 = if self.society.current_money_good() == Some(medium) {
            self.society
                .trades
                .iter()
                .filter(|trade| trade.good == staple)
                .map(|trade| u64::from(trade.qty))
                .sum()
        } else {
            0
        };
        barter_volume + spot_volume
    }

    /// S16 — the produced-bread provenance trace (read-only): the cumulative bread→medium
    /// trade volume split by STOCK ORIGIN — `(produced, minted)`. `produced` is the share
    /// the provenance ledger drew from cultivators' (or chain bakers') produced-bread
    /// balance; `minted` is the residual (seeded buffer / a hearth mint). Their sum equals
    /// [`Self::bread_for_salt_volume`]. This is the proof that closes the S12 caveat:
    /// whether the bread that monetizes the medium is produced (the S16 claim) or minted.
    /// `(0, 0)` off the money-from-produced-bread path. Reads only.
    pub fn bread_for_salt_volume_by_provenance(&self) -> (u64, u64) {
        (
            self.bread_provenance.salt_volume_produced,
            self.bread_provenance.salt_volume_minted,
        )
    }

    /// S16 (read-only): the same produced/minted bread→medium split, accumulated only over
    /// PRE-promotion ticks (frozen at the promotion tick, inclusive) — the causality probe
    /// for whether produced bread was material in the volume that fired (or failed to fire)
    /// the promotion. `(0, 0)` off the path.
    pub fn pre_promotion_bread_for_salt_by_provenance(&self) -> (u64, u64) {
        (
            self.bread_provenance.pre_promotion_salt_volume_produced,
            self.bread_provenance.pre_promotion_salt_volume_minted,
        )
    }

    /// S22a (read-only): the produced bread→SALT volume split by the PRODUCER's class recorded
    /// at PRODUCTION time (spatial lineage vs non-lineage entrant), plus the distinct producers
    /// per class. The entrant-class provenance — whether the food-producing class that
    /// monetized SALT formed from the pinned lineage or self-formed from non-lineage agents
    /// under hunger. Attribution is by who CULTIVATED the bread (the lot tag), NOT the seller's
    /// `cultivating` state at trade time (a cultivated loaf may sell a later tick or after
    /// transfer). All-zero off the produced-bread provenance path. Runtime-only; not digested.
    pub fn bread_for_salt_by_entrant_class(&self) -> EntrantClassSale {
        let bp = &self.bread_provenance;
        EntrantClassSale {
            lineage_volume: bp.salt_volume_produced_lineage,
            nonlineage_volume: bp.salt_volume_produced_nonlineage,
            pre_promotion_lineage_volume: bp.pre_promotion_salt_volume_produced_lineage,
            pre_promotion_nonlineage_volume: bp.pre_promotion_salt_volume_produced_nonlineage,
            lineage_sellers: bp.lineage_salt_producers.len(),
            nonlineage_sellers: bp.nonlineage_salt_producers.len(),
        }
    }

    /// S16 (read-only): total produced-origin bread currently held across living agents —
    /// the produced surplus inventory. `0` off the path.
    pub fn produced_bread_held(&self) -> u64 {
        self.bread_provenance.total_held()
    }

    /// S18 (read-only): the cumulative WOOD↔medium (SALT) trade volume — the WOOD leg of the
    /// indirect exchange (the woodcutters selling WOOD for the medium). `0` off the
    /// multi-good money path. Reads only.
    pub fn wood_for_salt_volume(&self) -> u64 {
        self.multigood.wood_for_salt
    }

    /// S18 (read-only): the same WOOD↔medium volume accumulated only over PRE-promotion
    /// ticks (frozen at the promotion tick, inclusive) — the WOOD volume that drove (or
    /// failed to drive) the promotion. `0` off the path. Reads only.
    pub fn pre_promotion_wood_for_salt_volume(&self) -> u64 {
        self.multigood.pre_promotion_wood_for_salt
    }

    /// S18 (read-only): the cumulative WOOD relocated node→econ over the run — the gather
    /// bound the total WOOD stock can never exceed (with every WOOD buffer + the mint zeroed,
    /// all WOOD enters here), so the salt-leg volume `pre_promotion_wood_for_salt` cannot
    /// exceed it (the buyer burns purchased WOOD for warmth — no recirculation). The WOOD
    /// provenance proof: traded WOOD is gathered, not minted. `0` off the path. Reads only.
    pub fn wood_gathered_total(&self) -> u64 {
        self.multigood.wood_gathered
    }

    /// S18 (read-only): the traced pending-indirect-SALT round-trip totals `(spent, accepted)`
    /// — of the medium accepted `IndirectFor{target}` (`accepted`), the share later SPENT on
    /// that same target (`spent`, the means role completing). The round-trip metric
    /// (`spent / accepted`) is the proof that the medium actually intermediates, not just
    /// pools on one side — `accepted > 0 && spent ≈ 0` is the hoarding failure. Maintained
    /// for any emergent barter medium. Reads only.
    pub fn salt_round_trip(&self) -> (u64, u64) {
        (
            self.multigood.indirect_spent_on_target,
            self.multigood.indirect_accepted,
        )
    }

    /// S18 (read-only): the round-trip fraction in basis points (`spent / accepted`). `0`
    /// when nothing was accepted indirectly (no division by zero). Post-promotion spot
    /// spends are target-good quantities capped by pending medium, so this is conservative
    /// rather than an exact price-denominated SALT ratio. See [`Self::salt_round_trip`].
    pub fn salt_round_trip_fraction_bps(&self) -> u32 {
        self.multigood.round_trip_fraction_bps()
    }

    /// S18 (read-only): the standing pending-indirect medium the agent at generation `index`
    /// still holds earmarked for `target` (accepted as a means, not yet spent on it). `0`
    /// off the path or for an unknown agent. Reads only.
    pub fn pending_indirect_salt(&self, index: usize, target: GoodId) -> u64 {
        self.colonists
            .get(index)
            .map(|c| self.multigood.pending_of(c.id, target))
            .unwrap_or(0)
    }

    /// S16 (read-only): the provenance conservation accumulators `(credited, sunk)` —
    /// produced bread ever booked by a production event, and ever removed by a true sink
    /// (eaten/spoiled/estate→commons). `credited == sunk + produced_bread_held()`.
    pub fn produced_bread_credited_and_sunk(&self) -> (u64, u64) {
        (
            self.bread_provenance.produced_credited,
            self.bread_provenance.produced_sunk,
        )
    }

    /// S16 (read-only): the first econ tick a produced surplus was held, and the first econ
    /// tick a produced bread→medium trade cleared — the instrumentation the DoD reports.
    pub fn first_produced_surplus_tick(&self) -> Option<u64> {
        self.bread_provenance.first_produced_surplus_tick
    }

    /// See [`Self::first_produced_surplus_tick`].
    pub fn first_produced_bread_for_salt_tick(&self) -> Option<u64> {
        self.bread_provenance.first_produced_bread_for_salt_tick
    }

    /// S8.0 emergence probe (read-only): the highest hunger any living colonist
    /// reached while the colony was still in barter, before any money promoted — the
    /// starvation pressure the emergence window has to survive (Tension A). Frozen at
    /// the promotion tick; for a settlement that never promotes it tracks the whole
    /// run. Diagnostic only, never serialized.
    pub fn peak_pre_promotion_hunger(&self) -> u16 {
        self.peak_pre_promotion_hunger
    }

    /// S8.0 emergence probe (read-only): the count of pre-promotion econ ticks on
    /// which at least one living colonist sat at or above the critical-hunger ceiling
    /// — the depth of the hunger trough the colony crosses before money emerges
    /// (Tension A). Frozen at the promotion tick. Diagnostic only, never serialized.
    pub fn critical_ticks_before_promotion(&self) -> u64 {
        self.critical_ticks_pre_promotion
    }
}

/// Draw a colonist's culture from the world-generation `Rng` only: time
/// preference within a small band above the vocation's base, and a leisure
/// weight in a fixed band. Mirrors `life::Camp::draw_culture` so the same
/// determinism discipline holds.
///
/// S11: the entrepreneurial **forecast bias** is jittered around `forecast_bias_base`
/// **without drawing any extra `Rng`** — it is a deterministic SplitMix of the two
/// values just drawn (so it varies per colonist) and a fixed salt. Drawing from the
/// `Rng` here would shift every later draw and break the byte-identical goldens; this
/// keeps the `Rng` sequence (and thus a flag-off run) bit-for-bit unchanged while still
/// giving each colonist its own heritable optimism. The result is clamped to
/// `5_000..=20_000` by `CultureParams::new_with_forecast_bias`. With `forecast_bias_base
/// == 10_000` and the jitter band, biases land symmetrically around ×1.0.
fn draw_culture(
    rng: &mut Rng,
    time_preference_base: u16,
    leisure_base: u16,
    forecast_bias_base: u16,
    forecast_bias_jitter: u16,
) -> CultureParams {
    let span = u16::try_from(rng.next_u64() % 500).unwrap_or(0);
    let time_preference_bps = time_preference_base.saturating_add(span);
    let leisure_weight_bps =
        leisure_base.saturating_add(u16::try_from(rng.next_u64() % 1_001).unwrap_or(0));
    let forecast_bias_bps = jitter_forecast_bias(
        forecast_bias_base,
        forecast_bias_jitter,
        time_preference_bps,
        leisure_weight_bps,
    );
    CultureParams::new_with_forecast_bias(
        time_preference_bps,
        leisure_weight_bps,
        forecast_bias_bps,
    )
}

/// S11: a deterministic per-colonist forecast-bias draw — `base` jittered by up to
/// ±`jitter` bps, derived from a SplitMix of the colonist's just-drawn
/// time-preference/leisure values (so it is heterogeneous across colonists) and a fixed
/// salt. `jitter == 0` returns `base` for every colonist (a UNIFORM colony — the
/// controlled-microtest path). Pure integer, draws no `Rng` (preserving the generation
/// sequence). The caller's `new_with_forecast_bias` re-clamps to `5_000..=20_000`.
fn jitter_forecast_bias(base: u16, jitter: u16, tp_bps: u16, leisure_bps: u16) -> u16 {
    if jitter == 0 {
        return base;
    }
    let span = u64::from(jitter) * 2 + 1;
    let seed = u64::from(tp_bps) ^ (u64::from(leisure_bps) << 20) ^ FORECAST_BIAS_GEN_SALT;
    let draw = deterministic_mix64(seed) % span;
    let delta = draw as i64 - i64::from(jitter);
    i64::from(base)
        .saturating_add(delta)
        .clamp(0, i64::from(u16::MAX)) as u16
}

/// S11: the generation-time forecast-bias jitter salt (distinct from the inheritance
/// salt in `life::culture`) — fixes the deterministic per-colonist draw.
const FORECAST_BIAS_GEN_SALT: u64 = 0x00f0_8ca5_9e11_7e57;

/// S11: the default half-width (bps) of the generation forecast-bias jitter band — wide
/// enough that a neutral-base colony spans optimists and accurate forecasters (the
/// selection substrate). A controlled microtest overrides it to `0` for a uniform colony.
const FORECAST_BIAS_GEN_JITTER_DEFAULT: u16 = 4_000;

/// S11: the agent's adaptive [`PriceBelief`] for `good`, or the neutral default belief
/// (which is NOT observed) if the agent has no slot for the good — so a never-seen good
/// falls back to the public realized price in [`forecast_output_price`].
fn agent_belief(agent: &Agent, good: GoodId) -> PriceBelief {
    agent
        .expect
        .get(usize::from(good.0))
        .copied()
        .unwrap_or_else(|| PriceBelief::new(Gold(1), Gold(1)))
}

/// S11: the per-agent **grounded fallible forecast** of `good`'s OUTPUT price — the
/// entrepreneurial estimate the role-choice / capital-build / input-bid appraisals weigh
/// instead of the raw realized price when `entrepreneurial_forecasts` is on.
///
/// `forecast = base × bias_bps / 10_000`, where `base` is GROUNDED (never the cold-start
/// neutral belief default): the agent's own belief `expected` ONLY once it has actually
/// observed the good (`belief.observed` — distinct from `last_seen == 0`, so a tick-0
/// observation still grounds on the belief), else the public `realized` price; if neither
/// exists (no trade ever cleared and the agent never observed) there is no forecast and
/// the decision is skipped (`None`), exactly as today with a missing realized price.
///
/// The bias is a *standing* multiplier, so a biased agent systematically over/under-shoots
/// even as its belief tracks the realized level — forecasting under uncertainty, never
/// clairvoyance. Pure integer + deterministic (belief + bias are digested state).
fn forecast_output_price(
    agent: &Agent,
    good: GoodId,
    realized: Option<Gold>,
    bias_bps: u16,
) -> Option<Gold> {
    let belief = agent_belief(agent, good);
    let base = if belief.observed {
        belief.expected
    } else {
        realized?
    };
    let forecast =
        base.0.saturating_mul(u64::from(bias_bps)) / u64::from(FORECAST_BIAS_NEUTRAL_BPS);
    Some(Gold(forecast))
}

/// S21e.0: the bread-seller classes observed in the mints-on control: latent chain
/// agents (`Unassigned`, non-household) and demographic household consumers. The
/// seeded-surplus probe adds finite bread only here.
fn seeded_surplus_seller_class(vocation: Vocation, household: Option<usize>) -> bool {
    matches!(
        (vocation, household),
        (Vocation::Unassigned, None) | (Vocation::Consumer, Some(_))
    )
}

/// C3R.e (impl-67): a producer-house SUBJECT for the A2 additive endowment and its bootstrap-sweep
/// origin-flag split — the seeded or latent producers (miller/baker/cycle), mirroring
/// [`Settlement::run_producer_subsistence`]'s producer gate exactly. Consumers and gatherers are
/// excluded, so the endowment lands only on the chain's producers.
fn is_producer_subject_vocation(vocation: Vocation, latent: Option<RecipeId>) -> bool {
    matches!(
        vocation,
        Vocation::Miller | Vocation::Baker | Vocation::CycleA | Vocation::CycleB | Vocation::CycleC
    ) || (vocation == Vocation::Unassigned && latent.is_some())
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
                // S21h.0: the non-lineage woodcutters (Gatherers are built here; lineage
                // members go through `build_demography_agent`) carry the dedicated
                // `gatherer_food_cushion` ADDED ON TOP of the shared `bread_buffer` they would
                // otherwise get from the `_ =>` arm — a dedicated knob so the demand-side
                // survival cushion can be raised without disturbing `bread_buffer` (which
                // seeds the chain's other non-consumer roles). `0` for every existing config,
                // so a gatherer's starting bread is exactly `bread_buffer` and the run is
                // byte-identical; on the S21h scenarios `bread_buffer == 0`, so the cushion is
                // the gatherer's only bread.
                Vocation::Gatherer => (
                    chain
                        .bread_buffer
                        .saturating_add(chain.gatherer_food_cushion),
                    chain.wood_buffer,
                ),
                _ => (chain.bread_buffer, chain.wood_buffer),
            };
            stock.add(staple, staple_buffer);
            // C3R.e (impl-67): the A2 additive endowment — a producer-house SUBJECT (seeded/latent
            // producer) starts with an extra `producer_house_starting_staple` of the staple, raising
            // aggregate bread. Split out of the tick-0 bootstrap sweep as origin-flagged lots
            // (`maybe_init_acquisition_ledger`), keyed on the SAME producer-subject predicate. `0`
            // for every non-A2 config, so no stock is added and the run is byte-identical.
            if chain.producer_house_starting_staple > 0
                && is_producer_subject_vocation(vocation, latent)
            {
                stock.add(staple, chain.producer_house_starting_staple);
            }
            if chain.seeded_surplus_bread > 0 && seeded_surplus_seller_class(vocation, None) {
                stock.add(chain.content.bread(), chain.seeded_surplus_bread);
            }
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
                Vocation::CycleA => {
                    stock.add(
                        chain
                            .content
                            .cycle_a_tool()
                            .expect("cycle role A requires cycle content"),
                        1,
                    );
                    stock.add(
                        chain
                            .content
                            .cycle_z()
                            .expect("cycle role A requires Z input"),
                        chain.cycle_a_input_buffer,
                    );
                    chain.producer_gold
                }
                Vocation::CycleB => {
                    stock.add(
                        chain
                            .content
                            .cycle_b_tool()
                            .expect("cycle role B requires cycle content"),
                        1,
                    );
                    stock.add(
                        chain
                            .content
                            .cycle_x()
                            .expect("cycle role B requires X input"),
                        chain.cycle_b_input_buffer,
                    );
                    chain.producer_gold
                }
                Vocation::CycleC => {
                    stock.add(
                        chain
                            .content
                            .cycle_c_tool()
                            .expect("cycle role C requires cycle content"),
                        1,
                    );
                    stock.add(
                        chain
                            .content
                            .cycle_y()
                            .expect("cycle role C requires Y input"),
                        chain.cycle_c_input_buffer,
                    );
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
                | Vocation::Confectioner
                | Vocation::CycleA
                | Vocation::CycleB
                | Vocation::CycleC => {
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
    // code, which only reached the `None` branch). S19 additionally lets the cycle
    // producers hold a small neutral SALT commodity balance so their derived input
    // demand can spend it before any money good exists.
    if let Some(barter) = &config.barter {
        let medium = match vocation {
            Vocation::Gatherer => barter.gatherer_medium_endowment,
            Vocation::Consumer => barter.consumer_medium_endowment,
            Vocation::CycleA | Vocation::CycleB | Vocation::CycleC => {
                barter.cycle_producer_medium_endowment
            }
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

/// S14: whether the forage-commons path is active for this *config* — the
/// generation-time analogue of [`Settlement::forage_commons_active`] (no `self` yet).
/// Own-labor subsistence can run (the flag + a forage good) AND a [`ForageCommons`] is
/// set. Off (every existing config), founders seed the hunger staple, byte-identical.
fn config_forage_commons_active(config: &SettlementConfig) -> bool {
    config
        .chain
        .as_ref()
        .is_some_and(chain_config_forage_commons_active)
}

fn config_mortal_chain_producers_active(config: &SettlementConfig) -> bool {
    config.demography.is_some()
        && config
            .chain
            .as_ref()
            .is_some_and(chain_config_mortal_chain_producers_active)
}

fn config_mortal_producer_inheritance_active(config: &SettlementConfig) -> bool {
    config.demography.is_some()
        && config
            .chain
            .as_ref()
            .is_some_and(chain_config_mortal_producer_inheritance_active)
}

fn own_labor_subsistence_fields_active(own_labor_subsistence: bool, forage_present: bool) -> bool {
    own_labor_subsistence && forage_present
}

fn forage_commons_fields_active(
    own_labor_subsistence: bool,
    forage_present: bool,
    forage_commons_present: bool,
) -> bool {
    own_labor_subsistence_fields_active(own_labor_subsistence, forage_present)
        && forage_commons_present
}

fn chain_config_forage_commons_active(chain: &ChainConfig) -> bool {
    forage_commons_fields_active(
        chain.own_labor_subsistence,
        chain.content.forage().is_some(),
        chain.forage_commons.is_some(),
    )
}

fn chain_config_own_use_cultivation_active(chain: &ChainConfig) -> bool {
    chain.own_use_cultivation
        && chain.content.cultivate_recipe().is_some()
        && (own_labor_subsistence_fields_active(
            chain.own_labor_subsistence,
            chain.content.forage().is_some(),
        ) || chain.household_barter_cultivation)
}

fn chain_config_cultivation_sells_surplus_active(chain: &ChainConfig) -> bool {
    chain.cultivation_sells_surplus && chain_config_own_use_cultivation_active(chain)
}

fn inheritance_regime_tag(regime: InheritanceRegime) -> u8 {
    match regime {
        InheritanceRegime::Impartible => 0,
        InheritanceRegime::Partible => 1,
    }
}

fn wage_labor_mode_tag(mode: WageLaborMode) -> u8 {
    match mode {
        WageLaborMode::Voluntary => 0,
        WageLaborMode::FiatWage => 1,
        WageLaborMode::SubsidisedWage => 2,
    }
}

fn share_tenancy_mode_tag(mode: ShareTenancyMode) -> u8 {
    match mode {
        ShareTenancyMode::Voluntary => 0,
        ShareTenancyMode::ForcedShare => 1,
        ShareTenancyMode::LineageWorker => 2,
    }
}

fn birth_stock_saving_mode_tag(mode: BirthStockSavingMode) -> u8 {
    match mode {
        BirthStockSavingMode::Off => 0,
        BirthStockSavingMode::Motive => 1,
        BirthStockSavingMode::SufficiencyControl => 2,
    }
}

fn share_bps_floor(qty: u64, bps: u16) -> u64 {
    (u128::from(qty) * u128::from(bps) / 10_000) as u64
}

fn chain_config_endogenous_cultivation_entry_active(chain: &ChainConfig) -> bool {
    chain.endogenous_cultivation_entry && chain_config_cultivation_sells_surplus_active(chain)
}

fn chain_config_private_land_tenure_active(chain: &ChainConfig) -> bool {
    (chain.private_land_tenure || chain.secure_land_tenure)
        && chain_config_endogenous_cultivation_entry_active(chain)
}

fn chain_config_secure_land_tenure_active(chain: &ChainConfig) -> bool {
    chain.secure_land_tenure && chain_config_endogenous_cultivation_entry_active(chain)
}

fn chain_config_land_market_active(chain: &ChainConfig) -> bool {
    chain.land_market && chain_config_private_land_tenure_active(chain)
}

fn chain_config_mortal_landowner_demography_active(chain: &ChainConfig) -> bool {
    chain.mortal_landowner_demography
        && chain_config_secure_land_tenure_active(chain)
        && !chain_config_land_market_active(chain)
}

fn chain_config_mortal_chain_producers_active(chain: &ChainConfig) -> bool {
    chain.mortal_chain_producers
}

fn chain_config_mortal_producer_inheritance_active(chain: &ChainConfig) -> bool {
    chain.mortal_producer_inheritance && chain_config_mortal_chain_producers_active(chain)
}

fn chain_config_earned_provisioning_active(chain: &ChainConfig) -> bool {
    chain.earned_provisioning && chain_config_mortal_producer_inheritance_active(chain)
}

fn chain_config_birth_stock_saving_active(chain: &ChainConfig) -> bool {
    chain.birth_stock_saving
        && chain.birth_stock_saving_mode == BirthStockSavingMode::Motive
        && chain_config_earned_provisioning_active(chain)
}

fn chain_config_rival_subsistence_commons_active(chain: &ChainConfig) -> bool {
    chain.rival_subsistence_commons
        && chain_config_mortal_landowner_demography_active(chain)
        && chain.emergency_hunger_threshold > 0
}

fn chain_config_wage_labor_active(chain: &ChainConfig) -> bool {
    chain.wage_labor && chain_config_rival_subsistence_commons_active(chain)
}

fn chain_config_share_tenancy_active(chain: &ChainConfig) -> bool {
    chain.share_tenancy && chain_config_rival_subsistence_commons_active(chain)
}

fn config_private_land_tenure_active(config: &SettlementConfig) -> bool {
    config
        .chain
        .as_ref()
        .is_some_and(chain_config_private_land_tenure_active)
}

/// S22e: the deterministic endowment-selection hash of `(world seed, household id)` — a SplitMix64
/// finalizer with a dedicated salt, so the endowed households are stable per `(seed, roster)` and
/// the draw does not collide with the founder-seed / lifespan / starting-age derivations. Ranking
/// households by this hash (rather than by raw id) keeps the endowed minority from privileging the
/// low-id roster segment.
fn endowment_hash(seed: u64, household_id: usize) -> u64 {
    deterministic_mix64(
        seed ^ ENDOWMENT_SELECT_SALT ^ (household_id as u64).wrapping_mul(0x9e37_79b9),
    )
}

/// S22e: the dedicated salt for [`endowment_hash`] (a distinct constant from the demography salts).
const ENDOWMENT_SELECT_SALT: u64 = 0x5322_e0ca_b1e5_eed5;
const COMMITMENT_NORM_SEED_SALT: u64 = 0x24a0_c011_7a51_5eed;
const COMMITMENT_NORM_RANDOM_SALT: u64 = 0x24a0_c011_7a51_4a9d;
const COMMITMENT_NORM_CLUSTER_CENTER_SALT: u64 = 0x24c0_11ec_1a57_0001;

/// S18: the generation-time analogue of [`Settlement::multigood_money_active`] (no `self`
/// yet) — drives the woodcutter→WOOD-node routing in colonist generation. Off (every
/// existing config), the gatherers keep their round-robin node assignment, byte-identical.
fn config_multigood_money_active(config: &SettlementConfig) -> bool {
    config.chain.as_ref().is_some_and(|chain| {
        chain.multigood_money
            && chain.cultivation_sells_surplus
            && chain_config_own_use_cultivation_active(chain)
    })
}

/// S22b: the per-trip grain-haul capacity for a cultivating agent of the given `skill` —
/// `carry_cap + carry_cap·(ceiling−1)·skill/skill_cap`, capped at `ceiling × carry_cap`. At
/// `skill == 0` (or `skill_cap == 0`, or `ceiling ≤ 1`) it is exactly `carry_cap`, so the lever
/// is a no-op (the cap-zero control / a fresh-skill cultivator behaves like S22a). At
/// `skill == skill_cap` it reaches `ceiling × carry_cap` (the shipped `ceiling = 2` ⇒ ≤2×).
/// Integer, saturating, deterministic — no RNG. NEVER scales the recipe ratio, so conservation
/// is untouched (a faster draw on the conserved grain node, bounded by `node.stock`).
fn cultivation_haul(carry_cap: u32, skill: u16, skill_cap: u16, ceiling: u32) -> u32 {
    if skill == 0 || skill_cap == 0 || ceiling <= 1 {
        return carry_cap;
    }
    let bonus = (u64::from(carry_cap) * u64::from(ceiling - 1) * u64::from(skill)
        / u64::from(skill_cap)) as u32;
    carry_cap
        .saturating_add(bonus)
        .min(carry_cap.saturating_mul(ceiling))
}

fn food_needed_to_reach_hunger(
    hunger: u16,
    deplete: u16,
    hunger_per_food: u16,
    target: u16,
) -> u32 {
    if hunger_per_food == 0 {
        return u32::MAX;
    }
    let projected = u32::from(hunger).saturating_add(u32::from(deplete));
    let target = u32::from(target);
    if projected <= target {
        return 0;
    }
    let deficit = projected - target;
    let per_food = u32::from(hunger_per_food);
    deficit.saturating_add(per_food - 1) / per_food
}

fn advance_hunger_after_food(
    hunger: u16,
    deplete: u16,
    hunger_per_food: u16,
    need_max: u16,
    food: u64,
) -> u16 {
    let raised = u128::from(u32::from(hunger).saturating_add(u32::from(deplete)));
    let replenished = u128::from(hunger_per_food).saturating_mul(u128::from(food));
    let lowered = raised.saturating_sub(replenished);
    u16::try_from(lowered.min(u128::from(need_max))).unwrap_or(need_max)
}

/// S14: the good a birth endows (parent gate + debit, newborn seed, founder seed) —
/// the FORAGE subsistence good on the forage-commons path (so births stall on FORAGE
/// scarcity, not bread), else the hunger staple `known.hunger` (byte-identical). The
/// shared selector behind [`Settlement::birth_food`] and the founder seed at
/// generation. `known.hunger` itself is never mutated, so consumption / the chain /
/// sales still key on the staple.
fn birth_food_good(commons_active: bool, known: &KnownGoods) -> GoodId {
    if commons_active {
        known.subsistence.unwrap_or(known.hunger)
    } else {
        known.hunger
    }
}

/// Build a G4b household member's econ agent (a founder or a newborn): a
/// householder endowed from its household's `spec` (gold + a food/WOOD buffer), with a
/// value scale generated from its need state and (inherited) culture. (Under
/// `spatial_households`, S13, the member also gets a mirrored world agent elsewhere; this
/// builds only the econ side.) The food buffer (`spec.starting_food`) is held in
/// `food_good` — the hunger staple (FOOD on a `lineages` colony, bread on the frontier)
/// off the forage-commons
/// path, and the FORAGE subsistence good on it (S14, via [`birth_food_good`]) — so a
/// founder starts with a buffer of the good it actually eats. Like every other colonist
/// it is a `Household`-role agent with neutral price beliefs; it has no labor capacity
/// and no world agent (it never hauls).
fn build_demography_agent(
    id: AgentId,
    need: &NeedState,
    culture: &CultureParams,
    known: &KnownGoods,
    spec: &crate::demography::HouseholdSpec,
    food_good: GoodId,
) -> Agent {
    let mut stock = Stock::new(NET.0);
    stock.add(food_good, spec.starting_food);
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
/// agent endowed only with the **conserved transfer** its parent gave it (a food
/// buffer plus, on closed-GOLD M1, any gold gift already represented in `gold`),
/// its value scale generated from a newborn-rested need state and its
/// inherited+mutated culture. The `food` buffer is held in `food_good` — the hunger
/// staple (FOOD on `lineages`, bread on the frontier) off the forage-commons path, and
/// the FORAGE subsistence good on it (S14) — the good the newborn actually eats. Its
/// `id` is overwritten by [`Society::add_agent`].
/// It carries no wood — the household provision supplies that from its first tick.
/// M3 callers install the newborn with zero ledger money and move any gold gift
/// afterward through [`Society::transfer_gold`], so this mints nothing.
fn build_newborn_agent(
    need: &NeedState,
    culture: &CultureParams,
    known: &KnownGoods,
    gold: u64,
    food: u32,
    food_good: GoodId,
) -> Agent {
    let mut stock = Stock::new(NET.0);
    stock.add(food_good, food);
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
/// Whether a recipe is simply profitable at realized prices: expected revenue
/// (`output_price × output_qty`) exceeds the present advance (`input_price ×
/// input_qty + operating_cost`). The recurring owner-operator adoption test (see
/// [`ChainConfig::recurring_motive`]) — independent of any savings want, so a
/// satiated producer still keeps producing while the trade pays. Declines without
/// an observed output price (no sale → no spread).
pub fn recipe_is_profitable(
    recipe: &Recipe,
    output_price: Option<Gold>,
    input_price: Option<Gold>,
    operating_cost: u64,
) -> bool {
    let Some(output_price) = output_price else {
        return false;
    };
    let input_qty = recipe.input_good.map_or(0, |(_input, qty)| qty);
    let revenue = output_price.0.saturating_mul(u64::from(recipe.output_qty));
    let input_cost = input_price
        .map_or(0, |price| price.0)
        .saturating_mul(u64::from(input_qty));
    revenue > input_cost.saturating_add(operating_cost)
}

/// S7.2: the build appraisal's surplus for one tool — the entrepreneurial test that a
/// durable mill/oven's expected multi-period proceeds repay its build cost:
///
/// ```text
/// expected_margin_per_run × capital_payback_cycles
///     − (WOOD_build_cost + labor_opportunity_cost + first_input)
/// ```
///
/// where `expected_margin_per_run` reuses the bundle appraisal's per-cycle spread
/// (`revenue − input_cost − operating_cost`, the same margin [`recipe_is_profitable`]
/// tests), `WOOD_build_cost = wood_price × tool_build_wood`, `labor_opportunity_cost =
/// operating_cost × tool_build_labor`, and `first_input` is one cycle's input (the
/// working capital the first run needs). Returns the surplus when it is strictly
/// positive, else `None` (the per-run margin is non-positive, or the durable tool does
/// not repay its build cost over the horizon). A durable tool's infinite life does NOT
/// imply near-zero cost: the committed WOOD and the waiting labor are charged against
/// the discounted output stream. Declines without BOTH an output and input realized
/// price (no spread to appraise), so an oven build waits until flour has a price.
///
/// Demand-driven and self-correcting: once bread demand is met the realized output
/// price falls, the per-run margin drops, and the surplus goes non-positive — so no
/// tool is built (the overinvestment guard). The current caller uses the returned
/// surplus as a strict pay/no-pay gate, then chooses the stage by capacity bottleneck.
fn capital_build_surplus(
    recipe: &Recipe,
    output_price: Option<Gold>,
    input_price: Option<Gold>,
    appraisal: &CapitalBuildAppraisal,
) -> Option<i128> {
    let output_price = output_price?.0;
    let input_price = input_price?.0;
    let input_qty = recipe.input_good.map_or(0, |(_input, qty)| qty);
    let revenue = output_price.saturating_mul(u64::from(recipe.output_qty));
    let input_cost = input_price.saturating_mul(u64::from(input_qty));
    let margin_per_run =
        i128::from(revenue) - i128::from(input_cost) - i128::from(appraisal.operating_cost);
    if margin_per_run <= 0 {
        return None;
    }
    let lhs = margin_per_run * i128::from(appraisal.payback_cycles);
    let wood_build_cost = i128::from(
        appraisal
            .wood_price
            .saturating_mul(u64::from(appraisal.tool_build_wood)),
    );
    let labor_opportunity_cost = i128::from(
        appraisal
            .operating_cost
            .saturating_mul(u64::from(appraisal.tool_build_labor)),
    );
    let first_input = i128::from(input_cost);
    let surplus = lhs - (wood_build_cost + labor_opportunity_cost + first_input);
    (surplus > 0).then_some(surplus)
}

/// S7.2: the builder-independent inputs to [`capital_build_surplus`] — the realized
/// WOOD price plus the chain's build-cost knobs (the operating cost, the per-tool WOOD
/// and labor, and the payback horizon). Bundled so the appraisal is a small, named call
/// rather than a long argument list, and so the same costs apply to the mill and the
/// oven appraisal in one tick.
struct CapitalBuildAppraisal {
    operating_cost: u64,
    wood_price: u64,
    tool_build_wood: u32,
    tool_build_labor: u32,
    payback_cycles: u32,
}

/// S10: why a per-agent capital-tool appraisal declined (or that it `Accepted`) — the
/// diagnostic surface a test reads to prove a build is a per-colonist ORDINAL decision
/// (an earlier-eligible colonist declined on its OWN scale) rather than the S7 planner's
/// slot-order-first assignment. `NonPositiveMargin`/`NoPrices` are price-global (the same
/// for every colonist this tick); `NoFutureProvision`/`PresentCostOutranks` are
/// scale-specific — a colonist declining for one of those declined on its own value scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapitalDeclineReason {
    /// The colonist accepted — the build was taken.
    Accepted,
    /// The tool recipe's realized output (or input) price is missing — no spread to
    /// appraise (price-global).
    NoPrices,
    /// The recipe's net margin per run is non-positive — the tool would lose money
    /// (price-global; the demand-anchored brake once output demand is met).
    NonPositiveMargin,
    /// The tool's gestation-delayed receipt stream newly provisions NO future-money
    /// savings want on this colonist's scale — its savings ladder is too shallow
    /// (present-biased) or already satiated (scale-specific).
    NoFutureProvision,
    /// The newly-provisioned saving is ranked BELOW the leisure the build sacrifices (or
    /// below a present good the committed WOOD provisions) — the present cost outranks the
    /// future gain on this colonist's scale (scale-specific).
    PresentCostOutranks,
}

/// S10: one per-agent build decision recorded by the per-agent capital-formation phase —
/// which colonist appraised toward which tool this tick, whether it accepted, the rank of
/// the savings want the tool's receipts would provision (when one is reached), and the
/// decline reason. Read-only/diagnostic (NOT serialized — it steers no future tick),
/// exposed via [`Settlement::last_capital_decisions`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapitalDecision {
    pub candidate: AgentId,
    pub slot: usize,
    pub tool: GoodId,
    pub accepted: bool,
    pub target_rank: Option<usize>,
    pub reason: CapitalDeclineReason,
}

/// S10: the outcome of one [`appraise_capital_tool_bundle_for_money`] call on a colonist's
/// own scale — whether the build was accepted, the rank of the savings want its receipts
/// would provision (when one is reached), and the decline reason. Public so the
/// originary-interest microtest can appraise it directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapitalBuildOutcome {
    pub accepted: bool,
    pub target_rank: Option<usize>,
    pub reason: CapitalDeclineReason,
}

/// S10 (microtest surface — the falsifiable bar): does a colonist with `culture` (rested,
/// holding `wood_held` WOOD and `gold` of `savings_good`, saving in `savings_good`) accept
/// building a durable tool whose recipe yields `output_price·output_qty − input_price·
/// input_qty − operating_cost` proceeds per run, on its OWN multi-horizon value scale?
///
/// The ONLY thing that changes the answer between two calls with everything else fixed is
/// `culture.time_preference_bps`: it sets the savings-ladder DEPTH
/// ([`life::savings_ladder_depth`]), so a patient colonist carries deep savings wants a
/// tool's gestation-delayed receipts can fill (→ accepts) while a present-biased one's
/// shallow near-savings are unreachable (→ declines). That is the horizon-depth formula,
/// testable directly — if capital formation does NOT vary with `time_preference_bps`, the
/// decision is not reading the ordinal scale and the milestone failed.
#[allow(clippy::too_many_arguments)]
pub fn capital_build_outcome_for_culture(
    culture: CultureParams,
    savings_good: GoodId,
    gold: u64,
    wood_held: u32,
    output_price: u64,
    output_qty: u32,
    input_price: u64,
    input_qty: u32,
    build_wood: u32,
    build_labor: u32,
    operating_cost: u64,
) -> CapitalBuildOutcome {
    let known = KnownGoods {
        hunger: FOOD,
        warmth: WOOD,
        savings: savings_good,
        subsistence: None,
    };
    // A rested colonist (no hunger/warmth/rest deficit), so the only present cost the
    // build charges is the committed WOOD and the displaced leisure — exactly what the
    // microtest isolates against the savings ladder its culture shapes.
    let scale = regenerate_scale_for_capital(&NeedState::rested(), &culture, &known);
    let mut stock = Stock::new(NET.0);
    stock.add(WOOD, wood_held);
    let agent = Agent {
        id: AgentId(1),
        scale,
        stock,
        gold: Gold(gold),
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: Vec::new(),
    };
    // A synthetic recipe carrying only the input/output quantities the margin reads.
    let recipe = Recipe {
        id: RecipeId::Mill,
        name: "CapitalBuildMicrotest",
        labor: build_labor,
        input_good: (input_qty > 0).then_some((SALT, input_qty)),
        required_tool: None,
        output_good: FOOD,
        output_qty,
        enabled: true,
    };
    appraise_capital_tool_bundle_for_money(
        &agent,
        &recipe,
        Some(Gold(output_price)),
        (input_qty > 0).then_some(Gold(input_price)),
        build_wood,
        build_labor,
        0,
        operating_cost,
        savings_good,
    )
}

/// S11 (the forecast microtest surface — the falsifiable bar): does a colonist with
/// `culture` build the tool when it appraises the output revenue against its OWN
/// **grounded fallible forecast** of `realized_output_price`, rather than the realized
/// price itself? The forecaster carries no prior belief, so its grounded forecast is the
/// public realized price tilted by its heritable bias:
/// `forecast = realized_output_price × culture.forecast_bias_bps / 10_000`. The INPUT
/// price stays observed (the one clean lever — output-revenue optimism).
///
/// This isolates the selection mechanism: hold everything else fixed and vary ONLY
/// `forecast_bias_bps`, and an over-optimist (`> 10_000`) appraises an inflated revenue
/// and ACCEPTS a build the accurate forecaster (`10_000`) DECLINES — the build a market
/// that clears at `realized_output_price` will under-pay. With a neutral bias this is
/// exactly [`capital_build_outcome_for_culture`] at the realized price.
#[allow(clippy::too_many_arguments)]
pub fn capital_build_outcome_with_forecast(
    culture: CultureParams,
    savings_good: GoodId,
    gold: u64,
    wood_held: u32,
    realized_output_price: u64,
    output_qty: u32,
    input_price: u64,
    input_qty: u32,
    build_wood: u32,
    build_labor: u32,
    operating_cost: u64,
) -> CapitalBuildOutcome {
    let forecast = realized_output_price.saturating_mul(u64::from(culture.forecast_bias_bps))
        / u64::from(FORECAST_BIAS_NEUTRAL_BPS);
    capital_build_outcome_for_culture(
        culture,
        savings_good,
        gold,
        wood_held,
        forecast,
        output_qty,
        input_price,
        input_qty,
        build_wood,
        build_labor,
        operating_cost,
    )
}

/// S10: one tool a colonist could build, with the realized prices its recipe-margin
/// appraisal needs — the oven (Bake: flour→bread) or the mill (Mill: grain→flour).
#[derive(Clone, Copy)]
struct ToolCandidate<'a> {
    tool: GoodId,
    recipe: &'a Recipe,
    template_id: ProjectTemplateId,
    output_price: Option<Gold>,
    input_price: Option<Gold>,
}

/// S10: the inputs the per-agent capital-formation phase pulls from the chain runtime —
/// the two tool recipes + their goods and the build-cost knobs — bundled so the appraisal
/// loop is a small call rather than a long argument list.
struct PerAgentBuildParams<'a> {
    mill_recipe: &'a Recipe,
    bake_recipe: &'a Recipe,
    grain: GoodId,
    flour: GoodId,
    bread: GoodId,
    mill_good: GoodId,
    oven_good: GoodId,
    operating_cost: u64,
    wood_qty: u32,
    build_labor: u32,
    hunger_max: u16,
    tick: u64,
}

/// S10: a tool candidate's net recipe margin per run (`revenue − input_cost −
/// operating_cost`), or `i128::MIN` when its prices are missing — used only to ORDER the
/// two candidates so each colonist prefers the more rewarding roundabout investment (a
/// per-agent choice, not a global planner stage choice). The accept/reject gate itself is
/// the ordinal [`appraise_capital_tool_bundle_for_money`], never this scalar.
fn recipe_net_margin(candidate: &ToolCandidate<'_>, operating_cost: u64) -> i128 {
    let Some(output_price) = candidate.output_price else {
        return i128::MIN;
    };
    let input_qty = candidate.recipe.input_good.map_or(0, |(_input, qty)| qty);
    if input_qty > 0 && candidate.input_price.is_none() {
        return i128::MIN;
    }
    let revenue = output_price
        .0
        .saturating_mul(u64::from(candidate.recipe.output_qty));
    let input_cost = candidate
        .input_price
        .map_or(0, |price| price.0)
        .saturating_mul(u64::from(input_qty));
    i128::from(revenue) - i128::from(input_cost) - i128::from(operating_cost)
}

fn gold_mul_qty(price: Gold, qty: u32) -> Option<Gold> {
    price.0.checked_mul(u64::from(qty)).map(Gold)
}

fn prorate_gold_floor(amount: Gold, numerator: u32, denominator: u32) -> Gold {
    if amount == Gold::ZERO || numerator == 0 || denominator == 0 {
        return Gold::ZERO;
    }
    let prorated =
        u128::from(amount.0).saturating_mul(u128::from(numerator)) / u128::from(denominator);
    Gold(prorated.min(u128::from(u64::MAX)) as u64)
}

fn prorate_u32_floor(amount: u32, numerator: u32, denominator: u32) -> u32 {
    if amount == 0 || numerator == 0 || denominator == 0 {
        return 0;
    }
    let prorated =
        u128::from(amount).saturating_mul(u128::from(numerator)) / u128::from(denominator);
    prorated.min(u128::from(u32::MAX)) as u32
}

/// The escrowed payment when a worker's total reservation ask (`worker_ask`, already priced for the
/// whole `labor_qty` bundle by `reservation_labor_ask_for_money`) is met by an employer's total wage
/// ceiling. The payment is the total ask itself, never `worker_ask * labor_qty`.
fn wage_hire_payment(worker_ask: Gold, max_total_wage: Gold) -> Option<Gold> {
    (max_total_wage >= worker_ask).then_some(worker_ask)
}

fn highest_appraised_labor_total_wage(
    agent: &Agent,
    expected_revenue: Gold,
    max_total_wage: Gold,
    tick: u64,
    money_good: GoodId,
) -> Option<Gold> {
    if max_total_wage == Gold::ZERO {
        return None;
    }
    let pays = |amount: u64| -> bool {
        appraise_labor_hire_for_money(agent, expected_revenue, Gold(amount), tick, money_good)
    };
    // `appraise_labor_hire_for_money` is NOT monotone in the wage, so a low-end binary search
    // (assuming it clears from 1 upward) is unsound. It clears only on a band whose floor is the
    // owner's surplus above its soonest future-money savings threshold `(gold − threshold, …]`:
    // when the owner holds gold above that threshold, a small wage (including 1) leaves the want
    // still provisioned by present gold — so the expected proceeds are not pivotal and the hire is
    // declined — while a larger wage newly un-provisions the want that the proceeds then restore
    // and clears. A `!pays(1)` early-out therefore dropped every owner whose valid wage band lay
    // above 1, suppressing otherwise-acceptable voluntary/subsidised hires. The §4.3 ceiling is the
    // HIGHEST clearing wage, so scan down from the affordability cap (bounded by the small
    // `spendable.min(expected_revenue)`) and take the first wage that clears.
    // Bound the descend-scan by the affordability cap (spec-review P2): a wage above the owner's
    // spendable gold can never clear (`appraise_labor_hire_for_money` does `gold.checked_sub(wage)`
    // and declines on overflow), so cap the ceiling at `min(max_total_wage, gold)` — the scan is then
    // O(gold), never the nominal `max_total_wage` up to u64::MAX (which could hang `econ_tick()` when
    // a high realized bread price inflates the cap). NOTE the clearing band CAN extend above
    // `expected_revenue` (see `highest_wage_ceiling_finds_the_band_above_a_declined_low_wage`), so
    // revenue is NOT a valid upper bound; spendable gold is.
    let ceiling = max_total_wage.0.min(agent.gold.0);
    (1..=ceiling).rev().find(|&amount| pays(amount)).map(Gold)
}

fn appraise_labor_hire_for_money(
    agent: &Agent,
    expected_revenue: Gold,
    wage_amount: Gold,
    tick: u64,
    money_good: GoodId,
) -> bool {
    if expected_revenue == Gold::ZERO || wage_amount == Gold::ZERO {
        return false;
    }
    let Some(after_gold) = agent.gold.checked_sub(wage_amount) else {
        return false;
    };
    let receipt = wage_labor_receipt(agent.id, tick, expected_revenue);
    let receipts = [receipt];
    let current = TemporalEndowment {
        stock: &agent.stock,
        gold: agent.gold,
        receivables: &[],
        payables: &[],
        tick: Tick(tick),
    };
    let current_bitmap = provisioning_bitmap_for_money(&agent.scale, &current, money_good);
    let debited = TemporalEndowment {
        stock: &agent.stock,
        gold: after_gold,
        receivables: &[],
        payables: &[],
        tick: Tick(tick),
    };
    let debited_bitmap = provisioning_bitmap_for_money(&agent.scale, &debited, money_good);
    let after = TemporalEndowment {
        stock: &agent.stock,
        gold: after_gold,
        receivables: &receipts,
        payables: &[],
        tick: Tick(tick),
    };
    let after_bitmap = provisioning_bitmap_for_money(&agent.scale, &after, money_good);
    let target = agent.scale.iter().enumerate().find_map(|(index, want)| {
        let future_money =
            want.kind == WantKind::Good(money_good) && matches!(want.horizon, Horizon::Later(_));
        (future_money
            && !debited_bitmap.get(index).copied().unwrap_or(false)
            && after_bitmap.get(index).copied().unwrap_or(false))
        .then_some(index)
    });
    let Some(target) = target else {
        return false;
    };
    preserved_provisioning_above(&current_bitmap, &after_bitmap, target)
}

fn wage_labor_receipt(owner: AgentId, tick: u64, due: Gold) -> DebtContract {
    DebtContract {
        id: DebtId(0),
        lender: CreditLender::Agent(owner),
        borrower: AgentId(0),
        opened_tick: Tick(tick),
        due_tick: Tick(tick.saturating_add(1)),
        principal: Gold::ZERO,
        due,
        paid: Gold::ZERO,
        state: DebtState::Open,
        purpose: DebtPurpose::Consumption,
        funding: CreditSource::Commodity,
    }
}

/// S10: appraise EVERY tool `agent` could build on its OWN scale (most-rewarding first)
/// and return the recorded decision plus the index of the tool it accepts (if any). The
/// colonist builds the first tool its scale accepts; otherwise the decision keeps the most
/// informative decline (a scale-specific reason over a price-global one) so the diagnostic
/// shows a genuine own-scale decline.
#[allow(clippy::too_many_arguments)]
fn appraise_capital_for_colonist(
    agent: &Agent,
    slot: usize,
    tool_candidates: &[ToolCandidate<'_>],
    wood_qty: u32,
    build_labor: u32,
    tick: u64,
    operating_cost: u64,
    money_good: GoodId,
) -> (CapitalDecision, Option<usize>) {
    let mut decision = CapitalDecision {
        candidate: agent.id,
        slot,
        tool: tool_candidates
            .first()
            .map_or(WOOD, |candidate| candidate.tool),
        accepted: false,
        target_rank: None,
        reason: CapitalDeclineReason::NoPrices,
    };
    for (index, candidate) in tool_candidates.iter().enumerate() {
        let outcome = appraise_capital_tool_bundle_for_money(
            agent,
            candidate.recipe,
            candidate.output_price,
            candidate.input_price,
            wood_qty,
            build_labor,
            tick,
            operating_cost,
            money_good,
        );
        if outcome.accepted {
            return (
                CapitalDecision {
                    candidate: agent.id,
                    slot,
                    tool: candidate.tool,
                    accepted: true,
                    target_rank: outcome.target_rank,
                    reason: CapitalDeclineReason::Accepted,
                },
                Some(index),
            );
        }
        if decline_rank(outcome.reason) > decline_rank(decision.reason) {
            decision = CapitalDecision {
                candidate: agent.id,
                slot,
                tool: candidate.tool,
                accepted: false,
                target_rank: outcome.target_rank,
                reason: outcome.reason,
            };
        }
    }
    (decision, None)
}

/// S10: how informative a decline reason is — a scale-specific decline
/// (`PresentCostOutranks` / `NoFutureProvision`, the colonist declining on its OWN scale)
/// outranks a price-global one (`NonPositiveMargin` / `NoPrices`, the same for everyone),
/// so the recorded diagnostic prefers a genuine own-scale decline.
fn decline_rank(reason: CapitalDeclineReason) -> u8 {
    match reason {
        CapitalDeclineReason::NoPrices => 0,
        CapitalDeclineReason::NonPositiveMargin => 1,
        CapitalDeclineReason::NoFutureProvision => 2,
        CapitalDeclineReason::PresentCostOutranks => 3,
        CapitalDeclineReason::Accepted => 4,
    }
}

/// S10: the per-agent ORDINAL capital-tool build appraisal — the milestone's core.
///
/// Decides, on `agent`'s OWN value scale, whether committing present WOOD + forgone
/// leisure to build a durable `recipe` tool whose multi-period proceeds provision one of
/// the agent's own future-money savings wants is worth it — generalizing
/// [`appraise_project_bundle_for_money`] to a dated receivable STREAM (not one receipt)
/// and charging the present sacrifice as the spec's PRESENT side (the WOOD removed from
/// stock + the displaced Leisure rank), with NO cardinal discount (originary interest is
/// expressed ordinally, via the agent's own savings ladder).
///
/// - PRESENT side: the `build_wood` WOOD is removed from the bundle stock (its present
///   warmth use forgone); the `build_labor` build ticks displace the agent's leisure want.
/// - FUTURE side: the recipe's net margin per run (`revenue − input_cost − operating_cost`)
///   arrives as a stream of dated receivables — one per tick from `tick + build_labor + 1`
///   out to the agent's deepest savings horizon. A tool's gestation pushes the first
///   receipt past the near horizons (`Later(4)`), so only a deep-enough savings ladder is
///   reached — which is exactly where `time_preference_bps` bites (a patient colonist
///   carries the deep wants; a present-biased one does not).
/// - ACCEPTANCE (the gate): the altered endowment newly provisions a future-money savings
///   want ([`appraise_project_bundle_for_money`]'s `bundle_accepts_due` generalized) while
///   preserving every higher-ranked want — AND that newly-provisioned want outranks the
///   displaced leisure. A patient colonist (deep ladder, savings ranked high) accepts; a
///   present-biased one (shallow ladder, savings below leisure / unreached by the stream)
///   declines — no rate imposed.
#[allow(clippy::too_many_arguments)]
fn appraise_capital_tool_bundle_for_money(
    agent: &Agent,
    recipe: &Recipe,
    output_price: Option<Gold>,
    input_price: Option<Gold>,
    build_wood: u32,
    build_labor: u32,
    tick: u64,
    operating_cost: u64,
    money_good: GoodId,
) -> CapitalBuildOutcome {
    let decline = |reason| CapitalBuildOutcome {
        accepted: false,
        target_rank: None,
        reason,
    };

    // ---- the recipe's net margin per run (the FUTURE side's per-tick proceeds).
    let Some(output_price) = output_price else {
        return decline(CapitalDeclineReason::NoPrices);
    };
    let input_qty = recipe.input_good.map_or(0, |(_input, qty)| qty);
    if input_qty > 0 && input_price.is_none() {
        return decline(CapitalDeclineReason::NoPrices);
    }
    let revenue = output_price.0.saturating_mul(u64::from(recipe.output_qty));
    let input_cost = input_price
        .map_or(0, |price| price.0)
        .saturating_mul(u64::from(input_qty));
    let margin = i128::from(revenue) - i128::from(input_cost) - i128::from(operating_cost);
    if margin <= 0 {
        return decline(CapitalDeclineReason::NonPositiveMargin);
    }
    let margin = u64::try_from(margin).unwrap_or(u64::MAX);

    // ---- PRESENT side: remove the committed WOOD from the bundle stock (its present
    // warmth use forgone). Eligibility already checks the WOOD is on hand; defensive.
    let mut bundle_stock = agent.stock.clone();
    if !bundle_stock.remove(WOOD, build_wood) {
        return decline(CapitalDeclineReason::NoFutureProvision);
    }

    // ---- FUTURE side: the dated receivable stream — one margin-receipt per tick from
    // tick+build_labor+1 (the first proceeds after the gestation) out to the agent's
    // deepest savings horizon (beyond it there is no want for a receipt to provision).
    let max_horizon = deepest_savings_horizon(&agent.scale, money_good);
    let mut stream: Vec<DebtContract> = Vec::new();
    let mut due_in = u64::from(build_labor).saturating_add(1);
    while due_in <= max_horizon {
        stream.push(capital_tool_receipt(agent.id, tick, due_in, Gold(margin)));
        due_in += 1;
    }
    if stream.is_empty() {
        // The gestation pushes every receipt past the agent's deepest savings horizon — a
        // present-biased colonist whose ladder never reaches the tool's payback window.
        return decline(CapitalDeclineReason::NoFutureProvision);
    }

    // ---- ACCEPTANCE: newly provision a future-money savings want while preserving every
    // higher-ranked want, and outrank the displaced leisure.
    let baseline = TemporalEndowment {
        stock: &agent.stock,
        gold: agent.gold,
        receivables: &[],
        payables: &[],
        tick: Tick(tick),
    };
    let baseline_bitmap = provisioning_bitmap_for_money(&agent.scale, &baseline, money_good);
    let bundle = TemporalEndowment {
        stock: &bundle_stock,
        gold: agent.gold,
        receivables: &stream,
        payables: &[],
        tick: Tick(tick),
    };
    let bundle_bitmap = provisioning_bitmap_for_money(&agent.scale, &bundle, money_good);

    // The first (highest-ranked) future-money savings want the stream newly provisions.
    let target = agent.scale.iter().enumerate().find_map(|(index, want)| {
        let future_money_want =
            want.kind == WantKind::Good(money_good) && matches!(want.horizon, Horizon::Later(_));
        (future_money_want
            && !baseline_bitmap.get(index).copied().unwrap_or(false)
            && bundle_bitmap.get(index).copied().unwrap_or(false))
        .then_some(index)
    });
    let Some(target) = target else {
        return decline(CapitalDeclineReason::NoFutureProvision);
    };

    // Preserve every higher-ranked want the committed WOOD might have provisioned (the
    // WOOD's present warmth use among the costs the future gain must outrank).
    if !preserved_provisioning_above(&baseline_bitmap, &bundle_bitmap, target) {
        return decline(CapitalDeclineReason::PresentCostOutranks);
    }
    // The displaced leisure: the build sacrifices the agent's leisure for the build
    // duration, so the newly-provisioned saving must OUTRANK the leisure want — else the
    // present sacrifice outranks the future gain on this colonist's own scale (a
    // present-biased colonist whose savings sink below its leisure).
    if let Some(leisure_rank) = agent
        .scale
        .iter()
        .position(|want| want.kind == WantKind::Leisure)
    {
        if target >= leisure_rank {
            return decline(CapitalDeclineReason::PresentCostOutranks);
        }
    }

    CapitalBuildOutcome {
        accepted: true,
        target_rank: Some(target),
        reason: CapitalDeclineReason::Accepted,
    }
}

/// S10: the deepest `Later` horizon at which `scale` carries a savings want for
/// `money_good` (the far end of the multi-horizon ladder), or `0` if none — the window the
/// capital-tool receipt stream is dated across.
fn deepest_savings_horizon(scale: &[Want], money_good: GoodId) -> u64 {
    scale
        .iter()
        .filter_map(|want| match (want.kind, want.horizon) {
            (WantKind::Good(good), Horizon::Later(later)) if good == money_good => {
                Some(u64::from(later))
            }
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

/// S10: a hypothetical dated tool-proceeds receipt for the
/// [`appraise_capital_tool_bundle_for_money`] stream — an in-memory `DebtContract`
/// carrying only the `(due_tick, due)` the provisioning math reads, never registered as a
/// real claim (the owner id is a per-call label, like the role-choice appraisal's).
fn capital_tool_receipt(owner: AgentId, tick: u64, due_in: u64, due: Gold) -> DebtContract {
    DebtContract {
        id: DebtId(0),
        lender: CreditLender::Agent(owner),
        borrower: AgentId(0),
        opened_tick: Tick(tick),
        due_tick: Tick(tick.saturating_add(due_in)),
        principal: Gold::ZERO,
        due,
        paid: Gold::ZERO,
        state: DebtState::Open,
        purpose: DebtPurpose::Consumption,
        funding: CreditSource::Commodity,
    }
}

/// S10: every want ranked above `target` that the baseline provisioned is still
/// provisioned in the bundle — the `preserved_above_target` invariant (the future
/// provision must not break a higher-ranked want), generalized to the capital-tool
/// bundle's two bitmaps.
fn preserved_provisioning_above(before: &[bool], after: &[bool], target: usize) -> bool {
    before
        .iter()
        .zip(after)
        .take(target)
        .all(|(was, now)| !*was || *now)
}

/// S11: cap an entrepreneurial project-input bid's posted limit at the observed input
/// price. The reservation may be higher because the producer forecasts dear output, but
/// the CDA fill price is the resting order's limit, so the posted limit must not become a
/// forecast-inflated input-price anchor. With forecasts off, or before any input price
/// exists, keep the legacy reservation-as-limit behavior.
fn project_input_bid_limit(
    reservation: Gold,
    observed_input_price: Option<Gold>,
    entrepreneurial: bool,
) -> Gold {
    if entrepreneurial {
        observed_input_price.map_or(reservation, |price| reservation.min(price))
    } else {
        reservation
    }
}

/// The producer's imputed reservation for ONE unit of its recipe input (S2),
/// derived by REUSING the project-bundle appraisal rather than a scalar profit.
///
/// Returns the highest per-unit input price at which running the recipe once,
/// framed as a project bundle, still provisions the producer's soonest savings
/// want on its current endowment — probing [`recipe_adoption_pays_for_money`] (the
/// same gate role-choice adopts on, which internally calls
/// [`appraise_project_bundle_for_money`]). The Mengerian ceiling — the output's
/// realized value minus the operating cost, per input unit — bounds the search:
/// the input is never worth more than the output it yields net of cost. The
/// highest tolerated price is found by binary search over `[1, ceiling]`.
///
/// The binary search is correct only because `recipe_adoption_pays_for_money` is
/// monotone-decreasing in the input price (a dearer input only raises the
/// bundle's `present_advance`, which `appraise_project_bundle_for_money` in the
/// `econ` crate treats monotonically: a larger advance is never easier to repay).
/// That invariant is load-bearing and crosses the crate boundary, so a debug
/// assertion below pins the found boundary against it — if a future appraisal
/// change broke monotonicity, the search could return a wrong reservation and
/// this would catch it in tests.
///
/// If the savings want is already satiated (no price provisions it) but
/// `recurring_motive` is set, fall back to the ceiling — a satiated owner-operator
/// still re-stocks at break-even to keep the consumption cycle going (the same
/// recurring gate role-choice keeps the role adopted on). `None` if nothing pays.
fn imputed_input_reservation(
    agent: &Agent,
    recipe: &Recipe,
    output_price: Gold,
    tick: u64,
    operating_cost: u64,
    recurring: bool,
    money_good: GoodId,
) -> Option<Gold> {
    let input_qty = u64::from(recipe.input_good.map_or(1, |(_input, qty)| qty).max(1));
    let revenue = output_price.0.saturating_mul(u64::from(recipe.output_qty));
    let ceiling = revenue.saturating_sub(operating_cost) / input_qty;
    if ceiling == 0 {
        return None;
    }
    let pays = |price: u64| {
        recipe_adoption_pays_for_money(
            agent,
            recipe,
            Some(output_price),
            Some(Gold(price)),
            tick,
            operating_cost,
            money_good,
        )
    };
    if pays(1) {
        // Binary-search the highest input price the bundle still tolerates.
        let mut low = 1u64;
        let mut high = ceiling;
        while low < high {
            // Round the midpoint UP (`div_ceil`): with `low + 1 == high` the
            // floor midpoint would equal `low`, and `pays(low)` already holds, so
            // `low = mid` would never advance — an infinite loop. Rounding up
            // probes `high` instead, which makes progress and biases the result
            // toward the higher tolerated price.
            let mid = low + (high - low).div_ceil(2);
            if pays(mid) {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        // Pin the boundary against the monotonicity the search relies on: `low`
        // must pay, and `low + 1` (when within the ceiling) must not. A
        // non-monotone appraisal change would trip this in debug/test builds
        // rather than silently mis-pricing the input.
        debug_assert!(pays(low), "the binary-search reservation must pay");
        debug_assert!(
            low == ceiling || !pays(low + 1),
            "the reservation must be the HIGHEST tolerated input price (monotone)"
        );
        return Some(Gold(low));
    }
    // Satiated savings want: a recurring owner-operator still re-stocks at the
    // Mengerian break-even ceiling (there is a positive spread, since ceiling ≥ 1).
    if recurring {
        return Some(Gold(ceiling));
    }
    None
}

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
    // appraise the same ordinal bundle as everyone else. Using this sentinel is safe
    // even when a real `AgentId(1)` exists: the owner id is stamped ONLY onto the two
    // hypothetical contracts the appraisal builds in-memory for this one call (the
    // imagined receivable/payable in `bundle_accepts_due`), and the provisioning math
    // those feed reads only their `(due_tick, remaining_due)` amounts, never the
    // borrower id and never a global claim registry (agio.rs). This wrapper passes the
    // real agent's own `receivables`/`payables` as empty (`&[]`), so no other agent's
    // claims are in scope to collide with. The owner is a per-call label, not a key.
    const APPRAISAL_OWNER_SENTINEL: AgentId = AgentId(1);
    let appraisal_owner = if agent.id == AgentId(0) {
        APPRAISAL_OWNER_SENTINEL
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
        Vocation::CycleA => Some(RecipeId::CycleA),
        Vocation::CycleB => Some(RecipeId::CycleB),
        Vocation::CycleC => Some(RecipeId::CycleC),
        Vocation::Unassigned => latent,
        Vocation::Gatherer | Vocation::Consumer => None,
    }?;
    match recipe {
        RecipeId::Mill => Some((content.mill(), content.grain())),
        RecipeId::Bake => Some((content.oven(), content.flour())),
        RecipeId::Research => Some((content.library()?, content.grain())),
        RecipeId::Confect => Some((content.atelier()?, content.flour())),
        RecipeId::CycleA => Some((content.cycle_a_tool()?, content.cycle_z()?)),
        RecipeId::CycleB => Some((content.cycle_b_tool()?, content.cycle_x()?)),
        RecipeId::CycleC => Some((content.cycle_c_tool()?, content.cycle_y()?)),
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

fn cycle_producer_scale_extension(
    scale: &mut Vec<Want>,
    tool: GoodId,
    input_good: GoodId,
    input_wants: u32,
    direct_good: GoodId,
    direct_wants: u32,
) {
    let insert_at = scale_input_insert_position(scale);
    let input_wants = input_wants as usize;
    let direct_wants = direct_wants as usize;
    let mut base = std::mem::take(scale);
    scale.reserve(base.len() + input_wants + direct_wants + 1);

    scale.push(Want {
        kind: WantKind::Good(tool),
        horizon: Horizon::Next,
        qty: 1,
        satisfied: false,
    });
    scale.extend(base.drain(..insert_at));
    for _ in 0..direct_wants {
        scale.push(Want {
            kind: WantKind::Good(direct_good),
            horizon: Horizon::Now,
            qty: 1,
            satisfied: false,
        });
    }
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

/// S9: extend a colonist's need scale with `qty` fixed `Good(good)/Now` direct
/// CONSUMPTION wants for the medium (SALT) — the heterogeneous real direct use
/// that seeds SALT's pre-monetary saleability. Unlike [`medium_scale_extension`]
/// (a `Horizon::Next` "hold the medium" SAVINGS demand), these are `Horizon::Now`
/// wants the consume arm eats into the `consumed` bucket: a real non-monetary use,
/// not a demand to hold SALT as money. Each is a SINGLE unit so the per-offer
/// (`qty == 1`) barter machinery can acquire them one at a time, exactly like the
/// medium wants. Inserted just below the present survival block (after the last
/// `Now` good want, before the savings ladder) — the colonist warms and feeds
/// first, then barters surplus for its SALT ration. Pure and deterministic; no RNG.
fn direct_use_scale_extension(scale: &mut Vec<Want>, good: GoodId, qty: u32) {
    if qty == 0 {
        return;
    }
    let insert_at = scale_input_insert_position(scale);
    let direct_wants = (0..qty).map(|_| Want {
        kind: WantKind::Good(good),
        horizon: Horizon::Now,
        qty: 1,
        satisfied: false,
    });
    let tail = scale.split_off(insert_at);
    scale.extend(direct_wants);
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

/// A stable serialization tag for a [`BenchSurface`] in [`Settlement::canonical_bytes`].
fn bench_surface_tag(surface: BenchSurface) -> u8 {
    match surface {
        BenchSurface::Spot => 0,
        BenchSurface::Debt => 1,
        BenchSurface::BankRepayment => 2,
        BenchSurface::IssuerRepayment => 3,
    }
}

// Canonical tags for econ enums are part of sim's persisted determinism contract, not
// ordinal casts from econ. Keep these as named, exhaustive matches: adding a variant
// upstream must fail compilation here until its byte is assigned deliberately. The
// `external_econ_canonical_tags_are_pinned` unit test below also pins the current map.
fn cycle_kind_tag(kind: CycleKind) -> u8 {
    match kind {
        CycleKind::CreditCycle => 0,
        CycleKind::SoundMoney => 1,
    }
}

fn scenario_name_tag(name: ScenarioName) -> u8 {
    match name {
        ScenarioName::CrusoeSurvival => 0,
        ScenarioName::CrusoeCapital => 1,
        ScenarioName::CrusoeAbandon => 2,
        ScenarioName::MarketBarterishGold => 3,
        ScenarioName::MarketPriceDiscovery => 4,
        ScenarioName::MarketNoMutualBenefit => 5,
        ScenarioName::TimeMarketBasic => 6,
        ScenarioName::RoundaboutCapital => 7,
        ScenarioName::BorrowToBuild => 8,
        ScenarioName::SoundMoney100Pct => 9,
        ScenarioName::CommodityCreditNeutral => 10,
        ScenarioName::FractionalReserve => 11,
        ScenarioName::SuspensionOfConvertibility => 12,
        ScenarioName::FiatCreditExpansion => 13,
        ScenarioName::FiatFiscalCantillon => 14,
        ScenarioName::CantillonIsolation => 15,
        ScenarioName::EmergedGoldSoundControl => 16,
        ScenarioName::EmergedGoldFiatDisplacement => 17,
        ScenarioName::EmergedGoldFiatRefusalControl => 18,
        ScenarioName::EmergedGoldFiatLegalTender => 19,
        ScenarioName::EmergedGoldFiatDebtRefusalControl => 20,
        ScenarioName::EmergedGoldFiatDebtLegalTender => 21,
        ScenarioName::EmergedGoldBankClaimDebtRefusalControl => 22,
        ScenarioName::EmergedGoldBankClaimDebtLegalTender => 23,
        ScenarioName::EmergedGoldBankClaimSpotRefusalControl => 24,
        ScenarioName::EmergedGoldBankClaimSpotLegalTender => 25,
        ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl => 26,
        ScenarioName::EmergedGoldBankLoanRepaymentClaimTender => 27,
        ScenarioName::EmergedGoldFractionalReserve => 28,
        ScenarioName::EmergedGoldFiatCreditExpansion => 29,
        ScenarioName::EmergedGoldFiatWageRefusalControl => 30,
        ScenarioName::EmergedGoldFiatWageLegalTender => 31,
        ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl => 32,
        ScenarioName::EmergedGoldIssuerRepaymentFiatTender => 33,
        ScenarioName::EmergedGoldReserveLeashControl => 34,
        ScenarioName::EmergedGoldSuspensionOfConvertibility => 35,
        ScenarioName::EmergedGoldRedemptionRun => 36,
        ScenarioName::EmergedGoldSuspendedRedemption => 37,
        ScenarioName::EmergedGoldTaxSpecieControl => 38,
        ScenarioName::EmergedGoldTaxFiatUnpayableDefaults => 39,
        ScenarioName::EmergedGoldTaxDrivesFiatLabor => 40,
        ScenarioName::EmergedGoldNoTaxIdleControl => 41,
        ScenarioName::MengerSaltMoney => 42,
        ScenarioName::MengerGoldMoney => 43,
        ScenarioName::MengerMarketabilityDurability => 44,
        ScenarioName::MengerTwoLayerSaleability => 45,
    }
}

fn recipe_id_tag(recipe: RecipeId) -> u8 {
    match recipe {
        RecipeId::GatherFood => 0,
        RecipeId::CutWood => 1,
        RecipeId::FishWithNet => 2,
        RecipeId::Mill => 3,
        RecipeId::Bake => 4,
        RecipeId::Research => 5,
        RecipeId::Confect => 6,
        RecipeId::Cultivate => 7,
        RecipeId::CycleA => 8,
        RecipeId::CycleB => 9,
        RecipeId::CycleC => 10,
    }
}

fn cantillon_sector_tag(sector: CantillonSector) -> u8 {
    match sector {
        CantillonSector::Capitalists => 0,
        CantillonSector::Households => 1,
        CantillonSector::Workers => 2,
        CantillonSector::Consumers => 3,
    }
}

fn public_debt_tender_tag(tender: PublicDebtTender) -> u8 {
    match tender {
        PublicDebtTender::ParAll => 0,
        PublicDebtTender::SpecieOnly => 1,
        PublicDebtTender::FiatAndSpecie => 2,
        PublicDebtTender::BankClaimsAndSpecie => 3,
    }
}

fn bank_repayment_tender_tag(tender: BankRepaymentTender) -> u8 {
    match tender {
        BankRepaymentTender::ParAll => 0,
        BankRepaymentTender::SpecieOnly => 1,
        BankRepaymentTender::FiatAndSpecie => 2,
        BankRepaymentTender::BankClaimsAndSpecie => 3,
    }
}

fn issuer_repayment_tender_tag(tender: IssuerRepaymentTender) -> u8 {
    match tender {
        IssuerRepaymentTender::FiatOnly => 0,
        IssuerRepaymentTender::FiatRefused => 1,
    }
}

fn labor_wage_tender_tag(tender: LaborWageTender) -> u8 {
    match tender {
        LaborWageTender::ParAll => 0,
        LaborWageTender::SpecieOnly => 1,
        LaborWageTender::FiatAndSpecie => 2,
    }
}

fn tax_receivability_tag(receivability: TaxReceivability) -> u8 {
    match receivability {
        TaxReceivability::SpecieOnly => 0,
        TaxReceivability::FiatOnly => 1,
        TaxReceivability::FiatAndSpecie => 2,
    }
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

fn m2_project_state_tag(state: M2ProjectState) -> u8 {
    match state {
        M2ProjectState::Forming => 0,
        M2ProjectState::Waiting => 1,
        M2ProjectState::Mature => 2,
        M2ProjectState::Sold => 3,
        M2ProjectState::Abandoned => 4,
    }
}

fn project_template_id_tag(id: ProjectTemplateId) -> u8 {
    match id {
        ProjectTemplateId::BuildNet => 0,
        ProjectTemplateId::BuildRoad => 1,
        ProjectTemplateId::BuildMill => 2,
        ProjectTemplateId::BuildOven => 3,
        ProjectTemplateId::BuildCultivationTool => 4,
    }
}

fn debt_state_tag(state: DebtState) -> u8 {
    match state {
        DebtState::Open => 0,
        DebtState::Settled => 1,
        DebtState::Defaulted => 2,
    }
}

fn belief_vec() -> Vec<PriceBelief> {
    let slots = usize::from(NET.0) + 1;
    vec![PriceBelief::new(Gold(2), Gold(1)); slots]
}

#[cfg(test)]
mod tests;
