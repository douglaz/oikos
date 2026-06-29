//! S23b acceptance suite: post-money alienable land market over S23a private land.
//!
//! The headline verdict is a classifier, not a tuned assertion. Run with `--nocapture` to read it:
//!   `cargo test -p sim --test land_market land_market_verdict -- --nocapture`

use std::collections::{BTreeMap, BTreeSet};

use econ::good::SALT;
use sim::{
    LandMarketSaleRow, Settlement, SettlementConfig, LAND_CARRYING_PERIOD, LAND_RENT_WINDOW,
};

const CHURN_DROP: f64 = 0.5;
const PERSIST_FRACTION: f64 = 0.5;
const PERSIST_COHORT: usize = 4;
const ROSTER_HOUSEHOLDS: usize = 8;
const OWNER_SHARE_MAX_BPS: u64 = 6_000;
const MONO_SHARE_BPS: u64 = 7_500;
const ROLLING_WINDOW: usize = 100;
const FINAL_WINDOW: u64 = 200;
const MATERIAL_BOUGHT_FLOOR: u64 = 4;
const MIN_BUYER_COHORT: usize = 2;
const MIN_LAND_TRADES: u64 = 8;
const LIQUID_CHURN_TRADES: u64 = 200;
const PRICE_RENT_GAP_BPS: u64 = 2_000;
const S23_TICKS: u64 = 300;
const S23_SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const CONTROL_SEED: u64 = 3;
const HEADLINE_TOTAL_PLOTS: u16 = 28;
const HEADLINE_GOOD_PLOTS: u16 = 4;
const PRICE_CAP_SWEEP: [u64; 4] = [0, 1, 2, 3];
const SWEEP_TICKS: u64 = 180;
const ROBUSTNESS_PRICE_CAP_SWEEP: [u64; 3] = [0, 1, 2];
const ROBUSTNESS_CARRYING_COST_SWEEP: [u64; 2] = [0, 1];
const TOTAL_PLOTS_SWEEP: [u16; 2] = [28, 48];
const MARGINAL_REGEN_SWEEP: [u32; 1] = [12];

fn persist_threshold() -> u32 {
    (PERSIST_FRACTION * FINAL_WINDOW as f64).ceil() as u32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    LandMarketInert,
    MoneyFailureFromLandMarket,
    ConservationBroken,
    Extinct,
    HardBarrier,
    LandMonopolyCull,
    LiquidChurn,
    TunedPriceDiagnostic,
    LandMarketStickySuccess,
    NoStickinessDespiteLandMarket,
}

#[derive(Clone, Debug)]
struct Metrics {
    seed: u64,
    conserved: bool,
    post_promotion_gold_conserved: bool,
    bread_minted_max: u64,
    extinct: bool,
    registry_ok: bool,
    provenance_clean: bool,
    promoted: bool,
    land_trades: u64,
    pre_promotion_trades: u64,
    pre_promotion_charges: u64,
    carrying_paid: u64,
    fee_pool: u64,
    foreclosure_listings: u64,
    priced_out: u64,
    lapsed_priced_out: u64,
    ask_bid_gap_mean: Option<u64>,
    good_sale_count: usize,
    marginal_sale_count: usize,
    good_mean_price: u64,
    marginal_mean_price: u64,
    price_rent_gap_bps: u64,
    churn_total: u32,
    ever_cultivating: usize,
    persistent_market_owner_cultivators: usize,
    owner_share_bps: u64,
    owner_grain_share_bps: u64,
    final_buyer_cohort: usize,
    post_promotion_bought: u64,
    final_affordable_listed_max: usize,
    universal_owner: bool,
    living: usize,
    title_original: usize,
    title_inherited: usize,
    title_bought: usize,
    title_foreclosed_out: usize,
}

impl Metrics {
    fn hard_guards_hold(&self) -> bool {
        self.conserved
            && self.post_promotion_gold_conserved
            && self.bread_minted_max == 0
            && !self.extinct
            && self.registry_ok
            && self.provenance_clean
            && self.pre_promotion_trades == 0
            && self.pre_promotion_charges == 0
    }

    fn churn_per_capita(&self) -> f64 {
        if self.ever_cultivating == 0 {
            0.0
        } else {
            f64::from(self.churn_total) / self.ever_cultivating as f64
        }
    }

    fn endogeneity_holds(&self) -> bool {
        self.good_sale_count > 0
            && self.marginal_sale_count > 0
            && self.good_mean_price.saturating_mul(10_000)
                >= self
                    .marginal_mean_price
                    .saturating_mul(10_000 + PRICE_RENT_GAP_BPS)
            && self.price_rent_gap_bps >= PRICE_RENT_GAP_BPS
    }

    fn market_non_vacuous(&self) -> bool {
        self.land_trades >= MIN_LAND_TRADES
            && self.pre_promotion_trades == 0
            && self.pre_promotion_charges == 0
            && self.endogeneity_holds()
            && self.lapsed_priced_out >= 1
    }

    fn monopoly_cull(&self) -> bool {
        self.owner_share_bps >= MONO_SHARE_BPS
            && (self.final_buyer_cohort < MIN_BUYER_COHORT
                || self.post_promotion_bought < MATERIAL_BOUGHT_FLOOR)
    }

    fn success_like(&self, baseline_churn: f64) -> bool {
        self.promoted
            && self.market_non_vacuous()
            && self.hard_guards_hold()
            && self.churn_per_capita() <= CHURN_DROP * baseline_churn
            && self.persistent_market_owner_cultivators >= PERSIST_COHORT
            && self.owner_share_bps <= OWNER_SHARE_MAX_BPS
            && self.final_buyer_cohort >= MIN_BUYER_COHORT
            && self.post_promotion_bought >= MATERIAL_BOUGHT_FLOOR
    }

    fn verdict(
        &self,
        baseline_churn: f64,
        controls_not_sticky: bool,
        adjacent_cap_success: bool,
    ) -> Verdict {
        if self.promoted && !self.market_non_vacuous() {
            return Verdict::LandMarketInert;
        }
        if !self.promoted {
            return Verdict::MoneyFailureFromLandMarket;
        }
        // Extinction is checked before the conservation gate: `hard_guards_hold` already folds in
        // `!extinct`, so without this an extinct colony would misclassify as `ConservationBroken`
        // and the `Extinct` arm would be dead. Both sit at the spec's `ConservationBroken/extinct`
        // classifier level (§2); the more specific colony-death cause is reported first.
        if self.extinct {
            return Verdict::Extinct;
        }
        if !self.hard_guards_hold() {
            return Verdict::ConservationBroken;
        }
        // HardBarrier is SUSTAINED inaccessibility "through the final window" (spec §2): the max
        // affordable-listed count over the window is zero (no eligible non-owner could afford any
        // listed plot at any tick), not a single transient zero — which would otherwise mask a
        // genuine success that merely dipped for one tick.
        if self.final_affordable_listed_max == 0 || self.universal_owner {
            return Verdict::HardBarrier;
        }
        if self.monopoly_cull() {
            return Verdict::LandMonopolyCull;
        }
        if self.land_trades >= LIQUID_CHURN_TRADES
            && self.churn_per_capita() > CHURN_DROP * baseline_churn
            && self.persistent_market_owner_cultivators < PERSIST_COHORT
        {
            return Verdict::LiquidChurn;
        }
        if self.success_like(baseline_churn) && !adjacent_cap_success {
            return Verdict::TunedPriceDiagnostic;
        }
        if self.success_like(baseline_churn) && controls_not_sticky && adjacent_cap_success {
            Verdict::LandMarketStickySuccess
        } else {
            Verdict::NoStickinessDespiteLandMarket
        }
    }

    fn line(
        &self,
        baseline_churn: f64,
        controls_not_sticky: bool,
        adjacent_cap_success: bool,
    ) -> String {
        format!(
            "seed={} {:?} | trades={} pre_trades={} pre_charges={} prices(good={} n={} marginal={} n={} gap_bps={}) \
             priced_out={}/{} carrying={} fee_pool={} foreclose={} ask_gap={:?} churn={:.2}/{:.2} \
             persist_market_owner={} owners_bps={} owner_grain_bps={} buyers={} bought={} affordable_max={} \
             titles={{original:{}, inherited:{}, bought:{}, foreclosed_out:{}}} living={} promoted={} guards={}",
            self.seed,
            self.verdict(baseline_churn, controls_not_sticky, adjacent_cap_success),
            self.land_trades,
            self.pre_promotion_trades,
            self.pre_promotion_charges,
            self.good_mean_price,
            self.good_sale_count,
            self.marginal_mean_price,
            self.marginal_sale_count,
            self.price_rent_gap_bps,
            self.lapsed_priced_out,
            self.priced_out,
            self.carrying_paid,
            self.fee_pool,
            self.foreclosure_listings,
            self.ask_bid_gap_mean,
            self.churn_per_capita(),
            baseline_churn,
            self.persistent_market_owner_cultivators,
            self.owner_share_bps,
            self.owner_grain_share_bps,
            self.final_buyer_cohort,
            self.post_promotion_bought,
            self.final_affordable_listed_max,
            self.title_original,
            self.title_inherited,
            self.title_bought,
            self.title_foreclosed_out,
            self.living,
            self.promoted,
            self.hard_guards_hold(),
        )
    }
}

fn living(s: &Settlement) -> usize {
    (0..s.population()).filter(|&i| s.is_alive(i)).count()
}

fn land_market_off_config() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_land_market();
    cfg.chain.as_mut().unwrap().land_market = false;
    cfg
}

fn control_zero_price() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_land_market();
    cfg.chain.as_mut().unwrap().land_price_cap_factor = 0;
    cfg
}

fn control_non_excludable_title() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_land_market();
    cfg.chain.as_mut().unwrap().harvest_gate = false;
    cfg
}

fn control_abundant_good_land() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_land_market();
    set_land_plot_counts(&mut cfg, 96, 96);
    cfg
}

fn control_no_carrying_cost() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_land_market();
    cfg.chain.as_mut().unwrap().land_carrying_cost = 0;
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

fn with_price_cap(mut cfg: SettlementConfig, cap: u64) -> SettlementConfig {
    cfg.chain.as_mut().unwrap().land_price_cap_factor = cap;
    cfg
}

fn with_carrying_cost(mut cfg: SettlementConfig, cost: u64) -> SettlementConfig {
    cfg.chain.as_mut().unwrap().land_carrying_cost = cost;
    cfg
}

fn with_marginal_regen(mut cfg: SettlementConfig, regen: u32) -> SettlementConfig {
    cfg.chain.as_mut().unwrap().land_marginal_regen = regen;
    cfg
}

fn baseline_churn_for(seed: u64, ticks: u64) -> f64 {
    run_metrics(seed, land_market_off_config(), ticks, true).churn_per_capita()
}

fn mean_price(rows: &[LandMarketSaleRow], good: bool) -> (usize, u64) {
    let mut sum = 0u64;
    let mut count = 0usize;
    for row in rows {
        if row.6 == good {
            count += 1;
            sum = sum.saturating_add(row.4);
        }
    }
    if count == 0 {
        (0, 0)
    } else {
        (count, (sum + count as u64 / 2) / count as u64)
    }
}

fn run_metrics(seed: u64, cfg: SettlementConfig, ticks: u64, require_guards: bool) -> Metrics {
    let mut s = Settlement::generate(seed, &cfg);
    assert_eq!(s.household_count(), ROSTER_HOUSEHOLDS);
    let bread = s.bread_good().expect("S23b base carries bread");
    let final_start = ticks.saturating_sub(FINAL_WINDOW);
    let mut conserved = true;
    let mut post_promotion_gold_conserved = true;
    let mut bread_minted_max = 0u64;
    let mut registry_ok = true;
    let mut prev_cultivating: BTreeMap<u64, bool> = BTreeMap::new();
    let mut churn_total = 0u32;
    let mut ever_cultivating = BTreeSet::new();
    let mut final_owner_cultivator_ticks: BTreeMap<u64, u32> = BTreeMap::new();
    let mut final_affordable_listed_max = 0usize;
    let mut rolling: Vec<bool> = Vec::new();

    for tick in 0..ticks {
        let money_before = s.current_money_good();
        let gold_before = s.total_gold().0;
        let report = s.econ_tick();
        conserved &= report.conserves();
        if money_before == Some(SALT) {
            post_promotion_gold_conserved &= gold_before == s.total_gold().0;
        }
        bread_minted_max = bread_minted_max.max(report.endowment_of(bread));
        registry_ok &= s.private_land_registry_invariant_holds();
        if require_guards {
            assert!(
                registry_ok,
                "private land registry invariant failed at tick {tick}"
            );
            assert!(conserved, "whole-system conservation failed at tick {tick}");
            assert!(
                post_promotion_gold_conserved,
                "post-promotion SALT accounting failed at tick {tick}"
            );
            assert_eq!(
                s.land_market_pre_promotion_trades_total(),
                0,
                "land trade occurred before SALT promotion at tick {tick}"
            );
            assert_eq!(
                s.land_market_pre_promotion_charges_total(),
                0,
                "land charge occurred before SALT promotion at tick {tick}"
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
        rolling.push(cultivating_now > 0);
        if rolling.len() > ROLLING_WINDOW {
            rolling.remove(0);
        }
        if tick >= final_start {
            final_affordable_listed_max = final_affordable_listed_max
                .max(s.land_market_affordable_listed_plots_for_nonowners());
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
    let (_, pp_minted) = s.pre_promotion_bread_for_salt_by_provenance();
    let provenance_clean = s.seeded_minted_bread_sold_for_salt() == 0 && pp_minted == 0;
    let persistent_market_owner_cultivators = final_owner_cultivator_ticks
        .iter()
        .filter(|&(&id, &ticks)| {
            ticks >= persist_threshold() && s.land_market_agent_market_stabilized(id, final_start)
        })
        .count();
    let sales = s.land_market_sale_rows();
    let (good_sale_count, good_mean_price) = mean_price(&sales, true);
    let (marginal_sale_count, marginal_mean_price) = mean_price(&sales, false);
    let price_rent_gap_bps = good_mean_price
        .saturating_sub(marginal_mean_price)
        .saturating_mul(10_000)
        .checked_div(marginal_mean_price)
        .unwrap_or(0);
    let (title_original, title_inherited, title_bought, title_foreclosed_out) =
        s.land_market_title_share_counts();
    let consumed = s.acquisition_consumed_by_channel();

    Metrics {
        seed,
        conserved,
        post_promotion_gold_conserved,
        bread_minted_max,
        extinct: living_final == 0,
        registry_ok: s.private_land_registry_invariant_holds() && registry_ok,
        provenance_clean,
        promoted: s.current_money_good() == Some(SALT),
        land_trades: s.land_market_trades_total(),
        pre_promotion_trades: s.land_market_pre_promotion_trades_total(),
        pre_promotion_charges: s.land_market_pre_promotion_charges_total(),
        carrying_paid: s.land_market_carrying_paid_total(),
        fee_pool: s.land_market_fee_pool_salt(),
        foreclosure_listings: s.land_market_foreclosure_listings_total(),
        priced_out: s.land_market_priced_out_total(),
        lapsed_priced_out: s.land_market_lapsed_priced_out_total(),
        ask_bid_gap_mean: s.land_market_ask_bid_gap_mean(),
        good_sale_count,
        marginal_sale_count,
        good_mean_price,
        marginal_mean_price,
        price_rent_gap_bps,
        churn_total,
        ever_cultivating: ever_cultivating.len(),
        persistent_market_owner_cultivators,
        owner_share_bps: if living_final == 0 {
            0
        } else {
            owners.len().saturating_mul(10_000) as u64 / living_final as u64
        },
        owner_grain_share_bps: s.private_land_owner_grain_share_bps(),
        final_buyer_cohort,
        post_promotion_bought: consumed.bought,
        final_affordable_listed_max: final_affordable_listed_max
            .max(s.land_market_affordable_listed_plots_for_nonowners()),
        universal_owner: living_final > 0 && owners.len() >= living_final,
        living: living_final,
        title_original,
        title_inherited,
        title_bought,
        title_foreclosed_out,
    }
}

fn success_like_for(seed: u64, cfg: SettlementConfig, ticks: u64) -> bool {
    let m = run_metrics(seed, cfg, ticks, true);
    m.success_like(baseline_churn_for(seed, ticks))
}

fn adjacent_cap_success(seed: u64, cfg: &SettlementConfig, ticks: u64) -> bool {
    let cap = cfg
        .chain
        .as_ref()
        .map_or(1, |chain| chain.land_price_cap_factor);
    let candidates = [cap.saturating_sub(1), cap.saturating_add(1)];
    candidates
        .into_iter()
        .filter(|&candidate| candidate != cap)
        .any(|candidate| success_like_for(seed, with_price_cap(cfg.clone(), candidate), ticks))
}

fn controls_not_sticky(seed: u64, ticks: u64) -> bool {
    [
        land_market_off_config(),
        control_zero_price(),
        control_non_excludable_title(),
        control_abundant_good_land(),
    ]
    .into_iter()
    .all(|cfg| !success_like_for(seed, cfg, ticks))
}

fn classify_metrics(seed: u64, cfg: SettlementConfig, ticks: u64) -> (Metrics, Verdict, f64) {
    let m = run_metrics(seed, cfg.clone(), ticks, true);
    let baseline = baseline_churn_for(seed, ticks);
    let (controls_not_sticky, adjacent_cap_success) = if m.success_like(baseline) {
        (
            controls_not_sticky(seed, ticks),
            adjacent_cap_success(seed, &cfg, ticks),
        )
    } else {
        (true, false)
    };
    let verdict = m.verdict(baseline, controls_not_sticky, adjacent_cap_success);
    (m, verdict, baseline)
}

#[test]
fn constants_are_well_formed() {
    let s = Settlement::generate(1, &SettlementConfig::frontier_land_market());
    assert_eq!(s.household_count(), ROSTER_HOUSEHOLDS);
    assert_eq!(s.private_land_plot_count(), HEADLINE_TOTAL_PLOTS as usize);
    assert_eq!(persist_threshold(), (FINAL_WINDOW / 2) as u32);
    assert_eq!(LAND_CARRYING_PERIOD, 12);
    assert_eq!(LAND_RENT_WINDOW, 100);

    let summaries = s.private_land_plot_summaries();
    let good = summaries
        .iter()
        .filter(|(_, _, _, regen, _, _)| *regen == 64)
        .count();
    assert_eq!(good, HEADLINE_GOOD_PLOTS as usize);

    let abundant = Settlement::generate(1, &control_abundant_good_land());
    assert!(
        abundant.private_land_plot_count() >= abundant.population(),
        "abundant-good-land control must have enough good plots for the full roster"
    );
}

#[test]
fn land_market_verdict() {
    let headline_cfg = SettlementConfig::frontier_land_market();
    let mut verdicts = BTreeMap::new();
    for &seed in &S23_SEEDS {
        let m = run_metrics(seed, headline_cfg.clone(), S23_TICKS, true);
        let baseline = baseline_churn_for(seed, S23_TICKS);
        let (controls, adjacent) = if m.success_like(baseline) {
            (
                controls_not_sticky(seed, S23_TICKS),
                adjacent_cap_success(seed, &headline_cfg, S23_TICKS),
            )
        } else {
            (true, false)
        };
        let verdict = m.verdict(baseline, controls, adjacent);
        eprintln!("S23b {}", m.line(baseline, controls, adjacent));
        assert!(
            m.hard_guards_hold(),
            "hard guard failed: {}",
            m.line(baseline, controls, adjacent)
        );
        verdicts.insert(seed, verdict);
    }
    eprintln!("S23b headline verdict map: {verdicts:?}");
}

#[test]
fn mandatory_non_vacuity_endogeneity_and_post_money() {
    let mut trades = 0u64;
    let mut pre_trades = 0u64;
    let mut pre_charges = 0u64;
    let mut lapsed_priced_out = 0u64;
    let mut good_sum = 0u64;
    let mut good_count = 0usize;
    let mut marginal_sum = 0u64;
    let mut marginal_count = 0usize;

    for &seed in &S23_SEEDS {
        let m = run_metrics(
            seed,
            SettlementConfig::frontier_land_market(),
            S23_TICKS,
            true,
        );
        eprintln!(
            "S23b non-vacuity {}",
            m.line(baseline_churn_for(seed, S23_TICKS), true, false)
        );
        trades = trades.saturating_add(m.land_trades);
        pre_trades = pre_trades.saturating_add(m.pre_promotion_trades);
        pre_charges = pre_charges.saturating_add(m.pre_promotion_charges);
        lapsed_priced_out = lapsed_priced_out.saturating_add(m.lapsed_priced_out);
        good_sum =
            good_sum.saturating_add(m.good_mean_price.saturating_mul(m.good_sale_count as u64));
        good_count += m.good_sale_count;
        marginal_sum = marginal_sum.saturating_add(
            m.marginal_mean_price
                .saturating_mul(m.marginal_sale_count as u64),
        );
        marginal_count += m.marginal_sale_count;
        assert!(
            m.promoted,
            "SALT must promote before market evidence is counted"
        );
        assert!(m.hard_guards_hold(), "hard guard failed for seed {seed}");
    }

    assert!(
        trades >= MIN_LAND_TRADES,
        "post-promotion land trades must be non-vacuous: {trades}"
    );
    assert_eq!(pre_trades, 0, "land trades must be post-promotion only");
    assert_eq!(pre_charges, 0, "land charges must be post-promotion only");
    assert!(
        lapsed_priced_out >= 1,
        "at least one lapsed seller must later be priced out"
    );
    assert!(good_count > 0, "good plots must have sale prices");
    assert!(marginal_count > 0, "marginal plots must have sale prices");
    let good_mean = (good_sum + good_count as u64 / 2) / good_count as u64;
    let marginal_mean = (marginal_sum + marginal_count as u64 / 2) / marginal_count as u64;
    assert!(
        good_mean.saturating_mul(10_000)
            >= marginal_mean.saturating_mul(10_000 + PRICE_RENT_GAP_BPS),
        "good plots must trade dearer than marginal plots by at least {PRICE_RENT_GAP_BPS} bps: \
         good={good_mean} marginal={marginal_mean}"
    );
}

#[test]
fn land_market_off_is_property_baseline() {
    let seed = CONTROL_SEED;
    let off = run_metrics(seed, land_market_off_config(), S23_TICKS, true);
    let baseline = baseline_churn_for(seed, S23_TICKS);
    assert_eq!(off.land_trades, 0);
    assert_eq!(off.pre_promotion_trades, 0);
    assert!(off.hard_guards_hold());
    assert_eq!(off.churn_per_capita(), baseline);
    eprintln!("S23b land_market_off {}", off.line(baseline, true, false));
}

#[test]
fn pre_money_land_market_forbidden() {
    for &seed in &S23_SEEDS {
        let m = run_metrics(
            seed,
            SettlementConfig::frontier_land_market(),
            S23_TICKS,
            true,
        );
        assert_eq!(m.pre_promotion_trades, 0);
        assert_eq!(m.pre_promotion_charges, 0);
    }
}

#[test]
fn free_rebuy_zero_price_is_not_sticky() {
    let seed = CONTROL_SEED;
    let cfg = control_zero_price();
    let m = run_metrics(seed, cfg.clone(), S23_TICKS, true);
    let baseline = baseline_churn_for(seed, S23_TICKS);
    let verdict = m.verdict(baseline, true, adjacent_cap_success(seed, &cfg, S23_TICKS));
    eprintln!("S23b zero_price {}", m.line(baseline, true, false));
    assert_ne!(verdict, Verdict::LandMarketStickySuccess);
    assert_eq!(m.good_mean_price, 0);
    assert_eq!(m.lapsed_priced_out, 0);
}

#[test]
fn non_excludable_title_is_not_sticky() {
    let seed = CONTROL_SEED;
    let cfg = control_non_excludable_title();
    let m = run_metrics(seed, cfg.clone(), S23_TICKS, true);
    let baseline = baseline_churn_for(seed, S23_TICKS);
    let verdict = m.verdict(baseline, true, adjacent_cap_success(seed, &cfg, S23_TICKS));
    eprintln!(
        "S23b non_excludable_title {}",
        m.line(baseline, true, false)
    );
    assert_ne!(verdict, Verdict::LandMarketStickySuccess);
    assert!(
        m.land_trades == 0 || !m.success_like(baseline),
        "title without harvest exclusion must not reproduce the sticky result"
    );
}

#[test]
fn abundant_good_land_is_not_sticky() {
    let seed = CONTROL_SEED;
    let cfg = control_abundant_good_land();
    let m = run_metrics(seed, cfg.clone(), S23_TICKS, true);
    let baseline = baseline_churn_for(seed, S23_TICKS);
    let verdict = m.verdict(baseline, true, adjacent_cap_success(seed, &cfg, S23_TICKS));
    eprintln!("S23b abundant_good_land {}", m.line(baseline, true, false));
    assert_ne!(verdict, Verdict::LandMarketStickySuccess);
}

#[test]
fn price_cap_sensitivity_is_outcome_driving() {
    let seed = CONTROL_SEED;
    let mut cells = BTreeMap::new();
    for cap in PRICE_CAP_SWEEP {
        let cfg = with_price_cap(SettlementConfig::frontier_land_market(), cap);
        let (m, verdict, baseline) = classify_metrics(seed, cfg, S23_TICKS);
        eprintln!("S23b cap={cap} {}", m.line(baseline, true, false));
        assert!(m.hard_guards_hold(), "hard guard failed for cap {cap}");
        cells.insert(
            cap,
            (
                verdict,
                m.land_trades,
                m.good_mean_price,
                m.marginal_mean_price,
                m.lapsed_priced_out,
            ),
        );
    }
    let distinct: BTreeSet<_> = cells.values().cloned().collect();
    assert!(
        distinct.len() > 1,
        "land_price_cap_factor must be outcome-driving: {cells:?}"
    );
}

#[test]
fn no_carrying_cost_is_reported_sensitivity() {
    let seed = CONTROL_SEED;
    let m = run_metrics(seed, control_no_carrying_cost(), S23_TICKS, true);
    let baseline = baseline_churn_for(seed, S23_TICKS);
    eprintln!("S23b no_carrying_cost {}", m.line(baseline, true, false));
    assert!(m.hard_guards_hold());
    assert_eq!(m.carrying_paid, 0);
    assert_eq!(m.fee_pool, 0);
}

#[test]
fn robustness_mini_sweep_classifies_without_tuning() {
    let seed = CONTROL_SEED;
    let mut cap_cells = BTreeSet::new();
    let mut carrying_cells = BTreeSet::new();
    let mut cells = 0usize;

    for cap in ROBUSTNESS_PRICE_CAP_SWEEP {
        for carrying in ROBUSTNESS_CARRYING_COST_SWEEP {
            for total_plots in TOTAL_PLOTS_SWEEP {
                for marginal_regen in MARGINAL_REGEN_SWEEP {
                    let mut cfg = SettlementConfig::frontier_land_market();
                    set_land_plot_counts(
                        &mut cfg,
                        total_plots,
                        HEADLINE_GOOD_PLOTS.min(total_plots),
                    );
                    cfg = with_price_cap(cfg, cap);
                    cfg = with_carrying_cost(cfg, carrying);
                    cfg = with_marginal_regen(cfg, marginal_regen);
                    let m = run_metrics(seed, cfg, SWEEP_TICKS, true);
                    let baseline = baseline_churn_for(seed, SWEEP_TICKS);
                    let verdict = m.verdict(baseline, true, false);
                    eprintln!(
                        "S23b sweep cap={cap} carrying={carrying} total={total_plots} \
                         marginal_regen={marginal_regen} {:?} trades={} churn={:.2} fee={} gap_bps={} guards={}",
                        verdict,
                        m.land_trades,
                        m.churn_per_capita(),
                        m.fee_pool,
                        m.price_rent_gap_bps,
                        m.hard_guards_hold(),
                    );
                    assert!(
                        m.hard_guards_hold(),
                        "hard guard failed in robustness cell cap={cap} carrying={carrying} \
                         total={total_plots} marginal_regen={marginal_regen}"
                    );
                    cap_cells.insert((cap, verdict, m.land_trades, m.good_mean_price));
                    carrying_cells.insert((carrying, verdict, m.fee_pool, m.foreclosure_listings));
                    cells += 1;
                }
            }
        }
    }

    assert!(cells > 0);
    assert!(
        cap_cells
            .iter()
            .map(|(_, verdict, trades, price)| (*verdict, *trades, *price))
            .collect::<BTreeSet<_>>()
            .len()
            > 1,
        "price-cap axis must move outcomes"
    );
    assert!(
        carrying_cells
            .iter()
            .map(|(_, verdict, fee, foreclosures)| (*verdict, *fee, *foreclosures))
            .collect::<BTreeSet<_>>()
            .len()
            > 1,
        "carrying-cost axis must move outcomes"
    );
}

#[test]
fn canonical_bytes_split_only_when_active() {
    let off = Settlement::generate(7, &land_market_off_config());
    let on = Settlement::generate(7, &SettlementConfig::frontier_land_market());
    let mut matched_s23a = SettlementConfig::frontier_private_land_tenure();
    set_land_plot_counts(&mut matched_s23a, HEADLINE_TOTAL_PLOTS, HEADLINE_GOOD_PLOTS);
    let matched_s23a = Settlement::generate(7, &matched_s23a);
    assert_eq!(
        matched_s23a.digest(),
        off.digest(),
        "land-market fields must be omitted when the flag is off"
    );
    assert_ne!(
        off.digest(),
        on.digest(),
        "active land market must split the canonical digest"
    );

    let mut off_knobs = land_market_off_config();
    if let Some(chain) = off_knobs.chain.as_mut() {
        chain.land_carrying_cost = 99;
        chain.land_price_cap_factor = 99;
    }
    assert_eq!(
        off.digest(),
        Settlement::generate(7, &off_knobs).digest(),
        "inactive market knobs must not steer the digest"
    );

    let mut inert = SettlementConfig::frontier();
    if let Some(chain) = inert.chain.as_mut() {
        chain.land_market = true;
    }
    let base = Settlement::generate(7, &SettlementConfig::frontier());
    let inert = Settlement::generate(7, &inert);
    assert_eq!(
        base.digest(),
        inert.digest(),
        "land_market must be inert off the private-land substrate"
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
