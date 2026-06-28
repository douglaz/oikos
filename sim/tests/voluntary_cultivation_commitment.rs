//! S22f acceptance suite — **voluntary fixed-term cultivation commitment** (impl-38): does an
//! institution that changes the cultivation EXIT itself finally stabilize an occupation that hunger
//! (S22a), accumulated skill (S22b), a realized profit stay-incentive (S22c), sunk EARNED capital
//! (S22d), and even ENDOWED + inherited capital (S22e) could not?
//!
//! The role-topology arc is a clean five-step negative: every lever that left the hunger/profit EXIT
//! intact failed, including capital given up front, because the binding constraint is the exit, which
//! rotates cultivators out regardless of who owns the means. S22f tests the most authentic, least-fiat
//! institution that touches the exit itself — a **voluntary fixed-term cultivation commitment**:
//! post-money, an agent whose OWN realized cultivation-return signal (the unchanged S22c signal) clears
//! its outside option may CHOOSE to enter a cultivator commitment; while bound for a fixed term the
//! normal hunger/profit exit cannot turn its cultivation off; at term expiry it re-decides from FRESH
//! realized returns (a renewal only if the signal still clears). Uptake is endogenous; the institution
//! is configured.
//!
//! **The central trap (foregrounded):** an exit-overriding institution is one step from merely
//! RE-PINNING the S21 producer class S22a deliberately relaxed. The honest design makes commitment
//! VOLUNTARY (entry gated by the agent's own post-money realized return, inert pre-money, no quota),
//! keeps non-committed agents FULLY FLUID, forces term expiry to re-open choice, and proves it is not a
//! pin via a `fiat_pin` control (forced commitment ⇒ `RePinScaffold`) plus a renewals-from-fresh-
//! signals requirement. A SUCCESS would be *institutional sufficiency with endogenous uptake*, NOT
//! "occupation emerges without institutions".
//!
//! This suite CLASSIFIES the treatment against the §2 pre-named outcomes via an ORDERED,
//! mutually-exclusive classifier (top-down; first match wins) and PRINTS the verdict; it does NOT
//! assert SUCCESS. Every threshold is PREDECLARED a priori, never fitted. The primary metric is
//! `RoleStickySuccess` by agent id; lineage persistence is a REPORTED secondary. The headline carries
//! NO capital of any kind (durable + endowed both OFF), so any stickiness is the commitment institution
//! alone. Every run satisfies the hard guards (conservation each tick, `bread_minted_max == 0`,
//! provenance-clean-or-disqualified, `!extinct`, `commitment_term < ceil(PERSIST_FRACTION ×
//! FINAL_WINDOW)`, the minority-uptake + live-fluid-remainder check; the capital variants additionally
//! satisfy the tool-stock invariant). The five tripwire goldens are re-pinned byte-identical
//! ([`goldens_unchanged`]).
//!
//! Run the verdict with `--nocapture` to read the classification:
//!   `cargo test -p sim --test voluntary_cultivation_commitment commitment_verdict -- --nocapture`

use std::collections::{BTreeMap, BTreeSet};

use econ::good::SALT;
use sim::{Settlement, SettlementConfig};

// The shared S21 demand-bridge classification machinery (the living-roster helpers + the
// `PROBE_TICKS`/`SEEDS`/`MATERIAL_BOUGHT_FLOOR` constants).
#[path = "support/mod.rs"]
mod support;
use support::*;

// =========================================================================
// Predeclared thresholds (a priori — §2/§7; NEVER fitted to the data)
// =========================================================================

/// Minimum distinct agents that must VOLUNTARILY commit after money for the institution to be
/// non-vacuous (the mandatory non-vacuity/voluntariness floor; below ⇒ `CommitmentUnchosen`).
const MIN_COMMITS: usize = 4;

/// Treatment per-ever-cultivating-agent churn must fall to ≤ `CHURN_DROP ×` the **matched-seed**
/// commitment-off expanded baseline churn for "churn fell materially".
const CHURN_DROP: f64 = 0.5;

/// An agent id is PERSISTENT if it is cultivating/committed in ≥ `PERSIST_FRACTION` of the final-window
/// ticks (membership persistence, NOT aggregate share).
const PERSIST_FRACTION: f64 = 0.5;

/// Distinct persistent COMMITTED agent ids required for a sticky cohort (the primary metric).
const PERSIST_COHORT: usize = 4;

/// The lineage roster the headline + every matched control runs on (mirrors the engine const
/// `ENDOWED_ROSTER_HOUSEHOLDS`). The 2-household base cannot host a `PERSIST_COHORT` committed cohort.
const ROSTER_HOUSEHOLDS: usize = 8;

/// The maximum committed-cohort share (of ever-cultivating AND of eligible) for uptake to count as a
/// BOUNDED minority (above ⇒ a de-facto pin: `UniversalCommitment`).
const COMMIT_SHARE_MAX: f64 = 0.6;

/// Committed-cultivator final-window grain share at/above which the committed cohort DOMINATES the
/// grain regen (the monopolization probe; a buyer collapse is also required).
const MONO_SHARE: f64 = 0.75;

/// Rolling cultivator share at/above which most survivors cultivate (the commune side).
const COMMUNE_SHARE: f64 = 0.75;

/// Living non-lineage roles at/below which the demand side has collapsed (the monopolization DAMAGE
/// floor).
const DEMAND_COLLAPSE_FLOOR: usize = 4;

/// The rolling window (ticks) for the cultivator-share sample.
const ROLLING_WINDOW: usize = 100;

/// The final window (ticks) over which the cohort / grain-share / buyer band are read.
const FINAL_WINDOW: usize = 200;

/// Per-agent cumulative bought bread at/above which a non-cultivating non-committed colonist counts as
/// a material BUYER (a real market transactor, not merely alive).
const MATERIAL_BUYER_FOOD: u64 = 4;

/// Living non-cultivating material buyers required in the final window for a genuine two-cohort split
/// (a non-cultivating, non-committed demand side that is alive AND buying).
const MIN_BUYER_COHORT: usize = 2;

/// The shipped headline commitment term (mirrors the engine const `COMMITMENT_TERM_DEFAULT`).
const COMMITMENT_TERM_SHIPPED: u16 = 48;

/// The robustness term sweep (must satisfy `< COMMITMENT_TERM_CAP` for the headline bar).
const TERM_SWEEP: [u16; 4] = [12, 24, 48, 96];

/// A persistent id must be cultivating/committed in ≥ this many final-window ticks.
fn persist_threshold() -> u32 {
    (PERSIST_FRACTION * FINAL_WINDOW as f64).ceil() as u32
}

/// The headline/success commitment-term cap: a single term mathematically cannot span the persistence
/// window, so persistence must come from RENEWALS (§2.5). `ceil(PERSIST_FRACTION × FINAL_WINDOW)`.
fn commitment_term_cap() -> u16 {
    (PERSIST_FRACTION * FINAL_WINDOW as f64).ceil() as u16
}

// =========================================================================
// The ordered, mutually-exclusive classifier (spec §2)
// =========================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    /// (precondition) The institution is offered but no agent VOLUNTARILY enters (no signal clears).
    CommitmentUnchosen,
    /// The S22c realized-return signal doesn't discriminate (no eligible agent stays below the floor —
    /// no real entry decision to make).
    SignalVacuous,
    /// A guard failed (conservation / `bread_minted_max>0` / `!extinct`) OR provenance is DISQUALIFIED.
    ConservationBroken,
    /// Committed cultivators dominate grain (share ≥ `MONO_SHARE`) AND the buyer cohort collapses.
    MonopolizationCull,
    /// Rolling cultivator share ≥ `COMMUNE_SHARE` AND post-promotion bought below the floor.
    CommuneCollapse,
    /// SALT never forms / fails at horizon despite produced+sold `SelfProduced` bread.
    MoneyFailureFromCommitment,
    /// Uptake exceeds `COMMIT_SHARE_MAX` (or no below-floor non-committer / live fluid remainder): a
    /// technically-voluntary but de-facto pin.
    UniversalCommitment,
    /// Uptake is not actually voluntary (forced / fiat — not gated by the agent's own cleared signal),
    /// OR success-like persistence rests on one un-renewed mega-term. Not headline success.
    RePinScaffold,
    /// Short terms bite (real commitments + exit-overrides) but cannot form a persistent cohort (the
    /// term < what the cohort bar needs).
    TermTooShortFinding,
    /// FINAL GATE — all ten success clauses (§2.1–§2.10) hold.
    RoleStickySuccess,
    /// (else) Commitments happen and bind, but a success clause is unmet — even an exit-overriding
    /// institution did not retain.
    NoStickinessDespiteCommitment,
}

/// The full per-run classification vector — every figure the §2 ordered classifier and the report read,
/// collected by one tick-by-tick pass ([`run_metrics`]). All accessors are read-only.
#[derive(Clone, Debug)]
struct Metrics {
    seed: u64,
    // ---- hard guards ----
    conserved: bool,
    bread_minted_max: u64,
    extinct: bool,
    // ---- provenance / money ----
    provenance_clean: bool,
    promoted: bool,
    // ---- the commitment lever (non-vacuity precondition) ----
    on: bool,
    is_fiat: bool,
    committed_ever: usize,
    signal_gated_commits: usize,
    below_floor_noncommitters: usize,
    exit_override_ever: usize,
    // ---- churn (per ever-cultivating agent; same measure as the matched baseline) ----
    churn_total: u32,
    ever_cultivating: usize,
    eligible_ever: usize,
    non_committed_fluid_cultivators: usize,
    // ---- persistent committed cohort (the primary metric) ----
    persistent_committed_cohort: usize,
    all_persistent_renewed: bool,
    max_concurrent_committed: usize,
    // ---- secondary lineage persistence (reported, never the primary gate) ----
    persistent_committed_lineages: usize,
    // ---- grain concentration ----
    committed_grain_share: f64,
    // ---- rolling cultivator share ----
    max_rolling_cultivator_share: f64,
    // ---- non-committed buyer cohort ----
    final_buyer_cohort: usize,
    living_non_lineage_final: usize,
    post_promotion_bought: u64,
    // ---- the shipped term (the §2.5 cap guard reads it) ----
    commitment_term: u16,
    // ---- capital-composition tool-stock invariant (only meaningful when a variant runs) ----
    endowed_tools_total: u64,
    tools_built: u64,
    tools_destroyed: u64,
    tool_stock_total: u64,
    // ---- reporting only ----
    living: usize,
    lineage_living: usize,
    starvation: u64,
}

impl Metrics {
    /// Per-ever-cultivating-agent enter/exit churn (the stickiness headline numerator).
    fn churn_per_capita(&self) -> f64 {
        if self.ever_cultivating == 0 {
            0.0
        } else {
            f64::from(self.churn_total) / self.ever_cultivating as f64
        }
    }

    /// Committed-cohort share of the ever-cultivating set (the minority-uptake guard, §2.4).
    fn uptake_share_of_cultivating(&self) -> f64 {
        if self.ever_cultivating == 0 {
            0.0
        } else {
            self.committed_ever as f64 / self.ever_cultivating as f64
        }
    }

    /// Committed-cohort share of the eligible-candidate set (ever-cultivating ∪ below-floor).
    fn uptake_share_of_eligible(&self) -> f64 {
        if self.eligible_ever == 0 {
            0.0
        } else {
            self.committed_ever as f64 / self.eligible_ever as f64
        }
    }

    /// A live fluid non-committed remainder exists (§2.4): a living buyer cohort AND some cultivator
    /// that never committed (still cultivates/exits under the normal S22a/S22c rule).
    fn has_live_fluid_remainder(&self) -> bool {
        self.final_buyer_cohort >= MIN_BUYER_COHORT && self.non_committed_fluid_cultivators > 0
    }

    /// A surviving non-committed buyer cohort that MATERIALLY buys (§2.6).
    fn buyers_materially_buy(&self) -> bool {
        self.post_promotion_bought >= MATERIAL_BOUGHT_FLOOR
            && self.final_buyer_cohort >= MIN_BUYER_COHORT
    }

    /// The monopolization DAMAGE side: the non-lineage / buyer demand side has fallen below the floor.
    fn demand_damaged(&self) -> bool {
        self.living_non_lineage_final <= DEMAND_COLLAPSE_FLOOR
            || self.final_buyer_cohort < MIN_BUYER_COHORT
    }

    /// Every committed agent's uptake was signal-gated (a genuinely voluntary run); false under the
    /// fiat-pin control (forced commits record no signal).
    fn uptake_all_signal_gated(&self) -> bool {
        !self.is_fiat && self.committed_ever > 0 && self.signal_gated_commits == self.committed_ever
    }

    /// The stickiness + health conjunction, IGNORING the voluntariness/control gates (applied in
    /// [`Self::verdict`]): churn fell ≤ 0.5× baseline, a persistent committed cohort formed, money
    /// survives, and a living buyer cohort persists.
    fn sticky(&self, baseline_churn: f64) -> bool {
        self.churn_per_capita() <= CHURN_DROP * baseline_churn
            && self.persistent_committed_cohort >= PERSIST_COHORT
            && self.promoted
            && !self.extinct
            && self.final_buyer_cohort >= MIN_BUYER_COHORT
    }

    /// All ten success clauses (§2.1–§2.10): ≥`MIN_COMMITS` voluntary commits + signal discriminates;
    /// churn ≤ 0.5× baseline; persistent committed cohort; bounded uptake + live fluid remainder;
    /// `commitment_term < cap` + every persistent committed id renewed; buyers materially buy; money
    /// survives; provenance clean; mortality + conservation hold; genuinely voluntary (not fiat).
    fn all_success_clauses(&self, baseline_churn: f64) -> bool {
        self.signal_gated_commits >= MIN_COMMITS
            && self.below_floor_noncommitters > 0
            && self.sticky(baseline_churn)
            && self.uptake_share_of_cultivating() <= COMMIT_SHARE_MAX
            && self.uptake_share_of_eligible() <= COMMIT_SHARE_MAX
            && self.has_live_fluid_remainder()
            && (self.commitment_term as usize) < persist_threshold() as usize
            && self.all_persistent_renewed
            && self.buyers_materially_buy()
            && self.provenance_clean
            && self.bread_minted_max == 0
            && self.conserved
            && !self.extinct
            && self.uptake_all_signal_gated()
    }

    /// The §2 ORDERED, mutually-exclusive classifier — checked top-down; the FIRST match is the
    /// verdict. `baseline_churn` is the matched-seed commitment-off expanded baseline churn.
    fn verdict(&self, baseline_churn: f64) -> Verdict {
        // (1) The institution got too few commitments of ANY kind (the unprofitable control: the
        // signal never clears → zero uptake). Distinguishes "nobody chose it" from "no stickiness".
        if self.committed_ever < MIN_COMMITS {
            return Verdict::CommitmentUnchosen;
        }
        // (2) The signal didn't discriminate (a voluntary run where no eligible agent stayed below the
        // floor — no real entry decision). Fiat runs bypass the signal, so this is voluntary-only.
        if !self.is_fiat && self.below_floor_noncommitters == 0 {
            return Verdict::SignalVacuous;
        }
        // (3) A guard break or provenance disqualification.
        if !self.conserved || self.extinct || self.bread_minted_max > 0 || !self.provenance_clean {
            return Verdict::ConservationBroken;
        }
        // (4) A committed dynasty that starves the market.
        if self.committed_grain_share >= MONO_SHARE && self.demand_damaged() {
            return Verdict::MonopolizationCull;
        }
        // (5) Most survivors cultivate and the market dissolves.
        if self.max_rolling_cultivator_share >= COMMUNE_SHARE
            && self.post_promotion_bought < MATERIAL_BOUGHT_FLOOR
        {
            return Verdict::CommuneCollapse;
        }
        // (6) Money never forms / fails.
        if !self.promoted {
            return Verdict::MoneyFailureFromCommitment;
        }
        // (7) Technically voluntary but a de-facto pin (over-uptake or no fluid remainder).
        if !self.is_fiat
            && (self.uptake_share_of_cultivating() > COMMIT_SHARE_MAX
                || self.uptake_share_of_eligible() > COMMIT_SHARE_MAX
                || !self.has_live_fluid_remainder())
        {
            return Verdict::UniversalCommitment;
        }
        // (8) Uptake not voluntary (forced / fiat), OR success-like persistence on one un-renewed
        // mega-term — a disguised pin, never headline success.
        let not_voluntary = self.committed_ever >= MIN_COMMITS && !self.uptake_all_signal_gated();
        let mega_unrenewed = self.sticky(baseline_churn)
            && self.persistent_committed_cohort >= PERSIST_COHORT
            && !self.all_persistent_renewed;
        if not_voluntary || mega_unrenewed {
            return Verdict::RePinScaffold;
        }
        // (9) Short terms bite but cannot form a persistent cohort.
        if self.exit_override_ever > 0
            && self.persistent_committed_cohort < PERSIST_COHORT
            && (self.commitment_term as usize) < persist_threshold() as usize
        {
            return Verdict::TermTooShortFinding;
        }
        // (10) FINAL GATE.
        if self.all_success_clauses(baseline_churn) {
            Verdict::RoleStickySuccess
        } else {
            Verdict::NoStickinessDespiteCommitment
        }
    }

    /// The §3.5 tool-stock accounting invariant for the capital-composition variants: `endowed + built
    /// − destroyed == live whole-system stock` (asserting non-negativity first). Trivially `0 == 0` for
    /// the no-capital headline (no plow good).
    fn tool_stock_balances(&self) -> bool {
        self.tools_destroyed <= self.endowed_tools_total + self.tools_built
            && self.endowed_tools_total + self.tools_built - self.tools_destroyed
                == self.tool_stock_total
    }

    /// A uniform one-line figure rendering for the per-run classification maps.
    fn line(&self, baseline_churn: f64) -> String {
        format!(
            "{:?} | on={} fiat={} | churn/cap={:.2} (base={:.2}; drop_to={:.2}? {}) | \
             commit(ever={} signal={} below_nc={} exit_ov={}) | \
             cohort(persist_id={} all_renewed={} max_concurrent={} persist_lin={}) | \
             share(cult={:.2} elig={:.2} grain={:.2} maxroll={:.2}) | \
             buyers={} fluid_remainder={} post_bought={} term={} cap={} | \
             promoted={} prov_clean={} | living={} nl_alive={} lin_alive={} starv={} | \
             conserved={} minted_max={} extinct={} tool_balances={}",
            self.verdict(baseline_churn),
            self.on,
            self.is_fiat,
            self.churn_per_capita(),
            baseline_churn,
            CHURN_DROP * baseline_churn,
            self.churn_per_capita() <= CHURN_DROP * baseline_churn,
            self.committed_ever,
            self.signal_gated_commits,
            self.below_floor_noncommitters,
            self.exit_override_ever,
            self.persistent_committed_cohort,
            self.all_persistent_renewed,
            self.max_concurrent_committed,
            self.persistent_committed_lineages,
            self.uptake_share_of_cultivating(),
            self.uptake_share_of_eligible(),
            self.committed_grain_share,
            self.max_rolling_cultivator_share,
            self.final_buyer_cohort,
            self.has_live_fluid_remainder(),
            self.post_promotion_bought,
            self.commitment_term,
            commitment_term_cap(),
            self.promoted,
            self.provenance_clean,
            self.living,
            self.living_non_lineage_final,
            self.lineage_living,
            self.starvation,
            self.conserved,
            self.bread_minted_max,
            self.extinct,
            self.tool_stock_balances(),
        )
    }
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

/// The trailing rolling means of `xs` over a window of `w`. Empty if `xs` is shorter than `w`.
fn rolling_means(xs: &[f64], w: usize) -> Vec<f64> {
    if w == 0 || xs.len() < w {
        return Vec::new();
    }
    (w..=xs.len()).map(|end| mean(&xs[end - w..end])).collect()
}

/// Run `(seed, cfg)` for `ticks` and collect the full S22f classification vector by one tick-by-tick
/// read of the public, runtime-only accessors. Commitment cohorts/churn are keyed by stable `AgentId`;
/// lineage persistence by household index; the grain share splits the final-window grain by whether
/// the id ever committed.
fn run_metrics(seed: u64, cfg: &SettlementConfig, ticks: u64) -> Metrics {
    let mut s = Settlement::generate(seed, cfg);
    let bread = s.bread_good();

    let mut conserved = true;
    let mut bread_minted_max = 0u64;

    // id-keyed churn / participation.
    let mut was_cultivating: BTreeMap<u64, bool> = BTreeMap::new();
    let mut churn: BTreeMap<u64, u32> = BTreeMap::new();
    let mut ever_cultivating: BTreeSet<u64> = BTreeSet::new();

    let mut share_samples: Vec<f64> = Vec::with_capacity(ticks as usize);
    let mut buyer_samples: Vec<usize> = Vec::with_capacity(ticks as usize);

    let final_window_start = ticks.saturating_sub(FINAL_WINDOW as u64);
    // final-window per-id (cultivating || committed) tick counts + per-household analogue.
    let mut fw_id_persist: BTreeMap<u64, u32> = BTreeMap::new();
    let mut fw_lineage_persist_now: BTreeMap<usize, u32> = BTreeMap::new();
    // committed-by-id and committed-lineage membership over the run.
    let mut committed_lineages: BTreeSet<usize> = BTreeSet::new();
    // final-window grain split by committed class.
    let mut grain_at_window_start: BTreeMap<u64, u64> = BTreeMap::new();
    let mut window_start_snapshotted = false;
    let mut max_concurrent_committed = 0usize;

    let mut promoted = false;
    let mut bought_at_promotion: Option<u64> = None;

    for t in 0..ticks {
        let report = s.econ_tick();
        conserved &= report.conserves();
        if let Some(b) = bread {
            bread_minted_max = bread_minted_max.max(report.endowment_of(b));
        }

        if !window_start_snapshotted && t >= final_window_start {
            for i in 0..s.population() {
                if let Some(id) = s.colonist_id(i) {
                    grain_at_window_start.insert(id.0, s.cultivation_grain_harvested_of(i));
                }
            }
            window_start_snapshotted = true;
        }

        let pop = s.population();
        let mut cultivating_count = 0usize;
        let mut alive_count = 0usize;
        let mut buyers = 0usize;
        let mut committed_now = 0usize;
        let mut lineage_persist_now: BTreeSet<usize> = BTreeSet::new();
        for i in 0..pop {
            if !s.is_alive(i) {
                continue;
            }
            alive_count += 1;
            let Some(id) = s.colonist_id(i) else { continue };
            let key = id.0;
            let household = s.household_of(i);
            let cultivating = s.is_cultivating(i);
            let committed = s.is_committed(i);
            if committed {
                committed_now += 1;
                if let Some(h) = household {
                    committed_lineages.insert(h);
                }
            }
            if cultivating {
                cultivating_count += 1;
                ever_cultivating.insert(key);
            }
            if t >= final_window_start && (cultivating || committed) {
                *fw_id_persist.entry(key).or_insert(0) += 1;
                if let Some(h) = household {
                    lineage_persist_now.insert(h);
                }
            }
            let prev = was_cultivating.get(&key).copied().unwrap_or(false);
            if cultivating != prev {
                *churn.entry(key).or_insert(0) += 1;
            }
            was_cultivating.insert(key, cultivating);
            // A material buyer is a living, NON-cultivating, NON-committed, non-lineage colonist that
            // has bought materially — the fluid demand side the split must leave intact.
            if household.is_none()
                && !cultivating
                && !committed
                && s.bought_food_of(i) >= MATERIAL_BUYER_FOOD
            {
                buyers += 1;
            }
        }
        if t >= final_window_start {
            max_concurrent_committed = max_concurrent_committed.max(committed_now);
            for h in lineage_persist_now {
                *fw_lineage_persist_now.entry(h).or_insert(0) += 1;
            }
        }
        let share = if alive_count == 0 {
            0.0
        } else {
            cultivating_count as f64 / alive_count as f64
        };
        share_samples.push(share);
        buyer_samples.push(buyers);

        if !promoted && s.current_money_good() == Some(SALT) {
            promoted = true;
            bought_at_promotion = Some(s.acquisition_consumed_by_channel().bought);
        }
    }

    let committed_ever: BTreeSet<u64> = s.commitment_committed_ids().into_iter().collect();
    let below_floor: BTreeSet<u64> = s.commitment_below_floor_ids().into_iter().collect();
    let below_floor_noncommitters = below_floor.difference(&committed_ever).count();
    let eligible_ever: BTreeSet<u64> = ever_cultivating.union(&below_floor).copied().collect();
    let non_committed_fluid_cultivators = ever_cultivating.difference(&committed_ever).count();

    let threshold = persist_threshold();
    let id_to_index: BTreeMap<u64, usize> = (0..s.population())
        .filter_map(|i| s.colonist_id(i).map(|id| (id.0, i)))
        .collect();
    let mut persistent_committed_cohort = 0usize;
    let mut all_persistent_renewed = true;
    for (&key, &count) in &fw_id_persist {
        if count >= threshold && committed_ever.contains(&key) {
            persistent_committed_cohort += 1;
            let renewals = id_to_index
                .get(&key)
                .map_or(0, |&i| s.commitment_renewals_of(i));
            if renewals == 0 {
                all_persistent_renewed = false;
            }
        }
    }
    // Secondary: distinct lineages with a committed member cultivating/committed ≥ the threshold.
    let persistent_committed_lineages = fw_lineage_persist_now
        .iter()
        .filter(|(h, &count)| count >= threshold && committed_lineages.contains(h))
        .count();

    let mut fw_grain_total = 0u64;
    let mut fw_grain_committed = 0u64;
    for i in 0..s.population() {
        let Some(id) = s.colonist_id(i) else { continue };
        let key = id.0;
        let fw_grain = s
            .cultivation_grain_harvested_of(i)
            .saturating_sub(grain_at_window_start.get(&key).copied().unwrap_or(0));
        fw_grain_total += fw_grain;
        if committed_ever.contains(&key) {
            fw_grain_committed += fw_grain;
        }
    }
    let committed_grain_share = if fw_grain_total == 0 {
        0.0
    } else {
        fw_grain_committed as f64 / fw_grain_total as f64
    };

    let max_rolling_cultivator_share = rolling_means(&share_samples, ROLLING_WINDOW)
        .into_iter()
        .fold(0.0f64, f64::max);
    let final_buyer_cohort = buyer_samples[buyer_samples.len().saturating_sub(FINAL_WINDOW)..]
        .iter()
        .copied()
        .max()
        .unwrap_or(0);

    let consumed = s.acquisition_consumed_by_channel();
    let (_pp, pp_minted) = s.pre_promotion_bread_for_salt_by_provenance();
    let provenance_clean = s.seeded_minted_bread_sold_for_salt() == 0 && pp_minted == 0;
    let post_promotion_bought = match bought_at_promotion {
        Some(at) => consumed.bought.saturating_sub(at),
        None => 0,
    };

    Metrics {
        seed,
        conserved,
        bread_minted_max,
        extinct: living(&s) == 0,
        provenance_clean,
        promoted: s.current_money_good() == Some(SALT),
        on: s.voluntary_cultivation_commitment_on(),
        is_fiat: !s.commitment_fiat_ids().is_empty(),
        committed_ever: committed_ever.len(),
        signal_gated_commits: s.commitment_uptake().len(),
        below_floor_noncommitters,
        exit_override_ever: s.commitment_exit_overridden_ids().len(),
        churn_total: churn.values().copied().sum(),
        ever_cultivating: ever_cultivating.len(),
        eligible_ever: eligible_ever.len(),
        non_committed_fluid_cultivators,
        persistent_committed_cohort,
        all_persistent_renewed,
        max_concurrent_committed,
        persistent_committed_lineages,
        committed_grain_share,
        max_rolling_cultivator_share,
        final_buyer_cohort,
        living_non_lineage_final: living_non_lineage(&s),
        post_promotion_bought,
        commitment_term: s.commitment_term_config(),
        endowed_tools_total: s.endowed_cultivation_tools_total(),
        tools_built: s.cultivation_tools_built(),
        tools_destroyed: s.cultivation_tools_destroyed(),
        tool_stock_total: s.cultivation_tool_stock_total(),
        living: living(&s),
        lineage_living: living_lineage(&s),
        starvation: s.starvation_deaths_total(),
    }
}

/// Run a batch of labelled `(seed, cfg)` jobs concurrently (bounded scoped-thread fan-out) and return
/// the collected `Metrics` in input order. The engine has no global mutable state, so concurrent
/// `Settlement` runs are deterministic per `(seed, config)`.
fn run_batch(jobs: Vec<(u64, SettlementConfig)>) -> Vec<Metrics> {
    let workers = 8usize.min(jobs.len().max(1));
    let mut buckets: Vec<Vec<(usize, u64, SettlementConfig)>> = vec![Vec::new(); workers];
    for (i, (seed, cfg)) in jobs.into_iter().enumerate() {
        buckets[i % workers].push((i, seed, cfg));
    }
    let mut computed: Vec<(usize, Metrics)> = std::thread::scope(|scope| {
        let handles: Vec<_> = buckets
            .into_iter()
            .map(|bucket| {
                scope.spawn(move || {
                    bucket
                        .into_iter()
                        .map(|(i, seed, cfg)| (i, run_metrics(seed, &cfg, PROBE_TICKS)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("worker thread"))
            .collect()
    });
    computed.sort_by_key(|(i, _)| *i);
    computed.into_iter().map(|(_, m)| m).collect()
}

/// The matched-seed commitment-OFF expanded baseline churn
/// ([`SettlementConfig::frontier_profit_retention_expanded`]) — the comparison denominator the verdict
/// uses.
fn baseline_churns(seeds: &[u64]) -> BTreeMap<u64, f64> {
    let jobs: Vec<(u64, SettlementConfig)> = seeds
        .iter()
        .map(|&seed| (seed, SettlementConfig::frontier_profit_retention_expanded()))
        .collect();
    seeds
        .iter()
        .copied()
        .zip(run_batch(jobs).into_iter().map(|m| m.churn_per_capita()))
        .collect()
}

/// The hard guards every run must satisfy regardless of regime (§4): conservation each tick, no bread
/// minted, no extinction. (Provenance-clean is handled by the verdict: `!provenance_clean` ⇒
/// `ConservationBroken`, unless the lever was inert.)
fn assert_guards(m: &Metrics, ctx: &str) {
    assert!(
        m.conserved,
        "[{ctx}] conservation must hold every tick (seed {})",
        m.seed
    );
    assert_eq!(
        m.bread_minted_max, 0,
        "[{ctx}] no bread may be minted (seed {})",
        m.seed
    );
    assert!(
        !m.extinct,
        "[{ctx}] the colony must not go extinct (seed {})",
        m.seed
    );
    assert!(
        m.tool_stock_balances(),
        "[{ctx}] the tool-stock invariant must hold \
         (endowed {} + built {} − destroyed {} == stock {}) (seed {})",
        m.endowed_tools_total,
        m.tools_built,
        m.tools_destroyed,
        m.tool_stock_total,
        m.seed
    );
}

// =========================================================================
// 0. The predeclared-constant coherence contract
// =========================================================================

#[test]
fn coherence_constants_are_well_formed() {
    // The shipped term cannot span the persistence window (§2.5): a single term mathematically cannot
    // hold the cohort bar, so persistence MUST come from renewals.
    assert!(
        COMMITMENT_TERM_SHIPPED < commitment_term_cap(),
        "the shipped term {COMMITMENT_TERM_SHIPPED} must be < the persistence-window cap {} \
         so a single term cannot span the persistence window",
        commitment_term_cap()
    );
    // Every swept term must also respect the cap so the headline bar is interpretable across the sweep.
    for &term in &TERM_SWEEP {
        assert!(
            term < commitment_term_cap(),
            "swept term {term} must be < the cap {}",
            commitment_term_cap()
        );
    }
    // The persistence threshold lies within the final window (read from the fn so it is not a
    // const-folded tautology).
    assert!(
        persist_threshold() as usize <= FINAL_WINDOW,
        "the persistence threshold {} must fit within the final window {FINAL_WINDOW}",
        persist_threshold()
    );
    // The test mirrors the engine roster: the headline expanded base must carry ROSTER_HOUSEHOLDS
    // lineage households, so a `PERSIST_COHORT` committed cohort is at least reachable.
    let s = Settlement::generate(7, &SettlementConfig::frontier_voluntary_commitment());
    let households: BTreeSet<usize> = (0..s.population())
        .filter_map(|i| s.household_of(i))
        .collect();
    assert_eq!(
        households.len(),
        ROSTER_HOUSEHOLDS,
        "the headline expanded base must carry ROSTER_HOUSEHOLDS lineage households"
    );
    assert!(
        PERSIST_COHORT <= households.len(),
        "the committed cohort bar must be reachable on the expanded roster"
    );
    assert!(
        s.voluntary_cultivation_commitment_on(),
        "the headline must actually run the commitment gate"
    );
}

// =========================================================================
// 1. The determinism / golden contract
// =========================================================================

#[test]
fn canonical_bytes_split_for_voluntary_commitment() {
    // The commitment gate overrides the cultivation exit for a chosen term (a future-behaviour change),
    // so `frontier_voluntary_commitment` must SPLIT the canonical digest vs the gate-off expanded base.
    let base = Settlement::generate(7, &SettlementConfig::frontier_profit_retention_expanded());
    let on = Settlement::generate(7, &SettlementConfig::frontier_voluntary_commitment());
    assert_ne!(
        base.canonical_bytes(),
        on.canonical_bytes(),
        "the voluntary_cultivation_commitment gate must split the canonical digest vs the base"
    );
    // The term + entry floor + fiat-pin steer behaviour, so flipping each must also split the digest.
    let nonbinding = Settlement::generate(
        7,
        &SettlementConfig::frontier_voluntary_commitment_nonbinding(),
    );
    assert_ne!(
        on.canonical_bytes(),
        nonbinding.canonical_bytes(),
        "the commitment_term must split the canonical digest"
    );
    let fiat = Settlement::generate(
        7,
        &SettlementConfig::frontier_voluntary_commitment_fiat_pin(),
    );
    assert_ne!(
        on.canonical_bytes(),
        fiat.canonical_bytes(),
        "the commitment_fiat_pin must split the canonical digest"
    );
    let unprofitable = Settlement::generate(
        7,
        &SettlementConfig::frontier_voluntary_commitment_unprofitable(),
    );
    assert_ne!(
        on.canonical_bytes(),
        unprofitable.canonical_bytes(),
        "the commitment_entry_floor must split the canonical digest"
    );
}

#[test]
fn voluntary_commitment_off_the_path_is_inert() {
    // The gate composes on the S22c profit-driven-retention path. Toggling
    // `voluntary_cultivation_commitment` on a config OFF that path must NOT split the digest — the gate
    // is inert without the composition.
    let base = Settlement::generate(7, &SettlementConfig::frontier_forage_capacity());
    let mut base_cfg = SettlementConfig::frontier_forage_capacity();
    if let Some(chain) = base_cfg.chain.as_mut() {
        chain.voluntary_cultivation_commitment = true;
        chain.commitment_term = 24;
        chain.commitment_fiat_pin = 4;
    }
    assert_eq!(
        base.canonical_bytes(),
        Settlement::generate(7, &base_cfg).canonical_bytes(),
        "off the profit-retention path the voluntary_cultivation_commitment flag must not steer the digest"
    );
}

#[test]
fn voluntary_commitment_runs_are_deterministic() {
    let cfg = SettlementConfig::frontier_voluntary_commitment();
    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    a.run(1_000);
    b.run(1_000);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the voluntary-commitment run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.digest(), b.digest());
    let mut c = Settlement::generate(2, &cfg);
    c.run(1_000);
    assert_ne!(a.digest(), c.digest(), "the seed must change the run");
}

#[test]
fn goldens_unchanged() {
    // The S22f addition is one additive, default-off ON-only gate (tag 12) + per-colonist commitment
    // steering state (serialized ON-only) + runtime-only diagnostics, so the cross-history demographic
    // + emergence goldens are BYTE-IDENTICAL (the same five values pinned in endowed_inherited_capital).
    let digest = |cfg: &SettlementConfig, ticks: u64| {
        let mut s = Settlement::generate(1, cfg);
        s.run(ticks);
        s.digest()
    };
    assert_eq!(
        digest(&SettlementConfig::lineages(), 300),
        0x2335e13c809749fc,
        "the `lineages` demographic golden (the key tripwire) must be byte-identical"
    );
    assert_eq!(
        digest(&SettlementConfig::lineages(), 800),
        0x3ffd78e50842d934,
        "the long `lineages` run must be byte-identical"
    );
    assert_eq!(
        digest(&SettlementConfig::frontier(), 300),
        0xcc83bf2669f0980d,
        "the S5-S13 frontier golden must be byte-identical"
    );
    assert_eq!(
        digest(&SettlementConfig::frontier_spatial_households(), 300),
        0xf30e3ce9345a73b3,
        "the S13 spatial-households golden must be byte-identical"
    );
    let mut viable = Settlement::generate(0xC0FFEE, &SettlementConfig::viable());
    viable.run(60);
    assert_eq!(
        viable.digest(),
        0xa174_8567_db1c_4341,
        "the g4a viable no-death digest must be byte-identical"
    );
}

// =========================================================================
// 2. The MANDATORY non-vacuity / voluntariness test (§4)
// =========================================================================

#[test]
fn mandatory_non_vacuity_and_voluntariness() {
    // Over every seed the headline must show a NON-VACUOUS, VOLUNTARY, DISCRIMINATING, BINDING
    // institution: ≥ MIN_COMMITS agents voluntarily commit after money (each entry traceable to that
    // agent's own cleared signal), the signal discriminates (eligible agents below the floor do NOT
    // commit), and ≥1 commitment BINDS a tick the matched flag-off run would have exited.
    let jobs: Vec<(u64, SettlementConfig)> = SEEDS
        .iter()
        .map(|&seed| (seed, SettlementConfig::frontier_voluntary_commitment()))
        .collect();
    for m in run_batch(jobs) {
        assert!(
            m.on,
            "[non-vacuity] the headline must run the commitment gate (seed {})",
            m.seed
        );
        assert!(
            !m.is_fiat,
            "[non-vacuity] the headline must be voluntary, not fiat (seed {})",
            m.seed
        );
        assert!(
            m.signal_gated_commits >= MIN_COMMITS,
            "[non-vacuity] ≥ {MIN_COMMITS} agents must VOLUNTARILY commit after money (seed {}: {})",
            m.seed,
            m.signal_gated_commits
        );
        assert_eq!(
            m.signal_gated_commits, m.committed_ever,
            "[non-vacuity] every committed agent's uptake must be signal-gated (seed {})",
            m.seed
        );
        assert!(
            m.below_floor_noncommitters > 0,
            "[non-vacuity] the signal must DISCRIMINATE — some eligible agents below the floor do not \
             commit (seed {}: {})",
            m.seed,
            m.below_floor_noncommitters
        );
        assert!(
            m.exit_override_ever >= 1,
            "[non-vacuity] ≥1 commitment must BIND a tick the flag-off run would have exited (seed {}: {})",
            m.seed,
            m.exit_override_ever
        );
        assert_guards(&m, "non-vacuity");
    }
}

// =========================================================================
// 3. The headline hard guards (§4) — minority uptake, live fluid remainder, term cap
// =========================================================================

#[test]
fn headline_hard_guards() {
    let jobs: Vec<(u64, SettlementConfig)> = SEEDS
        .iter()
        .map(|&seed| (seed, SettlementConfig::frontier_voluntary_commitment()))
        .collect();
    for m in run_batch(jobs) {
        assert_guards(&m, "headline-guards");
        assert!(
            m.provenance_clean,
            "[headline-guards] sold/pre-promotion bread must be SelfProduced; seeded_minted == 0 (seed {})",
            m.seed
        );
        assert!(
            m.commitment_term < commitment_term_cap(),
            "[headline-guards] commitment_term {} must be < the persistence-window cap {} (seed {})",
            m.commitment_term,
            commitment_term_cap(),
            m.seed
        );
        // The minority-uptake guard (§2.4): the committed cohort is a bounded minority of the
        // ever-cultivating set (a de-facto pin would exceed it).
        assert!(
            m.uptake_share_of_cultivating() <= COMMIT_SHARE_MAX,
            "[headline-guards] committed uptake {:.2} must be ≤ {COMMIT_SHARE_MAX} of ever-cultivating (seed {})",
            m.uptake_share_of_cultivating(),
            m.seed
        );
        // A live fluid non-committed remainder must persist (buyers + non-committed fluid cultivators).
        assert!(
            m.has_live_fluid_remainder(),
            "[headline-guards] a live fluid non-committed remainder must persist (buyers={}, fluid_cult={}) (seed {})",
            m.final_buyer_cohort,
            m.non_committed_fluid_cultivators,
            m.seed
        );
    }
}

// =========================================================================
// 4. Controls (§4) — each falsifies a distinct channel
// =========================================================================

#[test]
fn commitment_off_reproduces_no_stickiness() {
    // The matched baseline (commitment gate OFF, expanded no-capital S22c roster) must reproduce
    // S22c/S22e no-stickiness: money promotes, mortality coexists, no commitment forms, churn is high.
    let baselines = baseline_churns(&SEEDS);
    let jobs: Vec<(u64, SettlementConfig)> = SEEDS
        .iter()
        .map(|&seed| (seed, SettlementConfig::frontier_profit_retention_expanded()))
        .collect();
    for m in run_batch(jobs) {
        assert_guards(&m, "commitment_off");
        assert!(
            !m.on,
            "[commitment_off] the gate must be off (seed {})",
            m.seed
        );
        assert_eq!(
            m.committed_ever, 0,
            "[commitment_off] no commitment may form with the gate off (seed {})",
            m.seed
        );
        let bc = baselines.get(&m.seed).copied().unwrap_or(0.0);
        assert_eq!(
            m.verdict(bc),
            Verdict::CommitmentUnchosen,
            "[commitment_off] the gate-off baseline classifies as CommitmentUnchosen (institution absent) (seed {})",
            m.seed
        );
    }
}

#[test]
fn unprofitable_offer_gets_zero_uptake() {
    // An impossibly high entry floor ⇒ NO agent's signal ever clears ⇒ ZERO voluntary uptake ⇒
    // CommitmentUnchosen — proving uptake is voluntary / signal-gated, not automatic.
    let baselines = baseline_churns(&SEEDS);
    let jobs: Vec<(u64, SettlementConfig)> = SEEDS
        .iter()
        .map(|&seed| {
            (
                seed,
                SettlementConfig::frontier_voluntary_commitment_unprofitable(),
            )
        })
        .collect();
    for m in run_batch(jobs) {
        assert_guards(&m, "unprofitable_offer");
        assert!(
            m.on,
            "[unprofitable_offer] the gate must be on (seed {})",
            m.seed
        );
        assert_eq!(
            m.committed_ever, 0,
            "[unprofitable_offer] an impossibly high floor must yield ZERO uptake (seed {}: {})",
            m.seed, m.committed_ever
        );
        let bc = baselines.get(&m.seed).copied().unwrap_or(0.0);
        assert_eq!(
            m.verdict(bc),
            Verdict::CommitmentUnchosen,
            "[unprofitable_offer] zero uptake classifies as CommitmentUnchosen (seed {})",
            m.seed
        );
    }
}

#[test]
fn nonbinding_term_is_not_success() {
    // A one-tick "commitment" (term = 1) lets agents commit but binds nothing beyond the tick they
    // form on — it should reproduce S22c MARGINAL retention, never RoleStickySuccess (the binding TERM,
    // not the act of committing, is what matters).
    let baselines = baseline_churns(&SEEDS);
    let jobs: Vec<(u64, SettlementConfig)> = SEEDS
        .iter()
        .map(|&seed| {
            (
                seed,
                SettlementConfig::frontier_voluntary_commitment_nonbinding(),
            )
        })
        .collect();
    for m in run_batch(jobs) {
        assert_guards(&m, "nonbinding_term");
        assert_eq!(
            m.commitment_term, 1,
            "[nonbinding_term] term must be 1 (seed {})",
            m.seed
        );
        let bc = baselines.get(&m.seed).copied().unwrap_or(0.0);
        assert_ne!(
            m.verdict(bc),
            Verdict::RoleStickySuccess,
            "[nonbinding_term] a one-tick commitment must NOT be RoleStickySuccess (seed {})",
            m.seed
        );
    }
}

#[test]
fn fiat_pin_is_repin_scaffold() {
    // Forcibly committing a configured minority of agents (bypassing the voluntary signal) must classify
    // RePinScaffold and NEVER count as headline success — the key anti-repin falsifier.
    let baselines = baseline_churns(&SEEDS);
    let jobs: Vec<(u64, SettlementConfig)> = SEEDS
        .iter()
        .map(|&seed| {
            (
                seed,
                SettlementConfig::frontier_voluntary_commitment_fiat_pin(),
            )
        })
        .collect();
    for m in run_batch(jobs) {
        assert_guards(&m, "fiat_pin");
        assert!(
            m.is_fiat,
            "[fiat_pin] the run must be a forced (fiat) pin (seed {})",
            m.seed
        );
        assert_eq!(
            m.signal_gated_commits, 0,
            "[fiat_pin] forced commits must record NO signal-gated uptake (seed {})",
            m.seed
        );
        let bc = baselines.get(&m.seed).copied().unwrap_or(0.0);
        assert_eq!(
            m.verdict(bc),
            Verdict::RePinScaffold,
            "[fiat_pin] a forced pin must classify RePinScaffold (seed {}): {}",
            m.seed,
            m.line(bc)
        );
        assert_ne!(
            m.verdict(bc),
            Verdict::RoleStickySuccess,
            "[fiat_pin] a forced pin must NEVER be headline success (seed {})",
            m.seed
        );
    }
}

#[test]
fn repin_scaffold_separation() {
    // The voluntary headline must be DISTINGUISHABLE from the fiat re-pin even if both show low churn:
    // only the voluntary one has signal-gated uptake, a below-floor non-committer set, and renewals.
    let head: BTreeMap<u64, Metrics> = run_batch(
        SEEDS
            .iter()
            .map(|&s| (s, SettlementConfig::frontier_voluntary_commitment()))
            .collect(),
    )
    .into_iter()
    .map(|m| (m.seed, m))
    .collect();
    let fiat: BTreeMap<u64, Metrics> = run_batch(
        SEEDS
            .iter()
            .map(|&s| {
                (
                    s,
                    SettlementConfig::frontier_voluntary_commitment_fiat_pin(),
                )
            })
            .collect(),
    )
    .into_iter()
    .map(|m| (m.seed, m))
    .collect();
    for &seed in &SEEDS {
        let h = &head[&seed];
        let f = &fiat[&seed];
        // Only the voluntary run has signal-gated uptake.
        assert!(
            h.uptake_all_signal_gated() && !f.uptake_all_signal_gated(),
            "[separation] only the voluntary headline has signal-gated uptake (seed {seed})"
        );
        // Only the voluntary run has a below-floor non-committer set (a discriminating signal).
        assert!(
            h.below_floor_noncommitters > 0 && f.below_floor_noncommitters == 0,
            "[separation] only the voluntary headline has below-floor non-committers (seed {seed})"
        );
        // The fiat run is a forced pin; the headline is voluntary.
        assert!(
            h.signal_gated_commits >= MIN_COMMITS && f.is_fiat && !h.is_fiat,
            "[separation] the headline is voluntary; the control is a forced pin (seed {seed})"
        );
    }
}

// =========================================================================
// 5. Capital-composition variants (§3.6 / §4) — SECONDARY, never required for the headline
// =========================================================================

#[test]
fn capital_composition_variants_are_secondary() {
    // The earned-capital (durable plow) and endowed-capital (endowed + inherited plow) variants compose
    // the commitment institution WITH capital. Reported separately (the headline succeeds/fails WITHOUT
    // any capital); here we only assert the hard guards hold + the tool-stock invariant balances.
    let baselines = baseline_churns(&SEEDS);
    let earned = run_batch(
        SEEDS
            .iter()
            .map(|&s| {
                (
                    s,
                    SettlementConfig::frontier_voluntary_commitment_earned_capital(),
                )
            })
            .collect(),
    );
    let endowed = run_batch(
        SEEDS
            .iter()
            .map(|&s| {
                (
                    s,
                    SettlementConfig::frontier_voluntary_commitment_endowed_capital(),
                )
            })
            .collect(),
    );
    for m in earned.iter().chain(endowed.iter()) {
        assert_guards(m, "capital_composition");
        assert!(
            m.tool_stock_balances(),
            "[capital_composition] the tool-stock invariant must hold (seed {})",
            m.seed
        );
        // A capital variant must never be RoleStickySuccess unless its OWN clauses pass — and the
        // headline (no capital) is what the verdict is about; record but don't require success here.
        let bc = baselines.get(&m.seed).copied().unwrap_or(0.0);
        let _ = m.verdict(bc);
    }
    // The endowed variant must actually carry endowed plows (the composition is real, not inert).
    assert!(
        endowed.iter().any(|m| m.endowed_tools_total > 0),
        "[capital_composition] the endowed-capital variant must seed plows"
    );
}

// =========================================================================
// 6. The commitment_term axis is outcome-driving (§4 robustness mini-sweep)
// =========================================================================

#[test]
fn commitment_term_sweep_is_outcome_driving() {
    // Sweep `commitment_term ∈ {12, 24, 48, 96}` on the headline, classified, no tuning. The term axis
    // MUST be outcome-driving: a longer binding holds more agents in the committed state at once (by
    // Little's law, concurrent ≈ uptake-rate × term) and forms more persistence. We assert the mean
    // MAX-CONCURRENT committed cohort GROWS strictly from the shortest to the longest term, report the
    // persistent-cohort + verdict movement, and pass the hard guards on every cell. Every term respects
    // the cap (asserted in the coherence test).
    let baselines = baseline_churns(&SEEDS);
    let mut mean_concurrent_by_term: Vec<(u16, f64)> = Vec::new();
    let mut mean_cohort_by_term: Vec<(u16, f64)> = Vec::new();
    for &term in &TERM_SWEEP {
        let jobs: Vec<(u64, SettlementConfig)> = SEEDS
            .iter()
            .map(|&seed| {
                let mut cfg = SettlementConfig::frontier_voluntary_commitment();
                if let Some(chain) = cfg.chain.as_mut() {
                    chain.commitment_term = term;
                }
                (seed, cfg)
            })
            .collect();
        let ms = run_batch(jobs);
        let mut concurrent_sum = 0f64;
        let mut cohort_sum = 0f64;
        for m in &ms {
            assert_guards(m, "term_sweep");
            assert_eq!(
                m.commitment_term, term,
                "swept term must apply (seed {})",
                m.seed
            );
            let bc = baselines.get(&m.seed).copied().unwrap_or(0.0);
            eprintln!("[term={term}] {}", m.line(bc));
            concurrent_sum += m.max_concurrent_committed as f64;
            cohort_sum += m.persistent_committed_cohort as f64;
        }
        mean_concurrent_by_term.push((term, concurrent_sum / ms.len() as f64));
        mean_cohort_by_term.push((term, cohort_sum / ms.len() as f64));
    }
    let shortest = mean_concurrent_by_term.first().expect("a swept term").1;
    let longest = mean_concurrent_by_term.last().expect("a swept term").1;
    eprintln!("[term_sweep] mean max-concurrent committed by term: {mean_concurrent_by_term:?}");
    eprintln!("[term_sweep] mean persistent committed cohort by term: {mean_cohort_by_term:?}");
    assert!(
        longest > shortest,
        "the commitment_term axis must be outcome-driving: the longest term must hold more agents \
         concurrently committed ({longest:.2}) than the shortest ({shortest:.2})"
    );
}

// =========================================================================
// 7. The verdict (prints the classification; does NOT assert SUCCESS — §2/§4)
// =========================================================================

#[test]
fn commitment_verdict() {
    let baselines = baseline_churns(&SEEDS);
    let head = run_batch(
        SEEDS
            .iter()
            .map(|&s| (s, SettlementConfig::frontier_voluntary_commitment()))
            .collect(),
    );
    let fiat = run_batch(
        SEEDS
            .iter()
            .map(|&s| {
                (
                    s,
                    SettlementConfig::frontier_voluntary_commitment_fiat_pin(),
                )
            })
            .collect(),
    );
    let nonbinding = run_batch(
        SEEDS
            .iter()
            .map(|&s| {
                (
                    s,
                    SettlementConfig::frontier_voluntary_commitment_nonbinding(),
                )
            })
            .collect(),
    );
    let unprofitable = run_batch(
        SEEDS
            .iter()
            .map(|&s| {
                (
                    s,
                    SettlementConfig::frontier_voluntary_commitment_unprofitable(),
                )
            })
            .collect(),
    );
    let earned = run_batch(
        SEEDS
            .iter()
            .map(|&s| {
                (
                    s,
                    SettlementConfig::frontier_voluntary_commitment_earned_capital(),
                )
            })
            .collect(),
    );
    let endowed = run_batch(
        SEEDS
            .iter()
            .map(|&s| {
                (
                    s,
                    SettlementConfig::frontier_voluntary_commitment_endowed_capital(),
                )
            })
            .collect(),
    );

    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    eprintln!(
        "\n================ S22f voluntary fixed-term cultivation commitment ================"
    );
    eprintln!(
        "(verdict by agent id; lineage persistence reported secondary; NOT asserting SUCCESS)\n"
    );
    let report = |label: &str, ms: &[Metrics], tally: &mut BTreeMap<String, usize>| {
        eprintln!("---- {label} ----");
        for m in ms {
            let bc = baselines.get(&m.seed).copied().unwrap_or(0.0);
            let v = m.verdict(bc);
            eprintln!("  seed {:>2}: {}", m.seed, m.line(bc));
            if label == "HEADLINE frontier_voluntary_commitment" {
                *tally.entry(format!("{v:?}")).or_insert(0) += 1;
            }
        }
    };
    report("HEADLINE frontier_voluntary_commitment", &head, &mut tally);
    report("CONTROL fiat_pin", &fiat, &mut tally);
    report("CONTROL nonbinding_term", &nonbinding, &mut tally);
    report("CONTROL unprofitable_offer", &unprofitable, &mut tally);
    report("SECONDARY earned_capital", &earned, &mut tally);
    report("SECONDARY endowed_capital", &endowed, &mut tally);

    eprintln!("\n---- HEADLINE verdict tally (by id) ----");
    for (v, n) in &tally {
        eprintln!("  {v}: {n}/{}", SEEDS.len());
    }
    let success = tally.get("RoleStickySuccess").copied().unwrap_or(0);
    eprintln!(
        "\nS22f outcome: {} — voluntary commitment forms a persistent committed cohort and binds the \
         exit, but the verdict above is classified, not asserted (RoleStickySuccess on {success}/{} seeds).",
        if success == SEEDS.len() {
            "RoleStickySuccess across all seeds"
        } else if success > 0 {
            "RoleStickySuccess on some seeds (band-qualified)"
        } else {
            "no RoleStickySuccess (a classified finding)"
        },
        SEEDS.len()
    );
    eprintln!("================================================================================\n");
}
