//! S14.2 — the forage child endowment (a SELECTOR), a growth-capable demography,
//! and birth-block diagnostics.
//!
//! The birth-food selector routes the parent-endowment gate, the parent debit, the
//! newborn's initial buffer, AND the founder seed to the FORAGE subsistence good when
//! the forage-commons path is active — so a fed-by-forage lineage can endow children
//! from forage and reproduce, instead of stalling on a bread shortage (the food mint
//! is retired). `known.hunger` (the bread staple) is left untouched, so consumption /
//! the chain / sales still key on it. The demography is retuned so the population
//! GROWS while fed (the old `max_household_size` 5 no longer binds), and birth-block
//! counters record WHY a birth was skipped so the plateau is interpretable.

use econ::good::GoodId;
use sim::{ForageCommons, Settlement, SettlementConfig};

fn bread_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain.as_ref().expect("chain").content.bread()
}

fn forage_good(cfg: &SettlementConfig) -> GoodId {
    cfg.chain
        .as_ref()
        .expect("chain")
        .content
        .forage()
        .expect("own-labor subsistence interns a forage good")
}

fn living(s: &Settlement) -> usize {
    (0..s.population()).filter(|&i| s.is_alive(i)).count()
}

#[test]
fn population_grows_past_the_old_size_cap_toward_the_raised_cap() {
    // The growth-capable demography: with abundant forage (so scarcity does not bind),
    // the lineages grow well past the OLD `max_household_size` of 5 toward the RAISED
    // cap (3 households × 24 = 72). The old regime could never exceed the size cap; the
    // S14.2 demography can.
    let mut cfg = SettlementConfig::frontier_forage_capacity();
    cfg.chain.as_mut().unwrap().forage_commons = Some(ForageCommons {
        stock: 800,
        regen: 60,
        cap: 1600,
    });
    let mut s = Settlement::generate(1, &cfg);
    let start = living(&s);
    assert!(start <= 6, "the colony starts tiny (the founders)");
    for _ in 0..2000u64 {
        s.econ_tick();
    }
    let grown = living(&s);
    assert!(
        grown > 5,
        "population must grow past the old max_household_size of 5 (got {grown})"
    );
    assert!(
        grown >= 60,
        "with abundant forage the colony grows toward the raised cap 72 (got {grown})"
    );
    assert!(s.births_total() > 0, "the colony must reproduce");
}

#[test]
fn founders_and_children_are_endowed_from_forage_not_bread() {
    // The selector seeds founders AND newborns from FORAGE (the colony's actual food),
    // leaving the bread staple untouched: bread is never minted (endowment) nor eaten
    // (consumed) on this path, while FORAGE is foraged and eaten — so births stall on
    // forage scarcity, not a bread shortage, and the chain/consumption stay keyed on
    // the (unchanged) staple.
    let cfg = SettlementConfig::frontier_forage_capacity();
    let bread = bread_good(&cfg);
    let forage = forage_good(&cfg);
    let mut s = Settlement::generate(1, &cfg);

    // Founders are seeded with FORAGE, not bread.
    for i in 0..s.population() {
        if s.household_of(i).is_none() {
            continue;
        }
        let id = s.colonist_id(i).expect("founder id");
        let agent = s.society().agents.get(id).expect("founder agent");
        assert_eq!(agent.stock.get(bread), 0, "founders hold no bread");
        assert!(
            agent.stock.get(forage) > 0,
            "founders are seeded with FORAGE"
        );
    }

    let births_before = s.births_total();
    let mut consumed_forage = 0u64;
    for tick in 0..400u64 {
        let r = s.econ_tick();
        assert_eq!(
            r.endowment_of(bread),
            0,
            "the bread staple must never be minted on the forage path (tick {tick})"
        );
        assert_eq!(
            r.consumed_of(bread),
            0,
            "no bread is eaten — the colony eats FORAGE (tick {tick})"
        );
        consumed_forage += r.consumed_of(forage);
    }
    assert!(
        consumed_forage > 0,
        "the colony must actually eat FORAGE (the staple is left untouched)"
    );
    assert!(
        s.births_total() > births_before,
        "the colony reproduces on the FORAGE endowment (not stalled on bread)"
    );
}

#[test]
fn birth_block_diagnostics_attribute_the_stall() {
    // The counters record WHY births were skipped. On the (forage-bound) main config the
    // dominant stall is the HUNGER ceiling (the preventive check), with the endowment
    // and size-cap stalls negligible — proving the bound is forage scarcity, not a bread
    // shortage or the artificial knob. With a LOW size cap the stall flips to the size
    // cap (the knob), validating the attribution.
    let main = SettlementConfig::frontier_forage_capacity();
    let mut s = Settlement::generate(1, &main);
    for _ in 0..2000u64 {
        s.econ_tick();
    }
    let hunger = s.birth_block_hunger_ceiling();
    let size_cap = s.birth_block_size_cap();
    let endowment = s.birth_block_endowment();
    assert!(
        hunger > 0,
        "forage scarcity must stall some births via the hunger ceiling"
    );
    assert!(
        hunger > 10 * (size_cap + endowment + 1),
        "the hunger ceiling must DOMINATE the stalls (hunger {hunger}, sizecap {size_cap}, endow {endowment})"
    );

    let mut low = SettlementConfig::frontier_forage_capacity();
    low.demography.as_mut().unwrap().max_household_size = 5;
    let mut s = Settlement::generate(1, &low);
    for _ in 0..2000u64 {
        s.econ_tick();
    }
    assert!(
        s.birth_block_size_cap() > s.birth_block_hunger_ceiling(),
        "with a low size cap the stall flips to the size cap (the knob), not hunger"
    );
}

#[test]
fn selector_reverts_to_bread_when_the_commons_is_off() {
    // The selector keys on the forage-commons path: with the commons OFF (but the
    // demography otherwise identical) the endowment good reverts to the bread staple, so
    // founders seed bread again — the gating is real and in one place. (Off this path
    // every existing demography golden is byte-identical, asserted elsewhere.)
    let mut off = SettlementConfig::frontier_forage_capacity();
    off.chain.as_mut().unwrap().forage_commons = None;
    let bread = bread_good(&off);
    let forage = forage_good(&off);
    let s = Settlement::generate(1, &off);
    for i in 0..s.population() {
        if s.household_of(i).is_none() {
            continue;
        }
        let id = s.colonist_id(i).expect("founder id");
        let agent = s.society().agents.get(id).expect("founder agent");
        assert!(
            agent.stock.get(bread) > 0,
            "with the commons off, founders seed the bread staple again"
        );
        assert_eq!(
            agent.stock.get(forage),
            0,
            "with the commons off, the FORAGE selector is inactive"
        );
    }
}
