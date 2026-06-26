//! S21i acceptance suite — the **robustness appendix** (impl-32): do the four S21f/g/h capstone
//! regimes survive the parameter space, or is the S21h.1 emergency-floor SUCCESS a narrow band?
//!
//! This suite is **test-additive** (no engine code, no new config field): it only calls existing
//! scenario constructors with swept values of existing config fields and reads existing
//! runtime-only metrics through the shared classifier ([`support`]), so every existing golden is
//! byte-identical by construction (the `goldens_unchanged` tripwire below re-pins the five).
//!
//! Structure (spec §3.2):
//!   (A) seed-robustness spine — the three headline regimes across `WIDE_SEEDS`;
//!   (B) CORE-axis 1-D window maps — each band cell classified across `CROSS_SEEDS`, per-axis
//!       ROBUST / NARROW / BOUNDED-BY-AXIS criterion (the capstone verdict feeds on these);
//!   (B-sensitivity) SENSITIVITY axes — classified + printed but EXCLUDED from the core verdict;
//!   (B′) two interaction maps — classification-only, asserting only the guards + shipped SUCCESS;
//!   (C) `capstone_robustness_verdict` — aggregates the CORE criteria + headline stability into
//!       ROBUST / NARROW-BAND / MIXED and PRINTS the verdict (it does NOT assert ROBUST).
//!
//! Every cell, regardless of regime or seed, satisfies the broken-invariant guards (conserved,
//! `bread_minted_max == 0`, provenance-clean-or-disqualified) — these are hard asserts. Every
//! bound the suite imposes (single vs cross seed, band endpoints, skipped/infeasible cells) is
//! `eprintln!`-logged so the coverage is auditable (no silent caps).

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use sim::{Settlement, SettlementConfig};

#[path = "support/mod.rs"]
mod support;
use support::*;

// =========================================================================
// Seed sets (spec §5)
// =========================================================================

/// The extended (12-seed) robustness set for part (A), the headline-regime spine.
const WIDE_SEEDS: [u64; 12] = [3, 7, 11, 19, 23, 29, 31, 37, 41, 43, 47, 53];

/// Every 1-D band cell in part (B) is classified across all three (one headline seed + two
/// independent); a cell counts SUCCESS only if SUCCESS for all three.
const CROSS_SEEDS: [u64; 3] = [3, 7, 19];

// =========================================================================
// Parallel, memoized cell evaluation
// =========================================================================
//
// A 1600-tick classify is ~3s, and the suite runs a few hundred cells, so two things keep the
// wall-clock sane: (1) a process-wide memo keyed by `(cache key, seed)` so each (key, seed) is
// computed at most once across all tests in this binary (the verdict reuses what the per-axis
// maps computed); (2) scoped-thread fan-out within each batch. The key is a per-cell LABEL, not
// the config bytes, so a byte-identical config carrying distinct labels (e.g. the shipped
// `frontier_emergency_provision` appearing as several axes' `(shipped)` cells) recomputes once
// per label — a deliberate trade (labels stay readable; correctness is unaffected since the run
// is deterministic per (seed, config)). The engine has no global mutable state, so the concurrent
// `Settlement` runs are deterministic per (seed, config).

/// A unit of classification work: a stable cache key, the seed, and the config to run.
struct Job {
    key: String,
    seed: u64,
    cfg: SettlementConfig,
}

fn cell_cache() -> &'static Mutex<BTreeMap<(String, u64), Cell>> {
    static CACHE: OnceLock<Mutex<BTreeMap<(String, u64), Cell>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Classify a batch of jobs concurrently (bounded worker fan-out), consulting + filling the
/// process-wide cache so no (key, seed) is computed twice. Results are returned in input order.
fn classify_jobs(jobs: &[Job]) -> Vec<Cell> {
    let mut out: Vec<Option<Cell>> = vec![None; jobs.len()];
    let mut misses: Vec<usize> = Vec::new();
    {
        let cache = cell_cache().lock().expect("cell cache mutex");
        for (i, job) in jobs.iter().enumerate() {
            match cache.get(&(job.key.clone(), job.seed)) {
                Some(c) => out[i] = Some(*c),
                None => misses.push(i),
            }
        }
    }
    if !misses.is_empty() {
        let workers = 16usize.min(misses.len());
        let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); workers];
        for (k, &mi) in misses.iter().enumerate() {
            buckets[k % workers].push(mi);
        }
        let computed: Vec<(usize, Cell)> = std::thread::scope(|scope| {
            let handles: Vec<_> = buckets
                .into_iter()
                .map(|bucket| {
                    scope.spawn(move || {
                        bucket
                            .into_iter()
                            .map(|i| (i, classify(jobs[i].seed, &jobs[i].cfg, PROBE_TICKS)))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().expect("classify worker thread"))
                .collect()
        });
        let mut cache = cell_cache().lock().expect("cell cache mutex");
        for (i, cell) in computed {
            cache.insert((jobs[i].key.clone(), jobs[i].seed), cell);
            out[i] = Some(cell);
        }
    }
    out.into_iter()
        .map(|c| c.expect("every job classified"))
        .collect()
}

// =========================================================================
// Config builders for the swept axes (existing fields only)
// =========================================================================

/// CORE axis 2 — grain-flow: scale the grain node's `regen` around the shipped 24 on the S21h.1
/// scenario. `with_grain_regen(24)` is byte-identical to `frontier_emergency_provision`.
fn with_grain_regen(regen: u32) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_emergency_provision();
    let grain = cfg
        .chain
        .as_ref()
        .expect("the emergency chain carries a grain good")
        .content
        .grain();
    for node in cfg.nodes.iter_mut() {
        if node.good == grain {
            node.regen = regen;
        }
    }
    cfg
}

/// CORE axis 3 — WOOD-poor magnitude (the lane-driving WOOD scarcity). The bread→SALT
/// `IndirectFor{WOOD}` lane forms because the *lineage cultivators* (the bread sellers) have an
/// unsatisfied WOOD want; their WOOD comes from the demography `HouseholdSpec` (`wood_provision`
/// per tick + `starting_wood`), both held at 0 in the shipped scenario (maximally WOOD-poor).
///
/// The spec named `chain.wood_buffer` here, but that field only seeds the NON-lineage chain roles
/// (the woodcutters), and sweeping it is OUTCOME-INERT in this topology: the generation state
/// differs (the woodcutters' starting WOOD), yet the run washes it out completely — every Cell
/// figure is byte-identical across `{4,8,12,24,48}` for every seed, so it can never be anything
/// but a vacuous "ROBUST" (Reviewer-1 P2). The actual lane-driving scarcity is the lineage's own
/// recurring WOOD provision, so the axis sweeps THAT: `wood_provision = 0` (shipped) is the hard
/// WOOD floor (one cannot be poorer than receiving zero WOOD); raising it relaxes the scarcity.
/// `with_lineage_wood_provision(0)` is byte-identical to `frontier_emergency_provision`.
///
/// `wood_provision` mints WOOD as tracked `report.endowment` (conserved) and is NOT tracked food,
/// so it never enters the bread `SeededMinted` channel — the bread provenance stays clean.
fn with_lineage_wood_provision(provision: u32) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_emergency_provision();
    if let Some(demo) = cfg.demography.as_mut() {
        for household in demo.households.iter_mut() {
            household.wood_provision = provision;
        }
    }
    cfg
}

/// CORE axis 4 — SALT anchor density: the 1-in-N direct-use period around the shipped 8.
fn with_salt_period(period: u16) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_emergency_provision();
    if let Some(barter) = cfg.barter.as_mut() {
        barter.salt_direct_use_period = period;
    }
    cfg
}

/// SENSITIVITY axis 5 — strong-bar direct-use acceptor floor around the shipped 2.
fn with_acceptors(acceptors: u16) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_emergency_provision();
    if let Some(barter) = cfg.barter.as_mut() {
        barter.menger.min_direct_use_acceptors = acceptors;
    }
    cfg
}

/// SENSITIVITY axis 6 — role topology: gatherers / consumers / total lineage founders (spread
/// evenly over the households). `with_roles(8, 4, 4)` is byte-identical to the shipped scenario.
fn with_roles(gatherers: u16, consumers: u16, founders_total: u16) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_emergency_provision();
    cfg.gatherers = gatherers;
    cfg.consumers = consumers;
    if let Some(demo) = cfg.demography.as_mut() {
        // Spread `founders_total` evenly over the households (always ≥ 1 here), remainder to the
        // lowest indices. `with_roles(8, 4, 4)` → 2 households × 2 founders = the shipped roster.
        let n = (demo.households.len() as u16).max(1);
        for (i, household) in demo.households.iter_mut().enumerate() {
            let base = founders_total / n;
            let extra = u16::from((i as u16) < (founders_total % n));
            household.founders = base + extra;
        }
    }
    cfg
}

/// SENSITIVITY axis 7a — mortality preventive arm: the birth-hunger ceiling around the shipped 8.
fn with_birth_ceiling(ceiling: u16) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_emergency_provision();
    if let Some(demo) = cfg.demography.as_mut() {
        demo.birth_hunger_ceiling = ceiling;
    }
    cfg
}

/// SENSITIVITY axis 7b — mortality positive arm: the death window around the shipped 3
/// (`hunger_critical` held at `need_max` — the mortality on/off switch is not swept).
fn with_death_window(window: u16) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_emergency_provision();
    cfg.dynamics.death_window = window;
    cfg
}

/// Interaction map (B′-1) — emergency threshold × grain flow.
fn with_threshold_and_grain(threshold: u16, regen: u32) -> SettlementConfig {
    let mut cfg = with_grain_regen(regen);
    if let Some(chain) = cfg.chain.as_mut() {
        chain.emergency_hunger_threshold = threshold;
    }
    cfg
}

/// Interaction map (B′-2) — WOOD-poor magnitude (lineage `wood_provision`) × SALT anchor density.
fn with_wood_and_salt(provision: u32, period: u16) -> SettlementConfig {
    let mut cfg = with_lineage_wood_provision(provision);
    if let Some(barter) = cfg.barter.as_mut() {
        barter.salt_direct_use_period = period;
    }
    cfg
}

// =========================================================================
// Hard guards (every cell, every regime, every seed)
// =========================================================================

/// The broken-invariant guards every swept cell must satisfy regardless of its regime (spec §3):
/// conservation held every tick, no bread minted, and a non-provenance-clean cell is NEVER
/// counted as anything but DISQUALIFIED (so a seeded-supply promotion can never masquerade as a
/// success). These are asserts, not classifications.
fn assert_guards(cell: &Cell, label: &str) {
    assert!(cell.conserved, "{label}: conservation must hold every tick");
    assert_eq!(
        cell.bread_minted_max, 0,
        "{label}: no bread may be minted (the food mints are retired)"
    );
    // Extinction is a BROKEN INVARIANT, never a regime: a CULL (the S21g outcome) wipes only the
    // non-lineage demand side while the self-feeding lineage survives, so `living > 0`. If a swept
    // cell ever kills the WHOLE colony, `regime()` would silently label it `Cull` (because
    // `survived` reads the non-lineage roster) and the maps/verdict would accept it — so it is a
    // hard guard here, matching the per-cell `!extinct` bar the sibling S21h suite asserts.
    assert!(
        !cell.extinct,
        "{label}: the colony must not go extinct (a broken invariant, never a classified regime)"
    );
    assert!(
        cell.provenance_clean || cell.regime() == Regime::Disqualified,
        "{label}: a non-provenance-clean cell must be classified DISQUALIFIED, never clean"
    );
    assert!(
        !cell.is_success() || cell.provenance_clean,
        "{label}: a SUCCESS cell must be provenance-clean"
    );
}

// =========================================================================
// 1-D axis sweep + per-axis criterion (spec §2)
// =========================================================================

/// One band cell of a 1-D sweep: the displayed value label, the per-`CROSS_SEEDS` cells (in
/// order), and whether it is SUCCESS for EVERY cross seed (the cell-level SUCCESS the two-step
/// criterion reads).
struct AxisCell {
    label: String,
    cells: Vec<Cell>,
    success_all_seeds: bool,
}

/// The two-step robustness criterion (spec §2/§4 Q4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Criterion {
    /// Two SUCCESS steps each non-bounded side (where the band has the values).
    Robust,
    /// SUCCESS at shipped but a non-SUCCESS within two steps on at least one non-bounded side.
    Narrow,
    /// The shipped value sits at a hard physical/semantic bound of the axis, so "interior" is
    /// undefined on that side; the other side is robust. Reported as bounded, NOT robust.
    BoundedByAxis,
}

/// The full evaluation of one CORE axis: its band cells, the shipped index, per-side margins,
/// the hard-bound flags, and the computed criterion.
struct AxisEval {
    name: String,
    cells: Vec<AxisCell>,
    shipped_index: usize,
    low_bounded: bool,
    high_bounded: bool,
    low_margin: usize,
    high_margin: usize,
    criterion: Criterion,
}

/// Sweep a band (label, config) across `CROSS_SEEDS`: classify every (cell, seed), assert the
/// hard guards on each, `eprintln!` one regime line per (cell, seed), and return the per-cell
/// SUCCESS-across-all-seeds vector. Used by both the CORE axes and the SENSITIVITY axes.
fn sweep_band(name: &str, band: &[(String, SettlementConfig)]) -> Vec<AxisCell> {
    let mut jobs = Vec::with_capacity(band.len() * CROSS_SEEDS.len());
    for (label, cfg) in band {
        for &seed in &CROSS_SEEDS {
            jobs.push(Job {
                key: format!("{name}:{label}"),
                seed,
                cfg: cfg.clone(),
            });
        }
    }
    let results = classify_jobs(&jobs);
    let mut axis_cells = Vec::with_capacity(band.len());
    for (ci, (label, _cfg)) in band.iter().enumerate() {
        let mut cells = Vec::with_capacity(CROSS_SEEDS.len());
        let mut success_all_seeds = true;
        for (si, &seed) in CROSS_SEEDS.iter().enumerate() {
            let cell = results[ci * CROSS_SEEDS.len() + si];
            assert_guards(&cell, &format!("[{name}] {label} seed={seed}"));
            eprintln!("  [{name}] {label} seed={seed}: {}", cell.map_line());
            success_all_seeds &= cell.is_success();
            cells.push(cell);
        }
        axis_cells.push(AxisCell {
            label: label.clone(),
            cells,
            success_all_seeds,
        });
    }
    axis_cells
}

/// Evaluate a CORE axis: sweep the band, then compute the two-step ROBUST / NARROW /
/// BOUNDED-BY-AXIS criterion from the per-cell SUCCESS vector around the shipped index.
fn evaluate_core_axis(
    name: &str,
    band: &[(String, SettlementConfig)],
    shipped_index: usize,
    low_bounded: bool,
    high_bounded: bool,
) -> AxisEval {
    let cells = sweep_band(name, band);
    let success: Vec<bool> = cells.iter().map(|c| c.success_all_seeds).collect();
    let i = shipped_index;

    // Consecutive SUCCESS steps walking outward from the shipped index.
    let mut low_margin = 0usize;
    let mut j = i;
    while j > 0 && success[j - 1] {
        low_margin += 1;
        j -= 1;
    }
    let mut high_margin = 0usize;
    let mut j = i;
    while j + 1 < success.len() && success[j + 1] {
        high_margin += 1;
        j += 1;
    }

    // A non-bounded side needs two SUCCESS steps; a bounded side is waived.
    let low_ok = low_bounded || low_margin >= 2;
    let high_ok = high_bounded || high_margin >= 2;
    let criterion = if !success[i] {
        // The shipped value itself is not SUCCESS across all cross seeds — a headline failure,
        // not a band-width question.
        Criterion::Narrow
    } else if low_ok && high_ok {
        if low_bounded || high_bounded {
            Criterion::BoundedByAxis
        } else {
            Criterion::Robust
        }
    } else {
        Criterion::Narrow
    };

    AxisEval {
        name: name.to_string(),
        cells,
        shipped_index,
        low_bounded,
        high_bounded,
        low_margin,
        high_margin,
        criterion,
    }
}

impl AxisEval {
    /// The exact band values + shipped index + per-side margin + criterion, printed so the
    /// criterion is checkable, not asserted by fiat (spec §2).
    fn report(&self) {
        let band: Vec<&str> = self.cells.iter().map(|c| c.label.as_str()).collect();
        let low = if self.low_bounded {
            "BOUNDED".to_string()
        } else {
            format!("{} step(s)", self.low_margin)
        };
        let high = if self.high_bounded {
            "BOUNDED".to_string()
        } else {
            format!("{} step(s)", self.high_margin)
        };
        eprintln!(
            "AXIS {} :: band={:?} shipped_index={} (={}) | low margin: {} | high margin: {} | \
             criterion: {:?}",
            self.name,
            band,
            self.shipped_index,
            band[self.shipped_index],
            low,
            high,
            self.criterion,
        );
    }
}

// =========================================================================
// The CORE-axis band definitions (single source of truth for B + C)
// =========================================================================

fn core_axis_emergency_threshold() -> AxisEval {
    // Validator: cultivate_hunger_in (6) < threshold < hunger_critical (12), so only {7..=11} are
    // valid classifiable cells (4, 6, 12 are NOT regime cells). Shipped 11 sits at the TOP valid
    // bound → high side is BOUNDED-BY-AXIS; the two-step criterion applies only on the low side.
    let band = vec![
        ("thr=7".to_string(), with_emergency(7)),
        ("thr=8".to_string(), with_emergency(8)),
        ("thr=9".to_string(), with_emergency(9)),
        ("thr=10".to_string(), with_emergency(10)),
        ("thr=11(shipped)".to_string(), with_emergency(11)),
    ];
    evaluate_core_axis("emergency_hunger_threshold", &band, 4, false, true)
}

fn core_axis_grain_flow() -> AxisEval {
    let band = vec![
        ("regen=12".to_string(), with_grain_regen(12)),
        ("regen=18".to_string(), with_grain_regen(18)),
        ("regen=24(shipped)".to_string(), with_grain_regen(24)),
        ("regen=36".to_string(), with_grain_regen(36)),
        ("regen=48".to_string(), with_grain_regen(48)),
    ];
    evaluate_core_axis("grain_regen", &band, 2, false, false)
}

fn core_axis_wood_scarcity() -> AxisEval {
    // The lane-driving WOOD scarcity is the lineage's recurring `wood_provision`, shipped 0 — the
    // hard WOOD floor (one cannot be poorer than receiving zero WOOD per tick), so the LOW side is
    // BOUNDED-BY-AXIS and the two-step criterion applies only on the high side. Raising the
    // provision relaxes the lineage's WOOD want, which is exactly what gates the bread→SALT
    // `IndirectFor{WOOD}` lane.
    let band = vec![
        (
            "wood_provision=0(shipped)".to_string(),
            with_lineage_wood_provision(0),
        ),
        (
            "wood_provision=1".to_string(),
            with_lineage_wood_provision(1),
        ),
        (
            "wood_provision=2".to_string(),
            with_lineage_wood_provision(2),
        ),
        (
            "wood_provision=3".to_string(),
            with_lineage_wood_provision(3),
        ),
        (
            "wood_provision=4".to_string(),
            with_lineage_wood_provision(4),
        ),
    ];
    evaluate_core_axis("lineage_wood_provision", &band, 0, true, false)
}

fn core_axis_salt_period() -> AxisEval {
    // Smaller period = denser anchor; the period must stay > 0 (period 1 would be a universal
    // direct want, the Base-Fact-6 trap), so the band is {4,6,8,12,16}.
    let band = vec![
        ("period=4".to_string(), with_salt_period(4)),
        ("period=6".to_string(), with_salt_period(6)),
        ("period=8(shipped)".to_string(), with_salt_period(8)),
        ("period=12".to_string(), with_salt_period(12)),
        ("period=16".to_string(), with_salt_period(16)),
    ];
    evaluate_core_axis("salt_direct_use_period", &band, 2, false, false)
}

fn all_core_axes() -> Vec<AxisEval> {
    vec![
        core_axis_emergency_threshold(),
        core_axis_grain_flow(),
        core_axis_wood_scarcity(),
        core_axis_salt_period(),
    ]
}

// =========================================================================
// Spine helpers (part A)
// =========================================================================

fn spine_cells(key: &str, cfg: &SettlementConfig, seeds: &[u64]) -> Vec<Cell> {
    let jobs: Vec<Job> = seeds
        .iter()
        .map(|&seed| Job {
            key: key.to_string(),
            seed,
            cfg: cfg.clone(),
        })
        .collect();
    classify_jobs(&jobs)
}

// =========================================================================
// 0. The determinism / golden contract (spec §6 — re-pinned in this suite)
// =========================================================================

#[test]
fn goldens_unchanged() {
    // This suite adds no config field and no engine code, so the five pinned cross-history goldens
    // are BYTE-IDENTICAL (the same values pinned in tests/mortality.rs, tests/household_barter.rs,
    // tests/open_colony_mortality.rs and tests/demand_survival_bridge.rs).
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

#[test]
fn shipped_builders_are_byte_identical() {
    // Every axis builder called at its `(shipped)` band value must be byte-identical to
    // `frontier_emergency_provision` — otherwise the `(shipped)` label and the `shipped_index`
    // would silently desync from the real scenario if a scenario default later drifts off these
    // values (Reviewer-2 P3, mirroring the `with_cushion(0)`/`with_emergency(0)` revert checks in
    // demand_survival_bridge.rs). Compared by canonical_bytes at generation AND by a 500-tick
    // digest (so a drift in a non-digested-but-behavioural field is caught too).
    let shipped = SettlementConfig::frontier_emergency_provision();
    let mut base_gen = Settlement::generate(7, &shipped);
    let base_canonical = base_gen.canonical_bytes();
    base_gen.run(500);
    let base_digest = base_gen.digest();

    let cases: [(&str, SettlementConfig); 8] = [
        ("with_emergency(11)", with_emergency(11)),
        ("with_grain_regen(24)", with_grain_regen(24)),
        (
            "with_lineage_wood_provision(0)",
            with_lineage_wood_provision(0),
        ),
        ("with_salt_period(8)", with_salt_period(8)),
        ("with_acceptors(2)", with_acceptors(2)),
        ("with_roles(8,4,4)", with_roles(8, 4, 4)),
        ("with_birth_ceiling(8)", with_birth_ceiling(8)),
        ("with_death_window(3)", with_death_window(3)),
    ];
    for (label, cfg) in cases {
        let mut s = Settlement::generate(7, &cfg);
        assert_eq!(
            s.canonical_bytes(),
            base_canonical,
            "{label} must be canonically byte-identical to frontier_emergency_provision at \
             generation (the (shipped) band label desynced from the scenario default)"
        );
        s.run(500);
        assert_eq!(
            s.digest(),
            base_digest,
            "{label} must run byte-identically to frontier_emergency_provision (the (shipped) \
             band label desynced from the scenario default)"
        );
    }
}

// =========================================================================
// A. Seed robustness of the three headline regimes (the spine)
// =========================================================================

#[test]
fn headline_household_barter_success_across_wide_seeds() {
    // S21f → SUCCESS for every seed (mortality off): the 18 non-lineage roles survive, SALT
    // promotes on SelfProduced bread, food is materially bought, provenance clean.
    eprintln!(
        "=== S21i (A) spine: frontier_household_barter (S21f) across WIDE_SEEDS={WIDE_SEEDS:?} ==="
    );
    let cells = spine_cells(
        "spine:household_barter",
        &SettlementConfig::frontier_household_barter(),
        &WIDE_SEEDS,
    );
    for (&seed, cell) in WIDE_SEEDS.iter().zip(&cells) {
        assert_guards(cell, &format!("household_barter seed={seed}"));
        eprintln!(
            "  household_barter seed={seed}: survivors(non-lineage)={} | {}",
            cell.non_lineage,
            cell.map_line()
        );
        assert_eq!(
            cell.regime(),
            Regime::Success,
            "S21f must be SUCCESS for seed {seed}: {cell:?}"
        );
    }
}

#[test]
fn headline_open_colony_mortality_cull_across_wide_seeds() {
    // S21g → CULL for every seed: the positive check wipes the non-lineage demand side
    // (living_non_lineage == 0) before money forms (current_money_good() == None); the
    // self-feeding lineage survives.
    eprintln!(
        "=== S21i (A) spine: frontier_open_colony_mortality (S21g) across WIDE_SEEDS={WIDE_SEEDS:?} ==="
    );
    let cells = spine_cells(
        "spine:open_colony_mortality",
        &SettlementConfig::frontier_open_colony_mortality(),
        &WIDE_SEEDS,
    );
    for (&seed, cell) in WIDE_SEEDS.iter().zip(&cells) {
        assert_guards(cell, &format!("open_colony_mortality seed={seed}"));
        eprintln!(
            "  open_colony_mortality seed={seed}: lineage={} | {}",
            cell.lineage,
            cell.map_line()
        );
        assert_eq!(
            cell.regime(),
            Regime::Cull,
            "S21g must be CULL for seed {seed}: {cell:?}"
        );
        assert_eq!(
            cell.non_lineage, 0,
            "S21g seed {seed}: the demand side must be wiped out (living_non_lineage == 0)"
        );
        assert!(
            !cell.promoted,
            "S21g seed {seed}: SALT must never promote (current_money_good() == None)"
        );
        assert!(
            cell.lineage > 0,
            "S21g seed {seed}: the self-feeding lineage must survive the cull"
        );
    }
}

#[test]
fn headline_emergency_provision_success_across_wide_seeds() {
    // S21h.1 → SUCCESS for every seed: the produced near-critical emergency floor keeps the
    // non-lineage demand side alive AND demanding AND buying, SALT promotes on SelfProduced bread
    // with seeded_minted == 0. The 12/18 survivor figure is seed-7-specific — report the per-seed
    // survivor count, do NOT assert a fixed 12.
    eprintln!(
        "=== S21i (A) spine: frontier_emergency_provision (S21h.1) across WIDE_SEEDS={WIDE_SEEDS:?} ==="
    );
    let cells = spine_cells(
        "spine:emergency",
        &SettlementConfig::frontier_emergency_provision(),
        &WIDE_SEEDS,
    );
    let mut survivors = Vec::new();
    for (&seed, cell) in WIDE_SEEDS.iter().zip(&cells) {
        assert_guards(cell, &format!("emergency seed={seed}"));
        eprintln!(
            "  emergency seed={seed}: survivors(non-lineage)={}/18 | {}",
            cell.non_lineage,
            cell.map_line()
        );
        assert_eq!(
            cell.regime(),
            Regime::Success,
            "S21h.1 must be SUCCESS for seed {seed}: {cell:?}"
        );
        assert_eq!(
            cell.credited_seeded_minted, 0,
            "S21h.1 seed {seed}: no SeededMinted bread may ever enter (provenance fully restored)"
        );
        survivors.push((seed, cell.non_lineage));
    }
    eprintln!("  per-seed non-lineage survivors (NOT pinned): {survivors:?}");
}

// =========================================================================
// B. CORE-axis 1-D window maps (feed the capstone verdict)
// =========================================================================

/// Assert the shipped band cell is SUCCESS across all `CROSS_SEEDS` and print the criterion. The
/// criterion itself is reported, not gated here — the capstone verdict aggregates it.
fn run_core_axis(axis: AxisEval) {
    let shipped = &axis.cells[axis.shipped_index];
    for (&seed, cell) in CROSS_SEEDS.iter().zip(&shipped.cells) {
        assert!(
            cell.is_success(),
            "axis {} shipped cell {} must be SUCCESS for seed {seed}: {cell:?}",
            axis.name,
            shipped.label
        );
    }
    axis.report();
}

#[test]
fn core_axis_emergency_threshold_map() {
    eprintln!(
        "=== S21i (B) CORE axis: emergency_hunger_threshold across CROSS_SEEDS={CROSS_SEEDS:?} ==="
    );
    eprintln!(
        "  band {{7,8,9,10,11}} is the full validator-valid set (6 < threshold < 12); shipped 11 \
         is the TOP bound → high side BOUNDED-BY-AXIS, two-step criterion on the low side only."
    );
    run_core_axis(core_axis_emergency_threshold());
}

#[test]
fn core_axis_grain_flow_map() {
    eprintln!("=== S21i (B) CORE axis: grain node regen across CROSS_SEEDS={CROSS_SEEDS:?} ===");
    eprintln!("  band {{12,18,24,36,48}} brackets the shipped 24 (too little starves the lineage, too much over-feeds).");
    run_core_axis(core_axis_grain_flow());
}

#[test]
fn core_axis_wood_scarcity_map() {
    eprintln!(
        "=== S21i (B) CORE axis: lineage WOOD scarcity (wood_provision) across CROSS_SEEDS={CROSS_SEEDS:?} ==="
    );
    eprintln!(
        "  no silent cap: the spec named `chain.wood_buffer` here, but that field only seeds the \
         non-lineage woodcutters and is OUTCOME-INERT in this topology (every Cell figure is \
         byte-identical across {{4,8,12,24,48}} for every seed). The lane-driving scarcity is the \
         lineage sellers' own recurring WOOD provision, so the axis sweeps that instead (Reviewer-1 \
         P2)."
    );
    eprintln!(
        "  band {{0,1,2,3,4}}: wood_provision=0 (shipped) is the hard WOOD floor → LOW side \
         BOUNDED-BY-AXIS (cannot be poorer than zero WOOD); the two-step criterion applies on the \
         high side, where each notch of WOOD relaxes the lineage's unsatisfied WOOD want."
    );
    run_core_axis(core_axis_wood_scarcity());
}

#[test]
fn core_axis_salt_period_map() {
    eprintln!("=== S21i (B) CORE axis: barter.salt_direct_use_period across CROSS_SEEDS={CROSS_SEEDS:?} ===");
    eprintln!("  band {{4,6,8,12,16}} brackets the shipped 8 (smaller period = denser regression-theorem direct-use anchor).");
    run_core_axis(core_axis_salt_period());
}

// =========================================================================
// B-sensitivity. SENSITIVITY axes (classified + printed; EXCLUDED from the verdict)
// =========================================================================

#[test]
fn sensitivity_strong_bar_thresholds() {
    eprintln!(
        "=== S21i (B-sensitivity) strong-bar thresholds across CROSS_SEEDS={CROSS_SEEDS:?} (NOT in the core verdict) ==="
    );
    // Acceptors {1,2,3} is a sensitivity map (shipped 2).
    let band = vec![
        ("acceptors=1".to_string(), with_acceptors(1)),
        ("acceptors=2(shipped)".to_string(), with_acceptors(2)),
        ("acceptors=3".to_string(), with_acceptors(3)),
    ];
    let cells = sweep_band("min_direct_use_acceptors", &band);
    let any_success = cells.iter().any(|c| c.success_all_seeds);
    eprintln!("  min_direct_use_acceptors sensitivity: any SUCCESS-across-CROSS_SEEDS cell? {any_success}");
    // min_indirect_target_goods = 3 is INFEASIBLE-by-topology: the {bread, WOOD} world supplies
    // only two medium targets, so SALT can never reach three indirect target goods. It is NOT
    // classified as narrow — it is excluded as infeasible (no silent cap: logged here, not run).
    eprintln!(
        "  min_indirect_target_goods: shipped=2; =3 SKIPPED as INFEASIBLE-by-topology (the \
         {{bread, WOOD}} two-target world supplies only two medium targets — a failure there is \
         the topology, not an S21h narrowness, so it is excluded from any criterion)."
    );
}

#[test]
fn sensitivity_role_counts() {
    eprintln!(
        "=== S21i (B-sensitivity) role topology across CROSS_SEEDS={CROSS_SEEDS:?} (structural; NOT in the core verdict) ==="
    );
    eprintln!(
        "  changing counts shifts ID layout, demand-hub size, production capacity, and effective \
         SALT distribution — the spec does not pin how SALT supply/thresholds scale with \
         population, so this is a structural sensitivity map only."
    );
    let gatherers = vec![
        ("gatherers=6".to_string(), with_roles(6, 4, 4)),
        ("gatherers=8(shipped)".to_string(), with_roles(8, 4, 4)),
        ("gatherers=10".to_string(), with_roles(10, 4, 4)),
    ];
    let consumers = vec![
        ("consumers=3".to_string(), with_roles(8, 3, 4)),
        ("consumers=4(shipped)".to_string(), with_roles(8, 4, 4)),
        ("consumers=5".to_string(), with_roles(8, 5, 4)),
    ];
    let founders = vec![
        ("founders=2".to_string(), with_roles(8, 4, 2)),
        ("founders=4(shipped)".to_string(), with_roles(8, 4, 4)),
        ("founders=6".to_string(), with_roles(8, 4, 6)),
    ];
    for (name, band) in [
        ("gatherers", gatherers),
        ("consumers", consumers),
        ("founders_total", founders),
    ] {
        eprintln!("  -- role sub-axis: {name} --");
        let _ = sweep_band(&format!("roles:{name}"), &band);
    }
}

#[test]
fn sensitivity_mortality_timing() {
    eprintln!(
        "=== S21i (B-sensitivity) mortality timing across CROSS_SEEDS={CROSS_SEEDS:?} (NOT in the core verdict) ==="
    );
    eprintln!("  hunger_critical is the mortality on/off switch and is held at the shipped need_max (not swept).");
    // birth_hunger_ceiling band {6,8,10}: 6 is INFEASIBLE (validator requires cultivate_hunger_in
    // (6) < birth_hunger_ceiling), so it is skipped with a note (no silent cap), not classified.
    eprintln!(
        "  birth_hunger_ceiling: band {{6,8,10}} — 6 SKIPPED as INFEASIBLE (validator requires \
         cultivate_hunger_in (6) < birth_hunger_ceiling); classifying {{8,10}}."
    );
    let ceilings = vec![
        (
            "birth_ceiling=8(shipped)".to_string(),
            with_birth_ceiling(8),
        ),
        ("birth_ceiling=10".to_string(), with_birth_ceiling(10)),
    ];
    let _ = sweep_band("mortality:birth_hunger_ceiling", &ceilings);

    let windows = vec![
        ("death_window=2".to_string(), with_death_window(2)),
        ("death_window=3(shipped)".to_string(), with_death_window(3)),
        ("death_window=4".to_string(), with_death_window(4)),
    ];
    let _ = sweep_band("mortality:death_window", &windows);
}

// =========================================================================
// B′. Interaction maps (classification-only; guards + shipped SUCCESS only)
// =========================================================================

/// Classify a 3×3 interaction grid across `seeds`, assert the guards on every cell, print the
/// grid, and return whether the shipped (×,×) cell is SUCCESS across every seed.
fn interaction_map(
    name: &str,
    rows: &[(String, bool)],
    cols: &[(String, bool)],
    seeds: &[u64],
    build: impl Fn(usize, usize) -> SettlementConfig,
) -> bool {
    eprintln!("=== S21i (B′) interaction map: {name} across seeds={seeds:?} ===");
    let mut shipped_success = true;
    let mut shipped_seen = false;
    for (ri, (rlabel, rshipped)) in rows.iter().enumerate() {
        for (ci, (clabel, cshipped)) in cols.iter().enumerate() {
            let cfg = build(ri, ci);
            let jobs: Vec<Job> = seeds
                .iter()
                .map(|&seed| Job {
                    key: format!("ix:{name}:{rlabel}/{clabel}"),
                    seed,
                    cfg: cfg.clone(),
                })
                .collect();
            let cells = classify_jobs(&jobs);
            for (&seed, cell) in seeds.iter().zip(&cells) {
                assert_guards(cell, &format!("[{name}] {rlabel} × {clabel} seed={seed}"));
                eprintln!(
                    "  [{name}] {rlabel} × {clabel} seed={seed}: {}",
                    cell.map_line()
                );
                if *rshipped && *cshipped {
                    shipped_seen = true;
                    shipped_success &= cell.is_success();
                }
            }
        }
    }
    assert!(
        shipped_seen,
        "interaction {name}: the shipped (×,×) cell must be in the grid"
    );
    shipped_success
}

#[test]
fn interaction_threshold_x_grain() {
    // Does a leaner food supply narrow the threshold window? Threshold {9,10,11} (11 shipped, the
    // top bound) × grain regen {12,24,48} (24 shipped). Shipped cell = (11, 24).
    let thr_by_row = [9u16, 10, 11];
    let regens = [12u32, 24, 48];
    let rows = [
        ("thr=9".to_string(), false),
        ("thr=10".to_string(), false),
        ("thr=11(shipped)".to_string(), true),
    ];
    let cols = [
        ("regen=12".to_string(), false),
        ("regen=24(shipped)".to_string(), true),
        ("regen=48".to_string(), false),
    ];
    let shipped_success = interaction_map(
        "emergency_threshold × grain_flow",
        &rows,
        &cols,
        &[7, 19],
        |ri, ci| with_threshold_and_grain(thr_by_row[ri], regens[ci]),
    );
    assert!(
        shipped_success,
        "the shipped (threshold=11, regen=24) interaction cell must be SUCCESS across both seeds"
    );
}

#[test]
fn interaction_wood_x_salt() {
    // Does a denser SALT anchor compensate for less WOOD scarcity, or do they jointly gate SALT
    // leadership? Lineage wood_provision {0,1,2} (0 shipped, the WOOD floor) × salt period
    // {4,8,16} (8 shipped). Shipped cell = (provision=0, period=8). The WOOD dimension uses the
    // lane-driving lineage provision, not the outcome-inert `chain.wood_buffer` (see the
    // `lineage_wood_provision` CORE axis).
    let provisions = [0u32, 1, 2];
    let periods = [4u16, 8, 16];
    let rows = [
        ("wood_provision=0(shipped)".to_string(), true),
        ("wood_provision=1".to_string(), false),
        ("wood_provision=2".to_string(), false),
    ];
    let cols = [
        ("period=4".to_string(), false),
        ("period=8(shipped)".to_string(), true),
        ("period=16".to_string(), false),
    ];
    let shipped_success = interaction_map(
        "lineage_wood_provision × salt_anchor_density",
        &rows,
        &cols,
        &[7, 19],
        |ri, ci| with_wood_and_salt(provisions[ri], periods[ci]),
    );
    assert!(
        shipped_success,
        "the shipped (wood_provision=0, period=8) interaction cell must be SUCCESS across both seeds"
    );
}

// =========================================================================
// C. The capstone verdict (prints ROBUST / NARROW-BAND / MIXED; does NOT assert ROBUST)
// =========================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    Robust,
    NarrowBand,
    Mixed,
}

#[test]
fn capstone_robustness_verdict() {
    eprintln!("================ S21i CAPSTONE ROBUSTNESS VERDICT ================");

    // --- headline-regime stability across WIDE_SEEDS (part A, recomputed via the cache) ---
    let barter = spine_cells(
        "spine:household_barter",
        &SettlementConfig::frontier_household_barter(),
        &WIDE_SEEDS,
    );
    let mortality = spine_cells(
        "spine:open_colony_mortality",
        &SettlementConfig::frontier_open_colony_mortality(),
        &WIDE_SEEDS,
    );
    let emergency = spine_cells(
        "spine:emergency",
        &SettlementConfig::frontier_emergency_provision(),
        &WIDE_SEEDS,
    );
    for cell in barter.iter().chain(&mortality).chain(&emergency) {
        assert_guards(cell, "verdict headline cell");
    }
    let barter_ok = barter.iter().all(|c| c.regime() == Regime::Success);
    let mortality_ok = mortality.iter().all(|c| c.regime() == Regime::Cull);
    let emergency_ok = emergency.iter().all(|c| c.regime() == Regime::Success);
    let headline_stable = barter_ok && mortality_ok && emergency_ok;
    eprintln!(
        "headline regimes across WIDE_SEEDS: S21f(barter)=SUCCESS? {barter_ok} | \
         S21g(mortality)=CULL? {mortality_ok} | S21h.1(emergency)=SUCCESS? {emergency_ok} | \
         stable? {headline_stable}"
    );

    // --- CORE-axis criteria (part B) ---
    let axes = all_core_axes();
    eprintln!("---- CORE-axis summary table ----");
    for axis in &axes {
        // Classification sanity (NOT a verdict gate): the shipped cell is SUCCESS across CROSS_SEEDS.
        let shipped = &axis.cells[axis.shipped_index];
        assert!(
            shipped.success_all_seeds,
            "axis {} shipped cell {} must be SUCCESS across CROSS_SEEDS",
            axis.name, shipped.label
        );
        axis.report();
    }

    // --- aggregate into the §2 verdict ---
    let narrow_axes: Vec<&str> = axes
        .iter()
        .filter(|a| a.criterion == Criterion::Narrow)
        .map(|a| a.name.as_str())
        .collect();
    let robust_or_bounded = axes
        .iter()
        .filter(|a| a.criterion != Criterion::Narrow)
        .count();

    let verdict = if !headline_stable {
        Verdict::NarrowBand
    } else if narrow_axes.is_empty() {
        Verdict::Robust
    } else if robust_or_bounded > 0 {
        Verdict::Mixed
    } else {
        Verdict::NarrowBand
    };

    // Name the limiting axes / unstable headline regimes.
    let mut limiting: Vec<String> = narrow_axes.iter().map(|s| s.to_string()).collect();
    if !barter_ok {
        limiting.push("headline:S21f-barter".to_string());
    }
    if !mortality_ok {
        limiting.push("headline:S21g-mortality".to_string());
    }
    if !emergency_ok {
        limiting.push("headline:S21h.1-emergency".to_string());
    }

    eprintln!("----------------------------------------------------------------");
    match verdict {
        Verdict::Robust => eprintln!(
            "CAPSTONE ROBUSTNESS VERDICT: ROBUST — every CORE axis is ROBUST-on-axis (or \
             BOUNDED-BY-AXIS on a hard bound) and the three headline regimes hold across all \
             WIDE_SEEDS. The capstone headline stands as written (still 'in this configured \
             topology')."
        ),
        Verdict::NarrowBand => eprintln!(
            "CAPSTONE ROBUSTNESS VERDICT: NARROW-BAND FINDING — the S21h.1 coexistence holds as an \
             existence proof under a narrow survival-bridge band. Limiting: {limiting:?}."
        ),
        Verdict::Mixed => eprintln!(
            "CAPSTONE ROBUSTNESS VERDICT: MIXED — robust on some CORE axes, narrow on others. \
             Load-bearing / limiting axes: {limiting:?}."
        ),
    }
    eprintln!("================================================================");

    // This test classifies; it does NOT assert ROBUST. It asserts only that the guards held (above)
    // and that the verdict is a well-formed value the data supports.
    assert!(
        matches!(
            verdict,
            Verdict::Robust | Verdict::NarrowBand | Verdict::Mixed
        ),
        "the verdict must be one of the three §2 outcomes"
    );
}
