//! C3R.j Cut 1 (impl-75): diagnostic-only decomposition of absent flour asks at
//! the first post-Baker-death Bake `InputPriceAbsent` decline.

use std::collections::BTreeMap;

use econ::agent::AskOutcome;
use sim::settlement::FlourCensusRow;
use sim::{Settlement, SettlementConfig};

const SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
const MAX_TICKS: u64 = 1_600;
const WINDOW: u64 = 200;

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

fn bucket_of(row: &FlourCensusRow) -> Bucket {
    let holder_without_ask = row
        .colonists
        .iter()
        .any(|h| h.flour_held > 0 && h.reservation_ask.is_none());
    let no_living_holder = row.colonists.iter().all(|h| h.flour_held == 0);
    if holder_without_ask {
        Bucket::HolderWithoutAsk
    } else if no_living_holder && row.commons_flour > 0 {
        Bucket::CommonsLocked
    } else if no_living_holder && row.millers.is_empty() {
        Bucket::ZeroHolder
    } else {
        Bucket::MillerSide
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Reason {
    HasAsk,
    NoMoneyWantInRange,
    MoneySatiated,
    ProvisioningBreak,
    MoneyGoodOrNonHolder,
    DefensiveExit,
}

fn reason_of(outcome: AskOutcome) -> Reason {
    match outcome {
        AskOutcome::Price(_) => Reason::HasAsk,
        AskOutcome::MoneyGoodOrNonHolder => Reason::MoneyGoodOrNonHolder,
        AskOutcome::ProvisioningBreak => Reason::ProvisioningBreak,
        AskOutcome::NoMoneyGain {
            in_range_money_wants: 0,
            ..
        } => Reason::NoMoneyWantInRange,
        AskOutcome::NoMoneyGain { .. } => Reason::MoneySatiated,
        AskOutcome::DefensiveExit => Reason::DefensiveExit,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Classification {
    OtherWall(Bucket),
    NotPostDeathHeir,
    TransientOnly,
    Dominant(Reason),
    MixedBranch,
}

fn check_row(row: &FlourCensusRow) {
    for holder in &row.colonists {
        let projected = match holder.ask_outcome {
            AskOutcome::Price(price) => Some(price),
            _ => None,
        };
        assert_eq!(holder.reservation_ask, projected, "holder {:?}", holder.id);
        if let AskOutcome::NoMoneyGain {
            lost_rank,
            scale_len,
            in_range_money_wants,
            provided_wants,
            gold,
            cumulative_required,
        } = holder.ask_outcome
        {
            assert_eq!(
                provided_wants == in_range_money_wants,
                cumulative_required <= gold.0,
                "NoMoneyGain two-way split diverged for holder {:?}",
                holder.id
            );
            // The scan is bounded by the allocation the sale drops (ranks at or above
            // `lost_rank`), so the in-range count can never exceed the ranks it visited.
            let scanned = (lost_rank + 1).min(scale_len);
            assert!(
                in_range_money_wants <= scanned,
                "holder {:?}: {in_range_money_wants} money wants over {scanned} scanned ranks \
                 (lost_rank={lost_rank}, scale_len={scale_len})",
                holder.id
            );
        }
    }
}

/// The measured axes a classification must follow from.
#[derive(Clone, Copy, Debug)]
struct Axes {
    bucket: Bucket,
    /// Identity: recorded oven inheritance, read UNSHADOWED by `ToolProvenance`
    /// precedence (a seeded latent Baker that also inherited would report `SeededLatent`).
    inherited: bool,
    /// Persistence: the Bake `accepts` bucket incremented inside the window.
    resolves: bool,
    /// The modal holder reason; `None` on a tie.
    dominant: Option<Reason>,
}

/// Precedence: identity → persistence → dominant reason, gated on the C3R.i bucket.
fn classify(axes: &Axes) -> Classification {
    if axes.bucket != Bucket::HolderWithoutAsk {
        Classification::OtherWall(axes.bucket)
    } else if !axes.inherited {
        Classification::NotPostDeathHeir
    } else if axes.resolves {
        Classification::TransientOnly
    } else {
        axes.dominant
            .map_or(Classification::MixedBranch, Classification::Dominant)
    }
}

/// Exactly one outcome survives precedence. Checked as the INVERSE of [`classify`] —
/// each arm re-asserts the axis state that licenses it — so a classification that stops
/// honoring identity → persistence → reason fails here. (Counting `matches!` arms over
/// the variants of one enum value would instead be a type-level tautology.)
fn assert_determinate(seed: u64, class: Classification, axes: &Axes) {
    let on_wall = axes.bucket == Bucket::HolderWithoutAsk;
    let licensed = match class {
        Classification::OtherWall(bucket) => bucket == axes.bucket && !on_wall,
        Classification::NotPostDeathHeir => on_wall && !axes.inherited,
        Classification::TransientOnly => on_wall && axes.inherited && axes.resolves,
        Classification::Dominant(reason) => {
            on_wall && axes.inherited && !axes.resolves && axes.dominant == Some(reason)
        }
        Classification::MixedBranch => {
            on_wall && axes.inherited && !axes.resolves && axes.dominant.is_none()
        }
    };
    assert!(
        licensed,
        "seed {seed}: {class:?} is not the precedence outcome of {axes:?}"
    );
}

/// Per-tick texture of the persistence window, from the unconditional row accessor:
/// does the wall recur every tick, or does it oscillate? Read beside `resolves` (which
/// is the Bake `accepts` histogram alone) so recurrence is distinguishable from a state
/// that flickers without the chain ever re-igniting. The three counters partition the
/// window, so `wall_ticks` reads directly as "ticks the wall was still up".
#[derive(Clone, Copy, Debug, Default)]
struct WindowTexture {
    /// Ticks whose sampled row is still `HolderWithoutAsk`.
    wall_ticks: u32,
    /// Ticks where flour is held and every holder of it has an ask.
    held_with_ask_ticks: u32,
    /// Ticks where no living non-self colonist holds flour.
    no_holder_ticks: u32,
}

struct SeedOutcome {
    class: Classification,
    bucket: Bucket,
    dominant: Option<Reason>,
    resolves: bool,
    row: FlourCensusRow,
}

fn decompose(seed: u64) -> SeedOutcome {
    let cfg = config();
    let chain = cfg.chain.as_ref().expect("chain");
    assert!(
        chain.stale_input_price_fix,
        "seed {seed}: InputPriceAbsent is unreachable without stale_input_price_fix"
    );
    let oven = chain.content.oven();
    let mut settlement = Settlement::generate(seed, &cfg);
    let founder_bakers: Vec<_> = (0..settlement.population())
        .filter(|&index| settlement.stock_of(index, oven) > 0)
        .collect();
    assert!(!founder_bakers.is_empty(), "seed {seed}: no founder Baker");

    let mut first_baker_death_tick = None;
    let row = loop {
        settlement.debug_arm_flour_census();
        let bakers_before: Vec<_> = founder_bakers
            .iter()
            .copied()
            .filter(|&index| settlement.is_alive(index))
            .collect();
        let report = settlement.econ_tick();
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
        assert!(
            settlement.econ_tick_count() < MAX_TICKS,
            "seed {seed}: no post-Baker-death decline within {MAX_TICKS} ticks"
        );
    };
    let baker_death_tick = first_baker_death_tick.expect("capture follows a Baker death");
    check_row(&row);
    let bucket = bucket_of(&row);
    // The identity axis reads recorded inheritance directly. `oven_provenance` reports
    // `Inherited` only when the candidate is already in the inheritor set, so the
    // `ToolProvenance` value carries no identity signal `candidate_recorded_inheritor`
    // lacks — it is the reported provenance breakdown, not an axis input.
    let inherited = row.candidate_recorded_inheritor;
    if bucket != Bucket::HolderWithoutAsk {
        // Persistence and reason are NOT measured off the wall — the axes carry their
        // unmeasured defaults, and precedence gates on the bucket first.
        let axes = Axes {
            bucket,
            inherited,
            resolves: false,
            dominant: None,
        };
        let class = classify(&axes);
        println!(
            "seed {seed}: CLASS={class:?} wall={bucket:?} decline_tick={} \
             baker_death_tick={baker_death_tick} deaths={} candidate={:?} holders={} commons={} \
             millers={} | decomposition=skipped",
            row.decline_tick,
            row.deaths_before_decline,
            row.candidate_id,
            row.colonists.iter().filter(|h| h.flour_held > 0).count(),
            row.commons_flour,
            row.millers.len(),
        );
        assert_determinate(seed, class, &axes);
        return SeedOutcome {
            class,
            bucket,
            dominant: None,
            resolves: false,
            row,
        };
    }

    let holders: Vec<_> = row.colonists.iter().filter(|h| h.flour_held > 0).collect();
    let mut reasons = BTreeMap::<Reason, usize>::new();
    for holder in &holders {
        *reasons.entry(reason_of(holder.ask_outcome)).or_default() += 1;
    }
    let top = reasons.values().copied().max().unwrap_or(0);
    let modal: Vec<_> = reasons
        .iter()
        .filter(|&(_, &count)| count == top)
        .map(|(&reason, _)| reason)
        .collect();
    let dominant = (modal.len() == 1).then_some(modal[0]);

    let accepts_before = settlement.role_choice_diag().bake.accepts;
    let mut texture = WindowTexture::default();
    for _ in 0..WINDOW {
        settlement.econ_tick();
        let sample = settlement
            .debug_flour_census_row_now(row.candidate_id)
            .expect("configured chain has a money good");
        check_row(&sample);
        if bucket_of(&sample) == Bucket::HolderWithoutAsk {
            texture.wall_ticks += 1;
        } else if sample.colonists.iter().any(|h| h.flour_held > 0) {
            texture.held_with_ask_ticks += 1;
        } else {
            texture.no_holder_ticks += 1;
        }
    }
    // `accepts` only ever increments (`phases.rs:2331/2383/2504`, no reset site), so the
    // post-window compare is the whole window.
    let accepts_after = settlement.role_choice_diag().bake.accepts;
    let axes = Axes {
        bucket,
        inherited,
        resolves: accepts_after > accepts_before,
        dominant,
    };
    let resolves = axes.resolves;
    let class = classify(&axes);

    println!(
        "seed {seed}: CLASS={class:?} wall={bucket:?} decline_tick={} \
         baker_death_tick={baker_death_tick} deaths={} candidate={:?} vocation={:?} \
         holds_oven={} provenance={:?} recorded_inheritor={} | holders={} reasons={reasons:?} \
         dominant={dominant:?} | persistence={resolves} window={WINDOW} \
         accepts={accepts_before}->{accepts_after} texture={texture:?}",
        row.decline_tick,
        row.deaths_before_decline,
        row.candidate_id,
        row.candidate_vocation,
        row.candidate_holds_oven,
        row.candidate_provenance,
        row.candidate_recorded_inheritor,
        holders.len(),
    );
    for holder in holders {
        println!(
            "  holder={:?} vocation={:?} flour={} raw_ask={:?} outcome={:?}",
            holder.id,
            holder.vocation,
            holder.flour_held,
            holder.reservation_ask,
            holder.ask_outcome,
        );
    }
    assert_determinate(seed, class, &axes);
    SeedOutcome {
        class,
        bucket,
        dominant,
        resolves,
        row,
    }
}

#[test]
fn holder_ask_absence_decomposition() {
    for seed in SEEDS {
        let outcome = decompose(seed);
        // All five canonical seeds are MEASURED to reach the same state, so pin it for
        // every seed (not just seed 3). A regression to any sibling bucket, a resolved
        // window, an inheriting candidate, or a non-satiation reason must fail here —
        // `OtherWall` is reserved for non-canonical classification inputs.
        assert_eq!(
            outcome.bucket,
            Bucket::HolderWithoutAsk,
            "seed {seed}: expected the holder-without-ask wall"
        );
        assert!(
            outcome.row.colonists.iter().any(|h| h.flour_held > 0),
            "seed {seed}: flour must be physically present"
        );
        assert!(
            outcome
                .row
                .colonists
                .iter()
                .all(|h| h.reservation_ask.is_none()),
            "seed {seed}: every living non-self raw flour ask must be None"
        );
        assert_eq!(
            outcome.class,
            Classification::NotPostDeathHeir,
            "seed {seed}: the post-death Bake decliner is a seeded-latent oven holder, not an heir"
        );
        assert_eq!(
            outcome.dominant,
            Some(Reason::MoneySatiated),
            "seed {seed}: the absent ask is money-want satiation"
        );
        assert!(
            !outcome.resolves,
            "seed {seed}: the wall must persist across the window"
        );
    }
}

#[test]
fn canonical_bytes_excludes_holder_ask_census() {
    let cfg = config();
    let mut settlement = Settlement::generate(SEEDS[0], &cfg);
    settlement.run(300);
    let before = settlement.canonical_bytes();
    settlement.debug_arm_flour_census();
    let probe = settlement.colonist_id(0).expect("generated colonist");
    check_row(
        &settlement
            .debug_flour_census_row_now(probe)
            .expect("configured chain has a money good"),
    );
    assert_eq!(before, settlement.canonical_bytes());

    let mut plain = Settlement::generate(SEEDS[0], &cfg);
    let mut armed = Settlement::generate(SEEDS[0], &cfg);
    armed.debug_arm_flour_census();
    let probe = armed.colonist_id(0).expect("generated colonist");
    let mut captured = false;
    let mut sampled = false;
    for _ in 0..MAX_TICKS {
        plain.econ_tick();
        armed.econ_tick();
        if let Some(row) = armed.debug_flour_census_row_now(probe) {
            check_row(&row);
            sampled = true;
        }
        assert_eq!(plain.canonical_bytes(), armed.canonical_bytes());
        if armed.debug_take_flour_census().is_some() {
            captured = true;
            break;
        }
    }
    assert!(captured, "armed census never captured");
    assert!(sampled, "on-demand row was never sampled");
}
