//! The `Camp` driver — the minimal pre-`sim` G1 stand-in.
//!
//! `Camp` is deliberately *lean, not a framework*: a struct with `step()` that
//! drives a real `econ::Society` of generated colonists. G2's `sim` crate will
//! absorb and replace it. Each economic tick mirrors game-spec §4.3 (minus the
//! not-yet-existing fast loop):
//!
//! 1. update each living colonist's `NeedState` from last tick's realized
//!    consumption + whether it worked or rested;
//! 2. apply death-by-starvation as **real removal** (G4a): mark dead, settle the
//!    estate (gold + stock) to the camp commons, and free the arena slot — see
//!    [`Society::remove_agent`] and `docs/engine-divergence.md`;
//! 3. [`regenerate_scale`] for every living colonist, overwriting `Agent.scale`;
//! 4. add the per-tick resource endowment flow, then `Society::step()` — the
//!    unchanged econ market/labor clearing;
//! 5. (consumption/labor are read back at the top of the next tick).
//!
//! All generation randomness goes through `econ::rng::Rng`; the tick loop draws
//! nothing and is deterministic. The economy is endowment-asymmetric so trade is
//! emergent, not scripted: foragers harvest FOOD (and must buy/cut their WOOD),
//! woodcutters harvest WOOD (and — having no way to make FOOD — must buy it).

use econ::agent::{Agent, AgentId, Role};
use econ::expect::PriceBelief;
use econ::good::{Gold, GoodId, Stock, FOOD, GOLD, NET, WOOD};
use econ::money::{DesignatedMoney, MarketMoneyConfig};
use econ::project::{Recipe, RecipeId};
use econ::rng::Rng;
use econ::scenario::{MarketScenario, ScenarioName};
use econ::society::Society;

use crate::culture::CultureParams;
use crate::need::{NeedDynamics, NeedIntake, NeedState};
use crate::scale::{regenerate_scale, KnownGoods};

/// A year of ticks for the smoke test's "5 years" horizon (game-spec §4.3 uses
/// a slow loop; 60 ticks/year is a placeholder, not a balance figure).
pub const TICKS_PER_YEAR: u64 = 60;

/// A colonist's economic role in the camp. The asymmetry is what makes trade
/// emerge: each vocation harvests one staple and must obtain the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vocation {
    /// Harvests FOOD each tick; cuts or buys its WOOD.
    Forager,
    /// Harvests WOOD each tick; must buy its FOOD (cannot produce it).
    Woodcutter,
}

/// The camp's per-tick resource endowment flow and labor endowment — the knobs
/// that make a camp viable and that a harvest shock varies. Mechanism knobs, not
/// balance targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CampEnv {
    /// FOOD units a forager harvests each tick.
    pub food_flow: u32,
    /// WOOD units a woodcutter harvests each tick.
    pub wood_flow: u32,
    /// Labor capacity granted to every colonist (used for CutWood).
    pub labor_capacity: u32,
    /// Per-tick money a woodcutter earns from its off-camp wood trade. This funds
    /// the food buyers so the food market is effectively one-directional (no
    /// gold-poverty trap), which is what lets a lumpy saving reservation discover
    /// a food price that responds to scarcity. Pre-spatial G1 has no map, so
    /// off-camp trade is modeled as a flow; set to 0 for a closed camp.
    pub gold_income: u64,
    /// Starting money balance.
    pub starting_gold: u64,
    /// Starting FOOD buffer.
    pub starting_food: u32,
    /// Starting WOOD buffer.
    pub starting_wood: u32,
    /// Which side of the food market is activated first each tick, and so rests
    /// its orders in the book (a trade prints at the resting order's limit). With
    /// the food *sellers* leading (`false`), their floor reservation sets a low,
    /// stable food price and the camp runs indefinitely — the non-collapse
    /// configuration. With the food *buyers* leading (`true`), the price prints at
    /// the hungry bidder's bid, which climbs when scarcity leaves it holding
    /// unspent gold — the harvest-shock configuration.
    pub food_buyers_lead: bool,
    /// The lower bound of the colonists' time-preference draw (bps). Patience (a
    /// low value) raises the saving target, so colonists keep offering surplus and
    /// the market discovers prices; a present-biased camp (a high value) saves
    /// little, trades only to replace what it spends, and runs in balance.
    pub time_preference_base: u16,
}

impl CampEnv {
    /// A comfortably viable camp: harvests exceed aggregate need, with a gold and
    /// goods buffer to bootstrap trade.
    pub const fn viable() -> Self {
        Self {
            food_flow: 3,
            wood_flow: 3,
            labor_capacity: 3,
            // Closed camp: the food sellers lead, bleed gold buying their wood,
            // and keep selling at the floor reservation, so food stays cheap and
            // the camp runs indefinitely.
            gold_income: 0,
            // A low working balance against the unmet saving target keeps the
            // money demand live so surplus is offered; the staple buffer lets a
            // colonist eat while the first trades are discovered.
            starting_gold: 3,
            starting_food: 6,
            starting_wood: 6,
            food_buyers_lead: false,
            // Present-biased: small saving targets, so a colonist fills its
            // target and trades only to replace what it spends — gold circulates
            // in balance and the camp runs indefinitely.
            time_preference_base: 9_400,
        }
    }

    /// A harvest-shock configuration: patient colonists (large, unmet saving
    /// targets) keep offering their surplus, and the food buyers lead the book —
    /// so the realized FOOD price prints at the hungry bidder's bid and rises when
    /// a cut to the FOOD harvest leaves buyers holding gold they cannot spend. The
    /// instrument for the price-response acceptance test.
    pub const fn shockable() -> Self {
        Self {
            food_buyers_lead: true,
            gold_income: 2,
            food_flow: 4,
            time_preference_base: 500,
            ..Self::viable()
        }
    }

    /// A camp cut off from food: no FOOD harvest and no labor, so nothing can
    /// produce or replenish food and every colonist eventually starves. Used to
    /// exercise the real-death path (G4a).
    pub const fn starved() -> Self {
        Self {
            food_flow: 0,
            wood_flow: 3,
            labor_capacity: 0,
            // No income: gold is closed and conserved here, so a death settles
            // holdings to the commons without disturbing the conservation total.
            gold_income: 0,
            starting_gold: 8,
            starting_food: 2,
            starting_wood: 6,
            food_buyers_lead: false,
            time_preference_base: 9_400,
        }
    }
}

impl Default for CampEnv {
    fn default() -> Self {
        Self::viable()
    }
}

struct Colonist {
    id: AgentId,
    vocation: Vocation,
    need: NeedState,
    culture: CultureParams,
    critical_streak: u16,
    /// Mirrors real removal (see [`Society::remove_agent`]'s caller contract): set
    /// `false` the tick a colonist starves and checked in every `step` phase so a
    /// dead colonist is never re-scaled, re-endowed, or read back. After removal its
    /// id resolves to `None` in the arena; this driver-owned flag keeps the dead out
    /// of the per-tick phases that iterate the generated roster.
    alive: bool,
}

/// A camp of generated colonists driven over a real econ market.
pub struct Camp {
    society: Society,
    dynamics: NeedDynamics,
    known: KnownGoods,
    env: CampEnv,
    colonists: Vec<Colonist>,
    /// The camp **commons** (G4a real death): the conserved sink that holds a dead
    /// colonist's settled estate. When a colonist starves, [`Society::remove_agent`]
    /// frees its arena slot and hands back its gold + stock, which accrue here —
    /// nothing created or destroyed. The commons joins [`Camp::total_gold`] so gold
    /// conservation holds across a death even though the freed colonist's gold has
    /// left the society. Empty until the first death. G4b routes estates to heirs.
    commons_gold: Gold,
    /// The commons' physical-good holdings — a dead colonist's settled stock, kept
    /// so the estate transfer conserves every good (G4a). Read via
    /// [`Camp::commons_stock_of`].
    commons_stock: Stock,
}

impl Camp {
    /// Generate a camp of `population` colonists from `seed`. All randomness is
    /// drawn here (cultural parameters per colonist); the tick loop draws none.
    pub fn generate(seed: u64, population: u16, env: &CampEnv) -> Self {
        let dynamics = NeedDynamics::lab_default();
        let known = KnownGoods::lab_default();
        let mut rng = Rng::new(seed);

        let mut colonists = Vec::with_capacity(usize::from(population));
        let mut agents = Vec::with_capacity(usize::from(population));
        for index in 0..u32::from(population) {
            // The leading side rests its orders (a trade prints at the resting
            // limit). `food_buyers_lead` puts woodcutters (food buyers) on the
            // lower ids so their bids rest and set the realized food price.
            let buyer = index % 2 == 0;
            let vocation = if buyer == env.food_buyers_lead {
                Vocation::Woodcutter
            } else {
                Vocation::Forager
            };
            let culture = draw_culture(&mut rng, env.time_preference_base);
            let need = NeedState::rested();
            let id = AgentId(u64::from(index));
            agents.push(build_agent(id, &need, &culture, &known, env));
            colonists.push(Colonist {
                id,
                vocation,
                need,
                culture,
                critical_streak: 0,
                alive: true,
            });
        }

        let scenario = MarketScenario {
            name: "camp",
            // A SoundGold M1 (designated-gold spot) scenario: the natural seam
            // (base fact 2). Banks/issuers/projects are empty for this kind.
            scenario: ScenarioName::MarketBarterishGold,
            seed,
            periods: 0,
            agents,
            recipes: vec![cut_wood_recipe()],
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        };
        let mut society = Society::from_scenario(scenario);
        society.enable_consumption_log();

        Self {
            society,
            dynamics,
            known,
            env: *env,
            colonists,
            commons_gold: Gold::ZERO,
            commons_stock: Stock::new(NET.0),
        }
    }

    /// One economic tick (the five-step order documented on the module).
    pub fn step(&mut self) {
        // 1. update needs from the tick that just completed (zero on tick 0).
        let mut intakes = vec![NeedIntake::default(); self.colonists.len()];
        for &(agent, good, qty) in self.society.consumption_log_last_tick() {
            let Some(index) = colonist_slot_for_id(&self.colonists, agent) else {
                continue;
            };
            if !self.colonists[index].alive {
                continue;
            }
            if good == self.known.hunger {
                intakes[index].food_consumed = intakes[index].food_consumed.saturating_add(qty);
            } else if good == self.known.warmth {
                intakes[index].wood_consumed = intakes[index].wood_consumed.saturating_add(qty);
            }
        }
        for &(agent, labor) in self.society.labor_used_last_tick() {
            let Some(index) = colonist_slot_for_id(&self.colonists, agent) else {
                continue;
            };
            if self.colonists[index].alive {
                intakes[index].labor_used = intakes[index].labor_used.saturating_add(labor);
            }
        }

        for (index, colonist) in self.colonists.iter_mut().enumerate() {
            if !colonist.alive {
                continue;
            }
            colonist.need.advance(&self.dynamics, intakes[index]);
        }

        // 2. apply death-by-starvation as real removal (G4a): mark dead, then settle
        // the estate to the commons and free the arena slot. Deaths are collected
        // first (a mutable streak pass) and settled after, in generation order.
        let mut dying = Vec::new();
        for colonist in &mut self.colonists {
            if !colonist.alive {
                continue;
            }
            if colonist.need.is_critical(&self.dynamics) {
                colonist.critical_streak = colonist.critical_streak.saturating_add(1);
            } else {
                colonist.critical_streak = 0;
            }
            if colonist.critical_streak >= self.dynamics.death_window {
                colonist.alive = false;
                dying.push(colonist.id);
            }
        }
        for id in dying {
            self.settle_estate_to_commons(id);
        }

        // 3. regenerate the value scale from need state for every living colonist.
        let mut rewritten = Vec::with_capacity(self.living_count());
        for colonist in &self.colonists {
            if !colonist.alive {
                continue;
            }
            let scale = regenerate_scale(&colonist.need, &colonist.culture, &self.known);
            self.society
                .agents
                .get_mut(colonist.id)
                .expect("living colonist resolves in the arena")
                .scale = scale;
            rewritten.push(colonist.id);
        }
        self.society
            .cancel_changed_live_quotes_for_agents(&rewritten);

        // 4. deliver the per-tick endowment flow (the harvest), then clear markets.
        for colonist in &self.colonists {
            if !colonist.alive {
                continue;
            }
            let agent = self
                .society
                .agents
                .get_mut(colonist.id)
                .expect("living colonist resolves in the arena");
            match colonist.vocation {
                Vocation::Forager => {
                    if self.env.food_flow > 0 {
                        agent.stock.add(self.known.hunger, self.env.food_flow);
                    }
                }
                Vocation::Woodcutter => {
                    if self.env.wood_flow > 0 {
                        agent.stock.add(self.known.warmth, self.env.wood_flow);
                    }
                    if self.env.gold_income > 0 {
                        agent.gold = agent.gold.saturating_add(Gold(self.env.gold_income));
                    }
                }
            }
        }
        self.society.step();
    }

    /// Settle a starved colonist's estate to the commons and remove it (G4a real
    /// death). [`Society::remove_agent`] settles the estate (gold + stock), cancels
    /// the colonist's market presence, frees its arena slot, and reconciles the
    /// society's caches — handing back the [`econ::society::Estate`], which accrues
    /// to the commons. A conserved transfer: the gold and goods leave the society
    /// for the commons, nothing created or destroyed (heirs are G4b). The camp has
    /// no spatial layer, so there is no world-carried escrow to drain.
    fn settle_estate_to_commons(&mut self, id: AgentId) {
        let Some(estate) = self.society.remove_agent(id) else {
            return;
        };
        self.commons_gold = self.commons_gold.saturating_add(estate.gold);
        for good in estate.stock.positive_goods() {
            self.commons_stock.add(good, estate.stock.get(good));
        }
    }

    /// Run `ticks` economic ticks.
    pub fn run(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.step();
        }
    }

    /// Cut the FOOD harvest to `flow` (a harvest shock). Food cannot be produced
    /// in the camp, so a lower flow makes food scarcer and the realized FOOD
    /// price respond.
    pub fn set_food_flow(&mut self, flow: u32) {
        self.env.food_flow = flow;
    }

    /// Living colonist count (the dead are removed from the arena).
    pub fn living_count(&self) -> usize {
        self.colonists.iter().filter(|c| c.alive).count()
    }

    /// The colonists generated for this camp (living and dead).
    pub fn population(&self) -> usize {
        self.colonists.len()
    }

    /// Whether the colonist at generation `index` is still alive.
    pub fn is_alive(&self, index: usize) -> bool {
        self.colonists.get(index).is_some_and(|c| c.alive)
    }

    /// The current need state of the colonist at generation `index`.
    pub fn need_of(&self, index: usize) -> Option<NeedState> {
        self.colonists.get(index).map(|c| c.need)
    }

    /// The most recent realized FOOD price (last FOOD trade), or `None` if no
    /// FOOD trade has cleared yet.
    pub fn realized_food_price(&self) -> Option<Gold> {
        self.society.realized_price(self.known.hunger)
    }

    /// Total money balance across the camp — live econ gold plus the **commons**
    /// (a dead colonist's settled gold; G4a). The commons term is zero until the
    /// first death; including it keeps the total constant across a death (the dead
    /// colonist's gold leaves the society for the commons), so a constant across a
    /// death proves the estate transfer is conserved.
    pub fn total_gold(&self) -> Gold {
        self.society.total_gold().saturating_add(self.commons_gold)
    }

    /// The gold pooled in the camp commons — dead colonists' settled gold (G4a).
    pub fn commons_gold(&self) -> Gold {
        self.commons_gold
    }

    /// Units of `good` held in the camp commons — dead colonists' settled stock
    /// (G4a). Zero until the first death.
    pub fn commons_stock_of(&self, good: GoodId) -> u32 {
        self.commons_stock.get(good)
    }

    /// Read-only access to the underlying society (for conservation/holdings
    /// assertions in tests).
    pub fn society(&self) -> &Society {
        &self.society
    }

    /// The highest depletion any living colonist carries in any need — the
    /// boundedness probe for the non-collapse smoke test.
    pub fn max_living_need(&self) -> u16 {
        self.colonists
            .iter()
            .filter(|c| c.alive)
            .map(|c| c.need.hunger.max(c.need.warmth).max(c.need.rest))
            .max()
            .unwrap_or(0)
    }

    /// A small deterministic digest of camp state (living count, total gold,
    /// summed needs) for cross-run determinism checks.
    pub fn digest(&self) -> (usize, u64, u64) {
        let needs = self.colonists.iter().fold(0u64, |total, c| {
            total
                .saturating_add(u64::from(c.need.hunger))
                .saturating_add(u64::from(c.need.warmth))
                .saturating_add(u64::from(c.need.rest))
        });
        (self.living_count(), self.total_gold().0, needs)
    }
}

fn draw_culture(rng: &mut Rng, time_preference_base: u16) -> CultureParams {
    // Colonists' cultures differ within a band above the env's base; drawn from
    // the world-generation Rng only (the tick loop draws nothing). Time
    // preference governs the saving target, which shapes whether the camp runs in
    // balance or its market discovers prices.
    let span = u16::try_from(rng.next_u64() % 500).unwrap_or(0);
    let time_preference_bps = time_preference_base.saturating_add(span);
    let leisure_weight_bps = 2_000 + u16::try_from(rng.next_u64() % 3_001).unwrap_or(0);
    CultureParams::new(time_preference_bps, leisure_weight_bps)
}

fn colonist_slot_for_id(colonists: &[Colonist], id: AgentId) -> Option<usize> {
    colonists.iter().position(|colonist| colonist.id == id)
}

fn build_agent(
    id: AgentId,
    need: &NeedState,
    culture: &CultureParams,
    known: &KnownGoods,
    env: &CampEnv,
) -> Agent {
    // Every colonist starts with a buffer of both staples so it can eat and keep
    // warm while the first trades are discovered. Vocation differentiates the
    // per-tick endowment flow (applied in `step`), not the starting stock.
    let mut stock = Stock::new(NET.0);
    stock.add(FOOD, env.starting_food);
    stock.add(WOOD, env.starting_wood);
    Agent {
        id,
        scale: regenerate_scale(need, culture, known),
        stock,
        gold: Gold(env.starting_gold),
        labor_capacity: env.labor_capacity,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: belief_vec(),
    }
}

fn belief_vec() -> Vec<PriceBelief> {
    // Neutral starting beliefs; the savings-threshold reservation, not the
    // belief, anchors the discovered price.
    let slots = usize::from(NET.0) + 1;
    vec![PriceBelief::new(Gold(2), Gold(1)); slots]
}

fn cut_wood_recipe() -> Recipe {
    Recipe {
        id: RecipeId::CutWood,
        name: "CutWood",
        labor: 1,
        input_good: None,
        required_tool: None,
        output_good: WOOD,
        output_qty: 1,
        enabled: true,
    }
}
