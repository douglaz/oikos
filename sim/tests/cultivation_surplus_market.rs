//! S16.1 — the cultivated-bread→market surplus seam + the buy/sell split.
//!
//! With `cultivation_sells_surplus` on, two gated behaviors engage on the S15 cultivation
//! colony composed with a SALT-holding consumer buy side:
//!
//! 1. The **buy/sell split** (Codex P1c): forage/cultivation eligibility is scoped to
//!    LINEAGE spatial members (`household.is_some() && spatial_active`), so the seeded
//!    SALT-rich consumers (`household: None`) never self-forage/cultivate and stay the pure
//!    BUY side — while the lineages do the cultivating.
//!
//! 2. The **surplus seam**: a satiated cultivator leaves its surplus produced bread free in
//!    stock, and the EXISTING S9 direct/indirect barter offers it for the cultivator's
//!    normal unsatisfied wants (no special SALT want). The bread reaches the market and is
//!    traded for SALT — the realized `bread_for_salt_volume` is positive.
//!
//! These tests pin the MECHANISM (the seam connects, the split holds, conservation is
//! preserved). Whether the produced bread then *monetizes* SALT (promotion) is the S16.3
//! question — and the finding is that it does not (the produced-bread market forms but
//! SALT never promotes; see `money_from_produced_bread.rs`).

use econ::good::GoodId;
use sim::{Settlement, SettlementConfig, Vocation};

const RUN_TICKS: u64 = 1500;

fn bread_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain.as_ref().expect("chain").content.bread()
}

/// A non-lineage seeded consumer (the BUY side): `Vocation::Consumer`, no household.
fn is_seeded_consumer(s: &Settlement, i: usize) -> bool {
    s.household_of(i).is_none() && s.vocation_of(i) == Some(Vocation::Consumer)
}

#[test]
fn the_buy_sell_split_holds() {
    // The seeded SALT consumers NEVER cultivate (or forage); only LINEAGE members do.
    let cfg = SettlementConfig::frontier_money_from_cultivation();
    let mut s = Settlement::generate(1, &cfg);

    let mut any_consumer_worked = false;
    let mut any_lineage_cultivated = false;
    for _ in 0..RUN_TICKS {
        s.econ_tick();
        for i in 0..s.population() {
            if !s.is_alive(i) {
                continue;
            }
            if is_seeded_consumer(&s, i) && (s.is_cultivating(i) || s.is_foraging(i)) {
                any_consumer_worked = true;
            }
            if s.household_of(i).is_some() && s.is_cultivating(i) {
                any_lineage_cultivated = true;
            }
        }
    }
    assert!(
        !any_consumer_worked,
        "the buy/sell split must keep the seeded SALT consumers off the forage/cultivate path"
    );
    assert!(
        any_lineage_cultivated,
        "under forage pressure the spatial lineages must cultivate"
    );
}

#[test]
fn surplus_produced_bread_is_traded_for_salt() {
    // The seam connects: a satiated cultivator's surplus bread reaches the EXISTING barter
    // and is traded for SALT, so the realized bread-for-SALT volume is positive.
    let cfg = SettlementConfig::frontier_money_from_cultivation();
    let mut s = Settlement::generate(1, &cfg);
    for _ in 0..RUN_TICKS {
        s.econ_tick();
    }
    assert!(
        s.bread_for_salt_volume() > 0,
        "the cultivated surplus bread must reach the SALT market (bread_for_salt_volume > 0)"
    );
}

#[test]
fn surplus_seam_conserves_every_tick() {
    // The own-use vs sale boundary never double-counts: whole-system conservation holds
    // every tick (the surplus is sold only after eating to satiety, only free stock).
    let cfg = SettlementConfig::frontier_money_from_cultivation();
    let bread = bread_good(&cfg);
    let mut s = Settlement::generate(1, &cfg);
    for tick in 0..RUN_TICKS {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "whole-system conservation must hold at tick {tick}"
        );
        // No minted food: the staple is never an endowment (own-labor path, mint off).
        assert_eq!(
            report.endowment_of(bread),
            0,
            "the bread staple must never be minted (endowment) at tick {tick}"
        );
    }
}

#[test]
fn surplus_seam_run_is_deterministic() {
    let cfg = SettlementConfig::frontier_money_from_cultivation();
    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    for _ in 0..RUN_TICKS {
        a.econ_tick();
        b.econ_tick();
    }
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the surplus-seam run must be byte-identical for the same (seed, config)"
    );
}
