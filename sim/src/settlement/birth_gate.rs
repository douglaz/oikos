//! DH.b-obs (impl-70) — the birth-gate-stock diagnostic: the `Settlement`-side join over the
//! `Society`-owned atomic-logical-event staple-stock tape, the pure/total birth-gate classifier,
//! the single `BirthGateReport` reducer, and the shared `run_burden_grid` harness both the DH.b and
//! DH.b-obs suites drive.
//!
//! Everything here is PURE OBSERVATION: every runtime hook gates through
//! [`Settlement::birth_gate_obs_active`] (configured AND [`Settlement::closure_active`]), so a
//! force-disabled or non-closed run records nothing and stays byte-identical. Nothing is serialized
//! beyond the configured-only tag-35 flag byte (see `docs/impl-birth-gate-obs.md`).
//!
//! The classifier ([`classify_birth_gate_opportunity`]) replays `[WindowStart, event_end)` and
//! returns exactly one [`ClassifiedBirthGateOpportunity`] per endowment-gate opportunity; the
//! report ([`BirthGateReport::from_traces`]) is the SOLE path to every printed share.

use super::burden::{
    build_burden_windows, burden_start_tick, classify_birth_funding, classify_burden_cell,
    synthesize_burden_grid, BurdenBirthObs, BurdenCellInput, BurdenCellResult, BurdenCellVerdict,
    BurdenSavingArm, BurdenSynthesis, BurdenWindowObs, BURDEN_QS, BURDEN_RUN_TICKS, BURDEN_SEEDS,
};
use super::closure::{ClosureClass, ClosureEventKind};
use super::{BirthStockSavingMode, Settlement, SettlementConfig};
use econ::agent::AgentId;
use econ::good::GoodId;
use econ::society::StapleStockEvent;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

// ===========================================================================================
// The joined logical-event tape (Society events + Settlement-appended Production/BirthDebit)
// ===========================================================================================

/// The cause of a single logical staple-stock event, used to attribute the below-`q` crossing.
/// Only DRAINING causes appear (a settled-trade BUY leg raises free stock and never crosses down,
/// so it is not a distinct cause); `Production` raises stock, `BirthDebit` is post-gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventCause {
    Consumption,
    SettledTradeSell,
    AskChange,
    Production,
    BirthDebit,
}

/// One atomic logical staple-stock event, as a group of per-affected-member post-event free
/// values applied together (a settled trade updates BOTH members at once — no phantom
/// intermediate). `cause` is the event's below-`q` crossing attribution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BirthGateEvent {
    pub cause: EventCause,
    /// `(member, post-event free)` for every affected member of the tracked producer household.
    pub updates: Vec<(AgentId, u32)>,
}

/// The authoritative gate read taken INSIDE `run_births` immediately before parent selection and
/// the debit: each living member's free staple stock and the recorded PASS/FAIL decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BirthGateState {
    pub members: Vec<(AgentId, u32)>,
    pub recorded_pass: bool,
}

impl BirthGateState {
    fn household_total(&self) -> u32 {
        self.members
            .iter()
            .map(|&(_, free)| free)
            .fold(0u32, u32::saturating_add)
    }
}

/// One raw endowment-gate opportunity captured for one producer household in one tick — the
/// self-contained classifier input (window baseline, joined events up to the gate, the gate read).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BirthGateRawOpportunity {
    pub tick: u64,
    pub household: usize,
    pub producer_type: ClosureClass,
    pub q: u32,
    /// This household's members' `(member, baseline free)` right after death/estate (event 0).
    pub window_start: Vec<(AgentId, u32)>,
    /// The joined logical events `[WindowStart, event_end)`, filtered to this household's members.
    pub events: Vec<BirthGateEvent>,
    /// The authoritative gate state (per-member free + recorded decision).
    pub gate: BirthGateState,
}

// ===========================================================================================
// The pure/total classifier
// ===========================================================================================

/// The birth-gate outcome ladder (§3). Total over every input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BirthGateOutcome {
    /// Recorded PASS and the replay recomputes PASS.
    GatePassed,
    /// The recorded gate decision and the replay recomputation disagree — the observer/gate
    /// contradiction. HARD guard: must be zero grid-wide.
    GateDecisionMismatch { recorded: bool, recomputed: bool },
    /// A single member reached ≥q at some event but is <q at the gate.
    MemberDrainedBeforeGate {
        crossing: EventCause,
        member: AgentId,
    },
    /// No single member ever reached ≥q, but the household TOTAL is ≥q at the gate (the pure
    /// atomicity wall).
    SplitAtGate,
    /// No single member ever reached ≥q and the household total is <q at the gate, BUT the total
    /// reached ≥q at some event.
    HouseholdDrainedBeforeGate { crossing: EventCause },
    /// The household total never reached q at any event.
    NeverReachedQ,
}

/// The classifier envelope (§3, R2-6): the outcome plus the peak/gap/gate-total the sole reducer
/// derives every distribution from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClassifiedBirthGateOpportunity {
    pub outcome: BirthGateOutcome,
    /// The max household-total free stock over the window.
    pub household_peak: u32,
    /// `q.saturating_sub(household_peak)`.
    pub gap_to_q: u32,
    /// The household-total free stock at the gate.
    pub gate_household_total: u32,
}

/// The internal replay of `[WindowStart, event_end)` — the max single-member / household-total
/// trajectory plus the below-`q` crossing bookkeeping.
struct Replay {
    /// The terminal per-member free vector (reconciled against the gate state by the oracle).
    terminal_free: BTreeMap<AgentId, u32>,
    /// Per member: `(peak free, earliest at-or-above-q event index)`.
    member_reached: BTreeMap<AgentId, (u32, usize)>,
    /// Per member: the LAST ≥q→<q transition before the gate.
    member_last_cross: BTreeMap<AgentId, (usize, EventCause)>,
    /// The max household-total free over the window.
    household_peak: u32,
    /// Whether the household total ever reached ≥q.
    household_reached: bool,
    /// The household total's LAST ≥q→<q transition before the gate.
    household_last_cross: Option<(usize, EventCause)>,
}

/// Replay the window, counting `window_start` as event 0 (a member already at q is at-q from the
/// outset). Pure.
fn replay(window_start: &[(AgentId, u32)], events: &[BirthGateEvent], q: u32) -> Replay {
    let mut free: BTreeMap<AgentId, u32> = window_start.iter().copied().collect();
    // Event 0 — the baseline: `member_reached[m] = (peak, first at-or-above-q event index)`, the
    // at-q index `usize::MAX` (unreached) until the peak first hits q.
    let mut member_reached: BTreeMap<AgentId, (u32, usize)> = free
        .iter()
        .map(|(&agent, &f)| (agent, (f, if f >= q { 0 } else { usize::MAX })))
        .collect();
    let mut member_last_cross: BTreeMap<AgentId, (usize, EventCause)> = BTreeMap::new();
    let mut household_total = free.values().copied().fold(0u32, u32::saturating_add);
    let mut household_peak = household_total;
    let mut household_reached = household_total >= q;
    let mut household_last_cross: Option<(usize, EventCause)> = None;

    for (idx, event) in events.iter().enumerate() {
        let event_index = idx + 1;
        let prev_total = household_total;
        for &(agent, new_free) in &event.updates {
            let prev = free.get(&agent).copied().unwrap_or(0);
            free.insert(agent, new_free);
            let entry = member_reached
                .entry(agent)
                .or_insert((new_free, usize::MAX));
            if new_free > entry.0 {
                entry.0 = new_free;
            }
            if new_free >= q && entry.1 == usize::MAX {
                entry.1 = event_index;
            }
            if prev >= q && new_free < q {
                member_last_cross.insert(agent, (event_index, event.cause));
            }
        }
        household_total = free.values().copied().fold(0u32, u32::saturating_add);
        household_peak = household_peak.max(household_total);
        if household_total >= q {
            household_reached = true;
        }
        if prev_total >= q && household_total < q {
            household_last_cross = Some((event_index, event.cause));
        }
    }

    Replay {
        terminal_free: free,
        member_reached,
        member_last_cross,
        household_peak,
        household_reached,
        household_last_cross,
    }
}

/// Whether member `m` reached ≥q at some event.
fn reached_q(member_reached: &BTreeMap<AgentId, (u32, usize)>, q: u32, agent: AgentId) -> bool {
    member_reached
        .get(&agent)
        .is_some_and(|&(peak, _)| peak >= q)
}

/// The pure, total birth-gate classifier (§3). Replays `[window_start, gate)` and, taking the
/// recorded PASS/FAIL decision AND the recomputed one, returns exactly one envelope.
pub fn classify_birth_gate_opportunity(
    window_start: &[(AgentId, u32)],
    events: &[BirthGateEvent],
    gate_state: &BirthGateState,
    q: u32,
) -> ClassifiedBirthGateOpportunity {
    let replay = replay(window_start, events, q);
    let gate_household_total = gate_state.household_total();
    let household_peak = replay.household_peak.max(gate_household_total);
    let gap_to_q = q.saturating_sub(household_peak);

    // The recomputed gate pass = terminal max single-member free ≥ q.
    let terminal_max_member = replay.terminal_free.values().copied().max().unwrap_or(0);
    let recomputed_pass = terminal_max_member >= q;
    let recorded_pass = gate_state.recorded_pass;

    let outcome = if recorded_pass && recomputed_pass {
        BirthGateOutcome::GatePassed
    } else if recorded_pass != recomputed_pass {
        BirthGateOutcome::GateDecisionMismatch {
            recorded: recorded_pass,
            recomputed: recomputed_pass,
        }
    } else {
        // Both FAIL — the scientific ladder (first match).
        let drained_members: Vec<AgentId> = gate_state
            .members
            .iter()
            .map(|&(agent, _)| agent)
            .filter(|&agent| reached_q(&replay.member_reached, q, agent))
            .collect();
        if let Some(&member) = drained_members.iter().max_by_key(|&&agent| {
            let (peak, first_at) = replay.member_reached[&agent];
            // Focal tie rule (R1-5): highest peak, then EARLIEST at-or-above-q event, then
            // lowest AgentId. `max_by_key` picks the largest tuple, so invert the last two.
            (peak, std::cmp::Reverse(first_at), std::cmp::Reverse(agent))
        }) {
            let crossing = replay
                .member_last_cross
                .get(&member)
                .map(|&(_, cause)| cause)
                // A member that reached ≥q but is <q at the gate must have crossed; if the drain
                // is somehow unattributed (baseline-only reach with no recorded transition), name
                // the terminal gate read's cause conservatively as Consumption is wrong — instead
                // this cannot happen for a genuine drain, so default to `AskChange` (a reservation
                // lock) only defensively.
                .unwrap_or(EventCause::AskChange);
            BirthGateOutcome::MemberDrainedBeforeGate { crossing, member }
        } else if gate_household_total >= q {
            BirthGateOutcome::SplitAtGate
        } else if replay.household_reached {
            let crossing = replay
                .household_last_cross
                .map(|(_, cause)| cause)
                .unwrap_or(EventCause::AskChange);
            BirthGateOutcome::HouseholdDrainedBeforeGate { crossing }
        } else {
            BirthGateOutcome::NeverReachedQ
        }
    };

    ClassifiedBirthGateOpportunity {
        outcome,
        household_peak,
        gap_to_q,
        gate_household_total,
    }
}

/// Whether the `[WindowStart, gate)` replay reconciles with the authoritative gate state — the
/// terminal per-member free vector equals the gate read (a HARD oracle check, kept pure here).
pub fn birth_gate_replay_reconciles(opportunity: &BirthGateRawOpportunity) -> bool {
    let replay = replay(
        &opportunity.window_start,
        &opportunity.events,
        opportunity.q,
    );
    // The household's members are stable from WindowStart (after death) through the gate — no
    // deaths or new members intervene — so every gate member's terminal replay free must equal the
    // authoritative gate read.
    let gate_free: BTreeMap<AgentId, u32> = opportunity.gate.members.iter().copied().collect();
    gate_free.len() == opportunity.gate.members.len() && replay.terminal_free == gate_free
}

// ===========================================================================================
// The single report reducer (printed, never asserted)
// ===========================================================================================

/// A typed ratio that distinguishes a real 0% from a zero-denominator stratum (`NA`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ratio {
    /// `count / denominator` (denominator > 0).
    Share { count: u64, denominator: u64 },
    /// The denominator is zero — printed `NA`, never `0%`.
    Na,
}

impl Ratio {
    fn new(count: u64, denominator: u64) -> Self {
        if denominator == 0 {
            Ratio::Na
        } else {
            Ratio::Share { count, denominator }
        }
    }

    fn as_f64(self) -> Option<f64> {
        match self {
            Ratio::Share { count, denominator } => Some(count as f64 / denominator as f64),
            Ratio::Na => None,
        }
    }
}

impl std::fmt::Display for Ratio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.as_f64() {
            Some(share) => write!(f, "{share:.3}"),
            None => write!(f, "NA (denominator=0)"),
        }
    }
}

/// The per-stratum tallies the reducer derives from the classified envelopes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BirthGateTally {
    pub opportunities: u64,
    pub passes: u64,
    pub failures: u64,
    pub gate_decision_mismatch: u64,
    pub member_drained: u64,
    pub split_at_gate: u64,
    pub household_drained: u64,
    pub never_reached_q: u64,
    /// The crossing-cause breakdown over MemberDrained + HouseholdDrained.
    pub crossing_causes: BTreeMap<EventCause, u64>,
    /// The absolute `household_peak` distribution over ALL failure opportunities.
    pub peak_hist: BTreeMap<u32, u64>,
    /// The `gap_to_q` distribution over ALL failure opportunities.
    pub gap_hist: BTreeMap<u32, u64>,
}

impl BirthGateTally {
    fn record(&mut self, classified: &ClassifiedBirthGateOpportunity) {
        self.opportunities += 1;
        match classified.outcome {
            BirthGateOutcome::GatePassed => self.passes += 1,
            BirthGateOutcome::GateDecisionMismatch { .. } => {
                self.failures += 1;
                self.gate_decision_mismatch += 1;
            }
            BirthGateOutcome::MemberDrainedBeforeGate { crossing, .. } => {
                self.failures += 1;
                self.member_drained += 1;
                *self.crossing_causes.entry(crossing).or_insert(0) += 1;
                self.record_failure_dist(classified);
            }
            BirthGateOutcome::SplitAtGate => {
                self.failures += 1;
                self.split_at_gate += 1;
                self.record_failure_dist(classified);
            }
            BirthGateOutcome::HouseholdDrainedBeforeGate { crossing } => {
                self.failures += 1;
                self.household_drained += 1;
                *self.crossing_causes.entry(crossing).or_insert(0) += 1;
                self.record_failure_dist(classified);
            }
            BirthGateOutcome::NeverReachedQ => {
                self.failures += 1;
                self.never_reached_q += 1;
                self.record_failure_dist(classified);
            }
        }
    }

    fn record_failure_dist(&mut self, classified: &ClassifiedBirthGateOpportunity) {
        *self.peak_hist.entry(classified.household_peak).or_insert(0) += 1;
        *self.gap_hist.entry(classified.gap_to_q).or_insert(0) += 1;
    }

    /// The failure-conditional share of one outcome (denominator = failures).
    pub fn share(&self, count: u64) -> Ratio {
        Ratio::new(count, self.failures)
    }
}

/// The report — one typed reducer that calls the classifier exactly once per raw opportunity and
/// derives EVERY count/share/histogram from the returned envelope. The sole path to every printed
/// share. Keyed by `(q, arm, producer_type)` (arms NOT merged) plus a per-cell view.
#[derive(Clone, Debug, Default)]
pub struct BirthGateReport {
    /// `(q, arm, producer_type)` → tally (aggregated across seeds).
    pub by_stratum: BTreeMap<(u32, BurdenSavingArm, ClosureClass), BirthGateTally>,
    /// `(q, arm, seed, producer_type)` → tally (the per-cell view).
    pub by_cell: BTreeMap<(u32, BurdenSavingArm, u64, ClosureClass), BirthGateTally>,
    /// The global tally (all opportunities).
    pub global: BirthGateTally,
    /// The per-producer-type global tallies (Miller/Baker split of `global`).
    pub by_type: BTreeMap<ClosureClass, BirthGateTally>,
}

/// The input row for the reducer: a raw opportunity plus its `(q, arm, seed)` cell key.
pub struct BirthGateReportRow<'a> {
    pub q: u32,
    pub arm: BurdenSavingArm,
    pub seed: u64,
    pub opportunity: &'a BirthGateRawOpportunity,
}

impl BirthGateReport {
    /// Build the report from raw opportunities — the SOLE reducer. Calls
    /// [`classify_birth_gate_opportunity`] exactly once per opportunity.
    pub fn from_traces(rows: &[BirthGateReportRow<'_>]) -> Self {
        let mut report = BirthGateReport::default();
        for row in rows {
            let classified = classify_birth_gate_opportunity(
                &row.opportunity.window_start,
                &row.opportunity.events,
                &row.opportunity.gate,
                row.opportunity.q,
            );
            let producer_type = row.opportunity.producer_type;
            report
                .by_stratum
                .entry((row.q, row.arm, producer_type))
                .or_default()
                .record(&classified);
            report
                .by_cell
                .entry((row.q, row.arm, row.seed, producer_type))
                .or_default()
                .record(&classified);
            report.global.record(&classified);
            report
                .by_type
                .entry(producer_type)
                .or_default()
                .record(&classified);
        }
        report
    }
}

// ===========================================================================================
// The independent denominator recount snapshot (stored SEPARATELY from the observer)
// ===========================================================================================

/// A pre-`run_births` snapshot of one producer household's gate inputs, from which the recount
/// independently replays interval + non-empty + size-cap + hunger-ceiling + the exact stock gate —
/// reading NEITHER the observer NOR any `birth_block_*` counter (§4a).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BirthGateRecountSnapshot {
    pub tick: u64,
    pub household: usize,
    pub producer_type: ClosureClass,
    pub q: u32,
    /// `econ_tick >= next_eligible`.
    pub interval_ok: bool,
    pub member_count: usize,
    pub size_cap: usize,
    /// The max member hunger, or `None` if no living members.
    pub max_hunger: Option<u16>,
    pub hunger_ceiling: u16,
    /// The max member free staple stock (the endowment-gate input).
    pub max_member_free: u32,
}

impl BirthGateRecountSnapshot {
    /// Independently replay the gate strata: did this household reach the endowment gate?
    pub fn reached_endowment_gate(&self) -> bool {
        self.interval_ok
            && self.member_count > 0
            && self.member_count < self.size_cap
            && self
                .max_hunger
                .is_some_and(|hunger| hunger <= self.hunger_ceiling)
    }

    /// Independently replay the endowment stock gate (only meaningful if the gate was reached).
    pub fn gate_passed(&self) -> bool {
        self.max_member_free >= self.q
    }
}

// ===========================================================================================
// The runtime accumulator (a `Settlement` field; runtime-only, NEVER serialized)
// ===========================================================================================

/// DH.b-obs runtime accumulator: the per-tick working window + the per-run opportunity/recount
/// tapes. NEVER serialized (behaviorally inert; the tag-35 flag byte is the only digest footprint).
#[derive(Clone, Debug, Default)]
pub(crate) struct BirthGateObs {
    /// The per-tick WindowStart baseline (member → baseline free), cleared each window.
    window_start: BTreeMap<AgentId, u32>,
    /// The per-tick joined events (drained Society events + appended Production/BirthDebit),
    /// cleared each window.
    joined_events: Vec<BirthGateEvent>,
    /// The per-run captured endowment-gate opportunities.
    opportunities: Vec<BirthGateRawOpportunity>,
    /// The per-run independent recount snapshots (stored SEPARATELY from the opportunities).
    recount: Vec<BirthGateRecountSnapshot>,
}

impl BirthGateObs {
    /// Whether `agent` is a tracked producer-house member this window (a partial-borrow helper so
    /// `run_production`'s id-loop appends a `Production` event without a `&mut self` method call —
    /// see the parent module's hoisted flag).
    pub(crate) fn tracks(&self, agent: AgentId) -> bool {
        self.window_start.contains_key(&agent)
    }

    /// Append one `Production` event carrying the executing member's post-recipe free staple stock.
    pub(crate) fn push_production(&mut self, agent: AgentId, free: u32) {
        self.joined_events.push(BirthGateEvent {
            cause: EventCause::Production,
            updates: vec![(agent, free)],
        });
    }
}

impl Settlement {
    /// DH.b-obs: open the per-tick observation window right after death/estate (before
    /// `regenerate_scales`) — snapshot every living producer-house member's baseline free staple
    /// stock and open the Society tape. `active = birth_gate_obs_active()`; when configured but
    /// inactive (force-disabled / non-closed) the window opens inert (no writes). A no-op unless
    /// configured.
    pub(crate) fn birth_gate_obs_begin_window(&mut self) {
        if !self.birth_gate_obs_configured() {
            return;
        }
        let active = self.birth_gate_obs_active();
        let staple = self.birth_food();
        self.birth_gate_obs.window_start.clear();
        self.birth_gate_obs.joined_events.clear();
        if active {
            let members = self.birth_gate_obs_member_ids();
            for id in members {
                let free = self.society.free_stock_after_all_reserves(id, staple);
                self.birth_gate_obs.window_start.insert(id, free);
            }
        }
        self.society.begin_staple_obs_window(staple, active);
    }

    /// The live producer-house member AgentIds (the domain of the WindowStart baseline + the tape
    /// filter). On the closed base every household is a producer household.
    fn birth_gate_obs_member_ids(&self) -> Vec<AgentId> {
        self.live_colonist_slots
            .iter()
            .filter_map(|&slot| {
                let colonist = &self.colonists[slot];
                colonist
                    .household
                    .is_some_and(|h| self.is_producer_household(h))
                    .then_some(colonist.id)
            })
            .collect()
    }

    /// DH.b-obs: drain the Society tape immediately after the step and disable recording, folding
    /// each atomic Society event into the joined tape (filtered to producer-house members). A no-op
    /// unless active.
    pub(crate) fn birth_gate_obs_drain_after_step(&mut self) {
        if !self.birth_gate_obs_configured() {
            return;
        }
        let events = self.society.take_staple_stock_events();
        self.society.end_staple_obs_window();
        if !self.birth_gate_obs_active() {
            return;
        }
        let members: BTreeSet<AgentId> = self.birth_gate_obs.window_start.keys().copied().collect();
        for event in events {
            let joined = match event {
                StapleStockEvent::Consumption { agent, state } => {
                    Self::single_member_event(EventCause::Consumption, agent, state.free, &members)
                }
                StapleStockEvent::AskChange { agent, state } => {
                    Self::single_member_event(EventCause::AskChange, agent, state.free, &members)
                }
                StapleStockEvent::SettledTrade {
                    seller,
                    seller_state,
                    buyer,
                    buyer_state,
                } => {
                    let mut updates = Vec::new();
                    if members.contains(&seller) {
                        updates.push((seller, seller_state.free));
                    }
                    if members.contains(&buyer) {
                        updates.push((buyer, buyer_state.free));
                    }
                    (!updates.is_empty()).then_some(BirthGateEvent {
                        cause: EventCause::SettledTradeSell,
                        updates,
                    })
                }
            };
            if let Some(joined) = joined {
                self.birth_gate_obs.joined_events.push(joined);
            }
        }
    }

    fn single_member_event(
        cause: EventCause,
        agent: AgentId,
        free: u32,
        members: &BTreeSet<AgentId>,
    ) -> Option<BirthGateEvent> {
        members.contains(&agent).then_some(BirthGateEvent {
            cause,
            updates: vec![(agent, free)],
        })
    }

    /// DH.b-obs: append the post-debit BirthDebit event for a birth (tape completeness; it is
    /// post-`event_end`, so it never enters any classification). A no-op unless active.
    pub(crate) fn birth_gate_obs_append_birth_debit(&mut self, parent: AgentId) {
        if !self.birth_gate_obs_active() || !self.birth_gate_obs.window_start.contains_key(&parent)
        {
            return;
        }
        let staple = self.birth_food();
        let free = self.society.free_stock_after_all_reserves(parent, staple);
        self.birth_gate_obs.joined_events.push(BirthGateEvent {
            cause: EventCause::BirthDebit,
            updates: vec![(parent, free)],
        });
    }

    /// DH.b-obs: capture one endowment-gate opportunity for a producer household that reached the
    /// gate (interval/non-empty/size-cap/hunger-ceiling passed), INSIDE `run_births`, immediately
    /// before parent selection's debit. `recorded_pass` = a parent could endow (a birth follows).
    pub(crate) fn birth_gate_obs_capture_opportunity(
        &mut self,
        household: usize,
        member_slots: &[usize],
        q: u32,
        recorded_pass: bool,
    ) {
        if !self.birth_gate_obs_active() {
            return;
        }
        let Some(producer_type) = self.closure.household_class.get(&household).copied() else {
            return;
        };
        let staple = self.birth_food();
        let members: Vec<AgentId> = member_slots.iter().map(|&s| self.colonists[s].id).collect();
        let member_set: BTreeSet<AgentId> = members.iter().copied().collect();
        // The window baseline restricted to this household's members.
        let window_start: Vec<(AgentId, u32)> = members
            .iter()
            .map(|&id| {
                (
                    id,
                    self.birth_gate_obs
                        .window_start
                        .get(&id)
                        .copied()
                        .unwrap_or(0),
                )
            })
            .collect();
        // The joined events up to the gate (`event_end = joined_events.len()`), filtered to this
        // household's members.
        let events: Vec<BirthGateEvent> = self
            .birth_gate_obs
            .joined_events
            .iter()
            .filter_map(|event| {
                let updates: Vec<(AgentId, u32)> = event
                    .updates
                    .iter()
                    .copied()
                    .filter(|(agent, _)| member_set.contains(agent))
                    .collect();
                (!updates.is_empty()).then_some(BirthGateEvent {
                    cause: event.cause,
                    updates,
                })
            })
            .collect();
        let gate_members: Vec<(AgentId, u32)> = members
            .iter()
            .map(|&id| (id, self.society.free_stock_after_all_reserves(id, staple)))
            .collect();
        self.birth_gate_obs
            .opportunities
            .push(BirthGateRawOpportunity {
                tick: self.econ_tick,
                household,
                producer_type,
                q,
                window_start,
                events,
                gate: BirthGateState {
                    members: gate_members,
                    recorded_pass,
                },
            });
    }

    /// DH.b-obs: the independent recount snapshot pass, over ALL producer households, taken BEFORE
    /// `run_births` mutates anything. Stored SEPARATELY from the observer opportunities. A no-op
    /// unless active.
    pub(crate) fn capture_birth_gate_recount(&mut self) {
        if !self.birth_gate_obs_active() {
            return;
        }
        let Some(demo) = self.demography.clone() else {
            return;
        };
        let staple = self.birth_food();
        let mut snapshots = Vec::new();
        for h in 0..demo.households.len() {
            if !self.is_producer_household(h) {
                continue;
            }
            let Some(producer_type) = self.closure.household_class.get(&h).copied() else {
                continue;
            };
            let next_eligible = self.households[h]
                .last_birth_tick
                .map_or(demo.birth_interval, |t| t + demo.birth_interval);
            let member_slots: Vec<usize> = self
                .live_colonist_slots
                .iter()
                .copied()
                .filter(|&slot| self.colonists[slot].household == Some(h))
                .collect();
            let max_hunger = member_slots
                .iter()
                .map(|&slot| self.colonists[slot].need.hunger)
                .max();
            let max_member_free = member_slots
                .iter()
                .map(|&slot| {
                    self.society
                        .free_stock_after_all_reserves(self.colonists[slot].id, staple)
                })
                .max()
                .unwrap_or(0);
            snapshots.push(BirthGateRecountSnapshot {
                tick: self.econ_tick,
                household: h,
                producer_type,
                q: demo.child_food_endowment,
                interval_ok: self.econ_tick >= next_eligible,
                member_count: member_slots.len(),
                size_cap: self.birth_cap_for_household(h, demo.max_household_size),
                max_hunger,
                hunger_ceiling: demo.birth_hunger_ceiling,
                max_member_free,
            });
        }
        self.birth_gate_obs.recount.extend(snapshots);
    }

    /// DH.b-obs: the captured endowment-gate opportunities for this run (runtime-only, read by the
    /// harness). NEVER serialized, NEVER read by a decision path.
    pub fn birth_gate_opportunities(&self) -> &[BirthGateRawOpportunity] {
        &self.birth_gate_obs.opportunities
    }

    /// DH.b-obs: the independent recount snapshots for this run (stored SEPARATELY from the
    /// opportunities). Runtime-only.
    pub fn birth_gate_recount_snapshots(&self) -> &[BirthGateRecountSnapshot] {
        &self.birth_gate_obs.recount
    }
}

// ===========================================================================================
// The shared DH.b / DH.b-obs harness (§4a) — the SAME real pipeline both suites drive
// ===========================================================================================

const PRODUCER_FOUNDERS: usize = 6;

/// The cell key `(q, arm, seed)`.
pub type BirthGateCellKey = (u32, BurdenSavingArm, u64);

/// The observed per-cell bundle (present only when `run_burden_grid(true)`): the captured
/// opportunities, the independent recount snapshots (stored SEPARATELY), and the run's producer
/// birth + endowment-block counters (for the recount equality set).
#[derive(Clone, Debug)]
pub struct BirthGateCellObs {
    pub opportunities: Vec<BirthGateRawOpportunity>,
    pub recount: Vec<BirthGateRecountSnapshot>,
    pub births: u64,
    pub birth_block_endowment: u64,
}

/// One cell's run summary — the diagnostic figures both suites print and guard on.
#[derive(Clone, Debug)]
pub struct BurdenCellRunSummary {
    pub q: u32,
    pub arm: BurdenSavingArm,
    pub seed: u64,
    pub verdict: BurdenCellVerdict,
    pub founder_deaths: Vec<(AgentId, u64)>,
    pub start: u64,
    pub windows: Vec<BurdenWindowObs>,
    pub births: usize,
    pub guard_failures: Vec<String>,
}

/// The whole-grid result (§4a). `opportunities_by_cell` is `None` for `observe=false` and `Some`
/// (a possibly-empty map) for `observe=true`; the recount is carried SEPARATELY.
#[derive(Clone, Debug)]
pub struct BurdenGridResult {
    pub cells: Vec<BurdenCellResult>,
    pub births_by_cell: BTreeMap<BirthGateCellKey, usize>,
    pub audit_rows: Vec<String>,
    pub synthesis: BurdenSynthesis,
    pub paired_table: Vec<String>,
    pub cell_runs: Vec<BurdenCellRunSummary>,
    pub any_guard_failure: bool,
    pub wall_secs: f64,
    /// Per-cell captured raw opportunities WITH producer-type metadata (`Some` iff `observe`).
    pub opportunities_by_cell: Option<BTreeMap<BirthGateCellKey, Vec<BirthGateRawOpportunity>>>,
    /// Per-cell recount bundle, stored SEPARATELY from the opportunities (`Some` iff `observe`).
    pub recount_by_cell: Option<BTreeMap<BirthGateCellKey, BirthGateCellObs>>,
}

/// The per-cell config: `frontier_closed_circulation()` + EXACTLY {child_food_endowment=q, the
/// two-field saving arm}, plus the pure-observation `birth_gate_obs` flag when observing (which
/// changes NO behavior). q=4 stays canonical; the sweep lives only in the harness.
fn cell_config(q: u32, arm: BurdenSavingArm, observe: bool) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_closed_circulation();
    cfg.demography
        .as_mut()
        .expect("the closed base carries demography")
        .child_food_endowment = q;
    let chain = cfg.chain.as_mut().expect("the closed base carries a chain");
    if arm == BurdenSavingArm::On {
        chain.birth_stock_saving = true;
        chain.birth_stock_saving_mode = BirthStockSavingMode::Motive;
    }
    chain.birth_gate_obs = observe;
    cfg
}

struct CellRunFull {
    verdict: BurdenCellVerdict,
    founder_deaths: Vec<(AgentId, u64)>,
    start: u64,
    windows: Vec<BurdenWindowObs>,
    births: usize,
    guard_failures: Vec<String>,
    obs: Option<BirthGateCellObs>,
}

/// Run one cell through the REAL burden pipeline (identical to the landed DH.b `run_cell`), and,
/// when observing, collect the birth-gate opportunities + recount.
fn run_burden_cell(q: u32, arm: BurdenSavingArm, seed: u64, observe: bool) -> CellRunFull {
    let cfg = cell_config(q, arm, observe);
    let mut s = Settlement::generate(seed, &cfg);
    let mut guards: Vec<String> = Vec::new();

    // Founders: the generation-time closure-registry snapshot of the six Miller/Baker AgentIds.
    let founders: BTreeMap<AgentId, ClosureClass> = s
        .closure_registry()
        .iter()
        .filter(|(_, &class)| matches!(class, ClosureClass::Miller | ClosureClass::Baker))
        .map(|(&id, &class)| (id, class))
        .collect();
    if founders.len() != PRODUCER_FOUNDERS {
        guards.push(format!(
            "founder snapshot: want {PRODUCER_FOUNDERS} producer founders, got {}",
            founders.len()
        ));
    }
    let (mill_good, oven_good): (GoodId, GoodId) = {
        let chain = cfg.chain.as_ref().expect("chain");
        (chain.content.mill(), chain.content.oven())
    };

    let mut founder_alive: BTreeMap<AgentId, bool> =
        founders.keys().map(|&id| (id, true)).collect();
    let mut founder_deaths: Vec<(AgentId, u64)> = Vec::new();
    let mut fixed_class: BTreeMap<AgentId, ClosureClass> = BTreeMap::new();
    let mut continuity_by_tick: Vec<[bool; 2]> = Vec::with_capacity(BURDEN_RUN_TICKS as usize);
    let mut conserved = true;
    let mut money_ok = true;

    for tick in 0..BURDEN_RUN_TICKS {
        let report = s.econ_tick();
        conserved &= report.conserves();
        money_ok &= report.money_conserves();

        let living = s.burden_living_snapshot();
        let born: BTreeSet<AgentId> = s.burden_birth_events().iter().map(|b| b.child).collect();
        let mut continuity = [false; 2];
        for &(id, class, has_lifespan) in &living {
            let Some(class) = class else {
                guards.push(format!(
                    "registry invariant: living {id:?} has no class (t={tick})"
                ));
                continue;
            };
            match fixed_class.get(&id) {
                Some(&prior) if prior != class => guards.push(format!(
                    "registry invariant: {id:?} class changed {prior:?}->{class:?} (t={tick})"
                )),
                None => {
                    fixed_class.insert(id, class);
                }
                _ => {}
            }
            let producer = matches!(class, ClosureClass::Miller | ClosureClass::Baker);
            if producer && !has_lifespan {
                guards.push(format!(
                    "immortal producer {id:?} in class {class:?} (t={tick})"
                ));
            }
            if producer && !founders.contains_key(&id) && born.contains(&id) {
                let slot = match class {
                    ClosureClass::Miller => 0,
                    _ => 1,
                };
                continuity[slot] = true;
            }
        }
        continuity_by_tick.push(continuity);

        for (&fid, alive) in founder_alive.iter_mut() {
            if *alive && !living.iter().any(|&(id, _, _)| id == fid) {
                *alive = false;
                founder_deaths.push((fid, tick));
            }
        }

        if s.acquisition_intervention_held() != 0 {
            guards.push(format!("intervention-origin stock held (t={tick})"));
        }
        for v in s.burden_lot_audit() {
            guards.push(format!("lot audit (t={tick}): {v}"));
        }
    }
    if !conserved {
        guards.push("conservation failed".to_string());
    }
    if !money_ok {
        guards.push("money conservation failed".to_string());
    }
    for v in s.burden_seam_violations() {
        guards.push(format!("seam violation: {v}"));
    }
    if s.closure_event_tape()
        .iter()
        .any(|e| matches!(e.kind, ClosureEventKind::BSupportCredit { .. }))
    {
        guards.push("support-origin credit on the closed base".to_string());
    }

    let wants = s.birth_stock_wants_emitted();
    match (arm, q) {
        (BurdenSavingArm::On, q) if q > 0 => {
            if wants == 0 {
                guards.push(format!("saving arm On with q={q} emitted zero wants"));
            }
        }
        _ => {
            if wants != 0 {
                guards.push(format!("arm {arm:?} q={q} emitted {wants} wants (want 0)"));
            }
        }
    }

    if founder_deaths.len() != founders.len() {
        guards.push(format!(
            "founder extinction: {}/{} founders died within the run",
            founder_deaths.len(),
            founders.len()
        ));
    }

    let births = s.burden_birth_events();
    let funding = s.burden_birth_funding_records();
    let birth_ids: BTreeSet<AgentId> = births.iter().map(|b| b.child).collect();
    let funding_ids: BTreeSet<AgentId> = funding.iter().map(|f| f.child).collect();
    if birth_ids.len() != births.len() {
        guards.push("duplicate BirthOccurred child id".to_string());
    }
    if funding_ids.len() != funding.len() {
        guards.push("duplicate funding-record child id".to_string());
    }
    if births.len() != funding.len() || birth_ids != funding_ids {
        guards.push(format!(
            "stream completeness: {} BirthOccurred vs {} funding records",
            births.len(),
            funding.len()
        ));
    }
    for f in funding {
        if f.q != q {
            guards.push(format!("funding record q={} in a q={q} cell", f.q));
        }
        let drawn: u64 = f.lots.iter().map(|l| l.qty).sum();
        if q > 0 && drawn != u64::from(q) {
            guards.push(format!(
                "funding record for {:?} drew {drawn} units, want q={q}",
                f.child
            ));
        }
        if classify_birth_funding(&f.lots, s.burden_trade_records()).unverifiable {
            guards.push(format!(
                "funding record for {:?} is unverifiable live telemetry",
                f.child
            ));
        }
    }

    let last_death = founder_deaths.iter().map(|&(_, t)| t).max().unwrap_or(0);
    let start = burden_start_tick(last_death);
    let windows = build_burden_windows(
        start,
        BURDEN_RUN_TICKS,
        &continuity_by_tick,
        s.burden_stage_executions(),
    );

    let verdict = if guards.is_empty() {
        let input = BurdenCellInput {
            q,
            mill_good,
            oven_good,
            windows: windows.clone(),
            inheritances: s.burden_tool_inheritances().to_vec(),
            adoptions: s.burden_role_adoptions().to_vec(),
            executions: s.burden_stage_executions().to_vec(),
            births: births
                .iter()
                .map(|b| BurdenBirthObs {
                    tick: b.tick,
                    child: b.child,
                    funding: {
                        let f = funding
                            .iter()
                            .find(|f| f.child == b.child)
                            .expect("completeness guard passed");
                        classify_birth_funding(&f.lots, s.burden_trade_records())
                    },
                })
                .collect(),
        };
        classify_burden_cell(&input)
    } else {
        BurdenCellVerdict::PreconditionInvalid {
            guard: guards.join("; "),
        }
    };

    let births_count = births.len();
    let obs = observe.then(|| BirthGateCellObs {
        opportunities: s.birth_gate_opportunities().to_vec(),
        recount: s.birth_gate_recount_snapshots().to_vec(),
        births: s.burden_birth_events().len() as u64,
        birth_block_endowment: s.birth_block_endowment(),
    });

    CellRunFull {
        verdict,
        founder_deaths,
        start,
        windows,
        births: births_count,
        guard_failures: guards,
        obs,
    }
}

/// The shared 60-cell harness (§4a): serial, seeds outermost, q ascending, Off-then-On — the exact
/// landed DH.b loop order, run through the REAL `classify_burden_cell` / `synthesize_burden_grid`.
/// DH.b calls it `observe=false` (golden unchanged); DH.b-obs calls it ONCE `observe=true` and
/// DERIVES the same golden from the returned structure while feeding `opportunities_by_cell` into
/// the report. The grid is executed ONCE.
pub fn run_burden_grid(observe: bool) -> BurdenGridResult {
    let wall = Instant::now();
    let mut cells: Vec<BurdenCellResult> = Vec::new();
    let mut births_by_cell: BTreeMap<BirthGateCellKey, usize> = BTreeMap::new();
    let mut audit_rows: Vec<String> = Vec::new();
    let mut cell_runs: Vec<BurdenCellRunSummary> = Vec::new();
    let mut any_guard_failure = false;
    let mut opportunities_by_cell: BTreeMap<BirthGateCellKey, Vec<BirthGateRawOpportunity>> =
        BTreeMap::new();
    let mut recount_by_cell: BTreeMap<BirthGateCellKey, BirthGateCellObs> = BTreeMap::new();

    for &seed in &BURDEN_SEEDS {
        for &q in &BURDEN_QS {
            for arm in [BurdenSavingArm::Off, BurdenSavingArm::On] {
                let run = run_burden_cell(q, arm, seed, observe);
                if !run.guard_failures.is_empty() {
                    any_guard_failure = true;
                }
                births_by_cell.insert((q, arm, seed), run.births);
                audit_rows.push(format!(
                    "audit_cell|seed={seed}|q={q}|arm={arm:?}|births={}|rung={}|verdict={:?}",
                    run.births,
                    run.verdict.rung(),
                    run.verdict
                ));
                cells.push(BurdenCellResult {
                    q,
                    arm,
                    seed,
                    verdict: run.verdict.clone(),
                });
                if let Some(obs) = run.obs {
                    opportunities_by_cell.insert((q, arm, seed), obs.opportunities.clone());
                    recount_by_cell.insert((q, arm, seed), obs);
                }
                cell_runs.push(BurdenCellRunSummary {
                    q,
                    arm,
                    seed,
                    verdict: run.verdict,
                    founder_deaths: run.founder_deaths,
                    start: run.start,
                    windows: run.windows,
                    births: run.births,
                    guard_failures: run.guard_failures,
                });
            }
        }
    }

    // The paired births table (delta = On - Off), all six burdens including the q=0 control.
    let mut paired_table: Vec<String> = Vec::new();
    for &q in &BURDEN_QS {
        let off: Vec<usize> = BURDEN_SEEDS
            .iter()
            .map(|&seed| births_by_cell[&(q, BurdenSavingArm::Off, seed)])
            .collect();
        let on: Vec<usize> = BURDEN_SEEDS
            .iter()
            .map(|&seed| births_by_cell[&(q, BurdenSavingArm::On, seed)])
            .collect();
        let delta: Vec<i64> = on
            .iter()
            .zip(&off)
            .map(|(&on, &off)| on as i64 - off as i64)
            .collect();
        paired_table.push(format!(
            "paired_births|q={q}|seeds={BURDEN_SEEDS:?}|off={off:?}|on={on:?}|delta={delta:?}"
        ));
    }

    let synthesis = synthesize_burden_grid(&cells);

    BurdenGridResult {
        cells,
        births_by_cell,
        audit_rows,
        synthesis,
        paired_table,
        cell_runs,
        any_guard_failure,
        wall_secs: wall.elapsed().as_secs_f64(),
        opportunities_by_cell: observe.then_some(opportunities_by_cell),
        recount_by_cell: observe.then_some(recount_by_cell),
    }
}
