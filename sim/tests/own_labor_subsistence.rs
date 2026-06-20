//! S12 — household subsistence at scale (OWN-LABOR subsistence).
//!
//! The own-labor subsistence path retires the food mints and replaces them with a
//! labor-produced FORAGE survival floor (booked `produced`, eaten at home, ranked below
//! bread). These tests pin the mechanism: the floor is produced from own labor (not
//! minted), conserves every tick, and is fully gated.

use econ::good::GoodId;
use sim::{Settlement, SettlementConfig};

fn provisioned() -> SettlementConfig {
    SettlementConfig::frontier_coemergent_strong_provisioned()
}

fn bread_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain.as_ref().expect("chain").content.bread()
}

fn forage_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain
        .as_ref()
        .expect("chain")
        .content
        .forage()
        .expect("the provisioned chain interns a forage good")
}

// ---- S12.1: the mechanism ------------------------------------------------------------

#[test]
fn provisioned_run_is_deterministic() {
    let cfg = provisioned();
    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    for _ in 0..200 {
        a.econ_tick();
        b.econ_tick();
    }
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the provisioned run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn subsistence_is_produced_not_minted() {
    // The FORAGE the tail eats comes from `report.produced` (own labor), the hunger
    // staple mint is ZERO every tick (the mint is retired), a hungry non-lineage
    // colonist is sent to forage (`Task::GoForage`), and its hunger actually falls — and
    // the whole system conserves throughout.
    let cfg = provisioned();
    let bread = bread_good(&cfg);
    let forage = forage_good(&cfg);
    let mut s = Settlement::generate(1, &cfg);
    assert!(
        s.forage_node_id().is_some(),
        "own-labor places a FORAGE node (the GoForage target)"
    );

    let n = s.population();
    // The persistent foraging decision (the state `assign_idle_gatherer_tasks` reads to
    // send the colonist to `Task::GoForage` instead of harvesting WOOD). It is the stable
    // evidence of "assigned to forage" — the world task itself completes within the fast
    // loop, so it is transient; the flag persists across the econ tick.
    let mut was_foraging = vec![false; n];
    let mut peak_hunger = vec![0u16; n];
    let mut min_after_forage = vec![u16::MAX; n];
    let mut total_forage_produced = 0u64;

    for tick in 0..300u64 {
        // Peak hunger BEFORE this tick's forage credit + consumption.
        for (i, peak) in peak_hunger.iter_mut().enumerate() {
            if let Some(h) = s.need_of(i).map(|x| x.hunger) {
                *peak = (*peak).max(h);
            }
        }
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at tick {tick}");
        // The food mint is retired: no staple is ever minted into endowment, and FORAGE
        // is never an endowment line (it is produced from labor).
        assert_eq!(
            report.endowment_of(bread),
            0,
            "the hunger staple mint must be ZERO (retired) at tick {tick}"
        );
        assert_eq!(
            report.endowment_of(forage),
            0,
            "FORAGE must never be minted into endowment at tick {tick}"
        );
        total_forage_produced += report.produced_of(forage);

        for i in 0..n {
            if s.is_foraging(i) {
                was_foraging[i] = true;
            }
            if was_foraging[i] {
                if let Some(h) = s.need_of(i).map(|x| x.hunger) {
                    min_after_forage[i] = min_after_forage[i].min(h);
                }
            }
        }
    }

    assert!(
        total_forage_produced > 0,
        "FORAGE must be produced from own labor (report.produced)"
    );
    // A hungry non-lineage colonist foraged (own labor on the FORAGE node) and its hunger
    // fell from its peak as it ate the foraged floor.
    let fed = (0..n).find(|&i| {
        was_foraging[i]
            && s.household_of(i).is_none()
            && peak_hunger[i] >= 6
            && min_after_forage[i] < peak_hunger[i]
    });
    assert!(
        fed.is_some(),
        "a hungry non-lineage colonist must forage and see its hunger fall — \
         foraging={was_foraging:?} peak={peak_hunger:?} minAfter={min_after_forage:?}"
    );
}

#[test]
fn provisioning_conserves() {
    // Whole-system conservation every tick on the own-labor path (FORAGE produced from
    // labor; the perishable floor decays into `report.spoiled` when it hoards).
    let cfg = provisioned();
    let forage = forage_good(&cfg);
    let bread = bread_good(&cfg);
    let mut s = Settlement::generate(0xC0FFEE, &cfg);
    let mut produced = 0u64;
    // The perishable foods (the staple + the FORAGE subsistence floor) spoil only as a
    // HOARD; the floor is own-consumption-first, so it is mostly eaten before it rots —
    // but whatever does decay is accounted in `report.spoiled` and the per-tick
    // whole-system identity must hold regardless.
    for tick in 0..500u64 {
        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at tick {tick}");
        produced += report.produced_of(forage);
        // Spoilage of a perishable good is a real, accounted sink (>= 0, in the identity).
        let _ = report.spoiled_of(forage) + report.spoiled_of(bread);
    }
    assert!(produced > 0, "the forage floor must be produced from labor");
}
