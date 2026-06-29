//! S23a acceptance suite — **private land tenure**: scarce, heterogeneous, excludable, losable
//! grain plots over the expanded S22a endogenous-cultivation base.
//!
//! The verdict is a classifier, not a tuner. Run with `--nocapture` to read it:
//!   `cargo test -p sim --test private_land_tenure private_land_verdict -- --nocapture`

use std::collections::{BTreeMap, BTreeSet};

use econ::good::SALT;
use sim::{Settlement, SettlementConfig, LAND_VIABLE_CAP_FLOOR, LAND_VIABLE_REGEN_FLOOR};

#[path = "support/mod.rs"]
mod support;
use support::*;

const CHURN_DROP: f64 = 0.5;
const PERSIST_FRACTION: f64 = 0.5;
const PERSIST_COHORT: usize = 4;
const ROSTER_HOUSEHOLDS: usize = 8;
const OWNER_SHARE_MAX: f64 = 0.6;
const MONO_SHARE_BPS: u64 = 7_500;
const ROLLING_WINDOW: usize = 100;
const FINAL_WINDOW: usize = 200;
const MATERIAL_BOUGHT_FLOOR: u64 = 4;
const MIN_BUYER_COHORT: usize = 2;
// `CommonsEquivalent` is a GENUINE-inertness verdict (tenure on, but behaving like the commons), NOT a
// synonym for "stickiness failed". It fires only when churn is within `COMMONS_EQUIV_CHURN_FACTOR` of the
// matched commons baseline AND ownership did not concentrate (owner share <= `COMMONS_EQUIV_OWNER_SHARE_MAX`).
// A cell where churn explodes far above commons (use-it-or-lose-it thrash) is `NoStickinessDespiteLand`.
const COMMONS_EQUIV_CHURN_FACTOR: f64 = 1.5;
const COMMONS_EQUIV_OWNER_SHARE_MAX: f64 = 0.2;
// Shared with the engine (`sim::LAND_VIABLE_*`) so the §2a floors cannot silently drift apart.
const VIABLE_REGEN_FLOOR: u32 = LAND_VIABLE_REGEN_FLOOR;
const VIABLE_CAP_FLOOR: u32 = LAND_VIABLE_CAP_FLOOR;
const S23_TICKS: u64 = 300;
const S23_SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const CONTROL_SEED: u64 = 3;
const SWEEP_TICKS: u64 = 20;
const HEADLINE_TOTAL_PLOTS: u16 = 48;
const HEADLINE_GOOD_PLOTS: u16 = 4;
const TOTAL_PLOTS_SWEEP: [u16; 4] = [12, 24, 48, 96];
const IDLE_SWEEP: [u16; 4] = [6, 12, 24, 48];
const MARGINAL_REGEN_SWEEP: [u32; 3] = [6, 12, 24];
const GOOD_PLOTS_SWEEP: [u16; 4] = [2, 4, 6, 16];
const CAPACITY_SWEEP_SEEDS: [u64; 1] = [3];

fn persist_threshold() -> u32 {
    (PERSIST_FRACTION * FINAL_WINDOW as f64).ceil() as u32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    TenureLeverInert,
    ConservationBroken,
    HardBarrier,
    LandMonopolyCull,
    MoneyFailureFromTenure,
    CommonsEquivalent,
    LandTenureStickySuccess,
    NoStickinessDespiteLand,
}

#[derive(Clone, Debug)]
struct Metrics {
    seed: u64,
    conserved: bool,
    bread_minted_max: u64,
    extinct: bool,
    registry_ok: bool,
    provenance_clean: bool,
    promoted: bool,
    claims: u64,
    denials: u64,
    owner_gate_denials: u64,
    nonowner_harvest_of_owned: u64,
    idle_losses: u64,
    reclaims_by_other: u64,
    marginal_nonowner_claims: u64,
    lapsed_reentry_worse: u64,
    viable_marginal_min_final: usize,
    viable_marginal_final: usize,
    churn_total: u32,
    ever_cultivating: usize,
    persistent_owner_cultivators: usize,
    owner_share: f64,
    owner_grain_share_bps: u64,
    final_buyer_cohort: usize,
    post_promotion_bought: u64,
    max_rolling_cultivator_share: f64,
    living: usize,
}

impl Metrics {
    fn hard_guards_hold(&self) -> bool {
        self.conserved
            && self.bread_minted_max == 0
            && !self.extinct
            && self.registry_ok
            && self.provenance_clean
    }

    fn non_vacuous(&self) -> bool {
        // The mechanism must have *bitten*: plots claimed by homesteading, lost on idle and
        // re-homesteaded by a DIFFERENT agent (`reclaims_by_other` is now idle-cause-only), a real
        // spatial-hysteresis re-entrant, AND harvest is owner-EXCLUSIVE — no non-owner ever pulled
        // grain from a held plot (`nonowner_harvest_of_owned == 0`). The owner-exclusive property is
        // the faithful single-run proof the gate works; the `non_excludable_deed` control flips it
        // (> 0), and that pair is asserted in `mandatory_non_vacuity`. (`owner_gate_denials` — a
        // reroute OFF an owned plot — is ~always 0 by design: §3.5(b)'s reservation resolves all
        // contention while plots are still unowned, so it cannot be the non-vacuity signal.)
        self.claims > 0
            && self.idle_losses > 0
            && self.reclaims_by_other > 0
            && self.lapsed_reentry_worse > 0
            && self.nonowner_harvest_of_owned == 0
    }

    fn churn_per_capita(&self) -> f64 {
        if self.ever_cultivating == 0 {
            0.0
        } else {
            f64::from(self.churn_total) / self.ever_cultivating as f64
        }
    }

    fn success(&self, baseline_churn: f64, controls_not_sticky: bool) -> bool {
        self.non_vacuous()
            && self.churn_per_capita() <= CHURN_DROP * baseline_churn
            && self.persistent_owner_cultivators >= PERSIST_COHORT
            && self.owner_share <= OWNER_SHARE_MAX
            && self.viable_marginal_min_final >= 1
            && self.marginal_nonowner_claims >= 1
            && self.lapsed_reentry_worse >= 1
            && self.final_buyer_cohort >= MIN_BUYER_COHORT
            && self.post_promotion_bought >= MATERIAL_BOUGHT_FLOOR
            && self.promoted
            && self.hard_guards_hold()
            && controls_not_sticky
    }

    fn verdict(&self, baseline_churn: f64, controls_not_sticky: bool) -> Verdict {
        if !self.non_vacuous() {
            return Verdict::TenureLeverInert;
        }
        if !self.hard_guards_hold() {
            return Verdict::ConservationBroken;
        }
        // §2a open entry = (viable marginal land through the final window) ∧ (≥1 observed
        // non-owner claim). `HardBarrier` is its De Morgan negation — NO viable marginal plot
        // **OR** no non-owner ever entered — so this MUST be `||`. With `&&` it only fired when
        // entry was never even attempted, mislabeling a closed-entry world as `CommonsEquivalent`.
        if self.viable_marginal_min_final == 0 || self.marginal_nonowner_claims == 0 {
            return Verdict::HardBarrier;
        }
        if self.viable_marginal_final > 0
            && self.owner_grain_share_bps >= MONO_SHARE_BPS
            && self.final_buyer_cohort < MIN_BUYER_COHORT
        {
            return Verdict::LandMonopolyCull;
        }
        if !self.promoted {
            return Verdict::MoneyFailureFromTenure;
        }
        // GENUINE commons-equivalence (Codex review-of-results P1 fix): tenure is on + non-vacuous, yet it
        // behaves like the commons — churn ~ the matched commons baseline AND ownership did not concentrate.
        // This is NOT the "churn did not drop" case: a thrash regime (churn ~10x commons, owner share ~0.80)
        // is exclusion biting hard, which falls through to `NoStickinessDespiteLand` below.
        if self.churn_per_capita() <= COMMONS_EQUIV_CHURN_FACTOR * baseline_churn
            && self.owner_share <= COMMONS_EQUIV_OWNER_SHARE_MAX
        {
            return Verdict::CommonsEquivalent;
        }
        if self.success(baseline_churn, controls_not_sticky) {
            Verdict::LandTenureStickySuccess
        } else {
            Verdict::NoStickinessDespiteLand
        }
    }

    fn verdict_needs_baseline(&self) -> bool {
        self.non_vacuous()
            && self.hard_guards_hold()
            && self.viable_marginal_min_final > 0
            && self.marginal_nonowner_claims > 0
            && !(self.viable_marginal_final > 0
                && self.owner_grain_share_bps >= MONO_SHARE_BPS
                && self.final_buyer_cohort < MIN_BUYER_COHORT)
            && self.promoted
    }

    fn line(&self, baseline_churn: f64, controls_not_sticky: bool) -> String {
        format!(
            "seed={} {:?} | claims={} deny={} owner_deny={} nonowner_harv_owned={} lost={} \
             re_other={} lapse_worse={} viable_final={} viable_min={} churn={:.2}/{:.2} \
             owners={:.2} owner_grain_bps={} persist_owner={} buyers={} bought={} roll_max={:.2} \
             living={} promoted={} guards={}",
            self.seed,
            self.verdict(baseline_churn, controls_not_sticky),
            self.claims,
            self.denials,
            self.owner_gate_denials,
            self.nonowner_harvest_of_owned,
            self.idle_losses,
            self.reclaims_by_other,
            self.lapsed_reentry_worse,
            self.viable_marginal_final,
            self.viable_marginal_min_final,
            self.churn_per_capita(),
            baseline_churn,
            self.owner_share,
            self.owner_grain_share_bps,
            self.persistent_owner_cultivators,
            self.final_buyer_cohort,
            self.post_promotion_bought,
            self.max_rolling_cultivator_share,
            self.living,
            self.promoted,
            self.hard_guards_hold(),
        )
    }
}

fn property_off_baseline() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_private_land_tenure();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.private_land_tenure = false;
    }
    cfg
}

fn control_non_excludable() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_private_land_tenure();
    cfg.chain.as_mut().unwrap().harvest_gate = false;
    cfg
}

fn matched_non_excludable(mut cfg: SettlementConfig) -> SettlementConfig {
    cfg.chain.as_mut().unwrap().harvest_gate = false;
    cfg
}

fn control_free_reclaim() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_private_land_tenure();
    cfg.chain.as_mut().unwrap().reclaim_reserved_for_prior_owner = true;
    cfg
}

fn control_abundant_good_land() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_private_land_tenure();
    set_land_plot_counts(&mut cfg, HEADLINE_TOTAL_PLOTS, 16);
    cfg
}

fn control_no_forfeit() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_private_land_tenure();
    cfg.chain.as_mut().unwrap().forfeit_on_idle = false;
    cfg
}

fn set_land_plot_counts(cfg: &mut SettlementConfig, total_plots: u16, good_plots: u16) {
    assert!(
        good_plots <= total_plots,
        "good plots cannot exceed total plots"
    );
    let chain = cfg.chain.as_mut().unwrap();
    chain.land_good_plots = good_plots;
    chain.land_marginal_plots = total_plots - good_plots;
}

fn private_land_axis_config(
    total_plots: u16,
    good_plots: u16,
    idle_limit: u16,
    marginal_regen: u32,
) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_private_land_tenure();
    set_land_plot_counts(&mut cfg, total_plots, good_plots);
    let chain = cfg.chain.as_mut().unwrap();
    chain.land_idle_limit = idle_limit;
    chain.land_marginal_regen = marginal_regen;
    cfg
}

fn baseline_churn_for(seed: u64, ticks: u64, cfg: &SettlementConfig) -> f64 {
    // The matched-commons churn denominator must hold the SAME plot gradient and isolate
    // exclusion as the only difference. `property_off` does NOT: flipping `private_land_tenure`
    // off drops the whole layout (the world reverts to the single scaled grain commons), so its
    // churn conflates "carving the commons into scarce plots" with "ownership". `non_excludable_deed`
    // keeps the identical gradient geometry+supply but never gates harvest (each agent still spreads
    // to its nearest stocked plot), so it is the honest commons-over-the-gradient baseline.
    run_metrics(seed, matched_non_excludable(cfg.clone()), ticks, false).churn_per_capita()
}

fn run_metrics(seed: u64, cfg: SettlementConfig, ticks: u64, require_land_guards: bool) -> Metrics {
    let mut s = Settlement::generate(seed, &cfg);
    assert_eq!(s.household_count(), ROSTER_HOUSEHOLDS);
    let bread = s.bread_good().expect("S23a base carries bread");
    let mut conserved = true;
    let mut bread_minted_max = 0u64;
    let mut prev_cultivating: BTreeMap<u64, bool> = BTreeMap::new();
    let mut churn_total = 0u32;
    let mut ever_cultivating = BTreeSet::new();
    let mut final_owner_cultivator_ticks: BTreeMap<u64, u32> = BTreeMap::new();
    let mut rolling: Vec<bool> = Vec::new();
    let mut max_rolling_cultivator_share: f64 = 0.0;
    let mut viable_marginal_min_final = usize::MAX;
    let final_start = ticks.saturating_sub(FINAL_WINDOW as u64);

    for tick in 0..ticks {
        let report = s.econ_tick();
        conserved &= report.conserves();
        bread_minted_max = bread_minted_max.max(report.endowment_of(bread));
        if require_land_guards {
            assert!(
                s.private_land_registry_invariant_holds(),
                "private land registry invariant failed at tick {tick}"
            );
        }

        let owners: BTreeSet<u64> = s.private_land_owner_ids().into_iter().collect();
        let mut cultivating_now = 0usize;
        for i in 0..s.population() {
            let Some(id) = s.colonist_id(i).map(|id| id.0) else {
                continue;
            };
            let now = s.is_alive(i) && s.is_cultivating(i);
            if now {
                cultivating_now += 1;
                ever_cultivating.insert(id);
            }
            if let Some(prev) = prev_cultivating.insert(id, now) {
                if prev != now && (prev || now) {
                    churn_total += 1;
                }
            }
            if tick >= final_start && now && owners.contains(&id) {
                *final_owner_cultivator_ticks.entry(id).or_insert(0) += 1;
            }
        }
        let living_now = living(&s).max(1);
        rolling.push(cultivating_now > 0);
        if rolling.len() > ROLLING_WINDOW {
            rolling.remove(0);
        }
        let share = cultivating_now as f64 / living_now as f64;
        max_rolling_cultivator_share = max_rolling_cultivator_share.max(share);
        if tick >= final_start {
            viable_marginal_min_final =
                viable_marginal_min_final.min(s.private_land_viable_marginal_plots());
        }
    }

    let owners: BTreeSet<u64> = s.private_land_owner_ids().into_iter().collect();
    let living_final = living(&s);
    let final_buyer_cohort = (0..s.population())
        .filter(|&i| {
            s.is_alive(i)
                && !s.is_cultivating(i)
                && s.colonist_id(i)
                    .map(|id| !owners.contains(&id.0))
                    .unwrap_or(false)
                && s.bought_food_of(i) >= MATERIAL_BOUGHT_FLOOR
        })
        .count();
    let consumed = s.acquisition_consumed_by_channel();
    let (_, pp_minted) = s.pre_promotion_bread_for_salt_by_provenance();
    let provenance_clean = s.seeded_minted_bread_sold_for_salt() == 0 && pp_minted == 0;
    let persistent_owner_cultivators = final_owner_cultivator_ticks
        .values()
        .filter(|&&ticks| ticks >= persist_threshold())
        .count();

    Metrics {
        seed,
        conserved,
        bread_minted_max,
        extinct: living_final == 0,
        registry_ok: s.private_land_registry_invariant_holds(),
        provenance_clean,
        promoted: s.current_money_good() == Some(SALT),
        claims: s.private_land_claims_total(),
        denials: s.private_land_harvest_denials_total(),
        owner_gate_denials: s.private_land_owner_gate_denials_total(),
        nonowner_harvest_of_owned: s.private_land_nonowner_harvest_of_owned_total(),
        idle_losses: s.private_land_idle_losses_total(),
        reclaims_by_other: s.private_land_reclaims_by_other_total(),
        marginal_nonowner_claims: s.private_land_marginal_nonowner_claims_total(),
        lapsed_reentry_worse: s.private_land_lapsed_reentry_worse_total(),
        viable_marginal_min_final: viable_marginal_min_final
            .min(s.private_land_viable_marginal_plots()),
        viable_marginal_final: s.private_land_viable_marginal_plots(),
        churn_total,
        ever_cultivating: ever_cultivating.len(),
        persistent_owner_cultivators,
        owner_share: if living_final == 0 {
            0.0
        } else {
            owners.len() as f64 / living_final as f64
        },
        owner_grain_share_bps: s.private_land_owner_grain_share_bps(),
        final_buyer_cohort,
        post_promotion_bought: consumed.bought,
        max_rolling_cultivator_share,
        living: living_final,
    }
}

fn classify_metrics(
    seed: u64,
    cfg: SettlementConfig,
    ticks: u64,
    require_land_guards: bool,
    controls_not_sticky: bool,
) -> (Metrics, Verdict, f64) {
    let m = run_metrics(seed, cfg.clone(), ticks, require_land_guards);
    let baseline = if m.verdict_needs_baseline() {
        baseline_churn_for(seed, ticks, &cfg)
    } else {
        0.0
    };
    let verdict = m.verdict(baseline, controls_not_sticky);
    (m, verdict, baseline)
}

fn control_verdicts(seed: u64, ticks: u64) -> Vec<(String, Verdict)> {
    [
        ("property_off", property_off_baseline()),
        ("non_excludable_deed", control_non_excludable()),
        ("free_reclaim", control_free_reclaim()),
        ("abundant_good_land", control_abundant_good_land()),
        ("no_forfeit", control_no_forfeit()),
    ]
    .into_iter()
    .map(|(name, cfg)| {
        let (_, verdict, _) = classify_metrics(seed, cfg, ticks, false, true);
        (name.to_string(), verdict)
    })
    .collect()
}

fn assert_private_land_layout(s: &Settlement, total_plots: u16, good_plots: u16) {
    let summaries = s.private_land_plot_summaries();
    assert_eq!(summaries.len(), usize::from(total_plots));

    let mut distances = BTreeSet::new();
    for (_, _, _, _, _, distance) in &summaries {
        assert!(
            distances.insert(*distance),
            "private land plots must not collide at distance {distance}"
        );
        assert!(
            *distance < u32::from(s.private_land_grid_width()),
            "private land plot at {distance} must be in bounds for width {}",
            s.private_land_grid_width()
        );
    }

    let good = summaries
        .iter()
        .filter(|(_, _, _, regen, cap, _)| *regen == 64 && *cap == 8_000)
        .count();
    let marginal = summaries
        .iter()
        .filter(|(_, _, _, _, cap, _)| *cap == 1_000)
        .count();
    let viable_marginal = summaries
        .iter()
        .filter(|(_, _, _, regen, cap, _)| {
            *regen >= VIABLE_REGEN_FLOOR && *cap >= VIABLE_CAP_FLOOR && *cap == 1_000
        })
        .count();
    assert_eq!(good, usize::from(good_plots));
    assert_eq!(marginal, usize::from(total_plots - good_plots));
    assert_eq!(viable_marginal, usize::from(total_plots - good_plots));
}

fn print_capacity_axis_grid() -> usize {
    let mut cells = 0usize;
    for total_plots in TOTAL_PLOTS_SWEEP {
        for good_plots in GOOD_PLOTS_SWEEP {
            if good_plots > total_plots {
                eprintln!(
                    "S23a capacity total={total_plots} good={good_plots}: skipped \
                     (good plots exceed total plots)"
                );
                continue;
            }
            for idle in IDLE_SWEEP {
                for marginal_regen in MARGINAL_REGEN_SWEEP {
                    let mut verdicts = BTreeMap::new();
                    let mut viable_min = BTreeMap::new();
                    let mut marginal_claims = BTreeMap::new();
                    let mut buyers = BTreeMap::new();
                    for seed in CAPACITY_SWEEP_SEEDS {
                        let cfg =
                            private_land_axis_config(total_plots, good_plots, idle, marginal_regen);
                        let (m, verdict, baseline) =
                            classify_metrics(seed, cfg, SWEEP_TICKS, true, true);
                        assert!(
                            m.hard_guards_hold(),
                            "hard guard failed in capacity cell total={total_plots} \
                             good={good_plots} idle={idle} marginal_regen={marginal_regen}: {}",
                            m.line(baseline, true)
                        );
                        verdicts.insert(seed, verdict);
                        viable_min.insert(seed, m.viable_marginal_min_final);
                        marginal_claims.insert(seed, m.marginal_nonowner_claims);
                        buyers.insert(seed, m.final_buyer_cohort);
                    }
                    eprintln!(
                        "S23a capacity total={total_plots} good={good_plots} \
                         marginal={} idle={idle} marginal_regen={marginal_regen}: \
                         verdicts={verdicts:?} viable_min={viable_min:?} \
                         marginal_claims={marginal_claims:?} buyers={buyers:?}",
                        total_plots - good_plots
                    );
                    cells += 1;
                }
            }
        }
    }
    cells
}

#[test]
fn constants_are_well_formed() {
    let s = Settlement::generate(1, &SettlementConfig::frontier_private_land_tenure());
    assert_eq!(s.household_count(), ROSTER_HOUSEHOLDS);
    assert_eq!(
        s.private_land_plot_count(),
        usize::from(HEADLINE_TOTAL_PLOTS)
    );
    assert_eq!(persist_threshold(), (FINAL_WINDOW / 2) as u32);
    assert_private_land_layout(&s, HEADLINE_TOTAL_PLOTS, HEADLINE_GOOD_PLOTS);

    let largest = Settlement::generate(
        1,
        &private_land_axis_config(96, 16, 12, LAND_VIABLE_REGEN_FLOOR),
    );
    assert_private_land_layout(&largest, 96, 16);
    assert!(
        largest.private_land_grid_width() > 64,
        "the 96-plot cell must scale beyond the old 64-wide strip"
    );
}

#[test]
fn private_land_verdict() {
    let headline_cfg = SettlementConfig::frontier_private_land_tenure();
    // The dedicated controls test owns §2.10. Passing `true` here keeps the headline map focused
    // on the treatment seeds without duplicating the full control battery in this already-heavy
    // diagnostic test; a sticky control still fails the suite.
    let controls_not_sticky = true;

    let mut verdicts = BTreeMap::new();
    for &seed in &S23_SEEDS {
        let (m, verdict, baseline) = classify_metrics(
            seed,
            headline_cfg.clone(),
            S23_TICKS,
            true,
            controls_not_sticky,
        );
        eprintln!("S23a {}", m.line(baseline, controls_not_sticky));
        assert!(
            m.hard_guards_hold(),
            "hard guard failed: {}",
            m.line(baseline, controls_not_sticky)
        );
        verdicts.insert(seed, verdict);
    }
    eprintln!(
        "S23a headline verdict map total={HEADLINE_TOTAL_PLOTS} good={HEADLINE_GOOD_PLOTS}: \
         {verdicts:?} (controls_not_sticky={controls_not_sticky})"
    );
    print_capacity_axis_grid();
}

#[test]
fn mandatory_non_vacuity() {
    let mut claims = 0;
    let mut losses = 0;
    let mut reclaims = 0;
    let mut lapsed = 0;
    let mut headline_nonowner_harvest = 0;
    for &seed in &S23_SEEDS {
        let m = run_metrics(
            seed,
            SettlementConfig::frontier_private_land_tenure(),
            S23_TICKS,
            true,
        );
        claims += m.claims;
        losses += m.idle_losses;
        reclaims += m.reclaims_by_other;
        lapsed += m.lapsed_reentry_worse;
        headline_nonowner_harvest += m.nonowner_harvest_of_owned;
    }
    assert!(claims > 0, "plots must be claimed by homesteading");
    assert!(losses > 0, "at least one plot must be lost on idle");
    assert!(
        reclaims > 0,
        "at least one idle-lost plot must be reclaimed by a different agent"
    );
    assert!(
        lapsed > 0,
        "spatial-hysteresis trace needs a lapsed re-entrant worse/farther than a stayer"
    );
    // Ownership actually GATES harvest, proven by the control contrast: with the gate (headline)
    // harvest is owner-exclusive — no non-owner ever pulls grain from a held plot; remove the gate
    // (`non_excludable_deed`) and non-owners harvest held plots freely. Title without the gate is
    // inert bookkeeping, so this pair is the load-bearing non-vacuity for "ownership gates harvest".
    assert_eq!(
        headline_nonowner_harvest, 0,
        "headline harvest must be owner-exclusive (no non-owner harvest of a held plot)"
    );
    let ungated = run_metrics(CONTROL_SEED, control_non_excludable(), S23_TICKS, false);
    assert!(
        ungated.nonowner_harvest_of_owned > 0,
        "with the harvest gate OFF, non-owners must be able to harvest held plots (proves the gate bites)"
    );
}

#[test]
fn controls_do_not_reproduce_stickiness() {
    let seed = CONTROL_SEED;
    for (name, verdict) in control_verdicts(seed, S23_TICKS) {
        eprintln!("S23a control {name} seed={seed}: {verdict:?}");
        assert_ne!(
            verdict,
            Verdict::LandTenureStickySuccess,
            "{name} reproduced the headline sticky verdict for seed {seed}"
        );
    }
}

#[test]
fn canonical_bytes_split_only_when_active() {
    let base = Settlement::generate(7, &property_off_baseline());
    let on = Settlement::generate(7, &SettlementConfig::frontier_private_land_tenure());
    assert_ne!(base.digest(), on.digest());

    let mut inert = SettlementConfig::frontier();
    if let Some(chain) = inert.chain.as_mut() {
        chain.private_land_tenure = true;
    }
    let off = Settlement::generate(7, &SettlementConfig::frontier());
    let inert = Settlement::generate(7, &inert);
    assert_eq!(
        off.digest(),
        inert.digest(),
        "private_land_tenure must be inert off the S22a substrate"
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

#[test]
fn capacity_axis_layouts_are_generated_without_collision() {
    let mut cells = 0usize;
    for total_plots in TOTAL_PLOTS_SWEEP {
        for good_plots in GOOD_PLOTS_SWEEP {
            if good_plots > total_plots {
                continue;
            }
            let cfg = private_land_axis_config(total_plots, good_plots, 12, 12);
            let s = Settlement::generate(1, &cfg);
            assert_private_land_layout(&s, total_plots, good_plots);
            cells += 1;
        }
    }
    assert!(cells > 0, "capacity layout matrix must include valid cells");
}
