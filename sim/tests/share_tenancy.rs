//! C1R acceptance suite: output-share tenancy over the S23e marginal rival-commons base.
//! The headline verdict is printed and classified, not tuned into success.

use std::collections::{BTreeMap, BTreeSet};

use sim::{
    Settlement, SettlementConfig, ShareTenancyMode, ShareTenancyStats, WageLaborMode,
    RIVAL_COMMONS_BASELINE_FINAL_WINDOW_TICKS, RIVAL_COMMONS_PHI_ABUNDANT_BPS,
    RIVAL_COMMONS_PHI_MARGINAL_BPS, RIVAL_COMMONS_PHI_SCARCE_BPS, SHARE_TENANCY_BPS_DEFAULT,
    SHARE_TENANCY_TERM_DEFAULT,
};

#[path = "support/mod.rs"]
mod support;
use support::{living, living_non_lineage};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
const FINAL_WINDOW: u64 = RIVAL_COMMONS_BASELINE_FINAL_WINDOW_TICKS;
const MIN_CONTRACTS: u64 = 1;
const MIN_RENEWALS: u64 = 1;
const MIN_FINAL_OPEN_CONTRACTS: u64 = 1;
const MIN_SHARE_FUNDED_CONSUMPTION_BPS: u64 = 1_000;
const MIN_OWNER_GAIN: u64 = 1;
const MIN_SURVIVAL_LIFT: i64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScenarioMode {
    NoContract,
    Voluntary,
    ForwardProvisioning,
    Succession,
    SuccessionVoluntary,
    ForcedShare,
    WageComparative,
    LineageWorker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    BaseUnviable,
    ConservationBroken,
    RegistryBroken,
    ShareScaffoldOnly,
    ShareVacuous,
    ShareTenancyClears,
    ShareClearsButNoLift,
    ForwardGateInert,
    RenewalStillDeclined,
    StandingTenancyNoLift,
    StandingTenancyLifts,
    SuccessionInert,
    SuccessionButStillTransient,
    StandingTenureForms,
    StandingTenureLifts,
    /// The matched `NoContract` control: the S23e/C1 null shape (the lever is off by
    /// construction, so "share-vacuous" would mislabel it).
    SubsistenceBoundDespiteScarcity,
    /// The `WageComparative` cell prints the C1 wage classification, not the share one
    /// (review P2): the money-gap demonstration expects this verdict beside the share cell.
    WageMarketVacuous,
    /// The comparative wage cell saw final-window hires; the full circular-flow
    /// classification lives in the C1 `wage_labor` suite.
    WageHiresObserved,
}

#[derive(Clone, Debug)]
struct Metrics {
    seed: u64,
    mode: ScenarioMode,
    phi_bps: u32,
    share_bps: u16,
    share_term: u16,
    conserved: bool,
    registry_ok: bool,
    commons_ok: bool,
    money_ok: bool,
    succession_registry_ok: bool,
    anti_title_ok: bool,
    /// S23d owner-identity hard guards, accumulated per tick (spec §6 / review P2): the
    /// end-of-run disjointness check alone would miss a worker that transiently acquired
    /// title and died mid-run.
    owner_identity_ok: bool,
    immortal_owned_plot_ticks: u64,
    non_lineage_owner_plot_ticks: u64,
    owner_old_age_deaths: u64,
    extinct: bool,
    final_non_lineage: usize,
    survival_lift: i64,
    final_contracts: u64,
    final_renewals: u64,
    final_worker_share_bps: u64,
    final_worker_consumed: u64,
    final_worker_income: u64,
    final_owner_income: u64,
    final_wage_hires: u64,
    stats: ShareTenancyStats,
    worker_ids: Vec<u64>,
    commons_by_id: BTreeMap<u64, u64>,
    alive_by_id: BTreeMap<u64, bool>,
    wage_hires_post_promotion: u64,
}

impl Metrics {
    fn renewal_fate_total(&self) -> u64 {
        self.stats
            .renewal_fed_out
            .saturating_add(self.stats.renewal_base_ineligible)
            .saturating_add(self.stats.renewal_owner_not_candidate)
            .saturating_add(self.stats.renewal_bread_declined)
            .saturating_add(self.stats.renewal_matched_elsewhere)
    }

    fn renewal_fates_consistent(&self) -> bool {
        self.renewal_fate_total()
            == self
                .stats
                .renewal_hints_total
                .saturating_sub(self.stats.renewals_total)
    }

    fn succession_mark_consistent(&self) -> bool {
        self.stats.final_open_succeeded <= self.stats.open_contracts as u64
            && self.stats.post_succession_renewals <= self.stats.renewals_total
            && (self.stats.successions_total > 0
                || (self.stats.final_open_succeeded == 0
                    && self.stats.post_succession_renewals == 0))
    }

    fn verdict(&self) -> Verdict {
        if self.extinct {
            return Verdict::BaseUnviable;
        }
        if !self.conserved
            || !self.commons_ok
            || !self.money_ok
            || !self.renewal_fates_consistent()
            || !self.succession_mark_consistent()
            || self.stats.share_stock_drawdown > 0
            || self.stats.unattributed_share_deposit > 0
        {
            return Verdict::ConservationBroken;
        }
        if !self.registry_ok
            || !self.succession_registry_ok
            || !self.anti_title_ok
            || !self.owner_identity_ok
            || self.immortal_owned_plot_ticks > 0
            || self.non_lineage_owner_plot_ticks > 0
        {
            return Verdict::RegistryBroken;
        }
        if self.mode == ScenarioMode::ForcedShare {
            return Verdict::ShareScaffoldOnly;
        }
        if self.mode == ScenarioMode::WageComparative {
            // Minimal mirror of wage_labor.rs's vacuity clause (final-window hires < 1);
            // the full circular-flow classification stays in the C1 suite.
            return if self.final_wage_hires < 1 {
                Verdict::WageMarketVacuous
            } else {
                Verdict::WageHiresObserved
            };
        }
        if self.mode == ScenarioMode::NoContract {
            return Verdict::SubsistenceBoundDespiteScarcity;
        }
        if self.mode == ScenarioMode::ForwardProvisioning {
            if self.stats.forward_only_eligibility == 0 {
                return Verdict::ForwardGateInert;
            }
            if self.stats.renewals_total < MIN_RENEWALS
                || (self.stats.open_contracts as u64) < MIN_FINAL_OPEN_CONTRACTS
            {
                return Verdict::RenewalStillDeclined;
            }
            return if self.survival_lift >= MIN_SURVIVAL_LIFT {
                Verdict::StandingTenancyLifts
            } else {
                Verdict::StandingTenancyNoLift
            };
        }
        if matches!(
            self.mode,
            ScenarioMode::Succession | ScenarioMode::SuccessionVoluntary
        ) {
            if self.stats.successions_total == 0 {
                return Verdict::SuccessionInert;
            }
            if self.stats.final_open_succeeded < MIN_FINAL_OPEN_CONTRACTS
                && self.stats.post_succession_renewals < MIN_RENEWALS
            {
                return Verdict::SuccessionButStillTransient;
            }
            return if self.survival_lift >= MIN_SURVIVAL_LIFT {
                Verdict::StandingTenureLifts
            } else {
                Verdict::StandingTenureForms
            };
        }
        // Voluntary (headline) and LineageWorker (diagnostic) cells classify by the §2
        // rules. Vacuity is WHOLE-RUN (spec §2: "< MIN_CONTRACTS voluntary contracts EVER
        // clear"), not the final window: contracts that clear early then stop are the
        // ClearsButNoLift shape — a materially different finding (review P1).
        if self.stats.voluntary_contracts_total < MIN_CONTRACTS {
            return Verdict::ShareVacuous;
        }
        if self.stats.renewals_total >= MIN_RENEWALS
            && self.final_worker_share_bps >= MIN_SHARE_FUNDED_CONSUMPTION_BPS
            && self.survival_lift >= MIN_SURVIVAL_LIFT
            && self
                .stats
                .owner_bread_income
                .saturating_add(self.stats.owner_grain_settled)
                >= MIN_OWNER_GAIN
            && self.owner_old_age_deaths > 0
        {
            return Verdict::ShareTenancyClears;
        }
        Verdict::ShareClearsButNoLift
    }

    fn line(&self) -> String {
        format!(
            "C1R seed={} mode={:?} verdict={:?} phi_bps={} share_bps={} term={} \
             conserved={} registry={} commons={} money={} anti_title={} identity={} \
             immortal_owned_plot_ticks={} non_lineage_owner_plot_ticks={} \
             owner_old_age_deaths={} extinct={} \
             non_lineage={} survival_lift={} final_contracts={} final_renewals={} \
             worker_share_bps={} worker_consumed={} worker_income={} owner_income={} \
             worker_income_total={} owner_income_total={} owner_grain_settled={} \
             open_contracts={} total_contracts={} voluntary={} forced={} renewals_total={} \
             successions={} heir_declined={} worker_re_declined={} \
             post_succession_renewals={} final_open_succeeded={} succession_registry={} \
             forward_only={} renewal_hints={} fate_total={} fates_consistent={} \
             renewal_fed_out={} renewal_base_ineligible={} renewal_owner_not_candidate={} \
             renewal_bread_declined={} renewal_matched_elsewhere={} owner_candidates={} \
             distinct_workers={} distinct_owners={} worker_declined={} worker_unmatched={} \
             owner_no_atcap={} stock_refusal={} reservation_collision={} \
             share_stock_drawdown={} unattributed_share_deposit={} \
             wage_hires_final={} wage_hires_post_promotion={}",
            self.seed,
            self.mode,
            self.verdict(),
            self.phi_bps,
            self.share_bps,
            self.share_term,
            self.conserved,
            self.registry_ok,
            self.commons_ok,
            self.money_ok,
            self.anti_title_ok,
            self.owner_identity_ok,
            self.immortal_owned_plot_ticks,
            self.non_lineage_owner_plot_ticks,
            self.owner_old_age_deaths,
            self.extinct,
            self.final_non_lineage,
            self.survival_lift,
            self.final_contracts,
            self.final_renewals,
            self.final_worker_share_bps,
            self.final_worker_consumed,
            self.final_worker_income,
            self.final_owner_income,
            self.stats.worker_bread_income,
            self.stats.owner_bread_income,
            self.stats.owner_grain_settled,
            self.stats.open_contracts,
            self.stats.contracts_total,
            self.stats.voluntary_contracts_total,
            self.stats.forced_contracts_total,
            self.stats.renewals_total,
            self.stats.successions_total,
            self.stats.heir_declined,
            self.stats.worker_re_declined,
            self.stats.post_succession_renewals,
            self.stats.final_open_succeeded,
            self.succession_registry_ok,
            self.stats.forward_only_eligibility,
            self.stats.renewal_hints_total,
            self.renewal_fate_total(),
            self.renewal_fates_consistent(),
            self.stats.renewal_fed_out,
            self.stats.renewal_base_ineligible,
            self.stats.renewal_owner_not_candidate,
            self.stats.renewal_bread_declined,
            self.stats.renewal_matched_elsewhere,
            self.stats.owner_candidates_total,
            self.stats.distinct_workers,
            self.stats.distinct_owners,
            self.stats.worker_declined,
            self.stats.worker_unmatched,
            self.stats.owner_no_atcap_plot,
            self.stats.stock_opportunity_refusal,
            self.stats.reservation_collision,
            self.stats.share_stock_drawdown,
            self.stats.unattributed_share_deposit,
            self.final_wage_hires,
            self.wage_hires_post_promotion,
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
        SHARE_TENANCY_BPS_DEFAULT,
        SHARE_TENANCY_TERM_DEFAULT,
    )
}

fn scenario_config(
    mode: ScenarioMode,
    phi_bps: u32,
    share_bps: u16,
    share_term: u16,
) -> SettlementConfig {
    let mut cfg = base_config();
    let chain = cfg.chain.as_mut().expect("C1R base carries a chain");
    chain.rival_subsistence_commons = true;
    chain.rival_subsistence_commons_phi_bps = phi_bps;
    chain.acquisition_ledger = true;
    chain.share_bps = share_bps;
    chain.share_term = share_term;
    chain.share_forward_provisioning = false;
    chain.share_contract_succession = false;
    match mode {
        ScenarioMode::NoContract => {
            chain.share_tenancy = false;
            chain.share_tenancy_mode = ShareTenancyMode::Voluntary;
            chain.wage_labor = false;
        }
        ScenarioMode::Voluntary => {
            chain.share_tenancy = true;
            chain.share_tenancy_mode = ShareTenancyMode::Voluntary;
            chain.wage_labor = false;
        }
        ScenarioMode::ForwardProvisioning => {
            chain.share_tenancy = true;
            chain.share_tenancy_mode = ShareTenancyMode::Voluntary;
            chain.share_forward_provisioning = true;
            chain.wage_labor = false;
        }
        ScenarioMode::Succession => {
            chain.share_tenancy = true;
            chain.share_tenancy_mode = ShareTenancyMode::Voluntary;
            chain.share_forward_provisioning = true;
            chain.share_contract_succession = true;
            chain.wage_labor = false;
        }
        ScenarioMode::SuccessionVoluntary => {
            chain.share_tenancy = true;
            chain.share_tenancy_mode = ShareTenancyMode::Voluntary;
            chain.share_contract_succession = true;
            chain.wage_labor = false;
        }
        ScenarioMode::ForcedShare => {
            chain.share_tenancy = true;
            chain.share_tenancy_mode = ShareTenancyMode::ForcedShare;
            chain.wage_labor = false;
        }
        ScenarioMode::LineageWorker => {
            chain.share_tenancy = true;
            chain.share_tenancy_mode = ShareTenancyMode::LineageWorker;
            chain.wage_labor = false;
        }
        ScenarioMode::WageComparative => {
            chain.share_tenancy = false;
            chain.wage_labor = true;
            chain.wage_labor_mode = WageLaborMode::Voluntary;
        }
    }
    cfg
}

fn private_owner_ids(s: &Settlement) -> BTreeSet<u64> {
    s.private_land_owner_ids().into_iter().collect()
}

fn consumed_by_id(s: &Settlement) -> BTreeMap<u64, u64> {
    (0..s.population())
        .filter_map(|index| {
            s.colonist_id(index)
                .map(|id| (id.0, s.consumed_food_of(index)))
        })
        .collect()
}

fn commons_by_id(s: &Settlement) -> BTreeMap<u64, u64> {
    (0..s.population())
        .filter_map(|index| {
            s.colonist_id(index)
                .map(|id| (id.0, s.commons_food_of(index)))
        })
        .collect()
}

fn alive_by_id(s: &Settlement) -> BTreeMap<u64, bool> {
    (0..s.population())
        .filter_map(|index| s.colonist_id(index).map(|id| (id.0, s.is_alive(index))))
        .collect()
}

fn worker_consumed(
    worker_ids: &[u64],
    start: &BTreeMap<u64, u64>,
    end: &BTreeMap<u64, u64>,
) -> u64 {
    worker_ids
        .iter()
        .map(|id| {
            end.get(id)
                .copied()
                .unwrap_or(0)
                .saturating_sub(start.get(id).copied().unwrap_or(0))
        })
        .sum()
}

fn cohort_sum(cohort: &[u64], values: &BTreeMap<u64, u64>) -> u64 {
    cohort
        .iter()
        .map(|id| values.get(id).copied().unwrap_or(0))
        .sum()
}

fn dead_ids(cohort: &[u64], alive: &BTreeMap<u64, bool>) -> Vec<u64> {
    cohort
        .iter()
        .copied()
        .filter(|id| !alive.get(id).copied().unwrap_or(false))
        .collect()
}

fn substitution_line(seed: u64, voluntary: &Metrics, no_contract: &Metrics) -> String {
    let cohort = &voluntary.worker_ids;
    let voluntary_commons = cohort_sum(cohort, &voluntary.commons_by_id);
    let no_contract_commons = cohort_sum(cohort, &no_contract.commons_by_id);
    let commons_reduction = no_contract_commons.saturating_sub(voluntary_commons);
    let substitution_bps = commons_reduction
        .saturating_mul(10_000)
        .checked_div(voluntary.stats.worker_bread_income)
        .unwrap_or(0);
    let voluntary_dead = dead_ids(cohort, &voluntary.alive_by_id);
    let no_contract_dead = dead_ids(cohort, &no_contract.alive_by_id);
    format!(
        "C1R substitution seed={seed} cohort={} voluntary_commons={} \
         no_contract_commons={} commons_reduction={} share_income={} \
         substitution_bps={} dead_voluntary_ids={:?} dead_no_contract_ids={:?}",
        cohort.len(),
        voluntary_commons,
        no_contract_commons,
        commons_reduction,
        voluntary.stats.worker_bread_income,
        substitution_bps,
        voluntary_dead,
        no_contract_dead
    )
}

fn new_contracts_total(stats: &ShareTenancyStats) -> u64 {
    stats.contracts_total.saturating_sub(stats.renewals_total)
}

fn matched_volume_line(seed: u64, forward: &Metrics, forward_off: &Metrics) -> String {
    format!(
        "P1.5 matched_volume seed={seed} forward_new_contracts={} \
         forward_off_new_contracts={} forward_owner_candidates={} \
         forward_off_owner_candidates={}",
        new_contracts_total(&forward.stats),
        new_contracts_total(&forward_off.stats),
        forward.stats.owner_candidates_total,
        forward_off.stats.owner_candidates_total
    )
}

fn anti_title_ok(s: &Settlement) -> bool {
    let workers: BTreeSet<u64> = s.share_worker_ids().into_iter().collect();
    let owners = private_owner_ids(s);
    workers.is_disjoint(&owners)
}

fn plot_owner_counts(s: &Settlement) -> BTreeMap<u64, u64> {
    let mut counts = BTreeMap::new();
    for (_, owner, _, _, _, _) in s.private_land_plot_summaries() {
        if let Some(owner) = owner {
            *counts.entry(owner).or_insert(0) += 1;
        }
    }
    counts
}

/// The S23d owner-identity residue guard, accumulated per tick (the
/// mortal_landowner_demography pattern — spec §6 hard guards): every plot-owning agent
/// must be a mortal, reproduction-eligible lineage member every tick, or the immortal /
/// non-lineage tick counters accumulate and the run classifies `RegistryBroken`.
fn observe_owner_residue(s: &Settlement, immortal: &mut u64, non_lineage: &mut u64) -> bool {
    let rows: BTreeMap<u64, _> = s
        .private_land_owner_identity_rows()
        .into_iter()
        .map(|row| (row.owner, row))
        .collect();
    let mut identity_ok = true;
    for (owner, plots) in plot_owner_counts(s) {
        let Some(row) = rows.get(&owner) else {
            *non_lineage = non_lineage.saturating_add(plots);
            identity_ok = false;
            continue;
        };
        if row.lifespan.is_none() {
            *immortal = immortal.saturating_add(plots);
            identity_ok = false;
        } else if row.household.is_none()
            || row.lineage_id.is_none()
            || !row.reproduction_eligible
            || !row.in_birth_kinship_graph
        {
            *non_lineage = non_lineage.saturating_add(plots);
            identity_ok = false;
        }
    }
    identity_ok
}

fn run_metrics(seed: u64, mode: ScenarioMode) -> Metrics {
    run_metrics_cell(
        seed,
        mode,
        RIVAL_COMMONS_PHI_MARGINAL_BPS,
        SHARE_TENANCY_BPS_DEFAULT,
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
        SHARE_TENANCY_BPS_DEFAULT,
        SHARE_TENANCY_TERM_DEFAULT,
        Some(baseline_non_lineage),
    )
}

fn run_metrics_cell(
    seed: u64,
    mode: ScenarioMode,
    phi_bps: u32,
    share_bps: u16,
    share_term: u16,
    baseline_non_lineage: Option<usize>,
) -> Metrics {
    let cfg = scenario_config(mode, phi_bps, share_bps, share_term);
    let mut s = Settlement::generate(seed, &cfg);
    let initial_plot_count = s.private_land_plot_count();
    let final_start = RUN_TICKS.saturating_sub(FINAL_WINDOW);
    let mut conserved = true;
    let mut registry_ok = true;
    let mut commons_ok = true;
    let mut money_ok = true;
    let mut succession_registry_ok = true;
    let mut owner_identity_ok = true;
    let mut immortal_owned_plot_ticks = 0u64;
    let mut non_lineage_owner_plot_ticks = 0u64;
    let mut consumed_start = BTreeMap::new();
    let mut stats_start = ShareTenancyStats::default();
    let mut wage_stats_start = s.wage_labor_stats();
    let mut worker_ids_start = Vec::new();

    for tick in 0..RUN_TICKS {
        if tick == final_start {
            consumed_start = consumed_by_id(&s);
            stats_start = s.share_tenancy_stats();
            wage_stats_start = s.wage_labor_stats();
            worker_ids_start = s.share_worker_ids();
        }
        let report = s.econ_tick();
        conserved &= report.conserves();
        registry_ok &= s.private_land_registry_invariant_holds()
            && s.private_land_plot_count() == initial_plot_count;
        succession_registry_ok &= s.share_succession_registry_invariant_holds();
        commons_ok &= report.subsistence_commons_conserves();
        money_ok &= report.money_conserves();
        owner_identity_ok &= observe_owner_residue(
            &s,
            &mut immortal_owned_plot_ticks,
            &mut non_lineage_owner_plot_ticks,
        );
    }

    let stats = s.share_tenancy_stats();
    let consumed_end = consumed_by_id(&s);
    let mut worker_ids = worker_ids_start;
    for id in s.share_worker_ids() {
        if !worker_ids.contains(&id) {
            worker_ids.push(id);
        }
    }
    let final_worker_consumed = worker_consumed(&worker_ids, &consumed_start, &consumed_end);
    let final_worker_income = stats
        .worker_bread_income
        .saturating_sub(stats_start.worker_bread_income);
    let final_owner_income = stats
        .owner_bread_income
        .saturating_sub(stats_start.owner_bread_income);
    let final_worker_share_bps = final_worker_income
        .saturating_mul(10_000)
        .checked_div(final_worker_consumed)
        .unwrap_or(0);
    let final_non_lineage = living_non_lineage(&s);
    let survival_lift = baseline_non_lineage
        .map(|baseline| final_non_lineage as i64 - baseline as i64)
        .unwrap_or(0);
    let wage_stats = s.wage_labor_stats();
    let commons_by_id = commons_by_id(&s);
    let alive_by_id = alive_by_id(&s);

    Metrics {
        seed,
        mode,
        phi_bps,
        share_bps,
        share_term,
        conserved,
        registry_ok,
        commons_ok,
        money_ok,
        succession_registry_ok,
        // The disjointness guard is scoped to cells whose workers are non-lineage: in the
        // LineageWorker diagnostic a lineage EX-worker homesteading the frontier is the
        // legitimate outside option the cell probes, not contract-conferred title (the
        // per-tick S23d identity counters still catch every non-lineage owner there).
        anti_title_ok: mode == ScenarioMode::LineageWorker || anti_title_ok(&s),
        owner_identity_ok,
        immortal_owned_plot_ticks,
        non_lineage_owner_plot_ticks,
        owner_old_age_deaths: s.secure_land_owner_old_age_deaths_total(),
        extinct: living(&s) == 0,
        final_non_lineage,
        survival_lift,
        final_contracts: stats
            .contracts_total
            .saturating_sub(stats_start.contracts_total),
        final_renewals: stats
            .renewals_total
            .saturating_sub(stats_start.renewals_total),
        final_worker_share_bps,
        final_worker_consumed,
        final_worker_income,
        final_owner_income,
        final_wage_hires: wage_stats
            .hires_post_promotion
            .saturating_sub(wage_stats_start.hires_post_promotion),
        stats,
        worker_ids,
        commons_by_id,
        alive_by_id,
        wage_hires_post_promotion: wage_stats.hires_post_promotion,
    }
}

#[test]
fn precondition_no_contract_reproduces_marginal_null() {
    for seed in SEEDS {
        let metrics = run_metrics(seed, ScenarioMode::NoContract);
        eprintln!("{}", metrics.line());
        assert_eq!(metrics.verdict(), Verdict::SubsistenceBoundDespiteScarcity);
        assert_eq!(metrics.stats.contracts_total, 0);
        assert_eq!(metrics.stats.open_contracts, 0);
        assert!(metrics.renewal_fates_consistent());
    }
}

#[test]
fn verdict_prints_without_asserting_success() {
    for seed in SEEDS {
        let no_contract = run_metrics(seed, ScenarioMode::NoContract);
        let voluntary =
            run_metrics_with_baseline(seed, ScenarioMode::Voluntary, no_contract.final_non_lineage);
        eprintln!("{}", no_contract.line());
        eprintln!("{}", voluntary.line());
        eprintln!("{}", substitution_line(seed, &voluntary, &no_contract));
        assert_ne!(voluntary.verdict(), Verdict::ConservationBroken);
        assert_ne!(voluntary.verdict(), Verdict::RegistryBroken);
        // §6 hard guards, every run: the split's regen bound, the deposit attribution, and
        // the S23d owner-identity invariants (no contracted worker ever acquires title).
        assert_eq!(voluntary.stats.share_stock_drawdown, 0);
        assert_eq!(voluntary.stats.unattributed_share_deposit, 0);
        assert!(voluntary.renewal_fates_consistent());
        assert_eq!(voluntary.immortal_owned_plot_ticks, 0);
        assert_eq!(voluntary.non_lineage_owner_plot_ticks, 0);
        assert!(voluntary.anti_title_ok);
        // Vacuity is whole-run (spec §2: "voluntary contracts EVER clear") — the verdict
        // mapping is pinned here, never asserted toward success (review P1: the final-window
        // delta misclassified clears-then-stops runs as ShareVacuous).
        if voluntary.stats.voluntary_contracts_total < MIN_CONTRACTS {
            assert_eq!(voluntary.verdict(), Verdict::ShareVacuous);
        } else {
            assert!(matches!(
                voluntary.verdict(),
                Verdict::ShareTenancyClears | Verdict::ShareClearsButNoLift
            ));
        }
    }
}

#[test]
fn forward_provisioning_verdict_prints_without_asserting_success() {
    for seed in SEEDS {
        let no_contract = run_metrics(seed, ScenarioMode::NoContract);
        let forward_off =
            run_metrics_with_baseline(seed, ScenarioMode::Voluntary, no_contract.final_non_lineage);
        let forward = run_metrics_with_baseline(
            seed,
            ScenarioMode::ForwardProvisioning,
            no_contract.final_non_lineage,
        );
        eprintln!("{}", no_contract.line());
        eprintln!("{}", forward_off.line());
        eprintln!("{}", forward.line());
        eprintln!("{}", substitution_line(seed, &forward_off, &no_contract));
        eprintln!("{}", substitution_line(seed, &forward, &no_contract));
        eprintln!("{}", matched_volume_line(seed, &forward, &forward_off));

        assert_eq!(forward_off.verdict(), Verdict::ShareClearsButNoLift);
        assert_eq!(forward_off.stats.forward_only_eligibility, 0);
        assert_ne!(forward.verdict(), Verdict::ConservationBroken);
        assert_ne!(forward.verdict(), Verdict::RegistryBroken);
        assert_eq!(forward.stats.share_stock_drawdown, 0);
        assert_eq!(forward.stats.unattributed_share_deposit, 0);
        assert_eq!(forward.immortal_owned_plot_ticks, 0);
        assert_eq!(forward.non_lineage_owner_plot_ticks, 0);
        assert!(forward.anti_title_ok);
        assert!(forward.renewal_fates_consistent());

        if forward.stats.forward_only_eligibility == 0 {
            assert_eq!(forward.verdict(), Verdict::ForwardGateInert);
        } else if forward.stats.renewals_total < MIN_RENEWALS {
            assert_eq!(forward.verdict(), Verdict::RenewalStillDeclined);
        } else {
            assert!(matches!(
                forward.verdict(),
                Verdict::RenewalStillDeclined
                    | Verdict::StandingTenancyNoLift
                    | Verdict::StandingTenancyLifts
            ));
        }
    }
}

#[test]
fn succession_verdict_prints_without_asserting_success() {
    for seed in SEEDS {
        let no_contract = run_metrics(seed, ScenarioMode::NoContract);
        let forward = run_metrics_with_baseline(
            seed,
            ScenarioMode::ForwardProvisioning,
            no_contract.final_non_lineage,
        );
        let succession_voluntary = run_metrics_with_baseline(
            seed,
            ScenarioMode::SuccessionVoluntary,
            no_contract.final_non_lineage,
        );
        let succession = run_metrics_with_baseline(
            seed,
            ScenarioMode::Succession,
            no_contract.final_non_lineage,
        );
        eprintln!("{}", no_contract.line());
        eprintln!("{}", forward.line());
        eprintln!("{}", succession_voluntary.line());
        eprintln!("{}", succession.line());
        eprintln!("{}", matched_volume_line(seed, &succession, &forward));

        assert!(matches!(
            forward.verdict(),
            Verdict::ForwardGateInert
                | Verdict::RenewalStillDeclined
                | Verdict::StandingTenancyNoLift
                | Verdict::StandingTenancyLifts
        ));
        for metrics in [&succession_voluntary, &succession] {
            assert_ne!(metrics.verdict(), Verdict::ConservationBroken);
            assert_ne!(metrics.verdict(), Verdict::RegistryBroken);
            assert_eq!(metrics.stats.share_stock_drawdown, 0);
            assert_eq!(metrics.stats.unattributed_share_deposit, 0);
            assert_eq!(metrics.immortal_owned_plot_ticks, 0);
            assert_eq!(metrics.non_lineage_owner_plot_ticks, 0);
            assert!(metrics.anti_title_ok);
            assert!(metrics.renewal_fates_consistent());
            assert!(metrics.succession_mark_consistent());
            assert!(metrics.succession_registry_ok);
            if metrics.stats.successions_total == 0 {
                assert_eq!(metrics.verdict(), Verdict::SuccessionInert);
            } else if metrics.stats.final_open_succeeded < MIN_FINAL_OPEN_CONTRACTS
                && metrics.stats.post_succession_renewals < MIN_RENEWALS
            {
                assert_eq!(metrics.verdict(), Verdict::SuccessionButStillTransient);
            } else {
                assert!(matches!(
                    metrics.verdict(),
                    Verdict::StandingTenureForms | Verdict::StandingTenureLifts
                ));
            }
        }
    }
}

#[test]
fn forced_share_classifies_before_vacuity() {
    let metrics = run_metrics(SEEDS[0], ScenarioMode::ForcedShare);
    eprintln!("{}", metrics.line());
    assert_eq!(metrics.verdict(), Verdict::ShareScaffoldOnly);
    assert!(metrics.renewal_fates_consistent());
}

#[test]
fn wage_comparative_printed_beside_share_cell() {
    for seed in SEEDS {
        let no_contract = run_metrics(seed, ScenarioMode::NoContract);
        let share =
            run_metrics_with_baseline(seed, ScenarioMode::Voluntary, no_contract.final_non_lineage);
        let wage = run_metrics(seed, ScenarioMode::WageComparative);
        eprintln!(
            "C1R comparative share={} | wage={}",
            share.line(),
            wage.line()
        );
        assert_ne!(share.verdict(), Verdict::ConservationBroken);
        assert_ne!(wage.verdict(), Verdict::ConservationBroken);
        assert!(share.renewal_fates_consistent());
        assert!(wage.renewal_fates_consistent());
        // The wage cell prints a C1 WAGE classification (review P2) — the expected shape
        // is WageMarketVacuous; the mapping (not the outcome) is what's asserted.
        assert!(matches!(
            wage.verdict(),
            Verdict::WageMarketVacuous | Verdict::WageHiresObserved
        ));
    }
}

#[test]
fn lineage_worker_diagnostic_prints() {
    let metrics = run_metrics(SEEDS[0], ScenarioMode::LineageWorker);
    eprintln!("{}", metrics.line());
    assert_ne!(metrics.verdict(), Verdict::ConservationBroken);
    assert_ne!(metrics.verdict(), Verdict::RegistryBroken);
    assert!(metrics.renewal_fates_consistent());
}

#[test]
fn phi_share_and_term_sweeps_reported() {
    let phis = [
        ("scarce", RIVAL_COMMONS_PHI_SCARCE_BPS),
        ("marginal", RIVAL_COMMONS_PHI_MARGINAL_BPS),
        ("abundant", RIVAL_COMMONS_PHI_ABUNDANT_BPS),
    ];
    for (label, phi) in phis {
        let mut clears = 0usize;
        let mut forward_counts = BTreeMap::<Verdict, usize>::new();
        let mut succession_counts = BTreeMap::<Verdict, usize>::new();
        for seed in SEEDS {
            let base = run_metrics_cell(
                seed,
                ScenarioMode::NoContract,
                phi,
                SHARE_TENANCY_BPS_DEFAULT,
                SHARE_TENANCY_TERM_DEFAULT,
                None,
            );
            let metrics = run_metrics_cell(
                seed,
                ScenarioMode::Voluntary,
                phi,
                SHARE_TENANCY_BPS_DEFAULT,
                SHARE_TENANCY_TERM_DEFAULT,
                Some(base.final_non_lineage),
            );
            eprintln!("C1R phi={label} {}", metrics.line());
            if metrics.verdict() == Verdict::ShareTenancyClears {
                clears += 1;
            }
            let forward = run_metrics_cell(
                seed,
                ScenarioMode::ForwardProvisioning,
                phi,
                SHARE_TENANCY_BPS_DEFAULT,
                SHARE_TENANCY_TERM_DEFAULT,
                Some(base.final_non_lineage),
            );
            eprintln!("P1.5 phi={label} {}", forward.line());
            eprintln!("{}", matched_volume_line(seed, &forward, &metrics));
            *forward_counts.entry(forward.verdict()).or_insert(0) += 1;
            let succession = run_metrics_cell(
                seed,
                ScenarioMode::Succession,
                phi,
                SHARE_TENANCY_BPS_DEFAULT,
                SHARE_TENANCY_TERM_DEFAULT,
                Some(base.final_non_lineage),
            );
            eprintln!("C1S phi={label} {}", succession.line());
            *succession_counts.entry(succession.verdict()).or_insert(0) += 1;
        }
        eprintln!("C1R phi_sweep phi={label} clears={clears}/{}", SEEDS.len());
        eprintln!("P1.5 phi_sweep phi={label} verdicts={forward_counts:?}");
        eprintln!("C1S phi_sweep phi={label} verdicts={succession_counts:?}");
    }

    for share_bps in [2_500, SHARE_TENANCY_BPS_DEFAULT, 7_500] {
        let mut clears = 0usize;
        let mut forward_counts = BTreeMap::<Verdict, usize>::new();
        for seed in SEEDS {
            let base = run_metrics_cell(
                seed,
                ScenarioMode::NoContract,
                RIVAL_COMMONS_PHI_MARGINAL_BPS,
                share_bps,
                SHARE_TENANCY_TERM_DEFAULT,
                None,
            );
            let metrics = run_metrics_cell(
                seed,
                ScenarioMode::Voluntary,
                RIVAL_COMMONS_PHI_MARGINAL_BPS,
                share_bps,
                SHARE_TENANCY_TERM_DEFAULT,
                Some(base.final_non_lineage),
            );
            eprintln!("C1R share_sweep share_bps={share_bps} {}", metrics.line());
            if metrics.verdict() == Verdict::ShareTenancyClears {
                clears += 1;
            }
            let forward = run_metrics_cell(
                seed,
                ScenarioMode::ForwardProvisioning,
                RIVAL_COMMONS_PHI_MARGINAL_BPS,
                share_bps,
                SHARE_TENANCY_TERM_DEFAULT,
                Some(base.final_non_lineage),
            );
            eprintln!("P1.5 share_sweep share_bps={share_bps} {}", forward.line());
            eprintln!("{}", matched_volume_line(seed, &forward, &metrics));
            *forward_counts.entry(forward.verdict()).or_insert(0) += 1;
        }
        eprintln!(
            "C1R share_sweep share_bps={share_bps} clears={clears}/{}",
            SEEDS.len()
        );
        eprintln!("P1.5 share_sweep share_bps={share_bps} verdicts={forward_counts:?}");
    }

    for term in [6, SHARE_TENANCY_TERM_DEFAULT, 24] {
        let mut clears = 0usize;
        let mut forward_counts = BTreeMap::<Verdict, usize>::new();
        for seed in SEEDS {
            let base = run_metrics_cell(
                seed,
                ScenarioMode::NoContract,
                RIVAL_COMMONS_PHI_MARGINAL_BPS,
                SHARE_TENANCY_BPS_DEFAULT,
                term,
                None,
            );
            let metrics = run_metrics_cell(
                seed,
                ScenarioMode::Voluntary,
                RIVAL_COMMONS_PHI_MARGINAL_BPS,
                SHARE_TENANCY_BPS_DEFAULT,
                term,
                Some(base.final_non_lineage),
            );
            eprintln!("C1R term_sweep term={term} {}", metrics.line());
            if metrics.verdict() == Verdict::ShareTenancyClears {
                clears += 1;
            }
            let forward = run_metrics_cell(
                seed,
                ScenarioMode::ForwardProvisioning,
                RIVAL_COMMONS_PHI_MARGINAL_BPS,
                SHARE_TENANCY_BPS_DEFAULT,
                term,
                Some(base.final_non_lineage),
            );
            eprintln!("P1.5 term_sweep term={term} {}", forward.line());
            eprintln!("{}", matched_volume_line(seed, &forward, &metrics));
            *forward_counts.entry(forward.verdict()).or_insert(0) += 1;
        }
        eprintln!("C1R term_sweep term={term} clears={clears}/{}", SEEDS.len());
        eprintln!("P1.5 term_sweep term={term} verdicts={forward_counts:?}");
    }
}

#[test]
fn canonical_bytes_split_only_when_share_tenancy_active() {
    let off = Settlement::generate(7, &marginal_config(ScenarioMode::NoContract));
    let on = Settlement::generate(7, &marginal_config(ScenarioMode::Voluntary));
    assert_ne!(off.canonical_bytes(), on.canonical_bytes());

    let mut inert = base_config();
    inert
        .chain
        .as_mut()
        .expect("C1R base carries a chain")
        .share_tenancy = true;
    let active_off = Settlement::generate(7, &base_config());
    let inert_on = Settlement::generate(7, &inert);
    assert_eq!(
        active_off.canonical_bytes(),
        inert_on.canonical_bytes(),
        "share_tenancy must be inert off the S23e substrate"
    );
}

#[test]
fn canonical_bytes_include_share_terms_only_when_active() {
    let mut on_a = marginal_config(ScenarioMode::Voluntary);
    let mut on_b = marginal_config(ScenarioMode::Voluntary);
    {
        let chain = on_b.chain.as_mut().expect("C1R base carries a chain");
        chain.share_bps = 7_500;
        chain.share_term = 24;
    }
    assert_ne!(
        Settlement::generate(7, &on_a).canonical_bytes(),
        Settlement::generate(7, &on_b).canonical_bytes()
    );

    let mut off_b = marginal_config(ScenarioMode::NoContract);
    {
        let chain = off_b.chain.as_mut().expect("C1R base carries a chain");
        chain.share_bps = 7_500;
        chain.share_term = 24;
    }
    on_a.chain
        .as_mut()
        .expect("C1R base carries a chain")
        .share_tenancy = false;
    assert_eq!(
        Settlement::generate(7, &on_a).canonical_bytes(),
        Settlement::generate(7, &off_b).canonical_bytes()
    );
}

#[test]
fn canonical_bytes_split_for_share_forward_provisioning() {
    let forward_off = Settlement::generate(7, &marginal_config(ScenarioMode::Voluntary));
    let explicit_forward_off = {
        let mut cfg = marginal_config(ScenarioMode::Voluntary);
        cfg.chain
            .as_mut()
            .expect("C1R base carries a chain")
            .share_forward_provisioning = false;
        Settlement::generate(7, &cfg)
    };
    assert_eq!(
        forward_off.canonical_bytes(),
        explicit_forward_off.canonical_bytes(),
        "forward-off must reproduce the C1R share-tenancy bytes"
    );

    let forward_on = Settlement::generate(7, &marginal_config(ScenarioMode::ForwardProvisioning));
    assert_ne!(
        forward_off.canonical_bytes(),
        forward_on.canonical_bytes(),
        "forward-on must split canonical bytes under tag 24"
    );

    let forward_flag_without_share = {
        let mut cfg = marginal_config(ScenarioMode::NoContract);
        cfg.chain
            .as_mut()
            .expect("C1R base carries a chain")
            .share_forward_provisioning = true;
        Settlement::generate(7, &cfg)
    };
    let no_contract = Settlement::generate(7, &marginal_config(ScenarioMode::NoContract));
    assert_eq!(
        no_contract.canonical_bytes(),
        forward_flag_without_share.canonical_bytes(),
        "the forward sub-flag is inert unless share tenancy is active"
    );
}

#[test]
fn canonical_bytes_split_for_share_contract_succession() {
    let succession_off = Settlement::generate(7, &marginal_config(ScenarioMode::Voluntary));
    let explicit_succession_off = {
        let mut cfg = marginal_config(ScenarioMode::Voluntary);
        cfg.chain
            .as_mut()
            .expect("C1R base carries a chain")
            .share_contract_succession = false;
        Settlement::generate(7, &cfg)
    };
    assert_eq!(
        succession_off.canonical_bytes(),
        explicit_succession_off.canonical_bytes(),
        "succession-off must reproduce the C1R share-tenancy bytes"
    );

    let succession_on =
        Settlement::generate(7, &marginal_config(ScenarioMode::SuccessionVoluntary));
    assert_ne!(
        succession_off.canonical_bytes(),
        succession_on.canonical_bytes(),
        "succession-on must split canonical bytes under tag 26"
    );

    let succession_flag_without_share = {
        let mut cfg = marginal_config(ScenarioMode::NoContract);
        cfg.chain
            .as_mut()
            .expect("C1R base carries a chain")
            .share_contract_succession = true;
        Settlement::generate(7, &cfg)
    };
    let no_contract = Settlement::generate(7, &marginal_config(ScenarioMode::NoContract));
    assert_eq!(
        no_contract.canonical_bytes(),
        succession_flag_without_share.canonical_bytes(),
        "the succession sub-flag is inert unless share tenancy is active"
    );
}

#[test]
fn share_split_books_exact_cumulative_floor() {
    let cfg = marginal_config(ScenarioMode::ForcedShare);
    let mut s = Settlement::generate(SEEDS[0], &cfg);
    s.run(RUN_TICKS);
    let stats = s.share_tenancy_stats();
    let total_split = stats
        .worker_bread_income
        .saturating_add(stats.owner_bread_income);
    assert!(
        stats.contracts_total > 0,
        "forced scaffold should open at least one contract"
    );
    assert!(
        total_split > 0,
        "forced scaffold should realize at least one split"
    );
    assert!(
        stats.worker_bread_income > 0,
        "the cumulative floor must pay the worker (per-batch flooring of the 1-loaf \
         Cultivate output would zero it — review P1)"
    );
    // Cumulative-exact floor per contract: every contract pays the worker exactly
    // floor(N_c · share_bps / 10_000) of its cumulative output N_c (the split carries a
    // sub-unit remainder across 1-loaf batches), so the aggregate worker income is bounded
    // by the aggregate floor above and forfeits strictly less than one loaf per contract
    // below (each contract's final residue lapses to the owner at dissolution).
    let aggregate_floor =
        (u128::from(total_split) * u128::from(SHARE_TENANCY_BPS_DEFAULT) / 10_000) as u64;
    assert!(
        stats.worker_bread_income <= aggregate_floor,
        "worker income {} must never exceed the aggregate floor {}",
        stats.worker_bread_income,
        aggregate_floor
    );
    assert!(
        stats
            .worker_bread_income
            .saturating_add(stats.contracts_total)
            >= aggregate_floor,
        "worker income {} forfeits at most one sub-unit residue per contract ({}) below \
         the aggregate floor {}",
        stats.worker_bread_income,
        stats.contracts_total,
        aggregate_floor
    );
    assert_eq!(stats.share_stock_drawdown, 0);
    assert_eq!(stats.unattributed_share_deposit, 0);
    assert!(anti_title_ok(&s));
}

#[test]
fn goldens_unchanged_with_share_tenancy_off() {
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

    let base = Settlement::generate(7, &marginal_config(ScenarioMode::NoContract));
    let mut explicit_off = marginal_config(ScenarioMode::NoContract);
    explicit_off
        .chain
        .as_mut()
        .expect("C1R base carries a chain")
        .share_tenancy = false;
    let explicit_off = Settlement::generate(7, &explicit_off);
    assert_eq!(base.digest(), explicit_off.digest());
}
