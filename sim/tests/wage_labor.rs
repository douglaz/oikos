//! C1 acceptance suite: wage labor over the S23e marginal rival-commons base.
//! The headline verdict is printed and classified, not tuned into success.

use std::collections::{BTreeMap, BTreeSet};

use econ::good::{Gold, SALT};
use sim::{
    Settlement, SettlementConfig, WageLaborMode, WageLaborStats,
    RIVAL_COMMONS_BASELINE_FINAL_WINDOW_TICKS, RIVAL_COMMONS_PHI_ABUNDANT_BPS,
    RIVAL_COMMONS_PHI_MARGINAL_BPS, RIVAL_COMMONS_PHI_SCARCE_BPS,
};

#[path = "support/mod.rs"]
mod support;
use support::living;

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
const FINAL_WINDOW: u64 = RIVAL_COMMONS_BASELINE_FINAL_WINDOW_TICKS;
/// The final window is split into this many equal sub-windows; the velocity floor is the
/// MINIMUM per-sub-window trade value, so a single spike cannot satisfy the sustained-velocity
/// clause (§2/§7: "≥ V_MIN sustained across the WHOLE final window, not one spike").
const VELOCITY_SUBWINDOWS: u64 = 4;
const MIN_WAGE_HIRES: u64 = 1;
const MIN_WAGE_BUY_SHARE_BPS: u64 = 1_000;
/// Per-sub-window velocity floor (see [`VELOCITY_SUBWINDOWS`]).
const MIN_FINAL_VELOCITY: u64 = 1;
const MIN_LOOP_TURNOVERS: u64 = 1;
const MIN_BUYER_BOUGHT: u64 = 1;
/// §2 clause 3 / §7: the S23d mortality+inheritance base precondition must hold for a
/// `CircularFlowForms` positive (inherited threshold, mirrors
/// `mortal_landowner_demography::INHERIT_ELIGIBLE_OWNER_DEATHS_MIN`). A future positive must
/// not be declared without confirming the generational base actually turned over.
const MIN_INHERIT_ELIGIBLE_OWNER_DEATHS: u64 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScenarioMode {
    NoWageOffered,
    Voluntary,
    FiatWage,
    SubsidisedWage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    BaseUnviable,
    ConservationBroken,
    EscrowUnbalanced,
    ProvenanceDisqualified,
    WageScaffoldOnly,
    WageMarketVacuous,
    CircularFlowForms,
    WageInertDemandStillDead,
    SubsistenceBoundDespiteScarcity,
}

#[derive(Clone, Debug)]
struct Metrics {
    seed: u64,
    mode: ScenarioMode,
    conserved: bool,
    registry_ok: bool,
    commons_ok: bool,
    money_ok: bool,
    escrow_balanced: bool,
    extinct: bool,
    promoted: bool,
    money_persisted: bool,
    seeded_minted: u64,
    final_buyer_bought: u64,
    final_velocity: u64,
    final_velocity_floor: u64,
    final_wage_financed_buy_share_bps: u64,
    final_loop_turnovers: u64,
    final_hires: u64,
    inherit_eligible_owner_deaths: u64,
    stats: WageLaborStats,
}

impl Metrics {
    fn verdict(&self) -> Verdict {
        // §2 ordering: the disqualifying base-precondition failures are checked FIRST, ahead of
        // scaffold mode — a forced/subsidised control that also minted bread or went extinct is
        // the disqualifying base failure, not a scaffold pass.
        if !self.conserved || !self.registry_ok || !self.commons_ok || !self.money_ok {
            return Verdict::ConservationBroken;
        }
        if !self.escrow_balanced {
            return Verdict::EscrowUnbalanced;
        }
        if self.seeded_minted > 0 {
            return Verdict::ProvenanceDisqualified;
        }
        if self.extinct || !self.promoted || !self.money_persisted {
            return Verdict::BaseUnviable;
        }
        // Scaffold mode is keyed on the scenario FLAG and checked before voluntary vacuity
        // (spec-review round 4, §2): a forced/subsidised control legitimately carries
        // endowment-funded wages, so it must classify here — never as vacuous, and never as
        // provenance-disqualified for those by-design endowment wages.
        if matches!(
            self.mode,
            ScenarioMode::FiatWage | ScenarioMode::SubsidisedWage
        ) {
            return Verdict::WageScaffoldOnly;
        }
        // Voluntary headline only: an endowment-funded wage means the fund was not real earnings
        // (the anti-subsidy provenance guard, §4.6). Placed after the scaffold check so it can
        // only fire on the own-earnings-funded headline run.
        if self.stats.endowment_funded_wages > Gold::ZERO {
            return Verdict::ProvenanceDisqualified;
        }
        if self.mode == ScenarioMode::NoWageOffered {
            return Verdict::SubsistenceBoundDespiteScarcity;
        }
        if self.final_hires < MIN_WAGE_HIRES {
            return Verdict::WageMarketVacuous;
        }
        // Velocity uses the per-sub-window FLOOR (`final_velocity_floor`), not the window sum, so
        // one spike cannot pass the sustained-circulation clause (§7). The S23d mortality+
        // inheritance base precondition (§2 clause 3) is a required conjunct: a positive cannot be
        // declared without the generational base actually turning over.
        if self.final_wage_financed_buy_share_bps >= MIN_WAGE_BUY_SHARE_BPS
            && self.final_velocity_floor >= MIN_FINAL_VELOCITY
            && self.final_loop_turnovers >= MIN_LOOP_TURNOVERS
            && self.final_buyer_bought >= MIN_BUYER_BOUGHT
            && self.inherit_eligible_owner_deaths >= MIN_INHERIT_ELIGIBLE_OWNER_DEATHS
        {
            return Verdict::CircularFlowForms;
        }
        Verdict::WageInertDemandStillDead
    }

    fn line(&self) -> String {
        format!(
            "C1 seed={} mode={:?} verdict={:?} conserved={} registry={} commons={} money={} \
             escrow_balanced={} promoted={} money_persisted={} seeded_minted={} final_hires={} \
             final_wage_buy_share_bps={} final_velocity={} final_velocity_floor={} final_loops={} \
             final_buyer_bought={} inherit_eligible_owner_deaths={} escrow_gold={} open_escrows={} \
             retained={} wage_bucket={} endowment_wages={} total_hires={} workers={} employers={} \
             below_ask={}",
            self.seed,
            self.mode,
            self.verdict(),
            self.conserved,
            self.registry_ok,
            self.commons_ok,
            self.money_ok,
            self.escrow_balanced,
            self.promoted,
            self.money_persisted,
            self.seeded_minted,
            self.final_hires,
            self.final_wage_financed_buy_share_bps,
            self.final_velocity,
            self.final_velocity_floor,
            self.final_loop_turnovers,
            self.final_buyer_bought,
            self.inherit_eligible_owner_deaths,
            self.stats.escrow_gold.0,
            self.stats.open_escrows,
            self.stats.retained_earnings_total.0,
            self.stats.wage_proceeds_bucket_total.0,
            self.stats.endowment_funded_wages.0,
            self.stats.hires_total,
            self.stats.distinct_workers,
            self.stats.distinct_employers,
            self.stats.below_ask_not_hired,
        )
    }
}

fn base_config() -> SettlementConfig {
    SettlementConfig::frontier_mortal_landowner_demography()
}

fn marginal_config(mode: ScenarioMode) -> SettlementConfig {
    scenario_config(mode, RIVAL_COMMONS_PHI_MARGINAL_BPS)
}

fn scenario_config(mode: ScenarioMode, phi_bps: u32) -> SettlementConfig {
    let mut cfg = base_config();
    let chain = cfg.chain.as_mut().expect("C1 base carries a chain");
    chain.rival_subsistence_commons = true;
    chain.rival_subsistence_commons_phi_bps = phi_bps;
    match mode {
        ScenarioMode::NoWageOffered => {
            chain.wage_labor = false;
            chain.wage_labor_mode = WageLaborMode::Voluntary;
        }
        ScenarioMode::Voluntary => {
            chain.wage_labor = true;
            chain.wage_labor_mode = WageLaborMode::Voluntary;
        }
        ScenarioMode::FiatWage => {
            chain.wage_labor = true;
            chain.wage_labor_mode = WageLaborMode::FiatWage;
        }
        ScenarioMode::SubsidisedWage => {
            chain.wage_labor = true;
            chain.wage_labor_mode = WageLaborMode::SubsidisedWage;
        }
    }
    cfg
}

fn owner_ids(s: &Settlement) -> BTreeSet<u64> {
    s.private_land_owner_ids().into_iter().collect()
}

fn id_to_index(s: &Settlement) -> BTreeMap<u64, usize> {
    (0..s.population())
        .filter_map(|i| s.colonist_id(i).map(|id| (id.0, i)))
        .collect()
}

fn bought_by_id(s: &Settlement) -> BTreeMap<u64, u64> {
    (0..s.population())
        .filter_map(|i| s.colonist_id(i).map(|id| (id.0, s.bought_food_of(i))))
        .collect()
}

fn run_metrics(seed: u64, mode: ScenarioMode) -> Metrics {
    run_metrics_phi(seed, mode, RIVAL_COMMONS_PHI_MARGINAL_BPS)
}

fn run_metrics_phi(seed: u64, mode: ScenarioMode, phi_bps: u32) -> Metrics {
    let cfg = scenario_config(mode, phi_bps);
    let mut s = Settlement::generate(seed, &cfg);
    let initial_plot_count = s.private_land_plot_count();
    let final_start = RUN_TICKS.saturating_sub(FINAL_WINDOW);
    let subwindow_len = (FINAL_WINDOW / VELOCITY_SUBWINDOWS).max(1);
    let mut conserved = true;
    let mut registry_ok = true;
    let mut commons_ok = true;
    let mut money_ok = true;
    let mut final_bought_start = BTreeMap::new();
    let mut final_stats_start = WageLaborStats::default();
    let mut final_velocity = 0u64;
    let mut subwindow_velocity = vec![0u64; VELOCITY_SUBWINDOWS as usize];

    for tick in 0..RUN_TICKS {
        if tick == final_start {
            final_bought_start = bought_by_id(&s);
            final_stats_start = s.wage_labor_stats();
        }
        let trade_start = s.society().trades.len();
        let report = s.econ_tick();
        conserved &= report.conserves();
        registry_ok &= s.private_land_registry_invariant_holds()
            && s.private_land_plot_count() == initial_plot_count;
        commons_ok &= report.subsistence_commons_conserves();
        money_ok &= report.money_conserves();
        if tick >= final_start {
            let value: u64 = s.society().trades[trade_start..]
                .iter()
                .map(|trade| trade.price.0.saturating_mul(u64::from(trade.qty)))
                .sum();
            final_velocity = final_velocity.saturating_add(value);
            let sub = ((tick - final_start) / subwindow_len).min(VELOCITY_SUBWINDOWS - 1) as usize;
            subwindow_velocity[sub] = subwindow_velocity[sub].saturating_add(value);
        }
    }

    // §5 accounting-period close: settle any escrow opened on the final tick (its release lag
    // falls past the horizon) so a live market's last-tick hire is not misreported as
    // `EscrowUnbalanced`. Conserved (refund → employer gold); the money invariant already held
    // every tick above.
    s.settle_open_wage_escrows_at_horizon();
    // The velocity floor is the leanest sub-window: a single spike leaves some sub-window empty.
    let final_velocity_floor = subwindow_velocity.iter().copied().min().unwrap_or(0);

    let owners = owner_ids(&s);
    let final_bought = bought_by_id(&s);
    let final_buyer_bought = id_to_index(&s)
        .into_iter()
        .filter(|(id, i)| s.is_alive(*i) && !owners.contains(id))
        .map(|(id, _)| {
            final_bought.get(&id).copied().unwrap_or(0)
                - final_bought_start.get(&id).copied().unwrap_or(0)
        })
        .sum();
    let stats = s.wage_labor_stats();
    let final_wage_buys = stats
        .wage_financed_output_buys
        .saturating_sub(final_stats_start.wage_financed_output_buys)
        .0;
    let final_nonowner_buys = stats
        .nonowner_output_buys
        .saturating_sub(final_stats_start.nonowner_output_buys)
        .0;
    let final_wage_financed_buy_share_bps = final_wage_buys
        .saturating_mul(10_000)
        .checked_div(final_nonowner_buys)
        .unwrap_or(0);
    let (pre_promo_self_produced, pre_promo_minted) =
        s.pre_promotion_bread_for_salt_by_provenance();
    let _ = pre_promo_self_produced;

    Metrics {
        seed,
        mode,
        conserved,
        registry_ok,
        commons_ok,
        money_ok,
        escrow_balanced: s.wage_labor_escrow_balanced(),
        extinct: living(&s) == 0,
        promoted: s.promoted_at_tick().is_some(),
        money_persisted: s.current_money_good() == Some(SALT),
        seeded_minted: pre_promo_minted + s.seeded_minted_bread_sold_for_salt(),
        final_buyer_bought,
        final_velocity,
        final_velocity_floor,
        final_wage_financed_buy_share_bps,
        final_loop_turnovers: stats
            .circular_loop_turnovers
            .saturating_sub(final_stats_start.circular_loop_turnovers),
        final_hires: stats
            .hires_post_promotion
            .saturating_sub(final_stats_start.hires_post_promotion),
        inherit_eligible_owner_deaths: s.secure_land_inherit_eligible_owner_deaths_total(),
        stats,
    }
}

#[test]
fn precondition_wage_off_reproduces_marginal_null() {
    for seed in SEEDS {
        let metrics = run_metrics(seed, ScenarioMode::NoWageOffered);
        eprintln!("{}", metrics.line());
        assert_eq!(metrics.verdict(), Verdict::SubsistenceBoundDespiteScarcity);
        assert_eq!(
            metrics.final_buyer_bought, 0,
            "wage_labor OFF must reproduce the S23e marginal buyer null for seed {seed}"
        );
        assert_eq!(metrics.stats.hires_total, 0);
    }
}

#[test]
fn verdict_prints_without_asserting_success() {
    for seed in SEEDS {
        let no_wage = run_metrics(seed, ScenarioMode::NoWageOffered);
        let voluntary = run_metrics(seed, ScenarioMode::Voluntary);
        eprintln!("{}", no_wage.line());
        eprintln!("{}", voluntary.line());
        assert_eq!(no_wage.verdict(), Verdict::SubsistenceBoundDespiteScarcity);
        assert_ne!(voluntary.verdict(), Verdict::ConservationBroken);
        assert_ne!(voluntary.verdict(), Verdict::EscrowUnbalanced);
        if voluntary.final_hires == 0 {
            assert_eq!(voluntary.verdict(), Verdict::WageMarketVacuous);
        }
    }
}

#[test]
fn scaffold_controls_classify_before_voluntary_vacuity() {
    for mode in [ScenarioMode::FiatWage, ScenarioMode::SubsidisedWage] {
        let metrics = run_metrics(SEEDS[0], mode);
        eprintln!("{}", metrics.line());
        assert_eq!(metrics.verdict(), Verdict::WageScaffoldOnly);
    }
}

/// Whether a run would clear the full `CircularFlowForms` headline under the given (possibly
/// swept) thresholds. Only a clean, promoted, non-scaffold VOLUNTARY run can qualify — the same
/// gating [`Metrics::verdict`] applies, with each success threshold made a parameter so the sweep
/// can vary one axis at a time.
#[allow(clippy::too_many_arguments)]
fn headline_success(
    m: &Metrics,
    hires: u64,
    share_bps: u64,
    velocity: u64,
    loops: u64,
    buyer_bought: u64,
) -> bool {
    m.mode == ScenarioMode::Voluntary
        && m.conserved
        && m.registry_ok
        && m.commons_ok
        && m.money_ok
        && m.escrow_balanced
        && m.seeded_minted == 0
        && m.stats.endowment_funded_wages == Gold::ZERO
        && !m.extinct
        && m.promoted
        && m.money_persisted
        && m.final_hires >= hires
        && m.final_wage_financed_buy_share_bps >= share_bps
        && m.final_velocity_floor >= velocity
        && m.final_loop_turnovers >= loops
        && m.final_buyer_bought >= buyer_bought
        && m.inherit_eligible_owner_deaths >= MIN_INHERIT_ELIGIBLE_OWNER_DEATHS
}

/// §7 / Risk-5: report the φ sweep {scarce 0.25, marginal 0.5 (headline), abundant 1.25} and, on
/// the marginal cell, the per-axis threshold sweep, so a positive can't be a single tuned value
/// and a negative can't be a failure-to-search. Printed under `--nocapture`; nothing asserted —
/// the finding is classified, not forced.
#[test]
fn phi_and_threshold_sweep_reported() {
    let phis = [
        ("scarce", RIVAL_COMMONS_PHI_SCARCE_BPS),
        ("marginal", RIVAL_COMMONS_PHI_MARGINAL_BPS),
        ("abundant", RIVAL_COMMONS_PHI_ABUNDANT_BPS),
    ];
    let mut marginal_metrics = Vec::new();
    for (label, phi) in phis {
        let mut circular = 0usize;
        for seed in SEEDS {
            let metrics = run_metrics_phi(seed, ScenarioMode::Voluntary, phi);
            eprintln!("C1 phi={label} phi_bps={phi} {}", metrics.line());
            if metrics.verdict() == Verdict::CircularFlowForms {
                circular += 1;
            }
            if phi == RIVAL_COMMONS_PHI_MARGINAL_BPS {
                marginal_metrics.push(metrics);
            }
        }
        eprintln!(
            "C1 phi_sweep phi={label} circular_flow_forms={circular}/{}",
            SEEDS.len()
        );
    }

    // Marginal-cell threshold sweep: vary one success axis at a time (others at their headline
    // default) and report how many seeds would classify `CircularFlowForms`.
    let seeds = marginal_metrics.len();
    for hires in [MIN_WAGE_HIRES, 2, 4] {
        let count = marginal_metrics
            .iter()
            .filter(|m| {
                headline_success(
                    m,
                    hires,
                    MIN_WAGE_BUY_SHARE_BPS,
                    MIN_FINAL_VELOCITY,
                    MIN_LOOP_TURNOVERS,
                    MIN_BUYER_BOUGHT,
                )
            })
            .count();
        eprintln!("C1 sweep min_hires>={hires}: {count}/{seeds} circular_flow_forms");
    }
    for share in [500u64, MIN_WAGE_BUY_SHARE_BPS, 2_500] {
        let count = marginal_metrics
            .iter()
            .filter(|m| {
                headline_success(
                    m,
                    MIN_WAGE_HIRES,
                    share,
                    MIN_FINAL_VELOCITY,
                    MIN_LOOP_TURNOVERS,
                    MIN_BUYER_BOUGHT,
                )
            })
            .count();
        eprintln!("C1 sweep min_buy_share_bps>={share}: {count}/{seeds} circular_flow_forms");
    }
    for velocity in [MIN_FINAL_VELOCITY, 100, 1_000] {
        let count = marginal_metrics
            .iter()
            .filter(|m| {
                headline_success(
                    m,
                    MIN_WAGE_HIRES,
                    MIN_WAGE_BUY_SHARE_BPS,
                    velocity,
                    MIN_LOOP_TURNOVERS,
                    MIN_BUYER_BOUGHT,
                )
            })
            .count();
        eprintln!("C1 sweep min_velocity_floor>={velocity}: {count}/{seeds} circular_flow_forms");
    }
    for loops in [MIN_LOOP_TURNOVERS, 3, 10] {
        let count = marginal_metrics
            .iter()
            .filter(|m| {
                headline_success(
                    m,
                    MIN_WAGE_HIRES,
                    MIN_WAGE_BUY_SHARE_BPS,
                    MIN_FINAL_VELOCITY,
                    loops,
                    MIN_BUYER_BOUGHT,
                )
            })
            .count();
        eprintln!("C1 sweep min_loop_turnovers>={loops}: {count}/{seeds} circular_flow_forms");
    }
}

#[test]
fn canonical_bytes_split_only_when_wage_labor_active() {
    let off = Settlement::generate(7, &marginal_config(ScenarioMode::NoWageOffered));
    let on = Settlement::generate(7, &marginal_config(ScenarioMode::Voluntary));
    assert_ne!(off.canonical_bytes(), on.canonical_bytes());

    let mut inert = base_config();
    inert
        .chain
        .as_mut()
        .expect("C1 base carries a chain")
        .wage_labor = true;
    let active_off = Settlement::generate(7, &base_config());
    let inert_on = Settlement::generate(7, &inert);
    assert_eq!(
        active_off.canonical_bytes(),
        inert_on.canonical_bytes(),
        "wage_labor must be inert off the S23e substrate"
    );
}

#[test]
fn goldens_unchanged() {
    let digest = |cfg: &SettlementConfig, ticks: u64| {
        let mut s = Settlement::generate(1, cfg);
        s.run(ticks);
        s.digest()
    };
    assert_eq!(
        digest(&SettlementConfig::lineages(), 300),
        0x2335e13c809749fc
    );
    assert_eq!(
        digest(&SettlementConfig::lineages(), 800),
        0x3ffd78e50842d934
    );
    assert_eq!(
        digest(&SettlementConfig::frontier(), 300),
        0xcc83bf2669f0980d
    );
    assert_eq!(
        digest(&SettlementConfig::frontier_spatial_households(), 300),
        0xf30e3ce9345a73b3
    );
    let mut viable = Settlement::generate(0xC0FFEE, &SettlementConfig::viable());
    viable.run(60);
    assert_eq!(viable.digest(), 0xa1748567db1c4341);

    let base = Settlement::generate(7, &marginal_config(ScenarioMode::NoWageOffered));
    let mut explicit_off = marginal_config(ScenarioMode::NoWageOffered);
    explicit_off
        .chain
        .as_mut()
        .expect("C1 base carries a chain")
        .wage_labor = false;
    let explicit_off = Settlement::generate(7, &explicit_off);
    assert_eq!(base.digest(), explicit_off.digest());
}
