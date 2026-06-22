//! S16.2 — the produced-bread provenance LEDGER (stock-origin, not role).
//!
//! The ledger classifies a bread→medium trade as PRODUCED (the seller's bread was
//! cultivated) vs MINTED/residual (a seeded buffer or a hearth mint) by following the
//! STOCK ORIGIN, not the seller's role/cultivating-state at trade time (which is unsound:
//! S15 bread is produced post-market and sold a LATER tick, and a buyer can resell). The
//! struct-level rules (produced-first FIFO, origin-preserving transfer, minted not
//! mis-attributed, conservation) are pinned by the `bread_provenance_*` unit tests in
//! `settlement.rs`; this file pins the END-TO-END attribution on the shipped scenario.

use sim::{Settlement, SettlementConfig};

#[test]
fn the_traded_bread_is_produced_not_minted() {
    // In the money-from-produced-bread scenario the mint is off and the bread buffers are
    // absent, so EVERY loaf is cultivated. The ledger therefore attributes the whole
    // bread→medium volume to PRODUCED, and the minted contribution is PROVABLY zero — the
    // stock-origin proof that closes the S12 caveat (provenance, not just volume).
    let cfg = SettlementConfig::frontier_money_from_cultivation();
    let mut s = Settlement::generate(1, &cfg);
    for _ in 0..2000u64 {
        s.econ_tick();
    }

    let volume = s.bread_for_salt_volume();
    assert!(
        volume > 0,
        "the surplus seam must put produced bread on the medium market"
    );
    let (produced, minted) = s.bread_for_salt_volume_by_provenance();
    assert_eq!(
        produced + minted,
        volume,
        "the provenance split must sum to the realized bread-for-medium volume"
    );
    assert_eq!(
        minted, 0,
        "mint off + buffers absent: the minted contribution is provably zero"
    );
    assert_eq!(
        produced, volume,
        "the whole monetizing bread volume is produced (cultivated) bread"
    );

    // The provenance counters conserve: produced credited == produced sunk + still held.
    let (credited, sunk) = s.produced_bread_credited_and_sunk();
    assert!(credited > 0, "the cultivators must have produced bread");
    assert_eq!(
        credited,
        sunk + s.produced_bread_held(),
        "the produced-bread provenance counters must conserve"
    );
}

#[test]
fn provenance_run_is_deterministic() {
    let cfg = SettlementConfig::frontier_money_from_cultivation();
    let mut a = Settlement::generate(3, &cfg);
    let mut b = Settlement::generate(3, &cfg);
    for _ in 0..1200u64 {
        a.econ_tick();
        b.econ_tick();
    }
    assert_eq!(
        a.bread_for_salt_volume_by_provenance(),
        b.bread_for_salt_volume_by_provenance(),
        "the provenance attribution must be byte-identical for the same (seed, config)"
    );
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());
}
