//! S22a acceptance suite — **endogenous cultivation entry** (impl-33): does the food-producing
//! class form from agent hunger PRESSURE rather than lineage IDENTITY, while the open colony
//! still supports money and mortality?
//!
//! Through S21 the open colony's cultivator lineage is PINNED: a pre-identified spatial-household
//! lineage cultivates `SelfProduced` bread and sells the surplus, while the non-lineage roles
//! (the SALT-rich buyers + the woodcutters) never cultivate. S22a's single engine change is a
//! default-off `endogenous_cultivation_entry` gate that relaxes cultivation eligibility from
//! "spatial household lineage member" to "any spatial colonist under sustained hunger" (the
//! `Consumer|Gatherer|Unassigned` vocation filter preserved), reusing the EXISTING S15/S21f
//! pressure/patience hysteresis — no profit optimizer, no new threshold. The headline scenario
//! ([`SettlementConfig::frontier_endogenous_cultivation`]) is the S21h.1 demand-bridged money
//! colony (mortality on) with ONLY that flag flipped.
//!
//! This suite **classifies** the outcome against the §2 pre-named outcomes via an ORDERED,
//! mutually-exclusive classifier (checked top-down; the FIRST that matches is the verdict — the
//! S21i non-gameability discipline) and PRINTS the verdict; it does **not** assert SUCCESS. Every
//! numeric threshold is PREDECLARED a priori (§7), never fitted to the data. Every run satisfies
//! the hard guards (conservation each tick, `bread_minted_max == 0`, provenance-clean-or-
//! disqualified, `!extinct`). The five tripwire goldens are re-pinned byte-identical
//! (`goldens_unchanged`); off the flag the chain is byte-identical to the S21h stream
//! (`canonical_bytes_split_for_endogenous_cultivation`). All diagnostics — the entrant-class
//! provenance split, the rolling cultivator/buyer samples, the role churn, the per-agent bought
//! counter — are runtime-only, never in `canonical_bytes`.
//!
//! Run the verdict with `--nocapture` to read the classification:
//!   `cargo test -p sim --test endogenous_cultivation_entry endogenous_entry_verdict -- --nocapture`

use std::collections::{BTreeMap, BTreeSet};

use econ::good::SALT;
use sim::{Settlement, SettlementConfig};

// The shared S21 demand-bridge classification machinery (the `Cell`/`Regime`/`classify` vector,
// the living-roster helpers, and the `PROBE_TICKS`/`SEEDS`/`MATERIAL_BOUGHT_FLOOR` constants) —
// the S22a controls reuse the S21h baseline classifier so the pinned-topology control is judged
// by the SAME vector that landed S21h.
#[path = "support/mod.rs"]
mod support;
use support::*;

// =========================================================================
// Predeclared thresholds (a priori — §7; NEVER fitted to the data)
// =========================================================================

/// Distinct non-lineage cultivation entrants below which the seam is PINNED-LINEAGE NECESSITY
/// (the assigned producer identity was load-bearing). Four is non-token without requiring the
/// whole demand side to become cultivators (spec §8.2).
const MATERIAL_ENTRY_FLOOR: usize = 4;

/// Distinct non-lineage entrants that must sell `SelfProduced` bread for SALT (production-time
/// provenance) for SUCCESS (spec §7).
const MIN_NONLINEAGE_SELLERS: usize = 2;

/// Rolling cultivator share at/above which the run is on the commune side of the commune test
/// (most survivors cultivate). Spec §7.
const COMMUNE_SHARE: f64 = 0.75;

/// Enter/exit cultivation transitions per ever-cultivating non-lineage colonist over the run
/// above which (AND no settled final-window band) the run is OSCILLATION. PREDECLARED a priori,
/// NOT fitted to the no-hysteresis control (spec §7 / Codex P1).
const CHURN_LIMIT: f64 = 8.0;

/// Living non-lineage roles at/below which the demand side has collapsed before money formed
/// (the S21g cull re-emerging). The S21g/S21h survivor metric (spec §7).
const DEMAND_COLLAPSE_FLOOR: usize = 4;

/// The rolling window (ticks) for the cultivator-share / material-buyer samples.
const ROLLING_WINDOW: usize = 100;

/// The final window (ticks) over which the cultivator-share band + buyer cohort are read.
const FINAL_WINDOW: usize = 200;

/// A settled cultivator-share band's maximum range over the final window (above ⇒ not settled).
const BAND_WIDTH: f64 = 0.25;

/// Per-agent cumulative bought bread at/above which a non-cultivating non-lineage colonist counts
/// as a material BUYER (a real market transactor, not merely alive).
const MATERIAL_BUYER_FOOD: u64 = 4;

/// Living non-cultivating material buyers required in the final window for a genuine two-cohort
/// split (a non-cultivating demand side that is alive AND buying).
const MIN_BUYER_COHORT: usize = 2;

/// WOOD↔SALT volume at/above which the indirect-exchange lane is "material" (spec §7 criterion 6).
const MATERIAL_WOOD_FLOOR: u64 = 100;

// =========================================================================
// The ordered, mutually-exclusive classifier (spec §2)
// =========================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    /// (1) A guard failed (conservation / `bread_minted_max>0` / `!extinct`) OR the provenance is
    /// DISQUALIFIED (seeded `SeededMinted` bread sold for SALT, or pre-promotion minted volume).
    BrokenInvariant,
    /// (2) The living non-lineage demand side fell to ~0 before money formed (the S21g cull).
    DemandSideCollapse,
    /// (3) Distinct non-lineage entrants `< MATERIAL_ENTRY_FLOOR` — entry is negligible; only the
    /// old lineage cultivates materially (the assigned producer identity was load-bearing).
    PinnedLineageNecessity,
    /// (4) Rolling cultivator share `>= COMMUNE_SHARE` AND post-promotion bought below the floor —
    /// most survivors cultivate and the market dissolves.
    CommuneCollapse,
    /// (5) Role churn `>= CHURN_LIMIT` per ever-cultivating non-lineage colonist AND no settled
    /// cultivator-share band in the final window — output/trade never stabilize.
    Oscillation,
    /// (6) `SelfProduced` bread is produced and sold but SALT is not money at horizon (the
    /// medium-leadership conditions need the pinned structure), or a money-formed partial whose
    /// residual SUCCESS criterion (WOOD lane / buyer cohort / stable split) is unmet.
    MoneyFailureDespiteProduction,
    /// (7) None of the above AND all seven §SUCCESS criteria hold.
    Success,
}

/// The full per-run classification vector — every figure the §2 ordered classifier and the report
/// read, collected by one tick-by-tick pass ([`run_metrics`]). All accessors are read-only, so
/// collecting it perturbs the run not at all.
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
    // ---- entrants (by class, distinct over the run) ----
    distinct_lineage_entrants: usize,
    distinct_nonlineage_entrants: usize,
    // ---- production-time entrant-class SALT sale ----
    nonlineage_sellers: usize,
    lineage_sellers: usize,
    nonlineage_salt_volume: u64,
    lineage_salt_volume: u64,
    // ---- role churn ----
    churn_nonlineage_total: u32,
    ever_cultivating_nonlineage: usize,
    // ---- rolling cultivator share ----
    max_rolling_cultivator_share: f64,
    final_cultivator_share: f64,
    settled_band: bool,
    // ---- rolling non-cultivating material buyers ----
    final_buyer_cohort: usize,
    // ---- demand side ----
    living_non_lineage_final: usize,
    min_living_nonlineage_pre_promotion: usize,
    demand_side_collapsed: bool,
    // ---- buying ----
    bought_total: u64,
    post_promotion_bought: u64,
    // ---- WOOD exchange ----
    wood_for_salt: u64,
    // ---- reporting only ----
    living: usize,
    lineage_living: usize,
    starvation: u64,
}

impl Metrics {
    /// Role churn per ever-cultivating non-lineage colonist (the OSCILLATION numerator).
    fn churn_per_capita(&self) -> f64 {
        if self.ever_cultivating_nonlineage == 0 {
            0.0
        } else {
            f64::from(self.churn_nonlineage_total) / self.ever_cultivating_nonlineage as f64
        }
    }

    /// The SUCCESS conjunction (spec §2/§7): real non-lineage entry, ≥2 non-lineage sellers,
    /// provenance clean, SALT promoted, material post-promotion buying, a living non-cultivating
    /// buyer cohort, material WOOD exchange, and a stable two-cohort split.
    fn is_success(&self) -> bool {
        self.distinct_nonlineage_entrants >= MATERIAL_ENTRY_FLOOR
            && self.nonlineage_sellers >= MIN_NONLINEAGE_SELLERS
            && self.provenance_clean
            && self.promoted
            && self.post_promotion_bought >= MATERIAL_BOUGHT_FLOOR
            && self.final_buyer_cohort >= MIN_BUYER_COHORT
            && self.wood_for_salt >= MATERIAL_WOOD_FLOOR
            && self.final_cultivator_share > 0.0
            && self.final_cultivator_share < COMMUNE_SHARE
            && self.settled_band
    }

    /// The §2 ORDERED, mutually-exclusive classifier — checked top-down; the FIRST that matches is
    /// the verdict (the S21i non-gameability discipline). Every threshold is predeclared above.
    fn verdict(&self) -> Verdict {
        if !self.conserved || self.extinct || self.bread_minted_max > 0 || !self.provenance_clean {
            return Verdict::BrokenInvariant;
        }
        if self.demand_side_collapsed {
            return Verdict::DemandSideCollapse;
        }
        if self.distinct_nonlineage_entrants < MATERIAL_ENTRY_FLOOR {
            return Verdict::PinnedLineageNecessity;
        }
        if self.max_rolling_cultivator_share >= COMMUNE_SHARE
            && self.post_promotion_bought < MATERIAL_BOUGHT_FLOOR
        {
            return Verdict::CommuneCollapse;
        }
        if self.churn_per_capita() >= CHURN_LIMIT && !self.settled_band {
            return Verdict::Oscillation;
        }
        if !self.promoted {
            return Verdict::MoneyFailureDespiteProduction;
        }
        if self.is_success() {
            return Verdict::Success;
        }
        // Money formed, entry material, provenance clean, not commune / not oscillating — but a
        // residual SUCCESS criterion (WOOD lane / buyer cohort / stable split) is unmet. A
        // money-formed partial; reported as MONEY FAILURE DESPITE PRODUCTION (the money machinery
        // did not deliver a full self-organized split). The printed figures localize the gap.
        Verdict::MoneyFailureDespiteProduction
    }

    /// A uniform one-line figure rendering for the per-run `eprintln!` classification maps.
    fn line(&self) -> String {
        format!(
            "entrants(nl={} lin={}) sellers(nl={} lin={}) salt_vol(nl={} lin={}) | promoted={} \
             prov_clean={} | cult_share(final={:.2} max_roll={:.2} settled={}) churn/cap={:.1} \
             (nl_ever_cult={}) | buyers_final={} bought(tot={} post_promo={}) wood_salt={} | \
             living={} nl_alive={} (min_pre={}) lin_alive={} starv={} | conserved={} minted_max={} \
             extinct={}",
            self.distinct_nonlineage_entrants,
            self.distinct_lineage_entrants,
            self.nonlineage_sellers,
            self.lineage_sellers,
            self.nonlineage_salt_volume,
            self.lineage_salt_volume,
            self.promoted,
            self.provenance_clean,
            self.final_cultivator_share,
            self.max_rolling_cultivator_share,
            self.settled_band,
            self.churn_per_capita(),
            self.ever_cultivating_nonlineage,
            self.final_buyer_cohort,
            self.bought_total,
            self.post_promotion_bought,
            self.wood_for_salt,
            self.living,
            self.living_non_lineage_final,
            self.min_living_nonlineage_pre_promotion,
            self.lineage_living,
            self.starvation,
            self.conserved,
            self.bread_minted_max,
            self.extinct,
        )
    }
}

/// Run `(seed, cfg)` for `ticks` and collect the full S22a classification vector by one
/// tick-by-tick read of the public, runtime-only accessors. Entrants/churn are keyed by stable
/// `AgentId` (robust to slot reuse after a death); the cultivator-share and material-buyer
/// samples are taken each tick over the living roster.
fn run_metrics(seed: u64, cfg: &SettlementConfig, ticks: u64) -> Metrics {
    let mut s = Settlement::generate(seed, cfg);
    let bread = bread_good(&s);

    let mut conserved = true;
    let mut bread_minted_max = 0u64;

    let mut was_cultivating: BTreeMap<u64, bool> = BTreeMap::new();
    let mut churn_nonlineage: BTreeMap<u64, u32> = BTreeMap::new();
    let mut entrants_lineage: BTreeSet<u64> = BTreeSet::new();
    let mut entrants_nonlineage: BTreeSet<u64> = BTreeSet::new();

    let mut share_samples: Vec<f64> = Vec::with_capacity(ticks as usize);
    let mut buyer_samples: Vec<usize> = Vec::with_capacity(ticks as usize);

    let mut min_living_nl_pre = usize::MAX;
    let mut promoted = false;
    let mut bought_at_promotion: Option<u64> = None;

    for _ in 0..ticks {
        let report = s.econ_tick();
        conserved &= report.conserves();
        bread_minted_max = bread_minted_max.max(report.endowment_of(bread));

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
            }
            let prev = was_cultivating.get(&key).copied().unwrap_or(false);
            if cultivating && !prev {
                if lineage {
                    entrants_lineage.insert(key);
                } else {
                    entrants_nonlineage.insert(key);
                }
            }
            if cultivating != prev && !lineage {
                *churn_nonlineage.entry(key).or_insert(0) += 1;
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

        // Detect the promotion edge and snapshot the bought-channel cumulative there, so
        // post-promotion bought is `final - snapshot` (strictly after promotion).
        if !promoted && s.current_money_good() == Some(SALT) {
            promoted = true;
            bought_at_promotion = Some(s.acquisition_consumed_by_channel().bought);
        }
        if !promoted {
            min_living_nl_pre = min_living_nl_pre.min(living_non_lineage(&s));
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

    let consumed = s.acquisition_consumed_by_channel();
    let (_pp_produced, pp_minted) = s.pre_promotion_bread_for_salt_by_provenance();
    let provenance_clean = s.seeded_minted_bread_sold_for_salt() == 0 && pp_minted == 0;
    let entrant_sale = s.bread_for_salt_by_entrant_class();
    let bought_total = consumed.bought;
    let post_promotion_bought = match bought_at_promotion {
        Some(at) => bought_total.saturating_sub(at),
        None => 0,
    };
    let living_non_lineage_final = living_non_lineage(&s);
    let min_living_nonlineage_pre_promotion = if min_living_nl_pre == usize::MAX {
        living_non_lineage_final
    } else {
        min_living_nl_pre
    };

    Metrics {
        seed,
        conserved,
        bread_minted_max,
        extinct: living(&s) == 0,
        provenance_clean,
        promoted: s.current_money_good() == Some(SALT),
        distinct_lineage_entrants: entrants_lineage.len(),
        distinct_nonlineage_entrants: entrants_nonlineage.len(),
        nonlineage_sellers: entrant_sale.nonlineage_sellers,
        lineage_sellers: entrant_sale.lineage_sellers,
        nonlineage_salt_volume: entrant_sale.nonlineage_volume,
        lineage_salt_volume: entrant_sale.lineage_volume,
        churn_nonlineage_total: churn_nonlineage.values().copied().sum(),
        ever_cultivating_nonlineage: entrants_nonlineage.len(),
        max_rolling_cultivator_share,
        final_cultivator_share,
        settled_band,
        final_buyer_cohort,
        living_non_lineage_final,
        min_living_nonlineage_pre_promotion,
        // The S21g cull: the demand side fell to ~0 before money formed.
        demand_side_collapsed: min_living_nonlineage_pre_promotion <= DEMAND_COLLAPSE_FLOOR,
        bought_total,
        post_promotion_bought,
        wood_for_salt: s.wood_for_salt_volume(),
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

/// The trailing rolling means of `xs` over a window of `w` (one per end-position once the window
/// is full). Empty if `xs` is shorter than `w`.
fn rolling_means(xs: &[f64], w: usize) -> Vec<f64> {
    if w == 0 || xs.len() < w {
        return Vec::new();
    }
    (w..=xs.len()).map(|end| mean(&xs[end - w..end])).collect()
}

/// Run a batch of labelled `(seed, cfg)` jobs concurrently (bounded scoped-thread fan-out) and
/// return the collected `Metrics` in input order — keeps the 1600-tick sweeps' wall-clock sane.
/// The engine has no global mutable state, so concurrent `Settlement` runs are deterministic per
/// `(seed, config)`.
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
            m.verdict(),
            Verdict::BrokenInvariant,
            "[{ctx}] a provenance-dirty run must classify BrokenInvariant/disqualified (seed {})",
            m.seed
        );
    }
}

// =========================================================================
// 1. The determinism / golden contract
// =========================================================================

#[test]
fn canonical_bytes_split_for_endogenous_cultivation() {
    // The endogenous-entry gate relaxes who is eligible to cultivate (a future-behaviour change),
    // so `frontier_endogenous_cultivation` must SPLIT the canonical digest vs the S21h base...
    let base = Settlement::generate(7, &SettlementConfig::frontier_emergency_provision());
    let endo = Settlement::generate(7, &SettlementConfig::frontier_endogenous_cultivation());
    assert_ne!(
        base.canonical_bytes(),
        endo.canonical_bytes(),
        "the endogenous_cultivation_entry gate must split the canonical digest vs the S21h base"
    );

    // ...and reverting the flag to false must make it BYTE-IDENTICAL to
    // `frontier_emergency_provision` (the gate is the ONLY change, canonicalized ON-only).
    let mut reverted = SettlementConfig::frontier_endogenous_cultivation();
    reverted
        .chain
        .as_mut()
        .expect("chain")
        .endogenous_cultivation_entry = false;
    let mut a = Settlement::generate(7, &reverted);
    let mut b = Settlement::generate(7, &SettlementConfig::frontier_emergency_provision());
    a.run(1_000);
    b.run(1_000);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "reverting endogenous_cultivation_entry must equal frontier_emergency_provision byte-for-byte"
    );
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn endogenous_entry_off_the_path_is_inert() {
    // The gate composes on the money-from-produced-bread path, so toggling it on a forage-only
    // config (no Cultivate recipe / no cultivation_sells_surplus) must NOT split the digest —
    // preserving the byte layout for every config off the S22a path.
    let base = Settlement::generate(7, &SettlementConfig::frontier_forage_capacity());
    let mut base_cfg = SettlementConfig::frontier_forage_capacity();
    base_cfg
        .chain
        .as_mut()
        .expect("chain")
        .endogenous_cultivation_entry = true;
    assert_eq!(
        base.canonical_bytes(),
        Settlement::generate(7, &base_cfg).canonical_bytes(),
        "off the money-from-produced-bread path the endogenous_cultivation_entry flag must not steer the digest"
    );
}

#[test]
fn endogenous_entry_runs_are_deterministic() {
    // Byte-identical `(seed, config)` at a long horizon (the deaths + entry live in the colonist
    // state `canonical_bytes` pins; every S22a diagnostic is runtime-only).
    let cfg = SettlementConfig::frontier_endogenous_cultivation();
    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    a.run(2_000);
    b.run(2_000);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the endogenous-entry run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.digest(), b.digest());
    let mut c = Settlement::generate(2, &cfg);
    c.run(2_000);
    assert_ne!(a.digest(), c.digest(), "the seed must change the run");
}

#[test]
fn goldens_unchanged() {
    // The S22a addition is one additive, default-off ON-only gate + runtime-only diagnostics, so
    // the cross-history demographic + emergence goldens are BYTE-IDENTICAL (the same five values
    // pinned in demand_survival_bridge.rs / robustness_appendix.rs).
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
// 2. The headline verdict (the ordered classifier; prints, does NOT assert SUCCESS)
// =========================================================================

#[test]
fn endogenous_entry_verdict() {
    eprintln!("================ S22a ENDOGENOUS CULTIVATION ENTRY VERDICT ================");
    let cfgs: Vec<(u64, SettlementConfig)> = SEEDS
        .iter()
        .map(|&seed| (seed, SettlementConfig::frontier_endogenous_cultivation()))
        .collect();
    let cells = run_batch(cfgs);
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    for m in &cells {
        assert_guards(m, "verdict");
        let v = m.verdict();
        *tally.entry(format!("{v:?}")).or_insert(0) += 1;
        eprintln!("  seed={:>2}: {v:?} | {}", m.seed, m.line());
        // The verdict must be a well-formed §2 outcome — but SUCCESS is NOT asserted (the data
        // decides; a pinned-lineage / commune / money-failure finding is a first-class result).
        assert!(
            matches!(
                v,
                Verdict::BrokenInvariant
                    | Verdict::DemandSideCollapse
                    | Verdict::PinnedLineageNecessity
                    | Verdict::CommuneCollapse
                    | Verdict::Oscillation
                    | Verdict::MoneyFailureDespiteProduction
                    | Verdict::Success
            ),
            "the verdict must be one of the seven §2 outcomes"
        );
        // BrokenInvariant is never an acceptable regime — it is a hard failure (the guards above
        // already enforce conservation/minting/extinction; this catches a provenance disqualify).
        assert_ne!(
            v,
            Verdict::BrokenInvariant,
            "no run may be a broken invariant (seed {})",
            m.seed
        );
    }
    eprintln!("---- verdict tally across SEEDS={SEEDS:?}: {tally:?} ----");
    eprintln!("==========================================================================");
}

// =========================================================================
// 3. The six controls (spec §5) — classify, never tune
// =========================================================================

#[test]
fn control_pinned_topology_still_succeeds() {
    // Control 1: the S21h pinned-lineage baseline (gate OFF) must still be the S21h demand-bridge
    // SUCCESS across SEEDS, classified by the SAME shared `Cell`/`Regime` vector that landed S21h.
    for &seed in &SEEDS {
        let cell = classify(
            seed,
            &SettlementConfig::frontier_emergency_provision(),
            PROBE_TICKS,
        );
        assert!(cell.conserved, "pinned control must conserve (seed {seed})");
        assert!(
            !cell.extinct,
            "pinned control must not go extinct (seed {seed})"
        );
        assert_eq!(
            cell.bread_minted_max, 0,
            "pinned control mints no bread (seed {seed})"
        );
        eprintln!("  [pinned-topology] seed={seed}: {}", cell.map_line());
        assert_eq!(
            cell.regime(),
            Regime::Success,
            "the S21h pinned-lineage baseline must still succeed (seed {seed})"
        );
    }
}

#[test]
fn control_no_hysteresis_churns() {
    // Control 3: collapse the pressure hysteresis via the EXISTING fields (no new flag) —
    // cultivate_patience = 1 (no streak) AND the narrowest in/out band. NOTE: the engine
    // VALIDATES `cultivate_hunger_out < cultivate_hunger_in` at generation, so the spec's literal
    // `out = in` is rejected; the realizable no-hysteresis pin is patience = 1 (which removes the
    // streak — the dominant hysteresis) plus the MINIMUM one-notch band (`out = in - 1`). This is
    // disclosed, not a tuning of the classifier. Expected to cross CHURN_LIMIT; if it does NOT,
    // that is REPORTED as a control finding (the constant is NOT retuned — Codex P1).
    let mut crossed_any = false;
    for &seed in &SEEDS {
        let mut cfg = SettlementConfig::frontier_endogenous_cultivation();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.cultivate_patience = 1;
            chain.cultivate_hunger_out = chain.cultivate_hunger_in.saturating_sub(1);
        }
        let m = run_metrics(seed, &cfg, PROBE_TICKS);
        assert_guards(&m, "no-hysteresis");
        let crossed = m.churn_per_capita() >= CHURN_LIMIT;
        crossed_any |= crossed;
        eprintln!(
            "  [no-hysteresis] seed={seed}: churn/cap={:.1} (>= {CHURN_LIMIT}? {crossed}) verdict={:?} | {}",
            m.churn_per_capita(),
            m.verdict(),
            m.line()
        );
    }
    // A report, not a tuning gate. CONTROL FINDING (Codex review-of-results P2): crossing
    // CHURN_LIMIT does NOT by itself prove the hysteresis is load-bearing — the HEADLINE run also
    // has churn/cap far above CHURN_LIMIT (~23-24) yet classifies Success because the AGGREGATE
    // cultivator share settles. So removing most hysteresis does NOT create a distinct failure
    // regime: aggregate stability is robust to it, while per-agent churn stays high in both. The
    // honest reading is "fluid/rotating participation either way," not "hysteresis is load-bearing."
    if crossed_any {
        eprintln!(
            "  CONTROL FINDING: no-hysteresis crosses CHURN_LIMIT={CHURN_LIMIT}, but so does the \
             headline — removing most hysteresis creates NO distinct failure regime (aggregate \
             share still settles; per-agent churn stays high in both). The hysteresis is NOT \
             load-bearing for aggregate stability here."
        );
    } else {
        eprintln!(
            "  CONTROL FINDING: no-hysteresis did NOT cross CHURN_LIMIT={CHURN_LIMIT} on any seed \
             (reported, NOT retuned — the predeclared constant stands)."
        );
    }
}

#[test]
fn control_no_emergency_floor_culls() {
    // Control 4: endogenous entry with the S21h emergency floor OFF. The S21g/S21h expectation
    // was an S21g-like demand-side cull (the floor was the only survival bridge). UNDER
    // ENDOGENOUS ENTRY this is a CONTROL FINDING, not a fixed expectation: the relaxed
    // eligibility makes cultivation ITSELF a survival path (a hungry non-lineage role can now
    // feed itself by cultivating), so the floor may no longer be load-bearing. The test asserts
    // only the guards and REPORTS whether the cull reproduced — it does NOT force a cull (forcing
    // the pre-named expectation against the data would be the tuning trap, spec §10).
    let mut culled = 0usize;
    let mut succeeded = 0usize;
    for &seed in &SEEDS {
        let mut cfg = SettlementConfig::frontier_endogenous_cultivation();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.emergency_hunger_threshold = 0;
        }
        let m = run_metrics(seed, &cfg, PROBE_TICKS);
        assert_guards(&m, "no-emergency-floor");
        if m.demand_side_collapsed {
            culled += 1;
        }
        if m.is_success() {
            succeeded += 1;
        }
        eprintln!(
            "  [no-emergency-floor] seed={seed}: verdict={:?} | {}",
            m.verdict(),
            m.line()
        );
    }
    if culled == SEEDS.len() {
        eprintln!(
            "  no-emergency-floor reproduced the S21g demand-side cull on all seeds: the survival \
             bridge is still load-bearing."
        );
    } else {
        eprintln!(
            "  CONTROL FINDING: with endogenous entry ON, removing the emergency floor did NOT cull \
             on all seeds ({culled}/{} culled, {succeeded}/{} still SUCCESS). Endogenous \
             cultivation is itself a survival path — the floor is no longer the sole demand-side \
             bridge (reported, not forced).",
            SEEDS.len(),
            SEEDS.len()
        );
    }
}

#[test]
fn control_money_machinery_off_degrades() {
    // Control 5: endogenous entry with the two-layer saleability money machinery OFF — money
    // should fail/degrade (entry alone does not make money). NOT a SUCCESS.
    for &seed in &SEEDS {
        let mut cfg = SettlementConfig::frontier_endogenous_cultivation();
        if let Some(barter) = cfg.barter.as_mut() {
            barter.menger.two_layer_saleability = false;
        }
        let m = run_metrics(seed, &cfg, PROBE_TICKS);
        assert_guards(&m, "money-machinery-off");
        eprintln!(
            "  [money-machinery-off] seed={seed}: verdict={:?} | {}",
            m.verdict(),
            m.line()
        );
        assert!(
            !m.is_success(),
            "with the two-layer saleability machinery off the colony must NOT succeed (seed {seed})"
        );
    }
}

#[test]
fn control_low_grain_flow_does_not_fake_success() {
    // Control 6: endogenous entry with the grain node starved — cultivation entry without real
    // food input must NOT fake success (the produced-for-SALT provenance + the buying collapse).
    for &seed in &SEEDS {
        let mut cfg = SettlementConfig::frontier_endogenous_cultivation();
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
        eprintln!(
            "  [low-grain-flow] seed={seed}: verdict={:?} | {}",
            m.verdict(),
            m.line()
        );
        assert!(
            !m.is_success(),
            "with the grain commons starved the colony must NOT fake success (seed {seed})"
        );
    }
}

// =========================================================================
// 4. The mortality-off sanity variant (diagnostic only — spec §8.3)
// =========================================================================

#[test]
fn endogenous_entry_no_mortality_sanity() {
    // The clean control for "does relaxing the producer identity admit non-lineage cultivators at
    // all?" — mortality OFF, so the entry seam fires without the positive-check cold-start cull
    // confounding the read. Prints the entrant counts; classifies via the same vector.
    for &seed in &SEEDS {
        let m = run_metrics(
            seed,
            &SettlementConfig::frontier_endogenous_cultivation_no_mortality(),
            PROBE_TICKS,
        );
        assert_guards(&m, "no-mortality-sanity");
        eprintln!(
            "  [no-mortality] seed={seed}: verdict={:?} | {}",
            m.verdict(),
            m.line()
        );
    }
}

// =========================================================================
// 5. The robustness mini-sweep over the EXISTING pressure thresholds + grain flow
// =========================================================================

#[test]
fn robustness_sweep_over_existing_thresholds() {
    // Sweep the EXISTING S15/S21f pressure thresholds + grain flow (1-D axes, holding the others
    // at the shipped value), classify each cell across two seeds, and PRINT the regime map. No
    // tuning to pass: commune / pinned / failure outcomes are first-class findings. Bounded to two
    // seeds per cell to keep the 1600-tick sweep's wall-clock sane (no silent cap — stated here).
    const SWEEP_SEEDS: [u64; 2] = [3, 7];
    eprintln!("================ S22a ROBUSTNESS MINI-SWEEP (regime map) ================");
    eprintln!("  bounded to SWEEP_SEEDS={SWEEP_SEEDS:?} per cell (stated, not silent).");

    struct Axis {
        name: &'static str,
        cells: Vec<(String, SettlementConfig)>,
    }

    let mut axes: Vec<Axis> = Vec::new();

    // cultivate_hunger_in (shipped 6). The engine enforces the invariants
    // `cultivate_hunger_out < cultivate_hunger_in < birth_hunger_ceiling (8)`, so the feasible
    // band is {4,5,6,7} (the larger values the spec might imagine are structurally infeasible —
    // stated, not silently dropped).
    {
        let mut cells = Vec::new();
        for v in [4u16, 5, 6, 7] {
            let mut cfg = SettlementConfig::frontier_endogenous_cultivation();
            if let Some(chain) = cfg.chain.as_mut() {
                chain.cultivate_hunger_in = v;
                // keep the in/out band valid (out < in).
                chain.cultivate_hunger_out = chain.cultivate_hunger_out.min(v.saturating_sub(1));
            }
            cells.push((format!("cultivate_hunger_in={v}"), cfg));
        }
        axes.push(Axis {
            name: "cultivate_hunger_in",
            cells,
        });
    }
    // cultivate_patience (shipped 2).
    {
        let mut cells = Vec::new();
        for v in [1u16, 2, 4, 8] {
            let mut cfg = SettlementConfig::frontier_endogenous_cultivation();
            if let Some(chain) = cfg.chain.as_mut() {
                chain.cultivate_patience = v;
            }
            cells.push((format!("cultivate_patience={v}"), cfg));
        }
        axes.push(Axis {
            name: "cultivate_patience",
            cells,
        });
    }
    // grain node regen (shipped 24).
    {
        let mut cells = Vec::new();
        for v in [12u32, 24, 48, 96] {
            let mut cfg = SettlementConfig::frontier_endogenous_cultivation();
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
                eprintln!("  {label} seed={seed}: {:?} | {}", m.verdict(), m.line());
            }
        }
    }
    eprintln!("========================================================================");
}
