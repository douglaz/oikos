//! S21d — the OPEN-SURVIVAL money probe (impl-27).
//!
//! Tests whether endogenous medium money survives MARKET-FINANCED survival in an open colony with
//! mortality OFF: the food hearths are retired (`retire_food_mints`), so every agent must buy or
//! produce its food, while the full money machinery (S20 two-lane + S21a marketability + S21b
//! two-layer + S21c open-discovery) is composed on the strong co-emergent base.
//!
//! Per the spec this is a PROBE: the deliverable is an honest colony + instrumentation, run, and
//! the outcome CLASSIFIED with traces — either (a) SALT promotes AND the chain bootstraps under
//! market-financed survival (the capstone result), or (b) a clean deadlock with the gate
//! localized (a first-class finding).
//!
//! THE OBSERVED RESULT IS A PHASE-A FINDING (see `open_survival_classified`): retiring the food
//! scaffold collapses the pre-promotion barter market to ZERO trades. Pre-promotion bread supply
//! came entirely from the mint-fed surplus; with the mint gone every agent holds its seed bread to
//! eat (it is their only food, mortality off), so the universal bread want has no market supply,
//! the barter book never clears, and SALT never accrues the saleability it needs to promote. The
//! `mints_on_control_restores_the_market` control proves the cause is the scaffold removal, not the
//! instrumentation — restoring the mint restores a functioning bread market. The deadlock is
//! therefore localized UPSTREAM of promotion: production is post-promotion only, so there is no
//! endogenous pre-promotion food supply to replace the retired scaffold. This is the publishable
//! result; it is NOT tuned into a money-emergence (that would be value-scale surgery the spec
//! forbids).

use econ::good::{GoodId, SALT, WOOD};
use sim::{Settlement, SettlementConfig};

/// A horizon long enough to exhaust the disclosed cold-start seeds and let the strong-bar breadth
/// gate adjudicate (the strong-bar suite uses 1600).
const PROBE_TICKS: u64 = 1_600;

/// Cumulative whole-run flow totals the per-tick `EconTickReport` exposes (its `*_of` accessors
/// are per-tick, so the run sums them).
#[derive(Clone, Copy, Debug, Default)]
struct RunSums {
    food_mint_endowment: u64,
    wood_endowment: u64,
    bread_produced: u64,
}

fn run(cfg: &SettlementConfig) -> (Settlement, RunSums) {
    run_seed(7, cfg)
}

fn run_seed(seed: u64, cfg: &SettlementConfig) -> (Settlement, RunSums) {
    let mut s = Settlement::generate(seed, cfg);
    let bread = s.content().expect("the probe carries a chain").bread();
    let mut sums = RunSums::default();
    for tick in 0..PROBE_TICKS {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "whole-system conservation broke at tick {tick}"
        );
        sums.food_mint_endowment += report.endowment_of(bread);
        sums.wood_endowment += report.endowment_of(WOOD);
        sums.bread_produced += report.produced_of(bread);
    }
    (s, sums)
}

fn bread_good(s: &Settlement) -> GoodId {
    s.content().expect("the probe carries a chain").bread()
}

// ----------------------------------------------------------------------------------------------
// S21d.0 — the retire_food_mints flag (engine, gated; goldens byte-identical)
// ----------------------------------------------------------------------------------------------

#[test]
fn canonical_bytes_include_retire_food_mints() {
    // The flag is canonicalized ON-only (it changes the recurring staple-mint behaviour), so the
    // default-off and an explicit-off must digest identically, and ON must split the digest —
    // mirroring the S20/S21 menger gates.
    let off = Settlement::generate(7, &SettlementConfig::frontier_coemergent_strong());

    let mut explicit_off_cfg = SettlementConfig::frontier_coemergent_strong();
    explicit_off_cfg
        .chain
        .as_mut()
        .expect("chain")
        .retire_food_mints = false;
    let explicit_off = Settlement::generate(7, &explicit_off_cfg);

    let mut on_cfg = SettlementConfig::frontier_coemergent_strong();
    on_cfg.chain.as_mut().expect("chain").retire_food_mints = true;
    let on = Settlement::generate(7, &on_cfg);

    assert_eq!(
        off.canonical_bytes(),
        explicit_off.canonical_bytes(),
        "a flag-off config must keep its exact prior byte layout (off == explicit-off)"
    );
    assert_ne!(
        off.canonical_bytes(),
        on.canonical_bytes(),
        "the retire-food-mints flag-on config must have a distinct canonical identity (off != on)"
    );
}

#[test]
fn retire_food_mints_is_digest_inert_when_own_labor_already_retires_the_mints() {
    // Reviewer P3: when own-labor subsistence is already retiring the two food-mint sites (both
    // guarded by `!own_labor_subsistence_can_run() && !retire_food_mints()`), turning on
    // `retire_food_mints` changes NOTHING — the mints are off either way — so it must NOT split
    // the canonical digest (no false split for behaviour-identical configs). The provisioned
    // co-emergent base runs own-labor subsistence, so the flag is inert there.
    let off = Settlement::generate(
        7,
        &SettlementConfig::frontier_coemergent_strong_provisioned(),
    );
    let mut on_cfg = SettlementConfig::frontier_coemergent_strong_provisioned();
    on_cfg.chain.as_mut().expect("chain").retire_food_mints = true;
    let on = Settlement::generate(7, &on_cfg);
    assert_eq!(
        off.canonical_bytes(),
        on.canonical_bytes(),
        "retire_food_mints must be digest-inert when own-labor already retires the mints"
    );
}

#[test]
fn retire_food_marker_does_not_collide_with_the_multigood_marker() {
    // Reviewer P2: the retire-food marker must not be ambiguous with the bare `1` the adjacent
    // `multigood_money_active()` block emits. The concrete collision example was
    // `frontier_multigood()` with `gatherers = 0` (so the WOOD routing has no roster effect):
    // before the fix, (multigood=true, retire_food=false) and (multigood=false, retire_food=true)
    // both serialized a single `1` at the same position and digested IDENTICALLY despite skipping
    // the mints on subsequent ticks differently. The fix (behaviour-gate + distinct tag) makes
    // the two configs distinct.
    let multigood_only = {
        let mut c = SettlementConfig::frontier_multigood();
        c.gatherers = 0;
        let chain = c.chain.as_mut().expect("chain");
        chain.multigood_money = true;
        chain.retire_food_mints = false;
        c
    };
    let retire_only = {
        let mut c = SettlementConfig::frontier_multigood();
        c.gatherers = 0;
        let chain = c.chain.as_mut().expect("chain");
        chain.multigood_money = false;
        chain.retire_food_mints = true;
        c
    };
    assert_ne!(
        Settlement::generate(7, &multigood_only).canonical_bytes(),
        Settlement::generate(7, &retire_only).canonical_bytes(),
        "the retire-food and multigood gated markers must not collide in the canonical digest"
    );
}

#[test]
fn acquisition_ledger_is_not_in_canonical_bytes() {
    // The acquisition ledger is a runtime-only diagnostic (like `starvation_deaths_total`): a
    // config differing only in the flag must digest IDENTICALLY (no golden churn).
    let off = Settlement::generate(7, &SettlementConfig::frontier_coemergent_strong());
    let mut on_cfg = SettlementConfig::frontier_coemergent_strong();
    on_cfg.chain.as_mut().expect("chain").acquisition_ledger = true;
    let on = Settlement::generate(7, &on_cfg);
    assert_eq!(
        off.canonical_bytes(),
        on.canonical_bytes(),
        "the acquisition-ledger flag must not enter the digest"
    );
}

#[test]
fn retire_food_mints_zeroes_the_food_endowment_and_interns_no_forage() {
    // With the flag on, the food-mint endowment term must be zero over the whole run (no
    // demographic `food_provision`, no producer staple floor), and NO FORAGE subsistence good may
    // be interned (the explicit flag replaces the forage hack). WOOD provision is unaffected.
    let (s, sums) = run(&SettlementConfig::frontier_open_survival());

    assert_eq!(
        sums.food_mint_endowment, 0,
        "retire_food_mints must zero the recurring food-mint endowment term"
    );
    assert!(
        s.subsistence_good().is_none(),
        "the probe must intern no FORAGE/subsistence good (it is not the forage hack)"
    );
    assert!(
        sums.wood_endowment > 0,
        "WOOD/warmth provision is unaffected by retiring the FOOD mints"
    );
}

// ----------------------------------------------------------------------------------------------
// The localizing control: restoring the mint restores the market (the cause is scaffold removal)
// ----------------------------------------------------------------------------------------------

#[test]
fn mints_on_control_restores_the_market() {
    // The mints-ON control (`retire_food_mints = false`) is the old scaffolded path, NOT a
    // capstone success — but it proves WHY the probe collapses: with the mint feeding the colony
    // there IS a tradeable bread surplus, so the barter market clears and food is bought. The
    // open-survival probe (mint off) clears ZERO trades; this control clears many. That contrast
    // localizes the deadlock at the retired scaffold, not at the money machinery or the ledger.
    let mut cfg = SettlementConfig::frontier_open_survival();
    cfg.chain.as_mut().expect("chain").retire_food_mints = false;
    let (s, sums) = run(&cfg);

    assert!(
        sums.food_mint_endowment > 0,
        "with the mint ON the food-mint endowment term must be positive"
    );
    // Pin the exact headline-contrast figures the docs §9 table and the report cite
    // (deterministic at seed 7 over `PROBE_TICKS`), so the published "the market reappears"
    // numbers cannot silently drift from the engine. The probe (mint OFF) clears 0 of each; this
    // control clears these. (The `consumed.bought > 0` pin also proves the acquisition ledger's
    // `bought` channel is non-vacuous.)
    let consumed = s.acquisition_consumed_by_channel();
    assert_eq!(
        s.barter_trade_count(),
        431,
        "mint-ON control: barter trades cleared (docs §9 table / report headline)"
    );
    assert_eq!(
        s.trade_volume_of(bread_good(&s)),
        311,
        "mint-ON control: bread traded over the run (docs §9 table)"
    );
    assert_eq!(
        consumed.bought, 310,
        "mint-ON control: MARKET-bought food consumed (docs §9 table)"
    );
    assert_eq!(
        consumed.seeded_minted, 23_821,
        "mint-ON control: seeded/minted food consumed (docs §9 table)"
    );
}

#[test]
fn phase_a_collapse_holds_across_seeds() {
    // Robustness (Codex result-review P3: "one seed is one seed"). The Phase A collapse and its
    // mint-on restoration are not a seed-7 artifact: across several seeds, the open-survival probe
    // (mint OFF) clears ZERO barter trades and never promotes, while the identical scenario with the
    // mint restored clears a positive number of trades. (Exact magnitudes are seed-dependent and
    // pinned only at seed 7 in `mints_on_control_restores_the_market`; here the claim is the
    // qualitative 0-vs-positive contrast.)
    for seed in [1_u64, 7, 42, 0xC0FFEE] {
        let (off, _) = run_seed(seed, &SettlementConfig::frontier_open_survival());
        assert_eq!(
            off.barter_trade_count(),
            0,
            "seed {seed}: mint-OFF open survival must collapse the barter market to zero trades"
        );
        assert_eq!(
            off.current_money_good(),
            None,
            "seed {seed}: mint-OFF open survival must not promote any money good"
        );

        let mut on_cfg = SettlementConfig::frontier_open_survival();
        on_cfg.chain.as_mut().expect("chain").retire_food_mints = false;
        let (on, _) = run_seed(seed, &on_cfg);
        assert!(
            on.barter_trade_count() > 0,
            "seed {seed}: restoring the mint must restore a clearing barter market"
        );
    }
}

#[test]
fn money_machinery_controls_do_not_rescue_the_phase_a_collapse() {
    // The deadlock is UPSTREAM of the money machinery: with no pre-promotion food supply the
    // barter book cannot clear, so toggling the S20/S21 levers off does not change the zero-trade
    // outcome. (The faithful control matrix: each lever fails the same way because the gate is the
    // missing food scaffold, not the medium-discovery institution.)
    let two_layer_off = {
        let mut c = SettlementConfig::frontier_open_survival();
        c.barter.as_mut().unwrap().menger.two_layer_saleability = false;
        c
    };
    let marketability_off = {
        let mut c = SettlementConfig::frontier_open_survival();
        c.barter
            .as_mut()
            .unwrap()
            .menger
            .durability_aware_acceptance = false;
        c
    };
    let multi_offer_off = {
        let mut c = SettlementConfig::frontier_open_survival();
        c.barter.as_mut().unwrap().menger.multi_offer_medium = false;
        c
    };
    for (label, cfg) in [
        ("two_layer off", two_layer_off),
        ("marketability off", marketability_off),
        ("multi_offer off", multi_offer_off),
    ] {
        let (s, _) = run(&cfg);
        assert_eq!(
            s.current_money_good(),
            None,
            "{label}: no money can emerge while the pre-promotion food market is collapsed"
        );
        assert_eq!(
            s.barter_trade_count(),
            0,
            "{label}: the collapse is upstream of the money machinery — still zero trades"
        );
    }
}

// ----------------------------------------------------------------------------------------------
// S21d.1 — the acquisition-channel ledger
// ----------------------------------------------------------------------------------------------

#[test]
fn acquisition_ledger_conserves_and_is_consistent() {
    // The per-tick conservation invariant (`total_held == held tracked food`) is asserted inside
    // `finalize_acquisition_ledger`; here we cross-check the cumulative accounting is internally
    // consistent: per channel, credited >= held + consumed (the residual is non-consume outflow).
    let (s, _) = run(&SettlementConfig::frontier_open_survival());
    let credited = s.acquisition_credited_by_channel();
    let held = s.acquisition_held_by_channel();
    let consumed = s.acquisition_consumed_by_channel();
    for (name, c, h, e) in [
        ("bought", credited.bought, held.bought, consumed.bought),
        (
            "seeded_minted",
            credited.seeded_minted,
            held.seeded_minted,
            consumed.seeded_minted,
        ),
        (
            "self_produced",
            credited.self_produced,
            held.self_produced,
            consumed.self_produced,
        ),
        ("foraged", credited.foraged, held.foraged, consumed.foraged),
    ] {
        assert!(
            c >= h + e,
            "channel {name}: credited {c} < held {h} + consumed {e} (ledger inconsistent)"
        );
    }
    // No tracked food is ever foraged in this probe (FORAGE is a distinct, untracked good).
    assert_eq!(
        consumed.foraged, 0,
        "no tracked food is foraged in the probe"
    );
    assert_eq!(credited.foraged, 0, "no tracked food enters via forage");
}

#[test]
fn acquisition_ledger_mirrors_in_kind_subsistence_advances() {
    // Existing in-kind scenarios can opt into the runtime-only acquisition ledger. The
    // subsistence-advance phase moves tracked staple food donor→producer before the market; the
    // ledger must mirror that origin-preserving transfer or the per-tick conservation assert in
    // `finalize_acquisition_ledger` will trip once the producer eats/sells the advanced bread.
    let mut cfg = SettlementConfig::frontier_in_kind();
    cfg.chain.as_mut().expect("chain").acquisition_ledger = true;
    let (s, _) = run(&cfg);
    let credited = s.acquisition_credited_by_channel();
    let consumed = s.acquisition_consumed_by_channel();

    assert!(
        credited.total() > 0,
        "the acquisition ledger must be active on the in-kind scenario"
    );
    assert!(
        consumed.total() > 0,
        "the in-kind scenario must exercise tracked-food consumption"
    );
}

// ----------------------------------------------------------------------------------------------
// S21d.2a — the cross-tick bootstrap microtrace
// ----------------------------------------------------------------------------------------------

#[test]
fn bootstrap_microtrace_is_non_vacuous_on_the_working_economy() {
    // The microtrace is meaningless in the open colony (no promotion → no producer phase), so its
    // soundness is proven where the chain actually runs: the strong co-emergent base with the
    // instrumentation on. There the producers buy and eat food and their input-bid decisions are
    // recorded — the buy → eat → bid machinery the Exp-9 gate adjudicates.
    let mut cfg = SettlementConfig::frontier_coemergent_strong();
    cfg.chain.as_mut().expect("chain").acquisition_ledger = true;
    let (s, _) = run(&cfg);
    let trace = s.bootstrap_trace_summary();
    assert!(
        trace.food_buys > 0,
        "producers must be observed buying food on the working economy"
    );
    assert!(
        trace.food_eats > 0,
        "producers must be observed eating food on the working economy"
    );
    assert!(
        trace.bid_attempts > 0,
        "the input-bid decision (the Exp-9 gate point) must be exercised post-promotion"
    );
    assert!(
        trace.bids_posted > 0,
        "some prepared project-input attempts must reach the real spot order book or fill"
    );
    assert!(
        trace.bids_posted_after_recent_buy <= trace.bids_posted,
        "buy -> eat -> bid posts must be a subset of all posted input bids"
    );
    // Every attempt is accounted as posted or one of the two block reasons.
    assert_eq!(
        trace.bid_attempts,
        trace.bids_posted + trace.bids_blocked_cashless + trace.bids_blocked_reserved,
        "every bid attempt must be classified posted / cashless-block / reserved-block"
    );
}

#[test]
fn bootstrap_microtrace_is_silent_without_promotion() {
    // In the open colony nothing promotes, so the producer input-bid phase never runs and the
    // microtrace is empty — the trace correctly reports the bootstrap is MOOT (the gate is Phase
    // A, not the Phase B producer bootstrap).
    let (s, _) = run(&SettlementConfig::frontier_open_survival());
    let trace = s.bootstrap_trace_summary();
    assert_eq!(trace.bid_attempts, 0, "no input bids without promotion");
    assert_eq!(trace.first_bootstrap_bid_tick, None);
}

// ----------------------------------------------------------------------------------------------
// Determinism
// ----------------------------------------------------------------------------------------------

#[test]
fn open_survival_is_deterministic() {
    let (a, _) = run(&SettlementConfig::frontier_open_survival());
    let (b, _) = run(&SettlementConfig::frontier_open_survival());
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the same seed + config must produce a byte-identical run (no live RNG)"
    );
}

// ----------------------------------------------------------------------------------------------
// The run — classify the outcome (success OR finding) with live traces
// ----------------------------------------------------------------------------------------------

#[test]
fn open_survival_classified() {
    let (s, sums) = run(&SettlementConfig::frontier_open_survival());
    let bread = bread_good(&s);

    let promoted = s.current_money_good();
    let medium_leader = s
        .society()
        .emergence()
        .and_then(|e| e.medium_leader_shares())
        .map(|l| l.good);
    let indirect_targets = s.indirect_target_goods(SALT);
    let consumed = s.acquisition_consumed_by_channel();
    let held = s.acquisition_held_by_channel();
    let credited = s.acquisition_credited_by_channel();
    let trace = s.bootstrap_trace_summary();

    eprintln!("=== S21d OPEN-SURVIVAL PROBE — classification @ {PROBE_TICKS} ticks ===");
    eprintln!("promotion: current_money_good = {promoted:?}");
    eprintln!("medium_leader_shares().good = {medium_leader:?}");
    eprintln!(
        "indirect_target_goods(SALT) = {indirect_targets:?} (bread={bread:?}, WOOD={WOOD:?})"
    );
    eprintln!(
        "food-mint endowment(bread) over run = {}",
        sums.food_mint_endowment
    );
    eprintln!("barter trades cleared = {}", s.barter_trade_count());
    eprintln!("bread traded over run = {}", s.trade_volume_of(bread));
    eprintln!("acquisition consumed  = {consumed:?}");
    eprintln!("acquisition held      = {held:?}");
    eprintln!("acquisition credited  = {credited:?}");
    eprintln!("bootstrap trace       = {trace:?}");
    eprintln!("bread produced over run = {}", sums.bread_produced);
    eprintln!("max living hunger = {}", s.max_living_hunger());

    // Hard invariant: no recurring food mint under the probe.
    assert_eq!(
        sums.food_mint_endowment, 0,
        "no recurring food mint under the probe"
    );

    if promoted == Some(SALT) {
        // PHASE-B branch (not the observed outcome): SALT promoted — evaluate the bootstrap.
        eprintln!(
            "CLASSIFICATION: SALT promoted — evaluating Phase B (does the chain bootstrap?)."
        );
        assert_eq!(
            medium_leader,
            Some(SALT),
            "if SALT promotes it must lead on MEDIUM share (two-layer)"
        );
        let bootstrapped = trace.bids_posted_after_recent_buy > 0 && sums.bread_produced > 0;
        if bootstrapped && consumed.bought > consumed.seeded_minted {
            eprintln!("RESULT: capstone SUCCESS — money emerged and the chain bootstrapped.");
        } else {
            eprintln!(
                "RESULT: Phase B FINDING — SALT promoted but the cross-tick producer bootstrap \
                 stalled (bids_posted_after_recent_buy={}, blocked_cashless={}, \
                 blocked_reserved={}, bread_produced={}).",
                trace.bids_posted_after_recent_buy,
                trace.bids_blocked_cashless,
                trace.bids_blocked_reserved,
                sums.bread_produced,
            );
        }
    } else {
        // PHASE-A branch (the OBSERVED, publishable finding): SALT never promoted because the
        // pre-promotion barter market collapsed entirely once the food scaffold was retired.
        eprintln!(
            "CLASSIFICATION: Phase A FINDING — SALT did not promote. The pre-promotion barter \
             market collapsed: with the food scaffold retired and production post-promotion only, \
             every agent holds its seed bread to eat, the universal bread want has no market \
             supply, and the barter book clears zero trades — so SALT accrues no saleability."
        );
        // The localized facts (deterministic, qualitative):
        assert_eq!(
            s.barter_trade_count(),
            0,
            "the finding: retiring the scaffold collapses the pre-promotion barter market to zero \
             trades"
        );
        assert_eq!(
            s.trade_volume_of(bread),
            0,
            "no bread reaches the market (it is all held for own consumption)"
        );
        assert_eq!(
            consumed.bought, 0,
            "no agent eats market-bought food — survival rides the seed, not the market"
        );
        assert!(
            consumed.seeded_minted > 0,
            "agents do eat their cold-start seed bread (the colony is alive — mortality off)"
        );
        assert!(
            indirect_targets.is_empty(),
            "SALT accrues no indirect-exchange targets at all (no trades to seed saleability)"
        );
        // The bootstrap microtrace is correctly MOOT (Phase A, upstream of the producer bootstrap).
        assert_eq!(
            trace.bid_attempts, 0,
            "the producer input-bid phase never runs (no promotion) — the gate is Phase A"
        );
    }
}
