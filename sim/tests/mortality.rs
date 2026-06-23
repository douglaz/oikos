//! S17 acceptance suite — mortality: the Malthusian positive check (the DoD).
//!
//! S14 gave the colony an endogenous carrying capacity via the **preventive** check
//! (births stall when hunger rises); S15 let it intensify by cultivation. But action
//! under scarcity still had no survival consequence — `hunger_critical = need_max + 1`,
//! so starvation death could never fire. S17 turns the **positive** check back on, at the
//! principled lab-default threshold `hunger_critical = need_max` (the only change in
//! `frontier_mortality`), so on the fed-and-plateaued cultivation colony sustained
//! critical hunger kills. Population is now bounded by **births AND deaths** both
//! responding to the carrying capacity — the full Malthusian system.
//!
//! FINDING (the BAND — the hoped-for success, characterized at the principled threshold,
//! NOT tuned). At seed 1 over a 3000-tick window the colony settles into a
//! carrying-capacity band (living ~80–110), with the negative feedback plainly phased:
//! high-hunger windows carry MORE starvation deaths and FEWER births, low-hunger windows
//! the reverse (`corr(hunger, starvation) ≈ +0.65`, `corr(hunger, births) ≈ −0.68`).
//! Hunger oscillates across the critical ceiling (~47% of ticks at the ceiling, ~53%
//! below), the population does not drift (late-window mean ≈ early-window mean) and never
//! collapses (window min ≈ 83). All three death/birth channels are substantial (starvation,
//! old-age, and births all in the hundreds–thousands over the window) and the two death
//! types stay **attributable** via separate counters. More food (forage/grain flow) raises
//! the living band AND cuts starvation frequency; cultivation-on yields a higher viable
//! band than off. Conservation holds on every tick across all of it.
//!
//! Determinism note: `starvation_deaths_total` is a runtime-only diagnostic (NOT in
//! `canonical_bytes`); the deaths themselves live in the colonist liveness/estate state
//! the digest already pins, so the band is a fixed, reproducible trajectory and every
//! existing golden (incl. the live-starvation `g4a_death` configs) is byte-identical.

use econ::agent::{Agent, AgentId, Role};
use econ::good::{Gold, GoodId, Stock, FOOD, WOOD};
use sim::{ForageCommons, NodeSpec, Settlement, SettlementConfig};
use world::Pos;

// ---- shared helpers -----------------------------------------------------

fn living(s: &Settlement) -> usize {
    (0..s.population()).filter(|&i| s.is_alive(i)).count()
}

fn bread_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain.as_ref().expect("chain").content.bread()
}

/// A minimal econ agent for probing arena slot reuse (mirrors the g4a harness).
fn fresh_agent() -> Agent {
    Agent {
        id: AgentId(0),
        scale: Vec::new(),
        stock: Stock::new(WOOD.0),
        gold: Gold::ZERO,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: Vec::new(),
    }
}

/// A marginal-supply settlement that reliably starves its consumers with survivors —
/// the g4a `dieoff_config`, reused here as a controlled, repeatable starvation source
/// under the (already-live) positive check. Two gatherers feed six consumers from a
/// far, slow node, so the consumers starve while the gatherers survive.
fn dieoff_config() -> SettlementConfig {
    let mut cfg = SettlementConfig::viable();
    cfg.gatherers = 2;
    cfg.consumers = 6;
    cfg.consumer_food_buffer = 3;
    cfg.nodes = vec![NodeSpec {
        good: FOOD,
        pos: Pos::new(10, 0),
        stock: 4_000,
        regen: 4,
        cap: 4_000,
    }];
    cfg
}

/// (windowed-mean living population, starvation deaths PER TICK) over `[warmup,
/// warmup+measure)` on `cfg` at seed 1 — the carrying-capacity-response metric.
fn band(cfg: &SettlementConfig, warmup: u64, measure: u64) -> (f64, f64) {
    let mut s = Settlement::generate(1, cfg);
    s.run(warmup);
    let starv0 = s.starvation_deaths_total();
    let (mut sum, mut n) = (0u64, 0u64);
    for _ in 0..measure {
        s.econ_tick();
        sum += living(&s) as u64;
        n += 1;
    }
    let starv = (s.starvation_deaths_total() - starv0) as f64 / measure as f64;
    (sum as f64 / n as f64, starv)
}

fn corr(xs: &[f64], ys: &[f64]) -> f64 {
    let n = xs.len() as f64;
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for i in 0..xs.len() {
        sxy += (xs[i] - mx) * (ys[i] - my);
        sxx += (xs[i] - mx).powi(2);
        syy += (ys[i] - my).powi(2);
    }
    if sxx == 0.0 || syy == 0.0 {
        return 0.0;
    }
    sxy / (sxx.sqrt() * syy.sqrt())
}

// ---- 1. determinism -----------------------------------------------------

#[test]
fn mortality_run_is_deterministic() {
    // Byte-identical `(seed, config)` at ≥3000 ticks: the positive check changes only
    // `hunger_critical`, and the resulting deaths live in the colonist liveness/estate
    // state that canonical_bytes already pins (the runtime-only `starvation_deaths_total`
    // is NOT digested). The band is a fixed, reproducible trajectory.
    let cfg = SettlementConfig::frontier_mortality();
    let mut a = Settlement::generate(1, &cfg);
    let mut b = Settlement::generate(1, &cfg);
    a.run(3200);
    b.run(3200);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the mortality run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.digest(), b.digest());

    // The run actually exercised the positive check (else the determinism claim is
    // vacuous): both Malthusian checks fired.
    assert!(
        a.starvation_deaths_total() > 0,
        "the determinism run must exercise the positive check (starvation)"
    );
    assert!(
        a.old_age_deaths_total() > 0,
        "the determinism run must also see old-age deaths"
    );

    // The seed matters (founder cultures are drawn from it), so it is a real run.
    let mut c = Settlement::generate(2, &cfg);
    c.run(3200);
    assert_ne!(a.digest(), c.digest(), "the seed must change the run");
}

// ---- 2. streak-gated + conserved ---------------------------------------

#[test]
fn starvation_is_streak_gated_and_conserved() {
    // A colonist dies ONLY after `death_window` consecutive critical ticks (the built-in
    // hysteresis — one bad tick never kills), the death is attributed to the starvation
    // counter, and the estate settles conserving with the arena slot freed + reusable
    // (the g4a guarantees, now under the positive check). The single-hauler scenario
    // isolates one colonist's streak.
    let cfg = SettlementConfig::starved_hauler();
    let crit = cfg.dynamics.hunger_critical;
    let window = cfg.dynamics.death_window;
    let mut s = Settlement::generate(1, &cfg);
    let hauler = s.colonist_id(0).expect("the hauler exists");
    let gold_before = s.total_gold();

    let mut consec = 0u16;
    let mut death_tick = None;
    for t in 0..80u64 {
        let alive_before = s.is_alive(0);
        let r = s.econ_tick();
        assert!(r.conserves(), "conservation broke at tick {t}");
        if alive_before && !s.is_alive(0) {
            death_tick = Some(t);
            break;
        }
        // Still alive after this tick: its critical streak must be BELOW the death
        // window (it cannot outlive the window while alive).
        let hunger = s.need_of(0).expect("alive").hunger;
        if hunger >= crit {
            consec += 1;
        } else {
            consec = 0;
        }
        assert!(
            consec < window,
            "a colonist survived {consec} consecutive critical ticks (>= the death \
             window {window}) — the streak gate let it overrun"
        );
    }
    assert!(death_tick.is_some(), "the starved hauler must die");
    // Death fired exactly on the `death_window`-th consecutive critical tick: the last
    // streak we observed while it was alive is `window - 1`.
    assert_eq!(
        consec,
        window - 1,
        "death must fire exactly at the death_window-th consecutive critical tick \
         (sustained critical hunger, not a single bad tick)"
    );

    // Attributed to the positive check, and distinct from old age.
    assert_eq!(
        s.starvation_deaths_total(),
        1,
        "the starvation death is counted"
    );
    assert_eq!(
        s.old_age_deaths_total(),
        0,
        "a non-demographic hauler records no old-age death"
    );

    // Estate conserved: gold is a closed balance (society + commons unchanged), the
    // estate settled to the commons, the dead id resolves None, and the freed slot is
    // reusable with a bumped generation.
    assert_eq!(
        s.total_gold(),
        gold_before,
        "gold is conserved across the starvation death"
    );
    assert!(
        s.commons_gold() > Gold::ZERO || s.commons_stock_of(FOOD) > 0,
        "the dead colonist's estate settled to the commons"
    );
    assert!(
        s.society().agents.get(hauler).is_none(),
        "the dead colonist's id resolves to None"
    );
    let reused = s.society_mut().agents.insert(fresh_agent());
    assert_eq!(
        reused.index(),
        hauler.index(),
        "the freed numeric slot is reused"
    );
    assert!(
        reused.generation() > hauler.generation(),
        "reuse bumps the slot generation"
    );

    // Conservation keeps holding through a multi-death die-off (several starvation
    // deaths in close succession churning the arena) — the sustained-regime guarantee.
    let mut d = Settlement::generate(3, &dieoff_config());
    let pop = d.living_total();
    let gold_total = d.total_gold();
    for t in 0..40u64 {
        let r = d.econ_tick();
        assert!(r.conserves(), "die-off conservation broke at tick {t}");
        assert_eq!(
            d.total_gold(),
            gold_total,
            "a starvation death broke gold conservation at tick {t}"
        );
    }
    assert!(
        d.starvation_deaths_total() > 1 && d.living_total() < pop,
        "the die-off must record multiple starvation deaths with survivors"
    );
}

// ---- 3. attributable ----------------------------------------------------

#[test]
fn deaths_are_attributable() {
    // The two Malthusian checks have SEPARATE counters, each tracking its own death type,
    // and their sum matches the combined `report.deaths`.

    // (a) A starvation-only run: the starvation counter rises, old-age stays 0.
    let mut h = Settlement::generate(1, &dieoff_config());
    let mut report_deaths = 0u64;
    for _ in 0..40u64 {
        report_deaths += u64::from(h.econ_tick().deaths);
    }
    assert!(
        h.starvation_deaths_total() > 0,
        "the die-off must record starvation deaths"
    );
    assert_eq!(
        h.old_age_deaths_total(),
        0,
        "the non-demographic die-off records no old-age deaths"
    );
    assert_eq!(
        report_deaths,
        h.starvation_deaths_total() + h.old_age_deaths_total(),
        "report.deaths must be the sum of the two attributed death types"
    );

    // (b) An old-age-only run: the SAME demographic colony WITHOUT the positive check
    // (the inherited `need_max + 1` dodge) ages out colonists but never starves them, so
    // the old-age counter rises while the starvation counter stays 0.
    let cfg_off = SettlementConfig::frontier_forage_capacity();
    assert_eq!(
        cfg_off.dynamics.hunger_critical,
        cfg_off.dynamics.need_max + 1,
        "the S14 forage config must keep the positive check OFF (need_max + 1)"
    );
    let mut o = Settlement::generate(1, &cfg_off);
    let mut off_report_deaths = 0u64;
    for _ in 0..1000u64 {
        off_report_deaths += u64::from(o.econ_tick().deaths);
    }
    assert!(
        o.old_age_deaths_total() > 0,
        "the long forage run must record old-age deaths"
    );
    assert_eq!(
        o.starvation_deaths_total(),
        0,
        "with the positive check off, starvation can never fire"
    );
    assert_eq!(
        off_report_deaths,
        o.starvation_deaths_total() + o.old_age_deaths_total(),
        "report.deaths must equal the old-age count when no starvation occurs"
    );

    // (c) On the mortality scenario BOTH fire and stay separate — the attribution that
    // makes the full Malthusian dynamic legible.
    let mut m = Settlement::generate(1, &SettlementConfig::frontier_mortality());
    m.run(1500);
    assert!(
        m.starvation_deaths_total() > 0,
        "the positive check must fire on the mortality scenario"
    );
    assert!(
        m.old_age_deaths_total() > 0,
        "old age must still fire on the mortality scenario"
    );
}

// ---- 4. the carrying-capacity band (the core claim) --------------------

#[test]
fn population_settles_in_a_carrying_capacity_band() {
    // THE CORE CLAIM, by windowed PHASE behavior (not mere nonzero churn). At the
    // principled threshold the colony oscillates in a carrying-capacity band: deaths and
    // births phase-track hunger (the negative feedback), the population neither goes
    // extinct nor drifts downward, and hunger oscillates across the critical ceiling.
    let cfg = SettlementConfig::frontier_mortality();
    let crit = cfg.dynamics.hunger_critical;
    let warmup = 500u64;
    let measure = 3000u64;
    let w = 50u64;

    let mut s = Settlement::generate(1, &cfg);
    s.run(warmup);

    let (mut hunger, mut births, mut starv, mut pop, mut minpop) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut psum, mut hsum, mut n, mut minp) = (0u64, 0u64, 0u64, usize::MAX);
    let (mut at_crit, mut below) = (0u64, 0u64);
    let mut prev_b = s.births_total();
    let mut prev_s = s.starvation_deaths_total();
    for tick in 0..measure {
        let r = s.econ_tick();
        assert!(r.conserves(), "conservation broke at tick {tick}");
        let p = living(&s);
        psum += p as u64;
        let h = s.max_living_hunger();
        hsum += u64::from(h);
        if h >= crit {
            at_crit += 1;
        } else {
            below += 1;
        }
        minp = minp.min(p);
        n += 1;
        if (tick + 1) % w == 0 {
            let b = s.births_total();
            let st = s.starvation_deaths_total();
            pop.push(psum as f64 / n as f64);
            hunger.push(hsum as f64 / n as f64);
            births.push((b - prev_b) as f64);
            starv.push((st - prev_s) as f64);
            minpop.push(minp);
            prev_b = b;
            prev_s = st;
            psum = 0;
            hsum = 0;
            n = 0;
            minp = usize::MAX;
        }
    }

    // NOT the redundant outcome: starvation is the substantial, binding positive check,
    // and births keep the colony alive — the full Malthusian system, not a plateau.
    let total_starv = s.starvation_deaths_total();
    assert!(
        total_starv > 200,
        "starvation must be a substantial, binding check (not redundant): {total_starv}"
    );
    assert!(s.births_total() > 200, "births must keep replenishing");
    assert!(
        s.old_age_deaths_total() > 0,
        "old age still operates alongside the positive check"
    );

    // (a) THE NEGATIVE-FEEDBACK PHASE. Contemporaneously, high-hunger windows carry more
    // starvation deaths (positive correlation) and fewer births (negative correlation).
    let c_hs = corr(&hunger, &starv);
    let c_hb = corr(&hunger, &births);
    assert!(
        c_hs > 0.3,
        "starvation deaths must rise with hunger (negative feedback): corr {c_hs:+.3}"
    );
    assert!(
        c_hb < -0.3,
        "births must fall as hunger rises (the preventive arm): corr {c_hb:+.3}"
    );

    // (a, the literal phasing) high-hunger windows are FOLLOWED by more starvation deaths
    // than low-hunger windows are, and low-hunger windows are FOLLOWED by more births.
    let mut sorted = hunger.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let med = sorted[sorted.len() / 2];
    let (mut hi_s, mut hi_n, mut lo_s, mut lo_n, mut hi_b, mut lo_b) =
        (0.0, 0u32, 0.0, 0u32, 0.0, 0.0);
    for i in 0..hunger.len() - 1 {
        if hunger[i] >= med {
            hi_s += starv[i + 1];
            hi_b += births[i + 1];
            hi_n += 1;
        } else {
            lo_s += starv[i + 1];
            lo_b += births[i + 1];
            lo_n += 1;
        }
    }
    let (hi_s, lo_s) = (hi_s / f64::from(hi_n), lo_s / f64::from(lo_n));
    let (hi_b, lo_b) = (hi_b / f64::from(hi_n), lo_b / f64::from(lo_n));
    assert!(
        hi_s > lo_s,
        "high-hunger windows must be FOLLOWED by more starvation deaths \
         (hi {hi_s:.2} > lo {lo_s:.2})"
    );
    assert!(
        lo_b > hi_b,
        "low-hunger windows must be FOLLOWED by more births (lo {lo_b:.2} > hi {hi_b:.2})"
    );

    // (b) bounded away from extinction.
    let min_window_pop = *minpop.iter().min().expect("windows");
    assert!(
        min_window_pop > 40,
        "the population must not collapse — windowed min {min_window_pop} must stay well above 0"
    );

    // (c) no downward drift: late-window mean ≈ early-window mean (oscillating/flat, not
    // a slow collapse and not runaway growth).
    let early = &pop[..pop.len() / 3];
    let late = &pop[pop.len() * 2 / 3..];
    let em = early.iter().sum::<f64>() / early.len() as f64;
    let lm = late.iter().sum::<f64>() / late.len() as f64;
    assert!(
        lm > em * 0.85,
        "the population must not drift downward (early {em:.1} -> late {lm:.1})"
    );
    assert!(
        lm < em * 1.20,
        "the population must settle in a band, not grow without bound (early {em:.1} -> late {lm:.1})"
    );

    // (d) hunger oscillates across the critical ceiling: it spends a substantial fraction
    // of ticks AT the ceiling (driving deaths) AND a substantial fraction BELOW it
    // (recovery), rather than pinned at either.
    let frac_at = at_crit as f64 / measure as f64;
    let frac_below = below as f64 / measure as f64;
    assert!(
        frac_at > 0.1 && frac_below > 0.1,
        "max hunger must oscillate across the critical ceiling \
         (at-ceiling {frac_at:.3}, below {frac_below:.3})"
    );
}

// ---- 5. the food response (a disclosed carrying-capacity sweep) --------

#[test]
fn more_food_raises_the_band_and_cuts_starvation() {
    // The carrying-capacity response. Sweeping the food flow UP raises the living band
    // AND lowers starvation frequency (more food → fewer starve, more survive); and
    // cultivation-on yields a higher viable band than off (the S15 control, now with
    // mortality). A DISCLOSED sweep characterizing the response — the thresholds are NOT
    // tuned; only the food flow varies, holding everything else fixed.
    let warmup = 400u64;
    let measure = 1600u64;

    // Forage-flow sweep (cultivation on, mortality on): band strictly up, starvation
    // strictly down as forage regen rises.
    let mut points = Vec::new();
    for fregen in [1u32, 2, 3] {
        let mut cfg = SettlementConfig::frontier_mortality();
        cfg.chain.as_mut().expect("chain").forage_commons = Some(ForageCommons {
            stock: fregen * 45,
            regen: fregen,
            cap: 300,
        });
        points.push(band(&cfg, warmup, measure));
    }
    for w in points.windows(2) {
        assert!(
            w[1].0 > w[0].0,
            "more forage must RAISE the living band: {:?}",
            points.iter().map(|p| p.0).collect::<Vec<_>>()
        );
        assert!(
            w[1].1 < w[0].1,
            "more forage must CUT starvation frequency: {:?}",
            points.iter().map(|p| p.1).collect::<Vec<_>>()
        );
    }
    assert!(
        points.last().unwrap().0 - points.first().unwrap().0 > 20.0,
        "a higher food flow must give a meaningfully higher band: {:?}",
        points.iter().map(|p| p.0).collect::<Vec<_>>()
    );

    // Cultivation on vs off (mortality on): the escape valve lifts the viable band well
    // above forage-only AND cuts starvation frequency.
    let (on_band, on_starv) = band(&SettlementConfig::frontier_mortality(), warmup, measure);
    let mut off = SettlementConfig::frontier_forage_capacity();
    off.dynamics.hunger_critical = off.dynamics.need_max; // the positive check, forage-only
    let (off_band, off_starv) = band(&off, warmup, measure);
    assert!(
        on_band > off_band + 25.0,
        "cultivation-on must yield a higher viable band than forage-only \
         (on {on_band:.1} vs off {off_band:.1})"
    );
    assert!(
        on_starv < off_starv,
        "cultivation-on must cut starvation frequency vs forage-only \
         (on {on_starv:.3} vs off {off_starv:.3})"
    );
}

// ---- 6. conservation ----------------------------------------------------

#[test]
fn mortality_conserves() {
    // Whole-system conservation EVERY tick across births + starvation deaths + old-age
    // deaths, with no minted food (the staple mint is off — the only bread is cultivated)
    // and no leak. Births + deaths churn the arena slot rapidly over the long run; every
    // live colonist keeps resolving (world_id == econ_id holds through the churn).
    let cfg = SettlementConfig::frontier_mortality();
    let bread = bread_good(&cfg);
    let mut s = Settlement::generate(2, &cfg);
    for tick in 0..1800u64 {
        let r = s.econ_tick();
        assert!(r.conserves(), "conservation broke at tick {tick}");
        assert_eq!(
            r.endowment_of(bread),
            0,
            "no minted food: bread is cultivated (produced), never minted (tick {tick})"
        );
        if tick % 200 == 0 {
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
    // All three channels actually churned (conservation across them, not a quiet run).
    assert!(
        s.starvation_deaths_total() > 0 && s.old_age_deaths_total() > 0 && s.births_total() > 0,
        "births + starvation + old-age must all churn (starv {}, old {}, births {})",
        s.starvation_deaths_total(),
        s.old_age_deaths_total(),
        s.births_total(),
    );
}

// ---- 7. goldens unchanged ----------------------------------------------

#[test]
fn goldens_unchanged() {
    // The S17 additions are gated and additive: the new `starvation_deaths_total` is
    // runtime-only (NOT in canonical_bytes, so it shifts no digest), and the enabled
    // `hunger_critical` lives ONLY in the new `frontier_mortality` scenario — the existing
    // scarcity/frontier configs keep the `need_max + 1` dodge (the live-starvation harnesses
    // like g4a_death / starved_hauler already run starvation and are unaffected precisely
    // because the new counter is not digested). So the cross-history demographic and
    // emergence goldens are BYTE-IDENTICAL (the `lineages` golden is the key tripwire), and
    // the live-starvation g4a no-death digest is untouched.
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

    // The g4a no-death tripwire (seed 0xC0FFEE, 60 ticks): a viable run frees no agent
    // and reproduces the pre-G4a (tombstone-era) bytes exactly — adding the runtime-only
    // starvation counter must not perturb it.
    let mut viable = Settlement::generate(0xC0FFEE, &SettlementConfig::viable());
    viable.run(60);
    assert_eq!(
        viable.digest(),
        0xa174_8567_db1c_4341,
        "the g4a viable no-death digest must be byte-identical"
    );

    // The KEY additive proof for S17: `frontier_mortality` with the positive check
    // reverted to the `need_max + 1` dodge is BYTE-IDENTICAL to `frontier_cultivation` —
    // the ONLY behavioral change is the `hunger_critical` threshold.
    let mut reverted = SettlementConfig::frontier_mortality();
    reverted.dynamics.hunger_critical = reverted.dynamics.need_max + 1;
    let mut a = Settlement::generate(7, &reverted);
    let mut b = Settlement::generate(7, &SettlementConfig::frontier_cultivation());
    a.run(1200);
    b.run(1200);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "frontier_mortality with the positive check reverted must equal frontier_cultivation"
    );
    assert_eq!(a.digest(), b.digest());
}
