//! M1 multi-agent market society.

use std::collections::{BTreeMap, BTreeSet};

use crate::agent::{Agent, AgentId, Role, TickProvisions, WantKind};
use crate::agio::AgioSchedule;
use crate::arena::AgentArena;
use crate::bank::{Bank, BankPolicy};
use crate::barter::{BarterBook, BarterOffer, BarterReason, BarterTrade, SaleabilityContext};
use crate::bundle::{
    appraise_project_bundle_for_money, ProjectBundleCandidate, ProjectBundleEndowment,
};
use crate::cantillon::{CantillonReceipt, CantillonRoute, CantillonRouter};
use crate::capital::{
    abandon_project, aggregate_input_goods, borrow_to_build_line, builtin_project_lines,
    committed_input_goods, credit_boom_long_line, find_line, mature_project, record_project_sale,
    start_project, M2Project, M2ProjectId, M2ProjectState, ProjectFundingPlan, ProjectLine,
    ProjectLineId, ProjectOutputLot,
};
use crate::command::{CommandResult, RejectReason};
use crate::expect::PriceBelief;

/// Which path is applying an [`EventKind`].
///
/// The mutation logic is shared (game-spec §7); the *only* behavioural
/// difference is a handful of existence preconditions the lab tolerated
/// silently on the authored-event path but a player command must reject. `Event`
/// keeps the lab's unconditional mutation (byte-for-byte the goldens); `Command`
/// rejects those cases loudly and side-effect-free. See
/// [`Society::apply_event_kind`] and `docs/engine-divergence.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ApplyMode {
    /// Authored scenario event: lab-faithful mutation, silent tolerance.
    Event,
    /// Player command: reject missing targets rather than silently mutate.
    Command,
}
use crate::factor::{
    FactorSide, LaborBook, LaborMarketView, LaborOrder, LaborReservations, LaborTrade,
};
use crate::good::{Gold, GoodId, Stock, FOOD, GOLD, NET, WOOD};
use crate::issuer::{Issuer, IssuerPolicy};
use crate::ledger::{BankId, MoneySystem};
use crate::market::{ExecutedTrade, Order, OrderBook, OrderSide, Reservations, Trade};
use crate::marketability::{MarketabilityAcceptance, MarketabilityConfig};
use crate::menger::{MengerianEmergence, SaleabilitySnapshot};
use crate::metrics::{
    cumulative_project_profit, proxy_trades_from_schedules, structure_length_ticks_x100,
    weighted_loan_bps, MetricObservationAccumulator, MetricObservationInput,
};
use crate::money::{
    BankRepaymentTender, IssuerRepaymentTender, LaborWageTender, MarketMoneyConfig,
    MarketMoneyState, MoneyRegime, PublicDebtTender, PublicSpotTender, Regime, TaxReceivability,
};
use crate::project::{Recipe, RecipeId, Tick};
use crate::purpose::{CreditLender, CreditSource, DebtPurpose, LoanPurpose, ProjectPlanId};
use crate::record::{
    BankAuditRecord, BankRepaymentAuditRecord, DebtPaymentAuditRecord, IssuerRepaymentAuditRecord,
    M2Record, M3Record, MarketRecord, MetricObservation, MoneyAuditRecord, PaymentAuditRecord,
    PaymentKind, RedemptionAuditRecord, RedemptionOutcome, TaxAuditRecord, V2Phase, V2Record,
    WagePaymentAuditRecord,
};
use crate::registry::GoodRegistry;
use crate::rng::Rng;
use crate::scenario::{
    Event, EventKind, MarketScenario, RedemptionRoute, ScenarioKind, ScenarioName,
    ScenarioProjectLines,
};
use crate::shadow::ShadowSeries;
use crate::sim::{
    direct_recipe_candidates, direct_recipe_candidates_excluding_good,
    direct_recipe_candidates_for_money, execute_direct_recipe_for_agent,
    execute_direct_recipe_for_agent_for_money, DirectRecipeAction,
};
use crate::timemarket::{
    settle_due_debts_excluding_agents, settle_due_debts_m3_excluding_agents, DebtContract, DebtId,
    DebtSettlementM3Context, DebtState, LoanM3Context, LoanOrder, LoanOrderBook, LoanReservations,
    LoanSide, LoanTrade,
};

const ORDER_TTL: u64 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LiveQuote {
    agent: AgentId,
    side: OrderSide,
    good: GoodId,
    reservation: Gold,
    limit: Gold,
    qty: u32,
    seq: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FillKey {
    agent: AgentId,
    side: OrderSide,
    good: GoodId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QuotePlan {
    agent_index: usize,
    side: OrderSide,
    good: GoodId,
    reservation: Gold,
    limit: Gold,
    existing: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProjectDebtPaymentSnapshot {
    debt_payment: usize,
    bank_repayment: usize,
    paid_before: Vec<(DebtId, Gold)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProjectPlanDebtPaymentEffect {
    plan: ProjectPlanId,
    specie_paid: Gold,
    clear_reserved_gold: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ReservedAssets {
    gold: Gold,
    stock: Vec<(GoodId, u32)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct DebtCounts {
    open: u32,
    settled: u32,
    defaulted: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ProjectCounts {
    active: u32,
    waiting: u32,
    mature: u32,
    sold: u32,
    abandoned: u32,
    labor_advanced: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProjectInputDebit {
    Stock(GoodId, u32),
    Money(Gold),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct AgentDebtView {
    receivables: Vec<DebtContract>,
    payables: Vec<DebtContract>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocietyStepError {
    EmergentMoneyDeferred,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocietyBuildError {
    V2RequiresEmergentMoney,
    V2UnsupportedEvent,
    V2InitialMoneyBalance,
    InitialDesignatedMoneyStockOverflow,
    M3InitialBankDepositsInvalid,
}

impl std::fmt::Display for SocietyBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::V2RequiresEmergentMoney => "V2 market scenarios require emergent money config",
            Self::V2UnsupportedEvent => {
                "V2 scenarios must not carry bank, issuer, or regime events"
            }
            Self::V2InitialMoneyBalance => {
                "V2 barter scenarios must start without legacy money balances"
            }
            Self::InitialDesignatedMoneyStockOverflow => {
                "initial designated money stock must fit in spendable balances"
            }
            Self::M3InitialBankDepositsInvalid => {
                "M3 initial bank deposits must fit in spendable balances"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for SocietyBuildError {}

/// The accounted result of applying a direct [`Recipe`] to one live agent.
///
/// This is an additive driver seam: callers outside `econ` can reuse the
/// existing direct-recipe executor and record the exact conversion without
/// changing market clearing or planner behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirectRecipeExecution {
    pub labor: u32,
    pub input: Option<(GoodId, u32)>,
    pub output: (GoodId, u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V2PromotionFailure {
    pub tick: u64,
    pub money_good: GoodId,
    pub reason: V2PromotionFailureReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V2PromotionFailureReason {
    NonZeroMoneyBalance,
    BalanceOverflow,
    UnsupportedMoneyGood,
}

/// The settled estate of a removed colonist (G4a real death): the econ gold and
/// stock extracted from its arena slot at death, returned by
/// [`Society::remove_agent`] for the caller to route to the settlement commons or
/// to heirs — a conserved transfer, nothing created or destroyed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Estate {
    /// The dead colonist's money at death: the closed-GOLD M1 `Agent.gold` balance,
    /// or — in an M3 ledger society — the public **specie** drained out of the
    /// [`MoneySystem`] into the estate (G8a), so routing it to the commons or an heir
    /// conserves the ledger total.
    pub gold: Gold,
    /// The dead colonist's econ stock at death, every physical good it held.
    pub stock: Stock,
}

pub struct Society {
    pub tick: Tick,
    pub agents: AgentArena,
    initial_agents: Vec<Agent>,
    /// The good catalog. Constructed `lab_default()` so `GoodId` values, names,
    /// and `Stock`/belief slot counts are bit-for-bit the lab's (G0b).
    registry: GoodRegistry,
    pub recipes: Vec<Recipe>,
    pub books: Vec<OrderBook>,
    pub records: Vec<MarketRecord>,
    pub m2_records: Vec<M2Record>,
    pub m3_records: Vec<M3Record>,
    m3_shadow_attached: bool,
    pub metric_observations: Vec<MetricObservation>,
    pub money_audit: Vec<MoneyAuditRecord>,
    pub bank_audit: Vec<BankAuditRecord>,
    pub redemption_audit: Vec<RedemptionAuditRecord>,
    pub payment_audit: Vec<PaymentAuditRecord>,
    pub wage_payment_audit: Vec<WagePaymentAuditRecord>,
    pub debt_payment_audit: Vec<DebtPaymentAuditRecord>,
    pub bank_repayment_audit: Vec<BankRepaymentAuditRecord>,
    pub issuer_repayment_audit: Vec<IssuerRepaymentAuditRecord>,
    pub tax_audit: Vec<TaxAuditRecord>,
    pub barter_trades: Vec<BarterTrade>,
    pub saleability_snapshots: Vec<SaleabilitySnapshot>,
    pub v2_records: Vec<V2Record>,
    pub v2_promotion_failures: Vec<V2PromotionFailure>,
    metric_observation_accumulator: Option<MetricObservationAccumulator>,
    money_audit_enabled: bool,
    bank_audit_enabled: bool,
    metric_consumed_goods: Vec<(AgentId, GoodId, u32)>,
    pub trades: Vec<Trade>,
    realized_prices: BTreeMap<GoodId, Gold>,
    pub loan_trades: Vec<LoanTrade>,
    pub labor_trades: Vec<LaborTrade>,
    pub debts: Vec<DebtContract>,
    pub banks: Vec<Bank>,
    pub issuers: Vec<Issuer>,
    pub cantillon_receipts: Vec<CantillonReceipt>,
    pub m2_projects: Vec<M2Project>,
    pub project_funding_plans: Vec<ProjectFundingPlan>,
    pub project_output_lots: Vec<ProjectOutputLot>,
    #[allow(dead_code)]
    rng: Rng,
    seq: u64,
    next_debt_id: u64,
    next_m2_project_id: u64,
    next_project_plan_id: u64,
    events: Vec<Event>,
    pub reservations: Reservations,
    pub loan_reservations: LoanReservations,
    pub labor_reservations: LaborReservations,
    pub loan_book: LoanOrderBook,
    pub labor_book: LaborBook,
    pub money_system: Option<MoneySystem>,
    pub public_spot_tender: PublicSpotTender,
    pub public_debt_tender: PublicDebtTender,
    pub bank_repayment_tender: BankRepaymentTender,
    pub issuer_repayment_tender: IssuerRepaymentTender,
    pub labor_wage_tender: LaborWageTender,
    pub tax_receivability: TaxReceivability,
    tick_credit_retired: Gold,
    tick_bank_credit_issued: Gold,
    tick_fiat_credit_issued: Gold,
    tick_fiat_fiscal_issued: Gold,
    tick_fiat_fiscal_issued_by_issuer: Vec<(crate::ledger::IssuerId, Gold)>,
    tick_bank_loan_trades: u32,
    tick_fiat_loan_trades: u32,
    money: MarketMoneyState,
    v2_enabled: bool,
    multi_offer_medium: bool,
    durability_aware_acceptance: bool,
    marketability: MarketabilityConfig,
    barter_book: BarterBook,
    legacy_runner_enabled: bool,
    market_goods: Vec<GoodId>,
    max_good_id: u16,
    live_quotes: Vec<LiveQuote>,
    /// Per-`(agent, good)` spot-bid override (S1). A driver sets entries before a
    /// public step API and [`Society::ensure_bid`] uses the override's
    /// `(reservation, limit)` in place of the agent's own
    /// [`Agent::reservation_bid_for_money`]; the live-quote change detector
    /// consults it too, so the resting quote survives the tick's reconciliation.
    /// Cleared at the end of each [`Society::try_step`] (so also [`Society::step`]
    /// and [`Society::run`], which delegate to it) and
    /// [`Society::step_rejecting_v2_money_goods`], so an override is one-shot.
    /// Empty in every lab scenario (the lab never sets one),
    /// so the conformance goldens are byte-identical — an additive, gated seam.
    bid_overrides: BTreeMap<(AgentId, GoodId), (Gold, Gold)>,
    agent_order: Vec<usize>,
    /// Ids removed by [`Society::remove_agent`] (G4a real death), ascending. The
    /// id is freed from the arena, but it is recorded here so any capital project
    /// or open debt the dead colonist still owns stays frozen — heirs/capital
    /// inheritance are G4b. Empty in every lab scenario (the lab never frees an
    /// agent), so the project/debt freeze guards that binary-search it are no-ops
    /// and the conformance goldens are byte-identical.
    dead_agents: Vec<AgentId>,
    m2_enabled: bool,
    m3_enabled: bool,
    regime: Regime,
    project_lines: Vec<ProjectLine>,
    project_revenue: Gold,
    tick_labor_used: Vec<(AgentId, u32)>,
    capital_labor_consumed: u32,
    capital_goods_consumed: u32,
    capital_gold_loss: Gold,
    tick_self_funded_project_starts: Vec<(AgentId, ProjectLineId)>,
    /// G1 (`life` crate): per-tick per-agent consumed-good quantities, captured
    /// during the consume phase. Off by default and read by no engine path, so
    /// the conformance goldens are byte-identical; the `Camp` driver enables it
    /// to read realized FOOD/WOOD consumption back for need replenishment.
    consumption_log: Vec<(AgentId, GoodId, u32)>,
    consumption_log_enabled: bool,
}

impl Society {
    pub fn from_scenario(scenario: MarketScenario) -> Self {
        Self::try_from_scenario(scenario)
            .unwrap_or_else(|err| panic!("market scenario must be valid: {err}"))
    }

    pub fn try_from_scenario(scenario: MarketScenario) -> Result<Self, SocietyBuildError> {
        let market_goods = market_goods_for(&scenario);
        let max_good_id = max_good_id(&market_goods, &scenario.money);
        let money_good = scenario.money.current_money_good();
        let uses_emergent_money = matches!(&scenario.money, MarketMoneyConfig::Emergent(_));
        let scenario_kind = scenario.scenario.kind();
        let v2_enabled = scenario_kind == ScenarioKind::MarketV2;
        if v2_enabled && !uses_emergent_money {
            return Err(SocietyBuildError::V2RequiresEmergentMoney);
        }
        validate_v2_events_supported(v2_enabled, &scenario.events)?;
        validate_v2_initial_money_balances_zero(v2_enabled, &scenario.agents)?;
        let multi_offer_medium = match &scenario.money {
            MarketMoneyConfig::Emergent(config) => config.multi_offer_medium,
            MarketMoneyConfig::Designated(_) => false,
        };
        let (durability_aware_acceptance, marketability) = match &scenario.money {
            MarketMoneyConfig::Emergent(config) => (
                config.durability_aware_acceptance,
                config.marketability.clone(),
            ),
            MarketMoneyConfig::Designated(_) => (false, MarketabilityConfig::default()),
        };
        let money = MarketMoneyState::from_config(scenario.money);
        let books = market_goods.iter().copied().map(OrderBook::new).collect();
        let regime = scenario.scenario.regime();
        let m2_enabled = matches!(
            scenario_kind,
            ScenarioKind::MarketM2 | ScenarioKind::MarketM3
        );
        let m3_enabled = scenario_kind == ScenarioKind::MarketM3;
        let mut agents = scenario.agents;
        let initial_money_migrated = migrate_initial_money_stock(&mut agents, money_good);
        if !initial_money_migrated {
            return Err(SocietyBuildError::InitialDesignatedMoneyStockOverflow);
        }
        let banks = banks_for_scenario(scenario.scenario);
        let issuers = issuers_for_scenario(scenario.scenario);
        let initial_agents = agents.clone();
        let money_system = if m3_enabled {
            Some(
                MoneySystem::from_agents_with_banks(&agents, &banks)
                    .map_err(|_| SocietyBuildError::M3InitialBankDepositsInvalid)?,
            )
        } else {
            None
        };
        let legacy_runner_enabled =
            !v2_enabled && !uses_emergent_money && (!m3_enabled || money_system.is_some());
        let reservations = Reservations::new(&agents, max_good_id);
        let agent_order = agent_order_for(scenario.scenario.agent_order_priority(), &agents);
        Ok(Self {
            tick: Tick(0),
            agents: AgentArena::from_cast(agents),
            initial_agents,
            registry: GoodRegistry::lab_default(),
            recipes: scenario.recipes,
            books,
            records: Vec::new(),
            m2_records: Vec::new(),
            m3_records: Vec::new(),
            m3_shadow_attached: false,
            metric_observations: Vec::new(),
            money_audit: Vec::new(),
            bank_audit: Vec::new(),
            redemption_audit: Vec::new(),
            payment_audit: Vec::new(),
            wage_payment_audit: Vec::new(),
            debt_payment_audit: Vec::new(),
            bank_repayment_audit: Vec::new(),
            issuer_repayment_audit: Vec::new(),
            tax_audit: Vec::new(),
            barter_trades: Vec::new(),
            saleability_snapshots: Vec::new(),
            v2_records: Vec::new(),
            v2_promotion_failures: Vec::new(),
            metric_observation_accumulator: None,
            money_audit_enabled: false,
            bank_audit_enabled: false,
            metric_consumed_goods: Vec::new(),
            trades: Vec::new(),
            realized_prices: BTreeMap::new(),
            loan_trades: Vec::new(),
            labor_trades: Vec::new(),
            debts: Vec::new(),
            banks,
            issuers,
            cantillon_receipts: Vec::new(),
            m2_projects: Vec::new(),
            project_funding_plans: Vec::new(),
            project_output_lots: Vec::new(),
            rng: Rng::new(scenario.seed),
            seq: 0,
            next_debt_id: 1,
            next_m2_project_id: 1,
            next_project_plan_id: 1,
            events: scenario.events,
            reservations,
            loan_reservations: LoanReservations::new(),
            labor_reservations: LaborReservations::new(),
            loan_book: LoanOrderBook::new(),
            labor_book: LaborBook::new(),
            money_system,
            public_spot_tender: PublicSpotTender::ParAll,
            public_debt_tender: PublicDebtTender::ParAll,
            bank_repayment_tender: BankRepaymentTender::ParAll,
            issuer_repayment_tender: IssuerRepaymentTender::FiatOnly,
            labor_wage_tender: LaborWageTender::ParAll,
            tax_receivability: TaxReceivability::SpecieOnly,
            tick_credit_retired: Gold::ZERO,
            tick_bank_credit_issued: Gold::ZERO,
            tick_fiat_credit_issued: Gold::ZERO,
            tick_fiat_fiscal_issued: Gold::ZERO,
            tick_fiat_fiscal_issued_by_issuer: Vec::new(),
            tick_bank_loan_trades: 0,
            tick_fiat_loan_trades: 0,
            money,
            v2_enabled,
            multi_offer_medium,
            durability_aware_acceptance,
            marketability,
            barter_book: BarterBook::new(),
            legacy_runner_enabled,
            market_goods,
            max_good_id,
            live_quotes: Vec::new(),
            bid_overrides: BTreeMap::new(),
            agent_order,
            dead_agents: Vec::new(),
            m2_enabled,
            m3_enabled,
            regime,
            project_lines: project_lines_for_scenario(scenario.scenario),
            project_revenue: Gold::ZERO,
            tick_labor_used: Vec::new(),
            capital_labor_consumed: 0,
            capital_goods_consumed: 0,
            capital_gold_loss: Gold::ZERO,
            tick_self_funded_project_starts: Vec::new(),
            consumption_log: Vec::new(),
            consumption_log_enabled: false,
        })
    }

    pub fn try_run(&mut self, periods: u64) -> Result<(), SocietyStepError> {
        for _ in 0..periods {
            self.try_step()?;
        }
        Ok(())
    }

    pub fn run(&mut self, periods: u64) {
        match self.try_run(periods) {
            Ok(()) | Err(SocietyStepError::EmergentMoneyDeferred) => {}
        }
    }

    pub fn enable_metric_observations(&mut self) {
        self.metric_observations.clear();
        self.metric_consumed_goods.clear();
        self.metric_observation_accumulator = Some(MetricObservationAccumulator::default());
    }

    pub fn enable_money_audit(&mut self) {
        self.money_audit.clear();
        self.money_audit_enabled = true;
    }

    pub fn enable_bank_audit(&mut self) {
        self.bank_audit.clear();
        self.bank_audit_enabled = true;
    }

    pub fn attach_shadow(&mut self, shadow: &ShadowSeries) {
        assert_eq!(
            shadow.natural_rate_bps.len(),
            self.m3_records.len(),
            "shadow natural-rate series length must match live M3 records"
        );
        assert_eq!(
            shadow.structure_length_ticks_x100.len(),
            self.m3_records.len(),
            "shadow structure-length series length must match live M3 records"
        );
        for (index, record) in self.m3_records.iter_mut().enumerate() {
            let shadow_natural = shadow.natural_rate_bps[index];
            record.shadow_natural_rate_bps = shadow_natural;
            record.shadow_rate_gap_bps = match (shadow_natural, record.m2.market_rate_bps) {
                (Some(shadow), Some(market)) => Some(shadow - market),
                _ => None,
            };
        }
        self.m3_shadow_attached = true;
    }

    pub fn m3_shadow_attached(&self) -> bool {
        self.m3_shadow_attached
    }

    fn attach_sound_money_m3_view(&mut self) {
        if !self.m3_records.is_empty() {
            self.m3_shadow_attached = true;
            return;
        }
        let mut project_starts_by_tick = BTreeMap::new();
        for project in &self.m2_projects {
            let count = project_starts_by_tick
                .entry(project.started_at.0)
                .or_insert(0u32);
            *count = count.saturating_add(1);
        }

        let mut expected_tick = self.m2_records.first().map(|record| record.tick);
        let mut previous_abandoned = 0u32;
        for m2 in &self.m2_records {
            let expected = expected_tick.expect("sound-money M3 view has M2 records");
            assert_eq!(
                m2.tick, expected,
                "sound-money M3 view expects one chronological M2 record per tick"
            );
            expected_tick = m2.tick.checked_add(1);
            assert!(
                m2.abandoned_projects >= previous_abandoned,
                "sound-money M3 view expects cumulative abandoned project counts"
            );
            let boom_projects_started = project_starts_by_tick.get(&m2.tick).copied().unwrap_or(0);
            let bust_abandoned_projects = m2.abandoned_projects - previous_abandoned;
            previous_abandoned = m2.abandoned_projects;
            self.m3_records.push(M3Record {
                m2: m2.clone(),
                regime: Regime::SoundGold,
                public_specie: m2.total_gold,
                public_fiat: Gold::ZERO,
                demand_claims: Gold::ZERO,
                bank_reserves: Gold::ZERO,
                fiduciary: Gold::ZERO,
                time_deposits: Gold::ZERO,
                tms: m2.total_gold,
                bank_credit_issued: Gold::ZERO,
                fiat_credit_issued: Gold::ZERO,
                fiat_fiscal_issued: Gold::ZERO,
                credit_retired: Gold::ZERO,
                bank_loan_trades: 0,
                fiat_loan_trades: 0,
                shadow_natural_rate_bps: m2.natural_rate_proxy_bps,
                shadow_rate_gap_bps: m2.rate_gap_bps,
                boom_projects_started,
                bust_abandoned_projects,
                early_receiver_wealth_delta: 0,
                late_receiver_wealth_delta: 0,
            });
        }
        self.m3_shadow_attached = true;
    }

    fn try_step_inner(&mut self) -> Result<(), SocietyStepError> {
        if self.v2_enabled {
            self.step_v2(&[]);
            return Ok(());
        }
        if !self.legacy_runner_enabled {
            return Err(SocietyStepError::EmergentMoneyDeferred);
        }
        if self.m3_enabled {
            self.step_m3();
            return Ok(());
        }
        if self.m2_enabled {
            self.step_m2();
            return Ok(());
        }
        self.step_m1();
        Ok(())
    }

    pub fn try_step(&mut self) -> Result<(), SocietyStepError> {
        let result = self.try_step_inner();
        self.clear_bid_overrides();
        result
    }

    /// Advance the market society by one tick.
    ///
    /// Phase A exposes `try_step` for typed rejection of emergent-money
    /// scenarios until the Phase B V2 runner is wired.
    pub fn step(&mut self) {
        match self.try_step() {
            Ok(()) | Err(SocietyStepError::EmergentMoneyDeferred) => {}
        }
    }

    /// Set a spot-bid override for `agent` on `good` (S1). During the next
    /// [`Society::step`], [`Society::ensure_bid`] posts a bid for `good` using
    /// `reservation` (the agent's imputed max willingness to pay, used by the
    /// change detector) and `limit` (the posted price, still capped by the
    /// agent's free gold and tender), instead of the scale-derived
    /// [`Agent::reservation_bid_for_money`]. The bid enters the real order book
    /// through the sole [`Society::ensure_order`] path, so it reserves gold, can
    /// fill against a willing ask, and records a [`Trade`] exactly like any other
    /// quote. The override is consumed by exactly one step (cleared at the end),
    /// so a driver re-sets it each tick a producer stays active.
    ///
    /// Purely additive and gated: a society whose driver never calls this keeps an
    /// empty override table, so [`Society::ensure_bid`] and the change detector
    /// take their original branch and every conformance golden is byte-identical.
    pub fn set_bid_override(
        &mut self,
        agent: AgentId,
        good: GoodId,
        reservation: Gold,
        limit: Gold,
    ) {
        self.bid_overrides
            .insert((agent, good), (reservation, limit));
    }

    /// Drop every spot-bid override. The public step/run APIs call this at the end
    /// of each attempted tick so an override the driver set is consumed by a
    /// single step attempt. A no-op (so byte-identical) when the table is already
    /// empty.
    pub fn clear_bid_overrides(&mut self) {
        self.bid_overrides.clear();
    }

    /// The active spot-bid override for `(agent, good)`, if any (S1). `None` —
    /// the lab default — means [`Society::ensure_bid`] falls back to the agent's
    /// own [`Agent::reservation_bid_for_money`], the original behavior.
    fn bid_override_for(&self, agent: AgentId, good: GoodId) -> Option<(Gold, Gold)> {
        self.bid_overrides.get(&(agent, good)).copied()
    }

    /// Advance one tick while rejecting specific V2 commodity-money promotion
    /// goods. With an empty rejection list this is identical to [`Self::step`].
    ///
    /// This is an opt-in caller boundary for spatial simulations: econ's Mengerian
    /// `winner` rule still chooses the candidate, but a caller can decline to
    /// commit a money good that its own substrate cannot support (for example, a
    /// world-regenerated node good). Existing econ scenarios keep using `step()`.
    pub fn step_rejecting_v2_money_goods(&mut self, rejected_money_goods: &[GoodId]) {
        if self.v2_enabled {
            self.step_v2(rejected_money_goods);
            self.clear_bid_overrides();
            return;
        }
        self.step();
    }

    fn legacy_money_good(&self) -> GoodId {
        self.money
            .current_money_good()
            .expect("legacy market runner requires a current money good")
    }

    fn step_m1(&mut self) {
        let money_good = self.legacy_money_good();
        self.tick_labor_used.clear();
        if self.consumption_log_enabled {
            self.consumption_log.clear();
        }
        self.apply_events();
        let expired_orders = self.purge_expired_orders();

        for order_pos in 0..self.agent_order.len() {
            let index = self.agent_order[order_pos];
            let reserved_assets = self.take_reserved_assets(index);
            self.agents[index].clear_satisfaction();
            self.agents[index].recompute_satisfaction_for_money(money_good);
            let (_, mut provisions) =
                self.agents[index].consume_now_wants_with_provisions_for_money(money_good);
            self.record_consumed_provisions(index, &provisions);
            self.allocate_direct_labor(index, &mut provisions, Some(money_good), None);
            self.agents[index]
                .recompute_satisfaction_with_provisions_for_money(&provisions, money_good);
            self.restore_reserved_assets(index, reserved_assets);
        }

        self.cancel_changed_live_quotes();
        let trade_start = self.trades.len();
        let mut filled = Vec::new();
        for order_pos in 0..self.agent_order.len() {
            let agent_index = self.agent_order[order_pos];
            for good_pos in 0..self.market_goods.len() {
                let good = self.market_goods[good_pos];
                self.ensure_bid(agent_index, good, &mut filled);
                self.ensure_ask(agent_index, good, &mut filled);
            }
        }
        self.sync_live_quotes();

        let tick_trades = self.trades[trade_start..].to_vec();
        self.observe_tick_trades(&tick_trades);

        self.nudge_unfilled_quotes(&filled);
        self.records
            .push(self.build_record(expired_orders, &tick_trades));
        self.capture_metric_observation(&tick_trades);
        self.tick.0 += 1;
    }

    fn step_v2(&mut self, rejected_money_goods: &[GoodId]) {
        self.tick_labor_used.clear();
        // Mirror `step_m1`: when consumption logging is enabled, this tick's log
        // starts empty so `consumption_log_last_tick` reflects only what the V2
        // direct passes (barter or money phase) eat this tick. Inert (no clear)
        // for the lab's emergence goldens, which never enable the log — so M18/
        // M19/M20 stay byte-identical; the sim (G5a) enables it to read the
        // eaten sink for its whole-system conservation receipt.
        if self.consumption_log_enabled {
            self.consumption_log.clear();
        }
        self.apply_events();

        let trade_start = self.trades.len();
        let mut tick_barter_trades = Vec::new();
        let mut tick_spot_trades = Vec::new();
        let mut promoted_this_tick = false;
        let tick_phase;
        let expired_orders;

        if let Some(money_good) = self.money.current_money_good() {
            tick_phase = V2Phase::Money;
            expired_orders = self.purge_expired_orders();
            self.run_direct_pass_for_money(money_good);
            self.cancel_changed_live_quotes();
            let mut filled = Vec::new();
            for order_pos in 0..self.agent_order.len() {
                let agent_index = self.agent_order[order_pos];
                for good_pos in 0..self.market_goods.len() {
                    let good = self.market_goods[good_pos];
                    self.ensure_bid(agent_index, good, &mut filled);
                    self.ensure_ask(agent_index, good, &mut filled);
                }
            }
            self.sync_live_quotes();

            tick_spot_trades = self.trades[trade_start..].to_vec();
            self.observe_tick_trades(&tick_spot_trades);
            self.nudge_unfilled_quotes(&filled);
        } else {
            tick_phase = V2Phase::Barter;
            expired_orders = self.barter_book.expire_offers(self.tick.0);
            self.run_direct_pass_without_money();
            let saleability_context = self.v2_saleability_context();
            self.barter_book.cancel_invalid_with_saleability_context(
                self.agents.as_slice(),
                &saleability_context,
            );
            self.generate_direct_barter_offers(&saleability_context);
            self.generate_indirect_barter_offers(&saleability_context);
            tick_barter_trades = self.barter_book.clear_matches_with_saleability_context(
                self.agents.as_mut_slice(),
                self.tick.0,
                &saleability_context,
            );
            self.v2_observe_barter_trades(&tick_barter_trades);
            let snapshots = self.v2_saleability_snapshots();
            self.saleability_snapshots.extend(snapshots);

            let promotion_candidate = self.v2_promotion_candidate_after_tick();
            if let Some(money_good) = promotion_candidate {
                if rejected_money_goods.contains(&money_good) {
                    // One failure record per tick the rejected good keeps winning —
                    // the same every-tick-push shape as the `NonZeroMoneyBalance` /
                    // `BalanceOverflow` arms below, bounded by the run length. Not
                    // reachable in the shipped G5a configs (the curated `barter-camp`
                    // promotes SALT, a non-node good; the symmetric control never
                    // promotes), so the list stays empty there; a hypothetical run
                    // whose only viable candidate is a rejected (regenerated) good
                    // would accumulate one record per tick. Records are kept (not
                    // deduped) so the failure history is a faithful per-tick log.
                    self.v2_promotion_failures.push(V2PromotionFailure {
                        tick: self.tick.0,
                        money_good,
                        reason: V2PromotionFailureReason::UnsupportedMoneyGood,
                    });
                    let aborted_candidate = self.v2_end_saleability_tick_without_promotion();
                    debug_assert_eq!(aborted_candidate, Some(money_good));
                } else {
                    match self.promote_v2_money_good(money_good) {
                        Ok(()) => {
                            let committed = self.v2_end_saleability_tick();
                            assert_eq!(
                                committed,
                                Some(money_good),
                                "V2 promotion state must match the saleability tracker"
                            );
                            promoted_this_tick = true;
                        }
                        Err(reason) => {
                            self.v2_promotion_failures.push(V2PromotionFailure {
                                tick: self.tick.0,
                                money_good,
                                reason,
                            });
                            let aborted_candidate =
                                self.v2_end_saleability_tick_without_promotion();
                            debug_assert_eq!(aborted_candidate, Some(money_good));
                        }
                    }
                }
            } else {
                let committed = self.v2_end_saleability_tick();
                debug_assert_eq!(committed, None);
            }
            self.barter_trades
                .extend(tick_barter_trades.iter().cloned());
        }

        self.v2_records.push(self.build_v2_record(
            tick_phase,
            promoted_this_tick,
            &tick_barter_trades,
            &tick_spot_trades,
            expired_orders,
        ));
        self.capture_metric_observation(&tick_spot_trades);
        self.tick.0 += 1;
    }

    fn step_m2(&mut self) {
        let record = self.run_m2_tick();
        self.m2_records.push(record);
        debug_assert!(self.project_funding_invariants_hold());
        self.tick.0 += 1;
    }

    fn step_m3(&mut self) {
        let m2 = self.run_m3_tick();
        let record = self.build_m3_record(m2);
        self.m3_records.push(record);
        debug_assert!(self.project_funding_invariants_hold());
        assert!(
            self.money_ledgers_reconcile(),
            "M3 money ledgers must reconcile every tick"
        );
        self.tick.0 += 1;
    }

    fn run_m3_tick(&mut self) -> M2Record {
        let money_good = self.legacy_money_good();
        self.tick_labor_used.clear();
        // Mirror `step_m1`/`step_v2`: when consumption logging is enabled, this tick's
        // log starts empty so `consumption_log_last_tick` reflects only what this tick's
        // consume phase eats. Inert (no clear) for the lab's M3 goldens, which never
        // enable the log — so M3 stays byte-identical; the spatial sim (G8a) enables it
        // to read the eaten sink for its whole-system conservation receipt.
        if self.consumption_log_enabled {
            self.consumption_log.clear();
        }
        self.tick_self_funded_project_starts.clear();
        self.tick_credit_retired = Gold::ZERO;
        self.tick_bank_credit_issued = Gold::ZERO;
        self.tick_fiat_credit_issued = Gold::ZERO;
        self.tick_fiat_fiscal_issued = Gold::ZERO;
        self.tick_fiat_fiscal_issued_by_issuer.clear();
        self.tick_bank_loan_trades = 0;
        self.tick_fiat_loan_trades = 0;
        self.loan_reservations.reset_tick_lender_capacity();
        self.apply_events();
        let project_debt_payment_snapshot = self.project_debt_payment_snapshot();
        let settlement = {
            let money_system = self
                .money_system
                .as_mut()
                .expect("M3 society has a money system");
            settle_due_debts_m3_excluding_agents(
                DebtSettlementM3Context {
                    agents: self.agents.as_mut_slice(),
                    debts: &mut self.debts,
                    tick: self.tick,
                    money_system,
                    banks: &mut self.banks,
                    issuers: &mut self.issuers,
                    public_debt_tender: self.public_debt_tender,
                    bank_repayment_tender: self.bank_repayment_tender,
                    issuer_repayment_tender: self.issuer_repayment_tender,
                    tax_receivability: self.tax_receivability,
                    debt_payment_audit: &mut self.debt_payment_audit,
                    bank_repayment_audit: &mut self.bank_repayment_audit,
                    issuer_repayment_audit: &mut self.issuer_repayment_audit,
                    tax_audit: &mut self.tax_audit,
                },
                &self.dead_agents,
            )
        };
        if let Some(snapshot) = &project_debt_payment_snapshot {
            self.release_project_funding_reserves_for_debt_payments(snapshot);
        }
        self.tick_credit_retired = settlement.credit_retired;
        self.expire_project_funding_plans();
        let expired_orders = self.purge_expired_orders();
        let _expired_labor = self
            .labor_book
            .purge_expired(self.tick.0, &mut self.labor_reservations);
        let _expired_loans = self
            .loan_book
            .purge_expired(self.tick.0, &mut self.loan_reservations);

        for order_pos in 0..self.agent_order.len() {
            let index = self.agent_order[order_pos];
            let reserved_assets = self.take_reserved_assets(index);
            self.agents[index].clear_satisfaction();
            self.agents[index].recompute_satisfaction_for_money(money_good);
            let (_, mut provisions) =
                self.agents[index].consume_now_wants_with_provisions_for_money(money_good);
            // Capture the consumed sink before direct-labor provisioning (the same
            // conservative readback `step_m1` records), gated on the opt-in flags so
            // the M3 goldens are byte-identical. The G8a spatial sim reads this back
            // for its whole-system conservation receipt and need replenishment.
            self.record_consumed_provisions(index, &provisions);
            self.allocate_direct_labor(index, &mut provisions, Some(money_good), None);
            self.agents[index]
                .recompute_satisfaction_with_provisions_for_money(&provisions, money_good);
            self.restore_reserved_assets(index, reserved_assets);
        }

        self.mature_waiting_projects();
        let schedules = self.agent_schedules(money_good);
        self.abandon_unviable_projects(&schedules, money_good);
        self.labor_book.purge_invalid_hires(
            &mut self.labor_reservations,
            &self.m2_projects,
            &self.project_lines,
        );
        let labor_trade_start = self.labor_trades.len();
        self.plan_projects_and_hire(&schedules, money_good);
        self.post_labor_asks(money_good);
        let tick_labor_trades = self.labor_trades[labor_trade_start..].to_vec();

        self.cancel_changed_live_quotes();
        let trade_start = self.trades.len();
        let mut filled = Vec::new();
        for order_pos in 0..self.agent_order.len() {
            let agent_index = self.agent_order[order_pos];
            for good_pos in 0..self.market_goods.len() {
                let good = self.market_goods[good_pos];
                self.ensure_bid(agent_index, good, &mut filled);
                self.ensure_ask(agent_index, good, &mut filled);
            }
        }
        self.sync_live_quotes();

        let tick_trades = self.trades[trade_start..].to_vec();
        self.observe_tick_trades(&tick_trades);
        self.nudge_unfilled_quotes(&filled);

        let loan_schedules = self.agent_schedules(money_good);
        let proxy_trades = proxy_trades_from_schedules(self.tick.0, &loan_schedules);
        let natural_rate_proxy_bps = weighted_loan_bps(&proxy_trades);
        let loan_trade_start = self.loan_trades.len();
        self.post_loan_orders_m3(&loan_schedules, money_good);
        let tick_loan_trades = self.loan_trades[loan_trade_start..].to_vec();
        self.record_credit_cantillon_receipts(&tick_loan_trades);
        let (bank_credit_issued, bank_loan_trades) = bank_credit_stats(&tick_loan_trades);
        let (fiat_credit_issued, fiat_loan_trades) = fiat_credit_stats(&tick_loan_trades);
        self.tick_bank_credit_issued = bank_credit_issued;
        self.tick_bank_loan_trades = bank_loan_trades;
        self.tick_fiat_credit_issued = fiat_credit_issued;
        self.tick_fiat_loan_trades = fiat_loan_trades;
        let market_rate_bps = weighted_loan_bps(&tick_loan_trades);
        let rate_gap_bps = match (natural_rate_proxy_bps, market_rate_bps) {
            (Some(proxy), Some(market)) => Some(proxy - market),
            _ => None,
        };

        self.records
            .push(self.build_record(expired_orders, &tick_trades));
        let m2 = self.build_m2_record(
            &tick_trades,
            &tick_labor_trades,
            &tick_loan_trades,
            market_rate_bps,
            natural_rate_proxy_bps,
            rate_gap_bps,
        );
        self.capture_metric_observation(&tick_trades);
        self.capture_money_audit();
        self.capture_bank_audit();
        m2
    }

    fn run_m2_tick(&mut self) -> M2Record {
        let money_good = self.legacy_money_good();
        self.tick_labor_used.clear();
        self.tick_self_funded_project_starts.clear();
        self.apply_events();
        let project_debt_payment_snapshot = self.project_debt_payment_snapshot();
        let _settlement = settle_due_debts_excluding_agents(
            self.agents.as_mut_slice(),
            &mut self.debts,
            self.tick,
            &self.dead_agents,
        );
        if let Some(snapshot) = &project_debt_payment_snapshot {
            self.release_project_funding_reserves_for_debt_payments(snapshot);
        }
        self.expire_project_funding_plans();
        let expired_orders = self.purge_expired_orders();
        let _expired_labor = self
            .labor_book
            .purge_expired(self.tick.0, &mut self.labor_reservations);
        let _expired_loans = self
            .loan_book
            .purge_expired(self.tick.0, &mut self.loan_reservations);

        for order_pos in 0..self.agent_order.len() {
            let index = self.agent_order[order_pos];
            let reserved_assets = self.take_reserved_assets(index);
            self.agents[index].clear_satisfaction();
            self.agents[index].recompute_satisfaction_for_money(money_good);
            let (_, mut provisions) =
                self.agents[index].consume_now_wants_with_provisions_for_money(money_good);
            self.record_consumed_provisions(index, &provisions);
            self.allocate_direct_labor(index, &mut provisions, Some(money_good), None);
            self.agents[index]
                .recompute_satisfaction_with_provisions_for_money(&provisions, money_good);
            self.restore_reserved_assets(index, reserved_assets);
        }

        self.mature_waiting_projects();
        let schedules = self.agent_schedules(money_good);
        self.abandon_unviable_projects(&schedules, money_good);
        self.labor_book.purge_invalid_hires(
            &mut self.labor_reservations,
            &self.m2_projects,
            &self.project_lines,
        );
        let labor_trade_start = self.labor_trades.len();
        self.plan_projects_and_hire(&schedules, money_good);
        self.post_labor_asks(money_good);
        let tick_labor_trades = self.labor_trades[labor_trade_start..].to_vec();

        self.cancel_changed_live_quotes();
        let trade_start = self.trades.len();
        let mut filled = Vec::new();
        for order_pos in 0..self.agent_order.len() {
            let agent_index = self.agent_order[order_pos];
            for good_pos in 0..self.market_goods.len() {
                let good = self.market_goods[good_pos];
                self.ensure_bid(agent_index, good, &mut filled);
                self.ensure_ask(agent_index, good, &mut filled);
            }
        }
        self.sync_live_quotes();

        let tick_trades = self.trades[trade_start..].to_vec();
        self.observe_tick_trades(&tick_trades);
        self.nudge_unfilled_quotes(&filled);

        let loan_schedules = self.agent_schedules(money_good);
        let proxy_trades = proxy_trades_from_schedules(self.tick.0, &loan_schedules);
        let natural_rate_proxy_bps = weighted_loan_bps(&proxy_trades);
        let loan_trade_start = self.loan_trades.len();
        self.post_loan_orders(&loan_schedules, money_good);
        let tick_loan_trades = self.loan_trades[loan_trade_start..].to_vec();
        let market_rate_bps = weighted_loan_bps(&tick_loan_trades);
        let rate_gap_bps = match (natural_rate_proxy_bps, market_rate_bps) {
            (Some(proxy), Some(market)) => Some(proxy - market),
            _ => None,
        };

        self.records
            .push(self.build_record(expired_orders, &tick_trades));
        let m2 = self.build_m2_record(
            &tick_trades,
            &tick_labor_trades,
            &tick_loan_trades,
            market_rate_bps,
            natural_rate_proxy_bps,
            rate_gap_bps,
        );
        self.capture_metric_observation(&tick_trades);
        m2
    }

    pub fn total_money_balance(&self) -> Gold {
        if let Some(money_system) = &self.money_system {
            return money_system.base.commodity_base;
        }
        self.agents
            .iter()
            .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold))
    }

    pub fn total_gold(&self) -> Gold {
        self.total_money_balance()
    }

    pub fn total_stock(&self, good: GoodId) -> u32 {
        self.agents
            .iter()
            .map(|agent| agent.stock.get(good))
            .sum::<u32>()
    }

    pub fn current_money_good(&self) -> Option<GoodId> {
        self.money.current_money_good()
    }

    /// The runtime Mengerian emergence state, or `None` for a designated-money
    /// society. Read-only — G5a's spatial wiring reads the saleability leader,
    /// promotion tick, and the adopted config through this. The promotion
    /// DECISION still routes through the lab's `MengerianEmergence::winner`
    /// inside `step_v2`; this accessor adds no rule, only a read surface.
    pub fn emergence(&self) -> Option<&MengerianEmergence> {
        match &self.money {
            MarketMoneyState::Emergent(emergence) => Some(emergence),
            MarketMoneyState::Designated(_) => None,
        }
    }

    /// The econ tick at which a money good was promoted from realized barter, or
    /// `None` if no promotion has fired (or this is a designated-money society).
    pub fn money_promoted_at_tick(&self) -> Option<u64> {
        self.emergence().and_then(|e| e.promoted_at_tick())
    }

    /// The current provisional saleability leader the barter book routes indirect
    /// offers through, or `None` (no clear leader yet, or designated money).
    pub fn saleability_provisional_leader(&self) -> Option<GoodId> {
        self.emergence().and_then(|e| e.provisional_leader())
    }

    pub fn regime(&self) -> Regime {
        self.regime
    }

    pub fn market_goods(&self) -> &[GoodId] {
        &self.market_goods
    }

    pub fn live_barter_offer_count(&self) -> usize {
        self.barter_book.live_offers().len()
    }

    /// Read-only snapshot of live barter offers, in book order.
    pub fn live_barter_offers(&self) -> &[BarterOffer] {
        self.barter_book.live_offers()
    }

    pub fn live_spot_quote_count_for_good(&self, good: GoodId) -> usize {
        self.live_quotes
            .iter()
            .filter(|quote| quote.good == good)
            .count()
    }

    pub fn money_ledgers_reconcile(&self) -> bool {
        match &self.money_system {
            Some(money_system) => {
                money_system.invariants_hold_with_banks(self.agents.as_slice(), &self.banks)
            }
            None => true,
        }
    }

    fn apply_events(&mut self) {
        let tick = self.tick;
        let mut index = 0;
        while index < self.events.len() {
            if self.events[index].tick <= tick {
                let event = self.events.remove(index);
                self.apply_event(event.kind);
            } else {
                index += 1;
            }
        }
    }

    /// Apply an authored scenario event, then drop the result. Authored
    /// scenarios may silently tolerate a missing target (game-spec §7); the
    /// command path keeps the same mutation logic but surfaces the result.
    fn apply_event(&mut self, kind: EventKind) {
        // The event path is intentionally silent and lab-faithful: it performs
        // the lab's mutations (including the lab's silent tolerance) and
        // discards the result. It never enforces the command-only preconditions.
        let _ = self.apply_event_kind(kind, ApplyMode::Event);
    }

    /// Apply a player command over the existing `EventKind` surface, returning
    /// `Applied | Rejected(reason)`. Shares every check and mutation with the
    /// event path (G0b, game-spec §7) — nothing in `econ` calls this yet
    /// besides tests; it is plumbing for the sim crate's command queue.
    pub fn apply_command(&mut self, kind: EventKind) -> CommandResult {
        self.apply_event_kind(kind, ApplyMode::Command)
    }

    /// The shared logic both paths run. Each currently-silent event no-op
    /// becomes a named rejection; the event path discards it, the command path
    /// returns it. The mutations are byte-for-byte the lab's — the goldens gate
    /// this.
    ///
    /// Most rejections are mutation-preserving: the lab also performed *no*
    /// mutation when the target was missing, so an event-path `Rejected` (which
    /// is discarded) is byte-identical to the lab's silent no-op. A few handlers
    /// are *not* symmetric — the lab mutated regardless of whether an agent or
    /// bank existed (the silent-tolerance no-ops of game-spec §7). Those
    /// existence preconditions are gated on [`ApplyMode::Command`] so the event
    /// path keeps the lab's unconditional mutation while a command rejects
    /// loudly and side-effect-free.
    fn apply_event_kind(&mut self, kind: EventKind, mode: ApplyMode) -> CommandResult {
        match kind {
            EventKind::DisableRecipe(recipe_id) => {
                if let Some(recipe) = self
                    .recipes
                    .iter_mut()
                    .find(|recipe| recipe.id == recipe_id)
                {
                    recipe.enabled = false;
                    CommandResult::Applied
                } else {
                    CommandResult::rejected(
                        RejectReason::UnknownRecipe,
                        format!("no recipe {recipe_id:?}"),
                    )
                }
            }
            EventKind::SetRegime(regime) => {
                self.regime = regime;
                CommandResult::Applied
            }
            EventKind::SetPublicSpotTender(policy) => {
                self.public_spot_tender = policy;
                // Events apply before this tick's matching pass, so live quotes
                // must be rechecked under the new accepted-media policy now.
                self.cancel_changed_live_quotes();
                CommandResult::Applied
            }
            EventKind::SetPublicDebtTender(policy) => {
                self.public_debt_tender = policy;
                CommandResult::Applied
            }
            EventKind::SetBankRepaymentTender(policy) => {
                self.bank_repayment_tender = policy;
                CommandResult::Applied
            }
            EventKind::SetIssuerRepaymentTender(policy) => {
                self.issuer_repayment_tender = policy;
                CommandResult::Applied
            }
            EventKind::SetLaborWageTender(policy) => {
                self.labor_wage_tender = policy;
                CommandResult::Applied
            }
            EventKind::SetTaxReceivability(policy) => {
                self.tax_receivability = policy;
                CommandResult::Applied
            }
            EventKind::LevyTax {
                agent,
                amount,
                due_tick,
            } => self.apply_levy_tax(agent, amount, due_tick, mode),
            EventKind::SetDebtDueTick { debt, due_tick } => {
                if let Some(entry) = self.debts.iter_mut().find(|entry| entry.id == debt) {
                    entry.due_tick = due_tick;
                    CommandResult::Applied
                } else {
                    CommandResult::rejected(RejectReason::UnknownDebt, format!("no debt {debt:?}"))
                }
            }
            EventKind::SeedCommodityDebt {
                lender,
                borrower,
                principal,
                due,
                due_tick,
                purpose,
            } => {
                // Command-only precondition: the lab event path seeds the debt
                // regardless of whether the parties are live agents (silent
                // tolerance, game-spec §7); a command rejects an unowned debt.
                if mode == ApplyMode::Command {
                    if self.agents.get(lender).is_none() {
                        return CommandResult::rejected(
                            RejectReason::UnknownAgent,
                            format!("no lender {lender}"),
                        );
                    }
                    if self.agents.get(borrower).is_none() {
                        return CommandResult::rejected(
                            RejectReason::UnknownAgent,
                            format!("no borrower {borrower}"),
                        );
                    }
                }
                self.apply_seed_commodity_debt(lender, borrower, principal, due, due_tick, purpose)
            }
            EventKind::SeedStock { agent, good, qty } => self.apply_seed_stock(agent, good, qty),
            EventKind::SetReserveRatio { bank, ratio } => {
                if let Some(entry) = self.banks.iter_mut().find(|entry| entry.id == bank) {
                    entry.reserve_ratio_bps = ratio;
                    CommandResult::Applied
                } else {
                    CommandResult::rejected(RejectReason::UnknownBank, format!("no bank {bank:?}"))
                }
            }
            EventKind::SetBankConvertibility { bank, convertible } => {
                if let Some(entry) = self.banks.iter_mut().find(|entry| entry.id == bank) {
                    entry.convertible = convertible;
                    CommandResult::Applied
                } else {
                    CommandResult::rejected(RejectReason::UnknownBank, format!("no bank {bank:?}"))
                }
            }
            EventKind::SetBankCreditPolicy { bank, policy } => {
                if let Some(entry) = self.banks.iter_mut().find(|entry| entry.id == bank) {
                    entry.policy = policy;
                    CommandResult::Applied
                } else {
                    CommandResult::rejected(RejectReason::UnknownBank, format!("no bank {bank:?}"))
                }
            }
            EventKind::StopBankCredit { bank } => {
                let position = self.banks.iter().position(|entry| entry.id == bank);
                // A command for a missing bank rejects *before* touching the
                // order book, so a rejected command is side-effect-free. The
                // event path skips this and keeps the lab's unconditional cancel.
                if position.is_none() && mode == ApplyMode::Command {
                    return CommandResult::rejected(
                        RejectReason::UnknownBank,
                        format!("no bank {bank:?}"),
                    );
                }
                if let Some(index) = position {
                    self.banks[index].policy.enabled = false;
                }
                // Cancelling outstanding lender quotes is unconditional, exactly
                // as the lab does it — the result reports only the policy target.
                self.loan_book
                    .cancel_lender(CreditLender::Bank(bank), &mut self.loan_reservations);
                if position.is_some() {
                    CommandResult::Applied
                } else {
                    CommandResult::rejected(RejectReason::UnknownBank, format!("no bank {bank:?}"))
                }
            }
            EventKind::RedeemDemandClaims {
                bank,
                route,
                max_per_agent,
            } => self.apply_redemption_event(bank, &route, max_per_agent, mode),
            EventKind::FiatPrint {
                issuer,
                amount,
                route,
            } => self.apply_fiat_print(issuer, amount, &route, mode),
            EventKind::ResetPublicSpotBook => {
                self.cancel_all_live_quotes();
                CommandResult::Applied
            }
            EventKind::SetIssuerPolicy { issuer, policy } => {
                if let Some(entry) = self.issuers.iter_mut().find(|entry| entry.id == issuer) {
                    entry.policy = policy;
                    CommandResult::Applied
                } else {
                    CommandResult::rejected(
                        RejectReason::UnknownIssuer,
                        format!("no issuer {issuer:?}"),
                    )
                }
            }
            EventKind::StopIssuerCredit { issuer } => {
                let position = self.issuers.iter().position(|entry| entry.id == issuer);
                // As with `StopBankCredit`: a command for a missing issuer
                // rejects before the unconditional cancel; the event path keeps
                // the lab's unconditional cancel.
                if position.is_none() && mode == ApplyMode::Command {
                    return CommandResult::rejected(
                        RejectReason::UnknownIssuer,
                        format!("no issuer {issuer:?}"),
                    );
                }
                if let Some(index) = position {
                    self.issuers[index].policy.credit_enabled = false;
                }
                self.loan_book
                    .cancel_lender(CreditLender::Issuer(issuer), &mut self.loan_reservations);
                if position.is_some() {
                    CommandResult::Applied
                } else {
                    CommandResult::rejected(
                        RejectReason::UnknownIssuer,
                        format!("no issuer {issuer:?}"),
                    )
                }
            }
        }
    }

    /// Seed a commodity debt — the lab mutation, unconditional. This is the
    /// event path's byte-for-byte behavior (it seeds the debt regardless of
    /// whether the parties are live agents — game-spec §7 silent tolerance). The
    /// command path's `UnknownAgent` precondition lives in the dispatch arm.
    fn apply_seed_commodity_debt(
        &mut self,
        lender: AgentId,
        borrower: AgentId,
        principal: Gold,
        due: Gold,
        due_tick: Tick,
        purpose: DebtPurpose,
    ) -> CommandResult {
        self.debts.push(DebtContract {
            id: DebtId(self.next_debt_id),
            lender: CreditLender::Agent(lender),
            borrower,
            opened_tick: self.tick,
            due_tick,
            principal,
            due,
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose,
            funding: CreditSource::Commodity,
        });
        self.next_debt_id = self
            .next_debt_id
            .checked_add(1)
            .expect("seeded debt id overflow");
        CommandResult::Applied
    }

    /// Levies a tax (M21): seeds a `DebtContract` with the scenario's single
    /// issuer as lender, zero principal, and the levy as `due`. No money moves
    /// at levy time — the liability appears in the borrower's payables view and
    /// the existing earn-to-cover behavior does the rest. Without exactly one
    /// issuer the event is a no-op (no debt, no panic).
    fn apply_levy_tax(
        &mut self,
        agent: AgentId,
        amount: Gold,
        due_tick: Tick,
        mode: ApplyMode,
    ) -> CommandResult {
        // `LevyTax` intentionally carries no issuer id in M21: the controlled
        // tax scenarios have one state issuer. Future multi-issuer taxes need
        // an explicit event field instead of guessing which issuer receives it.
        // The issuer-count check is mutation-preserving (the lab also seeds no
        // debt without exactly one issuer), so it runs on both paths.
        let mut issuer_ids = self.issuers.iter().map(|issuer| issuer.id);
        let Some(issuer_id) = issuer_ids.next() else {
            return CommandResult::rejected(
                RejectReason::NoIssuer,
                "levy needs one issuer; found 0",
            );
        };
        if issuer_ids.next().is_some() {
            return CommandResult::rejected(
                RejectReason::NoIssuer,
                "levy needs one issuer; found several",
            );
        }
        // Command-only preconditions. The lab event path seeds the tax debt
        // unconditionally after the one-issuer check (silent tolerance,
        // game-spec §7): a zero amount seeds an open zero-due liability, and a
        // missing borrower an unowned one. A player command rejects both loudly
        // and side-effect-free, before mutating.
        if mode == ApplyMode::Command {
            if amount == Gold::ZERO {
                return CommandResult::rejected(RejectReason::Ineligible, "tax levy of zero");
            }
            if self.agents.get(agent).is_none() {
                return CommandResult::rejected(
                    RejectReason::UnknownAgent,
                    format!("no agent {agent}"),
                );
            }
        }
        self.debts.push(DebtContract {
            id: DebtId(self.next_debt_id),
            lender: CreditLender::Issuer(issuer_id),
            borrower: agent,
            opened_tick: self.tick,
            due_tick,
            principal: Gold::ZERO,
            due: amount,
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::TaxLiability,
            funding: CreditSource::Tax(issuer_id),
        });
        self.next_debt_id = self
            .next_debt_id
            .checked_add(1)
            .expect("levied tax debt id overflow");
        if let Some(issuer) = self
            .issuers
            .iter_mut()
            .find(|issuer| issuer.id == issuer_id)
        {
            issuer.record_tax_levied(amount);
        }
        CommandResult::Applied
    }

    fn apply_seed_stock(&mut self, agent: AgentId, good: GoodId, qty: u32) -> CommandResult {
        // The zero-qty / money-good guard is *mutation-preserving*: the lab event
        // path already returned early (silent no-op) for both cases, so it runs on
        // both paths and is intentionally NOT command-only. The event path
        // discards this `Rejected` and so stays byte-for-byte the lab's no-op; the
        // command path reports it loudly. (Reviewer-3: not a behavior change.)
        if qty == 0 || self.money.is_money_good(good) {
            return CommandResult::rejected(
                RejectReason::Ineligible,
                "seed stock needs a nonzero, non-money good",
            );
        }
        if let Some(entry) = self.agents.get_mut(agent) {
            entry.stock.add(good, qty);
            CommandResult::Applied
        } else {
            CommandResult::rejected(RejectReason::UnknownAgent, format!("no agent {agent}"))
        }
    }

    fn apply_redemption_event(
        &mut self,
        bank_id: BankId,
        route: &RedemptionRoute,
        max_per_agent: Option<Gold>,
        mode: ApplyMode,
    ) -> CommandResult {
        let Some(money_system) = self.money_system.as_mut() else {
            return CommandResult::rejected(
                RejectReason::NotApplicableToKernel,
                "redemption needs a money system",
            );
        };
        let bank_index = self.banks.iter().position(|bank| bank.id == bank_id);
        if mode == ApplyMode::Command {
            if bank_index.is_none() {
                return CommandResult::rejected(
                    RejectReason::UnknownBank,
                    format!("no bank {bank_id:?}"),
                );
            }
            // A zero per-agent cap requests nothing: every holder takes the
            // zero-request `continue` in the loop below, so the command would
            // otherwise fall through to `Applied` having mutated nothing — the
            // silent no-op the command surface exists to prevent. The event path
            // keeps that tolerance (and still records `NoClaim` rows for explicit
            // zero-claim holders, so this stays command-only); a command rejects.
            if max_per_agent == Some(Gold::ZERO) {
                return CommandResult::rejected(
                    RejectReason::Ineligible,
                    "redemption cap of zero requests nothing",
                );
            }
            if let RedemptionRoute::Agents(agents) = route {
                if agents.is_empty() {
                    return CommandResult::rejected(
                        RejectReason::Ineligible,
                        "redemption route reached no requesters",
                    );
                }
                for &agent in agents {
                    if self.agents.get(agent).is_none() {
                        return CommandResult::rejected(
                            RejectReason::UnknownAgent,
                            format!("no redemption requester {agent}"),
                        );
                    }
                }
            }
        }
        let requesters = match route {
            RedemptionRoute::AllClaimHolders => money_system
                .demand_claim_holders(bank_id)
                .into_iter()
                .map(|(agent, claim)| (agent, Some(claim), false))
                .collect::<Vec<_>>(),
            RedemptionRoute::Agents(agents) => agents
                .iter()
                .copied()
                .map(|agent| (agent, None, true))
                .collect::<Vec<_>>(),
        };
        // The command path already rejected a missing bank above, so only the
        // event path can still reach here with one. The lab records `BankMissing`
        // audit rows for a non-empty explicit `Agents` route but is a silent
        // no-op when the route reached no requesters (an empty `AllClaimHolders`
        // set or empty `Agents` list) — preserve that by bailing before the loop.
        if bank_index.is_none() && requesters.is_empty() {
            return CommandResult::rejected(
                RejectReason::UnknownBank,
                format!("no bank {bank_id:?}"),
            );
        }
        if mode == ApplyMode::Command && requesters.is_empty() {
            return CommandResult::rejected(
                RejectReason::Ineligible,
                "redemption route reached no requesters",
            );
        }
        let tick = self.tick.0;
        let regime = self.regime;
        let banks = &mut self.banks;
        let agents = self.agents.as_mut_slice();
        let redemption_audit = &mut self.redemption_audit;
        let mut cache_needs_reconcile = false;

        for (agent, claim, explicit) in requesters {
            let claim = claim.unwrap_or_else(|| money_system.demand_claim_on(agent, bank_id));
            let requested = max_per_agent.map_or(claim, |limit| claim.min(limit));

            let Some(found_bank_index) = bank_index else {
                if requested > Gold::ZERO || explicit {
                    redemption_audit.push(RedemptionAuditRecord {
                        tick,
                        bank: bank_id,
                        agent,
                        requested,
                        honored: Gold::ZERO,
                        failed: requested,
                        outcome: RedemptionOutcome::BankMissing,
                    });
                }
                continue;
            };

            if claim == Gold::ZERO {
                if explicit {
                    redemption_audit.push(RedemptionAuditRecord {
                        tick,
                        bank: bank_id,
                        agent,
                        requested: Gold::ZERO,
                        honored: Gold::ZERO,
                        failed: Gold::ZERO,
                        outcome: RedemptionOutcome::NoClaim,
                    });
                }
                continue;
            }
            if requested == Gold::ZERO {
                // A zero cap (max_per_agent == Some(Gold::ZERO)) on a holder that does
                // have a claim is a no-op: nothing requested, no mutation, and not a
                // failure. Per impl-12.md, audit rows are emitted only for nonzero
                // requests or explicit failures, and Honored requires requested > 0 — so
                // emit no row.
                continue;
            }

            if !banks[found_bank_index].convertible || regime == Regime::SuspendedConvertibility {
                redemption_audit.push(RedemptionAuditRecord {
                    tick,
                    bank: bank_id,
                    agent,
                    requested,
                    honored: Gold::ZERO,
                    failed: requested,
                    outcome: RedemptionOutcome::Suspended,
                });
                continue;
            }

            let planned_honored = requested.min(banks[found_bank_index].reserves);
            let mut honored = Gold::ZERO;
            let mut failed = requested;
            if planned_honored > Gold::ZERO {
                let mut staged_bank = banks[found_bank_index].clone();
                let staged = staged_bank
                    .retire_redeemed_claim(planned_honored)
                    .and_then(|_| staged_bank.debit_reserves(planned_honored));
                debug_assert!(
                    staged.is_ok(),
                    "staged bank redemption failed despite planned reserves"
                );
                if staged.is_ok() {
                    let redeemed = money_system.redeem_demand_claim_for_specie(
                        agent,
                        bank_id,
                        planned_honored,
                    );
                    debug_assert!(
                        redeemed.is_ok(),
                        "money-system redemption failed despite positive claim and staged bank"
                    );
                    if redeemed.is_ok() {
                        honored = planned_honored;
                        failed = requested.saturating_sub(honored);
                        banks[found_bank_index] = staged_bank;
                        cache_needs_reconcile = true;
                    }
                }
            }

            let outcome = if honored == requested {
                RedemptionOutcome::Honored
            } else if honored > Gold::ZERO {
                RedemptionOutcome::PartiallyHonored
            } else {
                RedemptionOutcome::ReserveExhausted
            };
            redemption_audit.push(RedemptionAuditRecord {
                tick,
                bank: bank_id,
                agent,
                requested,
                honored,
                failed,
                outcome,
            });
        }
        if cache_needs_reconcile {
            money_system.reconcile_agent_cache(agents);
        }
        CommandResult::Applied
    }

    fn apply_fiat_print(
        &mut self,
        issuer_id: crate::ledger::IssuerId,
        amount: Gold,
        route: &CantillonRoute,
        mode: ApplyMode,
    ) -> CommandResult {
        if amount == Gold::ZERO {
            return CommandResult::rejected(RejectReason::Ineligible, "fiat print of zero");
        }
        let Some(issuer_pos) = self
            .issuers
            .iter()
            .position(|issuer| issuer.id == issuer_id)
        else {
            return CommandResult::rejected(
                RejectReason::UnknownIssuer,
                format!("no issuer {issuer_id:?}"),
            );
        };
        if mode == ApplyMode::Command {
            if let CantillonRoute::Agents(agents) = route {
                if agents.is_empty() {
                    return CommandResult::rejected(
                        RejectReason::Ineligible,
                        "route reached no recipients",
                    );
                }
                for &agent in agents {
                    if self.agents.get(agent).is_none() {
                        return CommandResult::rejected(
                            RejectReason::UnknownAgent,
                            format!("no fiat recipient {agent}"),
                        );
                    }
                }
            }
        }
        let credits = CantillonRouter::route(route, self.agents.as_slice(), amount);
        if credits.is_empty() {
            return CommandResult::rejected(
                RejectReason::Ineligible,
                "route reached no recipients",
            );
        }
        let source = CreditSource::FiatFiscal(issuer_id);
        let mut staged_issuer = self.issuers[issuer_pos].clone();
        let already_issued =
            fiscal_issued_this_tick(&self.tick_fiat_fiscal_issued_by_issuer, issuer_id);
        if already_issued.saturating_add(amount) > staged_issuer.policy.max_fiscal_issue_per_tick {
            return CommandResult::rejected(
                RejectReason::Ineligible,
                "exceeds max fiscal issue per tick",
            );
        }
        if staged_issuer.fiscal_issue(self.regime, amount).is_err() {
            return CommandResult::rejected(
                RejectReason::Ineligible,
                "issuer refused fiscal issue",
            );
        }
        let Some(mut staged_money) = self.money_system.clone() else {
            return CommandResult::rejected(
                RejectReason::NotApplicableToKernel,
                "fiat print needs a money system",
            );
        };
        for (agent, share) in &credits {
            if staged_money.credit_fiat(*agent, *share).is_err() {
                return CommandResult::rejected(RejectReason::Ineligible, "fiat credit failed");
            }
        }
        self.issuers[issuer_pos] = staged_issuer;
        self.money_system = Some(staged_money);
        if let Some(money_system) = &self.money_system {
            money_system.reconcile_agent_cache(self.agents.as_mut_slice());
        }
        self.tick_fiat_fiscal_issued = self.tick_fiat_fiscal_issued.saturating_add(amount);
        record_fiscal_issued_this_tick(
            &mut self.tick_fiat_fiscal_issued_by_issuer,
            issuer_id,
            amount,
        );
        self.cantillon_receipts
            .extend(CantillonRouter::receipts(self.tick, &credits, source));
        CommandResult::Applied
    }

    fn purge_expired_orders(&mut self) -> u32 {
        let mut expired = 0;
        for book in &mut self.books {
            expired += book.purge_expired(self.tick.0, &mut self.reservations);
        }
        self.sync_live_quotes();
        expired
    }

    fn allocate_direct_labor(
        &mut self,
        index: usize,
        provisions: &mut TickProvisions,
        money_good: Option<GoodId>,
        excluded_recipe_good: Option<GoodId>,
    ) {
        let agent_id = self.agents[index].id;
        let mut remaining = self.agents[index]
            .labor_capacity
            .saturating_sub(self.labor_reservations.reserved_labor(agent_id))
            .saturating_sub(self.tick_labor_used(agent_id));
        let mut used = 0u32;
        while remaining > 0 {
            if let Some(money_good) = money_good {
                self.agents[index]
                    .recompute_satisfaction_with_provisions_for_money(provisions, money_good);
            } else {
                self.agents[index].recompute_satisfaction_with_provisions_without_money(provisions);
            }
            let candidates = if let Some(money_good) = money_good {
                if let Some(excluded_good) = excluded_recipe_good {
                    direct_recipe_candidates_excluding_good(
                        &self.agents[index],
                        &self.recipes,
                        remaining,
                        provisions,
                        true,
                        excluded_good,
                    )
                } else {
                    direct_recipe_candidates_for_money(
                        &self.agents[index],
                        &self.recipes,
                        remaining,
                        provisions,
                        true,
                        money_good,
                    )
                }
            } else {
                direct_recipe_candidates(
                    &self.agents[index],
                    &self.recipes,
                    remaining,
                    provisions,
                    true,
                )
            };
            let Some(candidate) = candidates.into_iter().next() else {
                break;
            };
            if self.agents[index]
                .first_unsatisfied_leisure_rank()
                .map(|rest_rank| candidate.rank >= rest_rank)
                .unwrap_or(false)
            {
                break;
            }
            let recipe_id = direct_recipe_action_recipe_id(candidate.action);
            let labor = if let Some(money_good) = money_good {
                if excluded_recipe_good.is_some() {
                    execute_direct_recipe_for_agent(
                        &mut self.agents[index],
                        &self.recipes,
                        recipe_id,
                        remaining,
                        candidate.rank,
                        provisions,
                    )
                } else {
                    self.execute_direct_recipe_for_agent_with_money(
                        index,
                        recipe_id,
                        remaining,
                        candidate.rank,
                        provisions,
                        money_good,
                    )
                }
            } else {
                execute_direct_recipe_for_agent(
                    &mut self.agents[index],
                    &self.recipes,
                    recipe_id,
                    remaining,
                    candidate.rank,
                    provisions,
                )
            };
            let Some(labor) = labor else {
                break;
            };
            if labor == 0 || labor > remaining {
                break;
            }
            remaining -= labor;
            used = used.saturating_add(labor);
        }
        if used > 0 {
            self.add_tick_labor_used(agent_id, used);
        }
    }

    fn execute_direct_recipe_for_agent_with_money(
        &mut self,
        index: usize,
        recipe_id: RecipeId,
        remaining: u32,
        rank: usize,
        provisions: &mut TickProvisions,
        money_good: GoodId,
    ) -> Option<u32> {
        let output = direct_recipe_output(&self.recipes, recipe_id);
        let money_output_qty = output
            .filter(|(good, qty)| *good == money_good && *qty > 0)
            .map(|(_, qty)| qty)
            .unwrap_or(0);
        if money_output_qty > 0
            && self.agents[index]
                .stock
                .get(money_good)
                .checked_add(money_output_qty)
                .is_none()
        {
            return None;
        }
        let money_output = Gold(u64::from(money_output_qty));
        let staged_credit = if money_output > Gold::ZERO {
            Some(self.stage_agent_money_credit(index, money_output)?)
        } else {
            None
        };
        let labor = execute_direct_recipe_for_agent_for_money(
            &mut self.agents[index],
            &self.recipes,
            recipe_id,
            remaining,
            rank,
            provisions,
            money_good,
        )?;
        if let Some((new_gold, ledger_credit)) = staged_credit {
            if money_output_qty > 0
                && !self.agents[index]
                    .stock
                    .remove(money_good, money_output_qty)
            {
                return None;
            }
            self.commit_agent_money_credit(index, new_gold, ledger_credit);
        }
        Some(labor)
    }

    fn run_direct_pass_for_money(&mut self, money_good: GoodId) {
        for order_pos in 0..self.agent_order.len() {
            let index = self.agent_order[order_pos];
            let reserved_assets = self.take_reserved_assets(index);
            self.agents[index].clear_satisfaction();
            self.agents[index].recompute_satisfaction_for_money(money_good);
            let (_, mut provisions) =
                self.agents[index].consume_now_wants_with_provisions_for_money(money_good);
            self.record_consumed_provisions(index, &provisions);
            self.allocate_direct_labor(index, &mut provisions, Some(money_good), Some(money_good));
            self.agents[index]
                .recompute_satisfaction_with_provisions_for_money(&provisions, money_good);
            self.restore_reserved_assets(index, reserved_assets);
        }
    }

    fn run_direct_pass_without_money(&mut self) {
        for order_pos in 0..self.agent_order.len() {
            let index = self.agent_order[order_pos];
            let reserved_assets = self.take_reserved_assets(index);
            self.agents[index].clear_satisfaction();
            self.agents[index].recompute_satisfaction_without_money();
            let (_, mut provisions) =
                self.agents[index].consume_now_wants_with_provisions_without_money();
            self.record_consumed_provisions(index, &provisions);
            self.allocate_direct_labor(index, &mut provisions, None, None);
            self.agents[index].recompute_satisfaction_with_provisions_without_money(&provisions);
            self.restore_reserved_assets(index, reserved_assets);
        }
    }

    fn generate_direct_barter_offers(&mut self, saleability_context: &SaleabilityContext) {
        let SaleabilityContext::Single(provisional_leader) = saleability_context else {
            self.generate_candidate_direct_barter_offers(saleability_context);
            return;
        };
        for order_pos in 0..self.agent_order.len() {
            let agent_index = self.agent_order[order_pos];
            let agent_id = self.agents[agent_index].id;
            let receive_goods = self.near_unsatisfied_goods(agent_index);
            let give_goods = self.agents[agent_index]
                .stock
                .positive_goods()
                .collect::<Vec<_>>();
            if let Some(leader) = provisional_leader {
                if self.multi_offer_medium {
                    self.cancel_non_lane_barter_offers_for_agent(agent_id, *leader);
                    self.post_first_direct_leader_offer(
                        agent_id,
                        &receive_goods,
                        &give_goods,
                        *leader,
                        saleability_context,
                    );
                    self.post_first_medium_spend_offer(
                        agent_id,
                        &receive_goods,
                        *leader,
                        saleability_context,
                    );
                    continue;
                }
                if self.agent_has_live_barter_offer(agent_id) {
                    continue;
                }
                if self.post_first_direct_barter_offer(
                    agent_id,
                    &receive_goods,
                    &give_goods,
                    saleability_context,
                    Some(*leader),
                ) {
                    continue;
                }
            }
            if self.multi_offer_medium {
                // No provisional leader: the legacy one-live-offer policy applies.
                // The two-lane path can leave a second `DirectWant` lane live from
                // an earlier leader tick (an agent that both directly wanted the
                // leader and spent it), and `cancel_invalid(.., None)` only drops
                // `IndirectFor` offers — so collapse to the agent's oldest live
                // offer before the one-live-offer skip, restoring legacy behaviour.
                self.collapse_to_single_barter_offer_for_agent(agent_id);
            }
            if self.agent_has_live_barter_offer(agent_id) {
                continue;
            }
            self.post_first_direct_barter_offer(
                agent_id,
                &receive_goods,
                &give_goods,
                saleability_context,
                None,
            );
        }
    }

    fn generate_candidate_direct_barter_offers(
        &mut self,
        saleability_context: &SaleabilityContext,
    ) {
        let SaleabilityContext::Candidates(candidates) = saleability_context else {
            return;
        };
        if candidates.is_empty() {
            self.generate_direct_barter_offers(&SaleabilityContext::single(None));
            return;
        }
        for order_pos in 0..self.agent_order.len() {
            let agent_index = self.agent_order[order_pos];
            let agent_id = self.agents[agent_index].id;
            let receive_goods = self.near_unsatisfied_goods(agent_index);
            let give_goods = self.agents[agent_index]
                .stock
                .positive_goods()
                .collect::<Vec<_>>();

            self.cancel_non_candidate_lane_barter_offers_for_agent(agent_id, candidates);
            for candidate in candidates {
                self.post_first_direct_leader_offer(
                    agent_id,
                    &receive_goods,
                    &give_goods,
                    *candidate,
                    saleability_context,
                );
                self.post_first_medium_spend_offer(
                    agent_id,
                    &receive_goods,
                    *candidate,
                    saleability_context,
                );
            }
            // S21c: keep one legacy direct-discovery lane for BELOW-FLOOR goods (neither
            // side a candidate), so a good with real direct demand can still accrue direct
            // acceptances and cross the direct-use floor late. Medium routing above stays
            // candidate-only. Skip if the cancel pass already preserved such a lane.
            if !self.agent_has_legacy_direct_discovery_lane(agent_id, candidates) {
                let below_floor_receive = receive_goods
                    .iter()
                    .copied()
                    .filter(|good| !candidates.contains(good))
                    .collect::<Vec<_>>();
                let below_floor_give = give_goods
                    .iter()
                    .copied()
                    .filter(|good| !candidates.contains(good))
                    .collect::<Vec<_>>();
                self.post_first_direct_barter_offer(
                    agent_id,
                    &below_floor_receive,
                    &below_floor_give,
                    saleability_context,
                    None,
                );
            }
        }
    }

    fn post_first_direct_barter_offer(
        &mut self,
        agent: AgentId,
        receive_goods: &[GoodId],
        give_goods: &[GoodId],
        saleability_context: &SaleabilityContext,
        required_leader: Option<GoodId>,
    ) -> bool {
        for receive_good in receive_goods {
            for give_good in give_goods {
                if let Some(leader) = required_leader {
                    if *receive_good != leader && *give_good != leader {
                        continue;
                    }
                }
                if self.post_barter_offer(
                    agent,
                    *give_good,
                    *receive_good,
                    BarterReason::DirectWant,
                    saleability_context,
                ) {
                    return true;
                }
            }
        }
        false
    }

    fn post_first_direct_leader_offer(
        &mut self,
        agent: AgentId,
        receive_goods: &[GoodId],
        give_goods: &[GoodId],
        leader: GoodId,
        saleability_context: &SaleabilityContext,
    ) {
        if !receive_goods.contains(&leader) {
            return;
        }
        let previous = self.cancel_live_barter_receive_leader_offers_for_agent(agent, leader);
        for give_good in give_goods {
            if *give_good == leader {
                continue;
            }
            if self.post_barter_offer(
                agent,
                *give_good,
                leader,
                BarterReason::DirectWant,
                saleability_context,
            ) {
                return;
            }
        }
        // Failure path: no direct-leader bid could be posted, so restore only the
        // prior DirectWant receive-leader offers. Any prior IndirectFor sell lane
        // is intentionally dropped here — this agent now wants the leader DIRECTLY
        // (edge case b), so `post_first_medium_sell_offer` deliberately won't
        // re-post it, and leaving it live would mislabel direct demand as indirect.
        let previous_direct = previous
            .into_iter()
            .filter(|offer| matches!(offer.reason, BarterReason::DirectWant))
            .collect();
        self.restore_barter_offers(previous_direct, saleability_context);
    }

    fn post_first_medium_spend_offer(
        &mut self,
        agent: AgentId,
        receive_goods: &[GoodId],
        leader: GoodId,
        saleability_context: &SaleabilityContext,
    ) {
        let previous = self.cancel_live_barter_spend_offers_for_agent(agent, leader);
        for receive_good in receive_goods {
            if *receive_good == leader {
                continue;
            }
            if self.post_barter_offer(
                agent,
                leader,
                *receive_good,
                BarterReason::DirectWant,
                saleability_context,
            ) {
                return;
            }
        }
        self.restore_barter_offers(previous, saleability_context);
    }

    fn generate_indirect_barter_offers(&mut self, saleability_context: &SaleabilityContext) {
        // S9 control: with indirect acceptance gated off, no agent posts an
        // `IndirectFor` offer — the leader still leads and still trades DIRECTLY, but
        // no indirect volume can accrue. Under a positive indirect-breadth gate this
        // is the clean no-indirect-acceptance control (it cannot monetize), without
        // lowering the leader floor (which would disable leadership itself).
        if !self.allow_indirect_acceptance() {
            return;
        }
        match saleability_context {
            SaleabilityContext::Single(Some(leader)) => {
                self.generate_single_indirect_barter_offers(*leader, saleability_context);
            }
            SaleabilityContext::Single(None) => {}
            SaleabilityContext::Candidates(candidates) => {
                self.generate_candidate_indirect_barter_offers(candidates, saleability_context);
            }
        }
    }

    fn generate_single_indirect_barter_offers(
        &mut self,
        leader: GoodId,
        saleability_context: &SaleabilityContext,
    ) {
        for order_pos in 0..self.agent_order.len() {
            let agent_index = self.agent_order[order_pos];
            let agent_id = self.agents[agent_index].id;
            let target_goods = self.near_unsatisfied_goods(agent_index);
            if self.multi_offer_medium {
                self.post_first_medium_sell_offer(
                    agent_index,
                    agent_id,
                    &target_goods,
                    leader,
                    saleability_context,
                );
                continue;
            }
            if self.agent_has_live_barter_offer(agent_id)
                && target_goods.iter().all(|good| *good == leader)
            {
                continue;
            }
            if target_goods.contains(&leader) {
                continue;
            }
            let give_goods = self.agents[agent_index]
                .stock
                .positive_goods()
                .collect::<Vec<_>>();
            let mut posted = false;
            for target in target_goods {
                if target == leader {
                    continue;
                }
                for give_good in &give_goods {
                    if *give_good == leader || *give_good == target {
                        continue;
                    }
                    if !self.agents[agent_index].would_accept_indirect_barter_swap_with_stock(
                        &self.agents[agent_index].stock,
                        *give_good,
                        leader,
                        target,
                        1,
                        MarketabilityAcceptance {
                            durability_aware_acceptance: self.durability_aware_acceptance,
                            config: &self.marketability,
                        },
                    ) {
                        continue;
                    }
                    if self.replace_live_barter_offers_for_agent_with(
                        agent_id,
                        *give_good,
                        leader,
                        BarterReason::IndirectFor { target },
                        saleability_context,
                    ) {
                        posted = true;
                        break;
                    }
                }
                if posted {
                    break;
                }
            }
        }
    }

    fn generate_candidate_indirect_barter_offers(
        &mut self,
        candidates: &[GoodId],
        saleability_context: &SaleabilityContext,
    ) {
        if candidates.is_empty() {
            return;
        }
        for order_pos in 0..self.agent_order.len() {
            let agent_index = self.agent_order[order_pos];
            let agent_id = self.agents[agent_index].id;
            let target_goods = self.near_unsatisfied_goods(agent_index);
            for candidate in candidates {
                self.post_first_medium_sell_offer(
                    agent_index,
                    agent_id,
                    &target_goods,
                    *candidate,
                    saleability_context,
                );
            }
        }
    }

    fn post_first_medium_sell_offer(
        &mut self,
        agent_index: usize,
        agent_id: AgentId,
        target_goods: &[GoodId],
        leader: GoodId,
        saleability_context: &SaleabilityContext,
    ) {
        if target_goods.contains(&leader) {
            // Edge case (b): the agent's relevant want is for the leader DIRECTLY,
            // so it must not post an indirect sell lane. The direct pass owns any
            // direct receive-leader bid and leaves it live here.
            return;
        }
        let previous = self.cancel_live_barter_receive_leader_offers_for_agent(agent_id, leader);
        let give_goods = self.agents[agent_index]
            .stock
            .positive_goods()
            .collect::<Vec<_>>();
        for target in target_goods {
            for give_good in &give_goods {
                if *give_good == leader || *give_good == *target {
                    continue;
                }
                if !self.agents[agent_index].would_accept_indirect_barter_swap_with_stock(
                    &self.agents[agent_index].stock,
                    *give_good,
                    leader,
                    *target,
                    1,
                    MarketabilityAcceptance {
                        durability_aware_acceptance: self.durability_aware_acceptance,
                        config: &self.marketability,
                    },
                ) {
                    continue;
                }
                let held = self.agents[agent_index].stock.get(*give_good);
                let reserved = self.barter_book.reserved_qty(agent_id, *give_good);
                let displaced_direct = if held <= reserved {
                    self.cancel_live_barter_offers_for_agent_matching(agent_id, |offer| {
                        offer.give_good == *give_good
                            && !matches!(offer.reason, BarterReason::IndirectFor { .. })
                    })
                } else {
                    Vec::new()
                };
                let reserved = self.barter_book.reserved_qty(agent_id, *give_good);
                if held <= reserved {
                    self.restore_barter_offers(displaced_direct, saleability_context);
                    continue;
                }
                if self.post_barter_offer(
                    agent_id,
                    *give_good,
                    leader,
                    BarterReason::IndirectFor { target: *target },
                    saleability_context,
                ) {
                    return;
                }
                self.restore_barter_offers(displaced_direct, saleability_context);
            }
        }
        self.restore_barter_offers(previous, saleability_context);
    }

    fn replace_live_barter_offers_for_agent_with(
        &mut self,
        agent: AgentId,
        give_good: GoodId,
        receive_good: GoodId,
        reason: BarterReason,
        saleability_context: &SaleabilityContext,
    ) -> bool {
        let previous = self
            .barter_book
            .live_offers()
            .iter()
            .filter(|offer| offer.agent == agent)
            .cloned()
            .collect::<Vec<_>>();
        for offer in &previous {
            self.barter_book.cancel_offer(offer.seq);
        }

        if self.post_barter_offer(agent, give_good, receive_good, reason, saleability_context) {
            return true;
        }

        self.restore_barter_offers(previous, saleability_context);
        false
    }

    /// The spend lane shape: `give leader → receive a non-leader want` (DirectWant).
    fn is_medium_spend_lane(offer: &BarterOffer, leader: GoodId) -> bool {
        offer.give_good == leader
            && offer.receive_good != leader
            && matches!(offer.reason, BarterReason::DirectWant)
    }

    /// The receive-leader lane is either direct leader demand or an indirect
    /// sell-for-medium order. It is still one slot, distinct from the spend lane.
    fn is_medium_receive_leader_lane(offer: &BarterOffer, leader: GoodId) -> bool {
        offer.give_good != leader
            && offer.receive_good == leader
            && matches!(
                offer.reason,
                BarterReason::DirectWant | BarterReason::IndirectFor { .. }
            )
    }

    fn is_medium_spend_lane_for_candidates(offer: &BarterOffer, candidates: &[GoodId]) -> bool {
        candidates.contains(&offer.give_good)
            && offer.receive_good != offer.give_good
            && matches!(offer.reason, BarterReason::DirectWant)
    }

    fn is_medium_receive_candidate_lane(offer: &BarterOffer, candidates: &[GoodId]) -> bool {
        offer.give_good != offer.receive_good
            && candidates.contains(&offer.receive_good)
            && matches!(
                offer.reason,
                BarterReason::DirectWant | BarterReason::IndirectFor { .. }
            )
    }

    /// S21c: the legacy direct-discovery lane — an ordinary `DirectWant` barter whose
    /// BOTH sides are below the direct-use floor (neither good is a candidate medium).
    /// It lets a not-yet-eligible good keep accruing direct acceptances so it can cross
    /// the direct-use floor LATE; without it, candidate-mode routing freezes discovery
    /// on whatever good crossed first (the S21b open-discovery path-dependence). It can
    /// neither spend nor acquire a candidate, so it never bypasses the medium.
    fn is_legacy_direct_discovery_lane(offer: &BarterOffer, candidates: &[GoodId]) -> bool {
        !candidates.contains(&offer.give_good)
            && !candidates.contains(&offer.receive_good)
            && matches!(offer.reason, BarterReason::DirectWant)
    }

    fn agent_has_legacy_direct_discovery_lane(
        &self,
        agent: AgentId,
        candidates: &[GoodId],
    ) -> bool {
        self.barter_book.live_offers().iter().any(|offer| {
            offer.agent == agent && Self::is_legacy_direct_discovery_lane(offer, candidates)
        })
    }

    fn cancel_live_barter_spend_offers_for_agent(
        &mut self,
        agent: AgentId,
        leader: GoodId,
    ) -> Vec<BarterOffer> {
        self.cancel_live_barter_offers_for_agent_matching(agent, |offer| {
            Self::is_medium_spend_lane(offer, leader)
        })
    }

    fn cancel_live_barter_receive_leader_offers_for_agent(
        &mut self,
        agent: AgentId,
        leader: GoodId,
    ) -> Vec<BarterOffer> {
        self.cancel_live_barter_offers_for_agent_matching(agent, |offer| {
            Self::is_medium_receive_leader_lane(offer, leader)
        })
    }

    /// Enforce the S20 "exactly two lanes" invariant: cancel every live offer for
    /// the agent that is neither the spend lane nor the receive-leader lane. These
    /// are pre-leader direct barters that were posted before the medium emerged;
    /// left live they would be a third order and could even clear as a direct
    /// cycle-good swap, bypassing the medium. Dropped outright (not restored) —
    /// they are stale.
    fn cancel_non_lane_barter_offers_for_agent(&mut self, agent: AgentId, leader: GoodId) {
        self.cancel_live_barter_offers_for_agent_matching(agent, |offer| {
            !Self::is_medium_spend_lane(offer, leader)
                && !Self::is_medium_receive_leader_lane(offer, leader)
        });
    }

    fn cancel_non_candidate_lane_barter_offers_for_agent(
        &mut self,
        agent: AgentId,
        candidates: &[GoodId],
    ) {
        self.cancel_live_barter_offers_for_agent_matching(agent, |offer| {
            !Self::is_medium_spend_lane_for_candidates(offer, candidates)
                && !Self::is_medium_receive_candidate_lane(offer, candidates)
                && !Self::is_legacy_direct_discovery_lane(offer, candidates)
        });
    }

    /// Restore the legacy one-live-offer policy for `agent`: keep only its oldest
    /// (lowest-`seq`) live offer and cancel the rest. Used in the no-leader direct
    /// pass under `multi_offer_medium`, where the two-lane path may have left a
    /// second `DirectWant` lane live from an earlier leader tick.
    fn collapse_to_single_barter_offer_for_agent(&mut self, agent: AgentId) {
        let mut seqs = self
            .barter_book
            .live_offers()
            .iter()
            .filter(|offer| offer.agent == agent)
            .map(|offer| offer.seq)
            .collect::<Vec<_>>();
        if seqs.len() <= 1 {
            return;
        }
        seqs.sort_unstable();
        for seq in seqs.into_iter().skip(1) {
            self.barter_book.cancel_offer(seq);
        }
    }

    fn cancel_live_barter_offers_for_agent_matching(
        &mut self,
        agent: AgentId,
        matches_lane: impl Fn(&BarterOffer) -> bool,
    ) -> Vec<BarterOffer> {
        let previous = self
            .barter_book
            .live_offers()
            .iter()
            .filter(|offer| offer.agent == agent && matches_lane(offer))
            .cloned()
            .collect::<Vec<_>>();
        for offer in &previous {
            self.barter_book.cancel_offer(offer.seq);
        }
        previous
    }

    fn restore_barter_offers(
        &mut self,
        offers: Vec<BarterOffer>,
        saleability_context: &SaleabilityContext,
    ) {
        for offer in offers {
            self.barter_book.post_offer_with_saleability_context(
                self.agents.as_slice(),
                offer,
                self.tick.0,
                saleability_context,
            );
        }
    }

    fn post_barter_offer(
        &mut self,
        agent: AgentId,
        give_good: GoodId,
        receive_good: GoodId,
        reason: BarterReason,
        saleability_context: &SaleabilityContext,
    ) -> bool {
        if give_good == receive_good {
            return false;
        }
        self.seq = self.seq.saturating_add(1);
        let offer = BarterOffer {
            agent,
            give_good,
            receive_good,
            qty: 1,
            reason,
            seq: self.seq,
            expires_tick: self.tick.0.saturating_add(ORDER_TTL),
        };
        self.barter_book.post_offer_with_saleability_context(
            self.agents.as_slice(),
            offer,
            self.tick.0,
            saleability_context,
        )
    }

    fn agent_has_live_barter_offer(&self, agent: AgentId) -> bool {
        self.barter_book
            .live_offers()
            .iter()
            .any(|offer| offer.agent == agent)
    }

    fn near_unsatisfied_goods(&self, agent_index: usize) -> Vec<GoodId> {
        self.agents[agent_index].near_unsatisfied_goods_without_money()
    }

    fn v2_saleability_context(&self) -> SaleabilityContext {
        match &self.money {
            MarketMoneyState::Emergent(emergence)
                if emergence.config().two_layer_saleability && self.multi_offer_medium =>
            {
                SaleabilityContext::candidates(emergence.provisional_media_candidates())
            }
            MarketMoneyState::Emergent(emergence) => {
                SaleabilityContext::single(emergence.provisional_leader())
            }
            MarketMoneyState::Designated(_) => SaleabilityContext::single(None),
        }
    }

    /// S9: whether the emergence envelope permits posting indirect barter offers.
    /// Defaults to `true` (the existing behaviour); a designated-money society has
    /// no barter phase, so the answer is moot but defaults `true`.
    fn allow_indirect_acceptance(&self) -> bool {
        match &self.money {
            MarketMoneyState::Emergent(emergence) => emergence.config().allow_indirect_acceptance,
            MarketMoneyState::Designated(_) => true,
        }
    }

    fn v2_observe_barter_trades(&mut self, trades: &[BarterTrade]) {
        let MarketMoneyState::Emergent(emergence) = &mut self.money else {
            return;
        };
        for trade in trades {
            emergence.observe_trade(trade);
        }
    }

    fn v2_saleability_snapshots(&self) -> Vec<SaleabilitySnapshot> {
        match &self.money {
            MarketMoneyState::Emergent(emergence) => emergence.snapshots(self.tick.0),
            MarketMoneyState::Designated(_) => Vec::new(),
        }
    }

    fn v2_promotion_candidate_after_tick(&self) -> Option<GoodId> {
        match &self.money {
            MarketMoneyState::Emergent(emergence) => emergence.promotion_candidate_after_tick(),
            MarketMoneyState::Designated(_) => None,
        }
    }

    fn v2_end_saleability_tick(&mut self) -> Option<GoodId> {
        match &mut self.money {
            MarketMoneyState::Emergent(emergence) => emergence.end_tick(self.tick.0),
            MarketMoneyState::Designated(_) => None,
        }
    }

    fn v2_end_saleability_tick_without_promotion(&mut self) -> Option<GoodId> {
        match &mut self.money {
            MarketMoneyState::Emergent(emergence) => emergence.end_tick_without_promotion(),
            MarketMoneyState::Designated(_) => None,
        }
    }

    fn promote_v2_money_good(
        &mut self,
        money_good: GoodId,
    ) -> Result<(), V2PromotionFailureReason> {
        let mut new_balances = Vec::with_capacity(self.agents.len());
        for agent in self.agents.as_slice() {
            if agent.gold != Gold::ZERO {
                return Err(V2PromotionFailureReason::NonZeroMoneyBalance);
            }
            let stock_units = Gold(u64::from(agent.stock.get(money_good)));
            let Some(new_balance) = agent.gold.checked_add(stock_units) else {
                return Err(V2PromotionFailureReason::BalanceOverflow);
            };
            new_balances.push(new_balance);
        }

        self.barter_book = BarterBook::new();
        let mut quote_index = 0;
        while quote_index < self.live_quotes.len() {
            if self.live_quotes[quote_index].good == money_good {
                self.cancel_existing(Some(quote_index));
            } else {
                quote_index += 1;
            }
        }

        for (agent, new_balance) in self.agents.as_mut_slice().iter_mut().zip(new_balances) {
            let qty = agent.stock.get(money_good);
            if qty > 0 {
                let removed = agent.stock.remove(money_good, qty);
                debug_assert!(removed);
            }
            agent.gold = new_balance;
        }

        self.market_goods.retain(|good| *good != money_good);
        self.books.retain(|book| book.good != money_good);
        self.sync_live_quotes();
        Ok(())
    }

    fn build_v2_record(
        &self,
        phase: V2Phase,
        promoted_this_tick: bool,
        tick_barter_trades: &[BarterTrade],
        tick_spot_trades: &[Trade],
        expired_orders: u32,
    ) -> V2Record {
        let report_candidate = self.money.current_money_good().is_none() || promoted_this_tick;
        let (candidate_good, candidate_share_bps, runner_up_share_bps) = if report_candidate {
            match &self.money {
                MarketMoneyState::Emergent(emergence) => emergence
                    .leader_shares()
                    .and_then(|leader| {
                        if leader.tied_best || leader.share_bps <= leader.runner_up_share_bps {
                            return None;
                        }
                        Some((
                            Some(leader.good),
                            Some(leader.share_bps),
                            Some(leader.runner_up_share_bps),
                        ))
                    })
                    .unwrap_or((None, None, None)),
                MarketMoneyState::Designated(_) => (None, None, None),
            }
        } else {
            (None, None, None)
        };
        let mut bid_count = 0u32;
        let mut ask_count = 0u32;
        for book in &self.books {
            let (bids, asks) = book.live_order_counts();
            bid_count = bid_count.saturating_add(bids);
            ask_count = ask_count.saturating_add(asks);
        }

        V2Record {
            tick: self.tick.0,
            phase,
            money_good: self.money.current_money_good(),
            promoted_this_tick,
            barter_trades: u32::try_from(tick_barter_trades.len()).unwrap_or(u32::MAX),
            spot_trades: u32::try_from(tick_spot_trades.len()).unwrap_or(u32::MAX),
            candidate_good,
            candidate_share_bps,
            runner_up_share_bps,
            total_money_units: self.total_money_balance(),
            bid_count,
            ask_count,
            expired_orders,
        }
    }

    fn credit_agent_money(&mut self, agent_index: usize, amount: Gold) -> bool {
        let Some((new_gold, ledger_credit)) = self.stage_agent_money_credit(agent_index, amount)
        else {
            return false;
        };
        self.commit_agent_money_credit(agent_index, new_gold, ledger_credit);
        true
    }

    fn stage_agent_money_credit(
        &self,
        agent_index: usize,
        amount: Gold,
    ) -> Option<(Gold, Option<Gold>)> {
        if amount == Gold::ZERO {
            return self
                .agents
                .as_slice()
                .get(agent_index)
                .map(|agent| (agent.gold, None));
        }
        let agent = self.agents.as_slice().get(agent_index)?;
        let new_gold = agent.gold.checked_add(amount)?;
        let ledger_credit = if self.m3_enabled {
            self.money_system
                .as_ref()?
                .validate_specie_credit(agent.id, amount)
                .ok()?;
            Some(amount)
        } else {
            None
        };
        Some((new_gold, ledger_credit))
    }

    fn stage_agent_money_debit(
        &self,
        agent_index: usize,
        amount: Gold,
    ) -> Option<(Gold, Option<Gold>)> {
        if amount == Gold::ZERO {
            return self
                .agents
                .as_slice()
                .get(agent_index)
                .map(|agent| (agent.gold, None));
        }
        let agent = self.agents.as_slice().get(agent_index)?;
        let new_gold = agent.gold.checked_sub(amount)?;
        let ledger_debit = if self.m3_enabled {
            self.money_system
                .as_ref()?
                .validate_specie_debit(agent.id, amount)
                .ok()?;
            Some(amount)
        } else {
            None
        };
        Some((new_gold, ledger_debit))
    }

    fn commit_agent_money_credit(
        &mut self,
        agent_index: usize,
        new_gold: Gold,
        ledger_credit: Option<Gold>,
    ) {
        if let Some(amount) = ledger_credit {
            let agent = self.agents[agent_index].id;
            self.money_system
                .as_mut()
                .expect("M3 money credit preflight requires a money system")
                .credit_specie(agent, amount)
                .expect("M3 money credit was preflighted");
        }
        if let Some(agent) = self.agents.as_mut_slice().get_mut(agent_index) {
            agent.gold = new_gold;
        }
    }

    fn commit_agent_money_debit(
        &mut self,
        agent_index: usize,
        new_gold: Gold,
        ledger_debit: Option<Gold>,
    ) -> bool {
        if let Some(amount) = ledger_debit {
            let agent = self.agents[agent_index].id;
            let Some(money_system) = self.money_system.as_mut() else {
                return false;
            };
            if money_system.debit_specie(agent, amount).is_err() {
                return false;
            }
            if let Some(agent) = self.agents.as_mut_slice().get_mut(agent_index) {
                agent.gold = new_gold;
                true
            } else {
                false
            }
        } else if let Some(agent) = self.agents.as_mut_slice().get_mut(agent_index) {
            agent.gold = new_gold;
            true
        } else {
            false
        }
    }

    fn ensure_bid(&mut self, agent_index: usize, good: GoodId, filled: &mut Vec<FillKey>) {
        if self.money.is_money_good(good) {
            return;
        }
        let Some(money_good) = self.money.current_money_good() else {
            return;
        };
        let agent_id = self.agents[agent_index].id;
        let existing = self.find_live_quote(agent_id, OrderSide::Bid, good);
        let Some(agent) = self.available_agent(agent_index, existing) else {
            self.cancel_existing(existing);
            return;
        };
        // S1: a driver-set override replaces the scale-derived reservation/limit
        // for this `(agent, good)`. Empty in every lab scenario, so the original
        // `reservation_bid_for_money` branch is taken and behavior is unchanged.
        let (reservation, mut limit) = match self.bid_override_for(agent_id, good) {
            Some((reservation, limit)) => (reservation, limit),
            None => {
                let Some(reservation) = agent.reservation_bid_for_money(good, 1, money_good) else {
                    self.cancel_existing(existing);
                    return;
                };
                let belief = belief_for(&self.agents[agent_index], good);
                (reservation, belief.shade_bid(reservation))
            }
        };
        limit = limit.min(agent.gold);
        limit = limit.min(self.free_spot_tender_after_all_reserves_for_quote(agent.id, existing));
        if limit == Gold::ZERO {
            self.cancel_existing(existing);
            return;
        }
        self.ensure_order(
            QuotePlan {
                agent_index,
                side: OrderSide::Bid,
                good,
                reservation,
                limit,
                existing,
            },
            filled,
        );
    }

    fn ensure_ask(&mut self, agent_index: usize, good: GoodId, filled: &mut Vec<FillKey>) {
        if self.money.is_money_good(good) {
            return;
        }
        let Some(money_good) = self.money.current_money_good() else {
            return;
        };
        let existing = self.find_live_quote(self.agents[agent_index].id, OrderSide::Ask, good);
        let Some(agent) = self.available_agent(agent_index, existing) else {
            self.cancel_existing(existing);
            return;
        };
        if agent.stock.get(good) == 0 {
            self.cancel_existing(existing);
            return;
        }
        let Some(reservation) = agent.reservation_ask_for_money(good, 1, money_good) else {
            self.cancel_existing(existing);
            return;
        };
        let belief = belief_for(&self.agents[agent_index], good);
        let limit = belief.shade_ask(reservation);
        self.ensure_order(
            QuotePlan {
                agent_index,
                side: OrderSide::Ask,
                good,
                reservation,
                limit,
                existing,
            },
            filled,
        );
    }

    fn ensure_order(&mut self, plan: QuotePlan, filled: &mut Vec<FillKey>) {
        if let Some(index) = plan.existing {
            let quote = self.live_quotes[index];
            if quote.reservation == plan.reservation && quote.limit == plan.limit {
                return;
            }
            self.cancel_existing(Some(index));
        }

        self.seq += 1;
        let order = Order {
            agent: self.agents[plan.agent_index].id,
            side: plan.side,
            good: plan.good,
            limit: plan.limit,
            qty: 1,
            seq: self.seq,
            expires_tick: self.tick.0 + ORDER_TTL,
        };
        if order.side == OrderSide::Bid {
            let Some(amount) = order.limit.mul_qty(order.qty) else {
                return;
            };
            if self.free_spot_tender_after_all_reserves(order.agent) < amount {
                return;
            }
        }
        if !self.reservations.reserve_order(&self.agents, &order) {
            return;
        }
        self.live_quotes.push(LiveQuote {
            agent: order.agent,
            side: plan.side,
            good: plan.good,
            reservation: plan.reservation,
            limit: plan.limit,
            qty: order.qty,
            seq: order.seq,
        });
        let book_index = self
            .books
            .iter()
            .position(|book| book.good == plan.good)
            .expect("market good has a book");
        let executions = if self.m3_enabled {
            let tender = self.public_spot_tender;
            let money_system = self
                .money_system
                .as_mut()
                .expect("M3 spot order requires money system");
            self.books[book_index].add_order_m3(
                order,
                self.tick.0,
                self.agents.as_mut_slice(),
                &mut self.reservations,
                money_system,
                tender.accepted_media(),
            )
        } else {
            self.books[book_index]
                .add_order(
                    order,
                    self.tick.0,
                    self.agents.as_mut_slice(),
                    &mut self.reservations,
                )
                .into_iter()
                .map(|trade| ExecutedTrade {
                    trade,
                    payment: None,
                })
                .collect()
        };
        let mut affected_agents = Vec::new();
        for execution in executions {
            let trade = execution.trade;
            if let Some(payment) = execution.payment {
                let amount = payment.total();
                let demand_claims = payment
                    .claims
                    .iter()
                    .fold(Gold::ZERO, |total, (_, claim)| total.saturating_add(*claim));
                self.payment_audit.push(PaymentAuditRecord {
                    tick: trade.tick,
                    kind: PaymentKind::Spot,
                    from: trade.buyer,
                    to: trade.seller,
                    amount,
                    public_fiat: payment.fiat,
                    demand_claims,
                    public_specie: payment.specie,
                    tender: self.public_spot_tender,
                });
            }
            affected_agents.push(trade.buyer);
            affected_agents.push(trade.seller);
            filled.push(FillKey {
                agent: trade.buyer,
                side: OrderSide::Bid,
                good: trade.good,
            });
            filled.push(FillKey {
                agent: trade.seller,
                side: OrderSide::Ask,
                good: trade.good,
            });
            if self.m2_enabled {
                self.attribute_project_sale(&trade);
            }
            self.record_realized_trade_price(&trade);
            self.trades.push(trade);
        }
        self.sync_live_quotes();
        if !affected_agents.is_empty() {
            self.cancel_changed_live_quotes_for_agents(&affected_agents);
        }
    }

    fn available_agent(&self, agent_index: usize, existing: Option<usize>) -> Option<Agent> {
        let source = &self.agents[agent_index];
        let reserved_gold = self.reserved_gold_all(source.id);
        if reserved_gold > source.gold {
            return None;
        }
        let mut stock = Stock::new(self.max_good_id);
        for good in &self.market_goods {
            let good = *good;
            let reserved = self.reservations.reserved_stock(source.id, good);
            let held = source.stock.get(good);
            if reserved > held {
                return None;
            }
            stock.add(good, held - reserved);
        }
        let mut gold = source.gold.checked_sub(reserved_gold)?;
        if let Some(quote_index) = existing {
            let quote = self.live_quotes[quote_index];
            match quote.side {
                OrderSide::Bid => {
                    let amount = quote.limit.mul_qty(quote.qty)?;
                    gold = gold.checked_add(amount)?;
                }
                OrderSide::Ask => stock.add(quote.good, quote.qty),
            }
        }
        let mut agent = source.clone();
        agent.stock = stock;
        agent.gold = gold;
        Some(agent)
    }

    fn take_reserved_assets(&mut self, agent_index: usize) -> ReservedAssets {
        let agent_id = self.agents[agent_index].id;
        let mut removed = ReservedAssets::default();
        let reserved_gold = self
            .reserved_gold_all(agent_id)
            .min(self.agents[agent_index].gold);
        if reserved_gold > Gold::ZERO {
            self.agents[agent_index].gold = self.agents[agent_index]
                .gold
                .checked_sub(reserved_gold)
                .expect("reserved gold is bounded by held gold");
            removed.gold = reserved_gold;
        }
        for good in &self.market_goods {
            let good = *good;
            let qty = self
                .reservations
                .reserved_stock(agent_id, good)
                .min(self.agents[agent_index].stock.get(good));
            if qty > 0 && self.agents[agent_index].stock.remove(good, qty) {
                removed.stock.push((good, qty));
            }
        }
        for (good, reserved) in self.barter_book.reserved_stock_for(agent_id) {
            let qty = reserved.min(self.agents[agent_index].stock.get(good));
            if qty > 0 && self.agents[agent_index].stock.remove(good, qty) {
                removed.stock.push((good, qty));
            }
        }
        removed
    }

    fn restore_reserved_assets(&mut self, agent_index: usize, removed: ReservedAssets) {
        if removed.gold > Gold::ZERO {
            self.agents[agent_index].gold = self.agents[agent_index]
                .gold
                .checked_add(removed.gold)
                .expect("restoring reserved gold cannot overflow");
        }
        for (good, qty) in removed.stock {
            self.agents[agent_index].stock.add(good, qty);
        }
    }

    fn cancel_changed_live_quotes(&mut self) {
        let mut index = 0;
        while index < self.live_quotes.len() {
            if self.live_quote_changed(index) {
                self.cancel_existing(Some(index));
            } else {
                index += 1;
            }
        }
    }

    /// Cancel resting spot quotes for any listed agent whose reservation no
    /// longer matches its current scale, holdings, or tender state.
    ///
    /// This batch form lets a driver rewrite many scales between ticks and scan
    /// the live quote list once after all rewrites.
    ///
    /// The changed-agent list is normalized once, then probed by binary search
    /// while scanning live quotes, so the hot pass stays hash-free and avoids a
    /// `live_quotes * agents` membership walk.
    pub fn cancel_changed_live_quotes_for_agents(&mut self, agents: &[AgentId]) {
        if agents.is_empty() {
            return;
        }
        let mut changed_agents = agents.to_vec();
        changed_agents.sort();
        changed_agents.dedup();

        let mut index = 0;
        while index < self.live_quotes.len() {
            if changed_agents
                .binary_search(&self.live_quotes[index].agent)
                .is_ok()
                && self.live_quote_changed(index)
            {
                self.cancel_existing(Some(index));
            } else {
                index += 1;
            }
        }
    }

    fn live_quote_changed(&self, quote_index: usize) -> bool {
        let quote = self.live_quotes[quote_index];
        let Some(money_good) = self.money.current_money_good() else {
            return true;
        };
        let Some(agent_index) = self.agent_index_for(quote.agent) else {
            return true;
        };
        let Some(agent) = self.available_agent(agent_index, Some(quote_index)) else {
            return true;
        };
        let belief = belief_for(&self.agents[agent_index], quote.good);
        match quote.side {
            OrderSide::Bid => {
                // S1: mirror `ensure_bid`'s override branch so an override quote is
                // judged against the SAME reservation/limit it was posted at — else
                // the change detector recomputes the scale price, sees a mismatch,
                // and cancels the override bid (the highest-risk failure mode). With
                // no override the original `reservation_bid_for_money` branch runs,
                // so the goldens are byte-identical.
                let (reservation, shaded_limit) =
                    match self.bid_override_for(quote.agent, quote.good) {
                        Some((reservation, limit)) => (reservation, limit),
                        None => {
                            let Some(reservation) =
                                agent.reservation_bid_for_money(quote.good, quote.qty, money_good)
                            else {
                                return true;
                            };
                            (reservation, belief.shade_bid(reservation))
                        }
                    };
                let limit = shaded_limit.min(agent.gold).min(
                    self.free_spot_tender_after_all_reserves_for_quote(
                        quote.agent,
                        Some(quote_index),
                    ),
                );
                quote.reservation != reservation || quote.limit != limit || limit == Gold::ZERO
            }
            OrderSide::Ask => {
                let Some(reservation) =
                    agent.reservation_ask_for_money(quote.good, quote.qty, money_good)
                else {
                    return true;
                };
                let limit = belief.shade_ask(reservation);
                quote.reservation != reservation || quote.limit != limit
            }
        }
    }

    fn cancel_existing(&mut self, existing: Option<usize>) {
        if let Some(index) = existing {
            let quote = self.live_quotes.remove(index);
            if let Some(book_index) = self.books.iter().position(|book| book.good == quote.good) {
                self.books[book_index].cancel(
                    quote.agent,
                    quote.side,
                    quote.good,
                    quote.seq,
                    &mut self.reservations,
                );
            }
        }
    }

    fn cancel_all_live_quotes(&mut self) {
        while !self.live_quotes.is_empty() {
            self.cancel_existing(Some(0));
        }
    }

    fn nudge_unfilled_quotes(&mut self, filled: &[FillKey]) {
        let live_quotes = self.live_quotes.clone();
        for quote in live_quotes {
            if filled.iter().any(|filled| {
                filled.agent == quote.agent
                    && filled.side == quote.side
                    && filled.good == quote.good
            }) {
                continue;
            }
            let Some(agent_index) = self.agent_index_for(quote.agent) else {
                continue;
            };
            let agent = &mut self.agents[agent_index];
            match quote.side {
                OrderSide::Bid => {
                    belief_mut(agent, quote.good).nudge_unfilled_bid(quote.reservation, self.tick.0)
                }
                OrderSide::Ask => {
                    belief_mut(agent, quote.good).nudge_unfilled_ask(quote.reservation, self.tick.0)
                }
            }
        }
    }

    fn observe_tick_trades(&mut self, tick_trades: &[Trade]) {
        if tick_trades.is_empty() {
            return;
        }

        let mut watchers: BTreeMap<GoodId, BTreeSet<AgentId>> = BTreeMap::new();
        for trade in tick_trades {
            let agents = watchers.entry(trade.good).or_default();
            agents.insert(trade.buyer);
            agents.insert(trade.seller);
        }
        for quote in &self.live_quotes {
            watchers.entry(quote.good).or_default().insert(quote.agent);
        }

        for trade in tick_trades {
            let Some(agents) = watchers.get(&trade.good) else {
                continue;
            };
            for agent in agents {
                if let Some(index) = self.agent_index_for(*agent) {
                    belief_mut(&mut self.agents[index], trade.good)
                        .observe(trade.price, self.tick.0);
                }
            }
        }
    }

    fn record_realized_trade_price(&mut self, trade: &Trade) {
        if trade.qty == 0 {
            return;
        }
        self.realized_prices.insert(trade.good, trade.price);
    }

    fn mature_waiting_projects(&mut self) {
        let money_good = self.money.current_money_good();
        let mut lots = Vec::new();
        // A dead owner's project stays frozen: G4a settles a dead colonist's gold
        // and stock to the commons but NOT its capital — heirs/capital inheritance
        // are G4b — so a project it still owns never matures and its output is
        // neither minted nor credited. The owner's arena slot is freed, so without
        // this guard `mature_project` would still produce a lot whose owner no
        // longer resolves. The guard reads the dead-agent list (see
        // [`Society::remove_agent`] and docs/engine-divergence.md) — empty in every
        // conformance scenario — so the goldens are byte-identical.
        let dead = &self.dead_agents;
        for project in self.m2_projects.iter_mut() {
            if dead.binary_search(&project.owner).is_ok() {
                continue;
            }
            if let Some(lot) = mature_project(project, self.tick) {
                lots.push(lot);
            }
        }
        for lot in lots {
            if money_good == Some(lot.good) {
                if let Some(agent_index) = self.agent_index_for(lot.owner) {
                    let proceeds = Gold(u64::from(lot.qty_remaining));
                    if self.credit_agent_money(agent_index, proceeds) {
                        if let Some(project) = self
                            .m2_projects
                            .iter_mut()
                            .find(|project| project.id == lot.project)
                        {
                            project.state = M2ProjectState::Sold;
                        }
                        self.project_revenue = self.project_revenue.saturating_add(proceeds);
                        self.project_output_lots.push(ProjectOutputLot {
                            qty_remaining: 0,
                            proceeds,
                            ..lot
                        });
                        self.release_completed_project_reserves();
                        continue;
                    }
                }
            }
            if let Some(agent_index) = self.agent_index_for(lot.owner) {
                self.agents[agent_index]
                    .stock
                    .add(lot.good, lot.qty_remaining);
            }
            self.project_output_lots.push(lot);
        }
    }

    fn agent_schedules(&self, money_good: GoodId) -> Vec<(AgentId, AgioSchedule)> {
        let debt_views = self.agent_debt_views();
        self.agent_order
            .iter()
            .map(|index| {
                let agent = &self.agents[*index];
                let view = &debt_views[*index];
                let gold = self.free_gold_after_all_reserves(agent.id);
                (
                    agent.id,
                    agent.derive_agio_schedule_from_claims_at_gold_for_money(
                        gold,
                        &view.receivables,
                        &view.payables,
                        self.tick,
                        money_good,
                    ),
                )
            })
            .collect()
    }

    fn agent_debt_views(&self) -> Vec<AgentDebtView> {
        let mut views = vec![AgentDebtView::default(); self.agents.len()];
        for debt in self.debts.iter().filter(|debt| debt.is_open()) {
            if let CreditLender::Agent(lender) = debt.lender {
                if let Some(index) = self.agent_index_for(lender) {
                    views[index].receivables.push(debt.clone());
                }
            }
            if let Some(index) = self.agent_index_for(debt.borrower) {
                views[index].payables.push(debt.clone());
            }
        }
        for (agent, horizon, due) in self.loan_reservations.future_due_entries() {
            if let Some(index) = self.agent_index_for(*agent) {
                views[index]
                    .payables
                    .push(self.reserved_future_due_contract(*agent, *horizon, *due));
            }
        }
        views
    }

    fn debt_view_for_agent(&self, agent: AgentId) -> AgentDebtView {
        let mut view = AgentDebtView::default();
        for debt in self.debts.iter().filter(|debt| debt.is_open()) {
            if debt.lender == CreditLender::Agent(agent) {
                view.receivables.push(debt.clone());
            }
            if debt.borrower == agent {
                view.payables.push(debt.clone());
            }
        }
        for (entry, horizon, due) in self.loan_reservations.future_due_entries() {
            if *entry == agent {
                view.payables
                    .push(self.reserved_future_due_contract(agent, *horizon, *due));
            }
        }
        view
    }

    fn reserved_future_due_contract(
        &self,
        borrower: AgentId,
        horizon: u8,
        due: Gold,
    ) -> DebtContract {
        DebtContract {
            id: DebtId(0),
            lender: CreditLender::Agent(AgentId(0)),
            borrower,
            opened_tick: self.tick,
            due_tick: Tick(self.tick.0.saturating_add(u64::from(horizon))),
            principal: Gold(1),
            due,
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::Commodity,
        }
    }

    fn schedule_for_agent(&self, agent: AgentId, money_good: GoodId) -> Option<AgioSchedule> {
        let index = self.agent_index_for(agent)?;
        let view = self.debt_view_for_agent(agent);
        let gold = self.free_gold_after_all_reserves(agent);
        Some(
            self.agents[index].derive_agio_schedule_from_claims_at_gold_for_money(
                gold,
                &view.receivables,
                &view.payables,
                self.tick,
                money_good,
            ),
        )
    }

    fn abandon_unviable_projects(
        &mut self,
        schedules: &[(AgentId, AgioSchedule)],
        money_good: GoodId,
    ) {
        let to_abandon = self
            .m2_projects
            .iter()
            .enumerate()
            .filter_map(|(index, project)| {
                self.project_should_abandon(project, schedules, money_good)
                    .then_some(index)
            })
            .collect::<Vec<_>>();

        for project_index in to_abandon {
            let owner = self.m2_projects[project_index].owner;
            let loss = if let Some(agent_index) = self.agent_index_for(owner) {
                let mut salvage = Stock::new(self.max_good_id);
                let project = &mut self.m2_projects[project_index];
                let loss = abandon_project(project, &mut salvage);
                self.return_project_salvage(agent_index, &salvage, money_good);
                loss
            } else {
                let mut discarded_salvage = Stock::new(self.max_good_id);
                abandon_project(&mut self.m2_projects[project_index], &mut discarded_salvage)
            };
            self.capital_labor_consumed = self
                .capital_labor_consumed
                .saturating_add(loss.labor_consumed);
            self.capital_goods_consumed = self
                .capital_goods_consumed
                .saturating_add(loss.goods_consumed);
            self.capital_gold_loss = self.capital_gold_loss.saturating_add(loss.gold_loss);
            self.release_completed_project_reserves();
        }
    }

    fn return_project_salvage(&mut self, agent_index: usize, salvage: &Stock, money_good: GoodId) {
        for good in salvage.positive_goods() {
            let qty = salvage.get(good);
            if qty == 0 {
                continue;
            }
            if good == money_good {
                if !self.credit_agent_money(agent_index, Gold(u64::from(qty))) {
                    self.agents[agent_index].stock.add(good, qty);
                }
            } else {
                self.agents[agent_index].stock.add(good, qty);
            }
        }
    }

    fn project_should_abandon(
        &self,
        project: &M2Project,
        schedules: &[(AgentId, AgioSchedule)],
        money_good: GoodId,
    ) -> bool {
        if !matches!(
            project.state,
            M2ProjectState::Forming | M2ProjectState::Waiting
        ) {
            return false;
        }
        if self.dead_agents.binary_search(&project.owner).is_ok() {
            return false;
        }
        let Some(line) = find_line(&self.project_lines, project.line) else {
            return true;
        };
        if project.state == M2ProjectState::Forming
            && self.tick >= project.maturity
            && project.labor_advanced < line.required_labor
        {
            return true;
        }
        let Some(schedule) = self.schedule_for_agent_from(project.owner, schedules, money_good)
        else {
            return true;
        };
        let horizon = remaining_project_horizon(project, self.tick);
        // No ordinal valuation at this horizon => the future product is
        // unvaluable to the owner, so the project is unviable. Never fall back
        // to face value (a hidden zero-discount path).
        let present_value = schedule
            .present_value(project.expected_revenue, horizon)
            .unwrap_or(Gold::ZERO);
        let Some(owner_index) = self.agent_index_for(project.owner) else {
            return false;
        };
        present_value < project_salvage_value(&self.agents[owner_index], project, Some(money_good))
    }

    fn plan_projects_and_hire(
        &mut self,
        schedules: &[(AgentId, AgioSchedule)],
        money_good: GoodId,
    ) {
        for order_pos in 0..self.agent_order.len() {
            let agent_index = self.agent_order[order_pos];
            if !self.agent_can_own_project(agent_index) {
                continue;
            }
            self.start_funded_project_plan(agent_index);
            self.ensure_project_started(agent_index, schedules, money_good);
            self.post_hire_for_projects(agent_index, schedules);
        }
    }

    fn agent_can_own_project(&self, agent_index: usize) -> bool {
        self.agents[agent_index]
            .roles
            .iter()
            .any(|role| matches!(role, Role::Producer | Role::Capitalist))
    }

    fn ensure_project_started(
        &mut self,
        agent_index: usize,
        schedules: &[(AgentId, AgioSchedule)],
        money_good: GoodId,
    ) {
        let owner = self.agents[agent_index].id;
        if self.owner_has_project(owner) {
            return;
        }
        let Some(schedule) = self.schedule_for_agent_from(owner, schedules, money_good) else {
            return;
        };

        struct ProjectCandidate {
            surplus: u64,
            line: ProjectLine,
            expected_revenue: Gold,
            input_cost_basis: Gold,
            input_goods: Vec<(GoodId, u32)>,
        }

        let mut best: Option<ProjectCandidate> = None;
        for line in &self.project_lines {
            if self.owner_has_active_project_plan_for_line(owner, line.id) {
                continue;
            }
            let expected_revenue =
                expected_revenue_for(&self.agents[agent_index], line, Some(money_good));
            // Skip any line the owner cannot value ordinally at its horizon —
            // no face-value fallback.
            let Some(present_value) = schedule.present_value(expected_revenue, line.period_len)
            else {
                continue;
            };
            let input_cost_basis = input_cost_basis(
                &self.agents[agent_index],
                &line.input_goods,
                Some(money_good),
            );
            let expected_cost =
                Gold(u64::from(line.required_labor)).saturating_add(input_cost_basis);
            if present_value <= expected_cost {
                continue;
            }
            let input_goods = committed_input_goods(line);
            if !input_goods.iter().all(|(good, qty)| {
                self.project_input_available(agent_index, *good, money_good, None) >= *qty
            }) {
                continue;
            }
            if !self.has_feasible_first_project_funding_step(
                agent_index,
                line,
                input_cost_basis,
                present_value,
                None,
            ) {
                continue;
            }
            let surplus = present_value.0 - expected_cost.0;
            let replace = best
                .as_ref()
                .map(|best| {
                    surplus > best.surplus
                        || (surplus == best.surplus
                            && (line.period_len, line.id) > (best.line.period_len, best.line.id))
                })
                .unwrap_or(true);
            if replace {
                best = Some(ProjectCandidate {
                    surplus,
                    line: line.clone(),
                    expected_revenue,
                    input_cost_basis,
                    input_goods,
                });
            }
        }
        let Some(ProjectCandidate {
            line,
            expected_revenue,
            input_cost_basis,
            input_goods,
            ..
        }) = best
        else {
            return;
        };

        let Some(_input_debits) =
            self.commit_project_inputs(agent_index, &input_goods, money_good, None)
        else {
            return;
        };

        let project = start_project(
            owner,
            &line,
            M2ProjectId(self.next_m2_project_id),
            self.tick,
            expected_revenue,
            input_cost_basis,
        );
        self.next_m2_project_id = self.next_m2_project_id.saturating_add(1);
        self.tick_self_funded_project_starts.push((owner, line.id));
        self.m2_projects.push(project);
    }

    fn start_funded_project_plan(&mut self, agent_index: usize) -> bool {
        let owner = self.agents[agent_index].id;
        if self.owner_defaulted_project_funding_debt_this_tick(owner) {
            return false;
        }
        if self.owner_has_project(owner) {
            return false;
        }
        let plan_ids = self
            .project_funding_plans
            .iter()
            .filter(|plan| {
                plan.owner == owner
                    && plan.started_project.is_none()
                    && plan.reserved_gold > Gold::ZERO
                    && plan.expires_tick > self.tick
            })
            .map(|plan| plan.id)
            .collect::<Vec<_>>();
        for plan_id in plan_ids {
            if self.start_project_from_plan(agent_index, plan_id) {
                return true;
            }
        }
        false
    }

    fn start_project_from_plan(&mut self, agent_index: usize, plan_id: ProjectPlanId) -> bool {
        let Some(plan_pos) = self
            .project_funding_plans
            .iter()
            .position(|plan| plan.id == plan_id)
        else {
            return false;
        };
        let plan = self.project_funding_plans[plan_pos].clone();
        let Some(line) = find_line(&self.project_lines, plan.line).cloned() else {
            return false;
        };
        let input_goods = committed_input_goods(&line);
        let money_good = self.legacy_money_good();
        if !input_goods.iter().all(|(good, qty)| {
            self.project_input_available(agent_index, *good, money_good, Some(plan_id)) >= *qty
        }) {
            return false;
        }
        let input_cost_basis = input_cost_basis(
            &self.agents[agent_index],
            &line.input_goods,
            Some(money_good),
        );
        let first_step_value =
            input_cost_basis.saturating_add(Gold(u64::from(line.required_labor)));
        if !self.has_feasible_first_project_funding_step(
            agent_index,
            &line,
            input_cost_basis,
            first_step_value,
            Some(plan_id),
        ) {
            return false;
        }
        let Some(_input_debits) =
            self.commit_project_inputs(agent_index, &input_goods, money_good, Some(plan_id))
        else {
            return false;
        };

        let project_id = M2ProjectId(self.next_m2_project_id);
        let project = start_project(
            plan.owner,
            &line,
            project_id,
            self.tick,
            plan.expected_revenue,
            input_cost_basis,
        );
        self.next_m2_project_id = self.next_m2_project_id.saturating_add(1);
        self.m2_projects.push(project);
        if let Some(plan) = self.project_funding_plans.get_mut(plan_pos) {
            plan.started_project = Some(project_id);
            plan.input_cost_basis = input_cost_basis;
            plan.required_labor = line.required_labor;
        }
        self.link_project_funding_debts(plan_id, project_id);
        true
    }

    fn link_project_funding_debts(&mut self, plan_id: ProjectPlanId, project_id: M2ProjectId) {
        for debt in &mut self.debts {
            if let DebtPurpose::ProjectFunding { plan, project } = &mut debt.purpose {
                if *plan == plan_id {
                    *project = Some(project_id);
                }
            }
        }
        for trade in &mut self.loan_trades {
            if trade.purpose.project_plan() == Some(plan_id) {
                trade.project = Some(project_id);
            }
        }
    }

    fn schedule_for_agent_from(
        &self,
        agent: AgentId,
        schedules: &[(AgentId, AgioSchedule)],
        money_good: GoodId,
    ) -> Option<AgioSchedule> {
        schedules
            .iter()
            .find(|(entry, _)| *entry == agent)
            .map(|(_, schedule)| schedule.clone())
            .or_else(|| self.schedule_for_agent(agent, money_good))
    }

    fn project_input_available(
        &self,
        agent_index: usize,
        good: GoodId,
        money_good: GoodId,
        plan_id: Option<ProjectPlanId>,
    ) -> u32 {
        if good == money_good {
            return self.project_money_input_available(agent_index, money_good, plan_id);
        }
        let agent = &self.agents[agent_index];
        agent
            .stock
            .get(good)
            .saturating_sub(self.reservations.reserved_stock(agent.id, good))
            .saturating_sub(agent.stock_reserved_for_near_wants_for_money(good, money_good))
    }

    fn project_money_input_available(
        &self,
        agent_index: usize,
        money_good: GoodId,
        plan_id: Option<ProjectPlanId>,
    ) -> u32 {
        let agent = &self.agents[agent_index];
        let held = if self.m3_enabled {
            self.money_system
                .as_ref()
                .and_then(|system| system.balance_snapshot(agent.id))
                .map(|balance| balance.public_specie)
                .unwrap_or(Gold::ZERO)
        } else {
            agent.gold
        };
        let reserved = self.reserved_gold_all_except_plan(agent.id, plan_id);
        let near = agent.money_reserved_for_near_wants_for_money(money_good);
        let available = held.saturating_sub(reserved).saturating_sub(near).0;
        u32::try_from(available).unwrap_or(u32::MAX)
    }

    fn commit_project_inputs(
        &mut self,
        agent_index: usize,
        input_goods: &[(GoodId, u32)],
        money_good: GoodId,
        plan_id: Option<ProjectPlanId>,
    ) -> Option<Vec<ProjectInputDebit>> {
        let mut debits = Vec::new();
        for (good, qty) in input_goods {
            if *good == money_good {
                if self.project_input_available(agent_index, *good, money_good, plan_id) < *qty {
                    self.rollback_project_inputs(agent_index, &debits);
                    return None;
                }
                let amount = Gold(u64::from(*qty));
                let (new_gold, ledger_debit) = self.stage_agent_money_debit(agent_index, amount)?;
                if !self.commit_agent_money_debit(agent_index, new_gold, ledger_debit) {
                    self.rollback_project_inputs(agent_index, &debits);
                    return None;
                }
                debits.push(ProjectInputDebit::Money(amount));
            } else if self.agents[agent_index].stock.remove(*good, *qty) {
                debits.push(ProjectInputDebit::Stock(*good, *qty));
            } else {
                self.rollback_project_inputs(agent_index, &debits);
                return None;
            }
        }
        Some(debits)
    }

    fn rollback_project_inputs(&mut self, agent_index: usize, debits: &[ProjectInputDebit]) {
        for debit in debits.iter().rev() {
            match *debit {
                ProjectInputDebit::Stock(good, qty) => {
                    self.agents[agent_index].stock.add(good, qty);
                }
                ProjectInputDebit::Money(amount) => {
                    let credited = self.credit_agent_money(agent_index, amount);
                    debug_assert!(credited);
                }
            }
        }
    }

    fn has_feasible_first_project_funding_step(
        &self,
        agent_index: usize,
        line: &ProjectLine,
        input_cost_basis: Gold,
        present_value: Gold,
        plan_id: Option<ProjectPlanId>,
    ) -> bool {
        if line.required_labor == 0 {
            return true;
        }
        let owner = self.agents[agent_index].id;
        let first_hire_bid = Gold(
            present_value
                .saturating_sub(input_cost_basis)
                .0
                .checked_div(u64::from(line.required_labor))
                .unwrap_or(0),
        );
        if first_hire_bid == Gold::ZERO {
            return false;
        }
        // The actual first hire order is bounded by free gold, so a positive
        // free balance is enough to take the next funding step. This prevents
        // input debit with no funding path while avoiding an all-labor-upfront
        // requirement.
        let free_gold = plan_id
            .map(|plan| self.free_gold_for_project_plan(owner, plan))
            .unwrap_or_else(|| self.free_gold_after_all_reserves(owner));
        let wage_funding = if self.m3_enabled {
            free_gold.min(self.wage_tender_spendable_cap(owner, plan_id))
        } else {
            free_gold
        };
        wage_funding > Gold::ZERO
    }

    fn owner_has_project(&self, owner: AgentId) -> bool {
        self.m2_projects
            .iter()
            .any(|project| project.owner == owner)
    }

    fn owner_has_forming_project(&self, owner: AgentId) -> bool {
        self.m2_projects
            .iter()
            .any(|project| project.owner == owner && project.state == M2ProjectState::Forming)
    }

    fn post_hire_for_projects(
        &mut self,
        agent_index: usize,
        schedules: &[(AgentId, AgioSchedule)],
    ) {
        let owner = self.agents[agent_index].id;
        if self.owner_defaulted_project_funding_debt_this_tick(owner) {
            self.labor_book
                .cancel(owner, FactorSide::Hire, &mut self.labor_reservations);
            return;
        }
        let Some(schedule) = schedules
            .iter()
            .find(|(agent, _)| *agent == owner)
            .map(|(_, schedule)| schedule.clone())
        else {
            return;
        };
        let project_ids = self
            .m2_projects
            .iter()
            .filter(|project| project.owner == owner && project.state == M2ProjectState::Forming)
            .map(|project| project.id)
            .collect::<Vec<_>>();
        for project_id in project_ids {
            if self.labor_book.has_live(owner, FactorSide::Hire) {
                continue;
            }
            let Some(max_wage) = self.project_max_wage(project_id, &schedule) else {
                continue;
            };
            if max_wage == Gold::ZERO {
                continue;
            }
            let plan_id = self.project_plan_for_project(project_id);
            let free_gold = plan_id
                .map(|plan| self.free_gold_for_project_plan(owner, plan))
                .unwrap_or_else(|| self.free_gold_after_all_reserves(owner));
            let wage_funding = if self.m3_enabled {
                free_gold.min(self.wage_tender_spendable_cap(owner, plan_id))
            } else {
                free_gold
            };
            let wage_limit = max_wage.min(wage_funding);
            if wage_limit == Gold::ZERO {
                continue;
            }
            self.seq += 1;
            let order = LaborOrder {
                agent: owner,
                side: FactorSide::Hire,
                wage_limit,
                qty: 1,
                seq: self.seq,
                expires_tick: self.tick.0 + ORDER_TTL,
            };
            let Some(amount) = order.wage_limit.mul_qty(order.qty) else {
                continue;
            };
            if wage_funding < amount {
                continue;
            }
            if !self.labor_reservations.reserve_order(&self.agents, &order) {
                continue;
            }
            let trades = {
                let wage_tender = self.labor_wage_tender;
                let wage_media = if self.m3_enabled {
                    wage_tender.accepted_media()
                } else {
                    LaborWageTender::ParAll.accepted_media()
                };
                let money_system = if self.m3_enabled {
                    self.money_system.as_mut()
                } else {
                    None
                };
                let wage_audit = if self.m3_enabled {
                    Some(&mut self.wage_payment_audit)
                } else {
                    None
                };
                let mut market = LaborMarketView {
                    agents: self.agents.as_mut_slice(),
                    reservations: &mut self.labor_reservations,
                    projects: &mut self.m2_projects,
                    lines: &self.project_lines,
                    money_system,
                    wage_media,
                    wage_audit,
                    wage_tender,
                };
                self.labor_book
                    .add_order(order, Some(project_id), self.tick.0, &mut market)
            };
            self.apply_project_labor_funding(&trades);
            self.record_labor_trades_used(&trades);
            self.labor_trades.extend(trades);
        }
    }

    fn apply_project_labor_funding(&mut self, trades: &[LaborTrade]) {
        for trade in trades {
            let Some(payment) = trade.wage.mul_qty(trade.qty) else {
                continue;
            };
            let Some(plan_id) = self.project_plan_for_project(trade.project) else {
                continue;
            };
            if let Some(plan) = self
                .project_funding_plans
                .iter_mut()
                .find(|plan| plan.id == plan_id)
            {
                plan.reserved_gold = plan.reserved_gold.saturating_sub(payment);
            }
        }
    }

    fn project_max_wage(&self, project_id: M2ProjectId, schedule: &AgioSchedule) -> Option<Gold> {
        let project = self
            .m2_projects
            .iter()
            .find(|project| project.id == project_id)?;
        let line = find_line(&self.project_lines, project.line)?;
        let remaining_labor = line.required_labor.saturating_sub(project.labor_advanced);
        if remaining_labor == 0 {
            return None;
        }
        if let Some(plan_id) = self.project_plan_for_project(project_id) {
            let reserved = self.reserved_project_gold_for_plan(plan_id);
            if reserved > Gold::ZERO {
                return Some(Gold(reserved.0 / u64::from(remaining_labor)).max(Gold(1)));
            }
        }
        let horizon = remaining_project_horizon(project, self.tick);
        // No ordinal valuation => no wage bid (never bid against face value).
        let present_value = schedule.present_value(project.expected_revenue, horizon)?;
        let remaining_surplus = present_value
            .saturating_sub(project.advanced_gold)
            .saturating_sub(project.input_cost_basis);
        Some(Gold(remaining_surplus.0 / u64::from(remaining_labor)))
    }

    fn post_labor_asks(&mut self, money_good: GoodId) {
        let debt_views = self.agent_debt_views();
        for order_pos in 0..self.agent_order.len() {
            let agent_index = self.agent_order[order_pos];
            let agent_id = self.agents[agent_index].id;
            let live_order = self.labor_book.live_order(agent_id, FactorSide::Work);
            if live_order.is_none()
                && self
                    .labor_reservations
                    .reserved_labor(agent_id)
                    .saturating_add(self.tick_labor_used(agent_id))
                    >= self.agents[agent_index].labor_capacity
            {
                continue;
            }
            let view = &debt_views[agent_index];
            let wage_limit = self.agents[agent_index].reservation_labor_ask_from_claims_for_money(
                1,
                &view.receivables,
                &view.payables,
                self.tick,
                money_good,
            );
            match (live_order, wage_limit) {
                (Some(order), Some(wage_limit)) if order.wage_limit == wage_limit => continue,
                (Some(_), wage_limit) => {
                    self.labor_book.cancel(
                        agent_id,
                        FactorSide::Work,
                        &mut self.labor_reservations,
                    );
                    if wage_limit.is_none() {
                        continue;
                    }
                }
                (None, None) => continue,
                (None, Some(_)) => {}
            }
            if self
                .labor_reservations
                .reserved_labor(agent_id)
                .saturating_add(self.tick_labor_used(agent_id))
                >= self.agents[agent_index].labor_capacity
            {
                continue;
            }
            let Some(wage_limit) = wage_limit else {
                continue;
            };
            self.seq += 1;
            let order = LaborOrder {
                agent: agent_id,
                side: FactorSide::Work,
                wage_limit,
                qty: 1,
                seq: self.seq,
                expires_tick: self.tick.0 + ORDER_TTL,
            };
            if !self.labor_reservations.reserve_order(&self.agents, &order) {
                continue;
            }
            let trades = {
                let wage_tender = self.labor_wage_tender;
                let wage_media = if self.m3_enabled {
                    wage_tender.accepted_media()
                } else {
                    LaborWageTender::ParAll.accepted_media()
                };
                let money_system = if self.m3_enabled {
                    self.money_system.as_mut()
                } else {
                    None
                };
                let wage_audit = if self.m3_enabled {
                    Some(&mut self.wage_payment_audit)
                } else {
                    None
                };
                let mut market = LaborMarketView {
                    agents: self.agents.as_mut_slice(),
                    reservations: &mut self.labor_reservations,
                    projects: &mut self.m2_projects,
                    lines: &self.project_lines,
                    money_system,
                    wage_media,
                    wage_audit,
                    wage_tender,
                };
                self.labor_book
                    .add_order(order, None, self.tick.0, &mut market)
            };
            self.record_labor_trades_used(&trades);
            self.labor_trades.extend(trades);
        }
    }

    fn tick_labor_used(&self, agent: AgentId) -> u32 {
        self.tick_labor_used
            .iter()
            .find(|(entry, _)| *entry == agent)
            .map(|(_, labor)| *labor)
            .unwrap_or(0)
    }

    /// The most recent realized spot price for `good` (the last trade's price),
    /// or `None` if no trade in `good` has ever cleared. Read accessor only — it
    /// is the G1 camp's window onto market scarcity (e.g. a harvest shock raising
    /// the FOOD price); it changes no engine behavior.
    pub fn realized_price(&self, good: GoodId) -> Option<Gold> {
        self.realized_prices.get(&good).copied()
    }

    /// Enable the per-tick consumption log (off by default). The G1 `Camp` calls
    /// this once after construction so it can read realized FOOD/WOOD consumption
    /// back via [`Society::consumption_log_last_tick`]. Goldens never enable it,
    /// so the conformance suite stays byte-identical.
    ///
    /// Regime contract: the log is captured by the M1 consume path (`step_m1`),
    /// the V2 direct passes (`step_v2`, both barter and money phases — the G5a
    /// addition), and the M3 consume path (`run_m3_tick` — the G8a addition, so the
    /// spatial sim can run on the M3 ledger and still read its consumed sink back).
    /// Only a **pure-M2** step path (M2 without M3) never touches it. Enabling it on
    /// such a society is therefore not *incorrect* but *inert* — the log simply stays
    /// empty, so a caller that reads it back would see no consumption and (in the
    /// camp's case) replenish no needs. The `debug_assert!` below turns that silent
    /// no-op into a loud failure in debug/test builds; in release builds the assert is
    /// compiled out and the inert-empty-log behavior is the documented fallback. G1
    /// runs M1, G5a runs V2, and G8a runs M3 — all capture the log — so the assert
    /// never fires in practice. Each capture is gated on `consumption_log_enabled`
    /// (which the lab goldens never set), so it is byte-identical for the conformance
    /// suite. A future milestone wanting the log under a pure-M2 regime must extend
    /// the capture in that step path rather than rely on this hook alone.
    pub fn enable_consumption_log(&mut self) {
        debug_assert!(
            self.m3_enabled || !self.m2_enabled,
            "consumption logging records the M1, V2, and M3 consume paths only"
        );
        self.consumption_log_enabled = true;
        self.consumption_log.clear();
    }

    /// The tick-local consumption log captured by [`Society::enable_consumption_log`].
    ///
    /// The log is read-only and populated by the M1, V2, and M3 consume paths. It
    /// is captured before direct-labor provisioning, so direct-labor recipes that
    /// satisfy a current want are not credited here. The G1 `Camp` scans this
    /// conservative slice once per tick to replenish needs without changing any
    /// econ rule or conformance output.
    pub fn consumption_log_last_tick(&self) -> &[(AgentId, GoodId, u32)] {
        &self.consumption_log
    }

    /// S15 own-use cultivation seam: record `qty` units of `good` consumed by `agent`
    /// outside the market consume passes (own-use eating of self-cultivated stock),
    /// appending to the same tick-local consumption log the readback reads. The CALLER
    /// debits the agent's stock; this only logs the consumption so the `sim` need
    /// readback advances hunger from it (a stock debit alone would conserve but never
    /// feed). Must be called AFTER the market step (which clears the log at its start),
    /// so the entry survives into the next tick's readback. A no-op when logging is
    /// disabled (every lab golden), so the emergence/conformance digests are unchanged.
    pub fn record_own_use_consumption(&mut self, agent: AgentId, good: GoodId, qty: u32) {
        if qty == 0 || !self.consumption_log_enabled {
            return;
        }
        self.consumption_log.push((agent, good, qty));
    }

    /// Tick-local labor usage by agent for the most recently completed tick.
    ///
    /// Read accessor only; the engine already maintains this tally for labor
    /// accounting, and `Camp` uses it to make rest depletion follow actual work.
    pub fn labor_used_last_tick(&self) -> &[(AgentId, u32)] {
        &self.tick_labor_used
    }

    /// Record labor performed by a driver-owned phase outside [`Society::step`].
    ///
    /// The spatial settlement uses this for conserved project labor that must happen
    /// before the market step for phase-order reasons, then replays the receipt after
    /// `step()` so the next needs readback sees the same tick-local labor log as
    /// direct recipe production. Existing econ scenarios never call it.
    pub fn record_external_labor_used(&mut self, agent: AgentId, labor: u32) {
        if labor > 0 {
            self.add_tick_labor_used(agent, labor);
        }
    }

    /// Cancel any resting spot quotes for `agent` whose reservation no longer
    /// matches its current scale, holdings, or tender state.
    ///
    /// G1's `Camp` overwrites value scales between econ ticks. Calling this
    /// immediately after a scale rewrite releases stale reservations before the
    /// next consume phase, so an agent's old ask cannot hide FOOD/WOOD it now
    /// needs to consume. Additive; no engine path calls it by default.
    pub fn cancel_changed_live_quotes_for_agent(&mut self, agent: AgentId) -> bool {
        if self.agents.position_of(agent).is_none() {
            return false;
        }
        self.cancel_changed_live_quotes_for_agents(&[agent]);
        true
    }

    /// Credit `qty` units of `good` to a live agent's stock, returning `false`
    /// for an unknown, stale, or removed `agent` (then nothing is credited).
    ///
    /// The G2b world→econ transfer seam (`docs/impl-g2b.md`). The `sim` crate's
    /// two-rate loop hauls a physical good through `world` (node → carry →
    /// exchange stockpile) and, once per econ tick, **relocates** the units it
    /// withdrew from that stockpile into the depositing colonist's econ stock
    /// via this accessor — the only crossing of the world↔econ boundary, and it
    /// is net-zero (world −n, econ +n). It is **purely additive**: it touches no
    /// scale, quote, money balance, or market state and is called by no engine
    /// path, so the conformance goldens are byte-identical (the proof is the
    /// unchanged suite). A removed colonist (G4a real death) frees its arena slot,
    /// so its stale id resolves to `None` and is rejected by the `position_of`
    /// guard; the `dead_agents` lookup is a cheap defensive backstop.
    ///
    /// A driver must only ever credit goods it has just removed from the world,
    /// and only up to the agent's remaining headroom — the stock add saturates at
    /// the `u32` ceiling — or conservation is broken on its side of the seam
    /// (`sim`'s transfer clamps to that headroom and leaves any remainder in the
    /// world stockpile, never losing it).
    pub fn credit_stock(&mut self, agent: AgentId, good: GoodId, qty: u32) -> bool {
        let Some(position) = self.agents.position_of(agent) else {
            return false;
        };
        // A resolved id is live by construction (a removed agent's slot is freed, so
        // its id resolves to None above); the dead-agents lookup is a defensive
        // backstop, and the list is sorted for binary search.
        if self.dead_agents.binary_search(&agent).is_ok() {
            return false;
        }
        self.agents[position].stock.add(good, qty);
        true
    }

    /// Debit `qty` units of `good` from a live agent's stock, returning `false`
    /// for an unknown, stale, or removed `agent`, **or** when the agent holds
    /// fewer than `qty` (then nothing is debited — the stock never goes
    /// negative). The withdrawing mirror of [`Society::credit_stock`].
    ///
    /// The G2c caravan seam (`docs/impl-g2c.md`). A `Region` shuttles a resident
    /// trader's wealth between two settlements: it debits the trader here and
    /// credits the same amount into its own route escrow, so the move is
    /// **net-zero** across the `[society ∪ escrow]` ledger — value is moved, never
    /// minted or burned. Like `credit_stock` it is **purely additive**: it touches
    /// no scale, quote, money balance, or market state and is called by no engine
    /// path, so the conformance goldens are byte-identical. The atomic
    /// [`Stock::remove`] is the never-negative guarantee.
    ///
    /// Caller contract (the seam): a driver must debit only an agent it has first
    /// quieted — a removed id is rejected here, and a live trader must have its
    /// resting quotes cancelled (its reservations released) before its stock or
    /// gold is debited, or a stale order could reference goods the agent no longer
    /// holds. `Region` clears a departing trader's scale and cancels its quotes
    /// before withdrawing its wealth into escrow.
    pub fn debit_stock(&mut self, agent: AgentId, good: GoodId, qty: u32) -> bool {
        let Some(position) = self.agents.position_of(agent) else {
            return false;
        };
        if self.dead_agents.binary_search(&agent).is_ok() {
            return false;
        }
        self.agents[position].stock.remove(good, qty)
    }

    /// Whether this society uses the legacy **closed-GOLD M1** money model — the
    /// only regime the gold accessors may touch. True iff there is no
    /// [`MoneySystem`] ledger (so [`Society::total_gold`] sums `Agent.gold`, making
    /// the gold field itself the money) **and** the market-money regime is
    /// `Designated`-GOLD.
    ///
    /// It is false for ledger-backed M3 societies (the `MoneySystem`, not the agent
    /// cache, is the source of truth) and — the case `money_system.is_some()` alone
    /// misses — for emergent-money regimes such as `MengerSaltMoney`, where the
    /// circulating medium is a *good* held in stock and keeps no `MoneySystem`, so
    /// `Agent.gold` is not money at all. Crediting `Agent.gold` there would mint a
    /// phantom balance that [`Society::total_gold`] would wrongly count.
    fn uses_closed_gold_money(&self) -> bool {
        self.money_system.is_none()
            && matches!(
                &self.money,
                MarketMoneyState::Designated(money) if money.money_good() == GOLD
            )
    }

    /// Credit `amount` gold to a live agent's balance, returning `false` for an
    /// unknown, stale, or removed `agent` or on overflow (then nothing is
    /// credited). The gold analog of [`Society::credit_stock`] and the depositing
    /// half of the G2c caravan seam (see [`Society::debit_stock`]).
    ///
    /// Operates only on the legacy closed-money model the `sim` settlement uses
    /// (a `Designated`-GOLD M1 society with no `money_system`), where
    /// [`Society::total_gold`] sums agent gold, so a `Region` can account a paired
    /// debit/credit as a conserved move. Both ledger-backed (M3) and emergent-money
    /// regimes are rejected up front via [`Society::uses_closed_gold_money`] —
    /// in neither is `Agent.gold` the circulating money, so touching it would
    /// desynchronize a `MoneySystem` or mint a phantom non-money balance.
    pub fn credit_gold(&mut self, agent: AgentId, amount: Gold) -> bool {
        if !self.uses_closed_gold_money() {
            return false;
        }
        let Some(position) = self.agents.position_of(agent) else {
            return false;
        };
        if self.dead_agents.binary_search(&agent).is_ok() {
            return false;
        }
        let Some(updated) = self.agents[position].gold.checked_add(amount) else {
            return false;
        };
        self.agents[position].gold = updated;
        true
    }

    /// Debit `amount` gold from a live agent's balance, returning `false` for an
    /// unknown, stale, or removed `agent`, **or** when the agent holds less
    /// than `amount` (then nothing is debited — the balance never goes negative).
    /// The withdrawing mirror of [`Society::credit_gold`]: legacy closed-GOLD M1
    /// societies only (see [`Society::uses_closed_gold_money`]), rejecting both
    /// ledger-backed and emergent-money regimes before any mutation. See
    /// [`Society::debit_stock`] for the caller contract.
    pub fn debit_gold(&mut self, agent: AgentId, amount: Gold) -> bool {
        if !self.uses_closed_gold_money() {
            return false;
        }
        let Some(position) = self.agents.position_of(agent) else {
            return false;
        };
        if self.dead_agents.binary_search(&agent).is_ok() {
            return false;
        }
        let Some(updated) = self.agents[position].gold.checked_sub(amount) else {
            return false;
        };
        self.agents[position].gold = updated;
        true
    }

    /// Transfer **spendable** gold-denominated money between two live agents without
    /// minting or burning. In closed-GOLD M1 this moves the raw `Agent.gold` field;
    /// in M3 it delegates to the [`MoneySystem`] and reconciles the affected agent
    /// caches. Emergent-money regimes reject the helper because `Agent.gold` is not
    /// the circulating medium there.
    ///
    /// "Spendable" is load-bearing: the transfer refuses to move money that a resting
    /// order, loan, labor quote, or project plan has already reserved, on **either**
    /// the M3 or the closed-GOLD path. Moving reserved gold would leave the source
    /// over-committed (`reserved_gold(from) > from.gold`), so a later clear or cancel
    /// would operate on an order it can no longer fund. A self-transfer is a no-op and
    /// needs no headroom.
    pub fn transfer_gold(&mut self, from: AgentId, to: AgentId, amount: Gold) -> bool {
        let Some(from_position) = self.agents.position_of(from) else {
            return false;
        };
        let Some(to_position) = self.agents.position_of(to) else {
            return false;
        };
        if self.dead_agents.binary_search(&from).is_ok()
            || self.dead_agents.binary_search(&to).is_ok()
        {
            return false;
        }
        if amount == Gold::ZERO {
            return true;
        }
        // Reservation guard (both regimes): never move gold already earmarked by a
        // resting order / loan / labor quote / project plan. The closed-GOLD raw-balance
        // check below and the M3 ledger's own spendable check are both blind to these
        // Society-level reservations, so the helper enforces its "spendable" contract here.
        if from != to && self.free_gold_after_all_reserves(from) < amount {
            return false;
        }

        if let Some(money_system) = self.money_system.as_mut() {
            if money_system.transfer_spendable(from, to, amount).is_err() {
                return false;
            }
            let agents = self.agents.as_mut_slice();
            money_system.reconcile_agent_cache_at(agents, from_position);
            money_system.reconcile_agent_cache_at(agents, to_position);
            return true;
        }

        if !self.uses_closed_gold_money() {
            return false;
        }
        if from_position == to_position {
            return self.agents[from_position].gold >= amount;
        }

        let from_gold = self.agents[from_position].gold;
        if from_gold < amount {
            return false;
        }
        let to_gold = self.agents[to_position].gold;
        let Some(updated_to) = to_gold.checked_add(amount) else {
            return false;
        };
        let updated_from = from_gold
            .checked_sub(amount)
            .expect("the balance was preflighted");
        self.agents[from_position].gold = updated_from;
        self.agents[to_position].gold = updated_to;
        true
    }

    /// Credit `gold` to a live agent as a conserved **estate hand-off** (G4a/G4b/G8a
    /// inheritance). In closed-GOLD M1 this adds to the recipient's `Agent.gold`; in M3
    /// it re-credits the public **specie** that [`Society::remove_agent`] drained out of
    /// the [`MoneySystem`] at the dead colonist's death, so `commodity_base` returns to
    /// its pre-death total, and reconciles the recipient's cache. It is the heir-side
    /// mirror of removal's estate drain: the two together move the estate from the dead
    /// colonist to a survivor without minting or burning.
    ///
    /// Unlike [`Society::transfer_gold`] there is no reservation guard and no
    /// `uses_closed_gold_money` gate — the source is the already-removed estate, not a
    /// live agent that could over-commit a resting order, so restoring it is conserved
    /// on **any** regime where `Agent.gold` carries money (closed-GOLD M1 *and*
    /// post-promotion emergent money, where the dead colonist's gold was real). Returns
    /// `false` (crediting nothing) for an unknown / stale / dead recipient or an
    /// overflow. A zero amount is a no-op success (pre-promotion barter estates are
    /// zero-gold, so they take this path harmlessly).
    pub fn credit_estate_gold(&mut self, agent: AgentId, gold: Gold) -> bool {
        if gold == Gold::ZERO {
            return true;
        }
        let Some(position) = self.agents.position_of(agent) else {
            return false;
        };
        if self.dead_agents.binary_search(&agent).is_ok() {
            return false;
        }
        if let Some(money_system) = self.money_system.as_mut() {
            if money_system.credit_specie(agent, gold).is_err() {
                return false;
            }
            money_system.reconcile_agent_cache_at(self.agents.as_mut_slice(), position);
            return true;
        }
        let Some(updated) = self.agents[position].gold.checked_add(gold) else {
            return false;
        };
        self.agents[position].gold = updated;
        true
    }

    fn record_consumed_provisions(&mut self, agent_index: usize, provisions: &TickProvisions) {
        if !self.consumption_log_enabled && self.metric_observation_accumulator.is_none() {
            return;
        }
        let agent_id = self.agents[agent_index].id;
        // `provisions.allocated` is built index-parallel to `agent.scale`
        // (`vec![0; scale.len()]`), so position i is the allocation for want i.
        // The assert pins it so a future reordering of the consume phase fails
        // loudly rather than silently misattributing consumption.
        debug_assert_eq!(
            self.agents[agent_index].scale.len(),
            provisions.allocated.len(),
            "consumption capture requires scale/allocated index-parallelism"
        );
        for (want, qty) in self.agents[agent_index]
            .scale
            .iter()
            .zip(&provisions.allocated)
        {
            if *qty == 0 {
                continue;
            }
            if let WantKind::Good(good) = want.kind {
                if self.consumption_log_enabled {
                    self.consumption_log.push((agent_id, good, *qty));
                }
                if self.metric_observation_accumulator.is_some() {
                    self.metric_consumed_goods.push((agent_id, good, *qty));
                }
            }
        }
    }

    /// Real death (G4a, game-spec §5.6): remove `agent` from the running society,
    /// returning its settled [`Estate`] (gold + econ stock) for the caller to route
    /// to the settlement commons or to heirs — a conserved hand-off, nothing created
    /// or destroyed. Returns `None` for an unknown, stale, or already-removed id, or
    /// for an M3 agent still holding fiat/demand-claims (banks/fiat are G8b/c); then
    /// nothing changes. A funded **specie** M3 balance is NOT refused (G8a): step 1
    /// drains it into the returned `Estate` (see below).
    ///
    /// This is the G4a successor to G1's freeze-in-place tombstone, now retired
    /// (`docs/engine-divergence.md`). The order of operations is load-bearing:
    ///
    /// 1. **SETTLE** the estate — extract the agent's gold and econ stock into the
    ///    returned [`Estate`], emptying its value scale and zeroing labor capacity
    ///    in the same step so the teardown posts nothing.
    /// 2. **CANCEL** its market presence — cancel every resting spot quote, barter
    ///    offer, labor order, and loan order, releasing each one's reservation. This
    ///    MUST run before the free, or freeing would strand a reservation against a
    ///    slot the arena no longer resolves.
    /// 3. **FREE** the arena slot — [`AgentArena::free`] bumps the slot generation,
    ///    so the dead id resolves to `None` and the slot becomes reusable.
    /// 4. **RECONCILE** the external position/id caches — rebuild `agent_order` so
    ///    no entry points at the freed or a relocated arena position, and forget the
    ///    dead id from every reservation/cache so none dangles.
    ///
    /// The dead id is recorded in `dead_agents` so any capital project or open debt
    /// it still owns stays frozen (heirs/capital inheritance are G4b), matching G1's
    /// freeze for that holdings class. The reconciliation is deterministic
    /// (id-ordered, integer, draws nothing). No lab scenario frees an agent, so this
    /// whole path is game-only and the six econ goldens are byte-identical by
    /// construction.
    ///
    /// Caller contract: a driver owns the dead/alive bookkeeping. After this returns
    /// the id resolves to `None`, so a driver MUST stop scaling, endowing, or reading
    /// it back (every `sim`/`life` driver mirrors removal in a per-colonist `alive`
    /// flag checked in each phase). It settles the **raw agent holdings** (the gold
    /// field + stock): in closed-GOLD M1 that gold field IS the money; in M3 the gold
    /// field is the reconciled cache of the ledger specie, and step 1 additionally
    /// drains that specie out of the [`MoneySystem`] into the same `Estate` so the
    /// hand-off is conserved on the ledger too (G8a — the resolved G4a/b deferral).
    ///
    /// Preflight for callers that need death bookkeeping to be transactional: `true`
    /// means `remove_agent` will run to completion. In G8a a funded M3 (ledger-money)
    /// agent IS removable — `remove_agent` drains its public **specie** into the
    /// returned [`Estate`], the conserved resolution of the G4a/b deferral. Only fiat
    /// and demand claims remain deferred (banks/fiat are G8b/G8c), so an agent holding
    /// either is still refused — there is no conserved estate route for them yet.
    pub fn can_remove_agent(&self, agent: AgentId) -> bool {
        if self.agents.position_of(agent).is_none() {
            return false;
        }
        if self.dead_agents.binary_search(&agent).is_ok() {
            return false;
        }
        if let Some(balance) = self
            .money_system
            .as_ref()
            .and_then(|money_system| money_system.balance_snapshot(agent))
        {
            return balance.public_fiat == Gold::ZERO
                && balance.demand_claims_total() == Gold::ZERO;
        }
        true
    }

    pub fn remove_agent(&mut self, agent: AgentId) -> Option<Estate> {
        let position = self.agents.position_of(agent)?;
        if !self.can_remove_agent(agent) {
            return None;
        }

        // 1. SETTLE the estate: extract gold + stock and quiet the agent. An empty
        // scale yields no reservations, so the order/quote machinery posts nothing.
        let max_good_id = self.max_good_id;
        let estate = {
            let dead = &mut self.agents[position];
            let gold = std::mem::replace(&mut dead.gold, Gold::ZERO);
            let stock = std::mem::replace(&mut dead.stock, Stock::new(max_good_id));
            dead.scale.clear();
            dead.labor_capacity = 0;
            Estate { gold, stock }
        };

        // 2. CANCEL market presence + release reservations, BEFORE the free. With
        // an empty scale (and now-zero holdings) every resting quote reports stale,
        // so the spot/labor/loan/barter cancellations clear all of the agent's
        // orders and un-earmark its reservations.
        self.cancel_changed_live_quotes_for_agents(&[agent]);
        while self
            .labor_book
            .cancel(agent, FactorSide::Work, &mut self.labor_reservations)
        {}
        while self
            .labor_book
            .cancel(agent, FactorSide::Hire, &mut self.labor_reservations)
        {}
        self.loan_book
            .cancel_agent(agent, &mut self.loan_reservations);
        self.barter_book.forget_agent(agent);
        self.freeze_project_funding_plans_for_dead_owner(agent);

        // Record the dead id so a project/debt it still owns stays frozen (heirs and
        // capital are G4b). Insert in sorted position (the `position_of` guard above
        // already rejects a repeat removal, so the id is never present yet). Inert in
        // M1/lab, which has no projects/debts and never frees, so the list stays
        // empty and every guard's binary-search is a no-op — the goldens are
        // byte-identical.
        if let Err(slot) = self.dead_agents.binary_search(&agent) {
            self.dead_agents.insert(slot, agent);
        }

        // 3. FREE the arena slot (order-preserving: later live agents slide down one
        // slot, the freed id resolves to `None`).
        self.agents.free(agent);

        // 4. RECONCILE the external caches against the freed/relocated positions.
        self.reconcile_agent_order_after_free(position);
        self.reservations.forget_agent(agent);
        self.labor_reservations.forget_agent(agent);
        self.loan_reservations.forget_agent(agent);
        // A ledger-money (M3) society keys a balance by agent id. G8a resolves the
        // G4a/b deferral for **specie**: drain the dead agent's public specie into the
        // `Estate` (a conserved hand-off — `commodity_base` falls by exactly the
        // drained amount, which the caller routes to the commons or an heir), zeroing
        // the ledger row so `forget_agent`'s empty-balance invariant holds and the
        // money invariant's "every balance has a live agent" check stays true. The
        // `Estate.gold` already holds this colonist's cached spendable total, which
        // equals its public specie for a removable agent (fiat/claims are refused by
        // `can_remove_agent` — banks/fiat are G8b/c). In closed-GOLD M1 the estate
        // lives in `Estate.gold` directly and there is no ledger to touch.
        if let Some(money_system) = self.money_system.as_mut() {
            if estate.gold > Gold::ZERO {
                money_system
                    .debit_specie(agent, estate.gold)
                    .expect("a removable M3 agent's specie equals its Estate gold");
            }
            money_system.forget_agent(agent);
        }

        Some(estate)
    }

    fn freeze_project_funding_plans_for_dead_owner(&mut self, owner: AgentId) {
        for plan in &mut self.project_funding_plans {
            if plan.owner != owner {
                continue;
            }
            plan.reserved_gold = Gold::ZERO;
            if plan.started_project.is_none() {
                plan.expires_tick = self.tick;
            }
        }
    }

    /// Rebuild `agent_order` after [`AgentArena::free`] removed the agent at
    /// `freed_position` (G4a real death). The free is order-preserving — every later
    /// live agent slid down exactly one slot — so dropping the freed position and
    /// decrementing every entry past it reproduces the surviving agents in their
    /// unchanged priority order, with no entry pointing at a freed or relocated slot.
    /// Deterministic: integer, order-stable, draws nothing.
    fn reconcile_agent_order_after_free(&mut self, freed_position: usize) {
        self.agent_order
            .retain(|&position| position != freed_position);
        for position in &mut self.agent_order {
            if *position > freed_position {
                *position -= 1;
            }
        }
    }

    /// Real birth (G4b, game-spec §5.6): insert `agent` into the running society
    /// and reconcile every external cache so it participates from the next econ
    /// tick, returning its assigned [`AgentId`] (a fresh or reused arena slot with
    /// a fresh generation). This is the exact insert-side **mirror** of
    /// [`Society::remove_agent`]: where removal frees a slot and forgets the dead id
    /// from every cache, birth inserts a slot and extends every cache for the new id.
    ///
    /// The reconciliation is the load-bearing work — a missed cache is a colonist
    /// that cannot trade:
    ///
    /// 1. **INSERT** into the arena — [`AgentArena::insert`] appends the agent at the
    ///    end of the dense live slice (a reused numeric index carries the bumped
    ///    generation the free recorded, so a stale ancestor id stays `None`), and
    ///    assigns the new id. Existing agents never relocate, so no other cache's
    ///    positions shift.
    /// 2. **RECONCILE `agent_order`** — append the new agent's (last) position, so the
    ///    per-tick activation loop iterates it. Without this the newborn is never
    ///    activated and posts no orders.
    /// 3. **EXTEND the reservation/money caches** — materialize the new id's (empty)
    ///    spot reservation slot via [`Reservations::ensure_agent_slot`], the mirror of
    ///    the `forget_agent` removal does. Ledger-backed M3 societies also get an
    ///    empty [`MoneySystem`] row for the new live id and reconcile `Agent.gold`
    ///    from that ledger row. The id-keyed labor/loan reservation tables hold only
    ///    nonzero entries by invariant, and a newborn reserves nothing, so they need
    ///    no eager slot — the lazy `reserve_order` adds one on the agent's first order,
    ///    which is the insert-side extension for those tables.
    ///
    /// Determinism: integer, order-stable, draws nothing (the caller supplies a fully
    /// formed `Agent`; any culture mutation or birth decision is made deterministically
    /// upstream, never by an `Rng` in this path). No lab scenario adds an agent at
    /// runtime, so this whole path is game-only and the six econ goldens are
    /// byte-identical by construction.
    ///
    /// Caller contract: the supplied `agent`'s `id` is ignored and overwritten by the
    /// arena (the slot owns identity). A driver mirrors the new id in its own
    /// per-colonist bookkeeping exactly as it mirrors removal. The agent's holdings
    /// must be a conserved transfer the driver has already debited from elsewhere
    /// (a household/commons), never a mint — `add_agent` moves no gold or goods of its
    /// own; it only installs the agent the caller endowed. In M3, install the agent
    /// with zero ledger money and then use [`Society::transfer_gold`] to move any
    /// birth endowment from the parent/household.
    pub fn add_agent(&mut self, agent: Agent) -> AgentId {
        // 1. INSERT: the arena appends at the end of the live slice and assigns the id.
        let id = self.agents.insert(agent);
        let position = self
            .agents
            .position_of(id)
            .expect("a just-inserted id resolves in the arena");

        // 2. RECONCILE agent_order: the insert appended the agent, so its position is
        // the new last slot — append it so the activation loop reaches the newborn.
        self.reconcile_agent_order_after_insert(position);

        // 3. EXTEND the reservation caches for the new id (the mirror of remove's
        // forget_agent). The spot table keeps an id-keyed slot per live agent; seed an
        // empty one now. The labor/loan tables are nonzero-only Vecs, so a newborn
        // (which reserves nothing) needs no eager slot — its first order lazily adds it.
        self.reservations.ensure_agent_slot(id);
        if let Some(money_system) = self.money_system.as_mut() {
            // M3 keeps money in the ledger, not in `Agent.gold`. Add an empty ledger
            // row for the new live id and reconcile the cache; any birth endowment
            // must arrive through `transfer_gold` after the caller has chosen the
            // funding source.
            money_system.ensure_agent_balance(id);
            money_system.reconcile_agent_cache_at(self.agents.as_mut_slice(), position);
        }

        id
    }

    /// Append the freshly inserted agent's `position` to `agent_order` (G4b birth) —
    /// the insert-side counterpart of [`Society::reconcile_agent_order_after_free`].
    /// [`AgentArena::insert`] always appends at the end of the dense slice without
    /// relocating any existing agent, so every existing `agent_order` entry stays
    /// valid and the new agent's position is simply pushed (it activates last in the
    /// tick, which is correct for a newborn). Deterministic: integer, draws nothing.
    fn reconcile_agent_order_after_insert(&mut self, inserted_position: usize) {
        self.agent_order.push(inserted_position);
    }

    fn capture_metric_observation(&mut self, tick_trades: &[Trade]) {
        if self.metric_observation_accumulator.is_none() {
            return;
        }

        let labor_capacity = self.agents.as_slice().iter().fold(0u32, |total, agent| {
            total.saturating_add(agent.labor_capacity)
        });
        let labor_used = self
            .tick_labor_used
            .iter()
            .fold(0u32, |total, (_, used)| total.saturating_add(*used));
        #[cfg(debug_assertions)]
        {
            let labor_used_by_agent = self.agents.as_slice().iter().fold(0u32, |total, agent| {
                total.saturating_add(self.tick_labor_used(agent.id))
            });
            debug_assert_eq!(labor_used, labor_used_by_agent);
        }
        debug_assert!(labor_used <= labor_capacity);

        let Some(money_good) = self.money.current_money_good() else {
            self.metric_consumed_goods.clear();
            return;
        };
        let accumulator = self
            .metric_observation_accumulator
            .as_mut()
            .expect("metric accumulator exists after early return");
        let observation = accumulator.observe(MetricObservationInput {
            tick: self.tick.0,
            agents: self.agents.as_slice(),
            initial_agents: &self.initial_agents,
            money_system: self.money_system.as_ref(),
            receipts: &self.cantillon_receipts,
            trades: &self.trades,
            tick_trades,
            consumed_goods: &self.metric_consumed_goods,
            money_good,
            stock_goods: &self.market_goods,
            labor_capacity,
            labor_used,
        });
        self.metric_observations.push(observation);
        self.metric_consumed_goods.clear();
    }

    fn capture_money_audit(&mut self) {
        if !self.money_audit_enabled {
            return;
        }
        let Some(money_system) = &self.money_system else {
            return;
        };
        let mut rows = self
            .agents
            .iter()
            .map(|agent| {
                let balance = money_system.balance_snapshot(agent.id);
                let public_specie = balance
                    .as_ref()
                    .map(|balance| balance.public_specie)
                    .unwrap_or(Gold::ZERO);
                let public_fiat = balance
                    .as_ref()
                    .map(|balance| balance.public_fiat)
                    .unwrap_or(Gold::ZERO);
                let demand_claims = balance
                    .as_ref()
                    .map(|balance| balance.demand_claims_total())
                    .unwrap_or(Gold::ZERO);
                let spendable_money = balance
                    .as_ref()
                    .map(|balance| balance.spendable_total())
                    .unwrap_or(Gold::ZERO);
                MoneyAuditRecord {
                    tick: self.tick.0,
                    agent: agent.id,
                    public_specie,
                    public_fiat,
                    demand_claims,
                    spendable_money,
                }
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| row.agent);
        self.money_audit.extend(rows);
    }

    fn capture_bank_audit(&mut self) {
        if !self.bank_audit_enabled {
            return;
        }
        let mut rows = self
            .banks
            .iter()
            .map(|bank| BankAuditRecord {
                tick: self.tick.0,
                bank: bank.id,
                reserves: bank.reserves,
                demand_deposits: bank.demand_deposits,
                time_deposits: bank.time_deposits,
                loans_outstanding: bank.loans_outstanding,
                fiduciary_issued: bank.fiduciary_issued,
                reserve_ratio_bps: bank.reserve_ratio_bps,
                convertible: bank.convertible,
                policy_enabled: bank.policy.enabled,
                policy_max_new_fiduciary_per_tick: bank.policy.max_new_fiduciary_per_tick,
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| row.bank);
        self.bank_audit.extend(rows);
    }

    fn add_tick_labor_used(&mut self, agent: AgentId, labor: u32) {
        if let Some((_, used)) = self
            .tick_labor_used
            .iter_mut()
            .find(|(entry, _)| *entry == agent)
        {
            *used = used.saturating_add(labor);
        } else {
            self.tick_labor_used.push((agent, labor));
            self.tick_labor_used.sort_by_key(|(entry, _)| *entry);
        }
    }

    fn record_labor_trades_used(&mut self, trades: &[LaborTrade]) {
        for trade in trades {
            self.add_tick_labor_used(trade.worker, trade.qty);
        }
    }

    fn post_loan_orders(&mut self, schedules: &[(AgentId, AgioSchedule)], money_good: GoodId) {
        for (agent, initial_schedule) in schedules {
            if self.owner_has_forming_project(*agent) {
                continue;
            }
            let mut schedule = initial_schedule.clone();
            for horizon in [1u8, 2, 4, 7] {
                if let Some(future_limit) = schedule.min_future_due_for_lending(Gold(1), horizon) {
                    if self.post_loan_order(
                        *agent,
                        LoanSide::Lend,
                        future_limit,
                        horizon,
                        LoanPurpose::Consumption,
                    ) {
                        let Some(updated) = self.schedule_for_agent(*agent, money_good) else {
                            break;
                        };
                        schedule = updated;
                    }
                }
            }
        }
        self.post_project_funding_borrows(money_good);
        for (agent, initial_schedule) in schedules {
            let mut schedule = initial_schedule.clone();
            for horizon in [1u8, 2, 4, 7] {
                if let Some(future_limit) = schedule.max_future_due_for_borrowing(Gold(1), horizon)
                {
                    if self.post_loan_order(
                        *agent,
                        LoanSide::Borrow,
                        future_limit,
                        horizon,
                        LoanPurpose::Consumption,
                    ) {
                        let Some(updated) = self.schedule_for_agent(*agent, money_good) else {
                            break;
                        };
                        schedule = updated;
                    }
                }
            }
        }
    }

    fn post_loan_orders_m3(&mut self, schedules: &[(AgentId, AgioSchedule)], money_good: GoodId) {
        for (agent, initial_schedule) in schedules {
            if self.owner_has_forming_project(*agent) {
                continue;
            }
            let mut schedule = initial_schedule.clone();
            for horizon in [1u8, 2, 4, 7] {
                if let Some(future_limit) = schedule.min_future_due_for_lending(Gold(1), horizon) {
                    if self.post_loan_order(
                        *agent,
                        LoanSide::Lend,
                        future_limit,
                        horizon,
                        LoanPurpose::Consumption,
                    ) {
                        let Some(updated) = self.schedule_for_agent(*agent, money_good) else {
                            break;
                        };
                        schedule = updated;
                    }
                }
            }
        }
        self.post_bank_lend_orders(schedules, money_good);
        self.post_issuer_lend_orders(schedules, money_good);
        self.post_project_funding_borrows(money_good);
        self.post_consumption_borrows(schedules, money_good);
    }

    fn post_consumption_borrows(
        &mut self,
        schedules: &[(AgentId, AgioSchedule)],
        money_good: GoodId,
    ) {
        for (agent, initial_schedule) in schedules {
            let mut schedule = initial_schedule.clone();
            for horizon in [1u8, 2, 4, 7] {
                if let Some(future_limit) = schedule.max_future_due_for_borrowing(Gold(1), horizon)
                {
                    if self.post_loan_order(
                        *agent,
                        LoanSide::Borrow,
                        future_limit,
                        horizon,
                        LoanPurpose::Consumption,
                    ) {
                        let Some(updated) = self.schedule_for_agent(*agent, money_good) else {
                            break;
                        };
                        schedule = updated;
                    }
                }
            }
        }
    }

    fn post_bank_lend_orders(&mut self, schedules: &[(AgentId, AgioSchedule)], money_good: GoodId) {
        let bank_ids = self.banks.iter().map(|bank| bank.id).collect::<Vec<_>>();
        for bank in bank_ids {
            while self.post_bank_lend_order(bank, schedules, money_good) {}
        }
    }

    fn post_bank_lend_order(
        &mut self,
        bank_id: BankId,
        schedules: &[(AgentId, AgioSchedule)],
        money_good: GoodId,
    ) -> bool {
        let Some((bank, present, future_limit, horizon, lender, funding)) =
            self.banks.iter().find_map(|bank| {
                if bank.id != bank_id {
                    return None;
                }
                let (present, future_limit) =
                    one_unit_policy_quote(bank.policy.loan_present, bank.policy.loan_future_due)?;
                let capacity = self
                    .loan_reservations
                    .bank_fiduciary_capacity(bank, self.regime);
                let lender = CreditLender::Bank(bank.id);
                let funding = CreditSource::BankFiduciary(bank.id);
                if capacity < present {
                    return None;
                }
                let live_supply = self.loan_book.live_lender_order_count(
                    lender,
                    funding,
                    present,
                    future_limit,
                    bank.policy.loan_horizon,
                );
                let demand = self.policy_lend_order_demand(
                    present,
                    future_limit,
                    bank.policy.loan_horizon,
                    schedules,
                    money_good,
                );
                if live_supply >= demand {
                    return None;
                }
                Some((
                    bank.id,
                    present,
                    future_limit,
                    bank.policy.loan_horizon,
                    lender,
                    funding,
                ))
            })
        else {
            return false;
        };
        self.seq += 1;
        let order = LoanOrder {
            agent: bank_order_agent(bank),
            lender,
            side: LoanSide::Lend,
            present,
            future_limit,
            horizon,
            seq: self.seq,
            expires_tick: self.tick.0 + ORDER_TTL,
            purpose: LoanPurpose::Consumption,
            funding,
        };
        if !self.loan_reservations.reserve_order_m3(
            &self.agents,
            &order,
            &self.banks,
            &self.issuers,
            self.regime,
        ) {
            return false;
        }
        let trades = {
            let money_system = self
                .money_system
                .as_mut()
                .expect("M3 bank order requires money system");
            self.loan_book.add_order_m3(
                order,
                self.tick.0,
                LoanM3Context {
                    agents: self.agents.as_mut_slice(),
                    reservations: &mut self.loan_reservations,
                    debts: &mut self.debts,
                    next_debt_id: &mut self.next_debt_id,
                    money_system,
                    banks: &mut self.banks,
                    issuers: &mut self.issuers,
                    regime: self.regime,
                },
            )
        };
        self.apply_project_loan_trades(&trades);
        self.loan_trades.extend(trades);
        true
    }

    fn post_issuer_lend_orders(
        &mut self,
        schedules: &[(AgentId, AgioSchedule)],
        money_good: GoodId,
    ) {
        let issuer_ids = self
            .issuers
            .iter()
            .map(|issuer| issuer.id)
            .collect::<Vec<_>>();
        for issuer in issuer_ids {
            while self.post_issuer_lend_order(issuer, schedules, money_good) {}
        }
    }

    fn post_issuer_lend_order(
        &mut self,
        issuer_id: crate::ledger::IssuerId,
        schedules: &[(AgentId, AgioSchedule)],
        money_good: GoodId,
    ) -> bool {
        let Some((issuer, present, future_limit, horizon, lender, funding)) =
            self.issuers.iter().find_map(|issuer| {
                if issuer.id != issuer_id {
                    return None;
                }
                let (present, future_limit) = one_unit_policy_quote(
                    issuer.policy.loan_present,
                    issuer.policy.loan_future_due,
                )?;
                let capacity = self
                    .loan_reservations
                    .issuer_fiat_credit_capacity(issuer, self.regime);
                let lender = CreditLender::Issuer(issuer.id);
                let funding = CreditSource::FiatCredit(issuer.id);
                if capacity < present {
                    return None;
                }
                let live_supply = self.loan_book.live_lender_order_count(
                    lender,
                    funding,
                    present,
                    future_limit,
                    issuer.policy.loan_horizon,
                );
                let demand = self.policy_lend_order_demand(
                    present,
                    future_limit,
                    issuer.policy.loan_horizon,
                    schedules,
                    money_good,
                );
                if live_supply >= demand {
                    return None;
                }
                Some((
                    issuer.id,
                    present,
                    future_limit,
                    issuer.policy.loan_horizon,
                    lender,
                    funding,
                ))
            })
        else {
            return false;
        };
        self.seq += 1;
        let order = LoanOrder {
            agent: issuer_order_agent(issuer),
            lender,
            side: LoanSide::Lend,
            present,
            future_limit,
            horizon,
            seq: self.seq,
            expires_tick: self.tick.0 + ORDER_TTL,
            purpose: LoanPurpose::Consumption,
            funding,
        };
        if !self.loan_reservations.reserve_order_m3(
            &self.agents,
            &order,
            &self.banks,
            &self.issuers,
            self.regime,
        ) {
            return false;
        }
        let trades = {
            let money_system = self
                .money_system
                .as_mut()
                .expect("M3 issuer order requires money system");
            self.loan_book.add_order_m3(
                order,
                self.tick.0,
                LoanM3Context {
                    agents: self.agents.as_mut_slice(),
                    reservations: &mut self.loan_reservations,
                    debts: &mut self.debts,
                    next_debt_id: &mut self.next_debt_id,
                    money_system,
                    banks: &mut self.banks,
                    issuers: &mut self.issuers,
                    regime: self.regime,
                },
            )
        };
        self.apply_project_loan_trades(&trades);
        self.loan_trades.extend(trades);
        true
    }

    fn policy_lend_order_demand(
        &self,
        present: Gold,
        future_limit: Gold,
        horizon: u8,
        schedules: &[(AgentId, AgioSchedule)],
        money_good: GoodId,
    ) -> usize {
        self.loan_book
            .crossable_borrow_count(present, future_limit, horizon)
            .saturating_add(self.potential_project_borrow_demand(
                present,
                future_limit,
                horizon,
                money_good,
            ))
            .saturating_add(self.potential_consumption_borrow_demand(
                present,
                future_limit,
                horizon,
                schedules,
            ))
    }

    fn potential_consumption_borrow_demand(
        &self,
        present: Gold,
        future_limit: Gold,
        horizon: u8,
        schedules: &[(AgentId, AgioSchedule)],
    ) -> usize {
        schedules
            .iter()
            .filter(|(agent, schedule)| {
                if self
                    .loan_book
                    .has_live(*agent, LoanSide::Borrow, present, horizon)
                {
                    return false;
                }
                match schedule.max_future_due_for_borrowing(present, horizon) {
                    Some(limit) => limit >= future_limit,
                    None => false,
                }
            })
            .count()
    }

    fn potential_project_borrow_demand(
        &self,
        present: Gold,
        future_limit: Gold,
        horizon: u8,
        money_good: GoodId,
    ) -> usize {
        let mut demand = 0usize;
        for order_pos in 0..self.agent_order.len() {
            let agent_index = self.agent_order[order_pos];
            if !self.agent_can_own_project(agent_index) {
                continue;
            }
            let owner = self.agents[agent_index].id;
            if self.owner_has_project(owner)
                || self.free_gold_after_all_reserves(owner) > Gold::ZERO
            {
                continue;
            }
            for line in &self.project_lines {
                if self
                    .tick_self_funded_project_starts
                    .iter()
                    .any(|(agent, started_line)| *agent == owner && *started_line == line.id)
                    || self.owner_has_active_project_plan_for_line(owner, line.id)
                {
                    continue;
                }
                let input_goods = committed_input_goods(line);
                if !input_goods.iter().all(|(good, qty)| {
                    self.project_input_available(agent_index, *good, money_good, None) >= *qty
                }) {
                    continue;
                }
                let Some(stock) = self.project_appraisal_stock(agent_index, money_good) else {
                    continue;
                };
                let view = self.debt_view_for_agent(owner);
                let expected_revenue =
                    expected_revenue_for(&self.agents[agent_index], line, Some(money_good));
                let loan_horizon = u32::from(line.period_len).saturating_add(5);
                let candidate = ProjectBundleCandidate {
                    owner,
                    line: line.id,
                    present_advance: present,
                    expected_revenue,
                    input_cost_basis: input_cost_basis(
                        &self.agents[agent_index],
                        &line.input_goods,
                        Some(money_good),
                    ),
                    required_labor: line.required_labor,
                    project_period: u32::from(line.period_len),
                    loan_horizon,
                    input_goods,
                };
                let endowment = ProjectBundleEndowment {
                    scale: &self.agents[agent_index].scale,
                    stock: &stock,
                    gold: self.free_gold_after_all_reserves(owner),
                    receivables: &view.receivables,
                    payables: &view.payables,
                    tick: self.tick,
                };
                let Some(quote) = appraise_project_bundle_for_money(
                    &endowment,
                    &candidate,
                    ProjectPlanId(self.next_project_plan_id),
                    money_good,
                ) else {
                    continue;
                };
                let Ok(quote_horizon) = u8::try_from(quote.loan_horizon) else {
                    continue;
                };
                if quote.present_advance == present
                    && quote_horizon == horizon
                    && quote.max_future_due >= future_limit
                {
                    demand = demand.saturating_add(1);
                    break;
                }
            }
        }
        demand
    }

    fn post_loan_order(
        &mut self,
        agent: AgentId,
        side: LoanSide,
        future_limit: Gold,
        horizon: u8,
        purpose: LoanPurpose,
    ) -> bool {
        if self.loan_book.has_live(agent, side, Gold(1), horizon) {
            return false;
        }
        if side == LoanSide::Lend && self.free_gold_after_all_reserves(agent) < Gold(1) {
            return false;
        }
        self.seq += 1;
        let order = LoanOrder {
            agent,
            lender: CreditLender::Agent(agent),
            side,
            present: Gold(1),
            future_limit,
            horizon,
            seq: self.seq,
            expires_tick: self.tick.0 + ORDER_TTL,
            purpose,
            funding: CreditSource::Commodity,
        };
        if !self.loan_reservations.reserve_order(&self.agents, &order) {
            return false;
        }
        let trades = if self.m3_enabled {
            let money_system = self
                .money_system
                .as_mut()
                .expect("M3 loan order requires money system");
            self.loan_book.add_order_m3(
                order,
                self.tick.0,
                LoanM3Context {
                    agents: self.agents.as_mut_slice(),
                    reservations: &mut self.loan_reservations,
                    debts: &mut self.debts,
                    next_debt_id: &mut self.next_debt_id,
                    money_system,
                    banks: &mut self.banks,
                    issuers: &mut self.issuers,
                    regime: self.regime,
                },
            )
        } else {
            self.loan_book.add_order(
                order,
                self.tick.0,
                self.agents.as_mut_slice(),
                &mut self.loan_reservations,
                &mut self.debts,
                &mut self.next_debt_id,
            )
        };
        self.apply_project_loan_trades(&trades);
        self.loan_trades.extend(trades);
        true
    }

    fn post_project_funding_borrows(&mut self, money_good: GoodId) {
        for order_pos in 0..self.agent_order.len() {
            let agent_index = self.agent_order[order_pos];
            if !self.agent_can_own_project(agent_index) {
                continue;
            }
            let owner = self.agents[agent_index].id;
            if self.owner_has_project(owner)
                || self.free_gold_after_all_reserves(owner) > Gold::ZERO
            {
                continue;
            }
            let lines = self.project_lines.clone();
            for line in lines {
                if self
                    .tick_self_funded_project_starts
                    .iter()
                    .any(|(agent, started_line)| *agent == owner && *started_line == line.id)
                    || self.owner_has_active_project_plan_for_line(owner, line.id)
                {
                    continue;
                }
                let input_goods = committed_input_goods(&line);
                if !input_goods.iter().all(|(good, qty)| {
                    self.project_input_available(agent_index, *good, money_good, None) >= *qty
                }) {
                    continue;
                }
                let expected_revenue =
                    expected_revenue_for(&self.agents[agent_index], &line, Some(money_good));
                let loan_horizon = u32::from(line.period_len).saturating_add(5);
                let candidate = ProjectBundleCandidate {
                    owner,
                    line: line.id,
                    present_advance: Gold(1),
                    expected_revenue,
                    input_cost_basis: input_cost_basis(
                        &self.agents[agent_index],
                        &line.input_goods,
                        Some(money_good),
                    ),
                    required_labor: line.required_labor,
                    project_period: u32::from(line.period_len),
                    loan_horizon,
                    input_goods,
                };
                let Some(stock) = self.project_appraisal_stock(agent_index, money_good) else {
                    continue;
                };
                let view = self.debt_view_for_agent(owner);
                let plan_id = ProjectPlanId(self.next_project_plan_id);
                let endowment = ProjectBundleEndowment {
                    scale: &self.agents[agent_index].scale,
                    stock: &stock,
                    gold: self.free_gold_after_all_reserves(owner),
                    receivables: &view.receivables,
                    payables: &view.payables,
                    tick: self.tick,
                };
                let Some(quote) =
                    appraise_project_bundle_for_money(&endowment, &candidate, plan_id, money_good)
                else {
                    continue;
                };
                let Ok(horizon) = u8::try_from(quote.loan_horizon) else {
                    continue;
                };
                let plan_id = self.ensure_project_funding_plan(owner, &line, &candidate, horizon);
                let purpose = LoanPurpose::ProjectFunding(plan_id);
                if self.loan_book.has_live_with_purpose(
                    owner,
                    LoanSide::Borrow,
                    quote.present_advance,
                    horizon,
                    &purpose,
                ) {
                    continue;
                }
                if self.post_loan_order(
                    owner,
                    LoanSide::Borrow,
                    quote.max_future_due,
                    horizon,
                    purpose,
                ) {
                    break;
                }
            }
        }
    }

    fn project_appraisal_stock(&self, agent_index: usize, money_good: GoodId) -> Option<Stock> {
        let source = &self.agents[agent_index];
        let mut stock = Stock::new(self.max_good_id);
        for good in &self.market_goods {
            let reserved = self.reservations.reserved_stock(source.id, *good);
            let near = source.stock_reserved_for_near_wants_for_money(*good, money_good);
            let held = source.stock.get(*good);
            if reserved > held {
                return None;
            }
            stock.add(*good, held.saturating_sub(reserved).saturating_sub(near));
        }
        Some(stock)
    }

    fn ensure_project_funding_plan(
        &mut self,
        owner: AgentId,
        line: &ProjectLine,
        candidate: &ProjectBundleCandidate,
        horizon: u8,
    ) -> ProjectPlanId {
        if let Some(plan) = self.project_funding_plans.iter_mut().find(|plan| {
            plan.owner == owner
                && plan.line == line.id
                && plan.started_project.is_none()
                && plan.expires_tick > self.tick
        }) {
            plan.expires_tick = Tick(self.tick.0.saturating_add(ORDER_TTL));
            plan.expected_revenue = candidate.expected_revenue;
            plan.input_cost_basis = candidate.input_cost_basis;
            plan.required_labor = candidate.required_labor;
            plan.funding_horizon = u32::from(horizon);
            return plan.id;
        }
        let id = ProjectPlanId(self.next_project_plan_id);
        self.next_project_plan_id = self
            .next_project_plan_id
            .checked_add(1)
            .expect("project plan id overflow");
        self.project_funding_plans.push(ProjectFundingPlan {
            id,
            owner,
            line: line.id,
            created_tick: self.tick,
            expires_tick: Tick(self.tick.0.saturating_add(ORDER_TTL)),
            expected_revenue: candidate.expected_revenue,
            input_cost_basis: candidate.input_cost_basis,
            required_labor: candidate.required_labor,
            funding_horizon: u32::from(horizon),
            borrowed_gold: Gold::ZERO,
            future_due_committed: Gold::ZERO,
            reserved_gold: Gold::ZERO,
            started_project: None,
        });
        id
    }

    fn apply_project_loan_trades(&mut self, trades: &[LoanTrade]) {
        for trade in trades {
            let Some(plan_id) = trade.purpose.project_plan() else {
                continue;
            };
            if let Some(plan) = self
                .project_funding_plans
                .iter_mut()
                .find(|plan| plan.id == plan_id && plan.expires_tick > self.tick)
            {
                plan.borrowed_gold = plan.borrowed_gold.saturating_add(trade.present);
                plan.reserved_gold = plan.reserved_gold.saturating_add(trade.present);
                plan.future_due_committed =
                    plan.future_due_committed.saturating_add(trade.future_due);
            }
        }
    }

    fn build_m2_record(
        &self,
        tick_spot_trades: &[Trade],
        tick_labor_trades: &[LaborTrade],
        tick_loan_trades: &[LoanTrade],
        market_rate_bps: Option<i64>,
        natural_rate_proxy_bps: Option<i64>,
        rate_gap_bps: Option<i64>,
    ) -> M2Record {
        let wages_paid = tick_labor_trades.iter().fold(Gold::ZERO, |total, trade| {
            total.saturating_add(trade.wage.mul_qty(trade.qty).unwrap_or(Gold::ZERO))
        });
        let debt_counts = self.debt_counts();
        let project_debt_counts = self.project_debt_counts();
        let project_counts = self.project_counts();
        M2Record {
            tick: self.tick.0,
            total_gold: self.total_money_balance(),
            spot_trades: u32::try_from(tick_spot_trades.len()).unwrap_or(u32::MAX),
            labor_trades: u32::try_from(tick_labor_trades.len()).unwrap_or(u32::MAX),
            loan_trades: u32::try_from(tick_loan_trades.len()).unwrap_or(u32::MAX),
            project_loan_trades: u32::try_from(
                tick_loan_trades
                    .iter()
                    .filter(|trade| trade.purpose.project_plan().is_some())
                    .count(),
            )
            .unwrap_or(u32::MAX),
            project_borrowed_gold: self.project_borrowed_gold(),
            debts_open: debt_counts.open,
            debts_settled: debt_counts.settled,
            debts_defaulted: debt_counts.defaulted,
            project_debts_open: project_debt_counts.open,
            project_debts_settled: project_debt_counts.settled,
            project_debts_defaulted: project_debt_counts.defaulted,
            project_funding_reserved_gold: self.project_funding_reserved_gold(),
            active_projects: project_counts.active,
            waiting_projects: project_counts.waiting,
            mature_projects: project_counts.mature,
            sold_projects: project_counts.sold,
            abandoned_projects: project_counts.abandoned,
            labor_advanced: project_counts.labor_advanced,
            wages_paid,
            project_revenue: self.project_revenue,
            project_profit: cumulative_project_profit(&self.m2_projects, &self.project_output_lots),
            capital_labor_consumed: self.capital_labor_consumed,
            capital_goods_consumed: self.capital_goods_consumed,
            capital_gold_loss: self.capital_gold_loss,
            market_rate_bps,
            natural_rate_proxy_bps,
            rate_gap_bps,
            structure_length_ticks_x100: structure_length_ticks_x100(&self.m2_projects, self.tick),
        }
    }

    fn build_m3_record(&self, m2: M2Record) -> M3Record {
        let stock = self
            .money_system
            .as_ref()
            .map(MoneySystem::snapshot)
            .unwrap_or_default();
        let (early_receiver_wealth_delta, late_receiver_wealth_delta) =
            self.cantillon_cohort_wealth_deltas();
        M3Record {
            m2,
            regime: self.regime,
            public_specie: stock.public_specie,
            public_fiat: stock.public_fiat,
            demand_claims: stock.demand_claims,
            bank_reserves: stock.bank_reserves,
            fiduciary: stock.fiduciary,
            time_deposits: stock.time_deposits,
            tms: stock.tms(),
            bank_credit_issued: self.tick_bank_credit_issued,
            fiat_credit_issued: self.tick_fiat_credit_issued,
            fiat_fiscal_issued: self.tick_fiat_fiscal_issued,
            credit_retired: self.tick_credit_retired,
            bank_loan_trades: self.tick_bank_loan_trades,
            fiat_loan_trades: self.tick_fiat_loan_trades,
            shadow_natural_rate_bps: None,
            shadow_rate_gap_bps: None,
            boom_projects_started: self.tick_project_starts(),
            bust_abandoned_projects: self.tick_project_abandonments(),
            early_receiver_wealth_delta,
            late_receiver_wealth_delta,
        }
    }

    fn tick_project_starts(&self) -> u32 {
        self.m2_projects
            .iter()
            .filter(|project| project.started_at == self.tick)
            .count()
            .try_into()
            .unwrap_or(u32::MAX)
    }

    fn tick_project_abandonments(&self) -> u32 {
        let previous = self
            .m3_records
            .last()
            .map(|record| record.m2.abandoned_projects)
            .unwrap_or(0);
        self.project_counts().abandoned.saturating_sub(previous)
    }

    /// Tag bank-fiduciary and fiat-credit first receivers — the borrowers whose loan
    /// orders crossed bank/issuer lend orders — on the Cantillon receipt tape, mirroring
    /// the fiscal-fiat tagging, so early-vs-late receiver reports cover the credit
    /// channels too (impl-05.md §5). Commodity credit transfers existing money and fiat
    /// fiscal is already tagged at issuance, so both are skipped.
    fn record_credit_cantillon_receipts(&mut self, tick_loan_trades: &[LoanTrade]) {
        let tick = self.tick;
        let receipts = tick_loan_trades
            .iter()
            .filter(|trade| {
                matches!(
                    trade.funding,
                    CreditSource::BankFiduciary(_) | CreditSource::FiatCredit(_)
                )
            })
            .map(|trade| CantillonReceipt {
                tick,
                agent: trade.borrower,
                amount: trade.present,
                source: trade.funding,
            });
        self.cantillon_receipts.extend(receipts);
    }

    fn cantillon_cohort_wealth_deltas(&self) -> (i64, i64) {
        let Some(first_tick) = self
            .cantillon_receipts
            .iter()
            .map(|receipt| receipt.tick)
            .min_by_key(|tick| tick.0)
        else {
            return (0, 0);
        };
        let mut early = self
            .cantillon_receipts
            .iter()
            .filter(|receipt| receipt.tick == first_tick)
            .map(|receipt| receipt.agent)
            .collect::<Vec<_>>();
        early.sort();
        early.dedup();
        if early.is_empty() {
            return (0, 0);
        }

        let mut late = self
            .cantillon_receipts
            .iter()
            .filter(|receipt| receipt.tick.0 > first_tick.0)
            .map(|receipt| receipt.agent)
            .filter(|agent| !early.contains(agent))
            .collect::<Vec<_>>();
        late.sort();
        late.dedup();
        (
            self.mean_realized_basket_delta(&early),
            self.mean_realized_basket_delta(&late),
        )
    }

    fn mean_realized_basket_delta(&self, cohort: &[AgentId]) -> i64 {
        if cohort.is_empty() {
            return 0;
        }
        let prices = self.realized_prices();
        let mut total = 0i64;
        let mut count = 0i64;
        for agent in cohort {
            let Some(current) = self
                .agents
                .as_slice()
                .iter()
                .find(|entry| entry.id == *agent)
            else {
                continue;
            };
            let Some(initial) = self.initial_agents.iter().find(|entry| entry.id == *agent) else {
                continue;
            };
            let current_value = realized_basket_value(current, &prices);
            let initial_value = realized_basket_value(initial, &prices);
            let consumed_value =
                realized_consumed_basket_value(*agent, current, initial, &prices, &self.trades);
            total = total.saturating_add(
                current_value
                    .saturating_sub(initial_value)
                    .saturating_add(consumed_value),
            );
            count += 1;
        }
        if count == 0 {
            0
        } else {
            total / count
        }
    }

    fn realized_prices(&self) -> Vec<(GoodId, Gold)> {
        self.realized_prices
            .iter()
            .map(|(good, price)| (*good, *price))
            .collect()
    }

    fn build_record(&self, expired_orders: u32, tick_trades: &[Trade]) -> MarketRecord {
        let mut record = MarketRecord {
            tick: self.tick.0,
            total_gold: self.total_money_balance(),
            trades: u32::try_from(tick_trades.len()).unwrap_or(u32::MAX),
            expired_orders,
            ..MarketRecord::default()
        };

        for trade in tick_trades {
            add_good_volume(&mut record.good_volumes, trade.good, trade.qty);
            set_last_price(&mut record.last_prices, trade.good, trade.price);
            match trade.good {
                FOOD => {
                    record.food_volume += trade.qty;
                    record.last_food_price = Some(trade.price);
                }
                WOOD => {
                    record.wood_volume += trade.qty;
                    record.last_wood_price = Some(trade.price);
                }
                NET => {
                    record.net_volume += trade.qty;
                    record.last_net_price = Some(trade.price);
                }
                _ => {}
            }
        }

        for book in &self.books {
            let (bids, asks) = book.live_order_counts();
            record.bid_count += bids;
            record.ask_count += asks;
        }

        record
    }

    fn find_live_quote(&self, agent: AgentId, side: OrderSide, good: GoodId) -> Option<usize> {
        self.live_quotes
            .iter()
            .position(|quote| quote.agent == agent && quote.side == side && quote.good == good)
    }

    fn sync_live_quotes(&mut self) {
        let books = &self.books;
        self.live_quotes.retain(|quote| {
            books
                .iter()
                .find(|book| book.good == quote.good)
                .map(|book| book.contains_seq(quote.seq))
                .unwrap_or(false)
        });
    }

    fn agent_index_for(&self, agent: AgentId) -> Option<usize> {
        self.agents.position_of(agent)
    }

    /// The society's good catalog (lab-default in G0b). The registry-aware
    /// successor to the free [`crate::good::good_name`] shim.
    pub fn good_registry(&self) -> &GoodRegistry {
        &self.registry
    }

    /// The name of a good via the society's registry — the registry-aware
    /// naming path. Equals [`crate::good::good_name`] for the lab catalog.
    pub fn good_name(&self, good: GoodId) -> &str {
        self.registry.name(good)
    }

    /// Intern `name` into the society's good catalog, returning its [`GoodId`]
    /// (the existing id if already known, else the next one). This is the G3a
    /// content seam: a driver that introduces content goods (grain/flour/bread
    /// and the tools) registers their names here so [`Society::good_name`]
    /// resolves them. Purely a naming extension — it touches no market state, so
    /// a society whose driver never calls it keeps the lab catalog and every
    /// golden byte-identical.
    pub fn intern_good(&mut self, name: &str) -> GoodId {
        self.registry.intern(name)
    }

    /// Apply the direct recipe `recipe_id` to a live agent using the existing
    /// `econ` direct-recipe executor, returning the accounted conversion.
    ///
    /// This is an additive seam for external drivers such as `sim`'s seeded G3a
    /// producers. It preflights output headroom so the underlying stock add
    /// cannot saturate after inputs are removed, rejects unknown/removed
    /// agents, records recipe labor in the tick-local labor log, and otherwise
    /// delegates the mutation to `execute_direct_recipe_for_agent`.
    pub fn execute_direct_recipe_for_agent_checked(
        &mut self,
        agent: AgentId,
        recipe_id: RecipeId,
    ) -> Option<DirectRecipeExecution> {
        self.execute_direct_recipe_for_agent_checked_with_labor(agent, recipe_id, u32::MAX)
    }

    /// Apply `recipe_id` to `agent` only if it fits inside `remaining_labor`, returning
    /// the accounted conversion.
    ///
    /// External drivers use this when they own a phase-specific labor envelope but still
    /// want econ's normal direct-recipe preflights, stock mutation, and labor receipt.
    pub fn execute_direct_recipe_for_agent_checked_with_labor(
        &mut self,
        agent: AgentId,
        recipe_id: RecipeId,
        remaining_labor: u32,
    ) -> Option<DirectRecipeExecution> {
        let position = self.agent_index_for(agent)?;
        if self.dead_agents.binary_search(&agent).is_ok() {
            return None;
        }

        let recipe = self
            .recipes
            .iter()
            .find(|recipe| recipe.id == recipe_id)?
            .clone();
        // An intentional fast-path, not a missed dedup: `execute_direct_recipe_for_agent`
        // re-checks the same `recipe.labor > remaining_labor` guard, but bailing here
        // skips the output-headroom preflight below for a recipe that cannot run anyway.
        if recipe.labor > remaining_labor {
            return None;
        }
        self.agents[position]
            .stock
            .get(recipe.output_good)
            .checked_add(recipe.output_qty)?;

        let mut provisions = TickProvisions::new(self.agents[position].scale.len());
        let labor = execute_direct_recipe_for_agent(
            &mut self.agents[position],
            &self.recipes,
            recipe_id,
            remaining_labor,
            0,
            &mut provisions,
        )?;
        if labor > 0 {
            self.add_tick_labor_used(agent, labor);
        }
        Some(DirectRecipeExecution {
            labor,
            input: recipe.input_good,
            output: (recipe.output_good, recipe.output_qty),
        })
    }

    /// Set the `enabled` flag of the recipe with `recipe_id`, returning `true` if a
    /// matching recipe was found (and `false` if not). The additive seam for `sim`'s
    /// G6b **tech-tier unlock**: a tier-gated recipe starts `enabled: false`, and
    /// crossing the Knowledge threshold flips it `true` for that settlement — reusing
    /// the existing `Recipe::enabled` gate the direct-recipe executor already honors
    /// (a disabled recipe never runs), rather than adding new gating machinery. It
    /// mutates only the recipe's
    /// flag (touches no scale, quote, money, or market state) and is called by no
    /// engine path, so the conformance goldens are byte-identical.
    pub fn set_recipe_enabled(&mut self, recipe_id: RecipeId, enabled: bool) -> bool {
        if let Some(recipe) = self
            .recipes
            .iter_mut()
            .find(|recipe| recipe.id == recipe_id)
        {
            recipe.enabled = enabled;
            true
        } else {
            false
        }
    }

    fn reserved_gold_all(&self, agent: AgentId) -> Gold {
        self.reservations
            .reserved_gold(agent)
            .saturating_add(self.loan_reservations.reserved_gold(agent))
            .saturating_add(self.labor_reservations.reserved_gold(agent))
            .saturating_add(self.reserved_project_gold(agent))
    }

    fn reserved_gold_all_except_plan(
        &self,
        agent: AgentId,
        plan_id: Option<ProjectPlanId>,
    ) -> Gold {
        let project_reserved = match plan_id {
            Some(plan_id) => self
                .reserved_project_gold(agent)
                .saturating_sub(self.reserved_project_gold_for_plan(plan_id)),
            None => self.reserved_project_gold(agent),
        };
        self.reservations
            .reserved_gold(agent)
            .saturating_add(self.loan_reservations.reserved_gold(agent))
            .saturating_add(self.labor_reservations.reserved_gold(agent))
            .saturating_add(project_reserved)
    }

    pub fn free_gold_after_all_reserves(&self, agent: AgentId) -> Gold {
        let Some(index) = self.agent_index_for(agent) else {
            return Gold::ZERO;
        };
        self.agents[index]
            .gold
            .saturating_sub(self.reserved_gold_all(agent))
    }

    pub fn free_stock_after_all_reserves(&self, agent: AgentId, good: GoodId) -> u32 {
        let Some(index) = self.agent_index_for(agent) else {
            return 0;
        };
        let reserved = self
            .reservations
            .reserved_stock(agent, good)
            .saturating_add(self.barter_book.reserved_qty(agent, good));
        self.agents[index].stock.get(good).saturating_sub(reserved)
    }

    fn free_spot_tender_after_all_reserves(&self, agent: AgentId) -> Gold {
        if !self.m3_enabled {
            return self.free_gold_after_all_reserves(agent);
        }
        let Some(money_system) = &self.money_system else {
            return Gold::ZERO;
        };
        money_system
            .accepted_spendable_total(agent, self.public_spot_tender.accepted_media())
            .saturating_sub(self.reserved_gold_all(agent))
    }

    fn wage_tender_spendable_cap(&self, agent: AgentId, plan_id: Option<ProjectPlanId>) -> Gold {
        let Some(money_system) = &self.money_system else {
            return Gold::ZERO;
        };
        let accepted =
            money_system.accepted_spendable_total(agent, self.labor_wage_tender.accepted_media());
        let reserved = self.reserved_gold_all_except_plan(agent, plan_id);
        accepted.saturating_sub(reserved)
    }

    fn free_spot_tender_after_all_reserves_for_quote(
        &self,
        agent: AgentId,
        existing: Option<usize>,
    ) -> Gold {
        let mut existing_bid_reservation = Gold::ZERO;
        if let Some(index) = existing {
            let quote = self.live_quotes[index];
            if quote.side == OrderSide::Bid {
                if let Some(amount) = quote.limit.mul_qty(quote.qty) {
                    existing_bid_reservation = amount;
                }
            }
        }
        let reserved = self
            .reserved_gold_all(agent)
            .saturating_sub(existing_bid_reservation);
        if !self.m3_enabled {
            let Some(index) = self.agent_index_for(agent) else {
                return Gold::ZERO;
            };
            return self.agents[index].gold.saturating_sub(reserved);
        }
        let Some(money_system) = &self.money_system else {
            return Gold::ZERO;
        };
        money_system
            .accepted_spendable_total(agent, self.public_spot_tender.accepted_media())
            .saturating_sub(reserved)
    }

    fn reserved_project_gold(&self, owner: AgentId) -> Gold {
        self.project_funding_plans
            .iter()
            .filter(|plan| plan.owner == owner)
            .fold(Gold::ZERO, |total, plan| {
                total.saturating_add(plan.reserved_gold)
            })
    }

    fn reserved_project_gold_for_plan(&self, plan_id: ProjectPlanId) -> Gold {
        self.project_funding_plans
            .iter()
            .find(|plan| plan.id == plan_id)
            .map(|plan| plan.reserved_gold)
            .unwrap_or(Gold::ZERO)
    }

    fn free_gold_for_project_plan(&self, owner: AgentId, plan_id: ProjectPlanId) -> Gold {
        let Some(index) = self.agent_index_for(owner) else {
            return Gold::ZERO;
        };
        self.agents[index]
            .gold
            .saturating_sub(self.reserved_gold_all_except_plan(owner, Some(plan_id)))
    }

    fn project_plan_for_project(&self, project_id: M2ProjectId) -> Option<ProjectPlanId> {
        self.project_funding_plans
            .iter()
            .find(|plan| plan.started_project == Some(project_id))
            .map(|plan| plan.id)
    }

    fn owner_has_active_project_plan_for_line(&self, owner: AgentId, line: ProjectLineId) -> bool {
        self.project_funding_plans.iter().any(|plan| {
            plan.owner == owner
                && plan.line == line
                && plan.started_project.is_none()
                && plan.expires_tick > self.tick
        })
    }

    fn owner_defaulted_project_funding_debt_this_tick(&self, owner: AgentId) -> bool {
        self.debts.iter().any(|debt| {
            debt.borrower == owner
                && debt.due_tick == self.tick
                && debt.state == DebtState::Defaulted
                && matches!(debt.purpose, DebtPurpose::ProjectFunding { .. })
        })
    }

    fn expire_project_funding_plans(&mut self) {
        for plan in &mut self.project_funding_plans {
            if plan.started_project.is_none() && plan.expires_tick <= self.tick {
                plan.reserved_gold = Gold::ZERO;
            }
        }
    }

    fn project_debt_payment_snapshot(&self) -> Option<ProjectDebtPaymentSnapshot> {
        let mut paid_before = self
            .debts
            .iter()
            .filter(|debt| debt.state == DebtState::Open && debt.due_tick <= self.tick)
            .map(|debt| (debt.id, debt.paid))
            .collect::<Vec<_>>();
        let has_due_project_debt = self.debts.iter().any(|debt| {
            debt.state == DebtState::Open
                && debt.due_tick <= self.tick
                && matches!(debt.purpose, DebtPurpose::ProjectFunding { .. })
        });
        let has_reserved_project_gold = self
            .project_funding_plans
            .iter()
            .any(|plan| plan.reserved_gold > Gold::ZERO);
        if !has_due_project_debt && !(has_reserved_project_gold && !paid_before.is_empty()) {
            return None;
        }
        paid_before.sort_by_key(|(debt, _)| *debt);
        Some(ProjectDebtPaymentSnapshot {
            debt_payment: self.debt_payment_audit.len(),
            bank_repayment: self.bank_repayment_audit.len(),
            paid_before,
        })
    }

    fn release_project_funding_reserves_for_debt_payments(
        &mut self,
        snapshot: &ProjectDebtPaymentSnapshot,
    ) {
        let mut specie_paid_by_debt = Vec::new();
        let mut audited_debts = Vec::new();
        let mut affected_owners = Vec::new();
        let open_project_debt_plans = self.open_project_funding_debt_plans();
        let mut plan_effects = Vec::new();
        for record in self.debt_payment_audit.iter().skip(snapshot.debt_payment) {
            audited_debts.push(DebtId(record.debt));
            if record.paid > Gold::ZERO {
                affected_owners.push(record.from);
            }
            if record.public_specie > Gold::ZERO {
                specie_paid_by_debt.push((DebtId(record.debt), record.public_specie));
            }
        }
        for record in self
            .bank_repayment_audit
            .iter()
            .skip(snapshot.bank_repayment)
        {
            audited_debts.push(DebtId(record.debt));
            if record.paid > Gold::ZERO {
                affected_owners.push(record.borrower);
            }
            if record.public_specie > Gold::ZERO {
                specie_paid_by_debt.push((DebtId(record.debt), record.public_specie));
            }
        }
        for debt in &self.debts {
            let Some(before) = snapshot
                .paid_before
                .binary_search_by_key(&debt.id, |(entry, _)| *entry)
                .ok()
                .map(|index| snapshot.paid_before[index].1)
            else {
                continue;
            };
            if debt.paid > before {
                affected_owners.push(debt.borrower);
            }
        }
        audited_debts.sort();
        audited_debts.dedup();
        specie_paid_by_debt.sort_by_key(|(debt, _)| *debt);
        let mut summed_specie_paid_by_debt: Vec<(DebtId, Gold)> = Vec::new();
        for (debt, paid) in specie_paid_by_debt {
            if let Some((entry, total)) = summed_specie_paid_by_debt.last_mut() {
                if *entry == debt {
                    *total = total.saturating_add(paid);
                    continue;
                }
            }
            summed_specie_paid_by_debt.push((debt, paid));
        }

        for debt in &self.debts {
            let DebtPurpose::ProjectFunding { plan, .. } = debt.purpose else {
                continue;
            };
            let Some(before) = snapshot
                .paid_before
                .binary_search_by_key(&debt.id, |(entry, _)| *entry)
                .ok()
                .map(|index| snapshot.paid_before[index].1)
            else {
                continue;
            };
            let paid_delta = debt.paid.saturating_sub(before);
            let audited_specie_paid = summed_specie_paid_by_debt
                .binary_search_by_key(&debt.id, |(entry, _)| *entry)
                .ok()
                .map(|index| summed_specie_paid_by_debt[index].1)
                .unwrap_or(Gold::ZERO);
            let has_audit_record = audited_debts.binary_search(&debt.id).is_ok();
            let specie_paid = if self.money_system.is_none() && !has_audit_record {
                paid_delta
            } else {
                audited_specie_paid
            };
            let obligation_finished = debt.state != DebtState::Open;
            let clear_reserved_gold =
                obligation_finished && open_project_debt_plans.binary_search(&plan).is_err();
            if paid_delta == Gold::ZERO && specie_paid == Gold::ZERO && !clear_reserved_gold {
                continue;
            }
            affected_owners.push(debt.borrower);
            plan_effects.push(ProjectPlanDebtPaymentEffect {
                plan,
                specie_paid,
                clear_reserved_gold,
            });
        }
        let plan_effects = merged_project_plan_debt_payment_effects(plan_effects);
        self.apply_project_plan_debt_payment_effects(&plan_effects);
        affected_owners.sort();
        affected_owners.dedup();
        self.clamp_project_funding_reserves_to_owner_balances(&affected_owners);
    }

    fn open_project_funding_debt_plans(&self) -> Vec<ProjectPlanId> {
        let mut plans = self
            .debts
            .iter()
            .filter(|debt| debt.state == DebtState::Open)
            .filter_map(|debt| debt.purpose.project_plan())
            .collect::<Vec<_>>();
        plans.sort();
        plans.dedup();
        plans
    }

    fn apply_project_plan_debt_payment_effects(
        &mut self,
        effects: &[ProjectPlanDebtPaymentEffect],
    ) {
        for plan in &mut self.project_funding_plans {
            let Ok(effect_index) = effects.binary_search_by_key(&plan.id, |effect| effect.plan)
            else {
                continue;
            };
            let effect = effects[effect_index];
            if effect.clear_reserved_gold {
                plan.reserved_gold = Gold::ZERO;
            } else {
                plan.reserved_gold = plan.reserved_gold.saturating_sub(effect.specie_paid);
            }
        }
    }

    fn clamp_project_funding_reserves_to_owner_balances(&mut self, owners: &[AgentId]) {
        if owners.is_empty() {
            return;
        }
        let mut owner_remaining_gold = owners
            .iter()
            .map(|owner| {
                let gold = self
                    .agent_index_for(*owner)
                    .and_then(|index| self.agents.as_slice().get(index))
                    .map(|agent| agent.gold)
                    .unwrap_or(Gold::ZERO);
                (*owner, gold)
            })
            .collect::<Vec<_>>();
        for plan in &mut self.project_funding_plans {
            if owners.binary_search(&plan.owner).is_err() {
                continue;
            }
            let index = owner_remaining_gold
                .binary_search_by_key(&plan.owner, |(agent, _)| *agent)
                .expect("owner was indexed before reserve clamp");
            let remaining = &mut owner_remaining_gold[index].1;
            if plan.reserved_gold > *remaining {
                plan.reserved_gold = *remaining;
            }
            *remaining = remaining.saturating_sub(plan.reserved_gold);
        }
    }

    #[cfg(test)]
    fn clamp_all_project_funding_reserves_to_owner_balances(&mut self) {
        let mut owners = self
            .project_funding_plans
            .iter()
            .filter(|plan| plan.reserved_gold > Gold::ZERO)
            .map(|plan| plan.owner)
            .collect::<Vec<_>>();
        owners.sort();
        owners.dedup();
        self.clamp_project_funding_reserves_to_owner_balances(&owners);
    }

    fn attribute_project_sale(&mut self, trade: &Trade) {
        let Some(payment) = trade.price.mul_qty(trade.qty) else {
            return;
        };
        let attributed = record_project_sale(
            &mut self.project_output_lots,
            &mut self.m2_projects,
            trade.seller,
            trade.good,
            trade.qty,
            payment,
        );
        self.project_revenue = self.project_revenue.saturating_add(attributed);
        self.release_completed_project_reserves();
    }

    fn release_completed_project_reserves(&mut self) {
        for plan in &mut self.project_funding_plans {
            let Some(project_id) = plan.started_project else {
                continue;
            };
            let completed = self
                .m2_projects
                .iter()
                .find(|project| project.id == project_id)
                .map(|project| {
                    matches!(
                        project.state,
                        M2ProjectState::Sold | M2ProjectState::Abandoned
                    )
                })
                .unwrap_or(true);
            if completed {
                plan.reserved_gold = Gold::ZERO;
            }
        }
    }

    fn debt_counts(&self) -> DebtCounts {
        let mut counts = DebtCounts::default();
        for debt in &self.debts {
            match debt.state {
                DebtState::Open => counts.open = counts.open.saturating_add(1),
                DebtState::Settled => counts.settled = counts.settled.saturating_add(1),
                DebtState::Defaulted => counts.defaulted = counts.defaulted.saturating_add(1),
            }
        }
        counts
    }

    fn project_debt_counts(&self) -> DebtCounts {
        let mut counts = DebtCounts::default();
        for debt in &self.debts {
            if debt.purpose.project_plan().is_none() {
                continue;
            }
            match debt.state {
                DebtState::Open => counts.open = counts.open.saturating_add(1),
                DebtState::Settled => counts.settled = counts.settled.saturating_add(1),
                DebtState::Defaulted => counts.defaulted = counts.defaulted.saturating_add(1),
            }
        }
        counts
    }

    fn project_borrowed_gold(&self) -> Gold {
        self.project_funding_plans
            .iter()
            .fold(Gold::ZERO, |total, plan| {
                total.saturating_add(plan.borrowed_gold)
            })
    }

    fn project_funding_reserved_gold(&self) -> Gold {
        self.project_funding_plans
            .iter()
            .fold(Gold::ZERO, |total, plan| {
                total.saturating_add(plan.reserved_gold)
            })
    }

    fn project_counts(&self) -> ProjectCounts {
        let mut counts = ProjectCounts::default();
        for project in &self.m2_projects {
            match project.state {
                M2ProjectState::Forming => counts.active = counts.active.saturating_add(1),
                M2ProjectState::Waiting => {
                    counts.active = counts.active.saturating_add(1);
                    counts.waiting = counts.waiting.saturating_add(1);
                }
                M2ProjectState::Mature => {
                    counts.active = counts.active.saturating_add(1);
                    counts.mature = counts.mature.saturating_add(1);
                }
                M2ProjectState::Sold => counts.sold = counts.sold.saturating_add(1),
                M2ProjectState::Abandoned => {
                    counts.abandoned = counts.abandoned.saturating_add(1);
                }
            }
            counts.labor_advanced = counts.labor_advanced.saturating_add(project.labor_advanced);
        }
        counts
    }

    fn project_funding_invariants_hold(&self) -> bool {
        let mut plan_ids = self
            .project_funding_plans
            .iter()
            .map(|plan| plan.id)
            .collect::<Vec<_>>();
        plan_ids.sort();
        let duplicate_plan_ids = plan_ids.windows(2).any(|window| window[0] == window[1]);
        if duplicate_plan_ids {
            return false;
        }
        let mut plan_project_links = self
            .project_funding_plans
            .iter()
            .filter_map(|plan| {
                plan.started_project
                    .map(|project| (plan.id, project, plan.owner, plan.line))
            })
            .collect::<Vec<_>>();
        plan_project_links.sort();
        let mut project_links = self
            .m2_projects
            .iter()
            .map(|project| (project.id, project.owner, project.line))
            .collect::<Vec<_>>();
        project_links.sort();

        for plan in &self.project_funding_plans {
            if self.dead_agents.binary_search(&plan.owner).is_ok() {
                if plan.reserved_gold > Gold::ZERO {
                    return false;
                }
            } else {
                let Some(owner_index) = self.agent_index_for(plan.owner) else {
                    return false;
                };
                if plan.reserved_gold > self.agents[owner_index].gold {
                    return false;
                }
            }
            if let Some(project_id) = plan.started_project {
                if project_links
                    .binary_search(&(project_id, plan.owner, plan.line))
                    .is_err()
                {
                    return false;
                }
            }
        }
        let mut owners = self
            .project_funding_plans
            .iter()
            .map(|plan| plan.owner)
            .collect::<Vec<_>>();
        owners.sort();
        owners.dedup();
        for owner in owners {
            if self.dead_agents.binary_search(&owner).is_ok() {
                if self.reserved_project_gold(owner) > Gold::ZERO {
                    return false;
                }
                continue;
            }
            let Some(owner_index) = self.agent_index_for(owner) else {
                return false;
            };
            if self.reserved_project_gold(owner) > self.agents[owner_index].gold {
                return false;
            }
        }
        for debt in &self.debts {
            let DebtPurpose::ProjectFunding { plan, project } = &debt.purpose else {
                continue;
            };
            let plan = *plan;
            let project = *project;
            if plan_ids.binary_search(&plan).is_err() {
                return false;
            }
            if let Some(project_id) = project {
                let valid_plan_project = plan_project_links
                    .binary_search_by_key(
                        &(plan, project_id),
                        |(entry_plan, entry_project, _, _)| (*entry_plan, *entry_project),
                    )
                    .is_ok();
                let valid_project = project_links
                    .binary_search_by_key(&project_id, |(entry_project, _, _)| *entry_project)
                    .is_ok();
                if !valid_plan_project || !valid_project {
                    return false;
                }
            }
        }
        for trade in &self.loan_trades {
            if let Some(plan) = trade.purpose.project_plan() {
                if plan_ids.binary_search(&plan).is_err() {
                    return false;
                }
                if let Some(project_id) = trade.project {
                    if plan_project_links
                        .binary_search_by_key(
                            &(plan, project_id),
                            |(entry_plan, entry_project, _, _)| (*entry_plan, *entry_project),
                        )
                        .is_err()
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
}

fn merged_project_plan_debt_payment_effects(
    mut effects: Vec<ProjectPlanDebtPaymentEffect>,
) -> Vec<ProjectPlanDebtPaymentEffect> {
    effects.sort_by_key(|effect| effect.plan);
    let mut merged = Vec::<ProjectPlanDebtPaymentEffect>::new();
    for effect in effects {
        if let Some(previous) = merged.last_mut() {
            if previous.plan == effect.plan {
                previous.specie_paid = previous.specie_paid.saturating_add(effect.specie_paid);
                previous.clear_reserved_gold |= effect.clear_reserved_gold;
                continue;
            }
        }
        merged.push(effect);
    }
    merged
}

/// Run a scenario, attach the credit-disabled shadow series for M3 records, and
/// capture per-tick metric observations for the M4 metrics builder.
pub fn run_m3_with_shadow(scenario: MarketScenario) -> Society {
    run_m3_with_shadow_impl(scenario, true)
}

/// Run a scenario and attach the credit-disabled shadow series without capturing
/// per-tick metric observations.
pub fn run_m3_with_shadow_without_metrics(scenario: MarketScenario) -> Society {
    run_m3_with_shadow_impl(scenario, false)
}

fn run_m3_with_shadow_impl(scenario: MarketScenario, capture_metrics: bool) -> Society {
    let periods = scenario.periods;
    let sound_money_view = scenario.scenario == ScenarioName::SoundMoney100Pct;
    if scenario.scenario.kind() != ScenarioKind::MarketM3 {
        let mut society = Society::from_scenario(scenario);
        if capture_metrics {
            society.enable_metric_observations();
        }
        society.run(periods);
        if sound_money_view {
            society.attach_sound_money_m3_view();
        }
        return society;
    }

    let shadow_scenario = scenario.clone();
    let mut society = Society::from_scenario(scenario);
    if capture_metrics {
        society.enable_metric_observations();
    }
    society.run(periods);
    let shadow = crate::shadow::run_credit_disabled_shadow(&shadow_scenario);
    society.attach_shadow(&shadow);
    society
}

fn agent_order_for(priority: &[AgentId], agents: &[Agent]) -> Vec<usize> {
    let mut indices = (0..agents.len()).collect::<Vec<_>>();
    indices.sort_by_key(|index| {
        let id = agents[*index].id;
        (agent_order_priority(priority, id).unwrap_or(usize::MAX), id)
    });
    indices
}

fn agent_order_priority(priority: &[AgentId], agent: AgentId) -> Option<usize> {
    priority.iter().position(|candidate| *candidate == agent)
}

fn validate_v2_events_supported(
    v2_enabled: bool,
    events: &[Event],
) -> Result<(), SocietyBuildError> {
    if !v2_enabled {
        return Ok(());
    }
    for event in events {
        if !matches!(event.kind, EventKind::DisableRecipe(_)) {
            return Err(SocietyBuildError::V2UnsupportedEvent);
        }
    }
    Ok(())
}

fn validate_v2_initial_money_balances_zero(
    v2_enabled: bool,
    agents: &[Agent],
) -> Result<(), SocietyBuildError> {
    if !v2_enabled {
        return Ok(());
    }
    if agents.iter().all(|agent| agent.gold == Gold::ZERO) {
        Ok(())
    } else {
        Err(SocietyBuildError::V2InitialMoneyBalance)
    }
}

pub(crate) fn banks_for_scenario(scenario: crate::scenario::ScenarioName) -> Vec<Bank> {
    match scenario {
        crate::scenario::ScenarioName::FractionalReserve
        | crate::scenario::ScenarioName::SuspensionOfConvertibility
        | crate::scenario::ScenarioName::EmergedGoldFractionalReserve => {
            vec![default_fractional_bank()]
        }
        crate::scenario::ScenarioName::EmergedGoldReserveLeashControl
        | crate::scenario::ScenarioName::EmergedGoldSuspensionOfConvertibility
        | crate::scenario::ScenarioName::EmergedGoldRedemptionRun
        | crate::scenario::ScenarioName::EmergedGoldSuspendedRedemption
        | crate::scenario::ScenarioName::EmergedGoldBankClaimDebtRefusalControl
        | crate::scenario::ScenarioName::EmergedGoldBankClaimDebtLegalTender
        | crate::scenario::ScenarioName::EmergedGoldBankClaimSpotRefusalControl
        | crate::scenario::ScenarioName::EmergedGoldBankClaimSpotLegalTender
        | crate::scenario::ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl
        | crate::scenario::ScenarioName::EmergedGoldBankLoanRepaymentClaimTender => {
            vec![reserve_leashed_bank()]
        }
        _ => Vec::new(),
    }
}

fn default_fractional_bank() -> Bank {
    Bank {
        id: BankId(1),
        name: "fractional bank",
        reserves: Gold::ZERO,
        demand_deposits: Gold::ZERO,
        time_deposits: Gold::ZERO,
        loans_outstanding: Gold::ZERO,
        fiduciary_issued: Gold::ZERO,
        reserve_ratio_bps: crate::money::ReserveRatioBps::FULL,
        convertible: true,
        policy: BankPolicy::default(),
    }
}

fn reserve_leashed_bank() -> Bank {
    Bank {
        id: BankId(1),
        name: "reserve-leashed bank",
        // Institutional reserve specie is accounted as bank reserves, separate
        // from the public emerged-gold bridge balances and excluded from TMS.
        reserves: Gold(2),
        demand_deposits: Gold::ZERO,
        time_deposits: Gold::ZERO,
        loans_outstanding: Gold::ZERO,
        fiduciary_issued: Gold::ZERO,
        reserve_ratio_bps: crate::money::ReserveRatioBps::FULL,
        convertible: true,
        policy: BankPolicy::default(),
    }
}

pub(crate) fn issuers_for_scenario(scenario: crate::scenario::ScenarioName) -> Vec<Issuer> {
    if scenario.starts_with_fiat_issuer() {
        vec![Issuer {
            id: crate::ledger::IssuerId(1),
            fiat_issued: Gold::ZERO,
            fiat_retired: Gold::ZERO,
            fiat_credit_outstanding: Gold::ZERO,
            policy: IssuerPolicy::default(),
            taxes_levied: Gold::ZERO,
            tax_receipts_fiat: Gold::ZERO,
            tax_receipts_specie: Gold::ZERO,
            taxes_defaulted: Gold::ZERO,
        }]
    } else {
        Vec::new()
    }
}

const BANK_ORDER_AGENT_OFFSET: u32 = 0;
const ISSUER_ORDER_AGENT_OFFSET: u32 = 1;

fn bank_order_agent(bank: BankId) -> AgentId {
    synthetic_order_agent(bank.0, BANK_ORDER_AGENT_OFFSET)
}

fn issuer_order_agent(issuer: crate::ledger::IssuerId) -> AgentId {
    synthetic_order_agent(issuer.0, ISSUER_ORDER_AGENT_OFFSET)
}

fn one_unit_policy_quote(policy_present: Gold, policy_future_due: Gold) -> Option<(Gold, Gold)> {
    if policy_present == Gold::ZERO || policy_future_due == Gold::ZERO {
        return None;
    }
    let present = Gold(1);
    let denominator = u128::from(policy_present.0);
    let due = u128::from(policy_future_due.0) / denominator;
    Some((
        present,
        Gold(u64::try_from(due).unwrap_or(u64::MAX)).max(Gold(1)),
    ))
}

fn synthetic_order_agent(id: u32, offset: u32) -> AgentId {
    let scaled = id
        .checked_mul(2)
        .and_then(|value| value.checked_add(offset))
        .unwrap_or(u32::MAX);
    AgentId(u64::from(u32::MAX.saturating_sub(scaled)))
}

fn fiscal_issued_this_tick(
    issued_by_issuer: &[(crate::ledger::IssuerId, Gold)],
    issuer: crate::ledger::IssuerId,
) -> Gold {
    issued_by_issuer
        .iter()
        .find(|(entry, _)| *entry == issuer)
        .map(|(_, amount)| *amount)
        .unwrap_or(Gold::ZERO)
}

fn record_fiscal_issued_this_tick(
    issued_by_issuer: &mut Vec<(crate::ledger::IssuerId, Gold)>,
    issuer: crate::ledger::IssuerId,
    amount: Gold,
) {
    if amount == Gold::ZERO {
        return;
    }
    if let Some((_, issued)) = issued_by_issuer
        .iter_mut()
        .find(|(entry, _)| *entry == issuer)
    {
        *issued = issued.saturating_add(amount);
    } else {
        issued_by_issuer.push((issuer, amount));
        issued_by_issuer.sort_by_key(|(entry, _)| *entry);
    }
}

fn bank_credit_stats(trades: &[LoanTrade]) -> (Gold, u32) {
    let mut issued = Gold::ZERO;
    let mut count = 0u32;
    for trade in trades {
        if matches!(trade.funding, CreditSource::BankFiduciary(_)) {
            issued = issued.saturating_add(trade.present);
            count = count.saturating_add(1);
        }
    }
    (issued, count)
}

fn fiat_credit_stats(trades: &[LoanTrade]) -> (Gold, u32) {
    let mut issued = Gold::ZERO;
    let mut count = 0u32;
    for trade in trades {
        if matches!(trade.funding, CreditSource::FiatCredit(_)) {
            issued = issued.saturating_add(trade.present);
            count = count.saturating_add(1);
        }
    }
    (issued, count)
}

fn expected_revenue_for(agent: &Agent, line: &ProjectLine, money_good: Option<GoodId>) -> Gold {
    if money_good == Some(line.output_good) {
        return Gold(u64::from(line.output_qty));
    }
    let belief = belief_for(agent, line.output_good);
    Gold(belief.expected.0.saturating_mul(u64::from(line.output_qty)))
}

fn input_cost_basis(
    agent: &Agent,
    input_goods: &[(GoodId, u32)],
    money_good: Option<GoodId>,
) -> Gold {
    input_goods_value(agent, input_goods, 10_000, money_good)
}

fn input_goods_value(
    agent: &Agent,
    input_goods: &[(GoodId, u32)],
    value_bps: u16,
    money_good: Option<GoodId>,
) -> Gold {
    let value_bps = u64::from(value_bps.min(10_000));
    aggregate_input_goods(input_goods)
        .iter()
        .fold(Gold::ZERO, |total, (good, qty)| {
            let valued_qty = u64::from(*qty).saturating_mul(value_bps) / 10_000;
            let unit_value = if money_good == Some(*good) {
                Gold(1)
            } else {
                belief_for(agent, *good).expected
            };
            total.saturating_add(Gold(unit_value.0.saturating_mul(valued_qty)))
        })
}

fn remaining_project_horizon(project: &M2Project, tick: Tick) -> u8 {
    let remaining = project.maturity.0.saturating_sub(tick.0).max(1);
    u8::try_from(remaining).unwrap_or(u8::MAX)
}

fn project_salvage_value(agent: &Agent, project: &M2Project, money_good: Option<GoodId>) -> Gold {
    input_goods_value(
        agent,
        &project.input_goods_committed,
        project.salvage_bps,
        money_good,
    )
}

fn add_good_volume(volumes: &mut Vec<(GoodId, u32)>, good: GoodId, qty: u32) {
    if let Some((_, volume)) = volumes.iter_mut().find(|(entry, _)| *entry == good) {
        *volume = volume.saturating_add(qty);
    } else {
        volumes.push((good, qty));
        volumes.sort_by_key(|(entry, _)| *entry);
    }
}

fn set_last_price(prices: &mut Vec<(GoodId, Gold)>, good: GoodId, price: Gold) {
    if let Some((_, last_price)) = prices.iter_mut().find(|(entry, _)| *entry == good) {
        *last_price = price;
    } else {
        prices.push((good, price));
        prices.sort_by_key(|(entry, _)| *entry);
    }
}

fn realized_basket_value(agent: &Agent, prices: &[(GoodId, Gold)]) -> i64 {
    let mut value = i64::try_from(agent.gold.0).unwrap_or(i64::MAX);
    for (good, price) in prices {
        let qty = u64::from(agent.stock.get(*good));
        let good_value = qty.saturating_mul(price.0);
        value = value.saturating_add(i64::try_from(good_value).unwrap_or(i64::MAX));
    }
    value
}

fn realized_consumed_basket_value(
    agent: AgentId,
    current: &Agent,
    initial: &Agent,
    prices: &[(GoodId, Gold)],
    trades: &[Trade],
) -> i64 {
    let mut value = 0i64;
    for (good, price) in prices {
        let acquired = u64::from(initial.stock.get(*good))
            .saturating_add(traded_qty(trades, agent, *good, true));
        let held_or_sold = u64::from(current.stock.get(*good))
            .saturating_add(traded_qty(trades, agent, *good, false));
        let consumed = acquired.saturating_sub(held_or_sold);
        let good_value = consumed.saturating_mul(price.0);
        value = value.saturating_add(i64::try_from(good_value).unwrap_or(i64::MAX));
    }
    value
}

fn traded_qty(trades: &[Trade], agent: AgentId, good: GoodId, bought: bool) -> u64 {
    trades
        .iter()
        .filter(|trade| {
            trade.good == good
                && (if bought {
                    trade.buyer == agent
                } else {
                    trade.seller == agent
                })
        })
        .fold(0u64, |total, trade| {
            total.saturating_add(u64::from(trade.qty))
        })
}

fn project_lines_for_scenario(scenario: crate::scenario::ScenarioName) -> Vec<ProjectLine> {
    match scenario.project_lines() {
        ScenarioProjectLines::None => Vec::new(),
        ScenarioProjectLines::Builtin => builtin_project_lines(),
        ScenarioProjectLines::BorrowToBuild => vec![borrow_to_build_line()],
        ScenarioProjectLines::CreditBoomLong => vec![credit_boom_long_line()],
    }
}

fn market_goods_for(scenario: &MarketScenario) -> Vec<GoodId> {
    let mut goods = BTreeSet::new();
    for good in [FOOD, WOOD, NET] {
        goods.insert(good);
    }
    for agent in &scenario.agents {
        for good in agent.stock.positive_goods() {
            goods.insert(good);
        }
        for want in &agent.scale {
            if let WantKind::Good(good) = want.kind {
                goods.insert(good);
            }
        }
    }
    for recipe in &scenario.recipes {
        goods.insert(recipe.output_good);
        if let Some((good, _)) = recipe.input_good {
            goods.insert(good);
        }
        if let Some(good) = recipe.required_tool {
            goods.insert(good);
        }
    }
    if let Some(money_good) = scenario.money.current_money_good() {
        goods.remove(&money_good);
    }
    goods.into_iter().collect()
}

fn max_good_id(market_goods: &[GoodId], money: &MarketMoneyConfig) -> u16 {
    let mut max = market_goods.iter().map(|good| good.0).max().unwrap_or(0);
    if let Some(money_good) = money.current_money_good() {
        max = max.max(money_good.0);
    }
    if let MarketMoneyConfig::Emergent(config) = money {
        for good in &config.candidate_goods {
            max = max.max(good.0);
        }
        for good in config.marketability.goods.keys() {
            max = max.max(good.0);
        }
    }
    max
}

fn migrate_initial_money_stock(agents: &mut [Agent], money_good: Option<GoodId>) -> bool {
    let Some(money_good) = money_good else {
        return true;
    };

    for agent in agents.iter() {
        let qty = agent.stock.get(money_good);
        if qty > 0 && agent.gold.checked_add(Gold(u64::from(qty))).is_none() {
            return false;
        }
    }

    for agent in agents {
        let qty = agent.stock.get(money_good);
        if qty == 0 {
            continue;
        }
        let removed = agent.stock.remove(money_good, qty);
        debug_assert!(removed);
        agent.gold = agent
            .gold
            .checked_add(Gold(u64::from(qty)))
            .expect("initial money-stock migration was preflighted");
    }
    true
}

fn direct_recipe_action_recipe_id(action: DirectRecipeAction) -> RecipeId {
    match action {
        DirectRecipeAction::GatherFood => RecipeId::GatherFood,
        DirectRecipeAction::CutWood => RecipeId::CutWood,
        DirectRecipeAction::FishWithNet => RecipeId::FishWithNet,
    }
}

fn direct_recipe_output(recipes: &[Recipe], recipe_id: RecipeId) -> Option<(GoodId, u32)> {
    recipes
        .iter()
        .find(|recipe| recipe.id == recipe_id)
        .map(|recipe| (recipe.output_good, recipe.output_qty))
}

fn belief_for(agent: &Agent, good: GoodId) -> PriceBelief {
    agent
        .expect
        .get(usize::from(good.0))
        .copied()
        .unwrap_or_else(|| PriceBelief::new(Gold(1), Gold(1)))
}

fn belief_mut(agent: &mut Agent, good: GoodId) -> &mut PriceBelief {
    let index = usize::from(good.0);
    if index >= agent.expect.len() {
        agent
            .expect
            .resize(index + 1, PriceBelief::new(Gold(1), Gold(1)));
    }
    &mut agent.expect[index]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Agent, AgentId, Role, Want, WantKind};
    use crate::agio::{AgioQuote, AgioSchedule};
    use crate::capital::{borrow_to_build_line, dry_fish_short_line, ProjectLineId};
    use crate::expect::PriceBelief;
    use crate::good::{Gold, GoodId, Horizon, Stock, CLOTH, FOOD, GOLD, SALT, WOOD};
    use crate::money::{
        DesignatedMoney, LaborWageTender, MarketMoneyConfig, MengerianConfig, ReserveRatioBps,
    };
    use crate::purpose::ProjectPlanId;
    use crate::scenario::{builtin_market_scenario, MarketScenario, ScenarioName};

    fn test_capitalist(stock: Stock) -> Agent {
        let slots = [GOLD, FOOD, WOOD]
            .into_iter()
            .map(|good| good.0)
            .max()
            .map(|good| usize::from(good) + 1)
            .unwrap_or(0);
        let mut expect = vec![PriceBelief::new(Gold::ZERO, Gold(1)); slots];
        expect[usize::from(FOOD.0)] = PriceBelief::new(Gold(1), Gold(1));
        expect[usize::from(WOOD.0)] = PriceBelief::new(Gold(1), Gold(1));

        Agent {
            id: AgentId(1),
            scale: Vec::new(),
            stock,
            gold: Gold(10),
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Capitalist],
            expect,
        }
    }

    fn test_society(agent: Agent) -> Society {
        Society::from_scenario(MarketScenario {
            name: "roundabout-capital",
            scenario: ScenarioName::RoundaboutCapital,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        })
    }

    fn consuming_test_agent() -> Agent {
        let mut stock = Stock::new(WOOD.0);
        stock.add(FOOD, 1);
        Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Now,
                qty: 1,
                satisfied: false,
            }],
            stock,
            gold: Gold::ZERO,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Consumer],
            expect: vec![PriceBelief::new(Gold(1), Gold(1)); usize::from(WOOD.0) + 1],
        }
    }

    // ---- S1: gated spot-bid override ------------------------------------------

    /// Belief slots wide enough for the lab goods used in the override tests.
    fn override_expect() -> Vec<PriceBelief> {
        let slots = [GOLD, FOOD, WOOD, SALT]
            .into_iter()
            .map(|good| usize::from(good.0))
            .max()
            .unwrap_or(0)
            + 1;
        vec![PriceBelief::new(Gold(1), Gold(1)); slots]
    }

    /// A buyer that, on its own scale, has NO want for `FOOD` and so posts no
    /// `FOOD` bid — the override is the only thing that can make it bid.
    fn override_buyer(gold: Gold) -> Agent {
        Agent {
            id: AgentId(2),
            scale: Vec::new(),
            stock: Stock::new(WOOD.0),
            gold,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Capitalist],
            expect: override_expect(),
        }
    }

    /// A seller holding one `FOOD`, willing to sell it for gold (its savings want
    /// out-ranks the `FOOD` want), so `reservation_ask(FOOD, 1) == Some(Gold(1))`.
    fn override_seller() -> Agent {
        let mut stock = Stock::new(WOOD.0);
        stock.add(FOOD, 1);
        Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(GOLD),
                    horizon: Horizon::Later(1),
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            gold: Gold::ZERO,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Consumer],
            expect: override_expect(),
        }
    }

    fn override_society(agents: Vec<Agent>) -> Society {
        Society::from_scenario(MarketScenario {
            name: "bid-override",
            scenario: ScenarioName::MarketPriceDiscovery,
            seed: 1,
            periods: 1,
            agents,
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        })
    }

    #[test]
    fn bid_override_enters_book_and_reserves_gold() {
        // A lone buyer with no FOOD want posts nothing on its own; with an override
        // it posts a resting FOOD bid into the real book and that bid reserves gold.
        let mut society = override_society(vec![override_buyer(Gold(5))]);
        let buyer = AgentId(2);

        society.step();
        assert_eq!(
            society.live_spot_quote_count_for_good(FOOD),
            0,
            "with no override the buyer posts no FOOD bid"
        );
        assert_eq!(
            society.free_gold_after_all_reserves(buyer),
            Gold(5),
            "nothing is reserved when no bid rests"
        );

        society.set_bid_override(buyer, FOOD, Gold(3), Gold(3));
        society.step();
        assert_eq!(
            society.live_spot_quote_count_for_good(FOOD),
            1,
            "the override posts exactly one resting FOOD bid into the real book"
        );
        assert_eq!(
            society.free_gold_after_all_reserves(buyer),
            Gold(2),
            "the resting override bid reserves its limit (3) out of the buyer's 5 gold"
        );
        // The override is one-shot: cleared at the end of the step, so it is gone.
        assert!(
            society.bid_overrides.is_empty(),
            "step clears the override table"
        );
    }

    #[test]
    fn bid_override_is_one_shot_on_try_step_and_run() {
        let buyer = AgentId(2);

        let mut try_step_society = override_society(vec![override_buyer(Gold(5))]);
        try_step_society.set_bid_override(buyer, FOOD, Gold(3), Gold(3));
        try_step_society.try_step().unwrap();
        assert!(
            try_step_society.bid_overrides.is_empty(),
            "try_step clears the override table"
        );
        assert_eq!(
            try_step_society.live_spot_quote_count_for_good(FOOD),
            1,
            "the first try_step posts the one-shot override bid"
        );

        try_step_society.try_step().unwrap();
        assert_eq!(
            try_step_society.live_spot_quote_count_for_good(FOOD),
            0,
            "without a reset override, the next try_step cancels the non-scale bid"
        );
        assert_eq!(
            try_step_society.free_gold_after_all_reserves(buyer),
            Gold(5),
            "the canceled one-shot bid releases the buyer's reserved gold"
        );

        let mut run_society = override_society(vec![override_buyer(Gold(5))]);
        run_society.set_bid_override(buyer, FOOD, Gold(3), Gold(3));
        run_society.run(2);
        assert!(
            run_society.bid_overrides.is_empty(),
            "run clears overrides through its try_run/try_step path"
        );
        assert_eq!(
            run_society.live_spot_quote_count_for_good(FOOD),
            0,
            "run does not keep re-posting a one-shot override on later periods"
        );
        assert_eq!(
            run_society.free_gold_after_all_reserves(buyer),
            Gold(5),
            "run releases the buyer's reserve after the one-shot bid is canceled"
        );
    }

    #[test]
    fn bid_override_fills_against_willing_ask_and_records_trade() {
        // Control: without the override the buyer has no FOOD want, so even though a
        // willing seller asks, no FOOD trade ever forms.
        let mut control = override_society(vec![override_seller(), override_buyer(Gold(5))]);
        control.step();
        assert!(
            control.trades.iter().all(|trade| trade.good != FOOD),
            "without the override there is no FOOD trade: {:?}",
            control.trades
        );

        // With the override the buyer's bid enters the book, crosses the seller's
        // ask, and records a real Trade through the sole `ensure_order` path.
        let mut society = override_society(vec![override_seller(), override_buyer(Gold(5))]);
        let buyer = AgentId(2);
        let seller = AgentId(1);
        let buyer_gold_before = society.agents.get(buyer).unwrap().gold;

        society.set_bid_override(buyer, FOOD, Gold(3), Gold(3));
        society.step();

        let food_trade = society
            .trades
            .iter()
            .find(|trade| trade.good == FOOD)
            .expect("the override bid should fill against the willing ask");
        assert_eq!(food_trade.buyer, buyer, "the override agent is the buyer");
        assert_eq!(food_trade.seller, seller, "the willing asker is the seller");
        assert_eq!(food_trade.qty, 1);
        assert!(
            food_trade.price >= Gold(1) && food_trade.price <= Gold(3),
            "the fill price sits between the ask and the override limit, got {:?}",
            food_trade.price
        );

        // The good and the money actually moved, conserved.
        assert_eq!(society.agents.get(buyer).unwrap().stock.get(FOOD), 1);
        assert_eq!(society.agents.get(seller).unwrap().stock.get(FOOD), 0);
        let paid = buyer_gold_before
            .checked_sub(society.agents.get(buyer).unwrap().gold)
            .unwrap();
        assert_eq!(
            paid, food_trade.price,
            "the buyer paid exactly the fill price"
        );
        assert_eq!(
            society.agents.get(seller).unwrap().gold,
            food_trade.price,
            "the seller received exactly the fill price"
        );
    }

    #[test]
    fn unused_bid_override_hook_is_byte_identical() {
        // The disabled-hook tripwire: a run that touches the (empty) override API
        // every tick is byte-identical to one that never touches it. Combined with
        // the m5..m9 conformance goldens, this proves the S1 seam is inert when no
        // override is set.
        let run = |touch_hook: bool| {
            let mut society =
                Society::from_scenario(builtin_market_scenario(ScenarioName::MarketPriceDiscovery));
            for _ in 0..16 {
                if touch_hook {
                    society.clear_bid_overrides();
                }
                society.step();
            }
            (society.records, society.trades)
        };

        let (plain_records, plain_trades) = run(false);
        let (hooked_records, hooked_trades) = run(true);
        assert_eq!(
            plain_records, hooked_records,
            "the unused override hook must not change market records"
        );
        assert_eq!(
            plain_trades, hooked_trades,
            "the unused override hook must not change trades"
        );
    }

    #[test]
    fn m1_metric_observations_are_captured_when_enabled() {
        let mut society =
            Society::from_scenario(builtin_market_scenario(ScenarioName::MarketBarterishGold));
        society.enable_metric_observations();

        society.step();

        assert_eq!(
            society.metric_observations.len(),
            1,
            "M1 should emit a metric observation when accumulation is enabled"
        );
        assert_eq!(society.metric_observations[0].tick, 0);
        assert!(
            !society.metric_observations[0].agent_wealth.is_empty(),
            "the observation should include agent wealth rows"
        );
    }

    #[test]
    fn v2_metric_observations_are_captured_after_promotion_when_enabled() {
        let mut society =
            Society::from_scenario(builtin_market_scenario(ScenarioName::MengerSaltMoney));
        society.enable_metric_observations();

        for _ in 0..12 {
            society.step();
        }

        assert!(
            society.money.current_money_good().is_some(),
            "the scenario should have promoted a money good"
        );
        assert!(
            !society.metric_observations.is_empty(),
            "V2 should emit metric observations once it reaches the money phase"
        );
    }

    #[test]
    fn direct_pass_consumption_capture_feeds_log_and_metrics() {
        let mut m1 = Society::from_scenario(MarketScenario {
            name: "m1-consumption-capture",
            scenario: ScenarioName::MarketPriceDiscovery,
            seed: 1,
            periods: 1,
            agents: vec![consuming_test_agent()],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        });
        m1.enable_consumption_log();
        m1.enable_metric_observations();

        m1.run_direct_pass_for_money(GOLD);

        let consumed = vec![(AgentId(1), FOOD, 1)];
        assert_eq!(m1.consumption_log, consumed);
        assert_eq!(m1.metric_consumed_goods, consumed);

        let mut v2 = Society::from_scenario(MarketScenario {
            name: "v2-consumption-capture",
            scenario: ScenarioName::MengerSaltMoney,
            seed: 1,
            periods: 1,
            agents: vec![consuming_test_agent()],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Emergent(MengerianConfig {
                candidate_goods: vec![FOOD, WOOD, SALT],
                ..MengerianConfig::default()
            }),
        });
        v2.enable_consumption_log();
        v2.enable_metric_observations();

        v2.run_direct_pass_without_money();

        assert_eq!(v2.consumption_log, consumed);
        assert_eq!(v2.metric_consumed_goods, consumed);
    }

    #[test]
    fn credit_stock_is_additive_and_id_addressed() {
        // The G2b transfer seam: credit a good to a live agent's stock and read
        // it back. Additive only — gold and scale are untouched — and a stale id
        // credits nothing (returns false).
        let mut society = test_society(test_capitalist(Stock::new(WOOD.0)));
        let id = AgentId(1);
        let gold_before = society.total_gold();

        assert_eq!(society.total_stock(FOOD), 0);
        assert!(society.credit_stock(id, FOOD, 5));
        assert!(society.credit_stock(id, FOOD, 2));
        assert_eq!(society.agents.get(id).unwrap().stock.get(FOOD), 7);
        assert_eq!(society.total_stock(FOOD), 7);
        assert_eq!(
            society.total_gold(),
            gold_before,
            "credit must not mint money"
        );

        // An unknown id is rejected and changes nothing.
        assert!(!society.credit_stock(AgentId(99), FOOD, 4));
        assert_eq!(society.total_stock(FOOD), 7);
    }

    #[test]
    fn credit_stock_rejects_removed_agent() {
        // G4a real removal: a dead colonist's slot is freed, so the additive seam
        // must reject its now-stale id — there is no slot to thaw.
        let mut society = test_society(test_capitalist(Stock::new(WOOD.0)));
        let id = AgentId(1);

        assert!(society.remove_agent(id).is_some());
        assert!(
            society.agents.get(id).is_none(),
            "removed id resolves to None"
        );
        assert!(!society.credit_stock(id, FOOD, 5));
        assert_eq!(society.total_stock(FOOD), 0);
    }

    #[test]
    fn debit_and_credit_gold_move_value_without_minting() {
        // The G2c caravan seam: the additive accessors MOVE value (debit one side,
        // credit another by the same amount) — they never create or destroy a unit
        // or a coin, and they never let a balance go negative.
        let mut society = test_society(test_capitalist(Stock::new(WOOD.0)));
        let id = AgentId(1);
        let gold_before = society.total_gold();

        // Stock first: credit, then debit it back out — net zero.
        assert!(society.credit_stock(id, FOOD, 5));
        assert!(society.debit_stock(id, FOOD, 2));
        assert_eq!(society.agents.get(id).unwrap().stock.get(FOOD), 3);
        assert_eq!(society.total_stock(FOOD), 3);
        // Over-debit is rejected atomically — nothing leaves, no negative.
        assert!(!society.debit_stock(id, FOOD, 4));
        assert_eq!(society.total_stock(FOOD), 3);

        // Gold: credit then debit, balance and total move together, no mint/burn.
        assert!(society.credit_gold(id, Gold(7)));
        assert_eq!(
            society.total_gold(),
            gold_before.saturating_add(Gold(7)),
            "credit_gold adds exactly the amount"
        );
        assert!(society.debit_gold(id, Gold(7)));
        assert_eq!(
            society.total_gold(),
            gold_before,
            "the paired debit restores the total"
        );
        // Over-debit gold is rejected; the balance never goes negative.
        let balance = society.agents.get(id).unwrap().gold;
        assert!(!society.debit_gold(id, balance.saturating_add(Gold(1))));
        assert_eq!(society.agents.get(id).unwrap().gold, balance);
    }

    #[test]
    fn remove_agent_reconciles_agent_order_for_a_middle_agent() {
        // A multi-agent M1 society: removing a middle agent must rebuild agent_order
        // so every entry resolves to a live survivor at its (relocated) arena
        // position, in unchanged priority order, with the freed id absent and the
        // survivors untouched. The reconciliation is the load-bearing G4a work.
        let mut society =
            Society::from_scenario(builtin_market_scenario(ScenarioName::MarketPriceDiscovery));
        society.step();

        // Live ids in activation order before the death.
        let order_ids_before: Vec<AgentId> = society
            .agent_order
            .iter()
            .map(|&pos| society.agents[pos].id)
            .collect();
        assert!(order_ids_before.len() >= 3, "need a middle agent to remove");
        let victim = order_ids_before[order_ids_before.len() / 2];
        let len_before = society.agents.len();

        assert!(
            society.remove_agent(victim).is_some(),
            "a live agent removes"
        );

        // The arena freed exactly one slot and the victim no longer resolves.
        assert_eq!(society.agents.len(), len_before - 1);
        assert!(
            society.agents.get(victim).is_none(),
            "freed id resolves to None"
        );

        // agent_order lists every survivor in the same relative order, each entry a
        // valid position whose agent is a live survivor — no dangling/relocated slot.
        let order_ids_after: Vec<AgentId> = society
            .agent_order
            .iter()
            .map(|&pos| {
                assert!(
                    pos < society.agents.len(),
                    "agent_order points past the live slice"
                );
                society.agents[pos].id
            })
            .collect();
        let expected: Vec<AgentId> = order_ids_before
            .iter()
            .copied()
            .filter(|&id| id != victim)
            .collect();
        assert_eq!(
            order_ids_after, expected,
            "survivors keep their order at the new positions"
        );

        // The victim's reservations are forgotten; a further tick runs without panic
        // (no stale order matching) and the survivors' closed gold stays conserved.
        assert_eq!(society.reservations.reserved_gold(victim), Gold::ZERO);
        let gold_after_removal = society.total_gold();
        society.step();
        assert_eq!(
            society.total_gold(),
            gold_after_removal,
            "survivors conserve gold across the next tick"
        );
    }

    /// A minimal market agent for the birth/insert tests: holds gold + a FOOD ask
    /// scale so it actually posts an order the tick after it is added.
    fn birth_agent(gold: Gold, food: u32) -> Agent {
        let mut stock = Stock::new(WOOD.0);
        stock.add(FOOD, food);
        let slots = usize::from(WOOD.0) + 1;
        Agent {
            id: AgentId(0), // overwritten by the arena on insert
            scale: vec![
                Want {
                    kind: WantKind::Good(GOLD),
                    horizon: Horizon::Later(4),
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(GOLD),
                    horizon: Horizon::Later(4),
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            gold,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Household],
            expect: vec![PriceBelief::new(Gold(2), Gold(1)); slots],
        }
    }

    #[test]
    fn add_agent_inserts_and_reconciles_agent_order() {
        // add_agent is the insert-side mirror of remove_agent: a new agent lands in
        // the arena, is appended to agent_order, gets a reservation slot, and
        // participates from the next step. Its endowment is whatever the caller
        // supplied (a transfer accounted upstream) — add_agent mints nothing.
        let mut society =
            Society::from_scenario(builtin_market_scenario(ScenarioName::MarketPriceDiscovery));
        society.step();

        let order_len_before = society.agent_order.len();
        let live_before = society.agents.len();
        let total_gold_before = society.total_gold();

        let newborn = birth_agent(Gold(3), 4);
        let id = society.add_agent(newborn);

        // The arena resolves the new id and it landed at the dense slice's end.
        let position = society
            .agents
            .position_of(id)
            .expect("the newborn resolves in the arena");
        assert_eq!(
            position, live_before,
            "the newborn appends to the live slice"
        );
        assert_eq!(society.agents.len(), live_before + 1);

        // agent_order grew by exactly one entry, pointing at the newborn's position;
        // every entry still resolves to a live agent (no dangling/relocated slot).
        assert_eq!(society.agent_order.len(), order_len_before + 1);
        assert_eq!(*society.agent_order.last().unwrap(), position);
        for &pos in &society.agent_order {
            assert!(
                pos < society.agents.len(),
                "agent_order points past live slice"
            );
        }

        // The newborn holds exactly the endowment it was handed — add_agent mints
        // nothing — so total gold rose only by the (caller-supplied) endowment.
        assert_eq!(society.agents.get(id).unwrap().gold, Gold(3));
        assert_eq!(
            society.total_gold(),
            total_gold_before.saturating_add(Gold(3)),
            "add_agent installs the endowment but mints no extra money"
        );

        // It participates the next tick without panic, and the run keeps clearing.
        society.step();
        assert!(
            society.agents.get(id).is_some(),
            "the newborn is still live after a step"
        );
    }

    #[test]
    fn add_agent_reuses_a_freed_slot_with_a_bumped_generation() {
        // Birth after a death reuses the freed numeric slot with a fresh generation,
        // so the dead ancestor's id stays None and the newborn's resolves.
        let mut society =
            Society::from_scenario(builtin_market_scenario(ScenarioName::MarketPriceDiscovery));
        society.step();

        let victim = society
            .agent_order
            .first()
            .map(|&pos| society.agents[pos].id);
        let victim = victim.expect("a live agent to remove");
        assert!(society.remove_agent(victim).is_some());
        assert!(society.agents.get(victim).is_none(), "victim is freed");

        let id = society.add_agent(birth_agent(Gold(1), 1));
        assert_eq!(
            id.index(),
            victim.index(),
            "the birth reuses the freed numeric slot"
        );
        assert!(
            id.generation() > victim.generation(),
            "reuse bumps the slot generation"
        );
        assert!(
            society.agents.get(victim).is_none(),
            "the stale ancestor id stays None after reuse"
        );
        assert!(
            society.agents.get(id).is_some(),
            "the reused id resolves to the newborn"
        );
        // The newborn carries no stale reservation and is not flagged dead.
        assert_eq!(society.reservations.reserved_gold(id), Gold::ZERO);
        society.step();
    }

    #[test]
    fn add_agent_then_remove_agent_round_trips_caches() {
        // A birth followed by the newborn's removal leaves every cache exactly as a
        // never-born society would: agent_order back to its length, no dangling
        // reservation, conserved holdings. The two operations are exact inverses.
        let mut society =
            Society::from_scenario(builtin_market_scenario(ScenarioName::MarketPriceDiscovery));
        society.step();

        let order_len = society.agent_order.len();
        let live = society.agents.len();

        let id = society.add_agent(birth_agent(Gold(2), 2));
        assert_eq!(society.agent_order.len(), order_len + 1);
        assert_eq!(society.agents.len(), live + 1);

        let estate = society.remove_agent(id).expect("the newborn removes");
        assert_eq!(estate.gold, Gold(2), "removal recovers the endowment");
        assert_eq!(society.agent_order.len(), order_len, "agent_order restored");
        assert_eq!(society.agents.len(), live, "live count restored");
        assert!(society.agents.get(id).is_none());
        assert_eq!(society.reservations.reserved_gold(id), Gold::ZERO);
        // Every surviving agent_order entry still resolves.
        for &pos in &society.agent_order {
            assert!(pos < society.agents.len());
        }
        society.step();
    }

    #[test]
    fn add_agent_extends_m3_money_system_without_minting() {
        let mut society = small_redemption_society();
        let total_before = society.total_gold();
        let money_before = society.money_system.as_ref().unwrap().snapshot();

        let id = society.add_agent(birth_agent(Gold(3), 1));

        let money_system = society.money_system.as_ref().unwrap();
        let balance = money_system
            .balance_snapshot(id)
            .expect("add_agent creates an empty M3 ledger row");
        assert_eq!(
            balance.spendable_total(),
            Gold::ZERO,
            "add_agent does not mint an M3 endowment from Agent.gold"
        );
        assert_eq!(
            society.agents.get(id).unwrap().gold,
            Gold::ZERO,
            "the newborn cache is reconciled from the ledger"
        );
        assert_eq!(society.total_gold(), total_before);
        assert_eq!(money_system.snapshot(), money_before);
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn transfer_gold_moves_m3_ledger_value_to_newborn() {
        let mut society = small_redemption_society();
        let parent = AgentId(1);
        let total_before = society.total_gold();
        let id = society.add_agent(birth_agent(Gold::ZERO, 1));

        assert!(society.transfer_gold(parent, id, Gold(1)));

        let money_system = society.money_system.as_ref().unwrap();
        assert_eq!(money_system.spendable_total(parent), Gold::ZERO);
        assert_eq!(money_system.spendable_total(id), Gold(1));
        assert_eq!(society.agents.get(parent).unwrap().gold, Gold::ZERO);
        assert_eq!(society.agents.get(id).unwrap().gold, Gold(1));
        assert_eq!(
            society.total_gold(),
            total_before,
            "the M3 transfer moves money, it does not mint"
        );
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn transfer_gold_refuses_to_spend_reserved_gold_m1() {
        // `transfer_gold` advertises *spendable* gold: a resting bid that has
        // earmarked part of the source's balance must not be raided by a transfer,
        // or a later clear/cancel would operate on an order it can no longer fund.
        let mut society = test_society(test_capitalist(Stock::new(WOOD.0)));
        let from = AgentId(1);
        let to = society.add_agent(birth_agent(Gold::ZERO, 0));
        let total_before = society.total_gold();

        // Agent 1 holds 10 gold; rest a bid that reserves 8 of it, leaving 2 free.
        let bid = Order {
            agent: from,
            side: OrderSide::Bid,
            good: FOOD,
            limit: Gold(8),
            qty: 1,
            seq: 1,
            expires_tick: 3,
        };
        assert!(society
            .reservations
            .reserve_order(society.agents.as_slice(), &bid));
        assert_eq!(society.free_gold_after_all_reserves(from), Gold(2));

        // Spending into the reservation is refused and mutates nothing.
        assert!(!society.transfer_gold(from, to, Gold(10)));
        assert!(!society.transfer_gold(from, to, Gold(3)));
        assert_eq!(society.agents.get(from).unwrap().gold, Gold(10));
        assert_eq!(society.agents.get(to).unwrap().gold, Gold::ZERO);
        assert_eq!(society.reservations.reserved_gold(from), Gold(8));

        // The unreserved remainder still transfers, and conserves.
        assert!(society.transfer_gold(from, to, Gold(2)));
        assert_eq!(society.agents.get(from).unwrap().gold, Gold(8));
        assert_eq!(society.agents.get(to).unwrap().gold, Gold(2));
        assert_eq!(society.free_gold_after_all_reserves(from), Gold::ZERO);
        assert_eq!(society.total_gold(), total_before);
    }

    #[test]
    fn transfer_gold_refuses_to_spend_reserved_gold_m3() {
        // Same spendable contract on the M3 ledger path: the ledger's own spendable
        // check is blind to Society-level order reservations, so the helper enforces
        // the guard for both regimes.
        let mut society = small_redemption_society();
        let from = AgentId(1);
        let to = AgentId(2);
        let total_before = society.total_gold();

        // Agent 1 holds Gold(1) spendable; a resting bid reserves all of it.
        let bid = Order {
            agent: from,
            side: OrderSide::Bid,
            good: FOOD,
            limit: Gold(1),
            qty: 1,
            seq: 1,
            expires_tick: 3,
        };
        assert!(society
            .reservations
            .reserve_order(society.agents.as_slice(), &bid));
        assert_eq!(society.free_gold_after_all_reserves(from), Gold::ZERO);

        // The reserved gold cannot be transferred, and nothing moves.
        assert!(!society.transfer_gold(from, to, Gold(1)));
        assert_eq!(society.total_gold(), total_before);
        assert!(society.money_ledgers_reconcile());

        // Once the reservation is released the same transfer succeeds and conserves.
        society.reservations.release_order(&bid);
        assert!(society.transfer_gold(from, to, Gold(1)));
        assert_eq!(society.total_gold(), total_before);
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn additive_accessors_reject_unknown_and_removed_ids() {
        let mut society = test_society(test_capitalist(Stock::new(WOOD.0)));
        let id = AgentId(1);
        // Seed some holdings to settle/reject later.
        assert!(society.credit_stock(id, FOOD, 5));
        assert!(society.credit_gold(id, Gold(5)));

        // Unknown ids are rejected on every accessor and change nothing.
        let unknown = AgentId(99);
        assert!(!society.debit_stock(unknown, FOOD, 1));
        assert!(!society.credit_gold(unknown, Gold(1)));
        assert!(!society.debit_gold(unknown, Gold(1)));
        assert_eq!(society.total_stock(FOOD), 5);

        // G4a real removal settles the estate out of the society (a conserved
        // hand-off) and frees the slot; the additive seam then rejects the stale id.
        let agent_gold = society.agents.get(id).unwrap().gold;
        let total_before = society.total_gold();
        let estate = society.remove_agent(id).expect("live id removes");
        assert_eq!(estate.gold, agent_gold, "estate carries the agent's gold");
        assert_eq!(
            estate.stock.get(FOOD),
            5,
            "estate carries the agent's stock"
        );
        assert!(
            society.agents.get(id).is_none(),
            "removed id resolves to None"
        );
        assert!(!society.debit_stock(id, FOOD, 1));
        assert!(!society.credit_gold(id, Gold(1)));
        assert!(!society.debit_gold(id, Gold(1)));
        assert_eq!(
            society.total_stock(FOOD),
            0,
            "settled stock left the society"
        );
        assert_eq!(
            society.total_gold(),
            total_before.saturating_sub(agent_gold),
            "settled gold left the society into the estate"
        );
    }

    #[test]
    fn gold_accessors_reject_ledger_backed_societies() {
        let mut society = small_redemption_society();
        let id = AgentId(1);
        let total_before = society.total_gold();
        let agent_gold_before = agent_gold_rows(society.agents.as_slice());
        let money_before = society.money_system.clone();

        assert!(society.money_ledgers_reconcile());
        assert!(!society.credit_gold(id, Gold(1)));
        assert!(!society.debit_gold(id, Gold(1)));

        assert_eq!(society.total_gold(), total_before);
        assert_eq!(
            agent_gold_rows(society.agents.as_slice()),
            agent_gold_before
        );
        assert_eq!(society.money_system, money_before);
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn gold_accessors_reject_emergent_money_societies() {
        // Emergent (Mengerian) money: the circulating medium is a *good* that
        // emerges from barter, not the `Agent.gold` field, and there is no
        // `MoneySystem` ledger — so `money_system.is_some()` alone would not catch
        // this regime. The gold accessors must still refuse to touch `Agent.gold`
        // here (it is not money), so they cannot mint a phantom balance that
        // `total_gold` would count.
        let mut society =
            Society::from_scenario(builtin_market_scenario(ScenarioName::MengerSaltMoney));
        assert!(
            society.money_system.is_none(),
            "Mengerian emergent money keeps no M3 ledger"
        );
        assert!(
            !society.uses_closed_gold_money(),
            "emergent money is not the closed-GOLD M1 regime"
        );

        let id = society.agents.as_slice()[0].id;
        let total_before = society.total_gold();
        let agent_gold_before = agent_gold_rows(society.agents.as_slice());

        assert!(
            !society.credit_gold(id, Gold(1)),
            "emergent money must reject credit_gold"
        );
        assert!(
            !society.debit_gold(id, Gold(1)),
            "emergent money must reject debit_gold"
        );

        assert_eq!(
            society.total_gold(),
            total_before,
            "no phantom gold was minted in an emergent-money regime"
        );
        assert_eq!(
            agent_gold_rows(society.agents.as_slice()),
            agent_gold_before,
            "no agent gold balance changed in an emergent-money regime"
        );
    }

    fn test_m3_society(agent: Agent, money_good: GoodId, recipes: Vec<Recipe>) -> Society {
        Society::from_scenario(MarketScenario {
            name: "commodity-credit-neutral",
            scenario: ScenarioName::CommodityCreditNeutral,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes,
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: money_good }),
        })
    }

    fn redemption_agent(id: u32, gold: Gold) -> Agent {
        Agent {
            id: AgentId(u64::from(id)),
            scale: Vec::new(),
            stock: Stock::new(3),
            gold,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        }
    }

    fn small_redemption_society() -> Society {
        let mut society = Society::from_scenario(MarketScenario {
            name: "small-redemption",
            scenario: ScenarioName::EmergedGoldReserveLeashControl,
            seed: 1,
            periods: 1,
            agents: vec![redemption_agent(2, Gold(1)), redemption_agent(1, Gold(1))],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        });
        society.regime = Regime::FractionalConvertible;
        society.banks[0].reserves = Gold(1);
        society.banks[0].demand_deposits = Gold(2);
        society.banks[0].fiduciary_issued = Gold(1);
        society.money_system = Some(
            MoneySystem::from_agents_with_banks(society.agents.as_slice(), &society.banks).unwrap(),
        );
        if let Some(money_system) = &society.money_system {
            money_system.reconcile_agent_cache(society.agents.as_mut_slice());
        }
        society
    }

    fn partial_redemption_society() -> Society {
        let mut society = Society::from_scenario(MarketScenario {
            name: "partial-redemption",
            scenario: ScenarioName::EmergedGoldReserveLeashControl,
            seed: 1,
            periods: 1,
            agents: vec![redemption_agent(1, Gold(2))],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        });
        society.regime = Regime::FractionalConvertible;
        society.banks[0].reserves = Gold(1);
        society.banks[0].demand_deposits = Gold(2);
        society.banks[0].fiduciary_issued = Gold(1);
        society.money_system = Some(
            MoneySystem::from_agents_with_banks(society.agents.as_slice(), &society.banks).unwrap(),
        );
        if let Some(money_system) = &society.money_system {
            money_system.reconcile_agent_cache(society.agents.as_mut_slice());
        }
        society
    }

    fn agent_gold_rows(agents: &[Agent]) -> Vec<(AgentId, Gold)> {
        let mut rows = agents
            .iter()
            .map(|agent| (agent.id, agent.gold))
            .collect::<Vec<_>>();
        rows.sort_by_key(|(agent, _)| *agent);
        rows
    }

    #[test]
    fn redemption_event_honors_until_reserves_exhausted() {
        let mut society = small_redemption_society();

        society.apply_redemption_event(
            BankId(1),
            &RedemptionRoute::AllClaimHolders,
            None,
            ApplyMode::Event,
        );

        assert_eq!(society.redemption_audit.len(), 2);
        assert_eq!(society.redemption_audit[0].agent, AgentId(1));
        assert_eq!(society.redemption_audit[0].requested, Gold(1));
        assert_eq!(society.redemption_audit[0].honored, Gold(1));
        assert_eq!(society.redemption_audit[0].failed, Gold::ZERO);
        assert_eq!(
            society.redemption_audit[0].outcome,
            RedemptionOutcome::Honored
        );
        assert_eq!(society.redemption_audit[1].agent, AgentId(2));
        assert_eq!(society.redemption_audit[1].requested, Gold(1));
        assert_eq!(society.redemption_audit[1].honored, Gold::ZERO);
        assert_eq!(society.redemption_audit[1].failed, Gold(1));
        assert_eq!(
            society.redemption_audit[1].outcome,
            RedemptionOutcome::ReserveExhausted
        );
        assert_eq!(society.banks[0].reserves, Gold::ZERO);
        assert_eq!(society.banks[0].demand_deposits, Gold(1));
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn redemption_event_partially_honors_single_oversized_claim() {
        let mut society = partial_redemption_society();

        society.apply_redemption_event(
            BankId(1),
            &RedemptionRoute::AllClaimHolders,
            None,
            ApplyMode::Event,
        );

        assert_eq!(society.redemption_audit.len(), 1);
        assert_eq!(society.redemption_audit[0].agent, AgentId(1));
        assert_eq!(society.redemption_audit[0].requested, Gold(2));
        assert_eq!(society.redemption_audit[0].honored, Gold(1));
        assert_eq!(society.redemption_audit[0].failed, Gold(1));
        assert_eq!(
            society.redemption_audit[0].outcome,
            RedemptionOutcome::PartiallyHonored
        );
        assert_eq!(society.banks[0].reserves, Gold::ZERO);
        assert_eq!(society.banks[0].demand_deposits, Gold(1));
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn targeted_redemption_route_honors_named_claim_holder() {
        let mut society = small_redemption_society();

        society.apply_redemption_event(
            BankId(1),
            &RedemptionRoute::Agents(vec![AgentId(2)]),
            None,
            ApplyMode::Event,
        );

        assert_eq!(society.redemption_audit.len(), 1);
        assert_eq!(society.redemption_audit[0].agent, AgentId(2));
        assert_eq!(society.redemption_audit[0].requested, Gold(1));
        assert_eq!(society.redemption_audit[0].honored, Gold(1));
        assert_eq!(society.redemption_audit[0].failed, Gold::ZERO);
        assert_eq!(
            society.redemption_audit[0].outcome,
            RedemptionOutcome::Honored
        );
        assert_eq!(society.banks[0].reserves, Gold::ZERO);
        assert_eq!(society.banks[0].demand_deposits, Gold(1));
        let money_system = society.money_system.as_ref().unwrap();
        assert_eq!(money_system.demand_claim_on(AgentId(1), BankId(1)), Gold(1));
        assert_eq!(
            money_system.demand_claim_on(AgentId(2), BankId(1)),
            Gold::ZERO
        );
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn redemption_event_respects_max_per_agent_cap() {
        let mut society = partial_redemption_society();
        society.banks[0].reserves = Gold(2);
        society.banks[0].fiduciary_issued = Gold::ZERO;
        society.money_system = Some(
            MoneySystem::from_agents_with_banks(society.agents.as_slice(), &society.banks).unwrap(),
        );
        if let Some(money_system) = &society.money_system {
            money_system.reconcile_agent_cache(society.agents.as_mut_slice());
        }

        society.apply_redemption_event(
            BankId(1),
            &RedemptionRoute::Agents(vec![AgentId(1)]),
            Some(Gold(1)),
            ApplyMode::Event,
        );

        assert_eq!(society.redemption_audit.len(), 1);
        assert_eq!(society.redemption_audit[0].agent, AgentId(1));
        assert_eq!(society.redemption_audit[0].requested, Gold(1));
        assert_eq!(society.redemption_audit[0].honored, Gold(1));
        assert_eq!(society.redemption_audit[0].failed, Gold::ZERO);
        assert_eq!(
            society.redemption_audit[0].outcome,
            RedemptionOutcome::Honored
        );
        assert_eq!(society.banks[0].reserves, Gold(1));
        assert_eq!(society.banks[0].demand_deposits, Gold(1));
        let money_system = society.money_system.as_ref().unwrap();
        assert_eq!(money_system.demand_claim_on(AgentId(1), BankId(1)), Gold(1));
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn targeted_redemption_route_zero_cap_emits_no_row_without_mutation() {
        let mut society = partial_redemption_society();
        let bank_before = society.banks.clone();
        let money_before = society.money_system.clone();
        let agent_gold_before = agent_gold_rows(society.agents.as_slice());

        society.apply_redemption_event(
            BankId(1),
            &RedemptionRoute::Agents(vec![AgentId(1)]),
            Some(Gold::ZERO),
            ApplyMode::Event,
        );

        // A zero cap requests nothing: it is neither a nonzero request nor a failure, so
        // no audit row is emitted (Honored requires requested > 0) and nothing mutates.
        assert!(society.redemption_audit.is_empty());
        assert_eq!(society.banks, bank_before);
        assert_eq!(society.money_system, money_before);
        assert_eq!(
            agent_gold_rows(society.agents.as_slice()),
            agent_gold_before
        );
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn suspended_redemption_event_records_refusal_without_mutation() {
        let mut society = small_redemption_society();
        society.regime = Regime::SuspendedConvertibility;
        let bank_before = society.banks.clone();
        let money_before = society.money_system.clone();
        let agent_gold_before = agent_gold_rows(society.agents.as_slice());

        society.apply_redemption_event(
            BankId(1),
            &RedemptionRoute::AllClaimHolders,
            None,
            ApplyMode::Event,
        );

        assert_eq!(society.redemption_audit.len(), 2);
        assert!(society.redemption_audit.iter().all(|row| {
            row.requested == Gold(1)
                && row.honored == Gold::ZERO
                && row.failed == Gold(1)
                && row.outcome == RedemptionOutcome::Suspended
        }));
        assert_eq!(society.banks, bank_before);
        assert_eq!(society.money_system, money_before);
        assert_eq!(
            agent_gold_rows(society.agents.as_slice()),
            agent_gold_before
        );
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn targeted_redemption_route_records_explicit_failures() {
        let mut society = small_redemption_society();
        let bank_before = society.banks.clone();
        let money_before = society.money_system.clone();
        let agent_gold_before = agent_gold_rows(society.agents.as_slice());

        society.apply_redemption_event(
            BankId(1),
            &RedemptionRoute::Agents(vec![AgentId(3)]),
            None,
            ApplyMode::Event,
        );

        assert_eq!(society.redemption_audit.len(), 1);
        assert_eq!(society.redemption_audit[0].agent, AgentId(3));
        assert_eq!(society.redemption_audit[0].requested, Gold::ZERO);
        assert_eq!(society.redemption_audit[0].honored, Gold::ZERO);
        assert_eq!(society.redemption_audit[0].failed, Gold::ZERO);
        assert_eq!(
            society.redemption_audit[0].outcome,
            RedemptionOutcome::NoClaim
        );
        assert_eq!(society.banks, bank_before);
        assert_eq!(society.money_system, money_before);
        assert_eq!(
            agent_gold_rows(society.agents.as_slice()),
            agent_gold_before
        );

        society.redemption_audit.clear();
        society.apply_redemption_event(
            BankId(2),
            &RedemptionRoute::Agents(vec![AgentId(1)]),
            None,
            ApplyMode::Event,
        );

        assert_eq!(society.redemption_audit.len(), 1);
        assert_eq!(society.redemption_audit[0].agent, AgentId(1));
        assert_eq!(society.redemption_audit[0].requested, Gold::ZERO);
        assert_eq!(society.redemption_audit[0].honored, Gold::ZERO);
        assert_eq!(society.redemption_audit[0].failed, Gold::ZERO);
        assert_eq!(
            society.redemption_audit[0].outcome,
            RedemptionOutcome::BankMissing
        );
        assert_eq!(society.banks, bank_before);
        assert_eq!(society.money_system, money_before);
        assert_eq!(
            agent_gold_rows(society.agents.as_slice()),
            agent_gold_before
        );
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn emergent_money_scenarios_construct_without_designated_money() {
        let agent = test_capitalist(Stock::new(6));

        let society = Society::from_scenario(MarketScenario {
            name: "phase-a-emergent-money",
            scenario: ScenarioName::MarketPriceDiscovery,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Emergent(MengerianConfig {
                candidate_goods: vec![SALT],
                ..MengerianConfig::default()
            }),
        });

        assert_eq!(society.money.current_money_good(), None);
        assert_eq!(society.max_good_id, SALT.0);
    }

    #[test]
    fn emergent_money_step_wrapper_is_noop_until_phase_b() {
        let agent = test_capitalist(Stock::new(6));

        let mut society = Society::from_scenario(MarketScenario {
            name: "phase-a-emergent-money",
            scenario: ScenarioName::MarketPriceDiscovery,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Emergent(MengerianConfig {
                candidate_goods: vec![SALT],
                ..MengerianConfig::default()
            }),
        });

        society.step();

        assert_eq!(society.tick, Tick(0));
        assert!(society.records.is_empty());
    }

    #[test]
    fn emergent_money_try_step_returns_typed_deferred_error() {
        let agent = test_capitalist(Stock::new(6));

        let mut society = Society::from_scenario(MarketScenario {
            name: "phase-a-emergent-money",
            scenario: ScenarioName::MarketPriceDiscovery,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Emergent(MengerianConfig {
                candidate_goods: vec![SALT],
                ..MengerianConfig::default()
            }),
        });

        assert_eq!(
            society.try_step(),
            Err(SocietyStepError::EmergentMoneyDeferred)
        );
        assert_eq!(
            society.try_run(1),
            Err(SocietyStepError::EmergentMoneyDeferred)
        );
    }

    #[test]
    fn v2_direct_pass_preserves_live_barter_reserved_stock() {
        let mut stock = Stock::new(6);
        stock.add(FOOD, 1);
        let agent = Agent {
            id: AgentId(1),
            scale: scale_entries(&[
                (WantKind::Good(SALT), Horizon::Next, 1),
                (WantKind::Good(WOOD), Horizon::Next, 1),
            ]),
            stock,
            gold: Gold::ZERO,
            labor_capacity: 1,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        };
        let mut society = Society::from_scenario(MarketScenario {
            name: "v2-reserved-barter-stock",
            scenario: ScenarioName::MengerSaltMoney,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: vec![Recipe {
                id: RecipeId::CutWood,
                name: "SpendFoodForWood",
                labor: 1,
                input_good: Some((FOOD, 1)),
                required_tool: None,
                output_good: WOOD,
                output_qty: 1,
                enabled: true,
            }],
            events: Vec::new(),
            money: MarketMoneyConfig::Emergent(MengerianConfig {
                candidate_goods: vec![FOOD, WOOD, SALT],
                min_total_acceptances: 12,
                promotion_threshold_bps: 4_500,
                lead_margin_bps: 1_500,
                min_acceptor_agents: 3,
                min_counterpart_goods: 2,
                stability_ticks: 2,
                indirect_min_acceptance_share_bps: 3_000,
                min_indirect_acceptances: 0,
                min_indirect_acceptor_agents: 0,
                min_indirect_target_goods: 0,
                allow_indirect_acceptance: true,
                multi_offer_medium: false,
                durability_aware_acceptance: false,
                two_layer_saleability: false,
                min_direct_use_acceptors: 0,
                marketability: Default::default(),
            }),
        });
        assert!(society.barter_book.post_offer(
            society.agents.as_slice(),
            BarterOffer {
                agent: AgentId(1),
                give_good: FOOD,
                receive_good: SALT,
                qty: 1,
                reason: BarterReason::DirectWant,
                seq: 1,
                expires_tick: 3,
            },
            0,
        ));

        society.step();

        assert_eq!(society.live_barter_offer_count(), 1);
        assert_eq!(society.agents[0].stock.get(FOOD), 1);
        assert_eq!(society.agents[0].stock.get(WOOD), 0);
    }

    #[test]
    fn v2_direct_pass_can_use_stock_released_by_expired_barter_offer() {
        let mut stock = Stock::new(6);
        stock.add(FOOD, 1);
        let agent = Agent {
            id: AgentId(1),
            scale: scale_entries(&[
                (WantKind::Good(SALT), Horizon::Next, 1),
                (WantKind::Good(WOOD), Horizon::Next, 1),
            ]),
            stock,
            gold: Gold::ZERO,
            labor_capacity: 1,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        };
        let mut society = Society::from_scenario(MarketScenario {
            name: "v2-expired-barter-stock",
            scenario: ScenarioName::MengerSaltMoney,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: vec![Recipe {
                id: RecipeId::CutWood,
                name: "SpendFoodForWood",
                labor: 1,
                input_good: Some((FOOD, 1)),
                required_tool: None,
                output_good: WOOD,
                output_qty: 1,
                enabled: true,
            }],
            events: Vec::new(),
            money: MarketMoneyConfig::Emergent(MengerianConfig {
                candidate_goods: vec![FOOD, WOOD, SALT],
                min_total_acceptances: 12,
                promotion_threshold_bps: 4_500,
                lead_margin_bps: 1_500,
                min_acceptor_agents: 3,
                min_counterpart_goods: 2,
                stability_ticks: 2,
                indirect_min_acceptance_share_bps: 3_000,
                min_indirect_acceptances: 0,
                min_indirect_acceptor_agents: 0,
                min_indirect_target_goods: 0,
                allow_indirect_acceptance: true,
                multi_offer_medium: false,
                durability_aware_acceptance: false,
                two_layer_saleability: false,
                min_direct_use_acceptors: 0,
                marketability: Default::default(),
            }),
        });
        assert!(society.barter_book.post_offer(
            society.agents.as_slice(),
            BarterOffer {
                agent: AgentId(1),
                give_good: FOOD,
                receive_good: SALT,
                qty: 1,
                reason: BarterReason::DirectWant,
                seq: 1,
                expires_tick: 1,
            },
            0,
        ));
        society.tick = Tick(1);

        society.step();

        assert_eq!(society.agents[0].stock.get(FOOD), 0);
        assert_eq!(society.agents[0].stock.get(WOOD), 1);
    }

    fn two_lane_micro_agent(id: u32, output: GoodId, input: GoodId) -> Agent {
        let mut stock = Stock::new(CLOTH.0);
        stock.add(output, 1);
        stock.add(SALT, 1);
        Agent {
            id: AgentId(u64::from(id)),
            scale: scale_entries(&[(WantKind::Good(input), Horizon::Next, 2)]),
            stock,
            gold: Gold::ZERO,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        }
    }

    fn two_lane_micro_society(multi_offer_medium: bool) -> Society {
        Society::from_scenario(MarketScenario {
            name: "two-lane-microtest",
            scenario: ScenarioName::MengerSaltMoney,
            seed: 1,
            periods: 1,
            agents: vec![
                two_lane_micro_agent(1, FOOD, WOOD),
                two_lane_micro_agent(2, WOOD, CLOTH),
                two_lane_micro_agent(3, CLOTH, FOOD),
            ],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Emergent(MengerianConfig {
                candidate_goods: vec![FOOD, WOOD, CLOTH, SALT],
                multi_offer_medium,
                ..MengerianConfig::default()
            }),
        })
    }

    #[test]
    fn two_layer_without_multi_offer_preserves_single_offer_context() {
        let mut society = Society::from_scenario(MarketScenario {
            name: "two-layer-one-offer-policy",
            scenario: ScenarioName::MengerSaltMoney,
            seed: 1,
            periods: 1,
            agents: vec![
                two_lane_micro_agent(1, FOOD, WOOD),
                two_lane_micro_agent(2, WOOD, CLOTH),
                two_lane_micro_agent(3, CLOTH, FOOD),
            ],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Emergent(MengerianConfig {
                candidate_goods: vec![FOOD, WOOD, CLOTH, SALT],
                two_layer_saleability: true,
                min_direct_use_acceptors: 0,
                multi_offer_medium: false,
                ..MengerianConfig::default()
            }),
        });

        let context = society.v2_saleability_context();
        assert_eq!(context, SaleabilityContext::single(None));

        society.generate_direct_barter_offers(&context);
        society.generate_indirect_barter_offers(&context);

        for agent in [AgentId(1), AgentId(2), AgentId(3)] {
            assert!(
                society
                    .live_barter_offers()
                    .iter()
                    .filter(|offer| offer.agent == agent)
                    .count()
                    <= 1,
                "multi_offer_medium=false must retain the one-live-offer policy"
            );
        }
    }

    #[test]
    fn candidate_sell_lane_skips_extra_candidate_when_surplus_stock_is_reserved() {
        let mut stock = Stock::new(CLOTH.0);
        stock.add(FOOD, 2);
        let agent = Agent {
            id: AgentId(1),
            scale: scale_entries(&[(WantKind::Good(WOOD), Horizon::Next, 1)]),
            stock,
            gold: Gold::ZERO,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        };
        let mut society = Society::from_scenario(MarketScenario {
            name: "candidate-sell-lane-reservation",
            scenario: ScenarioName::MengerSaltMoney,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Emergent(MengerianConfig {
                candidate_goods: vec![GOLD, FOOD, WOOD, SALT, CLOTH],
                two_layer_saleability: true,
                min_direct_use_acceptors: 0,
                multi_offer_medium: true,
                ..MengerianConfig::default()
            }),
        });
        let context = SaleabilityContext::candidates(vec![SALT, CLOTH, GOLD]);
        let target_goods = vec![WOOD];

        society.post_first_medium_sell_offer(0, AgentId(1), &target_goods, SALT, &context);
        society.post_first_medium_sell_offer(0, AgentId(1), &target_goods, CLOTH, &context);
        assert_eq!(society.barter_book.reserved_qty(AgentId(1), FOOD), 2);

        society.post_first_medium_sell_offer(0, AgentId(1), &target_goods, GOLD, &context);

        let receive_goods = society
            .live_barter_offers()
            .iter()
            .filter(|offer| {
                offer.agent == AgentId(1)
                    && offer.give_good == FOOD
                    && matches!(offer.reason, BarterReason::IndirectFor { target } if target == WOOD)
            })
            .map(|offer| offer.receive_good)
            .collect::<Vec<_>>();
        assert_eq!(receive_goods, vec![SALT, CLOTH]);
        assert_eq!(society.barter_book.reserved_qty(AgentId(1), FOOD), 2);
    }

    fn live_offer_lane_counts(society: &Society, agent: AgentId) -> (usize, usize) {
        let mut spend = 0;
        let mut sell = 0;
        for offer in society
            .live_barter_offers()
            .iter()
            .filter(|offer| offer.agent == agent)
        {
            if offer.give_good == SALT
                && offer.receive_good != SALT
                && matches!(offer.reason, BarterReason::DirectWant)
            {
                spend += 1;
            }
            if offer.give_good != SALT
                && offer.receive_good == SALT
                && matches!(offer.reason, BarterReason::IndirectFor { .. })
            {
                sell += 1;
            }
        }
        (spend, sell)
    }

    fn salt_indirect_acceptances(society: &Society) -> u64 {
        society
            .emergence()
            .expect("emergent state")
            .tracker()
            .candidate_saleability()
            .find(|candidate| candidate.good == SALT)
            .map(|candidate| candidate.indirect_acceptances)
            .unwrap_or(0)
    }

    #[test]
    fn two_lane_microtest_off_deadlocks_on_clears() {
        let mut off = two_lane_micro_society(false);
        let salt_context = SaleabilityContext::single(Some(SALT));
        off.generate_direct_barter_offers(&salt_context);
        off.generate_indirect_barter_offers(&salt_context);
        assert_eq!(off.live_barter_offer_count(), 3);
        assert!(
            off.live_barter_offers().iter().all(|offer| matches!(
                offer.reason,
                BarterReason::IndirectFor { .. }
            ) && offer.receive_good == SALT),
            "legacy replacement leaves only sell-for-SALT offers live"
        );
        let off_trades = off.barter_book.clear_matches_with_saleability_context(
            off.agents.as_mut_slice(),
            off.tick.0,
            &salt_context,
        );
        off.v2_observe_barter_trades(&off_trades);
        assert_eq!(off_trades.len(), 0);
        assert_eq!(salt_indirect_acceptances(&off), 0);

        let mut on = two_lane_micro_society(true);
        on.generate_direct_barter_offers(&salt_context);
        on.generate_indirect_barter_offers(&salt_context);
        assert_eq!(on.live_barter_offer_count(), 6);
        for id in [AgentId(1), AgentId(2), AgentId(3)] {
            assert_eq!(
                live_offer_lane_counts(&on, id),
                (1, 1),
                "the spend and sell lanes must coexist for agent {id:?}"
            );
        }
        let on_trades = on.barter_book.clear_matches_with_saleability_context(
            on.agents.as_mut_slice(),
            on.tick.0,
            &salt_context,
        );
        on.v2_observe_barter_trades(&on_trades);
        assert_eq!(on_trades.len(), 3);
        assert_eq!(salt_indirect_acceptances(&on), 3);
        for id in [AgentId(1), AgentId(2), AgentId(3)] {
            assert_eq!(
                live_offer_lane_counts(&on, id),
                (0, 0),
                "both lanes should fill and leave no live offer for agent {id:?}"
            );
        }
        assert!(on_trades.iter().all(|trade| {
            trade.a != trade.b
                && (trade.a_gives == SALT || trade.b_gives == SALT)
                && (matches!(trade.a_reason, BarterReason::IndirectFor { .. })
                    || matches!(trade.b_reason, BarterReason::IndirectFor { .. }))
        }));
    }

    #[test]
    fn two_lane_preserves_direct_leader_bids() {
        let direct_salt_user = Agent {
            id: AgentId(1),
            scale: scale_entries(&[(WantKind::Good(SALT), Horizon::Next, 1)]),
            stock: {
                let mut stock = Stock::new(CLOTH.0);
                stock.add(FOOD, 1);
                stock
            },
            gold: Gold::ZERO,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        };
        let salt_spender = Agent {
            id: AgentId(2),
            scale: scale_entries(&[(WantKind::Good(FOOD), Horizon::Next, 1)]),
            stock: {
                let mut stock = Stock::new(CLOTH.0);
                stock.add(SALT, 1);
                stock
            },
            gold: Gold::ZERO,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        };
        let mut society = Society::from_scenario(MarketScenario {
            name: "two-lane-direct-leader",
            scenario: ScenarioName::MengerSaltMoney,
            seed: 1,
            periods: 1,
            agents: vec![direct_salt_user, salt_spender],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Emergent(MengerianConfig {
                candidate_goods: vec![FOOD, SALT],
                multi_offer_medium: true,
                ..MengerianConfig::default()
            }),
        });

        let salt_context = SaleabilityContext::single(Some(SALT));
        society.generate_direct_barter_offers(&salt_context);
        society.generate_indirect_barter_offers(&salt_context);

        assert!(society.live_barter_offers().iter().any(|offer| {
            offer.agent == AgentId(1)
                && offer.give_good == FOOD
                && offer.receive_good == SALT
                && matches!(offer.reason, BarterReason::DirectWant)
        }));
        assert!(society
            .live_barter_offers()
            .iter()
            .filter(|offer| offer.agent == AgentId(1))
            .all(|offer| !matches!(offer.reason, BarterReason::IndirectFor { .. })));

        let trades = society.barter_book.clear_matches_with_saleability_context(
            society.agents.as_mut_slice(),
            society.tick.0,
            &salt_context,
        );
        assert_eq!(trades.len(), 1);
        assert!(matches!(trades[0].a_reason, BarterReason::DirectWant));
        assert!(matches!(trades[0].b_reason, BarterReason::DirectWant));
        assert_eq!(society.agents[0].stock.get(SALT), 1);
        assert_eq!(society.agents[1].stock.get(FOOD), 1);
    }

    #[test]
    fn two_lane_collapses_extra_lane_when_leader_disappears() {
        // Under a leader the two-lane path can leave an agent holding two live
        // `DirectWant` offers (the direct-leader bid `give output -> leader` plus
        // the spend lane `give leader -> input`). If the provisional leader later
        // becomes `None`, `cancel_invalid(.., None)` keeps both (it only drops
        // `IndirectFor` offers), so the no-leader direct pass must collapse them
        // back to the legacy one-live-offer policy. Two clean `DirectWant` offers
        // reproduce that post-leader state (once the leader is gone the goods are
        // ordinary, so only the lane count matters).
        let agent = Agent {
            id: AgentId(1),
            scale: scale_entries(&[
                (WantKind::Good(SALT), Horizon::Next, 1),
                (WantKind::Good(CLOTH), Horizon::Next, 1),
            ]),
            stock: {
                let mut stock = Stock::new(CLOTH.0);
                stock.add(FOOD, 1);
                stock.add(WOOD, 1);
                stock
            },
            gold: Gold::ZERO,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        };
        let mut society = Society::from_scenario(MarketScenario {
            name: "two-lane-collapse",
            scenario: ScenarioName::MengerSaltMoney,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Emergent(MengerianConfig {
                candidate_goods: vec![FOOD, WOOD, CLOTH, SALT],
                multi_offer_medium: true,
                ..MengerianConfig::default()
            }),
        });

        assert!(society.barter_book.post_offer(
            society.agents.as_slice(),
            BarterOffer {
                agent: AgentId(1),
                give_good: FOOD,
                receive_good: SALT,
                qty: 1,
                reason: BarterReason::DirectWant,
                seq: 1,
                expires_tick: 100,
            },
            0,
        ));
        assert!(society.barter_book.post_offer(
            society.agents.as_slice(),
            BarterOffer {
                agent: AgentId(1),
                give_good: WOOD,
                receive_good: CLOTH,
                qty: 1,
                reason: BarterReason::DirectWant,
                seq: 2,
                expires_tick: 100,
            },
            0,
        ));

        // The leader has disappeared; `cancel_invalid(.., None)` keeps both
        // `DirectWant` lanes — the leak this fix addresses.
        society
            .barter_book
            .cancel_invalid(society.agents.as_slice(), None);
        assert_eq!(
            society
                .live_barter_offers()
                .iter()
                .filter(|offer| offer.agent == AgentId(1))
                .count(),
            2,
            "cancel_invalid(.., None) leaves both direct lanes live"
        );

        society.generate_direct_barter_offers(&SaleabilityContext::single(None));

        assert_eq!(
            society
                .live_barter_offers()
                .iter()
                .filter(|offer| offer.agent == AgentId(1))
                .count(),
            1,
            "the no-leader direct pass must restore the one-live-offer policy"
        );
    }

    #[test]
    fn initial_money_stock_overflow_blocks_migration_without_panic() {
        let mut stock = Stock::new(6);
        stock.add(SALT, 1);
        let mut agent = test_capitalist(stock);
        agent.gold = Gold(u64::MAX);
        let mut agents = vec![agent];

        assert!(!migrate_initial_money_stock(&mut agents, Some(SALT)));
        assert_eq!(agents[0].gold, Gold(u64::MAX));
        assert_eq!(agents[0].stock.get(SALT), 1);
    }

    #[test]
    fn initial_money_stock_migration_failure_leaves_all_agents_unchanged() {
        let mut first_stock = Stock::new(6);
        first_stock.add(SALT, 1);
        let mut first = test_capitalist(first_stock);
        first.gold = Gold::ZERO;

        let mut second_stock = Stock::new(6);
        second_stock.add(SALT, 1);
        let mut second = test_capitalist(second_stock);
        second.id = AgentId(2);
        second.gold = Gold(u64::MAX);

        let mut agents = vec![first, second];

        assert!(!migrate_initial_money_stock(&mut agents, Some(SALT)));
        assert_eq!(agents[0].gold, Gold::ZERO);
        assert_eq!(agents[0].stock.get(SALT), 1);
        assert_eq!(agents[1].gold, Gold(u64::MAX));
        assert_eq!(agents[1].stock.get(SALT), 1);
    }

    #[test]
    fn non_gold_initial_money_stock_adds_to_existing_balance() {
        let mut stock = Stock::new(6);
        stock.add(SALT, 1);
        let mut agent = test_capitalist(stock);
        agent.gold = Gold(1);
        let mut agents = vec![agent];

        assert!(migrate_initial_money_stock(&mut agents, Some(SALT)));
        assert_eq!(agents[0].gold, Gold(2));
        assert_eq!(agents[0].stock.get(SALT), 0);
    }

    #[test]
    fn designated_gold_initial_stock_migrates_to_money_balance() {
        let mut stock = Stock::new(6);
        stock.add(GOLD, 4);
        let mut agent = test_capitalist(stock);
        agent.gold = Gold(1);
        let mut agents = vec![agent];

        assert!(migrate_initial_money_stock(&mut agents, Some(GOLD)));
        assert_eq!(agents[0].gold, Gold(5));
        assert_eq!(agents[0].stock.get(GOLD), 0);
    }

    #[test]
    fn designated_non_gold_money_stock_migrates_with_existing_balance() {
        let mut stock = Stock::new(6);
        stock.add(SALT, 1);
        let mut agent = test_capitalist(stock);
        agent.gold = Gold(1);

        let society = Society::from_scenario(MarketScenario {
            name: "salt-money-existing-balance",
            scenario: ScenarioName::MarketPriceDiscovery,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: SALT }),
        });

        assert_eq!(society.agents[0].gold, Gold(2));
        assert_eq!(society.agents[0].stock.get(SALT), 0);
    }

    #[test]
    fn initial_money_stock_overflow_fails_during_society_construction() {
        let mut stock = Stock::new(6);
        stock.add(SALT, 1);
        let mut agent = test_capitalist(stock);
        agent.gold = Gold(u64::MAX);

        let result = Society::try_from_scenario(MarketScenario {
            name: "overflowing-salt-money",
            scenario: ScenarioName::MarketPriceDiscovery,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: SALT }),
        });

        assert!(matches!(
            result,
            Err(SocietyBuildError::InitialDesignatedMoneyStockOverflow)
        ));
    }

    #[test]
    fn m3_initial_money_system_overflow_fails_loudly() {
        let mut first = test_capitalist(Stock::new(3));
        first.gold = Gold(u64::MAX);
        let mut second = test_capitalist(Stock::new(3));
        second.id = AgentId(2);
        second.gold = Gold(1);
        let result = Society::try_from_scenario(MarketScenario {
            name: "overflowing-m3-money",
            scenario: ScenarioName::CommodityCreditNeutral,
            seed: 1,
            periods: 1,
            agents: vec![first, second],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        });

        assert!(matches!(
            result,
            Err(SocietyBuildError::M3InitialBankDepositsInvalid)
        ));
    }

    #[test]
    fn m2_schedules_use_designated_money_good() {
        let mut agent = test_capitalist(Stock::new(6));
        agent.gold = Gold(2);
        agent.scale = scale_entries(&[(WantKind::Good(SALT), Horizon::Later(4), 4)]);
        let society = Society::from_scenario(MarketScenario {
            name: "salt-money-m2-schedule",
            scenario: ScenarioName::RoundaboutCapital,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: SALT }),
        });

        let schedules = society.agent_schedules(SALT);

        assert_eq!(
            schedules[0].1.min_future_due_for_lending(Gold(1), 4),
            Some(Gold(2))
        );
    }

    #[test]
    fn m2_labor_asks_use_designated_money_good() {
        let mut agent = test_capitalist(Stock::new(6));
        agent.gold = Gold::ZERO;
        agent.labor_capacity = 1;
        agent.scale = scale_entries(&[(WantKind::Good(SALT), Horizon::Now, 1)]);
        let mut society = Society::from_scenario(MarketScenario {
            name: "salt-money-m2-labor",
            scenario: ScenarioName::RoundaboutCapital,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: SALT }),
        });

        society.post_labor_asks(SALT);

        assert_eq!(
            society
                .labor_book
                .live_order(AgentId(1), FactorSide::Work)
                .map(|order| order.wage_limit),
            Some(Gold(1))
        );
    }

    #[test]
    fn direct_labor_money_good_output_updates_balance() {
        let mut agent = test_capitalist(Stock::new(6));
        agent.gold = Gold::ZERO;
        agent.labor_capacity = 1;
        agent.scale = scale_entries(&[(WantKind::Good(SALT), Horizon::Now, 1)]);
        let mut society = Society::from_scenario(MarketScenario {
            name: "salt-money-direct-output",
            scenario: ScenarioName::MarketPriceDiscovery,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: vec![Recipe {
                id: RecipeId::CutWood,
                name: "CutSalt",
                labor: 1,
                input_good: None,
                required_tool: None,
                output_good: SALT,
                output_qty: 1,
                enabled: true,
            }],
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: SALT }),
        });

        society.step();

        assert_eq!(society.agents[0].gold, Gold(1));
        assert_eq!(society.agents[0].stock.get(SALT), 0);
        assert!(society.agents[0].scale[0].satisfied);
    }

    #[test]
    fn m3_direct_labor_money_good_output_updates_ledgers() {
        let mut agent = test_capitalist(Stock::new(6));
        agent.gold = Gold::ZERO;
        agent.labor_capacity = 1;
        agent.scale = scale_entries(&[(WantKind::Good(SALT), Horizon::Now, 1)]);
        let mut society = test_m3_society(
            agent,
            SALT,
            vec![Recipe {
                id: RecipeId::CutWood,
                name: "CutSalt",
                labor: 1,
                input_good: None,
                required_tool: None,
                output_good: SALT,
                output_qty: 1,
                enabled: true,
            }],
        );

        society.step();

        let money_system = society
            .money_system
            .as_ref()
            .expect("M3 ledger is initialized");
        assert_eq!(society.agents[0].gold, Gold(1));
        assert_eq!(society.agents[0].stock.get(SALT), 0);
        assert_eq!(money_system.snapshot().public_specie, Gold(1));
        assert_eq!(money_system.base.commodity_base, Gold(1));
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn m3_direct_labor_money_good_input_is_not_spent_as_balance() {
        let mut agent = test_capitalist(Stock::new(6));
        agent.gold = Gold(1);
        agent.labor_capacity = 1;
        agent.scale = scale_entries(&[(WantKind::Good(FOOD), Horizon::Now, 1)]);
        let mut society = test_m3_society(
            agent,
            SALT,
            vec![Recipe {
                id: RecipeId::CutWood,
                name: "SpendSalt",
                labor: 1,
                input_good: Some((SALT, 1)),
                required_tool: None,
                output_good: FOOD,
                output_qty: 1,
                enabled: true,
            }],
        );

        society.step();

        let money_system = society
            .money_system
            .as_ref()
            .expect("M3 ledger is initialized");
        assert_eq!(society.agents[0].gold, Gold(1));
        assert_eq!(society.agents[0].stock.get(FOOD), 0);
        assert!(!society.agents[0].scale[0].satisfied);
        assert_eq!(money_system.snapshot().public_specie, Gold(1));
        assert_eq!(money_system.base.commodity_base, Gold(1));
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn money_good_project_expected_revenue_is_priced_at_par() {
        let mut agent = test_capitalist(Stock::new(6));
        belief_mut(&mut agent, SALT).expected = Gold(99);
        let line = ProjectLine {
            id: ProjectLineId(77),
            name: "SaltMoneyOutput",
            input_goods: Vec::new(),
            required_labor: 0,
            period_len: 1,
            output_good: SALT,
            output_qty: 3,
            salvage_bps: 5000,
        };

        assert_eq!(expected_revenue_for(&agent, &line, Some(SALT)), Gold(3));
        assert_eq!(expected_revenue_for(&agent, &line, Some(GOLD)), Gold(297));
    }

    #[test]
    fn money_good_project_input_values_are_priced_at_par() {
        let mut agent = test_capitalist(Stock::new(6));
        belief_mut(&mut agent, SALT).expected = Gold(99);

        assert_eq!(input_cost_basis(&agent, &[(SALT, 3)], Some(SALT)), Gold(3));
        assert_eq!(
            input_goods_value(&agent, &[(SALT, 3)], 5_000, Some(SALT)),
            Gold(1)
        );
        assert_eq!(
            input_cost_basis(&agent, &[(SALT, 3)], Some(GOLD)),
            Gold(297)
        );
        assert_eq!(
            input_goods_value(&agent, &[(SALT, 3)], 5_000, Some(GOLD)),
            Gold(99)
        );
    }

    #[test]
    fn money_good_project_output_moves_to_money_balance() {
        let mut society = test_society(test_capitalist(Stock::new(3)));
        society.m2_projects.push(M2Project {
            id: M2ProjectId(99),
            owner: AgentId(1),
            line: ProjectLineId(99),
            state: M2ProjectState::Waiting,
            started_at: Tick(0),
            maturity: Tick(0),
            labor_advanced: 1,
            input_goods_committed: Vec::new(),
            input_cost_basis: Gold::ZERO,
            advanced_gold: Gold(1),
            expected_revenue: Gold(3),
            output_good: GOLD,
            output_qty: 3,
            salvage_bps: 5000,
        });

        society.mature_waiting_projects();

        assert_eq!(society.agents[0].gold, Gold(13));
        assert_eq!(society.agents[0].stock.get(GOLD), 0);
        assert_eq!(society.m2_projects[0].state, M2ProjectState::Sold);
        assert_eq!(society.project_output_lots[0].qty_remaining, 0);
        assert_eq!(society.project_output_lots[0].proceeds, Gold(3));
        assert_eq!(society.project_revenue, Gold(3));
    }

    #[test]
    fn removal_freezes_owner_waiting_project_output() {
        // The same Waiting project that `money_good_project_output_moves_to_money_balance`
        // matures into a Gold credit must instead stay frozen once its owner dies:
        // G4a settles a dead colonist's gold/stock to the commons but NOT its
        // capital (heirs/capital are G4b), so the project never matures or abandons.
        // Without the lifecycle dead-owner guards, a normal step would first skip
        // maturation but then abandon the project because the freed owner has no
        // active schedule, returning salvage into a slot that no longer resolves.
        let mut society = test_society(test_capitalist(Stock::new(3)));
        society.m2_projects.push(M2Project {
            id: M2ProjectId(99),
            owner: AgentId(1),
            line: ProjectLineId(99),
            state: M2ProjectState::Waiting,
            started_at: Tick(0),
            maturity: Tick(0),
            labor_advanced: 1,
            input_goods_committed: Vec::new(),
            input_cost_basis: Gold::ZERO,
            advanced_gold: Gold(1),
            expected_revenue: Gold(3),
            output_good: GOLD,
            output_qty: 3,
            salvage_bps: 5000,
        });

        assert!(society.remove_agent(AgentId(1)).is_some());
        society.step();

        // Frozen: the project is still Waiting (not Mature/Sold), no output lot
        // was minted, and no abandonment loss was recorded — the dead owner's
        // capital stays in place (heirs are G4b) and its slot stays removed.
        assert_eq!(society.m2_projects[0].state, M2ProjectState::Waiting);
        assert!(society.project_output_lots.is_empty());
        assert_eq!(society.project_revenue, Gold::ZERO);
        assert_eq!(society.capital_goods_consumed, 0);
        assert_eq!(society.capital_labor_consumed, 0);
        assert_eq!(society.capital_gold_loss, Gold::ZERO);
        assert!(
            society.agents.get(AgentId(1)).is_none(),
            "the dead owner's slot stays freed"
        );
    }

    #[test]
    fn m3_project_money_good_output_updates_ledgers() {
        let mut agent = test_capitalist(Stock::new(6));
        agent.gold = Gold(2);
        let mut society = test_m3_society(agent, SALT, Vec::new());
        society.m2_projects.push(M2Project {
            id: M2ProjectId(99),
            owner: AgentId(1),
            line: ProjectLineId(99),
            state: M2ProjectState::Waiting,
            started_at: Tick(0),
            maturity: Tick(0),
            labor_advanced: 1,
            input_goods_committed: Vec::new(),
            input_cost_basis: Gold::ZERO,
            advanced_gold: Gold(1),
            expected_revenue: Gold(3),
            output_good: SALT,
            output_qty: 3,
            salvage_bps: 5000,
        });

        society.mature_waiting_projects();

        let money_system = society
            .money_system
            .as_ref()
            .expect("M3 ledger is initialized");
        assert_eq!(society.agents[0].gold, Gold(5));
        assert_eq!(society.agents[0].stock.get(SALT), 0);
        assert_eq!(money_system.snapshot().public_specie, Gold(5));
        assert_eq!(money_system.base.commodity_base, Gold(5));
        assert_eq!(society.m2_projects[0].state, M2ProjectState::Sold);
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn money_good_project_output_overflow_stays_as_stock_lot() {
        let mut agent = test_capitalist(Stock::new(3));
        agent.gold = Gold(u64::MAX);
        let mut society = test_society(agent);
        society.m2_projects.push(M2Project {
            id: M2ProjectId(99),
            owner: AgentId(1),
            line: ProjectLineId(99),
            state: M2ProjectState::Waiting,
            started_at: Tick(0),
            maturity: Tick(0),
            labor_advanced: 1,
            input_goods_committed: Vec::new(),
            input_cost_basis: Gold::ZERO,
            advanced_gold: Gold(1),
            expected_revenue: Gold(1),
            output_good: GOLD,
            output_qty: 1,
            salvage_bps: 5000,
        });

        society.mature_waiting_projects();

        assert_eq!(society.agents[0].gold, Gold(u64::MAX));
        assert_eq!(society.agents[0].stock.get(GOLD), 1);
        assert_eq!(society.m2_projects[0].state, M2ProjectState::Mature);
        assert_eq!(society.project_output_lots[0].qty_remaining, 1);
        assert_eq!(society.project_output_lots[0].proceeds, Gold::ZERO);
    }

    fn patient_capitalist() -> Agent {
        let mut agent = test_capitalist(Stock::new(3));
        agent.gold = Gold(10);
        agent.scale = [
            (WantKind::Good(GOLD), Horizon::Now, 2),
            (WantKind::Good(GOLD), Horizon::Later(4), 20),
        ]
        .into_iter()
        .flat_map(|(kind, horizon, n)| {
            (0..n).map(move |_| Want {
                kind,
                horizon,
                qty: 1,
                satisfied: false,
            })
        })
        .collect();
        agent.expect[usize::from(FOOD.0)] = PriceBelief::new(Gold(2), Gold(1));
        agent
    }

    fn cash_poor_project_capitalist(id: u32) -> Agent {
        let mut agent = test_capitalist(Stock::new(3));
        agent.id = AgentId(u64::from(id));
        agent.gold = Gold::ZERO;
        agent.scale = scale_entries(&[(WantKind::Good(GOLD), Horizon::Later(7), 6)]);
        agent.roles = vec![Role::Capitalist];
        agent.expect[usize::from(FOOD.0)] = PriceBelief::new(Gold(1), Gold(1));
        agent
    }

    fn scale_entries(entries: &[(WantKind, Horizon, usize)]) -> Vec<Want> {
        entries
            .iter()
            .flat_map(|(kind, horizon, count)| {
                (0..*count).map(move |_| Want {
                    kind: *kind,
                    horizon: *horizon,
                    qty: 1,
                    satisfied: false,
                })
            })
            .collect()
    }

    fn project_schedule() -> Vec<(AgentId, AgioSchedule)> {
        vec![(
            AgentId(1),
            AgioSchedule {
                lending: vec![
                    AgioQuote {
                        present: Gold(4),
                        future: Gold(2),
                        horizon: 1,
                    },
                    AgioQuote {
                        present: Gold(10),
                        future: Gold(9),
                        horizon: 4,
                    },
                ],
                borrowing: Vec::new(),
            },
        )]
    }

    fn duplicate_input_line() -> ProjectLine {
        ProjectLine {
            id: ProjectLineId(99),
            name: "DuplicateInputs",
            input_goods: vec![(WOOD, 2), (WOOD, 2)],
            required_labor: 1,
            period_len: 4,
            output_good: FOOD,
            output_qty: 9,
            salvage_bps: 5000,
        }
    }

    fn wood_input_line(qty: u32) -> ProjectLine {
        ProjectLine {
            id: ProjectLineId(100),
            name: "WoodInputs",
            input_goods: vec![(WOOD, qty)],
            required_labor: 0,
            period_len: 4,
            output_good: FOOD,
            output_qty: 9,
            salvage_bps: 5000,
        }
    }

    fn gold_input_line(qty: u32) -> ProjectLine {
        ProjectLine {
            id: ProjectLineId(101),
            name: "GoldInputs",
            input_goods: vec![(GOLD, qty)],
            required_labor: 1,
            period_len: 4,
            output_good: FOOD,
            output_qty: 9,
            salvage_bps: 5000,
        }
    }

    fn salt_money_input_line(qty: u32, required_labor: u32, salvage_bps: u16) -> ProjectLine {
        ProjectLine {
            id: ProjectLineId(102),
            name: "SaltMoneyInputs",
            input_goods: vec![(SALT, qty)],
            required_labor,
            period_len: 4,
            output_good: FOOD,
            output_qty: 9,
            salvage_bps,
        }
    }

    fn funding_plan(
        id: u64,
        owner: AgentId,
        line: ProjectLineId,
        reserved_gold: Gold,
    ) -> ProjectFundingPlan {
        ProjectFundingPlan {
            id: ProjectPlanId(id),
            owner,
            line,
            created_tick: Tick(0),
            expires_tick: Tick(3),
            expected_revenue: Gold(4),
            input_cost_basis: Gold::ZERO,
            required_labor: 1,
            funding_horizon: 4,
            borrowed_gold: reserved_gold,
            future_due_committed: Gold(2),
            reserved_gold,
            started_project: None,
        }
    }

    #[test]
    #[should_panic(expected = "shadow natural-rate series length must match live M3 records")]
    fn attach_shadow_requires_same_length_series() {
        let mut scenario = builtin_market_scenario(ScenarioName::FractionalReserve);
        scenario.periods = 2;
        let mut society = Society::from_scenario(scenario);
        society.run(2);

        society.attach_shadow(&ShadowSeries {
            natural_rate_bps: vec![Some(1)],
            structure_length_ticks_x100: vec![0],
        });
    }

    #[test]
    fn project_start_checks_duplicate_inputs_cumulatively() {
        let mut stock = Stock::new(3);
        stock.add(WOOD, 2);
        let mut society = test_society(test_capitalist(stock));
        society.project_lines = vec![duplicate_input_line()];

        society.ensure_project_started(0, &project_schedule(), GOLD);

        assert!(society.m2_projects.is_empty());
        assert_eq!(society.agents[0].stock.get(WOOD), 2);
    }

    #[test]
    fn project_start_respects_stock_reserved_by_spot_asks() {
        let mut stock = Stock::new(3);
        stock.add(WOOD, 4);
        let mut society = test_society(test_capitalist(stock));
        society.project_lines = vec![duplicate_input_line()];
        let ask = Order {
            agent: AgentId(1),
            side: OrderSide::Ask,
            good: WOOD,
            limit: Gold(1),
            qty: 1,
            seq: 1,
            expires_tick: 3,
        };
        assert!(society
            .reservations
            .reserve_order(society.agents.as_slice(), &ask));

        society.ensure_project_started(0, &project_schedule(), GOLD);

        assert!(society.m2_projects.is_empty());
        assert_eq!(society.agents[0].stock.get(WOOD), 4);
        assert_eq!(society.reservations.reserved_stock(AgentId(1), WOOD), 1);
    }

    #[test]
    fn project_start_respects_stock_provisioning_near_wants() {
        let mut stock = Stock::new(3);
        stock.add(WOOD, 4);
        let mut agent = test_capitalist(stock);
        agent.scale = vec![Want {
            kind: WantKind::Good(WOOD),
            horizon: Horizon::Next,
            qty: 1,
            satisfied: false,
        }];
        let mut society = test_society(agent);
        society.project_lines = vec![duplicate_input_line()];

        society.ensure_project_started(0, &project_schedule(), GOLD);

        assert!(society.m2_projects.is_empty());
        assert_eq!(society.agents[0].stock.get(WOOD), 4);
    }

    #[test]
    fn project_start_uses_designated_money_for_near_input_reserves() {
        let mut stock = Stock::new(6);
        stock.add(GOLD, 4);
        let mut agent = test_capitalist(stock);
        agent.scale = vec![Want {
            kind: WantKind::Good(GOLD),
            horizon: Horizon::Next,
            qty: 1,
            satisfied: false,
        }];
        let mut society = Society::from_scenario(MarketScenario {
            name: "salt-money-gold-inputs",
            scenario: ScenarioName::RoundaboutCapital,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: SALT }),
        });
        society.project_lines = vec![gold_input_line(4)];

        society.ensure_project_started(0, &project_schedule(), SALT);

        assert!(society.m2_projects.is_empty());
        assert_eq!(society.agents[0].stock.get(GOLD), 4);
    }

    #[test]
    fn project_start_can_spend_designated_money_input_from_balance() {
        let mut agent = test_capitalist(Stock::new(6));
        agent.gold = Gold(2);
        let mut society = Society::from_scenario(MarketScenario {
            name: "salt-money-project-input",
            scenario: ScenarioName::RoundaboutCapital,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: SALT }),
        });
        society.project_lines = vec![salt_money_input_line(2, 0, 5000)];

        society.ensure_project_started(0, &project_schedule(), SALT);

        assert_eq!(society.m2_projects.len(), 1);
        assert_eq!(society.agents[0].gold, Gold::ZERO);
        assert_eq!(society.agents[0].stock.get(SALT), 0);
        assert_eq!(
            society.m2_projects[0].input_goods_committed,
            vec![(SALT, 2)]
        );
    }

    #[test]
    fn project_start_reserves_satisfied_current_money_wants() {
        let mut agent = test_capitalist(Stock::new(6));
        agent.gold = Gold(2);
        agent.scale = vec![Want {
            kind: WantKind::Good(SALT),
            horizon: Horizon::Now,
            qty: 1,
            satisfied: false,
        }];
        agent.recompute_satisfaction_for_money(SALT);
        let mut society = Society::from_scenario(MarketScenario {
            name: "salt-money-project-input",
            scenario: ScenarioName::RoundaboutCapital,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: SALT }),
        });
        society.project_lines = vec![salt_money_input_line(2, 0, 5000)];

        society.ensure_project_started(0, &project_schedule(), SALT);

        assert!(society.m2_projects.is_empty());
        assert_eq!(society.agents[0].gold, Gold(2));
        assert!(society.agents[0].scale[0].satisfied);
    }

    #[test]
    fn m3_project_money_good_input_updates_ledgers() {
        let mut agent = test_capitalist(Stock::new(6));
        agent.gold = Gold(2);
        let mut society = test_m3_society(agent, SALT, Vec::new());
        society.project_lines = vec![salt_money_input_line(2, 0, 5000)];

        society.ensure_project_started(0, &project_schedule(), SALT);

        let money_system = society
            .money_system
            .as_ref()
            .expect("M3 ledger is initialized");
        assert_eq!(society.m2_projects.len(), 1);
        assert_eq!(society.agents[0].gold, Gold::ZERO);
        assert_eq!(money_system.snapshot().public_specie, Gold::ZERO);
        assert_eq!(money_system.base.commodity_base, Gold::ZERO);
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn abandoned_project_money_good_input_salvage_returns_to_money_balance() {
        let mut agent = test_capitalist(Stock::new(6));
        agent.gold = Gold(3);
        let mut society = Society::from_scenario(MarketScenario {
            name: "salt-money-project-salvage",
            scenario: ScenarioName::RoundaboutCapital,
            seed: 1,
            periods: 1,
            agents: vec![agent],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: SALT }),
        });
        society.project_lines = vec![salt_money_input_line(2, 1, 5000)];

        society.ensure_project_started(0, &project_schedule(), SALT);
        assert_eq!(society.agents[0].gold, Gold(1));

        society.abandon_unviable_projects(&[], SALT);

        assert_eq!(society.m2_projects[0].state, M2ProjectState::Abandoned);
        assert_eq!(society.agents[0].gold, Gold(2));
        assert_eq!(society.agents[0].stock.get(SALT), 0);
        assert_eq!(society.capital_goods_consumed, 1);
    }

    #[test]
    fn project_start_allows_stock_after_consumed_now_want() {
        let mut stock = Stock::new(3);
        stock.add(WOOD, 5);
        let mut agent = test_capitalist(stock);
        agent.scale = vec![Want {
            kind: WantKind::Good(WOOD),
            horizon: Horizon::Now,
            qty: 2,
            satisfied: false,
        }];
        let mut society = test_society(agent);
        society.agents[0].consume_now_wants_with_provisions();
        society.project_lines = vec![wood_input_line(3)];

        society.ensure_project_started(0, &project_schedule(), GOLD);

        assert_eq!(society.m2_projects.len(), 1);
        assert_eq!(society.agents[0].stock.get(WOOD), 0);
    }

    #[test]
    fn project_start_prices_input_cost_with_owner_belief() {
        let mut stock = Stock::new(3);
        stock.add(WOOD, 4);
        let mut agent = test_capitalist(stock);
        agent.expect[usize::from(WOOD.0)] = PriceBelief::new(Gold(3), Gold(1));
        let mut society = test_society(agent);
        society.project_lines = vec![duplicate_input_line()];

        society.ensure_project_started(0, &project_schedule(), GOLD);

        assert!(society.m2_projects.is_empty());
        assert_eq!(society.agents[0].stock.get(WOOD), 4);
    }

    #[test]
    fn project_start_does_not_debit_inputs_without_first_funding_step() {
        let mut stock = Stock::new(3);
        stock.add(WOOD, 4);
        let mut agent = test_capitalist(stock);
        agent.gold = Gold::ZERO;
        let mut society = test_society(agent);
        society.project_lines = vec![duplicate_input_line()];

        society.ensure_project_started(0, &project_schedule(), GOLD);

        assert!(society.m2_projects.is_empty());
        assert_eq!(society.agents[0].stock.get(WOOD), 4);
    }

    #[test]
    fn start_then_abandon_conserves_committed_input_goods() {
        let mut stock = Stock::new(3);
        stock.add(WOOD, 4);
        let mut society = test_society(test_capitalist(stock));
        society.project_lines = vec![duplicate_input_line()];

        society.ensure_project_started(0, &project_schedule(), GOLD);

        assert_eq!(society.m2_projects.len(), 1);
        assert_eq!(society.agents[0].stock.get(WOOD), 0);
        assert_eq!(
            society.m2_projects[0].input_goods_committed,
            vec![(WOOD, 4)]
        );
        assert_eq!(society.m2_projects[0].input_cost_basis, Gold(4));

        society.abandon_unviable_projects(&[], GOLD);

        assert_eq!(society.m2_projects[0].state, M2ProjectState::Abandoned);
        assert_eq!(society.agents[0].stock.get(WOOD), 2);
        assert_eq!(society.capital_goods_consumed, 2);
        assert_eq!(
            society.agents[0]
                .stock
                .get(WOOD)
                .saturating_add(society.capital_goods_consumed),
            4
        );
    }

    #[test]
    fn project_start_falls_back_to_affordable_line() {
        let mut stock = Stock::new(3);
        stock.add(WOOD, 2);
        let mut society = test_society(test_capitalist(stock));
        society.project_lines = vec![duplicate_input_line(), dry_fish_short_line()];

        society.ensure_project_started(0, &project_schedule(), GOLD);

        assert_eq!(society.m2_projects.len(), 1);
        assert_eq!(society.m2_projects[0].line, ProjectLineId(1));
        assert_eq!(society.agents[0].stock.get(WOOD), 2);
    }

    #[test]
    fn project_start_derives_missing_schedule_for_owner() {
        let mut society = test_society(patient_capitalist());

        society.ensure_project_started(0, &[], GOLD);

        assert_eq!(society.m2_projects.len(), 1);
        assert_eq!(society.m2_projects[0].line, ProjectLineId(2));
    }

    #[test]
    fn project_funding_reserve_blocks_non_project_spend() {
        let mut agent = test_capitalist(Stock::new(3));
        agent.gold = Gold(3);
        let mut society = test_society(agent);
        society
            .project_funding_plans
            .push(funding_plan(1, AgentId(1), ProjectLineId(1), Gold(2)));
        society.project_funding_plans.push(funding_plan(
            2,
            AgentId(1),
            ProjectLineId(2),
            Gold::ZERO,
        ));

        assert_eq!(society.reserved_project_gold(AgentId(1)), Gold(2));
        assert_eq!(society.free_gold_after_all_reserves(AgentId(1)), Gold(1));
        assert_eq!(
            society.free_gold_for_project_plan(AgentId(1), ProjectPlanId(2)),
            Gold(1)
        );
    }

    #[test]
    fn project_funding_reserve_can_pay_matching_project_labor() {
        let mut agent = test_capitalist(Stock::new(3));
        agent.gold = Gold(3);
        let mut society = test_society(agent);
        society
            .project_funding_plans
            .push(funding_plan(1, AgentId(1), ProjectLineId(1), Gold(2)));

        assert_eq!(society.free_gold_after_all_reserves(AgentId(1)), Gold(1));
        assert_eq!(
            society.free_gold_for_project_plan(AgentId(1), ProjectPlanId(1)),
            Gold(3)
        );
    }

    #[test]
    fn non_debt_gold_spend_clamps_stale_project_reserve() {
        let mut agent = test_capitalist(Stock::new(3));
        agent.gold = Gold(3);
        let mut society = test_society(agent);
        society
            .project_funding_plans
            .push(funding_plan(1, AgentId(1), ProjectLineId(1), Gold(3)));

        society.agents[0].gold = Gold(1);
        assert!(!society.project_funding_invariants_hold());

        society.clamp_all_project_funding_reserves_to_owner_balances();

        assert_eq!(society.project_funding_plans[0].reserved_gold, Gold(1));
        assert!(society.project_funding_invariants_hold());
    }

    #[test]
    fn non_debt_gold_spend_clamps_aggregate_project_reserves() {
        let mut agent = test_capitalist(Stock::new(3));
        agent.gold = Gold(6);
        let mut society = test_society(agent);
        society
            .project_funding_plans
            .push(funding_plan(1, AgentId(1), ProjectLineId(1), Gold(3)));
        society
            .project_funding_plans
            .push(funding_plan(2, AgentId(1), ProjectLineId(2), Gold(3)));

        society.agents[0].gold = Gold(4);
        assert!(!society.project_funding_invariants_hold());

        society.clamp_all_project_funding_reserves_to_owner_balances();

        assert_eq!(society.project_funding_plans[0].reserved_gold, Gold(3));
        assert_eq!(society.project_funding_plans[1].reserved_gold, Gold(1));
        assert!(society.project_funding_invariants_hold());
    }

    #[test]
    fn non_project_debt_payment_clamps_project_reserve_to_owner_balance() {
        let mut agent = test_capitalist(Stock::new(3));
        agent.gold = Gold(3);
        let mut society = test_society(agent);
        society
            .project_funding_plans
            .push(funding_plan(1, AgentId(1), ProjectLineId(1), Gold(3)));
        society.debts.push(DebtContract {
            id: DebtId(21),
            lender: CreditLender::Agent(AgentId(2)),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(0),
            principal: Gold(2),
            due: Gold(2),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::Commodity,
        });
        let snapshot = society
            .project_debt_payment_snapshot()
            .expect("reserved project gold plus due debt is tracked");

        society.agents[0].gold = Gold(1);
        society.debts[0].paid = Gold(2);
        society.debts[0].state = DebtState::Settled;
        society.debt_payment_audit.push(DebtPaymentAuditRecord {
            tick: 0,
            debt: 21,
            from: AgentId(1),
            to: AgentId(2),
            owed: Gold(2),
            paid: Gold(2),
            remaining: Gold::ZERO,
            public_fiat: Gold::ZERO,
            demand_claims: Gold::ZERO,
            public_specie: Gold(2),
            tender: PublicDebtTender::ParAll,
            state: crate::record::DebtPaymentState::Settled,
        });

        society.release_project_funding_reserves_for_debt_payments(&snapshot);

        assert_eq!(society.project_funding_plans[0].reserved_gold, Gold(1));
    }

    #[test]
    fn m2_debt_settlement_clamps_project_reserve_to_owner_balance() {
        let mut borrower = test_capitalist(Stock::new(3));
        borrower.gold = Gold(3);
        let mut lender = test_capitalist(Stock::new(3));
        lender.id = AgentId(2);
        lender.gold = Gold::ZERO;
        let mut society = Society::from_scenario(MarketScenario {
            name: "m2-reserve-clamp",
            scenario: ScenarioName::RoundaboutCapital,
            seed: 1,
            periods: 1,
            agents: vec![borrower, lender],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        });
        society.project_lines = Vec::new();
        society
            .project_funding_plans
            .push(funding_plan(1, AgentId(1), ProjectLineId(1), Gold(3)));
        society.debts.push(DebtContract {
            id: DebtId(31),
            lender: CreditLender::Agent(AgentId(2)),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(0),
            principal: Gold(2),
            due: Gold(2),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::Commodity,
        });

        society.step();

        let borrower_index = society.agent_index_for(AgentId(1)).unwrap();
        assert_eq!(society.agents[borrower_index].gold, Gold(1));
        assert_eq!(society.project_funding_plans[0].reserved_gold, Gold(1));
        assert!(society.project_funding_invariants_hold());
    }

    #[test]
    fn non_project_fiat_credit_payment_clamps_project_reserve_to_owner_balance() {
        let mut agent = test_capitalist(Stock::new(3));
        agent.gold = Gold(5);
        let mut society = test_society(agent);
        society
            .project_funding_plans
            .push(funding_plan(1, AgentId(1), ProjectLineId(1), Gold(5)));
        society.debts.push(DebtContract {
            id: DebtId(22),
            lender: CreditLender::Issuer(crate::ledger::IssuerId(1)),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(0),
            principal: Gold(3),
            due: Gold(3),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::FiatCredit(crate::ledger::IssuerId(1)),
        });
        let snapshot = society
            .project_debt_payment_snapshot()
            .expect("reserved project gold plus due fiat debt is tracked");

        society.agents[0].gold = Gold(2);
        society.debts[0].paid = Gold(3);
        society.debts[0].state = DebtState::Settled;

        society.release_project_funding_reserves_for_debt_payments(&snapshot);

        assert_eq!(society.project_funding_plans[0].reserved_gold, Gold(2));
        assert!(society.project_funding_invariants_hold());
    }

    #[test]
    fn project_debt_repayment_releases_specie_and_clears_completed_plan_reserves() {
        let mut agent = test_capitalist(Stock::new(3));
        agent.gold = Gold(10);
        let mut society = test_society(agent);
        society.debt_payment_audit.push(DebtPaymentAuditRecord {
            tick: 0,
            debt: 90,
            from: AgentId(1),
            to: AgentId(2),
            owed: Gold(3),
            paid: Gold(3),
            remaining: Gold::ZERO,
            public_fiat: Gold::ZERO,
            demand_claims: Gold::ZERO,
            public_specie: Gold(3),
            tender: PublicDebtTender::ParAll,
            state: crate::record::DebtPaymentState::Settled,
        });
        society
            .project_funding_plans
            .push(funding_plan(1, AgentId(1), ProjectLineId(1), Gold(5)));
        society
            .project_funding_plans
            .push(funding_plan(2, AgentId(1), ProjectLineId(2), Gold(4)));
        society.debts.push(DebtContract {
            id: DebtId(11),
            lender: CreditLender::Agent(AgentId(2)),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(0),
            principal: Gold(4),
            due: Gold(4),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::ProjectFunding {
                plan: ProjectPlanId(1),
                project: None,
            },
            funding: CreditSource::Commodity,
        });
        society.debts.push(DebtContract {
            id: DebtId(12),
            lender: CreditLender::Bank(BankId(1)),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(0),
            principal: Gold(2),
            due: Gold(2),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::ProjectFunding {
                plan: ProjectPlanId(2),
                project: None,
            },
            funding: CreditSource::BankFiduciary(BankId(1)),
        });
        let snapshot = society
            .project_debt_payment_snapshot()
            .expect("due project debts are tracked");
        society.debts[0].paid = Gold(4);
        society.debts[0].state = DebtState::Settled;
        society.debts[1].paid = Gold(2);
        society.debts[1].state = DebtState::Settled;
        society.debt_payment_audit.push(DebtPaymentAuditRecord {
            tick: 1,
            debt: 11,
            from: AgentId(1),
            to: AgentId(2),
            owed: Gold(4),
            paid: Gold(4),
            remaining: Gold::ZERO,
            public_fiat: Gold(1),
            demand_claims: Gold(1),
            public_specie: Gold(2),
            tender: PublicDebtTender::ParAll,
            state: crate::record::DebtPaymentState::Settled,
        });
        society.bank_repayment_audit.push(BankRepaymentAuditRecord {
            tick: 1,
            debt: 12,
            borrower: AgentId(1),
            bank: BankId(1),
            owed: Gold(2),
            paid: Gold(2),
            remaining: Gold::ZERO,
            public_fiat: Gold::ZERO,
            demand_claims: Gold(2),
            public_specie: Gold::ZERO,
            credit_retired: Gold(2),
            tender: BankRepaymentTender::ParAll,
            state: crate::record::DebtPaymentState::Settled,
        });

        society.release_project_funding_reserves_for_debt_payments(&snapshot);

        assert_eq!(society.project_funding_plans[0].reserved_gold, Gold::ZERO);
        assert_eq!(society.project_funding_plans[1].reserved_gold, Gold::ZERO);
        assert!(society.project_funding_invariants_hold());
    }

    #[test]
    fn project_funding_settlement_preserves_reserve_when_plan_debt_remains_open() {
        let mut agent = test_capitalist(Stock::new(3));
        agent.gold = Gold(5);
        let mut society = test_society(agent);
        society
            .project_funding_plans
            .push(funding_plan(1, AgentId(1), ProjectLineId(1), Gold(5)));
        society.debts.push(DebtContract {
            id: DebtId(11),
            lender: CreditLender::Agent(AgentId(2)),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(0),
            principal: Gold(4),
            due: Gold(4),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::ProjectFunding {
                plan: ProjectPlanId(1),
                project: None,
            },
            funding: CreditSource::Commodity,
        });
        society.debts.push(DebtContract {
            id: DebtId(12),
            lender: CreditLender::Agent(AgentId(3)),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(2),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::ProjectFunding {
                plan: ProjectPlanId(1),
                project: None,
            },
            funding: CreditSource::Commodity,
        });
        let snapshot = society
            .project_debt_payment_snapshot()
            .expect("due project debt is tracked");
        society.debts[0].paid = Gold(4);
        society.debts[0].state = DebtState::Settled;
        society.debt_payment_audit.push(DebtPaymentAuditRecord {
            tick: 1,
            debt: 11,
            from: AgentId(1),
            to: AgentId(2),
            owed: Gold(4),
            paid: Gold(4),
            remaining: Gold::ZERO,
            public_fiat: Gold::ZERO,
            demand_claims: Gold(4),
            public_specie: Gold::ZERO,
            tender: PublicDebtTender::ParAll,
            state: crate::record::DebtPaymentState::Settled,
        });

        society.release_project_funding_reserves_for_debt_payments(&snapshot);

        assert_eq!(society.project_funding_plans[0].reserved_gold, Gold(5));
        assert!(society.project_funding_invariants_hold());
    }

    #[test]
    fn project_funding_default_preserves_reserve_when_plan_debt_remains_open() {
        let mut agent = test_capitalist(Stock::new(3));
        agent.gold = Gold(5);
        let mut society = test_society(agent);
        society
            .project_funding_plans
            .push(funding_plan(1, AgentId(1), ProjectLineId(1), Gold(5)));
        society.debts.push(DebtContract {
            id: DebtId(11),
            lender: CreditLender::Agent(AgentId(2)),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(0),
            principal: Gold(4),
            due: Gold(4),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::ProjectFunding {
                plan: ProjectPlanId(1),
                project: None,
            },
            funding: CreditSource::Commodity,
        });
        society.debts.push(DebtContract {
            id: DebtId(12),
            lender: CreditLender::Agent(AgentId(3)),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(2),
            principal: Gold(1),
            due: Gold(1),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::ProjectFunding {
                plan: ProjectPlanId(1),
                project: None,
            },
            funding: CreditSource::Commodity,
        });
        let snapshot = society
            .project_debt_payment_snapshot()
            .expect("due project debt is tracked");
        society.debts[0].state = DebtState::Defaulted;

        society.release_project_funding_reserves_for_debt_payments(&snapshot);

        assert_eq!(society.project_funding_plans[0].reserved_gold, Gold(5));
        assert!(society.project_funding_invariants_hold());
    }

    #[test]
    fn project_funding_borrow_scan_reaches_multiple_capitalists() {
        let mut society = Society::from_scenario(MarketScenario {
            name: "multi-project-borrowers",
            scenario: ScenarioName::BorrowToBuild,
            seed: 1,
            periods: 1,
            agents: vec![
                cash_poor_project_capitalist(1),
                cash_poor_project_capitalist(2),
            ],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        });
        society.project_lines = vec![borrow_to_build_line()];

        society.post_project_funding_borrows(GOLD);

        assert_eq!(society.loan_book.borrows.len(), 2);
        assert_eq!(society.project_funding_plans.len(), 2);
    }

    #[test]
    fn abandoned_funded_project_releases_reserved_gold() {
        let stock = Stock::new(3);
        let mut society = test_society(test_capitalist(stock));
        society.m2_projects.push(M2Project {
            id: M2ProjectId(1),
            owner: AgentId(1),
            line: ProjectLineId(99),
            state: M2ProjectState::Waiting,
            started_at: Tick(0),
            maturity: Tick(4),
            labor_advanced: 1,
            input_goods_committed: Vec::new(),
            input_cost_basis: Gold::ZERO,
            advanced_gold: Gold(2),
            expected_revenue: Gold(1),
            output_good: FOOD,
            output_qty: 1,
            salvage_bps: 5000,
        });
        let mut plan = funding_plan(1, AgentId(1), ProjectLineId(99), Gold(2));
        plan.started_project = Some(M2ProjectId(1));
        society.project_funding_plans.push(plan);

        society.abandon_unviable_projects(&[], GOLD);

        assert_eq!(society.m2_projects[0].state, M2ProjectState::Abandoned);
        assert_eq!(society.project_funding_plans[0].reserved_gold, Gold::ZERO);
        assert_eq!(society.reserved_project_gold(AgentId(1)), Gold::ZERO);
    }

    #[test]
    fn abandoned_m2_project_returns_input_salvage_to_owner() {
        let stock = Stock::new(3);
        let mut society = test_society(test_capitalist(stock));
        society.m2_projects.push(M2Project {
            id: M2ProjectId(1),
            owner: AgentId(1),
            line: ProjectLineId(99),
            state: M2ProjectState::Forming,
            started_at: Tick(0),
            maturity: Tick(0),
            labor_advanced: 1,
            input_goods_committed: vec![(WOOD, 2)],
            input_cost_basis: Gold(2),
            advanced_gold: Gold(2),
            expected_revenue: Gold(1),
            output_good: FOOD,
            output_qty: 9,
            salvage_bps: 5000,
        });

        society.abandon_unviable_projects(&[], GOLD);

        assert_eq!(society.m2_projects[0].state, M2ProjectState::Abandoned);
        assert_eq!(society.agents[0].stock.get(WOOD), 1);
        assert_eq!(society.capital_goods_consumed, 1);
        assert_eq!(society.capital_labor_consumed, 1);
        assert_eq!(society.capital_gold_loss, Gold(2));
    }

    #[test]
    fn project_abandon_derives_missing_schedule_for_owner() {
        let stock = Stock::new(3);
        let mut society = test_society(test_capitalist(stock));
        society.project_lines = vec![duplicate_input_line()];
        society.m2_projects.push(M2Project {
            id: M2ProjectId(1),
            owner: AgentId(1),
            line: ProjectLineId(99),
            state: M2ProjectState::Waiting,
            started_at: Tick(0),
            maturity: Tick(4),
            labor_advanced: 1,
            input_goods_committed: vec![(WOOD, 8)],
            input_cost_basis: Gold(8),
            advanced_gold: Gold::ZERO,
            expected_revenue: Gold(3),
            output_good: FOOD,
            output_qty: 9,
            salvage_bps: 5000,
        });

        society.abandon_unviable_projects(&[], GOLD);

        assert_eq!(society.m2_projects[0].state, M2ProjectState::Abandoned);
        assert_eq!(society.agents[0].stock.get(WOOD), 4);
    }

    #[test]
    fn advanced_gold_is_not_abandonment_salvage() {
        let stock = Stock::new(3);
        let society = test_society(test_capitalist(stock));
        let project = M2Project {
            id: M2ProjectId(1),
            owner: AgentId(1),
            line: ProjectLineId(2),
            state: M2ProjectState::Waiting,
            started_at: Tick(0),
            maturity: Tick(4),
            labor_advanced: 1,
            input_goods_committed: Vec::new(),
            input_cost_basis: Gold::ZERO,
            advanced_gold: Gold(10),
            expected_revenue: Gold(1),
            output_good: FOOD,
            output_qty: 9,
            salvage_bps: 5000,
        };
        let schedules = vec![(AgentId(1), AgioSchedule::default())];

        assert!(!society.project_should_abandon(&project, &schedules, GOLD));
    }

    #[test]
    fn input_salvage_can_make_project_unviable() {
        let stock = Stock::new(3);
        let mut society = test_society(test_capitalist(stock));
        society.project_lines = vec![duplicate_input_line()];
        society.m2_projects.push(M2Project {
            id: M2ProjectId(1),
            owner: AgentId(1),
            line: ProjectLineId(99),
            state: M2ProjectState::Waiting,
            started_at: Tick(0),
            maturity: Tick(4),
            labor_advanced: 1,
            input_goods_committed: vec![(WOOD, 8)],
            input_cost_basis: Gold(8),
            advanced_gold: Gold::ZERO,
            expected_revenue: Gold(3),
            output_good: FOOD,
            output_qty: 9,
            salvage_bps: 5000,
        });
        let schedules = vec![(
            AgentId(1),
            AgioSchedule {
                lending: vec![AgioQuote {
                    present: Gold(2),
                    future: Gold(3),
                    horizon: 4,
                }],
                borrowing: Vec::new(),
            },
        )];

        society.abandon_unviable_projects(&schedules, GOLD);

        assert_eq!(society.m2_projects[0].state, M2ProjectState::Abandoned);
        assert_eq!(society.agents[0].stock.get(WOOD), 4);
        assert_eq!(society.capital_goods_consumed, 4);
    }

    #[test]
    fn synthetic_bank_and_issuer_order_agents_do_not_collide() {
        assert_ne!(
            bank_order_agent(BankId(1001)),
            issuer_order_agent(crate::ledger::IssuerId(1))
        );
        assert_ne!(
            bank_order_agent(BankId(1)),
            issuer_order_agent(crate::ledger::IssuerId(1))
        );
    }

    #[test]
    fn policy_quote_does_not_round_sub_unit_interest_up() {
        assert_eq!(
            one_unit_policy_quote(Gold(3), Gold(5)),
            Some((Gold(1), Gold(1)))
        );
        assert_eq!(
            one_unit_policy_quote(Gold(3), Gold(6)),
            Some((Gold(1), Gold(2)))
        );
        assert_eq!(
            one_unit_policy_quote(Gold(20), Gold(21)),
            Some((Gold(1), Gold(1)))
        );
        assert_eq!(
            one_unit_policy_quote(Gold(2), Gold(1)),
            Some((Gold(1), Gold(1)))
        );
        assert_eq!(one_unit_policy_quote(Gold::ZERO, Gold(1)), None);
        assert_eq!(one_unit_policy_quote(Gold(1), Gold::ZERO), None);
    }

    #[test]
    fn bank_policy_lender_does_not_post_without_borrow_demand() {
        let mut scenario = builtin_market_scenario(ScenarioName::FractionalReserve);
        scenario.agents = vec![test_capitalist(Stock::new(3))];
        scenario.events.clear();
        let mut society = Society::from_scenario(scenario);
        society.regime = Regime::FractionalConvertible;
        society.banks[0].reserve_ratio_bps = ReserveRatioBps(0);
        society.banks[0].policy = BankPolicy {
            max_new_fiduciary_per_tick: Gold(u64::MAX),
            loan_present: Gold(1),
            loan_horizon: 4,
            loan_future_due: Gold(1),
            enabled: true,
        };

        society.post_bank_lend_orders(&[], GOLD);

        assert_eq!(society.loan_book.lends.len(), 0);
        assert_eq!(
            society.loan_reservations.bank_fiduciary_open(BankId(1)),
            Gold::ZERO
        );
    }

    #[test]
    fn larger_bank_policy_amount_still_fills_unit_project_borrows() {
        let mut scenario = builtin_market_scenario(ScenarioName::FractionalReserve);
        for event in &mut scenario.events {
            if let EventKind::SetBankCreditPolicy { policy, .. } = &mut event.kind {
                policy.max_new_fiduciary_per_tick = Gold(2);
                policy.loan_present = Gold(2);
                policy.loan_future_due = Gold(2);
            }
        }
        let periods = scenario.periods;
        let mut society = Society::from_scenario(scenario);

        society.run(periods);

        let bank_trades = society
            .loan_trades
            .iter()
            .filter(|trade| matches!(trade.funding, CreditSource::BankFiduciary(_)))
            .collect::<Vec<_>>();
        assert!(!bank_trades.is_empty());
        assert!(bank_trades.iter().all(|trade| trade.present == Gold(1)));
    }

    #[test]
    fn m3_audit_tapes_are_opt_in() {
        let mut baseline = builtin_market_scenario(ScenarioName::FractionalReserve);
        baseline.periods = 1;
        let baseline = run_m3_with_shadow_without_metrics(baseline);
        assert!(baseline.money_audit.is_empty());
        assert!(baseline.bank_audit.is_empty());

        let mut money_scenario = builtin_market_scenario(ScenarioName::FractionalReserve);
        money_scenario.periods = 1;
        let mut money_society = Society::from_scenario(money_scenario);
        money_society.enable_money_audit();
        money_society.run(1);
        assert_eq!(money_society.money_audit.len(), money_society.agents.len());
        assert!(money_society.bank_audit.is_empty());

        let mut bank_scenario = builtin_market_scenario(ScenarioName::FractionalReserve);
        bank_scenario.periods = 1;
        let mut bank_society = Society::from_scenario(bank_scenario);
        bank_society.enable_bank_audit();
        bank_society.run(1);
        assert!(bank_society.money_audit.is_empty());
        assert_eq!(bank_society.bank_audit.len(), bank_society.banks.len());
    }

    #[test]
    fn set_public_spot_tender_event_updates_policy_without_money_mutation() {
        let mut society = Society::from_scenario(builtin_market_scenario(
            ScenarioName::CommodityCreditNeutral,
        ));
        let before = society.money_system.clone();

        society.apply_event_kind(
            EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly),
            ApplyMode::Event,
        );

        assert_eq!(society.public_spot_tender, PublicSpotTender::SpecieOnly);
        assert_eq!(society.money_system, before);
    }

    #[test]
    fn m3_spot_bid_cap_uses_accepted_media() {
        let buyer = redemption_agent(1, Gold::ZERO);
        let mut society = test_m3_society(buyer, GOLD, Vec::new());
        society
            .money_system
            .as_mut()
            .unwrap()
            .credit_fiat(AgentId(1), Gold(1))
            .unwrap();
        society
            .money_system
            .as_ref()
            .unwrap()
            .reconcile_agent_cache(society.agents.as_mut_slice());
        society.public_spot_tender = PublicSpotTender::SpecieOnly;

        let mut filled = Vec::new();
        society.ensure_order(
            QuotePlan {
                agent_index: 0,
                side: OrderSide::Bid,
                good: FOOD,
                reservation: Gold(1),
                limit: Gold(1),
                existing: None,
            },
            &mut filled,
        );

        assert!(filled.is_empty());
        assert!(society.live_quotes.is_empty());
        assert_eq!(society.books[0].live_order_counts().0, 0);
    }

    #[test]
    fn m3_wage_tender_cap_composes_media_with_amount_reserves() {
        let employer = redemption_agent(1, Gold::ZERO);
        let mut society = test_m3_society(employer, GOLD, Vec::new());
        let money_system = society
            .money_system
            .as_mut()
            .expect("M3 ledger is initialized");
        money_system
            .credit_specie(AgentId(1), Gold(5))
            .expect("specie credit succeeds");
        money_system
            .credit_fiat(AgentId(1), Gold(5))
            .expect("fiat credit succeeds");
        society
            .money_system
            .as_ref()
            .expect("M3 ledger is initialized")
            .reconcile_agent_cache(society.agents.as_mut_slice());
        society.labor_wage_tender = LaborWageTender::SpecieOnly;

        let spot_bid = Order {
            agent: AgentId(1),
            side: OrderSide::Bid,
            good: FOOD,
            limit: Gold(4),
            qty: 1,
            seq: 1,
            expires_tick: 3,
        };
        assert!(society
            .reservations
            .reserve_order(society.agents.as_slice(), &spot_bid));

        assert_eq!(society.free_gold_after_all_reserves(AgentId(1)), Gold(6));
        assert_eq!(society.wage_tender_spendable_cap(AgentId(1), None), Gold(1));

        society.reservations.release_order(&spot_bid);
        let larger_spot_bid = Order {
            limit: Gold(7),
            seq: 2,
            ..spot_bid
        };
        assert!(society
            .reservations
            .reserve_order(society.agents.as_slice(), &larger_spot_bid));

        assert_eq!(society.free_gold_after_all_reserves(AgentId(1)), Gold(3));
        assert_eq!(
            society.wage_tender_spendable_cap(AgentId(1), None),
            Gold::ZERO
        );

        society.reservations.release_order(&larger_spot_bid);
        society
            .project_funding_plans
            .push(funding_plan(1, AgentId(1), ProjectLineId(99), Gold(7)));

        assert_eq!(
            society.wage_tender_spendable_cap(AgentId(1), None),
            Gold::ZERO
        );
        assert_eq!(
            society.wage_tender_spendable_cap(AgentId(1), Some(ProjectPlanId(1))),
            Gold(5)
        );
    }

    #[test]
    fn wage_tender_policy_change_leaves_live_hire_order_resting() {
        let mut employer = test_capitalist(Stock::new(3));
        employer.gold = Gold::ZERO;
        let mut worker = redemption_agent(2, Gold::ZERO);
        worker.labor_capacity = 1;
        let mut society = Society::from_scenario(MarketScenario {
            name: "wage-tender-live-order",
            scenario: ScenarioName::CommodityCreditNeutral,
            seed: 1,
            periods: 1,
            agents: vec![employer, worker],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        });
        let line = dry_fish_short_line();
        society.project_lines = vec![line.clone()];
        society.m2_projects.push(start_project(
            AgentId(1),
            &line,
            M2ProjectId(1),
            Tick(0),
            Gold(10),
            Gold::ZERO,
        ));
        {
            let money_system = society
                .money_system
                .as_mut()
                .expect("M3 ledger is initialized");
            money_system.credit_fiat(AgentId(1), Gold(5)).unwrap();
            money_system.reconcile_agent_cache(society.agents.as_mut_slice());
        }
        society.labor_wage_tender = LaborWageTender::FiatAndSpecie;

        let hire = LaborOrder {
            agent: AgentId(1),
            side: FactorSide::Hire,
            wage_limit: Gold(5),
            qty: 1,
            seq: 1,
            expires_tick: 3,
        };
        assert!(society
            .labor_reservations
            .reserve_order(society.agents.as_slice(), &hire));
        {
            let mut market = LaborMarketView {
                agents: society.agents.as_mut_slice(),
                reservations: &mut society.labor_reservations,
                projects: &mut society.m2_projects,
                lines: &society.project_lines,
                money_system: society.money_system.as_mut(),
                wage_media: society.labor_wage_tender.accepted_media(),
                wage_audit: Some(&mut society.wage_payment_audit),
                wage_tender: society.labor_wage_tender,
            };
            assert!(society
                .labor_book
                .add_order(hire, Some(M2ProjectId(1)), 0, &mut market)
                .is_empty());
        }
        assert_eq!(
            society.labor_reservations.reserved_gold(AgentId(1)),
            Gold(5)
        );

        society.apply_event_kind(
            EventKind::SetLaborWageTender(LaborWageTender::SpecieOnly),
            ApplyMode::Event,
        );

        let unchanged = society
            .labor_book
            .live_order(AgentId(1), FactorSide::Hire)
            .expect("hire order remains live");
        assert_eq!(unchanged.wage_limit, Gold(5));
        assert_eq!(unchanged.qty, 1);
        assert_eq!(
            society.labor_reservations.reserved_gold(AgentId(1)),
            Gold(5)
        );

        let work = LaborOrder {
            agent: AgentId(2),
            side: FactorSide::Work,
            wage_limit: Gold(1),
            qty: 1,
            seq: 2,
            expires_tick: 3,
        };
        assert!(society
            .labor_reservations
            .reserve_order(society.agents.as_slice(), &work));
        let trades = {
            let mut market = LaborMarketView {
                agents: society.agents.as_mut_slice(),
                reservations: &mut society.labor_reservations,
                projects: &mut society.m2_projects,
                lines: &society.project_lines,
                money_system: society.money_system.as_mut(),
                wage_media: society.labor_wage_tender.accepted_media(),
                wage_audit: Some(&mut society.wage_payment_audit),
                wage_tender: society.labor_wage_tender,
            };
            society.labor_book.add_order(work, None, 0, &mut market)
        };

        assert!(trades.is_empty());
        assert!(society.wage_payment_audit.is_empty());
        let still_resting = society
            .labor_book
            .live_order(AgentId(1), FactorSide::Hire)
            .expect("unfunded hire order keeps resting");
        assert_eq!(still_resting.wage_limit, Gold(5));
        assert_eq!(
            society.labor_reservations.reserved_gold(AgentId(1)),
            Gold(5)
        );
    }

    #[test]
    fn tender_policy_change_cancels_fiat_backed_bid_reserve() {
        let buyer = redemption_agent(1, Gold::ZERO);
        let mut society = test_m3_society(buyer, GOLD, Vec::new());
        society
            .money_system
            .as_mut()
            .unwrap()
            .credit_fiat(AgentId(1), Gold(1))
            .unwrap();
        society
            .money_system
            .as_ref()
            .unwrap()
            .reconcile_agent_cache(society.agents.as_mut_slice());
        society.public_spot_tender = PublicSpotTender::FiatAndSpecie;

        let mut filled = Vec::new();
        society.ensure_order(
            QuotePlan {
                agent_index: 0,
                side: OrderSide::Bid,
                good: FOOD,
                reservation: Gold(1),
                limit: Gold(1),
                existing: None,
            },
            &mut filled,
        );

        assert!(filled.is_empty());
        assert_eq!(society.live_quotes.len(), 1);
        assert_eq!(society.books[0].live_order_counts().0, 1);
        assert_eq!(society.reservations.reserved_gold(AgentId(1)), Gold(1));

        society.apply_event_kind(
            EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly),
            ApplyMode::Event,
        );

        assert_eq!(society.public_spot_tender, PublicSpotTender::SpecieOnly);
        assert!(society.live_quotes.is_empty());
        assert_eq!(society.books[0].live_order_counts().0, 0);
        assert_eq!(society.reservations.reserved_gold(AgentId(1)), Gold::ZERO);
    }

    #[test]
    fn tender_policy_change_preserves_unaffected_ask() {
        let mut seller = redemption_agent(1, Gold::ZERO);
        seller.stock.add(FOOD, 1);
        seller.scale = vec![Want {
            kind: WantKind::Good(GOLD),
            horizon: crate::good::Horizon::Next,
            qty: 1,
            satisfied: false,
        }];
        let mut society = test_m3_society(seller, GOLD, Vec::new());
        society.public_spot_tender = PublicSpotTender::FiatAndSpecie;

        let mut filled = Vec::new();
        society.ensure_order(
            QuotePlan {
                agent_index: 0,
                side: OrderSide::Ask,
                good: FOOD,
                reservation: Gold(1),
                limit: Gold(1),
                existing: None,
            },
            &mut filled,
        );

        assert!(filled.is_empty());
        assert_eq!(society.live_quotes.len(), 1);
        assert_eq!(society.books[0].live_order_counts().1, 1);

        society.apply_event_kind(
            EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly),
            ApplyMode::Event,
        );

        assert_eq!(society.public_spot_tender, PublicSpotTender::SpecieOnly);
        assert_eq!(society.live_quotes.len(), 1);
        assert_eq!(society.books[0].live_order_counts().1, 1);
    }

    #[test]
    fn reset_public_spot_book_clears_unaffected_ask() {
        let mut seller = redemption_agent(1, Gold::ZERO);
        seller.stock.add(FOOD, 1);
        seller.scale = vec![Want {
            kind: WantKind::Good(GOLD),
            horizon: crate::good::Horizon::Next,
            qty: 1,
            satisfied: false,
        }];
        let mut society = test_m3_society(seller, GOLD, Vec::new());

        let mut filled = Vec::new();
        society.ensure_order(
            QuotePlan {
                agent_index: 0,
                side: OrderSide::Ask,
                good: FOOD,
                reservation: Gold(1),
                limit: Gold(1),
                existing: None,
            },
            &mut filled,
        );

        assert!(filled.is_empty());
        assert_eq!(society.live_quotes.len(), 1);

        society.apply_event_kind(EventKind::ResetPublicSpotBook, ApplyMode::Event);

        assert!(society.live_quotes.is_empty());
        assert_eq!(society.books[0].live_order_counts().1, 0);
        assert_eq!(society.reservations.reserved_stock(AgentId(1), FOOD), 0);
    }

    #[test]
    fn scale_rewrite_invalidation_releases_stale_spot_reservation() {
        let mut seller = redemption_agent(1, Gold::ZERO);
        seller.stock.add(FOOD, 1);
        seller.scale = vec![Want {
            kind: WantKind::Good(GOLD),
            horizon: Horizon::Next,
            qty: 1,
            satisfied: false,
        }];
        let mut society = test_m3_society(seller, GOLD, Vec::new());

        let mut filled = Vec::new();
        society.ensure_ask(0, FOOD, &mut filled);

        assert!(filled.is_empty());
        assert_eq!(society.live_quotes.len(), 1);
        assert_eq!(society.books[0].live_order_counts().1, 1);
        assert_eq!(society.reservations.reserved_stock(AgentId(1), FOOD), 1);

        society.agents[0].scale = vec![Want {
            kind: WantKind::Good(FOOD),
            horizon: Horizon::Now,
            qty: 1,
            satisfied: false,
        }];

        assert!(society.cancel_changed_live_quotes_for_agent(AgentId(1)));
        assert!(society.live_quotes.is_empty());
        assert_eq!(society.books[0].live_order_counts().1, 0);
        assert_eq!(society.reservations.reserved_stock(AgentId(1), FOOD), 0);
    }

    #[test]
    fn removal_returns_none_for_repeated_call() {
        // An empty-ledger agent: G4a frees the closed-GOLD M1 estate (here zero),
        // and routing a non-empty M3 ledger estate is G4b.
        let society_agent = redemption_agent(1, Gold::ZERO);
        let mut society = test_m3_society(society_agent, GOLD, Vec::new());

        assert!(society.remove_agent(AgentId(1)).is_some());
        // A second removal of the now-freed id is a no-op (real removal: the slot
        // is freed and the stale id resolves to None, not present-but-frozen).
        assert!(society.remove_agent(AgentId(1)).is_none());
        assert!(society.agent_order.is_empty());
        assert!(society.agents.get(AgentId(1)).is_none());
    }

    #[test]
    fn removal_drains_a_funded_m3_specie_balance_into_the_estate() {
        // G8a resolves the G4a/b deferral: a funded M3 agent is no longer refused —
        // its public specie drains into the returned `Estate` (conserved), the slot
        // frees, and the ledger reconciles. Two agents so a survivor stays funded.
        let mut society = Society::from_scenario(MarketScenario {
            name: "commodity-credit-neutral",
            scenario: ScenarioName::CommodityCreditNeutral,
            seed: 1,
            periods: 1,
            agents: vec![redemption_agent(1, Gold(2)), redemption_agent(2, Gold(3))],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        });
        let commodity_base_before = society
            .money_system
            .as_ref()
            .expect("M3 society has a money system")
            .base
            .commodity_base;
        assert_eq!(commodity_base_before, Gold(5));

        let estate = society
            .remove_agent(AgentId(1))
            .expect("a funded specie M3 agent is removable in G8a");
        assert_eq!(
            estate.gold,
            Gold(2),
            "the estate carries the drained specie"
        );

        // The slot is freed; the dead id no longer resolves.
        assert!(society.agents.get(AgentId(1)).is_none());

        let money_system = society
            .money_system
            .as_ref()
            .expect("M3 society has a money system");
        // `commodity_base` fell by exactly the drained specie — conserved end to end
        // (estate.gold + commodity_base_after == commodity_base_before).
        assert_eq!(money_system.base.commodity_base, Gold(3));
        assert_eq!(
            estate.gold.0 + money_system.base.commodity_base.0,
            commodity_base_before.0,
            "the drain is conserved: estate + remaining ledger == the original base"
        );
        // The dead agent's ledger row is gone; the survivor's balance is untouched and
        // its cache still mirrors the ledger.
        assert!(money_system.balance_snapshot(AgentId(1)).is_none());
        assert_eq!(money_system.spendable_total(AgentId(2)), Gold(3));
        assert_eq!(society.agents.get(AgentId(2)).unwrap().gold, Gold(3));
        assert!(
            society.money_ledgers_reconcile(),
            "the M3 ledger reconciles after the drain"
        );
    }

    #[test]
    fn removal_still_refuses_a_funded_m3_balance_with_fiat() {
        // G8a drains **specie** only; fiat (and demand claims) remain deferred to
        // G8b/c, so a balance holding either still refuses removal — there is no
        // conserved estate route for them yet.
        let society_agent = redemption_agent(1, Gold::ZERO);
        let mut society = test_m3_society(society_agent, GOLD, Vec::new());
        society
            .money_system
            .as_mut()
            .expect("M3 society has a money system")
            .credit_fiat(AgentId(1), Gold(2))
            .expect("crediting fiat to a live ledger agent");
        let len_before = society.agents.len();

        assert!(
            !society.can_remove_agent(AgentId(1)),
            "a fiat-bearing M3 balance is not yet drainable"
        );
        assert!(
            society.remove_agent(AgentId(1)).is_none(),
            "funded-fiat M3 removal is refused"
        );
        assert_eq!(
            society.agents.len(),
            len_before,
            "a refused removal mutates nothing"
        );
    }

    #[test]
    fn removal_cancels_resting_labor_order() {
        let mut worker = redemption_agent(1, Gold::ZERO);
        worker.labor_capacity = 1;
        let mut society = test_m3_society(worker, GOLD, Vec::new());
        let work = LaborOrder {
            agent: AgentId(1),
            side: FactorSide::Work,
            wage_limit: Gold(1),
            qty: 1,
            seq: 1,
            expires_tick: 3,
        };
        assert!(society
            .labor_reservations
            .reserve_order(society.agents.as_slice(), &work));
        let trades = {
            let mut market = LaborMarketView {
                agents: society.agents.as_mut_slice(),
                reservations: &mut society.labor_reservations,
                projects: &mut society.m2_projects,
                lines: &society.project_lines,
                money_system: society.money_system.as_mut(),
                wage_media: society.labor_wage_tender.accepted_media(),
                wage_audit: Some(&mut society.wage_payment_audit),
                wage_tender: society.labor_wage_tender,
            };
            society.labor_book.add_order(work, None, 0, &mut market)
        };

        assert!(trades.is_empty());
        assert!(society.labor_book.has_live(AgentId(1), FactorSide::Work));
        assert_eq!(society.labor_reservations.reserved_labor(AgentId(1)), 1);

        assert!(society.remove_agent(AgentId(1)).is_some());

        assert!(!society.labor_book.has_live(AgentId(1), FactorSide::Work));
        assert_eq!(society.labor_reservations.reserved_labor(AgentId(1)), 0);
        // Real removal frees the slot, so the worker id no longer resolves and the
        // labor reservation cache holds no entry for it (forget_agent).
        assert!(society.agents.get(AgentId(1)).is_none());
    }

    #[test]
    fn removal_cancels_resting_barter_offer() {
        let mut trader = redemption_agent(1, Gold::ZERO);
        trader.stock.add(FOOD, 1);
        trader.scale = vec![Want {
            kind: WantKind::Good(WOOD),
            horizon: Horizon::Now,
            qty: 1,
            satisfied: false,
        }];
        let mut counterparty = redemption_agent(2, Gold::ZERO);
        counterparty.stock.add(WOOD, 1);
        counterparty.scale = vec![Want {
            kind: WantKind::Good(FOOD),
            horizon: Horizon::Now,
            qty: 1,
            satisfied: false,
        }];
        let mut society = Society::from_scenario(MarketScenario {
            name: "g4a-barter-removal",
            scenario: ScenarioName::MarketPriceDiscovery,
            seed: 1,
            periods: 1,
            agents: vec![trader, counterparty],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        });
        assert!(society.barter_book.post_offer(
            society.agents.as_slice(),
            BarterOffer {
                agent: AgentId(1),
                give_good: FOOD,
                receive_good: WOOD,
                qty: 1,
                reason: BarterReason::DirectWant,
                seq: 1,
                expires_tick: 3,
            },
            0,
        ));
        assert_eq!(society.barter_book.reserved_qty(AgentId(1), FOOD), 1);

        assert!(society.remove_agent(AgentId(1)).is_some());

        assert!(society
            .barter_book
            .live_offers()
            .iter()
            .all(|offer| offer.agent != AgentId(1)));
        assert_eq!(society.barter_book.reserved_qty(AgentId(1), FOOD), 0);
        assert!(society.agents.get(AgentId(1)).is_none());
    }

    #[test]
    fn removal_freezes_dead_owner_project_funding_plans() {
        let mut owner = test_capitalist(Stock::new(3));
        owner.gold = Gold(3);
        let mut society = test_society(owner);
        society
            .project_funding_plans
            .push(funding_plan(1, AgentId(1), ProjectLineId(1), Gold(2)));
        assert_eq!(society.reserved_project_gold(AgentId(1)), Gold(2));

        // Conservation: `reserved_gold` is an earmark on the owner's own `gold`, not
        // a separate store. The estate sweeps the FULL balance (the reserved 2 of the
        // 3 included), so freezing the plan — zeroing its reserve below — destroys no
        // money. Whole-system gold balances exactly across the removal.
        let gold_before = society.total_gold();

        let estate = society.remove_agent(AgentId(1)).expect("owner removes");

        assert_eq!(estate.gold, Gold(3));
        assert_eq!(
            society.total_gold().saturating_add(estate.gold),
            gold_before,
            "freezing a dead owner's reserved project gold must not destroy money"
        );
        assert_eq!(society.project_funding_plans.len(), 1);
        assert_eq!(society.project_funding_plans[0].owner, AgentId(1));
        assert_eq!(society.project_funding_plans[0].reserved_gold, Gold::ZERO);
        assert_eq!(society.project_funding_plans[0].expires_tick, society.tick);
        assert_eq!(society.reserved_project_gold(AgentId(1)), Gold::ZERO);
        assert!(
            society.project_funding_invariants_hold(),
            "a frozen dead-owned plan must not dangle as a live-owner invariant"
        );
        society.step();
        assert!(society.project_funding_invariants_hold());
    }

    #[test]
    fn removal_skips_m2_due_debt_settlement_for_borrower() {
        let borrower = redemption_agent(1, Gold(5));
        let lender = redemption_agent(2, Gold::ZERO);
        let mut society = Society::from_scenario(MarketScenario {
            name: "m2-removal-debt-freeze",
            scenario: ScenarioName::TimeMarketBasic,
            seed: 1,
            periods: 1,
            agents: vec![borrower, lender],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        });
        society.debts.push(DebtContract {
            id: DebtId(901),
            lender: CreditLender::Agent(AgentId(2)),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(0),
            principal: Gold(3),
            due: Gold(3),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::Commodity,
        });

        let estate = society.remove_agent(AgentId(1)).expect("borrower removes");
        assert_eq!(estate.gold, Gold(5), "the dead borrower's gold settles out");
        society.step();

        // The dead borrower's debt is frozen, not settled: the borrower is removed,
        // the lender receives nothing, and the contract stays Open and unpaid.
        assert!(society.agents.get(AgentId(1)).is_none(), "borrower removed");
        assert_eq!(society.agents.get(AgentId(2)).unwrap().gold, Gold::ZERO);
        assert_eq!(society.debts[0].paid, Gold::ZERO);
        assert_eq!(society.debts[0].state, DebtState::Open);
    }

    #[test]
    fn removal_skips_m3_due_debt_settlement_for_agent_lender() {
        let lender = redemption_agent(1, Gold::ZERO);
        let borrower = redemption_agent(2, Gold(5));
        let mut society = Society::from_scenario(MarketScenario {
            name: "m3-removal-debt-freeze",
            scenario: ScenarioName::CommodityCreditNeutral,
            seed: 1,
            periods: 1,
            agents: vec![lender, borrower],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        });
        society.debts.push(DebtContract {
            id: DebtId(902),
            lender: CreditLender::Agent(AgentId(1)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(0),
            principal: Gold(3),
            due: Gold(3),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::Commodity,
        });

        // The agent-lender holds an empty ledger balance, so removal frees its slot
        // and forgets its (zero) ledger entry — the M3 money invariants still hold.
        assert!(society.remove_agent(AgentId(1)).is_some());
        society.step();

        // The dead lender's receivable is frozen, not settled: the lender is
        // removed, the borrower keeps its gold, and the contract stays Open/unpaid.
        assert!(society.agents.get(AgentId(1)).is_none(), "lender removed");
        assert_eq!(society.agents.get(AgentId(2)).unwrap().gold, Gold(5));
        assert_eq!(society.debts[0].paid, Gold::ZERO);
        assert_eq!(society.debts[0].state, DebtState::Open);
        assert!(society.debt_payment_audit.is_empty());
        assert!(society.money_ledgers_reconcile());
    }

    #[test]
    fn m3_spot_trade_records_payment_composition() {
        let buyer = redemption_agent(1, Gold(1));
        let mut seller = redemption_agent(2, Gold::ZERO);
        seller.stock.add(FOOD, 1);
        let mut society = Society::from_scenario(MarketScenario {
            name: "payment-audit-test",
            scenario: ScenarioName::CommodityCreditNeutral,
            seed: 1,
            periods: 1,
            agents: vec![buyer, seller],
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        });
        society
            .money_system
            .as_mut()
            .unwrap()
            .credit_fiat(AgentId(1), Gold(1))
            .unwrap();
        society
            .money_system
            .as_ref()
            .unwrap()
            .reconcile_agent_cache(society.agents.as_mut_slice());
        society.public_spot_tender = PublicSpotTender::FiatAndSpecie;

        let seller_index = society.agent_index_for(AgentId(2)).unwrap();
        let buyer_index = society.agent_index_for(AgentId(1)).unwrap();
        let mut filled = Vec::new();
        society.ensure_order(
            QuotePlan {
                agent_index: seller_index,
                side: OrderSide::Ask,
                good: FOOD,
                reservation: Gold(1),
                limit: Gold(1),
                existing: None,
            },
            &mut filled,
        );
        society.ensure_order(
            QuotePlan {
                agent_index: buyer_index,
                side: OrderSide::Bid,
                good: FOOD,
                reservation: Gold(1),
                limit: Gold(1),
                existing: None,
            },
            &mut filled,
        );

        assert_eq!(society.payment_audit.len(), 1);
        let row = &society.payment_audit[0];
        assert_eq!(row.from, AgentId(1));
        assert_eq!(row.to, AgentId(2));
        assert_eq!(row.amount, Gold(1));
        assert_eq!(row.public_fiat, Gold(1));
        assert_eq!(row.demand_claims, Gold::ZERO);
        assert_eq!(row.public_specie, Gold::ZERO);
        assert_eq!(row.tender, PublicSpotTender::FiatAndSpecie);
    }

    #[test]
    fn larger_issuer_policy_amount_still_fills_unit_project_borrows() {
        let mut scenario = builtin_market_scenario(ScenarioName::FiatCreditExpansion);
        for event in &mut scenario.events {
            if let EventKind::SetIssuerPolicy { policy, .. } = &mut event.kind {
                policy.max_credit_issue_per_tick = Gold(2);
                policy.loan_present = Gold(2);
                policy.loan_future_due = Gold(2);
            }
        }
        let periods = scenario.periods;
        let mut society = Society::from_scenario(scenario);

        society.run(periods);

        assert_eq!(society.m3_records[0].fiat_credit_issued, Gold(2));
        let issuer_trades = society
            .loan_trades
            .iter()
            .filter(|trade| matches!(trade.funding, CreditSource::FiatCredit(_)))
            .collect::<Vec<_>>();
        assert!(!issuer_trades.is_empty());
        assert!(issuer_trades.iter().all(|trade| trade.present == Gold(1)));
    }
}
