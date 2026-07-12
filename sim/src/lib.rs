//! `sim` — the two-rate orchestrator (G2b) and the multi-settlement region (G2c).
//!
//! G2a built the spatial substrate (`world`) in isolation; G2b makes space
//! **economically meaningful** by wiring it under the economy. `sim` owns a
//! [`world::World`], per-colonist [`life`] need state, and an [`econ::Society`],
//! and runs the **two-rate loop** the game-spec (§4.1, §4.3) calls for: a fast
//! loop of `FAST_TICKS_PER_ECON_TICK` `world` ticks (movement, harvest, haul)
//! under one economic tick (transfer → needs/death → scale regeneration →
//! market clearing → consumption read-back → task reassignment).
//!
//! The milestone proves two things:
//!
//! 1. **Whole-system conservation is exact.** Every physical good is accounted
//!    across its full spatial+economic lifecycle — node stock → hauler carry →
//!    exchange stockpile → econ stock → consumed — with node regen the only
//!    source and consumption the only sink. Goods in transit (a hauler's carry)
//!    are the §4.3 **delivery escrow**: conserved, retained (never destroyed)
//!    when a hauler is blocked or dies.
//! 2. **Distance measurably affects realized prices.** A good gathered from a
//!    node FAR from the exchange realizes a higher price than the same good
//!    gathered NEAR, because travel eats the fast-tick budget so fewer units
//!    reach the market per economic tick (sign only — the lab's
//!    direction-not-magnitude discipline).
//!
//! **The world→econ transfer seam** is the load-bearing design: a good has ONE
//! owner at a time — `world` (node / carry / stockpile) **or** `econ` (agent
//! stock). The econ-tick transfer is the only crossing, and it is net-zero
//! (`world` −n, `econ` +n): delivered exchange-stockpile units are *credited to
//! the depositing colonist's econ stock* and *withdrawn from the world*. If a
//! depositor has no stock headroom yet, the unit stays world-owned in the
//! exchange stockpile and is retried later. See `docs/engine-divergence.md`.
//!
//! `sim` reuses `life`'s `regenerate_scale` / `NeedState` / `CultureParams` /
//! death mechanism and `world` / `econ` as-is, adding only the two additive,
//! conserving accessors that realize the seam ([`world::World::stockpile_withdraw`]
//! and [`econ::society::Society::credit_stock`]). It **supersedes** `life::Camp`
//! as the driver (Camp stays as the G1 non-spatial reference harness).
//!
//! Pure std, integer state, deterministic: the `Rng` is consumed only at
//! [`Settlement::generate`]; neither the fast loop nor the econ tick draws any
//! randomness; iteration is `AgentId`-ordered; storage is `BTreeMap`/`Vec`,
//! never `HashMap`. Same seed + same [`SettlementConfig`] → byte-identical run.
//!
//! ## G2c — multiple settlements that trade (the [`Region`])
//!
//! [`Region`] composes the milestone's final slice **by composition, not internal
//! surgery**: it holds a `Vec<Settlement>`, each one **unchanged** from G2b, linked
//! by an abstract inter-settlement [`Route`] (a transit-tick count), with one
//! caravan carrying a good from where it is cheap to where it is dear.
//! The caravan is a **pair of permanent resident trader agents** (one per linked
//! settlement, created at generation — never a runtime roster change), whose wealth
//! the region shuttles along the route as escrow using the additive, conserving
//! `econ` transfer accessors. Because no `Settlement` and no `Society` internal
//! behaviour changes, the six econ conformance goldens and the whole G1/G2a/G2b/G2d
//! suites stay byte-identical. The region proves region-wide conservation is exact
//! (every good and all gold across all settlements plus the in-transit escrow) and
//! that trade converges prices (the gap narrows versus a no-caravan control — sign
//! only). See [`mod@region`].
//!
//! ## G6a — the era detector ([`mod@era`])
//!
//! [`EraDetector`] is a **read-only** classification of a settlement's institutional
//! era (`Forager → Barter → Money → Specialist → Capital`) from **measured**
//! quantities, with hysteresis — game-spec pillar 2, *"eras are earned, not timed."*
//! It is the engine's *"phase is measured, never set"* doctrine: the era is a derived
//! statistic, not a state the engine sets or a timer advances. It reads only the
//! existing accessors (vocations, the money good, the per-tick report, barter
//! volume), mutates nothing, draws no RNG, holds no `HashMap`, and is imported by **no
//! decision path** (a source-gate, like econ's `metrics`, enforces it) — so a run
//! observed by a detector is byte-identical to one that is not, and the six econ
//! goldens are untouched. The Credit/Modern rungs are deferred to G8 (finance). See
//! [`mod@era`] and `sim/tests/g6a_eras.rs`.
//!
//! ## G8a/G8b — M3 ledger money and banks
//!
//! [`SettlementConfig::m3_settlement`] runs the same viable settlement on econ's M3
//! ledger with pure specie; [`SettlementConfig::bank`] and
//! [`SettlementConfig::bank_full_reserve`] add one config-chartered bank on that
//! ledger. Deposits move specie into reserves and give colonists demand claims they
//! spend, while fractional-reserve lending issues fiduciary claims through econ's
//! existing `MoneySystem`/`Bank` paths. The full-reserve twin lends zero fiduciary,
//! isolating credit creation to the reserve ratio. Fiat, tender/tax policy, the
//! regime ladder, and the Credit/Modern era rungs remain deferred.

pub mod content;
pub mod demography;
pub mod era;
pub mod region;
pub mod settlement;

/// DH.a (impl-68): the closed-circulation closure vocabulary — the classification classes, the raw
/// audit-tape events, the per-tick aggregates, the pure `classify_closure` verdict, and its
/// `ClosureWindow`/`ClosureVerdict` types. Re-exported so the `ignition_withdrawal` oracle and the
/// DH.a tests name them through `sim` alone. See [`settlement::closure`].
pub use settlement::closure::{
    classify_closure, ClosureClass, ClosureCriterion, ClosureDebitFamily, ClosureEventKind,
    ClosurePhysicalEvent, ClosureTickAgg, ClosureVerdict, ClosureWindow,
};

pub use content::ContentSet;
pub use demography::{DemographyConfig, HouseholdSpec};
pub use era::{
    Era, EraDetector, DEFAULT_ERA_WINDOW, DEFAULT_MIN_BARTER_VOLUME, DEFAULT_MIN_PRODUCER_SHARE_BPS,
};
pub use region::{Region, RegionConfig, RegionTickReport, RoadPlan, Route};
pub use settlement::{
    capital_build_outcome_for_culture, capital_build_outcome_with_forecast, recipe_adoption_pays,
    recipe_adoption_pays_for_money, rival_subsistence_commons_regen_for_phi, AcquisitionChannels,
    BankConfig, BarterConfig, BenchSurface, BirthStockInjectionRecord, BirthStockSavingMode,
    BootstrapTraceSummary, CandidateAcceptances, CapitalBuildOutcome, CapitalDecision,
    CapitalDeclineReason, ChainConfig, CycleConfig, CycleKind, DirectIndirectAcceptances,
    EarnedProvisioningStats, EconTickReport, EntrantClassSale, EstateDestination, ForageCommons,
    InKindWageStats, InheritanceRegime, LandMarketSaleRow, LineageStats, MortalLandownerOwnerRow,
    NodeSpec, OrderStat, OwnerSurplusTelemetry, PriorityBasis, ProducerCash, ProducerRole,
    RivalSubsistenceCommonsState, SavingAllocationObs, SavingStockPhaseObs, SavingSupplyTick,
    SavingSupplyTotals, Settlement, SettlementConfig, ShareTenancyMode, ShareTenancyStats, TaxLevy,
    TaxPolicy, TenderBench, TenderPolicy, TraderEndowment, Vocation, WageLaborMode, WageLaborStats,
    WinnerIntent, ECON_TICKS_PER_YEAR, FAST_TICKS_PER_ECON_TICK, LAND_CARRYING_PERIOD,
    LAND_FORECLOSE_DISCOUNT_BPS, LAND_LIST_IDLE, LAND_MIN_RENT_HISTORY, LAND_PRICE_MIN,
    LAND_RENT_WINDOW, LAND_SALE_HISTORY_K, LAND_SALE_HISTORY_WEIGHT_BPS, LAND_VIABLE_CAP_FLOOR,
    LAND_VIABLE_REGEN_FLOOR, RIVAL_COMMONS_BASELINE_EMERGENCY_DRAW,
    RIVAL_COMMONS_BASELINE_FINAL_WINDOW_TICKS, RIVAL_COMMONS_K_TICKS,
    RIVAL_COMMONS_PHI_ABUNDANT_BPS, RIVAL_COMMONS_PHI_MARGINAL_BPS, RIVAL_COMMONS_PHI_SCARCE_BPS,
    SHARE_TENANCY_BPS_DEFAULT, SHARE_TENANCY_TERM_DEFAULT,
};

/// Read-only re-exports of the `econ`/`life` types that make up the settlement's
/// read surface — the return and element types of the accessors above, plus the
/// good ids a downstream reader names. The G2d `oikos` debug viewer renders
/// settlement state through these: the [`Society`] behind [`Settlement::society`]
/// and its trade tape's [`Trade`], the [`Agent`] behind `society().agents` and
/// its value scale's [`Want`] / [`WantKind`] / [`Horizon`], the realized-price
/// [`Gold`], the [`NeedState`] behind [`Settlement::need_of`], and [`GoodId`] /
/// [`FOOD`] / [`WOOD`]. These are pure re-exports: they add public surface but
/// change no behavior, so the econ conformance goldens and the G1/G2a/G2b suites
/// stay byte-identical (the unchanged workspace `cargo test` is the proof).
/// Keeping them on `sim` lets the viewer depend on `sim` alone — a thin binary
/// over one crate — instead of reaching into `econ`/`life` directly.
pub use econ::agent::{Agent, AgentId, Want, WantKind};
pub use econ::good::{Gold, GoodId, Horizon, FOOD, GOLD, SALT, WOOD};
pub use econ::ledger::MoneyStock;
pub use econ::market::Trade;
pub use econ::society::Society;
pub use life::{
    savings_ladder_depth, CultureParams, NeedState, FORECAST_BIAS_MAX_BPS, FORECAST_BIAS_MIN_BPS,
    FORECAST_BIAS_NEUTRAL_BPS,
};
/// The `world` resource-node id — the return type of [`Settlement::node_of`] /
/// [`Settlement::grain_node`], so the S6 acceptance suite can name a colonist's
/// assigned node through `sim` alone. A pure re-export (no behavior change).
pub use world::NodeId;

/// Re-exports of the econ **tender** enums and their stable lowercase labels — the
/// G8c-2 tender-policy levers and the names the viewer renders. Pure re-exports of
/// econ's *unchanged* tender machinery (G8c-2 adds no tender logic to econ): the sim
/// routes each settlement surface through these, and the viewer/tests name them
/// through `sim` alone (the thin-binary-over-one-crate rule). See
/// [`settlement::TenderPolicy`].
pub use econ::money::{
    bank_repayment_tender_name, issuer_repayment_tender_name, labor_wage_tender_name,
    public_debt_tender_name, public_spot_tender_name, tax_receivability_name, BankRepaymentTender,
    IssuerRepaymentTender, LaborWageTender, PublicDebtTender, PublicSpotTender, TaxReceivability,
};
