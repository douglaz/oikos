//! S8.0 — the emergence probe (read-only diagnostics).
//!
//! Before composing money emergence with the sustained specialized chain (S8.1–S8.3),
//! the probe surfaces the read-only diagnostics that separate a *principled* failure
//! from a *tuning* one: the promotion tick, the per-candidate barter saleability
//! (acceptances + acceptor/counterpart breadth), each chain producer's working capital
//! (the barter medium and FREE gold it holds — the Tension-B trace), the bread-for-SALT
//! leg that monetizes the medium, and the pre-promotion hunger trough (Tension A). The
//! probe is pure read-back — it steers no tick and is absent from `canonical_bytes`, so
//! it is deterministic and leaves the goldens untouched.
//!
//! It is exercised here on the existing `frontier` (G5b) emergent base, the precedent
//! barter→money composition; S8.1–S8.3 then build the co-emergent base on top.

use econ::good::SALT;
use sim::{ProducerRole, Settlement, SettlementConfig, Vocation};

fn frontier() -> SettlementConfig {
    SettlementConfig::frontier()
}

/// The probe records a real promotion tick and non-empty per-candidate acceptance
/// counts (with SALT — the medium — among the candidates and actually accruing
/// saleability), and reports the latent producer pool's working capital.
#[test]
fn probe_records_promotion_and_acceptances() {
    let config = frontier();
    let mut s = Settlement::generate(2_026, &config);

    // Walk to (just past) the promotion tick.
    let mut promotion_tick = None;
    for tick in 0..120u64 {
        let was_barter = s.current_money_good().is_none();
        s.econ_tick();
        if was_barter && s.current_money_good().is_some() {
            promotion_tick = Some(tick);
            break;
        }
    }
    assert_eq!(
        s.promoted_at_tick(),
        promotion_tick,
        "the probe must surface a real, latched promotion tick"
    );
    assert!(
        promotion_tick.is_some(),
        "the frontier must promote within the horizon"
    );

    // Per-candidate acceptances are non-empty, SALT is among them, and it actually
    // accrued saleability (acceptances + acceptor breadth) — the breadth+volume the
    // promotion rule reads.
    let acceptances = s.emergence_acceptances();
    assert!(
        !acceptances.is_empty(),
        "the probe must report candidate acceptances"
    );
    let salt = acceptances
        .iter()
        .find(|c| c.good == SALT)
        .expect("SALT must be a tracked money candidate");
    assert!(
        salt.acceptances > 0,
        "SALT must have accrued barter acceptances"
    );
    assert!(
        salt.acceptor_agents > 0 && salt.counterpart_goods > 0,
        "SALT's saleability breadth (acceptors, counterpart goods) must be non-empty"
    );

    // The producer-cash probe reports the latent producer pool with the right roles.
    let cash = s.producer_cash();
    assert!(
        cash.iter().any(|c| c.role == ProducerRole::LatentMiller),
        "the probe must surface the latent miller pool"
    );
    assert!(
        cash.iter().any(|c| c.role == ProducerRole::LatentBaker),
        "the probe must surface the latent baker pool"
    );

    // The bread-for-SALT leg and the pre-promotion hunger trough are surfaced as
    // read-only counters (the frontier is hunger-resilient, so the critical streak is 0,
    // but the peak hunger is a real bound the emergence window survived).
    let _ = s.bread_for_salt_volume();
    assert!(
        s.peak_pre_promotion_hunger() > 0,
        "the probe must record the pre-promotion hunger the colony reached"
    );
}

/// The probe is purely read-only: two runs of the same `(seed, config)` produce
/// identical probe readings at every tick, and the probe never moves the determinism
/// digest (it is absent from `canonical_bytes`).
#[test]
fn probe_is_read_only_and_deterministic() {
    let config = frontier();
    let mut a = Settlement::generate(7, &config);
    let mut b = Settlement::generate(7, &config);
    for tick in 0..120u64 {
        a.econ_tick();
        b.econ_tick();
        assert_eq!(
            a.emergence_acceptances(),
            b.emergence_acceptances(),
            "acceptances diverged at tick {tick}"
        );
        assert_eq!(
            a.producer_cash(),
            b.producer_cash(),
            "producer cash diverged at tick {tick}"
        );
        assert_eq!(a.bread_for_salt_volume(), b.bread_for_salt_volume());
        assert_eq!(a.peak_pre_promotion_hunger(), b.peak_pre_promotion_hunger());
        assert_eq!(
            a.critical_ticks_before_promotion(),
            b.critical_ticks_before_promotion()
        );
        // The probe is not behaviour state — the canonical digest stays in lockstep.
        assert_eq!(
            a.canonical_bytes(),
            b.canonical_bytes(),
            "digest drifted at tick {tick}"
        );
    }
}

/// A non-emergent (designated-money) settlement has no saleability tracker, so the
/// acceptance probe is empty — the probe is gated on the emergent regime and never
/// fabricates data for the designated path.
#[test]
fn probe_is_empty_for_a_designated_money_settlement() {
    let mut s = Settlement::generate(7, &SettlementConfig::frontier_endogenous());
    assert!(
        !s.is_emergent(),
        "frontier_endogenous is a designated-GOLD settlement"
    );
    s.run(50);
    assert!(
        s.emergence_acceptances().is_empty(),
        "a designated-money settlement has no barter saleability to report"
    );
    assert_eq!(
        s.bread_for_salt_volume(),
        0,
        "with no barter medium there is no bread-for-SALT leg"
    );
    // The producer-cash probe still enumerates the chain producers (a designated economy
    // has active producers), but they hold no barter medium.
    let cash = s.producer_cash();
    assert!(
        cash.iter().all(|c| c.medium == 0),
        "no barter medium without an overlay"
    );
    let _ = Vocation::Miller;
}
