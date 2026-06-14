//! `World` — the spatial substrate that ties the grid, nodes, stockpiles, and
//! agents together and advances them deterministically.
//!
//! `World` is a *pure spatial* component (game-spec §4.1, G2a): it knows
//! positions, terrain, movement, harvest yields, and storage. It does **not**
//! know prices, money, wants, or trades — goods are tracked only as integer
//! quantities of `GoodId` at locations / carried by agents. The economic
//! coupling (the two-rate loop, delivery escrow, distance-affects-price) is G2b
//! and lives in the integration, not here.
//!
//! Two invariants are the contract:
//!
//! - **Determinism.** Integer state throughout; the econ `Rng` is consumed at
//!   world *generation* only and `tick()` draws nothing; agents are always
//!   iterated in `AgentId` order; storage is `BTreeMap`/`Vec`, never `HashMap`.
//!   Same seed + same command sequence → byte-identical world.
//! - **Conservation.** Node regen is the *only* source of goods (clamped to
//!   `cap`, fully accounted in the per-tick [`TickReport`]); movement, harvest,
//!   and deposit relocate units without ever creating or destroying one. The
//!   world total ([`World::total_goods`]) therefore changes per tick by exactly
//!   the report's `regenerated`.

use std::collections::BTreeMap;

use econ::agent::AgentId;
use econ::good::GoodId;
use econ::rng::Rng;

use crate::grid::{Grid, Pos, Terrain};
use crate::node::{NodeId, ResourceNode};
use crate::path::{shortest_path, travel_cost};
use crate::stockpile::{Stockpile, StockpileId};

/// What an agent is currently trying to do. `Copy` so the `tick` borrow dance is
/// trivial. A spatial task completes (reverts to `Idle`) on arrival.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    /// No target; the agent stays put.
    Idle,
    /// Walk to the node's tile and harvest up to `want` units (once, on arrival).
    GoHarvest(NodeId, u32),
    /// Walk to the stockpile's tile and deposit all carried goods (on arrival).
    GoDeposit(StockpileId),
    /// Walk to an arbitrary tile.
    GoTo(Pos),
}

/// A snapshot view of where an agent is and what it is doing — the read model
/// the inspectors (G2d) and tests use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentStatus {
    /// Idle with no active task.
    Idle,
    /// Has a reachable target and is en route (or about to act on arrival).
    Moving,
    /// Has a target it cannot reach; left put, deterministically, no panic.
    Blocked,
}

/// Why a placement was rejected. Placement on impassable tiles is never silently
/// accepted (G2a milestone boundary).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementError {
    /// The position lies outside the grid.
    OutOfBounds,
    /// The position is an impassable tile.
    Impassable,
}

/// The per-tick spatial ledger report — the conservation receipt.
///
/// `harvested` and `deposited` count units *relocated* (which do not change the
/// world total); `regenerated` counts units *created* by node regen (the only
/// source). By construction `net == regenerated` (relocation conserves), so the
/// world total after the tick equals the total before plus `net`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickReport {
    pub harvested: u64,
    pub deposited: u64,
    pub regenerated: u64,
    pub net: i64,
}

/// Per-agent spatial state: position, carried inventory, task, and reachability.
#[derive(Clone, Debug, PartialEq, Eq)]
struct AgentState {
    pos: Pos,
    carry: BTreeMap<GoodId, u32>,
    carry_cap: u32,
    move_speed: u16,
    task: Task,
    blocked: bool,
    path_target: Option<Pos>,
    path: Vec<Pos>,
}

impl AgentState {
    fn carried_total(&self) -> u32 {
        self.carry.values().sum()
    }

    fn carry_room(&self) -> u32 {
        self.carry_cap.saturating_sub(self.carried_total())
    }

    fn add_carry(&mut self, good: GoodId, qty: u32) {
        if qty > 0 {
            *self.carry.entry(good).or_insert(0) += qty;
        }
    }

    /// Remove up to `qty` of `good` from carry, returning the amount removed.
    fn take_carry(&mut self, good: GoodId, qty: u32) -> u32 {
        let Some(held) = self.carry.get_mut(&good) else {
            return 0;
        };
        let taken = qty.min(*held);
        *held -= taken;
        if *held == 0 {
            self.carry.remove(&good);
        }
        taken
    }

    fn clear_path(&mut self) {
        self.path_target = None;
        self.path.clear();
    }

    fn write_canonical(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.pos.x.to_le_bytes());
        out.extend_from_slice(&self.pos.y.to_le_bytes());
        out.extend_from_slice(&self.carry_cap.to_le_bytes());
        out.extend_from_slice(&self.move_speed.to_le_bytes());
        write_task_canonical(self.task, out);
        out.push(u8::from(self.blocked));
        out.extend_from_slice(&(self.carry.len() as u32).to_le_bytes());
        for (good, qty) in &self.carry {
            out.extend_from_slice(&good.0.to_le_bytes());
            out.extend_from_slice(&qty.to_le_bytes());
        }
        match self.path_target {
            Some(target) => {
                out.push(1);
                out.extend_from_slice(&target.x.to_le_bytes());
                out.extend_from_slice(&target.y.to_le_bytes());
            }
            None => out.push(0),
        }
        out.extend_from_slice(&(self.path.len() as u32).to_le_bytes());
        for step in &self.path {
            out.extend_from_slice(&step.x.to_le_bytes());
            out.extend_from_slice(&step.y.to_le_bytes());
        }
    }
}

fn write_task_canonical(task: Task, out: &mut Vec<u8>) {
    out.push(task_tag(task));
    match task {
        Task::Idle => {}
        Task::GoHarvest(node, want) => {
            out.extend_from_slice(&node.0.to_le_bytes());
            out.extend_from_slice(&want.to_le_bytes());
        }
        Task::GoDeposit(stockpile) => {
            out.extend_from_slice(&stockpile.0.to_le_bytes());
        }
        Task::GoTo(pos) => {
            out.extend_from_slice(&pos.x.to_le_bytes());
            out.extend_from_slice(&pos.y.to_le_bytes());
        }
    }
}

fn task_tag(task: Task) -> u8 {
    match task {
        Task::Idle => 0,
        Task::GoHarvest(_, _) => 1,
        Task::GoDeposit(_) => 2,
        Task::GoTo(_) => 3,
    }
}

fn write_report_canonical(report: TickReport, out: &mut Vec<u8>) {
    out.extend_from_slice(&report.harvested.to_le_bytes());
    out.extend_from_slice(&report.deposited.to_le_bytes());
    out.extend_from_slice(&report.regenerated.to_le_bytes());
    out.extend_from_slice(&report.net.to_le_bytes());
}

/// A seed-driven recipe for procedurally generating a world. All randomness is
/// drawn from the econ `Rng` while building the world; the tick loop draws none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldGen {
    pub width: u16,
    pub height: u16,
    /// Number of impassable tiles scattered across the grid.
    pub impassable_count: u32,
    /// Number of resource nodes placed on distinct passable tiles.
    pub node_count: u32,
    /// Goods nodes may yield; each node's good is drawn from this pool. A node is
    /// only placed if the pool is non-empty.
    pub node_goods: Vec<GoodId>,
    pub node_stock: u32,
    pub node_regen: u32,
    pub node_cap: u32,
    /// Number of stockpiles placed on distinct passable tiles.
    pub stockpile_count: u32,
    pub stockpile_cap: u32,
    /// Number of agents placed on distinct passable tiles.
    pub agent_count: u32,
    pub agent_carry_cap: u32,
    pub agent_move_speed: u16,
}

impl WorldGen {
    /// A small, fully-featured demo world: obstacles, two food/wood node kinds,
    /// stockpiles, and a handful of agents. Mechanism knobs, not balance targets.
    pub fn demo() -> Self {
        use econ::good::{FOOD, WOOD};
        Self {
            width: 16,
            height: 16,
            impassable_count: 24,
            node_count: 6,
            node_goods: vec![FOOD, WOOD],
            node_stock: 20,
            node_regen: 1,
            node_cap: 40,
            stockpile_count: 2,
            stockpile_cap: 100,
            agent_count: 4,
            agent_carry_cap: 10,
            agent_move_speed: 1,
        }
    }
}

/// The spatial world: a grid plus nodes, stockpiles, and agents that move and
/// haul goods deterministically over it.
#[derive(Clone, Debug)]
pub struct World {
    grid: Grid,
    nodes: Vec<ResourceNode>,
    stockpiles: Vec<Stockpile>,
    agents: BTreeMap<AgentId, AgentState>,
    next_agent_index: u64,
    last_report: TickReport,
}

impl World {
    /// An empty world over `grid` (no nodes, stockpiles, or agents).
    pub fn new(grid: Grid) -> Self {
        Self {
            grid,
            nodes: Vec::new(),
            stockpiles: Vec::new(),
            agents: BTreeMap::new(),
            next_agent_index: 0,
            last_report: TickReport::default(),
        }
    }

    /// Generate a world from `seed` and a [`WorldGen`] recipe. Deterministic:
    /// the same `(seed, WorldGen)` yields a byte-identical world. The `Rng` is
    /// consumed only here.
    ///
    /// Positions are assigned by a partial Fisher–Yates draw over tile indices,
    /// materializing only the slots needed for impassable terrain, nodes,
    /// stockpiles, and agents. The slots are disjoint, so placements never
    /// collide, and every node/stockpile/agent lands on a passable tile
    /// (placement is valid by construction). Requested counts are clamped to the
    /// tiles actually available.
    pub fn generate(seed: u64, gen: &WorldGen) -> Self {
        let mut rng = Rng::new(seed);
        let mut grid = Grid::new(gen.width, gen.height);

        let node_slots = if gen.node_goods.is_empty() {
            0
        } else {
            gen.node_count
        };
        let requested_slots = gen
            .impassable_count
            .saturating_add(node_slots)
            .saturating_add(gen.stockpile_count)
            .saturating_add(gen.agent_count);
        let positions = sample_positions(gen.width, gen.height, requested_slots, &mut rng);

        let mut cursor = 0usize;
        let take = |count: u32, at: &mut usize| -> Vec<Pos> {
            let available = positions.len().saturating_sub(*at);
            let n = (count as usize).min(available);
            let slice = positions[*at..*at + n].to_vec();
            *at += n;
            slice
        };

        // 1. Impassable terrain.
        for pos in take(gen.impassable_count, &mut cursor) {
            grid.set_terrain(pos, Terrain::Impassable);
        }

        let mut world = World::new(grid);

        // 2. Resource nodes (good drawn from the pool, if any).
        if !gen.node_goods.is_empty() {
            for pos in take(gen.node_count, &mut cursor) {
                let pick = (rng.next_u64() % gen.node_goods.len() as u64) as usize;
                let good = gen.node_goods[pick];
                let node =
                    ResourceNode::new(pos, good, gen.node_stock, gen.node_regen, gen.node_cap);
                world
                    .add_node(node)
                    .expect("generated node lands on a passable tile");
            }
        }

        // 3. Stockpiles.
        for pos in take(gen.stockpile_count, &mut cursor) {
            world
                .add_stockpile(Stockpile::new(pos, gen.stockpile_cap))
                .expect("generated stockpile lands on a passable tile");
        }

        // 4. Agents.
        for pos in take(gen.agent_count, &mut cursor) {
            world
                .add_agent(pos, gen.agent_carry_cap, gen.agent_move_speed)
                .expect("generated agent lands on a passable tile");
        }

        world
    }

    /// Read-only access to the grid.
    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// Add a resource node, rejecting placement on an out-of-bounds or impassable
    /// tile. Returns the assigned [`NodeId`] on success.
    ///
    /// The node's regen invariant (`stock <= cap`) is normalized on insertion:
    /// `ResourceNode`'s fields are public, so a caller can build one with
    /// `stock > cap` (bypassing [`ResourceNode::new`]'s clamp). Were such a node
    /// admitted, the first regen tick would silently clamp the excess away while
    /// reporting `regenerated == 0` — destroying goods despite the conservation
    /// receipt. Clamping here (the single insertion choke point — `generate` also
    /// routes through it) keeps the per-tick report honest by construction.
    pub fn add_node(&mut self, mut node: ResourceNode) -> Result<NodeId, PlacementError> {
        self.check_placement(node.pos)?;
        node.stock = node.stock.min(node.cap);
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(node);
        Ok(id)
    }

    /// Add a stockpile, rejecting placement on an out-of-bounds or impassable
    /// tile. Returns the assigned [`StockpileId`] on success.
    pub fn add_stockpile(&mut self, stockpile: Stockpile) -> Result<StockpileId, PlacementError> {
        self.check_placement(stockpile.pos)?;
        let id = StockpileId(self.stockpiles.len() as u32);
        self.stockpiles.push(stockpile);
        Ok(id)
    }

    /// Place an agent at `pos` with the given carry capacity and move speed,
    /// rejecting an out-of-bounds or impassable tile. Returns the assigned
    /// [`AgentId`] (sequential, generation 0).
    pub fn add_agent(
        &mut self,
        pos: Pos,
        carry_cap: u32,
        move_speed: u16,
    ) -> Result<AgentId, PlacementError> {
        self.check_placement(pos)?;
        let id = AgentId(self.next_agent_index);
        self.next_agent_index += 1;
        self.agents.insert(
            id,
            AgentState {
                pos,
                carry: BTreeMap::new(),
                carry_cap,
                move_speed,
                task: Task::Idle,
                blocked: false,
                path_target: None,
                path: Vec::new(),
            },
        );
        Ok(id)
    }

    /// Remove a spatial agent from the world, returning any carry it still held.
    ///
    /// This is an out-of-tick estate cleanup hook for the sim layer: callers must
    /// settle the returned carry if it is non-empty. Removing an unknown id is a
    /// no-op. No generation path calls it, so generated worlds and no-death runs are
    /// byte-identical.
    pub fn remove_agent(&mut self, id: AgentId) -> Option<BTreeMap<GoodId, u32>> {
        self.agents.remove(&id).map(|agent| agent.carry)
    }

    fn check_placement(&self, pos: Pos) -> Result<(), PlacementError> {
        if !self.grid.in_bounds(pos) {
            Err(PlacementError::OutOfBounds)
        } else if !self.grid.is_passable(pos) {
            Err(PlacementError::Impassable)
        } else {
            Ok(())
        }
    }

    /// Assign a task to an agent, clearing any stale `Blocked` flag so the next
    /// tick re-evaluates reachability. Returns `false` (no change) if the agent
    /// is unknown or the task references an unknown node/stockpile or an
    /// out-of-bounds `GoTo` (a `GoTo` to an unreachable in-bounds tile is
    /// accepted — it deterministically becomes `Blocked`).
    pub fn assign_task(&mut self, agent: AgentId, task: Task) -> bool {
        let valid = match task {
            Task::Idle => true,
            Task::GoHarvest(node, _) => (node.0 as usize) < self.nodes.len(),
            Task::GoDeposit(sp) => (sp.0 as usize) < self.stockpiles.len(),
            Task::GoTo(pos) => self.grid.in_bounds(pos),
        };
        if !valid {
            return false;
        }
        match self.agents.get_mut(&agent) {
            Some(state) => {
                state.task = task;
                state.blocked = false;
                state.clear_path();
                true
            }
            None => false,
        }
    }

    /// Advance the world one tick (no RNG). The order is fixed (game-spec G2a):
    ///
    /// 1. advance each agent (id order) toward its target by `move_speed` steps;
    /// 2. apply arrivals (id order): harvest into carry / deposit into stockpile;
    /// 3. regen nodes (id order);
    /// 4. compile the per-tick [`TickReport`].
    ///
    /// Returns the report and also stores it as [`World::last_report`].
    pub fn tick(&mut self) -> TickReport {
        let mut report = TickReport::default();
        let ids: Vec<AgentId> = self.agents.keys().copied().collect();

        // 1. Movement.
        for &id in &ids {
            self.advance_agent(id);
        }

        // 2. Arrivals (harvest / deposit / go-to completion).
        for &id in &ids {
            self.apply_arrival(id, &mut report);
        }

        // 3. Node regeneration — the only source of goods.
        for node in &mut self.nodes {
            report.regenerated += u64::from(node.regen());
        }

        // 4. Conservation receipt: relocation conserves, so the net change in the
        //    world total is exactly what regen created.
        report.net = report.regenerated as i64;
        self.last_report = report;
        report
    }

    /// Run `ticks` ticks, returning the final tick's report (or the default
    /// report if `ticks == 0`).
    pub fn run(&mut self, ticks: u64) -> TickReport {
        let mut report = TickReport::default();
        for _ in 0..ticks {
            report = self.tick();
        }
        report
    }

    fn target_of(&self, id: AgentId) -> Option<Pos> {
        let state = self.agents.get(&id)?;
        match state.task {
            Task::Idle => None,
            Task::GoTo(pos) => Some(pos),
            Task::GoHarvest(node, _) => self.nodes.get(node.0 as usize).map(|n| n.pos),
            Task::GoDeposit(sp) => self.stockpiles.get(sp.0 as usize).map(|s| s.pos),
        }
    }

    fn advance_agent(&mut self, id: AgentId) {
        let Some(target) = self.target_of(id) else {
            return;
        };
        let (pos, speed) = {
            let state = &self.agents[&id];
            (state.pos, state.move_speed)
        };
        if pos == target {
            // Already standing on the target; arrivals will act on it.
            let state = self.agents.get_mut(&id).unwrap();
            state.blocked = false;
            state.clear_path();
            return;
        }
        if speed == 0 {
            let state = self.agents.get_mut(&id).unwrap();
            state.blocked = true;
            state.clear_path();
            state.path_target = Some(target);
            return;
        }

        let needs_path = {
            let state = &self.agents[&id];
            if state.blocked && state.path_target == Some(target) {
                return;
            }
            state.path_target != Some(target) || state.path.is_empty()
        };
        if needs_path {
            match shortest_path(&self.grid, pos, target) {
                Some(path) if !path.is_empty() => {
                    let state = self.agents.get_mut(&id).unwrap();
                    state.path_target = Some(target);
                    state.path = path;
                    state.blocked = false;
                }
                // Unreachable: leave it put, blocked.
                _ => {
                    let state = self.agents.get_mut(&id).unwrap();
                    state.blocked = true;
                    state.clear_path();
                    state.path_target = Some(target);
                    return;
                }
            }
        }

        let state = self.agents.get_mut(&id).unwrap();
        let steps = usize::from(speed).min(state.path.len());
        state.pos = state.path[steps - 1];
        state.path.drain(..steps);
        state.blocked = false;
        if state.pos == target {
            state.clear_path();
        }
    }

    fn apply_arrival(&mut self, id: AgentId, report: &mut TickReport) {
        let Some(target) = self.target_of(id) else {
            return;
        };
        let (pos, task) = {
            let state = &self.agents[&id];
            (state.pos, state.task)
        };
        if pos != target {
            return;
        }
        match task {
            Task::Idle => {}
            Task::GoTo(_) => {
                let state = self.agents.get_mut(&id).unwrap();
                state.task = Task::Idle;
                state.clear_path();
            }
            Task::GoHarvest(node_id, want) => {
                let room = self.agents[&id].carry_room();
                let node = &mut self.nodes[node_id.0 as usize];
                let good = node.good;
                let moved = node.harvest(want, room);
                let state = self.agents.get_mut(&id).unwrap();
                state.add_carry(good, moved);
                state.task = Task::Idle;
                state.clear_path();
                report.harvested += u64::from(moved);
            }
            Task::GoDeposit(sp_id) => {
                // Deposit every carried good (good order), keeping any overflow.
                let carried: Vec<(GoodId, u32)> = self.agents[&id]
                    .carry
                    .iter()
                    .map(|(&g, &q)| (g, q))
                    .collect();
                let stockpile = &mut self.stockpiles[sp_id.0 as usize];
                let mut deposited = 0u64;
                let mut accepted_per_good: Vec<(GoodId, u32)> = Vec::new();
                for (good, qty) in carried {
                    let accepted = stockpile.deposit(good, qty);
                    if accepted > 0 {
                        accepted_per_good.push((good, accepted));
                        deposited += u64::from(accepted);
                    }
                }
                let state = self.agents.get_mut(&id).unwrap();
                for (good, accepted) in accepted_per_good {
                    state.take_carry(good, accepted);
                }
                state.task = Task::Idle;
                state.clear_path();
                report.deposited += deposited;
            }
        }
    }

    // ---- Query accessors --------------------------------------------------

    /// The agent's current position, or `None` if the agent is unknown.
    pub fn agent_pos(&self, id: AgentId) -> Option<Pos> {
        self.agents.get(&id).map(|s| s.pos)
    }

    /// Units of `good` the agent is carrying.
    pub fn agent_carry(&self, id: AgentId, good: GoodId) -> u32 {
        self.agents
            .get(&id)
            .map(|s| s.carry.get(&good).copied().unwrap_or(0))
            .unwrap_or(0)
    }

    /// Total units the agent is carrying across all goods.
    pub fn agent_carry_total(&self, id: AgentId) -> u32 {
        self.agents.get(&id).map(|s| s.carried_total()).unwrap_or(0)
    }

    /// The agent's current task, or `None` if the agent is unknown.
    pub fn agent_task(&self, id: AgentId) -> Option<Task> {
        self.agents.get(&id).map(|s| s.task)
    }

    /// The agent's spatial status (`Idle` / `Moving` / `Blocked`).
    pub fn agent_status(&self, id: AgentId) -> Option<AgentStatus> {
        self.agents.get(&id).map(|s| {
            if s.blocked {
                AgentStatus::Blocked
            } else if matches!(s.task, Task::Idle) {
                AgentStatus::Idle
            } else {
                AgentStatus::Moving
            }
        })
    }

    /// Whether the agent is currently `Blocked` (target unreachable).
    pub fn agent_blocked(&self, id: AgentId) -> bool {
        matches!(self.agent_status(id), Some(AgentStatus::Blocked))
    }

    /// The agent ids in ascending order.
    pub fn agent_ids(&self) -> Vec<AgentId> {
        self.agents.keys().copied().collect()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn stockpile_count(&self) -> usize {
        self.stockpiles.len()
    }

    /// Read-only access to a node.
    pub fn node(&self, id: NodeId) -> Option<&ResourceNode> {
        self.nodes.get(id.0 as usize)
    }

    /// Read-only access to a stockpile.
    pub fn stockpile(&self, id: StockpileId) -> Option<&Stockpile> {
        self.stockpiles.get(id.0 as usize)
    }

    /// Units of `good` stored in a stockpile.
    pub fn stockpile_get(&self, id: StockpileId, good: GoodId) -> u32 {
        self.stockpiles
            .get(id.0 as usize)
            .map(|s| s.get(good))
            .unwrap_or(0)
    }

    /// Remove up to `qty` units of `good` from a stockpile, returning the amount
    /// removed (0 for an unknown stockpile). The mirror of a deposit: the units
    /// are *relocated out of the world* to the caller — this is the only public
    /// world sink, and it exists for the G2b world→econ transfer seam (the
    /// exchange stockpile is drained into econ stock once per econ tick;
    /// `docs/engine-divergence.md`). It is **out-of-tick**: `World::tick` never
    /// calls it, so the per-tick [`TickReport`] and the G2a conservation/tick
    /// tests are unaffected. After a withdraw, [`World::total_goods`] drops by
    /// exactly the amount removed (the world's only way to lose a unit).
    pub fn stockpile_withdraw(&mut self, id: StockpileId, good: GoodId, qty: u32) -> u32 {
        self.stockpiles
            .get_mut(id.0 as usize)
            .map(|s| s.withdraw(good, qty))
            .unwrap_or(0)
    }

    /// Remove up to `qty` units of `good` from an agent's carry, returning the
    /// amount removed (0 for an unknown agent). The carry analogue of
    /// [`World::stockpile_withdraw`]: the units are *relocated out of the world* to
    /// the caller. It exists for the G4a real-death estate seam — a dead colonist's
    /// carried delivery escrow settles to the settlement commons rather than
    /// freezing in the world (`docs/engine-divergence.md`). Like the stockpile
    /// withdraw it is **out-of-tick**: [`World::tick`] never calls it, so the
    /// per-tick [`TickReport`] and the G2a conservation/tick tests are unaffected.
    /// After a withdraw, [`World::total_goods`] drops by exactly the amount removed.
    pub fn withdraw_agent_carry(&mut self, id: AgentId, good: GoodId, qty: u32) -> u32 {
        self.agents
            .get_mut(&id)
            .map(|agent| agent.take_carry(good, qty))
            .unwrap_or(0)
    }

    /// The shortest-path step distance between two tiles around impassable
    /// terrain, or `None` if `to` is unreachable from `from`. On an open grid
    /// this equals their Manhattan distance and is monotone in separation.
    pub fn travel_estimate(&self, from: Pos, to: Pos) -> Option<u32> {
        travel_cost(&self.grid, from, to)
    }

    /// Obstacle-blind Manhattan distance between two tiles (a lower bound on
    /// travel cost; monotone in grid separation).
    pub fn grid_distance(&self, from: Pos, to: Pos) -> u32 {
        from.manhattan(to)
    }

    /// The total goods in the world: every node's stock, every carried unit, and
    /// every stockpile's contents. Changes per tick only by the report's `net`
    /// (== `regenerated`) — the spatial conservation invariant.
    pub fn total_goods(&self) -> u64 {
        let nodes: u64 = self.nodes.iter().map(|n| u64::from(n.stock)).sum();
        let carried: u64 = self
            .agents
            .values()
            .map(|a| u64::from(a.carried_total()))
            .sum();
        let stored: u64 = self.stockpiles.iter().map(|s| u64::from(s.total())).sum();
        nodes + carried + stored
    }

    /// The total of a single good across nodes, carries, and stockpiles.
    pub fn total_goods_of(&self, good: GoodId) -> u64 {
        let nodes: u64 = self
            .nodes
            .iter()
            .filter(|n| n.good == good)
            .map(|n| u64::from(n.stock))
            .sum();
        let carried: u64 = self
            .agents
            .values()
            .map(|a| u64::from(a.carry.get(&good).copied().unwrap_or(0)))
            .sum();
        let stored: u64 = self.stockpiles.iter().map(|s| u64::from(s.get(good))).sum();
        nodes + carried + stored
    }

    /// The most recent tick's spatial report.
    pub fn last_report(&self) -> TickReport {
        self.last_report
    }

    // ---- Determinism surface ---------------------------------------------

    /// A canonical, order-stable byte serialization of the entire world state.
    /// Two worlds are byte-identical iff their canonical bytes are equal — the
    /// determinism tripwire for generation and ticking.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.grid.write_canonical(&mut out);
        out.extend_from_slice(&self.next_agent_index.to_le_bytes());
        write_report_canonical(self.last_report, &mut out);
        out.extend_from_slice(&(self.nodes.len() as u32).to_le_bytes());
        for node in &self.nodes {
            node.write_canonical(&mut out);
        }
        out.extend_from_slice(&(self.stockpiles.len() as u32).to_le_bytes());
        for stockpile in &self.stockpiles {
            stockpile.write_canonical(&mut out);
        }
        out.extend_from_slice(&(self.agents.len() as u32).to_le_bytes());
        for (id, state) in &self.agents {
            out.extend_from_slice(&id.0.to_le_bytes());
            state.write_canonical(&mut out);
        }
        out
    }

    /// A 64-bit FNV-1a digest of [`World::canonical_bytes`] — a compact
    /// determinism check for cross-run equality.
    pub fn digest(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in self.canonical_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }
}

/// Deterministic partial Fisher–Yates sampling driven by the econ `Rng`.
///
/// This returns `count.min(width * height)` distinct positions in draw order
/// without allocating a full `width * height` position vector. A `BTreeMap`
/// stores the virtual swaps, preserving the crate's deterministic collection
/// discipline.
fn sample_positions(width: u16, height: u16, count: u32, rng: &mut Rng) -> Vec<Pos> {
    let tile_count = usize::from(width) * usize::from(height);
    let count = (count as usize).min(tile_count);
    let mut swaps: BTreeMap<usize, usize> = BTreeMap::new();
    let mut positions = Vec::with_capacity(count);

    for i in 0..count {
        let remaining = tile_count - i;
        let picked = i + (rng.next_u64() % remaining as u64) as usize;
        let picked_value = swaps.get(&picked).copied().unwrap_or(picked);
        let i_value = swaps.get(&i).copied().unwrap_or(i);
        swaps.insert(picked, i_value);
        positions.push(index_to_pos(width, picked_value));
    }

    positions
}

fn index_to_pos(width: u16, index: usize) -> Pos {
    let width = usize::from(width);
    Pos::new((index % width) as u16, (index / width) as u16)
}

#[cfg(test)]
mod tests {
    use super::{AgentStatus, Task, TickReport, World, WorldGen};
    use crate::grid::{Grid, Pos};
    use crate::node::ResourceNode;
    use crate::stockpile::{Stockpile, StockpileId};
    use econ::agent::AgentId;
    use econ::good::{FOOD, WOOD};

    fn open_world(w: u16, h: u16) -> World {
        World::new(Grid::new(w, h))
    }

    #[test]
    fn add_rejects_impassable_and_oob_placements() {
        let mut grid = Grid::new(4, 4);
        grid.set_impassable(Pos::new(1, 1));
        let mut world = World::new(grid);

        assert!(world
            .add_node(ResourceNode::new(Pos::new(1, 1), FOOD, 5, 0, 5))
            .is_err());
        assert!(world
            .add_stockpile(Stockpile::new(Pos::new(1, 1), 10))
            .is_err());
        assert!(world.add_agent(Pos::new(1, 1), 10, 1).is_err());
        assert!(world.add_agent(Pos::new(9, 9), 10, 1).is_err());
        // A passable tile is accepted.
        assert!(world.add_agent(Pos::new(0, 0), 10, 1).is_ok());
    }

    #[test]
    fn add_node_clamps_over_cap_stock_so_regen_cannot_destroy_goods() {
        // A node built via the public fields can carry `stock > cap`, bypassing
        // `ResourceNode::new`'s clamp. `add_node` must normalize it on insertion;
        // otherwise the first regen tick would clamp the excess away while
        // reporting `regenerated == 0`, destroying goods despite the report.
        let mut world = open_world(1, 1);
        let node = world
            .add_node(ResourceNode {
                pos: Pos::new(0, 0),
                good: FOOD,
                stock: 100,
                regen_per_tick: 0,
                cap: 10,
            })
            .unwrap();
        assert_eq!(
            world.node(node).unwrap().stock,
            10,
            "stock not clamped to cap"
        );

        let total_before = world.total_goods();
        let report = world.tick();
        assert_eq!(report.regenerated, 0);
        assert_eq!(report.net, 0);
        assert_eq!(
            world.total_goods(),
            total_before,
            "regen tick silently destroyed over-cap stock"
        );
        assert_eq!(world.node(node).unwrap().stock, 10);
    }

    #[test]
    fn assign_task_validates_targets() {
        let mut world = open_world(4, 4);
        let agent = world.add_agent(Pos::new(0, 0), 10, 1).unwrap();
        let node = world
            .add_node(ResourceNode::new(Pos::new(2, 2), FOOD, 5, 0, 5))
            .unwrap();

        assert!(world.assign_task(agent, Task::GoHarvest(node, 3)));
        assert!(world.assign_task(agent, Task::GoTo(Pos::new(3, 3))));
        // Unknown node / out-of-bounds GoTo / unknown agent are rejected.
        assert!(!world.assign_task(agent, Task::GoHarvest(crate::node::NodeId(9), 1)));
        assert!(!world.assign_task(agent, Task::GoTo(Pos::new(9, 9))));
        assert!(!world.assign_task(econ::agent::AgentId(123), Task::Idle));
    }

    #[test]
    fn canonical_bytes_include_task_payloads() {
        let mut base = open_world(6, 6);
        let agent = base.add_agent(Pos::new(0, 0), 10, 1).unwrap();
        let node = base
            .add_node(ResourceNode::new(Pos::new(4, 4), FOOD, 5, 0, 5))
            .unwrap();
        base.add_stockpile(Stockpile::new(Pos::new(1, 1), 10))
            .unwrap();
        base.add_stockpile(Stockpile::new(Pos::new(2, 2), 10))
            .unwrap();

        let mut a = base.clone();
        let mut b = base.clone();
        a.assign_task(agent, Task::GoTo(Pos::new(1, 0)));
        b.assign_task(agent, Task::GoTo(Pos::new(2, 0)));
        assert_ne!(a.canonical_bytes(), b.canonical_bytes());

        let mut a = base.clone();
        let mut b = base.clone();
        a.assign_task(agent, Task::GoHarvest(node, 1));
        b.assign_task(agent, Task::GoHarvest(node, 2));
        assert_ne!(a.digest(), b.digest());

        let mut a = base.clone();
        let mut b = base;
        a.assign_task(agent, Task::GoDeposit(StockpileId(0)));
        b.assign_task(agent, Task::GoDeposit(StockpileId(1)));
        assert_ne!(a.canonical_bytes(), b.canonical_bytes());
    }

    #[test]
    fn canonical_bytes_include_world_bookkeeping_and_last_report() {
        let a = open_world(2, 2);
        let mut b = a.clone();
        b.next_agent_index = 99;
        assert_ne!(a.canonical_bytes(), b.canonical_bytes());

        let c = open_world(2, 2);
        let mut d = c.clone();
        d.last_report = TickReport {
            harvested: 1,
            deposited: 2,
            regenerated: 3,
            net: 3,
        };
        assert_ne!(c.digest(), d.digest());
    }

    #[test]
    fn run_zero_returns_default_report_without_overwriting_last_report() {
        let mut world = open_world(1, 1);
        world
            .add_node(ResourceNode::new(Pos::new(0, 0), WOOD, 0, 3, 5))
            .unwrap();

        let prior = world.tick();
        assert_eq!(prior.regenerated, 3);
        assert_eq!(world.run(0), TickReport::default());
        assert_eq!(world.last_report(), prior);
    }

    #[test]
    fn move_speed_advances_multiple_steps_per_tick() {
        let mut world = open_world(10, 1);
        let agent = world.add_agent(Pos::new(0, 0), 10, 3).unwrap();
        world.assign_task(agent, Task::GoTo(Pos::new(9, 0)));
        world.tick();
        assert_eq!(world.agent_pos(agent), Some(Pos::new(3, 0)));
        world.tick();
        assert_eq!(world.agent_pos(agent), Some(Pos::new(6, 0)));
    }

    #[test]
    fn zero_speed_agent_with_target_blocks() {
        let mut world = open_world(3, 1);
        let agent = world.add_agent(Pos::new(0, 0), 10, 0).unwrap();
        world.assign_task(agent, Task::GoTo(Pos::new(2, 0)));

        world.tick();

        assert_eq!(world.agent_pos(agent), Some(Pos::new(0, 0)));
        assert_eq!(world.agent_status(agent), Some(AgentStatus::Blocked));
    }

    #[test]
    fn harvest_then_deposit_round_trip_conserves() {
        let mut world = open_world(5, 1);
        let agent = world.add_agent(Pos::new(0, 0), 10, 5).unwrap();
        let node = world
            .add_node(ResourceNode::new(Pos::new(2, 0), FOOD, 8, 0, 8))
            .unwrap();
        let sp = world
            .add_stockpile(Stockpile::new(Pos::new(4, 0), 100))
            .unwrap();
        let total_before = world.total_goods();

        // Harvest 5: distance 2, speed 5 → arrives and harvests same tick.
        world.assign_task(agent, Task::GoHarvest(node, 5));
        let report = world.tick();
        assert_eq!(report.harvested, 5);
        assert_eq!(world.agent_carry(agent, FOOD), 5);
        assert_eq!(world.node(node).unwrap().stock, 3);
        assert_eq!(world.total_goods(), total_before);

        // Deposit at the stockpile.
        world.assign_task(agent, Task::GoDeposit(sp));
        let report = world.tick();
        assert_eq!(report.deposited, 5);
        assert_eq!(world.stockpile_get(sp, FOOD), 5);
        assert_eq!(world.agent_carry(agent, FOOD), 0);
        assert_eq!(world.total_goods(), total_before);
    }

    #[test]
    fn withdraw_agent_carry_relocates_units_out_of_the_world() {
        // The G4a estate seam: a dead colonist's carried escrow is drained out of
        // the world (to the settlement commons). The withdraw removes up to the
        // requested amount and the world's total drops by exactly that — never more,
        // never destroyed in place.
        let mut world = open_world(3, 1);
        let agent = world.add_agent(Pos::new(0, 0), 10, 5).unwrap();
        let node = world
            .add_node(ResourceNode::new(Pos::new(1, 0), FOOD, 6, 0, 6))
            .unwrap();
        world.assign_task(agent, Task::GoHarvest(node, 6));
        world.tick();
        assert_eq!(world.agent_carry(agent, FOOD), 6);
        let total_before = world.total_goods();

        // A partial drain removes exactly the requested amount.
        assert_eq!(world.withdraw_agent_carry(agent, FOOD, 4), 4);
        assert_eq!(world.agent_carry(agent, FOOD), 2);
        assert_eq!(world.total_goods(), total_before - 4);

        // Over-asking drains only what remains; an unknown agent removes nothing.
        assert_eq!(world.withdraw_agent_carry(agent, FOOD, 99), 2);
        assert_eq!(world.agent_carry(agent, FOOD), 0);
        assert_eq!(world.withdraw_agent_carry(AgentId(999), FOOD, 1), 0);
        assert_eq!(world.total_goods(), total_before - 6);
    }

    #[test]
    fn remove_agent_returns_remaining_carry_and_forgets_agent() {
        let mut world = open_world(3, 1);
        let agent = world.add_agent(Pos::new(0, 0), 10, 5).unwrap();
        let node = world
            .add_node(ResourceNode::new(Pos::new(1, 0), FOOD, 6, 0, 6))
            .unwrap();
        world.assign_task(agent, Task::GoHarvest(node, 6));
        world.tick();
        let total_before = world.total_goods();

        let carry = world.remove_agent(agent).expect("agent removes");
        assert_eq!(carry.get(&FOOD), Some(&6));
        assert_eq!(world.agent_status(agent), None);
        assert_eq!(world.agent_carry(agent, FOOD), 0);
        assert_eq!(
            world.total_goods(),
            total_before - 6,
            "returned carry has relocated out of the world"
        );
        assert!(world.remove_agent(agent).is_none());
    }

    #[test]
    fn deposit_overflow_stays_carried() {
        let mut world = open_world(3, 1);
        let agent = world.add_agent(Pos::new(0, 0), 10, 5).unwrap();
        let node = world
            .add_node(ResourceNode::new(Pos::new(1, 0), FOOD, 6, 0, 6))
            .unwrap();
        let sp = world
            .add_stockpile(Stockpile::new(Pos::new(2, 0), 4))
            .unwrap();

        world.assign_task(agent, Task::GoHarvest(node, 6));
        world.tick();
        assert_eq!(world.agent_carry(agent, FOOD), 6);

        world.assign_task(agent, Task::GoDeposit(sp));
        world.tick();
        // Only 4 fit; 2 stay carried, nothing destroyed.
        assert_eq!(world.stockpile_get(sp, FOOD), 4);
        assert_eq!(world.agent_carry(agent, FOOD), 2);
    }

    #[test]
    fn regen_is_accounted_and_capped() {
        let mut world = open_world(2, 1);
        let node = world
            .add_node(ResourceNode::new(Pos::new(0, 0), WOOD, 8, 5, 10))
            .unwrap();
        let report = world.tick();
        assert_eq!(report.regenerated, 2); // 8 -> 10 (capped)
        assert_eq!(report.net, 2);
        assert_eq!(world.node(node).unwrap().stock, 10);

        let report = world.tick();
        assert_eq!(report.regenerated, 0); // already at cap
        assert_eq!(report.net, 0);
    }

    #[test]
    fn stockpile_withdraw_is_the_only_world_sink() {
        // Harvest then deposit into a stockpile (conserving), then withdraw —
        // the withdraw is the one path that lowers the world total, by exactly
        // the amount removed. This is the world side of the G2b transfer seam.
        let mut world = open_world(3, 1);
        let agent = world.add_agent(Pos::new(0, 0), 10, 5).unwrap();
        let node = world
            .add_node(ResourceNode::new(Pos::new(1, 0), FOOD, 7, 0, 7))
            .unwrap();
        let sp = world
            .add_stockpile(Stockpile::new(Pos::new(2, 0), 100))
            .unwrap();
        world.assign_task(agent, Task::GoHarvest(node, 7));
        world.tick();
        world.assign_task(agent, Task::GoDeposit(sp));
        world.tick();
        assert_eq!(world.stockpile_get(sp, FOOD), 7);
        let total_before = world.total_goods();

        // Withdraw 5: the world total drops by exactly 5; an unknown stockpile
        // is a deterministic no-op.
        assert_eq!(world.stockpile_withdraw(sp, FOOD, 5), 5);
        assert_eq!(world.stockpile_get(sp, FOOD), 2);
        assert_eq!(world.total_goods(), total_before - 5);
        assert_eq!(world.stockpile_withdraw(StockpileId(9), FOOD, 1), 0);
        // Over-withdraw clamps to what is held.
        assert_eq!(world.stockpile_withdraw(sp, FOOD, 9), 2);
        assert_eq!(world.total_goods(), total_before - 7);
    }

    #[test]
    fn generate_is_deterministic() {
        let gen = WorldGen::demo();
        let a = World::generate(99, &gen);
        let b = World::generate(99, &gen);
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
        assert_eq!(a.digest(), b.digest());
    }

    #[test]
    fn generate_places_everything_on_passable_tiles() {
        let gen = WorldGen::demo();
        let world = World::generate(7, &gen);
        for id in 0..world.node_count() {
            let node = world.node(crate::node::NodeId(id as u32)).unwrap();
            assert!(world.grid().is_passable(node.pos));
        }
        for id in 0..world.stockpile_count() {
            let sp = world
                .stockpile(crate::stockpile::StockpileId(id as u32))
                .unwrap();
            assert!(world.grid().is_passable(sp.pos));
        }
        for id in world.agent_ids() {
            assert!(world.grid().is_passable(world.agent_pos(id).unwrap()));
        }
    }
}
