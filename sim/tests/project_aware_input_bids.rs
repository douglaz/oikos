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

/// Run the colony, recording — for each NEW trade as it happens — the buyer's
/// vocation AT THAT TICK (role-choice runs at the start of the next tick, so a
/// buyer's vocation immediately after the tick is the one it traded under).
/// Returns `(grain trades bought by an active Miller, flour trades bought by an
/// active Baker, flour produced, bread produced)`.
fn run_and_classify(ticks: u64) -> (u64, u64, u64, u64) {
    let config = endogenous();
    let content = config.chain.as_ref().expect("chain").content.clone();
    let grain = content.grain();
    let flour = content.flour();
    let bread = content.bread();

    let mut settlement = Settlement::generate(1, &config);
    let (mut grain_to_miller, mut flour_to_baker) = (0u64, 0u64);
    let (mut flour_made, mut bread_made) = (0u64, 0u64);
    let mut seen = 0usize;
    for tick in 0..ticks {
        let report = settlement.econ_tick();
        assert!(
            report.conserves(),
            "the override-driven input bid must conserve (tick {tick})"
        );
        flour_made += report.produced_of(flour);
        bread_made += report.produced_of(bread);
        let trades = &settlement.society().trades;
        for trade in &trades[seen..] {
            if trade.buyer == trade.seller {
                continue;
            }
            match settlement.vocation_of_id(trade.buyer) {
                Some(Vocation::Miller) if trade.good == grain => grain_to_miller += 1,
                Some(Vocation::Baker) if trade.good == flour => flour_to_baker += 1,
                _ => {}
            }
        }
        seen = trades.len();
    }
    (grain_to_miller, flour_to_baker, flour_made, bread_made)
}

#[test]
fn producer_buys_input_through_a_real_order_book_trade_and_produces() {
    let (grain_to_miller, flour_to_baker, flour_made, bread_made) = run_and_classify(200);

    // The input is acquired by a real order-book Trade where an active producer
    // bought its recipe input from a different seller (Codex's clean metric)...
    assert!(
        grain_to_miller > 0 || flour_to_baker > 0,
        "an active producer should acquire its input through a real order-book Trade"
    );
    // ...and the acquired input is actually transformed: the chain produced output.
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
fn both_stages_buy_their_input_on_the_order_book() {
    // Both stages buy their own input through the market: at least one grain trade
    // to an active miller AND one flour trade to an active baker.
    let (grain_to_miller, flour_to_baker, _, _) = run_and_classify(200);
    assert!(
        grain_to_miller > 0,
        "a miller should buy grain through the order book, got {grain_to_miller}"
    );
    assert!(
        flour_to_baker > 0,
        "a baker should buy flour through the order book, got {flour_to_baker}"
    );
}
