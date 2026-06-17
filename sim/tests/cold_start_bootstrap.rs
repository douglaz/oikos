//! S4 — cold-start bootstrap.
//!
//! With no curated advance, the chain must bootstrap from SEEDED BUFFERS: the
//! latent millers' `latent_flour_seed` (the flour the first baker buys, which
//! gives flour a realized price) and the `bread_buffer` / bread-poor consumers
//! (the bread the consumers buy, which gives bread a realized price). Those first
//! realized prices are the spread the latent pool adopts on, in pipeline order:
//! bread demand pulls bakers in, the bakers' flour demand pulls millers in. This
//! is deterministic — the adoption ticks are a pure function of seed + config.

use sim::{Settlement, SettlementConfig, Vocation};

/// First tick at which (flour price, bread price, a living Miller, a living Baker)
/// each appears within `ticks`.
fn bootstrap_marks(
    config: &SettlementConfig,
    ticks: u64,
) -> (Option<u64>, Option<u64>, Option<u64>, Option<u64>) {
    let content = config.chain.as_ref().expect("chain").content.clone();
    let (flour, bread) = (content.flour(), content.bread());
    let mut s = Settlement::generate(1, config);
    let (mut f_price, mut b_price, mut miller, mut baker) = (None, None, None, None);
    for tick in 1..=ticks {
        s.econ_tick();
        if f_price.is_none() && s.realized_price(flour).is_some() {
            f_price = Some(tick);
        }
        if b_price.is_none() && s.realized_price(bread).is_some() {
            b_price = Some(tick);
        }
        if miller.is_none() && s.living_count(Vocation::Miller) > 0 {
            miller = Some(tick);
        }
        if baker.is_none() && s.living_count(Vocation::Baker) > 0 {
            baker = Some(tick);
        }
    }
    (f_price, b_price, miller, baker)
}

#[test]
fn adoption_emerges_from_seeded_buffers_in_pipeline_order() {
    let config = SettlementConfig::frontier_endogenous();
    // No curated advance is in play.
    let chain = config.chain.as_ref().unwrap();
    assert!(!chain.capital_advance && !chain.subsistence_advance && !chain.input_advance);
    assert!(
        chain.latent_flour_seed > 0,
        "millers need a flour bootstrap stock"
    );
    assert!(
        chain.bread_buffer > 0,
        "the colony needs a bread bootstrap stock"
    );

    let (f_price, b_price, miller, baker) = bootstrap_marks(&config, 40);

    // Both intermediate/final goods realize a price from the seeded buffers, and
    // both roles adopt — early, from those prices alone.
    let b_price = b_price.expect("bread should realize a price from the bread buffer");
    let f_price = f_price.expect("flour should realize a price from the flour seed");
    let baker = baker.expect("a baker should adopt from the bread spread");
    let miller = miller.expect("a miller should adopt from the flour spread");

    // Pipeline order: bread demand prices first and pulls a baker in; the baker's
    // flour purchase then prices flour and pulls a miller in.
    assert!(
        b_price <= f_price,
        "bread should realize a price no later than flour (demand pulls top-down), \
         got bread@{b_price} flour@{f_price}"
    );
    assert!(
        baker <= miller,
        "a baker should adopt no later than a miller (the baker's flour demand is \
         what prices flour for the miller), got baker@{baker} miller@{miller}"
    );
    assert!(
        miller <= 20,
        "the chain should bootstrap quickly from the seeded prices, miller@{miller}"
    );
}

#[test]
fn without_the_flour_seed_the_upstream_role_does_not_bootstrap() {
    // Falsification: remove the latent millers' flour bootstrap stock and the first
    // baker finds no flour to buy, so flour never prices and no miller adopts on a
    // flour spread that never forms — the seed is load-bearing for the cold start.
    let mut config = SettlementConfig::frontier_endogenous();
    if let Some(chain) = config.chain.as_mut() {
        chain.latent_flour_seed = 0;
    }
    let (f_price, _, miller, _) = bootstrap_marks(&config, 40);
    assert!(
        f_price.is_none() && miller.is_none(),
        "with no flour seed, flour should not price and no miller should adopt in \
         the bootstrap window, got flour@{f_price:?} miller@{miller:?}"
    );
}

#[test]
fn bootstrap_is_deterministic() {
    let config = SettlementConfig::frontier_endogenous();
    assert_eq!(
        bootstrap_marks(&config, 40),
        bootstrap_marks(&config, 40),
        "the bootstrap adoption schedule must be a pure function of seed + config"
    );
}
