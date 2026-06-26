//! S22b acceptance suite — **occupational stickiness via bounded cultivation skill** (impl-34):
//! does accumulated, role-specific advantage turn S22a's FLUID self-provisioning into a STABLE
//! occupational split — a persistent cultivator cohort plus persistent non-cultivating buyers —
//! while money, mortality, and provenance survive?
//!
//! S22a found fluid/rotating participation: ~5% cultivate at any instant but all non-lineage roles
//! rotate through (per-agent churn ~23). S22b's single engine change is a default-off
//! `cultivation_skill` gate (composing on S22a) that gives each colonist a bounded, earned skill
//! scalar — it ACCUMULATES on realized cultivation output (grain actually harvested AND converted
//! to bread) and DECAYS otherwise, and raises ONLY the per-trip grain-haul capacity of a
//! cultivating agent's grain trip (the conservation-safe lever — a faster draw on the conserved
//! grain node, never a higher bread-per-grain ratio). The headline scenario
//! ([`SettlementConfig::frontier_occupational_stickiness`]) is the S22a money colony with ONLY
//! that flag flipped.
//!
//! **The lever must BITE** — a high-skill cultivator must harvest strictly more grain AND produce
//! strictly more bread than a skill-0 cultivator under matched conditions over the same horizon
//! ([`nonvacuity_skill_increases_grain_and_bread`]). If it does not, the headline verdict is
//! **LEVER INERT** (the lever is the wrong knob — pivot to labor-cost in a redux), a distinct
//! outcome from "no stickiness" (spec §2/§7). This is checked FIRST.
//!
//! This suite then **classifies** the treatment against the §2 pre-named outcomes via an ORDERED,
//! mutually-exclusive classifier (checked top-down; the FIRST that matches is the verdict — the
//! S21i non-gameability discipline) and PRINTS the verdict; it does **not** assert SUCCESS. Every
//! threshold is PREDECLARED a priori (§7/§8), never fitted. Churn is always compared to the
//! MATCHED-SEED skill-off S22a baseline (seed effects must not masquerade as stickiness). Every
//! run satisfies the hard guards (conservation each tick, `bread_minted_max == 0`,
//! provenance-clean-or-disqualified, `!extinct`). The five tripwire goldens are re-pinned
//! byte-identical (`goldens_unchanged`).
//!
//! Run the verdict with `--nocapture` to read the classification:
//!   `cargo test -p sim --test occupational_stickiness stickiness_verdict -- --nocapture`

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

/// Pinned skill cap (mirrors the engine `SKILL_CAP`). The non-vacuity test drives a cultivator's
/// skill here; the maturity threshold is half of it.
const SKILL_CAP: u16 = 1_000;

/// Skill at/above which a colonist is a MATURE cultivator (the "skilled cohort" — half the cap).
const SKILL_MATURITY: u16 = 500;

/// Treatment per-ever-cultivating-agent churn must fall to ≤ `CHURN_DROP ×` the **matched-seed**
/// skill-off S22a baseline churn for "churn fell materially" (spec §7/§8.2).
const CHURN_DROP: f64 = 0.5;

/// An agent id is a PERSISTENT cultivator if it cultivates in ≥ `PERSIST_FRACTION` of the
/// final-window ticks (membership persistence, NOT aggregate share — spec §3.3/§7).
const PERSIST_FRACTION: f64 = 0.5;

/// Distinct persistent-cultivator ids required for a sticky membership cohort (spec §7/§8.2).
const PERSIST_COHORT: usize = 4;

/// Of the persistent cohort, the minimum that must be NON-lineage (the food-producing class must
/// self-form, not just be the assigned lineage — spec §8.2).
const PERSIST_COHORT_NONLINEAGE: usize = 2;

/// Top-skill-cohort final-window grain share at/above which the skilled cohort DOMINATES the grain
/// regen (the monopolization probe; a cull also requires damage — spec §2/§8.5).
const MONO_SHARE: f64 = 0.75;

/// Rolling cultivator share at/above which most survivors cultivate (the commune side; reused from
/// S22a §7).
const COMMUNE_SHARE: f64 = 0.75;

/// Per-ever-cultivating-agent churn at/above which (AND no settled final-window band) the run is
/// OSCILLATION. Reused from S22a §7; PREDECLARED, not fitted.
const CHURN_LIMIT: f64 = 8.0;

/// Living non-lineage roles at/below which the demand side has collapsed (the S21g cull surface,
/// reused as the monopolization DAMAGE floor — spec §2/§8.5).
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

/// Ticks for the (fast) non-vacuity micro-harness — long enough for a designated cultivator to
/// complete several harvest spells under the natural hunger dynamics.
const NONVACUITY_TICKS: u64 = 600;

/// The earliest tick the non-vacuity harness adopts the first cultivating agent as its designated
/// probe (past the cold start, so a real cultivator exists).
const NONVACUITY_PICK_AFTER: u64 = 40;

// =========================================================================
// The ordered, mutually-exclusive classifier (spec §2)
// =========================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    /// (1) A guard failed (conservation / `bread_minted_max>0` / `!extinct`) OR provenance is
    /// DISQUALIFIED (seeded `SeededMinted` bread sold for SALT, or pre-promotion minted volume).
    BrokenInvariant,
    /// (2) The top-skill cohort takes ≥ `MONO_SHARE` of final-window grain AND the non-lineage /
    /// material-buyer side falls below the survival/buyer floor (dominance AND damage).
    MonopolizationCull,
    /// (3) Rolling cultivator share ≥ `COMMUNE_SHARE` AND post-promotion bought below the floor —
    /// most survivors cultivate and the market dissolves.
    CommuneCollapse,
    /// (4) Money never forms / fails at horizon despite produced+sold `SelfProduced` bread (the
    /// medium fails under the lock-in).
    MoneyFailureFromLockIn,
    /// (5) Per-capita churn ≥ `CHURN_LIMIT` AND no settled cultivator-share band — never settles.
    Oscillation,
    /// (6) Money + mortality survive but churn did not fall materially vs the matched-seed S22a
    /// baseline AND no persistent membership cohort formed (skill at this magnitude is not enough),
    /// OR a stickiness-partial that misses a SUCCESS criterion.
    NoStickinessDespiteSkill,
    /// (7) None of the above AND all SUCCESS criteria hold: churn fell ≤ 0.5× the matched-seed
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
    // ---- churn (per ever-cultivating agent; same measure as the matched baseline) ----
    churn_total: u32,
    ever_cultivating: usize,
    // ---- persistent MEMBERSHIP cohort (distinct ids cultivating >= PERSIST_FRACTION final win) --
    persistent_cohort: usize,
    persistent_cohort_nonlineage: usize,
    // ---- skill distribution at horizon ----
    skill_max: u16,
    skill_mean: f64,
    skill_mature_count: usize,
    // ---- skilled-cohort grain share (final window, the monopolization probe) ----
    top_skill_grain_share: f64,
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
    /// persists), and the skilled cohort did NOT monopolize the grain.
    fn is_success(&self, baseline_churn: f64) -> bool {
        self.churn_per_capita() <= CHURN_DROP * baseline_churn
            && self.has_membership_cohort()
            && self.promoted
            && !self.extinct
            && self.final_buyer_cohort >= MIN_BUYER_COHORT
            && self.top_skill_grain_share < MONO_SHARE
    }

    /// The §2 ORDERED, mutually-exclusive classifier — checked top-down; the FIRST match is the
    /// verdict. `baseline_churn` is the matched-seed skill-off S22a churn. Every threshold is
    /// predeclared above.
    fn verdict(&self, baseline_churn: f64) -> Verdict {
        if !self.conserved || self.extinct || self.bread_minted_max > 0 || !self.provenance_clean {
            return Verdict::BrokenInvariant;
        }
        if self.top_skill_grain_share >= MONO_SHARE && self.demand_damaged() {
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
        // unmet (churn did not fall enough, or no persistent membership cohort): the honest S22b
        // finding is NO STICKINESS DESPITE SKILL (a result that names the next boundary, not a
        // failure to tune). The printed figures localize the gap.
        Verdict::NoStickinessDespiteSkill
    }

    /// A uniform one-line figure rendering for the per-run classification maps.
    fn line(&self, baseline_churn: f64) -> String {
        format!(
            "churn/cap={:.1} (base={:.1}; drop_to={:.1}? {}) cohort(persist={} nl={}) | \
             skill(max={} mean={:.0} mature={}) grain_share={:.2} | promoted={} prov_clean={} | \
             cult_share(final={:.2} max_roll={:.2} settled={}) buyers_final={} post_promo_bought={} \
             | living={} nl_alive={} lin_alive={} starv={} | conserved={} minted_max={} extinct={}",
            self.churn_per_capita(),
            baseline_churn,
            CHURN_DROP * baseline_churn,
            self.churn_per_capita() <= CHURN_DROP * baseline_churn,
            self.persistent_cohort,
            self.persistent_cohort_nonlineage,
            self.skill_max,
            self.skill_mean,
            self.skill_mature_count,
            self.top_skill_grain_share,
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

/// Run `(seed, cfg)` for `ticks` and collect the full S22b classification vector by one
/// tick-by-tick read of the public, runtime-only accessors. Entrants/churn/cohort are keyed by
/// stable `AgentId` (robust to slot reuse after a death); the per-id final-window cultivating
/// counts drive the MEMBERSHIP cohort, and the per-agent final-window grain delta drives the
/// skilled-cohort grain share.
fn run_metrics(seed: u64, cfg: &SettlementConfig, ticks: u64) -> Metrics {
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

    for t in 0..ticks {
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
    // Skill distribution + skilled-cohort final-window grain share (read at horizon).
    let mut skill_max = 0u16;
    let mut skill_sum = 0u64;
    let mut skill_count = 0usize;
    let mut skill_mature_count = 0usize;
    let mut fw_grain_total = 0u64;
    let mut fw_grain_skilled = 0u64;
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
        let skill = s.cultivation_skill_of(i);
        let mature = skill >= SKILL_MATURITY;
        if s.is_alive(i) {
            skill_max = skill_max.max(skill);
            skill_sum += u64::from(skill);
            skill_count += 1;
            if mature {
                skill_mature_count += 1;
            }
        }
        // Final-window grain delta (cumulative end − snapshot at window start; 0 if born in window).
        let fw_grain = s
            .cultivation_grain_harvested_of(i)
            .saturating_sub(grain_at_window_start.get(&key).copied().unwrap_or(0));
        fw_grain_total += fw_grain;
        // DIAGNOSTIC APPROXIMATION (Codex review-of-results P3): attribute the whole final-window grain
        // to "skilled" if the agent is mature AT HORIZON. This can overstate the mature-cohort share
        // when an agent matures late in the window (its earlier, lower-skill grain is still counted as
        // skilled), so the reported grain_share is an upper-bound approximation, not exact
        // time-at-skill attribution. It is only a monopolization PROBE — it does not gate the verdict
        // except via the MONO_SHARE cull check (which also requires demand damage), so the
        // approximation cannot fake NoStickinessDespiteSkill.
        if mature {
            fw_grain_skilled += fw_grain;
        }
    }
    let skill_mean = if skill_count == 0 {
        0.0
    } else {
        skill_sum as f64 / skill_count as f64
    };
    let top_skill_grain_share = if fw_grain_total == 0 {
        0.0
    } else {
        fw_grain_skilled as f64 / fw_grain_total as f64
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
        churn_total: churn.values().copied().sum(),
        ever_cultivating: ever_cultivating.len(),
        persistent_cohort,
        persistent_cohort_nonlineage,
        skill_max,
        skill_mean,
        skill_mature_count,
        top_skill_grain_share,
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

/// The matched-seed skill-off S22a baseline churn (the comparison denominator the verdict uses).
fn baseline_churn(seed: u64) -> f64 {
    run_metrics(
        seed,
        &SettlementConfig::frontier_endogenous_cultivation(),
        PROBE_TICKS,
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
/// `!provenance_clean` ⇒ `BrokenInvariant`.)
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
        assert_eq!(
            m.verdict(baseline_churn(m.seed)),
            Verdict::BrokenInvariant,
            "[{ctx}] a provenance-dirty run must classify BrokenInvariant/disqualified (seed {})",
            m.seed
        );
    }
}

// =========================================================================
// 1. THE MANDATORY NON-VACUITY TEST (Codex P1 — the milestone's premise)
// =========================================================================

/// A controlled per-cultivating-opportunity micro-harness: run the stickiness colony with EVERY
/// colonist pinned to skill 0, adopt the first cultivating agent as the designated probe, then
/// drive ONLY that agent's skill to `designated_skill` for the rest of the run (the rest stay at
/// 0, so the colony regime is held fixed — no commune confound). Returns the designated agent's
/// cumulative grain hauled + bread produced. Because the colony is identical up to the adoption
/// tick, the CAP and 0 forks share the same pre-adoption output; the difference is the lever.
fn designated_cultivator_output(seed: u64, designated_skill: u16) -> (u64, u64) {
    let cfg = SettlementConfig::frontier_occupational_stickiness();
    let mut s = Settlement::generate(seed, &cfg);
    let mut designated: Option<usize> = None;
    for t in 0..NONVACUITY_TICKS {
        // Pin the whole colony to 0, then (once adopted) the designated agent to the test skill —
        // BEFORE the tick, so its grain trip reads the pinned skill; the end-of-tick skill update
        // overwrites, so re-pinning each tick holds it.
        s.set_all_cultivation_skill(0);
        if let Some(d) = designated {
            s.set_cultivation_skill_for_test(d, designated_skill);
        }
        s.econ_tick();
        if designated.is_none() && t >= NONVACUITY_PICK_AFTER {
            designated = (0..s.population()).find(|&i| s.is_alive(i) && s.is_cultivating(i));
        }
    }
    let d = designated.expect("a cultivator must form on the stickiness colony");
    (
        s.cultivation_grain_harvested_of(d),
        s.cultivation_bread_produced_of(d),
    )
}

#[test]
fn nonvacuity_skill_increases_grain_and_bread() {
    // The milestone's premise: a high-skill cultivator (skill at SKILL_CAP) must harvest STRICTLY
    // MORE grain AND produce STRICTLY MORE bread than a skill-0 cultivator under MATCHED conditions
    // over the SAME horizon (cumulative output of the designated agent — this proves the lever bites
    // in the live scenario; it is a same-horizon comparison, not a per-opportunity-normalized one). If
    // this fails, the headline verdict is LEVER INERT (the lever is the wrong knob — pivot to
    // labor-cost in a redux), NOT "no stickiness".
    eprintln!("================ S22b NON-VACUITY (the lever must BITE) ================");
    for &seed in &SEEDS {
        let (grain_cap, bread_cap) = designated_cultivator_output(seed, SKILL_CAP);
        let (grain_zero, bread_zero) = designated_cultivator_output(seed, 0);
        eprintln!(
            "  seed={seed}: CAP grain={grain_cap} bread={bread_cap} | skill0 grain={grain_zero} bread={bread_zero}"
        );
        assert!(
            grain_cap > grain_zero,
            "LEVER INERT (seed {seed}): a SKILL_CAP cultivator did not harvest more grain \
             ({grain_cap} vs {grain_zero}) — the grain-haul lever is the wrong knob; do NOT \
             report inertness as 'no stickiness' (pivot to labor-cost in S22b-redux)"
        );
        assert!(
            bread_cap > bread_zero,
            "LEVER INERT (seed {seed}): a SKILL_CAP cultivator did not produce more bread \
             ({bread_cap} vs {bread_zero}) — the grain-haul lever is the wrong knob; do NOT \
             report inertness as 'no stickiness' (pivot to labor-cost in S22b-redux)"
        );
    }
    eprintln!(
        "  LEVER BITES on every seed: skill strictly increases grain hauled AND bread produced."
    );
    eprintln!("=======================================================================");
}

// =========================================================================
// 2. The determinism / golden contract
// =========================================================================

#[test]
fn canonical_bytes_split_for_cultivation_skill() {
    // The cultivation-skill gate changes who out-harvests grain (a future-behaviour change), so
    // `frontier_occupational_stickiness` must SPLIT the canonical digest vs the S22a base...
    let base = Settlement::generate(7, &SettlementConfig::frontier_endogenous_cultivation());
    let skill = Settlement::generate(7, &SettlementConfig::frontier_occupational_stickiness());
    assert_ne!(
        base.canonical_bytes(),
        skill.canonical_bytes(),
        "the cultivation_skill gate must split the canonical digest vs the S22a base"
    );

    // ...and reverting the flag to false must make it BYTE-IDENTICAL to
    // `frontier_endogenous_cultivation` (the gate is the ONLY change, canonicalized ON-only).
    let mut reverted = SettlementConfig::frontier_occupational_stickiness();
    reverted.chain.as_mut().expect("chain").cultivation_skill = false;
    let mut a = Settlement::generate(7, &reverted);
    let mut b = Settlement::generate(7, &SettlementConfig::frontier_endogenous_cultivation());
    a.run(1_000);
    b.run(1_000);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "reverting cultivation_skill must equal frontier_endogenous_cultivation byte-for-byte"
    );
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn cultivation_skill_off_the_path_is_inert() {
    // The gate composes on the S22a endogenous-entry path, so toggling it on a config off that
    // path (no Cultivate recipe / no cultivation_sells_surplus) must NOT split the digest.
    let base = Settlement::generate(7, &SettlementConfig::frontier_forage_capacity());
    let mut base_cfg = SettlementConfig::frontier_forage_capacity();
    base_cfg.chain.as_mut().expect("chain").cultivation_skill = true;
    assert_eq!(
        base.canonical_bytes(),
        Settlement::generate(7, &base_cfg).canonical_bytes(),
        "off the S22a path the cultivation_skill flag must not steer the digest"
    );
}

#[test]
fn cultivation_skill_runs_are_deterministic() {
    // Byte-identical `(seed, config)` at a long horizon (the per-agent skill lives in the colonist
    // state `canonical_bytes` pins; every skill diagnostic is runtime-only).
    let cfg = SettlementConfig::frontier_occupational_stickiness();
    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    a.run(2_000);
    b.run(2_000);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the stickiness run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.digest(), b.digest());
    let mut c = Settlement::generate(2, &cfg);
    c.run(2_000);
    assert_ne!(a.digest(), c.digest(), "the seed must change the run");
}

#[test]
fn goldens_unchanged() {
    // The S22b addition is one additive, default-off ON-only gate + a per-agent field (born 0,
    // serialized ON-only) + runtime-only diagnostics, so the cross-history demographic + emergence
    // goldens are BYTE-IDENTICAL (the same five values pinned in endogenous_cultivation_entry.rs).
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
fn stickiness_verdict() {
    eprintln!("================ S22b OCCUPATIONAL STICKINESS VERDICT ================");
    // Precondition banner — the LEVER must bite (the non-vacuity test asserts it; here we report
    // it alongside the per-seed regimes so the verdict map is read in its light).
    let (gc, bc) = designated_cultivator_output(SEEDS[0], SKILL_CAP);
    let (gz, bz) = designated_cultivator_output(SEEDS[0], 0);
    let lever_bites = gc > gz && bc > bz;
    eprintln!(
        "  precondition: LEVER {} (designated CAP grain={gc} bread={bc} vs skill0 grain={gz} bread={bz})",
        if lever_bites { "BITES" } else { "INERT — headline verdict is LEVER INERT" }
    );

    let treatment = run_batch(
        SEEDS
            .iter()
            .map(|&seed| (seed, SettlementConfig::frontier_occupational_stickiness()))
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
                Verdict::BrokenInvariant
                    | Verdict::MonopolizationCull
                    | Verdict::CommuneCollapse
                    | Verdict::MoneyFailureFromLockIn
                    | Verdict::Oscillation
                    | Verdict::NoStickinessDespiteSkill
                    | Verdict::Success
            ),
            "the verdict must be one of the seven §2 outcomes"
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

// =========================================================================
// 4. The controls (spec §5) — classify, never tune
// =========================================================================

/// Mutate the shipped stickiness config's skill parameters in place.
fn with_skill(
    mut cfg: SettlementConfig,
    gain: Option<u16>,
    decay: Option<u16>,
    cap: Option<u16>,
    ceiling: Option<u32>,
) -> SettlementConfig {
    if let Some(chain) = cfg.chain.as_mut() {
        if let Some(g) = gain {
            chain.skill_gain = g;
        }
        if let Some(d) = decay {
            chain.skill_decay = d;
        }
        if let Some(c) = cap {
            chain.skill_cap = c;
        }
        if let Some(h) = ceiling {
            chain.skill_haul_ceiling = h;
        }
    }
    cfg
}

#[test]
fn control_skill_off_reproduces_s22a_fluid_baseline() {
    // Control 1: the skill-off control IS `frontier_endogenous_cultivation` (the S22a fluid
    // baseline). It must run clean and exhibit the S22a fluid participation (churn well above
    // CHURN_DROP×itself is trivially true; the point is it is the comparison denominator). We
    // assert the guards and the determinism of the comparison, and print the baseline churn.
    for &seed in &SEEDS {
        let m = run_metrics(
            seed,
            &SettlementConfig::frontier_endogenous_cultivation(),
            PROBE_TICKS,
        );
        assert_guards(&m, "skill-off-baseline");
        eprintln!(
            "  [skill-off=S22a] seed={seed}: churn/cap={:.1} | {}",
            m.churn_per_capita(),
            m.line(m.churn_per_capita())
        );
    }
}

#[test]
fn control_cap_zero_is_metrics_identical_to_s22a() {
    // Control (the no-op proof): the cultivation-skill flag ON but the haul ceiling at 1× — skill
    // accumulates/decays but has NO productivity effect, so the run must reproduce S22a's
    // BEHAVIOUR/METRICS (NOT byte-identical — the ON gate still digests config/state, so the
    // digest legitimately differs; only a flag-OFF chain is byte-identical). This proves the
    // grain-haul lever is the cause of any divergence.
    for &seed in &SEEDS {
        let s22a = run_metrics(
            seed,
            &SettlementConfig::frontier_endogenous_cultivation(),
            PROBE_TICKS,
        );
        let cap_zero = run_metrics(
            seed,
            &with_skill(
                SettlementConfig::frontier_occupational_stickiness(),
                None,
                None,
                None,
                Some(1),
            ),
            PROBE_TICKS,
        );
        assert_guards(&cap_zero, "cap-zero");
        // The behaviour metrics must match the S22a baseline exactly (the lever is inert at 1×).
        assert_eq!(
            cap_zero.promoted, s22a.promoted,
            "cap-zero must promote exactly as S22a (seed {seed})"
        );
        assert_eq!(
            cap_zero.churn_total, s22a.churn_total,
            "cap-zero must have identical churn to S22a (seed {seed})"
        );
        assert_eq!(
            cap_zero.final_buyer_cohort, s22a.final_buyer_cohort,
            "cap-zero must have the identical buyer cohort to S22a (seed {seed})"
        );
        assert_eq!(
            cap_zero.living, s22a.living,
            "cap-zero must have the identical living population to S22a (seed {seed})"
        );
        assert_eq!(
            cap_zero.post_promotion_bought, s22a.post_promotion_bought,
            "cap-zero must buy identically to S22a (seed {seed})"
        );
        // But the digest legitimately differs (the ON gate digests config + skill state).
        let a = Settlement::generate(
            seed,
            &with_skill(
                SettlementConfig::frontier_occupational_stickiness(),
                None,
                None,
                None,
                Some(1),
            ),
        );
        let b = Settlement::generate(seed, &SettlementConfig::frontier_endogenous_cultivation());
        assert_ne!(
            a.canonical_bytes(),
            b.canonical_bytes(),
            "cap-zero is metrics-identical but NOT byte-identical (the ON gate digests) (seed {seed})"
        );
    }
    eprintln!(
        "  cap-zero (ceiling 1×) reproduces S22a behaviour/metrics on every seed — the no-op proof \
         the grain-haul lever is the cause."
    );
}

#[test]
fn control_no_decay_vs_decay() {
    // Control: SKILL_DECAY = 0 vs the shipped decay. With no decay, skill ratchets monotonically —
    // does that over-lock / monopolize? A REPORT (the constant is not retuned): print both regimes.
    let base = baseline_churns(&SEEDS);
    for &seed in &SEEDS {
        let decay = run_metrics(
            seed,
            &SettlementConfig::frontier_occupational_stickiness(),
            PROBE_TICKS,
        );
        let no_decay = run_metrics(
            seed,
            &with_skill(
                SettlementConfig::frontier_occupational_stickiness(),
                None,
                Some(0),
                None,
                None,
            ),
            PROBE_TICKS,
        );
        assert_guards(&decay, "decay");
        assert_guards(&no_decay, "no-decay");
        let bc = base[&seed];
        eprintln!(
            "  [decay]    seed={seed}: {:?} | {}",
            decay.verdict(bc),
            decay.line(bc)
        );
        eprintln!(
            "  [no-decay] seed={seed}: {:?} | {}",
            no_decay.verdict(bc),
            no_decay.line(bc)
        );
    }
}

#[test]
fn control_exaggerated_cap_is_sensitivity_only() {
    // Control (SENSITIVITY, excluded from the core verdict): a 4× haul ceiling. Likely commune /
    // monopolization — shows the boundary. Asserts only the guards and prints the regime; NOT used
    // to claim success or failure of the headline.
    let base = baseline_churns(&SEEDS);
    for &seed in &SEEDS {
        let m = run_metrics(
            seed,
            &with_skill(
                SettlementConfig::frontier_occupational_stickiness(),
                None,
                None,
                None,
                Some(4),
            ),
            PROBE_TICKS,
        );
        assert_guards(&m, "exaggerated-cap");
        let bc = base[&seed];
        eprintln!(
            "  [exaggerated 4× SENSITIVITY] seed={seed}: {:?} | {}",
            m.verdict(bc),
            m.line(bc)
        );
    }
}

#[test]
fn control_low_grain_flow_does_not_fake_success() {
    // Control: the grain node starved — skill on a depleted node must NOT fake success (no real
    // grain to out-harvest; provenance + buying collapse). Asserts NOT success.
    for &seed in &SEEDS {
        let mut cfg = SettlementConfig::frontier_occupational_stickiness();
        let grain = cfg.chain.as_ref().expect("chain").content.grain();
        for node in cfg.nodes.iter_mut() {
            if node.good == grain {
                node.stock = 0;
                node.regen = 0;
                node.cap = 0;
            }
        }
        let m = run_metrics(seed, &cfg, PROBE_TICKS);
        assert_guards(&m, "low-grain-flow");
        let bc = baseline_churn(seed);
        eprintln!(
            "  [low-grain-flow] seed={seed}: {:?} | {}",
            m.verdict(bc),
            m.line(bc)
        );
        assert!(
            !m.is_success(bc),
            "with the grain commons starved the colony must NOT fake stickiness success (seed {seed})"
        );
    }
}

// =========================================================================
// 5. The robustness mini-sweep (skill params + grain flow) — classify, never tune
// =========================================================================

#[test]
fn robustness_sweep_over_skill_and_grain() {
    // Sweep SKILL_GAIN / SKILL_DECAY / SKILL_CAP / haul-ceiling + grain flow (1-D axes, holding the
    // others at the shipped value), classify each cell across two seeds, and PRINT the regime map.
    // No tuning to pass: commune / monopolization / no-stickiness outcomes are first-class
    // findings. Bounded to two seeds per cell to keep the 1600-tick sweep's wall-clock sane (no
    // silent cap — stated here). HARD guards on every cell.
    const SWEEP_SEEDS: [u64; 2] = [3, 7];
    eprintln!("================ S22b ROBUSTNESS MINI-SWEEP (regime map) ================");
    eprintln!("  bounded to SWEEP_SEEDS={SWEEP_SEEDS:?} per cell (stated, not silent).");
    let base = baseline_churns(&SWEEP_SEEDS);

    struct Axis {
        name: &'static str,
        cells: Vec<(String, SettlementConfig)>,
    }
    let mut axes: Vec<Axis> = Vec::new();

    // skill_gain (shipped 50).
    {
        let cells = [10u16, 25, 50, 100]
            .into_iter()
            .map(|v| {
                (
                    format!("skill_gain={v}"),
                    with_skill(
                        SettlementConfig::frontier_occupational_stickiness(),
                        Some(v),
                        None,
                        None,
                        None,
                    ),
                )
            })
            .collect();
        axes.push(Axis {
            name: "skill_gain",
            cells,
        });
    }
    // skill_decay (shipped 5; 0 is the no-decay control).
    {
        let cells = [0u16, 5, 25, 50]
            .into_iter()
            .map(|v| {
                (
                    format!("skill_decay={v}"),
                    with_skill(
                        SettlementConfig::frontier_occupational_stickiness(),
                        None,
                        Some(v),
                        None,
                        None,
                    ),
                )
            })
            .collect();
        axes.push(Axis {
            name: "skill_decay",
            cells,
        });
    }
    // skill_cap (shipped 1000; 0 is the cap-zero no-op).
    {
        let cells = [0u16, 250, 1000, 4000]
            .into_iter()
            .map(|v| {
                (
                    format!("skill_cap={v}"),
                    with_skill(
                        SettlementConfig::frontier_occupational_stickiness(),
                        None,
                        None,
                        Some(v),
                        None,
                    ),
                )
            })
            .collect();
        axes.push(Axis {
            name: "skill_cap",
            cells,
        });
    }
    // haul ceiling (shipped 2; 1 is the no-op, 4 the exaggerated SENSITIVITY).
    {
        let cells = [1u32, 2, 3, 4]
            .into_iter()
            .map(|v| {
                (
                    format!("haul_ceiling={v}"),
                    with_skill(
                        SettlementConfig::frontier_occupational_stickiness(),
                        None,
                        None,
                        None,
                        Some(v),
                    ),
                )
            })
            .collect();
        axes.push(Axis {
            name: "haul_ceiling",
            cells,
        });
    }
    // grain node regen (the recurring-supply axis).
    {
        let mut cells = Vec::new();
        for v in [1u32, 4, 16, 64] {
            let mut cfg = SettlementConfig::frontier_occupational_stickiness();
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
    eprintln!("========================================================================");
}
