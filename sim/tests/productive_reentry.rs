//! S6.1/S6.2 — the productive re-entry phase (the core, gated, default-OFF) and
//! its re-entry hysteresis.
//!
//! S6.1: a hungry, unprovisioned spatial non-lineage colonist adopts edible-grain
//! gathering on its own value scale — an idle [`Vocation::Consumer`] (no node,
//! produces nothing) and a [`Vocation::Gatherer`] mis-allocated to a non-edible
//! (WOOD) node each become a grain gatherer once hunger reaches the entry
//! threshold. These tests prove the flip is REAL provisioning (grain actually
//! accumulates in econ stock and hunger actually falls), not a relabel, and that
//! the phase is inert when off (the disabled-phase byte-identical regression).
//!
//! S6.2: the re-entry is sticky and reversible. A fed re-entrant reverts to its
//! home role only once hunger falls below the exit threshold, so it does not thrash
//! node-to-node every tick, and fed colonists resume WOOD gathering — keeping the
//! WOOD supply alive (vs. the no-revert ablation that drains every WOOD gatherer to
//! grain).

use sim::{NodeId, Settlement, SettlementConfig, Vocation, WOOD};

/// `frontier_endogenous` with the S6 re-entry phase turned ON, at the default
/// hysteresis band (entry 8, exit 4). The economy is otherwise the endogenous one,
/// so the stranded set (4 consumers + 4 WOOD gatherers, all pinned at the hunger
/// ceiling) is present to provision.
fn reentry_on() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_endogenous();
    let chain = cfg
        .chain
        .as_mut()
        .expect("the endogenous config has a chain");
    chain.productive_reentry = true;
    chain.reentry_hunger_in = 8;
    chain.reentry_hunger_out = 4;
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
    // odd-indexed gatherers start on WOOD and pin at the hunger ceiling.) The S6.2
    // hysteresis lets a fed WOOD gatherer return to WOOD, so the re-pointing is an
    // EVENT to observe over the run, not a permanent end state.
    let config = reentry_on();
    let mut s = Settlement::generate(1, &config);
    let grain_node = s.grain_node().expect("a grain node");

    // The gatherers that start on a non-grain (WOOD) node — confirm at least one
    // exists to re-point.
    let wood_gatherers: Vec<usize> = (0..s.population())
        .filter(|&i| {
            s.vocation_of(i) == Some(Vocation::Gatherer) && s.node_of(i) != Some(grain_node)
        })
        .collect();
    assert!(
        !wood_gatherers.is_empty(),
        "the endogenous roster must seed WOOD gatherers to re-point"
    );

    // Every seeded WOOD gatherer must be re-pointed onto the edible grain node at
    // some point as its hunger climbs past the entry threshold.
    let mut reached_grain = vec![false; wood_gatherers.len()];
    for _ in 0..400u64 {
        s.econ_tick();
        for (k, &i) in wood_gatherers.iter().enumerate() {
            if s.vocation_of(i) == Some(Vocation::Gatherer) && s.node_of(i) == Some(grain_node) {
                reached_grain[k] = true;
            }
        }
    }
    for (k, &i) in wood_gatherers.iter().enumerate() {
        assert!(
            reached_grain[k],
            "a hungry WOOD gatherer (idx {i}) must be re-pointed to the edible grain node \
             at least once"
        );
    }
}

#[test]
fn reentry_is_sticky_and_does_not_thrash() {
    // S6.2: the H_in/H_out band makes re-entry sticky — a colonist holds its node
    // for many ticks rather than flipping grain<->home every tick. Over a 400-tick
    // tail, count node changes per non-lineage colonist; even the worst case is well
    // below the per-tick ceiling a thrashing flip would hit (~one flip per tick).
    let mut s = Settlement::generate(1, &reentry_on());
    s.run(600);

    let mut prev: Vec<Option<NodeId>> = (0..s.population()).map(|i| s.node_of(i)).collect();
    let mut flips = vec![0u32; s.population()];
    let window = 400u64;
    for _ in 0..window {
        s.econ_tick();
        for i in 0..s.population() {
            // The lineage members are hearth-fed and never re-entered; the re-entry
            // hysteresis governs the non-lineage roster.
            if s.household_of(i).is_some() {
                continue;
            }
            let node = s.node_of(i);
            if node != prev[i] {
                flips[i] += 1;
                prev[i] = node;
            }
        }
    }
    let max_flips = u64::from(flips.iter().copied().max().unwrap_or(0));
    assert!(
        max_flips * 4 < window,
        "re-entry must be sticky (no per-tick thrash): a colonist changed node {max_flips} \
         times over {window} tail ticks (flips={flips:?})"
    );
}

#[test]
fn reentry_hysteresis_keeps_wood_supply_alive() {
    // S6.2: the hysteresis (a fed re-entrant resumes its home WOOD gathering) keeps
    // the WOOD supply alive — vs. the no-revert ablation (exit threshold 0, so a
    // re-entrant never reverts), which drains every hungry WOOD gatherer onto grain
    // permanently and stalls WOOD gathering. Measure the live WOOD-gatherer count and
    // the whole-system WOOD stock at the tail under each.
    let tail = |h_out: u16| -> (usize, u64) {
        let mut cfg = reentry_on();
        cfg.chain.as_mut().unwrap().reentry_hunger_out = h_out;
        let mut s = Settlement::generate(1, &cfg);
        let grain_node = s.grain_node();
        s.run(1200);
        let wood_gatherers = (0..s.population())
            .filter(|&i| {
                s.is_alive(i)
                    && s.vocation_of(i) == Some(Vocation::Gatherer)
                    && s.node_of(i) != grain_node
            })
            .count();
        (wood_gatherers, s.whole_system_total(WOOD))
    };

    let (wg_band, wood_band) = tail(4); // the hysteresis band
    let (wg_none, wood_none) = tail(0); // the no-revert ablation

    assert!(
        wg_band > 0,
        "the hysteresis must keep WOOD gatherers working, got {wg_band}"
    );
    assert_eq!(
        wg_none, 0,
        "the no-revert ablation must drain every WOOD gatherer onto grain, got {wg_none}"
    );
    assert!(
        wood_band > wood_none,
        "the hysteresis must sustain a larger WOOD supply ({wood_band}) than the no-revert \
         ablation ({wood_none})"
    );
}
