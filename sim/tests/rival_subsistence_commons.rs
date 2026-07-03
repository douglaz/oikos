//! S23e acceptance suite: replace the S23d unlimited emergency floor with a finite,
//! regenerating, non-excludable rival subsistence commons. The scarce headline prints an
//! ordered verdict; success is not asserted.

use std::collections::{BTreeMap, BTreeSet};

use econ::good::SALT;
use sim::{
    rival_subsistence_commons_regen_for_phi, OwnerSurplusTelemetry, Settlement, SettlementConfig,
    Vocation, RIVAL_COMMONS_BASELINE_EMERGENCY_DRAW, RIVAL_COMMONS_BASELINE_FINAL_WINDOW_TICKS,
    RIVAL_COMMONS_K_TICKS, RIVAL_COMMONS_PHI_ABUNDANT_BPS, RIVAL_COMMONS_PHI_MARGINAL_BPS,
    RIVAL_COMMONS_PHI_SCARCE_BPS,
};

#[path = "support/mod.rs"]
mod support;
use support::living;

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
const FINAL_WINDOW: u64 = RIVAL_COMMONS_BASELINE_FINAL_WINDOW_TICKS;
const ADULT_AGE_TICKS: u64 = 6;
const BORN_IN_SIM_OWNER_COHORTS: usize = 3;
const INHERIT_ELIGIBLE_OWNER_DEATHS_MIN: u64 = 20;
const MATERIAL_BOUGHT_FLOOR: u64 = 1_000;
const MARKET_CAP: f64 = 0.50;
const SURVIVE_MIN: f64 = 0.10;
const SURVIVAL_LIFT: f64 = 0.05;
const OWNER_SURPLUS_FLOOR: u64 = 1;
const OWNER_ATTEMPT_MIN: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    ConservationBroken,
    Extinct,
    ImmortalOwnerResidue,
    NonLineageOwnerResidue,
    MoneyFailure,
    ScarcityStarves,
    AbundanceReproducesNull,
    NoSurplusUnderMortality,
    SubsistenceBoundDespiteScarcity,
    ScarcityForcesMarket,
}

#[derive(Clone, Debug)]
struct Metrics {
    seed: u64,
    label: &'static str,
    phi_bps: u32,
    conserved: bool,
    registry_ok: bool,
    commons_rival: bool,
    plot_count_preserved: bool,
    extinct: bool,
    promoted: bool,
    money_persisted: bool,
    pre_promo_self_produced: u64,
    seeded_minted: u64,
    final_survival_fraction: f64,
    final_commons_share: f64,
    final_commons_draw: u64,
    final_consumed: u64,
    final_buyer_bought: u64,
    final_hungry_buyers: usize,
    immortal_owner_ticks: u64,
    non_lineage_owner_ticks: u64,
    owner_identity_ok: bool,
    inherit_eligible_owner_deaths: u64,
    inheritance_transfers: usize,
    born_in_sim_owner_count: usize,
    owner_surplus_sold_before_death: u64,
    owner_seller_attributed_bought: u64,
    owner_tenure_before_death_max: u64,
    owner_inventory_at_death: u64,
    owner_surplus_produced_minus_consumed: i64,
    inherited_stock_to_heirs: u64,
    commons_depleted_ticks: u64,
    commons_unmet_total: u64,
    telemetry: OwnerSurplusTelemetry,
}

impl Metrics {
    fn conservation_ok(&self) -> bool {
        self.conserved && self.registry_ok && self.commons_rival && self.plot_count_preserved
    }

    fn owner_identity_ok(&self) -> bool {
        self.immortal_owner_ticks == 0
            && self.non_lineage_owner_ticks == 0
            && self.owner_identity_ok
    }

    fn money_ok(&self) -> bool {
        self.promoted
            && self.money_persisted
            && self.pre_promo_self_produced > 0
            && self.seeded_minted == 0
    }

    fn demographic_clauses_hold(&self) -> bool {
        self.owner_identity_ok()
            && self.inherit_eligible_owner_deaths >= INHERIT_ELIGIBLE_OWNER_DEATHS_MIN
            && self.inheritance_transfers >= INHERIT_ELIGIBLE_OWNER_DEATHS_MIN as usize
            && self.born_in_sim_owner_count >= BORN_IN_SIM_OWNER_COHORTS
            && self.money_ok()
    }

    fn success_clauses_hold(&self, no_owner_surplus: &Metrics) -> bool {
        self.conservation_ok()
            && !self.extinct
            && self.final_commons_share <= MARKET_CAP
            && self.final_buyer_bought >= MATERIAL_BOUGHT_FLOOR
            && self.final_survival_fraction
                >= no_owner_surplus.final_survival_fraction + SURVIVAL_LIFT
            && self.owner_surplus_sold_before_death >= OWNER_SURPLUS_FLOOR
            && self.owner_seller_attributed_bought >= OWNER_SURPLUS_FLOOR
            && self.demographic_clauses_hold()
    }

    fn verdict(&self, no_owner_surplus: Option<&Metrics>) -> Verdict {
        if !self.conservation_ok() {
            return Verdict::ConservationBroken;
        }
        if self.extinct {
            return Verdict::Extinct;
        }
        if self.immortal_owner_ticks > 0 {
            return Verdict::ImmortalOwnerResidue;
        }
        if self.non_lineage_owner_ticks > 0 || !self.owner_identity_ok {
            return Verdict::NonLineageOwnerResidue;
        }
        if !self.money_ok() {
            return Verdict::MoneyFailure;
        }
        if self.final_survival_fraction < SURVIVE_MIN {
            return Verdict::ScarcityStarves;
        }
        if let Some(no_owner) = no_owner_surplus {
            if self.final_survival_fraction < no_owner.final_survival_fraction + SURVIVAL_LIFT
                && self.final_buyer_bought < MATERIAL_BOUGHT_FLOOR
            {
                return Verdict::ScarcityStarves;
            }
            if self.success_clauses_hold(no_owner) {
                return Verdict::ScarcityForcesMarket;
            }
        }
        if self.phi_bps == RIVAL_COMMONS_PHI_ABUNDANT_BPS
            && (self.final_commons_share > MARKET_CAP
                || self.final_buyer_bought < MATERIAL_BOUGHT_FLOOR)
        {
            return Verdict::AbundanceReproducesNull;
        }
        if self.commons_depleted_ticks > 0
            && self.commons_unmet_total > 0
            && self.final_hungry_buyers > 0
            && self.owner_tenure_before_death_max >= OWNER_ATTEMPT_MIN
            && self.owner_surplus_sold_before_death < OWNER_SURPLUS_FLOOR
        {
            return Verdict::NoSurplusUnderMortality;
        }
        if self.owner_surplus_sold_before_death > 0
            && (self.final_commons_share > MARKET_CAP
                || self.final_buyer_bought < MATERIAL_BOUGHT_FLOOR)
        {
            return Verdict::SubsistenceBoundDespiteScarcity;
        }
        Verdict::ScarcityStarves
    }

    fn line(&self, no_owner_surplus: Option<&Metrics>) -> String {
        format!(
            "S23e {} seed={} phi={} {:?} conserved={} registry={} rival={} promoted={} \
             survival={:.3} commons={}/{} share={:.3} buyer_bought={} hungry_buyers={} \
             owner_sold={} owner_attributed_bought={} tenure_max={} inv_at_death={} \
             produced_minus_consumed={} inherited_stock={} depleted_ticks={} unmet={}",
            self.label,
            self.seed,
            self.phi_bps,
            self.verdict(no_owner_surplus),
            self.conserved,
            self.registry_ok,
            self.commons_rival,
            self.promoted,
            self.final_survival_fraction,
            self.final_commons_draw,
            self.final_consumed,
            self.final_commons_share,
            self.final_buyer_bought,
            self.final_hungry_buyers,
            self.owner_surplus_sold_before_death,
            self.owner_seller_attributed_bought,
            self.owner_tenure_before_death_max,
            self.owner_inventory_at_death,
            self.owner_surplus_produced_minus_consumed,
            self.inherited_stock_to_heirs,
            self.commons_depleted_ticks,
            self.commons_unmet_total,
        )
    }
}

fn base_config() -> SettlementConfig {
    SettlementConfig::frontier_mortal_landowner_demography()
}

fn commons_config(phi_bps: u32) -> SettlementConfig {
    let mut cfg = base_config();
    let chain = cfg.chain.as_mut().expect("S23e base carries a chain");
    chain.rival_subsistence_commons = true;
    chain.rival_subsistence_commons_phi_bps = phi_bps;
    cfg
}

fn scarce_no_owner_surplus_config() -> SettlementConfig {
    let mut cfg = commons_config(RIVAL_COMMONS_PHI_SCARCE_BPS);
    cfg.chain
        .as_mut()
        .expect("S23e base carries a chain")
        .cultivate_consume = u32::MAX;
    cfg
}

fn inherited_stock_diagnostic_config() -> SettlementConfig {
    commons_config(RIVAL_COMMONS_PHI_SCARCE_BPS)
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

fn plot_owner_counts(s: &Settlement) -> BTreeMap<u64, u64> {
    let mut counts = BTreeMap::new();
    for (_, owner, _, _, _, _) in s.private_land_plot_summaries() {
        if let Some(owner) = owner {
            *counts.entry(owner).or_insert(0) += 1;
        }
    }
    counts
}

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

fn final_hungry_buyer_count(s: &Settlement, threshold: u16) -> usize {
    let owners = owner_ids(s);
    (0..s.population())
        .filter(|&i| {
            s.is_alive(i)
                && s.colonist_id(i)
                    .map(|id| !owners.contains(&id.0))
                    .unwrap_or(false)
                && matches!(
                    s.vocation_of(i),
                    Some(Vocation::Consumer | Vocation::Gatherer)
                )
                && s.need_of(i).is_some_and(|need| need.hunger >= threshold)
        })
        .count()
}

fn born_in_sim_owner_count(s: &Settlement) -> usize {
    let index_by_id = id_to_index(s);
    s.private_land_owner_identity_rows()
        .into_iter()
        .filter(|row| {
            row.born_in_sim
                && index_by_id
                    .get(&row.owner)
                    .and_then(|&index| s.age_of(index))
                    .is_some_and(|age| age >= ADULT_AGE_TICKS)
        })
        .map(|row| row.owner)
        .collect::<BTreeSet<_>>()
        .len()
}

fn telemetry_sums(telemetry: &OwnerSurplusTelemetry) -> (u64, u64, u64, i64, u64) {
    let sold = telemetry
        .owner_surplus_sold_before_death
        .iter()
        .map(|&(_, qty)| qty)
        .sum();
    let tenure_max = telemetry
        .owner_tenure_before_death
        .iter()
        .map(|&(_, tenure)| tenure)
        .max()
        .unwrap_or(0);
    let inventory = telemetry
        .owner_inventory_at_death
        .iter()
        .map(|&(_, qty)| qty)
        .sum();
    let produced_minus_consumed = telemetry
        .owner_surplus_produced_minus_consumed
        .iter()
        .map(|&(_, net)| net)
        .sum();
    (
        sold,
        tenure_max,
        inventory,
        produced_minus_consumed,
        telemetry.inherited_stock_to_heirs,
    )
}

fn run_metrics(seed: u64, label: &'static str, cfg: SettlementConfig) -> Metrics {
    let mut s = Settlement::generate(seed, &cfg);
    let bread = s.bread_good().expect("S23e base carries bread");
    let threshold = cfg
        .chain
        .as_ref()
        .expect("S23e base carries a chain")
        .emergency_hunger_threshold;
    let initial_population = s.population().max(1) as f64;
    let initial_plot_count = s.private_land_plot_count();
    let final_start = RUN_TICKS.saturating_sub(FINAL_WINDOW);
    let mut conserved = true;
    let mut registry_ok = true;
    let mut commons_rival = true;
    let mut plot_count_preserved = true;
    let mut final_bought_start = BTreeMap::new();
    let mut final_consumed_total = 0u64;
    let mut final_commons_draw_start = 0u64;
    let mut final_owner_seller_attributed_bought_start = 0u64;
    let mut final_survival_sum = 0u64;
    let mut final_survival_ticks = 0u64;
    let mut immortal_owner_ticks = 0u64;
    let mut non_lineage_owner_ticks = 0u64;
    let mut owner_identity_ok = true;

    for tick in 0..RUN_TICKS {
        if tick == final_start {
            final_bought_start = bought_by_id(&s);
            final_commons_draw_start = s.rival_subsistence_commons_state().drawn_total;
            final_owner_seller_attributed_bought_start =
                s.owner_surplus_telemetry().owner_seller_attributed_bought;
        }
        let report = s.econ_tick();
        conserved &= report.conserves();
        registry_ok &= s.private_land_registry_invariant_holds();
        plot_count_preserved &= s.private_land_plot_count() == initial_plot_count;
        commons_rival &= report.subsistence_commons_conserves();
        commons_rival &= report.subsistence_commons_stock_after <= report.subsistence_commons_cap;
        commons_rival &= report.subsistence_commons_draw_of(bread)
            <= report
                .subsistence_commons_stock_before
                .saturating_add(report.subsistence_commons_regen_of(bread));
        owner_identity_ok &=
            observe_owner_residue(&s, &mut immortal_owner_ticks, &mut non_lineage_owner_ticks);

        if tick >= final_start {
            final_survival_sum = final_survival_sum.saturating_add(living(&s) as u64);
            final_survival_ticks = final_survival_ticks.saturating_add(1);
            final_consumed_total = final_consumed_total.saturating_add(report.consumed_of(bread));
        }
    }

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
    let final_consumed = final_consumed_total;
    let final_commons_draw = s
        .rival_subsistence_commons_state()
        .drawn_total
        .saturating_sub(final_commons_draw_start);
    let final_commons_share = if final_consumed == 0 {
        0.0
    } else {
        final_commons_draw as f64 / final_consumed as f64
    };
    let final_survival_fraction = if final_survival_ticks == 0 {
        0.0
    } else {
        final_survival_sum as f64 / final_survival_ticks as f64 / initial_population
    };
    let (pre_promo_self_produced, pre_promo_minted) =
        s.pre_promotion_bread_for_salt_by_provenance();
    let telemetry = s.owner_surplus_telemetry();
    let (sold, tenure_max, inventory, produced_minus_consumed, inherited_stock) =
        telemetry_sums(&telemetry);
    let state = s.rival_subsistence_commons_state();

    Metrics {
        seed,
        label,
        phi_bps: state.phi_bps,
        conserved,
        registry_ok,
        commons_rival,
        plot_count_preserved,
        extinct: living(&s) == 0,
        promoted: s.promoted_at_tick().is_some(),
        money_persisted: s.current_money_good() == Some(SALT),
        pre_promo_self_produced,
        seeded_minted: pre_promo_minted + s.seeded_minted_bread_sold_for_salt(),
        final_survival_fraction,
        final_commons_share,
        final_commons_draw,
        final_consumed,
        final_buyer_bought,
        final_hungry_buyers: final_hungry_buyer_count(&s, threshold),
        immortal_owner_ticks,
        non_lineage_owner_ticks,
        owner_identity_ok,
        inherit_eligible_owner_deaths: s.secure_land_inherit_eligible_owner_deaths_total(),
        inheritance_transfers: s
            .secure_land_inheritance_events()
            .into_iter()
            .filter(|event| event.heir.is_some())
            .count(),
        born_in_sim_owner_count: born_in_sim_owner_count(&s),
        owner_surplus_sold_before_death: sold,
        owner_seller_attributed_bought: telemetry
            .owner_seller_attributed_bought
            .saturating_sub(final_owner_seller_attributed_bought_start),
        owner_tenure_before_death_max: tenure_max,
        owner_inventory_at_death: inventory,
        owner_surplus_produced_minus_consumed: produced_minus_consumed,
        inherited_stock_to_heirs: inherited_stock,
        commons_depleted_ticks: state.depleted_ticks,
        commons_unmet_total: state.unmet_total,
        telemetry,
    }
}

fn print_metrics(metrics: &Metrics, no_owner_surplus: Option<&Metrics>) {
    eprintln!("{}", metrics.line(no_owner_surplus));
    eprintln!(
        "S23e {} seed={} owner_age_at_first_claim={:?} owner_tenure_before_death={:?} \
         owner_surplus_produced_minus_consumed={:?} owner_inventory_at_death={:?} \
         buyer_purchases_by_owner_age_cohort={:?}",
        metrics.label,
        metrics.seed,
        metrics.telemetry.owner_age_at_first_claim,
        metrics.telemetry.owner_tenure_before_death,
        metrics.telemetry.owner_surplus_produced_minus_consumed,
        metrics.telemetry.owner_inventory_at_death,
        metrics.telemetry.buyer_purchases_by_owner_age_cohort,
    );
}

fn baseline_d0_for_seed(seed: u64) -> (u64, usize) {
    let cfg = base_config();
    let threshold = cfg
        .chain
        .as_ref()
        .expect("S23d base carries a chain")
        .emergency_hunger_threshold;
    let mut s = Settlement::generate(seed, &cfg);
    let final_start = RUN_TICKS.saturating_sub(FINAL_WINDOW);
    let mut floor_start = 0u64;
    let mut eligible = BTreeSet::new();
    for tick in 0..RUN_TICKS {
        if tick == final_start {
            floor_start = s.emergency_bread_provisioned();
        }
        if tick >= final_start {
            for i in 0..s.population() {
                if s.is_alive(i)
                    && s.household_of(i).is_none()
                    && matches!(
                        s.vocation_of(i),
                        Some(Vocation::Consumer | Vocation::Gatherer)
                    )
                    && s.need_of(i).is_some_and(|need| need.hunger >= threshold)
                {
                    if let Some(id) = s.colonist_id(i) {
                        eligible.insert(id.0);
                    }
                }
            }
        }
        s.econ_tick();
    }
    (
        s.emergency_bread_provisioned().saturating_sub(floor_start),
        eligible.len(),
    )
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

    let base = Settlement::generate(7, &base_config());
    let mut off = base_config();
    off.chain
        .as_mut()
        .expect("S23d base carries a chain")
        .rival_subsistence_commons_phi_bps = RIVAL_COMMONS_PHI_SCARCE_BPS;
    let off = Settlement::generate(7, &off);
    assert_eq!(base.canonical_bytes(), off.canonical_bytes());
    assert_eq!(base.digest(), off.digest());
}

#[test]
fn s23d_baseline_reproduced_for_d0() {
    let mut draws = Vec::new();
    let mut consumers = Vec::new();
    for seed in SEEDS {
        let (draw, n0) = baseline_d0_for_seed(seed);
        draws.push(draw);
        consumers.push(n0);
        eprintln!(
            "S23e D0 seed={} draw={} per_tick={:.3} N0={} c_eff={:.3}",
            seed,
            draw,
            draw as f64 / FINAL_WINDOW as f64,
            n0,
            if n0 == 0 {
                0.0
            } else {
                draw as f64 / n0 as f64
            },
        );
    }
    assert!(draws
        .iter()
        .all(|&draw| draw == RIVAL_COMMONS_BASELINE_EMERGENCY_DRAW));
    assert!(consumers.iter().all(|&n0| n0 > 0));
    assert_eq!(
        rival_subsistence_commons_regen_for_phi(RIVAL_COMMONS_PHI_SCARCE_BPS),
        8
    );
    assert_eq!(
        rival_subsistence_commons_regen_for_phi(RIVAL_COMMONS_PHI_MARGINAL_BPS),
        16
    );
    assert_eq!(
        rival_subsistence_commons_regen_for_phi(RIVAL_COMMONS_PHI_ABUNDANT_BPS),
        40
    );
}

#[test]
fn canonical_bytes_split_only_when_commons_active() {
    let base = Settlement::generate(7, &base_config());
    let on = Settlement::generate(7, &commons_config(RIVAL_COMMONS_PHI_SCARCE_BPS));
    assert_ne!(base.canonical_bytes(), on.canonical_bytes());
    assert_ne!(base.digest(), on.digest());
    assert_eq!(
        on.rival_subsistence_commons_state().cap,
        RIVAL_COMMONS_K_TICKS
            * rival_subsistence_commons_regen_for_phi(RIVAL_COMMONS_PHI_SCARCE_BPS)
    );

    let mut inert = SettlementConfig::frontier_endogenous_cultivation();
    let inert_off = Settlement::generate(7, &inert);
    let chain = inert
        .chain
        .as_mut()
        .expect("endogenous base carries a chain");
    chain.rival_subsistence_commons = true;
    chain.rival_subsistence_commons_phi_bps = RIVAL_COMMONS_PHI_SCARCE_BPS;
    let inert_on = Settlement::generate(7, &inert);
    assert_eq!(inert_off.canonical_bytes(), inert_on.canonical_bytes());
}

#[test]
fn commons_conserves_and_is_rival() {
    for seed in SEEDS {
        let mut s = Settlement::generate(seed, &commons_config(RIVAL_COMMONS_PHI_SCARCE_BPS));
        let bread = s.bread_good().expect("S23e base carries bread");
        for _ in 0..RUN_TICKS {
            let report = s.econ_tick();
            assert!(
                report.conserves(),
                "whole-system conservation failed seed={seed}"
            );
            assert!(
                report.subsistence_commons_conserves(),
                "commons conservation failed seed={seed}"
            );
            assert!(
                report.subsistence_commons_draw_of(bread)
                    <= report
                        .subsistence_commons_stock_before
                        .saturating_add(report.subsistence_commons_regen_of(bread)),
                "commons draw exceeded available stock seed={seed}"
            );
            assert!(
                report.subsistence_commons_stock_after <= report.subsistence_commons_cap,
                "commons stock exceeded K seed={seed}"
            );
        }
    }
}

#[test]
fn scarce_headline_classifies() {
    for seed in SEEDS {
        let no_owner = run_metrics(
            seed,
            "scarce_no_owner_surplus",
            scarce_no_owner_surplus_config(),
        );
        let metrics = run_metrics(
            seed,
            "scarce_headline",
            commons_config(RIVAL_COMMONS_PHI_SCARCE_BPS),
        );
        print_metrics(&metrics, Some(&no_owner));
        assert_ne!(
            metrics.verdict(Some(&no_owner)),
            Verdict::ConservationBroken
        );
    }
}

#[test]
fn owners_supply_surplus_or_not() {
    for seed in SEEDS {
        let metrics = run_metrics(
            seed,
            "scarce_headline",
            commons_config(RIVAL_COMMONS_PHI_SCARCE_BPS),
        );
        print_metrics(&metrics, None);
        assert!(
            metrics.conservation_ok(),
            "hard accounting failed: {}",
            metrics.line(None)
        );
        if metrics.owner_tenure_before_death_max > 0 {
            assert!(
                !metrics.telemetry.owner_tenure_before_death.is_empty(),
                "owner tenure telemetry missing"
            );
        }
    }
}

#[test]
fn demographic_clauses_still_hold() {
    for seed in SEEDS {
        let metrics = run_metrics(
            seed,
            "scarce_headline",
            commons_config(RIVAL_COMMONS_PHI_SCARCE_BPS),
        );
        print_metrics(&metrics, None);
        assert!(
            metrics.owner_identity_ok(),
            "owner identity failed: {}",
            metrics.line(None)
        );
        if metrics.verdict(None) == Verdict::ScarcityForcesMarket {
            assert!(
                metrics.demographic_clauses_hold(),
                "success cannot omit demographic clauses: {}",
                metrics.line(None)
            );
        }
    }
}

#[test]
fn abundant_outside_option() {
    for seed in SEEDS {
        let metrics = run_metrics(
            seed,
            "abundant",
            commons_config(RIVAL_COMMONS_PHI_ABUNDANT_BPS),
        );
        print_metrics(&metrics, None);
        assert_eq!(
            metrics.verdict(None),
            Verdict::AbundanceReproducesNull,
            "abundant outside option must reproduce the S23d null: {}",
            metrics.line(None)
        );
    }
}

#[test]
fn scarce_no_owner_surplus() {
    for seed in SEEDS {
        let metrics = run_metrics(
            seed,
            "scarce_no_owner_surplus",
            scarce_no_owner_surplus_config(),
        );
        print_metrics(&metrics, None);
        assert!(
            matches!(
                metrics.verdict(None),
                Verdict::ScarcityStarves
                    | Verdict::NoSurplusUnderMortality
                    | Verdict::SubsistenceBoundDespiteScarcity
                    | Verdict::MoneyFailure
            ),
            "no-owner-surplus control must stay non-viable: {}",
            metrics.line(None)
        );
    }
}

#[test]
fn marginal_outside_option() {
    for seed in SEEDS {
        let metrics = run_metrics(
            seed,
            "marginal",
            commons_config(RIVAL_COMMONS_PHI_MARGINAL_BPS),
        );
        print_metrics(&metrics, None);
        assert_ne!(metrics.verdict(None), Verdict::ConservationBroken);
    }
}

#[test]
fn scarce_inherited_stock_diagnostic() {
    for seed in SEEDS {
        let metrics = run_metrics(
            seed,
            "scarce_inherited_stock_diagnostic",
            inherited_stock_diagnostic_config(),
        );
        eprintln!(
            "S23e diagnostic seed={} inherited_stock_to_heirs={} owner_inventory_at_death={} \
             verdict_excluded={:?}",
            seed,
            metrics.inherited_stock_to_heirs,
            metrics.owner_inventory_at_death,
            metrics.verdict(None),
        );
    }
}
