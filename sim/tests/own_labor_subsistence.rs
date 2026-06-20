//! S12 — household subsistence at scale (OWN-LABOR subsistence).
//!
//! The own-labor subsistence path retires the food mints and replaces them with a
//! labor-produced FORAGE survival floor (booked `produced`, eaten at home, ranked below
//! bread). These tests pin the mechanism (produced not minted, conserving, gated) and
//! the milestone's falsifiable core.

use econ::good::GoodId;
use sim::{Settlement, SettlementConfig, Vocation};

/// (mean, p95, max, chronically-hungry count) over the living roster, where "chronic" is
/// hunger >= `threshold`. The provisioning baseline metric (`provisioning_at_scale.rs`).
fn hunger_stats(s: &Settlement, threshold: u16) -> (u64, u16, u16, usize) {
    let mut h: Vec<u16> = (0..s.population())
        .filter(|&i| s.is_alive(i))
        .filter_map(|i| s.need_of(i).map(|n| n.hunger))
        .collect();
    h.sort_unstable();
    if h.is_empty() {
        return (0, 0, 0, 0);
    }
    let mean = h.iter().map(|&x| u64::from(x)).sum::<u64>() / h.len() as u64;
    let p95 = h[(h.len() * 95 / 100).min(h.len() - 1)];
    let max = *h.last().unwrap();
    let chronic = h.iter().filter(|&&x| x >= threshold).count();
    (mean, p95, max, chronic)
}

fn provisioned() -> SettlementConfig {
    SettlementConfig::frontier_coemergent_strong_provisioned()
}

fn provisioned_with_yield(yield_units: u32) -> SettlementConfig {
    let mut cfg = provisioned();
    cfg.chain.as_mut().expect("chain").forage_yield = yield_units;
    cfg
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

// ---- S12.2: the falsifiable core -----------------------------------------------------

#[test]
fn forage_floor_feeds_the_tail() {
    // The labor-produced floor pulls tail hunger strictly below the semi-hungry S11
    // baseline on every axis (mean / p95 / max / chronic count), and it does not drift.
    let baseline = SettlementConfig::frontier_coemergent_strong_entrepreneurial();
    let mut b = Settlement::generate(1, &baseline);
    b.run(1000);
    let (b_mean, b_p95, b_max, b_chronic) = hunger_stats(&b, 8);

    let cfg = provisioned();
    let mut s = Settlement::generate(1, &cfg);
    s.run(1000);
    let (mean_a, p95_a, max_a, chronic_a) = hunger_stats(&s, 8);
    s.run(600);
    let (mean_b, p95_b, max_b, chronic_b) = hunger_stats(&s, 8);

    assert!(
        mean_a < b_mean && p95_a < b_p95 && max_a < b_max && chronic_a < b_chronic,
        "provisioned tail hunger must be below the baseline ({mean_a}/{p95_a}/{max_a}/{chronic_a} \
         vs {b_mean}/{b_p95}/{b_max}/{b_chronic})"
    );
    // Non-drifting: the tail stays bounded over the further 600 ticks.
    assert!(
        mean_b <= b_mean && p95_b < b_p95 && max_b < b_max && chronic_b < b_chronic,
        "the bounded tail must not drift back up ({mean_b}/{p95_b}/{max_b}/{chronic_b})"
    );
}

#[test]
fn no_own_labor_production_control_stays_hungry() {
    // The control: own-labor ON (mints retired) but `forage_yield = 0` — no labor floor
    // is produced. The tail is materially hungrier than the provisioned colony, proving
    // the produced FORAGE floor (not some side effect of the gate) is what feeds it.
    let mut control = Settlement::generate(1, &provisioned_with_yield(0));
    control.run(1000);
    let (c_mean, _, _, c_chronic) = hunger_stats(&control, 8);

    let mut fed = Settlement::generate(1, &provisioned());
    fed.run(1000);
    let (f_mean, _, _, f_chronic) = hunger_stats(&fed, 8);

    assert!(
        c_mean > f_mean && c_chronic > f_chronic,
        "the no-forage control must stay hungrier than the provisioned colony \
         (control {c_mean}/{c_chronic} vs fed {f_mean}/{f_chronic})"
    );
}

#[test]
fn producer_food_path_is_feasible() {
    // Producer-hunger sanity: the latent producers (the only producer role present, since
    // money never emerges on the own-labor path) are an eligible part of the forage set,
    // so retiring the producer staple mint leaves them a feasible food path — none is
    // left permanently stranded at the hunger ceiling.
    let cfg = provisioned();
    let mut s = Settlement::generate(7, &cfg);
    s.run(1000);
    let mut latent_seen = false;
    let mut worst = 0u16;
    for i in 0..s.population() {
        if !s.is_alive(i) {
            continue;
        }
        if s.vocation_of(i) == Some(Vocation::Unassigned) {
            latent_seen = true;
            worst = worst.max(s.need_of(i).map(|n| n.hunger).unwrap_or(0));
        }
    }
    assert!(latent_seen, "the provisioned config seeds latent producers");
    assert!(
        worst <= 8,
        "a latent producer must keep a feasible food path (forage), not starve at the \
         ceiling — worst latent-producer hunger was {worst}"
    );
}
