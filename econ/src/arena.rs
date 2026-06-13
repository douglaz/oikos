//! Stable-identity agent storage with generational slot reuse.
//!
//! G0b migration (game-spec §11): identity must survive a changing cast so
//! demography (G4) can kill and birth colonists without dangling references.
//! `AgentArena` replaces `Society`'s `Vec<Agent>` + `agent_order` with the
//! invariant that a **never-freeing population behaves exactly like the old
//! `Vec`** (the lab case) — agents stored in insertion (cast) order, positional
//! access preserved, deterministic structure, no `HashMap`. On top of that it
//! adds id-resolution, id-ordered iteration, and — for the future demography
//! path — slot reuse with generation bumping so a stale `AgentId` resolves to
//! `None`. No engine path frees an agent in G0b; `free`/`insert` are exercised
//! by unit tests only (see `docs/engine-divergence.md`).

use std::collections::BTreeMap;
use std::ops::{Index, IndexMut};

use crate::agent::{Agent, AgentId};

/// Read-only lookup by stable agent id.
pub trait AgentLookup {
    fn get_agent(&self, id: AgentId) -> Option<&Agent>;
}

impl AgentLookup for AgentArena {
    fn get_agent(&self, id: AgentId) -> Option<&Agent> {
        self.get(id)
    }
}

impl AgentLookup for [Agent] {
    fn get_agent(&self, id: AgentId) -> Option<&Agent> {
        self.iter().find(|agent| agent.id == id)
    }
}

// `Vec<Agent>` delegates to the slice impl. This is NOT redundant with deref:
// callers that pass a `&Vec<Agent>` into a generic `A: AgentLookup` bound need
// the trait on `Vec` directly (deref coercion does not apply to type-parameter
// bounds), and several `reserve_order` call sites do exactly that.
impl AgentLookup for Vec<Agent> {
    fn get_agent(&self, id: AgentId) -> Option<&Agent> {
        self.as_slice().get_agent(id)
    }
}

/// Id-stable agent storage. Live agents live densely in insertion order (the
/// lab's cast order); a `BTreeMap` keyed by numeric index resolves ids and a
/// per-index generation makes reuse observable.
#[derive(Clone, Debug)]
pub struct AgentArena {
    /// Live agents in insertion order. For the never-freeing lab population
    /// this is bit-for-bit today's `Society.agents: Vec<Agent>`.
    agents: Vec<Agent>,
    /// numeric index → position in `agents`, live agents only.
    index_of: BTreeMap<u32, usize>,
    /// live id → position in `agents`, in full `AgentId` order.
    live_order: BTreeMap<AgentId, usize>,
    /// numeric index → current generation, for every index ever used (live or
    /// tombstoned). A live id matches `generation_of[index]`.
    generation_of: BTreeMap<u32, u32>,
    /// Freed numeric indices available for reuse by [`AgentArena::insert`].
    free_list: Vec<u32>,
    /// Next fresh numeric index for [`AgentArena::insert`] when no slot is free.
    /// `None` means the fresh-id space is exhausted; freed slots may still be
    /// reused through `free_list`.
    next_index: Option<u32>,
}

impl Default for AgentArena {
    fn default() -> Self {
        Self {
            agents: Vec::new(),
            index_of: BTreeMap::new(),
            live_order: BTreeMap::new(),
            generation_of: BTreeMap::new(),
            free_list: Vec::new(),
            next_index: Some(0),
        }
    }
}

impl AgentArena {
    /// An empty arena.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from a cast authored in insertion order (the lab path): every
    /// agent keeps its authored id and generation, never freed.
    pub fn from_cast(agents: Vec<Agent>) -> Self {
        let mut arena = Self::new();
        for agent in agents {
            arena.insert_with_id(agent);
        }
        arena
    }

    /// Number of live agents.
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Whether there are no live agents.
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    /// The live agents as a contiguous slice, in insertion (cast) order — the
    /// view the engine's per-tick modules consume. The lab never frees, so this
    /// is always dense.
    pub fn as_slice(&self) -> &[Agent] {
        &self.agents
    }

    /// The live agents as a mutable contiguous slice, insertion order.
    pub fn as_mut_slice(&mut self) -> &mut [Agent] {
        &mut self.agents
    }

    /// The position of a live id in [`AgentArena::as_slice`], or `None` if the
    /// id is unknown or stale (a bumped generation).
    pub fn position_of(&self, id: AgentId) -> Option<usize> {
        let numeric = id.index();
        let &position = self.index_of.get(&numeric)?;
        if self.generation_of.get(&numeric).copied() == Some(id.generation()) {
            Some(position)
        } else {
            None
        }
    }

    /// The agent for `id`, or `None` on an unknown or stale (freed/reused) id.
    pub fn get(&self, id: AgentId) -> Option<&Agent> {
        self.position_of(id).map(|position| &self.agents[position])
    }

    /// Mutable access to the agent for `id`, or `None` on a stale id.
    pub fn get_mut(&mut self, id: AgentId) -> Option<&mut Agent> {
        self.position_of(id)
            .map(move |position| &mut self.agents[position])
    }

    /// Live agents in ascending [`AgentId`] order. For the lab (every id is
    /// generation 0) this is ascending numeric index — exactly the legacy
    /// id-sorted iteration order.
    ///
    /// NOTE: this is **id order**, not [`AgentArena::as_slice`]'s insertion
    /// (cast) order. The two coincide for the lab (agents are authored in id
    /// order, never freed), but an order-sensitive caller must pick
    /// deliberately: `iter()` for stable id order, `as_slice()` for cast order.
    pub fn iter(&self) -> impl Iterator<Item = &Agent> {
        self.live_order
            .values()
            .map(move |&position| &self.agents[position])
    }

    /// Insert an agent that already carries its authored id (the lab cast path:
    /// sparse authored ids, generation 0, never freed).
    pub fn insert_with_id(&mut self, agent: Agent) {
        let numeric = agent.id.index();
        let generation = agent.id.generation();
        assert!(
            !self.index_of.contains_key(&numeric),
            "duplicate live agent index {numeric}"
        );
        if let Some(recorded_generation) = self.generation_of.get(&numeric).copied() {
            assert!(
                generation >= recorded_generation,
                "stale agent id generation {generation} for index {numeric}; recorded generation is {recorded_generation}"
            );
            assert!(
                generation != recorded_generation || self.free_list.contains(&numeric),
                "retired agent index {numeric} at generation {recorded_generation} cannot be reinserted"
            );
        }
        let position = self.agents.len();
        if let Some(freed_position) = self.free_list.iter().position(|&slot| slot == numeric) {
            self.free_list.swap_remove(freed_position);
        }
        self.index_of.insert(numeric, position);
        self.live_order.insert(agent.id, position);
        self.generation_of.insert(numeric, generation);
        if self.next_index.is_some_and(|next| numeric >= next) {
            self.next_index = numeric.checked_add(1);
        }
        self.agents.push(agent);
    }

    /// Insert an agent into a fresh or reused slot, assigning and returning its
    /// id. A reused slot carries the generation bumped at [`AgentArena::free`].
    pub fn insert(&mut self, mut agent: Agent) -> AgentId {
        let (numeric, generation) = if let Some(numeric) = self.free_list.pop() {
            let generation = self.generation_of.get(&numeric).copied().unwrap_or(0);
            (numeric, generation)
        } else {
            let numeric = self.next_index.expect("agent id index space exhausted");
            self.next_index = numeric.checked_add(1);
            (numeric, 0)
        };
        let id = AgentId::with_generation(numeric, generation);
        agent.id = id;
        let position = self.agents.len();
        self.index_of.insert(numeric, position);
        self.live_order.insert(id, position);
        self.generation_of.insert(numeric, generation);
        self.agents.push(agent);
        id
    }

    /// Free a live agent, bumping its slot generation so stale ids resolve to
    /// `None` and the slot can be reused. Returns the removed agent, or `None`
    /// if the id was already stale or unknown.
    ///
    /// Removal is **order-preserving**: the surviving agents keep their relative
    /// insertion order in [`AgentArena::as_slice`], so the slice's documented
    /// cast-order invariant holds even after a free (no engine path frees in
    /// G0b; this matters for the demography path in G4). Every later agent
    /// shifts down one slot, and the index maps shift with them.
    pub fn free(&mut self, id: AgentId) -> Option<Agent> {
        let numeric = id.index();
        let position = self.position_of(id)?;
        self.index_of.remove(&numeric);
        self.live_order.remove(&id);
        // Bump the slot generation so the just-freed id resolves to `None` and a
        // reused slot is handed a fresh id. If the generation cannot advance (the
        // slot is already at `u32::MAX`), reusing it would have to reissue the
        // freed id — defeating stale-id detection (Reviewer-1 P2). Retire the
        // slot permanently instead: leave the `u32::MAX` tombstone in
        // `generation_of` and never offer the index through `free_list`, so the
        // freed id stays resolvable to `None` forever. This mirrors the
        // fresh-index exhaustion path, which also declines to hand out a slot
        // rather than corrupting identity.
        match id.generation().checked_add(1) {
            Some(bumped) => {
                self.generation_of.insert(numeric, bumped);
                self.free_list.push(numeric);
            }
            None => {
                self.generation_of.insert(numeric, u32::MAX);
            }
        }
        let agent = self.agents.remove(position);
        // Order-preserving removal slides every later live agent down one slot;
        // re-point the index maps so positions stay accurate.
        for slot in self.index_of.values_mut() {
            if *slot > position {
                *slot -= 1;
            }
        }
        for slot in self.live_order.values_mut() {
            if *slot > position {
                *slot -= 1;
            }
        }
        Some(agent)
    }
}

impl Index<usize> for AgentArena {
    type Output = Agent;

    fn index(&self, position: usize) -> &Agent {
        &self.agents[position]
    }
}

impl IndexMut<usize> for AgentArena {
    fn index_mut(&mut self, position: usize) -> &mut Agent {
        &mut self.agents[position]
    }
}

#[cfg(test)]
mod tests {
    use super::AgentArena;
    use crate::agent::{Agent, AgentId, Role};
    use crate::good::{Gold, Stock};

    fn agent(id: u32) -> Agent {
        Agent {
            id: AgentId(u64::from(id)),
            scale: Vec::new(),
            stock: Stock::new(3),
            gold: Gold::ZERO,
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Household],
            expect: Vec::new(),
        }
    }

    /// Tiny deterministic LCG — pure std, no `rand` dependency, reproducible.
    struct Lcg(u64);

    impl Lcg {
        fn next_u32(&mut self) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            (self.0 >> 32) as u32
        }
    }

    /// The legacy construction: agents in cast order, `agent_order` = positions
    /// sorted by id. Iteration in id order is the agents sorted by id.
    fn legacy_id_order(cast: &[Agent]) -> Vec<AgentId> {
        let mut order: Vec<usize> = (0..cast.len()).collect();
        order.sort_by_key(|&index| cast[index].id);
        order.into_iter().map(|index| cast[index].id).collect()
    }

    #[test]
    fn arena_matches_vec_semantics_when_nothing_dies() {
        let mut lcg = Lcg(0x0BAD_C0DE_F00D);

        for round in 0..32 {
            // Random sparse authored ids, deduplicated, in random cast order.
            let mut ids = Vec::new();
            let count = 1 + (lcg.next_u32() % 24) as usize;
            while ids.len() < count {
                let candidate = lcg.next_u32() % 500;
                if !ids.contains(&candidate) {
                    ids.push(candidate);
                }
            }

            let cast: Vec<Agent> = ids.iter().map(|&id| agent(id)).collect();
            let arena = AgentArena::from_cast(cast.clone());

            // Count equals the legacy Vec.
            assert_eq!(arena.len(), cast.len(), "round {round} count");

            // Iteration order equals the legacy Vec + id-sorted order.
            let arena_order: Vec<AgentId> = arena.iter().map(|a| a.id).collect();
            assert_eq!(arena_order, legacy_id_order(&cast), "round {round} order");

            // Lookups resolve exactly the authored agents; positions agree with
            // the dense slice.
            for &id in &ids {
                let agent_id = AgentId(u64::from(id));
                let found = arena.get(agent_id).expect("authored id resolves");
                assert_eq!(found.id, agent_id);
                let position = arena.position_of(agent_id).expect("position resolves");
                assert_eq!(arena.as_slice()[position].id, agent_id);
                assert_eq!(arena[position].id, agent_id);
            }

            // Generation-1 of any authored id is stale: it does not resolve.
            let probe = AgentId::with_generation(ids[0], 1);
            assert!(arena.get(probe).is_none(), "round {round} stale gen");
        }
    }

    #[test]
    fn arena_reuse_bumps_generation() {
        let mut arena = AgentArena::new();
        arena.insert_with_id(agent(2));
        arena.insert_with_id(agent(5));

        // Fresh insert takes the next unused numeric index.
        let fresh = arena.insert(agent(0));
        assert_eq!(fresh, AgentId::with_generation(6, 0));
        assert_eq!(arena.len(), 3);

        // Free it: the stale id stops resolving and the agent comes back out.
        let freed = arena.free(fresh).expect("fresh id frees");
        assert_eq!(freed.id, fresh);
        assert!(arena.get(fresh).is_none(), "stale id resolves to None");
        assert_eq!(arena.len(), 2);
        assert!(arena.iter().all(|a| a.id != fresh), "iteration drops freed");

        // Reinsert: the slot is reused with a bumped generation; the new id
        // resolves, the stale one still does not.
        let reused = arena.insert(agent(0));
        assert_eq!(reused, AgentId::with_generation(6, 1));
        assert_eq!(reused.index(), fresh.index());
        assert_eq!(reused.generation(), 1);
        assert!(arena.get(reused).is_some(), "new id resolves");
        assert!(
            arena.get(fresh).is_none(),
            "stale id stays None after reuse"
        );
        assert_eq!(arena.len(), 3);

        // Freeing an already-stale id is a no-op.
        assert!(arena.free(fresh).is_none());

        // Surviving authored agents are untouched and still resolve in id order.
        let order: Vec<u32> = arena.iter().map(|a| a.id.index()).collect();
        assert_eq!(order, vec![2, 5, 6]);
    }

    #[test]
    fn free_preserves_slice_order_for_a_middle_agent() {
        // Author four agents in cast order, then free a NON-last one. The
        // documented `as_slice` invariant is insertion order; an order-preserving
        // free must keep the survivors in that order (not move the tail in).
        let mut arena = AgentArena::new();
        for id in [10, 20, 30, 40] {
            arena.insert_with_id(agent(id));
        }
        assert_eq!(
            arena
                .as_slice()
                .iter()
                .map(|a| a.id.index())
                .collect::<Vec<_>>(),
            vec![10, 20, 30, 40]
        );

        // Free the second agent: survivors keep their relative cast order, the
        // tail is NOT swapped into the hole.
        let freed = arena.free(AgentId(20)).expect("middle id frees");
        assert_eq!(freed.id, AgentId(20));
        assert_eq!(
            arena
                .as_slice()
                .iter()
                .map(|a| a.id.index())
                .collect::<Vec<_>>(),
            vec![10, 30, 40],
            "free preserves insertion order in as_slice"
        );

        // Every surviving id still resolves, and its position agrees with the
        // (now shifted) slice — the index maps tracked the shift.
        for id in [10u32, 30, 40] {
            let agent_id = AgentId(u64::from(id));
            let position = arena.position_of(agent_id).expect("survivor resolves");
            assert_eq!(arena.as_slice()[position].id, agent_id);
            assert_eq!(arena.get(agent_id).map(|a| a.id), Some(agent_id));
        }
        assert!(arena.get(AgentId(20)).is_none(), "freed id is stale");

        // Iteration (id order) drops the freed agent too.
        assert_eq!(
            arena.iter().map(|a| a.id.index()).collect::<Vec<_>>(),
            vec![10, 30, 40]
        );
    }

    #[test]
    #[should_panic(expected = "duplicate live agent index 7")]
    fn insert_with_id_rejects_duplicate_live_index() {
        let mut arena = AgentArena::new();
        arena.insert_with_id(agent(7));
        arena.insert_with_id(agent(7));
    }

    #[test]
    fn insert_with_id_rejects_tombstoned_stale_generation_without_corruption() {
        let mut arena = AgentArena::new();
        arena.insert_with_id(agent(7));

        let freed = arena.free(AgentId(7)).expect("id frees");
        assert_eq!(freed.id, AgentId(7));
        assert!(arena.get(AgentId(7)).is_none(), "stale id is tombstoned");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            arena.insert_with_id(agent(7));
        }));

        assert!(result.is_err(), "stale explicit insert must reject");
        assert_eq!(arena.len(), 0, "failed insert leaves no live agent");
        assert!(arena.get(AgentId(7)).is_none(), "stale id stays rejected");

        let reused = arena.insert(agent(99));
        assert_eq!(reused, AgentId::with_generation(7, 1));
        assert!(
            arena.get(AgentId(7)).is_none(),
            "old generation stays stale"
        );
        assert_eq!(arena.get(reused).map(|agent| agent.id), Some(reused));
    }

    #[test]
    fn fresh_insert_after_live_max_id_panics_without_corruption() {
        let mut arena = AgentArena::new();
        arena.insert_with_id(agent(u32::MAX));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            arena.insert(agent(0));
        }));

        assert!(result.is_err(), "fresh id exhaustion must reject");
        assert_eq!(arena.len(), 1);
        assert_eq!(
            arena
                .get(AgentId(u64::from(u32::MAX)))
                .map(|agent| agent.id),
            Some(AgentId(u64::from(u32::MAX)))
        );
        assert_eq!(
            arena
                .iter()
                .map(|agent| agent.id.index())
                .collect::<Vec<_>>(),
            vec![u32::MAX]
        );
    }

    #[test]
    fn free_at_max_generation_retires_slot_without_reissuing_id() {
        // A slot whose generation is already `u32::MAX` cannot advance on free.
        // The arena must retire it rather than reuse it and reissue an identical
        // `AgentId` — that would make the stale handle resolve to the new agent.
        let mut arena = AgentArena::new();
        let exhausted = AgentId::with_generation(3, u32::MAX);
        let mut maxed = agent(3);
        maxed.id = exhausted;
        arena.insert_with_id(maxed);
        assert_eq!(arena.get(exhausted).map(|a| a.id), Some(exhausted));

        // Free succeeds and returns the agent, but the slot's generation is at
        // the ceiling, so it is retired (not pushed onto the free list).
        let freed = arena.free(exhausted).expect("max-generation id frees");
        assert_eq!(freed.id, exhausted);
        assert!(
            arena.get(exhausted).is_none(),
            "freed max-generation id resolves to None"
        );
        assert_eq!(arena.len(), 0);

        // The explicit-id insertion path must not revive the retired stale id.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut explicit = agent(3);
            explicit.id = exhausted;
            arena.insert_with_id(explicit);
        }));
        assert!(result.is_err(), "retired explicit id must reject");
        assert_eq!(arena.len(), 0, "failed insert leaves the arena empty");
        assert!(
            arena.get(exhausted).is_none(),
            "retired id stays stale after explicit insert attempt"
        );

        // A fresh insert must NOT reuse the retired slot: it takes a fresh
        // numeric index, so the freed id can never be reissued and stays stale.
        let reused = arena.insert(agent(0));
        assert_ne!(reused, exhausted, "retired slot is never reused");
        assert_eq!(
            reused.index(),
            4,
            "fresh insert advances past the retired index"
        );
        assert!(
            arena.get(exhausted).is_none(),
            "stale max-generation id stays None after a later insert"
        );
        assert_eq!(arena.get(reused).map(|a| a.id), Some(reused));
    }

    #[test]
    fn fresh_insert_can_use_max_id_once_before_exhaustion() {
        let mut arena = AgentArena::new();
        arena.insert_with_id(agent(u32::MAX - 1));

        let max = arena.insert(agent(0));

        assert_eq!(max, AgentId(u64::from(u32::MAX)));
        assert_eq!(arena.len(), 2);
        assert_eq!(arena.get(max).map(|agent| agent.id), Some(max));
    }
}
