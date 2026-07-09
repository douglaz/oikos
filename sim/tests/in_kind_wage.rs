//! C1N acceptance suite: in-kind bread wages over the S23e marginal rival-commons base.
//! The headline verdict is printed and classified, not tuned into success.

use std::collections::{BTreeMap, BTreeSet};

use sim::{
    InKindWageStats, Settlement, SettlementConfig, ShareTenancyMode, ShareTenancyStats,
    WageLaborMode, WageLaborStats, RIVAL_COMMONS_BASELINE_FINAL_WINDOW_TICKS,
    RIVAL_COMMONS_PHI_ABUNDANT_BPS, RIVAL_COMMONS_PHI_MARGINAL_BPS, RIVAL_COMMONS_PHI_SCARCE_BPS,
    SHARE_TENANCY_BPS_DEFAULT, SHARE_TENANCY_TERM_DEFAULT,
};

#[path = "support/mod.rs"]
mod support;
use support::{living, living_non_lineage};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
const FINAL_WINDOW: u64 = RIVAL_COMMONS_BASELINE_FINAL_WINDOW_TICKS;
const MIN_HIRES: u64 = 1;
const MIN_SURVIVAL_LIFT: i64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScenarioMode {
    NoContract,
    InKindWage,
    ShareComparative,
    MoneyWageComparative,
    SubsidisedInKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    BaseUnviable,
    ConservationBroken,
    RegistryBroken,
    FundIsScaffold,
    InKindWageVacuous,
    InKindWageClears,
    InKindWageClearsAndLifts,
    ShareComparative,
    WageMarketVacuous,
    WageHiresObserved,
    SubsistenceBoundDespiteScarcity,
}

#[derive(Clone, Debug)]
struct Metrics {
    seed: u64,
    mode: ScenarioMode,
    phi_bps: u32,
    share_term: u16,
    conserved: bool,
    registry_ok: bool,
    commons_ok: bool,
    money_ok: bool,
    provenance_identity_ok: bool,
    extinct: bool,
    final_non_lineage: usize,
    survival_lift: i64,
    final_in_kind_hires: u64,
    final_in_kind_income: u64,
    final_in_kind_advances: u64,
    final_share_contracts: u64,
    final_wage_hires: u64,
    stats: InKindWageStats,
    share_stats: ShareTenancyStats,
    wage_stats: WageLaborStats,
}

impl Metrics {
    fn verdict(&self) -> Verdict {
        if !self.conserved || !self.commons_ok || !self.money_ok || !self.provenance_identity_ok {
            return Verdict::ConservationBroken;
        }
        if !self.registry_ok
            || self.stats.stock_drawdown > 0
            || self.stats.unattributed_deposit > 0
            || self.stats.term_starvations > 0
        {
            return Verdict::RegistryBroken;
        }
        if self.extinct {
            return Verdict::BaseUnviable;
        }
        if self.stats.endowment_funded_hires > 0 {
            return Verdict::FundIsScaffold;
        }
        match self.mode {
            ScenarioMode::NoContract => Verdict::SubsistenceBoundDespiteScarcity,
            ScenarioMode::ShareComparative => Verdict::ShareComparative,
            ScenarioMode::MoneyWageComparative => {
                if self.final_wage_hires < MIN_HIRES {
                    Verdict::WageMarketVacuous
                } else {
                    Verdict::WageHiresObserved
                }
            }
            ScenarioMode::InKindWage | ScenarioMode::SubsidisedInKind => {
                if self.final_in_kind_hires < MIN_HIRES {
                    Verdict::InKindWageVacuous
                } else if self.survival_lift >= MIN_SURVIVAL_LIFT {
                    Verdict::InKindWageClearsAndLifts
                } else {
                    Verdict::InKindWageClears
                }
            }
        }
    }

    fn line(&self) -> String {
        format!(
            "C1N seed={} mode={:?} verdict={:?} phi_bps={} term={} conserved={} \
             registry={} commons={} money={} provenance_identity={} extinct={} \
             survival_lift={} final_non_lineage={} final_hires={} total_hires={} \
             open_contracts={} distinct_workers={} distinct_employers={} \
             advance={} final_advance={} employer_income={} final_employer_income={} \
             expected_output={} worker_declined={} worker_unmatched={} owner_candidates={} \
             owner_no_atcap={} owner_insufficient_fund={} productivity_declined={} \
             reservation_collision={} stock_drawdown={} unattributed_deposit={} \
             employer_grain_settled={} endowment_funded_hires={} term_starvations={} \
             share_total={} share_final={} wage_hires_total={} wage_hires_final={}",
            self.seed,
            self.mode,
            self.verdict(),
            self.phi_bps,
            self.share_term,
            self.conserved,
            self.registry_ok,
            self.commons_ok,
            self.money_ok,
            self.provenance_identity_ok,
            self.extinct,
            self.survival_lift,
            self.final_non_lineage,
            self.final_in_kind_hires,
            self.stats.hires_total,
            self.stats.open_contracts,
            self.stats.distinct_workers,
            self.stats.distinct_employers,
            self.stats.worker_advance_bread,
            self.final_in_kind_advances,
            self.stats.employer_bread_income,
            self.final_in_kind_income,
            self.stats.expected_output_total,
            self.stats.worker_declined,
            self.stats.worker_unmatched,
            self.stats.owner_candidates_total,
            self.stats.owner_no_atcap_plot,
            self.stats.owner_insufficient_fund,
            self.stats.productivity_declined,
            self.stats.reservation_collision,
            self.stats.stock_drawdown,
            self.stats.unattributed_deposit,
            self.stats.employer_grain_settled,
            self.stats.endowment_funded_hires,
            self.stats.term_starvations,
            self.share_stats.contracts_total,
            self.final_share_contracts,
            self.wage_stats.hires_total,
            self.final_wage_hires,
        )
    }
}

fn base_config() -> SettlementConfig {
    SettlementConfig::frontier_mortal_landowner_demography()
}

fn marginal_config(mode: ScenarioMode) -> SettlementConfig {
    scenario_config(
        mode,
        RIVAL_COMMONS_PHI_MARGINAL_BPS,
        SHARE_TENANCY_TERM_DEFAULT,
    )
}

fn scenario_config(mode: ScenarioMode, phi_bps: u32, share_term: u16) -> SettlementConfig {
    let mut cfg = base_config();
    let chain = cfg.chain.as_mut().expect("C1N base carries a chain");
    chain.rival_subsistence_commons = true;
    chain.rival_subsistence_commons_phi_bps = phi_bps;
    chain.acquisition_ledger = true;
    chain.share_bps = SHARE_TENANCY_BPS_DEFAULT;
    chain.share_term = share_term;
    chain.share_forward_provisioning = false;
    chain.in_kind_wage = false;
    chain.seeded_surplus_bread = 0;
    match mode {
        ScenarioMode::NoContract => {
            chain.share_tenancy = false;
            chain.share_tenancy_mode = ShareTenancyMode::Voluntary;
            chain.wage_labor = false;
        }
        ScenarioMode::InKindWage => {
            chain.share_tenancy = true;
            chain.share_tenancy_mode = ShareTenancyMode::Voluntary;
            chain.in_kind_wage = true;
            chain.wage_labor = false;
        }
        ScenarioMode::ShareComparative => {
            chain.share_tenancy = true;
            chain.share_tenancy_mode = ShareTenancyMode::Voluntary;
            chain.wage_labor = false;
        }
        ScenarioMode::MoneyWageComparative => {
            chain.share_tenancy = false;
            chain.wage_labor = true;
            chain.wage_labor_mode = WageLaborMode::Voluntary;
        }
        ScenarioMode::SubsidisedInKind => {
            chain.share_tenancy = true;
            chain.share_tenancy_mode = ShareTenancyMode::Voluntary;
            chain.in_kind_wage = true;
            chain.wage_labor = false;
            chain.seeded_surplus_bread = 512;
        }
    }
    cfg
}

fn produced_identity_ok(s: &Settlement) -> bool {
    let (credited, sunk) = s.produced_bread_credited_and_sunk();
    credited == sunk.saturating_add(s.produced_bread_held())
}

fn run_metrics(seed: u64, mode: ScenarioMode) -> Metrics {
    run_metrics_cell(
        seed,
        mode,
        RIVAL_COMMONS_PHI_MARGINAL_BPS,
        SHARE_TENANCY_TERM_DEFAULT,
        None,
    )
}

fn run_metrics_with_baseline(
    seed: u64,
    mode: ScenarioMode,
    baseline_non_lineage: usize,
) -> Metrics {
    run_metrics_cell(
        seed,
        mode,
        RIVAL_COMMONS_PHI_MARGINAL_BPS,
        SHARE_TENANCY_TERM_DEFAULT,
        Some(baseline_non_lineage),
    )
}

fn run_metrics_cell(
    seed: u64,
    mode: ScenarioMode,
    phi_bps: u32,
    share_term: u16,
    baseline_non_lineage: Option<usize>,
) -> Metrics {
    let cfg = scenario_config(mode, phi_bps, share_term);
    let mut s = Settlement::generate(seed, &cfg);
    let initial_plot_count = s.private_land_plot_count();
    let final_start = RUN_TICKS.saturating_sub(FINAL_WINDOW);
    let mut conserved = true;
    let mut registry_ok = true;
    let mut commons_ok = true;
    let mut money_ok = true;
    let mut provenance_identity = true;
    let mut stats_start = InKindWageStats::default();
    let mut share_start = ShareTenancyStats::default();
    let mut wage_start = WageLaborStats::default();

    for tick in 0..RUN_TICKS {
        if tick == final_start {
            stats_start = s.in_kind_wage_stats();
            share_start = s.share_tenancy_stats();
            wage_start = s.wage_labor_stats();
        }
        let report = s.econ_tick();
        conserved &= report.conserves();
        registry_ok &= s.private_land_registry_invariant_holds()
            && s.private_land_plot_count() == initial_plot_count;
        commons_ok &= report.subsistence_commons_conserves();
        money_ok &= report.money_conserves();
        provenance_identity &= produced_identity_ok(&s);
    }

    let stats = s.in_kind_wage_stats();
    let share_stats = s.share_tenancy_stats();
    let wage_stats = s.wage_labor_stats();
    let final_non_lineage = living_non_lineage(&s);
    let survival_lift = baseline_non_lineage
        .map(|baseline| final_non_lineage as i64 - baseline as i64)
        .unwrap_or(0);

    Metrics {
        seed,
        mode,
        phi_bps,
        share_term,
        conserved,
        registry_ok,
        commons_ok,
        money_ok,
        provenance_identity_ok: provenance_identity,
        extinct: living(&s) == 0,
        final_non_lineage,
        survival_lift,
        final_in_kind_hires: stats.hires_total.saturating_sub(stats_start.hires_total),
        final_in_kind_income: stats
            .employer_bread_income
            .saturating_sub(stats_start.employer_bread_income),
        final_in_kind_advances: stats
            .worker_advance_bread
            .saturating_sub(stats_start.worker_advance_bread),
        final_share_contracts: share_stats
            .contracts_total
            .saturating_sub(share_start.contracts_total),
        final_wage_hires: wage_stats
            .hires_post_promotion
            .saturating_sub(wage_start.hires_post_promotion),
        stats,
        share_stats,
        wage_stats,
    }
}

fn choice_line(seed: u64, in_kind: &Metrics, share: &Metrics) -> String {
    let owner_in_kind = in_kind
        .stats
        .employer_bread_income
        .saturating_sub(in_kind.stats.worker_advance_bread);
    let worker_in_kind = in_kind.stats.worker_advance_bread;
    let share_total = share
        .share_stats
        .worker_bread_income
        .saturating_add(share.share_stats.owner_bread_income);
    let owner_share_model = share_total.saturating_mul(u64::from(
        10_000u16.saturating_sub(SHARE_TENANCY_BPS_DEFAULT),
    )) / 10_000;
    let worker_share_model =
        share_total.saturating_mul(u64::from(SHARE_TENANCY_BPS_DEFAULT)) / 10_000;
    format!(
        "C1N choice seed={seed} owner_Q_minus_W={} worker_W={} \
         share_total_Q={} owner_share_model={} worker_share_model={} \
         realized_share_owner={} realized_share_worker={}",
        owner_in_kind,
        worker_in_kind,
        share_total,
        owner_share_model,
        worker_share_model,
        share.share_stats.owner_bread_income,
        share.share_stats.worker_bread_income
    )
}

#[test]
fn precondition_no_contract_reproduces_marginal_null() {
    for seed in SEEDS {
        let metrics = run_metrics(seed, ScenarioMode::NoContract);
        eprintln!("{}", metrics.line());
        assert_eq!(metrics.verdict(), Verdict::SubsistenceBoundDespiteScarcity);
        assert_eq!(metrics.stats.hires_total, 0);
        assert_eq!(metrics.stats.open_contracts, 0);
    }
}

#[test]
fn verdict_prints_without_asserting_success() {
    for seed in SEEDS {
        let no_contract = run_metrics(seed, ScenarioMode::NoContract);
        let in_kind = run_metrics_with_baseline(
            seed,
            ScenarioMode::InKindWage,
            no_contract.final_non_lineage,
        );
        eprintln!("{}", no_contract.line());
        eprintln!("{}", in_kind.line());

        assert_ne!(in_kind.verdict(), Verdict::ConservationBroken);
        assert_ne!(in_kind.verdict(), Verdict::RegistryBroken);
        assert_eq!(in_kind.stats.endowment_funded_hires, 0);
        assert_eq!(in_kind.stats.stock_drawdown, 0);
        assert_eq!(in_kind.stats.unattributed_deposit, 0);
        assert_eq!(in_kind.stats.term_starvations, 0);

        if in_kind.final_in_kind_hires < MIN_HIRES {
            assert_eq!(in_kind.verdict(), Verdict::InKindWageVacuous);
        } else {
            assert!(matches!(
                in_kind.verdict(),
                Verdict::InKindWageClears | Verdict::InKindWageClearsAndLifts
            ));
        }
    }
}

#[test]
fn same_seed_share_and_money_comparatives_printed() {
    for seed in SEEDS {
        let no_contract = run_metrics(seed, ScenarioMode::NoContract);
        let in_kind = run_metrics_with_baseline(
            seed,
            ScenarioMode::InKindWage,
            no_contract.final_non_lineage,
        );
        let share = run_metrics_with_baseline(
            seed,
            ScenarioMode::ShareComparative,
            no_contract.final_non_lineage,
        );
        let wage = run_metrics(seed, ScenarioMode::MoneyWageComparative);
        eprintln!(
            "C1N comparative in_kind={} | share={} | wage={}",
            in_kind.line(),
            share.line(),
            wage.line()
        );
        eprintln!("{}", choice_line(seed, &in_kind, &share));

        assert_ne!(in_kind.verdict(), Verdict::ConservationBroken);
        assert_ne!(share.verdict(), Verdict::ConservationBroken);
        assert_ne!(wage.verdict(), Verdict::ConservationBroken);
        assert_eq!(in_kind.stats.endowment_funded_hires, 0);
        assert!(matches!(
            wage.verdict(),
            Verdict::WageMarketVacuous | Verdict::WageHiresObserved
        ));
        if wage.final_wage_hires < MIN_HIRES {
            assert_eq!(wage.verdict(), Verdict::WageMarketVacuous);
        }
    }
}

#[test]
fn subsidised_in_kind_control_cannot_fund_hires_from_endowment() {
    let metrics = run_metrics(SEEDS[0], ScenarioMode::SubsidisedInKind);
    eprintln!("{}", metrics.line());
    assert_ne!(metrics.verdict(), Verdict::ConservationBroken);
    assert_ne!(metrics.verdict(), Verdict::RegistryBroken);
    assert_eq!(
        metrics.stats.endowment_funded_hires, 0,
        "the advance must draw only owner self-produced bread"
    );
}

#[test]
fn phi_and_subsidy_sweeps_reported() {
    let phis = [
        ("scarce", RIVAL_COMMONS_PHI_SCARCE_BPS),
        ("marginal", RIVAL_COMMONS_PHI_MARGINAL_BPS),
        ("abundant", RIVAL_COMMONS_PHI_ABUNDANT_BPS),
    ];
    for (label, phi) in phis {
        let mut verdicts = BTreeMap::<Verdict, usize>::new();
        for seed in SEEDS {
            let base = run_metrics_cell(
                seed,
                ScenarioMode::NoContract,
                phi,
                SHARE_TENANCY_TERM_DEFAULT,
                None,
            );
            let metrics = run_metrics_cell(
                seed,
                ScenarioMode::InKindWage,
                phi,
                SHARE_TENANCY_TERM_DEFAULT,
                Some(base.final_non_lineage),
            );
            eprintln!("C1N phi={label} {}", metrics.line());
            assert_ne!(metrics.verdict(), Verdict::ConservationBroken);
            assert_ne!(metrics.verdict(), Verdict::RegistryBroken);
            assert_eq!(metrics.stats.endowment_funded_hires, 0);
            *verdicts.entry(metrics.verdict()).or_insert(0) += 1;
        }
        eprintln!("C1N phi_sweep phi={label} verdicts={verdicts:?}");
    }

    for term in [6, SHARE_TENANCY_TERM_DEFAULT, 24] {
        let mut verdicts = BTreeMap::<Verdict, usize>::new();
        for seed in SEEDS {
            let base = run_metrics_cell(
                seed,
                ScenarioMode::NoContract,
                RIVAL_COMMONS_PHI_MARGINAL_BPS,
                term,
                None,
            );
            let metrics = run_metrics_cell(
                seed,
                ScenarioMode::InKindWage,
                RIVAL_COMMONS_PHI_MARGINAL_BPS,
                term,
                Some(base.final_non_lineage),
            );
            eprintln!("C1N term_sweep term={term} {}", metrics.line());
            assert_ne!(metrics.verdict(), Verdict::ConservationBroken);
            assert_ne!(metrics.verdict(), Verdict::RegistryBroken);
            assert_eq!(metrics.stats.endowment_funded_hires, 0);
            *verdicts.entry(metrics.verdict()).or_insert(0) += 1;
        }
        eprintln!("C1N term_sweep term={term} verdicts={verdicts:?}");
    }

    let mut subsidy_verdicts = BTreeMap::<Verdict, usize>::new();
    for seed in SEEDS {
        let metrics = run_metrics(seed, ScenarioMode::SubsidisedInKind);
        eprintln!("C1N subsidised {}", metrics.line());
        assert_ne!(metrics.verdict(), Verdict::ConservationBroken);
        assert_ne!(metrics.verdict(), Verdict::RegistryBroken);
        assert_eq!(metrics.stats.endowment_funded_hires, 0);
        *subsidy_verdicts.entry(metrics.verdict()).or_insert(0) += 1;
    }
    eprintln!("C1N subsidised_sweep verdicts={subsidy_verdicts:?}");
}

#[test]
fn canonical_bytes_split_only_when_in_kind_wage_active() {
    let off = Settlement::generate(7, &marginal_config(ScenarioMode::ShareComparative));
    let on = Settlement::generate(7, &marginal_config(ScenarioMode::InKindWage));
    assert_ne!(
        off.canonical_bytes(),
        on.canonical_bytes(),
        "in_kind_wage ON must split canonical bytes under tag 25"
    );

    let mut explicit_off = marginal_config(ScenarioMode::ShareComparative);
    explicit_off
        .chain
        .as_mut()
        .expect("C1N base carries a chain")
        .in_kind_wage = false;
    assert_eq!(
        off.canonical_bytes(),
        Settlement::generate(7, &explicit_off).canonical_bytes(),
        "explicit in_kind_wage=false keeps share substrate bytes identical"
    );

    let mut inert = base_config();
    inert
        .chain
        .as_mut()
        .expect("C1N base carries a chain")
        .in_kind_wage = true;
    let active_off = Settlement::generate(7, &base_config());
    let inert_on = Settlement::generate(7, &inert);
    assert_eq!(
        active_off.canonical_bytes(),
        inert_on.canonical_bytes(),
        "in_kind_wage must be inert off the share-tenancy substrate"
    );
}

#[test]
fn goldens_unchanged_with_in_kind_wage_off() {
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

    let base = Settlement::generate(7, &marginal_config(ScenarioMode::ShareComparative));
    let mut explicit_off = marginal_config(ScenarioMode::ShareComparative);
    explicit_off
        .chain
        .as_mut()
        .expect("C1N base carries a chain")
        .in_kind_wage = false;
    assert_eq!(
        base.digest(),
        Settlement::generate(7, &explicit_off).digest()
    );
}

#[test]
fn no_plot_carries_both_share_and_in_kind_contracts() {
    for seed in SEEDS {
        let metrics = run_metrics(seed, ScenarioMode::InKindWage);
        eprintln!("{}", metrics.line());
        assert_ne!(metrics.verdict(), Verdict::RegistryBroken);
        assert_eq!(metrics.stats.reservation_collision, 0);
    }
}

#[test]
fn in_kind_workers_do_not_acquire_private_title() {
    let mut s = Settlement::generate(SEEDS[0], &marginal_config(ScenarioMode::InKindWage));
    s.run(RUN_TICKS);
    let workers: BTreeSet<u64> = s.in_kind_worker_ids().into_iter().collect();
    let owners: BTreeSet<u64> = s.private_land_owner_ids().into_iter().collect();
    assert!(
        workers.is_disjoint(&owners),
        "in-kind wage contracts must not transfer title to workers"
    );
}
