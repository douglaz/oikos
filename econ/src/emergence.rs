//! Emergence-robustness batch runner and per-world outcome extraction.
//!
//! This is a MEASUREMENT instrument in the M4 tradition: it generates many
//! untuned V2 worlds (see [`crate::worldgen`]), runs each through the existing
//! barter/saleability/promotion machinery unchanged, and reads outcomes from
//! the public V2 surfaces only. It changes no promotion rule and asserts no
//! target on the emergence rate — the rate is REPORTED as the baseline.

use crate::good::{good_name, GoodId};
use crate::rng::Rng;
use crate::society::Society;
use crate::worldgen::{
    generate_world, generate_world_from_seed_tuned, generate_world_tuned, EmergenceTuning,
    GeneratedWorld, WorldClass, WorldEnvelope, WorldFamily, GOOD_POOL,
};

/// Worlds in the automatic paired degenerate control run for each `--vary`
/// combo (a weak Goodhart guard on its own — fact 6 — kept alongside the
/// quality aggregates that are the real ones).
pub const DEGENERATE_CONTROL_WORLDS: u32 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmergenceFormat {
    Table,
    Csv,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmergenceOutcome {
    pub world_index: u32,
    pub world_seed: u64,
    pub family: WorldFamily,
    pub class: WorldClass,
    pub population: u16,
    pub candidate_goods: u8,
    pub surplus_goods: u8,
    pub promoted: bool,
    pub winner: Option<GoodId>,
    pub promotion_tick: Option<u64>,
    pub winner_share_bps: u16,
    pub runner_up_share_bps: u16,
    pub money_units: u32,
    pub money_conserved: bool,
    pub barter_trades: u32,
    pub post_promotion_spot_trades: u32,
    /// The world's demand-breadth skew target (H3), `None` at skew 0. Carried
    /// for programmatic/study access; the default per-world CSV does NOT render
    /// it (see [`render_emergence`]) so the M18 baseline rows stay byte-identical.
    pub skew_target: Option<GoodId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmergenceSummary {
    pub worlds: u32,
    pub in_envelope: u32,
    pub out_of_envelope: u32,
    pub degenerate: u32,
    pub promoted_in_envelope: u32,
    pub emergence_rate_bps: u32,
    /// Distinct winning goods, scoped to in-envelope promotions (see
    /// `winner_histogram`).
    pub distinct_winners: u32,
    /// Winning goods and their promotion counts, in GOOD_POOL id order. Scoped to
    /// PROMOTED IN-ENVELOPE worlds, to match the `emergence_rate_bps` denominator;
    /// out-of-envelope promotions (none at the calibrated envelope) are excluded.
    pub winner_histogram: Vec<(GoodId, u32)>,
    pub promotion_tick_median: Option<u64>,
    pub promotion_tick_p90: Option<u64>,
    pub degenerate_promotions: u32,
    pub conservation_failures: u32,
    /// Quality aggregate (Goodhart guard): among promoted in-envelope worlds,
    /// the share (bps) whose winner settles >=1 post-promotion spot trade.
    /// `None` when no in-envelope world promoted — empty statistics stay empty,
    /// never a zero standing in for data.
    pub money_use_share_bps: Option<u32>,
    /// Quality aggregate: the median `winner_share_bps` at promotion across
    /// promoted in-envelope worlds. `None` when no in-envelope world promoted.
    /// Collapsing medians under relaxed floors are the noise-promotion signature.
    pub winner_share_median_bps: Option<u32>,
}

/// Promotion facts read purely from a society's public V2 surfaces. Works on
/// any `Society` that ran an emergent-money scenario — generated or hand-built
/// — which is what lets the harness reproduce the salt/gold anchors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PromotionFacts {
    pub promoted: bool,
    pub winner: Option<GoodId>,
    pub promotion_tick: Option<u64>,
    pub winner_share_bps: u16,
    pub runner_up_share_bps: u16,
    pub money_units: u32,
    pub money_conserved: bool,
    pub barter_trades: u32,
    pub post_promotion_spot_trades: u32,
}

/// Extract promotion facts from a finished society's public surfaces only:
/// `v2_records`, `barter_trades`, and the spot tape (`trades`). No
/// instrumentation is added inside the tick loop.
pub fn promotion_facts(society: &Society) -> PromotionFacts {
    let barter_trades = u32::try_from(society.barter_trades.len()).unwrap_or(u32::MAX);

    let promotion_records = society
        .v2_records
        .iter()
        .filter(|record| record.promoted_this_tick)
        .collect::<Vec<_>>();
    let Some(promotion) = promotion_records
        .iter()
        .copied()
        .min_by_key(|record| record.tick)
    else {
        return PromotionFacts {
            promoted: false,
            winner: None,
            promotion_tick: None,
            winner_share_bps: 0,
            runner_up_share_bps: 0,
            money_units: 0,
            money_conserved: true,
            barter_trades,
            post_promotion_spot_trades: 0,
        };
    };

    let promotion_tick = promotion.tick;
    let total_at_promotion = promotion.total_money_units;
    let money_conserved = society
        .v2_records
        .iter()
        .filter(|record| record.tick >= promotion_tick)
        .all(|record| record.total_money_units == total_at_promotion)
        && society.total_gold() == total_at_promotion
        && promotion_records.len() == 1;
    let post_promotion_spot_trades = u32::try_from(
        society
            .trades
            .iter()
            .filter(|trade| trade.tick > promotion_tick)
            .count(),
    )
    .unwrap_or(u32::MAX);

    PromotionFacts {
        promoted: true,
        winner: promotion.money_good,
        promotion_tick: Some(promotion_tick),
        winner_share_bps: promotion.candidate_share_bps.unwrap_or(0),
        runner_up_share_bps: promotion.runner_up_share_bps.unwrap_or(0),
        money_units: u32::try_from(total_at_promotion.0).unwrap_or(u32::MAX),
        money_conserved,
        barter_trades,
        post_promotion_spot_trades,
    }
}

/// Generate, run, and return one world's society together with its generated
/// metadata. Exposed so tests can inspect the runtime surfaces directly.
pub fn run_generated_world(
    master_seed: u64,
    world_index: u32,
    family: WorldFamily,
    envelope: &WorldEnvelope,
) -> (GeneratedWorld, Society) {
    let world = generate_world(master_seed, world_index, family, envelope);
    let periods = world.scenario.periods;
    let mut society = Society::from_scenario(world.scenario.clone());
    society.run(periods);
    (world, society)
}

/// Tuned variant of [`run_generated_world`] applying the M19 [`EmergenceTuning`]
/// overrides. With `EmergenceTuning::default()` it is byte-identical.
pub fn run_generated_world_tuned(
    master_seed: u64,
    world_index: u32,
    family: WorldFamily,
    envelope: &WorldEnvelope,
    tuning: &EmergenceTuning,
) -> (GeneratedWorld, Society) {
    let world = generate_world_tuned(master_seed, world_index, family, envelope, tuning);
    let periods = world.scenario.periods;
    let mut society = Society::from_scenario(world.scenario.clone());
    society.run(periods);
    (world, society)
}

fn outcome_for(
    world_index: u32,
    world: &GeneratedWorld,
    family: WorldFamily,
    society: &Society,
) -> EmergenceOutcome {
    let facts = promotion_facts(society);
    EmergenceOutcome {
        world_index,
        world_seed: world.world_seed,
        family,
        class: world.profile.class,
        population: world.profile.population,
        candidate_goods: world.profile.candidate_goods,
        surplus_goods: world.profile.surplus_goods,
        promoted: facts.promoted,
        winner: facts.winner,
        promotion_tick: facts.promotion_tick,
        winner_share_bps: facts.winner_share_bps,
        runner_up_share_bps: facts.runner_up_share_bps,
        money_units: facts.money_units,
        money_conserved: facts.money_conserved,
        barter_trades: facts.barter_trades,
        post_promotion_spot_trades: facts.post_promotion_spot_trades,
        skew_target: world.profile.skew_target,
    }
}

/// Run the whole corpus: a deterministic, seed-pure batch of generated worlds.
/// Two runs with the same arguments are byte-identical. Worlds are never
/// resampled; the rate is computed over the in-envelope subset.
pub fn run_emergence_corpus(
    master_seed: u64,
    worlds: u32,
    family: WorldFamily,
    envelope: &WorldEnvelope,
) -> (Vec<EmergenceOutcome>, EmergenceSummary) {
    run_emergence_corpus_tuned(
        master_seed,
        worlds,
        family,
        envelope,
        &EmergenceTuning::default(),
    )
}

/// Tuned variant of [`run_emergence_corpus`] applying the M19 [`EmergenceTuning`]
/// overrides. With `EmergenceTuning::default()` it is byte-identical, which is
/// what makes the M18 baseline reproduce exactly under the study instrument.
pub fn run_emergence_corpus_tuned(
    master_seed: u64,
    worlds: u32,
    family: WorldFamily,
    envelope: &WorldEnvelope,
    tuning: &EmergenceTuning,
) -> (Vec<EmergenceOutcome>, EmergenceSummary) {
    let mut outcomes = Vec::with_capacity(worlds as usize);
    let mut seed_rng = Rng::new(master_seed);
    for world_index in 0..worlds {
        let world_seed = seed_rng.next_u64();
        let world = generate_world_from_seed_tuned(world_seed, family, envelope, tuning);
        let periods = world.scenario.periods;
        let mut society = Society::from_scenario(world.scenario.clone());
        society.run(periods);
        outcomes.push(outcome_for(world_index, &world, family, &society));
    }
    let summary = summarize(&outcomes);
    (outcomes, summary)
}

fn summarize(outcomes: &[EmergenceOutcome]) -> EmergenceSummary {
    let worlds = u32::try_from(outcomes.len()).unwrap_or(u32::MAX);
    let mut in_envelope = 0u32;
    let mut out_of_envelope = 0u32;
    let mut degenerate = 0u32;
    let mut promoted_in_envelope = 0u32;
    let mut degenerate_promotions = 0u32;
    let mut conservation_failures = 0u32;

    // Winner histogram over the static pool, in id order (no HashMap). Any winner
    // outside the pool (unreachable for generated worlds, whose candidates are
    // always drawn from GOOD_POOL) is appended and re-sorted below, so the id-order
    // invariant holds unconditionally.
    let mut histogram: Vec<(GoodId, u32)> = GOOD_POOL.iter().map(|good| (*good, 0u32)).collect();
    let mut promotion_ticks: Vec<u64> = Vec::new();

    // Quality-aggregate accumulators over promoted in-envelope worlds only, to
    // match the denominator used by the headline rate.
    let mut promoted_count = 0u32;
    let mut promoted_with_money_use = 0u32;
    let mut winner_shares: Vec<u64> = Vec::new();

    for outcome in outcomes {
        match outcome.class {
            WorldClass::InEnvelope => in_envelope = in_envelope.saturating_add(1),
            WorldClass::OutOfEnvelope => out_of_envelope = out_of_envelope.saturating_add(1),
        }
        if outcome.family == WorldFamily::Degenerate {
            degenerate = degenerate.saturating_add(1);
            if outcome.promoted {
                degenerate_promotions = degenerate_promotions.saturating_add(1);
            }
        }
        if outcome.promoted {
            if outcome.class == WorldClass::InEnvelope {
                promoted_in_envelope = promoted_in_envelope.saturating_add(1);
                promoted_count = promoted_count.saturating_add(1);
                if outcome.post_promotion_spot_trades > 0 {
                    promoted_with_money_use = promoted_with_money_use.saturating_add(1);
                }
                winner_shares.push(u64::from(outcome.winner_share_bps));
                if let Some(tick) = outcome.promotion_tick {
                    promotion_ticks.push(tick);
                }
                if let Some(winner) = outcome.winner {
                    if let Some(entry) = histogram.iter_mut().find(|(good, _)| *good == winner) {
                        entry.1 = entry.1.saturating_add(1);
                    } else {
                        histogram.push((winner, 1));
                    }
                }
            }
            if !outcome.money_conserved {
                conservation_failures = conservation_failures.saturating_add(1);
            }
        }
    }

    let mut winner_histogram: Vec<(GoodId, u32)> = histogram
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .collect();
    winner_histogram.sort_by_key(|(good, _)| good.0);
    let distinct_winners = u32::try_from(winner_histogram.len()).unwrap_or(u32::MAX);

    let emergence_rate_bps = if in_envelope == 0 {
        0
    } else {
        u32::try_from(u64::from(promoted_in_envelope) * 10_000 / u64::from(in_envelope))
            .unwrap_or(u32::MAX)
    };

    promotion_ticks.sort_unstable();
    let promotion_tick_median = percentile(&promotion_ticks, 50);
    let promotion_tick_p90 = percentile(&promotion_ticks, 90);

    // Quality aggregates: defined ONLY over promoted in-envelope worlds. With
    // none promoted they stay `None` — empty statistics stay empty.
    let money_use_share_bps = if promoted_count == 0 {
        None
    } else {
        Some(
            u32::try_from(u64::from(promoted_with_money_use) * 10_000 / u64::from(promoted_count))
                .unwrap_or(u32::MAX),
        )
    };
    winner_shares.sort_unstable();
    let winner_share_median_bps =
        percentile(&winner_shares, 50).map(|share| u32::try_from(share).unwrap_or(u32::MAX));

    EmergenceSummary {
        worlds,
        in_envelope,
        out_of_envelope,
        degenerate,
        promoted_in_envelope,
        emergence_rate_bps,
        distinct_winners,
        winner_histogram,
        promotion_tick_median,
        promotion_tick_p90,
        degenerate_promotions,
        conservation_failures,
        money_use_share_bps,
        winner_share_median_bps,
    }
}

/// Nearest-rank percentile over an ascending-sorted slice. Returns `None` for
/// an empty set. `p` is a whole percent in `0..=100`.
fn percentile(sorted: &[u64], p: u64) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let n = sorted.len() as u64;
    let rank = (n * p).div_ceil(100).max(1);
    let index = (rank - 1).min(n - 1) as usize;
    Some(sorted[index])
}

pub const EMERGENCE_CSV_HEADER: &str = "world_index,world_seed,family,class,population,candidate_goods,surplus_goods,promoted,winner,promotion_tick,winner_share_bps,runner_up_share_bps,money_units,money_conserved,barter_trades,post_promotion_spot_trades";

pub const EMERGENCE_SUMMARY_CSV_HEADER: &str = "worlds,in_envelope,out_of_envelope,degenerate,promoted_in_envelope,emergence_rate_bps,distinct_winners,promotion_tick_median,promotion_tick_p90,money_use_share_bps,winner_share_median_bps,degenerate_promotions,conservation_failures";

const EMERGENCE_TABLE_COLUMNS: [(&str, usize); 16] = [
    ("world", 5),
    ("seed", 20),
    ("family", 10),
    ("class", 12),
    ("pop", 3),
    ("cands", 5),
    ("surplus", 7),
    ("promoted", 8),
    ("winner", 6),
    ("tick", 4),
    ("share", 5),
    ("runner", 6),
    ("units", 5),
    ("conserved", 9),
    ("barter", 6),
    ("post_spot", 9),
];

pub fn render_emergence(
    outcomes: &[EmergenceOutcome],
    summary: &EmergenceSummary,
    format: EmergenceFormat,
) -> String {
    match format {
        EmergenceFormat::Csv => render_csv(outcomes, summary),
        EmergenceFormat::Table => render_table(outcomes, summary),
    }
}

fn winner_field(winner: Option<GoodId>) -> String {
    winner.map(good_name).unwrap_or("").to_string()
}

fn tick_field(tick: Option<u64>) -> String {
    tick.map(|tick| tick.to_string()).unwrap_or_default()
}

fn render_csv(outcomes: &[EmergenceOutcome], summary: &EmergenceSummary) -> String {
    let mut out = String::new();
    out.push_str(EMERGENCE_CSV_HEADER);
    out.push('\n');
    for outcome in outcomes {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            outcome.world_index,
            outcome.world_seed,
            outcome.family.as_str(),
            outcome.class.as_str(),
            outcome.population,
            outcome.candidate_goods,
            outcome.surplus_goods,
            outcome.promoted,
            winner_field(outcome.winner),
            tick_field(outcome.promotion_tick),
            outcome.winner_share_bps,
            outcome.runner_up_share_bps,
            outcome.money_units,
            outcome.money_conserved,
            outcome.barter_trades,
            outcome.post_promotion_spot_trades,
        ));
    }

    out.push('\n');
    out.push_str(EMERGENCE_SUMMARY_CSV_HEADER);
    out.push('\n');
    out.push_str(&format!(
        "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
        summary.worlds,
        summary.in_envelope,
        summary.out_of_envelope,
        summary.degenerate,
        summary.promoted_in_envelope,
        summary.emergence_rate_bps,
        summary.distinct_winners,
        tick_field(summary.promotion_tick_median),
        tick_field(summary.promotion_tick_p90),
        opt_u32_field(summary.money_use_share_bps),
        opt_u32_field(summary.winner_share_median_bps),
        summary.degenerate_promotions,
        summary.conservation_failures,
    ));

    out.push('\n');
    out.push_str("winner,promotions\n");
    for (good, count) in &summary.winner_histogram {
        out.push_str(&format!("{},{}\n", good_name(*good), count));
    }

    out
}

fn render_table(outcomes: &[EmergenceOutcome], summary: &EmergenceSummary) -> String {
    let mut out = String::new();
    let header = EMERGENCE_TABLE_COLUMNS
        .iter()
        .map(|(label, _)| (*label).to_string())
        .collect::<Vec<_>>();
    let rows = outcomes
        .iter()
        .map(emergence_table_record_values)
        .collect::<Vec<_>>();
    let widths = emergence_table_widths(&header, &rows);

    push_emergence_table_row(&mut out, &header, &widths);
    for row in &rows {
        push_emergence_table_row(&mut out, row, &widths);
    }

    out.push('\n');
    out.push_str("summary:\n");
    out.push_str(&format!("  worlds: {}\n", summary.worlds));
    out.push_str(&format!("  in_envelope: {}\n", summary.in_envelope));
    out.push_str(&format!("  out_of_envelope: {}\n", summary.out_of_envelope));
    out.push_str(&format!("  degenerate: {}\n", summary.degenerate));
    out.push_str(&format!(
        "  promoted_in_envelope: {}\n",
        summary.promoted_in_envelope
    ));
    out.push_str(&format!(
        "  emergence_rate_bps: {} (promoted/in_envelope)\n",
        summary.emergence_rate_bps
    ));
    out.push_str(&format!(
        "  distinct_winners: {}\n",
        summary.distinct_winners
    ));
    out.push_str(&format!(
        "  promotion_tick_median: {}\n",
        tick_field(summary.promotion_tick_median)
    ));
    out.push_str(&format!(
        "  promotion_tick_p90: {}\n",
        tick_field(summary.promotion_tick_p90)
    ));
    out.push_str(&format!(
        "  money_use_share_bps: {}\n",
        opt_u32_field(summary.money_use_share_bps)
    ));
    out.push_str(&format!(
        "  winner_share_median_bps: {}\n",
        opt_u32_field(summary.winner_share_median_bps)
    ));
    out.push_str(&format!(
        "  degenerate_promotions: {} (must be 0)\n",
        summary.degenerate_promotions
    ));
    out.push_str(&format!(
        "  conservation_failures: {} (must be 0)\n",
        summary.conservation_failures
    ));
    out.push_str("  winner_histogram:\n");
    for (good, count) in &summary.winner_histogram {
        out.push_str(&format!("    {}: {}\n", good_name(*good), count));
    }

    out
}

fn emergence_table_widths(header: &[String], rows: &[Vec<String>]) -> Vec<usize> {
    EMERGENCE_TABLE_COLUMNS
        .iter()
        .enumerate()
        .map(|(index, (label, min_width))| {
            let row_width = rows
                .iter()
                .filter_map(|row| row.get(index))
                .map(String::len)
                .max()
                .unwrap_or(0);
            (*min_width)
                .max(label.len())
                .max(header.get(index).map(String::len).unwrap_or(0))
                .max(row_width)
        })
        .collect()
}

fn push_emergence_table_row(out: &mut String, values: &[String], widths: &[usize]) {
    for (index, (value, width)) in values.iter().zip(widths).enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let width = *width;
        out.push_str(&format!("{value:>width$}"));
    }
    out.push('\n');
}

fn emergence_table_record_values(outcome: &EmergenceOutcome) -> Vec<String> {
    vec![
        outcome.world_index.to_string(),
        outcome.world_seed.to_string(),
        outcome.family.as_str().to_string(),
        outcome.class.as_str().to_string(),
        outcome.population.to_string(),
        outcome.candidate_goods.to_string(),
        outcome.surplus_goods.to_string(),
        outcome.promoted.to_string(),
        winner_field(outcome.winner),
        tick_field(outcome.promotion_tick),
        outcome.winner_share_bps.to_string(),
        outcome.runner_up_share_bps.to_string(),
        outcome.money_units.to_string(),
        outcome.money_conserved.to_string(),
        outcome.barter_trades.to_string(),
        outcome.post_promotion_spot_trades.to_string(),
    ]
}

/// A whitelisted `--vary` axis key (M19 study). Each maps to one
/// [`EmergenceTuning`] field; unknown keys are a parser error, exactly as the
/// M4 `sweep` whitelist. Kebab-case on the CLI, applied to the tuning struct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmergenceKey {
    PromotionThresholdBps,
    LeadMarginBps,
    MinCounterpartGoods,
    StabilityTicks,
    IndirectMinShareBps,
    MinTotalAcceptancesFloor,
    MinAcceptorShareBps,
    FutureWantShareBps,
    DemandBreadthSkewBps,
    Periods,
}

impl EmergenceKey {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "promotion-threshold-bps" => Some(Self::PromotionThresholdBps),
            "lead-margin-bps" => Some(Self::LeadMarginBps),
            "min-counterpart-goods" => Some(Self::MinCounterpartGoods),
            "stability-ticks" => Some(Self::StabilityTicks),
            "indirect-min-share-bps" => Some(Self::IndirectMinShareBps),
            "min-total-acceptances-floor" => Some(Self::MinTotalAcceptancesFloor),
            "min-acceptor-share-bps" => Some(Self::MinAcceptorShareBps),
            "future-want-share-bps" => Some(Self::FutureWantShareBps),
            "demand-breadth-skew-bps" => Some(Self::DemandBreadthSkewBps),
            "periods" => Some(Self::Periods),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PromotionThresholdBps => "promotion-threshold-bps",
            Self::LeadMarginBps => "lead-margin-bps",
            Self::MinCounterpartGoods => "min-counterpart-goods",
            Self::StabilityTicks => "stability-ticks",
            Self::IndirectMinShareBps => "indirect-min-share-bps",
            Self::MinTotalAcceptancesFloor => "min-total-acceptances-floor",
            Self::MinAcceptorShareBps => "min-acceptor-share-bps",
            Self::FutureWantShareBps => "future-want-share-bps",
            Self::DemandBreadthSkewBps => "demand-breadth-skew-bps",
            Self::Periods => "periods",
        }
    }

    /// Apply this axis value to `tuning`. Integer math only; out-of-range values
    /// for the field's target type are rejected (parser discipline).
    pub fn apply(self, tuning: &mut EmergenceTuning, value: u64) -> Result<(), String> {
        match self {
            Self::PromotionThresholdBps => {
                tuning.promotion_threshold_bps = Some(as_u16(self, value)?)
            }
            Self::LeadMarginBps => tuning.lead_margin_bps = Some(as_u16(self, value)?),
            Self::MinCounterpartGoods => tuning.min_counterpart_goods = Some(as_u16(self, value)?),
            Self::StabilityTicks => tuning.stability_ticks = Some(as_u32(self, value)?),
            Self::IndirectMinShareBps => tuning.indirect_min_share_bps = Some(as_u16(self, value)?),
            Self::MinTotalAcceptancesFloor => {
                tuning.min_total_acceptances_floor = Some(as_u32(self, value)?)
            }
            Self::MinAcceptorShareBps => tuning.min_acceptor_share_bps = Some(as_u16(self, value)?),
            Self::FutureWantShareBps => tuning.future_want_share_bps = Some(as_u16(self, value)?),
            Self::DemandBreadthSkewBps => {
                tuning.demand_breadth_skew_bps = Some(as_u16(self, value)?)
            }
            Self::Periods => tuning.periods = Some(value),
        }
        Ok(())
    }
}

fn as_u16(key: EmergenceKey, value: u64) -> Result<u16, String> {
    u16::try_from(value)
        .map_err(|_| format!("emergence sweep value for '{}' overflows", key.as_str()))
}

fn as_u32(key: EmergenceKey, value: u64) -> Result<u32, String> {
    u32::try_from(value)
        .map_err(|_| format!("emergence sweep value for '{}' overflows", key.as_str()))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmergenceSweepAxis {
    pub key: EmergenceKey,
    pub values: Vec<u64>,
}

/// Parse one `--vary KEY=V1,V2,...` axis spec. Same parser discipline as the M4
/// `sweep` whitelist: a missing `=`, an unknown key, an empty value list, or a
/// non-unsigned-integer value are all errors. Integer math only; no allocation
/// beyond the value vector.
pub fn parse_emergence_axis(spec: &str) -> Result<EmergenceSweepAxis, String> {
    let (key, raw_values) = spec
        .split_once('=')
        .ok_or_else(|| "invalid --vary".to_string())?;
    let key =
        EmergenceKey::parse(key).ok_or_else(|| format!("unknown emergence sweep key '{key}'"))?;
    if raw_values.is_empty() {
        return Err("empty emergence sweep value list".to_string());
    }

    let mut values = Vec::new();
    for raw in raw_values.split(',') {
        if raw.is_empty() {
            return Err("empty emergence sweep value list".to_string());
        }
        if raw.starts_with('-') {
            return Err("emergence sweep values must be unsigned integers".to_string());
        }
        values.push(
            raw.parse::<u64>()
                .map_err(|_| "invalid emergence sweep value".to_string())?,
        );
    }
    Ok(EmergenceSweepAxis { key, values })
}

/// One Cartesian-combo summary row. The swept parameters are in `variables`
/// (axis order); the statistics are the same family of measurements the no-vary
/// summary reports, plus the two quality aggregates and the paired degenerate
/// control's promotion count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmergenceComboRow {
    pub variables: Vec<(EmergenceKey, u64)>,
    pub worlds: u32,
    pub in_envelope: u32,
    pub promoted: u32,
    pub emergence_rate_bps: u32,
    pub distinct_winners: u32,
    pub promotion_tick_median: Option<u64>,
    pub promotion_tick_p90: Option<u64>,
    pub money_use_share_bps: Option<u32>,
    pub winner_share_median_bps: Option<u32>,
    pub degenerate_promotions: u32,
    pub conservation_failures: u32,
}

/// Run the `--vary` Cartesian sweep. Every combo uses the SAME `master_seed` and
/// `worlds` (paired comparison: across combos, world *i* differs only by the
/// swept parameters), and each combo is paired with an automatic degenerate
/// control of [`DEGENERATE_CONTROL_WORLDS`] worlds at the same seed. Combos run
/// in deterministic Cartesian order (axis 0 outermost). The master seed is NEVER
/// re-seeded per combo.
pub fn run_emergence_sweep(
    master_seed: u64,
    worlds: u32,
    envelope: &WorldEnvelope,
    axes: &[EmergenceSweepAxis],
) -> Result<Vec<EmergenceComboRow>, String> {
    for (index, axis) in axes.iter().enumerate() {
        if axis.values.is_empty() {
            return Err(format!(
                "emergence sweep axis '{}' has no values",
                axis.key.as_str()
            ));
        }
        if axes[..index].iter().any(|seen| seen.key == axis.key) {
            return Err(format!(
                "duplicate emergence sweep key '{}'",
                axis.key.as_str()
            ));
        }
    }

    let mut rows = Vec::new();
    let mut variables = Vec::new();
    run_emergence_sweep_axis(
        master_seed,
        worlds,
        envelope,
        axes,
        0,
        &mut variables,
        &mut rows,
    )?;
    Ok(rows)
}

#[allow(clippy::too_many_arguments)]
fn run_emergence_sweep_axis(
    master_seed: u64,
    worlds: u32,
    envelope: &WorldEnvelope,
    axes: &[EmergenceSweepAxis],
    axis_index: usize,
    variables: &mut Vec<(EmergenceKey, u64)>,
    rows: &mut Vec<EmergenceComboRow>,
) -> Result<(), String> {
    if axis_index == axes.len() {
        rows.push(emergence_combo_row(
            master_seed,
            worlds,
            envelope,
            variables,
        )?);
        return Ok(());
    }
    let axis = &axes[axis_index];
    for value in &axis.values {
        variables.push((axis.key, *value));
        run_emergence_sweep_axis(
            master_seed,
            worlds,
            envelope,
            axes,
            axis_index + 1,
            variables,
            rows,
        )?;
        variables.pop();
    }
    Ok(())
}

fn emergence_combo_row(
    master_seed: u64,
    worlds: u32,
    envelope: &WorldEnvelope,
    variables: &[(EmergenceKey, u64)],
) -> Result<EmergenceComboRow, String> {
    let mut tuning = EmergenceTuning::default();
    for (key, value) in variables {
        key.apply(&mut tuning, *value)?;
    }

    let (_random, summary) =
        run_emergence_corpus_tuned(master_seed, worlds, WorldFamily::Random, envelope, &tuning);
    let (_degenerate, degenerate_summary) = run_emergence_corpus_tuned(
        master_seed,
        DEGENERATE_CONTROL_WORLDS,
        WorldFamily::Degenerate,
        envelope,
        &tuning,
    );

    Ok(EmergenceComboRow {
        variables: variables.to_vec(),
        worlds: summary.worlds,
        in_envelope: summary.in_envelope,
        promoted: summary.promoted_in_envelope,
        emergence_rate_bps: summary.emergence_rate_bps,
        distinct_winners: summary.distinct_winners,
        promotion_tick_median: summary.promotion_tick_median,
        promotion_tick_p90: summary.promotion_tick_p90,
        money_use_share_bps: summary.money_use_share_bps,
        winner_share_median_bps: summary.winner_share_median_bps,
        degenerate_promotions: degenerate_summary.degenerate_promotions,
        conservation_failures: combined_conservation_failures(&summary, &degenerate_summary),
    })
}

fn combined_conservation_failures(
    summary: &EmergenceSummary,
    degenerate_summary: &EmergenceSummary,
) -> u32 {
    summary
        .conservation_failures
        .saturating_add(degenerate_summary.conservation_failures)
}

const EMERGENCE_SWEEP_STAT_COLUMNS: [&str; 11] = [
    "worlds",
    "in_envelope",
    "promoted",
    "emergence_rate_bps",
    "distinct_winners",
    "promotion_tick_median",
    "promotion_tick_p90",
    "money_use_share_bps",
    "winner_share_median_bps",
    "degenerate_promotions",
    "conservation_failures",
];

fn opt_u32_field(value: Option<u32>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn emergence_sweep_stat_values(row: &EmergenceComboRow) -> Vec<String> {
    vec![
        row.worlds.to_string(),
        row.in_envelope.to_string(),
        row.promoted.to_string(),
        row.emergence_rate_bps.to_string(),
        row.distinct_winners.to_string(),
        tick_field(row.promotion_tick_median),
        tick_field(row.promotion_tick_p90),
        opt_u32_field(row.money_use_share_bps),
        opt_u32_field(row.winner_share_median_bps),
        row.degenerate_promotions.to_string(),
        row.conservation_failures.to_string(),
    ]
}

fn emergence_sweep_header(axes: &[EmergenceSweepAxis]) -> Vec<String> {
    let mut header: Vec<String> = axes
        .iter()
        .map(|axis| axis.key.as_str().to_string())
        .collect();
    header.extend(
        EMERGENCE_SWEEP_STAT_COLUMNS
            .iter()
            .map(|label| (*label).to_string()),
    );
    header
}

fn emergence_sweep_row_values(axes: &[EmergenceSweepAxis], row: &EmergenceComboRow) -> Vec<String> {
    let mut values: Vec<String> = axes
        .iter()
        .map(|axis| {
            row.variables
                .iter()
                .find(|(key, _)| *key == axis.key)
                .map(|(_, value)| value.to_string())
                .unwrap_or_default()
        })
        .collect();
    values.extend(emergence_sweep_stat_values(row));
    values
}

pub fn render_emergence_sweep(
    axes: &[EmergenceSweepAxis],
    rows: &[EmergenceComboRow],
    format: EmergenceFormat,
) -> String {
    let header = emergence_sweep_header(axes);
    let body: Vec<Vec<String>> = rows
        .iter()
        .map(|row| emergence_sweep_row_values(axes, row))
        .collect();
    match format {
        EmergenceFormat::Csv => {
            let mut out = String::new();
            out.push_str(&header.join(","));
            out.push('\n');
            for values in &body {
                out.push_str(&values.join(","));
                out.push('\n');
            }
            out
        }
        EmergenceFormat::Table => {
            let widths: Vec<usize> = header
                .iter()
                .enumerate()
                .map(|(index, label)| {
                    body.iter()
                        .filter_map(|row| row.get(index))
                        .map(String::len)
                        .max()
                        .unwrap_or(0)
                        .max(label.len())
                })
                .collect();
            let mut out = String::new();
            push_emergence_table_row(&mut out, &header, &widths);
            for values in &body {
                push_emergence_table_row(&mut out, values, &widths);
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        combined_conservation_failures, percentile, render_emergence, run_emergence_corpus,
        summarize, EmergenceFormat, EmergenceOutcome, EmergenceSummary,
    };
    use crate::good::{FOOD, GOLD};
    use crate::worldgen::{WorldClass, WorldEnvelope, WorldFamily};

    #[test]
    fn percentile_uses_nearest_rank() {
        let data: Vec<u64> = (1..=10).collect();
        assert_eq!(percentile(&data, 50), Some(5));
        assert_eq!(percentile(&data, 90), Some(9));
        assert_eq!(percentile(&[], 50), None);
        assert_eq!(percentile(&[7], 90), Some(7));
    }

    #[test]
    fn corpus_is_deterministic() {
        let envelope = WorldEnvelope::default();
        let (first, summary_a) = run_emergence_corpus(18, 12, WorldFamily::Random, &envelope);
        let (second, summary_b) = run_emergence_corpus(18, 12, WorldFamily::Random, &envelope);
        assert_eq!(first, second);
        assert_eq!(summary_a, summary_b);
        let csv_a = render_emergence(&first, &summary_a, EmergenceFormat::Csv);
        let csv_b = render_emergence(&second, &summary_b, EmergenceFormat::Csv);
        assert_eq!(csv_a, csv_b);
    }

    #[test]
    fn quality_aggregates_use_in_envelope_promotions_only() {
        let outcomes = vec![
            test_outcome(
                WorldClass::InEnvelope,
                WorldFamily::Random,
                true,
                Some(GOLD),
                5_000,
                1,
            ),
            test_outcome(
                WorldClass::OutOfEnvelope,
                WorldFamily::Random,
                true,
                Some(FOOD),
                1_000,
                0,
            ),
        ];
        let summary = summarize(&outcomes);
        assert_eq!(summary.in_envelope, 1);
        assert_eq!(summary.out_of_envelope, 1);
        assert_eq!(summary.promoted_in_envelope, 1);
        assert_eq!(summary.distinct_winners, 1);
        assert_eq!(summary.winner_histogram, vec![(GOLD, 1)]);
        assert_eq!(summary.money_use_share_bps, Some(10_000));
        assert_eq!(summary.winner_share_median_bps, Some(5_000));
    }

    #[test]
    fn combo_rows_count_conservation_failures_from_controls() {
        let random_summary = test_summary(2);
        let degenerate_summary = test_summary(3);
        assert_eq!(
            combined_conservation_failures(&random_summary, &degenerate_summary),
            5
        );
    }

    fn test_outcome(
        class: WorldClass,
        family: WorldFamily,
        promoted: bool,
        winner: Option<crate::good::GoodId>,
        winner_share_bps: u16,
        post_promotion_spot_trades: u32,
    ) -> EmergenceOutcome {
        EmergenceOutcome {
            world_index: 0,
            world_seed: 0,
            family,
            class,
            population: 10,
            candidate_goods: 3,
            surplus_goods: if class == WorldClass::InEnvelope {
                2
            } else {
                1
            },
            promoted,
            winner,
            promotion_tick: promoted.then_some(1),
            winner_share_bps,
            runner_up_share_bps: 0,
            money_units: if promoted { 1 } else { 0 },
            money_conserved: true,
            barter_trades: 1,
            post_promotion_spot_trades,
            skew_target: None,
        }
    }

    fn test_summary(conservation_failures: u32) -> EmergenceSummary {
        EmergenceSummary {
            worlds: 0,
            in_envelope: 0,
            out_of_envelope: 0,
            degenerate: 0,
            promoted_in_envelope: 0,
            emergence_rate_bps: 0,
            distinct_winners: 0,
            winner_histogram: Vec::new(),
            promotion_tick_median: None,
            promotion_tick_p90: None,
            degenerate_promotions: 0,
            conservation_failures,
            money_use_share_bps: None,
            winner_share_median_bps: None,
        }
    }
}
