//! S2 — project-aware producer input bids (the endogenous fix).
//!
//! An active producer's recipe input is no longer placed by a planner; it is
//! BOUGHT on the real econ order book via a gated spot-bid override (S1), priced
//! at the reservation IMPUTED from the project-bundle appraisal. These tests prove
//! the input is acquired by a genuine `Society::trade` (not a bilateral transfer
//! and not the curated `input_advance`), that the acquired input is then consumed
//! as a recipe input (production happens), and that the new flow conserves.

use sim::{Settlement, SettlementConfig, Vocation};

/// The endogenous config drives input acquisition through the S1 override.
fn endogenous() -> SettlementConfig {
    SettlementConfig::frontier_endogenous()
}

#[test]
fn producer_buys_input_through_a_real_order_book_trade() {
    let config = endogenous();
    let content = config.chain.as_ref().expect("chain").content.clone();
    let grain = content.grain();
    let flour = content.flour();
    let bread = content.bread();

    let mut settlement = Settlement::generate(1, &config);
    let mut flour_made = 0u64;
    let mut bread_made = 0u64;
    for tick in 0..200u64 {
        let report = settlement.econ_tick();
        assert!(
            report.conserves(),
            "the override-driven input bid must conserve (tick {tick})"
        );
        flour_made += report.produced_of(flour);
        bread_made += report.produced_of(bread);
    }

    // There must exist a real order-book Trade where an active Miller/Baker bought
    // its recipe input (grain or flour) from a *different* seller — the input
    // acquired by market trade, not handed over. (Codex's clean metric.)
    let input_trade = settlement.society().trades.iter().find(|trade| {
        (trade.good == grain || trade.good == flour)
            && trade.buyer != trade.seller
            && matches!(
                settlement.vocation_of_id(trade.buyer),
                Some(Vocation::Miller) | Some(Vocation::Baker)
            )
    });
    assert!(
        input_trade.is_some(),
        "an active producer should acquire its input through a real order-book Trade"
    );

    // And the acquired input is actually transformed: the chain produced output.
    assert!(
        flour_made > 0,
        "a miller that bought grain should have milled flour, got {flour_made}"
    );
    assert!(
        bread_made > 0,
        "a baker that bought flour should have baked bread, got {bread_made}"
    );
}

#[test]
fn miller_buys_grain_and_baker_buys_flour() {
    // Both stages buy their own input through the market: at least one grain trade
    // to a miller AND one flour trade to a baker.
    let config = endogenous();
    let content = config.chain.as_ref().expect("chain").content.clone();
    let grain = content.grain();
    let flour = content.flour();

    let mut settlement = Settlement::generate(1, &config);
    for _ in 0..200u64 {
        settlement.econ_tick();
    }

    let mut miller_bought_grain = false;
    let mut baker_bought_flour = false;
    for trade in &settlement.society().trades {
        if trade.buyer == trade.seller {
            continue;
        }
        match settlement.vocation_of_id(trade.buyer) {
            Some(Vocation::Miller) if trade.good == grain => miller_bought_grain = true,
            Some(Vocation::Baker) if trade.good == flour => baker_bought_flour = true,
            _ => {}
        }
    }
    assert!(
        miller_bought_grain,
        "a miller should buy grain through the order book"
    );
    assert!(
        baker_bought_flour,
        "a baker should buy flour through the order book"
    );
}
