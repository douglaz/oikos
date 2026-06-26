//! Shared S21 demand-bridge classification machinery (impl-32 §3.1).
//!
//! Extracted verbatim from `tests/demand_survival_bridge.rs` so the S21h suite and the S21i
//! robustness appendix classify every swept cell by the *same* 5-tuple provenance/demand
//! vector — no re-derivation, no copy. Both consumers include this file via
//! `#[path = "support/mod.rs"] mod support;`. The `Cell`/`is_success`/`classify` machinery, the
//! mutators (`with_cushion`, `with_cushion_split`, `with_emergency`), the living-roster helpers,
//! and the constants (`PROBE_TICKS`, `SEEDS`, `MATERIAL_BOUGHT_FLOOR`) are the move; the `Regime`
//! enum + [`Cell::regime`] + [`Cell::map_line`] are the additive sweep-reporting layer S21i needs.
//!
//! Every accessor it calls on [`Settlement`] is read-only, so collecting a `Cell` perturbs the
//! run not at all and adds no engine code.
#![allow(dead_code)]

use econ::good::{GoodId, SALT};
use sim::{Settlement, SettlementConfig};

/// A horizon long enough to clear the cold-start, promote (or fail to), and settle (the S21f/g
/// money suites use 1600).
pub const PROBE_TICKS: u64 = 1_600;

/// The robustness seed set (the S21f/g suites use the same).
pub const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];

/// Bread bought over the run below which we do not call the market demand "material".
pub const MATERIAL_BOUGHT_FLOOR: u64 = 1_000;

// ---- shared helpers -----------------------------------------------------

pub fn living(s: &Settlement) -> usize {
    (0..s.population()).filter(|&i| s.is_alive(i)).count()
}

/// Living **lineage** members (the self-feeding cultivators — `household_of` is `Some`).
pub fn living_lineage(s: &Settlement) -> usize {
    (0..s.population())
        .filter(|&i| s.is_alive(i) && s.household_of(i).is_some())
        .count()
}

/// Living **non-lineage** roles (the SALT-rich buyers + the woodcutters — the demand side the
/// positive check culls in S21g; `household_of` is `None`).
pub fn living_non_lineage(s: &Settlement) -> usize {
    (0..s.population())
        .filter(|&i| s.is_alive(i) && s.household_of(i).is_none())
        .count()
}

pub fn bread_good(s: &Settlement) -> GoodId {
    s.bread_good()
        .expect("the demand-bridge chain carries a bread good")
}

/// Sweep the consumed-only cushion (both axes together: the buyers' `consumer_staple_buffer`
/// and the woodcutters' `gatherer_food_cushion` to the same size `c`).
pub fn with_cushion(c: u32) -> SettlementConfig {
    with_cushion_split(c, c)
}

/// Sweep the two cushion axes INDEPENDENTLY: the buyers' `consumer_staple_buffer` to `consumer`
/// and the woodcutters' `gatherer_food_cushion` to `gatherer`. The diagonal `with_cushion(c)`
/// is the `consumer == gatherer` case; this lets the knife-edge claim also bracket off-diagonal
/// (asymmetric) cushion combinations.
pub fn with_cushion_split(consumer: u32, gatherer: u32) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_demand_cushion();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.consumer_staple_buffer = consumer;
        chain.gatherer_food_cushion = gatherer;
    }
    cfg
}

/// The emergency seam at a swept `threshold`.
pub fn with_emergency(threshold: u16) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_emergency_provision();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.emergency_hunger_threshold = threshold;
    }
    cfg
}

// ---- the per-cell classification vector ---------------------------------

/// The full 5-tuple the knife-edge sweep classifies every cell by, plus the broken-invariant
/// guards a run must always pass. Read-only public accessors only, so collecting it perturbs
/// nothing.
#[derive(Clone, Copy, Debug)]
pub struct Cell {
    /// (1) The non-lineage demand side is alive at the end of the run (the S21g cull did NOT
    /// wipe it out).
    pub survived: bool,
    /// (2) The SURVIVING non-lineage roles still emit a present `Horizon::Now` bread want —
    /// the bridge did not satiate them out of the bread market.
    pub demanded: bool,
    /// (3) SALT promoted to money.
    pub promoted: bool,
    /// (4) Food was materially BOUGHT on the market over the run (a real demand side).
    pub bought_materially: bool,
    /// (5) NO `SeededMinted`/cushion bread was ever sold for SALT, and the pre-promotion
    /// bread that monetized SALT was `SelfProduced` (minted volume 0) — the hard provenance
    /// invariant. A cell where this is false is DISQUALIFIED (a seeded-supply result).
    pub provenance_clean: bool,
    // Broken-invariant guards (must hold regardless of the outcome).
    pub conserved: bool,
    pub bread_minted_max: u64,
    pub extinct: bool,
    pub credited_seeded_minted: u64,
    // Figures for reporting.
    pub non_lineage: usize,
    pub lineage: usize,
    pub starvation: u64,
    pub bought: u64,
    pub self_produced: u64,
}

/// The regime each swept cell maps to (impl-32 §2): the five labels the robustness appendix
/// classifies every (axis cell, seed) into. A total mapping of the 5-tuple + guards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Regime {
    /// All five of the 5-tuple hold — alive + still-hungry demand + SALT promoted + material
    /// buying + provenance clean (the S21h.1 success).
    Success,
    /// `!survived` — the positive check wiped the non-lineage demand side (the S21g regime).
    Cull,
    /// `survived && !demanded` — alive but the bridge satiated bread demand out of the market
    /// (the over-cushion / too-strong failure mode).
    Sated,
    /// `survived && demanded && !promoted` — the demand side persists but money never forms
    /// (a partial; distinguishes "demand alive but supply/anchor insufficient" from a cull).
    /// The catch-all partial: also absorbs the rare clean-but-not-materially-bought edge.
    SurvivedNoPromote,
    /// `!provenance_clean` — a promotion that sold seeded `SeededMinted` bread for SALT (the
    /// seeded-supply trap; never counted as a success).
    Disqualified,
}

impl Cell {
    /// The S21h SUCCESS: the demand side survives AND still demands AND SALT promotes AND food
    /// is materially bought AND the provenance is clean.
    pub fn is_success(&self) -> bool {
        self.survived
            && self.demanded
            && self.promoted
            && self.bought_materially
            && self.provenance_clean
    }

    /// Map the 5-tuple + guards to the §2 [`Regime`]. DISQUALIFIED takes precedence (a cell that
    /// sold seeded bread for SALT is disqualified regardless of any other property), then the
    /// full SUCCESS, then the cull / sated / survived-no-promote partition over the survivors.
    pub fn regime(&self) -> Regime {
        if !self.provenance_clean {
            Regime::Disqualified
        } else if self.is_success() {
            Regime::Success
        } else if !self.survived {
            Regime::Cull
        } else if !self.demanded {
            Regime::Sated
        } else {
            // survived && demanded && provenance_clean but not a full success: either SALT did
            // not promote, or it promoted without material buying. Both are partials.
            Regime::SurvivedNoPromote
        }
    }

    /// A uniform one-line regime + 5-tuple + figures rendering for the per-cell `eprintln!` maps
    /// both suites print, so every sweep reports the same fields in the same order.
    pub fn map_line(&self) -> String {
        format!(
            "{:?} | survived={} demanded={} promoted={} bought_mat={} clean={} | \
             non-lineage={} lineage={} starv={} bought={} self_prod={} seeded_credited={}",
            self.regime(),
            self.survived,
            self.demanded,
            self.promoted,
            self.bought_materially,
            self.provenance_clean,
            self.non_lineage,
            self.lineage,
            self.starvation,
            self.bought,
            self.self_produced,
            self.credited_seeded_minted,
        )
    }
}

/// Run `(seed, cfg)` for `ticks` and collect the classification vector. The demand probe is
/// measured on the LIVING non-lineage roles each tick and the max retained, so a cell where
/// the survivors demand bread at any point (pre- or post-promotion) reads `demanded = true`.
pub fn classify(seed: u64, cfg: &SettlementConfig, ticks: u64) -> Cell {
    let mut s = Settlement::generate(seed, cfg);
    let bread = bread_good(&s);
    let mut conserved = true;
    let mut bread_minted_max = 0u64;
    let mut demanded_while_alive = false;
    for _ in 0..ticks {
        let report = s.econ_tick();
        conserved &= report.conserves();
        bread_minted_max = bread_minted_max.max(report.endowment_of(bread));
        // Demand among the LIVING non-lineage roles — read each tick so a cell whose buyers
        // survive and demand (even briefly, before any cull) is distinguished from one whose
        // buyers are sated to zero Now-wants throughout.
        if living_non_lineage(&s) > 0 && s.living_non_lineage_with_bread_now_wants(bread) > 0 {
            demanded_while_alive = true;
        }
    }
    let consumed = s.acquisition_consumed_by_channel();
    let credited = s.acquisition_credited_by_channel();
    let (pp_produced, pp_minted) = s.pre_promotion_bread_for_salt_by_provenance();
    let _ = pp_produced;
    // The hard provenance invariant: NO cushion (SeededMinted) bread sold for SALT, AND the
    // pre-promotion bread that monetized SALT was SelfProduced (minted volume 0).
    let provenance_clean = s.seeded_minted_bread_sold_for_salt() == 0 && pp_minted == 0;
    // Demand among the survivors at the end (the persisting demand side).
    let demanded_at_end = s.living_non_lineage_with_bread_now_wants(bread) > 0;
    Cell {
        survived: living_non_lineage(&s) > 0,
        demanded: demanded_while_alive && demanded_at_end,
        promoted: s.current_money_good() == Some(SALT),
        bought_materially: consumed.bought >= MATERIAL_BOUGHT_FLOOR,
        provenance_clean,
        conserved,
        bread_minted_max,
        extinct: living(&s) == 0,
        credited_seeded_minted: credited.seeded_minted,
        non_lineage: living_non_lineage(&s),
        lineage: living_lineage(&s),
        starvation: s.starvation_deaths_total(),
        bought: consumed.bought,
        self_produced: consumed.self_produced,
    }
}
