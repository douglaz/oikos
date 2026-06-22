//! S16.3 — money from PRODUCED bread: the co-emergence scenario + DoD (the keystone).
//!
//! `frontier_money_from_cultivation` composes the strong-bar SALT machinery + S13 spatial
//! lineages + S14 forage commons + S15 own-use cultivation + `cultivation_sells_surplus`,
//! with the bread MINT OFF and the bread buffers absent — so the ONLY bread is the lineages'
//! cultivated bread, and the seeded consumers are a SALT-rich, goods-poor BUY side. The
//! question S16 puts is whether money emerges against PRODUCED (not minted) bread, closing
//! the S12 finding that the minted demographic bread was the load-bearing supply SALT
//! circulated against.
//!
//! THE OUTCOME — **(2) PRINCIPLED FAILURE**, robust across the labor/grain-flow sweep:
//! `produced_bread_does_not_monetize`. The surplus seam WORKS — the cultivators' produced
//! bread reaches the barter and is traded for SALT (a material, supply-scaling volume that
//! the provenance ledger attributes wholly to PRODUCED, minted provably zero), and bread is
//! MATERIAL in SALT's saleability (not incidental) — but SALT NEVER PROMOTES. The
//! characterized reason (timing/breadth): with the mint retired the colony is hunger-
//! stressed, so BREAD itself becomes the dominant saleable good (its acceptances dwarf
//! everything, but bread is consumed food and cannot be money), and SALT is never the
//! provisional leader, so its indirect-exchange breadth — what the strong-bar gate requires
//! — is identically ZERO. This SHARPENS S12: the minted bread was load-bearing not merely as
//! supply but because it kept hunger low enough that a durable non-food good could become
//! the saleability hub. The controls bracket it: the minted-bread S9 colony DOES promote
//! SALT (the gate works — the finding is about the supply, not a broken gate), and disabling
//! cultivation leaves zero bread→medium volume (no produced supply, nothing to trade). The
//! finding is NOT rescued by re-minting bread.

use econ::good::GoodId;
use sim::{Settlement, SettlementConfig, Vocation};

const RUN_TICKS: u64 = 2200;

fn salt_good(cfg: &SettlementConfig) -> GoodId {
    cfg.barter.as_ref().expect("a barter overlay").medium_good
}

/// The medium's accumulated INDIRECT-exchange acceptances — the breadth the strong-bar
/// gate reads. Zero here is the whole finding: produced bread never makes SALT a leader.
fn salt_indirect_acceptances(s: &Settlement, salt: GoodId) -> u64 {
    s.emergence_acceptances()
        .into_iter()
        .find(|c| c.good == salt)
        .map(|c| c.indirect_acceptances)
        .unwrap_or(0)
}

fn run(cfg: &SettlementConfig, ticks: u64) -> Settlement {
    let mut s = Settlement::generate(1, cfg);
    for _ in 0..ticks {
        s.econ_tick();
    }
    s
}

#[test]
fn money_from_cultivation_run_is_deterministic() {
    let cfg = SettlementConfig::frontier_money_from_cultivation();
    let mut a = Settlement::generate(5, &cfg);
    let mut b = Settlement::generate(5, &cfg);
    for _ in 0..RUN_TICKS {
        a.econ_tick();
        b.econ_tick();
    }
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the money-from-cultivation run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn produced_bread_does_not_monetize() {
    // THE FINDING. Where the only bread is produced (mint off, buffers absent), SALT does
    // NOT promote under the strong-bar gate — yet the produced surplus IS traded for SALT
    // (the seam works; the volume is material). The characterized reason is that SALT's
    // indirect-exchange breadth is identically zero: the hunger-stressed colony makes bread
    // the dominant saleable good, so the durable medium never leads and never accrues the
    // indirect breadth the gate requires.
    let cfg = SettlementConfig::frontier_money_from_cultivation();
    let salt = salt_good(&cfg);
    let s = run(&cfg, RUN_TICKS);

    assert!(
        s.promoted_at_tick().is_none(),
        "SALT must NOT promote against only produced bread (the principled failure)"
    );
    assert!(
        s.current_money_good().is_none(),
        "no money good emerges in the produced-only colony"
    );
    assert!(
        s.bread_for_salt_volume() > 0,
        "the surplus seam works — produced bread IS traded for the medium"
    );
    assert_eq!(
        salt_indirect_acceptances(&s, salt),
        0,
        "the characterized reason: SALT accrues ZERO indirect-exchange breadth"
    );
}

#[test]
fn the_monetizing_bread_is_produced_not_minted() {
    // Even though SALT does not monetize, the bread that reaches the medium is provably
    // PRODUCED — the provenance ledger attributes the whole bread→medium volume to produced,
    // and the minted contribution is provably ZERO (mint off, buffers absent). The failure
    // is monetization, not provenance; re-minting bread is explicitly not the rescue.
    let cfg = SettlementConfig::frontier_money_from_cultivation();
    let s = run(&cfg, RUN_TICKS);

    let volume = s.bread_for_salt_volume();
    assert!(volume > 0, "produced bread reaches the medium market");
    let (produced, minted) = s.bread_for_salt_volume_by_provenance();
    assert_eq!(
        produced + minted,
        volume,
        "the provenance split sums to the volume"
    );
    assert_eq!(
        minted, 0,
        "the minted contribution is provably zero (mint off, buffers absent)"
    );
    assert_eq!(
        produced, volume,
        "the bread that reaches the medium is produced"
    );
}

#[test]
fn produced_bread_is_material_not_incidental() {
    // The third-outcome guard. The FALSE success (SALT promotes via WOOD/forage breadth with
    // produced bread incidental) does NOT occur — SALT does not promote at all. And the
    // produced bread that reached the medium is MATERIAL in its saleability (bread is among
    // the medium's counterpart goods), so the finding is "produced bread reached the medium
    // but could not monetize it", not "produced bread was incidental".
    let cfg = SettlementConfig::frontier_money_from_cultivation();
    let s = run(&cfg, RUN_TICKS);

    assert!(
        s.promoted_at_tick().is_none(),
        "no promotion => the false-success third outcome is ruled out"
    );
    assert!(
        s.bread_in_medium_saleability(),
        "produced bread is MATERIAL in the medium's saleability (not incidental)"
    );
    let (produced, minted) = s.bread_for_salt_volume_by_provenance();
    assert!(
        produced > 0 && minted == 0,
        "the material bread→medium volume is produced bread"
    );
}

#[test]
fn cultivation_division_of_labor_forms() {
    // Under forage pressure the spatial LINEAGES cultivate a surplus and SELL it; the
    // SALT-holding CONSUMERS buy and do NOT self-cultivate (the buy/sell split). A produced-
    // bread market forms (not just own-use), with a non-trivial, sustained sale volume.
    let cfg = SettlementConfig::frontier_money_from_cultivation();
    let mut s = Settlement::generate(1, &cfg);

    let mut any_consumer_worked = false;
    let mut any_lineage_cultivated = false;
    let mut volume_at_half = 0u64;
    for tick in 0..RUN_TICKS {
        s.econ_tick();
        if tick == RUN_TICKS / 2 {
            volume_at_half = s.bread_for_salt_volume();
        }
        for i in 0..s.population() {
            if !s.is_alive(i) {
                continue;
            }
            if s.household_of(i).is_none()
                && s.vocation_of(i) == Some(Vocation::Consumer)
                && (s.is_cultivating(i) || s.is_foraging(i))
            {
                any_consumer_worked = true;
            }
            if s.household_of(i).is_some() && s.is_cultivating(i) {
                any_lineage_cultivated = true;
            }
        }
    }
    assert!(
        any_lineage_cultivated,
        "the spatial lineages must cultivate the produced bread"
    );
    assert!(
        !any_consumer_worked,
        "the SALT consumers must stay the buy side (never forage/cultivate)"
    );
    // Sustained: the sale volume keeps growing across the run (a real market, not a one-off).
    assert!(
        volume_at_half > 0 && s.bread_for_salt_volume() > volume_at_half,
        "the produced-bread sale volume must be non-trivial and sustained"
    );
}

#[test]
fn money_from_cultivation_conserves() {
    // Whole-system conservation every tick (grain regen the source; grain consumed_as_input
    // → bread produced → consumed/traded), no minted food (endowment[staple] == 0), and the
    // produced-bread provenance counters conserve.
    let cfg = SettlementConfig::frontier_money_from_cultivation();
    let bread = cfg.chain.as_ref().expect("chain").content.bread();
    let mut s = Settlement::generate(1, &cfg);
    for tick in 0..RUN_TICKS {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "whole-system conservation must hold at tick {tick}"
        );
        assert_eq!(
            report.endowment_of(bread),
            0,
            "the bread staple must never be minted at tick {tick}"
        );
    }
    let (credited, sunk) = s.produced_bread_credited_and_sunk();
    assert!(credited > 0, "the cultivators produced bread");
    assert_eq!(
        credited,
        sunk + s.produced_bread_held(),
        "the produced-bread provenance counters must conserve over the run"
    );
}

#[test]
fn controls_close_the_finding() {
    // (a) RE-ENABLE the minted bread: the strong-bar S9 colony DOES promote SALT — proving
    // the gate works, so the produced-only failure is about the SUPPLY, not a broken gate.
    let s9_cfg = SettlementConfig::frontier_coemergent_strong();
    let s9_salt = salt_good(&s9_cfg);
    let s9 = run(&s9_cfg, 1500);
    assert!(
        s9.promoted_at_tick().is_some(),
        "control (a): the minted-bread S9 colony reproduces money emergence"
    );
    assert_eq!(
        s9.current_money_good(),
        Some(s9_salt),
        "control (a): the good that promotes is SALT"
    );

    // (b) DISABLE cultivation: with no produced supply there is no bread to trade — the
    // bread→medium volume is zero and nothing promotes (the S12 finding: no produced supply).
    let mut nocult_cfg = SettlementConfig::frontier_money_from_cultivation();
    nocult_cfg
        .chain
        .as_mut()
        .expect("chain")
        .own_use_cultivation = false;
    let nocult = run(&nocult_cfg, RUN_TICKS);
    assert_eq!(
        nocult.bread_for_salt_volume(),
        0,
        "control (b): with cultivation off there is no produced bread to trade"
    );
    assert!(
        nocult.promoted_at_tick().is_none(),
        "control (b): no produced supply => no monetization"
    );
}

#[test]
fn produced_bread_market_scales_with_supply() {
    // Robustness (the labor/grain-flow sweep, encoded): raising the cultivated-grain flow
    // raises the produced bread→medium VOLUME, yet SALT still never promotes — a genuine
    // outcome-2 band, not a knife-edge that passes at one budget. (A wider sweep across
    // grain flow × SALT direct-use period × consumer count confirms `promoted == false`
    // and `salt_indirect == 0` in every cell; this pins the monotone supply response.)
    let cfg_for = |grain_regen: u32| {
        let mut cfg = SettlementConfig::frontier_money_from_cultivation();
        let grain = cfg.chain.as_ref().unwrap().content.grain();
        for node in cfg.nodes.iter_mut() {
            if node.good == grain {
                node.regen = grain_regen;
            }
        }
        cfg
    };
    let lean = run(&cfg_for(4), RUN_TICKS);
    let rich = run(&cfg_for(16), RUN_TICKS);

    assert!(
        lean.promoted_at_tick().is_none() && rich.promoted_at_tick().is_none(),
        "SALT must not promote at any grain flow (not a knife-edge)"
    );
    assert!(
        rich.bread_for_salt_volume() > lean.bread_for_salt_volume(),
        "more produced supply => more produced bread→medium volume (the monotone response)"
    );
}

#[test]
fn goldens_unchanged() {
    // With the S16 flags off, the shared-path goldens are BYTE-IDENTICAL — S16 perturbs no
    // existing scenario. The `lineages` demographic golden and the S15 `frontier_cultivation`
    // golden are the tripwires (any shared-path byte shift fails here); the broader S5-S15 +
    // econ + emergence suites and the `canonical_bytes_include_*` settlement unit tests
    // (the S16 flag + the per-agent provenance counters in the identity) are the rest.
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
        digest(&SettlementConfig::frontier_cultivation(), 300),
        0xd8cfd0b2e9674373,
        "the S15 cultivation golden must be byte-identical (the S16 ledger is fully gated)"
    );
}
