//! S14.3 — forage carrying capacity: the endogenous population plateau (the DoD).
//!
//! The shipped `frontier_forage_capacity` scenario composes the S12 own-labor path
//! (hearth food MINT OFF — forage IS the food), S13 spatial households (the lineages
//! forage), the S14.1 capped FORAGE commons (a depleting node, so per-capita yield
//! falls with the foraging population), and the S14.2 forage child endowment + a
//! growth-capable demography. The colony GROWS while the commons can feed it and
//! PLATEAUS when it cannot — the plateau set by the forage flow, bounded by the
//! birth-hunger **preventive check** (births stall when a member's hunger exceeds the
//! ceiling). The carrying capacity is ENDOGENOUS: the population RESPONSE to scarce
//! forage flow (the regen/cap are still parameters). Deaths are old-age only (no
//! mortality — `hunger_critical` stays disabled).
//!
//! FINDING: the population grows past the old `max_household_size` of 5 and plateaus at
//! a forage-determined level (low 50s living at regen 2, with inherited spoilage off)
//! that TRACKS the carrying capacity (lower regen < regen 2 < higher regen). The
//! controls bracket "endogenous vs knob": uncapping the forage lets it grow to the
//! raised household cap (~72, forage no longer binds), while keeping
//! `max_household_size` low pins it at the knob (~15). The plateau is bounded by the
//! hunger-ceiling preventive check (the dominant birth-block reason), not the parent
//! endowment or the size cap — so it is forage scarcity, not a bread shortage, that
//! bounds the colony.

use econ::good::GoodId;
use sim::{ForageCommons, Settlement, SettlementConfig};

const RUN_TICKS: u64 = 2500;
const WINDOW_FROM: u64 = 1200;

fn forage_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain
        .as_ref()
        .expect("chain")
        .content
        .forage()
        .expect("own-labor subsistence interns a forage good")
}

fn bread_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain.as_ref().expect("chain").content.bread()
}

fn living(s: &Settlement) -> usize {
    (0..s.population()).filter(|&i| s.is_alive(i)).count()
}

/// (mean, min, max) living population over the window `[WINDOW_FROM, RUN_TICKS)` —
/// the plateau metric (a windowed average smooths the demographic oscillation).
fn plateau(cfg: &SettlementConfig) -> (f64, usize, usize) {
    let mut s = Settlement::generate(1, cfg);
    let (mut sum, mut n) = (0u64, 0u64);
    let (mut min, mut max) = (usize::MAX, 0usize);
    for tick in 0..RUN_TICKS {
        s.econ_tick();
        if tick >= WINDOW_FROM {
            let pop = living(&s);
            sum += pop as u64;
            n += 1;
            min = min.min(pop);
            max = max.max(pop);
        }
    }
    (sum as f64 / n as f64, min, max)
}

/// The shipped config with the FORAGE commons regen overridden (the carrying-capacity
/// knob), holding everything else fixed.
fn with_regen(stock: u32, regen: u32, cap: u32) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_forage_capacity();
    cfg.chain.as_mut().expect("chain").forage_commons = Some(ForageCommons { stock, regen, cap });
    cfg
}

/// The shipped config with the FORAGE commons regen overridden AND `max_household_size`
/// lifted far above any of the sweep's forage plateaus, so the artificial knob can NEVER
/// bind: every sweep point is then purely **forage-bound** (the hunger ceiling is the
/// only stall, the size cap blocks no birth). This isolates the variable the milestone's
/// headline claim rests on — the rising plateau is the population's response to the
/// forage flow, not the population climbing into the size cap.
fn forage_bound_sweep(stock: u32, regen: u32, cap: u32) -> SettlementConfig {
    let mut cfg = with_regen(stock, regen, cap);
    cfg.demography
        .as_mut()
        .expect("demography")
        .max_household_size = 60;
    cfg
}

/// (windowed-mean living population, hunger-ceiling birth-blocks, size-cap birth-blocks)
/// over the plateau window — the carrying-capacity metric plus the birth-block reasons
/// that say WHETHER the plateau is forage-bound (hunger ceiling) or knob-bound (size cap).
fn plateau_blocks(cfg: &SettlementConfig) -> (f64, u64, u64) {
    let mut s = Settlement::generate(1, cfg);
    let (mut sum, mut n) = (0u64, 0u64);
    for tick in 0..RUN_TICKS {
        s.econ_tick();
        if tick >= WINDOW_FROM {
            sum += living(&s) as u64;
            n += 1;
        }
    }
    (
        sum as f64 / n as f64,
        s.birth_block_hunger_ceiling(),
        s.birth_block_size_cap(),
    )
}

#[test]
fn forage_capacity_run_is_deterministic() {
    // Byte-identical for the same (seed, config): the FORAGE node stock/regen/cap, the
    // forage-commons flag, the birth-food selector, the demography values, and the
    // birth-block counters all enter canonical_bytes deterministically (no live RNG).
    let cfg = SettlementConfig::frontier_forage_capacity();
    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    for _ in 0..1500u64 {
        a.econ_tick();
        b.econ_tick();
    }
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the forage-capacity run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.digest(), b.digest());

    // The seed matters (founder cultures are drawn from it), so it is a real run.
    let mut c = Settlement::generate(8, &cfg);
    for _ in 0..1500u64 {
        c.econ_tick();
    }
    assert_ne!(a.digest(), c.digest(), "the seed must change the run");
}

/// A controlled, demography-free commons: `consumers` immortal consumer-foragers (no
/// births to self-regulate, no old age, no starvation), so the foraging population is a
/// clean dial for the per-capita sweep. Built from the capacity config with the
/// lineages removed.
fn consumer_commons(consumers: u16, regen: u32) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_forage_capacity();
    cfg.consumers = consumers;
    cfg.demography = None;
    cfg.chain.as_mut().unwrap().forage_commons = Some(ForageCommons {
        stock: 0,
        regen,
        cap: 40,
    });
    cfg
}

#[test]
fn forage_commons_depletes_and_regenerates() {
    // PART A — the shipped (reproducing) colony: the FORAGE node depletes under harvest
    // and refills by regen, and the total forage drawn is bounded by the regen budget +
    // initial stock (the only source), NOT by the forager count. A real commons, not a
    // fixed credit (no `produced`).
    let cfg = SettlementConfig::frontier_forage_capacity();
    let fg = forage_good(&cfg);
    let node = cfg
        .chain
        .as_ref()
        .unwrap()
        .forage_commons
        .expect("the capacity config sets a commons");
    let mut s = Settlement::generate(1, &cfg);
    let node_id = s.forage_node_id().expect("the commons forage node exists");

    let (mut total_transferred, mut total_regen, mut total_produced) = (0u64, 0u64, 0u64);
    let mut min_stock = u32::MAX;
    let ticks = 1600u64;
    for _ in 0..ticks {
        let r = s.econ_tick();
        total_transferred += r.transferred_of(fg);
        total_regen += r.regen_of(fg);
        total_produced += r.produced_of(fg);
        min_stock = min_stock.min(s.world().node(node_id).map(|n| n.stock).unwrap_or(0));
    }

    assert_eq!(
        total_produced, 0,
        "the commons books no fixed `produced` credit"
    );
    assert!(
        total_regen > 0,
        "the FORAGE node must regenerate (the source)"
    );
    assert!(
        total_transferred > 0,
        "foragers must draw FORAGE through the haul cycle"
    );
    assert!(
        min_stock < node.cap,
        "the node must deplete below its cap under harvest (min stock {min_stock} < cap {})",
        node.cap
    );
    // Bounded by the source, NOT by the forager count: a fixed-credit path would scale
    // with foragers (×yield × tasks-per-tick), dwarfing this budget.
    let budget = u64::from(node.stock) + u64::from(node.regen) * 24 * ticks;
    assert!(
        total_transferred <= budget,
        "total draw {total_transferred} must be bounded by the regen budget {budget}"
    );

    // PART B — per-capita falls as foragers rise: with a controlled (immortal) forager
    // count and a binding regen, doubling the foragers does NOT double the draw (it
    // saturates at the commons budget), so each forager's per-capita share strictly
    // falls. At the reproducing colony's plateau, per-capita is instead pinned near
    // subsistence and the POPULATION is what tracks the carrying capacity (parts 3-4).
    let sweep_ticks = 300u64;
    let regen = 1u32;
    let mut per_capita = Vec::new();
    let mut last_total = 0u64;
    for &n in &[8u16, 16, 32, 48] {
        let cfg = consumer_commons(n, regen);
        let fg = forage_good(&cfg);
        let mut s = Settlement::generate(1, &cfg);
        let mut total = 0u64;
        for _ in 0..sweep_ticks {
            total += s.econ_tick().transferred_of(fg);
        }
        per_capita.push(total as f64 / f64::from(n));
        last_total = total;
    }
    for w in per_capita.windows(2) {
        assert!(
            w[1] < w[0],
            "per-capita FORAGE must fall as the forager count rises: {per_capita:?}"
        );
    }
    let budget = u64::from(regen) * 24 * sweep_ticks;
    assert!(
        last_total <= budget,
        "the total draw must saturate at the regen budget {budget}, not the forager count ({last_total})"
    );
}

#[test]
fn population_grows_then_plateaus() {
    // With the commons feeding it, the population RISES past the old size cap (5) and
    // PLATEAUS; births stall via the birth-hunger gate (the preventive check) as the
    // foragers cannot keep hunger below the ceiling; deaths are old-age only.
    let cfg = SettlementConfig::frontier_forage_capacity();
    let mut s = Settlement::generate(1, &cfg);
    let start = living(&s);
    assert!(start <= 6, "the colony starts at the founders");

    let mut total_deaths = 0u64;
    let mut peak = 0usize;
    let (mut sum, mut n, mut wmin, mut wmax) = (0u64, 0u64, usize::MAX, 0usize);
    for tick in 0..RUN_TICKS {
        let r = s.econ_tick();
        total_deaths += u64::from(r.deaths);
        let pop = living(&s);
        peak = peak.max(pop);
        if tick >= WINDOW_FROM {
            sum += pop as u64;
            n += 1;
            wmin = wmin.min(pop);
            wmax = wmax.max(pop);
        }
    }
    let avg = sum as f64 / n as f64;

    assert!(
        peak > 5,
        "population must grow past the old size cap of 5 (peak {peak})"
    );
    assert!(
        (32.0..56.0).contains(&avg),
        "the plateau must settle in a forage-determined band (avg {avg:.1})"
    );
    assert!(
        wmin >= 18,
        "the colony must not collapse at the plateau (min {wmin})"
    );
    assert!(
        wmax <= 68,
        "the colony must remain separated from the size cap 72 (max {wmax})"
    );
    assert!(
        s.births_total() > u64::from(start as u32),
        "the colony grows by births"
    );
    assert!(
        s.birth_block_hunger_ceiling() > 0,
        "births must stall via the birth-hunger gate (the preventive check)"
    );
    assert_eq!(
        total_deaths,
        s.old_age_deaths_total(),
        "deaths must be old-age only (no starvation)"
    );
}

#[test]
fn plateau_tracks_carrying_capacity() {
    // Lower forage regen → lower population plateau, monotone: the plateau tracks the
    // forage FLOW, not `max_household_size`. The size cap is lifted out of the way for
    // the whole sweep (`forage_bound_sweep`), so EVERY point is forage-bound — the
    // hunger ceiling is the only stall and the size cap blocks no birth. The rising
    // plateau is therefore the population's response to the forage flow (the headline
    // claim), not the population climbing into the artificial knob.
    let points = [
        plateau_blocks(&forage_bound_sweep(60, 1, 200)),
        plateau_blocks(&forage_bound_sweep(90, 2, 300)),
        plateau_blocks(&forage_bound_sweep(120, 3, 400)),
        plateau_blocks(&forage_bound_sweep(150, 4, 500)),
    ];
    for &(mean, hunger, size_cap) in &points {
        assert_eq!(
            size_cap, 0,
            "every sweep point must be forage-bound — the size cap must NEVER bind \
             (mean {mean:.1}, sizecap blocks {size_cap})"
        );
        assert!(
            hunger > 0,
            "every sweep point must be bounded by the hunger ceiling — the preventive \
             check (mean {mean:.1}, hunger blocks {hunger})"
        );
    }
    let means: Vec<f64> = points.iter().map(|&(m, _, _)| m).collect();
    for w in means.windows(2) {
        assert!(
            w[1] > w[0],
            "the forage-bound plateau must rise monotonically with forage regen: {means:?}"
        );
    }
    assert!(
        means.last().expect("means") - means.first().expect("means") > 8.0,
        "lower forage flow must give a meaningfully lower plateau: {means:?}"
    );
}

#[test]
fn forage_capacity_conserves() {
    // Whole-system conservation every tick, with the FORAGE node regen the ONLY source
    // (no `produced` credit, no minted food, no inherited spoilage): `endowment[staple] == 0`
    // proves the hearth food mint is off, so the plateau is forage-determined.
    let cfg = SettlementConfig::frontier_forage_capacity();
    assert_eq!(
        cfg.chain.as_ref().expect("chain").perishable_decay_bps,
        0,
        "the forage-capacity scenario must isolate the commons from spoilage"
    );
    let staple = bread_good(&cfg);
    let fg = forage_good(&cfg);
    let mut s = Settlement::generate(2, &cfg);
    let mut total_regen = 0u64;
    for tick in 0..1500u64 {
        let r = s.econ_tick();
        assert!(r.conserves(), "conservation broke at tick {tick}");
        assert_eq!(
            r.endowment_of(staple),
            0,
            "no minted food: the hearth staple mint must be OFF (tick {tick})"
        );
        assert_eq!(
            r.produced_of(fg),
            0,
            "no fixed FORAGE credit: node regen is the only source (tick {tick})"
        );
        assert_eq!(
            r.spoiled_of(fg) + r.spoiled_of(staple),
            0,
            "no inherited spoilage: forage scarcity must come from commons flow (tick {tick})"
        );
        total_regen += r.regen_of(fg);
    }
    assert!(total_regen > 0, "node regen must be the FORAGE source");
}

#[test]
fn controls_bracket_the_plateau() {
    // The two controls bracket "endogenous vs knob".
    let (main, _, _) = plateau(&SettlementConfig::frontier_forage_capacity());

    // Uncap the forage (huge regen) → forage no longer binds, so the population grows
    // to the RAISED household cap (~72) and the hunger ceiling stops stalling births.
    let huge = with_regen(800, 60, 1600);
    let (huge_avg, _, _) = plateau(&huge);
    let mut s = Settlement::generate(1, &huge);
    for _ in 0..RUN_TICKS {
        s.econ_tick();
    }
    assert!(
        huge_avg > main + 15.0,
        "uncapping forage must grow the population well above the forage-bound plateau (main {main:.1} -> huge {huge_avg:.1})"
    );
    assert_eq!(
        s.birth_block_hunger_ceiling(),
        0,
        "with abundant forage the hunger ceiling must never stall a birth"
    );
    assert!(
        s.birth_block_size_cap() > 0,
        "with abundant forage the RAISED household cap is what bounds growth"
    );

    // Keep `max_household_size` low → the artificial knob binds (the old regime): the
    // population cannot grow to pressure the forage flow.
    let mut low = SettlementConfig::frontier_forage_capacity();
    low.demography.as_mut().unwrap().max_household_size = 5;
    let (low_avg, _, low_max) = plateau(&low);
    let mut s = Settlement::generate(1, &low);
    for _ in 0..RUN_TICKS {
        s.econ_tick();
    }
    assert!(
        low_avg < main && low_max <= 16,
        "a low max_household_size must pin the population at the knob (low {low_avg:.1}, main {main:.1})"
    );
    assert!(
        s.birth_block_size_cap() > s.birth_block_hunger_ceiling(),
        "with a low size cap the knob (not forage) is the binding stall"
    );
}

#[test]
fn births_stall_on_forage_not_bread() {
    // The fed-by-forage colony reproduces (the birth-food selector lets parents endow
    // children from forage), and as scarcity bites the birth-block diagnostics attribute
    // the stall to the HUNGER ceiling — NOT the parent endowment or the size cap. So the
    // bound is the preventive check on forage scarcity, not a bread shortage.
    let cfg = SettlementConfig::frontier_forage_capacity();
    let staple = bread_good(&cfg);
    let mut s = Settlement::generate(1, &cfg);
    for tick in 0..2000u64 {
        let r = s.econ_tick();
        assert_eq!(
            r.endowment_of(staple),
            0,
            "the colony is fed by forage, never a minted bread staple (tick {tick})"
        );
    }
    assert!(
        s.births_total() > 6,
        "the fed-by-forage colony must reproduce"
    );

    let hunger = s.birth_block_hunger_ceiling();
    let size_cap = s.birth_block_size_cap();
    let endowment = s.birth_block_endowment();
    assert!(
        hunger > 0,
        "forage scarcity must stall births via the hunger ceiling"
    );
    assert!(
        hunger > 10 * (size_cap + endowment + 1),
        "the HUNGER ceiling must dominate the stalls (hunger {hunger}, sizecap {size_cap}, endow {endowment}) — not bread/endowment/knob"
    );
}

#[test]
fn goldens_unchanged() {
    // The S14 flags are gated and additive, so the pre-S14 configs are BYTE-IDENTICAL.
    // These cross-history digests (captured before S14) are the tripwire — the
    // `lineages` demographic golden most of all. Any shared-path change that shifted
    // bytes would fail here; the full S5-S13 + econ + emergence suites are the broader
    // proof, and the `canonical_bytes_include_*` regressions (settlement unit tests)
    // pin each new field's own identity.
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
        digest(&SettlementConfig::frontier(), 600),
        0x523bda8c17368d2c,
        "the long frontier run must be byte-identical"
    );
    assert_eq!(
        digest(&SettlementConfig::frontier_spatial_households(), 300),
        0xf30e3ce9345a73b3,
        "the S13 spatial-households golden must be byte-identical"
    );
    assert_eq!(
        digest(
            &SettlementConfig::frontier_coemergent_strong_provisioned(),
            300
        ),
        0xc353ae318ea8c6c2,
        "the S12 own-labor (fixed-credit forage) golden must be byte-identical"
    );
    assert_eq!(
        digest(&SettlementConfig::frontier_coemergent_strong(), 300),
        0x169eeca8b612f3f9,
        "the S11 coemergence golden must be byte-identical"
    );
    assert_eq!(
        digest(&SettlementConfig::emergent_chain(), 300),
        0xa2858ea4ab7f58c4,
        "the G3b emergent-chain golden must be byte-identical"
    );
}
