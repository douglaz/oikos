//! S22e acceptance suite — **endowed + inherited cultivation capital** (impl-37): can capital given
//! UP FRONT and passed DOWN A LINEAGE finally stabilize an occupation that earned capital (S22d)
//! could not?
//!
//! The role-topology arc is a clean four-step negative: hunger discovers the role (S22a, fluid),
//! accumulated skill doesn't change the hunger-gated exit (S22b), a realized profit stay-incentive
//! retains only marginally (S22c), and even sunk EARNED owned capital concentrates in a dominant few
//! rather than a class (S22d) — because the lock-in asset can only be earned by already sustaining
//! the fluid role (the chicken-and-egg). S22e side-steps the chicken-and-egg: a default-off gate (a)
//! ENDOWS a minority of lineage households with a durable plow at generation (a conservation-safe
//! initial endowment, no earning required) and (b) gates whether plows are INHERITED — the engine
//! already routes a dead colonist's plow to the household heir via `settle_estate_to_heirs`, so the
//! genuinely new lever is a SWITCH (inheritance ON keeps plows on that heir path; inheritance OFF
//! forces plows to the commons, the falsifying control). Everything else reuses S22d unchanged: the
//! owner-exclusive haul boost, and stickiness arising ONLY through the unmodified S22c profit-stay.
//!
//! **Crucial honesty (the main trap):** a SUCCESS here is institutional / endowment / dynastic
//! sufficiency, NOT endogenous occupation, and NOT a non-lineage occupational class (S22a–d already
//! showed that does not self-form). The honest claim is *"durable, endowed, inheritable capital can
//! stabilize a cultivator LINEAGE that earned capital could not."* A dedicated `EndowmentOnlyScaffold`
//! mode + the no-inheritance / no-endowment / productivity-only / too-many-tools controls guard
//! against merely re-pinning the S21 producer class via a static seed.
//!
//! The bar `PERSIST_COHORT = 4` owner LINEAGES is impossible on the 2-household base, so the
//! headline ([`SettlementConfig::frontier_endowed_capital`]) and ALL matched baselines/controls run
//! on an EXPANDED roster of `ROSTER_HOUSEHOLDS` (8) lineage households (proportionally expanded so
//! money + mortality still hold). The matched churn baseline is this expanded scenario with the gate
//! OFF ([`SettlementConfig::frontier_endowed_capital_expanded_base`]), NOT the original 2-household
//! frontier.
//!
//! This suite CLASSIFIES the treatment against the §2 pre-named outcomes via an ORDERED,
//! mutually-exclusive classifier (top-down; first match wins) and PRINTS the verdict; it does NOT
//! assert SUCCESS. Every threshold is PREDECLARED a priori, never fitted. The primary metric is
//! `LineageStickySuccess` (persistent owner LINEAGES + an inherited-tool heir in the final-window
//! sticky cohort, i.e. post-founder-death); id-stickiness is a REPORTED secondary. Every run
//! satisfies the hard guards (conservation each tick, `bread_minted_max == 0`,
//! provenance-clean-or-disqualified, `!extinct`, the §3.5 tool-stock invariant
//! `endowed + built − destroyed == stock_total`). The five tripwire goldens are re-pinned
//! byte-identical ([`goldens_unchanged`]).
//!
//! Run the verdict with `--nocapture` to read the classification:
//!   `cargo test -p sim --test endowed_inherited_capital endowed_verdict -- --nocapture`

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

/// Treatment per-ever-cultivating-agent churn must fall to ≤ `CHURN_DROP ×` the **matched-seed**
/// gate-off expanded baseline churn for "churn fell materially".
const CHURN_DROP: f64 = 0.5;

/// A lineage/id is PERSISTENT if it cultivates in ≥ `PERSIST_FRACTION` of the final-window ticks
/// (membership persistence, NOT aggregate share).
const PERSIST_FRACTION: f64 = 0.5;

/// Distinct persistent OWNER LINEAGES required for a sticky cohort (the primary metric).
const PERSIST_COHORT: usize = 4;

/// The lineage roster the headline + every matched control runs on (mirrors the engine const
/// `ENDOWED_ROSTER_HOUSEHOLDS`). The 2-household base cannot host a `PERSIST_COHORT` owner-lineage
/// cohort, so the expanded roster is the only interpretable base.
const ROSTER_HOUSEHOLDS: usize = 8;

/// The headline endowment count (mirrors the engine const `ENDOWED_TOOL_COUNT_DEFAULT`). Coherence
/// (asserted in [`coherence_constants_are_well_formed`]): `PERSIST_COHORT ≤ ENDOWED_TOOL_COUNT` (the
/// cohort is reachable) AND `2 × ENDOWED_TOOL_COUNT ≤ ROSTER_HOUSEHOLDS` (ownership stays a minority
/// edge).
const ENDOWED_TOOL_COUNT: u16 = 4;

/// Owner-class final-window grain share at/above which the owner class DOMINATES the grain regen
/// (the inherited-monopoly probe; a buyer collapse is also required).
const MONO_SHARE: f64 = 0.75;

/// Rolling cultivator share at/above which most survivors cultivate (the commune side).
const COMMUNE_SHARE: f64 = 0.75;

/// Living non-lineage roles at/below which the demand side has collapsed (the monopolization DAMAGE
/// floor).
const DEMAND_COLLAPSE_FLOOR: usize = 4;

/// The maximum owner-LINEAGE share for ownership to count as a *minority* edge (above ⇒
/// `UniversalOwnership` — a topology, not an occupation).
const OWNER_SHARE_MAX: f64 = 0.6;

/// The rolling window (ticks) for the cultivator-share / material-buyer samples.
const ROLLING_WINDOW: usize = 100;

/// The final window (ticks) over which the cohort / grain-share / buyer band are read. Founders die
/// of old age within the first ~36 ticks, so this window (ticks `PROBE_TICKS−FINAL_WINDOW`..) is
/// many generations POST-founder-death; inherited-tool heirs are tracked explicitly by the engine
/// rather than inferred from late-window plow ownership.
const FINAL_WINDOW: usize = 200;

/// A settled cultivator-share band's maximum range over the final window (above ⇒ not settled).
const BAND_WIDTH: f64 = 0.25;

/// Per-agent cumulative bought bread at/above which a non-cultivating non-lineage colonist counts as
/// a material BUYER (a real market transactor, not merely alive).
const MATERIAL_BUYER_FOOD: u64 = 4;

/// Living non-cultivating material buyers required in the final window for a genuine two-cohort
/// split (a non-cultivating, non-owner demand side that is alive AND buying).
const MIN_BUYER_COHORT: usize = 2;

/// The micro-harness horizon for the matched-condition non-vacuity comparison.
const MATCHED_HORIZON: u64 = 400;

// =========================================================================
// The ordered, mutually-exclusive classifier (spec §2)
// =========================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    /// (1) The endowment lever did not engage in this run — no household was endowed, or no owner
    /// entered the retention signal. A distinct outcome from "no stickiness".
    EndowmentLeverInert,
    /// (2) A guard failed (conservation / `bread_minted_max>0` / `!extinct` / the §3.5 tool-stock
    /// invariant) OR provenance is DISQUALIFIED — ConservationBroken.
    ConservationBroken,
    /// (3) The owner class takes ≥ `MONO_SHARE` of final-window grain AND the non-lineage /
    /// material-buyer side collapses (a dynasty that starves the market).
    InheritedMonopoly,
    /// (4) Rolling cultivator share ≥ `COMMUNE_SHARE` AND post-promotion bought below the floor —
    /// most survivors cultivate and the market dissolves.
    CommuneCollapse,
    /// (5) Money never forms / fails at horizon despite produced+sold `SelfProduced` bread.
    MoneyFailureFromLockIn,
    /// (6) Owner-LINEAGE share > `OWNER_SHARE_MAX` (ownership is not a minority; e.g. the
    /// too-many-tools control): topology, not an occupation.
    UniversalOwnership,
    /// (7) The productivity-only control (same boost to all, no owned/endowed asset) ALSO clears the
    /// stickiness bars — the effect is raw productivity, not capital.
    ProductivityOnly,
    /// (8) SUCCESS-like BUT the no-inheritance control ALSO clears over the post-death window (the
    /// persistence is the static initial seed acting as a one-generation pin, not inheritance
    /// stabilizing a lineage), OR the buyer cohort does not materially buy (a re-pinned dead market).
    EndowmentOnlyScaffold,
    /// (9) FINAL GATE — all eight success clauses (§2.1–§2.8) hold: churn ≤ 0.5× baseline, a
    /// persistent owner-LINEAGE cohort that is the capitalized one, inheritance load-bearing (an
    /// inherited-tool heir in the final-window cohort), a surviving non-owner buyer cohort that
    /// materially buys, money + provenance + conservation survive, ownership a minority, and NOT
    /// downgraded by the controls.
    LineageStickySuccess,
    /// (9, else) The lever bites and nothing above matched, but a success clause is unmet — even
    /// capital given up front and inherited did not retain.
    NoStickinessDespiteEndowment,
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
    endowed_tools_total: u64,
    tools_built: u64,
    tools_destroyed: u64,
    tool_stock_total: u64,
    owner_in_signal_ever: bool,
    // ---- inheritance (the decisive S22e lever) ----
    inherited_total: u64,
    inheritor_count: usize,
    inheritor_cultivated: bool,
    inheritance_load_bearing: bool,
    // ---- churn (per ever-cultivating agent; same measure as the matched baseline) ----
    churn_total: u32,
    ever_cultivating: usize,
    // ---- persistent owner-LINEAGE cohort (the primary metric) ----
    persistent_owner_lineage_cohort: usize,
    cultivating_lineages: usize,
    // ---- ownership concentration ----
    owner_lineage_share: f64,
    owner_id_share: f64,
    // ---- secondary id-stickiness (reported, never the primary gate) ----
    persistent_id_cohort: usize,
    persistent_id_cohort_owners: usize,
    // ---- owner-class grain share (final window, the monopolization probe) ----
    owner_class_grain_share: f64,
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

    /// The per-run non-vacuity precondition (§2): the lever engaged — a household was endowed AND ≥1
    /// owner entered the retention signal. `!lever_bites` ⇒ `EndowmentLeverInert` for this seed.
    fn lever_bites(&self) -> bool {
        self.endowed_tools_total > 0 && self.owner_in_signal_ever
    }

    /// The §3.5 tool-stock accounting invariant: `endowed + built − destroyed == live whole-system
    /// stock`, asserting NON-NEGATIVITY FIRST (no in-flight term; inheritance moves a plow between
    /// holders and never changes the total).
    fn tool_stock_balances(&self) -> bool {
        self.tools_destroyed <= self.endowed_tools_total + self.tools_built
            && self.endowed_tools_total + self.tools_built - self.tools_destroyed
                == self.tool_stock_total
    }

    /// The monopolization DAMAGE side: the non-lineage / material-buyer demand side has fallen below
    /// the survival/buyer floor.
    fn demand_damaged(&self) -> bool {
        self.living_non_lineage_final <= DEMAND_COLLAPSE_FLOOR
            || self.final_buyer_cohort < MIN_BUYER_COHORT
    }

    /// A surviving non-owner buyer cohort that MATERIALLY buys (success clause 4): post-promotion
    /// bought clears the material floor AND a living buyer cohort persists.
    fn buyers_materially_buy(&self) -> bool {
        self.post_promotion_bought >= MATERIAL_BOUGHT_FLOOR
            && self.final_buyer_cohort >= MIN_BUYER_COHORT
    }

    /// The stickiness + health conjunction, IGNORING the ownership-isolation guard and the controls:
    /// churn fell ≤ 0.5× baseline, a persistent owner-LINEAGE cohort formed, money survives, a
    /// living non-owner buyer cohort persists, and the owner class did NOT monopolize the grain.
    fn sticky(&self, baseline_churn: f64) -> bool {
        self.churn_per_capita() <= CHURN_DROP * baseline_churn
            && self.persistent_owner_lineage_cohort >= PERSIST_COHORT
            && self.promoted
            && !self.extinct
            && self.final_buyer_cohort >= MIN_BUYER_COHORT
            && self.owner_class_grain_share < MONO_SHARE
    }

    /// All eight success clauses (§2.1–§2.8) bar the control downgrades (which are applied in
    /// [`Self::verdict`]): sticky + inheritance load-bearing + buyers materially buy + provenance
    /// clean + the tool-stock invariant + ownership a minority.
    fn all_success_clauses(&self, baseline_churn: f64) -> bool {
        self.sticky(baseline_churn)
            && self.inheritance_load_bearing
            && self.buyers_materially_buy()
            && self.provenance_clean
            && self.tool_stock_balances()
            && self.owner_lineage_share <= OWNER_SHARE_MAX
    }

    /// The §2 ORDERED, mutually-exclusive classifier — checked top-down; the FIRST match is the
    /// verdict. `baseline_churn` is the matched-seed gate-off expanded baseline churn;
    /// `no_inheritance_sticky` / `productivity_only_sticky` say whether the falsifying controls on
    /// the SAME seed cleared the churn-drop + owner-lineage-cohort bars over the post-death window.
    fn verdict(
        &self,
        baseline_churn: f64,
        no_inheritance_sticky: bool,
        productivity_only_sticky: bool,
    ) -> Verdict {
        if !self.lever_bites() {
            return Verdict::EndowmentLeverInert;
        }
        if !self.conserved
            || self.extinct
            || self.bread_minted_max > 0
            || !self.provenance_clean
            || !self.tool_stock_balances()
        {
            return Verdict::ConservationBroken;
        }
        if self.owner_class_grain_share >= MONO_SHARE && self.demand_damaged() {
            return Verdict::InheritedMonopoly;
        }
        if self.max_rolling_cultivator_share >= COMMUNE_SHARE
            && self.post_promotion_bought < MATERIAL_BOUGHT_FLOOR
        {
            return Verdict::CommuneCollapse;
        }
        if !self.promoted {
            return Verdict::MoneyFailureFromLockIn;
        }
        if self.owner_lineage_share > OWNER_SHARE_MAX {
            return Verdict::UniversalOwnership;
        }
        if self.sticky(baseline_churn) && productivity_only_sticky {
            return Verdict::ProductivityOnly;
        }
        // EndowmentOnlyScaffold: SUCCESS-like, but the static seed alone (no-inheritance control)
        // also clears over the post-death window, OR the buyers do not materially buy (a re-pinned
        // dead market). Either way the persistence is not inheritance stabilizing a lineage.
        if self.sticky(baseline_churn) && (no_inheritance_sticky || !self.buyers_materially_buy()) {
            return Verdict::EndowmentOnlyScaffold;
        }
        if self.all_success_clauses(baseline_churn) {
            return Verdict::LineageStickySuccess;
        }
        Verdict::NoStickinessDespiteEndowment
    }

    /// A uniform one-line figure rendering for the per-run classification maps.
    fn line(&self, baseline_churn: f64, no_inh_sticky: bool, po_sticky: bool) -> String {
        format!(
            "churn/cap={:.1} (base={:.1}; drop_to={:.1}? {}) | endow(endowed={} built={} destroyed={} \
             stock={} owner_signal={} bites={}) | inherit(total={} heirs={} cult={} load_bearing={}) \
             | owner_lineage_cohort={}/{} owner_lineage_share={:.2} owner_id_share={:.2} | \
             id_cohort(persist={} owners={}) | owner_grain_share={:.2} promoted={} prov_clean={} | \
             cult_share(final={:.2} max_roll={:.2} settled={}) buyers={} post_promo_bought={} \
             no_inh_sticky={} po_sticky={} | living={} nl_alive={} lin_alive={} starv={} | \
             conserved={} minted_max={} extinct={} tool_balances={}",
            self.churn_per_capita(),
            baseline_churn,
            CHURN_DROP * baseline_churn,
            self.churn_per_capita() <= CHURN_DROP * baseline_churn,
            self.endowed_tools_total,
            self.tools_built,
            self.tools_destroyed,
            self.tool_stock_total,
            self.owner_in_signal_ever,
            self.lever_bites(),
            self.inherited_total,
            self.inheritor_count,
            self.inheritor_cultivated,
            self.inheritance_load_bearing,
            self.persistent_owner_lineage_cohort,
            self.cultivating_lineages,
            self.owner_lineage_share,
            self.owner_id_share,
            self.persistent_id_cohort,
            self.persistent_id_cohort_owners,
            self.owner_class_grain_share,
            self.promoted,
            self.provenance_clean,
            self.final_cultivator_share,
            self.max_rolling_cultivator_share,
            self.settled_band,
            self.final_buyer_cohort,
            self.post_promotion_bought,
            no_inh_sticky,
            po_sticky,
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

/// How to drive each tick of [`run_metrics`] — the normal run, or the PRODUCTIVITY-ONLY control that
/// pins every colonist's cultivation skill to the cap before each tick (so every cultivator draws
/// the SAME boosted haul the tool confers — a colony-wide productivity bump with no owned asset).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Drive {
    Normal,
    PinSkill,
}

/// Run `(seed, cfg)` for `ticks` under `drive` and collect the full S22e classification vector by
/// one tick-by-tick read of the public, runtime-only accessors. Entrants/churn/cohort are keyed by
/// stable `AgentId`; lineage cohorts by household index; ownership is read through the
/// cultivation-tool good's stock (NOT `acquired_tool`).
fn run_metrics(seed: u64, cfg: &SettlementConfig, ticks: u64, drive: Drive) -> Metrics {
    let mut s = Settlement::generate(seed, cfg);
    let bread = bread_good(&s);
    let skill_cap_pin = u16::MAX; // clamped to skill_cap inside the engine helper.

    let mut conserved = true;
    let mut bread_minted_max = 0u64;

    // id-keyed churn / participation.
    let mut was_cultivating: BTreeMap<u64, bool> = BTreeMap::new();
    let mut churn: BTreeMap<u64, u32> = BTreeMap::new();
    let mut ever_cultivating: BTreeSet<u64> = BTreeSet::new();
    let mut ever_owned_id: BTreeSet<u64> = BTreeSet::new();
    let mut owner_in_signal_ever = false;

    // lineage-keyed participation / ownership.
    let mut ever_owned_lineage: BTreeSet<usize> = BTreeSet::new();
    let mut ever_cultivating_lineage: BTreeSet<usize> = BTreeSet::new();

    // inheritance tracking: accumulate inheritor ids, and whether any inheritor was seen cultivating
    // on a tick at/after it became an inheritor (the cumulative engine set is read each tick).
    let mut inheritor_ever: BTreeSet<u64> = BTreeSet::new();
    let mut inheritor_cultivated = false;

    let mut share_samples: Vec<f64> = Vec::with_capacity(ticks as usize);
    let mut buyer_samples: Vec<usize> = Vec::with_capacity(ticks as usize);

    let final_window_start = ticks.saturating_sub(FINAL_WINDOW as u64);
    // final-window per-id cultivating-tick counts + per-household "had a cultivating member" counts.
    let mut fw_id_cultivating: BTreeMap<u64, u32> = BTreeMap::new();
    let mut fw_lineage_cultivating: BTreeMap<usize, u32> = BTreeMap::new();
    // final-window grain split by owner class (ids that ever owned a plow).
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

        // Pull the cumulative inheritor set this tick (heir ids that have received a plow).
        for id in s.cultivation_tool_inheritor_ids() {
            inheritor_ever.insert(id);
        }

        let pop = s.population();
        let mut cultivating_count = 0usize;
        let mut alive_count = 0usize;
        let mut buyers = 0usize;
        let mut owner_signal_now = 0usize;
        // per-household: did any living member cultivate THIS tick (final window only).
        let mut lineage_cultivating_now: BTreeSet<usize> = BTreeSet::new();
        for i in 0..pop {
            if !s.is_alive(i) {
                continue;
            }
            alive_count += 1;
            let Some(id) = s.colonist_id(i) else { continue };
            let key = id.0;
            let household = s.household_of(i);
            let cultivating = s.is_cultivating(i);
            let owns = s.owns_cultivation_tool(i);
            if owns {
                ever_owned_id.insert(key);
                if let Some(h) = household {
                    ever_owned_lineage.insert(h);
                }
            }
            if cultivating {
                cultivating_count += 1;
                ever_cultivating.insert(key);
                if let Some(h) = household {
                    ever_cultivating_lineage.insert(h);
                }
                if inheritor_ever.contains(&key) {
                    inheritor_cultivated = true;
                }
                if t >= final_window_start {
                    *fw_id_cultivating.entry(key).or_insert(0) += 1;
                    if let Some(h) = household {
                        lineage_cultivating_now.insert(h);
                    }
                }
            }
            let prev = was_cultivating.get(&key).copied().unwrap_or(false);
            if cultivating != prev {
                *churn.entry(key).or_insert(0) += 1;
            }
            was_cultivating.insert(key, cultivating);
            // A material buyer is a living, NON-cultivating, NON-owner, non-lineage colonist that has
            // bought materially — the demand side the role split must leave intact.
            if household.is_none()
                && !cultivating
                && !owns
                && s.bought_food_of(i) >= MATERIAL_BUYER_FOOD
            {
                buyers += 1;
            }
            if owns && s.recent_cultivation_proceeds_of(i) > 0 {
                owner_signal_now += 1;
            }
        }
        if t >= final_window_start {
            for h in lineage_cultivating_now {
                *fw_lineage_cultivating.entry(h).or_insert(0) += 1;
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

    // Persistent owner-LINEAGE cohort: distinct lineage households that (a) ever owned a plow AND (b)
    // had a living cultivating member in ≥ PERSIST_FRACTION of the final window.
    let mut persistent_owner_lineage_households: BTreeSet<usize> = BTreeSet::new();
    for (&h, &count) in &fw_lineage_cultivating {
        if count >= persist_threshold && ever_owned_lineage.contains(&h) {
            persistent_owner_lineage_households.insert(h);
        }
    }
    let persistent_owner_lineage_cohort = persistent_owner_lineage_households.len();

    // Secondary id-stickiness (reported, never the primary gate): distinct persistent cultivating
    // ids, and of those, the ones that ever owned a plow.
    let mut persistent_id_cohort = 0usize;
    let mut persistent_id_cohort_owners = 0usize;
    for (&key, &count) in &fw_id_cultivating {
        if count >= persist_threshold {
            persistent_id_cohort += 1;
            if ever_owned_id.contains(&key) {
                persistent_id_cohort_owners += 1;
            }
        }
    }

    // Inheritance load-bearing: ≥1 inherited-tool heir whose lineage is in the final-window owner
    // cohort (the lineage persisted PAST the founders via a real inherited plow). Map inheritor ids →
    // their household via the end-of-run roster (heirs are same-household members).
    let mut inheritor_households: BTreeSet<usize> = BTreeSet::new();
    let id_to_household: BTreeMap<u64, usize> = (0..s.population())
        .filter_map(|i| match (s.colonist_id(i), s.household_of(i)) {
            (Some(id), Some(h)) => Some((id.0, h)),
            _ => None,
        })
        .collect();
    for id in &inheritor_ever {
        if let Some(&h) = id_to_household.get(id) {
            inheritor_households.insert(h);
        }
    }
    let inheritance_load_bearing = inheritor_households
        .iter()
        .any(|h| persistent_owner_lineage_households.contains(h));

    // Owner-class final-window grain share: grain harvested by ids that ever owned a plow / total.
    let mut fw_grain_total = 0u64;
    let mut fw_grain_owner = 0u64;
    for i in 0..s.population() {
        let Some(id) = s.colonist_id(i) else { continue };
        let key = id.0;
        let fw_grain = s
            .cultivation_grain_harvested_of(i)
            .saturating_sub(grain_at_window_start.get(&key).copied().unwrap_or(0));
        fw_grain_total += fw_grain;
        if ever_owned_id.contains(&key) {
            fw_grain_owner += fw_grain;
        }
    }
    let owner_class_grain_share = if fw_grain_total == 0 {
        0.0
    } else {
        fw_grain_owner as f64 / fw_grain_total as f64
    };

    let owner_cultivating_lineages = ever_owned_lineage
        .intersection(&ever_cultivating_lineage)
        .count();
    let owner_cultivating_ids = ever_owned_id.intersection(&ever_cultivating).count();
    let owner_lineage_share = if ever_cultivating_lineage.is_empty() {
        0.0
    } else {
        owner_cultivating_lineages as f64 / ever_cultivating_lineage.len() as f64
    };
    let owner_id_share = if ever_cultivating.is_empty() {
        0.0
    } else {
        owner_cultivating_ids as f64 / ever_cultivating.len() as f64
    };
    debug_assert!(owner_lineage_share <= 1.0);
    debug_assert!(owner_id_share <= 1.0);

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
        endowed_tools_total: s.endowed_cultivation_tools_total(),
        tools_built: s.cultivation_tools_built(),
        tools_destroyed: s.cultivation_tools_destroyed(),
        tool_stock_total: s.cultivation_tool_stock_total(),
        owner_in_signal_ever,
        inherited_total: s.cultivation_tool_inherited_total(),
        inheritor_count: inheritor_ever.len(),
        inheritor_cultivated,
        inheritance_load_bearing,
        churn_total: churn.values().copied().sum(),
        ever_cultivating: ever_cultivating.len(),
        persistent_owner_lineage_cohort,
        cultivating_lineages: ever_cultivating_lineage.len(),
        owner_lineage_share,
        owner_id_share,
        persistent_id_cohort,
        persistent_id_cohort_owners,
        owner_class_grain_share,
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

/// The matched-seed gate-off expanded baseline churn
/// ([`SettlementConfig::frontier_endowed_capital_expanded_base`]) — the comparison denominator the
/// verdict uses.
fn baseline_churns(seeds: &[u64]) -> BTreeMap<u64, f64> {
    let jobs: Vec<(u64, SettlementConfig, Drive)> = seeds
        .iter()
        .map(|&seed| {
            (
                seed,
                SettlementConfig::frontier_endowed_capital_expanded_base(),
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
/// persistent owner-LINEAGE cohort forms over the SAME post-death final window. Used for both the
/// no-inheritance and productivity-only controls — the headline downgrades if either clears it.
fn cohort_sticky(m: &Metrics, baseline_churn: f64) -> bool {
    m.churn_per_capita() <= CHURN_DROP * baseline_churn
        && m.persistent_owner_lineage_cohort >= PERSIST_COHORT
}

/// Whether the NO-INHERITANCE control on each seed cleared the churn-drop + owner-lineage-cohort
/// bars over the post-death window (the §2 #8 `EndowmentOnlyScaffold` trigger). Plows are forced to
/// the commons on death, so if it ALSO clears, the persistence is the static seed, not inheritance.
fn no_inheritance_sticky(seeds: &[u64], baselines: &BTreeMap<u64, f64>) -> BTreeMap<u64, bool> {
    let jobs: Vec<(u64, SettlementConfig, Drive)> = seeds
        .iter()
        .map(|&seed| {
            (
                seed,
                SettlementConfig::frontier_endowed_capital_no_inheritance(),
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
            (seed, cohort_sticky(&m, bc))
        })
        .collect()
}

/// Whether the PRODUCTIVITY-ONLY control on each seed cleared the churn-drop + owner-lineage-cohort
/// bars (the §2 #7 `ProductivityOnly` trigger). Every cultivator gets the same boosted haul with NO
/// owned asset — if it is sticky, the headline's stickiness was raw output.
fn productivity_only_sticky(seeds: &[u64], baselines: &BTreeMap<u64, f64>) -> BTreeMap<u64, bool> {
    let jobs: Vec<(u64, SettlementConfig, Drive)> = seeds
        .iter()
        .map(|&seed| {
            (
                seed,
                SettlementConfig::frontier_endowed_capital_productivity_only(),
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
            (seed, cohort_sticky(&m, bc))
        })
        .collect()
}

/// The hard guards every run must satisfy regardless of regime (§2): conservation each tick, no
/// bread minted, no extinction, and the §3.5 tool-stock accounting invariant. (Provenance-clean is
/// handled by the verdict: `!provenance_clean` ⇒ `ConservationBroken`, unless the lever was inert.)
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
        "[{ctx}] the §3.5 tool-stock invariant must hold \
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
    // The cohort is reachable (≥ PERSIST_COHORT households endowed) yet ownership stays a minority
    // edge (≤ half the roster). Predeclared, never fitted.
    assert!(
        PERSIST_COHORT <= usize::from(ENDOWED_TOOL_COUNT),
        "the owner-lineage cohort floor must be reachable from the endowed count"
    );
    assert!(
        2 * usize::from(ENDOWED_TOOL_COUNT) <= ROSTER_HOUSEHOLDS,
        "the endowed count must be a minority edge (≤ half the roster)"
    );
    // The test mirrors the engine roster: the expanded base must actually carry ROSTER_HOUSEHOLDS
    // lineage households (founders + any tick-0 births), each with ≥1 founder.
    let s = Settlement::generate(7, &SettlementConfig::frontier_endowed_capital());
    let households: BTreeSet<usize> = (0..s.population())
        .filter_map(|i| s.household_of(i))
        .collect();
    assert_eq!(
        households.len(),
        ROSTER_HOUSEHOLDS,
        "the expanded endowed base must carry ROSTER_HOUSEHOLDS lineage households"
    );
    // The endowment actually fired at generation: ENDOWED_TOOL_COUNT plows granted into stock.
    assert_eq!(
        s.endowed_cultivation_tools_total(),
        u64::from(ENDOWED_TOOL_COUNT),
        "the headline must endow exactly ENDOWED_TOOL_COUNT plows at generation"
    );
    assert_eq!(
        s.endowed_household_indices().len(),
        usize::from(ENDOWED_TOOL_COUNT),
        "exactly ENDOWED_TOOL_COUNT households must be selected for endowment"
    );
}

// =========================================================================
// 1. THE PRECONDITION TEST (the expanded base must be a working, still-fluid colony)
// =========================================================================

#[test]
fn precondition_expanded_base_is_fluid_money_colony() {
    // The headline is only interpretable against a working, still-fluid base. On the EXPANDED base
    // with the gate OFF, the colony must reproduce S22d-style `NoStickiness`: money promotes,
    // mortality coexists (some starvation, not extinct), NO persistent owner-LINEAGE cohort forms,
    // and churn is HIGH. If this base can't sustain money+mortality or already shows stickiness,
    // that is a BASE PROBLEM to report (not an S22e success).
    eprintln!("============ S22e PRECONDITION (expanded gate-off base) ============");
    let base = run_batch(
        SEEDS
            .iter()
            .map(|&seed| {
                (
                    seed,
                    SettlementConfig::frontier_endowed_capital_expanded_base(),
                    Drive::Normal,
                )
            })
            .collect(),
    );
    for m in &base {
        assert_guards(m, "precondition");
        // The gate is OFF, so no household is endowed and no plow is inherited.
        assert_eq!(
            m.endowed_tools_total, 0,
            "the gate-off expanded base must endow no plow (seed {})",
            m.seed
        );
        assert!(
            m.promoted,
            "BASE PROBLEM: the expanded base must promote SALT to money (seed {})",
            m.seed
        );
        assert!(
            m.provenance_clean,
            "BASE PROBLEM: the expanded base must keep provenance clean (seed {})",
            m.seed
        );
        assert!(
            m.starvation > 0,
            "BASE PROBLEM: mortality must bind on the expanded base (seed {})",
            m.seed
        );
        assert!(
            m.persistent_owner_lineage_cohort < PERSIST_COHORT,
            "BASE PROBLEM: the gate-off base must NOT already show a persistent owner-lineage cohort \
             (seed {}: {} ≥ {})",
            m.seed,
            m.persistent_owner_lineage_cohort,
            PERSIST_COHORT
        );
        eprintln!(
            "  [precondition] seed={:>2}: promoted={} churn/cap={:.1} owner_lineage_cohort={} \
             buyers={} post_promo_bought={} living={} nl_alive={} starv={}",
            m.seed,
            m.promoted,
            m.churn_per_capita(),
            m.persistent_owner_lineage_cohort,
            m.final_buyer_cohort,
            m.post_promotion_bought,
            m.living,
            m.living_non_lineage_final,
            m.starvation,
        );
    }
    eprintln!("===================================================================");
}

// =========================================================================
// 2. THE MANDATORY NON-VACUITY TEST (spec §4)
// =========================================================================

/// A matched-condition micro-harness (reused from S22d): a designated cultivator is FORCED
/// cultivating every tick in two otherwise-identical runs from the same seed, with the build cost
/// set impossibly high so NO agent ever builds organically — so the ONLY difference is that in the
/// OWNER run the designated agent is GRANTED one plow (an initial endowment, conservation-safe) and
/// in the PLAIN run it is not. The plow boosts only its owner's grain haul while cultivating, so the
/// owner must harvest strictly more grain and (1:1) produce strictly more bread. Uses the SMALL
/// S22d base (the mechanic is the owner-haul boost the endowment confers, identical to S22d).
/// Returns `(grain_owner, bread_owner, grain_plain, bread_plain)`.
fn owner_vs_no_tool_matched(seed: u64) -> (u64, u64, u64, u64) {
    let mut cfg = SettlementConfig::frontier_cultivation_capital();
    cfg.carry_cap = 1;
    if let Some(chain) = cfg.chain.as_mut() {
        chain.tool_build_wood = u32::MAX;
    }
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
    (grain_owner, bread_owner, grain_plain, bread_plain)
}

#[test]
fn nonvacuity_endowment_lever_bites() {
    eprintln!("============ S22e NON-VACUITY (endowment lever bites) ============");
    // (a) The MATCHED-CONDITION mechanic: an endowed owner harvests strictly more grain AND produces
    // strictly more bread than a no-tool cultivator under matched forced cultivation.
    let mut matched_ok = 0usize;
    for &seed in &SEEDS {
        let (g_owner, b_owner, g_plain, b_plain) = owner_vs_no_tool_matched(seed);
        eprintln!(
            "  matched seed={seed}: grain owner={g_owner} plain={g_plain} | bread owner={b_owner} \
             plain={b_plain}"
        );
        assert!(
            g_plain > 0 && b_plain > 0,
            "the matched no-tool cultivator must actually cultivate (seed {seed})"
        );
        assert!(
            g_owner > g_plain && b_owner > b_plain,
            "ENDOWMENT LEVER INERT (seed {seed}): an endowed owner must harvest STRICTLY MORE grain \
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

    // (b)+(c) The ORGANIC engagement on the headline: across SEEDS, ≥1 REAL inheritance transfer
    // (an endowed owner dies holding a plow, a living household heir receives it — tool-stock total
    // unchanged — AND that heir subsequently cultivates) AND ≥1 owner enters the retention signal.
    let treatment = run_batch(
        SEEDS
            .iter()
            .map(|&seed| {
                (
                    seed,
                    SettlementConfig::frontier_endowed_capital(),
                    Drive::Normal,
                )
            })
            .collect(),
    );
    let mut seeds_with_inheritance = 0usize;
    let mut seeds_with_heir_cultivated = 0usize;
    let mut seeds_with_owner_signal = 0usize;
    for m in &treatment {
        assert_guards(m, "nonvacuity-organic");
        if m.inherited_total > 0 {
            seeds_with_inheritance += 1;
        }
        if m.inherited_total > 0 && m.inheritor_cultivated {
            seeds_with_heir_cultivated += 1;
        }
        if m.owner_in_signal_ever {
            seeds_with_owner_signal += 1;
        }
        eprintln!(
            "  organic seed={:>2}: endowed={} inherited={} heirs={} heir_cultivated={} \
             owner_signal={} bites={}",
            m.seed,
            m.endowed_tools_total,
            m.inherited_total,
            m.inheritor_count,
            m.inheritor_cultivated,
            m.owner_in_signal_ever,
            m.lever_bites()
        );
    }
    assert!(
        seeds_with_inheritance >= 1,
        "ENDOWMENT LEVER INERT: no headline seed saw a plow transfer to a living heir"
    );
    assert!(
        seeds_with_heir_cultivated >= 1,
        "ENDOWMENT LEVER INERT: no headline seed saw an inherited-plow heir subsequently cultivate"
    );
    assert!(
        seeds_with_owner_signal >= 1,
        "ENDOWMENT LEVER INERT: no owner ever entered the retention signal"
    );
    eprintln!(
        "  NON-VACUOUS: matched owner out-produces no-tool on every seed; \
         {seeds_with_inheritance}/{} seeds transfer a plow to a living heir; \
         {seeds_with_heir_cultivated}/{} seeds have that heir cultivate; \
         {seeds_with_owner_signal}/{} seeds put an owner in the retention signal.",
        SEEDS.len(),
        SEEDS.len(),
        SEEDS.len()
    );
    eprintln!("=================================================================");
}

// =========================================================================
// 3. The determinism / golden contract
// =========================================================================

#[test]
fn canonical_bytes_split_for_endowed_capital() {
    // The endowment gate seeds plows + gates inheritance (a future-behaviour change), so
    // `frontier_endowed_capital` must SPLIT the canonical digest vs the gate-off expanded base.
    let base = Settlement::generate(
        7,
        &SettlementConfig::frontier_endowed_capital_expanded_base(),
    );
    let endowed = Settlement::generate(7, &SettlementConfig::frontier_endowed_capital());
    assert_ne!(
        base.canonical_bytes(),
        endowed.canonical_bytes(),
        "the endowed_cultivation_capital gate must split the canonical digest vs the expanded base"
    );
    // The inheritance switch is a future-behaviour change too, so flipping it must also split.
    let no_inherit = Settlement::generate(
        7,
        &SettlementConfig::frontier_endowed_capital_no_inheritance(),
    );
    assert_ne!(
        endowed.canonical_bytes(),
        no_inherit.canonical_bytes(),
        "the cultivation_tool_inheritance switch must split the canonical digest"
    );
    let no_endowment = Settlement::generate(
        7,
        &SettlementConfig::frontier_endowed_capital_no_endowment(),
    );
    assert_eq!(
        base.canonical_bytes(),
        no_endowment.canonical_bytes(),
        "zero endowment with inheritance still on is behavior-identical to the expanded S22d base"
    );
}

#[test]
fn endowed_capital_off_the_path_is_inert() {
    // The gate composes on the durable-cultivation-capital path (which requires S22a entry + S22c
    // profit-stay + the plow content). Toggling `endowed_cultivation_capital` on a config OFF that
    // path must NOT split the digest — the gate is inert without the composition.
    let base = Settlement::generate(7, &SettlementConfig::frontier_forage_capacity());
    let mut base_cfg = SettlementConfig::frontier_forage_capacity();
    if let Some(chain) = base_cfg.chain.as_mut() {
        chain.endowed_cultivation_capital = true;
        chain.endowed_tool_count = 4;
    }
    assert_eq!(
        base.canonical_bytes(),
        Settlement::generate(7, &base_cfg).canonical_bytes(),
        "off the durable-capital path the endowed_cultivation_capital flag must not steer the digest"
    );
}

#[test]
fn endowed_capital_runs_are_deterministic() {
    let cfg = SettlementConfig::frontier_endowed_capital();
    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    a.run(1_000);
    b.run(1_000);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the endowed-capital run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.digest(), b.digest());
    let mut c = Settlement::generate(2, &cfg);
    c.run(1_000);
    assert_ne!(a.digest(), c.digest(), "the seed must change the run");
}

#[test]
fn goldens_unchanged() {
    // The S22e addition is one additive, default-off ON-only gate (tag 11) + a generation endowment
    // (placed in already-serialized agent stock) + a plow estate-routing switch (inert off the gate)
    // + runtime-only diagnostics, so the cross-history demographic + emergence goldens are
    // BYTE-IDENTICAL (the same five values pinned in durable_cultivation_capital.rs).
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
    // The cultivation tool is documented as "never traded" — GUARD it. Endowed/inherited plows are
    // durable producer goods no agent wants as a consumption or medium good, so across the headline
    // scenario and every seed the plow good's cleared trade volume must be exactly zero.
    let cfg = SettlementConfig::frontier_endowed_capital();
    for &seed in &SEEDS {
        let mut s = Settlement::generate(seed, &cfg);
        s.run(PROBE_TICKS);
        if let Some(plow) = s.cultivation_tool_good_id() {
            assert_eq!(
                s.trade_volume_of(plow),
                0,
                "the endowed/inherited plow must never trade for any seed (seed {seed})"
            );
        }
    }
}

// =========================================================================
// 4. The headline verdict (the ordered classifier; prints, does NOT assert SUCCESS)
// =========================================================================

#[test]
fn endowed_verdict() {
    eprintln!("================ S22e ENDOWED + INHERITED CAPITAL VERDICT ================");
    let baselines = baseline_churns(&SEEDS);
    let no_inh = no_inheritance_sticky(&SEEDS, &baselines);
    let po = productivity_only_sticky(&SEEDS, &baselines);
    let treatment = run_batch(
        SEEDS
            .iter()
            .map(|&seed| {
                (
                    seed,
                    SettlementConfig::frontier_endowed_capital(),
                    Drive::Normal,
                )
            })
            .collect(),
    );

    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    for m in &treatment {
        assert_guards(m, "verdict");
        let bc = baselines[&m.seed];
        let nis = no_inh[&m.seed];
        let pos = po[&m.seed];
        let v = m.verdict(bc, nis, pos);
        *tally.entry(format!("{v:?}")).or_insert(0) += 1;
        eprintln!("  seed={:>2}: {v:?} | {}", m.seed, m.line(bc, nis, pos));
        assert_ne!(
            v,
            Verdict::ConservationBroken,
            "no run may be a broken invariant (seed {})",
            m.seed
        );
    }
    eprintln!("---- PRIMARY verdict tally across SEEDS={SEEDS:?}: {tally:?} ----");
    eprintln!(
        "  (NO-INHERITANCE control cohort-sticky per seed: {no_inh:?} — the load-bearing isolator)"
    );
    eprintln!(
        "  (PRODUCTIVITY-ONLY control cohort-sticky per seed: {po:?} — the durability isolator)"
    );
    eprintln!(
        "  (SECONDARY id-stickiness is reported per-seed in the line above: \
         id_cohort/owners — never the primary gate)"
    );
    eprintln!("========================================================================");
}

// =========================================================================
// 5. The controls (spec §4) — classify, never tune
// =========================================================================

#[test]
fn control_flag_off_reproduces_no_stickiness() {
    // The flag-off control IS the expanded base (the matched churn baseline). It must run clean,
    // endow NO plow, promote, keep mortality, and show NO persistent owner-lineage cohort — the
    // still-fluid `NoStickinessDespiteCapital` regime the headline is measured against.
    for &seed in &SEEDS {
        let m = run_metrics(
            seed,
            &SettlementConfig::frontier_endowed_capital_expanded_base(),
            PROBE_TICKS,
            Drive::Normal,
        );
        assert_guards(&m, "flag-off-base");
        assert_eq!(
            m.endowed_tools_total, 0,
            "the flag-off expanded base must endow no plow (seed {seed})"
        );
        assert_eq!(
            m.inherited_total, 0,
            "the flag-off expanded base routes no S22e inheritance (seed {seed})"
        );
        assert!(
            m.persistent_owner_lineage_cohort < PERSIST_COHORT,
            "the flag-off base must not form a persistent owner-lineage cohort (seed {seed})"
        );
        eprintln!(
            "  [flag-off=base] seed={seed}: churn/cap={:.1} owner_lineage_cohort={} living={} \
             nl_alive={}",
            m.churn_per_capita(),
            m.persistent_owner_lineage_cohort,
            m.living,
            m.living_non_lineage_final
        );
    }
}

#[test]
fn control_no_inheritance_classify() {
    // No-inheritance control (the load-bearing isolator): endow on, plows FORCED to the commons on
    // death. Plows still endow at generation (the founders own them), but once the founders die the
    // capital leaves the lineage for the commons. Asserts the guards + that the gate endowed plows +
    // that NO S22e inheritance transfer occurred, and prints the post-death cohort (the verdict test
    // does the matched-seed `cohort_sticky` comparison via `no_inheritance_sticky`, so no baseline is
    // recomputed here).
    for &seed in &SEEDS {
        let m = run_metrics(
            seed,
            &SettlementConfig::frontier_endowed_capital_no_inheritance(),
            PROBE_TICKS,
            Drive::Normal,
        );
        assert_guards(&m, "no-inheritance");
        assert_eq!(
            m.endowed_tools_total,
            u64::from(ENDOWED_TOOL_COUNT),
            "the no-inheritance control still endows at generation (seed {seed})"
        );
        assert_eq!(
            m.inherited_total, 0,
            "the no-inheritance control must route NO plow to a heir (forced to commons) (seed {seed})"
        );
        eprintln!(
            "  [no-inheritance] seed={seed}: owner_lineage_cohort={} owner_lineage_share={:.2} \
             churn/cap={:.1} living={}",
            m.persistent_owner_lineage_cohort,
            m.owner_lineage_share,
            m.churn_per_capita(),
            m.living
        );
    }
}

#[test]
fn control_no_endowment_is_lever_inert() {
    // No-endowment control: endowed_tool_count = 0 (inheritance on). No household is endowed, so
    // tools must be EARNED (S22d on the expanded base) — the endowment lever never engages, so the
    // per-run verdict is `EndowmentLeverInert` (no fake success). The verdict is baseline-independent
    // here (it short-circuits on `!lever_bites`), so no baseline churn is computed.
    for &seed in &SEEDS {
        let m = run_metrics(
            seed,
            &SettlementConfig::frontier_endowed_capital_no_endowment(),
            PROBE_TICKS,
            Drive::Normal,
        );
        assert_guards(&m, "no-endowment");
        assert_eq!(
            m.endowed_tools_total, 0,
            "the no-endowment control must endow no plow (seed {seed})"
        );
        assert_eq!(
            m.verdict(0.0, false, false),
            Verdict::EndowmentLeverInert,
            "with no endowment the lever must be inert (seed {seed})"
        );
        eprintln!(
            "  [no-endowment] seed={seed}: EndowmentLeverInert | built={} churn/cap={:.1}",
            m.tools_built,
            m.churn_per_capita()
        );
    }
}

#[test]
fn control_too_many_tools_is_universal_ownership() {
    // Too-many-tools control: endowed_tool_count = ROSTER_HOUSEHOLDS (universal). Every lineage is
    // endowed, so the owner-LINEAGE share is driven above OWNER_SHARE_MAX → the classifier must
    // return `UniversalOwnership` (topology, not an occupation), never `LineageStickySuccess`.
    // `UniversalOwnership` is decided by the owner-lineage share (a stage BEFORE any baseline-using
    // stickiness check), so the verdict is baseline-independent here.
    let mut universal = 0usize;
    for &seed in &SEEDS {
        let m = run_metrics(
            seed,
            &SettlementConfig::frontier_endowed_capital_too_many_tools(),
            PROBE_TICKS,
            Drive::Normal,
        );
        assert_guards(&m, "too-many-tools");
        assert_eq!(
            m.endowed_tools_total, ROSTER_HOUSEHOLDS as u64,
            "the too-many-tools control endows the whole roster (seed {seed})"
        );
        let v = m.verdict(0.0, false, false);
        assert_ne!(
            v,
            Verdict::LineageStickySuccess,
            "universal ownership must never be a lineage success (seed {seed})"
        );
        if v == Verdict::UniversalOwnership {
            universal += 1;
        }
        eprintln!(
            "  [too-many-tools] seed={seed}: {v:?} | owner_lineage_share={:.2} owner_id_share={:.2} \
             owner_lineage_cohort={}",
            m.owner_lineage_share,
            m.owner_id_share,
            m.persistent_owner_lineage_cohort
        );
    }
    assert!(
        universal >= 1,
        "the too-many-tools control must classify UniversalOwnership on ≥1 seed (the \
         endowed_tool_count axis must be outcome-driving)"
    );
}

#[test]
fn control_productivity_only_must_not_be_capital_success() {
    // Productivity-only control (the durability isolator): every cultivator gets the SAME boosted
    // haul with NO owned/endowed/inheritable asset (skill pinned to cap). It must NOT be a lineage
    // success — no household is endowed, so `lever_bites()` is false (the verdict is
    // `EndowmentLeverInert` BY CONSTRUCTION). If it clears the headline's churn-drop + cohort bars,
    // the headline downgrades to `ProductivityOnly` (surfaced via `productivity_only_sticky` in the
    // verdict test). The per-run verdict here is baseline-independent (it short-circuits on
    // `!lever_bites`), so the cohort + churn are reported raw without a baseline comparison.
    for &seed in &SEEDS {
        let m = run_metrics(
            seed,
            &SettlementConfig::frontier_endowed_capital_productivity_only(),
            PROBE_TICKS,
            Drive::PinSkill,
        );
        assert_guards(&m, "productivity-only");
        assert_eq!(
            m.endowed_tools_total, 0,
            "the productivity-only control owns no endowed asset (seed {seed})"
        );
        assert_eq!(
            m.verdict(0.0, false, false),
            Verdict::EndowmentLeverInert,
            "the productivity-only control has no endowment, so the lever is inert (seed {seed})"
        );
        eprintln!(
            "  [productivity-only] seed={seed}: owner_lineage_cohort={} owner_lineage_share={:.2} \
             churn/cap={:.1} living={}",
            m.persistent_owner_lineage_cohort,
            m.owner_lineage_share,
            m.churn_per_capita(),
            m.living
        );
    }
}

#[test]
fn sensitivity_large_endowment_excluded_from_core_verdict() {
    // Free/large-endowment SENSITIVITY (NOT the headline): a free build cost (`tool_build_wood = 0`)
    // makes any cultivator able to ALSO build a plow on top of the endowment, tending ownership
    // toward universal. Classified SENSITIVITY (excluded from the core verdict); when it is
    // otherwise sticky the verdict tends to `UniversalOwnership`. Asserts only the guards + prints.
    // Excluded from the core verdict, so a baseline-free verdict (label only) suffices.
    for &seed in &SEEDS {
        let mut cfg = SettlementConfig::frontier_endowed_capital();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.tool_build_wood = 0;
        }
        let m = run_metrics(seed, &cfg, PROBE_TICKS, Drive::Normal);
        assert_guards(&m, "large-endowment-sensitivity");
        eprintln!(
            "  [free-build SENSITIVITY wood=0] seed={seed}: {:?} | owner_lineage_share={:.2} \
             owner_lineage_cohort={} built={} stock={} living={}",
            m.verdict(0.0, false, false),
            m.owner_lineage_share,
            m.persistent_owner_lineage_cohort,
            m.tools_built,
            m.tool_stock_total,
            m.living,
        );
    }
}

// =========================================================================
// 6. The robustness mini-sweep (endowed_tool_count / haul ceiling / grain flow) — classify, no tune
// =========================================================================

#[test]
fn robustness_sweep_over_endowment_ceiling_and_grain() {
    // Sweep endowed_tool_count (minority→universal), cultivation_tool_haul_ceiling, and grain flow
    // (1-D axes, holding the others at the shipped value), classify each cell across two seeds, and
    // PRINT the regime map. No tuning to pass: inert / no-stickiness / universal / scaffold outcomes
    // are first-class findings. Bounded to two seeds per cell (stated, not silent). HARD guards on
    // every cell. The endowed_tool_count axis MUST be outcome-driving — owner-share + verdict move
    // with it (minority → potential success; universal → UniversalOwnership).
    const SWEEP_SEEDS: [u64; 2] = [3, 11];
    eprintln!("============ S22e ROBUSTNESS MINI-SWEEP (regime map) ============");
    eprintln!("  bounded to SWEEP_SEEDS={SWEEP_SEEDS:?} per cell (stated, not silent).");
    let baselines = baseline_churns(&SWEEP_SEEDS);

    struct Axis {
        name: &'static str,
        cells: Vec<(String, SettlementConfig)>,
    }
    let with_endowed = |f: &dyn Fn(&mut sim::ChainConfig)| {
        let mut cfg = SettlementConfig::frontier_endowed_capital();
        if let Some(chain) = cfg.chain.as_mut() {
            f(chain);
        }
        cfg
    };
    let mut axes: Vec<Axis> = Vec::new();

    // endowed_tool_count (shipped 4; minority → universal). The outcome-driving axis.
    {
        let cells = [1u16, 2, 4, ROSTER_HOUSEHOLDS as u16]
            .into_iter()
            .map(|v| {
                (
                    format!("endowed_tool_count={v}"),
                    with_endowed(&move |c| c.endowed_tool_count = v),
                )
            })
            .collect();
        axes.push(Axis {
            name: "endowed_tool_count",
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
                    with_endowed(&move |c| c.cultivation_tool_haul_ceiling = v),
                )
            })
            .collect();
        axes.push(Axis {
            name: "haul_ceiling",
            cells,
        });
    }
    // grain node regen (the recurring-supply axis; scaled relative to the expanded commons).
    {
        let mut cells = Vec::new();
        for v in [24u32, 96, 192] {
            let mut cfg = SettlementConfig::frontier_endowed_capital();
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

    // Track owner-lineage share + verdict across the endowed_tool_count axis to confirm it moves.
    let mut endowment_axis_shares: Vec<(u16, f64, String)> = Vec::new();
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
                let v = m.verdict(bc, false, false);
                if axis.name == "endowed_tool_count" {
                    endowment_axis_shares.push((
                        m.endowed_tools_total as u16,
                        m.owner_lineage_share,
                        format!("{v:?}"),
                    ));
                }
                eprintln!(
                    "  {label} seed={seed}: {v:?} | {}",
                    m.line(bc, false, false)
                );
            }
        }
    }

    // The endowed_tool_count axis must be OUTCOME-DRIVING (the S21i vacuous-axis lesson): the
    // owner-lineage share at the universal end must strictly exceed the minority end.
    let min_share = endowment_axis_shares
        .iter()
        .filter(|(endowed, _, _)| *endowed <= 2)
        .map(|(_, s, _)| *s)
        .fold(f64::INFINITY, f64::min);
    let max_share = endowment_axis_shares
        .iter()
        .filter(|(endowed, _, _)| *endowed >= ROSTER_HOUSEHOLDS as u16)
        .map(|(_, s, _)| *s)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        max_share > min_share,
        "the endowed_tool_count axis must be outcome-driving: owner-lineage share at universal \
         ({max_share:.2}) must exceed the minority end ({min_share:.2})"
    );
    eprintln!(
        "  endowed_tool_count axis IS outcome-driving: owner_lineage_share minority≈{min_share:.2} \
         → universal≈{max_share:.2}"
    );
    eprintln!("================================================================");
}
