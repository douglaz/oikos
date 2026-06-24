//! Per-tick observable snapshots.

use crate::agent::{AgentId, Role};
use crate::capital::M2ProjectId;
use crate::good::{Gold, GoodId};
use crate::ledger::{BankId, IssuerId};
use crate::money::{
    BankRepaymentTender, IssuerRepaymentTender, LaborWageTender, PublicDebtTender,
    PublicSpotTender, Regime, ReserveRatioBps, TaxReceivability,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum V2Phase {
    #[default]
    Barter,
    Money,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct V2Record {
    pub tick: u64,
    pub phase: V2Phase,
    pub money_good: Option<GoodId>,
    pub promoted_this_tick: bool,
    pub barter_trades: u32,
    pub spot_trades: u32,
    pub candidate_good: Option<GoodId>,
    /// Saleability share of the leading candidate and its runner-up, in basis
    /// points. These track the leadership metric, which is flag-dependent:
    /// under `two_layer_saleability` they carry the MEDIUM (re-trade) share
    /// (`indirect_acceptances / total_indirect_acceptances`), since the
    /// leadership race is the medium race; otherwise they carry the combined
    /// total-acceptance share. Read alongside `money_good`/`candidate_good`,
    /// not as a fixed metric across configs.
    pub candidate_share_bps: Option<u16>,
    pub runner_up_share_bps: Option<u16>,
    pub total_money_units: Gold,
    pub bid_count: u32,
    pub ask_count: u32,
    /// Expired barter offers during `Barter`, expired spot orders during `Money`.
    pub expired_orders: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Record {
    pub tick: u64,
    pub food: u32,
    pub wood: u32,
    pub nets: u32,
    pub labor_used: u32,
    pub leisure_taken: u32,
    pub food_consumed: u32,
    pub hunger_deficit: u32,
    pub active_projects: u32,
    pub completed_projects: u32,
    pub abandoned_projects: u32,
    pub capital_labor_consumed: u32,
    pub capital_goods_consumed: u32,
    pub gather_actions: u32,
    pub cut_wood_actions: u32,
    pub fish_actions: u32,
    pub project_actions: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarketRecord {
    pub tick: u64,
    pub total_gold: Gold,
    pub trades: u32,
    pub food_volume: u32,
    pub wood_volume: u32,
    pub net_volume: u32,
    pub last_food_price: Option<Gold>,
    pub last_wood_price: Option<Gold>,
    pub last_net_price: Option<Gold>,
    pub bid_count: u32,
    pub ask_count: u32,
    pub expired_orders: u32,
    pub good_volumes: Vec<(GoodId, u32)>,
    pub last_prices: Vec<(GoodId, Gold)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct M2Record {
    pub tick: u64,
    pub total_gold: Gold,
    pub spot_trades: u32,
    pub labor_trades: u32,
    pub loan_trades: u32,
    pub project_loan_trades: u32,
    pub project_borrowed_gold: Gold,
    pub debts_open: u32,
    pub debts_settled: u32,
    pub debts_defaulted: u32,
    pub project_debts_open: u32,
    pub project_debts_settled: u32,
    pub project_debts_defaulted: u32,
    pub project_funding_reserved_gold: Gold,
    pub active_projects: u32,
    pub waiting_projects: u32,
    pub mature_projects: u32,
    pub sold_projects: u32,
    pub abandoned_projects: u32,
    pub labor_advanced: u32,
    pub wages_paid: Gold,
    pub project_revenue: Gold,
    pub project_profit: i64,
    pub capital_labor_consumed: u32,
    pub capital_goods_consumed: u32,
    pub capital_gold_loss: Gold,
    pub market_rate_bps: Option<i64>,
    pub natural_rate_proxy_bps: Option<i64>,
    pub rate_gap_bps: Option<i64>,
    pub structure_length_ticks_x100: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct M3Record {
    pub m2: M2Record,
    pub regime: Regime,
    pub public_specie: Gold,
    pub public_fiat: Gold,
    pub demand_claims: Gold,
    pub bank_reserves: Gold,
    pub fiduciary: Gold,
    pub time_deposits: Gold,
    pub tms: Gold,
    pub bank_credit_issued: Gold,
    pub fiat_credit_issued: Gold,
    pub fiat_fiscal_issued: Gold,
    pub credit_retired: Gold,
    pub bank_loan_trades: u32,
    pub fiat_loan_trades: u32,
    pub shadow_natural_rate_bps: Option<i64>,
    pub shadow_rate_gap_bps: Option<i64>,
    pub boom_projects_started: u32,
    pub bust_abandoned_projects: u32,
    pub early_receiver_wealth_delta: i64,
    pub late_receiver_wealth_delta: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CantillonCohort {
    EarlyReceiver,
    LateReceiver,
    #[default]
    NonReceiver,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentWealthRecord {
    pub tick: u64,
    pub agent: AgentId,
    pub cohort: CantillonCohort,
    pub primary_role: Option<Role>,
    pub spendable_money: Gold,
    pub stock_value: u128,
    pub real_wealth: i128,
    /// Current holdings plus explicitly consumed goods, valued only at realized
    /// trade prices. Unpriced consumed goods contribute zero, matching the M3
    /// realized-basket delta convention.
    pub realized_delta: i128,
    pub unpriced_stock_units: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MetricObservation {
    pub tick: u64,
    pub agent_wealth: Vec<AgentWealthRecord>,
    pub labor_capacity: u32,
    pub labor_used: u32,
    pub idle_labor_units: u32,
    pub tick_price_dispersion_bps: Option<u64>,
    pub cumulative_price_dispersion_bps: Option<u64>,
    /// Executed spot-good sector dispersion. `None` means fewer than two
    /// non-money good sectors traded, not that factor/project prices were
    /// imputed.
    pub tick_sector_price_dispersion_bps: Option<u64>,
    /// Executed spot-good sector dispersion through this tick. `None` means
    /// fewer than two non-money good sectors traded, not that factor/project
    /// prices were imputed.
    pub cumulative_sector_price_dispersion_bps: Option<u64>,
    pub unpriced_stock_units: u32,
    pub arithmetic_overflowed: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct M4Record {
    pub tick: u64,
    pub regime: Regime,
    pub tms: Gold,
    pub public_specie: Gold,
    pub public_fiat: Gold,
    pub demand_claims: Gold,
    pub fiduciary: Gold,
    pub bank_credit_issued: Gold,
    pub fiat_credit_issued: Gold,
    pub fiat_fiscal_issued: Gold,
    pub credit_retired: Gold,
    pub market_rate_bps: Option<i64>,
    pub shadow_natural_rate_bps: Option<i64>,
    pub shadow_rate_gap_bps: Option<i64>,
    pub structure_length_ticks_x100: u64,
    pub active_projects: u32,
    pub waiting_projects: u32,
    pub mature_projects: u32,
    pub sold_projects: u32,
    pub abandoned_projects: u32,
    pub bust_abandoned_projects: u32,
    pub capital_labor_consumed: u32,
    pub capital_goods_consumed: u32,
    pub debts_defaulted: u32,
    pub project_debts_defaulted: u32,
    pub agent_count: u32,
    pub early_receiver_count: u32,
    pub late_receiver_count: u32,
    pub non_receiver_count: u32,
    pub real_wealth_gini_bps: Option<u32>,
    pub lorenz_bottom_50_share_bps: Option<u32>,
    pub lorenz_top_10_share_bps: Option<u32>,
    pub early_receiver_mean_real_wealth: i128,
    pub late_receiver_mean_real_wealth: i128,
    pub non_receiver_mean_real_wealth: i128,
    pub early_late_real_wealth_gap: i128,
    /// Mean realized-basket delta for early receivers. Consumed goods without a
    /// realized trade price are zero-valued by design.
    pub early_receiver_mean_realized_delta: i128,
    /// Mean realized-basket delta for late receivers. Consumed goods without a
    /// realized trade price are zero-valued by design.
    pub late_receiver_mean_realized_delta: i128,
    pub tick_price_dispersion_bps: Option<u64>,
    pub cumulative_price_dispersion_bps: Option<u64>,
    /// Executed spot-good sector dispersion. `None` means fewer than two
    /// non-money good sectors traded, not that factor/project prices were
    /// imputed.
    pub tick_sector_price_dispersion_bps: Option<u64>,
    /// Executed spot-good sector dispersion through this tick. `None` means
    /// fewer than two non-money good sectors traded, not that factor/project
    /// prices were imputed.
    pub cumulative_sector_price_dispersion_bps: Option<u64>,
    pub unpriced_stock_units: u32,
    pub labor_capacity: u32,
    pub labor_used: u32,
    pub idle_labor_units: u32,
    pub idle_labor_bps: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoneyAuditRecord {
    pub tick: u64,
    pub agent: AgentId,
    pub public_specie: Gold,
    pub public_fiat: Gold,
    pub demand_claims: Gold,
    pub spendable_money: Gold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaymentKind {
    Spot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentAuditRecord {
    pub tick: u64,
    pub kind: PaymentKind,
    pub from: AgentId,
    pub to: AgentId,
    pub amount: Gold,
    pub public_fiat: Gold,
    pub demand_claims: Gold,
    pub public_specie: Gold,
    pub tender: PublicSpotTender,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WagePaymentAuditRecord {
    pub tick: u64,
    pub project: M2ProjectId,
    pub employer: AgentId,
    pub worker: AgentId,
    pub wage: Gold,
    pub qty: u32,
    pub amount: Gold,
    pub public_fiat: Gold,
    pub demand_claims: Gold,
    pub public_specie: Gold,
    pub tender: LaborWageTender,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebtPaymentState {
    Settled,
    Defaulted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebtPaymentAuditRecord {
    pub tick: u64,
    pub debt: u64,
    pub from: AgentId,
    pub to: AgentId,
    pub owed: Gold,
    pub paid: Gold,
    pub remaining: Gold,
    pub public_fiat: Gold,
    pub demand_claims: Gold,
    pub public_specie: Gold,
    pub tender: PublicDebtTender,
    pub state: DebtPaymentState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BankRepaymentAuditRecord {
    pub tick: u64,
    pub debt: u64,
    pub borrower: AgentId,
    pub bank: BankId,
    pub owed: Gold,
    pub paid: Gold,
    pub remaining: Gold,
    pub public_fiat: Gold,
    pub demand_claims: Gold,
    pub public_specie: Gold,
    pub credit_retired: Gold,
    pub tender: BankRepaymentTender,
    pub state: DebtPaymentState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssuerRepaymentAuditRecord {
    pub tick: u64,
    pub debt: u64,
    pub borrower: AgentId,
    pub issuer: IssuerId,
    pub owed: Gold,
    pub paid: Gold,
    pub remaining: Gold,
    pub public_fiat: Gold,
    pub public_specie: Gold,
    pub credit_retired: Gold,
    pub tender: IssuerRepaymentTender,
    pub state: DebtPaymentState,
}

/// One due tax liability, settled or defaulted (M21). `paid_fiat` and
/// `paid_specie` decompose the receipt; neither ever feeds a credit metric.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaxAuditRecord {
    pub tick: u64,
    pub debt: u64,
    pub agent: AgentId,
    pub issuer: IssuerId,
    pub owed: Gold,
    pub paid: Gold,
    pub remaining: Gold,
    pub paid_fiat: Gold,
    pub paid_specie: Gold,
    pub receivability: TaxReceivability,
    pub state: DebtPaymentState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BankAuditRecord {
    pub tick: u64,
    pub bank: BankId,
    pub reserves: Gold,
    pub demand_deposits: Gold,
    pub time_deposits: Gold,
    pub loans_outstanding: Gold,
    pub fiduciary_issued: Gold,
    pub reserve_ratio_bps: ReserveRatioBps,
    pub convertible: bool,
    pub policy_enabled: bool,
    pub policy_max_new_fiduciary_per_tick: Gold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedemptionOutcome {
    Honored,
    PartiallyHonored,
    ReserveExhausted,
    Suspended,
    NoClaim,
    BankMissing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedemptionAuditRecord {
    pub tick: u64,
    pub bank: BankId,
    pub agent: AgentId,
    pub requested: Gold,
    pub honored: Gold,
    pub failed: Gold,
    pub outcome: RedemptionOutcome,
}
