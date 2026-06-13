//! The `Settlement` orchestrator — the G2b two-rate loop and the world→econ
//! delivery-escrow seam for one settlement.
//!
//! A `Settlement` owns a [`World`], a per-colonist [`NeedState`] /
//! [`CultureParams`], and an [`econ::Society`], and advances them with
//! [`Settlement::econ_tick`]. Each econ tick runs the documented two-rate order
//! (game-spec §4.3):
//!
//! 1. **FAST** — run the `world` for [`FAST_TICKS_PER_ECON_TICK`] ticks
//!    (movement, harvest node→carry, deposit carry→exchange stockpile). No money
//!    moves. Dead colonists are frozen (idled), so they deliver nothing — their
//!    carried goods stay escrowed, never destroyed.
//! 2. **TRANSFER** — for each delivered exchange unit awaiting credit, *credit
//!    the depositing colonist's econ stock* and then *withdraw it from the world*
//!    (net-zero, conserved, recorded). A unit that cannot be credited stays
//!    world-owned in the exchange stockpile, never destroyed: a live depositor at
//!    its stock ceiling is retried on later ticks, while a tombstoned depositor is
//!    rejected for good and its units freeze there permanently (still conserved).
//! 3. **NEEDS** — advance each living colonist's [`NeedState`] from the last econ
//!    tick's realized consumption + labor; tombstone starvation deaths (the G1
//!    mechanism), idling the dead in the world so their carry freezes.
//! 4. **SCALES** — [`regenerate_scale`] for every living colonist, then cancel
//!    now-stale resting quotes (as G1 does).
//! 5. **MARKET** — [`Society::step`], the unchanged econ clearing. Money moves
//!    here only.
//! 6. **READ-BACK** — consumption is read at the top of the next tick's NEEDS.
//! 7. **ASSIGN** — idle gatherers get their next task (harvest → exchange);
//!    handled inline in the fast loop so a gatherer is never idle for a tick.
//!
//! ## The division of labor
//!
//! - **gatherers** harvest FOOD from a node and haul it to the exchange; the
//!   transfer credits the hauled FOOD to their econ stock; they sell it and buy
//!   the warmth good (WOOD) to keep their gold circulating.
//! - **consumers** sit at the exchange; they sell their WOOD endowment and buy
//!   FOOD, consuming it (their need/scale loop drives their bids).
//!
//! Consumers carry the **lower** ids, so their FOOD bids rest in the book first
//! and a gatherer's crossing ask prints at the resting bid — making the realized
//! FOOD price track the buyers' willingness to pay, which climbs when distance
//! starves the supply. That is the distance→price mechanism, sign only.
//!
//! ## Conservation
//!
//! For every physical good the whole-system total — node + carry + exchange
//! stockpile (all `world`) + econ stock — changes per econ tick by **exactly**
//! `+regen − consumed`. Harvest (node→carry), deposit (carry→stockpile), and the
//! transfer (stockpile→econ) are all relocations: net-zero. Node regen is the
//! only source, consumption the only sink. [`Settlement::econ_tick`] checks this
//! every tick and returns it in the [`EconTickReport`]; FOOD is the spatial good
//! (source = its node's regen), WOOD a closed provisioning good (source none,
//! sink consumption) that recirculates gold and keeps the market liquid.
//!
//! Money (GOLD) is a **closed** balance: no settlement path mints or burns it,
//! so the fast loop never moves money and [`Society::step`] only redistributes a
//! conserved total between colonists (the §4.3 rule; the report's gold
//! checkpoints are the proof).

use std::collections::BTreeMap;

use econ::agent::{Agent, AgentId, Role, WantKind};
use econ::expect::PriceBelief;
use econ::good::{Gold, GoodId, Horizon, Stock, FOOD, GOLD, NET, WOOD};
use econ::money::{DesignatedMoney, MarketMoneyConfig};
use econ::rng::Rng;
use econ::scenario::{MarketScenario, ScenarioName};
use econ::society::Society;

use life::{regenerate_scale, CultureParams, KnownGoods, NeedDynamics, NeedIntake, NeedState};

use world::{AgentStatus, Grid, NodeId, Pos, ResourceNode, Stockpile, StockpileId, Task, World};

/// Fast `world` ticks per economic tick — the two-rate ratio (game-spec §4.1).
/// A gatherer's round trip to a node costs `2 × distance` fast ticks, so a node
/// far from the exchange completes fewer trips inside this fixed budget and
/// delivers fewer units per econ tick. Holding this fixed while varying distance
/// is exactly the distance→price experiment.
pub const FAST_TICKS_PER_ECON_TICK: u64 = 24;

/// Econ ticks per settlement "year" — the horizon unit the smoke test counts in.
/// A placeholder cadence, not a balance figure.
pub const ECON_TICKS_PER_YEAR: u64 = 12;

/// A colonist's role in the settlement's minimal division of labor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vocation {
    /// Harvests FOOD from its node and hauls it to the exchange; sells FOOD,
    /// buys WOOD.
    Gatherer,
    /// Sits at the exchange; sells its WOOD endowment, buys and eats FOOD.
    Consumer,
}

/// A resource node to place: a good, a tile, and its stock/regen/cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeSpec {
    pub good: GoodId,
    pub pos: Pos,
    pub stock: u32,
    pub regen: u32,
    pub cap: u32,
}

/// The settlement recipe: geometry (grid, exchange, FOOD nodes), the
/// gatherer/consumer rosters, and the economic knobs. Mechanism knobs, not
/// balance targets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettlementConfig {
    pub width: u16,
    pub height: u16,
    /// Where the exchange stockpile sits; every colonist starts here.
    pub exchange: Pos,
    /// Exchange stockpile capacity — generous, since normal transfers drain it
    /// each econ tick (escrow comes from travel time, not overflow).
    pub exchange_cap: u32,
    /// FOOD nodes the gatherers harvest (assigned round-robin by gatherer index).
    pub nodes: Vec<NodeSpec>,
    pub gatherers: u16,
    pub consumers: u16,
    pub carry_cap: u32,
    pub move_speed: u16,
    pub starting_gold_gatherer: u64,
    pub starting_gold_consumer: u64,
    /// FOOD a gatherer starts with (a buffer to eat while the first hauls land).
    pub gatherer_food_buffer: u32,
    /// WOOD a gatherer starts with (a small warmth buffer).
    pub gatherer_wood_buffer: u32,
    /// FOOD a consumer starts with (a buffer to eat while the market warms up).
    pub consumer_food_buffer: u32,
    /// WOOD a consumer is endowed with — the closed provisioning battery it
    /// sells for gold and burns for warmth.
    pub consumer_wood_endowment: u32,
    /// Gatherers are present-biased (high bps) so they keep selling their haul
    /// to refill a small gold target, circulating gold by buying WOOD.
    pub gatherer_time_preference_base_bps: u16,
    /// Consumers are patient (low bps) so unspent gold accumulates when FOOD is
    /// scarce, lifting their bids — the price's scarcity response.
    pub consumer_time_preference_base_bps: u16,
    pub leisure_weight_base_bps: u16,
    pub dynamics: NeedDynamics,
}

impl SettlementConfig {
    /// A viable single-FOOD-node settlement: gatherers haul FOOD from a node a
    /// short distance east of the exchange; consumers sit at the exchange and
    /// trade their WOOD battery for FOOD.
    /// Patient colonists keep offering their surplus so the market clears and the
    /// settlement runs without collapse. Move the node with
    /// [`Self::with_food_node_distance`] for the distance experiment.
    pub fn viable() -> Self {
        let exchange = Pos::new(0, 0);
        Self {
            width: 64,
            height: 1,
            exchange,
            exchange_cap: 1_000_000,
            nodes: vec![NodeSpec {
                good: FOOD,
                pos: Pos::new(4, 0),
                stock: 4_000,
                regen: 16,
                cap: 4_000,
            }],
            // Supply-rich (more sellers than buyers) so the qty-1 book keeps the
            // buyers reliably fed, and the gold loop is **closed**, so it
            // circulates instead of pooling in the sellers — both make the
            // settlement sustain its colonists indefinitely over the smoke-test
            // horizon.
            gatherers: 8,
            consumers: 4,
            carry_cap: 2,
            move_speed: 1,
            starting_gold_gatherer: 4,
            starting_gold_consumer: 12,
            gatherer_food_buffer: 8,
            gatherer_wood_buffer: 6,
            consumer_food_buffer: 8,
            consumer_wood_endowment: 600,
            // Patient on both sides (low bps): sellers keep offering their haul
            // instead of sating a tiny gold target and hoarding, so food keeps
            // reaching the buyers and the settlement sustains.
            gatherer_time_preference_base_bps: 500,
            consumer_time_preference_base_bps: 500,
            leisure_weight_base_bps: 3_000,
            dynamics: NeedDynamics::lab_default(),
        }
    }

    /// A distance→price probe. Two changes from [`Self::viable`] isolate the
    /// supply→price channel for the near/far sign test:
    ///
    /// - enough gatherers for a nearby node to keep supply loose, plus a larger
    ///   initial closed gold balance so scarce far-node supply can lift bids
    ///   without any loop-time money mutation;
    /// - **hunger-resilient** consumers (hunger never reaches the critical
    ///   ceiling) so the market stays demand-heavy and bids up under scarcity
    ///   instead of dying off — the price reflects supply, not a death cascade.
    ///
    /// Both runs use this identical config; only the node distance differs. Sign
    /// only — it pins no magnitude.
    pub fn price_probe() -> Self {
        let mut config = Self::viable();
        config.starting_gold_consumer = 120;
        config.dynamics.hunger_critical = config.dynamics.need_max + 1;
        config
    }

    /// A single gatherer on a very long haul (no market): the node is so far that
    /// the round trip spans many econ ticks, so the gatherer's harvested FOOD
    /// stays locked in **carry** (the world, undeposited) while its small econ
    /// FOOD buffer runs out and it **starves mid-haul**. The escrow-on-death
    /// scenario for test 3: its carried goods must freeze (conserved, not
    /// destroyed, not transferred) when it is tombstoned.
    pub fn starved_hauler() -> Self {
        let mut config = Self::viable();
        config.width = 320;
        config.gatherers = 1;
        config.consumers = 0;
        config.carry_cap = 4;
        config.gatherer_food_buffer = 2;
        config.gatherer_wood_buffer = 0;
        config.nodes = vec![NodeSpec {
            good: FOOD,
            pos: Pos::new(140, 0),
            stock: 4_000,
            regen: 16,
            cap: 4_000,
        }];
        config
    }

    /// Place the (single) FOOD node `distance` tiles east of the exchange,
    /// holding everything else fixed — the only knob the distance→price test
    /// varies. Panics if there is not exactly one node (the experiment's shape).
    pub fn with_food_node_distance(mut self, distance: u16) -> Self {
        assert_eq!(
            self.nodes.len(),
            1,
            "the distance experiment expects exactly one node"
        );
        let y = self.exchange.y;
        let x = self.exchange.x.saturating_add(distance);
        assert!(x < self.width, "node distance puts the node off the grid");
        self.nodes[0].pos = Pos::new(x, y);
        self
    }
}

/// The per-econ-tick conservation + flow receipt. The conservation invariant the
/// G2b DoD pins, for every tracked good:
/// `whole_system_after == whole_system_before + regen − consumed` — the transfer
/// is net-zero and so never appears. The gold checkpoints prove no money moved
/// in the fast loop.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EconTickReport {
    pub econ_tick: u64,
    pub fast_ticks: u64,
    /// Goods created by node regen during the fast loop (the only source).
    pub regen: BTreeMap<GoodId, u64>,
    /// Goods relocated world→econ by the transfer (net-zero for the whole system).
    pub transferred: BTreeMap<GoodId, u64>,
    /// Goods consumed in [`Society::step`] (the only sink).
    pub consumed: BTreeMap<GoodId, u64>,
    /// Whole-system total per good at the start of the econ tick.
    pub whole_system_before: BTreeMap<GoodId, u64>,
    /// Whole-system total per good at the end of the econ tick.
    pub whole_system_after: BTreeMap<GoodId, u64>,
    /// Total money before the fast loop.
    pub total_gold_before_fast: u64,
    /// Total money after the fast loop (must equal `before_fast` — no money in
    /// the fast loop).
    pub total_gold_after_fast: u64,
    /// Total money after [`Society::step`] (a closed balance is conserved).
    pub total_gold_after_step: u64,
    pub deaths: u32,
}

impl EconTickReport {
    pub fn regen_of(&self, good: GoodId) -> u64 {
        self.regen.get(&good).copied().unwrap_or(0)
    }
    pub fn transferred_of(&self, good: GoodId) -> u64 {
        self.transferred.get(&good).copied().unwrap_or(0)
    }
    pub fn consumed_of(&self, good: GoodId) -> u64 {
        self.consumed.get(&good).copied().unwrap_or(0)
    }
    pub fn whole_system_before_of(&self, good: GoodId) -> u64 {
        self.whole_system_before.get(&good).copied().unwrap_or(0)
    }
    pub fn whole_system_after_of(&self, good: GoodId) -> u64 {
        self.whole_system_after.get(&good).copied().unwrap_or(0)
    }

    /// Whether the whole-system ledger balances for every tracked good:
    /// `after == before + regen − consumed`, with the transfer net-zero. This is
    /// the conservation DoD; [`Settlement::econ_tick`] also `debug_assert`s it.
    pub fn conserves(&self) -> bool {
        self.whole_system_before.keys().all(|good| {
            let before = self.whole_system_before_of(*good) as i128;
            let after = self.whole_system_after_of(*good) as i128;
            let regen = self.regen_of(*good) as i128;
            let consumed = self.consumed_of(*good) as i128;
            after == before + regen - consumed
        })
    }
}

struct Colonist {
    id: AgentId,
    vocation: Vocation,
    /// The node a gatherer harvests (round-robin over the config's nodes).
    node: Option<NodeId>,
    need: NeedState,
    culture: CultureParams,
    critical_streak: u16,
    /// Mirrors the engine tombstone (see [`Society::tombstone`]'s caller
    /// contract): set `false` the tick a colonist is tombstoned, checked in every
    /// phase so a dead colonist is never re-scaled, re-credited, re-tasked, or
    /// read back. A dead gatherer is idled in the world so its carry freezes.
    alive: bool,
}

/// A settlement of generated colonists driven over a real `world` + `econ`.
pub struct Settlement {
    world: World,
    society: Society,
    colonists: Vec<Colonist>,
    dynamics: NeedDynamics,
    known: KnownGoods,
    exchange: StockpileId,
    carry_cap: u32,
    /// Physical goods tracked for whole-system conservation (node goods ∪ goods
    /// any colonist starts with), `GoodId`-ordered. GOLD (money) is excluded.
    goods: Vec<GoodId>,
    /// Attribution for exchange-stockpile units that were delivered by a
    /// gatherer but have not yet crossed into econ stock. This is not a goods
    /// ledger: the units are counted only in the world stockpile until transfer
    /// succeeds. The map exists solely to retry a clipped credit against the
    /// original depositor once headroom opens.
    pending_deposits: BTreeMap<(AgentId, GoodId), u32>,
    econ_tick: u64,
    last_report: EconTickReport,
}

impl Settlement {
    /// Generate a settlement from `seed` and a [`SettlementConfig`]. All
    /// randomness (per-colonist culture) is drawn here; neither loop draws any.
    /// Deterministic: same `(seed, config)` → byte-identical settlement.
    pub fn generate(seed: u64, config: &SettlementConfig) -> Self {
        assert!(
            config.gatherers == 0 || !config.nodes.is_empty(),
            "a config with gatherers must define at least one resource node to harvest"
        );
        // Money (GOLD) is not a physical good: it never enters `self.goods`, so it
        // is excluded from deposit attribution, the transfer, and the conservation
        // report. A GOLD node would be harvested and deposited by the fast loop yet
        // never transferred or tracked — a silent world-side money leak. Reject it
        // at the seam rather than let the §4.3 "no money in the fast loop" rule and
        // whole-system conservation go blind to it.
        assert!(
            config.nodes.iter().all(|spec| spec.good != GOLD),
            "a resource node cannot harvest the money good (GOLD); money is not a \
             physical good and never crosses the world→econ transfer seam"
        );
        let dynamics = config.dynamics;
        let known = KnownGoods::lab_default();
        let mut rng = Rng::new(seed);

        // ---- world: grid, exchange stockpile, FOOD nodes ----
        let grid = Grid::new(config.width, config.height);
        let mut world = World::new(grid);
        let exchange = world
            .add_stockpile(Stockpile::new(config.exchange, config.exchange_cap))
            .expect("exchange lands on a passable tile");
        let mut node_ids = Vec::with_capacity(config.nodes.len());
        for spec in &config.nodes {
            let id = world
                .add_node(ResourceNode::new(
                    spec.pos, spec.good, spec.stock, spec.regen, spec.cap,
                ))
                .expect("node lands on a passable tile");
            node_ids.push(id);
        }

        // Consumers take the LOWER ids so their FOOD bids rest first and set the
        // realized price (the supply-sensitive, buyers-lead book; see the module
        // docs). Gatherers follow. World `AgentId`s match econ `AgentId`s by
        // construction (both assigned 0,1,2,… in this order).
        let consumers = usize::from(config.consumers);
        let gatherers = usize::from(config.gatherers);
        let population = consumers + gatherers;

        let mut colonists = Vec::with_capacity(population);
        let mut agents = Vec::with_capacity(population);
        for index in 0..population {
            let id = AgentId(index as u64);
            // World agent for every colonist (consumers idle at the exchange,
            // gatherers haul); placement at the exchange tile is always passable.
            let placed = world
                .add_agent(config.exchange, config.carry_cap, config.move_speed)
                .expect("colonist lands on the exchange tile");
            debug_assert_eq!(placed, id, "world and econ agent ids must coincide");

            let (vocation, node, tp_base) = if index < consumers {
                (
                    Vocation::Consumer,
                    None,
                    config.consumer_time_preference_base_bps,
                )
            } else {
                let node = node_ids[(index - consumers) % node_ids.len()];
                (
                    Vocation::Gatherer,
                    Some(node),
                    config.gatherer_time_preference_base_bps,
                )
            };
            let culture = draw_culture(&mut rng, tp_base, config.leisure_weight_base_bps);
            let need = NeedState::rested();
            agents.push(build_agent(id, &need, &culture, &known, vocation, config));
            colonists.push(Colonist {
                id,
                vocation,
                node,
                need,
                culture,
                critical_streak: 0,
                alive: true,
            });
        }

        // The goods tracked for conservation: node goods plus anything a colonist
        // starts holding (FOOD via nodes/buffers, WOOD via endowments). Money is
        // not a physical good, so it is excluded.
        let mut goods: Vec<GoodId> = Vec::new();
        let push_good = |g: GoodId, goods: &mut Vec<GoodId>| {
            if g != GOLD && !goods.contains(&g) {
                goods.push(g);
            }
        };
        for spec in &config.nodes {
            push_good(spec.good, &mut goods);
        }
        for agent in &agents {
            for g in agent.stock.positive_goods() {
                push_good(g, &mut goods);
            }
        }
        goods.sort();

        let scenario = MarketScenario {
            name: "settlement",
            // A SoundGold M1 (designated-gold spot) scenario, exactly as `Camp`
            // uses (the natural seam): the consumption-log readback and the
            // realized-price accessor live on this path.
            scenario: ScenarioName::MarketBarterishGold,
            seed,
            periods: 0,
            agents,
            recipes: Vec::new(),
            events: Vec::new(),
            money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
        };
        let mut society = Society::from_scenario(scenario);
        society.enable_consumption_log();

        Self {
            world,
            society,
            colonists,
            dynamics,
            known,
            exchange,
            carry_cap: config.carry_cap,
            goods,
            pending_deposits: BTreeMap::new(),
            econ_tick: 0,
            last_report: EconTickReport::default(),
        }
    }

    /// Advance the settlement by one economic tick (the module's documented
    /// phase order). Returns — and stores — the conservation + flow
    /// [`EconTickReport`].
    pub fn econ_tick(&mut self) -> EconTickReport {
        let mut report = EconTickReport {
            econ_tick: self.econ_tick,
            fast_ticks: FAST_TICKS_PER_ECON_TICK,
            ..EconTickReport::default()
        };

        // Snapshot the whole-system totals and the world-only totals before the
        // fast loop. The fast loop only adds goods via regen and only relocates
        // otherwise, so `world_after − world_before` is exactly the regen.
        let world_before: BTreeMap<GoodId, u64> = self
            .goods
            .iter()
            .map(|&g| (g, self.world.total_goods_of(g)))
            .collect();
        for &good in &self.goods {
            report
                .whole_system_before
                .insert(good, self.whole_system_total(good));
        }
        report.total_gold_before_fast = self.society.total_gold().0;

        // ---- 1. FAST: world ticks; track per-colonist deposits via carry deltas.
        let deposited = self.run_fast_loop();
        report.total_gold_after_fast = self.society.total_gold().0;
        debug_assert_eq!(
            report.total_gold_before_fast, report.total_gold_after_fast,
            "the fast loop must not move money"
        );
        for &good in &self.goods {
            let after_fast = self.world.total_goods_of(good);
            let before = world_before.get(&good).copied().unwrap_or(0);
            report.regen.insert(good, after_fast - before);
        }

        // ---- 2. TRANSFER: move delivered exchange units into econ stock, net-zero.
        // A unit that cannot be credited remains in the exchange stockpile, still
        // world-owned and counted there — never destroyed. Two cases, both
        // conserving: (a) a live depositor whose stock is momentarily at the `u32`
        // ceiling is *transient* — the attribution is retried each econ tick and
        // the units transfer once consumption opens headroom; (b) a tombstoned or
        // stale depositor is *permanent* — `credit_stock` rejects it forever, so
        // its pending units freeze in the exchange (still conserved, world-owned),
        // never crossing into econ. The map keeps the attribution without inventing
        // a second goods ledger. (Case (b) is unreachable today: clipping needs the
        // depositor's stock at `u32::MAX`, which starvation — the only death — can
        // never coexist with; it becomes live only with non-starvation death or
        // estate settlement, a later milestone.)
        self.record_pending_deposits(deposited);
        report.transferred = self.transfer_pending_deposits();

        // ---- 3. NEEDS + tombstone (the G1 mechanism).
        report.deaths = self.update_needs_and_tombstone();

        // ---- 4. SCALES.
        self.regenerate_scales();

        // ---- 5. MARKET: the unchanged econ clearing; money is redistributed
        // between colonists here.
        self.society.step();
        report.total_gold_after_step = self.society.total_gold().0;

        // ---- 6. READ-BACK happens at the top of the next tick's NEEDS phase.

        // Conservation receipt: consumed (the sink) is this tick's consumption
        // log; the whole-system after-totals must balance against before + regen
        // − consumed for every good.
        for &(_, good, qty) in self.society.consumption_log_last_tick() {
            *report.consumed.entry(good).or_insert(0) += u64::from(qty);
        }
        for &good in &self.goods {
            report
                .whole_system_after
                .insert(good, self.whole_system_total(good));
        }
        debug_assert!(
            report.conserves(),
            "whole-system conservation broke at econ tick {}",
            self.econ_tick
        );

        self.econ_tick += 1;
        self.last_report = report.clone();
        report
    }

    /// Run `ticks` economic ticks.
    pub fn run(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.econ_tick();
        }
    }

    // ---- the fast loop --------------------------------------------------

    /// Run [`FAST_TICKS_PER_ECON_TICK`] `world` ticks, keeping idle living
    /// gatherers busy (harvest → exchange), and return the per-colonist,
    /// per-good amounts deposited into the exchange stockpile this interval.
    ///
    /// Deposits are detected as carry **decreases**: a gatherer only ever
    /// deposits at the exchange and harvests at its node, and `world.tick` runs
    /// at most one arrival action per agent per tick, so a per-tick carry drop is
    /// exactly a deposit (the accepted amount — overflow stays carried). Escrow
    /// carried over from a previous interval is part of the opening carry, so it
    /// transfers on the arrival that finally lands it.
    fn run_fast_loop(&mut self) -> BTreeMap<(AgentId, GoodId), u32> {
        let mut deposited: BTreeMap<(AgentId, GoodId), u32> = BTreeMap::new();
        // Opening carry baseline (the current escrow), per living gatherer/good.
        let mut prev_carry: BTreeMap<(AgentId, GoodId), u32> = BTreeMap::new();
        for colonist in &self.colonists {
            if colonist.alive && colonist.vocation == Vocation::Gatherer {
                for &good in &self.goods {
                    prev_carry.insert(
                        (colonist.id, good),
                        self.world.agent_carry(colonist.id, good),
                    );
                }
            }
        }
        // Exchange contents before the interval. Transfer runs *after* this
        // loop, so the only thing that changes exchange contents here is
        // deposits — letting us cross-check our carry-delta attribution against
        // the stockpile's own ledger below (debug only), even when prior clipped
        // deposits are still waiting there.
        #[cfg(debug_assertions)]
        let exchange_before: BTreeMap<GoodId, u32> = self
            .goods
            .iter()
            .map(|&g| (g, self.world.stockpile_get(self.exchange, g)))
            .collect();

        for _ in 0..FAST_TICKS_PER_ECON_TICK {
            self.assign_idle_gatherer_tasks();
            self.world.tick();
            for colonist in &self.colonists {
                if !colonist.alive || colonist.vocation != Vocation::Gatherer {
                    continue;
                }
                for &good in &self.goods {
                    let now = self.world.agent_carry(colonist.id, good);
                    let prev = prev_carry.get(&(colonist.id, good)).copied().unwrap_or(0);
                    if now < prev {
                        *deposited.entry((colonist.id, good)).or_insert(0) += prev - now;
                    }
                    prev_carry.insert((colonist.id, good), now);
                }
            }
        }

        // Defend the deposit-attribution assumption: a carry decrease is taken to
        // be a deposit into the exchange, so the per-good carry drops we summed
        // must equal the exchange stockpile's actual increase over the interval
        // (it is the only stockpile, only living gatherers deposit, and transfer
        // runs after this loop). A future task that drained carry elsewhere would
        // break this equality and trip the check rather than silently misattribute.
        #[cfg(debug_assertions)]
        for &good in &self.goods {
            let increase = self
                .world
                .stockpile_get(self.exchange, good)
                .saturating_sub(exchange_before.get(&good).copied().unwrap_or(0));
            let mut attributed = 0u32;
            for (&(_, g), &q) in &deposited {
                if g == good {
                    attributed += q;
                }
            }
            debug_assert_eq!(
                attributed, increase,
                "carry-delta deposits must equal the exchange increase for {good:?}"
            );
        }

        deposited
    }

    fn record_pending_deposits(&mut self, deposited: BTreeMap<(AgentId, GoodId), u32>) {
        for (key, qty) in deposited {
            if qty == 0 {
                continue;
            }
            let pending = self.pending_deposits.entry(key).or_insert(0);
            *pending = pending
                .checked_add(qty)
                .expect("pending exchange-deposit attribution exceeded stockpile capacity");
        }
        #[cfg(debug_assertions)]
        self.debug_assert_pending_matches_exchange();
    }

    /// Move pending exchange-stockpile units into econ stock when the depositing
    /// colonist can receive them. Credit is attempted before the world withdraw,
    /// so a rejected stale/tombstoned id cannot destroy a unit; the bounded
    /// withdraw then removes exactly the credited units from the exchange.
    ///
    /// A still-live depositor whose stock is momentarily full retries here every
    /// econ tick and transfers once headroom opens. A **tombstoned** depositor is
    /// rejected permanently by [`Society::credit_stock`], so its pending units
    /// freeze in the exchange stockpile for good — world-owned and conserved, but
    /// never transferred (an orphaned attribution entry, harmless to conservation
    /// and unreachable in the current starvation-only death model).
    fn transfer_pending_deposits(&mut self) -> BTreeMap<GoodId, u64> {
        let mut transferred = BTreeMap::new();
        let mut remaining = BTreeMap::new();
        let pending = std::mem::take(&mut self.pending_deposits);

        for ((agent, good), qty) in pending {
            if qty == 0 {
                continue;
            }
            let headroom = self
                .society
                .agents
                .get(agent)
                .map_or(0, |a| u32::MAX - a.stock.get(good));
            let available = self.world.stockpile_get(self.exchange, good);
            let take = qty.min(headroom).min(available);
            if take == 0 {
                remaining.insert((agent, good), qty);
                continue;
            }

            if self.society.credit_stock(agent, good, take) {
                let removed = self.world.stockpile_withdraw(self.exchange, good, take);
                assert_eq!(removed, take, "exchange must hold every credited unit");
                if qty > take {
                    remaining.insert((agent, good), qty - take);
                }
                *transferred.entry(good).or_insert(0) += u64::from(removed);
            } else {
                remaining.insert((agent, good), qty);
            }
        }

        self.pending_deposits = remaining;
        #[cfg(debug_assertions)]
        self.debug_assert_pending_matches_exchange();
        transferred
    }

    #[cfg(debug_assertions)]
    fn debug_assert_pending_matches_exchange(&self) {
        for &good in &self.goods {
            let pending = self
                .pending_deposits
                .iter()
                .filter(|((_, g), _)| *g == good)
                .map(|(_, &qty)| qty)
                .sum::<u32>();
            debug_assert_eq!(
                pending,
                self.world.stockpile_get(self.exchange, good),
                "pending transfer attribution must match exchange stock for {good:?}"
            );
        }
    }

    /// Give every idle, living gatherer its next task: deposit if it is carrying
    /// anything, else harvest a full load from its node. Deterministic (id order,
    /// no RNG). Dead gatherers are idled and skipped, so their carry stays frozen.
    fn assign_idle_gatherer_tasks(&mut self) {
        for index in 0..self.colonists.len() {
            let colonist = &self.colonists[index];
            if !colonist.alive || colonist.vocation != Vocation::Gatherer {
                continue;
            }
            let Some(node) = colonist.node else { continue };
            let id = colonist.id;
            if self.world.agent_status(id) != Some(AgentStatus::Idle) {
                continue;
            }
            let task = if self.world.agent_carry_total(id) > 0 {
                Task::GoDeposit(self.exchange)
            } else {
                Task::GoHarvest(node, self.carry_cap)
            };
            self.world.assign_task(id, task);
        }
    }

    // ---- the econ-tick phases ------------------------------------------

    /// NEEDS phase: advance living colonists' needs from the last econ tick's
    /// realized consumption + labor, then tombstone starvation deaths (the G1
    /// mechanism), idling the dead in the world so their carry freezes. Returns
    /// the number of deaths.
    fn update_needs_and_tombstone(&mut self) -> u32 {
        let mut intakes = vec![NeedIntake::default(); self.colonists.len()];
        for &(agent, good, qty) in self.society.consumption_log_last_tick() {
            let Some(index) = self.slot_for_id(agent) else {
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
            let Some(index) = self.slot_for_id(agent) else {
                continue;
            };
            if self.colonists[index].alive {
                intakes[index].labor_used = intakes[index].labor_used.saturating_add(labor);
            }
        }

        for (index, colonist) in self.colonists.iter_mut().enumerate() {
            if colonist.alive {
                colonist.need.advance(&self.dynamics, intakes[index]);
            }
        }

        let mut deaths = 0;
        // Collect deaths first (immutable read of `dynamics`), then apply.
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
                deaths += 1;
            }
        }
        for id in dying {
            self.society.tombstone(id);
            // Freeze the dead colonist in the world so it hauls/deposits nothing
            // more — its carried goods stay escrowed (conserved, not destroyed).
            self.world.assign_task(id, Task::Idle);
        }
        deaths
    }

    /// SCALES phase: regenerate every living colonist's value scale from its need
    /// state, overwriting the econ scale, then cancel now-stale resting quotes.
    fn regenerate_scales(&mut self) {
        let mut rewritten = Vec::new();
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
    }

    fn slot_for_id(&self, id: AgentId) -> Option<usize> {
        // Colonist `i` has `AgentId(i)`, so the numeric index is its slot — an
        // O(1) hit. Fall back to a search to stay correct if that ever changes.
        let guess = id.index() as usize;
        if self.colonists.get(guess).map(|c| c.id) == Some(id) {
            return Some(guess);
        }
        self.colonists.iter().position(|c| c.id == id)
    }

    // ---- accessors ------------------------------------------------------

    /// The whole-system total of `good`: every node, carry, and stockpile
    /// (`world`) plus every colonist's econ stock. The conserved quantity.
    pub fn whole_system_total(&self, good: GoodId) -> u64 {
        self.world.total_goods_of(good) + self.econ_stock_total(good)
    }

    /// Total of `good` held in econ agent stock across all (living and frozen)
    /// colonists.
    pub fn econ_stock_total(&self, good: GoodId) -> u64 {
        self.society
            .agents
            .iter()
            .map(|a| u64::from(a.stock.get(good)))
            .sum()
    }

    /// The goods tracked for whole-system conservation (`GoodId`-ordered).
    pub fn tracked_goods(&self) -> &[GoodId] {
        &self.goods
    }

    /// The most recent realized spot price for `good` (the last trade), or `None`
    /// if no trade in `good` has cleared.
    pub fn realized_price(&self, good: GoodId) -> Option<Gold> {
        self.society.realized_price(good)
    }

    /// The most recent realized FOOD price — the distance→price observable.
    pub fn realized_food_price(&self) -> Option<Gold> {
        self.realized_price(self.known.hunger)
    }

    /// Total money across the settlement (a closed, conserved balance).
    pub fn total_gold(&self) -> Gold {
        self.society.total_gold()
    }

    /// Read-only access to the underlying world (carry/stockpile/node inspection).
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Read-only access to the underlying society (holdings/price assertions).
    pub fn society(&self) -> &Society {
        &self.society
    }

    /// The exchange stockpile id.
    pub fn exchange(&self) -> StockpileId {
        self.exchange
    }

    /// The most recent econ tick's report.
    pub fn last_report(&self) -> &EconTickReport {
        &self.last_report
    }

    /// Completed econ ticks.
    pub fn econ_tick_count(&self) -> u64 {
        self.econ_tick
    }

    /// The colonist count (living and dead).
    pub fn population(&self) -> usize {
        self.colonists.len()
    }

    /// The stable id of the colonist at generation `index`.
    pub fn colonist_id(&self, index: usize) -> Option<AgentId> {
        self.colonists.get(index).map(|c| c.id)
    }

    /// The vocation of the colonist at generation `index`.
    pub fn vocation_of(&self, index: usize) -> Option<Vocation> {
        self.colonists.get(index).map(|c| c.vocation)
    }

    /// Whether the colonist at generation `index` is still alive.
    pub fn is_alive(&self, index: usize) -> bool {
        self.colonists.get(index).is_some_and(|c| c.alive)
    }

    /// The current need state of the colonist at generation `index`.
    pub fn need_of(&self, index: usize) -> Option<NeedState> {
        self.colonists.get(index).map(|c| c.need)
    }

    /// Units of `good` the colonist at generation `index` is carrying in the
    /// world (its delivery escrow).
    pub fn carry_of(&self, index: usize, good: GoodId) -> u32 {
        self.colonists
            .get(index)
            .map(|c| self.world.agent_carry(c.id, good))
            .unwrap_or(0)
    }

    /// Living colonists of a vocation.
    pub fn living_count(&self, vocation: Vocation) -> usize {
        self.colonists
            .iter()
            .filter(|c| c.alive && c.vocation == vocation)
            .count()
    }

    /// Total living colonists.
    pub fn living_total(&self) -> usize {
        self.colonists.iter().filter(|c| c.alive).count()
    }

    /// The highest hunger any living colonist carries — the boundedness probe for
    /// the smoke test (hunger is the need that kills).
    pub fn max_living_hunger(&self) -> u16 {
        self.colonists
            .iter()
            .filter(|c| c.alive)
            .map(|c| c.need.hunger)
            .max()
            .unwrap_or(0)
    }

    // ---- determinism surface -------------------------------------------

    /// A canonical, order-stable byte serialization of the whole settlement —
    /// world, econ holdings, needs, and realized prices. Two settlements are
    /// byte-identical iff these are equal (the determinism tripwire).
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.econ_tick.to_le_bytes());
        out.extend_from_slice(&self.world.canonical_bytes());

        // Config-derived parameters that steer future ticks but are not otherwise
        // captured by the dynamic state below, so two settlements differing only
        // in one of them never digest equal — the tripwire stays honest for
        // non-equivalent configs, not only same-config reruns.
        out.extend_from_slice(&self.carry_cap.to_le_bytes());
        out.extend_from_slice(&self.exchange.0.to_le_bytes());
        push_dynamics_bytes(&mut out, &self.dynamics);

        // Delivered exchange-stockpile units that are still awaiting econ credit
        // affect future transfers, so attribution belongs in the canonical state.
        out.extend_from_slice(&(self.pending_deposits.len() as u32).to_le_bytes());
        for (&(agent, good), &qty) in &self.pending_deposits {
            out.extend_from_slice(&agent.0.to_le_bytes());
            out.extend_from_slice(&good.0.to_le_bytes());
            out.extend_from_slice(&qty.to_le_bytes());
        }

        // Econ agent state in id order. This includes every mutable public field
        // that can affect later stepping: holdings, labor, full value scales,
        // roles, and adaptive price beliefs (0 scale entries for a tombstone).
        out.extend_from_slice(&(self.society.agents.len() as u32).to_le_bytes());
        for agent in self.society.agents.iter() {
            out.extend_from_slice(&agent.id.0.to_le_bytes());
            out.extend_from_slice(&agent.gold.0.to_le_bytes());
            out.extend_from_slice(&agent.labor_capacity.to_le_bytes());
            out.extend_from_slice(&agent.hunger_deficit.to_le_bytes());

            out.extend_from_slice(&(agent.roles.len() as u32).to_le_bytes());
            for &role in &agent.roles {
                push_role_bytes(&mut out, role);
            }

            out.extend_from_slice(&(agent.scale.len() as u32).to_le_bytes());
            for want in &agent.scale {
                push_want_kind_bytes(&mut out, want.kind);
                push_horizon_bytes(&mut out, want.horizon);
                out.extend_from_slice(&want.qty.to_le_bytes());
                out.push(u8::from(want.satisfied));
            }

            // Every physical good an agent can hold is already in `self.goods`
            // (node goods ∪ starting goods; trade only relocates them and no
            // recipe mints a new one here), and `self.goods` is sorted — so
            // serialize against it directly, with no per-agent clone/merge/re-sort.
            // The debug check pins that "complete and sorted" assumption.
            #[cfg(debug_assertions)]
            for good in agent.stock.positive_goods() {
                debug_assert!(
                    good == GOLD || self.goods.contains(&good),
                    "agent holds an untracked good {good:?} the digest would miss"
                );
            }
            out.extend_from_slice(&(self.goods.len() as u32).to_le_bytes());
            for &good in &self.goods {
                out.extend_from_slice(&good.0.to_le_bytes());
                out.extend_from_slice(&agent.stock.get(good).to_le_bytes());
            }

            out.extend_from_slice(&(agent.expect.len() as u32).to_le_bytes());
            for belief in &agent.expect {
                out.extend_from_slice(&belief.expected.0.to_le_bytes());
                out.extend_from_slice(&belief.step.0.to_le_bytes());
                out.extend_from_slice(&belief.last_seen.to_le_bytes());
            }
        }

        // Colonist need/liveness state in generation order.
        out.extend_from_slice(&(self.colonists.len() as u32).to_le_bytes());
        for colonist in &self.colonists {
            out.extend_from_slice(&colonist.id.0.to_le_bytes());
            out.push(u8::from(colonist.alive));
            out.push(u8::from(colonist.vocation == Vocation::Gatherer));
            out.extend_from_slice(&colonist.need.hunger.to_le_bytes());
            out.extend_from_slice(&colonist.need.warmth.to_le_bytes());
            out.extend_from_slice(&colonist.need.rest.to_le_bytes());
            out.extend_from_slice(&colonist.critical_streak.to_le_bytes());
            // Culture drives the next scale regeneration and the node the next
            // harvest target, so both belong in the future-behavior identity.
            out.extend_from_slice(&colonist.culture.time_preference_bps.to_le_bytes());
            out.extend_from_slice(&colonist.culture.leisure_weight_bps.to_le_bytes());
            match colonist.node {
                Some(node) => {
                    out.push(1);
                    out.extend_from_slice(&node.0.to_le_bytes());
                }
                None => out.push(0),
            }
        }

        // Realized prices for the tracked goods.
        for &good in &self.goods {
            out.extend_from_slice(&good.0.to_le_bytes());
            match self.realized_price(good) {
                Some(price) => {
                    out.push(1);
                    out.extend_from_slice(&price.0.to_le_bytes());
                }
                None => out.push(0),
            }
        }
        out
    }

    /// A 64-bit FNV-1a digest of [`Settlement::canonical_bytes`] — a compact
    /// cross-run determinism check.
    pub fn digest(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in self.canonical_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }
}

/// Draw a colonist's culture from the world-generation `Rng` only: time
/// preference within a small band above the vocation's base, and a leisure
/// weight in a fixed band. Mirrors `life::Camp::draw_culture` so the same
/// determinism discipline holds.
fn draw_culture(rng: &mut Rng, time_preference_base: u16, leisure_base: u16) -> CultureParams {
    let span = u16::try_from(rng.next_u64() % 500).unwrap_or(0);
    let time_preference_bps = time_preference_base.saturating_add(span);
    let leisure_weight_bps =
        leisure_base.saturating_add(u16::try_from(rng.next_u64() % 1_001).unwrap_or(0));
    CultureParams::new(time_preference_bps, leisure_weight_bps)
}

fn build_agent(
    id: AgentId,
    need: &NeedState,
    culture: &CultureParams,
    known: &KnownGoods,
    vocation: Vocation,
    config: &SettlementConfig,
) -> Agent {
    let mut stock = Stock::new(NET.0);
    let (gold, food, wood) = match vocation {
        Vocation::Gatherer => (
            config.starting_gold_gatherer,
            config.gatherer_food_buffer,
            config.gatherer_wood_buffer,
        ),
        Vocation::Consumer => (
            config.starting_gold_consumer,
            config.consumer_food_buffer,
            config.consumer_wood_endowment,
        ),
    };
    stock.add(FOOD, food);
    stock.add(WOOD, wood);
    Agent {
        id,
        scale: regenerate_scale(need, culture, known),
        stock,
        gold: Gold(gold),
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: belief_vec(),
    }
}

fn push_dynamics_bytes(out: &mut Vec<u8>, d: &NeedDynamics) {
    out.extend_from_slice(&d.need_max.to_le_bytes());
    out.extend_from_slice(&d.hunger_deplete.to_le_bytes());
    out.extend_from_slice(&d.warmth_deplete.to_le_bytes());
    out.extend_from_slice(&d.hunger_per_food.to_le_bytes());
    out.extend_from_slice(&d.warmth_per_wood.to_le_bytes());
    out.extend_from_slice(&d.rest_per_labor.to_le_bytes());
    out.extend_from_slice(&d.rest_recover.to_le_bytes());
    out.extend_from_slice(&d.hunger_critical.to_le_bytes());
    out.extend_from_slice(&d.death_window.to_le_bytes());
}

fn push_role_bytes(out: &mut Vec<u8>, role: Role) {
    out.push(match role {
        Role::Household => 0,
        Role::Producer => 1,
        Role::Trader => 2,
        Role::Capitalist => 3,
        Role::Worker => 4,
        Role::Consumer => 5,
    });
}

fn push_want_kind_bytes(out: &mut Vec<u8>, kind: WantKind) {
    match kind {
        WantKind::Good(good) => {
            out.push(0);
            out.extend_from_slice(&good.0.to_le_bytes());
        }
        WantKind::Leisure => out.push(1),
    }
}

fn push_horizon_bytes(out: &mut Vec<u8>, horizon: Horizon) {
    match horizon {
        Horizon::Now => out.push(0),
        Horizon::Next => out.push(1),
        Horizon::Later(ticks) => {
            out.push(2);
            out.push(ticks);
        }
    }
}

fn belief_vec() -> Vec<PriceBelief> {
    let slots = usize::from(NET.0) + 1;
    vec![PriceBelief::new(Gold(2), Gold(1)); slots]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_places_one_world_agent_per_colonist_at_the_exchange() {
        let config = SettlementConfig::viable();
        let s = Settlement::generate(1, &config);
        assert_eq!(
            s.population(),
            usize::from(config.consumers) + usize::from(config.gatherers)
        );
        // Consumers take the lower ids, gatherers the higher.
        for index in 0..s.population() {
            let expected = if index < usize::from(config.consumers) {
                Vocation::Consumer
            } else {
                Vocation::Gatherer
            };
            assert_eq!(s.vocation_of(index), Some(expected));
            assert_eq!(s.colonist_id(index), Some(AgentId(index as u64)));
        }
        // Everyone starts on the exchange tile.
        for index in 0..s.population() {
            let id = s.colonist_id(index).unwrap();
            assert_eq!(s.world().agent_pos(id), Some(config.exchange));
        }
    }

    #[test]
    fn tracked_goods_are_food_and_wood_only() {
        let s = Settlement::generate(1, &SettlementConfig::viable());
        assert_eq!(s.tracked_goods(), &[FOOD, WOOD]);
    }

    #[test]
    #[should_panic(expected = "cannot harvest the money good")]
    fn generate_rejects_a_money_good_resource_node() {
        // GOLD is excluded from `self.goods`, so a GOLD node would be harvested
        // and deposited by the fast loop yet never transferred or conserved — a
        // silent world-side money leak. `generate` must reject it at the seam.
        let mut config = SettlementConfig::viable();
        config.nodes[0].good = GOLD;
        let _ = Settlement::generate(1, &config);
    }

    #[test]
    #[should_panic(expected = "must define at least one resource node")]
    fn generate_rejects_gatherers_without_nodes() {
        let mut config = SettlementConfig::viable();
        config.nodes.clear();
        let _ = Settlement::generate(1, &config);
    }

    #[test]
    fn canonical_bytes_include_value_scale_contents() {
        let config = SettlementConfig::viable();
        let a = Settlement::generate(1, &config);
        let mut b = Settlement::generate(1, &config);

        let agent = b
            .society
            .agents
            .get_mut(AgentId(0))
            .expect("generated consumer resolves");
        assert!(
            !agent.scale.is_empty(),
            "generated agents have value scales"
        );
        agent.scale[0].qty = agent.scale[0].qty.saturating_add(1);

        assert_ne!(a.canonical_bytes(), b.canonical_bytes());
        assert_ne!(a.digest(), b.digest());
    }

    #[test]
    fn first_econ_tick_transfers_some_food_and_conserves() {
        let config = SettlementConfig::viable().with_food_node_distance(3);
        let mut s = Settlement::generate(1, &config);
        let report = s.econ_tick();
        // A near node delivers FOOD within the first interval.
        assert!(
            report.transferred_of(FOOD) > 0,
            "no FOOD reached the market"
        );
        // No WOOD is ever hauled (it never enters the world).
        assert_eq!(report.transferred_of(WOOD), 0);
        assert_eq!(s.world().total_goods_of(WOOD), 0);
        assert!(report.conserves(), "first tick broke conservation");
    }
}
