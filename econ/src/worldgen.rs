//! Deterministic randomized V2 world generation for the emergence harness.
//!
//! This module is an instrument, not a decision module: it constructs
//! `MarketScenario` casts that the existing barter/saleability/promotion
//! machinery then runs unchanged. All randomness lives here, at world
//! GENERATION time, consumed from `Rng` (xorshift64*) in a fixed order and
//! sub-seeded per world index. The tick loop draws nothing.
//!
//! The generator privileges no good: it draws a candidate subset of the
//! static V2 pool uniformly and distributes endowments and wants across the
//! draw uniformly. There is no per-good bias constant anywhere below;
//! `generator_designates_no_winner` is the guard.

use crate::agent::{Agent, AgentId, Role, Want, WantKind};
use crate::expect::PriceBelief;
use crate::good::{Gold, GoodId, Horizon, Stock, CLOTH, FOOD, GOLD, ORE, SALT, WOOD};
use crate::money::{MarketMoneyConfig, MengerianConfig};
use crate::rng::Rng;
use crate::scenario::{MarketScenario, ScenarioName};

/// The static V2 candidate good pool (id order, excluding NET). No good is
/// privileged; the generator draws subsets and distributes endowments/wants
/// uniformly across the draw.
pub const GOOD_POOL: [GoodId; 6] = [GOLD, FOOD, WOOD, SALT, CLOTH, ORE];

/// Cosmetic scenario name for generated worlds. The harness owns world
/// generation, so this is display-only and is never parsed as a built-in.
pub const GENERATED_WORLD_NAME: &str = "emergence-world";

/// Econ ticks each generated world runs (a non-promoting world at the final
/// tick counts as non-emergent).
pub const WORLD_PERIODS: u64 = 40;

/// Default demand-breadth skew (M20). The M19 study
/// (`docs/emergence-study.md`, "Recommended G5 worldgen envelope") found
/// asymmetry to be the Goodhart-safe lever: at 7500 bps the emergence rate
/// roughly doubles while quality IMPROVES (money-use 10000 bps,
/// winner-share median 5000) and false positives stay at zero. Symmetric
/// worlds are Mengerian deserts; a world with a saleability differential is
/// the representative case. The M18 symmetric baseline remains reachable
/// with an explicit `--vary demand-breadth-skew-bps=0` (the skew target is
/// drawn from a forked per-world stream, so skew 0 reproduces the M18
/// worlds byte-identically).
pub const DEFAULT_DEMAND_BREADTH_SKEW_BPS: u16 = 7_500;

/// Internal generator constants for the store-of-value (future-horizon) demand.
/// These are NOT envelope knobs; they shape the patience ingredient the salt
/// cast hand-authored, here randomized so no good is privileged.
const FUTURE_NEED_QTY: (u8, u8) = (3, 6);
const FUTURE_HORIZON: (u8, u8) = (1, 4);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldEnvelope {
    pub population: (u16, u16),
    pub candidate_goods: (u8, u8),
    pub endowed_goods_per_agent: (u8, u8),
    pub endowment_qty: (u8, u8),
    pub wants_per_agent: (u8, u8),
    /// Share of agents (in bps) given an additional `Later`-horizon want.
    pub future_want_share_bps: u16,
}

impl Default for WorldEnvelope {
    fn default() -> Self {
        Self {
            population: (6, 40),
            candidate_goods: (3, 6),
            endowed_goods_per_agent: (1, 2),
            endowment_qty: (1, 4),
            wants_per_agent: (1, 6),
            future_want_share_bps: 3_000,
        }
    }
}

/// Per-run overrides for the emergence STUDY (M19). Every field is `None` by
/// default; `EmergenceTuning::default()` reproduces the M18 instrument exactly,
/// bit-for-bit. The study's `--vary` sweep is the *only* way variation enters:
/// nothing here re-defaults the worldgen envelope, the pinned `MengerianConfig`
/// constants, the floor formulas, or the 40-tick budget.
///
/// The two floor FORMULAS are parameterized through these fields only:
///   `min_total_acceptances = max(floor, population)`            (floor default 12)
///   `min_acceptor_agents   = max(3, population * share_bps / 10_000)` (share default 3000)
/// With the defaults these are bit-identical to the `build_scenario` helper today.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EmergenceTuning {
    // Generated-world `MengerianConfig` overrides (`None` = pinned value).
    pub promotion_threshold_bps: Option<u16>,
    pub lead_margin_bps: Option<u16>,
    pub min_counterpart_goods: Option<u16>,
    pub stability_ticks: Option<u32>,
    pub indirect_min_share_bps: Option<u16>,
    /// The pinned `12` in `min_total_acceptances = max(12, population)`.
    pub min_total_acceptances_floor: Option<u32>,
    /// The pinned `3000` (= 3/10) in `min_acceptor_agents = max(3, pop * bps / 10000)`.
    pub min_acceptor_share_bps: Option<u16>,
    // Envelope overrides.
    pub future_want_share_bps: Option<u16>,
    /// H3 axis: per-world demand-breadth skew toward one randomly drawn
    /// good. Defaults to [`DEFAULT_DEMAND_BREADTH_SKEW_BPS`] (M20, per the
    /// M19 study). At an explicit `0` the generator consumes ZERO
    /// additional RNG draws, so every M18 world is byte-identical
    /// (golden-guarded via the skew-zero path).
    pub demand_breadth_skew_bps: Option<u16>,
    // Run budget.
    pub periods: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldFamily {
    Random,
    Degenerate,
}

impl WorldFamily {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "random" => Some(Self::Random),
            "degenerate" => Some(Self::Degenerate),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Random => "random",
            Self::Degenerate => "degenerate",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldClass {
    InEnvelope,
    OutOfEnvelope,
}

impl WorldClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InEnvelope => "in-envelope",
            Self::OutOfEnvelope => "out-of-envelope",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldProfile {
    pub population: u16,
    pub candidate_goods: u8,
    pub surplus_goods: u8,
    pub class: WorldClass,
    /// The randomly drawn demand-breadth skew target (H3), or `None` when
    /// the effective skew is `0` (the explicit M18-baseline override; the
    /// M20 default is [`DEFAULT_DEMAND_BREADTH_SKEW_BPS`]). It is per-world
    /// random, drawn from this world's own candidate subset; no good is
    /// designated globally. At skew 0 it is always `None` and no draw is
    /// consumed.
    pub skew_target: Option<GoodId>,
}

#[derive(Clone, Debug)]
pub struct GeneratedWorld {
    pub world_seed: u64,
    pub scenario: MarketScenario,
    pub profile: WorldProfile,
}

/// Sub-seed for world `world_index`: the master `Rng::new(master_seed)` draws
/// one u64 per world index in order; world *i* uses the `(i+1)`-th draw. This
/// makes the corpus a pure function of `(master_seed, count, family,
/// envelope)` while keeping each world reproducible in isolation.
pub fn sub_seed(master_seed: u64, world_index: u32) -> u64 {
    let mut rng = Rng::new(master_seed);
    let mut seed = 0u64;
    for _ in 0..=world_index {
        seed = rng.next_u64();
    }
    seed
}

/// Generate world `world_index` from `master_seed`. Worlds are never resampled:
/// the returned world is profiled and classified, never discarded.
pub fn generate_world(
    master_seed: u64,
    world_index: u32,
    family: WorldFamily,
    envelope: &WorldEnvelope,
) -> GeneratedWorld {
    generate_world_tuned(
        master_seed,
        world_index,
        family,
        envelope,
        &EmergenceTuning::default(),
    )
}

/// Tuned variant of [`generate_world`]: applies the M19 [`EmergenceTuning`]
/// overrides. With `EmergenceTuning::default()` it is byte-identical to
/// [`generate_world`].
pub fn generate_world_tuned(
    master_seed: u64,
    world_index: u32,
    family: WorldFamily,
    envelope: &WorldEnvelope,
    tuning: &EmergenceTuning,
) -> GeneratedWorld {
    let world_seed = sub_seed(master_seed, world_index);
    generate_world_from_seed_tuned(world_seed, family, envelope, tuning)
}

pub(crate) fn generate_world_from_seed_tuned(
    world_seed: u64,
    family: WorldFamily,
    envelope: &WorldEnvelope,
    tuning: &EmergenceTuning,
) -> GeneratedWorld {
    let mut rng = Rng::new(world_seed);

    let candidate_count = draw_in_range(
        &mut rng,
        u64::from(envelope.candidate_goods.0),
        u64::from(envelope.candidate_goods.1),
    ) as usize;
    let candidates = draw_subset(&mut rng, &GOOD_POOL, candidate_count);

    let population = draw_in_range(
        &mut rng,
        u64::from(envelope.population.0),
        u64::from(envelope.population.1),
    ) as u16;

    let future_want_share_bps = tuning
        .future_want_share_bps
        .unwrap_or(envelope.future_want_share_bps);
    let future_want_share_is_swept = tuning.future_want_share_bps.is_some();
    let skew_bps = tuning
        .demand_breadth_skew_bps
        .unwrap_or(DEFAULT_DEMAND_BREADTH_SKEW_BPS);

    let agent_stock_slots = max_good_id(&GOOD_POOL);
    let (agents, skew_target) = match family {
        WorldFamily::Random => {
            // The skew target is drawn ONLY when skewing is active, from an
            // ISOLATED per-world stream that never touches the main `rng`. So at
            // skew 0 no draw is consumed (M18 golden), and at skew > 0 the main
            // stream stays byte-identical to skew 0 — population, candidates,
            // endowments and base wants are all PAIRED across every skew level
            // for the same seed; only redirected near-want VALUES differ.
            let skew_target = if skew_bps > 0 {
                let mut skew_rng = Rng::new(skew_world_seed(world_seed));
                Some(candidates[draw_index(&mut skew_rng, candidates.len())])
            } else {
                None
            };
            let agents = generate_random_agents(
                &mut rng,
                population,
                &candidates,
                envelope,
                RandomAgentTuning {
                    future_want_share_bps,
                    future_want_share_is_swept,
                    world_seed,
                    skew_target,
                    skew_bps,
                },
            );
            (agents, skew_target)
        }
        WorldFamily::Degenerate => (
            generate_degenerate_agents(
                &mut rng,
                population,
                &candidates,
                envelope,
                agent_stock_slots,
            ),
            None,
        ),
    };

    let surplus = surplus_goods(&agents);
    let class = if surplus >= 2 {
        WorldClass::InEnvelope
    } else {
        WorldClass::OutOfEnvelope
    };
    let profile = WorldProfile {
        population,
        candidate_goods: u8::try_from(candidates.len()).unwrap_or(u8::MAX),
        surplus_goods: surplus,
        class,
        skew_target,
    };

    let scenario = build_scenario(world_seed, population, agents, candidates, tuning);
    GeneratedWorld {
        world_seed,
        scenario,
        profile,
    }
}

fn generate_random_agents(
    rng: &mut Rng,
    population: u16,
    candidates: &[GoodId],
    envelope: &WorldEnvelope,
    tuning: RandomAgentTuning,
) -> Vec<Agent> {
    let mut agents = Vec::with_capacity(usize::from(population));
    let stock_slots = max_good_id(&GOOD_POOL);
    for id in 1..=population {
        let endowed_count = draw_in_range(
            rng,
            u64::from(envelope.endowed_goods_per_agent.0),
            u64::from(envelope.endowed_goods_per_agent.1),
        ) as usize;
        let endowed = draw_subset(rng, candidates, endowed_count);

        let mut stock = Stock::new(stock_slots);
        for good in &endowed {
            let qty = draw_in_range(
                rng,
                u64::from(envelope.endowment_qty.0),
                u64::from(envelope.endowment_qty.1),
            ) as u32;
            stock.add(*good, qty);
        }

        // Wants sit predominantly on non-held goods so gains from trade exist
        // by construction of preferences, not of outcomes.
        let non_held: Vec<GoodId> = candidates
            .iter()
            .copied()
            .filter(|good| !endowed.contains(good))
            .collect();
        let want_pool: &[GoodId] = if non_held.is_empty() {
            candidates
        } else {
            &non_held
        };

        let want_count = draw_in_range(
            rng,
            u64::from(envelope.wants_per_agent.0),
            u64::from(envelope.wants_per_agent.1),
        ) as usize;

        // Demand-breadth skew (H3): the redirect DECISION is taken from an
        // ISOLATED per-agent stream so the main `rng` draws exactly one uniform
        // want per slot, byte-identical to skew 0. Endowments, want counts and
        // future wants therefore stay paired across every skew level for the same
        // seed; only the value of a redirected near-want changes. Across positive
        // skew levels the isolated stream is identical (it is seeded from the
        // world+agent, not the bps), so more bps -> a superset of redirected
        // wants — monotone by construction.
        let mut skew_rng =
            (tuning.skew_bps > 0).then(|| Rng::new(skew_agent_seed(tuning.world_seed, id)));
        let mut scale = Vec::new();
        for _ in 0..want_count {
            let drawn = want_pool[draw_index(rng, want_pool.len())];
            let good = match (&mut skew_rng, tuning.skew_target) {
                (Some(skew_rng), Some(target)) => {
                    // Redirect to the target only when it is genuinely demandable
                    // for this agent (in the want pool of non-held goods).
                    // Redirecting onto an already-held good would make a
                    // self-satisfied want — inert, no demand — so the boost
                    // concentrates REAL demand breadth, the cleanest test of the
                    // saleability-differential hypothesis.
                    let roll = draw_in_range(skew_rng, 0, 9_999) as u16;
                    if roll < tuning.skew_bps && want_pool.contains(&target) {
                        target
                    } else {
                        drawn
                    }
                }
                _ => drawn,
            };
            scale.push(near_want(good));
        }

        // A future_want_share_bps fraction of agents additionally carry a
        // larger Later-horizon want on a randomly drawn good — the demand for
        // a store of value, randomized so the winner varies.
        let roll = draw_in_range(rng, 0, 9_999) as u16;
        if tuning.future_want_share_is_swept {
            add_swept_future_wants(
                rng,
                &mut scale,
                candidates,
                envelope.future_want_share_bps,
                tuning.future_want_share_bps,
                roll,
                tuning.world_seed,
                id,
            );
        } else if roll < tuning.future_want_share_bps {
            let (good, need, horizon) = draw_future_want_details(rng, candidates);
            add_future_wants(&mut scale, good, need, horizon);
        }

        agents.push(build_agent(id, stock, scale, stock_slots));
    }
    agents
}

#[derive(Clone, Copy)]
struct RandomAgentTuning {
    future_want_share_bps: u16,
    future_want_share_is_swept: bool,
    world_seed: u64,
    skew_target: Option<GoodId>,
    skew_bps: u16,
}

#[allow(clippy::too_many_arguments)]
fn add_swept_future_wants(
    rng: &mut Rng,
    scale: &mut Vec<Want>,
    candidates: &[GoodId],
    stream_anchor_bps: u16,
    future_want_share_bps: u16,
    roll: u16,
    world_seed: u64,
    agent_id: u16,
) {
    let add_future = roll < future_want_share_bps;
    if roll < stream_anchor_bps {
        // Burn exactly the draws the pinned envelope default would have consumed.
        // This keeps explicit `--vary future-want-share-bps=<default>` identical
        // to the M18 default while lower values only remove Later wants.
        let (good, need, horizon) = draw_future_want_details(rng, candidates);
        if add_future {
            add_future_wants(scale, good, need, horizon);
        }
    } else if add_future {
        // Above the pinned stream anchor, add patience demand from an isolated
        // per-agent stream so higher future-share values do not shift later
        // agents' endowments or near wants.
        let mut future_rng = Rng::new(future_want_seed(world_seed, agent_id));
        let (good, need, horizon) = draw_future_want_details(&mut future_rng, candidates);
        add_future_wants(scale, good, need, horizon);
    }
}

fn draw_future_want_details(rng: &mut Rng, candidates: &[GoodId]) -> (GoodId, usize, u8) {
    let good = candidates[draw_index(rng, candidates.len())];
    let need = draw_in_range(
        rng,
        u64::from(FUTURE_NEED_QTY.0),
        u64::from(FUTURE_NEED_QTY.1),
    ) as usize;
    let horizon = draw_in_range(
        rng,
        u64::from(FUTURE_HORIZON.0),
        u64::from(FUTURE_HORIZON.1),
    ) as u8;
    (good, need, horizon)
}

fn add_future_wants(scale: &mut Vec<Want>, good: GoodId, need: usize, horizon: u8) {
    for _ in 0..need {
        scale.push(future_want(good, horizon));
    }
}

fn future_want_seed(world_seed: u64, agent_id: u16) -> u64 {
    world_seed ^ 0xA076_1D64_78BD_642F ^ u64::from(agent_id).wrapping_mul(0xE703_7ED1_A0B4_28DB)
}

/// Isolated world-level stream that picks the demand-breadth skew target without
/// drawing from the main generation `rng`, so the skew axis is paired against the
/// skew-0 baseline (main stream untouched).
fn skew_world_seed(world_seed: u64) -> u64 {
    world_seed ^ 0x9E37_79B9_7F4A_7C15
}

/// Isolated per-agent stream for skew redirect rolls. Seeded from world+agent
/// (NOT the bps value) so the roll sequence is identical across skew levels — the
/// guarantee that makes demand breadth monotone in the bps.
fn skew_agent_seed(world_seed: u64, agent_id: u16) -> u64 {
    world_seed ^ 0x2545_F491_4F6C_DD1D ^ u64::from(agent_id).wrapping_mul(0xBF58_476D_1CE4_E5B9)
}

fn generate_degenerate_agents(
    rng: &mut Rng,
    population: u16,
    candidates: &[GoodId],
    envelope: &WorldEnvelope,
    stock_slots: u16,
) -> Vec<Agent> {
    // Every agent receives the identical endowment and identical want scale,
    // and every want is satisfied by the agent's own holdings: zero gains from
    // barter. Mechanically valid worlds in which no good should ever promote.
    let good = candidates[draw_index(rng, candidates.len())];
    let want_count = draw_in_range(
        rng,
        u64::from(envelope.wants_per_agent.0),
        u64::from(envelope.wants_per_agent.1),
    ) as u32;
    let drawn_qty = draw_in_range(
        rng,
        u64::from(envelope.endowment_qty.0),
        u64::from(envelope.endowment_qty.1),
    ) as u32;
    let hold_qty = drawn_qty.max(want_count).max(1);

    let mut agents = Vec::with_capacity(usize::from(population));
    for id in 1..=population {
        let mut stock = Stock::new(stock_slots);
        stock.add(good, hold_qty);
        let mut scale = Vec::new();
        for _ in 0..want_count {
            scale.push(near_want(good));
        }
        agents.push(build_agent(id, stock, scale, stock_slots));
    }
    agents
}

fn near_want(good: GoodId) -> Want {
    Want {
        kind: WantKind::Good(good),
        horizon: Horizon::Next,
        qty: 1,
        satisfied: false,
    }
}

fn future_want(good: GoodId, horizon: u8) -> Want {
    Want {
        kind: WantKind::Good(good),
        horizon: Horizon::Later(horizon),
        qty: 1,
        satisfied: false,
    }
}

fn build_agent(id: u16, stock: Stock, scale: Vec<Want>, max_good_id: u16) -> Agent {
    let belief_slots = usize::from(max_good_id) + 1;
    let expect = vec![PriceBelief::new(Gold(2), Gold(1)); belief_slots];
    Agent {
        id: AgentId(u64::from(id)),
        scale,
        stock,
        gold: Gold::ZERO,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Trader],
        expect,
    }
}

fn build_scenario(
    world_seed: u64,
    population: u16,
    agents: Vec<Agent>,
    candidates: Vec<GoodId>,
    tuning: &EmergenceTuning,
) -> MarketScenario {
    // Pinned floor formulas (recorded in the calibration report); the M19
    // overrides parameterize the constants, with `None` reproducing them
    // bit-for-bit:
    //   min_total_acceptances = max(floor, population)         (floor default 12)
    //   min_acceptor_agents   = max(3, population * share / 10000) (share default 3000)
    let acceptance_floor = tuning.min_total_acceptances_floor.unwrap_or(12);
    let config = MengerianConfig {
        candidate_goods: candidates,
        min_total_acceptances: u32::from(population).max(acceptance_floor),
        promotion_threshold_bps: tuning.promotion_threshold_bps.unwrap_or(4_500),
        lead_margin_bps: tuning.lead_margin_bps.unwrap_or(1_500),
        min_acceptor_agents: min_acceptor_agents(population, tuning.min_acceptor_share_bps),
        min_counterpart_goods: tuning.min_counterpart_goods.unwrap_or(2),
        stability_ticks: tuning.stability_ticks.unwrap_or(2),
        indirect_min_acceptance_share_bps: tuning.indirect_min_share_bps.unwrap_or(3_000),
        // S9 strong-bar gate stays inert in the generated-world envelope (no
        // indirect-volume requirement, indirect acceptance on), so the calibration
        // floors are unchanged.
        min_indirect_acceptances: 0,
        min_indirect_acceptor_agents: 0,
        min_indirect_target_goods: 0,
        allow_indirect_acceptance: true,
        multi_offer_medium: false,
    };

    MarketScenario {
        name: GENERATED_WORLD_NAME,
        scenario: ScenarioName::MengerGoldMoney,
        seed: world_seed,
        periods: tuning.periods.unwrap_or(WORLD_PERIODS),
        agents,
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Emergent(config),
    }
}

/// `max(3, population * share_bps / 10_000)`. `None` uses the pinned `3/10`
/// formula literally so the default config is bit-identical to M18; `Some(3000)`
/// yields the same value but is the explicit-sweep form.
fn min_acceptor_agents(population: u16, share_bps: Option<u16>) -> u16 {
    let scaled = match share_bps {
        None => u32::from(population) * 3 / 10,
        Some(bps) => u32::from(population) * u32::from(bps) / 10_000,
    };
    u16::try_from(scaled.max(3)).unwrap_or(u16::MAX)
}

fn max_good_id(goods: &[GoodId]) -> u16 {
    goods.iter().map(|good| good.0).max().unwrap_or(0)
}

/// Number of goods `g` for which at least one agent holds `g` beyond its own
/// near (Now/Next) `g`-wants AND at least one other agent has a near `g`-want
/// without holding any `g`. Classification is computed on the generated cast;
/// worlds are never resampled to force a class.
pub fn surplus_goods(agents: &[Agent]) -> u8 {
    let mut count = 0u8;
    for &good in &GOOD_POOL {
        let surplus_holder = agents
            .iter()
            .any(|agent| agent.stock.get(good) > near_wants_for(agent, good));
        let needy_non_holder = agents
            .iter()
            .any(|agent| agent.stock.get(good) == 0 && near_wants_for(agent, good) > 0);
        if surplus_holder && needy_non_holder {
            count = count.saturating_add(1);
        }
    }
    count
}

fn near_wants_for(agent: &Agent, good: GoodId) -> u32 {
    let count = agent
        .scale
        .iter()
        .filter(|want| {
            want.kind == WantKind::Good(good)
                && matches!(want.horizon, Horizon::Now | Horizon::Next)
        })
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

fn draw_in_range(rng: &mut Rng, lo: u64, hi: u64) -> u64 {
    if hi <= lo {
        return lo;
    }
    let span = hi - lo + 1;
    lo + rng.next_u64() % span
}

fn draw_index(rng: &mut Rng, len: usize) -> usize {
    if len <= 1 {
        return 0;
    }
    (rng.next_u64() % len as u64) as usize
}

/// Uniform random `k`-subset of `pool` via partial Fisher-Yates, returned in
/// id order. No good is privileged: every good is equally likely to be drawn.
fn draw_subset(rng: &mut Rng, pool: &[GoodId], k: usize) -> Vec<GoodId> {
    let mut items = pool.to_vec();
    let k = k.min(items.len()).max(1);
    for i in 0..k {
        let j = i + draw_index(rng, items.len() - i);
        items.swap(i, j);
    }
    let mut chosen = items[..k].to_vec();
    chosen.sort();
    chosen
}

#[cfg(test)]
mod tests {
    use super::{
        generate_world, sub_seed, surplus_goods, WorldClass, WorldEnvelope, WorldFamily, GOOD_POOL,
        WORLD_PERIODS,
    };
    use crate::good::GoodId;
    use crate::money::MarketMoneyConfig;

    #[test]
    fn generation_is_a_pure_function_of_seed_and_index() {
        let envelope = WorldEnvelope::default();
        let a = generate_world(7, 13, WorldFamily::Random, &envelope);
        let b = generate_world(7, 13, WorldFamily::Random, &envelope);
        assert_eq!(a.world_seed, b.world_seed);
        assert_eq!(a.scenario.agents.len(), b.scenario.agents.len());
        assert_eq!(a.profile, b.profile);
    }

    #[test]
    fn sub_seed_matches_sequential_master_draws() {
        use crate::rng::Rng;
        let mut master = Rng::new(99);
        for index in 0..8 {
            assert_eq!(master.next_u64(), sub_seed(99, index));
        }
    }

    #[test]
    fn generated_world_is_valid_v2_with_scaled_floors() {
        let envelope = WorldEnvelope::default();
        let world = generate_world(18, 0, WorldFamily::Random, &envelope);
        let population = world.profile.population;
        assert!((6..=40).contains(&population));
        let MarketMoneyConfig::Emergent(config) = &world.scenario.money else {
            panic!("generated worlds must use emergent money");
        };
        assert_eq!(config.min_total_acceptances, u32::from(population).max(12));
        assert_eq!(
            config.min_acceptor_agents,
            u16::try_from((u32::from(population) * 3 / 10).max(3)).unwrap()
        );
        assert_eq!(config.promotion_threshold_bps, 4_500);
        assert_eq!(world.scenario.periods, WORLD_PERIODS);
        assert!(world.scenario.agents.iter().all(|agent| agent.gold.0 == 0));
    }

    #[test]
    fn degenerate_world_has_no_surplus_goods() {
        let envelope = WorldEnvelope::default();
        for index in 0..25 {
            let world = generate_world(5, index, WorldFamily::Degenerate, &envelope);
            assert_eq!(surplus_goods(&world.scenario.agents), 0);
            assert_eq!(world.profile.class, WorldClass::OutOfEnvelope);
        }
    }

    #[test]
    fn candidate_goods_are_drawn_from_the_pool() {
        let envelope = WorldEnvelope::default();
        let world = generate_world(18, 3, WorldFamily::Random, &envelope);
        let MarketMoneyConfig::Emergent(config) = &world.scenario.money else {
            panic!("emergent money expected");
        };
        for good in &config.candidate_goods {
            assert!(GOOD_POOL.contains(good));
        }
        assert!(!config.candidate_goods.contains(&GoodId(3)), "NET excluded");
    }
}
