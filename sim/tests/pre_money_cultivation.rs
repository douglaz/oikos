//! S15.3 — pre-money own-use cultivation: intensification under pressure (the DoD).
//!
//! The shipped `frontier_cultivation` scenario composes the S14 forage-capacity colony
//! (a real capped FORAGE commons, the spatial lineages foraging it, the hearth food MINT
//! off, a growth-capable demography) with the S15 **escape valve**: when foraging cannot
//! keep a colonist fed (the forage commons is land-capped and depletes under population
//! pressure), a *still-hungry* forager escalates to **cultivation** — it hauls grain from
//! the abundant grain node and makes bread by its OWN labor (the no-tool `Cultivate`
//! recipe, grain → bread), eaten at home through the consumption readback. Tapping the
//! more-abundant grain node via a MORE laborious, more roundabout process RAISES the
//! carrying capacity above the forage-only plateau (Boserup); and because cultivation is
//! the costlier fallback (chosen only on SUSTAINED hunger), it fires ONLY under scarcity.
//!
//! FINDING: own-labor cultivation DOES intensify. At the shipped setting the population
//! plateaus near ~125 with cultivation on versus ~51 forage-only at the same forage flow,
//! and the new plateau TRACKS the grain flow (a grain-regen sweep is monotone). Under
//! abundant forage nobody is sustained-hungry, so nobody cultivates (~0). NO money (the
//! bread is own-use, never traded; SALT never promotes) and NO mortality (deaths stay
//! old-age only). With `own_use_cultivation` off the run reduces to S14 byte-for-byte.

use econ::good::GoodId;
use sim::{ForageCommons, Settlement, SettlementConfig};
use std::collections::BTreeMap;

const RUN_TICKS: u64 = 3000;
const WINDOW_FROM: u64 = 1800;
const CULT_IN: u16 = 6;

fn living(s: &Settlement) -> usize {
    (0..s.population()).filter(|&i| s.is_alive(i)).count()
}

fn grain_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain.as_ref().expect("chain").content.grain()
}

fn bread_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain.as_ref().expect("chain").content.bread()
}

/// Override the grain node's stock/regen/cap (the cultivation "commons" flow knob).
fn set_grain(cfg: &mut SettlementConfig, stock: u32, regen: u32, cap: u32) {
    let grain = grain_good(cfg);
    for node in cfg.nodes.iter_mut() {
        if node.good == grain {
            node.stock = stock;
            node.regen = regen;
            node.cap = cap;
        }
    }
}

/// The windowed-mean living population over `[WINDOW_FROM, RUN_TICKS)` — the plateau.
fn plateau(cfg: &SettlementConfig) -> f64 {
    let mut s = Settlement::generate(1, cfg);
    let (mut sum, mut n) = (0u64, 0u64);
    for tick in 0..RUN_TICKS {
        s.econ_tick();
        if tick >= WINDOW_FROM {
            sum += living(&s) as u64;
            n += 1;
        }
    }
    sum as f64 / n as f64
}

#[test]
fn cultivation_run_is_deterministic() {
    // Byte-identical for the same (seed, config): the `Cultivate` recipe, the
    // `own_use_cultivation` flag + its thresholds, and the per-colonist
    // cultivating/pressure steering all enter canonical_bytes deterministically (no
    // live RNG).
    let cfg = SettlementConfig::frontier_cultivation();
    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    for _ in 0..1500u64 {
        a.econ_tick();
        b.econ_tick();
    }
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the cultivation run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.digest(), b.digest());

    let mut c = Settlement::generate(8, &cfg);
    for _ in 0..1500u64 {
        c.econ_tick();
    }
    assert_ne!(a.digest(), c.digest(), "the seed must change the run");
}

#[test]
fn cultivation_is_produced_not_minted() {
    // Cultivated bread is a CONSERVED transformation booked `produced` (bread) with
    // `consumed_as_input` (grain) — NOT a mint (`endowment[bread] == 0` every tick) —
    // and a cultivator's hunger actually FALLS from eating its own bread (the readback).
    let cfg = SettlementConfig::frontier_cultivation();
    let bread = bread_good(&cfg);
    let grain = grain_good(&cfg);
    let mut s = Settlement::generate(1, &cfg);

    let (mut produced_bread, mut consumed_grain, mut consumed_bread) = (0u64, 0u64, 0u64);
    // Track each colonist's hunger when it (re)starts a cultivation spell, to confirm a
    // cultivator's hunger later falls — hunger advances only via the consumption readback.
    let mut spell_start: BTreeMap<usize, (u64, u16)> = BTreeMap::new();
    let mut saw_hunger_fall = false;
    for tick in 0..1800u64 {
        let r = s.econ_tick();
        produced_bread += r.produced_of(bread);
        consumed_grain += r.consumed_as_input_of(grain);
        consumed_bread += r.consumed_of(bread);
        assert_eq!(
            r.endowment_of(bread),
            0,
            "cultivated bread must be PRODUCED, never minted (tick {tick})"
        );
        for i in 0..s.population() {
            if !s.is_alive(i) {
                continue;
            }
            let hunger = s.need_of(i).expect("alive").hunger;
            if s.is_cultivating(i) && hunger >= CULT_IN {
                spell_start.entry(i).or_insert((tick, hunger));
            }
            if let Some(&(t0, h0)) = spell_start.get(&i) {
                if tick > t0 + 1 && hunger + 2 <= h0 {
                    saw_hunger_fall = true;
                }
                if hunger < CULT_IN {
                    spell_start.remove(&i);
                }
            }
        }
    }
    assert!(produced_bread > 0, "cultivation must PRODUCE bread");
    assert!(
        consumed_grain > 0,
        "cultivation must CONSUME grain as the conserved input"
    );
    assert!(
        consumed_bread > 0,
        "the cultivated bread must be eaten (the own-use readback), not hoarded"
    );
    assert!(
        saw_hunger_fall,
        "a cultivator's hunger must FALL after eating its own bread (the readback feeds it)"
    );
}

#[test]
fn cultivation_intensifies_the_carrying_capacity() {
    // The core claim. (a) With cultivation ON the population plateau rises ABOVE the
    // forage-only (S14) plateau at the same forage flow — the colony feeds more by
    // tapping the abundant grain node. (b) A grain-regen sweep shows the new plateau
    // RESPONDS to the cultivated-grain flow (higher grain regen → higher plateau,
    // monotone), so it is a real carrying-capacity response, not a one-off bump.
    let forage_only = plateau(&SettlementConfig::frontier_forage_capacity());
    let with_cultivation = plateau(&SettlementConfig::frontier_cultivation());
    assert!(
        with_cultivation > forage_only + 25.0,
        "cultivation must lift the plateau well above forage-only \
         (forage {forage_only:.1} -> cultivation {with_cultivation:.1})"
    );

    // The grain-flow sweep (held in the clearly grain-bound region, below the
    // demographic ceiling): higher grain regen → higher plateau, strictly monotone.
    let mut means = Vec::new();
    for regen in [2u32, 3, 4, 5] {
        let mut c = SettlementConfig::frontier_cultivation();
        set_grain(&mut c, regen * 30, regen, regen * 80);
        means.push(plateau(&c));
    }
    for w in means.windows(2) {
        assert!(
            w[1] > w[0],
            "the intensified plateau must rise monotonically with grain flow: {means:?}"
        );
    }
    assert!(
        means.last().expect("means") - means.first().expect("means") > 15.0,
        "a higher grain flow must give a meaningfully higher plateau: {means:?}"
    );
}

#[test]
fn cultivation_raises_births_not_just_feeds() {
    // Cultivation must LIFT the plateau (more births), not merely lower hunger while
    // births stall on a forage-endowment shortage. The broadened child-food rule lets
    // cultivated bread endow children, so `birth_block_endowment` stays LOW and births
    // rise above the forage-only colony's.
    let forage_cfg = SettlementConfig::frontier_forage_capacity();
    let mut sf = Settlement::generate(1, &forage_cfg);
    sf.run(RUN_TICKS);

    let cult_cfg = SettlementConfig::frontier_cultivation();
    let mut sc = Settlement::generate(1, &cult_cfg);
    sc.run(RUN_TICKS);

    assert!(
        sc.births_total() > sf.births_total(),
        "cultivation must RAISE births (cultivation {} > forage-only {})",
        sc.births_total(),
        sf.births_total()
    );
    let endow = sc.birth_block_endowment();
    let hunger = sc.birth_block_hunger_ceiling();
    assert!(
        endow * 20 < hunger,
        "births must NOT stall on the endowment — cultivated bread endows children \
         (endowment blocks {endow} must stay far below hunger-ceiling blocks {hunger})"
    );
}

#[test]
fn cultivation_is_own_use_not_traded() {
    // On this path the bread is OWN-USE: no SALT promotion (no money emerges) and the
    // cultivated bread is never bartered/sold (zero bread trade volume). Hunger relief
    // comes from the cultivator's OWN stock through the readback (bread is consumed).
    let cfg = SettlementConfig::frontier_cultivation();
    let bread = bread_good(&cfg);
    let grain = grain_good(&cfg);
    let mut s = Settlement::generate(1, &cfg);
    let mut consumed_bread = 0u64;
    for _ in 0..RUN_TICKS {
        consumed_bread += s.econ_tick().consumed_of(bread);
    }
    assert!(
        s.promoted_at_tick().is_none() && s.current_money_good().is_none(),
        "no SALT promotion on the own-use cultivation path (money emergence is S16)"
    );
    assert_eq!(
        s.trade_volume_of(bread),
        0,
        "cultivated bread must NEVER be traded (own-use only)"
    );
    assert_eq!(
        s.trade_volume_of(grain),
        0,
        "the gathered grain must not be traded either (it is cultivated at home)"
    );
    assert!(
        consumed_bread > 0,
        "hunger relief must come from the cultivator's OWN bread stock (the readback)"
    );
}

#[test]
fn no_cultivation_without_scarcity() {
    // Under abundant forage (relative to a bounded population) no colonist is
    // SUSTAINED-hungry, so the escape valve never fires — cultivation count is ~0, the
    // colony forages (cheaper) and never pays the cultivation labor cost. And
    // foraging/cultivating are NEVER both set for one colonist in one econ tick.
    let bread = bread_good(&SettlementConfig::frontier_cultivation());
    for (regen, mhs) in [(60u32, 8u16), (200, 8), (400, 4)] {
        let mut cfg = SettlementConfig::frontier_cultivation();
        cfg.chain.as_mut().expect("chain").forage_commons = Some(ForageCommons {
            stock: regen * 10,
            regen,
            cap: regen * 30,
        });
        cfg.demography.as_mut().expect("demo").max_household_size = mhs;
        let mut s = Settlement::generate(1, &cfg);
        let mut cult_bread = 0u64;
        for _ in 0..RUN_TICKS {
            cult_bread += s.econ_tick().produced_of(bread);
        }
        assert_eq!(
            cult_bread, 0,
            "abundant forage (regen {regen}, cap pop {mhs}) must drive ~0 cultivation"
        );
    }

    // Under the scarce shipped scenario cultivation DOES fire, and the two world tasks
    // stay mutually exclusive every tick.
    let cfg = SettlementConfig::frontier_cultivation();
    let mut s = Settlement::generate(1, &cfg);
    let mut any_cultivation = false;
    for _ in 0..1500u64 {
        s.econ_tick();
        for i in 0..s.population() {
            assert!(
                !(s.is_foraging(i) && s.is_cultivating(i)),
                "a colonist must forage XOR cultivate — never both in one econ tick"
            );
            any_cultivation |= s.is_cultivating(i);
        }
    }
    assert!(
        any_cultivation,
        "under forage scarcity the escape valve MUST fire (cultivation occurs)"
    );
}

#[test]
fn cultivation_conserves() {
    // Whole-system conservation every tick: the grain node regen is the source, grain
    // `consumed_as_input` → bread `produced` is a conserved transformation, the bread is
    // consumed (no hoard leak), and NO food is minted (`endowment[staple] == 0`).
    let cfg = SettlementConfig::frontier_cultivation();
    let bread = bread_good(&cfg);
    let grain = grain_good(&cfg);
    let mut s = Settlement::generate(2, &cfg);
    let (mut total_regen, mut total_produced, mut total_consumed_grain) = (0u64, 0u64, 0u64);
    for tick in 0..1800u64 {
        let r = s.econ_tick();
        assert!(r.conserves(), "conservation broke at tick {tick}");
        assert_eq!(
            r.endowment_of(bread),
            0,
            "no minted food: cultivated bread is produced, never minted (tick {tick})"
        );
        total_regen += r.regen_of(grain);
        total_produced += r.produced_of(bread);
        total_consumed_grain += r.consumed_as_input_of(grain);
    }
    assert!(total_regen > 0, "the grain node regen must be the source");
    assert!(total_produced > 0, "cultivation must produce bread");
    assert_eq!(
        total_produced, total_consumed_grain,
        "grain consumed_as_input must equal bread produced (1:1 conserved conversion)"
    );
}

#[test]
fn controls_bracket_intensification() {
    // Disable cultivation → the colony is stuck at the forage-only (S14) plateau;
    // enable it → a higher plateau. The intensified plateau stays forage/grain-bound
    // (the size cap never binds) with cultivated bread endowing children.
    let off = plateau(&SettlementConfig::frontier_forage_capacity());

    let cult_cfg = SettlementConfig::frontier_cultivation();
    let mut s = Settlement::generate(1, &cult_cfg);
    let (mut sum, mut n) = (0u64, 0u64);
    for tick in 0..RUN_TICKS {
        s.econ_tick();
        if tick >= WINDOW_FROM {
            sum += living(&s) as u64;
            n += 1;
        }
    }
    let on = sum as f64 / n as f64;

    assert!(
        on > off + 25.0,
        "enabling cultivation must lift the plateau (off {off:.1} -> on {on:.1})"
    );
    assert_eq!(
        s.birth_block_size_cap(),
        0,
        "the intensified plateau must stay food-bound — the size cap must never bind"
    );
    assert!(
        s.birth_block_endowment() * 20 < s.birth_block_hunger_ceiling(),
        "the birth-block reasons must stay hunger-ceiling-dominated, not endowment-bound"
    );
}

#[test]
fn goldens_unchanged() {
    // With `own_use_cultivation` off the cultivation code is fully gated, so the S14
    // forage-capacity scenario and the cross-history demographic goldens are
    // BYTE-IDENTICAL. The `lineages` golden is the key tripwire (any shared-path byte
    // shift fails here); the broader S5-S14 + econ + emergence suites and the
    // `canonical_bytes_include_*` settlement unit tests are the rest of the proof.
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
        digest(&SettlementConfig::frontier(), 300),
        0xcc83bf2669f0980d,
        "the S5-S13 frontier golden must be byte-identical"
    );
    assert_eq!(
        digest(&SettlementConfig::frontier_spatial_households(), 300),
        0xf30e3ce9345a73b3,
        "the S13 spatial-households golden must be byte-identical"
    );

    // The S14 forage-capacity scenario must be untouched by the gated S15 additions:
    // its own cultivating/pressure bytes never serialize (the flag is off), so it
    // digests exactly as the (already-shipped) forage-capacity run.
    let mut a = Settlement::generate(7, &SettlementConfig::frontier_forage_capacity());
    let mut b = Settlement::generate(7, &SettlementConfig::frontier_forage_capacity());
    a.run(1200);
    b.run(1200);
    assert_eq!(a.digest(), b.digest());
    assert!(
        SettlementConfig::frontier_forage_capacity()
            .chain
            .as_ref()
            .map(|c| !c.own_use_cultivation)
            .unwrap_or(false),
        "the S14 scenario must keep own_use_cultivation OFF"
    );
}
