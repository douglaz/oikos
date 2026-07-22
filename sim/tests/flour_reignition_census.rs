//! One-seed census for the first post-founder-death `InputPriceAbsent` Bake appraisal.
//! Classification uses measured flour state, not the reason code; the default-off
//! capture is excluded from `canonical_bytes`.

use sim::{Settlement, SettlementConfig};

const CENSUS_SEED: u64 = 3;
const MAX_TICKS: u64 = 1_600;

fn config() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_mortal_producers_heritable();
    let households = &mut cfg
        .demography
        .as_mut()
        .expect("heritable demography")
        .households;
    let producer_start = households.len().checked_sub(6).expect("producer houses");
    for house in &mut households[producer_start..] {
        house.food_provision = 0;
    }
    let chain = cfg.chain.as_mut().expect("chain");
    chain.producer_house_cap = 2;
    chain.mortal_producer_tool_inheritance = true;
    chain.stale_input_price_fix = true;
    cfg
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Bucket {
    ZeroHolder,
    HolderWithoutAsk,
    CommonsLocked,
    MillerSide,
}

#[test]
fn first_post_death_input_absence() {
    let cfg = config();
    let oven = cfg.chain.as_ref().expect("chain").content.oven();
    let mut settlement = Settlement::generate(CENSUS_SEED, &cfg);
    let founder_bakers: Vec<_> = (0..settlement.population())
        .filter(|&index| settlement.stock_of(index, oven) > 0)
        .collect();
    assert!(
        !founder_bakers.is_empty(),
        "the base must seed founder Bakers"
    );

    let mut first_baker_death_tick = None;

    let row = loop {
        // Arm every tick so the FIRST post-death Bake decline is captured even when the
        // founder death and the heir's role choice fall in the SAME econ_tick: death is
        // processed in phase 3b and role choice in phase 4b, so a census armed only after
        // the death is observed would skip that tick's appraisal and record the next one.
        settlement.debug_arm_flour_census();
        let bakers_before: Vec<_> = founder_bakers
            .iter()
            .copied()
            .filter(|&index| settlement.is_alive(index))
            .collect();
        let report = settlement.econ_tick();
        let econ_tick = settlement.econ_tick_count();
        if first_baker_death_tick.is_none()
            && bakers_before
                .iter()
                .any(|&index| !settlement.is_alive(index))
        {
            first_baker_death_tick = Some(report.econ_tick);
        }
        let captured = settlement.debug_take_flour_census();
        if first_baker_death_tick.is_some() {
            if let Some(row) = captured {
                break row;
            }
        }
        // A pre-death bootstrap decline is discarded; the census re-arms next tick.
        assert!(
            econ_tick < MAX_TICKS,
            "no post-death InputPriceAbsent Bake decline captured within {MAX_TICKS} ticks"
        );
    };
    let first_baker_death_tick =
        first_baker_death_tick.expect("the census is armed only after a Baker actually dies");

    let holder_without_ask = row
        .colonists
        .iter()
        .any(|h| h.flour_held > 0 && h.reservation_ask.is_none());
    let no_living_holder = row.colonists.iter().all(|h| h.flour_held == 0);
    let commons_locked = no_living_holder && row.commons_flour > 0;
    let zero_holder = no_living_holder && row.commons_flour == 0 && row.millers.is_empty();
    let miller_side = no_living_holder && row.commons_flour == 0 && !row.millers.is_empty();
    let bucket = if holder_without_ask {
        Bucket::HolderWithoutAsk
    } else if commons_locked {
        Bucket::CommonsLocked
    } else if zero_holder {
        Bucket::ZeroHolder
    } else {
        Bucket::MillerSide
    };

    println!("=== C3R.i flour re-ignition census (seed {CENSUS_SEED}, mortal base) ===");
    println!(
        "decline_tick={} (society clock); first Baker death at report tick={}; \
         producer old-age deaths by decline={}",
        row.decline_tick, first_baker_death_tick, row.deaths_before_decline,
    );
    println!("{row:#?}");
    if bucket == Bucket::MillerSide {
        assert!(
            row.bootstrap_trace_active,
            "MillerSide cannot be explained from disabled bootstrap telemetry"
        );
        let reason = if row.millers.iter().all(|m| m.gold == 0) {
            "no miller holds gold to fund a grain bid (cashless)"
        } else if row.bootstrap.bids_posted == 0 && row.bootstrap.bids_blocked_reserved > 0 {
            "grain bid blocked (gold reserved for a higher-ranked want)"
        } else if row.bootstrap.bids_posted == 0 {
            "no grain bid posted"
        } else {
            "grain bid posted but no crossing seller / no Mill execution"
        };
        println!("MillerSide sub-reason (from measured miller/grain-bid state): {reason}");
    }
    println!("CLASSIFIED BUCKET = {bucket:?}");

    assert!(
        row.deaths_before_decline > 0,
        "the decline must be post a mortal chain-producer old-age death (deaths_before_decline > 0)"
    );
    // Post-death ordering is guaranteed by construction: the loop accepts a row only after a
    // founder Baker death is observed (`first_baker_death_tick` set), and `deaths_before_decline`
    // pins it from the society-side counter. No cross-counter assert here — `report.econ_tick`
    // (settlement counter) and `row.decline_tick` (`society.tick.0`) are different clocks.
    assert!(
        row.colonists.iter().all(|h| h.reservation_ask.is_none()),
        "the trigger requires every living non-self colonist's flour reservation ask to be None"
    );
    let classified = [holder_without_ask, commons_locked, zero_holder, miller_side]
        .iter()
        .filter(|&&b| b)
        .count();
    assert_eq!(
        classified, 1,
        "exactly one census bucket must classify (holder_without_ask={holder_without_ask}, \
         commons_locked={commons_locked}, zero_holder={zero_holder}, miller_side={miller_side})"
    );
    assert_eq!(
        bucket,
        Bucket::HolderWithoutAsk,
        "seed 3 mortal base pins HolderWithoutAsk: at the first post-death Bake decline, living \
         holders hold flour yet none has a computable reservation ask, so the candidate Baker's \
         appraisal declines though flour exists. This closes R2-as-specced (cap-gated dose + \
         reservation-respecting offer) as inert against THIS state; it is NOT a claim that \
         stock-adding in general is inert, nor that the state persists beyond this tick."
    );
    // The appraiser holds no flour itself — its own ask is structurally outside the census
    // (`fresh_input_ask` self-excludes), so "no computable ask" is not an artifact of that gap.
    assert_eq!(
        row.candidate_own_flour, 0,
        "the appraiser is expected to hold no flour; a self-held askable stock would mean a \
         different oven-holder could appraise against it and the decline would not follow"
    );
    // Strengthen the pin so a regression cannot pass on a weaker state. Flour is present at the
    // appraisal, not absent: a Miller (R2's dose target) holds flour far above R2's ceiling
    // (2*output_qty = 6, output_qty = 3), so R2-as-specced cannot dose it; and every holder's ask
    // is None (asserted above), so R2's reservation-respecting offer leg cannot actuate either.
    // (A Consumer heir also holds inherited flour here — flour is not scarce; only asks are.)
    assert!(
        row.millers.iter().any(|m| m.flour_held > 6),
        "a Miller (R2's dose target) must hold flour above R2's 2*output_qty ceiling for \
         R2-as-specced to be inert: {:?}",
        row.millers
    );
}

#[test]
fn canonical_bytes_excludes_flour_census() {
    let cfg = config();

    let mut settlement = Settlement::generate(CENSUS_SEED, &cfg);
    settlement.run(300);
    let before = settlement.canonical_bytes();
    settlement.debug_arm_flour_census();
    assert_eq!(
        before,
        settlement.canonical_bytes(),
        "arming the flour census must not change canonical_bytes"
    );

    let mut plain = Settlement::generate(CENSUS_SEED, &cfg);
    let mut armed = Settlement::generate(CENSUS_SEED, &cfg);
    armed.debug_arm_flour_census();
    let mut captured = false;
    for _ in 0..MAX_TICKS {
        plain.econ_tick();
        armed.econ_tick();
        assert_eq!(
            plain.canonical_bytes(),
            armed.canonical_bytes(),
            "capturing the flour census must not change canonical_bytes"
        );
        if armed.debug_take_flour_census().is_some() {
            captured = true;
            break;
        }
    }
    assert!(
        captured,
        "the armed census must capture a row so the non-steering proof is not vacuous"
    );
}
