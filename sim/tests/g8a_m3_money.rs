//! G8a acceptance suite — the M3-ledger money settlement (finance foundation).
//!
//! Every settlement before G8a ran on econ's **closed-GOLD M1** money (`Agent.gold`,
//! no ledger). G8a runs the spatial settlement on econ's **M3 `MoneySystem`** instead:
//! money is M3 **specie** (there are NO banks, NO fiat, NO demand claims — those are
//! G8b/G8c), and every sim money flow (the spot market, the world→econ settlement, and
//! the wage/birth/estate transfers) is a **ledger move** rather than an `Agent.gold`
//! mutation. It also resolves the G4a/b deferral: a funded M3 colonist's death now
//! **drains** its ledger specie into the estate (conserved) instead of refusing removal.
//!
//! These pin the contract and its tripwires:
//! - the M3 run is deterministic (test 1);
//! - whole-system conservation spans the M3 ledger (specie) + goods, and the ledger's
//!   own conservation holds, every econ tick (test 2);
//! - an M3 specie settlement is economically equivalent to the M1 settlement — same
//!   trades, prices, provisioning — because M3 specie with no banks/fiat *is* M1, only
//!   ledger-accounted (test 3);
//! - a death routes the M3 ledger balance to the estate/commons, conserved; the funded
//!   M3 removal no longer refuses (test 4 — the resolved G4a/b deferral);
//! - a birth's endowment is a conserved within-ledger transfer, not a mint (test 5);
//! - the M3 composition is pure specie — fiat, demand claims, reserves all zero (test 6);
//! - the engine is otherwise unperturbed: the no-M3 path is byte-identical and the M3
//!   run keeps the econ invariants (test 7; the six econ goldens + the workspace
//!   `cargo clippy -D warnings` + `cargo fmt --check` are the real gate).
//!
//! Scope is M3 **specie** money only: no banks/deposits/credit (G8b), no fiat/regime/
//! tender/taxation (G8c), no Credit/Modern era rungs.

use econ::good::{Gold, FOOD};
use sim::{NodeSpec, Settlement, SettlementConfig};
use world::Pos;

/// An M3 die-off: the `g4a` marginal-supply geometry (two gatherers feed six consumers
/// from a far, slow node, so consumers starve while gatherers survive) run on the M3
/// ledger. The consumers carry seeded specie they cannot spend (FOOD is too scarce to
/// buy), so they die **funded** — exercising the G8a estate drain that G4a/b deferred.
fn m3_dieoff() -> SettlementConfig {
    let mut cfg = SettlementConfig::m3_settlement();
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

/// The G4b `lineages` demography colony run on the M3 ledger — two households age,
/// reproduce, and inherit, with every endowment and estate a ledger move (G8a).
fn m3_lineages() -> SettlementConfig {
    let mut cfg = SettlementConfig::lineages();
    cfg.m3 = true;
    cfg
}

/// 1. Same `(seed, config)` → a byte-identical run on M3 money. Integer state, the econ
///    `Rng` consumed only at generation, nothing drawn in the loops.
#[test]
fn m3_settlement_run_is_deterministic() {
    let config = SettlementConfig::m3_settlement();
    let mut a = Settlement::generate(0xC0FFEE, &config);
    let mut b = Settlement::generate(0xC0FFEE, &config);
    a.run(60);
    b.run(60);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "an M3 settlement run must be byte-identical for the same seed + config"
    );
    assert!(
        a.is_m3(),
        "the m3_settlement config builds an M3 ledger society"
    );
}

/// 2. Whole-system conservation spans the M3 ledger (specie) + goods every econ tick;
///    the M3 ledger's OWN conservation holds; no specie or good is created or destroyed.
#[test]
fn m3_settlement_conserves() {
    let mut s = Settlement::generate(7, &SettlementConfig::m3_settlement());
    assert!(s.is_m3());
    let total_money = s.total_gold();
    for t in 0..80 {
        let report = s.econ_tick();
        // Goods conservation (the generalized whole-system identity).
        assert!(
            report.conserves(),
            "goods conservation broke at econ tick {t}"
        );
        // The M3 ledger's own conservation: specie/fiat/claims/reserves all reconcile
        // and every balance maps to a live agent.
        assert!(
            s.society().money_ledgers_reconcile(),
            "the M3 ledger must reconcile at econ tick {t}"
        );
        // Whole-system money (ledger specie + commons) is a closed, conserved total —
        // a plain settlement mints/burns no specie, so it never moves.
        assert_eq!(
            s.total_gold(),
            total_money,
            "M3 specie was created or destroyed at econ tick {t}"
        );
    }
}

/// 3. An M3 specie settlement (no banks/fiat) produces the same economic outcomes —
///    the spatial good's prices and provisioning, and the colony's sustenance — as the
///    M1 equivalent. M3 here IS M1, only ledger-accounted: the proof the wiring is
///    correct.
///
/// What is asserted **identical** every tick: the FOOD (spatial good) realized price,
/// its world→econ settlement, its consumption, and the living population — the entire
/// distance→price→provisioning→sustenance story the milestone is about. Plus the total
/// money is conserved and **equal** between the twins.
///
/// The one place they drift is the WOOD micro-price late in the run, because econ's M3
/// tick runs additional **institutional** market passes (the loan/labor markets) that
/// are inert here (nothing clears) yet still advance the order-sequence counter, which
/// perturbs CDA price-time tie-breaks. That is reused econ M3 behavior, not the sim's
/// money wiring; total provisioning still matches within a tight tolerance.
#[test]
fn m3_specie_is_economically_equivalent_to_m1() {
    const SEED: u64 = 12_345;
    const TICKS: u64 = 60;
    let mut m1 = Settlement::generate(SEED, &SettlementConfig::viable());
    let mut m3 = Settlement::generate(SEED, &SettlementConfig::m3_settlement());
    assert!(!m1.is_m3(), "viable is the closed-GOLD M1 baseline");
    assert!(m3.is_m3(), "m3_settlement runs on the M3 ledger");
    assert_eq!(
        m1.tracked_goods(),
        m3.tracked_goods(),
        "both trade the same goods"
    );

    let mut m1_consumed = 0u64;
    let mut m3_consumed = 0u64;
    for t in 0..TICKS {
        let r1 = m1.econ_tick();
        let r3 = m3.econ_tick();
        // The spatial good (FOOD): the price the milestone forms, the world→econ
        // settlement that delivers it, and the provisioning it feeds are all identical
        // on the ledger — exactly the M1 outcome.
        assert_eq!(
            m1.realized_price(FOOD),
            m3.realized_price(FOOD),
            "FOOD price diverged at tick {t}: M3 must price the spatial good like M1"
        );
        assert_eq!(
            r1.transferred_of(FOOD),
            r3.transferred_of(FOOD),
            "FOOD world→econ settlement diverged at tick {t}"
        );
        assert_eq!(
            r1.consumed_of(FOOD),
            r3.consumed_of(FOOD),
            "FOOD provisioning diverged at tick {t}"
        );
        // The colony sustains identically — same living population every tick.
        assert_eq!(
            m1.living_total(),
            m3.living_total(),
            "the living population diverged at tick {t}"
        );
        for &g in m1.tracked_goods() {
            m1_consumed += r1.consumed_of(g);
            m3_consumed += r3.consumed_of(g);
        }
    }

    // Both ran a functioning designated-money spot market (real trades cleared).
    assert!(
        !m1.society().trades.is_empty() && !m3.society().trades.is_empty(),
        "both twins must clear real trades"
    );
    // Total money is conserved and EQUAL between the twins (M1's summed Agent.gold
    // equals M3's ledger commodity base).
    assert_eq!(
        m1.total_gold(),
        m3.total_gold(),
        "the total money must match between the M1 and M3 twins"
    );
    // Total provisioning matches within a tight tolerance (only the WOOD micro-drift
    // above separates them — well under 2% over the run).
    let delta = m1_consumed.abs_diff(m3_consumed);
    assert!(
        delta * 100 <= m1_consumed * 2,
        "total provisioning must match within 2% (m1={m1_consumed}, m3={m3_consumed}, delta={delta})"
    );
}

/// 4. A death with an M3 specie balance settles: the balance drains to the estate
///    (commons here), conserved; the slot frees; caches reconcile; `remove_agent` no
///    longer refuses a funded M3 agent. (The G4a/b deferral, resolved.)
#[test]
fn m3_death_routes_ledger_balance_to_estate() {
    let mut s = Settlement::generate(1, &m3_dieoff());
    assert!(s.is_m3(), "the die-off runs on the M3 ledger");
    let total_money = s.total_gold();

    let mut deaths = 0u32;
    let mut saw_death = false;
    for t in 0..80 {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "goods+ledger conservation broke across a death at tick {t}"
        );
        assert!(
            s.society().money_ledgers_reconcile(),
            "the M3 ledger must reconcile across a death at tick {t}"
        );
        assert_eq!(
            s.total_gold(),
            total_money,
            "an M3 death created or destroyed specie at tick {t}"
        );
        deaths += report.deaths;
        if report.deaths > 0 {
            saw_death = true;
        }
    }

    assert!(
        saw_death && deaths > 0,
        "the M3 die-off must kill a colonist — a funded M3 removal no longer refuses"
    );
    // A funded colonist died holding specie, which the estate drained into the commons:
    // the ledger total fell and the commons rose by the same, leaving total_gold fixed.
    assert!(
        s.commons_gold() > Gold::ZERO,
        "a dead funded colonist's specie must drain to the commons (conserved)"
    );
    // Every dead colonist's id no longer resolves in the arena (the slot freed).
    for i in 0..s.population() {
        if !s.is_alive(i) {
            let id = s.colonist_id(i).expect("colonist id");
            assert!(
                s.society().agents.get(id).is_none(),
                "a dead M3 colonist's slot must be freed"
            );
        }
    }
}

/// 5. A birth's endowment is a conserved M3 ledger transfer (from the parent), not a
///    mint; demography runs on the M3 ledger (births AND old-age deaths conserve specie).
#[test]
fn m3_birth_endows_from_ledger() {
    let mut s = Settlement::generate(3, &m3_lineages());
    assert!(s.is_m3(), "M3 lineages runs on the ledger");
    let total_money = s.total_gold();

    let mut births = 0u32;
    for t in 0..300 {
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at econ tick {t}");
        assert!(
            s.society().money_ledgers_reconcile(),
            "the M3 ledger must reconcile through births/deaths at tick {t}"
        );
        // A birth endowment is a transfer debited from the parent's ledger balance and
        // credited to the child's; a death drains specie to an heir. Both are conserved
        // within-ledger moves, so the closed total never changes — no mint, no burn.
        assert_eq!(
            s.total_gold(),
            total_money,
            "an M3 birth/death broke specie conservation at tick {t}"
        );
        births += report.births;
    }
    assert!(
        births > 0,
        "the M3 demography colony must bear a child (the endowment is a ledger transfer)"
    );
}

/// 6. The M3 composition is pure specie — fiat, demand claims, bank reserves, fiduciary,
///    and time deposits are all zero. Banks and fiat are G8b/G8c.
#[test]
fn m3_money_has_no_fiat_or_claims() {
    let mut s = Settlement::generate(9, &SettlementConfig::m3_settlement());
    s.run(30);
    let composition = s
        .money_composition()
        .expect("an M3 settlement has a money composition");
    assert!(
        composition.public_specie > Gold::ZERO,
        "M3 specie is the circulating money and must be positive"
    );
    assert_eq!(composition.public_fiat, Gold::ZERO, "G8a has no fiat (G8c)");
    assert_eq!(
        composition.demand_claims,
        Gold::ZERO,
        "G8a has no demand claims — no banks (G8b)"
    );
    assert_eq!(
        composition.bank_reserves,
        Gold::ZERO,
        "G8a has no bank reserves — no banks (G8b)"
    );
    assert_eq!(
        composition.fiduciary,
        Gold::ZERO,
        "G8a has no fiduciary credit — no banks (G8b)"
    );
    assert_eq!(
        composition.time_deposits,
        Gold::ZERO,
        "G8a has no time deposits — no banks (G8b)"
    );
    // The specie is exactly the settlement's total money (no commons drained yet).
    assert_eq!(
        composition.public_specie,
        s.total_gold(),
        "with no deaths, all specie is held by live colonists"
    );
}

/// 7. `econ_unchanged` — the no-M3 path is byte-identical (the M3 additions are inert
///    when `m3` is false) and an M3 run keeps the econ invariants (the M3 ledger
///    reconciles and money is conserved). The six econ goldens staying byte-identical and
///    the full workspace suite + `cargo clippy --workspace --all-targets -- -D warnings`
///    + `cargo fmt --check` are the real gate; this checks the local seam.
#[test]
fn econ_unchanged() {
    // A no-M3 (closed-GOLD M1) settlement is byte-identical to a twin — the G8a
    // additions are present but unexercised — and runs no M3 ledger.
    let plain = SettlementConfig::viable();
    let mut a = Settlement::generate(42, &plain);
    let mut b = Settlement::generate(42, &plain);
    a.run(30);
    b.run(30);
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    assert!(
        !a.is_m3(),
        "viable runs the closed-GOLD M1 path, no money system"
    );
    assert!(
        a.money_composition().is_none(),
        "a no-M3 settlement surfaces no M3 composition"
    );

    // An M3 run keeps the econ invariants tick over tick: the M3 ledger reconciles and
    // no live agent's money cache drifts from its ledger balance.
    let mut m3 = Settlement::generate(3, &SettlementConfig::m3_settlement());
    for _ in 0..120 {
        let report = m3.econ_tick();
        assert!(report.conserves());
        assert!(
            m3.society().money_ledgers_reconcile(),
            "the M3 ledger reconciled every tick"
        );
    }
}
