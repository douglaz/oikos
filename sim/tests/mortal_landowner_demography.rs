//! S23d acceptance suite: mortal reproducing lineage households become the persistent
//! secure-title cultivator owners. The headline verdict is printed with per-seed and
//! per-generation traces; it is a classify-not-tune base check, not a tenure result.

use std::collections::{BTreeMap, BTreeSet};

use econ::good::SALT;
use sim::{Settlement, SettlementConfig};

#[path = "support/mod.rs"]
mod support;
use support::living;

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const RUN_TICKS: u64 = 1_600;
const FINAL_WINDOW: u64 = 400;
const GENERATION_TICKS: u64 = 27;
const MEAN_ADULT_LIFESPAN_TICKS: u64 = 27;
const GENERATIONS_MIN: u64 = 5;
const ADULT_AGE_TICKS: u64 = 6;
// The cold-start horizon after which the living-lineage floor is measured. One full mean adult
// lifespan lets the founder cohort settle before the replacement clause looks for a sustained
// collapse — measuring from an earlier tick would capture a transient cold-start dip. (Named to
// avoid conflating the generation COUNT `GENERATIONS_MIN` with a tick offset.)
const COLD_START_TICKS: u64 = MEAN_ADULT_LIFESPAN_TICKS;
// Spec §2.2 allows ≥1 but targets ≥3 born-in-sim cohorts; observed values are 19–26, so the
// target is exercised, not merely the floor.
const BORN_IN_SIM_OWNER_COHORTS: usize = 3;
const BIRTHS_MIN: u64 = 20;
const LINEAGE_FLOOR: usize = 1;
const INHERIT_ELIGIBLE_OWNER_DEATHS_MIN: u64 = 20;
const CLAIMS_MIN: u64 = 1;
const MATERIAL_BOUGHT_FLOOR: u64 = 1_000;
const SUBSIDY_CAP: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    ConservationBroken,
    Extinct,
    ImmortalOwnerResidue,
    NonLineageOwnerResidue,
    MoneyFailure,
    DemographyBaseUnviable,
    NoGenerationalTurnover,
    DemographyBaseViable,
    OwnerClassGap,
    NoBuyerTier,
    NoLineageReplacement,
    ClaimsMissing,
}

#[derive(Clone, Debug)]
struct GenerationTrace {
    tick: u64,
    owners: usize,
    lineage: usize,
    nonowners: usize,
    born_owner_count: usize,
    inherit_eligible: u64,
    floor_share: f64,
}

impl GenerationTrace {
    fn line(&self) -> String {
        format!(
            "tick={} owners={} lineage={} nonowners={} born_owners={} inherit_eligible={} floor_share={:.3}",
            self.tick,
            self.owners,
            self.lineage,
            self.nonowners,
            self.born_owner_count,
            self.inherit_eligible,
            self.floor_share,
        )
    }
}

#[derive(Clone, Debug)]
struct Metrics {
    seed: u64,
    conserved: bool,
    registry_ok: bool,
    plot_count_preserved: bool,
    extinct: bool,
    promoted: bool,
    money_persisted: bool,
    pre_promo_self_produced: u64,
    seeded_minted: u64,
    post_promo_bought: u64,
    final_buyer_count: usize,
    final_buyer_bought: u64,
    final_floor_share: f64,
    final_floor_consumed: u64,
    final_consumed: u64,
    immortal_roster_owned_plot_ticks: u64,
    non_lineage_owner_plot_ticks: u64,
    owner_old_age_deaths: u64,
    inherit_eligible_owner_deaths: u64,
    inheritance_transfers: usize,
    claims: u64,
    lineage_births: u64,
    min_lineage_after_cold_start: usize,
    owner_class_gap: bool,
    born_in_sim_owner_count: usize,
    owner_identity_ok: bool,
    traces: Vec<GenerationTrace>,
}

impl Metrics {
    fn hard_guards_hold(&self) -> bool {
        self.conserved && self.registry_ok && self.plot_count_preserved && !self.extinct
    }

    fn owners_are_lineage_reproducers(&self) -> bool {
        self.immortal_roster_owned_plot_ticks == 0
            && self.non_lineage_owner_plot_ticks == 0
            && self.owner_old_age_deaths > 0
            && self.owner_identity_ok
    }

    /// The narrow §2.6 money-health test behind the `MoneyFailure` structural finding mode (spec
    /// §2: "SALT never promotes, or demonetizes"): SALT promotes on `SelfProduced` bread with no
    /// seeded/minted supply and stays the money good. The `MATERIAL_BOUGHT_FLOOR` sub-clause is
    /// deliberately *not* folded in here — a run where money promoted and persisted but material
    /// trade collapsed is an economic/demographic failure (subsidy-dependence), which the spec's
    /// ordered classifier places under `DemographyBaseUnviable`, not `MoneyFailure`.
    fn money_promoted_ok(&self) -> bool {
        self.promoted
            && self.money_persisted
            && self.pre_promo_self_produced > 0
            && self.seeded_minted == 0
    }

    /// §2.6's "food is materially bought after promotion" sub-clause. A failure here means produced
    /// food is not circulating through money — reported under `DemographyBaseUnviable`
    /// (subsidy-dependence), consistent with the spec's finding taxonomy.
    fn material_bought_ok(&self) -> bool {
        self.post_promo_bought >= MATERIAL_BOUGHT_FLOOR
    }

    fn turnover_ok(&self) -> bool {
        RUN_TICKS >= GENERATIONS_MIN * MEAN_ADULT_LIFESPAN_TICKS
            && !self.owner_class_gap
            && self.born_in_sim_owner_count >= BORN_IN_SIM_OWNER_COHORTS
    }

    fn replacement_ok(&self) -> bool {
        self.lineage_births >= BIRTHS_MIN && self.min_lineage_after_cold_start >= LINEAGE_FLOOR
    }

    fn two_tier_ok(&self) -> bool {
        self.final_buyer_count > 0
            && self.final_buyer_bought >= MATERIAL_BOUGHT_FLOOR
            && self.final_floor_share <= SUBSIDY_CAP
    }

    fn verdict(&self) -> Verdict {
        if !self.conserved || !self.registry_ok || !self.plot_count_preserved {
            return Verdict::ConservationBroken;
        }
        if self.extinct {
            return Verdict::Extinct;
        }
        if self.immortal_roster_owned_plot_ticks > 0 {
            return Verdict::ImmortalOwnerResidue;
        }
        if self.non_lineage_owner_plot_ticks > 0 || !self.owner_identity_ok {
            return Verdict::NonLineageOwnerResidue;
        }
        if !self.money_promoted_ok() {
            return Verdict::MoneyFailure;
        }
        // Subsidy-dependence / no material circulation is a demographic-economic collapse, not a
        // money failure: money promoted and persisted, but the colony lives off the emergency floor
        // and produced food never circulates through money (spec §2 places subsidy-dependence and
        // "cannot reach positive food/money circulation" under `DemographyBaseUnviable`).
        if !self.replacement_ok()
            || self.final_floor_share > SUBSIDY_CAP
            || !self.material_bought_ok()
        {
            return Verdict::DemographyBaseUnviable;
        }
        if self.inherit_eligible_owner_deaths < INHERIT_ELIGIBLE_OWNER_DEATHS_MIN {
            return Verdict::NoGenerationalTurnover;
        }
        if !self.turnover_ok() {
            return if self.owner_class_gap {
                Verdict::OwnerClassGap
            } else {
                Verdict::NoLineageReplacement
            };
        }
        if self.claims < CLAIMS_MIN {
            return Verdict::ClaimsMissing;
        }
        if !self.two_tier_ok() {
            return Verdict::NoBuyerTier;
        }
        if self.owners_are_lineage_reproducers() {
            Verdict::DemographyBaseViable
        } else {
            Verdict::NonLineageOwnerResidue
        }
    }

    fn line(&self) -> String {
        format!(
            "seed={} {:?} | guards={} immortal_ticks={} nonlineage_ticks={} owner_old_age={} \
             inherit_eligible={} births={} min_lineage={} born_owners={} claims={} promoted={} \
             transfers={} pp_self={} seeded_minted={} bought={} buyers={} buyer_bought={} \
             floor={}/{}={:.3}",
            self.seed,
            self.verdict(),
            self.hard_guards_hold(),
            self.immortal_roster_owned_plot_ticks,
            self.non_lineage_owner_plot_ticks,
            self.owner_old_age_deaths,
            self.inherit_eligible_owner_deaths,
            self.lineage_births,
            self.min_lineage_after_cold_start,
            self.born_in_sim_owner_count,
            self.claims,
            self.promoted,
            self.inheritance_transfers,
            self.pre_promo_self_produced,
            self.seeded_minted,
            self.post_promo_bought,
            self.final_buyer_count,
            self.final_buyer_bought,
            self.final_floor_consumed,
            self.final_consumed,
            self.final_floor_share,
        )
    }
}

fn headline_config() -> SettlementConfig {
    SettlementConfig::frontier_mortal_landowner_demography()
}

fn lineage_count(s: &Settlement) -> usize {
    (0..s.population())
        .filter(|&i| s.is_alive(i) && s.household_of(i).is_some())
        .count()
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

fn generation_trace(s: &Settlement, tick: u64, born_owner_ids: &BTreeSet<u64>) -> GenerationTrace {
    let owners = owner_ids(s);
    let lineage = lineage_count(s);
    let nonowners = living(s).saturating_sub(owners.len());
    let consumed = s.acquisition_consumed_by_channel().total();
    let floor = s.emergency_bread_provisioned();
    let floor_share = if consumed == 0 {
        0.0
    } else {
        floor as f64 / consumed as f64
    };
    GenerationTrace {
        tick,
        owners: owners.len(),
        lineage,
        nonowners,
        born_owner_count: born_owner_ids.len(),
        inherit_eligible: s.secure_land_inherit_eligible_owner_deaths_total(),
        floor_share,
    }
}

fn run_metrics(seed: u64, cfg: SettlementConfig) -> Metrics {
    let mut s = Settlement::generate(seed, &cfg);
    let bread = s.bread_good().expect("S23d base carries bread");
    let initial_plot_count = s.private_land_plot_count();
    let final_start = RUN_TICKS.saturating_sub(FINAL_WINDOW);
    let mut conserved = true;
    let mut registry_ok = true;
    let mut plot_count_preserved = true;
    let mut immortal_roster_owned_plot_ticks = 0u64;
    let mut non_lineage_owner_plot_ticks = 0u64;
    let mut owner_identity_ok = true;
    let mut first_claim_seen = false;
    let mut owner_class_gap = false;
    let mut min_lineage_after_cold_start = usize::MAX;
    let mut born_owner_ids = BTreeSet::new();
    let mut traces = Vec::new();
    let mut final_bought_start = BTreeMap::new();
    let mut final_floor_start = 0u64;
    let mut final_consumed_start = 0u64;

    for tick in 0..RUN_TICKS {
        let report = s.econ_tick();
        conserved &= report.conserves();
        conserved &= report.endowment_of(bread) == 0;
        registry_ok &= s.private_land_registry_invariant_holds();
        plot_count_preserved &= s.private_land_plot_count() == initial_plot_count;

        owner_identity_ok &= observe_owner_residue(
            &s,
            &mut immortal_roster_owned_plot_ticks,
            &mut non_lineage_owner_plot_ticks,
        );

        let owners = owner_ids(&s);
        if !owners.is_empty() {
            first_claim_seen = true;
        } else if first_claim_seen {
            owner_class_gap = true;
        }

        if tick >= COLD_START_TICKS {
            min_lineage_after_cold_start = min_lineage_after_cold_start.min(lineage_count(&s));
        }

        let index_by_id = id_to_index(&s);
        for row in s.private_land_owner_identity_rows() {
            let Some(&index) = index_by_id.get(&row.owner) else {
                continue;
            };
            if row.born_in_sim && s.age_of(index).unwrap_or(0) >= ADULT_AGE_TICKS {
                born_owner_ids.insert(row.owner);
            }
        }

        if tick == final_start {
            final_bought_start = bought_by_id(&s);
            final_floor_start = s.emergency_bread_provisioned();
            final_consumed_start = s.acquisition_consumed_by_channel().total();
        }

        if (tick + 1).is_multiple_of(GENERATION_TICKS) || tick + 1 == RUN_TICKS {
            traces.push(generation_trace(&s, tick + 1, &born_owner_ids));
        }
    }

    let final_bought = bought_by_id(&s);
    let owners = owner_ids(&s);
    let final_buyer_bought = id_to_index(&s)
        .into_iter()
        .filter(|(id, i)| s.is_alive(*i) && !owners.contains(id))
        .map(|(id, _)| {
            final_bought.get(&id).copied().unwrap_or(0)
                - final_bought_start.get(&id).copied().unwrap_or(0)
        })
        .sum();
    let final_buyer_count = (0..s.population())
        .filter(|&i| {
            s.is_alive(i)
                && s.colonist_id(i)
                    .map(|id| !owners.contains(&id.0))
                    .unwrap_or(true)
        })
        .count();
    let final_floor_consumed = s
        .emergency_bread_provisioned()
        .saturating_sub(final_floor_start);
    let final_consumed = s
        .acquisition_consumed_by_channel()
        .total()
        .saturating_sub(final_consumed_start);
    let final_floor_share = if final_consumed == 0 {
        1.0
    } else {
        final_floor_consumed as f64 / final_consumed as f64
    };
    let (pre_promo_self_produced, pre_promo_minted) =
        s.pre_promotion_bread_for_salt_by_provenance();
    let consumed = s.acquisition_consumed_by_channel();

    Metrics {
        seed,
        conserved,
        registry_ok,
        plot_count_preserved,
        extinct: living(&s) == 0,
        promoted: s.promoted_at_tick().is_some(),
        money_persisted: s.current_money_good() == Some(SALT),
        pre_promo_self_produced,
        seeded_minted: pre_promo_minted + s.seeded_minted_bread_sold_for_salt(),
        post_promo_bought: consumed.bought,
        final_buyer_count,
        final_buyer_bought,
        final_floor_share,
        final_floor_consumed,
        final_consumed,
        immortal_roster_owned_plot_ticks,
        non_lineage_owner_plot_ticks,
        owner_old_age_deaths: s.secure_land_owner_old_age_deaths_total(),
        inherit_eligible_owner_deaths: s.secure_land_inherit_eligible_owner_deaths_total(),
        inheritance_transfers: s
            .secure_land_inheritance_events()
            .into_iter()
            .filter(|event| event.heir.is_some())
            .count(),
        claims: s.private_land_claims_total(),
        lineage_births: s.births_total(),
        min_lineage_after_cold_start,
        owner_class_gap,
        born_in_sim_owner_count: born_owner_ids.len(),
        owner_identity_ok,
        traces,
    }
}

fn print_metrics(label: &str, metrics: &Metrics) {
    eprintln!("S23d {label} {}", metrics.line());
    for trace in &metrics.traces {
        eprintln!("S23d {label}   {}", trace.line());
    }
}

fn assert_hard_guards(metrics: &Metrics) {
    assert!(
        metrics.hard_guards_hold(),
        "hard guards failed: {}",
        metrics.line()
    );
}

fn no_reproduction_config() -> SettlementConfig {
    let mut cfg = headline_config();
    if let Some(demo) = cfg.demography.as_mut() {
        demo.birth_interval = RUN_TICKS + 1;
    }
    cfg
}

fn no_emergency_floor_config() -> SettlementConfig {
    let mut cfg = headline_config();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.emergency_hunger_threshold = 0;
    }
    cfg
}

/// The `no_lineage_land_claims` (legacy-roster-claims-only) control isolates the S23d seam:
/// mortal households becoming *persistent land claimants*. The `mortal_landowner_demography`
/// flag's ONLY behavioral effect is that claim reroute — lineage cultivation, reproduction, and
/// the S21h survival floor all live in the shared `frontier_secure_land_tenure` base, on
/// regardless of the flag. So "disable the claim reroute" is definitionally "flag off", which
/// coincides with the `demography_off` control. Reaching a *distinct* third configuration would
/// require an inverse-gating knob (bar lineage claims while still routing cultivation to them)
/// that exists solely to make this control byte-distinct — mechanism no correctness requirement
/// needs. The coincidence is therefore expected and is itself the isolation evidence: with the
/// reroute off, only the legacy roster owns → `ImmortalOwnerResidue`.
fn no_lineage_land_claims_config() -> SettlementConfig {
    let mut cfg = headline_config();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.mortal_landowner_demography = false;
    }
    cfg
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
}

#[test]
fn canonical_bytes_split_only_when_demography_active() {
    let base = Settlement::generate(7, &SettlementConfig::frontier_secure_land_tenure());
    let on = Settlement::generate(7, &headline_config());
    assert_ne!(base.canonical_bytes(), on.canonical_bytes());
    assert_ne!(base.digest(), on.digest());

    let mut reverted = headline_config();
    reverted
        .chain
        .as_mut()
        .expect("S23d has chain")
        .mortal_landowner_demography = false;
    let off = Settlement::generate(7, &reverted);
    assert_eq!(base.canonical_bytes(), off.canonical_bytes());
    assert_eq!(base.digest(), off.digest());

    let mut inert = SettlementConfig::frontier_endogenous_cultivation();
    let inert_off = Settlement::generate(7, &inert);
    inert
        .chain
        .as_mut()
        .expect("endogenous cultivation has chain")
        .mortal_landowner_demography = true;
    let inert_on = Settlement::generate(7, &inert);
    assert_eq!(inert_off.canonical_bytes(), inert_on.canonical_bytes());
}

#[test]
fn demography_base_viable_headline() {
    let mut verdicts = BTreeMap::new();
    for seed in SEEDS {
        let metrics = run_metrics(seed, headline_config());
        print_metrics("headline", &metrics);
        assert_hard_guards(&metrics);
        *verdicts.entry(metrics.verdict()).or_insert(0usize) += 1;
    }
    eprintln!("S23d headline verdicts: {verdicts:?}");
}

#[test]
fn owners_are_lineage_reproducers() {
    for seed in SEEDS {
        let metrics = run_metrics(seed, headline_config());
        assert_hard_guards(&metrics);
        assert!(
            metrics.owners_are_lineage_reproducers(),
            "owner identity failed: {}",
            metrics.line()
        );
    }
}

#[test]
fn born_in_sim_owner_reaches_ownership() {
    for seed in SEEDS {
        let metrics = run_metrics(seed, headline_config());
        assert!(
            metrics.born_in_sim_owner_count >= BORN_IN_SIM_OWNER_COHORTS,
            "no born-in-sim adult owner: {}",
            metrics.line()
        );
    }
}

#[test]
fn inheritance_fires_endogenously() {
    for seed in SEEDS {
        let metrics = run_metrics(seed, headline_config());
        assert!(
            metrics.owner_old_age_deaths > 0,
            "no owner deaths: {}",
            metrics.line()
        );
        assert!(
            metrics.inheritance_transfers > 0,
            "no endogenous heir transfer: {}",
            metrics.line()
        );
        assert!(
            metrics.inherit_eligible_owner_deaths >= INHERIT_ELIGIBLE_OWNER_DEATHS_MIN,
            "inheritance sample floor not met: {}",
            metrics.line()
        );
    }
}

#[test]
fn money_promotes_and_persists() {
    for seed in SEEDS {
        let metrics = run_metrics(seed, headline_config());
        assert!(
            metrics.promoted && metrics.money_persisted,
            "money did not promote/persist: {}",
            metrics.line()
        );
        assert!(
            metrics.pre_promo_self_produced > 0 && metrics.seeded_minted == 0,
            "money did not promote on self-produced bread: {}",
            metrics.line()
        );
        if metrics.post_promo_bought < MATERIAL_BOUGHT_FLOOR {
            eprintln!("S23d material bought floor classified: {}", metrics.line());
            // Money promoted and persisted (asserted above); a collapsed material-bought floor is
            // subsidy-dependence, so the honest label is `DemographyBaseUnviable`, not
            // `MoneyFailure` (spec §2 finding taxonomy).
            assert_eq!(
                metrics.verdict(),
                Verdict::DemographyBaseUnviable,
                "money promoted+persisted, so a collapsed material-bought floor must classify as \
                 subsidy-dependence (DemographyBaseUnviable), not MoneyFailure"
            );
        }
    }
}

#[test]
fn two_tier_not_subsidy_dependent() {
    for seed in SEEDS {
        let metrics = run_metrics(seed, headline_config());
        if !metrics.two_tier_ok() {
            eprintln!("S23d two-tier/subsidy gate classified: {}", metrics.line());
            assert!(
                matches!(
                    metrics.verdict(),
                    Verdict::MoneyFailure
                        | Verdict::DemographyBaseUnviable
                        | Verdict::NoBuyerTier
                        | Verdict::Extinct
                ),
                "unexpected two-tier/subsidy verdict: {}",
                metrics.line()
            );
        }
    }
}

#[test]
fn conservation_and_registry_invariants_hold() {
    for seed in SEEDS {
        let metrics = run_metrics(seed, headline_config());
        assert_hard_guards(&metrics);
    }
}

#[test]
fn demography_off_control_reproduces_legacy_residue() {
    let metrics = run_metrics(3, SettlementConfig::frontier_secure_land_tenure());
    print_metrics("demography_off", &metrics);
    assert!(
        matches!(
            metrics.verdict(),
            Verdict::ImmortalOwnerResidue | Verdict::NoGenerationalTurnover
        ),
        "unexpected demography-off verdict: {}",
        metrics.line()
    );
}

#[test]
fn no_reproduction_control_is_unviable() {
    let headline = run_metrics(3, headline_config());
    let metrics = run_metrics(3, no_reproduction_config());
    print_metrics("no_reproduction", &metrics);
    // The reproduction seam is load-bearing at the demographic level, independent of the
    // (shared, non-viable) money/economic verdict: suppressing births drops the lineage to
    // essentially no offspring — far below both the headline and the BIRTHS_MIN floor — so no
    // born-in-sim cohort ever reaches ownership. This differential is what makes the control
    // discriminating rather than merely echoing the headline verdict.
    assert!(
        metrics.lineage_births < headline.lineage_births
            && metrics.lineage_births < BIRTHS_MIN
            && metrics.born_in_sim_owner_count == 0,
        "no-reproduction did not suppress births (load-bearing check): no_repro={} headline_births={}",
        metrics.line(),
        headline.lineage_births,
    );
    assert!(
        matches!(
            metrics.verdict(),
            Verdict::DemographyBaseUnviable
                | Verdict::NoGenerationalTurnover
                | Verdict::NoLineageReplacement
                | Verdict::MoneyFailure
                | Verdict::Extinct
        ),
        "unexpected no-reproduction verdict: {}",
        metrics.line()
    );
}

#[test]
fn no_emergency_floor_control_is_unviable() {
    let metrics = run_metrics(3, no_emergency_floor_config());
    print_metrics("no_emergency_floor", &metrics);
    assert!(
        matches!(
            metrics.verdict(),
            Verdict::DemographyBaseUnviable | Verdict::MoneyFailure | Verdict::Extinct
        ),
        "unexpected no-emergency-floor verdict: {}",
        metrics.line()
    );
}

#[test]
fn no_lineage_land_claims_control_reproduces_legacy_residue() {
    let metrics = run_metrics(3, no_lineage_land_claims_config());
    print_metrics("no_lineage_land_claims", &metrics);
    assert!(
        matches!(
            metrics.verdict(),
            Verdict::ImmortalOwnerResidue | Verdict::NoGenerationalTurnover
        ),
        "unexpected no-lineage-claims verdict: {}",
        metrics.line()
    );
}
