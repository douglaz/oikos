//! S21g acceptance suite — **mortality-ON over the open-market colony** (impl-30): the
//! Malthusian positive check turned on over the S21f endogenous household-barter colony.
//!
//! S21f closed the *supply* question: an open colony where agents survive by buying food on the
//! market bootstraps endogenous money from pre-money household cultivation-for-barter — with
//! mortality OFF. S21g turns the S17 positive check (starvation) ON over that exact colony and
//! asks the capstone question: does the working money/food market survive real positive-check
//! pressure? The ONLY two deltas vs S21f are the S17 lab-default values (both disclosed, neither
//! tuned): `hunger_critical = need_max` (13 → 12, the positive check) and `birth_hunger_ceiling =
//! 8` (12 → 8, the preventive arm restored BELOW the positive one).
//!
//! THE OBSERVED RESULT IS A COLD-START FINDING (see `positive_check_culls_the_market_roles`),
//! exactly one of the outcomes the spec pre-names ("money failing under mortality pressure").
//! The pre-money market bootstrap requires the DEMAND side to survive a prolonged hungry, foodless
//! wait for the market to form (in S21f, mortality off, the SALT-rich consumers stay pinned at
//! `need_max` for ~40–70 ticks until SALT promotes, THEN buy). The positive check kills exactly
//! that patience: the non-self-provisioning market roles — the SALT-rich buyers and the specialist
//! woodcutters, who hold no food and do not cultivate — starve in a single cold-start cull at
//! tick 7, before any market can form. The self-feeding cultivation lineage survives exactly as
//! the spec's cold-start budget predicted (founders eat their own first cultivated bread at the
//! tick-6 needs phase: `first_hunger_drop < first_starvation_death`). What remains is a quiescent
//! subsistence-cultivation commune: zero starvation after the cull, no buyers, no woodcutters, no
//! SALT circulation, no promotion. The money market is killed at the bootstrap, before it forms —
//! NOT in an ongoing Malthusian band.
//!
//! The `mortality_off_positive_control_money_works` control proves the cause is the positive check,
//! not the scenario: the identical colony with mortality off (S21f) keeps its 18 non-lineage roles
//! alive and promotes SALT on `SelfProduced` bread. The spec's two endorsed provenance-clean rescue
//! levers (grain-flow and `cultivate_*` timing, i.e. faster first production) cannot rescue it —
//! `grain_flow_lever_does_not_rescue_money` shows even a 10× grain flow leaves money dead, because
//! the dying roles do not cultivate; faster bread production helps only the cultivators, who already
//! survive. This localizes precisely WHERE the working colony stops surviving the positive check:
//! the non-cultivating demand side and specialists, in the pre-money window. It is the publishable
//! result; it is NOT tuned into a money-emergence (seed bread would break the `seeded_minted == 0`
//! provenance the milestone rests on, and is the spec's forbidden last resort).
//!
//! Determinism note: `starvation_deaths_total` is runtime-only (NOT in `canonical_bytes`);
//! `hunger_critical` and `birth_hunger_ceiling` are digested but live ONLY in this new scenario, so
//! every existing golden is byte-identical (mirroring `frontier_mortality`).

use econ::good::{GoodId, SALT, WOOD};
use sim::{Settlement, SettlementConfig};

/// A horizon long enough to clear the cold-start cull and let the lineage commune settle (the
/// S21f money suite uses 1600).
const PROBE_TICKS: u64 = 1_600;

// ---- shared helpers -----------------------------------------------------

fn living(s: &Settlement) -> usize {
    (0..s.population()).filter(|&i| s.is_alive(i)).count()
}

/// Living **lineage** members (the self-feeding cultivators — `household_of` is `Some`).
fn living_lineage(s: &Settlement) -> usize {
    (0..s.population())
        .filter(|&i| s.is_alive(i) && s.household_of(i).is_some())
        .count()
}

/// Living **non-lineage** roles (the SALT-rich buyers + the specialist woodcutters — the base
/// colony, `household_of` is `None`). These are the roles the positive check culls.
fn living_non_lineage(s: &Settlement) -> usize {
    (0..s.population())
        .filter(|&i| s.is_alive(i) && s.household_of(i).is_none())
        .count()
}

fn bread_good(s: &Settlement) -> GoodId {
    s.bread_good()
        .expect("the open-colony-mortality chain carries a bread good")
}

fn grain_good(s: &Settlement) -> GoodId {
    s.content()
        .expect("the open-colony-mortality chain carries content")
        .grain()
}

// ---- the cold-start timing trace (Codex spec-review P2) ------------------

/// The runtime-only `cold_start_timing_trace` (§2): the ordered first-occurrence ticks of the
/// cold-start cultivation→survival chain, plus the surviving population at the end. The cold-start
/// SUCCEEDS iff `first_hunger_drop < first_starvation_death` with survivors — i.e. the founders eat
/// their own first cultivated bread before the positive check can kill them. Read-only: it only
/// observes public accessors, so it perturbs nothing (it is NOT digested).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ColdStartTimingTrace {
    /// First tick any living colonist is `cultivating`.
    first_cultivation: Option<u64>,
    /// First tick a `cultivating` lineage member holds hauled grain in its own econ stock.
    first_grain_deposit: Option<u64>,
    /// First tick own-use (`SelfProduced`) bread is consumed (the cumulative channel crosses 0).
    first_own_use_consume: Option<u64>,
    /// First tick any living colonist's hunger DROPS below its own previous-tick hunger (it ate).
    first_hunger_drop: Option<u64>,
    /// First tick max living hunger reaches the critical ceiling.
    first_critical: Option<u64>,
    /// First tick a starvation death is recorded.
    first_starvation_death: Option<u64>,
    /// Living colonists at the end of the trace window.
    survivors: usize,
}

impl ColdStartTimingTrace {
    /// The §2 cold-start bar: the first cultivated bread is eaten (hunger drops) STRICTLY before
    /// the earliest starvation death, and the colony is not extinct.
    fn founders_survive_the_cold_start(&self) -> bool {
        match (self.first_hunger_drop, self.first_starvation_death) {
            // A hunger drop landed before any starvation death.
            (Some(drop), Some(death)) => drop < death && self.survivors > 0,
            // A hunger drop with no starvation death at all is also survival.
            (Some(_), None) => self.survivors > 0,
            // No hunger drop ever: the cultivation→eat chain never closed (a cold-start failure).
            (None, _) => false,
        }
    }
}

/// Collect the cold-start timing trace over `[0, ticks)` on `(seed, cfg)`.
fn cold_start_timing_trace(seed: u64, cfg: &SettlementConfig, ticks: u64) -> ColdStartTimingTrace {
    let mut s = Settlement::generate(seed, cfg);
    let grain = grain_good(&s);
    let crit = cfg.dynamics.hunger_critical;
    let mut trace = ColdStartTimingTrace::default();

    // Per-colonist previous-tick hunger (seeded from the generated state so a founder's very first
    // observation cannot register a spurious drop; newborns are seeded on first sight likewise).
    let mut prev_hunger: Vec<u16> = (0..s.population())
        .map(|i| s.need_of(i).map(|n| n.hunger).unwrap_or(0))
        .collect();

    for t in 0..ticks {
        let _ = s.econ_tick();

        // Grow the prev-hunger map if births appended new colonist slots (seed new slots to their
        // current hunger so they do not register a spurious first-sight drop).
        if s.population() > prev_hunger.len() {
            for i in prev_hunger.len()..s.population() {
                prev_hunger.push(s.need_of(i).map(|n| n.hunger).unwrap_or(0));
            }
        }

        for i in 0..s.population() {
            if !s.is_alive(i) || !s.is_cultivating(i) {
                continue;
            }
            trace.first_cultivation.get_or_insert(t);
            if s.household_of(i).is_some()
                && s.stock_of(i, grain) > 0
                && trace.first_grain_deposit.is_none()
            {
                trace.first_grain_deposit = Some(t);
            }
        }

        // A LINEAGE founder's hunger DROPS once it eats its own cultivated bread (vs its previous
        // tick). Filtered to `household_of(i).is_some()` (Codex result-review P2) so this records the
        // self-feeding cultivators specifically — the cold-start budget is about THEM surviving,
        // while the non-lineage demand side is what the positive check culls.
        for (i, prev) in prev_hunger.iter_mut().enumerate() {
            if !s.is_alive(i) || s.household_of(i).is_none() {
                continue;
            }
            if let Some(n) = s.need_of(i) {
                if n.hunger < *prev {
                    trace.first_hunger_drop.get_or_insert(t);
                }
                *prev = n.hunger;
            }
        }

        if trace.first_own_use_consume.is_none()
            && s.acquisition_consumed_by_channel().self_produced > 0
        {
            trace.first_own_use_consume = Some(t);
        }
        if trace.first_critical.is_none() && s.max_living_hunger() >= crit {
            trace.first_critical = Some(t);
        }
        if trace.first_starvation_death.is_none() && s.starvation_deaths_total() > 0 {
            trace.first_starvation_death = Some(t);
        }
    }
    trace.survivors = living(&s);
    trace
}

// ---- the finding signals over a full run --------------------------------

/// The accumulated finding signals over a full run, plus the settlement at the end.
struct Run {
    s: Settlement,
    conserved_every_tick: bool,
    /// Max bread minted in any single tick's endowment term (must stay 0 — the mints are retired).
    bread_minted_max: u64,
    /// Whether non-lineage roles were alive at the END of the run.
    non_lineage_survivors: usize,
}

fn run_full(seed: u64, cfg: &SettlementConfig, ticks: u64) -> Run {
    let mut s = Settlement::generate(seed, cfg);
    let bread = bread_good(&s);
    let mut conserved_every_tick = true;
    let mut bread_minted_max = 0u64;
    for _ in 0..ticks {
        let report = s.econ_tick();
        conserved_every_tick &= report.conserves();
        bread_minted_max = bread_minted_max.max(report.endowment_of(bread));
    }
    let non_lineage_survivors = living_non_lineage(&s);
    Run {
        s,
        conserved_every_tick,
        bread_minted_max,
        non_lineage_survivors,
    }
}

fn run(seed: u64, cfg: &SettlementConfig) -> Run {
    run_full(seed, cfg, PROBE_TICKS)
}

fn with_grain_flow(stock: u32, regen: u32, cap: u32) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_open_colony_mortality();
    let grain = cfg.chain.as_ref().expect("chain").content.grain();
    for node in cfg.nodes.iter_mut() {
        if node.good == grain {
            node.stock = stock;
            node.regen = regen;
            node.cap = cap;
        }
    }
    cfg
}

// ---- 1. the determinism / golden contract -------------------------------

#[test]
fn canonical_bytes_split_for_open_colony_mortality() {
    // The two S17 deltas (`hunger_critical` and `birth_hunger_ceiling`) are both digested, so the
    // new scenario must SPLIT the canonical digest vs the S21f household-barter base...
    let base = Settlement::generate(7, &SettlementConfig::frontier_household_barter());
    let on = Settlement::generate(7, &SettlementConfig::frontier_open_colony_mortality());
    assert_ne!(
        base.canonical_bytes(),
        on.canonical_bytes(),
        "the positive check + restored preventive arm must split the canonical digest"
    );

    // ...and reverting BOTH deltas (the positive check back to `need_max + 1`, the ceiling back to
    // 12) must make it BYTE-IDENTICAL to `frontier_household_barter` — the ONLY behavioural changes
    // are those two thresholds (the additive proof, mirroring `frontier_mortality`).
    let mut reverted = SettlementConfig::frontier_open_colony_mortality();
    reverted.dynamics.hunger_critical = reverted.dynamics.need_max + 1;
    reverted
        .demography
        .as_mut()
        .expect("demography")
        .birth_hunger_ceiling = 12;
    let mut a = Settlement::generate(7, &reverted);
    let mut b = Settlement::generate(7, &SettlementConfig::frontier_household_barter());
    a.run(1200);
    b.run(1200);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "reverting both S17 deltas must equal frontier_household_barter byte-for-byte"
    );
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn goldens_unchanged() {
    // The S21g additions are a new scenario only (no engine change), and `starvation_deaths_total`
    // stays runtime-only, so the cross-history demographic + emergence goldens are BYTE-IDENTICAL
    // (the values are those pinned in tests/mortality.rs and tests/household_barter.rs).
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
fn open_colony_mortality_run_is_deterministic() {
    // Byte-identical `(seed, config)` at ≥3000 ticks: the deaths live in the colonist
    // liveness/estate state `canonical_bytes` already pins (the runtime-only
    // `starvation_deaths_total` is NOT digested).
    let cfg = SettlementConfig::frontier_open_colony_mortality();
    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    a.run(3200);
    b.run(3200);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the open-colony-mortality run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.digest(), b.digest());

    // The run actually exercised the positive check (else the determinism claim is vacuous).
    assert!(
        a.starvation_deaths_total() > 0,
        "the determinism run must exercise the positive check (starvation)"
    );
    assert!(
        a.old_age_deaths_total() > 0,
        "the determinism run must also see old-age deaths (the lineage churns)"
    );

    // The seed matters (founder cultures are drawn from it), so it is a real run.
    let mut c = Settlement::generate(2, &cfg);
    c.run(3200);
    assert_ne!(a.digest(), c.digest(), "the seed must change the run");
}

// ---- 2. the cold-start timing trace (S21g.0) ----------------------------

#[test]
fn cold_start_timing_trace_founders_survive() {
    // THE §2 cold-start assertion (Codex spec-review P2): the explicit timing trace, not just
    // no-extinction. The founders cultivate, haul + deposit grain, eat their own first cultivated
    // bread (hunger drops) STRICTLY before the earliest starvation death — so they survive the
    // zero-starting-food cultivation lag.
    let cfg = SettlementConfig::frontier_open_colony_mortality();
    let trace = cold_start_timing_trace(7, &cfg, 200);

    // The full ordered chain fired (the cultivation→survival sequence closed).
    assert!(
        trace.first_cultivation.is_some(),
        "cultivation must start in the cold-start window"
    );
    assert!(
        trace.first_grain_deposit.is_some(),
        "a cultivating lineage member must deposit hauled grain"
    );
    assert!(
        trace.first_own_use_consume.is_some(),
        "own-use (SelfProduced) bread must be consumed in the cold-start window"
    );
    assert!(
        trace.first_hunger_drop.is_some(),
        "a founder's hunger must DROP (it ate its own cultivated bread)"
    );
    assert!(
        trace.first_starvation_death.is_some(),
        "the positive check must fire in the cold-start window (the non-provisioning roles)"
    );

    // THE BAR: the first cultivated bread is eaten before the earliest starvation death, with
    // survivors. The cold-start SUCCEEDS for the founders (the spec's predicted ~2-tick margin).
    assert!(
        trace.founders_survive_the_cold_start(),
        "the cold-start must succeed: first_hunger_drop < first_starvation_death with survivors \
         (else a cold-start finding) — trace {trace:?}"
    );

    // The deterministic seed-7 cold-start budget (documents §2's quantified tick budget: cultivate
    // ~tick 3, grain deposit + own-use ~tick 4, hunger drops + critical ~tick 5, first death ~tick
    // 7 — the founders clear by ~2 ticks). Pinned because the trace is deterministic.
    assert_eq!(
        (
            trace.first_cultivation,
            trace.first_grain_deposit,
            trace.first_own_use_consume,
            trace.first_hunger_drop,
            trace.first_critical,
            trace.first_starvation_death,
        ),
        (Some(3), Some(4), Some(4), Some(5), Some(5), Some(7)),
        "the seed-7 cold-start timing budget must hold (the founders eat before the cull)"
    );

    // Non-vacuity: mortality actually fired AND the colony did not go extinct (the lineage commune
    // survives the cull).
    assert!(
        trace.survivors > 0,
        "the colony must not go extinct in the cold-start"
    );
}

// ---- 3. THE FINDING: the positive check culls the non-provisioning roles --

#[test]
fn positive_check_culls_the_market_roles() {
    // THE HEADLINE FINDING (seed 7, deterministic figures pinned — the report cites them). Under
    // the positive check the 18 non-self-provisioning market roles (the SALT-rich buyers + the
    // specialist woodcutters) are wiped out in a SINGLE cold-start cull, while the self-feeding
    // cultivation lineage survives. The money market is killed at the bootstrap, before it forms.
    let r = run(7, &SettlementConfig::frontier_open_colony_mortality());
    let s = &r.s;

    assert!(r.conserved_every_tick, "conservation must hold every tick");
    assert_eq!(
        r.bread_minted_max, 0,
        "no bread may be minted in any tick (the food mints are retired)"
    );

    // (a) The cull: ALL 18 non-lineage roles starve (the cold-start cull); the lineage survives.
    assert_eq!(
        living_non_lineage(s),
        0,
        "the positive check must cull EVERY non-provisioning market role (buyers + woodcutters)"
    );
    assert_eq!(
        s.starvation_deaths_total(),
        18,
        "exactly the 18 non-provisioning roles starve (the deterministic seed-7 cull)"
    );
    assert!(
        living_lineage(s) > 0,
        "the self-feeding cultivation lineage must survive the cull (no extinction)"
    );
    assert!(
        s.old_age_deaths_total() > 0,
        "the surviving lineage churns through births + old age (it is a live commune)"
    );

    // (b) The cull is a ONE-OFF, not an ongoing Malthusian band: no starvation death occurs after
    // the early cold-start window (run far past it and the count is unchanged).
    let mut s2 = Settlement::generate(7, &SettlementConfig::frontier_open_colony_mortality());
    s2.run(50);
    let early = s2.starvation_deaths_total();
    s2.run(3_000);
    assert_eq!(
        s2.starvation_deaths_total(),
        early,
        "starvation is a one-off cold-start cull, not an ongoing band (no late starvation deaths)"
    );

    // (c) The money market is dead: SALT never promotes, no bread is ever bought, no pre-promotion
    // bread is sold for SALT — there are no buyers and no SALT-holders left after the cull.
    assert_eq!(
        s.current_money_good(),
        None,
        "SALT must NOT promote: the demand side is dead, so the medium lane never circulates"
    );
    assert_eq!(
        s.promoted_at_tick(),
        None,
        "there must be no promotion tick"
    );
    assert_eq!(
        s.acquisition_consumed_by_channel().bought,
        0,
        "no food is ever bought (the buyers starved before the market could form)"
    );
    assert_eq!(
        s.pre_promotion_bread_for_salt_by_provenance(),
        (0, 0),
        "no pre-promotion bread is sold for SALT (no buyers to clear the lane)"
    );

    // (d) But the lineage DOES feed itself by cultivation (the colony is a live subsistence
    // commune, not trivially dead) — and provenance stays clean (no SeededMinted bread ever).
    let consumed = s.acquisition_consumed_by_channel();
    assert!(
        consumed.self_produced > 0,
        "the lineage eats its own cultivated (SelfProduced) bread — a live subsistence commune"
    );
    assert_eq!(
        consumed.seeded_minted, 0,
        "no SeededMinted food is ever eaten (the provenance the milestone rests on)"
    );
    assert_eq!(
        s.acquisition_credited_by_channel().seeded_minted,
        0,
        "NO SeededMinted bread ever enters (every buffer + starting_food zeroed)"
    );
}

// ---- 3b. THE LIVE RUN: classify the outcome with observable traces -------

/// The §7 "live run": run the scenario and PRINT the observed classification trace (mirrors
/// `open_survival_classified`), so the finding is observable under `--nocapture` and the report can
/// cite the figures. It branches on `current_money_good()`: the SUCCESS branch (a Malthusian band
/// over a working money market) is the spec's hoped-for outcome; the OBSERVED branch is the
/// cold-start-cull FINDING. The branch keeps the test honest if the engine ever changes — it reports
/// whichever outcome actually occurs and only fails on a broken invariant (conservation / minted
/// food / extinction), never on which outcome is observed.
#[test]
fn live_classification() {
    let cfg = SettlementConfig::frontier_open_colony_mortality();
    let r = run(7, &cfg);
    let s = &r.s;
    let bread = bread_good(s);
    let trace = cold_start_timing_trace(7, &cfg, 200);
    let consumed = s.acquisition_consumed_by_channel();
    let credited = s.acquisition_credited_by_channel();

    eprintln!("=== S21g OPEN-COLONY-MORTALITY — classification @ {PROBE_TICKS} ticks (seed 7) ===");
    eprintln!(
        "promotion: current_money_good = {:?}",
        s.current_money_good()
    );
    eprintln!("promoted_at_tick = {:?}", s.promoted_at_tick());
    eprintln!(
        "living total={} | lineage(cultivators)={} | non-lineage(market roles)={}",
        living(s),
        living_lineage(s),
        living_non_lineage(s),
    );
    eprintln!(
        "deaths: starvation={} | old-age={} | births={}",
        s.starvation_deaths_total(),
        s.old_age_deaths_total(),
        s.births_total(),
    );
    eprintln!("acquisition consumed = {consumed:?}");
    eprintln!("acquisition credited = {credited:?}");
    eprintln!(
        "pre-promotion bread→SALT by provenance (produced, minted) = {:?}",
        s.pre_promotion_bread_for_salt_by_provenance(),
    );
    eprintln!(
        "indirect_target_goods(SALT) = {:?} (bread={bread:?}, WOOD={WOOD:?})",
        s.indirect_target_goods(SALT),
    );
    eprintln!(
        "barter trades cleared = {} | bread traded = {} | max living hunger = {}",
        s.barter_trade_count(),
        s.trade_volume_of(bread),
        s.max_living_hunger(),
    );
    eprintln!("cold-start timing trace = {trace:?}");

    // Broken-invariant guards (fail regardless of which outcome is observed).
    assert!(r.conserved_every_tick, "conservation must hold every tick");
    assert_eq!(
        r.bread_minted_max, 0,
        "no bread may be minted in any tick (the food mints are retired)"
    );
    assert!(living(s) > 0, "the colony must not go extinct");
    assert_eq!(
        credited.seeded_minted, 0,
        "NO SeededMinted bread ever enters (the provenance the milestone rests on)"
    );

    if s.current_money_good() == Some(SALT) {
        // The spec's hoped-for SUCCESS branch (NOT the observed outcome): a Malthusian band over a
        // working money market — money survived the positive check.
        eprintln!(
            "CLASSIFICATION: SUCCESS — SALT promoted under the positive check (a Malthusian band \
             over a working money market). Money survived mortality."
        );
        assert!(
            s.starvation_deaths_total() > 0 && s.births_total() > 0,
            "a band needs both checks binding (starvation + births)"
        );
    } else {
        // The OBSERVED, publishable FINDING: the positive check culls the non-cultivating demand
        // side in a one-off cold-start, so the money market is killed at the bootstrap.
        eprintln!(
            "CLASSIFICATION: FINDING — the positive check culled the non-cultivating market roles \
             (buyers + woodcutters) in a one-off cold-start cull, killing the money market before \
             it could form. What remains is a quiescent subsistence-cultivation commune: SALT \
             never promotes, no food is bought, and the self-feeding lineage survives. Money fails \
             under mortality pressure — one of the spec's pre-named outcomes."
        );
        assert_eq!(
            living_non_lineage(s),
            0,
            "the finding: the non-cultivating market roles are culled"
        );
        assert!(
            living_lineage(s) > 0,
            "the finding: the self-feeding lineage survives the cull"
        );
        assert_eq!(
            consumed.bought, 0,
            "the finding: no food is bought (the demand side starved before any market formed)"
        );
        assert!(
            consumed.self_produced > 0,
            "the finding: the lineage feeds itself by cultivation (a live subsistence commune)"
        );
        // The founders themselves clear the cold-start (the §2 bar) — the cull is the demand side,
        // not the cultivators.
        assert!(
            trace.founders_survive_the_cold_start(),
            "the founders survive the cold-start (the cull is the non-provisioning roles) — {trace:?}"
        );
    }
}

// ---- 4. the localizing control: mortality off → money works -------------

#[test]
fn mortality_off_positive_control_money_works() {
    // The localizing control (the S21f positive control). The IDENTICAL colony with mortality OFF
    // (`frontier_household_barter`, `hunger_critical = need_max + 1`) keeps its 18 non-lineage roles
    // alive and promotes SALT on `SelfProduced` bread. So the cause of the S21g collapse is the
    // POSITIVE CHECK, not the scenario / instrumentation — exactly the S21d mints-on contrast.
    let r = run(7, &SettlementConfig::frontier_household_barter());
    let s = &r.s;
    assert!(r.conserved_every_tick, "conservation must hold every tick");
    assert_eq!(
        s.starvation_deaths_total(),
        0,
        "mortality is OFF in the positive control (no starvation)"
    );
    assert!(
        r.non_lineage_survivors > 0,
        "the non-lineage market roles SURVIVE with mortality off (the demand side persists)"
    );
    assert_eq!(
        s.current_money_good(),
        Some(SALT),
        "SALT promotes on the cultivated supply with mortality off (the S21f success)"
    );
    assert!(
        s.indirect_target_goods(SALT).contains(&WOOD),
        "the indirect breadth includes the non-food WOOD target (the S21f {{bread, WOOD}} bar)"
    );
    assert!(
        s.acquisition_consumed_by_channel().bought > 0,
        "food IS bought on the market with mortality off (a working money/food market)"
    );
    assert_eq!(
        s.acquisition_credited_by_channel().seeded_minted,
        0,
        "the S21f provenance holds: no SeededMinted bread ever enters"
    );
}

// ---- 5. the endorsed lever does not rescue the collapse -----------------

#[test]
fn grain_flow_lever_does_not_rescue_money() {
    // The first of the spec's two endorsed provenance-clean rescue levers (faster first production):
    // GRAIN-FLOW. It CANNOT rescue the money market, because the dying roles do not cultivate:
    // faster bread production helps only the cultivators, who already survive. Sweeping the grain
    // flow up to 10× leaves money dead and the demand side culled at every level.
    // (The other endorsed lever — `cultivate_*` timing — is tested separately in
    // `cultivate_timing_lever_does_not_rescue_money` below.)
    for (label, stock, regen, cap) in [
        ("2x", 960u32, 48u32, 1920u32),
        ("5x", 2400, 120, 4800),
        ("10x", 4800, 240, 9600),
    ] {
        let r = run(7, &with_grain_flow(stock, regen, cap));
        assert_eq!(
            r.s.current_money_good(),
            None,
            "grain {label}: a faster grain flow must NOT rescue promotion (the dying roles do not \
             cultivate)"
        );
        assert_eq!(
            r.non_lineage_survivors, 0,
            "grain {label}: the non-provisioning roles are culled regardless of grain flow"
        );
        assert_eq!(
            r.s.acquisition_consumed_by_channel().bought,
            0,
            "grain {label}: still no food is ever bought (no surviving demand side)"
        );
    }
}

#[test]
fn cultivate_timing_lever_does_not_rescue_money() {
    // The second endorsed provenance-clean rescue lever (Codex result-review P1): faster CULTIVATE
    // timing — trigger cultivation earlier (lower `cultivate_hunger_in`, down to the validator floor
    // of `cultivate_hunger_out + 1 = 4`) and with no patience delay (`cultivate_patience = 1`) — so
    // sellable bread reaches the book sooner. It STILL does not rescue: the dying roles
    // (buyers/woodcutters) do not cultivate, and even bread-in-book ~2 ticks earlier does not let
    // them acquire+eat before the tick-7 cull. So BOTH endorsed levers fail; only seed bread (the
    // spec's forbidden last resort, which would break `seeded_minted == 0`) could change it.
    for (label, hunger_in, patience) in [("hi=4,pat=1", 4u16, 1u16), ("hi=5,pat=1", 5, 1)] {
        let mut cfg = SettlementConfig::frontier_open_colony_mortality();
        if let Some(chain) = cfg.chain.as_mut() {
            chain.cultivate_hunger_in = hunger_in;
            chain.cultivate_patience = patience;
        }
        let r = run(7, &cfg);
        assert_eq!(
            r.s.current_money_good(),
            None,
            "cultivate {label}: faster cultivation must NOT rescue promotion (the dying roles do not cultivate)"
        );
        assert_eq!(
            r.non_lineage_survivors, 0,
            "cultivate {label}: the non-provisioning demand side is culled regardless of cultivate timing"
        );
        assert!(
            r.s.starvation_deaths_total() > 0,
            "cultivate {label}: the positive-check cull still fires"
        );
    }
}

#[test]
fn degenerate_ceiling_is_the_same_collapse() {
    // The disclosed degenerate control (birth_hunger_ceiling = 12, the inherited S21f value, where
    // the preventive check binds at the SAME hunger as the positive one). It is reported as a band
    // SHAPE comparison, not a separate success: the money collapse is identical (the cull happens
    // in the cold-start regardless of the preventive arm's position), confirming the finding does
    // not hinge on the `= 8` ceiling choice.
    let mut deg = SettlementConfig::frontier_open_colony_mortality();
    deg.demography
        .as_mut()
        .expect("demography")
        .birth_hunger_ceiling = 12;
    let r = run(7, &deg);
    assert!(r.conserved_every_tick, "conservation must hold every tick");
    assert_eq!(
        r.s.current_money_good(),
        None,
        "the degenerate ceiling=12 band shows the SAME money collapse (the cull is cold-start)"
    );
    assert_eq!(
        r.non_lineage_survivors, 0,
        "the non-provisioning roles are culled under the degenerate ceiling too"
    );
    assert!(
        r.s.starvation_deaths_total() > 0,
        "the positive check still fires under the degenerate ceiling"
    );
}

// ---- 6. cross-seed robustness -------------------------------------------

#[test]
fn finding_holds_across_seeds() {
    // Robustness ("one seed is one seed"): the cold-start cull of the demand side and the money
    // collapse are not a seed-7 artifact. Across several seeds the non-provisioning roles are
    // culled, SALT never promotes, and the self-feeding lineage survives — while the cold-start
    // timing bar (founders survive) holds.
    for seed in [3u64, 7, 11, 19, 23] {
        let r = run(seed, &SettlementConfig::frontier_open_colony_mortality());
        let s = &r.s;
        assert!(
            r.conserved_every_tick,
            "seed {seed}: conservation must hold every tick"
        );
        assert_eq!(
            living_non_lineage(s),
            0,
            "seed {seed}: the positive check must cull every non-provisioning market role"
        );
        assert_eq!(
            s.current_money_good(),
            None,
            "seed {seed}: SALT must NOT promote (the demand side is dead)"
        );
        assert!(
            s.starvation_deaths_total() > 0,
            "seed {seed}: the positive check must fire (the cold-start cull)"
        );
        assert!(
            living_lineage(s) > 0,
            "seed {seed}: the self-feeding lineage must survive (no extinction)"
        );

        let trace = cold_start_timing_trace(
            seed,
            &SettlementConfig::frontier_open_colony_mortality(),
            200,
        );
        assert!(
            trace.founders_survive_the_cold_start(),
            "seed {seed}: the founders must survive the cold-start (hunger drop before the cull) — {trace:?}"
        );
    }
}

// ---- 7. conservation ----------------------------------------------------

#[test]
fn open_colony_mortality_conserves() {
    // Whole-system conservation EVERY tick across the cold-start cull, the surviving lineage's
    // births + old-age deaths, with no minted bread (the only bread is cultivated). Every live
    // colonist keeps resolving through the arena churn.
    let cfg = SettlementConfig::frontier_open_colony_mortality();
    let bread = bread_good(&Settlement::generate(2, &cfg));
    let mut s = Settlement::generate(2, &cfg);
    for tick in 0..1800u64 {
        let r = s.econ_tick();
        assert!(r.conserves(), "conservation broke at tick {tick}");
        assert_eq!(
            r.endowment_of(bread),
            0,
            "no minted food: bread is cultivated (produced), never minted (tick {tick})"
        );
        if tick % 300 == 0 {
            for i in 0..s.population() {
                if !s.is_alive(i) {
                    continue;
                }
                let id = s.colonist_id(i).expect("a live colonist has an id");
                assert!(
                    s.society().agents.get(id).is_some(),
                    "a live colonist's id must resolve in the arena (no dangling slot)"
                );
            }
        }
    }
    // All three channels churned across the run (starvation in the cold-start, old age + births
    // ongoing in the surviving lineage).
    assert!(
        s.starvation_deaths_total() > 0 && s.old_age_deaths_total() > 0 && s.births_total() > 0,
        "starvation + old-age + births must all churn (starv {}, old {}, births {})",
        s.starvation_deaths_total(),
        s.old_age_deaths_total(),
        s.births_total(),
    );
}

// ---- 8. persistence smoke (the 10k-tick robustness appendix) ------------

#[test]
fn cultivation_commune_persists_to_10k() {
    // The surviving subsistence-cultivation commune persists over a long (10k-tick) horizon: no
    // extinction, money stays dead, and the cold-start cull stays a one-off (no late die-off) —
    // conservation holds throughout.
    let cfg = SettlementConfig::frontier_open_colony_mortality();
    let mut s = Settlement::generate(7, &cfg);
    s.run(500); // past the cold-start cull
    let starv_after_cull = s.starvation_deaths_total();
    let births0 = s.births_total();
    let measure = 9_500u64;
    let mut min_living = usize::MAX;
    for i in 0..measure {
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at measure tick {i}");
        min_living = min_living.min(living(&s));
    }
    assert!(
        min_living > 0,
        "no extinction over 10k ticks (min living {min_living})"
    );
    assert_eq!(
        s.starvation_deaths_total(),
        starv_after_cull,
        "the cull stays a one-off: no further starvation over the 10k horizon"
    );
    assert!(
        s.births_total() > births0,
        "the surviving lineage keeps reproducing over the 10k horizon"
    );
    assert_eq!(
        s.current_money_good(),
        None,
        "money stays dead over the 10k horizon (a quiescent subsistence commune)"
    );
}
