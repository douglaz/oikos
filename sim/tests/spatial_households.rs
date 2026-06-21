//! S13 — spatial households: unify the colonist model so the reproducing
//! population can forage.
//!
//! Behind `DemographyConfig::spatial_households` (default off), every lineage
//! member — founders at generation and newborns at birth — is given a **world
//! agent** at its exact econ `AgentId` (`world_id == econ_id` by construction,
//! even after a death recycled an arena slot), so the colony that GROWS can now be
//! assigned forage/gather/haul tasks like anyone else. This is the structural
//! unification that unblocks the scarcity arc (S14+); it is purely structural —
//! no forage scarcity, cultivation, or mortality is added here.
//!
//! The DoD: id coincidence holds across births AND deaths, conservation and
//! determinism hold, the reproducing population can forage (the one scoped
//! behavior change), and with the flag off every existing scenario/golden is
//! byte-identical.

use std::collections::{BTreeMap, BTreeSet};

use econ::agent::AgentId;
use econ::good::GoodId;
use sim::{Settlement, SettlementConfig};

/// The shipped S13 scenario: the G5b frontier with spatial households on and NO
/// forage scarcity (the hearth still feeds the lineages, so the spatial members
/// sit idle and demography is unchanged — the milestone grants the capability).
fn spatial_frontier() -> SettlementConfig {
    SettlementConfig::frontier_spatial_households()
}

/// The S12 own-labor (food-mints-retired) colony with spatial households flipped
/// on — the config where the forage *motivation* exists, so a hungry lineage
/// member is actually assigned a forage task (the S13.3 eligibility relaxation,
/// exercised live).
fn provisioned_spatial() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_coemergent_strong_provisioned();
    cfg.demography
        .as_mut()
        .expect("the provisioned config carries a demography overlay")
        .spatial_households = true;
    cfg
}

fn forage_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain
        .as_ref()
        .expect("the provisioned chain")
        .content
        .forage()
        .expect("own-labor subsistence interns a forage good")
}

#[test]
fn spatial_households_run_is_deterministic() {
    // Byte-identical for the same (seed, config): the new world agents + the
    // spatial-households flag enter canonical_bytes deterministically (the mirrored
    // arena generations come from the already-digested econ arena; no live RNG).
    let cfg = spatial_frontier();
    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    for _ in 0..500u64 {
        a.econ_tick();
        b.econ_tick();
    }
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the spatial-households run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn lineage_members_are_spatial() {
    // With the flag on, every founder AND every mid-run newborn has a world agent
    // whose world_id == econ_id, and EVERY living colonist has a spatial world agent.
    // The right invariant is "every living colonist is spatial", NOT "total world-agent
    // count == colonist count" (the world may also hold resident traders).
    let cfg = spatial_frontier();
    let mut s = Settlement::generate(1, &cfg);
    let initial_pop = s.population();
    for _ in 0..800u64 {
        s.econ_tick();
    }

    let mut living = 0usize;
    let mut living_lineage = 0usize;
    let mut living_newborn_is_spatial = false;
    let mut reused_slot_is_spatial = false;
    for i in 0..s.population() {
        if !s.is_alive(i) {
            continue;
        }
        living += 1;
        let id = s.colonist_id(i).expect("a living colonist has an id");
        // world_id == econ_id by construction: the world agent resolves at the EXACT
        // econ id (generation included), so a `Some` position proves coincidence.
        assert!(
            s.world().agent_pos(id).is_some(),
            "living colonist {id} has no world agent at its exact econ id"
        );
        if s.household_of(i).is_some() {
            living_lineage += 1;
        }
        if i >= initial_pop {
            // A colonist born mid-run (its slot is past the generation roster).
            living_newborn_is_spatial = true;
        }
        if id.generation() >= 1 {
            // A newborn that reused a freed arena slot (born after a death).
            reused_slot_is_spatial = true;
        }
    }
    assert!(
        living_lineage > 0,
        "the frontier carries living lineage members"
    );
    assert!(
        living_newborn_is_spatial,
        "the run must include a living mid-run newborn, and it must be spatial"
    );
    assert!(
        reused_slot_is_spatial,
        "a newborn born at a reused slot#gen must also be spatial"
    );
    // Sanity: the world holds at least one agent per living colonist (it may hold more —
    // resident traders — so this is a lower bound, never an equality).
    assert!(s.world().agent_ids().len() >= living);
}

#[test]
fn lineage_members_can_forage() {
    // The ONE scoped behavior change: with spatial households on AND own-labor
    // subsistence active, a hungry LINEAGE member is now forage-eligible — the
    // own-labor phase ASSIGNS it a forage task (its world-task slot is occupied
    // foraging, the `foraging` flag), it PRODUCES the FORAGE floor from its OWN labor
    // (booked `produced`, credited straight into its econ stock), and its hunger FALLS
    // as it eats. The reproducing population can now forage — the structural goal.
    //
    // NB on movement: the FORAGE node sits ON the exchange tile ("eaten at home", S12),
    // and the world round trip completes inside one econ tick's fast loop — so a
    // once-per-tick world sample cannot observe travel/carry. The OBSERVABLE, attributable
    // proof that the world→econ loop ran for a lineage member is its labor-produced FORAGE
    // (only a colonist that completes a forage task is credited) and the hunger it relieves.
    let cfg = provisioned_spatial();
    let forage = forage_good(&cfg);
    let mut s = Settlement::generate(1, &cfg);
    assert!(
        s.forage_node_id().is_some(),
        "own-labor subsistence places a FORAGE node (the GoForage target)"
    );

    let mut foraged_lineage: BTreeSet<AgentId> = BTreeSet::new();
    let mut peak_before: BTreeMap<AgentId, u16> = BTreeMap::new();
    let mut min_after_forage: BTreeMap<AgentId, u16> = BTreeMap::new();
    let mut total_forage = 0u64;

    for tick in 0..600u64 {
        // Peak hunger BEFORE this tick's forage credit + consumption (lineage only).
        for i in 0..s.population() {
            if s.is_alive(i) && s.household_of(i).is_some() {
                if let (Some(id), Some(need)) = (s.colonist_id(i), s.need_of(i)) {
                    let entry = peak_before.entry(id).or_insert(0);
                    *entry = (*entry).max(need.hunger);
                }
            }
        }

        let report = s.econ_tick();
        assert!(report.conserves(), "conservation broke at tick {tick}");
        total_forage += report.produced_of(forage);

        for i in 0..s.population() {
            if !s.is_alive(i) || s.household_of(i).is_none() {
                continue;
            }
            let Some(id) = s.colonist_id(i) else { continue };
            // Every lineage member is spatial — it must resolve a world agent at its id.
            assert!(
                s.world().agent_pos(id).is_some(),
                "lineage member {id} is not spatial"
            );
            if s.is_foraging(i) {
                foraged_lineage.insert(id);
            }
            if foraged_lineage.contains(&id) {
                if let Some(need) = s.need_of(i) {
                    let entry = min_after_forage.entry(id).or_insert(u16::MAX);
                    *entry = (*entry).min(need.hunger);
                }
            }
        }
    }

    assert!(
        !foraged_lineage.is_empty(),
        "a spatial lineage member must be assigned to forage (the eligibility relaxation)"
    );
    assert!(
        total_forage > 0,
        "the forage floor must be produced from own labor (report.produced)"
    );
    // A hungry lineage member foraged (occupied its world-task slot) and its hunger fell
    // from its peak as it ate the floor its own labor produced — the world→econ loop.
    let fed = foraged_lineage.iter().find(|&&id| {
        peak_before.get(&id).copied().unwrap_or(0) >= 6
            && min_after_forage.get(&id).copied().unwrap_or(u16::MAX)
                < peak_before.get(&id).copied().unwrap_or(0)
    });
    assert!(
        fed.is_some(),
        "a hungry spatial lineage member must forage and see its hunger fall — \
         foraged={foraged_lineage:?} peak={peak_before:?} min={min_after_forage:?}"
    );
}

#[test]
fn feeding_and_demography_unchanged_in_substance() {
    // With the flag on but NO forage scarcity (the frontier hearth still feeds the
    // lineages), the spatial world agents sit Idle — so feeding (per-colonist hunger)
    // and demography (births / deaths / population) match the non-spatial frontier
    // baseline tick for tick. The milestone adds capability, not a behavior change.
    let base = SettlementConfig::frontier();
    let spatial = spatial_frontier();
    let mut b = Settlement::generate(1, &base);
    let mut s = Settlement::generate(1, &spatial);
    for tick in 0..1200u64 {
        b.econ_tick();
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "the spatial frontier broke conservation at tick {tick}"
        );
        assert_eq!(
            b.births_total(),
            s.births_total(),
            "births diverged at tick {tick}"
        );
        assert_eq!(
            b.old_age_deaths_total(),
            s.old_age_deaths_total(),
            "deaths diverged at tick {tick}"
        );
        assert_eq!(
            b.population(),
            s.population(),
            "population diverged at tick {tick}"
        );
    }
    // Per-colonist feeding is identical at the end (the slot order coincides — both runs
    // share the same generation roster and the same birth trajectory).
    for i in 0..b.population() {
        assert_eq!(
            b.is_alive(i),
            s.is_alive(i),
            "liveness diverged at colonist {i}"
        );
        assert_eq!(
            b.need_of(i).map(|n| (n.hunger, n.warmth, n.rest)),
            s.need_of(i).map(|n| (n.hunger, n.warmth, n.rest)),
            "feeding diverged for colonist {i}"
        );
    }
}

#[test]
fn spatial_households_conserves() {
    // Whole-system conservation every tick across the founder + newborn world agents —
    // both when they actually forage/gather (the own-labor + spatial config, active
    // world flows) and in the shipped no-scarcity scenario. Relocation + node regen are
    // the only sources; no new conservation surface is introduced.
    for cfg in [provisioned_spatial(), spatial_frontier()] {
        let mut s = Settlement::generate(0xC0FFEE, &cfg);
        for tick in 0..700u64 {
            let report = s.econ_tick();
            assert!(report.conserves(), "conservation broke at tick {tick}");
        }
    }
}

#[test]
fn id_coincidence_holds_across_births_and_deaths() {
    // The load-bearing invariant: over a long run with births AND deaths, world_id ==
    // econ_id for EVERY living colonist on EVERY tick — including a newborn born at a
    // reused slot#gen (after a death recycled the arena slot). No world-agent leak.
    let cfg = spatial_frontier();
    let mut s = Settlement::generate(1, &cfg);
    let mut saw_reused_slot = false;
    for tick in 0..1200u64 {
        s.econ_tick();
        for i in 0..s.population() {
            if !s.is_alive(i) {
                continue;
            }
            let id = s.colonist_id(i).expect("a living colonist has an id");
            assert!(
                s.world().agent_pos(id).is_some(),
                "world_id != econ_id for living colonist {id} at tick {tick}"
            );
            if id.generation() >= 1 {
                saw_reused_slot = true;
            }
        }
    }
    assert!(
        s.births_total() > 0 && s.old_age_deaths_total() > 0,
        "the run must exercise both births and deaths"
    );
    assert!(
        saw_reused_slot,
        "the run must exercise a newborn born at a reused slot#gen (birth after death)"
    );
}

#[test]
fn goldens_unchanged() {
    // Flag off is the default and is byte-identical to an explicitly-false config (the
    // new field never perturbs a flag-off run), and flipping the flag on changes the
    // bytes — the gating is real, in one place. The cross-history byte-identity of the
    // S5-S12 + econ + emergence + `lineages` goldens is the unchanged existing suites'
    // job (the builders are unmutated); the `canonical_bytes_include_spatial_households`
    // regression (in the settlement unit tests) pins the flag's own identity.
    let builders: [fn() -> SettlementConfig; 2] =
        [SettlementConfig::lineages, SettlementConfig::frontier];
    let run = |cfg: &SettlementConfig| {
        let mut s = Settlement::generate(1, cfg);
        s.run(300);
        (s.canonical_bytes(), s.digest())
    };
    for build in builders {
        let (off_bytes, off_digest) = run(&build());
        let mut explicit_off = build();
        explicit_off
            .demography
            .as_mut()
            .expect("demography")
            .spatial_households = false;
        let (explicit_bytes, _) = run(&explicit_off);
        let mut on = build();
        on.demography
            .as_mut()
            .expect("demography")
            .spatial_households = true;
        let (on_bytes, on_digest) = run(&on);

        assert_eq!(
            off_bytes, explicit_bytes,
            "the default-off run must equal the explicit-off run (the field is inert off)"
        );
        // Determinism of the flag-off run (the byte-identical tripwire).
        assert_eq!(
            run(&build()).1,
            off_digest,
            "the flag-off run must be deterministic"
        );
        assert_ne!(
            off_bytes, on_bytes,
            "the spatial-households flag must change the bytes when on"
        );
        assert_ne!(off_digest, on_digest);
    }
}
