//! S22c acceptance suite — **profit-driven cultivation retention** (impl-35): does a realized,
//! post-money profit-stay rule on the cultivation *exit* turn S22a's FLUID participation into a
//! STABLE role split — a persistent cultivator cohort plus persistent non-cultivating buyers —
//! while money, mortality, and provenance survive?
//!
//! S22b found that accumulated productivity (cultivation skill) BITES but does NOT produce
//! occupation: the cultivation *exit* is hunger-only, so agents leave as soon as hunger eases, no
//! matter how skilled. S22c's single engine change is a default-off `profit_driven_retention` gate
//! (composing on S22a; orthogonal to S22b skill) that makes the cultivation EXIT profit-modulated:
//! a currently-cultivating agent stays past the normal hunger exit when — ONLY after money exists
//! (`current_money_good() == Some(SALT)`, the hard anti-circularity gate) — its realized
//! cultivation-sale return over a rolling window clears both a material floor and its outside
//! option. Entry stays hunger/pressure-gated (S22a/b unchanged). The headline scenario
//! ([`SettlementConfig::frontier_profit_retention`]) is the S22a money colony (skill OFF) with ONLY
//! that flag flipped.
//!
//! **The signal must be NON-VACUOUS** — under the treatment a post-money agent past its hunger exit
//! with a clean cultivation-sale return ≥ its outside option must be RETAINED where the matched
//! flag-off run EXITS (a real COUNTERFACTUAL exit flip), AND the cultivation-proceeds signal must
//! VARY across agents ([`nonvacuity_counterfactual_exit_flip`]). If not, the headline verdict is
//! **SIGNAL VACUOUS** (the S22b lever-bite lesson — an inert signal is NOT "no stay"). Checked
//! FIRST.
//!
//! This suite then **classifies** the treatment against the §2 pre-named outcomes via an ORDERED,
//! mutually-exclusive classifier (checked top-down; the FIRST that matches is the verdict) and
//! PRINTS the verdict; it does **not** assert SUCCESS. Every threshold is PREDECLARED a priori
//! (§7/§8), never fitted. Churn is always compared to the MATCHED-SEED same-skill no-retention
//! baseline (S22a for the skill-off headline). Every run satisfies the hard guards (conservation
//! each tick, `bread_minted_max == 0`, provenance-clean-or-disqualified, `!extinct`). The five
//! tripwire goldens are re-pinned byte-identical (`goldens_unchanged`).
//!
//! Run the verdict with `--nocapture` to read the classification:
//!   `cargo test -p sim --test profit_driven_retention retention_verdict -- --nocapture`

use std::collections::{BTreeMap, BTreeSet};

use econ::good::SALT;
use sim::{Settlement, SettlementConfig};

// The shared S21 demand-bridge classification machinery (the living-roster helpers + the
// `PROBE_TICKS`/`SEEDS`/`MATERIAL_BOUGHT_FLOOR` constants).
#[path = "support/mod.rs"]
mod support;
use support::*;

// =========================================================================
// Predeclared thresholds (a priori — §7/§8; NEVER fitted to the data)
// =========================================================================

/// Treatment per-ever-cultivating-agent churn must fall to ≤ `CHURN_DROP ×` the **matched-seed**
/// same-skill no-retention baseline churn (S22a for the headline) for "churn fell materially".
const CHURN_DROP: f64 = 0.5;

/// An agent id is a PERSISTENT cultivator if it cultivates in ≥ `PERSIST_FRACTION` of the
/// final-window ticks (membership persistence, NOT aggregate share — spec §2/§7).
const PERSIST_FRACTION: f64 = 0.5;

/// Distinct persistent-cultivator ids required for a sticky membership cohort (spec §2/§7).
const PERSIST_COHORT: usize = 4;

/// Of the persistent cohort, the minimum that must be NON-lineage (the food-producing class must
/// self-form, not just be the assigned lineage — spec §2/§7).
const PERSIST_COHORT_NONLINEAGE: usize = 2;

/// Top-cultivator final-window grain share at/above which one cultivator DOMINATES the grain regen
/// (the monopolization probe; a cull also requires demand damage — spec §2).
const MONO_SHARE: f64 = 0.75;

/// Rolling cultivator share at/above which most survivors cultivate (the commune side; reused from
/// S22a/b §7).
const COMMUNE_SHARE: f64 = 0.75;

/// Per-ever-cultivating-agent churn at/above which (AND no settled final-window band) the run is
/// OSCILLATION. Reused from S22a/b §7; PREDECLARED, not fitted.
const CHURN_LIMIT: f64 = 8.0;

/// Living non-lineage roles at/below which the demand side has collapsed (the monopolization DAMAGE
/// floor — spec §2).
const DEMAND_COLLAPSE_FLOOR: usize = 4;

/// The rolling window (ticks) for the cultivator-share / material-buyer samples.
const ROLLING_WINDOW: usize = 100;

/// The final window (ticks) over which the cohort / grain-share / buyer band are read.
const FINAL_WINDOW: usize = 200;

/// A settled cultivator-share band's maximum range over the final window (above ⇒ not settled).
const BAND_WIDTH: f64 = 0.25;

/// Per-agent cumulative bought bread at/above which a non-cultivating non-lineage colonist counts
/// as a material BUYER (a real market transactor, not merely alive).
const MATERIAL_BUYER_FOOD: u64 = 4;

/// Living non-cultivating material buyers required in the final window for a genuine two-cohort
/// split (a non-cultivating demand side that is alive AND buying).
const MIN_BUYER_COHORT: usize = 2;

/// Distinct agents that must accrue the cultivation-proceeds signal for it to "vary across agents"
/// (not a single agent firing once — the non-vacuity premise, spec §7).
const SIGNAL_MIN_AGENTS: usize = 2;

// =========================================================================
// The ordered, mutually-exclusive classifier (spec §2)
// =========================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    /// (1) The recent-return / outside-option signal does not exist or does not discriminate — no
    /// counterfactual exit flip ever fired, or the cultivation-proceeds signal did not vary across
    /// agents. A distinct outcome from "no stay" (the S22b LEVER-INERT lesson).
    SignalVacuous,
    /// (2) A guard failed (conservation / `bread_minted_max>0` / `!extinct`) OR provenance is
    /// DISQUALIFIED (seeded `SeededMinted` bread sold for SALT, or pre-promotion minted volume).
    BrokenInvariant,
    /// (3) The top cultivator takes ≥ `MONO_SHARE` of final-window grain AND the non-lineage /
    /// material-buyer side falls below the survival/buyer floor (dominance AND damage).
    MonopolizationCull,
    /// (4) Rolling cultivator share ≥ `COMMUNE_SHARE` AND post-promotion bought below the floor —
    /// most survivors cultivate and the market dissolves.
    CommuneCollapse,
    /// (5) Money never forms / fails at horizon despite produced+sold `SelfProduced` bread (the
    /// medium fails under the lock-in).
    MoneyFailureFromLockIn,
    /// (6) Per-capita churn ≥ `CHURN_LIMIT` AND no settled cultivator-share band — never settles.
    Oscillation,
    /// (7) Money + mortality survive but churn did not fall materially vs the matched-seed S22a
    /// baseline AND no persistent membership cohort formed (the stay-decision did not translate
    /// into a persistent cohort), OR a stickiness-partial that misses a SUCCESS criterion.
    NoStayDespiteProfit,
    /// (8) None of the above AND all SUCCESS criteria hold: churn fell ≤ 0.5× the matched-seed
    /// baseline, a persistent membership cohort (`PERSIST_COHORT`, ≥`PERSIST_COHORT_NONLINEAGE`
    /// non-lineage) formed, money + mortality survive, no monopolization.
    Success,
}

/// The full per-run classification vector — every figure the §2 ordered classifier and the report
/// read, collected by one tick-by-tick pass ([`run_metrics`]). All accessors are read-only.
#[derive(Clone, Debug)]
struct Metrics {
    seed: u64,
    // ---- hard guards (asserted on every run) ----
    conserved: bool,
    bread_minted_max: u64,
    extinct: bool,
    // ---- provenance / money ----
    provenance_clean: bool,
    promoted: bool,
    // ---- the profit signal (non-vacuity precondition) ----
    profit_retained_ever: usize,
    max_retained_now: usize,
    signal_agents_max: usize,
    retained_before_promotion: bool,
    // ---- churn (per ever-cultivating agent; same measure as the matched baseline) ----
    churn_total: u32,
    ever_cultivating: usize,
    // ---- persistent MEMBERSHIP cohort (distinct ids cultivating >= PERSIST_FRACTION final win) --
    persistent_cohort: usize,
    persistent_cohort_nonlineage: usize,
    // ---- top-cultivator grain share (final window, the monopolization probe) ----
    top_cultivator_grain_share: f64,
    // ---- rolling cultivator share ----
    max_rolling_cultivator_share: f64,
    final_cultivator_share: f64,
    settled_band: bool,
    // ---- rolling non-cultivating material buyers ----
    final_buyer_cohort: usize,
    // ---- demand side ----
    living_non_lineage_final: usize,
    // ---- buying ----
    post_promotion_bought: u64,
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

    /// The non-vacuity premise (spec §7): the rule produced ≥1 counterfactual exit flip AND the
    /// cultivation-proceeds signal varied across ≥`SIGNAL_MIN_AGENTS` agents (not one agent firing
    /// once). `!signal_nonvacuous` ⇒ the headline `SignalVacuous` verdict.
    fn signal_nonvacuous(&self) -> bool {
        self.profit_retained_ever > 0 && self.signal_agents_max >= SIGNAL_MIN_AGENTS
    }

    /// The monopolization DAMAGE side: the non-lineage / material-buyer demand side has fallen
    /// below the survival/buyer floor (dominance alone with surviving buyers is NOT a cull).
    fn demand_damaged(&self) -> bool {
        self.living_non_lineage_final <= DEMAND_COLLAPSE_FLOOR
            || self.final_buyer_cohort < MIN_BUYER_COHORT
    }

    /// A sticky membership cohort: `PERSIST_COHORT` distinct persistent cultivators, of which at
    /// least `PERSIST_COHORT_NONLINEAGE` are non-lineage (the class self-formed).
    fn has_membership_cohort(&self) -> bool {
        self.persistent_cohort >= PERSIST_COHORT
            && self.persistent_cohort_nonlineage >= PERSIST_COHORT_NONLINEAGE
    }

    /// The SUCCESS conjunction (spec §2/§7): churn fell ≤ 0.5× the MATCHED-SEED baseline, a
    /// persistent membership cohort formed, money + mortality survive (a living buyer cohort
    /// persists), and the top cultivator did NOT monopolize the grain.
    fn is_success(&self, baseline_churn: f64) -> bool {
        self.churn_per_capita() <= CHURN_DROP * baseline_churn
            && self.has_membership_cohort()
            && self.promoted
            && !self.extinct
            && self.final_buyer_cohort >= MIN_BUYER_COHORT
            && self.top_cultivator_grain_share < MONO_SHARE
    }

    /// The §2 ORDERED, mutually-exclusive classifier — checked top-down; the FIRST match is the
    /// verdict. `baseline_churn` is the matched-seed same-skill no-retention churn. Every threshold
    /// is predeclared above.
    fn verdict(&self, baseline_churn: f64) -> Verdict {
        if !self.signal_nonvacuous() {
            return Verdict::SignalVacuous;
        }
        if !self.conserved || self.extinct || self.bread_minted_max > 0 || !self.provenance_clean {
            return Verdict::BrokenInvariant;
        }
        if self.top_cultivator_grain_share >= MONO_SHARE && self.demand_damaged() {
            return Verdict::MonopolizationCull;
        }
        if self.max_rolling_cultivator_share >= COMMUNE_SHARE
            && self.post_promotion_bought < MATERIAL_BOUGHT_FLOOR
        {
            return Verdict::CommuneCollapse;
        }
        if !self.promoted {
            return Verdict::MoneyFailureFromLockIn;
        }
        if self.churn_per_capita() >= CHURN_LIMIT && !self.settled_band {
            return Verdict::Oscillation;
        }
        if self.is_success(baseline_churn) {
            return Verdict::Success;
        }
        // Money + mortality survive, not commune / not oscillating, but a SUCCESS criterion is
        // unmet (churn did not fall enough, or no persistent membership cohort): the honest S22c
        // finding is NO STAY DESPITE PROFIT (a result that names the next boundary, not a failure
        // to tune). The printed figures localize the gap.
        Verdict::NoStayDespiteProfit
    }

    /// A uniform one-line figure rendering for the per-run classification maps.
    fn line(&self, baseline_churn: f64) -> String {
        format!(
            "churn/cap={:.1} (base={:.1}; drop_to={:.1}? {}) cohort(persist={} nl={}) | \
             signal(ever_retained={} max_now={} agents={} pre_money={}) grain_share={:.2} | \
             promoted={} prov_clean={} | cult_share(final={:.2} max_roll={:.2} settled={}) \
             buyers_final={} post_promo_bought={} | living={} nl_alive={} lin_alive={} starv={} | \
             conserved={} minted_max={} extinct={}",
            self.churn_per_capita(),
            baseline_churn,
            CHURN_DROP * baseline_churn,
            self.churn_per_capita() <= CHURN_DROP * baseline_churn,
            self.persistent_cohort,
            self.persistent_cohort_nonlineage,
            self.profit_retained_ever,
            self.max_retained_now,
            self.signal_agents_max,
            self.retained_before_promotion,
            self.top_cultivator_grain_share,
            self.promoted,
            self.provenance_clean,
            self.final_cultivator_share,
            self.max_rolling_cultivator_share,
            self.settled_band,
            self.final_buyer_cohort,
            self.post_promotion_bought,
            self.living,
            self.living_non_lineage_final,
            self.lineage_living,
            self.starvation,
            self.conserved,
            self.bread_minted_max,
            self.extinct,
        )
    }
}

/// How to drive each tick of [`run_metrics`] — the normal run, or the ZERO-RETURNS control that
/// empties every colonist's return window before each tick (so the rule's signal is forced absent).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Drive {
    Normal,
    ZeroReturns,
}

/// Run `(seed, cfg)` for `ticks` under `drive` and collect the full S22c classification vector by
/// one tick-by-tick read of the public, runtime-only accessors. Entrants/churn/cohort are keyed by
/// stable `AgentId` (robust to slot reuse after a death); the per-id final-window cultivating
/// counts drive the MEMBERSHIP cohort, and the per-agent final-window grain delta drives the
/// top-cultivator grain share.
fn run_metrics(seed: u64, cfg: &SettlementConfig, ticks: u64, drive: Drive) -> Metrics {
    let mut s = Settlement::generate(seed, cfg);
    let bread = bread_good(&s);

    let mut conserved = true;
    let mut bread_minted_max = 0u64;

    let mut was_cultivating: BTreeMap<u64, bool> = BTreeMap::new();
    let mut churn: BTreeMap<u64, u32> = BTreeMap::new();
    let mut ever_cultivating: BTreeSet<u64> = BTreeSet::new();

    let mut share_samples: Vec<f64> = Vec::with_capacity(ticks as usize);
    let mut buyer_samples: Vec<usize> = Vec::with_capacity(ticks as usize);

    // MEMBERSHIP cohort: per-id count of cultivating ticks within the final window, plus the
    // lineage tag (read at the end, but recorded here for ids that may die before the end).
    let final_window_start = ticks.saturating_sub(FINAL_WINDOW as u64);
    let mut final_window_cultivating: BTreeMap<u64, u32> = BTreeMap::new();
    // Per-agent cumulative grain snapshot at the final-window start (for the grain-share delta).
    let mut grain_at_window_start: BTreeMap<u64, u64> = BTreeMap::new();
    let mut window_start_snapshotted = false;

    let mut promoted = false;
    let mut bought_at_promotion: Option<u64> = None;

    // The profit-signal diagnostics.
    let mut max_retained_now = 0usize;
    let mut signal_agents_max = 0usize;
    let mut retained_before_promotion = false;

    for t in 0..ticks {
        if drive == Drive::ZeroReturns {
            s.clear_cultivation_return_windows();
        }
        let report = s.econ_tick();
        conserved &= report.conserves();
        bread_minted_max = bread_minted_max.max(report.endowment_of(bread));

        // Snapshot per-agent cumulative grain exactly once, at the first tick of the final window.
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
        let mut signal_agents = 0usize;
        for i in 0..pop {
            if !s.is_alive(i) {
                continue;
            }
            alive_count += 1;
            let Some(id) = s.colonist_id(i) else { continue };
            let key = id.0;
            let lineage = s.household_of(i).is_some();
            let cultivating = s.is_cultivating(i);
            if cultivating {
                cultivating_count += 1;
                ever_cultivating.insert(key);
                if t >= final_window_start {
                    *final_window_cultivating.entry(key).or_insert(0) += 1;
                }
            }
            let prev = was_cultivating.get(&key).copied().unwrap_or(false);
            if cultivating != prev {
                *churn.entry(key).or_insert(0) += 1;
            }
            was_cultivating.insert(key, cultivating);
            if !lineage && !cultivating && s.bought_food_of(i) >= MATERIAL_BUYER_FOOD {
                buyers += 1;
            }
            if s.recent_cultivation_proceeds_of(i) > 0 {
                signal_agents += 1;
            }
        }
        let share = if alive_count == 0 {
            0.0
        } else {
            cultivating_count as f64 / alive_count as f64
        };
        share_samples.push(share);
        buyer_samples.push(buyers);
        signal_agents_max = signal_agents_max.max(signal_agents);

        let retained_now = s.profit_retained_now();
        max_retained_now = max_retained_now.max(retained_now);
        if retained_now > 0 && !promoted {
            retained_before_promotion = true;
        }

        if !promoted && s.current_money_good() == Some(SALT) {
            promoted = true;
            bought_at_promotion = Some(s.acquisition_consumed_by_channel().bought);
        }
    }
    // The retained-ever set only grows, so the final count is the run max.
    let profit_retained_ever = s.profit_retained_ever_count();

    // Rolling cultivator-share statistics.
    let max_rolling_cultivator_share = rolling_means(&share_samples, ROLLING_WINDOW)
        .into_iter()
        .fold(0.0f64, f64::max);
    let final_slice = &share_samples[share_samples.len().saturating_sub(FINAL_WINDOW)..];
    let final_cultivator_share = mean(final_slice);
    let band_min = final_slice.iter().copied().fold(f64::INFINITY, f64::min);
    let band_max = final_slice
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let settled_band = final_slice.is_empty() || (band_max - band_min) <= BAND_WIDTH;
    let final_buyer_cohort = buyer_samples[buyer_samples.len().saturating_sub(FINAL_WINDOW)..]
        .iter()
        .copied()
        .max()
        .unwrap_or(0);

    // The persistent MEMBERSHIP cohort: ids cultivating in >= PERSIST_FRACTION of the final window.
    let persist_threshold = (PERSIST_FRACTION * FINAL_WINDOW as f64).ceil() as u32;
    let mut persistent_cohort = 0usize;
    let mut persistent_cohort_nonlineage = 0usize;
    // Top-cultivator final-window grain share (read at horizon): the single largest cultivator's
    // share of the final-window grain regen — the monopolization probe.
    let mut fw_grain_total = 0u64;
    let mut fw_grain_top = 0u64;
    let lineage_by_id: BTreeMap<u64, bool> = (0..s.population())
        .filter_map(|i| {
            s.colonist_id(i)
                .map(|id| (id.0, s.household_of(i).is_some()))
        })
        .collect();
    for i in 0..s.population() {
        let Some(id) = s.colonist_id(i) else { continue };
        let key = id.0;
        // Persistent membership (over any id seen cultivating in the window — living or since dead).
        if let Some(&count) = final_window_cultivating.get(&key) {
            if count >= persist_threshold {
                persistent_cohort += 1;
                if !lineage_by_id.get(&key).copied().unwrap_or(false) {
                    persistent_cohort_nonlineage += 1;
                }
            }
        }
        // Final-window grain delta (cumulative end − snapshot at window start; 0 if born in window).
        let fw_grain = s
            .cultivation_grain_harvested_of(i)
            .saturating_sub(grain_at_window_start.get(&key).copied().unwrap_or(0));
        fw_grain_total += fw_grain;
        fw_grain_top = fw_grain_top.max(fw_grain);
    }
    let top_cultivator_grain_share = if fw_grain_total == 0 {
        0.0
    } else {
        fw_grain_top as f64 / fw_grain_total as f64
    };

    let consumed = s.acquisition_consumed_by_channel();
    let (_pp_produced, pp_minted) = s.pre_promotion_bread_for_salt_by_provenance();
    let provenance_clean = s.seeded_minted_bread_sold_for_salt() == 0 && pp_minted == 0;
    let bought_total = consumed.bought;
    let post_promotion_bought = match bought_at_promotion {
        Some(at) => bought_total.saturating_sub(at),
        None => 0,
    };

    Metrics {
        seed,
        conserved,
        bread_minted_max,
        extinct: living(&s) == 0,
        provenance_clean,
        promoted: s.current_money_good() == Some(SALT),
        profit_retained_ever,
        max_retained_now,
        signal_agents_max,
        retained_before_promotion,
        churn_total: churn.values().copied().sum(),
        ever_cultivating: ever_cultivating.len(),
        persistent_cohort,
        persistent_cohort_nonlineage,
        top_cultivator_grain_share,
        max_rolling_cultivator_share,
        final_cultivator_share,
        settled_band,
        final_buyer_cohort,
        living_non_lineage_final: living_non_lineage(&s),
        post_promotion_bought,
        living: living(&s),
        lineage_living: living_lineage(&s),
        starvation: s.starvation_deaths_total(),
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

/// Run a batch of labelled `(seed, cfg)` jobs concurrently (bounded scoped-thread fan-out) and
/// return the collected `Metrics` in input order. The engine has no global mutable state, so
/// concurrent `Settlement` runs are deterministic per `(seed, config)`.
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
                        .map(|(i, seed, cfg)| {
                            (i, run_metrics(seed, &cfg, PROBE_TICKS, Drive::Normal))
                        })
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

/// The matched-seed same-skill no-retention baseline churn (S22a for the headline). The comparison
/// denominator the verdict uses.
fn baseline_churn(seed: u64) -> f64 {
    run_metrics(
        seed,
        &SettlementConfig::frontier_endogenous_cultivation(),
        PROBE_TICKS,
        Drive::Normal,
    )
    .churn_per_capita()
}

/// The matched-seed baselines for a set of seeds, computed once in parallel (so a control/sweep
/// does not re-run the 1600-tick S22a baseline per cell).
fn baseline_churns(seeds: &[u64]) -> BTreeMap<u64, f64> {
    let jobs: Vec<(u64, SettlementConfig)> = seeds
        .iter()
        .map(|&seed| (seed, SettlementConfig::frontier_endogenous_cultivation()))
        .collect();
    seeds
        .iter()
        .copied()
        .zip(run_batch(jobs).into_iter().map(|m| m.churn_per_capita()))
        .collect()
}

/// The hard guards every run must satisfy regardless of regime (spec §7): conservation each tick,
/// no bread minted, no extinction. (Provenance-clean-or-disqualified is handled by the verdict:
/// `!provenance_clean` ⇒ `BrokenInvariant`, unless the signal is vacuous, which takes precedence.)
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
    if !m.provenance_clean {
        assert!(
            matches!(
                m.verdict(baseline_churn(m.seed)),
                Verdict::BrokenInvariant | Verdict::SignalVacuous
            ),
            "[{ctx}] a provenance-dirty run must classify BrokenInvariant/disqualified (seed {})",
            m.seed
        );
    }
}

// =========================================================================
// 1. THE MANDATORY NON-VACUITY TEST (spec §7 — a real counterfactual exit FLIP)
// =========================================================================

#[test]
fn nonvacuity_counterfactual_exit_flip() {
    // The milestone's premise (spec §7): under the treatment, ≥1 post-money agent with
    // `hunger < cultivate_hunger_out`, no input in flight, and a clean cultivation-sale return ≥ its
    // outside option is RETAINED where the matched flag-off run (S22a) would have EXITED — a
    // DEMONSTRATED counterfactual stay — AND the cultivation-proceeds signal VARIES across agents.
    //
    // The flip is shown in lockstep: run the treatment (ON) and the matched S22a baseline (OFF) from
    // the same seed. The return window steers behaviour ONLY through `profit_stay_active`, which only
    // changes the EXIT on a counterfactual flip — so up to the FIRST flip the two runs are
    // behaviourally identical, and at that tick the OFF run, from the same state, exits the very
    // agent the ON run retains. If no flip ever fires (or the signal never varies), the headline is
    // SIGNAL VACUOUS, not "no stay" (the S22b lever-bite lesson).
    eprintln!("============ S22c NON-VACUITY (a real counterfactual exit FLIP) ============");
    for &seed in &SEEDS {
        let mut on = Settlement::generate(seed, &SettlementConfig::frontier_profit_retention());
        let mut off =
            Settlement::generate(seed, &SettlementConfig::frontier_endogenous_cultivation());
        let cult_out = on.cultivate_hunger_out();

        let mut flip_tick: Option<u64> = None;
        let mut flips = 0usize;
        for t in 0..PROBE_TICKS {
            on.econ_tick();
            off.econ_tick();
            if on.profit_retained_now() == 0 {
                continue;
            }
            // The FIRST tick the ON run retains an agent by profit: up to here the two runs are
            // behaviourally identical, so OFF (same state) exits each retained agent.
            for i in 0..on.population() {
                if !on.is_profit_retained(i) {
                    continue;
                }
                let id = on.colonist_id(i).expect("a retained agent has an id");
                let hunger = on.need_of(i).map(|n| n.hunger).unwrap_or(u16::MAX);
                // The retained agent satisfies every flip precondition by construction.
                assert!(
                    on.is_cultivating(i),
                    "seed {seed}: a retained agent must be cultivating in the ON run"
                );
                assert_eq!(
                    on.current_money_good(),
                    Some(SALT),
                    "seed {seed}: retention is post-money only (the anti-circularity gate)"
                );
                assert!(
                    hunger < cult_out,
                    "seed {seed}: a retained agent is past the hunger exit ({hunger} < {cult_out})"
                );
                // The COUNTERFACTUAL: the matched flag-off run exits the same agent.
                let off_i = (0..off.population()).find(|&j| off.colonist_id(j) == Some(id));
                let off_cultivating = off_i.is_some_and(|j| off.is_cultivating(j));
                if !off_cultivating {
                    flips += 1;
                }
            }
            flip_tick = Some(t);
            break;
        }

        // The signal must VARY across agents (not a single agent firing once): run the SAME
        // treatment settlement on to the horizon and require ≥ SIGNAL_MIN_AGENTS distinct agents to
        // have accrued cultivation proceeds, with ≥2 distinct positive proceeds magnitudes.
        if let Some(t) = flip_tick {
            on.run(PROBE_TICKS - 1 - t);
        }
        let signal_agents = (0..on.population())
            .filter(|&i| on.recent_cultivation_proceeds_of(i) > 0)
            .count();
        let distinct_values: BTreeSet<u64> = (0..on.population())
            .map(|i| on.recent_cultivation_proceeds_of(i))
            .filter(|&p| p > 0)
            .collect();

        eprintln!(
            "  seed={seed}: flip@{flip_tick:?} flips={flips} | signal_agents={signal_agents} \
             distinct_proceeds_values={} ever_retained={}",
            distinct_values.len(),
            on.profit_retained_ever_count(),
        );
        assert!(
            flip_tick.is_some() && flips >= 1,
            "SIGNAL VACUOUS (seed {seed}): no counterfactual exit flip fired under the treatment — \
             a post-money agent was never RETAINED where the flag-off run would have exited; do NOT \
             report this as 'no stay' (it is the SignalVacuous finding)"
        );
        assert!(
            signal_agents >= SIGNAL_MIN_AGENTS && distinct_values.len() >= 2,
            "SIGNAL VACUOUS (seed {seed}): the cultivation-proceeds signal did not vary across \
             agents (agents={signal_agents}, distinct_values={}) — a single agent firing once is \
             not a signal",
            distinct_values.len(),
        );
    }
    eprintln!(
        "  SIGNAL NON-VACUOUS on every seed: a real counterfactual exit flip fires AND the \
         cultivation-proceeds signal varies across agents."
    );
    eprintln!("===========================================================================");
}

// =========================================================================
// 2. The determinism / golden contract
// =========================================================================

#[test]
fn canonical_bytes_split_for_profit_retention() {
    // The profit-retention gate changes who stays cultivating (a future-behaviour change), so
    // `frontier_profit_retention` must SPLIT the canonical digest vs the S22a base...
    let base = Settlement::generate(7, &SettlementConfig::frontier_endogenous_cultivation());
    let retain = Settlement::generate(7, &SettlementConfig::frontier_profit_retention());
    assert_ne!(
        base.canonical_bytes(),
        retain.canonical_bytes(),
        "the profit_driven_retention gate must split the canonical digest vs the S22a base"
    );

    // ...and reverting the flag to false must make it BYTE-IDENTICAL to
    // `frontier_endogenous_cultivation` (the gate is the ONLY change, canonicalized ON-only).
    let mut reverted = SettlementConfig::frontier_profit_retention();
    reverted
        .chain
        .as_mut()
        .expect("chain")
        .profit_driven_retention = false;
    let mut a = Settlement::generate(7, &reverted);
    let mut b = Settlement::generate(7, &SettlementConfig::frontier_endogenous_cultivation());
    a.run(1_000);
    b.run(1_000);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "reverting profit_driven_retention must equal frontier_endogenous_cultivation byte-for-byte"
    );
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn profit_retention_off_the_path_is_inert() {
    // The gate composes on the S22a endogenous-entry path, so toggling it on a config off that path
    // (no Cultivate recipe / no cultivation_sells_surplus) must NOT split the digest.
    let base = Settlement::generate(7, &SettlementConfig::frontier_forage_capacity());
    let mut base_cfg = SettlementConfig::frontier_forage_capacity();
    base_cfg
        .chain
        .as_mut()
        .expect("chain")
        .profit_driven_retention = true;
    assert_eq!(
        base.canonical_bytes(),
        Settlement::generate(7, &base_cfg).canonical_bytes(),
        "off the S22a path the profit_driven_retention flag must not steer the digest"
    );
}

#[test]
fn profit_retention_runs_are_deterministic() {
    // Byte-identical `(seed, config)` at a long horizon (the per-agent return window lives in the
    // colonist state `canonical_bytes` pins; every flip/proceeds diagnostic is runtime-only).
    let cfg = SettlementConfig::frontier_profit_retention();
    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    a.run(2_000);
    b.run(2_000);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the retention run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.digest(), b.digest());
    let mut c = Settlement::generate(2, &cfg);
    c.run(2_000);
    assert_ne!(a.digest(), c.digest(), "the seed must change the run");
}

#[test]
fn goldens_unchanged() {
    // The S22c addition is one additive, default-off ON-only gate + a per-agent window (born empty,
    // serialized ON-only) + runtime-only diagnostics, so the cross-history demographic + emergence
    // goldens are BYTE-IDENTICAL (the same five values pinned in occupational_stickiness.rs).
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
// 3. The headline verdict (the ordered classifier; prints, does NOT assert SUCCESS)
// =========================================================================

#[test]
fn retention_verdict() {
    eprintln!("================ S22c PROFIT-DRIVEN RETENTION VERDICT ================");
    let treatment = run_batch(
        SEEDS
            .iter()
            .map(|&seed| (seed, SettlementConfig::frontier_profit_retention()))
            .collect(),
    );
    let baselines = run_batch(
        SEEDS
            .iter()
            .map(|&seed| (seed, SettlementConfig::frontier_endogenous_cultivation()))
            .collect(),
    );

    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    for (m, base) in treatment.iter().zip(baselines.iter()) {
        assert_guards(m, "verdict");
        let base_churn = base.churn_per_capita();
        let v = m.verdict(base_churn);
        *tally.entry(format!("{v:?}")).or_insert(0) += 1;
        eprintln!("  seed={:>2}: {v:?} | {}", m.seed, m.line(base_churn));
        assert!(
            matches!(
                v,
                Verdict::SignalVacuous
                    | Verdict::BrokenInvariant
                    | Verdict::MonopolizationCull
                    | Verdict::CommuneCollapse
                    | Verdict::MoneyFailureFromLockIn
                    | Verdict::Oscillation
                    | Verdict::NoStayDespiteProfit
                    | Verdict::Success
            ),
            "the verdict must be one of the eight §2 outcomes"
        );
        // BrokenInvariant is never an acceptable regime — a hard failure (the guards above enforce
        // conservation/minting/extinction; this catches a provenance disqualify).
        assert_ne!(
            v,
            Verdict::BrokenInvariant,
            "no run may be a broken invariant (seed {})",
            m.seed
        );
    }
    eprintln!("---- verdict tally across SEEDS={SEEDS:?}: {tally:?} ----");
    eprintln!("=====================================================================");
}

#[test]
fn skill_on_variant_verdict_is_composition() {
    // The skill-ON variant (S22b base + the flag), compared to its MATCHED no-retention baseline
    // (S22b — frontier_occupational_stickiness), reported as composition (skill may raise the
    // surplus a cultivator can sell, but the stay is mediated by realized gain, not by "skilled").
    // Prints the regime; asserts only the guards + a valid verdict (does NOT assert SUCCESS).
    eprintln!("======== S22c PROFIT-RETENTION × SKILL (composition) VERDICT ========");
    let treatment = run_batch(
        SEEDS
            .iter()
            .map(|&seed| (seed, SettlementConfig::frontier_profit_retention_skill()))
            .collect(),
    );
    let baselines = run_batch(
        SEEDS
            .iter()
            .map(|&seed| (seed, SettlementConfig::frontier_occupational_stickiness()))
            .collect(),
    );
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    for (m, base) in treatment.iter().zip(baselines.iter()) {
        assert_guards(m, "skill-variant-verdict");
        let bc = base.churn_per_capita();
        let v = m.verdict(bc);
        *tally.entry(format!("{v:?}")).or_insert(0) += 1;
        eprintln!("  seed={:>2}: {v:?} | {}", m.seed, m.line(bc));
        assert_ne!(
            v,
            Verdict::BrokenInvariant,
            "no skill-variant run may be a broken invariant (seed {})",
            m.seed
        );
    }
    eprintln!("---- skill-variant tally across SEEDS={SEEDS:?}: {tally:?} ----");
    eprintln!("=====================================================================");
}

// =========================================================================
// 4. The controls (spec §5) — classify, never tune
// =========================================================================

#[test]
fn control_flag_off_reproduces_s22a_fluid_baseline() {
    // Control 1: the flag-off control IS `frontier_endogenous_cultivation` (the S22a fluid
    // baseline) — the matched-skill no-retention baseline the churn-drop bar is measured against. It
    // must run clean and exhibit the S22a fluid participation (and fire NO retention — the rule is
    // off). Asserts the guards + that no agent is ever retained-by-profit, and prints the churn.
    for &seed in &SEEDS {
        let m = run_metrics(
            seed,
            &SettlementConfig::frontier_endogenous_cultivation(),
            PROBE_TICKS,
            Drive::Normal,
        );
        assert_guards(&m, "flag-off-baseline");
        assert_eq!(
            m.profit_retained_ever, 0,
            "the flag-off S22a baseline must never retain by profit (seed {seed})"
        );
        eprintln!(
            "  [flag-off=S22a] seed={seed}: churn/cap={:.1} | {}",
            m.churn_per_capita(),
            m.line(m.churn_per_capita())
        );
    }
}

#[test]
fn control_signal_inert_pre_money() {
    // Signal-unavailable control (spec §5): the profit-stay rule is HARD-GATED on
    // `current_money_good() == Some(SALT)`, so before promotion the signal is unavailable and the
    // rule must create NO stickiness. Asserts that NO agent is ever retained-by-profit before the
    // promotion tick (the rule is inert pre-money) — proving it needs the realized post-money
    // signal, not the rule's mere presence.
    for &seed in &SEEDS {
        let m = run_metrics(
            seed,
            &SettlementConfig::frontier_profit_retention(),
            PROBE_TICKS,
            Drive::Normal,
        );
        assert_guards(&m, "signal-inert-pre-money");
        assert!(
            !m.retained_before_promotion,
            "the profit-stay rule must be inert pre-money (no retention before promotion) (seed {seed})"
        );
        eprintln!(
            "  [signal-inert-pre-money] seed={seed}: ever_retained={} (all post-money) | promoted={}",
            m.profit_retained_ever, m.promoted
        );
    }
}

#[test]
fn control_zero_returns_kills_stickiness() {
    // Shuffle/zero-returns control (spec §5): keep the rule ON but force the return signal EMPTY
    // (every colonist's window cleared before each tick). With no signal the rule can never fire, so
    // stickiness must DISAPPEAR — the run must reproduce the S22a fluid baseline's churn and fire NO
    // retention. This proves the SIGNAL (not the rule's mere presence) drives any stickiness.
    let base = baseline_churns(&SEEDS);
    for &seed in &SEEDS {
        let zero = run_metrics(
            seed,
            &SettlementConfig::frontier_profit_retention(),
            PROBE_TICKS,
            Drive::ZeroReturns,
        );
        assert_guards(&zero, "zero-returns");
        assert_eq!(
            zero.profit_retained_ever, 0,
            "with the returns zeroed the rule must never fire (seed {seed})"
        );
        let bc = base[&seed];
        eprintln!(
            "  [zero-returns] seed={seed}: {:?} | churn/cap={:.1} (base={:.1}) | {}",
            zero.verdict(bc),
            zero.churn_per_capita(),
            bc,
            zero.line(bc)
        );
        assert!(
            !zero.is_success(bc),
            "the zero-returns control must NOT fake stickiness success (seed {seed})"
        );
    }
    eprintln!(
        "  zero-returns kills retention on every seed — the SIGNAL, not the rule's presence, drives \
         any stickiness."
    );
}

#[test]
fn control_skill_on_profit_off_is_s22b() {
    // Control (spec §5): skill-ON + profit-stay-OFF = S22b (`frontier_occupational_stickiness`).
    // Compared to its OWN S22b baseline it must NOT be a profit-driven success and must fire NO
    // retention — isolating that PROFIT-STAY (not skill) is the new lever.
    for &seed in &SEEDS {
        let m = run_metrics(
            seed,
            &SettlementConfig::frontier_occupational_stickiness(),
            PROBE_TICKS,
            Drive::Normal,
        );
        assert_guards(&m, "skill-on-profit-off");
        assert_eq!(
            m.profit_retained_ever, 0,
            "S22b (profit-stay off) must never retain by profit (seed {seed})"
        );
        // Its baseline is the S22b colony itself, so churn/cap == baseline ⇒ never ≤ 0.5×.
        let bc = m.churn_per_capita();
        eprintln!(
            "  [skill-on profit-off=S22b] seed={seed}: {:?} | {}",
            m.verdict(bc),
            m.line(bc)
        );
        assert!(
            !m.is_success(bc),
            "S22b alone (no profit-stay) must not be a profit-driven success (seed {seed})"
        );
    }
}

#[test]
fn control_high_retention_sensitivity_is_excluded() {
    // SENSITIVITY control (spec §5, excluded from the core verdict): a permissive regime — material
    // floor 0 (any dust sale qualifies) AND a long window (4× the shipped one). Likely commune /
    // monopolization — shows the boundary. Asserts only the guards and prints the regime; NOT used
    // to claim success or failure of the headline.
    let base = baseline_churns(&SEEDS);
    for &seed in &SEEDS {
        let mut cfg = SettlementConfig::frontier_profit_retention();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.retention_material_floor = 0;
            chain.return_window = 192;
        }
        let m = run_metrics(seed, &cfg, PROBE_TICKS, Drive::Normal);
        assert_guards(&m, "high-retention-sensitivity");
        let bc = base[&seed];
        eprintln!(
            "  [permissive SENSITIVITY floor=0 window=192] seed={seed}: {:?} | {}",
            m.verdict(bc),
            m.line(bc)
        );
    }
}

#[test]
fn control_low_grain_flow_does_not_fake_success() {
    // Control: the grain node starved — profit-stay on a depleted node must NOT fake success (no
    // grain to cultivate/sell ⇒ no cultivation proceeds ⇒ the rule cannot fire; provenance + buying
    // collapse). Asserts NOT success.
    for &seed in &SEEDS {
        let mut cfg = SettlementConfig::frontier_profit_retention();
        let grain = cfg.chain.as_ref().expect("chain").content.grain();
        for node in cfg.nodes.iter_mut() {
            if node.good == grain {
                node.stock = 0;
                node.regen = 0;
                node.cap = 0;
            }
        }
        let m = run_metrics(seed, &cfg, PROBE_TICKS, Drive::Normal);
        assert_guards(&m, "low-grain-flow");
        let bc = baseline_churn(seed);
        eprintln!(
            "  [low-grain-flow] seed={seed}: {:?} | {}",
            m.verdict(bc),
            m.line(bc)
        );
        assert!(
            !m.is_success(bc),
            "with the grain commons starved the colony must NOT fake retention success (seed {seed})"
        );
    }
}

// =========================================================================
// 5. The robustness mini-sweep (window / margin / floor + grain flow) — classify, never tune
// =========================================================================

#[test]
fn robustness_sweep_over_window_margin_and_grain() {
    // Sweep RETURN_WINDOW / RETENTION_MARGIN_BPS / material floor + grain flow (1-D axes, holding
    // the others at the shipped value), classify each cell across two seeds, and PRINT the regime
    // map. No tuning to pass: commune / monopolization / no-stay outcomes are first-class findings.
    // Bounded to two seeds per cell to keep the 1600-tick sweep's wall-clock sane (no silent cap —
    // stated here). HARD guards on every cell.
    const SWEEP_SEEDS: [u64; 2] = [3, 7];
    eprintln!("============ S22c ROBUSTNESS MINI-SWEEP (regime map) ============");
    eprintln!("  bounded to SWEEP_SEEDS={SWEEP_SEEDS:?} per cell (stated, not silent).");
    let base = baseline_churns(&SWEEP_SEEDS);

    struct Axis {
        name: &'static str,
        cells: Vec<(String, SettlementConfig)>,
    }
    let with_retention = |f: &dyn Fn(&mut sim::ChainConfig)| {
        let mut cfg = SettlementConfig::frontier_profit_retention();
        if let Some(chain) = cfg.chain.as_mut() {
            f(chain);
        }
        cfg
    };
    let mut axes: Vec<Axis> = Vec::new();

    // return_window (shipped 48).
    {
        let cells = [12u64, 24, 48, 96]
            .into_iter()
            .map(|v| {
                (
                    format!("return_window={v}"),
                    with_retention(&move |c| c.return_window = v),
                )
            })
            .collect();
        axes.push(Axis {
            name: "return_window",
            cells,
        });
    }
    // retention_margin_bps (shipped 0; higher = stricter outside bar).
    {
        let cells = [0u64, 500, 2_000, 10_000]
            .into_iter()
            .map(|v| {
                (
                    format!("retention_margin_bps={v}"),
                    with_retention(&move |c| c.retention_margin_bps = v),
                )
            })
            .collect();
        axes.push(Axis {
            name: "retention_margin_bps",
            cells,
        });
    }
    // retention_material_floor (shipped 2; 0 permissive, higher = stricter).
    {
        let cells = [0u64, 2, 8, 32]
            .into_iter()
            .map(|v| {
                (
                    format!("material_floor={v}"),
                    with_retention(&move |c| c.retention_material_floor = v),
                )
            })
            .collect();
        axes.push(Axis {
            name: "material_floor",
            cells,
        });
    }
    // grain node regen (the recurring-supply axis).
    {
        let mut cells = Vec::new();
        for v in [1u32, 4, 16, 64] {
            let mut cfg = SettlementConfig::frontier_profit_retention();
            let grain = cfg.chain.as_ref().expect("chain").content.grain();
            for node in cfg.nodes.iter_mut() {
                if node.good == grain {
                    node.regen = v;
                }
            }
            cells.push((format!("grain_regen={v}"), cfg));
        }
        axes.push(Axis {
            name: "grain_regen",
            cells,
        });
    }

    for axis in &axes {
        eprintln!("---- axis: {} ----", axis.name);
        let jobs: Vec<(u64, SettlementConfig)> = axis
            .cells
            .iter()
            .flat_map(|(_, cfg)| SWEEP_SEEDS.iter().map(move |&seed| (seed, cfg.clone())))
            .collect();
        let results = run_batch(jobs);
        for (cell_idx, (label, _)) in axis.cells.iter().enumerate() {
            for (k, &seed) in SWEEP_SEEDS.iter().enumerate() {
                let m = &results[cell_idx * SWEEP_SEEDS.len() + k];
                assert_guards(m, axis.name);
                let bc = base[&seed];
                eprintln!(
                    "  {label} seed={seed}: {:?} | {}",
                    m.verdict(bc),
                    m.line(bc)
                );
            }
        }
    }
    eprintln!("================================================================");
}
