//! Ordinal present/future gold quote derivation.

use crate::agent::{Agent, Want, WantKind};
use crate::good::{Gold, GoodId, Horizon, Stock, GOLD};
use crate::project::Tick;
use crate::purpose::{CreditLender, CreditSource, DebtPurpose};
use crate::timemarket::{DebtContract, DebtId, DebtState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AgioQuote {
    pub present: Gold,
    pub future: Gold,
    pub horizon: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgioSchedule {
    pub lending: Vec<AgioQuote>,
    pub borrowing: Vec<AgioQuote>,
}

pub struct TemporalEndowment<'a> {
    pub stock: &'a Stock,
    pub gold: Gold,
    pub receivables: &'a [DebtContract],
    pub payables: &'a [DebtContract],
    pub tick: Tick,
}

#[derive(Clone, Debug)]
struct TemporalProvisioning {
    provided: Vec<bool>,
}

#[derive(Default)]
struct TemporalScratch {
    provided: Vec<bool>,
    reserved_stock: Vec<(crate::good::GoodId, u32)>,
    receivables: Vec<(Tick, u64)>,
    payables: Vec<(Tick, u64)>,
}

struct QuoteContext<'a> {
    scale: &'a [Want],
    stock: &'a Stock,
    gold: Gold,
    receivables: &'a [DebtContract],
    payables: &'a [DebtContract],
    tick: Tick,
    money_good: GoodId,
}

const MAX_EXACT_QUOTE_WALK: u64 = 4_096;

impl AgioSchedule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn min_future_due_for_lending(&self, present: Gold, horizon: u8) -> Option<Gold> {
        self.lending
            .iter()
            .filter(|quote| quote.present == present && quote.horizon == horizon)
            .map(|quote| quote.future)
            .min()
    }

    pub fn max_future_due_for_borrowing(&self, present: Gold, horizon: u8) -> Option<Gold> {
        self.borrowing
            .iter()
            .filter(|quote| quote.present == present && quote.horizon == horizon)
            .map(|quote| quote.future)
            .max()
    }

    pub fn present_value(&self, future: Gold, horizon: u8) -> Option<Gold> {
        // The most present gold the agent would exchange for a future-gold
        // receipt at this horizon — a genuine lower bound on its valuation.
        // Derived ONLY from LENDING quotes: a lend quote `(present, future)` means
        // "I would pay `present` now to acquire `future` later", so `future` future
        // is worth at least `present` present. Borrowing quotes are deliberately
        // EXCLUDED: `max_future_due_for_borrowing` is a financing-capacity ceiling,
        // not an indifference point, so it does not bound valuation from below and
        // using it here would over-value future revenue (see concerns.md Concern 1).
        // `horizon` is the requested time-to-receipt; a quote at a LONGER horizon
        // lower-bounds the value of sooner gold (sooner is worth at least as much),
        // so accept any quote with `quote.horizon >= horizon`.
        self.lending
            .iter()
            .filter(|quote| quote.horizon >= horizon && quote.future <= future)
            .map(|quote| quote.present)
            .max()
    }
}

impl Agent {
    pub fn derive_agio_schedule(
        &self,
        existing_debts: &[DebtContract],
        tick: Tick,
    ) -> AgioSchedule {
        let receivables = existing_debts
            .iter()
            .filter(|debt| debt.is_open() && debt.lender == CreditLender::Agent(self.id))
            .cloned()
            .collect::<Vec<_>>();
        let payables = existing_debts
            .iter()
            .filter(|debt| debt.is_open() && debt.borrower == self.id)
            .cloned()
            .collect::<Vec<_>>();
        self.derive_agio_schedule_from_claims(&receivables, &payables, tick)
    }

    pub(crate) fn derive_agio_schedule_from_claims(
        &self,
        receivables: &[DebtContract],
        payables: &[DebtContract],
        tick: Tick,
    ) -> AgioSchedule {
        self.derive_agio_schedule_from_claims_at_gold(self.gold, receivables, payables, tick)
    }

    pub(crate) fn derive_agio_schedule_from_claims_at_gold(
        &self,
        gold: Gold,
        receivables: &[DebtContract],
        payables: &[DebtContract],
        tick: Tick,
    ) -> AgioSchedule {
        self.derive_agio_schedule_from_claims_at_gold_for_money(
            gold,
            receivables,
            payables,
            tick,
            GOLD,
        )
    }

    pub(crate) fn derive_agio_schedule_from_claims_at_gold_for_money(
        &self,
        gold: Gold,
        receivables: &[DebtContract],
        payables: &[DebtContract],
        tick: Tick,
        money_good: GoodId,
    ) -> AgioSchedule {
        let horizons = money_later_horizons(&self.scale, money_good);
        let mut schedule = AgioSchedule::new();
        let ctx = QuoteContext {
            scale: &self.scale,
            stock: &self.stock,
            gold,
            receivables,
            payables,
            tick,
            money_good,
        };

        for horizon in horizons {
            let max_lending_present = lending_present_bound(&self.scale, gold, horizon, money_good);
            let can_lend = has_unprovided_later_money(&ctx, horizon);
            let max_present = max_lending_present.max(borrowing_present_bound(
                &self.scale,
                payables,
                tick,
                horizon,
                money_good,
            ));
            for present in quote_amount_candidates(&self.scale, max_present, money_good) {
                if can_lend && present <= max_lending_present {
                    if let Some(future) = min_lending_future(&ctx, Gold(present), horizon) {
                        schedule.lending.push(AgioQuote {
                            present: Gold(present),
                            future,
                            horizon,
                        });
                    }
                }
                if let Some(future) = max_borrowing_future(&ctx, Gold(present), horizon) {
                    schedule.borrowing.push(AgioQuote {
                        present: Gold(present),
                        future,
                        horizon,
                    });
                }
            }
        }

        schedule
    }
}

fn has_unprovided_later_money(ctx: &QuoteContext<'_>, horizon: u8) -> bool {
    let before_endowment = TemporalEndowment {
        stock: ctx.stock,
        gold: ctx.gold,
        receivables: ctx.receivables,
        payables: ctx.payables,
        tick: ctx.tick,
    };
    let mut before = TemporalScratch::default();
    temporal_provisioning_into(ctx.scale, &before_endowment, &mut before, ctx.money_good);
    ctx.scale.iter().enumerate().any(|(index, want)| {
        want.kind == WantKind::Good(ctx.money_good)
            && matches!(want.horizon, Horizon::Later(later) if later >= horizon)
            && !before.provided.get(index).copied().unwrap_or(false)
    })
}

fn lending_present_bound(scale: &[Want], gold: Gold, horizon: u8, money_good: GoodId) -> u64 {
    if gold == Gold::ZERO {
        return 0;
    }
    let money_wants = scale
        .iter()
        .filter(|want| {
            want.kind == WantKind::Good(money_good)
                && (matches!(want.horizon, Horizon::Now | Horizon::Next)
                    || matches!(want.horizon, Horizon::Later(later) if later >= horizon))
        })
        .map(|want| u64::from(want.qty))
        .fold(0u64, u64::saturating_add);
    gold.0.min(money_wants)
}

pub fn want_provisioned_temporally(
    scale: &[Want],
    idx: usize,
    endowment: &TemporalEndowment<'_>,
) -> bool {
    want_provisioned_temporally_for_money(scale, idx, endowment, GOLD)
}

pub fn want_provisioned_temporally_for_money(
    scale: &[Want],
    idx: usize,
    endowment: &TemporalEndowment<'_>,
    money_good: GoodId,
) -> bool {
    temporal_provisioning_bitmap_for_money(scale, endowment, money_good)
        .get(idx)
        .copied()
        .unwrap_or(false)
}

pub(crate) fn temporal_provisioning_bitmap_for_money(
    scale: &[Want],
    endowment: &TemporalEndowment<'_>,
    money_good: GoodId,
) -> Vec<bool> {
    temporal_provisioning_for_money(scale, endowment, money_good).provided
}

fn min_lending_future(ctx: &QuoteContext<'_>, present: Gold, horizon: u8) -> Option<Gold> {
    if present == Gold::ZERO || ctx.gold < present {
        return None;
    }

    let before_endowment = TemporalEndowment {
        stock: ctx.stock,
        gold: ctx.gold,
        receivables: ctx.receivables,
        payables: ctx.payables,
        tick: ctx.tick,
    };
    let mut before = TemporalScratch::default();
    temporal_provisioning_into(ctx.scale, &before_endowment, &mut before, ctx.money_good);
    let bound = lending_future_bound(ctx.scale, ctx.gold, present, horizon, ctx.money_good);
    let mut after_receivables = Vec::with_capacity(ctx.receivables.len().saturating_add(1));
    let mut after = TemporalScratch::default();
    for future in lending_future_candidates(ctx, present, horizon, bound, &before.provided) {
        let debt = hypothetical_debt(ctx.tick, horizon, present, Gold(future));
        after_receivables.clear();
        after_receivables.extend_from_slice(ctx.receivables);
        after_receivables.push(debt);
        let after_gold = ctx.gold.checked_sub(present)?;
        let after_endowment = TemporalEndowment {
            stock: ctx.stock,
            gold: after_gold,
            receivables: &after_receivables,
            payables: ctx.payables,
            tick: ctx.tick,
        };
        temporal_provisioning_into(ctx.scale, &after_endowment, &mut after, ctx.money_good);
        let Some(target) = gained_future_money_rank(
            ctx.scale,
            &before.provided,
            &after.provided,
            horizon,
            ctx.money_good,
        ) else {
            continue;
        };
        if preserved_above_target(&before.provided, &after.provided, target) {
            return Some(Gold(future));
        }
    }
    None
}

fn max_borrowing_future(ctx: &QuoteContext<'_>, present: Gold, horizon: u8) -> Option<Gold> {
    if present == Gold::ZERO {
        return None;
    }
    let before_endowment = TemporalEndowment {
        stock: ctx.stock,
        gold: ctx.gold,
        receivables: ctx.receivables,
        payables: ctx.payables,
        tick: ctx.tick,
    };
    let mut before = TemporalScratch::default();
    temporal_provisioning_into(ctx.scale, &before_endowment, &mut before, ctx.money_good);
    let after_gold = ctx.gold.checked_add(present)?;
    let after_present_endowment = TemporalEndowment {
        stock: ctx.stock,
        gold: after_gold,
        receivables: ctx.receivables,
        payables: ctx.payables,
        tick: ctx.tick,
    };
    let mut after_present = TemporalScratch::default();
    temporal_provisioning_into(
        ctx.scale,
        &after_present_endowment,
        &mut after_present,
        ctx.money_good,
    );
    let target = ctx.scale.iter().enumerate().find_map(|(index, want)| {
        if want.kind == WantKind::Good(ctx.money_good)
            && matches!(want.horizon, Horizon::Now | Horizon::Next)
            && !before.provided.get(index).copied().unwrap_or(false)
            && after_present.provided.get(index).copied().unwrap_or(false)
        {
            return Some(index);
        }
        None
    })?;
    let future_capacity =
        future_money_capacity_below_target(ctx.scale, target, horizon, ctx.money_good)
            .saturating_sub(payable_burden(ctx.payables, ctx.tick, horizon));
    if future_capacity < present.0 {
        return None;
    }

    let mut after_payables = Vec::with_capacity(ctx.payables.len().saturating_add(1));
    let mut after = TemporalScratch::default();
    for future in borrowing_future_candidates(ctx.scale, present, future_capacity, ctx.money_good) {
        let debt = hypothetical_debt(ctx.tick, horizon, present, Gold(future));
        after_payables.clear();
        after_payables.extend_from_slice(ctx.payables);
        after_payables.push(debt);
        let after_endowment = TemporalEndowment {
            stock: ctx.stock,
            gold: after_gold,
            receivables: ctx.receivables,
            payables: &after_payables,
            tick: ctx.tick,
        };
        temporal_provisioning_into(ctx.scale, &after_endowment, &mut after, ctx.money_good);
        if after.provided.get(target).copied().unwrap_or(false)
            && preserved_above_target(&before.provided, &after.provided, target)
        {
            return Some(Gold(future));
        }
    }
    None
}

fn temporal_provisioning_for_money(
    scale: &[Want],
    endowment: &TemporalEndowment<'_>,
    money_good: GoodId,
) -> TemporalProvisioning {
    let mut scratch = TemporalScratch::default();
    temporal_provisioning_into(scale, endowment, &mut scratch, money_good);
    TemporalProvisioning {
        provided: scratch.provided,
    }
}

fn temporal_provisioning_into(
    scale: &[Want],
    endowment: &TemporalEndowment<'_>,
    scratch: &mut TemporalScratch,
    money_good: GoodId,
) {
    scratch.provided.clear();
    scratch.provided.resize(scale.len(), false);
    scratch.reserved_stock.clear();
    scratch.receivables.clear();
    scratch.payables.clear();
    let mut current_gold = endowment.gold.0;
    scratch.receivables.extend(
        endowment
            .receivables
            .iter()
            .filter(|debt| debt.state == DebtState::Open)
            .map(|debt| (debt.due_tick, debt.remaining_due().0)),
    );
    scratch.payables.extend(
        endowment
            .payables
            .iter()
            .filter(|debt| debt.state == DebtState::Open)
            .map(|debt| (debt.due_tick, debt.remaining_due().0)),
    );
    scratch.receivables.sort_by_key(|(due_tick, _)| *due_tick);
    scratch.payables.sort_by_key(|(due_tick, _)| *due_tick);

    for (index, want) in scale.iter().enumerate() {
        let WantKind::Good(good) = want.kind else {
            continue;
        };
        if want.qty == 0 {
            scratch.provided[index] = true;
            continue;
        }

        if good == money_good {
            let needed = u64::from(want.qty);
            match want.horizon {
                Horizon::Now | Horizon::Next => {
                    if current_gold >= needed {
                        current_gold -= needed;
                        scratch.provided[index] = true;
                    }
                }
                Horizon::Later(horizon) => {
                    let due_by = Tick(endowment.tick.0.saturating_add(u64::from(horizon)));
                    if future_capacity_due_by(
                        current_gold,
                        &scratch.receivables,
                        &scratch.payables,
                        due_by,
                    ) >= needed
                    {
                        let provisioned = consume_future_capacity(
                            &mut current_gold,
                            &mut scratch.receivables,
                            due_by,
                            needed,
                        );
                        scratch.provided[index] = provisioned == needed;
                    }
                }
            }
            continue;
        }

        if matches!(want.horizon, Horizon::Later(_)) {
            continue;
        }
        let available = available_after_reserved(endowment.stock, &scratch.reserved_stock, good);
        if available >= want.qty {
            reserve_stock(&mut scratch.reserved_stock, good, want.qty);
            scratch.provided[index] = true;
        }
    }
}

fn future_capacity_due_by(
    current_gold: u64,
    receivables: &[(Tick, u64)],
    payables: &[(Tick, u64)],
    due_by: Tick,
) -> u64 {
    let resources = current_gold.saturating_add(sum_due_capacity(receivables, due_by));
    resources.saturating_sub(sum_due_capacity(payables, due_by))
}

fn sum_due_capacity(entries: &[(Tick, u64)], due_by: Tick) -> u64 {
    entries
        .iter()
        .filter(|(due_tick, _)| *due_tick <= due_by)
        .map(|(_, remaining)| *remaining)
        .fold(0u64, u64::saturating_add)
}

fn consume_future_capacity(
    current_gold: &mut u64,
    receivables: &mut [(Tick, u64)],
    due_by: Tick,
    mut qty: u64,
) -> u64 {
    let mut consumed = 0;
    for (due_tick, remaining) in receivables.iter_mut().rev() {
        if *due_tick > due_by || qty == 0 {
            continue;
        }
        let take = (*remaining).min(qty);
        *remaining -= take;
        qty -= take;
        consumed += take;
    }
    if qty > 0 {
        let take = (*current_gold).min(qty);
        *current_gold -= take;
        consumed += take;
    }
    consumed
}

fn money_later_horizons(scale: &[Want], money_good: GoodId) -> Vec<u8> {
    let mut horizons = Vec::new();
    for want in scale {
        if let (WantKind::Good(good), Horizon::Later(horizon)) = (want.kind, want.horizon) {
            if good != money_good {
                continue;
            }
            if !horizons.contains(&horizon) {
                horizons.push(horizon);
            }
        }
    }
    horizons.sort_unstable();
    horizons
}

fn lending_future_bound(
    scale: &[Want],
    gold: Gold,
    present: Gold,
    horizon: u8,
    money_good: GoodId,
) -> u64 {
    let future_wants = scale
        .iter()
        .filter(|want| {
            want.kind == WantKind::Good(money_good)
                && matches!(want.horizon, Horizon::Later(later) if later >= horizon)
        })
        .map(|want| u64::from(want.qty))
        .fold(0u64, u64::saturating_add);
    let listed_money_wants = listed_money_wants(scale, money_good);
    present
        .0
        .saturating_add(future_wants)
        .saturating_add(gold.0.min(listed_money_wants))
}

fn listed_money_wants(scale: &[Want], money_good: GoodId) -> u64 {
    scale
        .iter()
        .filter(|want| want.kind == WantKind::Good(money_good))
        .map(|want| u64::from(want.qty))
        .fold(0u64, u64::saturating_add)
}

fn quote_amount_candidates(scale: &[Want], upper: u64, money_good: GoodId) -> Vec<u64> {
    if upper == 0 {
        return Vec::new();
    }
    let mut candidates = Vec::new();
    push_exact_prefix(&mut candidates, upper);
    push_money_rank_thresholds(&mut candidates, scale, upper, money_good);
    push_candidate(&mut candidates, upper, upper);
    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

fn lending_future_candidates(
    ctx: &QuoteContext<'_>,
    present: Gold,
    horizon: u8,
    upper: u64,
    before: &[bool],
) -> Vec<u64> {
    if upper <= present.0 {
        return Vec::new();
    }
    let mut candidates = Vec::new();
    push_exact_range(&mut candidates, present.0.saturating_add(1), upper);
    push_candidate(&mut candidates, present.0.saturating_add(1), upper);
    for (index, want) in ctx.scale.iter().enumerate() {
        if want.kind != WantKind::Good(ctx.money_good)
            || before.get(index).copied().unwrap_or(false)
            || !matches!(want.horizon, Horizon::Later(later) if later >= horizon)
        {
            continue;
        }
        let needed = future_due_needed_for_later_rank(ctx, present, index);
        push_candidate(
            &mut candidates,
            needed.max(present.0.saturating_add(1)),
            upper,
        );
    }
    push_candidate(&mut candidates, upper, upper);
    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

fn borrowing_future_candidates(
    scale: &[Want],
    present: Gold,
    upper: u64,
    money_good: GoodId,
) -> Vec<u64> {
    if upper < present.0 {
        return Vec::new();
    }
    let mut candidates = Vec::new();
    push_exact_range(&mut candidates, present.0, upper);
    push_money_rank_thresholds(&mut candidates, scale, upper, money_good);
    push_candidate(&mut candidates, present.0, upper);
    push_candidate(&mut candidates, upper, upper);
    candidates.sort_unstable_by(|left, right| right.cmp(left));
    candidates.dedup();
    candidates
}

fn push_exact_prefix(candidates: &mut Vec<u64>, upper: u64) {
    push_exact_range(candidates, 1, upper);
}

fn push_exact_range(candidates: &mut Vec<u64>, start: u64, upper: u64) {
    if start > upper {
        return;
    }
    let end = upper.min(start.saturating_add(MAX_EXACT_QUOTE_WALK).saturating_sub(1));
    for value in start..=end {
        candidates.push(value);
    }
}

fn push_money_rank_thresholds(
    candidates: &mut Vec<u64>,
    scale: &[Want],
    upper: u64,
    money_good: GoodId,
) {
    let mut cumulative = 0u64;
    for want in scale {
        if want.kind != WantKind::Good(money_good) {
            continue;
        }
        cumulative = cumulative.saturating_add(u64::from(want.qty));
        push_candidate(candidates, cumulative, upper);
        push_candidate(candidates, cumulative.saturating_add(1), upper);
    }
}

fn push_candidate(candidates: &mut Vec<u64>, value: u64, upper: u64) {
    if value > 0 && value <= upper {
        candidates.push(value);
    }
}

fn future_due_needed_for_later_rank(ctx: &QuoteContext<'_>, present: Gold, target: usize) -> u64 {
    let Some(after_gold) = ctx.gold.checked_sub(present) else {
        return u64::MAX;
    };
    let Some(Horizon::Later(horizon)) = ctx.scale.get(target).map(|want| want.horizon) else {
        return u64::MAX;
    };
    let due_by = Tick(ctx.tick.0.saturating_add(u64::from(horizon)));
    let required = money_required_through_rank(ctx.scale, target, ctx.money_good);
    let available = after_gold
        .0
        .saturating_add(debt_capacity_due_by(ctx.receivables, due_by))
        .saturating_sub(debt_capacity_due_by(ctx.payables, due_by));
    required.saturating_sub(available)
}

fn money_required_through_rank(scale: &[Want], target: usize, money_good: GoodId) -> u64 {
    scale
        .iter()
        .take(target.saturating_add(1))
        .filter(|want| want.kind == WantKind::Good(money_good))
        .map(|want| u64::from(want.qty))
        .fold(0u64, u64::saturating_add)
}

fn debt_capacity_due_by(debts: &[DebtContract], due_by: Tick) -> u64 {
    debts
        .iter()
        .filter(|debt| debt.state == DebtState::Open && debt.due_tick <= due_by)
        .map(|debt| debt.remaining_due().0)
        .fold(0u64, u64::saturating_add)
}

fn borrowing_present_bound(
    scale: &[Want],
    payables: &[DebtContract],
    tick: Tick,
    horizon: u8,
    money_good: GoodId,
) -> u64 {
    scale
        .iter()
        .enumerate()
        .filter(|(_, want)| {
            want.kind == WantKind::Good(money_good)
                && matches!(want.horizon, Horizon::Now | Horizon::Next)
        })
        .map(|(target, _)| {
            future_money_capacity_below_target(scale, target, horizon, money_good)
                .saturating_sub(payable_burden(payables, tick, horizon))
        })
        .max()
        .unwrap_or(0)
}

fn gained_future_money_rank(
    scale: &[Want],
    before: &[bool],
    after: &[bool],
    horizon: u8,
    money_good: GoodId,
) -> Option<usize> {
    scale
        .iter()
        .enumerate()
        .find(|(index, want)| {
            want.kind == WantKind::Good(money_good)
                && matches!(want.horizon, Horizon::Later(later) if later >= horizon)
                && !before.get(*index).copied().unwrap_or(false)
                && after.get(*index).copied().unwrap_or(false)
        })
        .map(|(index, _)| index)
}

fn future_money_capacity_below_target(
    scale: &[Want],
    target: usize,
    horizon: u8,
    money_good: GoodId,
) -> u64 {
    scale
        .iter()
        .enumerate()
        .skip(target + 1)
        .filter(|(_, want)| {
            want.kind == WantKind::Good(money_good)
                && matches!(want.horizon, Horizon::Later(later) if later >= horizon)
        })
        .map(|(_, want)| u64::from(want.qty))
        .fold(0u64, u64::saturating_add)
}

fn payable_burden(payables: &[DebtContract], tick: Tick, horizon: u8) -> u64 {
    let due_by = Tick(tick.0.saturating_add(u64::from(horizon)));
    payables
        .iter()
        .filter(|debt| debt.state == DebtState::Open && debt.due_tick <= due_by)
        .map(|debt| debt.remaining_due().0)
        .fold(0u64, u64::saturating_add)
}

fn preserved_above_target(before: &[bool], after: &[bool], target: usize) -> bool {
    before
        .iter()
        .zip(after)
        .take(target)
        .all(|(was, now)| !*was || *now)
}

fn available_after_reserved(
    stock: &Stock,
    reservations: &[(crate::good::GoodId, u32)],
    good: crate::good::GoodId,
) -> u32 {
    let reserved = reservations
        .iter()
        .filter(|(reserved_good, _)| *reserved_good == good)
        .map(|(_, qty)| *qty)
        .sum::<u32>();
    stock.get(good).saturating_sub(reserved)
}

fn reserve_stock(
    reservations: &mut Vec<(crate::good::GoodId, u32)>,
    good: crate::good::GoodId,
    qty: u32,
) {
    if let Some((_, reserved)) = reservations
        .iter_mut()
        .find(|(reserved_good, _)| *reserved_good == good)
    {
        *reserved = reserved.saturating_add(qty);
    } else {
        reservations.push((good, qty));
    }
}

fn hypothetical_debt(tick: Tick, horizon: u8, principal: Gold, due: Gold) -> DebtContract {
    DebtContract {
        id: DebtId(0),
        lender: CreditLender::Agent(crate::agent::AgentId(0)),
        borrower: crate::agent::AgentId(0),
        opened_tick: tick,
        due_tick: Tick(tick.0.saturating_add(u64::from(horizon))),
        principal,
        due,
        paid: Gold::ZERO,
        state: DebtState::Open,
        purpose: DebtPurpose::Consumption,
        funding: CreditSource::Commodity,
    }
}

#[cfg(test)]
mod tests {
    use super::{want_provisioned_temporally, AgioQuote, AgioSchedule, TemporalEndowment};
    use crate::agent::{Agent, AgentId, Role, Want, WantKind};
    use crate::good::{Gold, Horizon, Stock, GOLD};
    use crate::project::Tick;
    use crate::purpose::{CreditLender, CreditSource, DebtPurpose};
    use crate::timemarket::{DebtContract, DebtId, DebtState};

    fn want(kind: WantKind, horizon: Horizon) -> Want {
        Want {
            kind,
            horizon,
            qty: 1,
            satisfied: false,
        }
    }

    fn repeat(scale: &mut Vec<Want>, kind: WantKind, horizon: Horizon, n: usize) {
        for _ in 0..n {
            scale.push(want(kind, horizon));
        }
    }

    fn agent(id: u32, gold: Gold, scale: Vec<Want>) -> Agent {
        Agent {
            id: AgentId(id),
            scale,
            stock: Stock::new(3),
            gold,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Trader],
            expect: Vec::new(),
        }
    }

    fn debt(lender: u32, borrower: u32, due_tick: u64, due: Gold) -> DebtContract {
        DebtContract {
            id: DebtId(1),
            lender: CreditLender::Agent(AgentId(lender)),
            borrower: AgentId(borrower),
            opened_tick: Tick(0),
            due_tick: Tick(due_tick),
            principal: Gold(1),
            due,
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::Commodity,
        }
    }

    #[test]
    fn agio_lending_quote_respects_scale() {
        let mut scale = Vec::new();
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Now, 4);
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Later(4), 8);
        let agent = agent(1, Gold(5), scale);
        let schedule = agent.derive_agio_schedule(&[], Tick(0));

        assert_eq!(
            schedule.min_future_due_for_lending(Gold(1), 4),
            Some(Gold(2))
        );
    }

    #[test]
    fn agio_lending_quotes_are_not_capped_at_thirty_two_gold() {
        let mut scale = Vec::new();
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Now, 7);
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Later(4), 100);
        let agent = agent(1, Gold(40), scale);
        let schedule = agent.derive_agio_schedule(&[], Tick(0));

        assert_eq!(
            schedule.min_future_due_for_lending(Gold(33), 4),
            Some(Gold(34))
        );
    }

    #[test]
    fn present_value_does_not_use_borrowing_capacity_as_lower_bound() {
        // A cash-poor agent that can only BORROW (urgent present-gold wants, no
        // spare gold to lend) has borrowing quotes but no lending quotes. Its
        // borrowing capacity must NOT be read as a positive present valuation of
        // future gold — present_value is a lower bound and only lending quotes
        // provide one (see concerns.md Concern 1).
        let mut scale = Vec::new();
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Now, 2);
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Later(4), 4);
        let agent = agent(2, Gold::ZERO, scale);
        let schedule = agent.derive_agio_schedule(&[], Tick(0));

        // setup is meaningful: it really does have borrowing capacity ...
        assert!(schedule.max_future_due_for_borrowing(Gold(1), 4).is_some());
        assert!(schedule.lending.is_empty());
        // ... but no lending quote, so it has no positive present value of future gold.
        assert_eq!(schedule.present_value(Gold(100), 4), None);
        assert_eq!(schedule.present_value(Gold(5), 4), None);
    }

    #[test]
    fn agio_borrowing_quote_respects_scale() {
        let mut scale = Vec::new();
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Now, 6);
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Later(4), 3);
        let agent = agent(2, Gold(4), scale);
        let schedule = agent.derive_agio_schedule(&[], Tick(0));

        assert_eq!(
            schedule.max_future_due_for_borrowing(Gold(1), 4),
            Some(Gold(3))
        );
    }

    #[test]
    fn cash_poor_agent_can_derive_borrowing_quote() {
        let mut scale = Vec::new();
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Now, 1);
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Later(4), 2);
        let agent = agent(2, Gold::ZERO, scale);
        let schedule = agent.derive_agio_schedule(&[], Tick(0));

        assert_eq!(
            schedule.max_future_due_for_borrowing(Gold(1), 4),
            Some(Gold(2))
        );
    }

    #[test]
    fn present_value_uses_only_lending_quotes_as_lower_bounds() {
        let schedule = AgioSchedule {
            lending: vec![AgioQuote {
                present: Gold(3),
                future: Gold(8),
                horizon: 4,
            }],
            borrowing: vec![AgioQuote {
                present: Gold(2),
                future: Gold(8),
                horizon: 4,
            }],
        };

        assert_eq!(schedule.present_value(Gold(8), 4), Some(Gold(3)));
        assert_eq!(schedule.present_value(Gold(8), 2), Some(Gold(3)));
        assert_eq!(schedule.present_value(Gold(7), 4), None);
    }

    #[test]
    fn agio_derivation_does_not_scan_large_holdings_linearly() {
        let scale = vec![Want {
            kind: WantKind::Good(GOLD),
            horizon: Horizon::Later(4),
            qty: 1,
            satisfied: false,
        }];
        let agent = agent(2, Gold(u64::MAX), scale);
        let debts = vec![DebtContract {
            id: DebtId(1),
            lender: CreditLender::Agent(AgentId(1)),
            borrower: AgentId(2),
            opened_tick: Tick(0),
            due_tick: Tick(4),
            principal: Gold(1),
            due: Gold(u64::MAX),
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::Commodity,
        }];

        let schedule = agent.derive_agio_schedule(&debts, Tick(0));

        assert!(schedule.lending.is_empty());
        assert!(schedule.borrowing.is_empty());
    }

    #[test]
    fn receivable_provisions_only_due_future_gold() {
        let mut scale = Vec::new();
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Now, 1);
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Next, 1);
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Later(4), 1);
        let receivables = vec![debt(1, 2, 3, Gold(1))];
        let stock = Stock::new(3);
        let endowment = TemporalEndowment {
            stock: &stock,
            gold: Gold::ZERO,
            receivables: &receivables,
            payables: &[],
            tick: Tick(0),
        };

        assert!(!want_provisioned_temporally(&scale, 0, &endowment));
        assert!(!want_provisioned_temporally(&scale, 1, &endowment));
        assert!(want_provisioned_temporally(&scale, 2, &endowment));
    }

    #[test]
    fn payable_reduces_future_borrowing_capacity() {
        let mut scale = Vec::new();
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Now, 6);
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Later(4), 3);
        let agent = agent(2, Gold(4), scale);
        let debts = vec![debt(1, 2, 4, Gold(2))];
        let schedule = agent.derive_agio_schedule(&debts, Tick(0));

        assert_eq!(
            schedule.max_future_due_for_borrowing(Gold(1), 4),
            Some(Gold(1))
        );
    }

    #[test]
    fn payable_reduces_future_temporal_resources() {
        let mut scale = Vec::new();
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Later(4), 10);
        let payables = vec![debt(1, 2, 4, Gold(3))];
        let stock = Stock::new(3);
        let endowment = TemporalEndowment {
            stock: &stock,
            gold: Gold(7),
            receivables: &[],
            payables: &payables,
            tick: Tick(0),
        };
        let provisioned = scale
            .iter()
            .enumerate()
            .filter(|(index, _)| want_provisioned_temporally(&scale, *index, &endowment))
            .count();

        assert_eq!(provisioned, 4);
    }
}
