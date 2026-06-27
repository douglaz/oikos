//! S22d acceptance suite — **durable role-specific cultivation capital** (impl-36): does a
//! default-off, buildable, DURABLE, agent-OWNED, role-specific cultivation tool (capital) finally
//! turn S22a's FLUID participation into a STABLE role split — a persistent cohort of tool-owning
//! cultivators plus persistent non-owner buyers — while money/mortality/provenance/conservation
//! survive, AND is any stickiness from durability/OWNERSHIP rather than raw productivity?
//!
//! The role-topology arc is a clean negative so far: hunger discovers the role (S22a, fluid),
//! accumulated SKILL doesn't change the hunger-gated exit (S22b, no stickiness), and a realized
//! PROFIT stay-incentive retains only marginally (S22c, no-stay-despite-profit). S22d is the
//! sufficiency test for the named boundary — durable lock-in via sunk, owned, asset-specific
//! capital. The single engine change composed here is a default-off `durable_cultivation_tool`
//! gate (composing on S22c profit-stay, which composes on S22a entry): a sustained-PRODUCING
//! cultivator invests a SUNK cost (WOOD + labor) into a durable plow it then OWNS; the plow raises
//! ONLY its owner's grain-haul ceiling WHILE it cultivates (asset specificity). The owner's higher
//! realized cultivation return then flows through the UNMODIFIED S22c profit-stay exit — no stay
//! flag is added, no exit branch edited — so any stickiness comes from durable OWNERSHIP, not raw
//! productivity. The headline scenario ([`SettlementConfig::frontier_cultivation_capital`]) is the
//! S22c money colony (skill OFF) with ONLY that gate flipped (+ the plow good interned).
//!
//! **The lever must be NON-VACUOUS** ([`nonvacuity_capital_lever_bites`]): under MATCHED conditions
//! a tool-OWNING cultivator harvests STRICTLY MORE grain AND produces STRICTLY MORE bread than a
//! no-tool cultivator over the same horizon, AND ≥1 tool is actually built from CONSUMED WOOD, AND
//! ≥1 owner enters the retention signal. Else the headline is **CAPITAL LEVER INERT** (not "no
//! stickiness" — the S22b/c lever-bite lesson). Checked FIRST.
//!
//! This suite then **classifies** the treatment against the §2 pre-named outcomes via an ORDERED,
//! mutually-exclusive classifier (checked top-down; the FIRST that matches is the verdict) and
//! PRINTS the verdict; it does **not** assert SUCCESS. Every threshold is PREDECLARED a priori
//! (§7), never fitted. Churn is always compared to the MATCHED-SEED S22c baseline
//! ([`SettlementConfig::frontier_profit_retention`]). Two falsifying controls flank the headline:
//! the **productivity-only** control (the SAME boost to every cultivator, no owned/durable asset)
//! and the **non-durable/rented** control (the same owner-only boost but the tool is consumed after
//! one cultivation opportunity) — if either reproduces the stickiness, the result is PRODUCTIVITY
//! ONLY, not capital. Every run satisfies the hard guards (conservation each tick,
//! `bread_minted_max == 0`, provenance-clean-or-disqualified, `!extinct`, the tool-stock accounting
//! invariant). The five tripwire goldens are re-pinned byte-identical ([`goldens_unchanged`]).
//!
//! Run the verdict with `--nocapture` to read the classification:
//!   `cargo test -p sim --test durable_cultivation_capital capital_verdict -- --nocapture`

use std::collections::{BTreeMap, BTreeSet};

use econ::good::SALT;
use sim::{Settlement, SettlementConfig};

// The shared S21 demand-bridge classification machinery (the living-roster helpers + the
// `PROBE_TICKS`/`SEEDS`/`MATERIAL_BOUGHT_FLOOR` constants).
#[path = "support/mod.rs"]
mod support;
use support::*;

// =========================================================================
// Predeclared thresholds (a priori — §7; NEVER fitted to the data)
// =========================================================================

/// Treatment per-ever-cultivating-agent churn must fall to ≤ `CHURN_DROP ×` the **matched-seed**
/// S22c baseline churn ([`SettlementConfig::frontier_profit_retention`]) for "churn fell
/// materially".
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

/// Rolling cultivator share at/above which most survivors cultivate (the commune side).
const COMMUNE_SHARE: f64 = 0.75;

/// Living non-lineage roles at/below which the demand side has collapsed (the monopolization DAMAGE
/// floor — spec §2).
const DEMAND_COLLAPSE_FLOOR: usize = 4;

/// The maximum owner-share-among-ever-cultivators for ownership to count as a *minority* edge
/// (above ⇒ universal ownership ⇒ a cosmetic global productivity boost, PRODUCTIVITY ONLY — §3.3/§7).
const OWNER_SHARE_MAX: f64 = 0.6;

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
/// split (a non-cultivating, non-owner demand side that is alive AND buying).
const MIN_BUYER_COHORT: usize = 2;

/// The micro-harness horizon for the matched-condition non-vacuity comparison (long enough for the
/// owner-haul edge to accumulate a clear, strict grain/bread lead).
const MATCHED_HORIZON: u64 = 400;

// =========================================================================
// The ordered, mutually-exclusive classifier (spec §2)
// =========================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    /// (1) PRECONDITION (from non-vacuity): the lever did not engage in this run — no tool built,
    /// or no owner entered the retention signal, so capitalized agents cannot have out-produced.
    /// A distinct outcome from "no stickiness" (the S22b/c lever-bite lesson).
    CapitalLeverInert,
    /// (2) A guard failed (conservation / `bread_minted_max>0` / `!extinct` / the tool-stock
    /// accounting invariant `built − destroyed == stock_total`) OR provenance is DISQUALIFIED
    /// (seeded `SeededMinted` bread sold for SALT, or pre-promotion minted volume) — ConservationBroken.
    BrokenInvariant,
    /// (3) The top cultivator takes ≥ `MONO_SHARE` of final-window grain AND the non-lineage /
    /// material-buyer side falls below the survival/buyer floor (dominance AND damage).
    MonopolizationCull,
    /// (4) Rolling cultivator share ≥ `COMMUNE_SHARE` AND post-promotion bought below the floor —
    /// most survivors cultivate and the market dissolves.
    CommuneCollapse,
    /// (5) Money never forms / fails at horizon despite produced+sold `SelfProduced` bread.
    MoneyFailureFromLockIn,
    /// (6) The run would be SUCCESS but a falsifying control ALSO clears the churn-drop + cohort
    /// bars (productivity-only, or non-durable/rented), OR ownership is universal
    /// (owner-share > `OWNER_SHARE_MAX`) — the effect is raw output / per-use boost, not durable
    /// capital; S22d did not isolate capital.
    ProductivityOnly,
    /// (7) Money + mortality survive, the tool bites, but churn did not fall materially vs the
    /// matched-seed S22c baseline AND no persistent OWNER cohort formed.
    NoStickinessDespiteCapital,
    /// (8) None of the above AND all SUCCESS criteria hold: churn ≤ 0.5× baseline, a persistent
    /// membership cohort (`PERSIST_COHORT`, ≥`PERSIST_COHORT_NONLINEAGE` non-lineage) that ARE
    /// tool-owners, owner-share ≤ `OWNER_SHARE_MAX`, a surviving non-owner buyer cohort, and
    /// money + mortality survive (the sticky cohort is the capitalized one, isolated from
    /// productivity by the controls).
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
    // ---- capital lever (non-vacuity precondition, per run) ----
    tools_built: u64,
    tools_destroyed: u64,
    tool_stock_total: u64,
    tool_wood_consumed: u64,
    owner_in_signal_ever: bool,
    // ---- churn (per ever-cultivating agent; same measure as the matched baseline) ----
    churn_total: u32,
    ever_cultivating: usize,
    // ---- persistent MEMBERSHIP cohort (distinct ids cultivating >= PERSIST_FRACTION final win) --
    persistent_cohort: usize,
    persistent_cohort_nonlineage: usize,
    // ---- of the persistent cohort, those that ARE tool-owners (ever owned a plow) ----
    persistent_cohort_owners: usize,
    persistent_cohort_owners_nonlineage: usize,
    // ---- ownership concentration (the isolation guard) ----
    owner_share_ever: f64,
    // ---- top-cultivator grain share (final window, the monopolization probe) ----
    top_cultivator_grain_share: f64,
    // ---- rolling cultivator share ----
    max_rolling_cultivator_share: f64,
    final_cultivator_share: f64,
    settled_band: bool,
    // ---- rolling non-cultivating NON-OWNER material buyers ----
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

    /// The non-vacuity precondition IN THIS RUN (spec §7): the lever engaged — ≥1 tool built from
    /// consumed WOOD AND ≥1 owner entered the retention signal. `!lever_bites` ⇒ `CapitalLeverInert`
    /// for this seed (capital never formed, so capitalized agents cannot have out-produced).
    fn lever_bites(&self) -> bool {
        self.tools_built > 0 && self.tool_wood_consumed > 0 && self.owner_in_signal_ever
    }

    /// The tool-stock accounting invariant: cumulative produced − destroyed = live whole-system
    /// stock (no decay; the build deposits immediately, so no completed-but-undeposited in-flight).
    fn tool_stock_balances(&self) -> bool {
        // Assert non-negativity FIRST (Codex review-of-results P3): a saturating_sub would let a
        // hypothetical future `destroyed > built` with zero stock pass silently.
        self.tools_destroyed <= self.tools_built
            && self.tools_built - self.tools_destroyed == self.tool_stock_total
    }

    /// The monopolization DAMAGE side: the non-lineage / material-buyer demand side has fallen
    /// below the survival/buyer floor.
    fn demand_damaged(&self) -> bool {
        self.living_non_lineage_final <= DEMAND_COLLAPSE_FLOOR
            || self.final_buyer_cohort < MIN_BUYER_COHORT
    }

    /// A persistent OWNER cohort: `PERSIST_COHORT` distinct persistent cultivators that ARE
    /// tool-owners, of which at least `PERSIST_COHORT_NONLINEAGE` are non-lineage (the capitalized
    /// food-producing class self-formed — the sticky ids ARE the owners, not a coincidental
    /// rotation).
    fn has_owner_cohort(&self) -> bool {
        self.persistent_cohort_owners >= PERSIST_COHORT
            && self.persistent_cohort_owners_nonlineage >= PERSIST_COHORT_NONLINEAGE
    }

    /// The stickiness + health conjunction, IGNORING the ownership-isolation guard (owner-share ≤
    /// max) and the productivity-only control: churn fell ≤ 0.5× the matched-seed baseline, a
    /// persistent OWNER cohort formed, money + mortality survive (a living non-owner buyer cohort
    /// persists), and the top cultivator did NOT monopolize the grain.
    fn sticky(&self, baseline_churn: f64) -> bool {
        self.churn_per_capita() <= CHURN_DROP * baseline_churn
            && self.has_owner_cohort()
            && self.promoted
            && !self.extinct
            && self.final_buyer_cohort >= MIN_BUYER_COHORT
            && self.top_cultivator_grain_share < MONO_SHARE
    }

    /// The full SUCCESS conjunction (spec §2/§7): `sticky` AND ownership is a MINORITY edge
    /// (owner-share ≤ `OWNER_SHARE_MAX`). The productivity-only-control isolation is applied in
    /// [`Self::verdict`] (it needs the control's result).
    fn is_success(&self, baseline_churn: f64) -> bool {
        self.sticky(baseline_churn) && self.owner_share_ever <= OWNER_SHARE_MAX
    }

    /// The §2 ORDERED, mutually-exclusive classifier — checked top-down; the FIRST match is the
    /// verdict. `baseline_churn` is the matched-seed S22c baseline churn;
    /// `productivity_only_sticky` / `non_durable_sticky` say whether the falsifying controls on
    /// the SAME seed cleared the churn-drop + membership-cohort bars. Every threshold is
    /// predeclared above.
    fn verdict(
        &self,
        baseline_churn: f64,
        productivity_only_sticky: bool,
        non_durable_sticky: bool,
    ) -> Verdict {
        if !self.lever_bites() {
            return Verdict::CapitalLeverInert;
        }
        if !self.conserved
            || self.extinct
            || self.bread_minted_max > 0
            || !self.provenance_clean
            || !self.tool_stock_balances()
        {
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
        // PRODUCTIVITY ONLY: the run is otherwise sticky+healthy, but the effect is not isolated to
        // durable ownership — a falsifying control reproduces it, or ownership is universal (a
        // cosmetic global boost, not a minority capital edge).
        if self.sticky(baseline_churn)
            && (productivity_only_sticky
                || non_durable_sticky
                || self.owner_share_ever > OWNER_SHARE_MAX)
        {
            return Verdict::ProductivityOnly;
        }
        if self.is_success(baseline_churn) {
            return Verdict::Success;
        }
        // Money + mortality survive, not commune / not monopolized, the tool bit, but a SUCCESS
        // criterion is unmet (churn did not fall enough, or no persistent OWNER cohort): the honest
        // S22d finding is NO STICKINESS DESPITE CAPITAL (it names the next boundary, not a failure
        // to tune). The printed figures localize the gap.
        Verdict::NoStickinessDespiteCapital
    }

    /// A uniform one-line figure rendering for the per-run classification maps.
    fn line(&self, baseline_churn: f64, po_sticky: bool, nd_sticky: bool) -> String {
        format!(
            "churn/cap={:.1} (base={:.1}; drop_to={:.1}? {}) | capital(built={} destroyed={} \
             stock={} wood_sunk={} owner_signal={} bites={}) | cohort(persist={} nl={} \
             owners={} owner_nl={}) owner_share={:.2} | grain_share={:.2} promoted={} \
             prov_clean={} | cult_share(final={:.2} max_roll={:.2} settled={}) buyers={} \
             post_promo_bought={} po_sticky={} nd_sticky={} | living={} nl_alive={} lin_alive={} \
             starv={} | conserved={} minted_max={} extinct={} tool_balances={}",
            self.churn_per_capita(),
            baseline_churn,
            CHURN_DROP * baseline_churn,
            self.churn_per_capita() <= CHURN_DROP * baseline_churn,
            self.tools_built,
            self.tools_destroyed,
            self.tool_stock_total,
            self.tool_wood_consumed,
            self.owner_in_signal_ever,
            self.lever_bites(),
            self.persistent_cohort,
            self.persistent_cohort_nonlineage,
            self.persistent_cohort_owners,
            self.persistent_cohort_owners_nonlineage,
            self.owner_share_ever,
            self.top_cultivator_grain_share,
            self.promoted,
            self.provenance_clean,
            self.final_cultivator_share,
            self.max_rolling_cultivator_share,
            self.settled_band,
            self.final_buyer_cohort,
            self.post_promotion_bought,
            po_sticky,
            nd_sticky,
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

/// How to drive each tick of [`run_metrics`] — the normal run, or the PRODUCTIVITY-ONLY control
/// that pins every colonist's cultivation skill to the cap before each tick (so every cultivator
/// draws the SAME boosted haul the tool confers — a colony-wide productivity bump with no owned
/// asset).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Drive {
    Normal,
    PinSkill,
}

/// Run `(seed, cfg)` for `ticks` under `drive` and collect the full S22d classification vector by
/// one tick-by-tick read of the public, runtime-only accessors. Entrants/churn/cohort are keyed by
/// stable `AgentId`; ownership is read through the cultivation-tool good's stock (NOT `acquired_tool`).
fn run_metrics(seed: u64, cfg: &SettlementConfig, ticks: u64, drive: Drive) -> Metrics {
    let mut s = Settlement::generate(seed, cfg);
    let bread = bread_good(&s);
    let skill_cap_pin = u16::MAX; // clamped to skill_cap inside the engine helper.

    let mut conserved = true;
    let mut bread_minted_max = 0u64;

    let mut was_cultivating: BTreeMap<u64, bool> = BTreeMap::new();
    let mut churn: BTreeMap<u64, u32> = BTreeMap::new();
    let mut ever_cultivating: BTreeSet<u64> = BTreeSet::new();
    // Distinct ids that owned a plow on any tick (the ownership-concentration numerator).
    let mut ever_owned: BTreeSet<u64> = BTreeSet::new();
    let mut owner_in_signal_ever = false;

    let mut share_samples: Vec<f64> = Vec::with_capacity(ticks as usize);
    let mut buyer_samples: Vec<usize> = Vec::with_capacity(ticks as usize);

    let final_window_start = ticks.saturating_sub(FINAL_WINDOW as u64);
    let mut final_window_cultivating: BTreeMap<u64, u32> = BTreeMap::new();
    let mut grain_at_window_start: BTreeMap<u64, u64> = BTreeMap::new();
    let mut window_start_snapshotted = false;

    let mut promoted = false;
    let mut bought_at_promotion: Option<u64> = None;

    for t in 0..ticks {
        if drive == Drive::PinSkill {
            s.set_all_cultivation_skill(skill_cap_pin);
        }
        let report = s.econ_tick();
        conserved &= report.conserves();
        bread_minted_max = bread_minted_max.max(report.endowment_of(bread));

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
        let mut owner_signal_now = 0usize;
        for i in 0..pop {
            if !s.is_alive(i) {
                continue;
            }
            alive_count += 1;
            let Some(id) = s.colonist_id(i) else { continue };
            let key = id.0;
            let lineage = s.household_of(i).is_some();
            let cultivating = s.is_cultivating(i);
            let owns = s.owns_cultivation_tool(i);
            if owns {
                ever_owned.insert(key);
            }
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
            // A material buyer is a living, NON-cultivating, NON-owner, non-lineage colonist that
            // has bought materially — the demand side the role split must leave intact.
            if !lineage && !cultivating && !owns && s.bought_food_of(i) >= MATERIAL_BUYER_FOOD {
                buyers += 1;
            }
            if owns && s.recent_cultivation_proceeds_of(i) > 0 {
                owner_signal_now += 1;
            }
        }
        if owner_signal_now > 0 {
            owner_in_signal_ever = true;
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

    let persist_threshold = (PERSIST_FRACTION * FINAL_WINDOW as f64).ceil() as u32;
    let mut persistent_cohort = 0usize;
    let mut persistent_cohort_nonlineage = 0usize;
    let mut persistent_cohort_owners = 0usize;
    let mut persistent_cohort_owners_nonlineage = 0usize;
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
        if let Some(&count) = final_window_cultivating.get(&key) {
            if count >= persist_threshold {
                persistent_cohort += 1;
                let nl = !lineage_by_id.get(&key).copied().unwrap_or(false);
                if nl {
                    persistent_cohort_nonlineage += 1;
                }
                // The sticky id is a tool-OWNER iff it ever held a plow (SUCCESS criterion 2).
                if ever_owned.contains(&key) {
                    persistent_cohort_owners += 1;
                    if nl {
                        persistent_cohort_owners_nonlineage += 1;
                    }
                }
            }
        }
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
    let owner_share_ever = if ever_cultivating.is_empty() {
        0.0
    } else {
        ever_owned.len() as f64 / ever_cultivating.len() as f64
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
        tools_built: s.cultivation_tools_built(),
        tools_destroyed: s.cultivation_tools_destroyed(),
        tool_stock_total: s.cultivation_tool_stock_total(),
        tool_wood_consumed: s.cultivation_tool_wood_consumed(),
        owner_in_signal_ever,
        churn_total: churn.values().copied().sum(),
        ever_cultivating: ever_cultivating.len(),
        persistent_cohort,
        persistent_cohort_nonlineage,
        persistent_cohort_owners,
        persistent_cohort_owners_nonlineage,
        owner_share_ever,
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

/// Run a batch of labelled `(seed, cfg, drive)` jobs concurrently (bounded scoped-thread fan-out)
/// and return the collected `Metrics` in input order. The engine has no global mutable state, so
/// concurrent `Settlement` runs are deterministic per `(seed, config)`.
fn run_batch(jobs: Vec<(u64, SettlementConfig, Drive)>) -> Vec<Metrics> {
    let workers = 8usize.min(jobs.len().max(1));
    let mut buckets: Vec<Vec<(usize, u64, SettlementConfig, Drive)>> = vec![Vec::new(); workers];
    for (i, (seed, cfg, drive)) in jobs.into_iter().enumerate() {
        buckets[i % workers].push((i, seed, cfg, drive));
    }
    let mut computed: Vec<(usize, Metrics)> = std::thread::scope(|scope| {
        let handles: Vec<_> = buckets
            .into_iter()
            .map(|bucket| {
                scope.spawn(move || {
                    bucket
                        .into_iter()
                        .map(|(i, seed, cfg, drive)| {
                            (i, run_metrics(seed, &cfg, PROBE_TICKS, drive))
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

/// The matched-seed S22c baseline churn ([`SettlementConfig::frontier_profit_retention`]) — the
/// comparison denominator the verdict uses.
fn baseline_churns(seeds: &[u64]) -> BTreeMap<u64, f64> {
    let jobs: Vec<(u64, SettlementConfig, Drive)> = seeds
        .iter()
        .map(|&seed| {
            (
                seed,
                SettlementConfig::frontier_profit_retention(),
                Drive::Normal,
            )
        })
        .collect();
    seeds
        .iter()
        .copied()
        .zip(run_batch(jobs).into_iter().map(|m| m.churn_per_capita()))
        .collect()
}

/// The falsifier-control stickiness bar: churn drops materially vs the matched baseline AND a
/// persistent cultivating MEMBERSHIP cohort forms. This deliberately does not read tool-owner
/// stock, because the non-durable/rented control can rebuild at tick end after consuming the
/// previous plow; its falsifier is the persistence of cultivating ids under a per-use boost.
fn membership_sticky(m: &Metrics, baseline_churn: f64) -> bool {
    m.churn_per_capita() <= CHURN_DROP * baseline_churn
        && m.persistent_cohort >= PERSIST_COHORT
        && m.persistent_cohort_nonlineage >= PERSIST_COHORT_NONLINEAGE
}

/// Whether the PRODUCTIVITY-ONLY control on each seed cleared the churn-drop + membership-cohort
/// bars (the §2 #6 ProductivityOnly trigger). The control gives every cultivator the same boosted
/// haul (skill pinned to cap, haul ceiling = the tool's) with NO owned asset — if it is sticky, the
/// headline's stickiness was raw output.
fn productivity_only_sticky(seeds: &[u64], baselines: &BTreeMap<u64, f64>) -> BTreeMap<u64, bool> {
    let jobs: Vec<(u64, SettlementConfig, Drive)> = seeds
        .iter()
        .map(|&seed| {
            (
                seed,
                SettlementConfig::frontier_cultivation_capital_productivity_only(),
                Drive::PinSkill,
            )
        })
        .collect();
    seeds
        .iter()
        .copied()
        .zip(run_batch(jobs))
        .map(|(seed, m)| {
            let bc = baselines.get(&seed).copied().unwrap_or(0.0);
            (seed, membership_sticky(&m, bc))
        })
        .collect()
}

/// Whether the NON-DURABLE / rented-tool control cleared the churn-drop + membership-cohort bars
/// on each seed. This is the durability falsifier: the same owner-only per-use boost is present,
/// but no persistent tool stock can accumulate, so a sticky membership cohort here means the
/// headline did not isolate durable capital.
fn non_durable_sticky(seeds: &[u64], baselines: &BTreeMap<u64, f64>) -> BTreeMap<u64, bool> {
    let jobs: Vec<(u64, SettlementConfig, Drive)> = seeds
        .iter()
        .map(|&seed| {
            (
                seed,
                SettlementConfig::frontier_cultivation_capital_non_durable(),
                Drive::Normal,
            )
        })
        .collect();
    seeds
        .iter()
        .copied()
        .zip(run_batch(jobs))
        .map(|(seed, m)| {
            let bc = baselines.get(&seed).copied().unwrap_or(0.0);
            (seed, membership_sticky(&m, bc))
        })
        .collect()
}

/// The hard guards every run must satisfy regardless of regime (spec §7): conservation each tick,
/// no bread minted, no extinction, and the tool-stock accounting invariant. (Provenance-clean is
/// handled by the verdict: `!provenance_clean` ⇒ `BrokenInvariant`, unless the lever was inert.)
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
        "[{ctx}] the tool-stock accounting invariant must hold \
         (built {} − destroyed {} == stock {}) (seed {})",
        m.tools_built,
        m.tools_destroyed,
        m.tool_stock_total,
        m.seed
    );
}

// =========================================================================
// 1. THE MANDATORY NON-VACUITY TEST (spec §7)
// =========================================================================

/// A matched-condition micro-harness: a designated cultivator is FORCED cultivating every tick in
/// two otherwise-identical runs from the same seed, with the build cost set impossibly high so NO
/// agent ever builds organically — so the ONLY difference is that in the OWNER run the designated
/// agent is GRANTED one plow (an initial endowment, conservation-safe) and in the PLAIN run it is
/// not. The plow boosts only its owner's grain haul while cultivating, so the owner must harvest
/// strictly more grain and (1:1) produce strictly more bread. Returns
/// `(grain_owner, bread_owner, grain_plain, bread_plain)`.
fn owner_vs_no_tool_matched(seed: u64) -> (u64, u64, u64, u64) {
    // Build cost impossibly high ⇒ no organic build in EITHER run, so the plain agent stays a
    // genuine no-tool cultivator (it cannot become an owner) and the owner run's only owner is the
    // granted designated agent. `carry_cap = 1` keeps cultivators GRAIN-limited (each haul is far
    // below the per-tick own-use labor budget), so the owner-only haul boost — a faster conserved-
    // node draw — translates strictly into more bread (a tool owner hauls `ceiling × carry_cap` per
    // trip vs a non-owner's `carry_cap`); at the default `carry_cap` cultivators saturate the labor
    // budget and the extra grain only accumulates, masking the boost on the bread side.
    let mut cfg = SettlementConfig::frontier_cultivation_capital();
    cfg.carry_cap = 1;
    if let Some(chain) = cfg.chain.as_mut() {
        chain.tool_build_wood = u32::MAX;
    }
    // The designated index: the agent that converts the most bread under forced cultivation in the
    // plain run (definitely eligible + with grain access). Found once on the plain run.
    let mut plain = Settlement::generate(seed, &cfg);
    let pop = plain.population();
    for _ in 0..MATCHED_HORIZON {
        for i in 0..pop {
            plain.set_cultivating_for_test(i, true);
        }
        let r = plain.econ_tick();
        assert!(
            r.conserves(),
            "matched plain run must conserve (seed {seed})"
        );
    }
    let idx = (0..pop)
        .max_by_key(|&i| plain.cultivation_bread_produced_of(i))
        .expect("a non-empty roster");
    let grain_plain = plain.cultivation_grain_harvested_of(idx);
    let bread_plain = plain.cultivation_bread_produced_of(idx);

    // The OWNER run: same seed/cfg, the designated agent granted a plow at generation (an initial
    // endowment, so conservation holds every tick), forced cultivating each tick like the plain run.
    let mut owner = Settlement::generate(seed, &cfg);
    owner.grant_cultivation_tool_for_test(idx);
    assert!(
        owner.owns_cultivation_tool(idx),
        "the designated agent must own the granted plow (seed {seed})"
    );
    for _ in 0..MATCHED_HORIZON {
        for i in 0..pop {
            owner.set_cultivating_for_test(i, true);
        }
        let r = owner.econ_tick();
        assert!(
            r.conserves(),
            "matched owner run must conserve (seed {seed})"
        );
    }
    let grain_owner = owner.cultivation_grain_harvested_of(idx);
    let bread_owner = owner.cultivation_bread_produced_of(idx);
    assert!(
        owner.owns_cultivation_tool(idx),
        "the durable plow must persist (seed {seed})"
    );
    (grain_owner, bread_owner, grain_plain, bread_plain)
}

#[test]
fn nonvacuity_capital_lever_bites() {
    eprintln!("============ S22d NON-VACUITY (capital lever bites) ============");
    // (a) The MATCHED-CONDITION mechanic: a tool-OWNER harvests strictly more grain AND produces
    // strictly more bread than a no-tool cultivator under matched forced cultivation.
    let mut matched_ok = 0usize;
    for &seed in &SEEDS {
        let (g_owner, b_owner, g_plain, b_plain) = owner_vs_no_tool_matched(seed);
        eprintln!(
            "  matched seed={seed}: grain owner={g_owner} plain={g_plain} | \
             bread owner={b_owner} plain={b_plain}"
        );
        assert!(
            g_plain > 0 && b_plain > 0,
            "the matched no-tool cultivator must actually cultivate (seed {seed})"
        );
        assert!(
            g_owner > g_plain && b_owner > b_plain,
            "CAPITAL LEVER INERT (seed {seed}): a tool owner must harvest STRICTLY MORE grain \
             ({g_owner} > {g_plain}) AND produce STRICTLY MORE bread ({b_owner} > {b_plain}) than a \
             no-tool cultivator under matched conditions"
        );
        matched_ok += 1;
    }
    assert_eq!(
        matched_ok,
        SEEDS.len(),
        "the matched mechanic must hold on every seed"
    );

    // (b) The ORGANIC engagement: across SEEDS, ≥1 headline run builds ≥1 tool from CONSUMED WOOD
    // AND ≥1 owner enters the retention signal (the full sunk-cost → owned-tool → boosted-return →
    // retention-signal lifecycle fires for real, not just in the granted micro-harness).
    let treatment = run_batch(
        SEEDS
            .iter()
            .map(|&seed| {
                (
                    seed,
                    SettlementConfig::frontier_cultivation_capital(),
                    Drive::Normal,
                )
            })
            .collect(),
    );
    let mut seeds_with_build = 0usize;
    let mut seeds_with_owner_signal = 0usize;
    for m in &treatment {
        assert_guards(m, "nonvacuity-organic");
        if m.tools_built > 0 && m.tool_wood_consumed > 0 {
            seeds_with_build += 1;
        }
        if m.owner_in_signal_ever {
            seeds_with_owner_signal += 1;
        }
        eprintln!(
            "  organic seed={:>2}: built={} wood_sunk={} owner_signal={} bites={}",
            m.seed,
            m.tools_built,
            m.tool_wood_consumed,
            m.owner_in_signal_ever,
            m.lever_bites()
        );
    }
    assert!(
        seeds_with_build >= 1,
        "CAPITAL LEVER INERT: no headline seed built a tool from consumed WOOD"
    );
    assert!(
        seeds_with_owner_signal >= 1,
        "CAPITAL LEVER INERT: no owner ever entered the retention signal"
    );
    eprintln!(
        "  NON-VACUOUS: matched owner out-produces no-tool on every seed; {seeds_with_build}/{} \
         seeds build organically; {seeds_with_owner_signal}/{} seeds put an owner in the retention \
         signal.",
        SEEDS.len(),
        SEEDS.len()
    );
    eprintln!("===============================================================");
}

// =========================================================================
// 2. The determinism / golden contract
// =========================================================================

#[test]
fn canonical_bytes_split_for_cultivation_capital() {
    // The durable-cultivation-capital gate builds an owned plow + boosts the owner's haul (a
    // future-behaviour change) AND interns the plow good, so `frontier_cultivation_capital` must
    // SPLIT the canonical digest vs the S22c base.
    let base = Settlement::generate(7, &SettlementConfig::frontier_profit_retention());
    let cap = Settlement::generate(7, &SettlementConfig::frontier_cultivation_capital());
    assert_ne!(
        base.canonical_bytes(),
        cap.canonical_bytes(),
        "the durable_cultivation_tool gate must split the canonical digest vs the S22c base"
    );
}

#[test]
fn cultivation_capital_off_the_path_is_inert() {
    // The gate composes on the S22c profit-driven-retention path (which requires the S22a entry
    // path), so toggling `durable_cultivation_tool` on a config OFF that path (no profit-retention,
    // no Cultivate recipe) must NOT split the digest — the gate is inert without the composition.
    let base = Settlement::generate(7, &SettlementConfig::frontier_forage_capacity());
    let mut base_cfg = SettlementConfig::frontier_forage_capacity();
    base_cfg
        .chain
        .as_mut()
        .expect("chain")
        .durable_cultivation_tool = true;
    assert_eq!(
        base.canonical_bytes(),
        Settlement::generate(7, &base_cfg).canonical_bytes(),
        "off the S22c path the durable_cultivation_tool flag must not steer the digest"
    );
}

#[test]
fn cultivation_capital_without_tool_content_is_inert() {
    // A caller that manually flips the public flag without also interning the plow good has not
    // configured the S22d content surface. That malformed toggle must not emit tag-10 canonical
    // bytes, accrue tenure, or silently no-op after changing the digest.
    let base_cfg = SettlementConfig::frontier_profit_retention();
    let base = Settlement::generate(7, &base_cfg);
    let mut missing_tool_cfg = base_cfg.clone();
    missing_tool_cfg
        .chain
        .as_mut()
        .expect("chain")
        .durable_cultivation_tool = true;

    let mut missing_tool = Settlement::generate(7, &missing_tool_cfg);
    missing_tool.run(500);
    assert_eq!(
        base.canonical_bytes(),
        Settlement::generate(7, &missing_tool_cfg).canonical_bytes(),
        "without the cultivation-tool content good, the durable_cultivation_tool flag must not \
         steer canonical state"
    );
    assert_eq!(
        missing_tool.cultivation_tools_built(),
        0,
        "without the cultivation-tool content good, no tool can be built"
    );
}

#[test]
fn cultivation_capital_runs_are_deterministic() {
    // Byte-identical `(seed, config)` at a long horizon (the per-agent tenure + in-flight builds
    // live in the digested state; every owner/sunk-cost diagnostic is runtime-only).
    let cfg = SettlementConfig::frontier_cultivation_capital();
    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    a.run(2_000);
    b.run(2_000);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the cultivation-capital run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.digest(), b.digest());
    let mut c = Settlement::generate(2, &cfg);
    c.run(2_000);
    assert_ne!(a.digest(), c.digest(), "the seed must change the run");
}

#[test]
fn goldens_unchanged() {
    // The S22d addition is one additive, default-off ON-only gate (tag 10) + a per-agent tenure
    // counter (born 0, serialized ON-only) + a new plow good interned ONLY on the new scenario's
    // content set + runtime-only diagnostics, so the cross-history demographic + emergence goldens
    // are BYTE-IDENTICAL (the same five values pinned in profit_driven_retention.rs).
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
fn plow_never_trades() {
    // Codex review-of-results P2: the cultivation tool is documented as "never traded" — GUARD it
    // rather than merely assert it in prose. The plow is a durable producer good no agent ever wants
    // as a consumption or medium good, so it should never be offered/cleared. Across the headline
    // scenario and every seed, the plow good's cleared trade volume must be exactly zero.
    let cfg = SettlementConfig::frontier_cultivation_capital();
    for &seed in &SEEDS {
        let mut s = Settlement::generate(seed, &cfg);
        s.run(PROBE_TICKS);
        if let Some(plow) = s.cultivation_tool_good_id() {
            assert_eq!(
                s.trade_volume_of(plow),
                0,
                "the cultivation tool (plow) must never trade for any seed (seed {seed})"
            );
        }
    }
}

// =========================================================================
// 3. The headline verdict (the ordered classifier; prints, does NOT assert SUCCESS)
// =========================================================================

#[test]
fn capital_verdict() {
    eprintln!("================ S22d DURABLE CULTIVATION CAPITAL VERDICT ================");
    let baselines = baseline_churns(&SEEDS);
    let po_sticky = productivity_only_sticky(&SEEDS, &baselines);
    let nd_sticky = non_durable_sticky(&SEEDS, &baselines);
    let treatment = run_batch(
        SEEDS
            .iter()
            .map(|&seed| {
                (
                    seed,
                    SettlementConfig::frontier_cultivation_capital(),
                    Drive::Normal,
                )
            })
            .collect(),
    );

    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    for m in &treatment {
        assert_guards(m, "verdict");
        let bc = baselines[&m.seed];
        let pos = po_sticky[&m.seed];
        let nds = nd_sticky[&m.seed];
        let v = m.verdict(bc, pos, nds);
        *tally.entry(format!("{v:?}")).or_insert(0) += 1;
        eprintln!("  seed={:>2}: {v:?} | {}", m.seed, m.line(bc, pos, nds));
        assert_ne!(
            v,
            Verdict::BrokenInvariant,
            "no run may be a broken invariant (seed {})",
            m.seed
        );
    }
    eprintln!("---- verdict tally across SEEDS={SEEDS:?}: {tally:?} ----");
    eprintln!(
        "  (PRODUCTIVITY-ONLY control sticky per seed: {po_sticky:?} — the durability isolator)"
    );
    eprintln!("  (NON-DURABLE control sticky per seed: {nd_sticky:?} — the persistence isolator)");
    eprintln!("========================================================================");
}

#[test]
fn no_stay_variant_verdict_is_capital_alone() {
    // The profit-stay-OFF variant (capital active, but the profit-stay EFFECT neutralized via an
    // impossibly-high material floor): does capital ALONE move the hunger-only exit? Expected: no.
    // Prints the regime; asserts only the guards + a valid verdict (does NOT assert any outcome).
    eprintln!("======== S22d CAPITAL-ALONE (profit-stay OFF) VARIANT VERDICT ========");
    let baselines = baseline_churns(&SEEDS);
    let treatment = run_batch(
        SEEDS
            .iter()
            .map(|&seed| {
                (
                    seed,
                    SettlementConfig::frontier_cultivation_capital_no_stay(),
                    Drive::Normal,
                )
            })
            .collect(),
    );
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    for m in &treatment {
        assert_guards(m, "no-stay-variant");
        let bc = baselines[&m.seed];
        let v = m.verdict(bc, false, false);
        *tally.entry(format!("{v:?}")).or_insert(0) += 1;
        eprintln!("  seed={:>2}: {v:?} | {}", m.seed, m.line(bc, false, false));
        assert_ne!(
            v,
            Verdict::BrokenInvariant,
            "no capital-alone run may be a broken invariant (seed {})",
            m.seed
        );
    }
    eprintln!("---- capital-alone tally across SEEDS={SEEDS:?}: {tally:?} ----");
    eprintln!("=====================================================================");
}

// =========================================================================
// 4. The controls (spec §5) — classify, never tune
// =========================================================================

#[test]
fn control_flag_off_reproduces_s22c_baseline() {
    // Control 1: the flag-off control IS `frontier_profit_retention` (the S22c baseline) — the
    // matched-seed no-capital baseline the churn-drop bar is measured against. It must run clean and
    // build NO tool (the gate is off). Asserts the guards + zero tools, and prints the churn.
    for &seed in &SEEDS {
        let m = run_metrics(
            seed,
            &SettlementConfig::frontier_profit_retention(),
            PROBE_TICKS,
            Drive::Normal,
        );
        assert_guards(&m, "flag-off-baseline");
        assert_eq!(
            m.tools_built, 0,
            "the flag-off S22c baseline must never build a cultivation tool (seed {seed})"
        );
        eprintln!(
            "  [flag-off=S22c] seed={seed}: churn/cap={:.1} living={} nl_alive={}",
            m.churn_per_capita(),
            m.living,
            m.living_non_lineage_final
        );
    }
}

#[test]
fn control_productivity_only_must_not_be_capital_success() {
    // Productivity-only control (spec §5, the key isolator): every cultivator gets the SAME boosted
    // haul the tool confers (skill pinned to cap, ceiling = the tool's) with NO owned/durable asset.
    // It must NOT be a capital success — no tool is ever built (no buildable asset) and there is no
    // owner cohort, so `lever_bites()` is false (the verdict is `CapitalLeverInert` BY CONSTRUCTION
    // for the capital classifier). The point: if this colony-wide bump produces the SAME churn-drop
    // + cohort the headline does, the headline's stickiness was productivity, not capital — surfaced
    // by `productivity_only_sticky` feeding the headline `ProductivityOnly` verdict. Here we assert
    // it builds no tool and print whether it clears the stickiness bars.
    let baselines = baseline_churns(&SEEDS);
    for &seed in &SEEDS {
        let m = run_metrics(
            seed,
            &SettlementConfig::frontier_cultivation_capital_productivity_only(),
            PROBE_TICKS,
            Drive::PinSkill,
        );
        assert_guards(&m, "productivity-only");
        assert_eq!(
            m.tools_built, 0,
            "the productivity-only control owns no buildable asset (seed {seed})"
        );
        let bc = baselines[&seed];
        let sticky = membership_sticky(&m, bc);
        assert!(
            !sticky,
            "the productivity-only control must not produce stickiness (seed {seed})"
        );
        eprintln!(
            "  [productivity-only] seed={seed}: churn/cap={:.1} (base={:.1}) persist={} nl={} \
             clears_stickiness_bars={sticky} living={}",
            m.churn_per_capita(),
            bc,
            m.persistent_cohort,
            m.persistent_cohort_nonlineage,
            m.living
        );
    }
}

#[test]
fn control_non_durable_must_not_produce_stickiness() {
    // Non-durable / rented control (spec §5, the durability isolator): the same owner-only boost,
    // but the plow is CONSUMED after one cultivation opportunity (no persistent stock). It must NOT
    // produce the churn-drop + persistent cultivating membership cohort. Asserts the guards (incl.
    // the tool-stock accounting invariant with destruction) + that tools are actually destroyed
    // whenever any are built, and prints the membership-cohort signal.
    let baselines = baseline_churns(&SEEDS);
    for &seed in &SEEDS {
        let m = run_metrics(
            seed,
            &SettlementConfig::frontier_cultivation_capital_non_durable(),
            PROBE_TICKS,
            Drive::Normal,
        );
        assert_guards(&m, "non-durable");
        if m.tools_built > 0 {
            assert!(
                m.tools_destroyed > 0,
                "the rented tool must be consumed after use (seed {seed})"
            );
        }
        let bc = baselines[&seed];
        let sticky = membership_sticky(&m, bc);
        assert!(
            !sticky,
            "the non-durable/rented control must not produce stickiness (seed {seed})"
        );
        eprintln!(
            "  [non-durable] seed={seed}: built={} destroyed={} stock={} persist={} nl={} \
             clears_stickiness_bars={sticky} churn/cap={:.1} (base={:.1}) living={}",
            m.tools_built,
            m.tools_destroyed,
            m.tool_stock_total,
            m.persistent_cohort,
            m.persistent_cohort_nonlineage,
            m.churn_per_capita(),
            bc,
            m.living
        );
    }
}

#[test]
fn control_zero_build_input_reproduces_s22c() {
    // Zero-build-input control (spec §5): the build cost set impossibly high (WOOD starved) ⇒ NO
    // tool is ever built ⇒ the cultivation path is exactly S22c. Must build no tool and so be
    // `CapitalLeverInert` (the lever never engaged — no fake success).
    let baselines = baseline_churns(&SEEDS);
    for &seed in &SEEDS {
        let mut cfg = SettlementConfig::frontier_cultivation_capital();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.tool_build_wood = u32::MAX;
        }
        let m = run_metrics(seed, &cfg, PROBE_TICKS, Drive::Normal);
        assert_guards(&m, "zero-build-input");
        assert_eq!(
            m.tools_built, 0,
            "WOOD-starved build cost must build no tool (seed {seed})"
        );
        let bc = baselines[&seed];
        assert_eq!(
            m.verdict(bc, false, false),
            Verdict::CapitalLeverInert,
            "with no tool ever built the lever must be inert (seed {seed})"
        );
        eprintln!(
            "  [zero-build-input] seed={seed}: CapitalLeverInert | built={} churn/cap={:.1}",
            m.tools_built,
            m.churn_per_capita()
        );
    }
}

#[test]
fn sensitivity_free_tool_is_excluded_from_core_verdict() {
    // Free-tool SENSITIVITY (spec §5, NOT the headline): `tool_build_wood = 0` (no sunk cost) ⇒
    // ownership tends to UNIVERSAL among ever-cultivators ⇒ a cosmetic global productivity boost,
    // not a minority capital edge. Classified SENSITIVITY (excluded from the core verdict); when it
    // is otherwise sticky the verdict is `ProductivityOnly` (owner-share > max). Asserts only the
    // guards and prints the regime.
    let baselines = baseline_churns(&SEEDS);
    for &seed in &SEEDS {
        let mut cfg = SettlementConfig::frontier_cultivation_capital();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.tool_build_wood = 0;
        }
        let m = run_metrics(seed, &cfg, PROBE_TICKS, Drive::Normal);
        assert_guards(&m, "free-tool-sensitivity");
        let bc = baselines[&seed];
        eprintln!(
            "  [free-tool SENSITIVITY wood=0] seed={seed}: {:?} | owner_share={:.2} | {}",
            m.verdict(bc, false, false),
            m.owner_share_ever,
            m.line(bc, false, false)
        );
    }
}

// =========================================================================
// 5. The robustness mini-sweep (build cost / haul ceiling + grain flow) — classify, never tune
// =========================================================================

#[test]
fn robustness_sweep_over_build_cost_ceiling_and_grain() {
    // Sweep tool_build_wood / cultivation_tool_haul_ceiling / tool_build_patience + grain flow (1-D
    // axes, holding the others at the shipped value), classify each cell across two seeds, and PRINT
    // the regime map. No tuning to pass: inert / no-stickiness / productivity-only / commune /
    // monopolization outcomes are first-class findings. Bounded to two seeds per cell (stated, not
    // silent). HARD guards on every cell.
    const SWEEP_SEEDS: [u64; 2] = [3, 11];
    eprintln!("============ S22d ROBUSTNESS MINI-SWEEP (regime map) ============");
    eprintln!("  bounded to SWEEP_SEEDS={SWEEP_SEEDS:?} per cell (stated, not silent).");
    let baselines = baseline_churns(&SWEEP_SEEDS);

    struct Axis {
        name: &'static str,
        cells: Vec<(String, SettlementConfig)>,
    }
    let with_capital = |f: &dyn Fn(&mut sim::ChainConfig)| {
        let mut cfg = SettlementConfig::frontier_cultivation_capital();
        if let Some(chain) = cfg.chain.as_mut() {
            f(chain);
        }
        cfg
    };
    let mut axes: Vec<Axis> = Vec::new();

    // tool_build_wood (shipped 1; higher = scarcer ownership).
    {
        let cells = [0u32, 1, 4, 16]
            .into_iter()
            .map(|v| {
                (
                    format!("tool_build_wood={v}"),
                    with_capital(&move |c| c.tool_build_wood = v),
                )
            })
            .collect();
        axes.push(Axis {
            name: "tool_build_wood",
            cells,
        });
    }
    // cultivation_tool_haul_ceiling (shipped 3).
    {
        let cells = [1u32, 2, 3, 6]
            .into_iter()
            .map(|v| {
                (
                    format!("haul_ceiling={v}"),
                    with_capital(&move |c| c.cultivation_tool_haul_ceiling = v),
                )
            })
            .collect();
        axes.push(Axis {
            name: "haul_ceiling",
            cells,
        });
    }
    // tool_build_patience (shipped 12).
    {
        let cells = [3u16, 6, 12, 48]
            .into_iter()
            .map(|v| {
                (
                    format!("build_patience={v}"),
                    with_capital(&move |c| c.tool_build_patience = v),
                )
            })
            .collect();
        axes.push(Axis {
            name: "build_patience",
            cells,
        });
    }
    // grain node regen (the recurring-supply axis).
    {
        let mut cells = Vec::new();
        for v in [1u32, 4, 16, 64] {
            let mut cfg = SettlementConfig::frontier_cultivation_capital();
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
        let jobs: Vec<(u64, SettlementConfig, Drive)> = axis
            .cells
            .iter()
            .flat_map(|(_, cfg)| {
                SWEEP_SEEDS
                    .iter()
                    .map(move |&seed| (seed, cfg.clone(), Drive::Normal))
            })
            .collect();
        let results = run_batch(jobs);
        for (cell_idx, (label, _)) in axis.cells.iter().enumerate() {
            for (k, &seed) in SWEEP_SEEDS.iter().enumerate() {
                let m = &results[cell_idx * SWEEP_SEEDS.len() + k];
                assert_guards(m, axis.name);
                let bc = baselines[&seed];
                eprintln!(
                    "  {label} seed={seed}: {:?} | {}",
                    m.verdict(bc, false, false),
                    m.line(bc, false, false)
                );
            }
        }
    }
    eprintln!("================================================================");
}
