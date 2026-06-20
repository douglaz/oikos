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
use std::collections::{BTreeMap, BTreeSet};

use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::agio::{provisioning_bitmap_for_money, TemporalEndowment};
use econ::bank::{Bank, BankPolicy};
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
use econ::menger::MengerianEmergence;
use econ::money::{
    BankRepaymentTender, DesignatedMoney, IssuerRepaymentTender, LaborWageTender,
    MarketMoneyConfig, MengerianConfig, PublicDebtTender, PublicSpotTender, Regime,
    ReserveRatioBps, TaxReceivability,
};
use econ::project::{
    advance_project, build_mill_template, build_oven_template, complete_project_if_ready,
    start_project, Project, ProjectId, ProjectTemplate, ProjectTemplateId, Recipe, RecipeId, Tick,
};
use econ::purpose::{CreditLender, CreditSource, DebtPurpose, ProjectPlanId};
use econ::rng::Rng;
use econ::scenario::{
    builtin_market_scenario, Event, EventKind, MarketScenario, RedemptionRoute, ScenarioName,
};
use econ::shadow::run_credit_disabled_shadow;
use econ::society::Society;
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
            subsistence_on_grain: false,
            // Own-labor subsistence off by default (S12): the food mints stay, no FORAGE
            // is produced, and the forage knobs are inert — so every existing config and
            // its goldens are byte-identical.
            own_labor_subsistence: false,
            forage_yield: 0,
            forage_hunger_in: 8,
            forage_hunger_out: 4,
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
            subsistence_on_grain: false,
            own_labor_subsistence: false,
            forage_yield: 0,
            forage_hunger_in: 8,
            forage_hunger_out: 4,
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
                salt_direct_use_qty: 0,
                salt_direct_use_period: 0,
            }),
            bank: None,
            cycle: None,
            tender_bench: None,
            tax: None,
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
                salt_direct_use_qty: 0,
                salt_direct_use_period: 0,
            }),
            bank: None,
            cycle: None,
            tender_bench: None,
            tax: None,
        }
    }

    /// EXPERIMENTAL (progression probe — not a golden path): `frontier` with the
    /// whole productive bundle scaled by `scale` — the food supply (grain/WOOD
    /// node regen, cap, stock), the gathering labor force, and the chain
    /// processing throughput — under a fixed-generous demographic headroom held
    /// CONSTANT across scales (so demography is never the binding cap and is not
    /// the variable under test). It answers one question: is the colony's
    /// long-run equilibrium carrying-capacity-bound (output and sustained
    /// population rise ~linearly with the productive bundle) or pinned by a fixed
    /// cap (they saturate)? Additive and game-only; the six econ goldens and
    /// every existing scenario are untouched.
    pub fn frontier_probe(scale: u32) -> Self {
        let scale = scale.max(1);
        let mut cfg = Self::frontier();
        for node in &mut cfg.nodes {
            node.regen = node.regen.saturating_mul(scale);
            node.cap = node.cap.saturating_mul(scale);
            node.stock = node.stock.saturating_mul(scale);
        }
        let scale_u16 =
            |n: u16| -> u16 { ((n as u32).saturating_mul(scale)).min(u16::MAX as u32) as u16 };
        cfg.gatherers = scale_u16(cfg.gatherers);
        cfg.consumers = scale_u16(cfg.consumers);
        if let Some(chain) = cfg.chain.as_mut() {
            chain.throughput = chain.throughput.saturating_mul(scale);
            chain.millers = scale_u16(chain.millers);
            chain.bakers = scale_u16(chain.bakers);
            chain.latent_millers = scale_u16(chain.latent_millers);
            chain.latent_bakers = scale_u16(chain.latent_bakers);
            chain.bread_buffer = chain.bread_buffer.saturating_mul(scale);
            chain.latent_flour_seed = chain.latent_flour_seed.saturating_mul(scale);
        }
        if let Some(d) = cfg.demography.as_mut() {
            // Generous, constant headroom: demography never binds, so any change
            // in the equilibrium across scales comes from carrying capacity, not
            // from a demographic ceiling.
            d.max_household_size = 60;
        }
        cfg
    }

    /// EXPERIMENTAL (millisats / divisibility probe — not a golden path):
    /// `frontier` redenominated into a `precision`-times-finer money unit (the
    /// Lightning-millisat idea — same real economy, many more money units). It
    /// scales every money-denominated SUPPLY/WANT in the barter config (the SALT
    /// endowments and the medium want) by `precision`, leaving goods, recipes,
    /// labor, and demography identical. The point: the post-promotion savings
    /// demand is a count of single money-unit wants capped at `MAX_SAVE_UNITS`
    /// (life::scale) — a NOMINAL, unit-denominated demand. With only a few
    /// hundred money units in the base `frontier`, a handful of patient savers
    /// corner the whole supply and circulation freezes. A finer unit gives the
    /// economy enough units that the same capped nominal savings demand can no
    /// longer absorb the supply. Additive and game-only; econ goldens untouched.
    pub fn frontier_millisats(precision: u32) -> Self {
        let precision = precision.max(1);
        let mut cfg = Self::frontier();
        if let Some(b) = cfg.barter.as_mut() {
            b.consumer_medium_endowment = b.consumer_medium_endowment.saturating_mul(precision);
            b.gatherer_medium_endowment = b.gatherer_medium_endowment.saturating_mul(precision);
            b.medium_want_qty = b.medium_want_qty.saturating_mul(precision);
        }
        cfg
    }

    /// EXPERIMENTAL (no-pure-consumer probe — not a golden path): `frontier`
    /// with the pure-consumer class removed. The "consumers" — agents that hold
    /// the money, eat, and never produce — are folded into the gathering labor
    /// force, and the SALT money endowment is moved onto the gatherers (total
    /// supply preserved). The controlled variable vs `frontier` is ONLY who
    /// holds the money: working gatherers instead of a non-producing consumer
    /// class; the chain, food model, nodes, and demography are otherwise
    /// identical. It tests whether segregating money from production (the
    /// consumer class) is what causes the circular-flow cold-start deadlock —
    /// the producer-working-capital finding in `docs/experiment-money-
    /// circulation.md`. Additive and game-only; econ goldens untouched.
    pub fn frontier_no_consumers() -> Self {
        let mut cfg = Self::frontier();
        let ex_consumers = u32::from(cfg.consumers);
        // Fold the removed consumers into the gathering labor force so the
        // population and the number of mouths are preserved; only their role
        // (idle money-holder -> producing gatherer) changes.
        cfg.gatherers = cfg.gatherers.saturating_add(cfg.consumers);
        cfg.consumers = 0;
        if let Some(b) = cfg.barter.as_mut() {
            // Move the SALT endowment from the removed consumers onto the
            // gatherers, preserving the total supply: money is now held by
            // producers, not an idle consumer class.
            let total_salt = ex_consumers.saturating_mul(b.consumer_medium_endowment);
            let gatherers = u32::from(cfg.gatherers).max(1);
            b.gatherer_medium_endowment = total_salt / gatherers;
            b.consumer_medium_endowment = 0;
        }
        cfg
    }

    /// EXPERIMENTAL (subsistence floor — not a golden path): `frontier` with raw
    /// grain made a directly-edible subsistence food (`subsistence_on_grain`),
    /// so the grain→flour→bread chain is **optional specialization on top of a
    /// subsistence base** rather than the sole food source. Colonists prefer
    /// bread but eat the raw grain they already over-gather to survive when the
    /// chain stalls. Tests whether a subsistence floor keeps the colony fed (no
    /// chronic-hunger collapse) over a long horizon while specialization still
    /// emerges — the synthesis of `docs/experiment-money-circulation.md`.
    /// Additive and game-only; econ goldens untouched.
    pub fn frontier_subsistence() -> Self {
        let mut cfg = Self::frontier();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.subsistence_on_grain = true;
        }
        cfg
    }

    /// EXPERIMENTAL (capital-advance probe — not a golden path): `frontier` on a
    /// finer money unit (built from [`Self::frontier_millisats`] so concentration
    /// and the integer price floor are not confounds), plus the conserved
    /// capital-advance phase ([`ChainConfig::capital_advance`]). It isolates the
    /// producer-working-capital thesis: after promotion, cashless active
    /// producers are funded from the richest saver so they can buy inputs. If
    /// missing working capital is the binding cause, the chain keeps producing
    /// past the seed-exhaustion tick instead of stalling. Additive and game-only;
    /// econ goldens untouched.
    pub fn frontier_capital_advance() -> Self {
        let mut cfg = Self::frontier_millisats(1_000);
        if let Some(chain) = cfg.chain.as_mut() {
            chain.capital_advance = true;
        }
        cfg
    }

    /// EXPERIMENTAL (spoilage / inventory carrying cost — not a golden path):
    /// `frontier_capital_advance` (the revolving working-capital loan) PLUS
    /// per-tick spoilage on the perishable chain foods. Codex's primary fix for
    /// the distribution-seizure halt: with the loan supplying working capital and
    /// spoilage forcing satiated holders' bread/grain hoards back into
    /// circulation (hunger returns, raw grain must sell before it rots), the test
    /// is whether production sustains past the ~tick-300 halt without the colony
    /// bifurcating into a hoarding consumer class and a starving producer class.
    /// Additive and game-only; econ goldens untouched.
    pub fn frontier_spoilage() -> Self {
        let mut cfg = Self::frontier_capital_advance();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.perishable_decay_bps = 2_000;
        }
        cfg
    }

    /// EXPERIMENTAL (in-kind subsistence advance — not a golden path): the
    /// revolving working-capital loan (`frontier_capital_advance`) PLUS an in-kind
    /// staple-food advance to hungry producers (`ChainConfig::subsistence_advance`).
    /// The live order-book trace (Experiment 9) proved a loan-funded but hungry
    /// miller posts no grain bid because its money is reserved for its own unmet
    /// bread want; feeding it in kind frees that money so it buys grain and the
    /// chain runs. The faithful fix: a saver advances both money (loan) and
    /// present goods (food) to the producer, keeping each worker's value scale
    /// intact. Additive and game-only; econ goldens untouched.
    pub fn frontier_in_kind() -> Self {
        let mut cfg = Self::frontier_capital_advance();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.subsistence_advance = true;
        }
        cfg
    }

    /// EXPERIMENTAL (in-kind INPUT advance — not a golden path): the in-kind
    /// subsistence colony (`frontier_in_kind`: loan + food in kind, so the colony
    /// stays fed) PLUS the in-kind **input** advance
    /// ([`ChainConfig::input_advance`]) — a capitalist buys each producer's recipe
    /// input in kind and places it in its hands, so production runs without the
    /// producer having to out-rank its own savings to bid for inputs (the residual
    /// blocker from Experiment 10). Tests whether placing inputs makes the
    /// production chain self-sustain past the halt. Additive and game-only; econ
    /// goldens untouched.
    pub fn frontier_input_advance() -> Self {
        let mut cfg = Self::frontier_in_kind();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.input_advance = true;
        }
        cfg
    }

    /// EXPERIMENTAL (the subsistence→specialization arc — not a golden path): the
    /// full in-kind capital-advance colony (`frontier_input_advance`, which
    /// advances the loan, food, and inputs in kind on a fed subsistence base) plus
    /// the recurring owner-operator motive ([`ChainConfig::recurring_motive`]). The
    /// recurring motive stops producers retiring once their savings fill (the
    /// satiation wall that collapsed Experiment 11), so — with inputs placed and
    /// the colony fed — specialization that emerges from the subsistence base can
    /// *sustain*: a self-employment economy, no firms. Tests the whole arc:
    /// subsistence, emergent money, then sustained specialized production. Additive
    /// and game-only; econ goldens untouched.
    pub fn frontier_economy() -> Self {
        let mut cfg = Self::frontier_input_advance();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.recurring_motive = true;
        }
        cfg
    }

    /// EXPERIMENTAL (ablation — `economy` minus the in-kind INPUT advance): loan +
    /// food-in-kind + recurring motive, but producers must acquire inputs through
    /// the **market** (no `input_advance`). If tail production collapses relative
    /// to `frontier_economy`, the sustained chain was mostly scripted input
    /// placement, not market coordination (Codex's sharp ablation). Game-only.
    pub fn frontier_economy_no_input() -> Self {
        let mut cfg = Self::frontier_in_kind();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.recurring_motive = true;
        }
        cfg
    }

    /// EXPERIMENTAL (endogenous ablation — recurring motive ALONE): the divisible-
    /// money base plus only `recurring_motive` — NO curated capital/food/input
    /// advances. The market, latent producers, and subsistence wants are as in the
    /// base colony. Tests whether specialization sustains **endogenously** (inputs
    /// acquired by market trade, not placed). The falsification of the "economy"
    /// being self-organizing: if this does not sustain, `frontier_economy` is
    /// scaffolded, not endogenous. Game-only; econ goldens untouched.
    pub fn frontier_recurring_only() -> Self {
        let mut cfg = Self::frontier_millisats(1_000);
        if let Some(chain) = cfg.chain.as_mut() {
            chain.recurring_motive = true;
        }
        cfg
    }

    /// EXPERIMENTAL (the ENDOGENOUS economy — the genuine test): divisible money,
    /// a revolving working-capital loan, the recurring owner-operator motive, and
    /// project-aware input bids — but **NO curated food or input placement**. The
    /// producer feeds and supplies itself through the market: it borrows working
    /// capital, **buys** its input at an imputed price from a willing seller
    /// (`project_input_bids`), mills/bakes, and sells the output. If the chain
    /// sustains here, specialization is **self-organizing**, not scaffolded — the
    /// falsification of the Experiment-12 "scaffolded" verdict. Game-only; econ
    /// goldens untouched.
    pub fn frontier_endogenous() -> Self {
        // The ENDOGENOUS economy (the S5 DoD): the grain→flour→bread division of
        // labor emerges atop a HOUSEHOLD/SUBSISTENCE base and sustains on REAL
        // MARKET TRADE, with NO chain-specific global placement.
        //
        // Base (local/household allocation — allowed, not scaffolding):
        // - the household demography hearth feeds the consumer lineages their staple
        //   + WOOD (`deliver_demography_provisions`), and reproduces/ages them;
        // - each chain producer feeds from its OWN local hearth
        //   (`producer_subsistence`: staple + WOOD) so its money frees ENTIRELY for
        //   recipe inputs rather than its own subsistence;
        // - raw grain is a directly-edible subsistence floor (`subsistence_on_grain`)
        //   — the roundabout bread chain is OPTIONAL specialization on top of it.
        //
        // Coordination (S1–S4, all real market trade):
        // - producers BUY their recipe inputs on the real order book at the imputed
        //   bundle reservation (`project_input_bids`, S1/S2), restocking only as they
        //   clear their output (working-capital discipline, S3);
        // - `recurring_motive` keeps an owner-operator producing while profitable;
        // - the cold-start buffers (`latent_flour_seed`, `bread_buffer`) seed the
        //   first realized flour/bread prices so latent millers→bakers adopt in
        //   pipeline order (S4).
        //
        // NO curated scaffolds: NO per-tick planner loan (`capital_advance` off —
        // working capital is real retained earnings), NO global food redistribution
        // (`subsistence_advance` off), NO global input placement (`input_advance`
        // off). A designated-GOLD market (`barter = None`) so the study is the chain,
        // not money emergence (that is G5a/G5b); the money supply is the colonists'
        // starting gold, which circulates rather than pooling.
        let mut cfg = Self::frontier();
        cfg.barter = None;
        cfg.starting_gold_gatherer = 60;
        cfg.starting_gold_consumer = 60;
        if let Some(chain) = cfg.chain.as_mut() {
            chain.recurring_motive = true;
            chain.project_input_bids = true;
            chain.subsistence_on_grain = true;
            chain.producer_subsistence = 4;
            chain.producer_gold = 16;
            // Smaller bread/flour bootstrap than the barter frontier (no barter
            // window to bridge): enough to seed the first prices, not so much that
            // reshuffling the buffer drowns out new production.
            chain.bread_buffer = 8;
            chain.consumer_staple_buffer = 2;
            chain.latent_flour_seed = 12;
            // Threshold carrying cost on the staple + raw-grain HOARDS (working
            // stock under the free-storage floor is exempt): a satiated holder's
            // bread/grain pile decays, so hunger recurs and the holder re-enters
            // the market — keeping demand (and the chain) running rather than
            // letting bread/grain accumulate unbounded. Bounds every stock, so the
            // colony is genuinely stationary, not slowly hoarding.
            chain.perishable_decay_bps = 1_500;
        }
        if let Some(demo) = cfg.demography.as_mut() {
            demo.child_gold_endowment = 16;
            for household in &mut demo.households {
                household.starting_gold = 60;
            }
        }
        cfg
    }

    /// THE SCALING ECONOMY (the S6 DoD): the endogenous economy
    /// ([`Self::frontier_endogenous`]) plus **productive re-entry** turned ON, so no
    /// hungry, unprovisioned colonist is left permanently stranded. A hungry spatial
    /// non-lineage colonist — an idle consumer or a WOOD-mis-allocated gatherer —
    /// adopts edible-grain gathering on its own value scale and feeds itself, and a
    /// fed re-entrant resumes WOOD gathering (the S6.2 hysteresis keeps WOOD alive).
    /// To exercise *growth* (not just the fixed stranded set), it also seeds a
    /// **larger colony** (more consumers + gatherers) and raises the household-size
    /// cap so population climbs further while provisioning keeps pace. Everything else
    /// is the endogenous economy: the grain→flour→bread chain still self-organizes and
    /// sustains on real market trade, with NO chain-specific global placement.
    pub fn frontier_endogenous_scaling() -> Self {
        let mut cfg = Self::frontier_endogenous();
        if let Some(chain) = cfg.chain.as_mut() {
            // The gated phase, ON. Re-enter at chronic hunger (the stranded tail sits
            // at the need ceiling) and revert once comfortably fed — a wide band so a
            // re-entrant holds its node for many ticks rather than thrashing.
            chain.productive_reentry = true;
            chain.reentry_hunger_in = 8;
            chain.reentry_hunger_out = 4;
        }
        // A larger non-lineage base — the stranded set re-entry must provision — so
        // the metric is exercised at scale, not only on the fixed 4 consumers + 4 WOOD
        // gatherers of the endogenous roster.
        cfg.consumers = 8;
        cfg.gatherers = 12;
        // Let the lineages grow further so total population climbs above the
        // endogenous plateau (the "tracks a growing population" half of the DoD).
        if let Some(demo) = cfg.demography.as_mut() {
            demo.max_household_size = 8;
        }
        cfg
    }

    /// THE CAPITAL ECONOMY (the S7 DoD): the scaling economy
    /// ([`Self::frontier_endogenous_scaling`]) plus **producible capital goods** — the
    /// tooled grain→flour→bread chain can now GROW, not just the untooled gathering
    /// base. Under the larger colony's sustained unmet bread demand a fed, non-latent
    /// colonist appraises that building a mill/oven will pay, invests its own saved
    /// WOOD + labor in a conserved build (S7.2), then — holding the new tool — is
    /// admitted to the adoption appraisal (S7.1), adopts, buys its input on the real
    /// market, and produces. So bread output tracks demand rather than flat-lining at
    /// the seeded tool count, with NO planner placement of tools and NO over-building
    /// (capital formation stops when demand is met). Everything else is the scaling
    /// economy: re-entry still provisions the untooled tail, and the chain still
    /// self-organizes on real market trade.
    pub fn frontier_capital() -> Self {
        let mut cfg = Self::frontier_endogenous_scaling();
        if let Some(chain) = cfg.chain.as_mut() {
            // S7.1: a colonist that holds a mill/oven is admitted to the adoption
            // appraisal (and anchors the tool so it is never sold before it adopts).
            chain.tool_acquisition_eligibility = true;
            // S7.2: the per-builder BuildMill/BuildOven phase, on. A modest WOOD/labor
            // cost (a WOOD-gatherer or a hearth-provisioned lineage member can save it)
            // amortized over a generous payback window, so building pays under genuine
            // unmet bread demand yet stops once the spread thins (the overinvestment
            // guard). A fed colonist (hunger at/below the comfortable revert level)
            // invests; a hungry one feeds first.
            chain.producible_capital = true;
            chain.capital_payback_cycles = 16;
            chain.tool_build_wood = 6;
            chain.tool_build_labor = 4;
            chain.capital_build_hunger_max = 4;
        }
        // A larger consumer base than `scaling` — more mouths than the seeded
        // grain→flour→bread chain (3 latent millers + 3 latent bakers) can feed — so
        // bread demand genuinely OUTRUNS the seeded tool count and there is real room
        // for built capital to raise output. Without producible capital this same colony
        // (the test control) leaves bread demand unmet; with it, builders add the
        // bottleneck producers until demand is met. The extra WOOD gatherers keep the
        // builders' WOOD (and the warmth battery) supplied as capital is committed.
        cfg.consumers = 44;
        cfg.gatherers = 24;
        cfg
    }

    /// THE CO-EMERGENT ECONOMY (the S8 DoD): money, the grain→flour→bread division
    /// of labor, and capital all CO-EMERGE in one run — with NO designated money and
    /// NO curated placement. Unlike [`Self::frontier_endogenous`] (which is HANDED
    /// designated GOLD and only then calculates, bids, and builds), this starts from
    /// the barter-start emergent base [`Self::frontier`] — `barter = Some(..)`, the
    /// SALT medium, **every gold endowment zero** — and lets SALT promote by
    /// saleability from real indirect exchange. After promotion the (money-good-
    /// agnostic) S5 sustain stack and the S7 capital phase run on the EMERGED unit.
    ///
    /// What it adds to [`Self::frontier`] (all of which thread `current_money_good`,
    /// never hard-coded GOLD — Base Fact 1):
    /// - the S5 sustain stack: `recurring_motive` (an owner-operator keeps producing
    ///   while profitable), `project_input_bids` (producers BUY recipe inputs at the
    ///   imputed reservation, S1/S2), and threshold spoilage (`perishable_decay_bps`)
    ///   so a satiated holder's bread/grain pile decays and demand recurs;
    /// - the local `producer_subsistence` hearth set to a **partial** floor (2, not
    ///   the endogenous economy's 4): enough to free a producer's emerged money for
    ///   inputs across the cutover, but NOT so much that a fully-fed producer hoards
    ///   its whole margin and drains the (scarce) emerged money out of circulation —
    ///   the balance that lets the chain *sustain* on emerged money rather than freeze
    ///   once the post-promotion money pulse is absorbed (see the S8 finding below);
    /// - a **lean demographic hearth** (`food_provision`/`wood_provision` = 1, not 3):
    ///   a hearth-fed lineage that mints a large staple/WOOD *surplus* sells it for the
    ///   emerged money and — being fed — hoards the cash, a money sink that scales with
    ///   the supply and starves the productive loop. Trimming the surplus keeps the
    ///   scarce emerged money circulating (the colony is still hearth-fed and still
    ///   reproduces; it just no longer pumps money into idle savings).
    ///
    /// What it deliberately does NOT do (Base Fact 6 / the two tensions):
    /// - `subsistence_on_grain` stays OFF — a raw-grain floor would thin the
    ///   bread-for-SALT trade that monetizes SALT (Tension A), so it would starve
    ///   promotion; S6 productive re-entry stays OFF too (it is inert without the
    ///   grain floor and would re-enable the crowd-out with it);
    /// - it seeds NO money: `producer_gold` stays 0 and every gold endowment is zero
    ///   (the barter overlay asserts it at generation). A producer's working capital
    ///   across the cutover is EARNED — it sells its seeded cold-start output into the
    ///   real money market post-promotion, no curated advance (the S8.2 finding: the
    ///   `frontier` saleability hub concentrates SALT in consumers who barely spend it
    ///   before the fast promotion, so producers earn ~no *barter* SALT — Base Fact 5 —
    ///   yet the chain survives the cutover on these post-promotion earnings anyway).
    /// - NO curated placement (`subsistence_advance`/`input_advance`/`capital_advance`
    ///   all off). A modest colony (≈ the endogenous size, not the S6 scaling colony):
    ///   provisioning-at-scale under emergence is deferred (S9).
    pub fn frontier_coemergent() -> Self {
        let mut cfg = Self::frontier();
        if let Some(chain) = cfg.chain.as_mut() {
            // The S5 sustain stack on emerged money (the only GOLD-hardcoded path,
            // the unused `recipe_adoption_pays` wrapper, is never called here).
            chain.recurring_motive = true;
            chain.project_input_bids = true;
            // Threshold carrying cost on the staple + raw-grain HOARDS (working stock
            // under the free-storage floor is exempt): satiated piles decay so hunger
            // recurs, demand stays alive, and stocks stay bounded — the same lever the
            // endogenous economy uses, here keeping the post-promotion chain churning.
            chain.perishable_decay_bps = 1_500;
            // A PARTIAL local producer hearth (staple + WOOD): each active producer
            // feeds mostly from its own hearth so its emerged money frees for recipe
            // inputs — but not fully (2, vs the endogenous 4), so it still buys some
            // food and recirculates its margin rather than hoarding it. And (unlike
            // `subsistence_on_grain`) it adds no raw-grain consumer floor that would
            // crowd out the bread-for-SALT trade (Base Fact 6).
            chain.producer_subsistence = 2;
            // S8.3 — the S7 producible-capital phase, composed onto the EMERGED money:
            // a colonist holding a mill/oven is admitted to adoption (S7.1) and a fed
            // colonist can BUILD a mill/oven from its own WOOD + labor (S7.2), all
            // priced in the emerged unit. The build cost is DEARER than the S7 scaling
            // colony's (12 WOOD, not 6) over a longer payback window (32, not 16): the
            // small co-emergent colony's high producer margins would otherwise justify
            // runaway building, so a higher real-resource bar keeps capital formation
            // modest and demand-anchored (a few tools, then it stops) rather than an
            // over-build that drains WOOD and destabilizes the chain.
            chain.tool_acquisition_eligibility = true;
            chain.producible_capital = true;
            chain.capital_payback_cycles = 32;
            chain.tool_build_wood = 12;
            chain.tool_build_labor = 4;
            chain.capital_build_hunger_max = 4;
            // `subsistence_on_grain`, `productive_reentry`, `producer_gold = 0`, and
            // every gold endowment stay at the `frontier` (barter-start) defaults.
        }
        // Trim the demographic hearth to a lean floor (1, not 3): the hearth still
        // feeds and reproduces the lineage, but no longer mints a large surplus the
        // fed lineage sells and hoards — the money sink that otherwise drains the
        // scarce emerged supply out of the productive loop.
        if let Some(demo) = cfg.demography.as_mut() {
            for household in &mut demo.households {
                household.food_provision = 1;
                household.wood_provision = 1;
            }
        }
        cfg
    }

    /// S9 — the STRONG-BAR emergence experiment, derived from
    /// [`Self::frontier_coemergent`] (never mutating it). It removes the remaining
    /// circularity Codex flagged in S8: SALT promoted there only because every
    /// colonist was configured to want SALT *as a medium* (`medium_want_qty`), i.e.
    /// to desire it as money before it was money. Here that pre-monetary medium want
    /// is **off** (`medium_want_qty = 0`); instead SALT is given a modest,
    /// **heterogeneous real direct use** (a `Good(SALT)/Now` consumption want on a
    /// subset of colonists, `salt_direct_use_period = 8` → ~1-in-8), and promotion is
    /// gated on genuine **indirect-exchange breadth** — a good monetizes only after
    /// enough indirect acceptances, by enough distinct indirect acceptors, for at
    /// least one end other than the good's own use. The Mengerian chain runs forward:
    /// heterogeneous direct use → saleability → provisional leader → indirect
    /// acceptance by the OTHERS → breadth gate → promotion. No designated money, no
    /// seeded gold, no re-added medium want.
    ///
    /// Observed result (`docs/impl-strong-bar-emergence.md`): money EMERGES — SALT
    /// promotes from real saleability across seeds, then the S8 chain + capital
    /// sustain on the emerged unit. The indirect demand concentrates on the staple
    /// (bread) — the one near-universal unmet want the colony re-trades SALT to reach
    /// — so the realized indirect-target breadth is one dominant end, which the gate
    /// requires (`min_indirect_target_goods = 1`) while the distinct-acceptor floor
    /// (`6`) rules out a few-agent churn.
    pub fn frontier_coemergent_strong() -> Self {
        let mut cfg = Self::frontier_coemergent();
        if let Some(barter) = cfg.barter.as_mut() {
            // Remove the circular pre-monetary medium want — SALT is no longer wanted
            // AS money before it is money. (Its physical endowment stays, so SALT is
            // still present to circulate and to convert 1:1 at promotion.)
            barter.medium_want_qty = 0;
            // The real, heterogeneous direct use that replaces it: one fixed
            // `Good(SALT)/Now` consumption want on ~1-in-8 colonists (the band that
            // both seeds saleability and leaves enough non-wanters to accept SALT
            // indirectly — Base Fact 6; a denser want would suppress indirect offers).
            barter.salt_direct_use_qty = 1;
            barter.salt_direct_use_period = 8;
            // The strong-bar promotion gate: real indirect-exchange breadth.
            // Withholds promotion (which the weak S8 bar fires by ~tick 19 on direct
            // churn alone) until SALT has accrued sustained indirect volume, spread
            // across distinct acceptors, for an end other than its own use.
            barter.menger.min_indirect_acceptances = 12;
            barter.menger.min_indirect_acceptor_agents = 6;
            barter.menger.min_indirect_target_goods = 1;
            // Indirect acceptance stays ON (the headline path); the
            // `allow_indirect_acceptance = false` control derives from here.
        }
        cfg
    }

    /// S10 — THE ORIGINARY-INTEREST ECONOMY (the flagship): the strong-bar co-emergent
    /// colony ([`Self::frontier_coemergent_strong`], never mutated) with **per-agent
    /// intertemporal capital choice** on. Money still EMERGES (SALT promotes from real
    /// indirect-exchange breadth), then the chain + capital sustain on the emerged unit —
    /// but capital now forms through a **per-colonist ORDINAL** decision instead of S7's
    /// settlement-level planner: each eligible colonist appraises, on its OWN value scale,
    /// whether committing present WOOD + forgone leisure to build a durable mill/oven whose
    /// recipe-margin receipt stream provisions one of its OWN future-money savings wants is
    /// worth it ([`appraise_capital_tool_bundle_for_money`]). Capital formation then tracks
    /// each colonist's `time_preference_bps` — patient colonists invest in the roundabout
    /// tooled chain, present-biased ones do not — with NO cardinal discount (originary
    /// interest expressed ordinally via the multi-horizon savings ladder), no global stage
    /// choice, and no first-eligible-builder assignment.
    ///
    /// Derived from the strong-bar base (only the build seam changes): `per_agent_capital`
    /// is switched on (which leaves `capital_payback_cycles` inert), the build's WOOD cost
    /// is trimmed from the co-emergent 12 to 6 so a colonist that has saved a modest WOOD
    /// surplus can fund it from its OWN endowment (the per-agent decision is the brake now,
    /// not a dear real-resource bar), and gatherers carry a slightly larger WOOD battery so
    /// a fed, rested saver actually accumulates the build's WOOD. Everything that makes SALT
    /// emerge (the heterogeneous direct use, the breadth gate, the WOOD-poor consumer hub)
    /// is untouched, so money still emerges from real saleability.
    pub fn frontier_coemergent_strong_originary() -> Self {
        let mut cfg = Self::frontier_coemergent_strong();
        if let Some(chain) = cfg.chain.as_mut() {
            // The per-agent ordinal decision replaces S7's build planner. The S7 gates
            // (tool-acquisition eligibility + producible capital) stay on — per-agent mode
            // steers the SAME per-builder substrate.
            chain.per_agent_capital = true;
            // Trim the build's WOOD cost (12 → 6): the per-agent appraisal is the brake
            // now (a colonist builds only when the tool provisions its own deep savings
            // want without breaking a higher one), so the build need not be made dear to
            // hold capital formation in check — it just has to be fundable from a saver's
            // own WOOD surplus.
            chain.tool_build_wood = 6;
            // A larger WOOD battery so a fed builder accumulates the build's WOOD without
            // running its own warmth short (removing the committed WOOD must not break a
            // higher-ranked warmth want, or the appraisal declines — the WOOD's present
            // use is one of the costs the future gain must outrank).
            chain.wood_buffer = 64;
        }
        cfg
    }

    /// S11 — THE ENTREPRENEURIAL-UNCERTAINTY ECONOMY (the flagship): the S10 originary
    /// base ([`Self::frontier_coemergent_strong_originary`], never mutated) with
    /// **per-agent fallible forecasts** on. Every entrepreneurial appraisal — the
    /// role-choice adopt, the per-agent capital build, the project input-bid — now weighs
    /// its OUTPUT-revenue estimate against the colonist's OWN grounded forecast (its
    /// adaptive [`PriceBelief`] tilted by the heritable
    /// [`CultureParams::forecast_bias_bps`]) instead of the shared last realized price.
    /// The market still clears at the REAL price, so an over-optimist that adopts/builds
    /// on an inflated forecast earns the real (lower) revenue: its committed WOOD/inputs
    /// are sunk and it ends with LESS capital to invest, while an accurate/conservative
    /// forecaster accumulates and expands — **profit/loss selection through capital, not
    /// mortality** (`hunger_critical` stays disabled). Money still EMERGES and the S10
    /// multi-horizon ladder + per-agent capital choice are intact (the originary base is
    /// untouched); only the appraisal's price expectation becomes individual and fallible.
    ///
    /// Derived from the originary base by flipping a single flag
    /// ([`ChainConfig::entrepreneurial_forecasts`]) — so with that flag reverted it is
    /// byte-identical to `frontier_coemergent_strong_originary`. The per-colonist forecast
    /// biases come from the heritable jitter around the neutral base
    /// ([`SettlementConfig::forecast_bias_base_bps`], left neutral here).
    pub fn frontier_coemergent_strong_entrepreneurial() -> Self {
        let mut cfg = Self::frontier_coemergent_strong_originary();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.entrepreneurial_forecasts = true;
        }
        cfg
    }

    /// S12 — THE PROVISIONED ECONOMY (the flagship): the S11 entrepreneurial co-emergent
    /// colony ([`Self::frontier_coemergent_strong_entrepreneurial`], never mutated) with
    /// **own-labor subsistence** on and the food mints retired. The exogenous food
    /// hearths that minted bread/staple with no labor (the producer-subsistence staple
    /// floor and the demographic `food_provision`) are gone; instead a hungry, eligible,
    /// unprovisioned colonist with spare labor **forages** a low-grade survival floor
    /// (the FORAGE good) from its OWN labor — booked `produced`, eaten at home, ranked
    /// BELOW bread. Everything that makes SALT emerge and the chain + capital + forecasts
    /// sustain is untouched, so the test is whether the colony can be both
    /// **bounded-hunger** (the forage floor feeds the surviving spatial tail) AND keep
    /// money emerging (bread stays the superior good that monetizes SALT). Derived by flipping the
    /// own-labor flags (and interning the FORAGE good into the content), so with them
    /// reverted it is byte-identical to the entrepreneurial base.
    ///
    /// The `forage_yield` is the default diagnostic yield the S12 sweep probes: enough
    /// to pull sustained spatial-tail hunger below the semi-hungry S9/S11 baseline, but
    /// not enough to rescue money emergence under the one-scalar food model.
    pub fn frontier_coemergent_strong_provisioned() -> Self {
        let mut cfg = Self::frontier_coemergent_strong_entrepreneurial();
        if let Some(chain) = cfg.chain.as_mut() {
            // Intern the FORAGE subsistence good (no recipe — produced from labor) and
            // turn on the own-labor path: the food mints retire and a hungry forager is
            // credited `forage_yield` FORAGE after completing a forage task.
            chain.content = chain.content.clone().with_forage();
            chain.own_labor_subsistence = true;
            // The survival-floor knob used by the S12 no-middle-band diagnostic.
            chain.forage_yield = 3;
            // Forage when hunger reaches the band's top, stop once comfortably fed —
            // wide enough that a gatherer does not thrash between foraging and WOOD.
            chain.forage_hunger_in = 6;
            chain.forage_hunger_out = 2;
        }
        cfg
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
            after
                == before + regen + endowment + produced
                    - consumed_as_input
                    - consumed
                    - promoted
                    - spoiled
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
    capital_payback_cycles: u32,
    tool_build_wood: u32,
    tool_build_labor: u32,
    capital_build_hunger_max: u16,
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

impl Settlement {
    /// Generate a settlement from `seed` and a [`SettlementConfig`]. All
    /// randomness (per-colonist culture) is drawn here; neither loop draws any.
    /// Deterministic: same `(seed, config)` → byte-identical settlement.
    pub fn generate(seed: u64, config: &SettlementConfig) -> Self {
        // G8c-1/G8c-2/G8c-3: a finance settlement (the credit cycle, a tender bench, or
        // the tax overlay on the cycle) is built from econ's unchanged scenario, not a
        // spatial colony — branch before the spatial setup. The guards live in
        // `generate_finance`.
        if config.cycle.is_some() || config.tender_bench.is_some() || config.tax.is_some() {
            return Self::generate_finance(seed, config);
        }
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
        // The directly-edible subsistence fallback a bread-staple chain ranks below
        // bread. S12 own-labor subsistence wires the FORAGE good (the labor-produced
        // floor); otherwise the legacy `subsistence_on_grain` raw-grain edibility (off
        // by default). Both are `KnownGoods::subsistence`, read back as hunger relief
        // and interleaved below the staple by the subsistence offset (`scale.rs`).
        let chain_subsistence = |chain: &ChainConfig| -> Option<GoodId> {
            if chain.own_labor_subsistence {
                // A flag set without a forage good degrades to off (None), matching
                // `own_labor_subsistence_can_run` (the per-tick gate) — so a misconfigured
                // flag is treated as off everywhere rather than panicking in this path
                // while the per-tick gate silently disables it.
                chain.content.forage()
            } else {
                chain.subsistence_on_grain.then(|| chain.content.grain())
            }
        };
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
                subsistence: chain_subsistence(chain),
            },
            (Some(chain), _) if chain.bread_is_staple => KnownGoods {
                hunger: chain.content.bread(),
                warmth: WOOD,
                savings: GOLD,
                subsistence: chain_subsistence(chain),
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
                subsistence: None,
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
                subsistence: None,
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
        // S12: the FORAGE node — a pure location marker for the `GoForage` task. It is
        // placed OUTSIDE `config.nodes` (so the gatherer round-robin never targets it)
        // and carries NO stock/regen/cap: foraging produces no world good (the floor is
        // credited at the econ layer, booked `produced`), so node regen stays the
        // world's only source and conservation is untouched. Placed at the exchange tile
        // ("eaten at home") only when own-labor subsistence is on, so every other config
        // adds no node and stays byte-identical.
        // Place the node only when the own-labor path can actually run (the flag AND a
        // forage good in the content), matching `own_labor_subsistence_can_run`; a flag
        // set without a forage good degrades to off (no node) rather than panicking.
        if let Some(forage) = config
            .chain
            .as_ref()
            .filter(|chain| chain.own_labor_subsistence)
            .and_then(|chain| chain.content.forage())
        {
            world
                .add_node(ResourceNode::new(config.exchange, forage, 0, 0, 0))
                .expect("the forage node lands on the (passable) exchange tile");
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
            let culture = draw_culture(
                &mut rng,
                tp_base,
                config.leisure_weight_base_bps,
                config.forecast_bias_base_bps,
                config.forecast_bias_jitter_bps,
            );
            let need = NeedState::rested();
            agents.push(build_agent(
                id, &need, &culture, &known, vocation, latent, config,
            ));
            colonists.push(Colonist {
                id,
                vocation,
                node,
                // S6: a spatial colonist's home role is its generated vocation+node.
                home_vocation: vocation,
                home_node: node,
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
                acquired_tool: false,
                foraging: false,
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
                        config.forecast_bias_base_bps,
                        config.forecast_bias_jitter_bps,
                    );
                    let need = NeedState::rested();
                    agents.push(build_demography_agent(id, &need, &culture, &known, spec));
                    colonists.push(Colonist {
                        id,
                        vocation: Vocation::Consumer,
                        node: None,
                        // A lineage founder is hearth-fed and never re-entered.
                        home_vocation: Vocation::Consumer,
                        home_node: None,
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
                        acquired_tool: false,
                        foraging: false,
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
            // S6 hysteresis invariant: for an active re-entry phase, the exit
            // threshold must sit strictly below the entry threshold. Otherwise a
            // re-entrant can satisfy both sides of the band and churn between grain
            // and its home role.
            assert!(
                !chain.productive_reentry
                    || !chain.subsistence_on_grain
                    || chain.reentry_hunger_out < chain.reentry_hunger_in,
                "re-entry hysteresis requires reentry_hunger_out < reentry_hunger_in"
            );
            // S12 hysteresis invariant: when the own-labor forage path can run, the
            // exit threshold must sit strictly below the entry threshold. Otherwise
            // a forager can satisfy both sides of the band and stay in a degenerate
            // always-forage state.
            assert!(
                !chain.own_labor_subsistence
                    || chain.content.forage().is_none()
                    || chain.forage_hunger_out < chain.forage_hunger_in,
                "own-labor subsistence hysteresis requires forage_hunger_out < forage_hunger_in"
            );
            // S7.2 prerequisite: a built tool is useless unless holding it makes the
            // builder eligible to adopt (S7.1), so producible capital requires the
            // tool-acquisition gate. The capital scenario composes both; this guards a
            // misconfiguration that would build tools no colonist could ever use.
            assert!(
                !chain.producible_capital || chain.tool_acquisition_eligibility,
                "producible capital (S7.2) requires tool-acquisition eligibility (S7.1)"
            );
            // S10: the per-agent intertemporal decision steers the SAME per-builder build
            // substrate, so it requires the producible-capital phase to be on (the
            // decision is meaningless without a build to start).
            assert!(
                !chain.per_agent_capital || chain.producible_capital,
                "per-agent capital (S10) requires producible capital (S7.2)"
            );
            assert!(
                !chain.per_agent_capital
                    || u64::from(chain.tool_build_labor) < max_savings_ladder_horizon(),
                "per-agent capital requires tool_build_labor below the deepest savings horizon"
            );
            ChainRuntime {
                content: chain.content.clone(),
                throughput: chain.throughput,
                operating_cost: chain.operating_cost,
                tier2_threshold: chain.tier2_threshold,
                tier2_recipe_id: chain.content.tier2_recipe_id(),
                scholar_grain_buffer: chain.scholar_grain_buffer,
                confectioner_flour_buffer: chain.confectioner_flour_buffer,
                capital_advance: chain.capital_advance,
                perishable_decay_bps: chain.perishable_decay_bps,
                subsistence_advance: chain.subsistence_advance,
                input_advance: chain.input_advance,
                recurring_motive: chain.recurring_motive,
                project_input_bids: chain.project_input_bids,
                producer_subsistence: chain.producer_subsistence,
                own_labor_subsistence: chain.own_labor_subsistence,
                forage_yield: chain.forage_yield,
                forage_hunger_in: chain.forage_hunger_in,
                forage_hunger_out: chain.forage_hunger_out,
                productive_reentry: chain.productive_reentry,
                reentry_hunger_in: chain.reentry_hunger_in,
                reentry_hunger_out: chain.reentry_hunger_out,
                tool_acquisition_eligibility: chain.tool_acquisition_eligibility,
                producible_capital: chain.producible_capital,
                per_agent_capital: chain.per_agent_capital,
                entrepreneurial_forecasts: chain.entrepreneurial_forecasts,
                capital_payback_cycles: chain.capital_payback_cycles,
                tool_build_wood: chain.tool_build_wood,
                tool_build_labor: chain.tool_build_labor,
                capital_build_hunger_max: chain.capital_build_hunger_max,
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
            capital_loans: BTreeMap::new(),
            capital_builds: Vec::new(),
            next_capital_project_id: 0,
            tools_built: 0,
            last_capital_decisions: Vec::new(),
            peak_pre_promotion_hunger: 0,
            critical_ticks_pre_promotion: 0,
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
            // S9: the heterogeneous real direct use of the medium (SALT). Active only
            // when both the consumption quantity and the heterogeneity period are set
            // (default off — `None` — for every pre-S9 scenario).
            salt_direct_use: config.barter.as_ref().and_then(|barter| {
                (barter.salt_direct_use_qty > 0 && barter.salt_direct_use_period > 0).then_some((
                    barter.medium_good,
                    barter.salt_direct_use_qty,
                    barter.salt_direct_use_period,
                ))
            }),
            // G6b: Knowledge starts at zero and tier 2 starts locked. A non-research
            // settlement never touches either (no scholar runs, the threshold is 0),
            // so its digest is byte-identical.
            knowledge: 0,
            tier2_unlocked_at: None,
            // G8b: the chartered-bank config (or `None`). A detached copy — the bank
            // entity itself lives in `society.banks`; this drives `run_bank_phase`.
            bank: config.bank,
            // G8c-1/G8c-2: a spatial settlement runs no credit cycle and no tender
            // bench (the finance path returns early from `generate`), so these are
            // always `None` here.
            cycle: None,
            shadow_cycle_cache: RefCell::new(None),
            bench: None,
            // G8c-3: a spatial settlement levies no tax (the finance path returns early
            // from `generate`), so the tax overlay is always absent here.
            tax: None,
        }
    }

    /// Generate a G8c-1 **finance** settlement: the Austrian business cycle (or its
    /// sound-money control) on econ's unchanged credit-ladder scenario. There is no
    /// spatial colony — the society IS the cycle, and [`Settlement::econ_tick`] just
    /// steps it. The shadow scenario is retained so the natural-rate baseline can be
    /// replayed on demand.
    fn generate_finance(seed: u64, config: &SettlementConfig) -> Self {
        // Scope: a finance settlement is either the G8c-1 credit cycle/control or a
        // G8c-2 tender bench. It requires the M3 ledger and is mutually exclusive with
        // every spatial overlay — its colony is empty by construction.
        assert!(
            config.m3,
            "a finance (G8c-1/G8c-2) settlement requires the M3 ledger (m3 = true)"
        );
        assert!(
            !(config.cycle.is_some() && config.tender_bench.is_some()),
            "a finance settlement is either the credit cycle or a tender bench, not both"
        );
        assert!(
            config.chain.is_none()
                && config.demography.is_none()
                && config.barter.is_none()
                && config.bank.is_none()
                && config.resident_traders.is_empty(),
            "a finance (G8c-1/G8c-2) settlement has no spatial overlay \
             (chain/demography/barter/bank/resident_traders); the demonstration runs in \
             the econ society"
        );
        assert!(
            config.gatherers == 0 && config.consumers == 0 && config.nodes.is_empty(),
            "a finance (G8c-1/G8c-2) settlement has no spatial colony (no \
             gatherers/consumers/nodes); use the credit_cycle / sound_money / \
             *_tender_* constructors"
        );
        // G8c-3: the tax overlay rides on the credit-cycle settlement (the chartalist
        // counter-lever to the wage refusal) — never on a tender bench, which exercises
        // a different surface. The levy/receivability route through econ's M21 machinery
        // on the cycle society.
        assert!(
            config.tax.is_none() || config.cycle.is_some(),
            "a G8c-3 tax overlay requires the credit cycle (use tax_in_fiat / tax_in_specie)"
        );

        // Build the society from econ's unchanged scenario — the credit-ladder cycle
        // (with its tender policy layered in) or a fiat-displacement tender bench. The
        // scenario is stamped with this run's seed so the demonstration is reproducible
        // per `(seed, config)`; the cycle additionally retains it (credit-disabled) for
        // the shadow replay.
        let (mut scenario, cycle_runtime, bench_runtime) = match (config.cycle, config.tender_bench)
        {
            (Some(cycle), None) => {
                let scenario = cycle_scenario(cycle.kind, cycle.tender);
                (scenario, Some(cycle.kind), None)
            }
            (None, Some(bench)) => {
                let scenario = tender_bench_scenario(bench);
                (scenario, None, Some(bench.surface))
            }
            _ => unreachable!(
                "the finance branch is taken only with a cycle, a bench, or a tax on the cycle \
                 (cycle and bench are asserted mutually exclusive above)"
            ),
        };
        // G8c-3: layer the tax overlay's M21 events (SetTaxReceivability + the levies)
        // onto the cycle scenario, before stamping the seed and building the society, so
        // the events flow into the society, the retained cycle scenario (canonical bytes
        // + shadow replay), and the run identically. A `None` overlay adds nothing, so a
        // tax-free cycle is byte-identical.
        let tax_runtime = config.tax.as_ref().map(|tax| {
            tax.apply_to(&mut scenario);
            TaxRuntime {
                receivability: tax.receivability,
                levied: tax.total_levied(),
            }
        });
        scenario.seed = seed;
        let mut society = Society::from_scenario(scenario.clone());
        society.enable_consumption_log();

        // A minimal spatial shell: an empty grid + an exchange stockpile so the
        // (no-op) world phases and the exchange accessor have a valid world to read.
        let grid = Grid::new(config.width.max(1), config.height.max(1));
        let mut world = World::new(grid);
        let exchange = world
            .add_stockpile(Stockpile::new(config.exchange, config.exchange_cap))
            .expect("exchange lands on a passable tile");

        Self {
            world,
            society,
            colonists: Vec::new(),
            live_colonist_slots: Vec::new(),
            colonist_slot_by_id: BTreeMap::new(),
            dynamics: config.dynamics,
            known: KnownGoods {
                hunger: FOOD,
                warmth: WOOD,
                savings: GOLD,
                subsistence: None,
            },
            exchange,
            carry_cap: config.carry_cap,
            // No spatial goods are tracked: the demonstration's goods live inside
            // econ's own (conserving) market + project machinery, and the finance
            // settlement's conservation is the M3 ledger reconcile + the fiat base
            // identity. An empty set makes the per-tick whole-system receipt vacuously
            // hold.
            goods: Vec::new(),
            money_rejection_goods: Vec::new(),
            pending_deposits: BTreeMap::new(),
            trader_ids: Vec::new(),
            chain: None,
            capital_loans: BTreeMap::new(),
            capital_builds: Vec::new(),
            next_capital_project_id: 0,
            tools_built: 0,
            last_capital_decisions: Vec::new(),
            peak_pre_promotion_hunger: 0,
            critical_ticks_pre_promotion: 0,
            econ_tick: 0,
            last_report: EconTickReport::default(),
            commons_gold: Gold::ZERO,
            commons_stock: BTreeMap::new(),
            demography: None,
            households: Vec::new(),
            birth_seq: 0,
            births_total: 0,
            old_age_deaths_total: 0,
            barter: None,
            barter_medium: None,
            salt_direct_use: None,
            knowledge: 0,
            tier2_unlocked_at: None,
            bank: None,
            cycle: cycle_runtime.map(|kind| CycleRuntime {
                kind,
                scenario: scenario.clone(),
            }),
            shadow_cycle_cache: RefCell::new(None),
            bench: bench_runtime.map(|surface| BenchRuntime { surface, scenario }),
            tax: tax_runtime,
        }
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
        let fast = self.run_fast_loop();
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
        self.record_pending_deposits(fast.deposited);
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
        for (agent, labor) in capital_labor_used {
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
        // ---- 7. SPOILAGE (EXPERIMENT): decay perishable food holdings, a real
        // sink recorded in `report.spoiled`. After all production/consumption so
        // it decays end-of-tick holdings, before the whole-system snapshot so the
        // conservation identity accounts it. A no-op unless enabled.
        self.run_spoilage(&mut report);
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
    /// per-good amounts deposited into the exchange stockpile this interval plus
    /// the agents that actually completed a forage task.
    ///
    /// Deposits are detected as carry **decreases**: a gatherer only ever
    /// deposits at the exchange and harvests at its node, and `world.tick` runs
    /// at most one arrival action per agent per tick, so a per-tick carry drop is
    /// exactly a deposit (the accepted amount — overflow stays carried). Escrow
    /// carried over from a previous interval is part of the opening carry, so it
    /// transfers on the arrival that finally lands it.
    fn run_fast_loop(&mut self) -> FastLoopReport {
        let mut deposited: BTreeMap<(AgentId, GoodId), u32> = BTreeMap::new();
        let mut foraged: BTreeSet<AgentId> = BTreeSet::new();
        let detect_forage = self.own_labor_subsistence_can_run();
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
            if detect_forage {
                let foraging_before: Vec<AgentId> = self
                    .live_colonist_slots
                    .iter()
                    .filter_map(|&slot| {
                        let id = self.colonists[slot].id;
                        matches!(self.world.agent_task(id), Some(Task::GoForage(_, _)))
                            .then_some(id)
                    })
                    .collect();
                self.world.tick();
                for id in foraging_before {
                    if self.world.agent_status(id) == Some(AgentStatus::Idle) {
                        foraged.insert(id);
                    }
                }
            } else {
                self.world.tick();
            }
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

        FastLoopReport { deposited, foraged }
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
    ///
    /// S12: a colonist marked `foraging` (a hungry, eligible forager — possibly a
    /// Gatherer, an idle Consumer, or a latent producer) is instead sent to deposit any
    /// carry it still holds and then [`Task::GoForage`] the FORAGE node — occupying its
    /// one world-task slot foraging *instead of* harvesting WOOD (the structural
    /// opportunity cost). Only spatial colonists reach the body (a non-spatial founder
    /// has no Idle world agent), and the foraging branch is taken only on the gated
    /// own-labor path (where `forage_node` resolves), so every other run is unchanged.
    fn assign_idle_gatherer_tasks(&mut self) {
        let forage_node = self.forage_node();
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            let id = colonist.id;
            if self.world.agent_status(id) != Some(AgentStatus::Idle) {
                continue;
            }
            if colonist.foraging {
                if let Some(forage_node) = forage_node {
                    let task = if self.world.agent_carry_total(id) > 0 {
                        Task::GoDeposit(self.exchange)
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
            if good == self.known.hunger || Some(good) == self.known.subsistence {
                // The preferred staple OR the directly-edible subsistence food
                // (e.g. raw grain) both reduce hunger. This is final
                // consumption (want satisfaction), not chain-input use, so
                // grain milled into flour is not counted here.
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
        // S12: own-labor subsistence retires the demographic FOOD mint (the food
        // scaffold) — only the WOOD/warmth provision stays an endowment (hunger-only
        // scope). The lineage then earns its food from the market (selling its WOOD
        // provision for the staple), exactly the retirement test 2 pins
        // (`endowment[staple] == 0`).
        let mint_food = !self.own_labor_subsistence_can_run();
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
            if mint_food {
                self.deliver_demography_provision_unit(id, staple, spec.food_provision, report);
            }
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
                // A newborn is a hearth-fed lineage member, never re-entered.
                home_vocation: Vocation::Consumer,
                home_node: None,
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
                acquired_tool: false,
                foraging: false,
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
                            Vocation::Miller | Vocation::Baker if chain.project_input_bids => 0,
                            Vocation::Miller | Vocation::Baker => chain.throughput.max(1),
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
            if let Some((good, qty, period)) = self.salt_direct_use {
                if self.society.current_money_good().is_none()
                    && period > 0
                    && colonist.id.index().is_multiple_of(u32::from(period))
                {
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

    /// CAPITAL-ADVANCE phase (EXPERIMENT — see [`ChainConfig::capital_advance`]).
    /// Once money has emerged, top up any cashless active chain producer
    /// (Miller/Baker) to a small working-capital floor by transferring real,
    /// conserved money from the richest saver — so the producer can buy inputs
    /// ahead of selling output. Funded (no fiduciary credit), no repayment yet:
    /// a causal probe of whether missing working capital is what stalls the
    /// chain. A no-op unless enabled and money has emerged, so every other run is
    /// byte-identical. Deterministic: integer state, id-ordered, no RNG; the
    /// donor is chosen by most free (unreserved) gold, ties broken by lowest id.
    fn run_capital_advance(&mut self) {
        let enabled = self
            .chain
            .as_ref()
            .is_some_and(|chain| chain.capital_advance);
        if !enabled || self.society.current_money_good().is_none() {
            return;
        }
        // Per-producer working-capital floor for one tick of input purchases.
        const FLOOR: u64 = 20;
        let live = self.live_colonist_slots.clone();
        for &slot in &live {
            let (producer_id, vocation) = {
                let colonist = &self.colonists[slot];
                (colonist.id, colonist.vocation)
            };
            if !matches!(vocation, Vocation::Miller | Vocation::Baker) {
                continue;
            }
            // One revolving loan at a time: re-borrow only once the prior loan is
            // repaid, so a producer's debt stays bounded by the floor.
            if self.capital_loans.contains_key(&producer_id) {
                continue;
            }
            let free = self.society.free_gold_after_all_reserves(producer_id).0;
            if free >= FLOOR {
                continue;
            }
            let need = FLOOR - free;
            // Richest lender by free (unreserved) gold — a saver, never a chain
            // producer, never the borrower; deterministic (ties -> lowest id).
            let lender = live
                .iter()
                .filter_map(|&lender_slot| {
                    let colonist = &self.colonists[lender_slot];
                    if colonist.id == producer_id
                        || matches!(colonist.vocation, Vocation::Miller | Vocation::Baker)
                    {
                        return None;
                    }
                    let free = self.society.free_gold_after_all_reserves(colonist.id).0;
                    (free > 0).then_some((free, colonist.id))
                })
                .max_by_key(|&(free, lender_id)| (free, std::cmp::Reverse(lender_id)));
            if let Some((free, lender_id)) = lender {
                let amount = need.min(free);
                if amount > 0 && self.move_money_conserved(lender_id, producer_id, Gold(amount)) {
                    self.capital_loans
                        .insert(producer_id, (lender_id, Gold(amount)));
                }
            }
        }
    }

    /// LOCAL PRODUCER SUBSISTENCE phase (S5 — the household/subsistence base, see
    /// [`ChainConfig::producer_subsistence`]). Before the market, top each chain
    /// producer (active Miller/Baker AND the latent pool that will adopt) up to a
    /// small staple-food floor, minting the staple FRESH from the producer's own
    /// renewable household hearth — exactly like [`Self::deliver_demography_provisions`]
    /// and NOT taken from any other agent. This is the LOCAL household allocation
    /// the endogenous milestone keeps (a producer's subsistence garden / its
    /// lineage's hearth), as distinct from the GLOBAL `run_subsistence_advance`
    /// redistribution (richest holder → producer) it turns OFF. A fed producer's
    /// money frees to bid for recipe inputs rather than reserve for its own hunger,
    /// and a latent producer survives the cold-start window to adopt. Conserved:
    /// the food is a source (`report.endowment`), eaten in the consume phase like
    /// any provision. Deterministic: slot order, integer; a no-op unless enabled.
    fn run_producer_subsistence(&mut self, report: &mut EconTickReport) {
        let target = self
            .chain
            .as_ref()
            .map_or(0, |chain| chain.producer_subsistence);
        if target == 0 {
            return;
        }
        let staple = self.known.hunger;
        // S12: own-labor subsistence retires the producer's STAPLE mint (the food
        // scaffold) — only the WOOD/warmth provision stays an endowment (hunger-only
        // scope). A producer then earns its food by buying bread or, when idle/too
        // hungry to produce, foraging, exactly like the rest of the tail.
        let mint_staple = !self.own_labor_subsistence_can_run();
        let live_len = self.live_colonist_slots.len();
        for live_index in 0..live_len {
            let slot = self.live_colonist_slots[live_index];
            let (id, vocation, latent) = {
                let colonist = &self.colonists[slot];
                (colonist.id, colonist.vocation, colonist.latent)
            };
            let is_producer = matches!(vocation, Vocation::Miller | Vocation::Baker)
                || (vocation == Vocation::Unassigned && latent.is_some());
            if !is_producer {
                continue;
            }
            // The producer's own hearth provisions the hunger staple AND WOOD (warmth)
            // up to the floor — exactly the two goods the demography hearth mints for
            // its members — so a producer's whole subsistence is met locally and its
            // money frees for recipe inputs. Under own-labor subsistence the staple
            // line is retired (only WOOD stays).
            for good in [staple, WOOD] {
                if good == staple && !mint_staple {
                    continue;
                }
                let held = self
                    .society
                    .agents
                    .get(id)
                    .map_or(0, |agent| agent.stock.get(good));
                if held < target {
                    self.deliver_demography_provision_unit(id, good, target - held, report);
                }
            }
        }
    }

    /// S12 — OWN-LABOR SUBSISTENCE phase (see [`ChainConfig::own_labor_subsistence`]).
    /// Before the market (so the floor is on hand to eat this tick), credit a hungry,
    /// eligible, **unprovisioned** colonist with spare labor only when it completed a
    /// [`Task::GoForage`] in the preceding fast loop. The credited
    /// [`ChainConfig::forage_yield`] units of the FORAGE subsistence good land in its
    /// OWN econ stock — booked `report.produced` (its own labor on the forage node),
    /// NOT `report.endowment` (a mint). The same call sets the colonist's `foraging`
    /// flag, which steers the NEXT fast loop to send it to [`Task::GoForage`] instead
    /// of harvesting WOOD (the structural opportunity cost). Eligible = a
    /// spatial non-lineage colonist (`household: None`) in an untooled-or-latent role
    /// (`Consumer`/`Gatherer`/`Unassigned` — NOT an actively-producing Miller/Baker that
    /// has no spare labor). Hysteresis (`forage_hunger_in`/`out`) keeps a gatherer from
    /// thrashing between foraging and selling WOOD. FORAGE is `KnownGoods::subsistence`,
    /// ranked below bread, eaten in the consume phase and read back as hunger relief —
    /// and perishes via [`Self::run_spoilage`] if a decay rate is set. A no-op unless
    /// the gated own-labor path is active, so every other run is byte-identical.
    /// Deterministic: slot order, integer thresholds, nothing drawn.
    fn run_own_labor_subsistence(
        &mut self,
        completed_forage: &BTreeSet<AgentId>,
        report: &mut EconTickReport,
    ) {
        if !self.own_labor_subsistence_can_run() {
            return;
        }
        let chain = self
            .chain
            .as_ref()
            .expect("the own-labor path carries a chain");
        let forage = chain
            .content
            .forage()
            .expect("the own-labor path carries a forage good");
        let yield_units = chain.forage_yield;
        let h_in = chain.forage_hunger_in;
        let h_out = chain.forage_hunger_out;
        let live = self.live_colonist_slots.clone();
        for slot in live {
            let (id, eligible, hunger, was_foraging) = {
                let colonist = &self.colonists[slot];
                // The spatial non-lineage poor with spare labor (`household: None`,
                // untooled-or-latent role). An actively-producing role (Miller/Baker/
                // Scholar/Confectioner) is excluded: it spends its one world-task slot
                // producing and is meant to earn its food by buying bread. TRACKED GAP:
                // with its staple mint retired, an active producer's only food path is
                // the bread market — unreachable on this path because SALT never
                // monetizes, so no active producer ever forms (asserted in
                // `producer_food_path_is_feasible`). Before any own-labor config that DOES
                // monetize (the differentiated-food / S13 follow-on), an active producer
                // must get a forage-when-too-hungry-to-produce path, or it would starve.
                // A lineage member is non-spatial (no world task) and fed through the market.
                let eligible = colonist.household.is_none()
                    && matches!(
                        colonist.vocation,
                        Vocation::Consumer | Vocation::Gatherer | Vocation::Unassigned
                    );
                (
                    colonist.id,
                    eligible,
                    colonist.need.hunger,
                    colonist.foraging,
                )
            };
            // Hysteresis: start foraging at/above `h_in`, stop below `h_out`, else hold.
            // A non-eligible colonist (a lineage member, or one that adopted an active
            // producer role) never forages and clears any stale flag.
            let forage_now = if !eligible {
                false
            } else if hunger >= h_in {
                true
            } else if hunger < h_out {
                false
            } else {
                was_foraging
            };
            self.colonists[slot].foraging = forage_now;
            if completed_forage.contains(&id) {
                self.credit_produced(id, forage, yield_units, report);
            }
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

    /// IN-KIND SUBSISTENCE ADVANCE phase (EXPERIMENT — see
    /// [`ChainConfig::subsistence_advance`]). Before the market, feed each hungry
    /// active chain producer (Miller/Baker) up to a small staple floor by
    /// transferring staple food **in kind** from the richest food-holder (a
    /// saver, never another producer, which keeps at least the same floor for
    /// itself). The live order-book trace proved a funded-but-hungry producer
    /// posts no input bid because its money is reserved for its own unmet bread
    /// want; provisioning that want frees the money so it bids for grain. The
    /// food moves holder→producer and is later eaten — conserved, no new sink.
    /// A no-op unless enabled and money has emerged. Deterministic: id-ordered,
    /// integer; donor chosen by most staple held (ties → lowest id).
    fn run_subsistence_advance(&mut self) {
        let enabled = self
            .chain
            .as_ref()
            .is_some_and(|chain| chain.subsistence_advance);
        if !enabled || self.society.current_money_good().is_none() {
            return;
        }
        // Staple floor that provisions a producer's present hunger ladder.
        const FEED_TARGET: u32 = 4;
        let staple = self.known.hunger;
        let live = self.live_colonist_slots.clone();
        for &slot in &live {
            let (producer_id, vocation) = {
                let colonist = &self.colonists[slot];
                (colonist.id, colonist.vocation)
            };
            if !matches!(vocation, Vocation::Miller | Vocation::Baker) {
                continue;
            }
            let held = self
                .society
                .agents
                .get(producer_id)
                .map_or(0, |agent| agent.stock.get(staple));
            if held >= FEED_TARGET {
                continue;
            }
            let need = FEED_TARGET - held;
            // Richest food-holder, never a producer, never the producer itself,
            // and keeping at least FEED_TARGET for its own subsistence.
            let donor = live
                .iter()
                .filter_map(|&donor_slot| {
                    let colonist = &self.colonists[donor_slot];
                    if colonist.id == producer_id
                        || matches!(colonist.vocation, Vocation::Miller | Vocation::Baker)
                    {
                        return None;
                    }
                    let stock = self
                        .society
                        .agents
                        .get(colonist.id)
                        .map_or(0, |agent| agent.stock.get(staple));
                    (stock > FEED_TARGET).then_some((stock, colonist.id))
                })
                .max_by_key(|&(stock, donor_id)| (stock, std::cmp::Reverse(donor_id)));
            if let Some((stock, donor_id)) = donor {
                let give = need.min(stock - FEED_TARGET);
                if give > 0 && self.society.debit_stock(donor_id, staple, give) {
                    // Conserved transfer; roll back to the donor if the credit
                    // can't land (overflow), so no food is created or destroyed.
                    if !self.society.credit_stock(producer_id, staple, give) {
                        self.society.credit_stock(donor_id, staple, give);
                    }
                }
            }
        }
    }

    /// IN-KIND INPUT ADVANCE phase (EXPERIMENT — see [`ChainConfig::input_advance`]).
    /// Before production, a capitalist (the richest money-holder) buys each active
    /// producer's recipe input **in kind** from the holder with the most of it
    /// (grain for a miller from the gatherers, flour for a baker from the millers),
    /// paying the seller real money and placing the input in the producer's hands.
    /// This bypasses the value-scale gate: production no longer needs a producer to
    /// out-rank its own consumption/savings to *bid* for inputs. Conserved (money
    /// capitalist→seller, input seller→producer); it also recirculates the
    /// capitalist's idle money to the sellers. No-op unless enabled and money has
    /// emerged. Deterministic: id-ordered; capitalist/seller by most free
    /// gold / most stock (ties → lowest id).
    fn run_input_advance(&mut self) {
        let (grain, flour) = match self.chain.as_ref() {
            Some(chain) if chain.input_advance => (chain.content.grain(), chain.content.flour()),
            _ => return,
        };
        if self.society.current_money_good().is_none() {
            return;
        }
        // A small per-tick input float — enough for a recipe application.
        const STOCK_TARGET: u32 = 3;
        let live = self.live_colonist_slots.clone();
        for &slot in &live {
            let (producer_id, vocation) = {
                let colonist = &self.colonists[slot];
                (colonist.id, colonist.vocation)
            };
            let input = match vocation {
                Vocation::Miller => grain,
                Vocation::Baker => flour,
                _ => continue,
            };
            let held = self
                .society
                .agents
                .get(producer_id)
                .map_or(0, |agent| agent.stock.get(input));
            if held >= STOCK_TARGET {
                continue;
            }
            let need = STOCK_TARGET - held;
            let pick = |key: &dyn Fn(AgentId) -> u64| -> Option<(u64, AgentId)> {
                live.iter()
                    .filter_map(|&s| {
                        let id = self.colonists[s].id;
                        if id == producer_id {
                            return None;
                        }
                        let v = key(id);
                        (v > 0).then_some((v, id))
                    })
                    .max_by_key(|&(v, id)| (v, std::cmp::Reverse(id)))
            };
            let seller = pick(&|id| {
                u64::from(
                    self.society
                        .agents
                        .get(id)
                        .map_or(0, |a| a.stock.get(input)),
                )
            });
            let capitalist = pick(&|id| self.society.free_gold_after_all_reserves(id).0);
            let (Some((seller_stock, seller_id)), Some((cap_free, cap_id))) = (seller, capitalist)
            else {
                continue;
            };
            if cap_id == seller_id {
                continue;
            }
            let price = self.realized_price(input).map_or(1, |g| g.0.max(1));
            let affordable = u32::try_from(cap_free / price).unwrap_or(u32::MAX);
            let qty = need
                .min(u32::try_from(seller_stock).unwrap_or(u32::MAX))
                .min(affordable);
            if qty == 0 {
                continue;
            }
            let cost = Gold(u64::from(qty) * price);
            // Pay the seller, then place the input. Roll back on any failure so no
            // good or money is created or destroyed.
            if self.move_money_conserved(cap_id, seller_id, cost) {
                if self.society.debit_stock(seller_id, input, qty) {
                    if !self.society.credit_stock(producer_id, input, qty) {
                        self.society.credit_stock(seller_id, input, qty);
                        self.move_money_conserved(seller_id, cap_id, cost);
                    }
                } else {
                    self.move_money_conserved(seller_id, cap_id, cost);
                }
            }
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
            grain,
            flour,
            bread,
            operating_cost,
            recurring,
            subsistence,
            entrepreneurial,
        ) = match self.chain.as_ref() {
            Some(chain) if chain.project_input_bids => (
                chain.content.mill_recipe().clone(),
                chain.content.bake_recipe().clone(),
                chain.content.grain(),
                chain.content.flour(),
                chain.content.bread(),
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
            self.society
                .set_bid_override(producer_id, input, reservation, limit);
        }
    }

    /// Capital-advance REPAYMENT phase (EXPERIMENT): after the market clears,
    /// each borrower repays its revolving working-capital loan from its sales,
    /// keeping it cash-light so its future-money want stays UNMET — the incentive
    /// to keep producing survives (unlike an unrepaid gift, which satisfies the
    /// want and gets the producer de-adopted). Conserved; a no-op when there are
    /// no loans. Deterministic: id-ordered over the loan ledger.
    fn run_capital_repayment(&mut self) {
        if self.capital_loans.is_empty() {
            return;
        }
        let borrowers: Vec<AgentId> = self.capital_loans.keys().copied().collect();
        for borrower in borrowers {
            let Some(&(lender, owed)) = self.capital_loans.get(&borrower) else {
                continue;
            };
            // Drop loans whose borrower or lender is no longer live — the estate
            // already settled that gold elsewhere; the money stays conserved in
            // the system, only the (unrecoverable) bookkeeping is dropped.
            if self.society.agents.get(borrower).is_none()
                || self.society.agents.get(lender).is_none()
            {
                self.capital_loans.remove(&borrower);
                continue;
            }
            let free = self.society.free_gold_after_all_reserves(borrower);
            let repay = free.min(owed);
            if repay > Gold::ZERO && self.move_money_conserved(borrower, lender, repay) {
                let remaining = owed.saturating_sub(repay);
                if remaining == Gold::ZERO {
                    self.capital_loans.remove(&borrower);
                } else {
                    self.capital_loans.insert(borrower, (lender, remaining));
                }
            }
        }
    }

    /// SPOILAGE phase (EXPERIMENT — see [`ChainConfig::perishable_decay_bps`]):
    /// decay every colonist's (and the commons') holdings of the **staple** food
    /// (the hunger good, plus the subsistence food if any) by the configured
    /// per-tick rate. A real sink: every removed unit is recorded in
    /// `report.spoiled` so whole-system conservation accounts it exactly. This is
    /// the inventory carrying cost that stops a satiated agent from hoarding its
    /// way out of the market — the staple decays, hunger returns, so it must keep
    /// acquiring (buying or producing). Deliberately does NOT spoil the chain's
    /// intermediates (grain/flour) — their small working stocks and large
    /// bootstrap seed buffers must survive — nor durable goods (WOOD, SALT,
    /// tools, money). A no-op unless enabled, so every other settlement is
    /// byte-identical. Deterministic: integer floor decay, id-ordered.
    fn run_spoilage(&mut self, report: &mut EconTickReport) {
        let bps = match self.chain.as_ref() {
            Some(chain) if chain.perishable_decay_bps > 0 => u64::from(chain.perishable_decay_bps),
            _ => return,
        };
        // Spoil the **staple** food a satiated agent hoards (and the subsistence
        // food, if any) — NOT the chain's intermediates (grain/flour), whose
        // small working stocks and large bootstrap seed buffers must survive for
        // the chain to run. Targeting the satiation hoard is the point: when the
        // staple decays, hunger returns and the holder must re-enter the market.
        let mut perishable = vec![self.known.hunger];
        if let Some(subsistence) = self.known.subsistence {
            if subsistence != self.known.hunger {
                perishable.push(subsistence);
            }
        }
        // Also pressure the raw-grain hoard (threshold-protected, so the miller's
        // small working stock is exempt) so gatherers must sell before it rots.
        if let Some(chain) = self.chain.as_ref() {
            let grain = chain.content.grain();
            if !perishable.contains(&grain) {
                perishable.push(grain);
            }
        }
        // Carrying cost hits only HOARDS: the portion of holdings above a free
        // storage threshold decays. Working stock and a baker's fresh
        // about-to-be-sold output (both small) sit under the threshold and are
        // exempt, so spoilage curbs hoarding without destroying production.
        const FREE_STORAGE: u64 = 20;
        let decay = |held: u64| -> u64 { held.saturating_sub(FREE_STORAGE) * bps / 10_000 };
        let live = self.live_colonist_slots.clone();
        for &good in &perishable {
            for &slot in &live {
                let id = self.colonists[slot].id;
                let held = u64::from(self.society.agents.get(id).map_or(0, |a| a.stock.get(good)));
                let spoil = u32::try_from(decay(held)).unwrap_or(u32::MAX);
                if spoil > 0 && self.society.debit_stock(id, good, spoil) {
                    *report.spoiled.entry(good).or_insert(0) += u64::from(spoil);
                }
            }
            let commons_held = self.commons_stock.get(&good).copied().unwrap_or(0);
            let commons_spoil = decay(commons_held);
            if commons_spoil > 0 {
                if let Some(qty) = self.commons_stock.get_mut(&good) {
                    *qty -= commons_spoil;
                    *report.spoiled.entry(good).or_insert(0) += commons_spoil;
                }
            }
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
        let mill_good = chain.content.mill();
        let oven_good = chain.content.oven();
        let operating_cost = chain.operating_cost;
        let recurring_motive = chain.recurring_motive;
        // S7.1: when tool-acquisition eligibility is on, a colonist that HOLDS the
        // required tool is admitted to this appraisal even with no seeded `latent`.
        let tool_eligibility = chain.tool_acquisition_eligibility;
        // S11: route each colonist's per-agent fallible OUTPUT-price forecast into the
        // adopt appraisal instead of the raw realized price (input price stays observed).
        let entrepreneurial = chain.entrepreneurial_forecasts;
        let tick = self.society.tick.0;
        let mut changed = false;

        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            // The recipe(s) this colonist may (re)appraise this tick, in deterministic
            // mill-before-oven order. A seeded `latent` yields exactly its one specialty
            // (the pre-S7 path; with the gate off this is the only branch, so role-choice
            // is byte-identical). Under S7.1 a colonist with no seeded `latent` yields the
            // recipe whose durable tool it now HOLDS (a built or handed mill/oven) — and
            // BOTH, when an estate transfer or inheritance leaves it holding a mill and an
            // oven, so the oven is appraised too instead of being stranded behind a
            // hard-coded mill-first pick. A colonist that is neither latent nor a
            // tool-holder keeps its vocation untouched. Re-appraising an already-adopted
            // tool-holder each tick is what lets it de-adopt when its spread collapses.
            let mut candidates: [Option<RecipeId>; 2] = [None, None];
            match colonist.latent {
                Some(recipe) => candidates[0] = Some(recipe),
                None if tool_eligibility => {
                    if let Some(agent) = self.society.agents.get(colonist.id) {
                        if agent.stock.get(mill_good) > 0 {
                            candidates[0] = Some(RecipeId::Mill);
                        }
                        if agent.stock.get(oven_good) > 0 {
                            candidates[1] = Some(RecipeId::Bake);
                        }
                    }
                }
                None => {}
            }
            if candidates.iter().all(Option::is_none) {
                continue;
            }
            let id = colonist.id;
            // S11: the colonist's heritable forecast bias (×1.0 = neutral). A Copy value,
            // so no borrow is held into the appraisal below.
            let forecast_bias = colonist.culture.forecast_bias_bps;
            // Adopt the FIRST candidate whose recipe pays on this colonist's own scale
            // (mill before oven). A colonist runs ONE vocation, so a holder of both tools
            // commits to one recipe; appraising both means the oven is chosen when the
            // milling spread does not pay (and vice versa) rather than the mill always
            // winning by position. For a seeded latent (one candidate) this is the
            // pre-S7 appraisal unchanged.
            let mut adoption: Option<Vocation> = None;
            for recipe_id in candidates.iter().flatten().copied() {
                let (recipe, output_good, input_price, adopted) = match recipe_id {
                    RecipeId::Mill => (
                        &mill_recipe,
                        flour,
                        self.society.realized_price(grain),
                        Vocation::Miller,
                    ),
                    RecipeId::Bake => (
                        &bake_recipe,
                        bread,
                        self.society.realized_price(flour),
                        Vocation::Baker,
                    ),
                    // No other recipe is a latent specialty (set only at generation).
                    _ => continue,
                };
                // The OUTPUT-price estimate: the colonist's grounded fallible forecast when
                // entrepreneurial forecasts are on, else the raw last realized price (the
                // pre-S11 path). The market still clears at the REAL price either way.
                let realized_output = self.society.realized_price(output_good);
                let pays = {
                    let agent = self
                        .society
                        .agents
                        .get(id)
                        .expect("living colonist resolves in the arena");
                    let output_price = if entrepreneurial {
                        forecast_output_price(agent, output_good, realized_output, forecast_bias)
                    } else {
                        realized_output
                    };
                    let base_pays = recipe_adoption_pays_for_money(
                        agent,
                        recipe,
                        output_price,
                        input_price,
                        tick,
                        operating_cost,
                        money_good,
                    );
                    // Recurring owner-operator motive: also keep the role while the recipe
                    // is simply profitable at the appraised output price, so a producer
                    // whose savings ladder is full does not retire (consumption recurs — it
                    // keeps producing to keep eating). A no-op unless enabled.
                    base_pays
                        || (recurring_motive
                            && recipe_is_profitable(
                                recipe,
                                output_price,
                                input_price,
                                operating_cost,
                            ))
                };
                if pays {
                    adoption = Some(adopted);
                    break;
                }
            }
            // When no candidate pays: a seeded latent or an adopted producer reverts to
            // Unassigned (the pre-S7 behaviour — it holds its tool, idle). An S7
            // tool-holder that is still feeding itself by gathering/consuming (it
            // acquired a tool but has not yet adopted) keeps that survival role rather
            // than being stranded Unassigned — it tries again next tick.
            let next = match adoption {
                Some(adopted) => adopted,
                None => match self.colonists[slot].vocation {
                    Vocation::Miller | Vocation::Baker | Vocation::Unassigned => {
                        Vocation::Unassigned
                    }
                    other => other,
                },
            };
            if self.colonists[slot].vocation != next {
                if !self.role_choice_switch_ready(id, self.colonists[slot].vocation, next) {
                    continue;
                }
                self.colonists[slot].vocation = next;
                changed = true;
            }
        }
        changed
    }

    /// PRODUCTIVE RE-ENTRY (S6 — provisioning at scale). A gated, default-OFF
    /// `econ_tick` phase that runs each live **spatial non-lineage** colonist through
    /// a two-sided hysteresis on its own hunger:
    ///
    /// - **Re-enter** (hunger ≥ [`ChainConfig::reentry_hunger_in`] and **not already
    ///   feeding itself on the edible grain node**): adopt edible-grain gathering. An
    ///   idle [`Consumer`](Vocation::Consumer) (no node, produces nothing) becomes a
    ///   grain [`Gatherer`](Vocation::Gatherer); a `Gatherer` mis-allocated to a
    ///   non-edible (WOOD) node is re-pointed to the edible grain node (a hungry actor
    ///   gathers food before wood — hunger outranks wood-for-trade on its scale).
    /// - **Revert** (hunger < [`ChainConfig::reentry_hunger_out`] and currently
    ///   displaced from its home role): resume the **home** role captured at
    ///   generation — a WOOD gatherer returns to WOOD (keeping the WOOD supply alive),
    ///   an idle consumer goes idle. The gap `H_in − H_out` is the hysteresis band: a
    ///   colonist inside it holds its current node, so the phase does not thrash
    ///   node-to-node every tick.
    ///
    /// Scope (Base Fact 4): only colonists with a **world agent** and `household ==
    /// None` whose HOME is an untooled spatial role are touched. Lineage members are
    /// hearth-fed (`deliver_demography_provisions`); the latent/seeded **tooled** chain
    /// producers (Miller/Baker/Scholar/Confectioner and the latent pool) feed from
    /// `run_producer_subsistence` and belong to the S7 capital-goods milestone, never
    /// re-entry. The vocation flip is between two **untooled** spatial roles whose
    /// econ value scale is identical (`production_specialty` is `None` for both
    /// `Consumer` and `Gatherer`), so it perturbs no resting quote and needs no scale
    /// regeneration — it only steers the next fast loop's `assign_idle_gatherer_tasks`.
    /// It mints nothing: a re-entrant feeds by gathering grain (the existing conserved
    /// node-regen source) and eating it (`subsistence_on_grain`).
    ///
    /// A no-op unless [`ChainConfig::productive_reentry`] is set AND raw grain is
    /// edible (so the gathered grain actually relieves hunger), so every existing
    /// run is byte-identical. Deterministic: slot-ordered, integer thresholds,
    /// nothing drawn.
    fn run_productive_reentry(&mut self) {
        let Some(chain) = &self.chain else {
            return;
        };
        if !chain.productive_reentry {
            return;
        }
        let grain = chain.content.grain();
        // Without an edible-grain fallback the gathered grain would not feed anyone,
        // so re-entry would relabel without provisioning — stay inert.
        if self.known.subsistence != Some(grain) {
            return;
        }
        let h_in = chain.reentry_hunger_in;
        let h_out = chain.reentry_hunger_out;
        // Single canonical edible node: `node_for_good` resolves the lowest-id node
        // yielding grain, and the shipped frontier seeds exactly one grain node, so
        // `grain_node`/`on_grain` below are unambiguous. A future config that seeded two
        // grain-yielding nodes would read a gatherer home-assigned to the second as "not
        // on grain" and re-point it to the first (still edible, but it abandons its home
        // node) — revisit this resolution and `on_grain` before adding such configs.
        let Some(grain_node) = self.node_for_good(grain) else {
            return;
        };
        for &slot in &self.live_colonist_slots {
            let colonist = &self.colonists[slot];
            // Lineage members are hearth-fed; the tooled chain producers (latent or
            // active Miller/Baker/Scholar/Confectioner) are the S7 path — skip both.
            // This includes a formerly non-latent tool-holder that adopted Miller/Baker
            // earlier this same tick: its home role is still spatial, but re-entry must
            // not revert an active capital holder before market/production.
            if colonist.household.is_some()
                || colonist.latent.is_some()
                || matches!(
                    colonist.vocation,
                    Vocation::Miller | Vocation::Baker | Vocation::Scholar | Vocation::Confectioner
                )
            {
                continue;
            }
            // Re-enter only colonists whose HOME is an untooled spatial role: an idle
            // Consumer, a Gatherer, or a non-latent Unassigned (the spec's stranded
            // idle worker). Latent/seeded producers were already skipped above, so the
            // `Unassigned` arm never catches one of those; it stays for the non-latent
            // stranded case even though current generation produces no such colonist,
            // and is the home a fed re-entrant reverts to once relieved.
            if !matches!(
                colonist.home_vocation,
                Vocation::Consumer | Vocation::Gatherer | Vocation::Unassigned
            ) {
                continue;
            }
            let hunger = colonist.need.hunger;
            let on_grain =
                colonist.vocation == Vocation::Gatherer && colonist.node == Some(grain_node);
            let displaced =
                colonist.vocation != colonist.home_vocation || colonist.node != colonist.home_node;
            let (next_vocation, next_node) = if hunger >= h_in && !on_grain {
                // Hungry and not yet feeding on grain: adopt grain gathering.
                (Vocation::Gatherer, Some(grain_node))
            } else if hunger < h_out && displaced {
                if !self.reentry_revert_ready(colonist.id) {
                    continue;
                }
                // Fed re-entrant: revert to the home role it was displaced from.
                (colonist.home_vocation, colonist.home_node)
            } else {
                // In the hysteresis band, or already where it should be: hold.
                continue;
            };
            let colonist = &mut self.colonists[slot];
            if colonist.vocation != next_vocation || colonist.node != next_node {
                colonist.vocation = next_vocation;
                colonist.node = next_node;
            }
        }
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

    /// CAPITAL FORMATION (S7.2 — producible capital goods). A gated, default-OFF
    /// `econ_tick` phase (after the scale regeneration, before role-choice) driving the
    /// **per-builder** project lifecycle: one builder, its OWN WOOD, its OWN labor.
    ///
    /// - **Advance + complete** every in-flight build by one labor unit (the builder's
    ///   own labor). On completion the durable tool credits the builder's own stock —
    ///   booked into `report.produced` (the produced side of the conserved build) —
    ///   and the formerly-non-latent builder is marked (observability). A build whose
    ///   builder has died is dropped: its WOOD was already consumed at the start tick.
    /// - **Start** a new build for each fed, non-latent colonist (in a survival/idle
    ///   role, holding no chain tool and no in-flight build) that holds enough saved
    ///   WOOD and whose entrepreneurial appraisal ([`capital_build_surplus`]) says the
    ///   tool's expected multi-period proceeds repay its build cost. The builder's own
    ///   WOOD is committed up front by [`start_project`] and booked into
    ///   `report.consumed_as_input` (the consumed side) — so the build conserves: WOOD
    ///   in at the start tick, the tool out at completion.
    ///
    /// Praxeological: each colonist decides for itself on its own value scale (hunger
    /// outranks building — a hungry colonist is skipped and gathers/feeds first), there
    /// is no global quota, no tool placement or transfer, and the WOOD + labor are the
    /// builder's own. Self-correcting: the appraisal is demand/price-driven, so once
    /// bread demand is met the per-run margin falls below the payback bar and no tool is
    /// built (the overinvestment guard).
    ///
    /// Returns `true` if any build completed this tick (so the caller regenerates the
    /// scales — the fresh tool-holder must carry its tool-anchor into the market step).
    /// A no-op unless [`ChainConfig::producible_capital`] is on and money has emerged,
    /// so every other run is byte-identical. Deterministic: slot-ordered, integer state.
    fn run_capital_formation(
        &mut self,
        report: &mut EconTickReport,
        labor_used: &mut Vec<(AgentId, u32)>,
    ) -> bool {
        let Some(chain) = &self.chain else {
            return false;
        };
        if !chain.producible_capital {
            return false;
        }
        // Gate on the money phase: the build appraisal weighs realized money prices.
        if self.current_money_good().is_none() {
            return false;
        }
        // Pull content/knobs into owned locals so the `&self.chain` borrow is released
        // before the loops mutate `self.society`/`self.capital_builds`/`self.colonists`.
        let mill_recipe = chain.content.mill_recipe().clone();
        let bake_recipe = chain.content.bake_recipe().clone();
        let grain = chain.content.grain();
        let flour = chain.content.flour();
        let bread = chain.content.bread();
        let mill_good = chain.content.mill();
        let oven_good = chain.content.oven();
        let operating_cost = chain.operating_cost;
        let payback = chain.capital_payback_cycles;
        let wood_qty = chain.tool_build_wood;
        let build_labor = chain.tool_build_labor;
        let hunger_max = chain.capital_build_hunger_max;
        let per_agent = chain.per_agent_capital;
        let tick = self.society.tick.0;

        let mut built = false;
        if per_agent {
            self.last_capital_decisions.clear();
        }

        // ---- 1. ADVANCE + COMPLETE in-flight builds (each its own labor).
        let mut finished: Vec<usize> = Vec::new();
        for bi in 0..self.capital_builds.len() {
            let builder = self.capital_builds[bi].builder;
            let slot = self.capital_builds[bi].slot;
            // Drop a build whose builder has died: its committed WOOD was already booked
            // `consumed_as_input` at the start tick, so the forfeit needs no further
            // booking (conservation already balanced — like an abandonment).
            let alive = self
                .colonist_slot_by_id
                .get(&builder)
                .is_some_and(|&s| self.colonists[s].alive);
            if !alive {
                finished.push(bi);
                continue;
            }
            // Advance with the builder's own labor (one unit per tick), then try to
            // complete it against the builder's own stock. A project already at its
            // required labor completes without an extra advance, so a one-labor build
            // started last tick is never charged N+1 units.
            {
                let build = &mut self.capital_builds[bi];
                if build.project.labor_advanced < build.template.required_labor
                    && advance_project(&mut build.project)
                {
                    labor_used.push((builder, 1));
                }
            }
            let tool = self.capital_builds[bi].project.output_good;
            let qty = self.capital_builds[bi].project.output_qty;
            let completed = match self.society.agents.get_mut(builder) {
                Some(agent) => {
                    let build = &mut self.capital_builds[bi];
                    complete_project_if_ready(&mut build.project, &build.template, &mut agent.stock)
                }
                None => false,
            };
            if completed {
                *report.produced.entry(tool).or_insert(0) += u64::from(qty);
                self.tools_built = self.tools_built.saturating_add(u64::from(qty));
                // Tie the produced tool to its formerly-non-latent builder (test 6).
                if slot < self.colonists.len() && self.colonists[slot].latent.is_none() {
                    self.colonists[slot].acquired_tool = true;
                }
                built = true;
                finished.push(bi);
            }
        }
        for &bi in finished.iter().rev() {
            self.capital_builds.remove(bi);
        }
        if built {
            return true;
        }

        // ---- 2. START new builds. S10 (per_agent_capital): each eligible colonist runs
        // its OWN ordinal appraisal and any it accepts starts its own build — no global
        // stage choice, no first-eligible assignment, no single-in-flight gate (the
        // per-builder substrate is reused). Behind the gate; the S7 heuristic below is
        // byte-identical for every existing config.
        if per_agent {
            return self.start_per_agent_builds(
                report,
                labor_used,
                &PerAgentBuildParams {
                    mill_recipe: &mill_recipe,
                    bake_recipe: &bake_recipe,
                    grain,
                    flour,
                    bread,
                    mill_good,
                    oven_good,
                    operating_cost,
                    wood_qty,
                    build_labor,
                    hunger_max,
                    tick,
                },
            );
        }

        // ---- 2 (S7 heuristic). START a new build when a demand-anchored real-resource
        // investment appraisal clears. This is a settlement-level heuristic, NOT a
        // per-colonist ordinal-scale appraisal: the opportunity depends only on prices, so the
        // better-paying stage is appraised once (scalar margin x payback vs build cost);
        // it is then funded by the first eligible fed builder from its OWN WOOD + labor
        // (no tool placement, no quota). A fully individual ordinal appraisal is a
        // follow-on; here each eligible fed builder with enough WOOD can take the build.
        let wood_price = self
            .society
            .realized_price(WOOD)
            .map_or(operating_cost.max(1), |g| g.0);
        let flour_price = self.society.realized_price(flour);
        let grain_price = self.society.realized_price(grain);
        let bread_price = self.society.realized_price(bread);
        let appraisal = CapitalBuildAppraisal {
            operating_cost,
            wood_price,
            tool_build_wood: wood_qty,
            tool_build_labor: build_labor,
            payback_cycles: payback,
        };
        // Which tool to build is set by the chain's BOTTLENECK, anchored on the final
        // good's real demand — Menger's imputation in mechanism form: flour is worth
        // building a mill for only because bread is demanded and ovens turn flour into
        // it. Build only while BREAD is actually clearing (real demand for the chain's
        // output); when bread demand is met it stops clearing / its spread thins and
        // building stops — the demand-anchored brake. Given that demand, build the
        // scarcer stage: if the active bakers out-demand the active millers' flour
        // supply, the MILL is the bottleneck; otherwise flour is plentiful relative to
        // baking capacity, so an OVEN turns more of it into bread. This builds ovens
        // first (raising bread), pulls mills in only when bakers truly need flour, and
        // keeps the two stages balanced instead of flooding one — the naive
        // higher-margin-wins rule floods mills on a stale flour price and starves the
        // baker side. The chosen tool's own output must also have traded recently, and
        // its amortized margin must clear the payback bar.
        // The bottleneck is read from usable installed CAPACITY (the count of live
        // colonists HOLDING a tool), not active-producer counts and not whole-system
        // conserved totals: active counts loop (a just-built tool whose holder has not
        // yet adopted would read as zero capacity and drive an endless build of the same
        // stage), while a tool settled to the commons is conserved but inaccessible and
        // must not suppress replacement builds. Counting holders, not raw units, also
        // means a colonist that came to hold two tools of a kind (an inherited/transferred
        // stack) cannot overstate capacity: it still runs one vocation, one throughput.
        // The market clears one seller-side lot per producer per tick, so the practical
        // throughput ratio is one live mill holder to one live oven holder even when the
        // milling recipe emits a multi-unit flour batch.
        let held_mills = self.live_colonist_holder_count(mill_good);
        let held_ovens = self.live_colonist_holder_count(oven_good);
        let active_millers = self.living_count(Vocation::Miller) as u64;
        let active_bakers = self.living_count(Vocation::Baker) as u64;
        let oven_capacity = held_ovens;
        let mill_capacity = held_mills;
        // Utilization guard against idle-tool overbuild: only add a tool of a kind while
        // the kind already in the colony is close to fully employed — held tools at most
        // the active producers plus a small slack. The slack absorbs the emergent chain's
        // tick-to-tick adoption churn (a producer that briefly de-adopts still holds a
        // productive tool), so building is not stalled by a transient dip, while idle
        // tools cannot accumulate without bound: built capital tracks the active producer
        // count, the structural half of the overinvestment guard.
        let bread_signal = self.good_traded_within(bread, CAPITAL_BUILD_RECENCY)
            || (held_ovens == 0 && bread_price.is_some());
        let flour_signal = self.good_traded_within(flour, CAPITAL_BUILD_RECENCY)
            || (held_mills == 0 && flour_price.is_some());
        let choice: Option<(GoodId, ProjectTemplateId)> = if !bread_signal {
            // Build only while the chain's FINAL good (bread) is actually clearing —
            // real demand for the chain's output. When bread demand is met it stops
            // clearing / its spread thins and building stops (the demand brake). If a
            // stage has collapsed to ZERO usable capacity, a prior realized price is
            // enough to restart replacement building; no observed price still means no
            // appraisal basis.
            None
        } else if oven_capacity < mill_capacity {
            // Flour-milling capacity outruns baking capacity: add an OVEN to turn the
            // surplus flour into bread — unless idle ovens already sit unemployed.
            (held_ovens <= active_bakers.saturating_add(CAPITAL_IDLE_SLACK))
                .then(|| capital_build_surplus(&bake_recipe, bread_price, flour_price, &appraisal))
                .flatten()
                .map(|_| (oven_good, ProjectTemplateId::BuildOven))
        } else {
            // Baking capacity outruns milling: the bakers need more flour, so the
            // MILL is the bottleneck — gated on mills not already sitting idle, on
            // flour actually clearing (so the mill's output has a real buyer), and on
            // the milling spread paying.
            (held_mills <= active_millers.saturating_add(CAPITAL_IDLE_SLACK) && flour_signal)
                .then(|| capital_build_surplus(&mill_recipe, flour_price, grain_price, &appraisal))
                .flatten()
                .map(|_| (mill_good, ProjectTemplateId::BuildMill))
        };
        let Some((tool, template_id)) = choice else {
            return built;
        };

        // Capital forms GRADUALLY: only one build is in flight at a time, so each new
        // tool is completed, adopted, and its price impact realized and re-appraised
        // before the next build starts — the entrepreneurial signal propagates and the
        // chain re-equilibrates, instead of a same-tick cluster of speculative idle
        // tools that whipsaws the intermediate price (the overinvestment guard). This is
        // pacing, not a quota: over the run the colony builds as many tools as the
        // demand-driven appraisal supports, and each builder still decides for itself.
        if !self.capital_builds.is_empty() {
            return built;
        }
        for idx in 0..self.live_colonist_slots.len() {
            let slot = self.live_colonist_slots[idx];
            let colonist = &self.colonists[slot];
            // Formerly-non-latent builders only (a seeded latent/producer already holds
            // a tool); only a fed colonist in a survival/idle role (not a producer).
            if colonist.latent.is_some() {
                continue;
            }
            if !matches!(
                colonist.vocation,
                Vocation::Gatherer | Vocation::Consumer | Vocation::Unassigned
            ) {
                continue;
            }
            // Feed first: a hungry colonist gathers/feeds before investing in capital.
            if colonist.need.hunger > hunger_max {
                continue;
            }
            let id = colonist.id;
            // Skip a colonist that already has an in-flight build.
            if self.capital_builds.iter().any(|build| build.builder == id) {
                continue;
            }
            // Must hold no chain tool yet (else it is a producer/holder, not a builder)
            // and enough saved WOOD to fund the build from its OWN endowment.
            let can_fund = self.society.agents.get(id).is_some_and(|agent| {
                agent.stock.get(mill_good) == 0
                    && agent.stock.get(oven_good) == 0
                    && agent.stock.get(WOOD) >= wood_qty
            });
            if !can_fund {
                continue;
            }
            let template = match template_id {
                ProjectTemplateId::BuildOven => build_oven_template(tool, wood_qty, build_labor),
                _ => build_mill_template(tool, wood_qty, build_labor),
            };
            let pid = ProjectId(self.next_capital_project_id);
            // Commit the builder's own WOOD up front (booked consumed_as_input), then
            // advance one labor unit — mirroring the lab World BuildNet path (start then
            // advance). If that satisfies the labor requirement, complete immediately
            // so required_labor counts exact contributed units, not an extra wait tick.
            let started = match self.society.agents.get_mut(id) {
                Some(agent) => start_project(&template, &mut agent.stock, pid, Tick(tick)),
                None => None,
            };
            if let Some(mut project) = started {
                *report.consumed_as_input.entry(WOOD).or_insert(0) += u64::from(wood_qty);
                if project.labor_advanced < template.required_labor && advance_project(&mut project)
                {
                    labor_used.push((id, 1));
                }
                self.next_capital_project_id = self.next_capital_project_id.wrapping_add(1);
                let completed = match self.society.agents.get_mut(id) {
                    Some(agent) => {
                        complete_project_if_ready(&mut project, &template, &mut agent.stock)
                    }
                    None => false,
                };
                if completed {
                    let qty = project.output_qty;
                    *report.produced.entry(project.output_good).or_insert(0) += u64::from(qty);
                    self.tools_built = self.tools_built.saturating_add(u64::from(qty));
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
                // One new build per tick (the gradual-accumulation pacing above).
                break;
            }
        }

        built
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
            if colonist.latent.is_some()
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
                *report.produced.entry(project.output_good).or_insert(0) += u64::from(qty);
                self.tools_built = self.tools_built.saturating_add(u64::from(qty));
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

    /// S12: whether the own-labor subsistence path is active — the food mints are
    /// retired and a hungry, eligible colonist forages the FORAGE floor. Gated on the
    /// chain flag (and, for a real path, a forage good in the content). When this holds,
    /// `canonical_bytes` serializes the forage knobs + the per-colonist `foraging` state;
    /// with it off the stream is byte-identical to pre-S12.
    fn own_labor_subsistence_can_run(&self) -> bool {
        self.chain
            .as_ref()
            .is_some_and(|chain| chain.own_labor_subsistence && chain.content.forage().is_some())
    }

    /// S12: the FORAGE node (the `GoForage` target / forage location), or `None` when
    /// own-labor subsistence is off. Resolved by good, like [`Self::grain_node`].
    fn forage_node(&self) -> Option<NodeId> {
        self.chain
            .as_ref()
            .and_then(|chain| chain.content.forage())
            .and_then(|forage| self.node_for_good(forage))
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

    /// Total **fiat** paid out as wages over the run (summed from econ's wage-payment
    /// audit). Positive when fiat wages are legal tender and the employers hold fiat;
    /// `Gold::ZERO` when wages are specie-only (the fiat is refused, so no wage trade
    /// settles in fiat). The wage surface's composition signal — gating, not totals.
    pub fn wage_fiat_settled(&self) -> Gold {
        self.society
            .wage_payment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| sum.saturating_add(row.public_fiat))
    }

    /// Total **specie** paid out as wages over the run (summed from the wage-payment
    /// audit).
    pub fn wage_specie_settled(&self) -> Gold {
        self.society
            .wage_payment_audit
            .iter()
            .fold(Gold::ZERO, |sum, row| sum.saturating_add(row.public_specie))
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

    /// S8.0 emergence probe (read-only): the cumulative units of the chain staple
    /// (bread) exchanged against the barter MEDIUM (SALT) over the run — the
    /// bread-for-SALT leg of indirect exchange that actually monetizes SALT. Derived
    /// from the society's retained barter-trade log (which survives the promotion
    /// barter-book wipe), so it is the realized volume, not an estimate. `0` for a
    /// settlement with no chain or no barter medium. Reads only.
    pub fn bread_for_salt_volume(&self) -> u64 {
        let (Some((medium, _)), Some(content)) = (self.barter_medium, self.content()) else {
            return 0;
        };
        let staple = content.bread();
        self.society
            .barter_trades
            .iter()
            .filter(|trade| {
                (trade.a_gives == staple && trade.b_gives == medium)
                    || (trade.a_gives == medium && trade.b_gives == staple)
            })
            .map(|trade| u64::from(trade.qty))
            .sum()
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
        // S7.1: with tool-acquisition eligibility on, role-choice acts on a colonist
        // that merely HOLDS a tool — so the role-choice appraisal can steer a future
        // tick even with no seeded latent pool. Widen the role-choice digest gate to
        // "a latent pool OR S7 eligibility on", so the operating cost and the
        // per-colonist latent block below serialize in that case too. With the gate
        // off this is exactly `has_latent_pool`, so every pre-S7 stream is unchanged.
        let role_choice_active = has_latent_pool || self.tool_acquisition_can_run();
        // S11: whether per-agent forecasts steer the appraisals — gates the per-belief
        // `observed` flag and the per-colonist `forecast_bias_bps` below into the digest.
        // Off the flag neither is emitted, so the pre-S11 stream is byte-identical.
        let entrepreneurial_serialized = self.entrepreneurial_can_run();
        if let Some(chain) = &self.chain {
            out.extend_from_slice(&chain.throughput.to_le_bytes());
            // The G3b operating cost steers nothing but the role-choice appraisal, so
            // it is part of the future-behaviour identity only when that appraisal can
            // run — a latent pool OR S7 tool-acquisition eligibility (the widened
            // gate). Without either (a seeded G3a chain) two settlements differing only
            // in it behave identically, so it is omitted — keeping the tripwire's
            // "byte-identical iff future behaviour identical" contract honest rather
            // than splitting equivalent seeded chains apart.
            if role_choice_active {
                out.extend_from_slice(&chain.operating_cost.to_le_bytes());
            }
            // The S2/S5 endogenous knobs steer future ticks but never show up in the
            // generated holdings, so two chains differing only in one would collide
            // at generation and then diverge — they belong in the "byte-identical iff
            // future behaviour identical" identity, exactly like the operating cost.
            // `producer_subsistence` mints the local staple/WOOD floor for producers
            // each tick; `project_input_bids` switches input acquisition to the
            // project-aware market bid. Both are included unconditionally (not
            // latent-pool-gated like the operating cost): every chain config has
            // producers and a money path, so each always eventually steers a tick —
            // there is no behaviour-identical config pair they would falsely split.
            out.extend_from_slice(&chain.producer_subsistence.to_le_bytes());
            out.push(u8::from(chain.project_input_bids));
            // `recurring_motive` keeps an owner-operator adopted while the recipe
            // stays profitable, steering future role-choice ticks without ever
            // showing up in generated holdings — the same identity contract as the
            // two knobs above, so it joins them unconditionally.
            out.push(u8::from(chain.recurring_motive));
            // The capital-advance / in-kind-subsistence / in-kind-input / spoilage
            // knobs each gate a future settlement phase (run_capital_advance,
            // run_subsistence_advance, run_input_advance, run_spoilage) that runs for
            // any chain regardless of a latent pool, so two configs differing only in
            // one generate identically and then diverge — the same identity contract,
            // joined unconditionally. (perishable_decay_bps is the spoilage rate, not
            // a bool: 0 disables the phase, any other value steers it.)
            out.push(u8::from(chain.capital_advance));
            out.push(u8::from(chain.subsistence_advance));
            out.push(u8::from(chain.input_advance));
            out.extend_from_slice(&chain.perishable_decay_bps.to_le_bytes());
            // The S6 productive-re-entry state steers future ticks only while the
            // phase can actually feed a colonist: the gate is on, raw grain is the
            // subsistence fallback, AND a grain-yielding node exists. When it cannot
            // run, omit these bytes entirely (no marker) — like every other gated
            // block here (latent pool, research, the per-colonist home below) — so a
            // re-entry-OFF or inert config stays byte-identical to the pre-S6 stream
            // and two behavior-identical configs never digest apart.
            if self.productive_reentry_can_run() {
                out.extend_from_slice(&chain.reentry_hunger_in.to_le_bytes());
                out.extend_from_slice(&chain.reentry_hunger_out.to_le_bytes());
            }
            // S7.1: the tool-acquisition eligibility gate relaxes role-choice and adds
            // the acquired-tool scale anchor, steering every future tick for any chain
            // once a colonist comes to hold a tool. It is emitted only when on, so a
            // pre-S7 (flag-off) chain stays byte-identical to the pre-S7 stream — the
            // same gated-block discipline as the re-entry thresholds above.
            if self.tool_acquisition_can_run() {
                out.push(1);
            }
            // S7.2: the per-builder capital-formation phase + its appraisal knobs steer
            // every future tick once on, so they join the identity when the phase can
            // run. Emitted only when on (the same gated-block discipline), so a
            // producible-capital-OFF chain stays byte-identical to the pre-S7 stream.
            if self.producible_capital_can_run() {
                // S10: in the per-agent path the `per_agent_capital` flag steers every
                // future tick and `capital_payback_cycles` is behaviour-INERT — so
                // serialize the flag in its place (digesting the inert knob would split
                // behaviour-identical per-agent configs). The legacy heuristic path
                // serializes `capital_payback_cycles` and no flag, byte-identical to pre-S10.
                if self.per_agent_capital_can_run() {
                    out.push(1);
                } else {
                    out.extend_from_slice(&chain.capital_payback_cycles.to_le_bytes());
                }
                out.extend_from_slice(&chain.tool_build_wood.to_le_bytes());
                out.extend_from_slice(&chain.tool_build_labor.to_le_bytes());
                out.extend_from_slice(&chain.capital_build_hunger_max.to_le_bytes());
                out.extend_from_slice(&self.next_capital_project_id.to_le_bytes());
                // The in-flight per-builder builds are live state two runs through the
                // build can differ in (which builder, how much labor it has advanced),
                // so they are part of the future-behaviour identity. Serialized in the
                // stored (slot-ordered, deterministic) order. Each build's WOOD cost and
                // output are fixed by the template; labor_advanced is the progress.
                out.extend_from_slice(&(self.capital_builds.len() as u32).to_le_bytes());
                for build in &self.capital_builds {
                    out.extend_from_slice(&build.builder.0.to_le_bytes());
                    out.extend_from_slice(&build.project.id.0.to_le_bytes());
                    out.push(project_template_id_tag(build.project.template));
                    out.extend_from_slice(&build.project.started_at.0.to_le_bytes());
                    out.extend_from_slice(&build.project.output_good.0.to_le_bytes());
                    out.extend_from_slice(&build.project.output_qty.to_le_bytes());
                    out.extend_from_slice(&build.template.required_labor.to_le_bytes());
                    out.extend_from_slice(&build.project.labor_advanced.to_le_bytes());
                }
            }
            // S11: the entrepreneurial-forecasts gate steers every future tick once on
            // (each appraisal weighs a per-agent forecast instead of the realized price),
            // so it joins the identity when the phase can run. Emitted only when on (the
            // same gated-block discipline as S7/S10 above + the per-colonist forecast bias
            // and the per-belief `observed` flag below), so a flag-off chain stays
            // byte-identical to the pre-S11 stream.
            if self.entrepreneurial_can_run() {
                out.push(1);
            }
            // S12: the own-labor subsistence gate retires the food mints and steers the
            // forage phase + the per-colonist `foraging` state below. When it can run,
            // serialize a marker + the forage knobs (yield + the hysteresis band) that
            // steer how much FORAGE is produced and who forages. Emitted only when on
            // (the same gated-block discipline as S7/S10/S11 above), so a flag-off chain
            // stays byte-identical to the pre-S12 stream. (The FORAGE good id itself is
            // already captured by `known.subsistence` below and `good_entries`.)
            if self.own_labor_subsistence_can_run() {
                out.push(1);
                out.extend_from_slice(&chain.forage_yield.to_le_bytes());
                out.extend_from_slice(&chain.forage_hunger_in.to_le_bytes());
                out.extend_from_slice(&chain.forage_hunger_out.to_le_bytes());
            }
            // The staple mapping steers the next needs/scale phase for *any* chain,
            // role-choice or not, so it is included whenever a chain is active. The
            // G3b no-spread control shares the emergent config's physical state but
            // maps hunger to FOOD instead of bread, and that divergence must show.
            out.extend_from_slice(&self.known.hunger.0.to_le_bytes());
            out.extend_from_slice(&self.known.warmth.0.to_le_bytes());
            out.extend_from_slice(&self.known.savings.0.to_le_bytes());
            // `subsistence_on_grain` is realised at construction as
            // `known.subsistence` (a directly-edible staple fallback) and steers the
            // needs/scale phase (settlement.rs:4586, 5750) exactly like the three
            // mappings above, so it joins their identity. Encode the Option as a
            // presence byte plus the good id when set.
            match self.known.subsistence {
                Some(good) => {
                    out.push(1);
                    out.extend_from_slice(&good.0.to_le_bytes());
                }
                None => out.push(0),
            }
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

        // The G8c-1 credit-cycle state. Omitted entirely for a non-finance settlement
        // (so every pre-G8c-1 canonical layout is byte-identical); present for a
        // finance settlement so the cycle trajectory — the regime rung, the issuer's
        // fiat base, and the per-tick boom/bust/structure/rate the M3 records carry —
        // is part of the "byte-identical iff future behaviour identical" identity. The
        // money_system + agent blocks above already carry the ledger and balances; the
        // ABCT records below are the cycle-specific state two runs through the
        // boom→stop→bust can otherwise differ in (the test-1 determinism tripwire).
        if let Some(cycle) = &self.cycle {
            push_cycle_runtime_bytes(&mut out, cycle);
            out.push(regime_tag(self.society.regime()));
            out.extend_from_slice(&(self.society.issuers.len() as u32).to_le_bytes());
            for issuer in &self.society.issuers {
                out.extend_from_slice(&issuer.fiat_issued.0.to_le_bytes());
                out.extend_from_slice(&issuer.fiat_retired.0.to_le_bytes());
                out.extend_from_slice(&issuer.fiat_credit_outstanding.0.to_le_bytes());
            }
            push_cycle_live_m2_bytes(&mut out, &self.society);
            out.extend_from_slice(&(self.society.m3_records.len() as u32).to_le_bytes());
            for record in &self.society.m3_records {
                out.push(regime_tag(record.regime));
                out.extend_from_slice(&record.public_specie.0.to_le_bytes());
                out.extend_from_slice(&record.public_fiat.0.to_le_bytes());
                out.extend_from_slice(&record.fiduciary.0.to_le_bytes());
                out.extend_from_slice(&record.boom_projects_started.to_le_bytes());
                out.extend_from_slice(&record.bust_abandoned_projects.to_le_bytes());
                out.extend_from_slice(&record.m2.structure_length_ticks_x100.to_le_bytes());
                out.extend_from_slice(&record.m2.market_rate_bps.unwrap_or(i64::MIN).to_le_bytes());
                out.extend_from_slice(&record.m2.capital_labor_consumed.to_le_bytes());
                out.extend_from_slice(&record.m2.capital_goods_consumed.to_le_bytes());
            }
        }

        // The G8c-2 tender-bench state. Omitted entirely for a non-bench settlement
        // (so every pre-G8c-2 canonical layout is byte-identical); present for a bench
        // so the surface + the tender-policy timeline its scenario carries are part of
        // the "byte-identical iff future behaviour identical" identity (a spot bench
        // and a debt bench, or two benches differing only in the surface tender, must
        // never collide). The agent/money blocks above already carry the live ledger +
        // balances; this adds the bench-specific policy steering.
        if let Some(bench) = &self.bench {
            out.push(bench_surface_tag(bench.surface));
            push_cycle_runtime_bytes_for_scenario(&mut out, &bench.scenario);
            out.push(public_spot_tender_tag(self.society.public_spot_tender));
            out.push(public_debt_tender_tag(self.society.public_debt_tender));
            out.push(bank_repayment_tender_tag(
                self.society.bank_repayment_tender,
            ));
            out.push(issuer_repayment_tender_tag(
                self.society.issuer_repayment_tender,
            ));
        }

        // The G8c-3 tax-overlay state. Omitted entirely for a non-tax settlement (so
        // every pre-G8c-3 canonical layout — and the tax-free cycle — is byte-identical);
        // present for a tax settlement so the configured + active receivability and the
        // issuer tax accounts (the levy/receipt/default outcome) are part of the
        // "byte-identical iff future behaviour identical" identity. The levy events are
        // already carried by the cycle scenario block above; this pins the settled
        // outcome the test-1 determinism tripwire reads back.
        if let Some(tax) = &self.tax {
            out.push(tax_receivability_tag(tax.receivability));
            out.extend_from_slice(&tax.levied.0.to_le_bytes());
            out.push(tax_receivability_tag(self.society.tax_receivability));
            out.extend_from_slice(&(self.society.issuers.len() as u32).to_le_bytes());
            for issuer in &self.society.issuers {
                out.extend_from_slice(&issuer.taxes_levied.0.to_le_bytes());
                out.extend_from_slice(&issuer.tax_receipts_fiat.0.to_le_bytes());
                out.extend_from_slice(&issuer.tax_receipts_specie.0.to_le_bytes());
                out.extend_from_slice(&issuer.taxes_defaulted.0.to_le_bytes());
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

            // A finance settlement (the G8c-1 credit cycle or a G8c-2 tender bench)
            // tracks no spatial goods (its goods live inside econ's own conserving
            // market/project machinery), yet its agents hold and trade goods that DO
            // steer the run — so serialize each agent's full (GoodId-sorted) stock
            // directly, with GOLD excluded (it is money, already serialized as
            // `agent.gold` + the money-system block). A spatial settlement keeps the
            // original path: every physical good an agent can hold is already in the
            // sorted `self.goods` (node goods ∪ starting goods; trade only relocates
            // them and no recipe mints a new one here), so serialize against it
            // directly, the debug check pinning that "complete and sorted" assumption.
            if self.cycle.is_some() || self.bench.is_some() {
                let mut held: Vec<(GoodId, u32)> = agent
                    .stock
                    .positive_goods()
                    .filter(|&good| good != GOLD)
                    .map(|good| (good, agent.stock.get(good)))
                    .collect();
                held.sort_by_key(|&(good, _)| good.0);
                out.extend_from_slice(&(held.len() as u32).to_le_bytes());
                for (good, qty) in held {
                    out.extend_from_slice(&good.0.to_le_bytes());
                    out.extend_from_slice(&qty.to_le_bytes());
                }
            } else {
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
            }

            out.extend_from_slice(&(agent.expect.len() as u32).to_le_bytes());
            for belief in &agent.expect {
                out.extend_from_slice(&belief.expected.0.to_le_bytes());
                out.extend_from_slice(&belief.step.0.to_le_bytes());
                out.extend_from_slice(&belief.last_seen.to_le_bytes());
                // S11: the `observed` flag steers the grounded forecast (belief vs realized
                // fallback) and is NOT derivable from `last_seen` (0 is ambiguous between
                // never-observed and a tick-0 observation), so it is part of the
                // future-behaviour identity once forecasts run. Emitted only under the flag,
                // so a flag-off agent block is byte-identical to the pre-S11 stream.
                if entrepreneurial_serialized {
                    out.push(u8::from(belief.observed));
                }
            }
        }

        // Colonist need/liveness state in generation order.
        let has_estate_destinations = self
            .colonists
            .iter()
            .any(|colonist| colonist.estate_destination.is_some());
        // The S6 re-entry home (vocation+node) decides the revert target of a
        // displaced re-entrant, so it steers future ticks only while the phase can run.
        // Gate its per-colonist bytes on the same active-phase predicate as the
        // thresholds above: a re-entry-OFF or non-edible config never reads the home
        // and keeps its pre-S6 per-colonist layout byte-identical.
        let reentry_serialized = self.productive_reentry_can_run();
        // S12: the per-colonist `foraging` flag steers the next fast loop (forage vs
        // harvest WOOD) only while the own-labor phase can run; gate its byte on the same
        // active-phase predicate, so a non-own-labor config keeps its pre-S12
        // per-colonist layout byte-identical.
        let own_labor_serialized = self.own_labor_subsistence_can_run();
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
            // S11: the heritable forecast bias steers every future entrepreneurial
            // appraisal, so it joins the identity once forecasts run. Emitted only under
            // the flag (the same gated-block discipline as the per-belief `observed`
            // above), so a flag-off colonist block stays byte-identical to pre-S11.
            if entrepreneurial_serialized {
                out.extend_from_slice(&colonist.culture.forecast_bias_bps.to_le_bytes());
            }
            match colonist.node {
                Some(node) => {
                    out.push(1);
                    out.extend_from_slice(&node.0.to_le_bytes());
                }
                None => out.push(0),
            }
            if reentry_serialized {
                // The home vocation+node the colonist reverts to once fed
                // (`run_productive_reentry`). Two states with identical CURRENT
                // vocation/node but different homes diverge on the revert path, so the
                // home is part of the future-behaviour identity whenever re-entry runs.
                out.push(colonist.home_vocation.tag());
                match colonist.home_node {
                    Some(node) => {
                        out.push(1);
                        out.extend_from_slice(&node.0.to_le_bytes());
                    }
                    None => out.push(0),
                }
            }
            if own_labor_serialized {
                // S12: whether the colonist is foraging — it steers the next fast loop
                // (forage the FORAGE node instead of harvesting WOOD). Two states with
                // identical current vocation/node but different foraging flags diverge
                // on the next task, so it is part of the future-behaviour identity
                // whenever the own-labor phase runs.
                out.push(u8::from(colonist.foraging));
            }
            if role_choice_active {
                // The latent specialty (G3b) steers each tick's role-choice
                // re-appraisal, so it is part of the future-behavior identity. This
                // block is omitted entirely when role-choice cannot run (no latent pool
                // AND no S7 eligibility), preserving the pre-G3b canonical layout for
                // plain and seeded-only configs. Under S7 eligibility it serializes the
                // latent (mostly `None`) for every colonist, since role-choice now acts
                // on a tool-holder even with an empty seeded latent pool.
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
    // S9: the heterogeneous direct-use seed steers which colonists barter for SALT
    // pre-promotion (and thus the saleability the promotion reads), so both knobs
    // are part of the future-behaviour identity. Appended last so every pre-S9
    // barter config's prefix is unchanged.
    out.extend_from_slice(&barter.salt_direct_use_qty.to_le_bytes());
    out.extend_from_slice(&barter.salt_direct_use_period.to_le_bytes());
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
    push_bank_policy_bytes(out, &bank.policy);
}

fn push_cycle_runtime_bytes(out: &mut Vec<u8>, cycle: &CycleRuntime) {
    out.push(cycle_kind_tag(cycle.kind));
    push_cycle_runtime_bytes_for_scenario(out, &cycle.scenario);
}

/// Encode a finance scenario's identity (name, seed, periods, money config, and the
/// full event timeline — including the G8c-2 `SetXTender` levers). Shared by the
/// cycle and the tender bench so a settlement's future behaviour is pinned by the
/// policy timeline it carries.
fn push_cycle_runtime_bytes_for_scenario(out: &mut Vec<u8>, scenario: &MarketScenario) {
    out.push(scenario_name_tag(scenario.scenario));
    out.extend_from_slice(&scenario.seed.to_le_bytes());
    out.extend_from_slice(&scenario.periods.to_le_bytes());
    push_market_money_config_bytes(out, &scenario.money);
    out.extend_from_slice(&(scenario.events.len() as u32).to_le_bytes());
    for event in &scenario.events {
        out.extend_from_slice(&event.tick.0.to_le_bytes());
        push_event_kind_bytes(out, &event.kind);
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

fn push_cycle_live_m2_bytes(out: &mut Vec<u8>, society: &Society) {
    out.extend_from_slice(&(society.m2_projects.len() as u32).to_le_bytes());
    for project in &society.m2_projects {
        push_m2_project_bytes(out, project);
    }

    out.extend_from_slice(&(society.debts.len() as u32).to_le_bytes());
    for debt in &society.debts {
        push_debt_contract_bytes(out, debt);
    }

    out.extend_from_slice(&(society.project_funding_plans.len() as u32).to_le_bytes());
    for plan in &society.project_funding_plans {
        push_project_funding_plan_bytes(out, plan);
    }

    out.extend_from_slice(&(society.project_output_lots.len() as u32).to_le_bytes());
    for lot in &society.project_output_lots {
        push_project_output_lot_bytes(out, lot);
    }
}

fn push_m2_project_bytes(out: &mut Vec<u8>, project: &M2Project) {
    out.extend_from_slice(&project.id.0.to_le_bytes());
    out.extend_from_slice(&project.owner.0.to_le_bytes());
    out.extend_from_slice(&project.line.0.to_le_bytes());
    out.push(m2_project_state_tag(project.state));
    out.extend_from_slice(&project.started_at.0.to_le_bytes());
    out.extend_from_slice(&project.maturity.0.to_le_bytes());
    out.extend_from_slice(&project.labor_advanced.to_le_bytes());
    out.extend_from_slice(&(project.input_goods_committed.len() as u32).to_le_bytes());
    for &(good, qty) in &project.input_goods_committed {
        out.extend_from_slice(&good.0.to_le_bytes());
        out.extend_from_slice(&qty.to_le_bytes());
    }
    out.extend_from_slice(&project.input_cost_basis.0.to_le_bytes());
    out.extend_from_slice(&project.advanced_gold.0.to_le_bytes());
    out.extend_from_slice(&project.expected_revenue.0.to_le_bytes());
    out.extend_from_slice(&project.output_good.0.to_le_bytes());
    out.extend_from_slice(&project.output_qty.to_le_bytes());
    out.extend_from_slice(&project.salvage_bps.to_le_bytes());
}

fn push_debt_contract_bytes(out: &mut Vec<u8>, debt: &DebtContract) {
    out.extend_from_slice(&debt.id.0.to_le_bytes());
    push_credit_lender_bytes(out, debt.lender);
    out.extend_from_slice(&debt.borrower.0.to_le_bytes());
    out.extend_from_slice(&debt.opened_tick.0.to_le_bytes());
    out.extend_from_slice(&debt.due_tick.0.to_le_bytes());
    out.extend_from_slice(&debt.principal.0.to_le_bytes());
    out.extend_from_slice(&debt.due.0.to_le_bytes());
    out.extend_from_slice(&debt.paid.0.to_le_bytes());
    out.push(debt_state_tag(debt.state));
    push_debt_purpose_bytes(out, &debt.purpose);
    push_credit_source_bytes(out, debt.funding);
}

fn push_project_funding_plan_bytes(out: &mut Vec<u8>, plan: &ProjectFundingPlan) {
    out.extend_from_slice(&plan.id.0.to_le_bytes());
    out.extend_from_slice(&plan.owner.0.to_le_bytes());
    out.extend_from_slice(&plan.line.0.to_le_bytes());
    out.extend_from_slice(&plan.created_tick.0.to_le_bytes());
    out.extend_from_slice(&plan.expires_tick.0.to_le_bytes());
    out.extend_from_slice(&plan.expected_revenue.0.to_le_bytes());
    out.extend_from_slice(&plan.input_cost_basis.0.to_le_bytes());
    out.extend_from_slice(&plan.required_labor.to_le_bytes());
    out.extend_from_slice(&plan.funding_horizon.to_le_bytes());
    out.extend_from_slice(&plan.borrowed_gold.0.to_le_bytes());
    out.extend_from_slice(&plan.future_due_committed.0.to_le_bytes());
    out.extend_from_slice(&plan.reserved_gold.0.to_le_bytes());
    match plan.started_project {
        Some(project) => {
            out.push(1);
            out.extend_from_slice(&project.0.to_le_bytes());
        }
        None => out.push(0),
    }
}

fn push_project_output_lot_bytes(out: &mut Vec<u8>, lot: &ProjectOutputLot) {
    out.extend_from_slice(&lot.project.0.to_le_bytes());
    out.extend_from_slice(&lot.owner.0.to_le_bytes());
    out.extend_from_slice(&lot.good.0.to_le_bytes());
    out.extend_from_slice(&lot.qty_remaining.to_le_bytes());
    out.extend_from_slice(&lot.proceeds.0.to_le_bytes());
}

fn push_market_money_config_bytes(out: &mut Vec<u8>, money: &MarketMoneyConfig) {
    match money {
        MarketMoneyConfig::Designated(money) => {
            out.push(0);
            out.extend_from_slice(&money.good.0.to_le_bytes());
        }
        MarketMoneyConfig::Emergent(menger) => {
            out.push(1);
            push_mengerian_config_bytes(out, menger);
        }
    }
}

fn push_event_kind_bytes(out: &mut Vec<u8>, kind: &EventKind) {
    match kind {
        EventKind::DisableRecipe(recipe) => {
            out.push(0);
            out.push(recipe_id_tag(*recipe));
        }
        EventKind::SetRegime(regime) => {
            out.push(1);
            out.push(regime_tag(*regime));
        }
        EventKind::SetReserveRatio { bank, ratio } => {
            out.push(2);
            out.extend_from_slice(&bank.0.to_le_bytes());
            out.extend_from_slice(&ratio.0.to_le_bytes());
        }
        EventKind::SetBankConvertibility { bank, convertible } => {
            out.push(3);
            out.extend_from_slice(&bank.0.to_le_bytes());
            out.push(u8::from(*convertible));
        }
        EventKind::SetBankCreditPolicy { bank, policy } => {
            out.push(4);
            out.extend_from_slice(&bank.0.to_le_bytes());
            push_bank_policy_bytes(out, policy);
        }
        EventKind::StopBankCredit { bank } => {
            out.push(5);
            out.extend_from_slice(&bank.0.to_le_bytes());
        }
        EventKind::RedeemDemandClaims {
            bank,
            route,
            max_per_agent,
        } => {
            out.push(6);
            out.extend_from_slice(&bank.0.to_le_bytes());
            push_redemption_route_bytes(out, route);
            match max_per_agent {
                Some(max) => {
                    out.push(1);
                    out.extend_from_slice(&max.0.to_le_bytes());
                }
                None => out.push(0),
            }
        }
        EventKind::FiatPrint {
            issuer,
            amount,
            route,
        } => {
            out.push(7);
            out.extend_from_slice(&issuer.0.to_le_bytes());
            out.extend_from_slice(&amount.0.to_le_bytes());
            push_cantillon_route_bytes(out, route);
        }
        EventKind::ResetPublicSpotBook => out.push(8),
        EventKind::SetPublicSpotTender(tender) => {
            out.push(9);
            out.push(public_spot_tender_tag(*tender));
        }
        EventKind::SetPublicDebtTender(tender) => {
            out.push(10);
            out.push(public_debt_tender_tag(*tender));
        }
        EventKind::SetBankRepaymentTender(tender) => {
            out.push(11);
            out.push(bank_repayment_tender_tag(*tender));
        }
        EventKind::SetIssuerRepaymentTender(tender) => {
            out.push(12);
            out.push(issuer_repayment_tender_tag(*tender));
        }
        EventKind::SetLaborWageTender(tender) => {
            out.push(13);
            out.push(labor_wage_tender_tag(*tender));
        }
        EventKind::SetTaxReceivability(receivability) => {
            out.push(14);
            out.push(tax_receivability_tag(*receivability));
        }
        EventKind::LevyTax {
            agent,
            amount,
            due_tick,
        } => {
            out.push(15);
            out.extend_from_slice(&agent.0.to_le_bytes());
            out.extend_from_slice(&amount.0.to_le_bytes());
            out.extend_from_slice(&due_tick.0.to_le_bytes());
        }
        EventKind::SetDebtDueTick { debt, due_tick } => {
            out.push(16);
            out.extend_from_slice(&debt.0.to_le_bytes());
            out.extend_from_slice(&due_tick.0.to_le_bytes());
        }
        EventKind::SeedCommodityDebt {
            lender,
            borrower,
            principal,
            due,
            due_tick,
            purpose,
        } => {
            out.push(17);
            out.extend_from_slice(&lender.0.to_le_bytes());
            out.extend_from_slice(&borrower.0.to_le_bytes());
            out.extend_from_slice(&principal.0.to_le_bytes());
            out.extend_from_slice(&due.0.to_le_bytes());
            out.extend_from_slice(&due_tick.0.to_le_bytes());
            push_debt_purpose_bytes(out, purpose);
        }
        EventKind::SeedStock { agent, good, qty } => {
            out.push(18);
            out.extend_from_slice(&agent.0.to_le_bytes());
            out.extend_from_slice(&good.0.to_le_bytes());
            out.extend_from_slice(&qty.to_le_bytes());
        }
        EventKind::SetIssuerPolicy { issuer, policy } => {
            out.push(19);
            out.extend_from_slice(&issuer.0.to_le_bytes());
            push_issuer_policy_bytes(out, policy);
        }
        EventKind::StopIssuerCredit { issuer } => {
            out.push(20);
            out.extend_from_slice(&issuer.0.to_le_bytes());
        }
    }
}

fn push_bank_policy_bytes(out: &mut Vec<u8>, policy: &BankPolicy) {
    out.extend_from_slice(&policy.max_new_fiduciary_per_tick.0.to_le_bytes());
    out.extend_from_slice(&policy.loan_present.0.to_le_bytes());
    out.push(policy.loan_horizon);
    out.extend_from_slice(&policy.loan_future_due.0.to_le_bytes());
    out.push(u8::from(policy.enabled));
}

fn push_issuer_policy_bytes(out: &mut Vec<u8>, policy: &econ::issuer::IssuerPolicy) {
    out.push(u8::from(policy.fiscal_enabled));
    out.push(u8::from(policy.credit_enabled));
    out.extend_from_slice(&policy.max_fiscal_issue_per_tick.0.to_le_bytes());
    out.extend_from_slice(&policy.max_credit_issue_per_tick.0.to_le_bytes());
    out.extend_from_slice(&policy.loan_present.0.to_le_bytes());
    out.push(policy.loan_horizon);
    out.extend_from_slice(&policy.loan_future_due.0.to_le_bytes());
}

fn push_redemption_route_bytes(out: &mut Vec<u8>, route: &RedemptionRoute) {
    match route {
        RedemptionRoute::Agents(agents) => {
            out.push(0);
            out.extend_from_slice(&(agents.len() as u32).to_le_bytes());
            for agent in agents {
                out.extend_from_slice(&agent.0.to_le_bytes());
            }
        }
        RedemptionRoute::AllClaimHolders => out.push(1),
    }
}

fn push_cantillon_route_bytes(out: &mut Vec<u8>, route: &CantillonRoute) {
    match route {
        CantillonRoute::Agents(agents) => {
            out.push(0);
            out.extend_from_slice(&(agents.len() as u32).to_le_bytes());
            for agent in agents {
                out.extend_from_slice(&agent.0.to_le_bytes());
            }
        }
        CantillonRoute::Sector(sector) => {
            out.push(1);
            out.push(cantillon_sector_tag(*sector));
        }
        CantillonRoute::Helicopter => out.push(2),
    }
}

fn push_debt_purpose_bytes(out: &mut Vec<u8>, purpose: &DebtPurpose) {
    match purpose {
        DebtPurpose::Consumption => out.push(0),
        DebtPurpose::ProjectFunding { plan, project } => {
            out.push(1);
            out.extend_from_slice(&plan.0.to_le_bytes());
            match project {
                Some(project) => {
                    out.push(1);
                    out.extend_from_slice(&project.0.to_le_bytes());
                }
                None => out.push(0),
            }
        }
        DebtPurpose::TaxLiability => out.push(2),
    }
}

fn push_credit_lender_bytes(out: &mut Vec<u8>, lender: CreditLender) {
    match lender {
        CreditLender::Agent(agent) => {
            out.push(0);
            out.extend_from_slice(&agent.0.to_le_bytes());
        }
        CreditLender::Bank(bank) => {
            out.push(1);
            out.extend_from_slice(&bank.0.to_le_bytes());
        }
        CreditLender::Issuer(issuer) => {
            out.push(2);
            out.extend_from_slice(&issuer.0.to_le_bytes());
        }
    }
}

fn push_credit_source_bytes(out: &mut Vec<u8>, source: CreditSource) {
    match source {
        CreditSource::Commodity => out.push(0),
        CreditSource::BankFiduciary(bank) => {
            out.push(1);
            out.extend_from_slice(&bank.0.to_le_bytes());
        }
        CreditSource::FiatFiscal(issuer) => {
            out.push(2);
            out.extend_from_slice(&issuer.0.to_le_bytes());
        }
        CreditSource::FiatCredit(issuer) => {
            out.push(3);
            out.extend_from_slice(&issuer.0.to_le_bytes());
        }
        CreditSource::Tax(issuer) => {
            out.push(4);
            out.extend_from_slice(&issuer.0.to_le_bytes());
        }
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
    }
}

fn debt_state_tag(state: DebtState) -> u8 {
    match state {
        DebtState::Open => 0,
        DebtState::Settled => 1,
        DebtState::Defaulted => 2,
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
        // S9: the indirect-exchange breadth (volume + distinct indirect acceptors +
        // distinct indirect targets) the strong-bar gate reads. A future acceptance
        // only advances the gate when its acceptor/target is new, so the member sets
        // — not just their counts — are part of the future-behaviour identity.
        out.extend_from_slice(&candidate.indirect_acceptances.to_le_bytes());
        out.extend_from_slice(&(candidate.indirect_acceptor_agents.len() as u32).to_le_bytes());
        for agent in candidate.indirect_acceptor_agents {
            out.extend_from_slice(&agent.0.to_le_bytes());
        }
        out.extend_from_slice(&(candidate.indirect_target_goods.len() as u32).to_le_bytes());
        for good in candidate.indirect_target_goods {
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
    // S9 strong-bar gate: these steer the future promotion decision (they withhold
    // promotion until indirect breadth accrues / disable indirect acceptance), so
    // they are part of the future-behaviour identity. Appended last so every pre-S9
    // Mengerian config's prefix is unchanged.
    out.extend_from_slice(&menger.min_indirect_acceptances.to_le_bytes());
    out.extend_from_slice(&menger.min_indirect_acceptor_agents.to_le_bytes());
    out.extend_from_slice(&menger.min_indirect_target_goods.to_le_bytes());
    out.push(u8::from(menger.allow_indirect_acceptance));
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
    fn external_econ_canonical_tags_are_pinned() {
        assert_eq!(cycle_kind_tag(CycleKind::CreditCycle), 0);
        assert_eq!(cycle_kind_tag(CycleKind::SoundMoney), 1);

        let scenarios = [
            ScenarioName::CrusoeSurvival,
            ScenarioName::CrusoeCapital,
            ScenarioName::CrusoeAbandon,
            ScenarioName::MarketBarterishGold,
            ScenarioName::MarketPriceDiscovery,
            ScenarioName::MarketNoMutualBenefit,
            ScenarioName::TimeMarketBasic,
            ScenarioName::RoundaboutCapital,
            ScenarioName::BorrowToBuild,
            ScenarioName::SoundMoney100Pct,
            ScenarioName::CommodityCreditNeutral,
            ScenarioName::FractionalReserve,
            ScenarioName::SuspensionOfConvertibility,
            ScenarioName::FiatCreditExpansion,
            ScenarioName::FiatFiscalCantillon,
            ScenarioName::CantillonIsolation,
            ScenarioName::EmergedGoldSoundControl,
            ScenarioName::EmergedGoldFiatDisplacement,
            ScenarioName::EmergedGoldFiatRefusalControl,
            ScenarioName::EmergedGoldFiatLegalTender,
            ScenarioName::EmergedGoldFiatDebtRefusalControl,
            ScenarioName::EmergedGoldFiatDebtLegalTender,
            ScenarioName::EmergedGoldBankClaimDebtRefusalControl,
            ScenarioName::EmergedGoldBankClaimDebtLegalTender,
            ScenarioName::EmergedGoldBankClaimSpotRefusalControl,
            ScenarioName::EmergedGoldBankClaimSpotLegalTender,
            ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl,
            ScenarioName::EmergedGoldBankLoanRepaymentClaimTender,
            ScenarioName::EmergedGoldFractionalReserve,
            ScenarioName::EmergedGoldFiatCreditExpansion,
            ScenarioName::EmergedGoldFiatWageRefusalControl,
            ScenarioName::EmergedGoldFiatWageLegalTender,
            ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl,
            ScenarioName::EmergedGoldIssuerRepaymentFiatTender,
            ScenarioName::EmergedGoldReserveLeashControl,
            ScenarioName::EmergedGoldSuspensionOfConvertibility,
            ScenarioName::EmergedGoldRedemptionRun,
            ScenarioName::EmergedGoldSuspendedRedemption,
            ScenarioName::EmergedGoldTaxSpecieControl,
            ScenarioName::EmergedGoldTaxFiatUnpayableDefaults,
            ScenarioName::EmergedGoldTaxDrivesFiatLabor,
            ScenarioName::EmergedGoldNoTaxIdleControl,
            ScenarioName::MengerSaltMoney,
            ScenarioName::MengerGoldMoney,
        ];
        for (expected, scenario) in scenarios.into_iter().enumerate() {
            assert_eq!(scenario_name_tag(scenario), expected as u8);
        }

        assert_eq!(recipe_id_tag(RecipeId::GatherFood), 0);
        assert_eq!(recipe_id_tag(RecipeId::CutWood), 1);
        assert_eq!(recipe_id_tag(RecipeId::FishWithNet), 2);
        assert_eq!(recipe_id_tag(RecipeId::Mill), 3);
        assert_eq!(recipe_id_tag(RecipeId::Bake), 4);
        assert_eq!(recipe_id_tag(RecipeId::Research), 5);
        assert_eq!(recipe_id_tag(RecipeId::Confect), 6);

        assert_eq!(cantillon_sector_tag(CantillonSector::Capitalists), 0);
        assert_eq!(cantillon_sector_tag(CantillonSector::Households), 1);
        assert_eq!(cantillon_sector_tag(CantillonSector::Workers), 2);
        assert_eq!(cantillon_sector_tag(CantillonSector::Consumers), 3);

        assert_eq!(public_debt_tender_tag(PublicDebtTender::ParAll), 0);
        assert_eq!(public_debt_tender_tag(PublicDebtTender::SpecieOnly), 1);
        assert_eq!(public_debt_tender_tag(PublicDebtTender::FiatAndSpecie), 2);
        assert_eq!(
            public_debt_tender_tag(PublicDebtTender::BankClaimsAndSpecie),
            3
        );

        assert_eq!(bank_repayment_tender_tag(BankRepaymentTender::ParAll), 0);
        assert_eq!(
            bank_repayment_tender_tag(BankRepaymentTender::SpecieOnly),
            1
        );
        assert_eq!(
            bank_repayment_tender_tag(BankRepaymentTender::FiatAndSpecie),
            2
        );
        assert_eq!(
            bank_repayment_tender_tag(BankRepaymentTender::BankClaimsAndSpecie),
            3
        );

        assert_eq!(
            issuer_repayment_tender_tag(IssuerRepaymentTender::FiatOnly),
            0
        );
        assert_eq!(
            issuer_repayment_tender_tag(IssuerRepaymentTender::FiatRefused),
            1
        );

        assert_eq!(labor_wage_tender_tag(LaborWageTender::ParAll), 0);
        assert_eq!(labor_wage_tender_tag(LaborWageTender::SpecieOnly), 1);
        assert_eq!(labor_wage_tender_tag(LaborWageTender::FiatAndSpecie), 2);

        assert_eq!(tax_receivability_tag(TaxReceivability::SpecieOnly), 0);
        assert_eq!(tax_receivability_tag(TaxReceivability::FiatOnly), 1);
        assert_eq!(tax_receivability_tag(TaxReceivability::FiatAndSpecie), 2);

        assert_eq!(regime_tag(Regime::SoundGold), 0);
        assert_eq!(regime_tag(Regime::FractionalConvertible), 1);
        assert_eq!(regime_tag(Regime::SuspendedConvertibility), 2);
        assert_eq!(regime_tag(Regime::Fiat), 3);

        assert_eq!(public_spot_tender_tag(PublicSpotTender::ParAll), 0);
        assert_eq!(public_spot_tender_tag(PublicSpotTender::SpecieOnly), 1);
        assert_eq!(public_spot_tender_tag(PublicSpotTender::FiatAndSpecie), 2);
        assert_eq!(
            public_spot_tender_tag(PublicSpotTender::BankClaimsAndSpecie),
            3
        );

        assert_eq!(bench_surface_tag(BenchSurface::Spot), 0);
        assert_eq!(bench_surface_tag(BenchSurface::Debt), 1);
        assert_eq!(bench_surface_tag(BenchSurface::BankRepayment), 2);
        assert_eq!(bench_surface_tag(BenchSurface::IssuerRepayment), 3);

        assert_eq!(project_template_id_tag(ProjectTemplateId::BuildNet), 0);
        assert_eq!(project_template_id_tag(ProjectTemplateId::BuildRoad), 1);
        assert_eq!(project_template_id_tag(ProjectTemplateId::BuildMill), 2);
        assert_eq!(project_template_id_tag(ProjectTemplateId::BuildOven), 3);
    }

    /// The G8c-2 tender policy emits a `SetXTender` event **only** for a knob that
    /// differs from econ's default — so a default policy contributes zero events
    /// (keeping the G8c-1 finance bytes byte-identical), and each non-default knob
    /// emits exactly its surface's event at `Tick(0)`.
    #[test]
    fn tender_events_emit_only_non_default_knobs() {
        // The default policy is inert: no events at all.
        assert!(TenderPolicy::default().tender_events().is_empty());

        // A single non-default knob (the wage refusal) emits exactly one wage event.
        let wage_only = TenderPolicy {
            wage: LaborWageTender::SpecieOnly,
            ..TenderPolicy::default()
        };
        let events = wage_only.tender_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].tick, Tick(0));
        assert!(matches!(
            events[0].kind,
            EventKind::SetLaborWageTender(LaborWageTender::SpecieOnly)
        ));

        // Every surface set to a non-default emits one event per surface, in the fixed
        // order spot, debt, bank-repayment, issuer-repayment, wage.
        let all = TenderPolicy {
            spot: PublicSpotTender::SpecieOnly,
            wage: LaborWageTender::FiatAndSpecie,
            debt: PublicDebtTender::SpecieOnly,
            bank_repayment: BankRepaymentTender::SpecieOnly,
            issuer_repayment: IssuerRepaymentTender::FiatRefused,
        };
        let kinds: Vec<_> = all.tender_events().into_iter().map(|e| e.kind).collect();
        assert!(matches!(kinds[0], EventKind::SetPublicSpotTender(_)));
        assert!(matches!(kinds[1], EventKind::SetPublicDebtTender(_)));
        assert!(matches!(kinds[2], EventKind::SetBankRepaymentTender(_)));
        assert!(matches!(kinds[3], EventKind::SetIssuerRepaymentTender(_)));
        assert!(matches!(kinds[4], EventKind::SetLaborWageTender(_)));
        assert_eq!(kinds.len(), 5);
    }

    /// The default `TenderPolicy` equals econ's per-surface defaults, so a default
    /// cycle is byte-identical to the policy-free G8c-1 cycle (the finance-bytes
    /// tripwire).
    #[test]
    fn default_tender_policy_matches_econ_defaults() {
        let default = TenderPolicy::default();
        assert_eq!(default.spot, PublicSpotTender::ParAll);
        assert_eq!(default.wage, LaborWageTender::ParAll);
        assert_eq!(default.debt, PublicDebtTender::ParAll);
        assert_eq!(default.bank_repayment, BankRepaymentTender::ParAll);
        assert_eq!(default.issuer_repayment, IssuerRepaymentTender::FiatOnly);
    }

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
    fn direct_use_scale_extension_inserts_now_consumption_wants_below_survival() {
        // S9: the heterogeneous direct use is a `Horizon::Now` CONSUMPTION want (not
        // a `Horizon::Next` savings want like the medium). It lands between the
        // survival present wants and the savings ladder, exactly like the medium
        // block, but tagged `Now` so the consume arm eats it into the `consumed`
        // bucket.
        let mut scale = vec![
            Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Now,
                qty: 1,
                satisfied: false,
            },
            Want {
                kind: WantKind::Good(GOLD),
                horizon: Horizon::Later(4),
                qty: 1,
                satisfied: false,
            },
        ];
        direct_use_scale_extension(&mut scale, SALT, 2);
        assert_eq!(scale.len(), 4, "two direct-use wants were added");
        // Survival (the Now food want) stays first.
        assert_eq!(scale[0].kind, WantKind::Good(FOOD));
        assert!(matches!(scale[0].horizon, Horizon::Now));
        // The two SALT direct-use wants follow as single-unit `Now` consumption
        // wants, before the Later savings want.
        assert_eq!(scale[1].kind, WantKind::Good(SALT));
        assert_eq!(scale[1].horizon, Horizon::Now);
        assert_eq!(scale[1].qty, 1);
        assert_eq!(scale[2].kind, WantKind::Good(SALT));
        assert_eq!(scale[2].horizon, Horizon::Now);
        assert!(matches!(scale[3].horizon, Horizon::Later(_)));

        // Zero qty is a no-op.
        let mut empty = scale.clone();
        let before = empty.clone();
        direct_use_scale_extension(&mut empty, SALT, 0);
        assert_eq!(empty, before);
    }

    #[test]
    fn canonical_bytes_include_salt_direct_use() {
        // S9: the heterogeneous direct-use seed steers which colonists barter for
        // SALT pre-promotion (and thus the saleability the promotion reads), so both
        // knobs are part of the determinism identity before the first tick.
        let base = SettlementConfig::frontier_coemergent();

        let mut with_qty = SettlementConfig::frontier_coemergent();
        let b = with_qty.barter.as_mut().expect("barter overlay");
        b.salt_direct_use_qty = 1;
        b.salt_direct_use_period = 8;

        let mut other_period = SettlementConfig::frontier_coemergent();
        let b = other_period.barter.as_mut().expect("barter overlay");
        b.salt_direct_use_qty = 1;
        b.salt_direct_use_period = 4;

        let base = Settlement::generate(7, &base);
        let with_qty = Settlement::generate(7, &with_qty);
        let other_period = Settlement::generate(7, &other_period);

        assert_ne!(
            base.canonical_bytes(),
            with_qty.canonical_bytes(),
            "salt_direct_use_qty/period must be part of the barter config identity"
        );
        assert_ne!(
            with_qty.canonical_bytes(),
            other_period.canonical_bytes(),
            "the heterogeneity period must be part of the barter config identity"
        );
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
            let fast = s.run_fast_loop();
            s.record_pending_deposits(fast.deposited);
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
    fn canonical_bytes_include_tool_acquisition_eligibility() {
        // S7.1: the tool-acquisition eligibility gate relaxes role-choice and adds the
        // acquired-tool scale anchor, steering future ticks for any chain — so two
        // chains differing only in it must digest apart.
        let mut off = SettlementConfig::frontier_endogenous();
        off.chain
            .as_mut()
            .expect("chain")
            .tool_acquisition_eligibility = false;
        let mut on = SettlementConfig::frontier_endogenous();
        on.chain
            .as_mut()
            .expect("chain")
            .tool_acquisition_eligibility = true;
        assert_ne!(
            Settlement::generate(7, &off).canonical_bytes(),
            Settlement::generate(7, &on).canonical_bytes(),
            "the tool-acquisition eligibility gate must be part of the chain config identity"
        );

        // The widened role-choice gate: even a SEEDED chain with no latent pool now
        // serializes the operating cost when eligibility is on (role-choice can act on
        // a tool-holder), so a chain that the latent-pool gate alone would have left
        // operating-cost-blind splits on the operating cost under eligibility.
        let mut elig = SettlementConfig::grain_flour_bread_chain();
        elig.chain
            .as_mut()
            .expect("chain")
            .tool_acquisition_eligibility = true;
        let mut elig_dearer = elig.clone();
        elig_dearer.chain.as_mut().expect("chain").operating_cost += 1;
        assert_ne!(
            Settlement::generate(7, &elig).canonical_bytes(),
            Settlement::generate(7, &elig_dearer).canonical_bytes(),
            "with eligibility on, the operating cost must steer the digest even with no latent pool"
        );

        // Tripwire: with eligibility OFF, the same seeded chain stays
        // operating-cost-blind (the pre-S7 contract) — proven by
        // `seeded_chain_digest_ignores_unused_operating_cost`, re-checked here against
        // the eligibility-on twin so the widening is the ONLY thing that flips it.
        let base = SettlementConfig::grain_flour_bread_chain();
        let mut base_dearer = base.clone();
        base_dearer.chain.as_mut().expect("chain").operating_cost += 1;
        assert_eq!(
            Settlement::generate(7, &base).canonical_bytes(),
            Settlement::generate(7, &base_dearer).canonical_bytes(),
            "with eligibility off, the seeded chain must stay operating-cost-blind"
        );
    }

    #[test]
    fn tool_acquisition_admits_a_non_latent_tool_holder() {
        // S7.1 (the keystone): a colonist that is NOT seeded latent but is handed a
        // mill mid-run is admitted to the adoption appraisal, DOES NOT sell the mill on
        // the market step, adopts Miller, and actually produces flour. With the gate
        // OFF the same handed mill changes nothing — a non-latent colonist holding a
        // mill is not eligible, never adopts, and never mills.
        let mut on = SettlementConfig::frontier_endogenous();
        on.chain
            .as_mut()
            .expect("chain")
            .tool_acquisition_eligibility = true;
        let off = SettlementConfig::frontier_endogenous();
        let mill = on.chain.as_ref().expect("chain").content.mill();
        let flour = on.chain.as_ref().expect("chain").content.flour();

        // The first spatial, non-latent, non-producer colonist (a gatherer/consumer).
        // Deterministic, so the same index is picked across runs of the same config.
        let pick = |s: &Settlement| -> usize {
            (0..s.population())
                .find(|&i| {
                    s.is_alive(i)
                        && !s.is_tool_acquisition_eligible(i)
                        && matches!(
                            s.vocation_of(i),
                            Some(Vocation::Gatherer) | Some(Vocation::Consumer)
                        )
                })
                .expect("a non-latent, non-producer spatial colonist")
        };

        // Gate ON: hand the mill once prices have formed, then run.
        let mut s = Settlement::generate(42, &on);
        s.run(400);
        let idx = pick(&s);
        let id = s.colonist_id(idx).expect("a living colonist id");
        let mill_before = s.whole_system_total(mill);
        assert!(s.society_mut().credit_stock(id, mill, 1), "mill credited");
        // It is now eligible the very next appraisal — the gate relaxation, not a relabel.
        assert!(
            s.is_tool_acquisition_eligible(idx),
            "holding the mill must make the non-latent colonist eligible"
        );
        let mut flour_made = 0u64;
        for _ in 0..200 {
            let report = s.econ_tick();
            flour_made += report.produced_of(flour);
        }
        assert_eq!(
            s.vocation_of(idx),
            Some(Vocation::Miller),
            "the eligible tool-holder must adopt Miller"
        );
        assert!(
            s.society().agents.get(id).expect("agent").stock.get(mill) >= 1,
            "the eligible tool-holder must still hold its mill (not sold before adoption)"
        );
        assert!(
            s.whole_system_total(mill) > mill_before,
            "the handed mill must remain in the whole system (the tool count did not drop)"
        );
        assert!(
            flour_made > 0,
            "the adopted tool-holder must actually produce flour, got {flour_made}"
        );

        // Gate OFF: the same handed mill at the same point changes nothing — the colonist
        // is not eligible, never adopts, and never mills.
        let mut s_off = Settlement::generate(42, &off);
        s_off.run(400);
        let off_idx = pick(&s_off);
        let off_id = s_off.colonist_id(off_idx).expect("id");
        let voc_before = s_off.vocation_of(off_idx);
        assert!(s_off.society_mut().credit_stock(off_id, mill, 1));
        assert!(
            !s_off.is_tool_acquisition_eligible(off_idx),
            "with the gate off, holding a mill must not make a non-latent colonist eligible"
        );
        s_off.run(200);
        assert_eq!(
            s_off.vocation_of(off_idx),
            voc_before,
            "with the gate off a handed mill must not turn a non-latent colonist into a producer"
        );
    }

    #[test]
    fn tool_acquisition_waits_for_gatherer_spatial_state_to_settle() {
        // A non-latent gatherer handed a tool must finish any world-side haul before
        // switching to Miller/Baker. Otherwise its later deposit would not be attributed
        // by the fast loop, because deposits are tracked only for current gatherers.
        let mut cfg = SettlementConfig::frontier_endogenous();
        cfg.chain
            .as_mut()
            .expect("chain")
            .tool_acquisition_eligibility = true;
        let mill = cfg.chain.as_ref().expect("chain").content.mill();

        let mut s = Settlement::generate(42, &cfg);
        s.run(400);
        let idx = (0..s.population())
            .find(|&i| {
                s.is_alive(i)
                    && !s.is_tool_acquisition_eligible(i)
                    && s.vocation_of(i) == Some(Vocation::Gatherer)
                    && s.node_of(i).is_some()
            })
            .expect("a non-latent gatherer");
        let id = s.colonist_id(idx).expect("id");
        let node = s.node_of(idx).expect("gatherer node");
        let carried_good = s.world().node(node).expect("node").good;

        assert!(s.world.assign_task(id, Task::GoHarvest(node, s.carry_cap)));
        for _ in 0..64 {
            s.world.tick();
            if s.world.agent_carry_total(id) > 0 {
                break;
            }
        }
        assert!(
            s.world.agent_carry_total(id) > 0,
            "test setup must put a real harvested load in carry"
        );
        assert!(s.world.assign_task(id, Task::GoTo(Pos::new(48, 0))));
        for _ in 0..64 {
            s.world.tick();
            if s.world.agent_pos(id) == Some(Pos::new(48, 0)) {
                break;
            }
        }
        assert_eq!(
            s.world.agent_pos(id),
            Some(Pos::new(48, 0)),
            "test setup must park the loaded gatherer far from the exchange"
        );
        assert!(s.world.assign_task(id, Task::GoDeposit(s.exchange)));
        assert_eq!(s.world.agent_status(id), Some(AgentStatus::Moving));
        assert!(s.society_mut().credit_stock(id, mill, 1));

        let mut saw_unsettled_wait = false;
        let mut adopted = false;
        for tick in 0..200u64 {
            let report = s.econ_tick();
            assert!(report.conserves(), "tick {tick} must conserve");
            assert_eq!(
                s.world().stockpile_get(s.exchange(), carried_good),
                0,
                "each deposit must be attributed and transferred at tick {tick}"
            );
            let unsettled = s.world().agent_status(id) != Some(AgentStatus::Idle)
                || s.world().agent_carry_total(id) > 0;
            if unsettled {
                saw_unsettled_wait = true;
                assert_eq!(
                    s.vocation_of(idx),
                    Some(Vocation::Gatherer),
                    "a gatherer must not adopt while its haul is unsettled at tick {tick}"
                );
            }
            if s.vocation_of(idx) == Some(Vocation::Miller) {
                adopted = true;
                break;
            }
        }
        assert!(
            saw_unsettled_wait,
            "the regression must exercise a tool-holder with unsettled spatial state"
        );
        assert!(
            adopted,
            "the settled tool-holder must still adopt once the appraisal pays"
        );
    }

    #[test]
    fn reentry_does_not_revert_an_adopted_tool_holder() {
        // A non-latent spatial colonist that adopts from a held tool keeps its producer
        // role through the later same-tick re-entry phase. Its home role remains spatial,
        // but active tool producers are outside the S6 re-entry path.
        let mut cfg = SettlementConfig::frontier_endogenous_scaling();
        {
            let chain = cfg.chain.as_mut().expect("chain");
            chain.tool_acquisition_eligibility = true;
            chain.producible_capital = false;
        }
        let mill = cfg.chain.as_ref().expect("chain").content.mill();
        let mut s = Settlement::generate(42, &cfg);
        s.run(400);
        let idx = (0..s.population())
            .find(|&i| {
                s.is_alive(i)
                    && !s.is_tool_acquisition_eligible(i)
                    && matches!(
                        s.vocation_of(i),
                        Some(Vocation::Gatherer) | Some(Vocation::Consumer)
                    )
            })
            .expect("a non-latent spatial colonist");
        let id = s.colonist_id(idx).expect("id");
        assert!(s.society_mut().credit_stock(id, mill, 1));

        let slot = s.slot_for_id(id).expect("slot");
        let mut adopted = false;
        for _ in 0..200 {
            s.colonists[slot].need.hunger = 0;
            s.econ_tick();
            if s.vocation_of(idx) == Some(Vocation::Miller) {
                adopted = true;
                break;
            }
        }
        assert!(
            adopted,
            "re-entry must not revert an active tool-holder back to its spatial home role"
        );
    }

    #[test]
    fn tool_acquisition_off_is_byte_identical() {
        // S7.1 inertness: with the gate OFF, flipping the (unused) eligibility flag in
        // isolation is a no-op — the gate never fires without a non-latent tool-holder,
        // and generation is untouched, so a fresh run is byte-identical. (The autonomous
        // path that would create such a holder is S7.2, gated separately.)
        let off = SettlementConfig::frontier_endogenous();
        let mut a = Settlement::generate(0xC0FFEE, &off);
        let mut b = Settlement::generate(0xC0FFEE, &off);
        a.run(600);
        b.run(600);
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
        assert_eq!(a.digest(), b.digest());
    }

    /// A capital economy for the S7.2 mechanism tests: the scaling economy with both S7
    /// gates on and a larger colony, so bread demand genuinely outruns the seeded chain
    /// and the per-builder phase has real demand to respond to. Self-contained (does not
    /// depend on the S7.3 `frontier_capital` scenario).
    fn capital_test_config() -> SettlementConfig {
        let mut cfg = SettlementConfig::frontier_endogenous_scaling();
        {
            let c = cfg.chain.as_mut().expect("chain");
            c.tool_acquisition_eligibility = true;
            c.producible_capital = true;
            c.capital_payback_cycles = 16;
            c.tool_build_wood = 6;
            c.tool_build_labor = 4;
            c.capital_build_hunger_max = 4;
        }
        cfg.consumers = 44;
        cfg.gatherers = 24;
        cfg
    }

    #[test]
    fn canonical_bytes_include_producible_capital() {
        // S7.2: producible_capital and its appraisal knobs steer future ticks (whether
        // and when a tool is built), so two chains differing only in one must digest
        // apart — and with the phase OFF the unused knobs must NOT split the digest.
        let mut off = SettlementConfig::frontier_endogenous();
        off.chain
            .as_mut()
            .expect("chain")
            .tool_acquisition_eligibility = true;
        let mut on = off.clone();
        on.chain.as_mut().expect("chain").producible_capital = true;
        let off_bytes = Settlement::generate(7, &off).canonical_bytes();
        assert_ne!(
            off_bytes,
            Settlement::generate(7, &on).canonical_bytes(),
            "the producible-capital phase gate must be part of the chain config identity"
        );

        // Phase ON: each appraisal knob must split the digest.
        for mutate in [
            (|c: &mut ChainConfig| c.capital_payback_cycles += 1) as fn(&mut ChainConfig),
            |c: &mut ChainConfig| c.tool_build_wood += 1,
            |c: &mut ChainConfig| c.tool_build_labor += 1,
            |c: &mut ChainConfig| c.capital_build_hunger_max += 1,
        ] {
            let mut tweaked = on.clone();
            mutate(tweaked.chain.as_mut().expect("chain"));
            assert_ne!(
                Settlement::generate(7, &on).canonical_bytes(),
                Settlement::generate(7, &tweaked).canonical_bytes(),
                "with producible capital on, every appraisal knob must steer the digest"
            );
        }

        // Phase OFF: the same (unused) knobs must NOT split the digest, or the tripwire
        // would call two behaviour-identical configs unequal.
        let mut off_tweaked = off.clone();
        {
            let c = off_tweaked.chain.as_mut().expect("chain");
            c.capital_payback_cycles += 5;
            c.tool_build_wood += 5;
            c.tool_build_labor += 5;
            c.capital_build_hunger_max += 5;
        }
        assert_eq!(
            off_bytes,
            Settlement::generate(7, &off_tweaked).canonical_bytes(),
            "with producible capital off, the unused build knobs must not steer the digest"
        );

        let id_a = Settlement::generate(7, &on);
        let mut id_b = Settlement::generate(7, &on);
        id_b.next_capital_project_id = id_b.next_capital_project_id.saturating_add(1);
        assert_ne!(
            id_a.canonical_bytes(),
            id_b.canonical_bytes(),
            "the next capital project id steers future project ids and must be serialized"
        );

        let build_cfg = capital_test_config();
        let mut build_state = Settlement::generate(7, &build_cfg);
        for _ in 0..900 {
            build_state.econ_tick();
            if !build_state.capital_builds.is_empty() {
                break;
            }
        }
        assert!(
            !build_state.capital_builds.is_empty(),
            "the capital config should produce an in-flight build for the digest check"
        );
        let before = build_state.canonical_bytes();
        build_state.capital_builds[0].project.id =
            ProjectId(build_state.capital_builds[0].project.id.0.saturating_add(1));
        assert_ne!(
            before,
            build_state.canonical_bytes(),
            "an in-flight capital project's id must be serialized"
        );
    }

    #[test]
    fn canonical_bytes_include_per_agent_capital() {
        // S10: the per_agent_capital flag steers every future tick (it replaces the S7
        // build planner with a per-colonist ordinal decision), so it is part of the chain
        // config identity. And in per-agent mode capital_payback_cycles is behaviour-INERT
        // — two per-agent configs differing only in it must NOT digest apart (no false
        // split for a behaviour-inert knob).
        let mut heuristic = capital_test_config();
        heuristic.chain.as_mut().expect("chain").per_agent_capital = false;
        let mut per_agent = heuristic.clone();
        per_agent.chain.as_mut().expect("chain").per_agent_capital = true;

        // per-agent ON vs the S7 heuristic must digest apart (the gate is in the identity).
        assert_ne!(
            Settlement::generate(7, &heuristic).canonical_bytes(),
            Settlement::generate(7, &per_agent).canonical_bytes(),
            "per_agent_capital must be part of the chain config identity"
        );

        // In per-agent mode capital_payback_cycles is inert — including across a live run
        // (the per-agent decision never reads it) — so the digest must not split on it.
        let mut per_agent_other = per_agent.clone();
        per_agent_other
            .chain
            .as_mut()
            .expect("chain")
            .capital_payback_cycles += 9;
        let mut a = Settlement::generate(7, &per_agent);
        let mut b = Settlement::generate(7, &per_agent_other);
        a.run(120);
        b.run(120);
        assert_eq!(
            a.canonical_bytes(),
            b.canonical_bytes(),
            "capital_payback_cycles is inert in per-agent mode and must not split the digest"
        );
        assert_eq!(a.digest(), b.digest());

        // The still-active build knobs DO steer per-agent builds, so each must split.
        for mutate in [
            (|c: &mut ChainConfig| c.tool_build_wood += 1) as fn(&mut ChainConfig),
            |c: &mut ChainConfig| c.tool_build_labor += 1,
            |c: &mut ChainConfig| c.capital_build_hunger_max += 1,
        ] {
            let mut tweaked = per_agent.clone();
            mutate(tweaked.chain.as_mut().expect("chain"));
            assert_ne!(
                Settlement::generate(7, &per_agent).canonical_bytes(),
                Settlement::generate(7, &tweaked).canonical_bytes(),
                "with per-agent capital on, the active build knobs must steer the digest"
            );
        }
    }

    #[test]
    fn forecast_output_price_grounds_on_belief_then_realized() {
        // S11: the grounded fallible forecast — belief.expected when observed (× bias),
        // else the public realized price (× bias), else None. The bias is a standing
        // multiplier, so a biased agent systematically over/under-shoots.
        let mut agent = Agent {
            id: AgentId(1),
            scale: Vec::new(),
            stock: Stock::new(NET.0),
            gold: Gold::ZERO,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Household],
            expect: belief_vec(),
        };
        // FOOD: an OBSERVED belief grounds the forecast on `expected`, IGNORING realized.
        agent.expect[usize::from(FOOD.0)] = PriceBelief {
            expected: Gold(10),
            step: Gold(1),
            last_seen: 0,
            observed: true,
        };
        assert_eq!(
            forecast_output_price(&agent, FOOD, Some(Gold(5)), 10_000),
            Some(Gold(10)),
            "neutral bias on an observed belief forecasts the belief level, not realized"
        );
        assert_eq!(
            forecast_output_price(&agent, FOOD, Some(Gold(5)), 20_000),
            Some(Gold(20)),
            "an optimist over-shoots its belief by ×2"
        );
        assert_eq!(
            forecast_output_price(&agent, FOOD, None, 5_000),
            Some(Gold(5)),
            "a pessimist under-shoots its belief by ×0.5 (realized absent is irrelevant)"
        );
        // WOOD: an UN-observed belief falls back to the public realized price.
        assert!(!agent_belief(&agent, WOOD).observed);
        assert_eq!(
            forecast_output_price(&agent, WOOD, Some(Gold(6)), 20_000),
            Some(Gold(12)),
            "an un-observed good grounds on realized × bias"
        );
        // No belief AND no realized price → no forecast (the decision is skipped).
        assert_eq!(forecast_output_price(&agent, WOOD, None, 10_000), None);
    }

    #[test]
    fn project_input_bid_limit_anchors_forecast_bid_to_observed_input_price() {
        // S11: the forecast-inflated reservation can make a producer willing to buy input,
        // but the posted limit stays at the observed input price when one exists. That
        // keeps a resting producer bid from setting a higher input price solely because its
        // output forecast was optimistic.
        assert_eq!(
            project_input_bid_limit(Gold(9), Some(Gold(4)), true),
            Gold(4),
            "an optimistic reservation is capped at the observed input price"
        );
        assert_eq!(
            project_input_bid_limit(Gold(3), Some(Gold(4)), true),
            Gold(3),
            "the cap never raises a conservative reservation"
        );
        assert_eq!(
            project_input_bid_limit(Gold(9), None, true),
            Gold(9),
            "without an observed input price, the first discovery bid keeps its reservation"
        );
        assert_eq!(
            project_input_bid_limit(Gold(9), Some(Gold(4)), false),
            Gold(9),
            "with forecasts off, the legacy reservation-as-limit path is byte-identical"
        );
    }

    #[test]
    fn canonical_bytes_include_forecast_bias() {
        // S11: under entrepreneurial forecasts the per-colonist forecast bias steers every
        // appraisal, so two configs whose forecast-bias base differs — and thus whose drawn
        // per-colonist biases differ — digest apart.
        let base = SettlementConfig::frontier_coemergent_strong_entrepreneurial();
        let mut tilted = base.clone();
        tilted.forecast_bias_base_bps = 15_000;
        assert_ne!(
            Settlement::generate(7, &base).canonical_bytes(),
            Settlement::generate(7, &tilted).canonical_bytes(),
            "forecast_bias must be part of the entrepreneurial identity"
        );

        // With the flag OFF the forecast bias is never serialized, so the SAME base change
        // is invisible (byte-identical) — the additivity anchor.
        let off = SettlementConfig::frontier_coemergent_strong_originary();
        let mut off_tilted = off.clone();
        off_tilted.forecast_bias_base_bps = 15_000;
        assert_eq!(
            Settlement::generate(7, &off).canonical_bytes(),
            Settlement::generate(7, &off_tilted).canonical_bytes(),
            "forecast bias must be invisible to the digest with forecasts off"
        );
    }

    #[test]
    fn canonical_bytes_include_entrepreneurial_flag_and_belief_observed() {
        // S11: the entrepreneurial_forecasts flag is part of the chain config identity (it
        // flips every appraisal from realized price to a per-agent forecast), so the
        // flagship and the originary base it derives from digest apart at generation.
        let on = SettlementConfig::frontier_coemergent_strong_entrepreneurial();
        let off = SettlementConfig::frontier_coemergent_strong_originary();
        assert_ne!(
            Settlement::generate(7, &on).canonical_bytes(),
            Settlement::generate(7, &off).canonical_bytes(),
            "the entrepreneurial_forecasts flag must be part of the identity"
        );

        // The per-belief `observed` flag is in the digest under the flag (it steers the
        // belief-vs-realized grounding) and is NOT derivable from `last_seen`. Flip one
        // belief's `observed` and the digest must move under the flag…
        let a = Settlement::generate(7, &on);
        let mut b = Settlement::generate(7, &on);
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
        b.society.agents.as_mut_slice()[0].expect[usize::from(FOOD.0)].observed = true;
        assert_ne!(
            a.canonical_bytes(),
            b.canonical_bytes(),
            "the per-belief `observed` flag must be part of the entrepreneurial identity"
        );

        // …and must stay invisible with the flag off (byte-identical).
        let c = Settlement::generate(7, &off);
        let mut d = Settlement::generate(7, &off);
        d.society.agents.as_mut_slice()[0].expect[usize::from(FOOD.0)].observed = true;
        assert_eq!(
            c.canonical_bytes(),
            d.canonical_bytes(),
            "the belief `observed` flag must be invisible to the digest with forecasts off"
        );
    }

    #[test]
    fn per_agent_capital_builds_by_appraisal_with_a_visible_decliner() {
        // S10.1: on a simple designated-GOLD capital config with per_agent_capital ON, an
        // individual colonist builds via its OWN ordinal appraisal, and the per-tick
        // decision diagnostic shows at least one tick where an EARLIER-eligible colonist
        // declined on its own scale while a LATER one accepted — proving the builder is
        // chosen by its own appraisal, not slot-order-first. (The flagship emergence
        // variant is covered by the integration suite; this isolates the gated core.)
        let mut cfg = capital_test_config();
        cfg.chain.as_mut().expect("chain").per_agent_capital = true;
        // A leaner roster than the full capital config keeps this isolated core test fast
        // while still leaving bread demand the seeded chain cannot meet (so builds fire).
        cfg.consumers = 20;
        cfg.gatherers = 12;
        let mut s = Settlement::generate(1, &cfg);

        let mut saw_later_accept_after_earlier_own_decline = false;
        for _ in 0..500u64 {
            s.econ_tick();
            let decisions = s.last_capital_decisions();
            let earliest_own_decline = decisions
                .iter()
                .filter(|d| {
                    !d.accepted
                        && matches!(
                            d.reason,
                            CapitalDeclineReason::NoFutureProvision
                                | CapitalDeclineReason::PresentCostOutranks
                        )
                })
                .map(|d| d.slot)
                .min();
            if let Some(decline_slot) = earliest_own_decline {
                if decisions
                    .iter()
                    .any(|d| d.accepted && d.slot > decline_slot)
                {
                    saw_later_accept_after_earlier_own_decline = true;
                }
            }
        }

        assert!(
            s.tools_built() > 0,
            "an individual colonist must build via its own appraisal with per-agent on"
        );
        assert!(
            saw_later_accept_after_earlier_own_decline,
            "the diagnostic must show an earlier-eligible colonist declining on its own \
             scale while a later one accepts (per-agent, not slot-order-first)"
        );

        // Flag OFF (the S7 heuristic) records no per-agent decision diagnostic at all, and
        // the multi-horizon savings ladder never activates — byte-identical to S7.
        let mut off = capital_test_config();
        off.chain.as_mut().expect("chain").per_agent_capital = false;
        let mut t = Settlement::generate(1, &off);
        t.run(200);
        assert!(
            t.last_capital_decisions().is_empty(),
            "the per-agent diagnostic must be empty on the S7 heuristic path"
        );
    }

    #[test]
    fn per_agent_capital_ignores_stale_output_prices() {
        let mut cfg = capital_test_config();
        {
            let c = cfg.chain.as_mut().expect("chain");
            c.per_agent_capital = true;
            c.tool_build_labor = 1;
        }
        cfg.consumers = 20;
        cfg.gatherers = 12;
        let chain = cfg.chain.as_ref().expect("chain");
        let bread = chain.content.bread();
        let flour = chain.content.flour();
        let grain = chain.content.grain();
        let mill = chain.content.mill();
        let oven = chain.content.oven();
        let wood_qty = chain.tool_build_wood;
        let mut s = Settlement::generate(1, &cfg);

        for _ in 0..500u64 {
            s.econ_tick();
            if s.society.tick.0 > CAPITAL_BUILD_RECENCY + 2
                && s.realized_price(bread).is_some()
                && s.realized_price(flour).is_some()
                && s.realized_price(grain).is_some()
            {
                break;
            }
        }
        assert!(
            s.realized_price(bread).is_some()
                && s.realized_price(flour).is_some()
                && s.realized_price(grain).is_some(),
            "test setup must establish realized recipe prices"
        );

        let old_tick = s.society.tick.0.saturating_sub(CAPITAL_BUILD_RECENCY + 2);
        for trade in &mut s.society.trades {
            trade.tick = old_tick;
        }
        assert!(!s.good_traded_within(bread, CAPITAL_BUILD_RECENCY));
        assert!(!s.good_traded_within(flour, CAPITAL_BUILD_RECENCY));

        let mut eligible = 0u32;
        for &slot in &s.live_colonist_slots {
            let colonist = &mut s.colonists[slot];
            if colonist.latent.is_some()
                || !matches!(
                    colonist.vocation,
                    Vocation::Gatherer | Vocation::Consumer | Vocation::Unassigned
                )
            {
                continue;
            }
            colonist.need.hunger = 0;
            colonist.need.warmth = 0;
            colonist.need.rest = 0;
            let Some(agent) = s.society.agents.get_mut(colonist.id) else {
                continue;
            };
            if agent.stock.get(mill) != 0 || agent.stock.get(oven) != 0 {
                continue;
            }
            let held = agent.stock.get(WOOD);
            if held < wood_qty {
                agent.stock.add(WOOD, wood_qty - held);
            }
            eligible += 1;
        }
        assert!(eligible > 0, "test setup must leave eligible builders");

        let built_before = s.tools_built();
        s.econ_tick();
        let decisions = s.last_capital_decisions();
        assert!(
            !decisions.is_empty(),
            "test setup must exercise per-agent appraisals"
        );
        assert!(
            decisions
                .iter()
                .all(|d| !d.accepted && d.reason == CapitalDeclineReason::NoPrices),
            "stale output prices must not support per-agent builds: {decisions:?}"
        );
        assert_eq!(
            s.tools_built(),
            built_before,
            "no tool may be built from stale realized output prices"
        );
    }

    #[test]
    fn per_agent_capital_clears_decisions_on_completion_ticks() {
        let mut cfg = capital_test_config();
        cfg.chain.as_mut().expect("chain").per_agent_capital = true;
        cfg.consumers = 20;
        cfg.gatherers = 12;
        let mill = cfg.chain.as_ref().expect("chain").content.mill();
        let oven = cfg.chain.as_ref().expect("chain").content.oven();
        let mut s = Settlement::generate(1, &cfg);

        let mut saw_started_build = false;
        for _ in 0..700u64 {
            s.econ_tick();
            if s.active_capital_builds() > 0
                && s.last_capital_decisions().iter().any(|d| d.accepted)
            {
                saw_started_build = true;
                break;
            }
        }
        assert!(
            saw_started_build,
            "test setup must start an in-flight per-agent build"
        );

        for _ in 0..16u64 {
            let report = s.econ_tick();
            if report.produced_of(mill) + report.produced_of(oven) > 0 {
                assert!(
                    s.last_capital_decisions().is_empty(),
                    "a completion-only tick must not expose stale per-agent decisions"
                );
                return;
            }
        }
        panic!("test setup did not reach a capital completion tick");
    }

    #[test]
    #[should_panic(
        expected = "per-agent capital requires tool_build_labor below the deepest savings horizon"
    )]
    fn per_agent_capital_build_labor_must_fit_the_savings_horizon() {
        let mut cfg = capital_test_config();
        {
            let c = cfg.chain.as_mut().expect("chain");
            c.per_agent_capital = true;
            c.tool_build_labor = u32::try_from(max_savings_ladder_horizon()).unwrap_or(u32::MAX);
        }
        let _ = Settlement::generate(1, &cfg);
    }

    #[test]
    fn capital_capacity_counts_only_live_tool_holders() {
        let cfg = capital_test_config();
        let mill = cfg.chain.as_ref().expect("chain").content.mill();
        let mut s = Settlement::generate(1, &cfg);
        let holders_before = s.live_colonist_holder_count(mill);
        let whole_before = s.whole_system_total(mill);

        // Commons tools are conserved (whole-system) but inaccessible — never usable
        // capital capacity.
        s.commons_stock.insert(mill, 10);
        assert_eq!(
            s.live_colonist_holder_count(mill),
            holders_before,
            "commons tools are conserved but not usable capital capacity"
        );
        assert_eq!(
            s.whole_system_total(mill),
            whole_before + 10,
            "whole-system conservation still includes commons tools"
        );

        // Concentration cannot overstate capacity: stacking a SECOND mill on a colonist
        // that already holds one (an inherited/transferred estate) adds a conserved unit
        // but no capacity — the holder still runs one vocation, one throughput, so the
        // bottleneck/idle-tool guards must count holders, not raw units.
        let stacked = s
            .live_colonist_slots
            .iter()
            .map(|&slot| s.colonists[slot].id)
            .find(|&id| {
                s.society
                    .agents
                    .get(id)
                    .is_some_and(|a| a.stock.get(mill) > 0)
            })
            .expect("the seeded latent pool holds at least one mill");
        s.society
            .agents
            .get_mut(stacked)
            .expect("the stacked holder resolves")
            .stock
            .add(mill, 1);
        assert_eq!(
            s.live_colonist_holder_count(mill),
            holders_before,
            "a second tool stacked on one holder must not raise usable capacity"
        );
        assert_eq!(
            s.whole_system_total(mill),
            whole_before + 11,
            "the stacked unit is still conserved in the whole-system total"
        );
    }

    #[test]
    fn one_labor_capital_build_completes_on_start_and_records_labor() {
        let mut cfg = capital_test_config();
        cfg.chain.as_mut().expect("chain").tool_build_labor = 1;
        let mill = cfg.chain.as_ref().expect("chain").content.mill();
        let oven = cfg.chain.as_ref().expect("chain").content.oven();
        let mut s = Settlement::generate(1, &cfg);

        let mut builder = None;
        for tick in 0..900u64 {
            let report = s.econ_tick();
            let tool_produced = report.produced_of(mill) + report.produced_of(oven);
            if report.consumed_as_input_of(WOOD) > 0 {
                assert!(
                    tool_produced > 0,
                    "a one-labor capital build must complete on its start tick, failed at {tick}"
                );
                assert_eq!(
                    s.active_capital_builds(),
                    0,
                    "a completion tick must not immediately start the next capital build"
                );
                builder = (0..s.population())
                    .find(|&i| s.acquired_tool_of(i))
                    .and_then(|i| s.colonist_id(i).map(|id| (i, id)));
                break;
            }
        }
        let (builder_index, builder_id) = builder.expect("a one-labor build completed");
        assert!(
            s.society
                .labor_used_last_tick()
                .iter()
                .any(|&(id, labor)| id == builder_id && labor > 0),
            "capital build labor must be recorded in the society labor log"
        );

        let builder_slot = s.slot_for_id(builder_id).expect("builder slot");
        s.colonists[builder_slot].need.rest = 0;
        s.econ_tick();
        assert!(
            s.need_of(builder_index).expect("builder need").rest > 0,
            "recorded capital labor must feed the next needs update"
        );
    }

    #[test]
    #[should_panic(expected = "producible capital (S7.2) requires tool-acquisition eligibility")]
    fn producible_capital_requires_eligibility() {
        let mut cfg = SettlementConfig::frontier_endogenous();
        {
            let c = cfg.chain.as_mut().expect("chain");
            c.tool_acquisition_eligibility = false;
            c.producible_capital = true;
        }
        let _ = Settlement::generate(7, &cfg);
    }

    #[test]
    fn capital_is_built_under_demand_and_conserves() {
        // S7.2 (the headline): under the scaling economy's unmet bread demand a builder
        // commits its OWN WOOD, completes a BuildMill/BuildOven, the whole-system tool
        // count rises, produced_of(tool) > 0, WOOD is booked to consumed_as_input, and
        // conservation holds EVERY tick across the build. The builder then adopts and
        // becomes a producer (a formerly-non-latent colonist with a produced tool).
        let cfg = capital_test_config();
        let mill = cfg.chain.as_ref().expect("chain").content.mill();
        let oven = cfg.chain.as_ref().expect("chain").content.oven();
        let mut s = Settlement::generate(1, &cfg);
        let tools_before = s.whole_system_total(mill) + s.whole_system_total(oven);

        let mut wood_consumed_as_input = 0u64;
        let mut tool_produced = 0u64;
        // A formerly-non-latent colonist that built a tool and adopted the trade — sampled
        // across the run, since adoption fluctuates tick to tick in the emergent chain.
        let mut built_adopter = false;
        for tick in 0..1200u64 {
            let report = s.econ_tick();
            assert!(
                report.conserves(),
                "whole-system conservation must hold every tick across a tool build, broke at {tick}"
            );
            // WOOD is consumed_as_input ONLY by a capital build (no recipe consumes it).
            wood_consumed_as_input += report.consumed_as_input_of(WOOD);
            tool_produced += report.produced_of(mill) + report.produced_of(oven);
            if !built_adopter {
                built_adopter = (0..s.population()).any(|i| {
                    s.acquired_tool_of(i)
                        && matches!(
                            s.vocation_of(i),
                            Some(Vocation::Miller) | Some(Vocation::Baker)
                        )
                });
            }
        }

        assert!(
            s.tools_built() > 0,
            "a builder must complete at least one tool under unmet demand, got {}",
            s.tools_built()
        );
        assert!(
            tool_produced > 0,
            "produced_of(tool) must be > 0, got {tool_produced}"
        );
        assert!(
            wood_consumed_as_input > 0,
            "the build must book its WOOD to consumed_as_input, got {wood_consumed_as_input}"
        );
        let tools_after = s.whole_system_total(mill) + s.whole_system_total(oven);
        assert!(
            tools_after > tools_before,
            "whole-system tool count must rise ({tools_before} -> {tools_after})"
        );
        assert!(
            built_adopter,
            "a formerly-non-latent builder must have adopted a producer role with its built tool"
        );
    }

    #[test]
    fn no_capital_built_when_the_appraisal_declines() {
        // S7.2 overinvestment guard: a payback horizon of 0 puts the amortized margin
        // (margin × 0 = 0) below any positive build cost, so the appraisal always
        // declines — no tool is ever built, however strong the demand. Proves building
        // is the appraisal's decision, not blind: the per-run margin must clear the
        // payback bar (the demand-driven version is the acceptance suite's test 7).
        let mut cfg = capital_test_config();
        cfg.chain.as_mut().expect("chain").capital_payback_cycles = 0;
        let mut s = Settlement::generate(1, &cfg);
        for _ in 0..1200u64 {
            s.econ_tick();
        }
        assert_eq!(
            s.tools_built(),
            0,
            "no tool may be built when the amortized margin is below the payback bar"
        );
        assert_eq!(
            s.active_capital_builds(),
            0,
            "no build may be left in flight"
        );
    }

    #[test]
    fn canonical_bytes_include_producer_subsistence() {
        // `producer_subsistence` mints a local staple/WOOD floor for the chain's
        // producers every tick — it steers future behaviour, yet it is a pure
        // runtime knob that never shows up in the generated holdings. Two chains
        // differing only in it must therefore digest differently, or the
        // determinism tripwire would call two non-equivalent configs equal.
        let mut base = SettlementConfig::emergent_chain();
        base.chain.as_mut().expect("chain").producer_subsistence = 0;
        let mut fed = SettlementConfig::emergent_chain();
        fed.chain.as_mut().expect("chain").producer_subsistence = 4;
        let base = Settlement::generate(7, &base);
        let fed = Settlement::generate(7, &fed);
        assert_ne!(
            base.canonical_bytes(),
            fed.canonical_bytes(),
            "the producer-subsistence floor must be part of the chain config identity"
        );
    }

    #[test]
    fn canonical_bytes_include_project_input_bids() {
        // `project_input_bids` switches input acquisition from the generic spot bid
        // to the project-aware imputed market bid — a runtime knob that steers
        // future ticks without changing generation, so it too must split the digest.
        let mut base = SettlementConfig::emergent_chain();
        base.chain.as_mut().expect("chain").project_input_bids = false;
        let mut bidding = SettlementConfig::emergent_chain();
        bidding.chain.as_mut().expect("chain").project_input_bids = true;
        let base = Settlement::generate(7, &base);
        let bidding = Settlement::generate(7, &bidding);
        assert_ne!(
            base.canonical_bytes(),
            bidding.canonical_bytes(),
            "the project-aware input bid flag must be part of the chain config identity"
        );
    }

    #[test]
    fn canonical_bytes_include_recurring_motive() {
        // `recurring_motive` keeps an owner-operator adopted while the recipe stays
        // profitable — a runtime knob that steers future role-choice ticks without
        // changing generation, so two chains differing only in it must digest apart.
        let mut base = SettlementConfig::emergent_chain();
        base.chain.as_mut().expect("chain").recurring_motive = false;
        let mut motivated = SettlementConfig::emergent_chain();
        motivated.chain.as_mut().expect("chain").recurring_motive = true;
        let base = Settlement::generate(7, &base);
        let motivated = Settlement::generate(7, &motivated);
        assert_ne!(
            base.canonical_bytes(),
            motivated.canonical_bytes(),
            "the recurring-motive flag must be part of the chain config identity"
        );
    }

    #[test]
    fn canonical_bytes_include_subsistence_on_grain() {
        // `subsistence_on_grain` is realised as `known.subsistence`, a directly
        // edible staple fallback that steers the future needs/scale phase yet leaves
        // generation untouched, so two chains differing only in it must digest apart.
        let mut base = SettlementConfig::emergent_chain();
        base.chain.as_mut().expect("chain").subsistence_on_grain = false;
        let mut edible = SettlementConfig::emergent_chain();
        edible.chain.as_mut().expect("chain").subsistence_on_grain = true;
        let base = Settlement::generate(7, &base);
        let edible = Settlement::generate(7, &edible);
        assert_ne!(
            base.canonical_bytes(),
            edible.canonical_bytes(),
            "the edible-grain subsistence fallback must be part of the chain config identity"
        );
    }

    #[test]
    fn canonical_bytes_include_own_labor_subsistence() {
        // S12: the own-labor gate retires the food mints, wires FORAGE as the
        // subsistence good, and steers the forage phase + yield — so the provisioned
        // flagship (own-labor ON) must digest apart from the entrepreneurial base
        // (own-labor OFF, no forage good).
        let base = Settlement::generate(
            7,
            &SettlementConfig::frontier_coemergent_strong_entrepreneurial(),
        );
        let provisioned = Settlement::generate(
            7,
            &SettlementConfig::frontier_coemergent_strong_provisioned(),
        );
        assert_ne!(
            base.canonical_bytes(),
            provisioned.canonical_bytes(),
            "own-labor subsistence (the FORAGE good + retired mints) must be part of the identity"
        );

        // ON: a different forage yield or hysteresis threshold steers the forage phase,
        // so each must split the digest.
        let mut y_a = SettlementConfig::frontier_coemergent_strong_provisioned();
        y_a.chain.as_mut().expect("chain").forage_yield = 2;
        let mut y_b = SettlementConfig::frontier_coemergent_strong_provisioned();
        y_b.chain.as_mut().expect("chain").forage_yield = 5;
        assert_ne!(
            Settlement::generate(7, &y_a).canonical_bytes(),
            Settlement::generate(7, &y_b).canonical_bytes(),
            "with own-labor on, the forage yield must be part of the digest"
        );
        let mut h = SettlementConfig::frontier_coemergent_strong_provisioned();
        h.chain.as_mut().expect("chain").forage_hunger_in = 9;
        assert_ne!(
            provisioned.canonical_bytes(),
            Settlement::generate(7, &h).canonical_bytes(),
            "with own-labor on, the forage entry threshold must be part of the digest"
        );

        // OFF: the (unused) forage knobs must NOT split a flag-off chain's digest, or
        // the tripwire would call two behaviour-identical configs unequal.
        let off_bytes = base.canonical_bytes();
        let mut off_knobs = SettlementConfig::frontier_coemergent_strong_entrepreneurial();
        {
            let c = off_knobs.chain.as_mut().expect("chain");
            c.forage_yield = 9;
            c.forage_hunger_in = 11;
            c.forage_hunger_out = 1;
        }
        assert_eq!(
            off_bytes,
            Settlement::generate(7, &off_knobs).canonical_bytes(),
            "with own-labor off, the unused forage knobs must not steer the digest"
        );
    }

    #[test]
    fn canonical_bytes_include_foraging() {
        // S12: the per-colonist `foraging` flag steers the next fast loop (forage the
        // FORAGE node vs harvest WOOD), so two own-labor states differing only in it must
        // digest apart — and a flag-off chain must NOT serialize it (byte-identical).
        let mut on = Settlement::generate(
            7,
            &SettlementConfig::frontier_coemergent_strong_provisioned(),
        );
        let before = on.canonical_bytes();
        on.colonists[0].foraging = !on.colonists[0].foraging;
        assert_ne!(
            before,
            on.canonical_bytes(),
            "with own-labor on, a colonist's foraging flag must be part of the digest"
        );

        let mut off = Settlement::generate(
            7,
            &SettlementConfig::frontier_coemergent_strong_entrepreneurial(),
        );
        let off_before = off.canonical_bytes();
        off.colonists[0].foraging = !off.colonists[0].foraging;
        assert_eq!(
            off_before,
            off.canonical_bytes(),
            "with own-labor off, the unused foraging flag must not steer the digest"
        );
    }

    #[test]
    fn own_labor_credit_requires_completed_forage_task() {
        // A stale `foraging` decision is not enough to create FORAGE. The agent must
        // actually complete `Task::GoForage` in the preceding fast loop; a hungry
        // colonist busy walking somewhere else keeps the flag for the next assignment
        // but produces nothing this tick.
        let cfg = SettlementConfig::frontier_coemergent_strong_provisioned();
        let forage = cfg
            .chain
            .as_ref()
            .expect("chain")
            .content
            .forage()
            .expect("forage good");
        let mut s = Settlement::generate(7, &cfg);
        let slot = s
            .live_colonist_slots
            .iter()
            .copied()
            .find(|&slot| s.colonists[slot].household.is_none())
            .expect("a spatial non-lineage colonist");
        let id = s.colonists[slot].id;
        s.colonists[slot].need.hunger = 12;
        s.colonists[slot].foraging = true;
        assert!(s.world.assign_task(id, Task::GoTo(Pos::new(63, 0))));
        assert_eq!(s.world.agent_status(id), Some(AgentStatus::Moving));

        let report = s.econ_tick();
        assert!(report.conserves());
        assert_eq!(
            report.produced_of(forage),
            0,
            "FORAGE credit must be gated on a completed GoForage task, not just the flag"
        );
        assert!(
            s.colonists[slot].foraging,
            "the hungry colonist should still be marked to forage once its current task settles"
        );
    }

    #[test]
    fn canonical_bytes_include_reentry_flags() {
        // S6: `productive_reentry` gates the re-entry phase that flips spatial
        // colonists' vocations/nodes. It steers future ticks while leaving generation
        // untouched only when raw grain is edible, so two active chains differing only
        // in it must digest apart. The two hysteresis thresholds steer behaviour only
        // while the phase runs, so they join the digest when (and only when) re-entry
        // is active: two active chains differing in a threshold split, but inactive
        // chains differing only in a threshold stay byte-identical.
        let mut off = SettlementConfig::frontier_endogenous();
        off.chain.as_mut().expect("chain").productive_reentry = false;
        let mut on = SettlementConfig::frontier_endogenous();
        on.chain.as_mut().expect("chain").productive_reentry = true;
        let off_bytes = Settlement::generate(7, &off).canonical_bytes();
        assert_ne!(
            off_bytes,
            Settlement::generate(7, &on).canonical_bytes(),
            "the re-entry phase gate must be part of the chain config identity"
        );

        // Phase ON: a different entry OR exit threshold must split the digest.
        let mut on_hi = on.clone();
        on_hi.chain.as_mut().expect("chain").reentry_hunger_in = 6;
        assert_ne!(
            Settlement::generate(7, &on).canonical_bytes(),
            Settlement::generate(7, &on_hi).canonical_bytes(),
            "with re-entry on, the entry threshold must be part of the digest"
        );
        let mut on_lo = on.clone();
        on_lo.chain.as_mut().expect("chain").reentry_hunger_out = 2;
        assert_ne!(
            Settlement::generate(7, &on).canonical_bytes(),
            Settlement::generate(7, &on_lo).canonical_bytes(),
            "with re-entry on, the exit threshold must be part of the digest"
        );

        // Phase OFF: the (unused) thresholds must NOT split the digest, or the
        // tripwire would call two behaviour-identical configs unequal.
        let mut off_thresholds = off.clone();
        {
            let c = off_thresholds.chain.as_mut().expect("chain");
            c.reentry_hunger_in = 6;
            c.reentry_hunger_out = 2;
        }
        assert_eq!(
            off_bytes,
            Settlement::generate(7, &off_thresholds).canonical_bytes(),
            "with re-entry off, the unused thresholds must not steer the digest"
        );

        // Phase ON but no edible-grain fallback: the runtime phase exits before
        // mutating, so the flag and thresholds must serialize as a no-op.
        let inert = SettlementConfig::grain_flour_bread_chain();
        assert!(
            !inert.chain.as_ref().expect("chain").subsistence_on_grain,
            "the seeded chain does not make raw grain directly edible"
        );
        let mut inert_on = inert.clone();
        {
            let c = inert_on.chain.as_mut().expect("chain");
            c.productive_reentry = true;
            c.reentry_hunger_in = 6;
            c.reentry_hunger_out = 2;
        }
        assert_eq!(
            Settlement::generate(7, &inert).canonical_bytes(),
            Settlement::generate(7, &inert_on).canonical_bytes(),
            "without edible grain, re-entry is behavior-identical and must not split the digest"
        );
    }

    #[test]
    fn canonical_bytes_include_reentry_home() {
        // S6: with re-entry ON, a colonist's HOME vocation+node decide whether and where
        // a displaced re-entrant reverts once fed (`run_productive_reentry`). Two states
        // with identical CURRENT vocation/node but different homes diverge on the revert
        // path, so the home is part of the future-behaviour identity — `canonical_bytes`
        // must read it. With re-entry OFF the home is never consulted, so it must NOT
        // steer the digest (the `endogenous` byte-identity tripwire).
        let mut on = SettlementConfig::frontier_endogenous();
        on.chain.as_mut().expect("chain").productive_reentry = true;
        let on_bytes = Settlement::generate(7, &on).canonical_bytes();

        // Re-entry ON: perturbing a colonist's home NODE must split the digest.
        let mut on_node = Settlement::generate(7, &on);
        let node_slot = on_node
            .colonists
            .iter()
            .position(|c| c.home_node.is_some())
            .expect("a spatial gatherer with a home node");
        on_node.colonists[node_slot].home_node = None;
        assert_ne!(
            on_bytes,
            on_node.canonical_bytes(),
            "with re-entry on, the home node must be part of the digest"
        );

        // Re-entry ON: perturbing a colonist's home VOCATION must split the digest.
        let mut on_voc = Settlement::generate(7, &on);
        let voc_slot = on_voc
            .colonists
            .iter()
            .position(|c| c.home_vocation == Vocation::Consumer)
            .expect("a non-lineage consumer with a Consumer home");
        on_voc.colonists[voc_slot].home_vocation = Vocation::Gatherer;
        assert_ne!(
            on_bytes,
            on_voc.canonical_bytes(),
            "with re-entry on, the home vocation must be part of the digest"
        );

        // Re-entry OFF: the same home perturbation must NOT split the digest, or the
        // pre-S6 per-colonist layout (and the `endogenous` byte-identity) would break.
        let off = SettlementConfig::frontier_endogenous();
        let off_bytes = Settlement::generate(7, &off).canonical_bytes();
        let mut off_node = Settlement::generate(7, &off);
        let off_slot = off_node
            .colonists
            .iter()
            .position(|c| c.home_node.is_some())
            .expect("a spatial gatherer with a home node");
        off_node.colonists[off_slot].home_node = None;
        assert_eq!(
            off_bytes,
            off_node.canonical_bytes(),
            "with re-entry off, the home must not steer the digest"
        );

        // Re-entry ON but raw grain not edible: the phase is inert, so the home also
        // must not steer the digest.
        let mut inert = SettlementConfig::grain_flour_bread_chain();
        inert.chain.as_mut().expect("chain").productive_reentry = true;
        let inert_bytes = Settlement::generate(7, &inert).canonical_bytes();
        let mut inert_node = Settlement::generate(7, &inert);
        let inert_slot = inert_node
            .colonists
            .iter()
            .position(|c| c.home_node.is_some())
            .expect("a spatial gatherer with a home node");
        inert_node.colonists[inert_slot].home_node = None;
        assert_eq!(
            inert_bytes,
            inert_node.canonical_bytes(),
            "without edible grain, re-entry home state must not steer the digest"
        );
    }

    #[test]
    #[should_panic(
        expected = "re-entry hysteresis requires reentry_hunger_out < reentry_hunger_in"
    )]
    fn active_reentry_rejects_invalid_hysteresis() {
        let mut cfg = SettlementConfig::frontier_endogenous();
        {
            let c = cfg.chain.as_mut().expect("chain");
            c.productive_reentry = true;
            c.reentry_hunger_in = 4;
            c.reentry_hunger_out = 4;
        }
        let _ = Settlement::generate(7, &cfg);
    }

    #[test]
    #[should_panic(
        expected = "own-labor subsistence hysteresis requires forage_hunger_out < forage_hunger_in"
    )]
    fn active_own_labor_subsistence_rejects_invalid_hysteresis() {
        let mut cfg = SettlementConfig::frontier_coemergent_strong_provisioned();
        {
            let c = cfg.chain.as_mut().expect("chain");
            c.forage_hunger_in = 4;
            c.forage_hunger_out = 4;
        }
        let _ = Settlement::generate(7, &cfg);
    }

    #[test]
    fn canonical_bytes_include_phase_gating_flags() {
        // capital_advance / subsistence_advance / input_advance / perishable_decay_bps
        // each gate a future settlement phase that runs for any chain, so a config
        // differing only in one steers later ticks while generating identically — the
        // determinism digest must split them or it would call two non-equivalent
        // configs equal. Flip each in isolation from a common base.
        let base = SettlementConfig::emergent_chain();
        let base_bytes = Settlement::generate(7, &base).canonical_bytes();
        let flip = |mutate: &dyn Fn(&mut ChainConfig)| {
            let mut cfg = SettlementConfig::emergent_chain();
            mutate(cfg.chain.as_mut().expect("chain"));
            Settlement::generate(7, &cfg).canonical_bytes()
        };
        assert_ne!(
            base_bytes,
            flip(&|c| c.capital_advance = !c.capital_advance),
            "the capital-advance flag must be part of the chain config identity"
        );
        assert_ne!(
            base_bytes,
            flip(&|c| c.subsistence_advance = !c.subsistence_advance),
            "the in-kind subsistence-advance flag must be part of the chain config identity"
        );
        assert_ne!(
            base_bytes,
            flip(&|c| c.input_advance = !c.input_advance),
            "the in-kind input-advance flag must be part of the chain config identity"
        );
        assert_ne!(
            base_bytes,
            flip(&|c| c.perishable_decay_bps = c.perishable_decay_bps.wrapping_add(50)),
            "the spoilage decay rate must be part of the chain config identity"
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

    /// A barter config with the heterogeneous SALT direct use + the indirect-breadth
    /// gate armed (the strong-bar shape), derived inline from `frontier_coemergent`
    /// so the S9.2 digest regressions do not depend on the S9.3 builder.
    #[cfg(test)]
    fn strong_bar_barter_config() -> SettlementConfig {
        let mut cfg = SettlementConfig::frontier_coemergent();
        let barter = cfg.barter.as_mut().expect("barter overlay");
        barter.medium_want_qty = 0;
        barter.salt_direct_use_qty = 1;
        barter.salt_direct_use_period = 8;
        barter.menger.min_indirect_acceptances = 12;
        barter.menger.min_indirect_acceptor_agents = 6;
        barter.menger.min_indirect_target_goods = 1;
        cfg
    }

    #[test]
    fn canonical_bytes_include_indirect_breadth_gate() {
        // S9: each strong-bar gate knob steers the future promotion decision, so all
        // four ride in the determinism identity before the first tick.
        let base = SettlementConfig::frontier_coemergent();

        let knobs: [fn(&mut MengerianConfig); 4] = [
            |m| m.min_indirect_acceptances += 1,
            |m| m.min_indirect_acceptor_agents += 1,
            |m| m.min_indirect_target_goods += 1,
            |m| m.allow_indirect_acceptance = false,
        ];
        let base_bytes = Settlement::generate(7, &base).canonical_bytes();
        for knob in knobs {
            let mut cfg = SettlementConfig::frontier_coemergent();
            knob(&mut cfg.barter.as_mut().expect("barter overlay").menger);
            assert_ne!(
                base_bytes,
                Settlement::generate(7, &cfg).canonical_bytes(),
                "an indirect-breadth gate knob must be part of the Mengerian config identity"
            );
        }
    }

    #[test]
    fn canonical_bytes_include_indirect_acceptance_runtime() {
        // S9: a strong-bar run accumulates per-candidate INDIRECT breadth (the
        // distinct indirect acceptors/targets behind the gate) that steers the future
        // promotion tick. Reconstruct the runtime bytes from econ's accessors and
        // assert they appear verbatim in the digest input.
        let mut s = Settlement::generate(1, &strong_bar_barter_config());
        // Advance into barter far enough that indirect acceptance has accrued but
        // stop before promotion so the tracker is still live.
        for _ in 0..120 {
            if !s.in_barter_phase() {
                break;
            }
            s.econ_tick();
        }
        assert!(
            s.in_barter_phase(),
            "the run must still be bartering so the tracker is live"
        );
        let emergence = s
            .society
            .emergence()
            .expect("a strong-bar run uses econ's emergence");
        let salt_indirect = emergence
            .tracker()
            .candidate_saleability()
            .find(|c| c.good == SALT)
            .map(|c| c.indirect_acceptances)
            .unwrap_or(0);
        assert!(
            salt_indirect > 0,
            "the test is vacuous — no indirect acceptance was observed"
        );

        let mut expected = Vec::new();
        push_emergence_runtime_bytes(&mut expected, emergence);
        let bytes = s.canonical_bytes();
        assert!(
            bytes
                .windows(expected.len())
                .any(|window| window == expected.as_slice()),
            "the accumulated indirect-acceptance runtime is missing from the canonical bytes"
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
