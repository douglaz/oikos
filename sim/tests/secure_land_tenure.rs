//! S23c acceptance suite: secure private land tenure over the S23a property base.
//!
//! The headline verdict is classified and printed. Success is not asserted by the verdict
//! test; the suite asserts hard guards and the predeclared controls.

use std::collections::{BTreeMap, BTreeSet};

use econ::good::SALT;
use sim::{
    HouseholdSpec, InheritanceRegime, Settlement, SettlementConfig, LAND_VIABLE_REGEN_FLOOR,
};

#[path = "support/mod.rs"]
mod support;
use support::living;

const BASE_SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
const GENERATION_TICKS: u64 = 180;
const PRIMARY_DENSITIES: [u16; 2] = [24, 48];
const BOUNDARY_DENSITIES: [u16; 2] = [12, 96];
const HEADLINE_GOOD_PLOTS: u16 = 4;
const LONG_IDLE_HORIZON: u16 = 200;

const OWNER_MIN: f64 = 0.10;
const OWNER_MAX: f64 = 0.45;
const NON_OWNER_MIN: f64 = 0.50;
const OWNER_CULTIVATION_MIN: f64 = 0.60;
const OWNER_PRODUCTION_MIN: f64 = 0.50;
const OWNER_GRAIN_TO_BUYER_FOOD_MIN: f64 = 0.35;
const NON_OWNER_BOUGHT_MIN: f64 = 0.25;
const SURVIVAL_RATIO_MIN: f64 = 0.60;
const NON_DEATH_CHURN_MAX: f64 = 0.05;
const INERT_CULTIVATION_MAX: f64 = 0.30;
const INERT_OWNER_GRAIN_MAX: f64 = 0.20;
const INERT_BOUGHT_MAX: f64 = 0.10;
const UNIVERSAL_OWNER_MIN: f64 = 0.75;
const UNIVERSAL_NON_OWNER_MAX: f64 = 0.25;
const GINI_RISE: f64 = 0.15;
const TOP_OWNER_SHARE: f64 = 0.50;
const LANDLESS_SHARE: f64 = 0.60;
const FRAGMENT_STRANDED_SHARE: f64 = 0.35;
const MEDIAN_LAND_DROP: f64 = 0.40;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Scenario {
    Secure,
    LongForfeiture,
    S23aForfeiture,
    PropertyOff,
    NonExcludableDeed,
    S22fRobustness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    ConservationBroken,
    DisqualifiedNoInheritance,
    StillThrashes,
    UniversalOwnershipNoBuyers,
    FragmentationCollapse,
    LandlessProletariatFailure,
    LandlessProletariatHealthy,
    HereditaryConcentrationFailure,
    HereditaryConcentrationCoexisting,
    SeedClusterOnly,
    TenureInertStaticPin,
    SecureTenureStableClass,
    NoStableClass,
}

#[derive(Clone, Debug)]
struct GenerationTrace {
    generation: u64,
    owner_count: usize,
    owner_share: f64,
    land_gini: f64,
    top10_share: f64,
    land_per_capita: f64,
    landless_share: f64,
    inheritance_events: usize,
    heirless_events: usize,
    viable_plots: usize,
    stranded_shares: u64,
    buyer_survival: usize,
}

impl GenerationTrace {
    fn line(&self) -> String {
        format!(
            "gen={} owners={} owner_share={:.2} gini={:.2} top10={:.2} land_pc={:.2} \
             landless={:.2} inherit={} heirless={} viable={} stranded={} buyers={}",
            self.generation,
            self.owner_count,
            self.owner_share,
            self.land_gini,
            self.top10_share,
            self.land_per_capita,
            self.landless_share,
            self.inheritance_events,
            self.heirless_events,
            self.viable_plots,
            self.stranded_shares,
            self.buyer_survival,
        )
    }
}

#[derive(Clone, Debug)]
struct Metrics {
    seed: u64,
    scenario: Scenario,
    regime: InheritanceRegime,
    density: u16,
    conserved: bool,
    bread_minted_max: u64,
    provenance_clean: bool,
    promoted: bool,
    registry_ok: bool,
    extinct: bool,
    secure_tenure_on: bool,
    window_inheritance_events: usize,
    owner_share: f64,
    non_owner_share: f64,
    owner_count: usize,
    first_owner_count: usize,
    owner_cultivated_plot_share: f64,
    owner_production_share: f64,
    owner_grain_to_nonowner_food: f64,
    nonowner_bought_food_share: f64,
    nonowner_survival_ratio: f64,
    non_death_churn_per_generation: f64,
    land_gini_delta: f64,
    top10_share: f64,
    landless_share: f64,
    landless_rising_two_gens: bool,
    stranded_inherited_share: f64,
    median_land_drop: f64,
    food_or_survival_decline: bool,
    idle_losses: u64,
    traces: Vec<GenerationTrace>,
}

impl Metrics {
    fn hard_guards_hold(&self) -> bool {
        self.conserved
            && self.bread_minted_max == 0
            && self.provenance_clean
            && self.promoted
            && self.registry_ok
            && !self.extinct
    }

    fn concentration_tripped(&self) -> bool {
        self.land_gini_delta >= GINI_RISE || self.top10_share >= TOP_OWNER_SHARE
    }

    fn success(&self) -> bool {
        // A stable owner CLASS across generations presupposes ownership actually turned over
        // through death→inheritance in the window; a frozen registry cannot qualify.
        self.window_inheritance_events > 0
            && self.owner_share >= OWNER_MIN
            && self.owner_share <= OWNER_MAX
            && self.non_owner_share >= NON_OWNER_MIN
            && self.owner_cultivated_plot_share >= OWNER_CULTIVATION_MIN
            && self.owner_production_share >= OWNER_PRODUCTION_MIN
            && self.owner_grain_to_nonowner_food >= OWNER_GRAIN_TO_BUYER_FOOD_MIN
            && self.nonowner_bought_food_share >= NON_OWNER_BOUGHT_MIN
            && self.nonowner_survival_ratio >= SURVIVAL_RATIO_MIN
            && self.non_death_churn_per_generation <= NON_DEATH_CHURN_MAX
            && !self.concentration_tripped()
            && self.hard_guards_hold()
    }

    fn verdict(&self) -> Verdict {
        if !self.hard_guards_hold() {
            return Verdict::ConservationBroken;
        }
        if self.scenario == Scenario::S23aForfeiture
            && self.non_death_churn_per_generation > NON_DEATH_CHURN_MAX
        {
            return Verdict::StillThrashes;
        }
        if self.owner_share >= UNIVERSAL_OWNER_MIN || self.non_owner_share < UNIVERSAL_NON_OWNER_MAX
        {
            return Verdict::UniversalOwnershipNoBuyers;
        }
        // Under secure tenure, ownership turns over ONLY through death→inheritance. If no
        // inheritance fired in the evaluation window, the owner set is frozen and every
        // generational verdict below (concentration, fragmentation, landless, stable class,
        // seed-cluster stall) would describe a static registry rather than heritable-tenure
        // dynamics. That is the §5 honesty trap ("stable ownership" standing in for "a working
        // land economy") in its strongest form, so the run is DISQUALIFIED, not classified.
        // Homesteading-driven verdicts above (UniversalOwnershipNoBuyers, StillThrashes) are
        // reached first because they do not depend on generational turnover.
        if self.secure_tenure_on && self.window_inheritance_events == 0 {
            return Verdict::DisqualifiedNoInheritance;
        }
        if self.regime == InheritanceRegime::Partible
            && (self.stranded_inherited_share >= FRAGMENT_STRANDED_SHARE
                || self.median_land_drop >= MEDIAN_LAND_DROP)
            && self.food_or_survival_decline
        {
            return Verdict::FragmentationCollapse;
        }
        if self.regime == InheritanceRegime::Impartible
            && self.landless_share >= LANDLESS_SHARE
            && self.landless_rising_two_gens
        {
            return if self.nonowner_survival_ratio < SURVIVAL_RATIO_MIN
                || self.nonowner_bought_food_share < NON_OWNER_BOUGHT_MIN
            {
                Verdict::LandlessProletariatFailure
            } else {
                Verdict::LandlessProletariatHealthy
            };
        }
        if self.concentration_tripped() {
            return if self.nonowner_bought_food_share < NON_OWNER_BOUGHT_MIN
                || self.nonowner_survival_ratio < SURVIVAL_RATIO_MIN
            {
                Verdict::HereditaryConcentrationFailure
            } else {
                Verdict::HereditaryConcentrationCoexisting
            };
        }
        // SeedClusterOnly = the owner class never grew past its initial seed cohort AND never
        // turned over. The turnover clause is load-bearing: under impartible tenure with finite
        // plots one heir replaces one dead owner, so a genuinely healthy bounded class keeps a
        // ~constant owner COUNT — gating on `count <= first` alone would mislabel it. Requiring
        // zero window inheritance turnover distinguishes a real stall from a stable class.
        if self.owner_share < OWNER_MIN
            || (self.window_inheritance_events == 0 && self.owner_count <= self.first_owner_count)
        {
            return Verdict::SeedClusterOnly;
        }
        if self.owner_cultivated_plot_share < INERT_CULTIVATION_MAX
            || self.owner_grain_to_nonowner_food < INERT_OWNER_GRAIN_MAX
            || self.nonowner_bought_food_share < INERT_BOUGHT_MAX
        {
            return Verdict::TenureInertStaticPin;
        }
        if self.success() {
            Verdict::SecureTenureStableClass
        } else {
            Verdict::NoStableClass
        }
    }

    fn line(&self) -> String {
        format!(
            "seed={} {:?}/{:?} density={} {:?} | owners={:.2} nonowners={:.2} \
             cultivate={:.2} owner_prod={:.2} owner_grain/nonowner_food={:.2} \
             nonowner_bought={:.2} survival={:.2} churn={:.3} gini_delta={:.2} \
             top10={:.2} landless={:.2} stranded={:.2} median_drop={:.2} idle={} \
             secure={} inherit_win={} guards={}",
            self.seed,
            self.scenario,
            self.regime,
            self.density,
            self.verdict(),
            self.owner_share,
            self.non_owner_share,
            self.owner_cultivated_plot_share,
            self.owner_production_share,
            self.owner_grain_to_nonowner_food,
            self.nonowner_bought_food_share,
            self.nonowner_survival_ratio,
            self.non_death_churn_per_generation,
            self.land_gini_delta,
            self.top10_share,
            self.landless_share,
            self.stranded_inherited_share,
            self.median_land_drop,
            self.idle_losses,
            self.secure_tenure_on,
            self.window_inheritance_events,
            self.hard_guards_hold(),
        )
    }
}

fn set_land_plot_counts(cfg: &mut SettlementConfig, total_plots: u16) {
    let good = HEADLINE_GOOD_PLOTS.min(total_plots);
    let chain = cfg.chain.as_mut().expect("chain");
    chain.land_good_plots = good;
    chain.land_marginal_plots = total_plots - good;
}

fn secure_config(regime: InheritanceRegime, total_plots: u16) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_secure_land_tenure();
    set_land_plot_counts(&mut cfg, total_plots);
    let chain = cfg.chain.as_mut().expect("chain");
    chain.inheritance_regime = regime;
    cfg
}

fn s23a_forfeiture_config(total_plots: u16, idle: u16) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_private_land_tenure();
    set_land_plot_counts(&mut cfg, total_plots);
    let chain = cfg.chain.as_mut().expect("chain");
    chain.secure_land_tenure = false;
    chain.forfeit_on_idle = true;
    chain.harvest_gate = true;
    chain.land_idle_limit = idle;
    cfg
}

fn property_off_config(total_plots: u16) -> SettlementConfig {
    let mut cfg = secure_config(InheritanceRegime::Impartible, total_plots);
    let chain = cfg.chain.as_mut().expect("chain");
    chain.private_land_tenure = false;
    chain.secure_land_tenure = false;
    cfg
}

fn non_excludable_deed_config(total_plots: u16) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_private_land_tenure();
    set_land_plot_counts(&mut cfg, total_plots);
    let chain = cfg.chain.as_mut().expect("chain");
    chain.forfeit_on_idle = false;
    chain.harvest_gate = false;
    cfg
}

fn s22f_robustness_config(regime: InheritanceRegime, total_plots: u16) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_voluntary_commitment();
    set_land_plot_counts(&mut cfg, total_plots);
    let chain = cfg.chain.as_mut().expect("chain");
    chain.private_land_tenure = true;
    chain.secure_land_tenure = true;
    chain.harvest_gate = true;
    chain.forfeit_on_idle = false;
    chain.inheritance_regime = regime;
    cfg
}

fn inheritance_probe_config(
    regime: InheritanceRegime,
    max_household_size: u16,
) -> SettlementConfig {
    let mut cfg = secure_config(regime, 1);
    cfg.gatherers = 0;
    cfg.consumers = 0;
    cfg.dynamics.hunger_critical = cfg.dynamics.need_max.saturating_add(1);
    if let Some(chain) = cfg.chain.as_mut() {
        chain.millers = 0;
        chain.bakers = 0;
        chain.latent_millers = 0;
        chain.latent_bakers = 0;
        chain.bread_buffer = 0;
        chain.consumer_staple_buffer = 0;
        chain.latent_flour_seed = 0;
    }
    let demo = cfg
        .demography
        .as_mut()
        .expect("secure land base carries demography");
    demo.households = vec![HouseholdSpec {
        founders: 1,
        time_preference_base_bps: 500,
        food_provision: 0,
        wood_provision: 0,
        starting_gold: 0,
        starting_food: 0,
        starting_wood: 0,
    }];
    demo.spatial_households = true;
    demo.ticks_per_year = 200;
    demo.old_age_onset_years = 1;
    demo.lifespan_span_years = 0;
    demo.birth_interval = 4;
    demo.birth_hunger_ceiling = cfg.dynamics.need_max;
    demo.max_household_size = max_household_size;
    demo.child_food_endowment = 0;
    demo.child_gold_endowment = 0;
    demo.mutation_delta_bps = 0;
    cfg
}

fn run_inheritance_probe(regime: InheritanceRegime, max_household_size: u16) -> Settlement {
    let mut s = Settlement::generate(3, &inheritance_probe_config(regime, max_household_size));
    for tick in 0..260 {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "inheritance probe leaked at tick {tick}"
        );
        assert!(
            s.private_land_registry_invariant_holds(),
            "inheritance probe registry failed at tick {tick}"
        );
        if !s.secure_land_inheritance_events().is_empty() {
            return s;
        }
    }
    panic!("inheritance probe did not produce a secure-title transfer");
}

fn owner_ids(s: &Settlement) -> BTreeSet<u64> {
    s.private_land_owner_ids().into_iter().collect()
}

fn id_to_index(s: &Settlement) -> BTreeMap<u64, usize> {
    (0..s.population())
        .filter_map(|i| s.colonist_id(i).map(|id| (id.0, i)))
        .collect()
}

fn current_grain_by_id(s: &Settlement) -> BTreeMap<u64, u64> {
    (0..s.population())
        .filter_map(|i| {
            s.colonist_id(i)
                .map(|id| (id.0, s.cultivation_grain_harvested_of(i)))
        })
        .collect()
}

fn current_bought_by_id(s: &Settlement) -> BTreeMap<u64, u64> {
    (0..s.population())
        .filter_map(|i| s.colonist_id(i).map(|id| (id.0, s.bought_food_of(i))))
        .collect()
}

fn current_consumed_by_id(s: &Settlement) -> BTreeMap<u64, u64> {
    (0..s.population())
        .filter_map(|i| s.colonist_id(i).map(|id| (id.0, s.consumed_food_of(i))))
        .collect()
}

fn plot_harvest_totals(s: &Settlement) -> BTreeMap<u32, u64> {
    s.private_land_plot_harvest_totals().into_iter().collect()
}

fn capacity_by_owner(s: &Settlement) -> BTreeMap<u64, u32> {
    s.private_land_effective_capacity_by_owner()
        .into_iter()
        .map(|(owner, _, cap)| (owner, cap))
        .collect()
}

fn gini(values: &[u32]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let sum: f64 = sorted.iter().map(|&v| f64::from(v)).sum();
    if sum == 0.0 {
        return 0.0;
    }
    let n = sorted.len() as f64;
    let weighted: f64 = sorted
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64 + 1.0) * f64::from(v))
        .sum();
    (2.0 * weighted) / (n * sum) - (n + 1.0) / n
}

fn top10_share(values: &[u32]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let total: u64 = values.iter().map(|&v| u64::from(v)).sum();
    if total == 0 {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| b.cmp(a));
    let take = sorted.len().div_ceil(10).max(1);
    let top: u64 = sorted.into_iter().take(take).map(u64::from).sum();
    top as f64 / total as f64
}

fn median(values: &mut [u32]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_unstable();
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (f64::from(values[mid - 1]) + f64::from(values[mid])) / 2.0
    } else {
        f64::from(values[mid])
    }
}

fn generation_trace(s: &Settlement, generation: u64, event_start: usize) -> GenerationTrace {
    let owners = owner_ids(s);
    let living_now = living(s).max(1);
    let capacities = capacity_by_owner(s);
    let cap_values: Vec<u32> = capacities.values().copied().collect();
    let land_total: u64 = cap_values.iter().map(|&v| u64::from(v)).sum();
    let landless = (0..s.population())
        .filter(|&i| {
            s.is_alive(i)
                && s.colonist_id(i)
                    .map(|id| !owners.contains(&id.0))
                    .unwrap_or(true)
        })
        .count();
    let events = s.secure_land_inheritance_events();
    let gen_events = events.len().saturating_sub(event_start);
    let heirless = events[event_start..]
        .iter()
        .filter(|event| event.heir.is_none())
        .count();
    let viable_plots = s
        .private_land_plot_summaries()
        .into_iter()
        .filter(|(_, owner, _, regen, cap, _)| {
            owner.is_some() && *regen >= LAND_VIABLE_REGEN_FLOOR && *cap > 0
        })
        .count();
    GenerationTrace {
        generation,
        owner_count: owners.len(),
        owner_share: owners.len() as f64 / living_now as f64,
        land_gini: gini(&cap_values),
        top10_share: top10_share(&cap_values),
        land_per_capita: land_total as f64 / living_now as f64,
        landless_share: landless as f64 / living_now as f64,
        inheritance_events: gen_events,
        heirless_events: heirless,
        viable_plots,
        stranded_shares: s.secure_land_stranded_shares_total(),
        buyer_survival: living_now.saturating_sub(owners.len()),
    }
}

fn landless_rising_two(traces: &[GenerationTrace]) -> bool {
    traces.windows(3).any(|window| {
        window[0].landless_share < window[1].landless_share
            && window[1].landless_share < window[2].landless_share
    })
}

fn run_metrics(
    seed: u64,
    cfg: SettlementConfig,
    regime: InheritanceRegime,
    density: u16,
    scenario: Scenario,
) -> Metrics {
    let mut s = Settlement::generate(seed, &cfg);
    let bread = s.bread_good().expect("S23c base carries bread");
    let mut conserved = true;
    let mut bread_minted_max = 0u64;
    let mut registry_ok = true;
    let mut traces = Vec::new();
    let mut generation_event_start = 0usize;
    let mut generation_plot_start = plot_harvest_totals(&s);
    let mut generation = 0u64;
    let final_start = RUN_TICKS / 2;
    let mut final_owner_share_sum = 0.0;
    let mut final_non_owner_share_sum = 0.0;
    let mut final_non_owner_alive_sum = 0usize;
    let mut final_samples = 0usize;
    let mut final_owned_plot_generations = 0usize;
    let mut final_cultivated_owned_plot_generations = 0usize;
    let mut first_gini = None;
    let mut first_median = None;
    let mut final_grain_start = None;
    let mut final_bought_start = None;
    let mut final_consumed_start = None;

    for tick in 0..RUN_TICKS {
        let report = s.econ_tick();
        conserved &= report.conserves();
        bread_minted_max = bread_minted_max.max(report.endowment_of(bread));
        registry_ok &= s.private_land_registry_invariant_holds();
        assert!(
            s.private_land_registry_invariant_holds(),
            "plot registry invariant failed at seed={seed} scenario={scenario:?} tick={tick}"
        );

        if tick == final_start {
            final_grain_start = Some(current_grain_by_id(&s));
            final_bought_start = Some(current_bought_by_id(&s));
            final_consumed_start = Some(current_consumed_by_id(&s));
        }
        if tick >= final_start {
            let owners = owner_ids(&s);
            let living_now = living(&s).max(1);
            final_owner_share_sum += owners.len() as f64 / living_now as f64;
            final_non_owner_share_sum +=
                living_now.saturating_sub(owners.len()) as f64 / living_now as f64;
            final_non_owner_alive_sum += living_now.saturating_sub(owners.len());
            final_samples += 1;
        }

        if (tick + 1).is_multiple_of(GENERATION_TICKS) || tick + 1 == RUN_TICKS {
            let trace = generation_trace(&s, generation, generation_event_start);
            if tick + 1 >= final_start {
                let current_plot_totals = plot_harvest_totals(&s);
                let owned_nodes: BTreeSet<u32> = s
                    .private_land_plot_summaries()
                    .into_iter()
                    .filter_map(|(node, owner, _, regen, cap, _)| {
                        (owner.is_some() && regen >= LAND_VIABLE_REGEN_FLOOR && cap > 0)
                            .then_some(node)
                    })
                    .collect();
                for node in owned_nodes {
                    final_owned_plot_generations += 1;
                    let before = generation_plot_start.get(&node).copied().unwrap_or(0);
                    let after = current_plot_totals.get(&node).copied().unwrap_or(0);
                    if after > before {
                        final_cultivated_owned_plot_generations += 1;
                    }
                }
                generation_plot_start = current_plot_totals;
            }
            if first_gini.is_none() && trace.owner_count > 0 {
                let mut caps: Vec<u32> = capacity_by_owner(&s).values().copied().collect();
                first_gini = Some(trace.land_gini);
                first_median = Some(median(&mut caps));
            }
            generation_event_start = s.secure_land_inheritance_events().len();
            traces.push(trace);
            generation += 1;
        }
    }

    let owners = owner_ids(&s);
    let living_final = living(&s);
    let owner_share = if final_samples == 0 {
        0.0
    } else {
        final_owner_share_sum / final_samples as f64
    };
    let non_owner_share = if final_samples == 0 {
        0.0
    } else {
        final_non_owner_share_sum / final_samples as f64
    };
    let mean_non_owner_alive = if final_samples == 0 {
        0.0
    } else {
        final_non_owner_alive_sum as f64 / final_samples as f64
    };
    let final_non_owner_alive = living_final.saturating_sub(owners.len()) as f64;
    let nonowner_survival_ratio = if mean_non_owner_alive == 0.0 {
        1.0
    } else {
        final_non_owner_alive / mean_non_owner_alive
    };
    let grain_start = final_grain_start.unwrap_or_default();
    let bought_start = final_bought_start.unwrap_or_default();
    let consumed_start = final_consumed_start.unwrap_or_default();
    let grain_end = current_grain_by_id(&s);
    let bought_end = current_bought_by_id(&s);
    let consumed_end = current_consumed_by_id(&s);
    let index_by_id = id_to_index(&s);

    let owner_grain_window: u64 = owners
        .iter()
        .map(|id| {
            grain_end.get(id).copied().unwrap_or(0) - grain_start.get(id).copied().unwrap_or(0)
        })
        .sum();
    let nonowner_bought_window: u64 = index_by_id
        .iter()
        .filter(|(id, idx)| s.is_alive(**idx) && !owners.contains(id))
        .map(|(id, _)| {
            bought_end.get(id).copied().unwrap_or(0) - bought_start.get(id).copied().unwrap_or(0)
        })
        .sum();
    // The non-owner food-intake denominator for §3.4/§3.5: the food NON-OWNERS actually ate over
    // the window (not the whole colony's consumption), the same non-owner set and window basis the
    // bought-food numerator uses, so both ratios measure a share of non-owner intake.
    let nonowner_consumed_window: u64 = index_by_id
        .iter()
        .filter(|(id, idx)| s.is_alive(**idx) && !owners.contains(id))
        .map(|(id, _)| {
            consumed_end.get(id).copied().unwrap_or(0)
                - consumed_start.get(id).copied().unwrap_or(0)
        })
        .sum();
    // §3.3: at least half the OWNERS themselves produce grain in the window. Measured per owning
    // agent, not per demography household: on this base owners are the standalone cultivator roster
    // with no household, so a household-membership denominator is structurally empty.
    let owner_producers_window = owners
        .iter()
        .filter(|id| {
            grain_end.get(*id).copied().unwrap_or(0) > grain_start.get(*id).copied().unwrap_or(0)
        })
        .count();
    let capacities = capacity_by_owner(&s);
    let cap_values: Vec<u32> = capacities.values().copied().collect();
    let mut median_caps = cap_values.clone();
    let final_median = median(&mut median_caps);
    let first_median = first_median.unwrap_or(final_median);
    let median_land_drop = if first_median <= 0.0 {
        0.0
    } else {
        ((first_median - final_median) / first_median).max(0.0)
    };
    let final_gini = gini(&cap_values);
    let land_gini_delta = (final_gini - first_gini.unwrap_or(final_gini)).max(0.0);
    let inheritance_events = s.secure_land_inheritance_events();
    let window_inheritance_events = inheritance_events
        .iter()
        .filter(|event| event.tick >= final_start)
        .count();
    let stranded_inherited_share = if inheritance_events.is_empty() {
        0.0
    } else {
        s.secure_land_stranded_shares_total() as f64 / inheritance_events.len() as f64
    };
    let landless_share = traces.last().map_or(0.0, |trace| trace.landless_share);
    let non_death_churn_per_generation = if density == 0 {
        0.0
    } else {
        s.private_land_idle_losses_total() as f64
            / f64::from(density)
            / (RUN_TICKS as f64 / GENERATION_TICKS as f64)
    };
    let consumed = s.acquisition_consumed_by_channel();
    let (_, pp_minted) = s.pre_promotion_bread_for_salt_by_provenance();
    let provenance_clean = s.seeded_minted_bread_sold_for_salt() == 0 && pp_minted == 0;
    let owner_cultivated_plot_share = if final_owned_plot_generations == 0 {
        0.0
    } else {
        final_cultivated_owned_plot_generations as f64 / final_owned_plot_generations as f64
    };
    let nonowner_bought_food_share = if nonowner_consumed_window == 0 {
        0.0
    } else {
        nonowner_bought_window as f64 / nonowner_consumed_window as f64
    };
    let owner_grain_to_nonowner_food = if nonowner_consumed_window == 0 {
        0.0
    } else {
        owner_grain_window as f64 / nonowner_consumed_window as f64
    };
    let owner_production_share = if owners.is_empty() {
        0.0
    } else {
        owner_producers_window as f64 / owners.len() as f64
    };
    let food_or_survival_decline =
        nonowner_survival_ratio < SURVIVAL_RATIO_MIN || consumed.bought == 0;

    Metrics {
        seed,
        scenario,
        regime,
        density,
        conserved,
        bread_minted_max,
        provenance_clean,
        promoted: s.current_money_good() == Some(SALT),
        registry_ok,
        extinct: living_final == 0,
        secure_tenure_on: s.secure_land_tenure_on(),
        window_inheritance_events,
        owner_share,
        non_owner_share,
        owner_count: owners.len(),
        first_owner_count: traces.first().map_or(0, |trace| trace.owner_count),
        owner_cultivated_plot_share,
        owner_production_share,
        owner_grain_to_nonowner_food,
        nonowner_bought_food_share,
        nonowner_survival_ratio,
        non_death_churn_per_generation,
        land_gini_delta,
        top10_share: top10_share(&cap_values),
        landless_share,
        landless_rising_two_gens: landless_rising_two(&traces),
        stranded_inherited_share,
        median_land_drop,
        food_or_survival_decline,
        idle_losses: s.private_land_idle_losses_total(),
        traces,
    }
}

fn print_metrics(metrics: &Metrics) {
    eprintln!("S23c {}", metrics.line());
    for trace in &metrics.traces {
        eprintln!("S23c   {}", trace.line());
    }
}

fn assert_guards(metrics: &Metrics) {
    assert!(
        metrics.hard_guards_hold(),
        "hard guard failed: {}",
        metrics.line()
    );
}

#[test]
fn inheritance_regime_headline_axis_prints_verdicts() {
    let mut success_by_density: BTreeMap<(InheritanceRegime, u16), usize> = BTreeMap::new();
    for regime in [InheritanceRegime::Impartible, InheritanceRegime::Partible] {
        for density in PRIMARY_DENSITIES {
            let mut successes = 0usize;
            for seed in BASE_SEEDS {
                let metrics = run_metrics(
                    seed,
                    secure_config(regime, density),
                    regime,
                    density,
                    Scenario::Secure,
                );
                print_metrics(&metrics);
                assert_guards(&metrics);
                successes += usize::from(metrics.verdict() == Verdict::SecureTenureStableClass);
            }
            success_by_density.insert((regime, density), successes);
        }
    }
    eprintln!("S23c primary success counts: {success_by_density:?}");
}

#[test]
fn plot_density_band_and_boundary_probes_are_classified() {
    for density in PRIMARY_DENSITIES.into_iter().chain(BOUNDARY_DENSITIES) {
        for regime in [InheritanceRegime::Impartible, InheritanceRegime::Partible] {
            let metrics = run_metrics(
                BASE_SEEDS[0],
                secure_config(regime, density),
                regime,
                density,
                Scenario::Secure,
            );
            print_metrics(&metrics);
            assert_guards(&metrics);
        }
    }
}

#[test]
fn idle_forfeiture_horizon_sweep_is_reported() {
    for (scenario, cfg) in [
        (
            Scenario::Secure,
            secure_config(InheritanceRegime::Impartible, 24),
        ),
        (
            Scenario::LongForfeiture,
            s23a_forfeiture_config(24, LONG_IDLE_HORIZON),
        ),
        (Scenario::S23aForfeiture, s23a_forfeiture_config(24, 12)),
    ] {
        let metrics = run_metrics(
            BASE_SEEDS[0],
            cfg,
            InheritanceRegime::Impartible,
            24,
            scenario,
        );
        print_metrics(&metrics);
        assert_guards(&metrics);
    }
}

#[test]
fn property_off_and_non_excludable_deed_controls_are_classified() {
    for (scenario, cfg) in [
        (Scenario::PropertyOff, property_off_config(24)),
        (Scenario::NonExcludableDeed, non_excludable_deed_config(24)),
    ] {
        let metrics = run_metrics(
            BASE_SEEDS[0],
            cfg,
            InheritanceRegime::Impartible,
            24,
            scenario,
        );
        print_metrics(&metrics);
        assert_guards(&metrics);
    }
}

#[test]
fn s22f_base_robustness_appendix_is_labeled_control() {
    let metrics = run_metrics(
        BASE_SEEDS[0],
        s22f_robustness_config(InheritanceRegime::Impartible, 24),
        InheritanceRegime::Impartible,
        24,
        Scenario::S22fRobustness,
    );
    print_metrics(&metrics);
    assert_guards(&metrics);
}

#[test]
fn impartible_universal_heir_keeps_one_secure_title() {
    let s = run_inheritance_probe(InheritanceRegime::Impartible, 4);
    let events = s.secure_land_inheritance_events();
    assert_eq!(
        events.len(),
        1,
        "impartible transfer must choose exactly one successor per plot"
    );
    let event = events[0];
    let heir = event
        .heir
        .expect("impartible secure title needs a live heir");
    assert_eq!(event.regime, InheritanceRegime::Impartible);
    assert_eq!(event.pre_regen, event.post_regen);
    assert_eq!(event.pre_cap, event.post_cap);
    assert!(
        s.private_land_owner_ids().contains(&heir),
        "heir must hold the inherited plot"
    );
    assert_eq!(s.private_land_owner_ids().len(), 1);
    assert_eq!(s.secure_land_stranded_shares_total(), 0);
    assert!(s.private_land_registry_invariant_holds());
}

#[test]
fn impartible_heir_order_picks_eldest_household_child() {
    // Positive assertion of the pinned deterministic heir order (§4 steps a/b): on the plot
    // owner's death the successor is the eldest LIVING member of the owner's household (age
    // descending, then stable agent id), preferring a child of the deceased. Recompute the
    // expected successor from the observed roster with the same rule and assert the engine chose
    // exactly it — not merely that "some" household member inherited.
    let s = run_inheritance_probe(InheritanceRegime::Impartible, 6);
    let events = s.secure_land_inheritance_events();
    assert_eq!(
        events.len(),
        1,
        "the single-plot probe transfers exactly one title"
    );
    let event = events[0];
    let deceased = event.deceased;
    let heir = event.heir.expect("a household member must inherit");

    let deceased_index = (0..s.population())
        .find(|&i| s.colonist_id(i).map(|id| id.0) == Some(deceased))
        .expect("the deceased stays in the roster after settlement");
    let deceased_household = s.household_of(deceased_index);

    let order_key = |i: usize| {
        (
            std::cmp::Reverse(s.age_of(i).unwrap_or(0)),
            s.colonist_id(i).map(|id| id.0).unwrap_or(u64::MAX),
        )
    };
    let same_household_member = |i: usize| {
        s.is_alive(i)
            && s.household_of(i) == deceased_household
            && s.colonist_id(i).map(|id| id.0) != Some(deceased)
    };
    let children: Vec<usize> = (0..s.population())
        .filter(|&i| same_household_member(i) && s.parent_of(i).map(|p| p.0) == Some(deceased))
        .collect();
    let candidates: Vec<usize> = if children.is_empty() {
        (0..s.population())
            .filter(|&i| same_household_member(i))
            .collect()
    } else {
        children
    };
    assert!(
        candidates.len() >= 2,
        "the heir-order probe must offer >=2 live candidates so the age/id ordering is exercised"
    );
    let expected = candidates
        .into_iter()
        .min_by_key(|&i| order_key(i))
        .and_then(|i| s.colonist_id(i))
        .expect("a live candidate exists")
        .0;
    assert_eq!(
        heir, expected,
        "heir must be the eldest (age desc, then id asc) household child of the deceased"
    );
    assert!(
        s.private_land_owner_ids().contains(&heir),
        "the chosen heir must hold the inherited plot"
    );
    assert!(s.private_land_registry_invariant_holds());
}

#[test]
fn partition_writeoff_guard_conserves_partible_capacity() {
    let s = run_inheritance_probe(InheritanceRegime::Partible, 16);
    let events = s.secure_land_inheritance_events();
    assert!(
        !events.is_empty(),
        "partible probe must exercise death inheritance"
    );
    assert!(events
        .iter()
        .all(|event| event.regime == InheritanceRegime::Partible));
    assert!(
        events.iter().any(|event| event.post_regen == 0),
        "sub-floor inherited shares must be logged as stranded writeoffs"
    );
    assert!(
        s.secure_land_stranded_shares_total() > 0,
        "stranded partible shares must be counted"
    );
    let pre_regen = events[0].pre_regen;
    let pre_cap = events[0].pre_cap;
    let share_regen: u32 = s
        .private_land_share_summaries()
        .into_iter()
        .map(|(_, _, regen, _, _)| regen)
        .sum();
    let share_cap: u32 = s
        .private_land_share_summaries()
        .into_iter()
        .map(|(_, _, _, cap, _)| cap)
        .sum();
    let stranded_regen: u32 = s
        .private_land_stranded_capacity_summaries()
        .into_iter()
        .map(|(_, regen, _)| regen)
        .sum();
    let stranded_cap: u32 = s
        .private_land_stranded_capacity_summaries()
        .into_iter()
        .map(|(_, _, cap)| cap)
        .sum();
    assert_eq!(
        share_regen + stranded_regen,
        pre_regen,
        "partible regen shares plus stranded capacity must equal pre-death capacity"
    );
    assert_eq!(
        share_cap + stranded_cap,
        pre_cap,
        "partible cap shares plus stranded capacity must equal pre-death capacity"
    );
    assert!(s.private_land_registry_invariant_holds());
}

#[test]
fn canonical_bytes_split_only_when_secure_land_active() {
    let base = Settlement::generate(7, &SettlementConfig::frontier_private_land_tenure());
    let secure = Settlement::generate(7, &SettlementConfig::frontier_secure_land_tenure());
    assert_ne!(base.digest(), secure.digest());

    let mut inert = SettlementConfig::frontier();
    if let Some(chain) = inert.chain.as_mut() {
        chain.secure_land_tenure = true;
    }
    let off = Settlement::generate(7, &SettlementConfig::frontier());
    let inert = Settlement::generate(7, &inert);
    assert_eq!(
        off.digest(),
        inert.digest(),
        "secure_land_tenure must be inert off the S22a substrate"
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
        0x2335_e13c_8097_49fc
    );
    assert_eq!(
        digest(&SettlementConfig::lineages(), 800),
        0x3ffd_78e5_0842_d934
    );
    assert_eq!(
        digest(&SettlementConfig::frontier(), 300),
        0xcc83_bf26_69f0_980d
    );
    assert_eq!(
        digest(&SettlementConfig::frontier_spatial_households(), 300),
        0xf30e_3ce9_345a_73b3
    );

    let mut viable = Settlement::generate(0xC0FFEE, &SettlementConfig::viable());
    viable.run(60);
    assert_eq!(viable.digest(), 0xa174_8567_db1c_4341);
}
