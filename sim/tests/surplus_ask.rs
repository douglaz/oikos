//! impl-76 / C3R.k — the satiated-surplus ask: does a marginal money bid on a *costless*
//! surplus re-coordinate the mortal grain→flour→bread chain past the C3R.j money-satiation
//! wall? A PAIRED, pre-registered experiment (`docs/impl-surplus-ask.md`, §§−0.5/−0).
//!
//! **Measured result — `DOWNSTREAM_NULL` on all five seeds.** The lever DELIVERS: with the
//! flag active a satiated flour holder prices its costless surplus at 1, `fresh_input_ask`
//! reads that price, the Bake margin clears, and an oven heir actually ADOPTS `Baker`. Exact
//! allocation-trace sequence ids prove the gate-only flour ask crosses and fills that same heir
//! on its transition tick; flour then flows into baking, while the OFF control bakes none. The
//! pre-activation realized-state prefix is byte-identical after omitting only the configured
//! future-policy record (the full canonical identity correctly differs before activation). But
//! the chain does NOT sustain: the adopted bakers die out (mortal producers, no hearth), so at
//! the horizon there are zero living bakers, zero baker-class gold, and zero final-window bread.
//! The satiation wall was NOT the sole causal blocker — pricing the costless surplus unblocks
//! the immediate flour-purchase wall, but the mortal chain still collapses downstream.
//!
//! The assertions pin that finding: a legitimate economy change that makes the lever REIGNITE a
//! solvent chain, or that makes the OFF control bake flour on its own, will FAIL this suite and
//! force a re-read of `docs/impl-surplus-ask.md`.

use std::collections::{BTreeMap, BTreeSet};

use econ::agent::AskOutcome;
use econ::society::{AllocationExecutionStatus, AllocationRecord};
// `SurplusAskScope` is reached through the already-public `settlement` module rather than a new
// crate-root re-export, so the change stays inside the locked file set.
use sim::settlement::SurplusAskScope;
use sim::{AgentId, Gold, GoodId, Settlement, SettlementConfig, Vocation};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const MAX_TICKS: u64 = 1_600;
/// The horizon the sustained-chain lens is measured over (matches `baker_roundtrip_2x2.rs`).
const HORIZON: u64 = 1_600;
const FINAL_WINDOW: u64 = 160;
/// A functioning baker produces sustainably: this many loaves in the final window
/// (`baker_roundtrip_2x2.rs::PRODUCE_FLOOR`). A collapsed stage makes ~0.
const PRODUCE_FLOOR: u64 = 300;
/// The Bake recipe: 1 flour in (`FLOUR_PER_BAKE`), 3 bread out (`BREAD_PER_BAKE`).
const BAKE_OUTPUT_QTY: u64 = 3;
const BAKE_INPUT_QTY: u64 = 1;
/// Depth the buyer-willingness formula needs: Bake yields 3 bread but every quote is qty 1, so
/// three resting bread bids back the imputed revenue (`impl-surplus-ask.md` §−0.5 item 8).
const BREAD_DEPTH: usize = 3;

/// The census config: the mortal grain→flour→bread chain with `stale_input_price_fix`
/// EXPLICITLY on (the L2 fresh-input appraisal, without which `InputPriceAbsent` is
/// unreachable and the lever is dead by construction) and `project_input_bids` inherited (an
/// adopted Baker posts a flour bid only when it is on). Both are load-bearing.
/// (Copied from `flour_holder_ask_census.rs:14-30`.)
fn config() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_mortal_producers_heritable();
    let households = &mut cfg
        .demography
        .as_mut()
        .expect("heritable demography")
        .households;
    let producer_start = households.len().checked_sub(6).expect("producer houses");
    for house in &mut households[producer_start..] {
        house.food_provision = 0;
    }
    let chain = cfg.chain.as_mut().expect("chain");
    chain.producer_house_cap = 2;
    chain.mortal_producer_tool_inheritance = true;
    chain.stale_input_price_fix = true;
    cfg
}

fn flour_of(cfg: &SettlementConfig) -> GoodId {
    cfg.chain.as_ref().expect("chain").content.flour()
}
fn bread_of(cfg: &SettlementConfig) -> GoodId {
    cfg.chain.as_ref().expect("chain").content.bread()
}
fn oven_of(cfg: &SettlementConfig) -> GoodId {
    cfg.chain.as_ref().expect("chain").content.oven()
}
fn operating_cost(cfg: &SettlementConfig) -> u64 {
    cfg.chain.as_ref().expect("chain").operating_cost
}

/// Total gold held by the currently-`vocation` colonist class — the solvency signal
/// (`baker_roundtrip_2x2.rs::baker_class_gold`).
fn class_gold(s: &Settlement, vocation: Vocation) -> u64 {
    let mut gold = 0u64;
    for idx in 0..s.population() {
        if s.is_alive(idx) && s.vocation_of(idx) == Some(vocation) {
            if let Some(id) = s.colonist_id(idx) {
                if let Some(agent) = s.society().agents.get(id) {
                    gold = gold.saturating_add(agent.gold.0);
                }
            }
        }
    }
    gold
}

/// The OFF control's first post-Baker-death Bake `InputPriceAbsent` wall tick `W`
/// (the census capture pattern, `flour_holder_ask_census.rs:198-239`).
fn find_wall(seed: u64) -> u64 {
    let cfg = config();
    let oven = oven_of(&cfg);
    let mut s = Settlement::generate(seed, &cfg);
    let founders: Vec<_> = (0..s.population())
        .filter(|&i| s.stock_of(i, oven) > 0)
        .collect();
    assert!(!founders.is_empty(), "seed {seed}: no founder Baker");
    let mut death_seen = false;
    loop {
        s.debug_arm_flour_census();
        let before: Vec<_> = founders
            .iter()
            .copied()
            .filter(|&i| s.is_alive(i))
            .collect();
        s.econ_tick();
        if !death_seen && before.iter().any(|&i| !s.is_alive(i)) {
            death_seen = true;
        }
        if death_seen {
            if let Some(row) = s.debug_take_flour_census() {
                return row.decline_tick;
            }
        }
        assert!(
            s.econ_tick_count() < MAX_TICKS,
            "seed {seed}: no post-Baker-death decline within {MAX_TICKS} ticks"
        );
    }
}

/// Build the CAUSAL-arm ON config: the lever activates at the measured wall `W`, scoped to
/// flour (the single-good causal arm, `impl-surplus-ask.md` §−0 item 4).
fn on_config(w: u64, scope: SurplusAskScope) -> SettlementConfig {
    let mut cfg = config();
    let chain = cfg.chain.as_mut().expect("chain");
    chain.satiated_surplus_ask_at = Some(w);
    chain.satiated_surplus_ask_scope = scope;
    cfg
}

/// The pre-registered outcome buckets (`impl-surplus-ask.md` §−0 item 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    /// A gate-fired flour ask fills a Bake candidate → an ACTUAL Baker adoption → the
    /// sustained-chain lens holds. The lever is causal.
    Reignites,
    /// No gate-fired flour ask delivers into the chain (no flour baked, no adoption) —
    /// tells us nothing about the hypothesis, NOT a null.
    NotDelivered,
    /// Asks fill + an heir adopts + flour flows, but the chain still fails downstream — the
    /// honest causal null.
    DownstreamNull,
}

struct SeedResult {
    outcome: Outcome,
    /// A satiated flour holder the flag flips from decline to `Price(1)` exists at `W`.
    gate_target_present: bool,
    gate_asks_posted: u64,
    gate_ask_crosses: u64,
    gate_ask_fills: u64,
    /// Buyers whose successful gate-ask fill occurred on their actual transition tick to Baker.
    gate_fill_adoptions: u64,
    /// The matched adopting buyers that are recorded oven inheritors.
    gate_fill_heir_adoptions: u64,
    /// Cumulative Bake accepts over the post-`W` window (the appraisal accepted the gate price).
    accepts_delta: u64,
    /// Flour consumed as a Bake input over the window (the ON arm; the causal delivery signal).
    on_flour_baked: u64,
    /// The same measurement on the paired OFF control — the causal trace: it MUST be zero.
    off_flour_baked: u64,
    on_window_bread: u64,
    bakers_end: usize,
    baker_gold: u64,
    miller_gold: u64,
    first_gate_ask_tick: Option<u64>,
    /// Fill-latency lens: ticks from the first gate-fired ask to its first successful fill.
    fill_latency: Option<u64>,
    /// Gate-fired ask limit changes, `(tick, seller_id, limit)`, separating each seller's belief
    /// walk-down from refusal.
    ask_limit_trajectory: Vec<(u64, u64, u64)>,
}

fn sustained(r: &SeedResult) -> bool {
    r.bakers_end > 0
        && r.on_window_bread >= PRODUCE_FLOOR
        && r.baker_gold > 0
        // Gold pools in the Millers under a working chain (`baker_roundtrip_2x2.rs:19-22`), so
        // the distribution leg checks BOTH classes, not baker gold alone.
        && r.miller_gold > 0
}

fn delivered(r: &SeedResult) -> bool {
    // A gate-fired FLOUR ask posted, crossed, and filled the SAME recorded oven heir on the tick
    // that heir actually transitioned to Baker. `Accepts` alone is recorded before the switch
    // can be rejected (`phases.rs:2369-2415`), so it is diagnostic-only.
    r.gate_asks_posted > 0
        && r.gate_ask_crosses > 0
        && r.gate_ask_fills > 0
        && r.gate_fill_adoptions > 0
        && r.gate_fill_heir_adoptions > 0
        && r.on_flour_baked > 0
}

fn downstream_reason(r: &SeedResult) -> &'static str {
    if !delivered(r) {
        "not_delivered"
    } else if r.bakers_end == 0 {
        "no_living_baker_at_horizon"
    } else if r.on_window_bread < PRODUCE_FLOOR {
        "final_window_production_below_floor"
    } else if r.baker_gold == 0 {
        "baker_insolvent"
    } else if r.miller_gold == 0 {
        "miller_insolvent"
    } else {
        "none"
    }
}

fn classify(r: &SeedResult) -> Outcome {
    if !delivered(r) {
        Outcome::NotDelivered
    } else if sustained(r) {
        Outcome::Reignites
    } else {
        Outcome::DownstreamNull
    }
}

/// Exactly one bucket survives precedence — checked as the INVERSE of [`classify`], so a
/// classification that stops honoring delivery → sustain fails here (the pattern of
/// `flour_holder_ask_census.rs::assert_determinate`).
fn assert_determinate(seed: u64, r: &SeedResult) {
    let licensed = match r.outcome {
        Outcome::NotDelivered => !delivered(r),
        Outcome::Reignites => delivered(r) && sustained(r),
        Outcome::DownstreamNull => delivered(r) && !sustained(r),
    };
    assert!(
        licensed,
        "seed {seed}: {:?} is not the precedence outcome of delivered={} sustained={}",
        r.outcome,
        delivered(r),
        sustained(r),
    );
}

/// Whether some living non-`candidate` colonist holds flour whose reservation the lever flips
/// from decline (`None`, off) to `Price(1)` (on) — the satiated costless surplus the gate
/// targets. Read as a pure counterfactual off the live society; steers nothing.
fn gate_target_present(s: &Settlement, flour: GoodId, candidate: Option<sim::AgentId>) -> bool {
    let Some(money) = s.society().current_money_good() else {
        return false;
    };
    for idx in 0..s.population() {
        if !s.is_alive(idx) {
            continue;
        }
        let Some(id) = s.colonist_id(idx) else {
            continue;
        };
        if Some(id) == candidate {
            continue;
        }
        let Some(agent) = s.society().agents.get(id) else {
            continue;
        };
        if agent.stock.get(flour) == 0 {
            continue;
        }
        let off = agent.reservation_ask_for_money(flour, 1, money, false);
        let on = agent.reservation_ask_for_money(flour, 1, money, true);
        if off.is_none() && on == Some(econ::good::Gold(1)) {
            return true;
        }
    }
    false
}

/// Actor-independence (`impl-surplus-ask.md` §3, per-holder): EVERY living flour holder at the wall
/// is at the money-satiation state — its OFF-flag ask outcome is a fully-provided `NoMoneyGain`
/// (not `ProvisioningBreak`, not an actual `Price`). This is the durable form of the C3R.j
/// "actor-independent satiation wall": the wall is not one lucky holder declining while others
/// decline for a different reason (which would leave the gate's traced delivery unattributable to
/// the wall). Reads the OFF projection off the live society; steers nothing.
fn assert_actor_independent_satiation(seed: u64, s: &Settlement, flour: GoodId) {
    let money = s.society().current_money_good().expect("money good");
    let mut holders = 0usize;
    for idx in 0..s.population() {
        if !s.is_alive(idx) {
            continue;
        }
        let Some(id) = s.colonist_id(idx) else {
            continue;
        };
        let Some(agent) = s.society().agents.get(id) else {
            continue;
        };
        if agent.stock.get(flour) == 0 {
            continue;
        }
        holders += 1;
        match agent.reservation_ask_outcome(flour, 1, money, false) {
            AskOutcome::NoMoneyGain {
                lost_rank,
                scale_len,
                in_range_money_wants,
                provided_wants,
                cumulative_required,
                ..
            } => {
                // Fully money-satiated: every in-range money want is already provided …
                assert_eq!(
                    provided_wants, in_range_money_wants,
                    "seed {seed}: flour holder {id:?} declines but is not fully money-satiated \
                     (provided={provided_wants} of in_range={in_range_money_wants}) — the wall is \
                     not actor-independent"
                );
                // … that money demand is REAL, not a vacuous `0 == 0` or a zero-quantity want …
                assert!(
                    in_range_money_wants > 0 && cumulative_required > 0,
                    "seed {seed}: flour holder {id:?} has no real in-range money demand \
                     (in_range={in_range_money_wants} cumulative_required={cumulative_required}) — \
                     `provided == in_range` is vacuous, so the gate does not target this holder"
                );
                // … and parting with the unit drops NO allocation (a costless surplus). These are
                // the outcome-exposed gate fields; the enabled projection below checks the full
                // predicate, including receivable headroom and the Later-want exclusion.
                assert_eq!(
                    lost_rank, scale_len,
                    "seed {seed}: flour holder {id:?} is money-satiated but its surplus is NOT \
                     costless (lost_rank={lost_rank} < scale_len={scale_len}) — the gate does not \
                     target this holder"
                );
            }
            other => panic!(
                "seed {seed}: flour holder {id:?} is not at the money-satiation wall (OFF outcome \
                {other:?}) — the wall is not actor-independent"
            ),
        }
        assert_eq!(
            agent.reservation_ask_for_money(flour, 1, money, true),
            Some(Gold(1)),
            "seed {seed}: flour holder {id:?} satisfies the visible NoMoneyGain gate fields but \
             fails the complete enabled gate (unreceivable minimum price or unexpired Later want)"
        );
    }
    assert!(
        holders > 0,
        "seed {seed}: no living flour holder at the wall — actor-independence is vacuous"
    );
}

fn run_causal(seed: u64, w: u64, scope: SurplusAskScope) -> SeedResult {
    let off_cfg = config();
    let on_cfg = on_config(w, scope);
    let flour = flour_of(&on_cfg);
    let bread = bread_of(&on_cfg);

    let mut off = Settlement::generate(seed, &off_cfg);
    let mut on = Settlement::generate(seed, &on_cfg);

    // A configured future policy belongs in canonical identity before it fires, so compare the
    // complete realized state with only that policy record omitted. This proves the ON run's
    // `[0, W)` STATE prefix is byte-identical to the OFF control without colliding two states that
    // have different future behavior.
    for _ in 0..w {
        off.econ_tick();
        on.econ_tick();
        assert_eq!(
            off.canonical_bytes_without_satiated_surplus_config(),
            on.canonical_bytes_without_satiated_surplus_config(),
            "seed {seed}: the ON [0, W) state prefix diverged from the OFF control before the wall"
        );
    }
    // The appraisal (settlement tick) and the market (society tick) must activate together.
    assert_eq!(
        on.econ_tick_count(),
        on.society().tick.0,
        "seed {seed}: settlement tick and society tick disagree at the wall boundary"
    );
    assert_eq!(on.econ_tick_count(), w, "seed {seed}: prefix ran to W");

    // The wall is actor-independent: every living flour holder is money-satiated (declines off-flag
    // for the NoMoneyGain reason), so the gate targets the whole wall, not a single outlier.
    assert_actor_independent_satiation(seed, &on, flour);
    let gate_target = gate_target_present(&on, flour, None);
    let accepts_before = on.role_choice_diag().bake.accepts;
    // Reuse the exact CDA allocation trace: `SatiatedSurplusAsk` marks gate-only ask seqs and
    // `Execution` identifies the matching buyer/seller, crossing, and settlement outcome.
    on.society_mut().enable_allocation_trace();

    let mut on_flour_baked = 0u64;
    let mut off_flour_baked = 0u64;
    let mut on_window_bread = 0u64;
    let mut gate_asks = BTreeMap::<u64, (AgentId, u64)>::new();
    let mut gate_asks_posted = 0u64;
    let mut gate_ask_crosses = 0u64;
    let mut gate_ask_fills = 0u64;
    let mut gate_fills = Vec::<(u64, AgentId)>::new();
    let mut first_gate_ask_tick = None;
    let mut first_gate_fill_tick = None;
    let mut ask_limit_trajectory = Vec::<(u64, u64, u64)>::new();
    let mut last_gate_limit = BTreeMap::<AgentId, u64>::new();
    let mut baker_ids: BTreeSet<AgentId> = (0..on.population())
        .filter(|&i| on.is_alive(i) && on.vocation_of(i) == Some(Vocation::Baker))
        .filter_map(|i| on.colonist_id(i))
        .collect();
    let mut baker_transitions = BTreeMap::<AgentId, (u64, bool)>::new();
    let remaining = HORIZON - w;
    for k in 0..remaining {
        let ro = on.econ_tick();
        let rf = off.econ_tick();
        let trace = on.society_mut().take_allocation_trace();

        // Gate markers are appended after the ordinary quote/execution records for that attempt;
        // gather every marker first, then join executions by seq.
        for record in &trace {
            if let AllocationRecord::SatiatedSurplusAsk {
                tick,
                seller,
                good,
                seq,
                limit,
            } = *record
            {
                if good != flour {
                    continue;
                }
                gate_asks_posted += 1;
                gate_asks.insert(seq, (seller, tick));
                first_gate_ask_tick =
                    Some(first_gate_ask_tick.map_or(tick, |first: u64| first.min(tick)));
                if last_gate_limit.get(&seller).copied() != Some(limit.0) {
                    ask_limit_trajectory.push((tick, seller.0, limit.0));
                    last_gate_limit.insert(seller, limit.0);
                }
            }
        }
        for record in trace {
            let AllocationRecord::Execution {
                tick,
                incoming_seq,
                resting_seq,
                good,
                buyer,
                seller,
                status,
                ..
            } = record
            else {
                continue;
            };
            if good != flour {
                continue;
            }
            let gate = gate_asks
                .get(&incoming_seq)
                .or_else(|| gate_asks.get(&resting_seq));
            let Some(&(gate_seller, _)) = gate else {
                continue;
            };
            assert_eq!(
                seller, gate_seller,
                "seed {seed}: gate ask seq joined to the wrong seller"
            );
            gate_ask_crosses += 1;
            if status == AllocationExecutionStatus::Succeeded {
                gate_ask_fills += 1;
                gate_fills.push((tick, buyer));
                first_gate_fill_tick =
                    Some(first_gate_fill_tick.map_or(tick, |first: u64| first.min(tick)));
            }
        }

        let fb = ro.consumed_as_input_of(flour);
        on_flour_baked += fb;
        off_flour_baked += rf.consumed_as_input_of(flour);
        let current_bakers: BTreeSet<AgentId> = (0..on.population())
            .filter(|&i| on.is_alive(i) && on.vocation_of(i) == Some(Vocation::Baker))
            .filter_map(|i| on.colonist_id(i))
            .collect();
        for &adopter in current_bakers.difference(&baker_ids) {
            let inherited = on
                .debug_flour_census_row_now(adopter)
                .is_some_and(|row| row.candidate_recorded_inheritor);
            baker_transitions
                .entry(adopter)
                .or_insert((ro.econ_tick, inherited));
        }
        baker_ids = current_bakers;
        if k >= remaining - FINAL_WINDOW {
            on_window_bread += ro.produced_of(bread);
        }
    }
    let adopting_buyers: BTreeSet<AgentId> = gate_fills
        .iter()
        .filter_map(|&(fill_tick, buyer)| {
            baker_transitions
                .get(&buyer)
                .is_some_and(|(transition_tick, _)| *transition_tick == fill_tick)
                .then_some(buyer)
        })
        .collect();
    let gate_fill_adoptions = adopting_buyers.len() as u64;
    let gate_fill_heir_adoptions = adopting_buyers
        .iter()
        .filter(|buyer| {
            baker_transitions
                .get(buyer)
                .is_some_and(|(_, inherited)| *inherited)
        })
        .count() as u64;
    let fill_latency = first_gate_ask_tick
        .zip(first_gate_fill_tick)
        .map(|(ask, fill)| fill.saturating_sub(ask));

    let mut r = SeedResult {
        outcome: Outcome::NotDelivered,
        gate_target_present: gate_target,
        gate_asks_posted,
        gate_ask_crosses,
        gate_ask_fills,
        gate_fill_adoptions,
        gate_fill_heir_adoptions,
        accepts_delta: on.role_choice_diag().bake.accepts - accepts_before,
        on_flour_baked,
        off_flour_baked,
        on_window_bread,
        bakers_end: on.living_count(Vocation::Baker),
        baker_gold: class_gold(&on, Vocation::Baker),
        miller_gold: class_gold(&on, Vocation::Miller),
        first_gate_ask_tick,
        fill_latency,
        ask_limit_trajectory,
    };
    r.outcome = classify(&r);
    r
}

/// The buyer-willingness baseline (OFF, at `W`): each Bake-eligible candidate's max flour bid,
/// derived from EXECUTABLE live bread demand — the third-unit-marginal resting non-self bread
/// bid — NOT `realized_price(bread)` (the arc's oldest over-read). `impl-surplus-ask.md`
/// §3 / §−0.5 item 8.
fn buyer_willingness_baseline(seed: u64) {
    let cfg = config();
    let bread = bread_of(&cfg);
    let oven = oven_of(&cfg);
    let opcost = operating_cost(&cfg);
    let w = find_wall(seed);
    let mut s = Settlement::generate(seed, &cfg);
    for _ in 0..w {
        s.econ_tick();
    }
    // The Bake-eligible candidates at the wall: every living oven holder.
    let candidates: Vec<sim::AgentId> = (0..s.population())
        .filter(|&i| s.is_alive(i) && s.stock_of(i, oven) > 0)
        .filter_map(|i| s.colonist_id(i))
        .collect();
    assert!(
        !candidates.is_empty(),
        "seed {seed}: buyer baseline has no Bake-eligible candidate at W={w}"
    );
    let candidate_count = candidates.len();
    let mut measured = 0usize;
    let mut buyer_willing = 0usize;
    let mut no_executable_surplus_measured = 0usize;
    let mut third_unit_bids = Vec::new();
    let mut max_flour_bids = Vec::new();
    let mut free_tenders = Vec::new();
    for candidate in candidates {
        let bids = s.society().live_non_self_bid_prices(bread, candidate);
        // Executable free tender after every spot/labor/loan/project reservation — never gross
        // gold, which could double-spend a balance already committed to a resting order.
        let free_tender = s.society().free_gold_after_all_reserves(candidate).0;
        if bids.len() < BREAD_DEPTH {
            no_executable_surplus_measured += 1;
            continue;
        }
        measured += 1;
        // The third-unit marginal bread bid (every quote is qty 1, so `bids[2]` is the 3rd unit).
        let bread_bid = bids[BREAD_DEPTH - 1].0;
        // max flour bid = bread_bid × output_qty − operating_cost, ÷ input_qty
        // (matches `imputed_input_reservation`, `mod.rs`).
        let imputed_revenue = bread_bid.saturating_mul(BAKE_OUTPUT_QTY);
        let max_flour_bid = imputed_revenue.saturating_sub(opcost) / BAKE_INPUT_QTY;
        third_unit_bids.push(bread_bid);
        max_flour_bids.push(max_flour_bid);
        free_tenders.push(free_tender);
        if max_flour_bid >= 1 && free_tender >= max_flour_bid {
            buyer_willing += 1;
        }
        // The baseline is computed from a LIVE resting bid, never from `realized_price(bread)`.
        assert!(
            max_flour_bid <= imputed_revenue,
            "seed {seed}: the buyer-willingness formula must subtract the operating cost"
        );
    }
    let classification = if buyer_willing > 0 {
        "BUYER_WILLING"
    } else if measured == 0 {
        "no executable buyer surplus measured"
    } else {
        "BUYER_NOT_WILLING"
    };
    println!(
        "C3R.k buyer_willingness seed={seed} W={w} classification={classification} \
         candidates={candidate_count} measured={measured} willing={buyer_willing} \
         no_depth={no_executable_surplus_measured} bread_bid_3rd_units={third_unit_bids:?} \
         max_flour_bids={max_flour_bids:?} free_tenders={free_tenders:?}",
    );
    // A no-depth seed is explicitly an unmeasured baseline, not a false buyer-willing result.
    // Whenever executable depth exists, the pre-registered buyer-willingness claim must hold for
    // at least one Bake-eligible candidate using genuinely free tender.
    if measured > 0 {
        assert!(
            buyer_willing > 0,
            "seed {seed}: live third-unit bread demand exists, but no candidate can tender a \
             positive imputed flour bid"
        );
    } else {
        assert_eq!(
            no_executable_surplus_measured, candidate_count,
            "seed {seed}: the no-executable-surplus classification is indeterminate"
        );
    }
}

#[test]
fn satiated_surplus_ask_causal() {
    let mut outcomes = Vec::new();
    for seed in SEEDS {
        let w = find_wall(seed);
        let r = run_causal(seed, w, SurplusAskScope::Flour);
        println!(
            "C3R.k causal seed={seed} W={w} outcome={:?} downstream_reason={} gate_target={} \
             gate_posts={} gate_crosses={} gate_fills={} gate_adoptions={} heir_adoptions={} \
             accepts_delta={} \
             on_flour_baked={} off_flour_baked={} on_window_bread={} \
             bakers_end={} baker_gold={} miller_gold={} first_gate_ask={:?} \
             fill_latency={:?} ask_limit_trajectory={:?}",
            r.outcome,
            downstream_reason(&r),
            r.gate_target_present,
            r.gate_asks_posted,
            r.gate_ask_crosses,
            r.gate_ask_fills,
            r.gate_fill_adoptions,
            r.gate_fill_heir_adoptions,
            r.accepts_delta,
            r.on_flour_baked,
            r.off_flour_baked,
            r.on_window_bread,
            r.bakers_end,
            r.baker_gold,
            r.miller_gold,
            r.first_gate_ask_tick,
            r.fill_latency,
            r.ask_limit_trajectory,
        );
        assert_determinate(seed, &r);
        // The control must remain walled. Delivery attribution itself is stricter: `delivered`
        // joins an exact gate-only ask seq to its successful fill and the buyer's real heir-Baker
        // transition, rather than inferring it from the arm-level difference.
        assert_eq!(
            r.off_flour_baked, 0,
            "seed {seed}: the OFF control must stay walled (no flour baked) — else the delivery \
             is not attributable to the lever"
        );
        assert!(
            r.gate_target_present,
            "seed {seed}: the wall has no satiated flour holder the gate targets"
        );
        outcomes.push((seed, r));
    }

    // Every seed is assigned exactly one pre-registered causal bucket. Do not post-hoc promote an
    // arm-level production difference to delivery: only the gate-seq → fill → heir-adoption join
    // licenses DOWNSTREAM_NULL or REIGNITES.
    for (seed, r) in &outcomes {
        assert_determinate(*seed, r);
        assert!(
            delivered(r) && !sustained(r),
            "seed {seed}: measured result changed from DOWNSTREAM_NULL: delivered={} sustained={} \
             posts={} crosses={} fills={} heir_adoptions={} bakers_end={} window_bread={} \
             baker_gold={} miller_gold={}",
            delivered(r),
            sustained(r),
            r.gate_asks_posted,
            r.gate_ask_crosses,
            r.gate_ask_fills,
            r.gate_fill_heir_adoptions,
            r.bakers_end,
            r.on_window_bread,
            r.baker_gold,
            r.miller_gold,
        );
        assert_eq!(r.outcome, Outcome::DownstreamNull);
    }

    // The durable buyer-willingness baseline, computed from executable live bread demand.
    for seed in SEEDS {
        buyer_willingness_baseline(seed);
    }
}

/// Determinism (a): the off-flag (`None`) run is byte-identical tick-by-tick whether the field
/// is left at its constructor default or set to `None` explicitly — proving the field's mere
/// presence and an explicit-`None` both reproduce the base stream (`impl-surplus-ask.md` item 11).
#[test]
fn off_flag_is_byte_identical() {
    for seed in SEEDS {
        let default_cfg = config();
        let mut explicit_cfg = config();
        explicit_cfg
            .chain
            .as_mut()
            .expect("chain")
            .satiated_surplus_ask_at = None;

        let mut a = Settlement::generate(seed, &default_cfg);
        let mut b = Settlement::generate(seed, &explicit_cfg);
        for _ in 0..MAX_TICKS {
            a.econ_tick();
            b.econ_tick();
            assert_eq!(
                a.canonical_bytes(),
                b.canonical_bytes(),
                "seed {seed}: default-None and explicit-None diverged off-flag"
            );
        }
    }
}

/// A configured future activation is part of canonical identity before it fires: two snapshots
/// with different future behavior must not collide. The paired-prefix experiment uses the focused
/// state-only view above rather than weakening this invariant.
#[test]
fn configured_policy_is_canonical_before_activation() {
    let seed = SEEDS[0];
    let off_cfg = config();
    let flour_cfg = on_config(MAX_TICKS, SurplusAskScope::Flour);
    let all_goods_cfg = on_config(MAX_TICKS, SurplusAskScope::AllGoods);

    let off = Settlement::generate(seed, &off_cfg);
    let flour_on = Settlement::generate(seed, &flour_cfg);
    let all_goods_on = Settlement::generate(seed, &all_goods_cfg);

    // The activation tick + scope discriminant both land in the digest: an off config, the
    // Flour arm, and the AllGoods arm are pairwise distinct before activation.
    assert_ne!(off.canonical_bytes(), flour_on.canonical_bytes());
    assert_ne!(flour_on.canonical_bytes(), all_goods_on.canonical_bytes());
    assert_eq!(
        off.canonical_bytes_without_satiated_surplus_config(),
        flour_on.canonical_bytes_without_satiated_surplus_config(),
        "only the configured future policy may distinguish the pre-activation initial state"
    );
}

/// Determinism (b) — the flap-off guard (`impl-surplus-ask.md` §−0 item 1, P0): a gate-fired
/// flour ask SURVIVES a reconciliation pass. If the live-quote change detector recomputed the
/// reservation WITHOUT the lever it would see `None`, judge the quote changed, and cancel the
/// gate-fired ask every pass. Because both ask sites read the SAME per-tick flag, the detector
/// reports it unchanged and the ask keeps resting across the pass.
#[test]
fn gate_fired_ask_survives_reconciliation() {
    let seed = SEEDS[0];
    let w = find_wall(seed);
    let cfg = on_config(w, SurplusAskScope::Flour);
    let flour = flour_of(&cfg);
    let mut s = Settlement::generate(seed, &cfg);
    for _ in 0..w {
        s.econ_tick();
    }
    let money = s.society().current_money_good().expect("money good");

    // Whether the agent is a flour holder the lever flips from decline (`None`, off) to
    // `Price(1)` (on) — a satiated costless-surplus flour state.
    let is_gate_holder = |s: &Settlement, id: sim::AgentId| -> bool {
        s.society().agents.get(id).is_some_and(|agent| {
            agent.stock.get(flour) > 0
                && agent
                    .reservation_ask_for_money(flour, 1, money, false)
                    .is_none()
                && agent.reservation_ask_for_money(flour, 1, money, true)
                    == Some(econ::good::Gold(1))
        })
    };
    let has_live_ask = |s: &Settlement, id: sim::AgentId| {
        s.society()
            .books
            .iter()
            .find(|book| book.good == flour)
            .is_some_and(|book| book.asks.values().any(|order| order.agent == id))
    };

    // Run the live-quote change detector IN ISOLATION over each gate holder (the public
    // reconciliation seam `Society::cancel_changed_live_quotes_for_agent`, the very pass that runs
    // at the start of every market step). It is the cleanest possible flap-off test: no
    // `ensure_ask` re-post, no fills, no expiry, no belief walk between posting and the pass to
    // confound it. A gate-fired ask whose reservation is unchanged survives the pass. Were the
    // detector to recompute the reservation WITHOUT the lever it would see `None`, judge the quote
    // changed, and cancel it EVERY pass regardless of belief — so a single survival proves the
    // detector honors the flag.
    let mut survived = 0usize;
    'outer: for _ in 0..(HORIZON - w) {
        let holders: Vec<sim::AgentId> = (0..s.population())
            .filter(|&i| s.is_alive(i))
            .filter_map(|i| s.colonist_id(i))
            .filter(|&id| is_gate_holder(&s, id) && has_live_ask(&s, id))
            .collect();
        for id in holders {
            s.society_mut().cancel_changed_live_quotes_for_agent(id);
            if has_live_ask(&s, id) {
                survived += 1;
                if survived >= 3 {
                    break 'outer;
                }
            }
        }
        s.econ_tick();
    }
    assert!(
        survived >= 1,
        "seed {seed}: no gate-fired flour ask ever survived an isolated reconciliation pass — \
         the change detector is cancelling gate-fired asks (the lever flaps off), OR no gate-fired \
         ask ever rested (the guard is vacuous)"
    );
}

/// The BLAST-RADIUS arm (`impl-surplus-ask.md` §−0 item 4 / §5): the GENERAL (`AllGoods`) rule
/// opens satiated asks on EVERY good, not just flour. Measured SEPARATELY from the causal claim,
/// on the calibrated IMMORTAL L2 base (`baker_roundtrip_2x2.rs`): if the flag makes that base
/// regress (production collapses, solvency floor breached), that is `DESTABILIZES`.
#[test]
fn blast_radius_all_goods() {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum BlastOutcome {
        Destabilizes,
        NoRegression,
    }

    /// The immortal L2 base (`baker_roundtrip_2x2.rs::config(L2)`): a calibrated chain that
    /// sustains a solvent baker stage. Mortality OFF so a hunger bound is vacuous here.
    fn immortal_base() -> SettlementConfig {
        let mut cfg = SettlementConfig::frontier_mortal_producers_heritable();
        let households = &mut cfg
            .demography
            .as_mut()
            .expect("heritable demography")
            .households;
        let producer_start = households.len().checked_sub(6).expect("producer houses");
        for house in &mut households[producer_start..] {
            house.food_provision = 0;
        }
        let chain = cfg.chain.as_mut().expect("chain");
        chain.producer_house_cap = 2;
        chain.mortal_producer_tool_inheritance = true;
        chain.mortal_chain_producers = false;
        chain.mortal_producer_inheritance = false;
        chain.stale_input_price_fix = true;
        cfg
    }

    /// The sustained-chain lens over `cfg`: living Baker, final-window production, and the
    /// Miller/Baker gold distribution.
    fn run(seed: u64, cfg: &SettlementConfig) -> (usize, u64, u64, u64) {
        let bread = bread_of(cfg);
        let mut s = Settlement::generate(seed, cfg);
        let mut window = 0u64;
        for tick in 0..HORIZON {
            let r = s.econ_tick();
            if tick >= HORIZON - FINAL_WINDOW {
                window = window.saturating_add(r.produced_of(bread));
            }
        }
        (
            s.living_count(Vocation::Baker),
            window,
            class_gold(&s, Vocation::Baker),
            class_gold(&s, Vocation::Miller),
        )
    }

    for seed in SEEDS {
        let base = immortal_base();
        // The general rule from an early tick (50), so it is live for the whole run.
        let mut on = immortal_base();
        {
            let chain = on.chain.as_mut().expect("chain");
            chain.satiated_surplus_ask_at = Some(50);
            chain.satiated_surplus_ask_scope = SurplusAskScope::AllGoods;
        }
        let (base_bakers, base_window, base_baker_gold, base_miller_gold) = run(seed, &base);
        let (on_bakers, on_window, on_baker_gold, on_miller_gold) = run(seed, &on);
        // The calibrated base sustains; the flag DESTABILIZES it if it regresses below the lens.
        let base_ok = base_bakers > 0
            && base_window >= PRODUCE_FLOOR
            && base_baker_gold > 0
            && base_miller_gold > 0;
        let on_ok =
            on_bakers > 0 && on_window >= PRODUCE_FLOOR && on_baker_gold > 0 && on_miller_gold > 0;
        let outcome = if base_ok && !on_ok {
            BlastOutcome::Destabilizes
        } else {
            BlastOutcome::NoRegression
        };
        println!(
            "C3R.k blast_radius seed={seed} scope=AllGoods outcome={outcome:?} \
             base=(bakers={base_bakers},window={base_window},baker_gold={base_baker_gold},\
             miller_gold={base_miller_gold}) \
             on=(bakers={on_bakers},window={on_window},baker_gold={on_baker_gold},\
             miller_gold={on_miller_gold})",
        );
        assert!(
            base_ok,
            "seed {seed}: the immortal base must itself sustain for the blast-radius arm to mean \
             anything (control regressed): bakers={base_bakers} window={base_window} \
             baker_gold={base_baker_gold} miller_gold={base_miller_gold}"
        );
        // Pre-registered result: the GENERAL (AllGoods) rule DESTABILIZES the calibrated immortal
        // base on every seed (`impl-surplus-ask.md` §−0 item 7 / §5). Pin it — a legitimate change
        // that makes AllGoods stop breaking the base (or that regresses the base itself) FAILS
        // here and forces a re-read, exactly as the causal arm pins DOWNSTREAM_NULL.
        assert_eq!(
            outcome,
            BlastOutcome::Destabilizes,
            "seed {seed}: AllGoods no longer destabilizes the immortal base — the blast-radius \
             finding changed: on=(bakers={on_bakers},window={on_window},baker_gold={on_baker_gold},\
             miller_gold={on_miller_gold})"
        );
    }
}
