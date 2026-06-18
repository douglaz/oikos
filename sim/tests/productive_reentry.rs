//! S6.1 — the productive re-entry phase (the core, gated, default-OFF).
//!
//! A hungry, unprovisioned spatial non-lineage colonist adopts edible-grain
//! gathering on its own value scale — an idle [`Vocation::Consumer`] (no node,
//! produces nothing) and a [`Vocation::Gatherer`] mis-allocated to a non-edible
//! (WOOD) node each become a grain gatherer once hunger reaches the entry
//! threshold. These tests prove the flip is REAL provisioning (grain actually
//! accumulates in econ stock and hunger actually falls), not a relabel, and that
//! the phase is inert when off (the disabled-phase byte-identical regression).

use sim::{Settlement, SettlementConfig, Vocation};

/// `frontier_endogenous` with the S6 re-entry phase turned ON (entry threshold
/// only — the S6.2 hysteresis is layered on top later). The economy is otherwise
/// the endogenous one, so the stranded set (4 consumers + 4 WOOD gatherers, all
/// pinned at the hunger ceiling) is present to provision.
fn reentry_on() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_endogenous();
    let chain = cfg
        .chain
        .as_mut()
        .expect("the endogenous config has a chain");
    chain.productive_reentry = true;
    chain.reentry_hunger_in = 8;
    cfg
}

#[test]
fn reentry_phase_off_is_byte_identical() {
    // The endogenous scenario calls `run_productive_reentry` every tick, but the
    // phase is default-OFF: it returns immediately, mutating nothing. So the run is
    // byte-identical to the pre-S6 endogenous economy — and, since the entry
    // threshold cannot steer a phase that never runs, byte-identical regardless of
    // its value (the gated, additive seam is inert when unused — the tripwire).
    let off = SettlementConfig::frontier_endogenous();
    assert!(
        !off.chain.as_ref().unwrap().productive_reentry,
        "the endogenous scenario must keep re-entry OFF"
    );
    let mut other = SettlementConfig::frontier_endogenous();
    other.chain.as_mut().unwrap().reentry_hunger_in = 1; // unused: phase still OFF

    let mut a = Settlement::generate(0xC0FFEE, &off);
    let mut b = Settlement::generate(0xC0FFEE, &other);
    a.run(800);
    b.run(800);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "with the phase OFF the run must be byte-identical regardless of the unused threshold"
    );
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn reentry_feeds_a_stranded_consumer() {
    // S6.1 (the highest-risk claim): a stranded, hungry, cash-poor non-lineage
    // consumer becomes a grain GATHERER on the edible grain node, ACTUALLY
    // accumulates grain in its econ stock, and its hunger FALLS as it eats what it
    // gathers — proving real provisioning, not just a relabeled vocation.
    let config = reentry_on();
    let chain = config.chain.as_ref().unwrap();
    let h_in = chain.reentry_hunger_in;
    let grain = chain.content.grain();
    let n_consumers = usize::from(config.consumers);
    let mut s = Settlement::generate(1, &config);
    let grain_node = s
        .grain_node()
        .expect("the endogenous chain has a grain node");

    // The seeded non-lineage consumers (indices 0..n_consumers) start idle: no node,
    // produce nothing, no household — the permanently-stranded set.
    for i in 0..n_consumers {
        assert_eq!(s.vocation_of(i), Some(Vocation::Consumer));
        assert_eq!(s.node_of(i), None);
        assert!(s.household_of(i).is_none());
    }

    // Track, per consumer: did it re-enter as a grain gatherer; how hungry was it at
    // entry; its grain econ-stock floor at entry; the most grain it later held; and
    // the lowest hunger it reached after entry.
    let mut entered = vec![false; n_consumers];
    let mut hunger_at_entry = vec![0u16; n_consumers];
    let mut grain_at_entry = vec![0u64; n_consumers];
    let mut max_grain = vec![0u64; n_consumers];
    let mut min_hunger_after = vec![u16::MAX; n_consumers];

    for _ in 0..500u64 {
        s.econ_tick();
        for i in 0..n_consumers {
            let on_grain =
                s.vocation_of(i) == Some(Vocation::Gatherer) && s.node_of(i) == Some(grain_node);
            let hunger = s.need_of(i).map(|n| n.hunger).unwrap_or(0);
            let held = s.stock_of(i, grain);
            if on_grain && !entered[i] {
                entered[i] = true;
                hunger_at_entry[i] = hunger;
                grain_at_entry[i] = held;
            }
            if entered[i] {
                max_grain[i] = max_grain[i].max(held);
                min_hunger_after[i] = min_hunger_after[i].min(hunger);
            }
        }
    }

    // At least one stranded consumer re-entered hungry (>= the entry threshold),
    // accumulated grain in its econ stock above what it held at entry (real food
    // acquired off the edible node), and saw its hunger fall as it ate that grain.
    let fed = (0..n_consumers).find(|&i| {
        entered[i]
            && hunger_at_entry[i] >= h_in
            && max_grain[i] > grain_at_entry[i]
            && min_hunger_after[i] < hunger_at_entry[i]
    });
    assert!(
        fed.is_some(),
        "a stranded non-lineage consumer must re-enter as a grain gatherer, accumulate \
         grain in econ stock, and see its hunger fall — entered={entered:?} \
         hungerAtEntry={hunger_at_entry:?} grainAtEntry={grain_at_entry:?} \
         maxGrain={max_grain:?} minHungerAfter={min_hunger_after:?}"
    );
}

#[test]
fn reentry_repoints_a_hungry_wood_gatherer_to_grain() {
    // S6.1(b): a hungry Gatherer mis-allocated to the non-edible WOOD node is
    // re-pointed to the edible grain node — hunger outranks wood-for-trade on its
    // value scale. (The endogenous roster alternates gatherers grain/WOOD, so the
    // odd-indexed gatherers start on WOOD and pin at the hunger ceiling.)
    let config = reentry_on();
    let mut s = Settlement::generate(1, &config);
    let grain_node = s.grain_node().expect("a grain node");

    // A gatherer that does NOT start on the grain node (i.e. on WOOD): confirm at
    // least one exists at generation, then confirm every such hungry gatherer ends
    // up re-pointed onto grain.
    let wood_gatherers: Vec<usize> = (0..s.population())
        .filter(|&i| {
            s.vocation_of(i) == Some(Vocation::Gatherer) && s.node_of(i) != Some(grain_node)
        })
        .collect();
    assert!(
        !wood_gatherers.is_empty(),
        "the endogenous roster must seed WOOD gatherers to re-point"
    );

    s.run(400);

    for &i in &wood_gatherers {
        assert_eq!(
            s.node_of(i),
            Some(grain_node),
            "a hungry WOOD gatherer (idx {i}) must be re-pointed to the edible grain node"
        );
    }
}
