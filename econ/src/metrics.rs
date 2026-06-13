//! Read-only simulation measurements.

use std::collections::BTreeMap;

use crate::agent::{Agent, AgentId};
use crate::agio::AgioSchedule;
use crate::cantillon::CantillonReceipt;
use crate::capital::{M2Project, M2ProjectState, ProjectOutputLot};
use crate::good::{Gold, GoodId, FOOD, NET, WOOD};
use crate::ledger::MoneySystem;
use crate::market::Trade;
use crate::project::Tick;
use crate::purpose::{CreditLender, CreditSource, LoanPurpose};
use crate::record::{AgentWealthRecord, CantillonCohort, M3Record, M4Record, MetricObservation};
use crate::society::Society;
use crate::timemarket::{DebtId, LoanOrder, LoanSide, LoanTrade};

const BPS_SCALE: u128 = 10_000;
// FNV-1a 128-bit constants, used only to detect when a caller replaces an
// already-observed trade prefix while reusing the same accumulator.
const TRADE_FINGERPRINT_OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
const TRADE_FINGERPRINT_MULTIPLIER: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

struct RealizedDeltaContext<'a> {
    money_system: Option<&'a MoneySystem>,
    prices: &'a [(GoodId, Gold)],
    money_good: GoodId,
    stock_goods: &'a [GoodId],
}

struct AgentWealthBuilder<'a, 'b> {
    tick: u64,
    money_system: Option<&'a MoneySystem>,
    prices: &'a [(GoodId, Gold)],
    money_good: GoodId,
    stock_goods: &'a [GoodId],
    cohorts: &'a [(AgentId, CantillonCohort)],
    trade_quantities: &'a RealizedTradeQuantities,
    already_overflowed: bool,
    agent_wealth: &'b mut Vec<AgentWealthRecord>,
    unpriced_stock_units: &'b mut u32,
    overflowed: &'b mut bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct RealizedTradeQuantities {
    consumed: BTreeMap<(AgentId, GoodId), u128>,
}

pub struct MetricObservationInput<'a> {
    pub tick: u64,
    pub agents: &'a [Agent],
    pub initial_agents: &'a [Agent],
    pub money_system: Option<&'a MoneySystem>,
    pub receipts: &'a [CantillonReceipt],
    pub trades: &'a [Trade],
    pub tick_trades: &'a [Trade],
    /// New consumption events since the previous observation. Stateless callers
    /// can pass all events for the single observation they are building.
    pub consumed_goods: &'a [(AgentId, GoodId, u32)],
    pub money_good: GoodId,
    pub stock_goods: &'a [GoodId],
    pub labor_capacity: u32,
    pub labor_used: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MetricObservationAccumulator {
    trades_seen: usize,
    receipts_seen: usize,
    observed_trade_fingerprint: u128,
    observed_trade_tail: Option<Trade>,
    prices: Vec<(GoodId, Gold)>,
    trade_quantities: RealizedTradeQuantities,
    basket_dispersion: BasketPriceDispersionState,
    sector_dispersion: SectorPriceDispersionState,
    first_receipt_tick: Option<Tick>,
    early_receivers: Vec<AgentId>,
    late_receivers: Vec<AgentId>,
    cohort_agent_ids: Vec<AgentId>,
    cohort_cache: Vec<(AgentId, CantillonCohort)>,
    agent_order_cache_ids: Vec<AgentId>,
    agent_order_cache_indices: Vec<usize>,
    initial_order_cache_ids: Vec<AgentId>,
    initial_order_cache_indices: Vec<usize>,
    cohorts_dirty: bool,
    overflowed: bool,
}

impl Default for MetricObservationAccumulator {
    fn default() -> Self {
        Self {
            trades_seen: 0,
            receipts_seen: 0,
            observed_trade_fingerprint: TRADE_FINGERPRINT_OFFSET,
            observed_trade_tail: None,
            prices: Vec::new(),
            trade_quantities: RealizedTradeQuantities::default(),
            basket_dispersion: BasketPriceDispersionState::default(),
            sector_dispersion: SectorPriceDispersionState::default(),
            first_receipt_tick: None,
            early_receivers: Vec::new(),
            late_receivers: Vec::new(),
            cohort_agent_ids: Vec::new(),
            cohort_cache: Vec::new(),
            agent_order_cache_ids: Vec::new(),
            agent_order_cache_indices: Vec::new(),
            initial_order_cache_ids: Vec::new(),
            initial_order_cache_indices: Vec::new(),
            cohorts_dirty: false,
            overflowed: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct BasketPriceDispersionState {
    goods: BTreeMap<GoodId, GoodPriceStats>,
    total_qty: u128,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct GoodPriceStats {
    total_value: u128,
    total_qty: u128,
    qty_by_price: BTreeMap<Gold, u128>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct SectorPriceDispersionState {
    sectors: BTreeMap<M4Sector, (u128, u128)>,
}

pub fn agio_bps_per_tick(present: Gold, future: Gold, horizon: u8) -> Option<i64> {
    if present == Gold::ZERO || future < present || horizon == 0 {
        return None;
    }
    let premium = i128::from(future.0.saturating_sub(present.0));
    let denom = i128::from(present.0).checked_mul(i128::from(horizon))?;
    let bps = premium
        .checked_mul(i128::try_from(BPS_SCALE).ok()?)?
        .checked_div(denom)?;
    i64::try_from(bps).ok()
}

pub fn weighted_loan_bps(trades: &[LoanTrade]) -> Option<i64> {
    let mut weighted = 0i128;
    let mut weight = 0i128;
    for trade in trades {
        let bps = i128::from(agio_bps_per_tick(
            trade.present,
            trade.future_due,
            trade.horizon,
        )?);
        let present = i128::from(trade.present.0);
        weighted = weighted.checked_add(bps.checked_mul(present)?)?;
        weight = weight.checked_add(present)?;
    }
    if weight == 0 {
        None
    } else {
        i64::try_from(weighted / weight).ok()
    }
}

pub fn proxy_trades_from_schedules(
    tick: u64,
    schedules: &[(AgentId, AgioSchedule)],
) -> Vec<LoanTrade> {
    let mut lends = Vec::new();
    let mut borrows = Vec::new();
    let mut seq = 0u64;
    for (agent, schedule) in schedules {
        for horizon in [1u8, 2, 4, 7] {
            if let Some(future_limit) = schedule.min_future_due_for_lending(Gold(1), horizon) {
                seq += 1;
                lends.push(LoanOrder {
                    agent: *agent,
                    lender: CreditLender::Agent(*agent),
                    side: LoanSide::Lend,
                    present: Gold(1),
                    future_limit,
                    horizon,
                    seq,
                    expires_tick: tick.saturating_add(3),
                    purpose: LoanPurpose::Consumption,
                    funding: CreditSource::Commodity,
                });
            }
            if let Some(future_limit) = schedule.max_future_due_for_borrowing(Gold(1), horizon) {
                seq += 1;
                borrows.push(LoanOrder {
                    agent: *agent,
                    lender: CreditLender::Agent(*agent),
                    side: LoanSide::Borrow,
                    present: Gold(1),
                    future_limit,
                    horizon,
                    seq,
                    expires_tick: tick.saturating_add(3),
                    purpose: LoanPurpose::Consumption,
                    funding: CreditSource::Commodity,
                });
            }
        }
    }

    // Mirror the live loan book's resting-lend pricing: every lend order rests
    // first, then each borrow order (in arrival/seq order) crosses against the
    // best resting lend (lowest future_limit, earliest seq) and trades at that
    // resting order's limit.
    borrows.sort_by_key(|order| order.seq);
    let mut trades = Vec::new();
    let mut resting_lends: BTreeMap<(Gold, u8), BTreeMap<(Gold, u64), LoanOrder>> = BTreeMap::new();
    for lend in lends {
        resting_lends
            .entry((lend.present, lend.horizon))
            .or_default()
            .insert((lend.future_limit, lend.seq), lend);
    }
    for borrow in &borrows {
        let Some(book) = resting_lends.get_mut(&(borrow.present, borrow.horizon)) else {
            continue;
        };
        let selected = book
            .range(..=(borrow.future_limit, u64::MAX))
            .find(|(_, lend)| lend.agent != borrow.agent)
            .map(|(key, _)| *key);
        if let Some(key) = selected {
            let lend = book.remove(&key).expect("selected lend exists");
            trades.push(LoanTrade {
                tick,
                lender: CreditLender::Agent(lend.agent),
                borrower: borrow.agent,
                present: lend.present,
                future_due: lend.future_limit,
                horizon: lend.horizon,
                debt: DebtId(0),
                purpose: LoanPurpose::Consumption,
                project: None,
                funding: CreditSource::Commodity,
            });
        }
    }
    trades
}

pub fn structure_length_ticks_x100(projects: &[M2Project], tick: Tick) -> u64 {
    let mut weighted = 0u128;
    let mut total_weight = 0u128;
    for project in projects {
        if !matches!(
            project.state,
            M2ProjectState::Forming | M2ProjectState::Waiting
        ) {
            continue;
        }
        let remaining = project.maturity.0.saturating_sub(tick.0);
        let weight =
            u128::from(project.advanced_gold.0).saturating_add(u128::from(project.labor_advanced));
        if weight == 0 {
            continue;
        }
        weighted = weighted.saturating_add(u128::from(remaining).saturating_mul(weight));
        total_weight = total_weight.saturating_add(weight);
    }
    weighted
        .saturating_mul(100)
        .checked_div(total_weight)
        .and_then(|value| u64::try_from(value).ok())
        .unwrap_or(0)
}

pub fn cumulative_project_revenue(lots: &[ProjectOutputLot]) -> Gold {
    lots.iter()
        .fold(Gold::ZERO, |total, lot| total.saturating_add(lot.proceeds))
}

pub fn cumulative_project_profit(projects: &[M2Project], lots: &[ProjectOutputLot]) -> i64 {
    let mut profit = 0i128;
    for project in projects {
        if project.state != M2ProjectState::Sold {
            continue;
        }
        let revenue = lots
            .iter()
            .filter(|lot| lot.project == project.id)
            .fold(Gold::ZERO, |total, lot| total.saturating_add(lot.proceeds));
        profit += i128::from(revenue.0);
        profit -= i128::from(project.advanced_gold.0);
        profit -= i128::from(project.input_cost_basis.0);
    }
    i64::try_from(profit).unwrap_or_else(|_| {
        if profit.is_negative() {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

/// Total absolute project forecast error for projects that reached sale or abandonment.
///
/// Returns `None` instead of saturating if an extreme finished run exceeds `u128`.
pub fn aggregate_project_forecast_error(
    projects: &[M2Project],
    lots: &[ProjectOutputLot],
) -> Option<u128> {
    let mut realized_by_project = BTreeMap::new();
    for lot in lots {
        let realized = realized_by_project.entry(lot.project).or_insert(0u128);
        *realized = realized.checked_add(u128::from(lot.proceeds.0))?;
    }

    let mut total = 0u128;
    for project in projects {
        if !matches!(
            project.state,
            M2ProjectState::Sold | M2ProjectState::Abandoned
        ) {
            continue;
        }
        let realized = realized_by_project.get(&project.id).copied().unwrap_or(0);
        let expected = u128::from(project.expected_revenue.0);
        total = total.checked_add(expected.abs_diff(realized))?;
    }
    Some(total)
}

/// Population variance of per-tick TMS changes, computed with integer arithmetic.
pub fn tms_growth_variance(records: &[M3Record]) -> Option<u128> {
    let n = u128::try_from(records.len().saturating_sub(1)).ok()?;
    let sum = records.windows(2).try_fold(0i128, |sum, window| {
        sum.checked_add(signed_gold_delta(window[0].tms, window[1].tms))
    })?;
    integer_population_variance_from_parts(
        records
            .windows(2)
            .map(|window| signed_gold_delta(window[0].tms, window[1].tms)),
        n,
        sum,
    )
}

/// Run-wide realized price instability, measured as each trade's relative
/// deviation from its own good's volume-weighted average price.
///
/// The final basket is weighted by raw traded quantity across goods. That is a
/// deliberately simple realized-volume basket for the M3 hard/soft-money
/// direction test, not an equal-weighted index.
/// Returns `None` instead of saturating if an extreme tape exceeds `u128`.
pub fn basket_relative_price_dispersion(trades: &[Trade]) -> Option<u128> {
    let mut totals_by_good = BTreeMap::new();
    for trade in trades {
        if trade.qty == 0 {
            continue;
        }
        let qty = u128::from(trade.qty);
        let value = u128::from(trade.price.0).checked_mul(qty)?;
        let (total_value, total_qty) = totals_by_good.entry(trade.good).or_insert((0u128, 0u128));
        *total_value = total_value.checked_add(value)?;
        *total_qty = total_qty.checked_add(qty)?;
    }

    let mut weighted_bps = 0u128;
    let mut total_qty = 0u128;
    for trade in trades {
        if trade.qty == 0 {
            continue;
        }
        let (baseline_value, baseline_qty) = totals_by_good.get(&trade.good).copied()?;
        if baseline_qty == 0 {
            return None;
        }
        let qty = u128::from(trade.qty);
        if baseline_value == 0 {
            // Zero-price goods still count in the realized basket weight; they
            // contribute no within-good price dispersion.
            total_qty = total_qty.checked_add(qty)?;
            continue;
        }
        let diff_bps = scaled_relative_deviation_bps(trade.price, baseline_qty, baseline_value)?;
        weighted_bps = weighted_bps.checked_add(diff_bps.checked_mul(qty)?)?;
        total_qty = total_qty.checked_add(qty)?;
    }
    if total_qty == 0 {
        Some(0)
    } else {
        weighted_bps.checked_div(total_qty)
    }
}

pub fn realized_price_table(trades: &[Trade], through_tick: Option<u64>) -> Vec<(GoodId, Gold)> {
    let mut prices = BTreeMap::new();
    for trade in trades {
        if matches!(through_tick, Some(tick) if trade.tick > tick) || trade.qty == 0 {
            continue;
        }
        prices.insert(trade.good, trade.price);
    }
    prices.into_iter().collect()
}

/// Current M4 real wealth is gross of open future debts by design: liabilities
/// remain visible in debt/default records instead of being scalar-discounted.
pub fn agent_current_real_wealth(
    agent: &Agent,
    money_system: Option<&MoneySystem>,
    prices: &[(GoodId, Gold)],
    money_good: GoodId,
    stock_goods: &[GoodId],
) -> Option<(Gold, u128, i128, u32)> {
    let spendable_money = money_system
        .map(|money| money.spendable_total(agent.id))
        .unwrap_or(agent.gold);
    let mut stock_value = 0u128;
    let mut unpriced_stock_units = 0u32;
    for good in stock_goods {
        if *good == money_good {
            continue;
        }
        let qty = agent.stock.get(*good);
        if qty == 0 {
            continue;
        }
        let Some(price) = price_for(prices, *good) else {
            unpriced_stock_units = unpriced_stock_units.checked_add(qty)?;
            continue;
        };
        let value = u128::from(price.0).checked_mul(u128::from(qty))?;
        stock_value = stock_value.checked_add(value)?;
    }
    let real_wealth = u128::from(spendable_money.0)
        .checked_add(stock_value)
        .and_then(|value| i128::try_from(value).ok())?;
    Some((
        spendable_money,
        stock_value,
        real_wealth,
        unpriced_stock_units,
    ))
}

fn agent_realized_delta_with_quantities(
    agent: AgentId,
    current: &Agent,
    initial: &Agent,
    context: &RealizedDeltaContext<'_>,
    trade_quantities: &RealizedTradeQuantities,
) -> Option<i128> {
    let current_value = realized_position_value(
        current,
        context.money_system,
        context.prices,
        context.money_good,
        context.stock_goods,
    )?;
    let initial_value = realized_position_value(
        initial,
        None,
        context.prices,
        context.money_good,
        context.stock_goods,
    )?;
    let consumed_value = realized_consumed_value(agent, context.prices, trade_quantities)?;
    current_value
        .checked_sub(initial_value)?
        .checked_add(consumed_value)
}

pub fn cantillon_cohorts(
    receipts: &[CantillonReceipt],
    agents: &[Agent],
) -> Vec<(AgentId, CantillonCohort)> {
    let Some(first_tick) = receipts
        .iter()
        .filter(|receipt| is_new_money_receipt(receipt.source))
        .map(|receipt| receipt.tick)
        .min_by_key(|tick| tick.0)
    else {
        return cohorts_for_agents(agents, &[], &[]);
    };
    let mut early = Vec::new();
    let mut late = Vec::new();
    for receipt in receipts {
        if !is_new_money_receipt(receipt.source) {
            continue;
        }
        if receipt.tick == first_tick {
            early.push(receipt.agent);
        } else {
            late.push(receipt.agent);
        }
    }

    normalize_receivers(&mut early, &mut late);

    cohorts_for_agents(agents, &early, &late)
}

pub fn gini_bps(values: &[i128]) -> Option<u32> {
    let mut wealth = non_negative_values(values)?;
    let n = u128::try_from(wealth.len()).ok()?;
    if n == 0 {
        return Some(0);
    }
    wealth.sort();
    let total = wealth
        .iter()
        .try_fold(0u128, |sum, value| sum.checked_add(*value))?;
    if total == 0 {
        return Some(0);
    }

    // For sorted wealth, each adjacent gap contributes to every pair split by
    // that gap. Accumulate gap * left_count * right_count * BPS_SCALE / total
    // as quotient/remainder pieces so extreme valid u128 wealth vectors do not
    // require a wider intermediate type.
    let mut quotient_sum = 0u128;
    let mut remainder_sum = 0u128;
    for index in 1..wealth.len() {
        let gap = wealth[index].checked_sub(wealth[index - 1])?;
        if gap == 0 {
            continue;
        }
        let left_count = u128::try_from(index).ok()?;
        let right_count = u128::try_from(wealth.len() - index).ok()?;
        let crossing_pairs = left_count.checked_mul(right_count)?;
        let (pair_quotient, pair_remainder) = mul_div_rem(gap, crossing_pairs, total)?;
        let scaled_quotient = pair_quotient.checked_mul(BPS_SCALE)?;
        let (remainder_quotient, remainder) = mul_div_rem(pair_remainder, BPS_SCALE, total)?;
        (quotient_sum, remainder_sum) = add_quotient_remainder(
            quotient_sum,
            remainder_sum,
            scaled_quotient.checked_add(remainder_quotient)?,
            remainder,
            total,
        )?;
    }
    u32::try_from(quotient_sum.checked_div(n)?).ok()
}

pub fn lorenz_bottom_50_share_bps(values: &[i128]) -> Option<u32> {
    let mut wealth = non_negative_values(values)?;
    wealth.sort();
    let total = wealth
        .iter()
        .try_fold(0u128, |sum, value| sum.checked_add(*value))?;
    if total == 0 {
        return Some(0);
    }
    let count = wealth.len() / 2;
    let group_sum = wealth
        .iter()
        .take(count)
        .try_fold(0u128, |sum, value| sum.checked_add(*value))?;
    share_bps(group_sum, total)
}

pub fn lorenz_top_10_share_bps(values: &[i128]) -> Option<u32> {
    let mut wealth = non_negative_values(values)?;
    wealth.sort();
    let total = wealth
        .iter()
        .try_fold(0u128, |sum, value| sum.checked_add(*value))?;
    if total == 0 {
        return Some(0);
    }
    let count = wealth.len().div_ceil(10).max(1);
    let group_sum = wealth
        .iter()
        .rev()
        .take(count)
        .try_fold(0u128, |sum, value| sum.checked_add(*value))?;
    share_bps(group_sum, total)
}

/// Sectoral dispersion over executed spot-good trades only.
///
/// Returns `None` when fewer than two non-money good sectors have realized
/// spot trades. Project labor, loan trades, beliefs, and expected project
/// revenue are deliberately excluded so the output remains auditable to the
/// spot trade tape.
pub fn sector_price_dispersion_bps(trades: &[Trade], money_good: GoodId) -> Option<u64> {
    let mut state = SectorPriceDispersionState::default();
    for trade in trades {
        state.observe_trade(trade, money_good)?;
    }
    state.dispersion_bps()
}

fn sector_dispersion_from_totals(sectors: &BTreeMap<M4Sector, (u128, u128)>) -> Option<u64> {
    if sectors.len() < 2 {
        return None;
    }

    let (total_value, total_qty) =
        sectors
            .values()
            .try_fold((0u128, 0u128), |(value_sum, qty_sum), (value, qty)| {
                Some((value_sum.checked_add(*value)?, qty_sum.checked_add(*qty)?))
            })?;
    if total_qty == 0 {
        return None;
    }
    if total_value == 0 {
        return Some(0);
    }

    let mut weighted_bps = 0u128;
    for (sector_value, sector_qty) in sectors.values() {
        let sector_scaled = sector_value.checked_mul(total_qty)?;
        let baseline_scaled = total_value.checked_mul(*sector_qty)?;
        let numerator = sector_scaled.abs_diff(baseline_scaled);
        let denominator = sector_qty.checked_mul(total_value)?;
        let bps = mul_div_floor(numerator, BPS_SCALE, denominator)?;
        weighted_bps = weighted_bps.checked_add(bps.checked_mul(*sector_qty)?)?;
    }
    u64::try_from(weighted_bps.checked_div(total_qty)?).ok()
}

pub fn idle_labor_bps(capacity: u32, used: u32) -> Option<u32> {
    if capacity == 0 {
        return None;
    }
    let idle = capacity.saturating_sub(used);
    let bps = u128::from(idle)
        .checked_mul(BPS_SCALE)?
        .checked_div(u128::from(capacity))?;
    u32::try_from(bps).ok()
}

pub fn build_metric_observation(input: MetricObservationInput<'_>) -> MetricObservation {
    MetricObservationAccumulator::default().observe(input)
}

impl MetricObservationAccumulator {
    pub(crate) fn observe(&mut self, input: MetricObservationInput<'_>) -> MetricObservation {
        self.observe_new_trades(input.trades, input.money_good);
        self.observe_new_consumed_goods(input.consumed_goods);
        self.observe_new_receipts(input.receipts);

        self.refresh_cohort_cache(input.agents);
        self.refresh_agent_order_caches(input.agents, input.initial_agents);
        let prices = &self.prices;
        let cohorts = &self.cohort_cache;

        let mut agent_wealth = Vec::new();
        let mut unpriced_stock_units = 0u32;
        let already_overflowed = self.overflowed;
        let mut overflowed = self.overflowed;
        let mut wealth_builder = AgentWealthBuilder {
            tick: input.tick,
            money_system: input.money_system,
            prices,
            cohorts,
            money_good: input.money_good,
            stock_goods: input.stock_goods,
            trade_quantities: &self.trade_quantities,
            already_overflowed,
            agent_wealth: &mut agent_wealth,
            unpriced_stock_units: &mut unpriced_stock_units,
            overflowed: &mut overflowed,
        };
        observe_agent_wealth(
            &input,
            &self.agent_order_cache_indices,
            &self.initial_order_cache_indices,
            &mut wealth_builder,
        );
        self.overflowed = overflowed;

        let tick_price_dispersion_bps =
            observed_basket_price_dispersion_bps(input.tick_trades, &mut self.overflowed);
        let tick_sector_price_dispersion_bps = observed_sector_price_dispersion_bps(
            input.tick_trades,
            input.money_good,
            &mut self.overflowed,
        );
        let cumulative_price_dispersion_bps = self.cumulative_basket_price_dispersion_bps();
        let cumulative_sector_price_dispersion_bps = self.cumulative_sector_price_dispersion_bps();

        MetricObservation {
            tick: input.tick,
            agent_wealth,
            labor_capacity: input.labor_capacity,
            labor_used: input.labor_used,
            idle_labor_units: input.labor_capacity.saturating_sub(input.labor_used),
            tick_price_dispersion_bps,
            cumulative_price_dispersion_bps,
            tick_sector_price_dispersion_bps,
            cumulative_sector_price_dispersion_bps,
            unpriced_stock_units,
            arithmetic_overflowed: self.overflowed,
        }
    }

    fn observe_new_trades(&mut self, trades: &[Trade], money_good: GoodId) {
        if self.trades_prefix_changed(trades) {
            self.reset_trade_observations();
        }

        let start = self.trades_seen;
        for trade in &trades[start..] {
            self.observed_trade_fingerprint =
                extend_trade_fingerprint(self.observed_trade_fingerprint, trade);
            self.observed_trade_tail = Some(trade.clone());
            if trade.qty == 0 {
                continue;
            }
            set_realized_price(&mut self.prices, trade.good, trade.price);
            if self.basket_dispersion.observe_trade(trade).is_none()
                || self
                    .sector_dispersion
                    .observe_trade(trade, money_good)
                    .is_none()
            {
                self.overflowed = true;
            }
        }
        self.trades_seen = trades.len();
    }

    fn trades_prefix_changed(&self, trades: &[Trade]) -> bool {
        if self.trades_seen > trades.len() {
            return true;
        }
        if self.trades_seen == 0 {
            return false;
        }
        if trades.get(self.trades_seen - 1) != self.observed_trade_tail.as_ref() {
            return true;
        }
        trade_fingerprint(&trades[..self.trades_seen]) != self.observed_trade_fingerprint
    }

    fn reset_trade_observations(&mut self) {
        self.trades_seen = 0;
        self.observed_trade_fingerprint = TRADE_FINGERPRINT_OFFSET;
        self.observed_trade_tail = None;
        self.prices.clear();
        self.trade_quantities.consumed.clear();
        self.basket_dispersion = BasketPriceDispersionState::default();
        self.sector_dispersion = SectorPriceDispersionState::default();
        self.overflowed = false;
    }

    fn observe_new_consumed_goods(&mut self, consumed_goods: &[(AgentId, GoodId, u32)]) {
        for (agent, good, qty) in consumed_goods {
            if self
                .trade_quantities
                .add_consumed(*agent, *good, *qty)
                .is_none()
            {
                self.overflowed = true;
            }
        }
    }

    fn observe_new_receipts(&mut self, receipts: &[CantillonReceipt]) {
        let mut changed = false;
        if self.receipts_seen > receipts.len()
            || receipts[self.receipts_seen..].iter().any(|receipt| {
                is_new_money_receipt(receipt.source)
                    && matches!(self.first_receipt_tick, Some(tick) if receipt.tick.0 < tick.0)
            })
        {
            self.receipts_seen = 0;
            self.first_receipt_tick = None;
            self.early_receivers.clear();
            self.late_receivers.clear();
            self.cohorts_dirty = true;
            changed = true;
        }

        for receipt in &receipts[self.receipts_seen..] {
            if self.observe_receipt(receipt) {
                self.cohorts_dirty = true;
                changed = true;
            }
        }
        self.receipts_seen = receipts.len();
        if changed {
            normalize_receivers(&mut self.early_receivers, &mut self.late_receivers);
        }
    }

    fn observe_receipt(&mut self, receipt: &CantillonReceipt) -> bool {
        observe_cantillon_receipt(
            receipt,
            &mut self.first_receipt_tick,
            &mut self.early_receivers,
            &mut self.late_receivers,
        )
    }

    fn refresh_cohort_cache(&mut self, agents: &[Agent]) {
        let agents_changed = self.cohort_agent_ids.len() != agents.len()
            || self
                .cohort_agent_ids
                .iter()
                .zip(agents)
                .any(|(agent_id, agent)| *agent_id != agent.id);
        if !self.cohorts_dirty && !agents_changed {
            return;
        }

        self.cohort_agent_ids.clear();
        self.cohort_agent_ids
            .extend(agents.iter().map(|agent| agent.id));
        self.cohort_cache.clear();
        self.cohort_cache.extend(cohorts_for_agents(
            agents,
            &self.early_receivers,
            &self.late_receivers,
        ));
        self.cohorts_dirty = false;
    }

    fn refresh_agent_order_caches(&mut self, agents: &[Agent], initial_agents: &[Agent]) {
        refresh_agent_order_cache(
            agents,
            &mut self.agent_order_cache_ids,
            &mut self.agent_order_cache_indices,
        );
        refresh_agent_order_cache(
            initial_agents,
            &mut self.initial_order_cache_ids,
            &mut self.initial_order_cache_indices,
        );
    }

    fn cumulative_basket_price_dispersion_bps(&mut self) -> Option<u64> {
        if self.overflowed {
            return None;
        }
        checked_basket_price_dispersion_bps(
            self.basket_dispersion.dispersion_bps(),
            &mut self.overflowed,
        )
    }

    fn cumulative_sector_price_dispersion_bps(&mut self) -> Option<u64> {
        if self.overflowed {
            return None;
        }
        let dispersion = self.sector_dispersion.dispersion_bps();
        if dispersion.is_none() && self.sector_dispersion.has_multiple_sectors() {
            self.overflowed = true;
        }
        dispersion
    }
}

fn trade_fingerprint(trades: &[Trade]) -> u128 {
    trades
        .iter()
        .fold(TRADE_FINGERPRINT_OFFSET, |fingerprint, trade| {
            extend_trade_fingerprint(fingerprint, trade)
        })
}

fn extend_trade_fingerprint(mut fingerprint: u128, trade: &Trade) -> u128 {
    for part in [
        trade.tick,
        u64::from(trade.good.0),
        trade.buyer.0,
        trade.seller.0,
        trade.price.0,
        u64::from(trade.qty),
    ] {
        fingerprint ^= u128::from(part);
        fingerprint = fingerprint.wrapping_mul(TRADE_FINGERPRINT_MULTIPLIER);
    }
    fingerprint
}

impl BasketPriceDispersionState {
    fn observe_trade(&mut self, trade: &Trade) -> Option<()> {
        if trade.qty == 0 {
            return Some(());
        }
        let qty = u128::from(trade.qty);
        let value = u128::from(trade.price.0).checked_mul(qty)?;
        let stats = self.goods.entry(trade.good).or_default();
        stats.total_value = stats.total_value.checked_add(value)?;
        stats.total_qty = stats.total_qty.checked_add(qty)?;
        let price_qty = stats.qty_by_price.entry(trade.price).or_insert(0);
        *price_qty = price_qty.checked_add(qty)?;
        self.total_qty = self.total_qty.checked_add(qty)?;
        Some(())
    }

    fn dispersion_bps(&self) -> Option<u128> {
        if self.total_qty == 0 {
            return Some(0);
        }

        let mut weighted_bps = 0u128;
        for stats in self.goods.values() {
            if stats.total_qty == 0 {
                return None;
            }
            if stats.total_value == 0 {
                continue;
            }
            for (price, qty) in &stats.qty_by_price {
                let diff_bps =
                    scaled_relative_deviation_bps(*price, stats.total_qty, stats.total_value)?;
                weighted_bps = weighted_bps.checked_add(diff_bps.checked_mul(*qty)?)?;
            }
        }
        weighted_bps.checked_div(self.total_qty)
    }
}

impl SectorPriceDispersionState {
    fn observe_trade(&mut self, trade: &Trade, money_good: GoodId) -> Option<()> {
        if trade.good == money_good || trade.qty == 0 {
            return Some(());
        }
        let qty = u128::from(trade.qty);
        let value = u128::from(trade.price.0).checked_mul(qty)?;
        let entry = self.sectors.entry(sector_for(trade.good)).or_insert((0, 0));
        entry.0 = entry.0.checked_add(value)?;
        entry.1 = entry.1.checked_add(qty)?;
        Some(())
    }

    fn dispersion_bps(&self) -> Option<u64> {
        sector_dispersion_from_totals(&self.sectors)
    }

    fn has_multiple_sectors(&self) -> bool {
        self.sectors.len() >= 2
    }
}

fn observed_basket_price_dispersion_bps(trades: &[Trade], overflowed: &mut bool) -> Option<u64> {
    if *overflowed {
        return None;
    }
    checked_basket_price_dispersion_bps(basket_relative_price_dispersion(trades), overflowed)
}

fn checked_basket_price_dispersion_bps(
    dispersion: Option<u128>,
    overflowed: &mut bool,
) -> Option<u64> {
    match dispersion {
        Some(value) => match u64::try_from(value) {
            Ok(value) => Some(value),
            Err(_) => {
                *overflowed = true;
                None
            }
        },
        None => {
            *overflowed = true;
            None
        }
    }
}

fn observed_sector_price_dispersion_bps(
    trades: &[Trade],
    money_good: GoodId,
    overflowed: &mut bool,
) -> Option<u64> {
    if *overflowed {
        return None;
    }
    let mut state = SectorPriceDispersionState::default();
    for trade in trades {
        if state.observe_trade(trade, money_good).is_none() {
            *overflowed = true;
            return None;
        }
    }
    let dispersion = state.dispersion_bps();
    if dispersion.is_none() && state.has_multiple_sectors() {
        *overflowed = true;
    }
    dispersion
}

fn set_realized_price(prices: &mut Vec<(GoodId, Gold)>, good: GoodId, price: Gold) {
    match prices.binary_search_by_key(&good, |(entry, _)| *entry) {
        Ok(index) => prices[index].1 = price,
        Err(index) => prices.insert(index, (good, price)),
    }
}

fn normalize_receivers(early: &mut Vec<AgentId>, late: &mut Vec<AgentId>) {
    early.sort();
    early.dedup();
    late.sort();
    late.dedup();
    late.retain(|agent| early.binary_search(agent).is_err());
}

pub fn build_m4_records(society: &Society) -> Result<Vec<M4Record>, String> {
    if society.m3_records.len() != society.metric_observations.len() {
        return Err("M4 records require one metric observation per M3 record".to_string());
    }
    if !society.m3_shadow_attached() {
        return Err(
            "M4 records require run_m3_with_shadow so shadow fields are attached".to_string(),
        );
    }

    let mut records = Vec::with_capacity(society.m3_records.len());
    for index in 0..society.m3_records.len() {
        let m3 = &society.m3_records[index];
        let observation = &society.metric_observations[index];
        if m3.m2.tick != observation.tick {
            return Err("M4 records require aligned M3 and metric ticks".to_string());
        }
        if observation.arithmetic_overflowed {
            return Err("M4 metric observation overflowed".to_string());
        }
        records.push(build_m4_record(m3, observation)?);
    }
    Ok(records)
}

fn build_m4_record(m3: &M3Record, observation: &MetricObservation) -> Result<M4Record, String> {
    assert_eq!(m3.m2.tick, observation.tick);
    let wealth_values = observation
        .agent_wealth
        .iter()
        .map(|record| record.real_wealth)
        .collect::<Vec<_>>();
    let early_receiver_mean_real_wealth =
        cohort_mean_real_wealth(&observation.agent_wealth, CantillonCohort::EarlyReceiver)
            .ok_or_else(|| "M4 cohort mean overflowed".to_string())?;
    let late_receiver_mean_real_wealth =
        cohort_mean_real_wealth(&observation.agent_wealth, CantillonCohort::LateReceiver)
            .ok_or_else(|| "M4 cohort mean overflowed".to_string())?;
    let early_receiver_count =
        cohort_count(&observation.agent_wealth, CantillonCohort::EarlyReceiver)
            .ok_or_else(|| "M4 cohort count overflowed".to_string())?;
    let late_receiver_count =
        cohort_count(&observation.agent_wealth, CantillonCohort::LateReceiver)
            .ok_or_else(|| "M4 cohort count overflowed".to_string())?;
    Ok(M4Record {
        tick: m3.m2.tick,
        regime: m3.regime,
        tms: m3.tms,
        public_specie: m3.public_specie,
        public_fiat: m3.public_fiat,
        demand_claims: m3.demand_claims,
        fiduciary: m3.fiduciary,
        bank_credit_issued: m3.bank_credit_issued,
        fiat_credit_issued: m3.fiat_credit_issued,
        fiat_fiscal_issued: m3.fiat_fiscal_issued,
        credit_retired: m3.credit_retired,
        market_rate_bps: m3.m2.market_rate_bps,
        shadow_natural_rate_bps: m3.shadow_natural_rate_bps,
        shadow_rate_gap_bps: m3.shadow_rate_gap_bps,
        structure_length_ticks_x100: m3.m2.structure_length_ticks_x100,
        active_projects: m3.m2.active_projects,
        waiting_projects: m3.m2.waiting_projects,
        mature_projects: m3.m2.mature_projects,
        sold_projects: m3.m2.sold_projects,
        abandoned_projects: m3.m2.abandoned_projects,
        bust_abandoned_projects: m3.bust_abandoned_projects,
        capital_labor_consumed: m3.m2.capital_labor_consumed,
        capital_goods_consumed: m3.m2.capital_goods_consumed,
        debts_defaulted: m3.m2.debts_defaulted,
        project_debts_defaulted: m3.m2.project_debts_defaulted,
        agent_count: u32::try_from(observation.agent_wealth.len())
            .map_err(|_| "M4 agent count overflowed".to_string())?,
        early_receiver_count,
        late_receiver_count,
        non_receiver_count: cohort_count(&observation.agent_wealth, CantillonCohort::NonReceiver)
            .ok_or_else(|| "M4 cohort count overflowed".to_string())?,
        real_wealth_gini_bps: gini_bps(&wealth_values),
        lorenz_bottom_50_share_bps: lorenz_bottom_50_share_bps(&wealth_values),
        lorenz_top_10_share_bps: lorenz_top_10_share_bps(&wealth_values),
        early_receiver_mean_real_wealth,
        late_receiver_mean_real_wealth,
        non_receiver_mean_real_wealth: cohort_mean_real_wealth(
            &observation.agent_wealth,
            CantillonCohort::NonReceiver,
        )
        .ok_or_else(|| "M4 cohort mean overflowed".to_string())?,
        early_late_real_wealth_gap: early_receiver_mean_real_wealth
            .checked_sub(late_receiver_mean_real_wealth)
            .ok_or_else(|| "M4 cohort gap overflowed".to_string())?,
        early_receiver_mean_realized_delta: cohort_mean_realized_delta(
            &observation.agent_wealth,
            CantillonCohort::EarlyReceiver,
        )
        .ok_or_else(|| "M4 cohort mean overflowed".to_string())?,
        late_receiver_mean_realized_delta: cohort_mean_realized_delta(
            &observation.agent_wealth,
            CantillonCohort::LateReceiver,
        )
        .ok_or_else(|| "M4 cohort mean overflowed".to_string())?,
        tick_price_dispersion_bps: observation.tick_price_dispersion_bps,
        cumulative_price_dispersion_bps: observation.cumulative_price_dispersion_bps,
        tick_sector_price_dispersion_bps: observation.tick_sector_price_dispersion_bps,
        cumulative_sector_price_dispersion_bps: observation.cumulative_sector_price_dispersion_bps,
        unpriced_stock_units: observation.unpriced_stock_units,
        labor_capacity: observation.labor_capacity,
        labor_used: observation.labor_used,
        idle_labor_units: observation.idle_labor_units,
        idle_labor_bps: idle_labor_bps(observation.labor_capacity, observation.labor_used),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum M4Sector {
    Consumer,
    Capital,
    // impl-06.md: every good outside the consumer/capital map collapses into ONE "other"
    // sector, not a per-good sector — so a basket of unknown goods is a single sector.
    Other,
}

fn sector_for(good: GoodId) -> M4Sector {
    match good {
        FOOD => M4Sector::Consumer,
        WOOD | NET => M4Sector::Capital,
        _ => M4Sector::Other,
    }
}

fn observe_agent_wealth(
    input: &MetricObservationInput<'_>,
    agent_order: &[usize],
    initial_order: &[usize],
    builder: &mut AgentWealthBuilder<'_, '_>,
) {
    let mut initial_pos = 0usize;
    for agent_index in agent_order.iter().copied() {
        let agent = &input.agents[agent_index];
        while initial_pos < initial_order.len()
            && input.initial_agents[initial_order[initial_pos]].id < agent.id
        {
            initial_pos += 1;
        }
        if initial_pos == initial_order.len()
            || input.initial_agents[initial_order[initial_pos]].id != agent.id
        {
            continue;
        }
        push_agent_wealth(
            agent,
            &input.initial_agents[initial_order[initial_pos]],
            builder,
        );
    }
}

fn push_agent_wealth(agent: &Agent, initial: &Agent, builder: &mut AgentWealthBuilder<'_, '_>) {
    let Some((spendable_money, stock_value, real_wealth, unpriced)) = agent_current_real_wealth(
        agent,
        builder.money_system,
        builder.prices,
        builder.money_good,
        builder.stock_goods,
    ) else {
        *builder.overflowed = true;
        return;
    };
    let realized_delta = if builder.already_overflowed || *builder.overflowed {
        0
    } else {
        let delta_context = RealizedDeltaContext {
            money_system: builder.money_system,
            prices: builder.prices,
            money_good: builder.money_good,
            stock_goods: builder.stock_goods,
        };
        match agent_realized_delta_with_quantities(
            agent.id,
            agent,
            initial,
            &delta_context,
            builder.trade_quantities,
        ) {
            Some(value) => value,
            None => {
                *builder.overflowed = true;
                0
            }
        }
    };
    if let Some(total) = builder.unpriced_stock_units.checked_add(unpriced) {
        *builder.unpriced_stock_units = total;
    } else {
        *builder.overflowed = true;
        *builder.unpriced_stock_units = u32::MAX;
    }
    builder.agent_wealth.push(AgentWealthRecord {
        tick: builder.tick,
        agent: agent.id,
        cohort: cohort_for(builder.cohorts, agent.id),
        primary_role: agent.roles.first().copied(),
        spendable_money,
        stock_value,
        real_wealth,
        realized_delta,
        unpriced_stock_units: unpriced,
    });
}

fn price_for(prices: &[(GoodId, Gold)], good: GoodId) -> Option<Gold> {
    prices
        .binary_search_by_key(&good, |(entry, _)| *entry)
        .ok()
        .map(|index| prices[index].1)
}

fn realized_position_value(
    agent: &Agent,
    money_system: Option<&MoneySystem>,
    prices: &[(GoodId, Gold)],
    money_good: GoodId,
    stock_goods: &[GoodId],
) -> Option<i128> {
    let (_, _, real_wealth, _) =
        agent_current_real_wealth(agent, money_system, prices, money_good, stock_goods)?;
    Some(real_wealth)
}

fn realized_consumed_value(
    agent: AgentId,
    prices: &[(GoodId, Gold)],
    trade_quantities: &RealizedTradeQuantities,
) -> Option<i128> {
    let mut value = 0i128;
    for (good, consumed) in trade_quantities.consumed_for_agent(agent) {
        let Some(price) = price_for(prices, good) else {
            continue;
        };
        let good_value = consumed.checked_mul(u128::from(price.0))?;
        value = value.checked_add(i128::try_from(good_value).ok()?)?;
    }
    Some(value)
}

impl RealizedTradeQuantities {
    fn add_consumed(&mut self, agent: AgentId, good: GoodId, qty: u32) -> Option<()> {
        let total = self.consumed.entry((agent, good)).or_insert(0);
        *total = total.checked_add(u128::from(qty))?;
        Some(())
    }

    fn consumed_for_agent(&self, agent: AgentId) -> impl Iterator<Item = (GoodId, u128)> + '_ {
        let first_key = (agent, GoodId(0));
        let last_key = (agent, GoodId(u16::MAX));
        self.consumed
            .range(first_key..=last_key)
            .map(|((_, good), qty)| (*good, *qty))
    }
}

fn is_new_money_receipt(source: CreditSource) -> bool {
    matches!(
        source,
        CreditSource::FiatFiscal(_) | CreditSource::BankFiduciary(_) | CreditSource::FiatCredit(_)
    )
}

fn observe_cantillon_receipt(
    receipt: &CantillonReceipt,
    first_receipt_tick: &mut Option<Tick>,
    early_receivers: &mut Vec<AgentId>,
    late_receivers: &mut Vec<AgentId>,
) -> bool {
    if !is_new_money_receipt(receipt.source) {
        return false;
    }
    match *first_receipt_tick {
        None => {
            *first_receipt_tick = Some(receipt.tick);
            early_receivers.push(receipt.agent);
        }
        Some(first_tick) if receipt.tick.0 < first_tick.0 => {
            late_receivers.append(early_receivers);
            *first_receipt_tick = Some(receipt.tick);
            early_receivers.push(receipt.agent);
        }
        Some(first_tick) if receipt.tick == first_tick => {
            early_receivers.push(receipt.agent);
        }
        Some(_) => late_receivers.push(receipt.agent),
    }
    true
}

fn cohorts_for_agents(
    agents: &[Agent],
    early_receivers: &[AgentId],
    late_receivers: &[AgentId],
) -> Vec<(AgentId, CantillonCohort)> {
    let mut cohorts = agents
        .iter()
        .map(|agent| {
            (
                agent.id,
                cohort_from_receivers(early_receivers, late_receivers, agent.id),
            )
        })
        .collect::<Vec<_>>();
    if !agent_ids_are_sorted(agents) {
        cohorts.sort_by_key(|(agent, _)| *agent);
    }
    cohorts
}

fn agent_ids_are_sorted(agents: &[Agent]) -> bool {
    agents.windows(2).all(|window| window[0].id <= window[1].id)
}

fn refresh_agent_order_cache(
    agents: &[Agent],
    cached_ids: &mut Vec<AgentId>,
    cached_indices: &mut Vec<usize>,
) {
    let agents_changed = cached_ids.len() != agents.len()
        || cached_ids
            .iter()
            .zip(agents)
            .any(|(agent_id, agent)| *agent_id != agent.id);
    if !agents_changed {
        return;
    }

    cached_ids.clear();
    cached_ids.extend(agents.iter().map(|agent| agent.id));
    cached_indices.clear();
    cached_indices.extend(0..agents.len());
    if !agent_ids_are_sorted(agents) {
        cached_indices.sort_by_key(|index| agents[*index].id);
    }
}

fn cohort_from_receivers(
    early_receivers: &[AgentId],
    late_receivers: &[AgentId],
    agent: AgentId,
) -> CantillonCohort {
    if early_receivers.binary_search(&agent).is_ok() {
        CantillonCohort::EarlyReceiver
    } else if late_receivers.binary_search(&agent).is_ok() {
        CantillonCohort::LateReceiver
    } else {
        CantillonCohort::NonReceiver
    }
}

fn cohort_for(cohorts: &[(AgentId, CantillonCohort)], agent: AgentId) -> CantillonCohort {
    cohorts
        .binary_search_by_key(&agent, |(entry, _)| *entry)
        .ok()
        .map(|index| cohorts[index].1)
        .unwrap_or(CantillonCohort::NonReceiver)
}

fn non_negative_values(values: &[i128]) -> Option<Vec<u128>> {
    values
        .iter()
        .map(|value| u128::try_from(*value).ok())
        .collect()
}

fn share_bps(group_sum: u128, total: u128) -> Option<u32> {
    let bps = mul_div_floor(group_sum, BPS_SCALE, total)?;
    u32::try_from(bps).ok()
}

fn cohort_count(records: &[AgentWealthRecord], cohort: CantillonCohort) -> Option<u32> {
    records
        .iter()
        .filter(|record| record.cohort == cohort)
        .count()
        .try_into()
        .ok()
}

fn cohort_mean_real_wealth(records: &[AgentWealthRecord], cohort: CantillonCohort) -> Option<i128> {
    let count = cohort_count_usize(records, cohort);
    mean_i128_counted(
        records
            .iter()
            .filter(|record| record.cohort == cohort)
            .map(|record| record.real_wealth),
        count,
    )
}

fn cohort_mean_realized_delta(
    records: &[AgentWealthRecord],
    cohort: CantillonCohort,
) -> Option<i128> {
    let count = cohort_count_usize(records, cohort);
    mean_i128_counted(
        records
            .iter()
            .filter(|record| record.cohort == cohort)
            .map(|record| record.realized_delta),
        count,
    )
}

fn cohort_count_usize(records: &[AgentWealthRecord], cohort: CantillonCohort) -> usize {
    records
        .iter()
        .filter(|record| record.cohort == cohort)
        .count()
}

fn mean_i128_counted(values: impl IntoIterator<Item = i128>, count: usize) -> Option<i128> {
    let count = i128::try_from(count).ok()?;
    if count == 0 {
        return Some(0);
    }
    let divisor = u128::try_from(count).ok()?;
    let mut quotient_sum = 0i128;
    let mut remainder_sum = 0u128;
    // Split each term into quotient + Euclidean remainder before summing, so
    // cohorts with extreme but valid i128 values do not overflow an aggregate
    // sum merely to divide it back down by the same count.
    for value in values.into_iter() {
        quotient_sum = quotient_sum.checked_add(value.div_euclid(count))?;
        let remainder = u128::try_from(value.rem_euclid(count)).ok()?;
        let (next_remainder, carry) = add_remainders(remainder_sum, remainder, divisor);
        remainder_sum = next_remainder;
        quotient_sum = quotient_sum.checked_add(i128::try_from(carry).ok()?)?;
    }
    if quotient_sum < 0 && remainder_sum > 0 {
        quotient_sum.checked_add(1)
    } else {
        Some(quotient_sum)
    }
}

fn scaled_relative_deviation_bps(
    price: Gold,
    baseline_qty: u128,
    baseline_value: u128,
) -> Option<u128> {
    if baseline_value == 0 {
        return None;
    }
    let realized_value_at_baseline_qty = u128::from(price.0).checked_mul(baseline_qty)?;
    let deviation = realized_value_at_baseline_qty.abs_diff(baseline_value);
    mul_div_floor(deviation, BPS_SCALE, baseline_value)
}

fn signed_gold_delta(previous: Gold, next: Gold) -> i128 {
    i128::from(next.0) - i128::from(previous.0)
}

#[cfg(test)]
fn integer_population_variance(values: &[i128]) -> Option<u128> {
    let n = u128::try_from(values.len()).ok()?;
    let sum = values
        .iter()
        .try_fold(0i128, |sum, value| sum.checked_add(*value))?;
    integer_population_variance_from_parts(values.iter().copied(), n, sum)
}

fn integer_population_variance_from_parts(
    values: impl IntoIterator<Item = i128>,
    n: u128,
    sum: i128,
) -> Option<u128> {
    if n < 2 {
        return Some(0);
    }
    let (center, residual) = signed_div_euclid_by_u128(sum, n)?;

    // Compute E[(x - floor(mean))^2] - fractional_mean^2 without forming
    // distance^2 * n or sum^2. `square_div_rem` keeps each term split around
    // the final divisor, which is the only reason this accepts near-u128 inputs.
    let mut mean_square = 0u128;
    let mut remainder_sum = 0u128;
    for value in values {
        let distance = value.checked_sub(center)?.unsigned_abs();
        let (quotient, remainder) = square_div_rem(distance, n)?;
        mean_square = mean_square.checked_add(quotient)?;
        remainder_sum = remainder_sum.checked_add(remainder)?;
        if remainder_sum >= n {
            let carry = remainder_sum / n;
            mean_square = mean_square.checked_add(carry)?;
            remainder_sum %= n;
        }
    }

    let (residual_square_quotient, residual_square_remainder) = square_div_rem(residual, n)?;
    let fractional_remainder_covers_square = remainder_sum > residual_square_quotient
        || (remainder_sum == residual_square_quotient && residual_square_remainder == 0);
    Some(if fractional_remainder_covers_square {
        mean_square
    } else {
        mean_square.saturating_sub(1)
    })
}

fn signed_div_euclid_by_u128(value: i128, denominator: u128) -> Option<(i128, u128)> {
    if denominator == 0 {
        return None;
    }
    let magnitude = value.unsigned_abs();
    if value >= 0 {
        let quotient = magnitude / denominator;
        let remainder = magnitude % denominator;
        return Some((i128::try_from(quotient).ok()?, remainder));
    }

    let quotient_magnitude = magnitude.div_ceil(denominator);
    let quotient = negative_u128_to_i128(quotient_magnitude)?;
    let remainder = quotient_magnitude
        .checked_mul(denominator)?
        .checked_sub(magnitude)?;
    Some((quotient, remainder))
}

fn negative_u128_to_i128(magnitude: u128) -> Option<i128> {
    const I128_MIN_MAGNITUDE: u128 = 1u128 << 127;

    if magnitude == I128_MIN_MAGNITUDE {
        Some(i128::MIN)
    } else {
        i128::try_from(magnitude).ok()?.checked_neg()
    }
}

fn square_div_rem(value: u128, denominator: u128) -> Option<(u128, u128)> {
    mul_div_rem(value, value, denominator)
}

fn mul_div_floor(multiplicand: u128, multiplier: u128, denominator: u128) -> Option<u128> {
    mul_div_rem(multiplicand, multiplier, denominator).map(|(quotient, _)| quotient)
}

fn mul_div_rem(
    multiplicand: u128,
    mut multiplier: u128,
    denominator: u128,
) -> Option<(u128, u128)> {
    if denominator == 0 {
        return None;
    }

    let mut quotient = 0u128;
    let mut remainder = 0u128;
    let mut term_quotient = multiplicand / denominator;
    let mut term_remainder = multiplicand % denominator;

    // Binary long multiplication over quotient/remainder pairs computes
    // floor(multiplicand * multiplier / denominator) without ever materializing
    // the potentially overflowing product.
    while multiplier > 0 {
        if multiplier & 1 == 1 {
            (quotient, remainder) = add_quotient_remainder(
                quotient,
                remainder,
                term_quotient,
                term_remainder,
                denominator,
            )?;
        }

        multiplier >>= 1;
        if multiplier == 0 {
            break;
        }
        (term_quotient, term_remainder) =
            double_quotient_remainder(term_quotient, term_remainder, denominator)?;
    }

    Some((quotient, remainder))
}

fn double_quotient_remainder(
    quotient: u128,
    remainder: u128,
    denominator: u128,
) -> Option<(u128, u128)> {
    let quotient = quotient.checked_mul(2)?;
    let (remainder, carry) = add_remainders(remainder, remainder, denominator);
    Some((quotient.checked_add(carry)?, remainder))
}

fn add_quotient_remainder(
    quotient: u128,
    remainder: u128,
    add_quotient: u128,
    add_remainder: u128,
    denominator: u128,
) -> Option<(u128, u128)> {
    let quotient = quotient.checked_add(add_quotient)?;
    let (remainder, carry) = add_remainders(remainder, add_remainder, denominator);
    Some((quotient.checked_add(carry)?, remainder))
}

fn add_remainders(left: u128, right: u128, denominator: u128) -> (u128, u128) {
    debug_assert!(denominator > 0);
    debug_assert!(left < denominator);
    debug_assert!(right < denominator);
    let wrap_threshold = denominator - right;
    if left >= wrap_threshold {
        (left - wrap_threshold, 1)
    } else {
        (left + right, 0)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        aggregate_project_forecast_error, basket_relative_price_dispersion, build_m4_records,
        build_metric_observation, cantillon_cohorts, gini_bps, idle_labor_bps,
        integer_population_variance, lorenz_bottom_50_share_bps, lorenz_top_10_share_bps,
        realized_price_table, scaled_relative_deviation_bps, sector_price_dispersion_bps,
        structure_length_ticks_x100, BasketPriceDispersionState, GoodPriceStats,
        MetricObservationAccumulator, MetricObservationInput,
    };
    use crate::agent::{Agent, AgentId, Role};
    use crate::cantillon::CantillonReceipt;
    use crate::capital::{
        build_fish_trap_line, start_project, M2ProjectId, M2ProjectState, ProjectOutputLot,
    };
    use crate::expect::PriceBelief;
    use crate::good::{Gold, GoodId, Stock, FOOD, GOLD, NET, WOOD};
    use crate::ledger::{BankId, IssuerId};
    use crate::market::Trade;
    use crate::project::Tick;
    use crate::purpose::CreditSource;
    use crate::record::{CantillonCohort, M2Record, M3Record, MetricObservation};
    use crate::scenario::{builtin_market_scenario, ScenarioName};
    use crate::shadow::ShadowSeries;
    use crate::society::{run_m3_with_shadow, run_m3_with_shadow_without_metrics};

    fn metric_agent(id: u32) -> Agent {
        Agent {
            id: AgentId(u64::from(id)),
            scale: Vec::new(),
            stock: Stock::new(3),
            gold: Gold::ZERO,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Household],
            expect: vec![PriceBelief::new(Gold(1), Gold(1)); 4],
        }
    }

    #[test]
    fn structure_length_is_derived_from_projects() {
        let line = build_fish_trap_line();
        let mut project = start_project(
            AgentId(1),
            &line,
            M2ProjectId(1),
            Tick(0),
            Gold(9),
            Gold::ZERO,
        );
        project.advanced_gold = Gold(3);
        project.labor_advanced = 1;

        assert_eq!(structure_length_ticks_x100(&[project], Tick(1)), 300);
    }

    #[test]
    fn variance_handles_large_practical_common_offset() {
        let high = i128::from(u64::MAX);
        assert_eq!(integer_population_variance(&[high - 2, high]), Some(1));
    }

    #[test]
    fn variance_scales_before_accumulating_large_squares() {
        let high = i128::from(u64::MAX);
        let values = (0..256)
            .map(|index| if index % 2 == 0 { 0 } else { high })
            .collect::<Vec<_>>();
        let high_u128 = u128::from(u64::MAX);
        let expected = high_u128
            .checked_mul(high_u128)
            .and_then(|value| value.checked_div(4))
            .expect("u64 square fits in u128");

        assert_eq!(integer_population_variance(&values), Some(expected));
    }

    #[test]
    fn variance_handles_large_total_displacement_from_center() {
        let high = u64::MAX / 32;
        let mut values = vec![i128::from(high); 256];
        values.push(0);
        let high_u128 = u128::from(high);
        let expected = high_u128
            .checked_mul(high_u128)
            .and_then(|value| value.checked_mul(256))
            .and_then(|value| value.checked_div(257 * 257))
            .expect("scaled u64 square fits in u128");

        assert_eq!(integer_population_variance(&values), Some(expected));
    }

    #[test]
    fn variance_handles_large_symmetric_deltas() {
        let high = i128::from(u64::MAX);
        let expected = u128::from(u64::MAX)
            .checked_mul(u128::from(u64::MAX))
            .expect("u64 square fits in u128");

        assert_eq!(integer_population_variance(&[-high, high]), Some(expected));
    }

    #[test]
    fn variance_handles_odd_valid_gold_swings() {
        let high = i128::from(u64::MAX);
        let high_u128 = u128::from(u64::MAX);
        let square = high_u128
            .checked_mul(high_u128)
            .expect("u64 square fits in u128");
        let expected = (square / 9)
            .checked_mul(8)
            .and_then(|value| value.checked_add((square % 9) * 8 / 9))
            .expect("scaled u64 square fits in u128");

        assert_eq!(
            integer_population_variance(&[-high, high, high]),
            Some(expected)
        );
    }

    #[test]
    fn variance_accepts_record_counts_above_i128_max() {
        let huge_count = (i128::MAX as u128) + 1;

        assert_eq!(
            super::signed_div_euclid_by_u128(-1, huge_count),
            Some((-1, huge_count - 1))
        );
        assert_eq!(
            super::integer_population_variance_from_parts([0, 0], huge_count, 0),
            Some(0)
        );
        assert_eq!(
            super::integer_population_variance_from_parts([0], huge_count, -1),
            Some(0)
        );
    }

    #[test]
    fn variance_reports_impractical_intermediate_overflow() {
        assert_eq!(integer_population_variance(&[i128::MIN, i128::MAX]), None);
    }

    #[test]
    fn forecast_error_aggregates_finished_projects_only() {
        let line = build_fish_trap_line();
        let mut sold = start_project(
            AgentId(1),
            &line,
            M2ProjectId(1),
            Tick(0),
            Gold(20),
            Gold::ZERO,
        );
        sold.state = M2ProjectState::Sold;
        let mut abandoned = start_project(
            AgentId(1),
            &line,
            M2ProjectId(2),
            Tick(0),
            Gold(7),
            Gold::ZERO,
        );
        abandoned.state = M2ProjectState::Abandoned;
        let waiting = start_project(
            AgentId(1),
            &line,
            M2ProjectId(3),
            Tick(0),
            Gold(50),
            Gold::ZERO,
        );
        let lots = [
            ProjectOutputLot {
                project: M2ProjectId(1),
                owner: AgentId(1),
                good: FOOD,
                qty_remaining: 0,
                proceeds: Gold(12),
            },
            ProjectOutputLot {
                project: M2ProjectId(1),
                owner: AgentId(1),
                good: FOOD,
                qty_remaining: 0,
                proceeds: Gold(5),
            },
        ];

        assert_eq!(
            aggregate_project_forecast_error(&[sold, abandoned, waiting], &lots),
            Some(10)
        );
    }

    #[test]
    fn basket_dispersion_uses_realized_average_price_baseline() {
        let trades = [
            Trade {
                tick: 0,
                good: FOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(10),
                qty: 1,
            },
            Trade {
                tick: 1,
                good: FOOD,
                buyer: AgentId(2),
                seller: AgentId(1),
                price: Gold(30),
                qty: 1,
            },
        ];

        assert_eq!(basket_relative_price_dispersion(&trades), Some(5_000));
    }

    #[test]
    fn basket_dispersion_keeps_fractional_average_baseline() {
        let trades = [
            Trade {
                tick: 0,
                good: FOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(1),
                qty: 1,
            },
            Trade {
                tick: 1,
                good: FOOD,
                buyer: AgentId(2),
                seller: AgentId(1),
                price: Gold(2),
                qty: 1,
            },
        ];

        assert_eq!(basket_relative_price_dispersion(&trades), Some(3_333));
    }

    #[test]
    fn basket_dispersion_handles_zero_price_baseline() {
        let trades = [
            Trade {
                tick: 0,
                good: FOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold::ZERO,
                qty: 1,
            },
            Trade {
                tick: 1,
                good: FOOD,
                buyer: AgentId(2),
                seller: AgentId(1),
                price: Gold::ZERO,
                qty: 1,
            },
        ];

        assert_eq!(basket_relative_price_dispersion(&trades), Some(0));
    }

    #[test]
    fn basket_dispersion_counts_zero_price_goods_in_basket_weight() {
        let trades = [
            Trade {
                tick: 0,
                good: FOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(10),
                qty: 1,
            },
            Trade {
                tick: 1,
                good: FOOD,
                buyer: AgentId(2),
                seller: AgentId(1),
                price: Gold(30),
                qty: 1,
            },
            Trade {
                tick: 0,
                good: WOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold::ZERO,
                qty: 3,
            },
            Trade {
                tick: 1,
                good: WOOD,
                buyer: AgentId(2),
                seller: AgentId(1),
                price: Gold::ZERO,
                qty: 3,
            },
        ];

        assert_eq!(basket_relative_price_dispersion(&trades), Some(1_250));
    }

    #[test]
    fn realized_price_table_keeps_last_price_through_tick() {
        let trades = [
            Trade {
                tick: 0,
                good: FOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(10),
                qty: 1,
            },
            Trade {
                tick: 1,
                good: FOOD,
                buyer: AgentId(2),
                seller: AgentId(1),
                price: Gold(20),
                qty: 1,
            },
            Trade {
                tick: 2,
                good: WOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(30),
                qty: 0,
            },
        ];

        assert_eq!(
            realized_price_table(&trades, Some(0)),
            vec![(FOOD, Gold(10))]
        );
        assert_eq!(
            realized_price_table(&trades, Some(1)),
            vec![(FOOD, Gold(20))]
        );
    }

    #[test]
    fn basket_dispersion_preserves_positive_subunit_baseline() {
        let trades = [
            Trade {
                tick: 0,
                good: FOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(1),
                qty: 1,
            },
            Trade {
                tick: 0,
                good: FOOD,
                buyer: AgentId(2),
                seller: AgentId(1),
                price: Gold::ZERO,
                qty: 20_000,
            },
        ];

        assert_eq!(basket_relative_price_dispersion(&trades), Some(19_999));
    }

    #[test]
    fn relative_deviation_avoids_scaled_trade_value_overflow() {
        let baseline_qty = u128::from(u64::MAX) + 1;
        let baseline_value = u128::from(u64::MAX);
        let expected = u128::from(u64::MAX)
            .checked_mul(10_000)
            .expect("scaled u64 max fits in u128");

        assert_eq!(
            scaled_relative_deviation_bps(Gold(u64::MAX), baseline_qty, baseline_value),
            Some(expected)
        );
    }

    #[test]
    fn gini_is_zero_for_equal_real_wealth() {
        assert_eq!(gini_bps(&[7, 7, 7, 7]), Some(0));
    }

    #[test]
    fn gini_handles_extreme_values_without_scaled_overflow() {
        assert_eq!(gini_bps(&[i128::MAX, i128::MAX]), Some(0));
        assert_eq!(gini_bps(&[0, i128::MAX]), Some(5_000));
        assert_eq!(gini_bps(&[0, i128::MAX, i128::MAX]), Some(3_333));
    }

    #[test]
    fn distribution_metrics_return_none_for_negative_wealth() {
        assert_eq!(gini_bps(&[-1, 1]), None);
        assert_eq!(lorenz_bottom_50_share_bps(&[-1, 1]), None);
        assert_eq!(lorenz_top_10_share_bps(&[-1, 1]), None);
    }

    #[test]
    fn gini_rises_under_fiat_fiscal_cantillon() {
        let society = run_m3_with_shadow(builtin_market_scenario(ScenarioName::CantillonIsolation));
        let records = build_m4_records(&society).expect("cantillon isolation builds M4 records");
        let first = records
            .first()
            .and_then(|record| record.real_wealth_gini_bps)
            .expect("cantillon isolation emits M4 records");
        let peak = records
            .iter()
            .filter_map(|record| record.real_wealth_gini_bps)
            .max()
            .expect("cantillon isolation has gini observations");

        assert!(peak > first);
    }

    #[test]
    fn m4_builder_reports_missing_observations() {
        let mut scenario = builtin_market_scenario(ScenarioName::FiatCreditExpansion);
        scenario.periods = 1;
        let society = run_m3_with_shadow_without_metrics(scenario);

        assert!(build_m4_records(&society).is_err());
    }

    #[test]
    fn m4_builder_reports_overflowed_metric_observation() {
        let mut society = crate::society::Society::from_scenario(builtin_market_scenario(
            ScenarioName::FiatCreditExpansion,
        ));
        society.m3_records.push(M3Record {
            m2: M2Record {
                tick: 0,
                natural_rate_proxy_bps: Some(0),
                ..M2Record::default()
            },
            shadow_natural_rate_bps: Some(0),
            ..M3Record::default()
        });
        society.metric_observations.push(MetricObservation {
            tick: 0,
            arithmetic_overflowed: true,
            ..MetricObservation::default()
        });
        society.attach_shadow(&ShadowSeries {
            natural_rate_bps: vec![Some(0)],
            structure_length_ticks_x100: vec![0],
        });

        assert_eq!(
            build_m4_records(&society),
            Err("M4 metric observation overflowed".to_string())
        );
    }

    #[test]
    fn m4_builder_reports_unattached_shadow() {
        let mut society = crate::society::Society::from_scenario(builtin_market_scenario(
            ScenarioName::FiatCreditExpansion,
        ));
        society.m3_records.push(M3Record {
            m2: M2Record {
                tick: 0,
                ..M2Record::default()
            },
            ..M3Record::default()
        });
        society.metric_observations.push(MetricObservation {
            tick: 0,
            ..MetricObservation::default()
        });

        assert_eq!(
            build_m4_records(&society),
            Err("M4 records require run_m3_with_shadow so shadow fields are attached".to_string())
        );
    }

    #[test]
    fn cumulative_price_dispersion_overflow_marks_metric_observation() {
        let agents = vec![metric_agent(1)];
        let mut qty_by_price = BTreeMap::new();
        qty_by_price.insert(Gold(u64::MAX), 1);
        let mut goods = BTreeMap::new();
        goods.insert(
            FOOD,
            GoodPriceStats {
                total_value: 1,
                total_qty: u128::MAX,
                qty_by_price,
            },
        );
        let mut accumulator = MetricObservationAccumulator {
            basket_dispersion: BasketPriceDispersionState {
                goods,
                total_qty: 1,
            },
            ..MetricObservationAccumulator::default()
        };

        let observation = accumulator.observe(MetricObservationInput {
            tick: 0,
            agents: &agents,
            initial_agents: &agents,
            money_system: None,
            receipts: &[],
            trades: &[],
            tick_trades: &[],
            consumed_goods: &[],
            money_good: GOLD,
            stock_goods: &[FOOD],
            labor_capacity: 0,
            labor_used: 0,
        });

        assert_eq!(observation.cumulative_price_dispersion_bps, None);
        assert!(observation.arithmetic_overflowed);
    }

    #[test]
    fn lorenz_points_are_stable_for_known_distribution() {
        let values = [0, 10, 30, 60];

        assert_eq!(lorenz_bottom_50_share_bps(&values), Some(1_000));
        assert_eq!(lorenz_top_10_share_bps(&values), Some(6_000));
    }

    #[test]
    fn lorenz_handles_extreme_values_without_scaled_overflow() {
        let values = [i128::MAX, i128::MAX];

        assert_eq!(lorenz_bottom_50_share_bps(&values), Some(5_000));
        assert_eq!(lorenz_top_10_share_bps(&values), Some(5_000));
    }

    #[test]
    fn cohort_mean_handles_extreme_i128_values_without_panicking() {
        assert_eq!(
            super::mean_i128_counted([i128::MAX, i128::MAX], 2),
            Some(i128::MAX)
        );
        assert_eq!(
            super::mean_i128_counted([i128::MIN, i128::MIN], 2),
            Some(i128::MIN)
        );
        assert_eq!(super::mean_i128_counted([i128::MIN, i128::MAX], 2), Some(0));
    }

    #[test]
    fn credit_channel_receipts_feed_m4_cohort_metrics() {
        let agents = vec![metric_agent(1), metric_agent(2), metric_agent(3)];
        let receipts = vec![
            CantillonReceipt {
                tick: Tick(0),
                agent: AgentId(3),
                amount: Gold(9),
                source: CreditSource::Commodity,
            },
            CantillonReceipt {
                tick: Tick(0),
                agent: AgentId(1),
                amount: Gold(1),
                source: CreditSource::BankFiduciary(BankId(1)),
            },
            CantillonReceipt {
                tick: Tick(1),
                agent: AgentId(2),
                amount: Gold(1),
                source: CreditSource::FiatCredit(IssuerId(1)),
            },
        ];

        let cohorts = cantillon_cohorts(&receipts, &agents);

        assert_eq!(cohorts[0], (AgentId(1), CantillonCohort::EarlyReceiver));
        assert_eq!(cohorts[1], (AgentId(2), CantillonCohort::LateReceiver));
        assert_eq!(cohorts[2], (AgentId(3), CantillonCohort::NonReceiver));
    }

    #[test]
    fn cantillon_cohorts_use_earliest_new_money_tick_when_unordered() {
        let agents = vec![
            metric_agent(1),
            metric_agent(2),
            metric_agent(3),
            metric_agent(4),
        ];
        let receipts = vec![
            CantillonReceipt {
                tick: Tick(5),
                agent: AgentId(1),
                amount: Gold(1),
                source: CreditSource::BankFiduciary(BankId(1)),
            },
            CantillonReceipt {
                tick: Tick(3),
                agent: AgentId(2),
                amount: Gold(1),
                source: CreditSource::FiatFiscal(IssuerId(1)),
            },
            CantillonReceipt {
                tick: Tick(1),
                agent: AgentId(3),
                amount: Gold(9),
                source: CreditSource::Commodity,
            },
            CantillonReceipt {
                tick: Tick(7),
                agent: AgentId(4),
                amount: Gold(1),
                source: CreditSource::FiatCredit(IssuerId(1)),
            },
        ];

        let cohorts = cantillon_cohorts(&receipts, &agents);

        assert_eq!(cohorts[0], (AgentId(1), CantillonCohort::LateReceiver));
        assert_eq!(cohorts[1], (AgentId(2), CantillonCohort::EarlyReceiver));
        assert_eq!(cohorts[2], (AgentId(3), CantillonCohort::NonReceiver));
        assert_eq!(cohorts[3], (AgentId(4), CantillonCohort::LateReceiver));
    }

    #[test]
    fn incremental_metric_observation_matches_stateless_observation() {
        let initial_agents = vec![metric_agent(1), metric_agent(2)];
        let mut agents = initial_agents.clone();
        agents[0].stock.add(FOOD, 1);
        agents[1].stock.add(WOOD, 2);
        let trades = vec![
            Trade {
                tick: 0,
                good: FOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(10),
                qty: 1,
            },
            Trade {
                tick: 1,
                good: WOOD,
                buyer: AgentId(2),
                seller: AgentId(1),
                price: Gold(20),
                qty: 2,
            },
        ];
        let receipts = vec![
            CantillonReceipt {
                tick: Tick(0),
                agent: AgentId(1),
                amount: Gold(1),
                source: CreditSource::FiatFiscal(IssuerId(1)),
            },
            CantillonReceipt {
                tick: Tick(1),
                agent: AgentId(2),
                amount: Gold(1),
                source: CreditSource::BankFiduciary(BankId(1)),
            },
        ];
        let mut accumulator = MetricObservationAccumulator::default();

        let first = accumulator.observe(MetricObservationInput {
            tick: 0,
            agents: &agents,
            initial_agents: &initial_agents,
            money_system: None,
            receipts: &receipts[..1],
            trades: &trades[..1],
            tick_trades: &trades[..1],
            consumed_goods: &[],
            money_good: GOLD,
            stock_goods: &[FOOD, WOOD],
            labor_capacity: 10,
            labor_used: 4,
        });
        let first_stateless = build_metric_observation(MetricObservationInput {
            tick: 0,
            agents: &agents,
            initial_agents: &initial_agents,
            money_system: None,
            receipts: &receipts[..1],
            trades: &trades[..1],
            tick_trades: &trades[..1],
            consumed_goods: &[],
            money_good: GOLD,
            stock_goods: &[FOOD, WOOD],
            labor_capacity: 10,
            labor_used: 4,
        });

        let second = accumulator.observe(MetricObservationInput {
            tick: 1,
            agents: &agents,
            initial_agents: &initial_agents,
            money_system: None,
            receipts: &receipts,
            trades: &trades,
            tick_trades: &trades[1..],
            consumed_goods: &[],
            money_good: GOLD,
            stock_goods: &[FOOD, WOOD],
            labor_capacity: 10,
            labor_used: 8,
        });
        let second_stateless = build_metric_observation(MetricObservationInput {
            tick: 1,
            agents: &agents,
            initial_agents: &initial_agents,
            money_system: None,
            receipts: &receipts,
            trades: &trades,
            tick_trades: &trades[1..],
            consumed_goods: &[],
            money_good: GOLD,
            stock_goods: &[FOOD, WOOD],
            labor_capacity: 10,
            labor_used: 8,
        });

        assert_eq!(first, first_stateless);
        assert_eq!(second, second_stateless);
    }

    #[test]
    fn metric_accumulator_rebuilds_when_trade_slice_is_replaced_at_same_len() {
        let initial_agents = vec![metric_agent(1), metric_agent(2)];
        let mut agents = initial_agents.clone();
        agents[0].stock.add(FOOD, 1);
        let low_price = [Trade {
            tick: 0,
            good: FOOD,
            buyer: AgentId(1),
            seller: AgentId(2),
            price: Gold(10),
            qty: 1,
        }];
        let high_price = [Trade {
            price: Gold(20),
            ..low_price[0].clone()
        }];
        let mut accumulator = MetricObservationAccumulator::default();

        let first = accumulator.observe(MetricObservationInput {
            tick: 0,
            agents: &agents,
            initial_agents: &initial_agents,
            money_system: None,
            receipts: &[],
            trades: &low_price,
            tick_trades: &low_price,
            consumed_goods: &[],
            money_good: GOLD,
            stock_goods: &[FOOD],
            labor_capacity: 0,
            labor_used: 0,
        });
        let second = accumulator.observe(MetricObservationInput {
            tick: 0,
            agents: &agents,
            initial_agents: &initial_agents,
            money_system: None,
            receipts: &[],
            trades: &high_price,
            tick_trades: &high_price,
            consumed_goods: &[],
            money_good: GOLD,
            stock_goods: &[FOOD],
            labor_capacity: 0,
            labor_used: 0,
        });

        assert_eq!(first.agent_wealth[0].stock_value, 10);
        assert_eq!(second.agent_wealth[0].stock_value, 20);
    }

    #[test]
    fn metric_accumulator_rebuilds_when_trade_prefix_middle_changes() {
        let initial_agents = vec![metric_agent(1), metric_agent(2)];
        let mut agents = initial_agents.clone();
        agents[0].stock.add(FOOD, 1);
        let mut original = vec![
            Trade {
                tick: 0,
                good: WOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(10),
                qty: 1,
            },
            Trade {
                tick: 0,
                good: FOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(10),
                qty: 1,
            },
        ];
        let mut changed = original.clone();
        changed[0].price = Gold(20);
        let mut accumulator = MetricObservationAccumulator::default();

        let first = accumulator.observe(MetricObservationInput {
            tick: 0,
            agents: &agents,
            initial_agents: &initial_agents,
            money_system: None,
            receipts: &[],
            trades: &original,
            tick_trades: &original,
            consumed_goods: &[],
            money_good: GOLD,
            stock_goods: &[WOOD, FOOD],
            labor_capacity: 0,
            labor_used: 0,
        });
        original[0].price = Gold(99);
        let second = accumulator.observe(MetricObservationInput {
            tick: 0,
            agents: &agents,
            initial_agents: &initial_agents,
            money_system: None,
            receipts: &[],
            trades: &changed,
            tick_trades: &changed,
            consumed_goods: &[],
            money_good: GOLD,
            stock_goods: &[WOOD, FOOD],
            labor_capacity: 0,
            labor_used: 0,
        });

        assert_eq!(first.cumulative_sector_price_dispersion_bps, Some(0));
        assert_eq!(second.cumulative_sector_price_dispersion_bps, Some(3_333));
    }

    #[test]
    fn metric_accumulator_rebuilds_later_tick_when_trade_prefix_middle_changes() {
        let initial_agents = vec![metric_agent(1), metric_agent(2)];
        let mut agents = initial_agents.clone();
        agents[0].stock.add(FOOD, 1);
        let original = vec![
            Trade {
                tick: 0,
                good: WOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(10),
                qty: 1,
            },
            Trade {
                tick: 0,
                good: FOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(10),
                qty: 1,
            },
        ];
        let mut changed = original.clone();
        changed[0].price = Gold(20);
        let mut accumulator = MetricObservationAccumulator::default();

        let first = accumulator.observe(MetricObservationInput {
            tick: 0,
            agents: &agents,
            initial_agents: &initial_agents,
            money_system: None,
            receipts: &[],
            trades: &original,
            tick_trades: &original,
            consumed_goods: &[],
            money_good: GOLD,
            stock_goods: &[WOOD, FOOD],
            labor_capacity: 0,
            labor_used: 0,
        });
        let second = accumulator.observe(MetricObservationInput {
            tick: 1,
            agents: &agents,
            initial_agents: &initial_agents,
            money_system: None,
            receipts: &[],
            trades: &changed,
            tick_trades: &[],
            consumed_goods: &[],
            money_good: GOLD,
            stock_goods: &[WOOD, FOOD],
            labor_capacity: 0,
            labor_used: 0,
        });

        assert_eq!(first.cumulative_sector_price_dispersion_bps, Some(0));
        assert_eq!(second.cumulative_sector_price_dispersion_bps, Some(3_333));
    }

    #[test]
    fn realized_delta_counts_explicit_self_consumption() {
        let initial_agents = vec![metric_agent(1), metric_agent(2), metric_agent(3)];
        let agents = initial_agents.clone();
        let trades = [Trade {
            tick: 0,
            good: FOOD,
            buyer: AgentId(2),
            seller: AgentId(3),
            price: Gold(5),
            qty: 1,
        }];
        let consumed_goods = [(AgentId(1), FOOD, 2)];

        let observation = build_metric_observation(MetricObservationInput {
            tick: 0,
            agents: &agents,
            initial_agents: &initial_agents,
            money_system: None,
            receipts: &[],
            trades: &trades,
            tick_trades: &trades,
            consumed_goods: &consumed_goods,
            money_good: GOLD,
            stock_goods: &[FOOD],
            labor_capacity: 0,
            labor_used: 0,
        });

        let producer = observation
            .agent_wealth
            .iter()
            .find(|record| record.agent == AgentId(1))
            .expect("agent 1 has a wealth observation");
        assert_eq!(producer.realized_delta, 10);
    }

    #[test]
    fn realized_delta_zero_values_unpriced_explicit_consumption() {
        let initial_agents = vec![metric_agent(1)];
        let agents = initial_agents.clone();

        let observation = build_metric_observation(MetricObservationInput {
            tick: 0,
            agents: &agents,
            initial_agents: &initial_agents,
            money_system: None,
            receipts: &[],
            trades: &[],
            tick_trades: &[],
            consumed_goods: &[(AgentId(1), FOOD, 2)],
            money_good: GOLD,
            stock_goods: &[FOOD],
            labor_capacity: 0,
            labor_used: 0,
        });

        assert_eq!(observation.agent_wealth[0].realized_delta, 0);
    }

    #[test]
    fn realized_delta_accumulates_incremental_consumption_buffers() {
        let initial_agents = vec![metric_agent(1), metric_agent(2)];
        let agents = initial_agents.clone();
        let trades = [Trade {
            tick: 0,
            good: FOOD,
            buyer: AgentId(2),
            seller: AgentId(1),
            price: Gold(5),
            qty: 1,
        }];
        let mut accumulator = MetricObservationAccumulator::default();

        let first = accumulator.observe(MetricObservationInput {
            tick: 0,
            agents: &agents,
            initial_agents: &initial_agents,
            money_system: None,
            receipts: &[],
            trades: &trades,
            tick_trades: &trades,
            consumed_goods: &[(AgentId(1), FOOD, 1)],
            money_good: GOLD,
            stock_goods: &[FOOD],
            labor_capacity: 0,
            labor_used: 0,
        });
        let second = accumulator.observe(MetricObservationInput {
            tick: 1,
            agents: &agents,
            initial_agents: &initial_agents,
            money_system: None,
            receipts: &[],
            trades: &trades,
            tick_trades: &[],
            consumed_goods: &[(AgentId(1), FOOD, 2)],
            money_good: GOLD,
            stock_goods: &[FOOD],
            labor_capacity: 0,
            labor_used: 0,
        });

        assert_eq!(first.agent_wealth[0].realized_delta, 5);
        assert_eq!(second.agent_wealth[0].realized_delta, 15);
    }

    #[test]
    fn sector_price_dispersion_uses_realized_trades_only() {
        let trades = [
            Trade {
                tick: 0,
                good: GOLD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(99),
                qty: 9,
            },
            Trade {
                tick: 0,
                good: FOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(1),
                qty: 1,
            },
            Trade {
                tick: 0,
                good: WOOD,
                buyer: AgentId(2),
                seller: AgentId(1),
                price: Gold(3),
                qty: 1,
            },
        ];

        assert_eq!(sector_price_dispersion_bps(&trades, GOLD), Some(5_000));
    }

    #[test]
    fn sector_price_dispersion_returns_none_with_one_sector() {
        let trades = [Trade {
            tick: 0,
            good: FOOD,
            buyer: AgentId(1),
            seller: AgentId(2),
            price: Gold(1),
            qty: 1,
        }];

        assert_eq!(sector_price_dispersion_bps(&trades, GOLD), None);
    }

    #[test]
    fn sector_price_dispersion_aggregates_unknown_goods_into_one_sector() {
        // impl-06.md: every unknown good maps to ONE "other" sector. Two unknown goods
        // alone are a single non-money sector, so dispersion is None.
        let two_unknowns = [
            Trade {
                tick: 0,
                good: GoodId(4),
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(10),
                qty: 1,
            },
            Trade {
                tick: 0,
                good: GoodId(5),
                buyer: AgentId(2),
                seller: AgentId(1),
                price: Gold(30),
                qty: 1,
            },
        ];
        assert_eq!(sector_price_dispersion_bps(&two_unknowns, GOLD), None);

        // Add a FOOD (consumer) trade: now two sectors, Consumer and the aggregated
        // Other. The two unknowns aggregate to avg (10+30)/2 = 20, equal to the FOOD
        // price, so dispersion is 0 — proving they collapsed into one sector (distinct
        // per-good sectors would give a nonzero value).
        let with_consumer = [
            Trade {
                tick: 0,
                good: FOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(20),
                qty: 1,
            },
            Trade {
                tick: 0,
                good: GoodId(4),
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(10),
                qty: 1,
            },
            Trade {
                tick: 0,
                good: GoodId(5),
                buyer: AgentId(2),
                seller: AgentId(1),
                price: Gold(30),
                qty: 1,
            },
        ];
        assert_eq!(sector_price_dispersion_bps(&with_consumer, GOLD), Some(0));
    }

    #[test]
    fn sectoral_price_dispersion_peaks_at_or_after_boom() {
        let pre_boom = [
            Trade {
                tick: 0,
                good: FOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(10),
                qty: 1,
            },
            Trade {
                tick: 0,
                good: NET,
                buyer: AgentId(2),
                seller: AgentId(1),
                price: Gold(10),
                qty: 1,
            },
        ];
        let boom = [
            pre_boom[0].clone(),
            pre_boom[1].clone(),
            Trade {
                tick: 1,
                good: FOOD,
                buyer: AgentId(1),
                seller: AgentId(2),
                price: Gold(5),
                qty: 1,
            },
            Trade {
                tick: 1,
                good: NET,
                buyer: AgentId(2),
                seller: AgentId(1),
                price: Gold(25),
                qty: 1,
            },
        ];

        let baseline = sector_price_dispersion_bps(&pre_boom, GOLD).unwrap();
        let peak = sector_price_dispersion_bps(&boom, GOLD).unwrap();

        assert!(peak > baseline);
    }

    #[test]
    fn idle_labor_bps_counts_unused_capacity() {
        assert_eq!(idle_labor_bps(10, 6), Some(4_000));
        assert_eq!(idle_labor_bps(0, 0), None);
        assert_eq!(idle_labor_bps(10, 12), Some(0));
    }

    #[test]
    fn idle_labor_proxy_rises_after_credit_stop() {
        let society =
            run_m3_with_shadow(builtin_market_scenario(ScenarioName::FiatCreditExpansion));
        let records = build_m4_records(&society).expect("fiat-credit-expansion builds M4 records");
        let during_credit = records
            .iter()
            .filter(|record| record.tick > 0 && record.fiat_credit_issued > Gold::ZERO)
            .filter_map(|record| record.idle_labor_bps)
            .max()
            .expect("fiat-credit-expansion issues fiat credit");
        let after_stop = records
            .iter()
            .filter(|record| record.tick >= 3 && record.fiat_credit_issued == Gold::ZERO)
            .filter_map(|record| record.idle_labor_bps)
            .max()
            .expect("fiat-credit-expansion has post-stop observations");

        assert!(after_stop > during_credit);
    }
}
