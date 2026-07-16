//! Money and provenance accounting types.
//!
//! The earned-gold FIFO buckets, produced-bread provenance ledger, multi-good money
//! accounting, food-acquisition channel tally, and the acquisition ledger. Extracted
//! verbatim from `mod.rs` (pure code motion); the types become `pub(super)` and are
//! re-imported by the parent via `use accounting::*`. Their associated value types
//! (EarnedGoldLot, ProducedLot, ...) stay in `mod.rs` and are seen here via `use super::*`.
//! Field/method visibilities were widened to `pub(super)` only where the parent or a
//! sibling module already reached them — preserving the exact pre-extraction scope.

use super::*;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct EarnedGoldBuckets {
    pub(super) earned: VecDeque<Gold>,
    pub(super) endowed: VecDeque<Gold>,
}
impl EarnedGoldBuckets {
    pub(super) fn credit(&mut self, lot: EarnedGoldLot) {
        if lot.amount == Gold::ZERO {
            return;
        }
        match lot.source {
            EarnedGoldSource::Earned => self.earned.push_back(lot.amount),
            EarnedGoldSource::Endowed => self.endowed.push_back(lot.amount),
        }
    }

    pub(super) fn debit(&mut self, mut amount: Gold) -> (Gold, Gold, Gold) {
        let earned = Self::debit_queue(&mut self.earned, &mut amount);
        let endowed = Self::debit_queue(&mut self.endowed, &mut amount);
        (earned, endowed, amount)
    }

    pub(super) fn debit_lots(&mut self, mut amount: Gold) -> (Vec<EarnedGoldLot>, Gold) {
        let mut lots = Vec::new();
        let earned = Self::debit_queue(&mut self.earned, &mut amount);
        if earned > Gold::ZERO {
            lots.push(EarnedGoldLot {
                source: EarnedGoldSource::Earned,
                amount: earned,
            });
        }
        let endowed = Self::debit_queue(&mut self.endowed, &mut amount);
        if endowed > Gold::ZERO {
            lots.push(EarnedGoldLot {
                source: EarnedGoldSource::Endowed,
                amount: endowed,
            });
        }
        (lots, amount)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.earned.is_empty() && self.endowed.is_empty()
    }

    pub(super) fn debit_queue(queue: &mut VecDeque<Gold>, amount: &mut Gold) -> Gold {
        let mut debited = Gold::ZERO;
        while *amount > Gold::ZERO {
            let Some(front) = queue.front_mut() else {
                break;
            };
            let take = (*front).min(*amount);
            *front = (*front).saturating_sub(take);
            *amount = (*amount).saturating_sub(take);
            debited = debited.saturating_add(take);
            if *front == Gold::ZERO {
                queue.pop_front();
            }
        }
        debited
    }
}
/// S16: the **produced-bread provenance ledger** — a per-agent, stock-origin balance that
/// classifies a bread→medium trade as **produced** (the seller's bread was cultivated, the
/// claim S16 makes) vs **minted/residual** (seeded buffer or a hearth mint). Role/
/// cultivating-state at trade time is unsound (S15 bread is produced post-market and sold a
/// LATER tick when `cultivating` may be false, and a consumer can resell bought bread), so
/// provenance must follow the STOCK ORIGIN, not the role (Base Fact 8).
///
/// The ledger stores one counter per agent — `produced[id]`, the produced-origin bread that
/// agent currently holds. The other-origin (minted/residual) bread an agent holds is the
/// residual `stock(bread) − produced[id]`, so the two-counter (produced vs other) split is
/// represented without a second map and is conserved by construction: every bread DEBIT
/// draws produced-origin FIRST (deterministic produced-first priority, in stock-removal
/// order; never proportional, and not first-in-first-out by acquisition order), so
/// `produced[id] ≤ stock(bread)` holds for every living agent.
///
/// - **Credit** `produced` when a producer books bread `produced` (cultivation or a chain
///   baker). A MINT (demography/producer-subsistence hearth) or a seeded buffer is NOT
///   credited, so it falls into the residual other-origin pool automatically.
/// - **Debit** produced-first when bread leaves an agent's stock: a SINK (eaten, spoiled,
///   or estate→commons) draws to `produced_sunk`; a TRANSFER (sale, birth endowment, or
///   estate→heir) moves the drawn produced units to the receiver, preserving origin (so a
///   resold produced loaf stays produced, and a resold MINTED loaf stays minted — the
///   resold-bought-bread case is not mis-attributed).
/// - A bread→medium trade's bread is **produced** to the extent the seller's debit draws
///   produced, else **minted**.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct BreadProvenance {
    /// Produced-origin bread each living agent currently holds. `stock(bread) − produced`
    /// is the agent's other-origin (minted/residual) bread.
    pub(super) produced: BTreeMap<AgentId, u64>,
    /// Whole-run conservation accumulators. `produced_credited`: produced bread ever booked
    /// by a production event. `produced_sunk`: produced bread ever removed by a true sink
    /// (eaten/spoiled/estate→commons). Transfers move produced units between agents and
    /// touch neither, so `produced_credited == produced_sunk + Σ produced[id]` holds.
    pub(super) produced_credited: u64,
    pub(super) produced_sunk: u64,
    /// Cumulative bread→medium trade volume attributed by stock origin.
    pub(super) salt_volume_produced: u64,
    pub(super) salt_volume_minted: u64,
    /// The same split, accumulated only on pre-promotion ticks (frozen at the promotion
    /// tick, inclusive) — the causality probe for whether produced bread drove a promotion.
    pub(super) pre_promotion_salt_volume_produced: u64,
    pub(super) pre_promotion_salt_volume_minted: u64,
    /// Instrumentation (diagnostic): the first econ tick a produced surplus was held, and
    /// the first econ tick a produced bread→medium trade cleared.
    pub(super) first_produced_surplus_tick: Option<u64>,
    pub(super) first_produced_bread_for_salt_tick: Option<u64>,
    /// S22a (runtime-only, NOT digested): per-agent FIFO of produced-bread lots tagged by the
    /// PRODUCER's class, mirroring `produced` exactly (same credit/draw/transfer/sink order),
    /// so a bread→SALT sale's produced units can be split lineage vs non-lineage by who
    /// cultivated them — the production-time entrant-class provenance.
    pub(super) produced_lots: BTreeMap<AgentId, VecDeque<ProducedLot>>,
    /// S22a: cumulative produced bread→SALT volume split by producer class (whole-run), plus
    /// the pre-promotion-only split (frozen at the promotion tick). Their sums equal the
    /// produced totals above.
    pub(super) salt_volume_produced_lineage: u64,
    pub(super) salt_volume_produced_nonlineage: u64,
    pub(super) pre_promotion_salt_volume_produced_lineage: u64,
    pub(super) pre_promotion_salt_volume_produced_nonlineage: u64,
    /// S22a: distinct producers whose `SelfProduced` bread reached a bread→SALT sale, split by
    /// class — the count of non-lineage ENTRANTS that actually monetized their cultivated bread.
    pub(super) lineage_salt_producers: BTreeSet<AgentId>,
    pub(super) nonlineage_salt_producers: BTreeSet<AgentId>,
}
impl BreadProvenance {
    /// Credit `qty` produced-origin bread to `agent` (a production event). `lineage` is the
    /// producer's class at production time (S22a), recorded in the class-tagged lot.
    pub(super) fn credit_produced(&mut self, agent: AgentId, qty: u64, lineage: bool) {
        if qty == 0 {
            return;
        }
        *self.produced.entry(agent).or_insert(0) += qty;
        self.produced_credited += qty;
        self.produced_lots
            .entry(agent)
            .or_default()
            .push_back(ProducedLot {
                producer: agent,
                lineage,
                qty,
            });
    }

    /// Pop `qty` of `agent`'s produced-class lots FIFO, returning the drawn lots (their sum is
    /// `min(qty, held)`). Keeps `produced_lots` in lockstep with the flat `produced` balance.
    pub(super) fn pop_lots(&mut self, agent: AgentId, mut qty: u64) -> Vec<ProducedLot> {
        let mut drawn = Vec::new();
        let Some(queue) = self.produced_lots.get_mut(&agent) else {
            return drawn;
        };
        while qty > 0 {
            let Some(front) = queue.front_mut() else {
                break;
            };
            let take = front.qty.min(qty);
            drawn.push(ProducedLot {
                producer: front.producer,
                lineage: front.lineage,
                qty: take,
            });
            front.qty -= take;
            qty -= take;
            if front.qty == 0 {
                queue.pop_front();
            }
        }
        if queue.is_empty() {
            self.produced_lots.remove(&agent);
        }
        drawn
    }

    /// Draw `qty` of `agent`'s bread, produced-origin FIRST; returns the class-tagged lots
    /// drawn (in FIFO order, summing to the produced units drawn). The caller decides whether
    /// the drawn units are a sink or a transfer.
    pub(super) fn draw(&mut self, agent: AgentId, qty: u64) -> Vec<ProducedLot> {
        let held = self.produced.get(&agent).copied().unwrap_or(0);
        let drawn = held.min(qty);
        if drawn > 0 {
            let remaining = held - drawn;
            if remaining == 0 {
                self.produced.remove(&agent);
            } else {
                self.produced.insert(agent, remaining);
            }
        }
        self.pop_lots(agent, drawn)
    }

    /// A SINK debit (eaten/spoiled/estate→commons): draw produced-first to `produced_sunk`.
    /// Returns the produced units sunk.
    pub(super) fn sink(&mut self, agent: AgentId, qty: u64) -> u64 {
        let drawn: u64 = self.draw(agent, qty).iter().map(|lot| lot.qty).sum();
        self.produced_sunk += drawn;
        drawn
    }

    /// A TRANSFER debit (sale/endowment/inheritance): draw produced-first from `from` and
    /// credit the same produced units (with their producer-class lots) to `to`, preserving
    /// origin. Returns the class-tagged lots moved (the produced share of the transfer).
    pub(super) fn transfer(&mut self, from: AgentId, to: AgentId, qty: u64) -> Vec<ProducedLot> {
        let lots = self.draw(from, qty);
        let drawn: u64 = lots.iter().map(|lot| lot.qty).sum();
        if drawn > 0 {
            *self.produced.entry(to).or_insert(0) += drawn;
            let queue = self.produced_lots.entry(to).or_default();
            for lot in &lots {
                queue.push_back(*lot);
            }
        }
        lots
    }

    /// A filtered TRANSFER debit for C1N wage advances: move only self-produced lots
    /// (`producer == from`) from `from` to `to`. Unlike [`Self::transfer`], this never draws
    /// produced bread the sender bought or inherited from someone else.
    pub(super) fn transfer_self_produced(&mut self, from: AgentId, to: AgentId, qty: u64) -> u64 {
        if qty == 0 {
            return 0;
        }
        let mut remaining = qty;
        let mut moved = Vec::new();
        let queue_empty = {
            let Some(queue) = self.produced_lots.get_mut(&from) else {
                return 0;
            };
            let mut kept = VecDeque::new();
            while let Some(mut lot) = queue.pop_front() {
                if lot.producer == from && remaining > 0 {
                    let take = lot.qty.min(remaining);
                    moved.push(ProducedLot {
                        producer: lot.producer,
                        lineage: lot.lineage,
                        qty: take,
                    });
                    lot.qty -= take;
                    remaining -= take;
                }
                if lot.qty > 0 {
                    kept.push_back(lot);
                }
            }
            *queue = kept;
            queue.is_empty()
        };
        if queue_empty {
            self.produced_lots.remove(&from);
        }
        let moved_qty: u64 = moved.iter().map(|lot| lot.qty).sum();
        if moved_qty == 0 {
            return 0;
        }
        let held = self.produced.get(&from).copied().unwrap_or(0);
        let remaining_held = held.saturating_sub(moved_qty);
        if remaining_held == 0 {
            self.produced.remove(&from);
        } else {
            self.produced.insert(from, remaining_held);
        }
        *self.produced.entry(to).or_insert(0) += moved_qty;
        self.produced_lots.entry(to).or_default().extend(moved);
        moved_qty
    }

    /// Move the newest self-produced lots from `from` to `to`. C1N uses this immediately
    /// after own-use conversion credits the worker's contract crop, so the split transfers
    /// the newly produced product rather than older produced bread the worker already held.
    pub(super) fn transfer_recent_self_produced(
        &mut self,
        from: AgentId,
        to: AgentId,
        qty: u64,
    ) -> u64 {
        if qty == 0 {
            return 0;
        }
        let mut remaining = qty;
        let mut moved = Vec::new();
        let queue_empty = {
            let Some(queue) = self.produced_lots.get_mut(&from) else {
                return 0;
            };
            while remaining > 0 {
                let pop_empty = {
                    let Some(back) = queue.back_mut() else {
                        break;
                    };
                    if back.producer != from {
                        break;
                    }
                    let take = back.qty.min(remaining);
                    moved.push(ProducedLot {
                        producer: back.producer,
                        lineage: back.lineage,
                        qty: take,
                    });
                    back.qty -= take;
                    remaining -= take;
                    back.qty == 0
                };
                if pop_empty {
                    queue.pop_back();
                }
            }
            queue.is_empty()
        };
        if queue_empty {
            self.produced_lots.remove(&from);
        }
        let moved_qty: u64 = moved.iter().map(|lot| lot.qty).sum();
        if moved_qty == 0 {
            return 0;
        }
        let held = self.produced.get(&from).copied().unwrap_or(0);
        let remaining_held = held.saturating_sub(moved_qty);
        if remaining_held == 0 {
            self.produced.remove(&from);
        } else {
            self.produced.insert(from, remaining_held);
        }
        *self.produced.entry(to).or_insert(0) += moved_qty;
        let queue = self.produced_lots.entry(to).or_default();
        for lot in moved.into_iter().rev() {
            queue.push_back(lot);
        }
        moved_qty
    }

    /// Attribute a cleared bread→medium trade by STOCK ORIGIN: the `drawn_lots` the seller's
    /// debit drew are PRODUCED, the residual `qty - Σ drawn_lots` is MINTED (seeded buffer / a
    /// hearth mint). Accumulates the run-total split, the pre-promotion-only split (the
    /// causality probe, frozen at the promotion tick), latches the first produced bread→medium
    /// tick once, and (S22a) splits the produced volume + distinct sellers by PRODUCER class.
    /// `tick` is the current econ tick for the latch.
    pub(super) fn attribute_medium_sale(
        &mut self,
        drawn_lots: &[ProducedLot],
        qty: u64,
        was_pre_promotion: bool,
        tick: u64,
    ) {
        let drawn_produced: u64 = drawn_lots.iter().map(|lot| lot.qty).sum();
        let minted = qty - drawn_produced;
        self.salt_volume_produced += drawn_produced;
        self.salt_volume_minted += minted;
        if was_pre_promotion {
            self.pre_promotion_salt_volume_produced += drawn_produced;
            self.pre_promotion_salt_volume_minted += minted;
        }
        if drawn_produced > 0 && self.first_produced_bread_for_salt_tick.is_none() {
            self.first_produced_bread_for_salt_tick = Some(tick);
        }
        // S22a: split the produced volume + distinct producers by the entrant class recorded
        // at PRODUCTION time (the lot's `lineage`), not the seller's trade-time state.
        for lot in drawn_lots {
            if lot.qty == 0 {
                continue;
            }
            if lot.lineage {
                self.salt_volume_produced_lineage += lot.qty;
                if was_pre_promotion {
                    self.pre_promotion_salt_volume_produced_lineage += lot.qty;
                }
                self.lineage_salt_producers.insert(lot.producer);
            } else {
                self.salt_volume_produced_nonlineage += lot.qty;
                if was_pre_promotion {
                    self.pre_promotion_salt_volume_produced_nonlineage += lot.qty;
                }
                self.nonlineage_salt_producers.insert(lot.producer);
            }
        }
    }

    /// Drop a removed agent's produced balance to a sink (the conserved exit when its bread
    /// could not be routed to a living heir — estate→commons). Returns the sunk units.
    pub(super) fn drop_to_sink(&mut self, agent: AgentId) -> u64 {
        let held = self.produced.remove(&agent).unwrap_or(0);
        self.produced_lots.remove(&agent);
        self.produced_sunk += held;
        held
    }

    /// Total produced-origin bread held across all living agents.
    pub(super) fn total_held(&self) -> u64 {
        self.produced.values().copied().sum()
    }
}
/// S18: runtime-only multi-good money instrumentation — NOT serialized into
/// `canonical_bytes` (diagnostic/proof state, like S17's `starvation_deaths_total`), so it
/// shifts no digest and every existing golden is byte-identical. Maintained only while
/// [`Settlement::multigood_money_active`] holds; the empty default otherwise.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct MultigoodMoney {
    /// Cumulative WOOD relocated node→econ (the gather bound for traded WOOD). With every
    /// WOOD buffer + the WOOD mint zeroed, all WOOD enters the economy through this haul, so
    /// the total WOOD stock can never exceed it. The asserted bound is the WOOD↔medium leg:
    /// `pre_promotion_wood_for_salt <= wood_gathered` — each gathered unit is sold to the
    /// medium at most once because the buyer consumes it for warmth (no recirculation), so
    /// the salt-leg volume cannot exceed the gather. The WOOD provenance bound (Codex P1a),
    /// the WOOD analogue of the produced-bread provenance ledger.
    pub(super) wood_gathered: u64,
    /// Cumulative WOOD↔medium (SALT) trade volume, and the pre-promotion share (frozen at
    /// the promotion tick, inclusive) — the WOOD leg of the indirect exchange.
    pub(super) wood_for_salt: u64,
    pub(super) pre_promotion_wood_for_salt: u64,
    /// The **pending-indirect-SALT round-trip ledger** (Codex P1c). Per `(agent, target)`:
    /// the medium accepted `IndirectFor{target}` but not yet spent on that target. Credited
    /// when an agent accepts the medium as a MEANS to `target`; decremented when it later
    /// trades medium→target. Tracing the actual sequence is stronger than net-acquiring the
    /// target (which could come from direct barter, buffers, or an estate). Maintained
    /// whenever a barter medium exists, so it works on any emergent economy.
    pub(super) pending: BTreeMap<(AgentId, GoodId), u64>,
    /// Cumulative medium accepted `IndirectFor{target}` (the round-trip denominator).
    pub(super) indirect_accepted: u64,
    /// Cumulative pending-indirect medium later SPENT on its earmarked target (the round-trip
    /// numerator — the means role completing, the medium actually intermediating).
    pub(super) indirect_spent_on_target: u64,
}
impl MultigoodMoney {
    /// Credit pending medium an agent accepted as a means to `target`.
    pub(super) fn credit_indirect(&mut self, agent: AgentId, target: GoodId, qty: u64) {
        *self.pending.entry((agent, target)).or_insert(0) += qty;
        self.indirect_accepted = self.indirect_accepted.saturating_add(qty);
    }

    /// Decrement when an agent spends earmarked medium on its target — the round-trip leg
    /// completing. Draws the lesser of the spend and the standing pending (only the means
    /// role completes; a spend beyond it is ordinary spending, not a round-trip).
    pub(super) fn spend_on_target(&mut self, agent: AgentId, target: GoodId, qty: u64) {
        if let Some(pending) = self.pending.get_mut(&(agent, target)) {
            let drawn = (*pending).min(qty);
            *pending -= drawn;
            if *pending == 0 {
                self.pending.remove(&(agent, target));
            }
            self.indirect_spent_on_target = self.indirect_spent_on_target.saturating_add(drawn);
        }
    }

    /// The standing pending-indirect medium an agent still holds earmarked for `target`.
    pub(super) fn pending_of(&self, agent: AgentId, target: GoodId) -> u64 {
        self.pending.get(&(agent, target)).copied().unwrap_or(0)
    }

    /// The round-trip fraction in basis points: of the medium accepted as a means, the share
    /// later spent on its target. Post-promotion spot trades record target-good quantity
    /// rather than the original medium units, so the pending cap makes this a conservative
    /// completion metric, not a price-denominated exact ratio. `0` when nothing was
    /// accepted indirectly (no division by zero) — distinct from "accepted but hoarded"
    /// (accepted > 0, spent ≈ 0).
    pub(super) fn round_trip_fraction_bps(&self) -> u32 {
        if self.indirect_accepted == 0 {
            return 0;
        }
        ((u128::from(self.indirect_spent_on_target) * 10_000) / u128::from(self.indirect_accepted))
            as u32
    }
}
/// S21d.1: the **acquisition channel** a tracked-food (bread) unit entered an agent's stock
/// by — the mutually-exclusive ways food can reach a colonist. A FIFO lot is tagged with
/// its channel so that when food leaves (eaten, sold, spoiled, inherited) it is debited against
/// the channel it actually ARRIVED through, not whatever stock happens to be on hand — which is
/// what lets the probe claim "after warm-up, survivors eat food they BOUGHT" without a resold or
/// mixed-stock unit being mis-attributed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum FoodChannel {
    /// Entered stock via a market trade record (`Society::trades` spot or `barter_trades`) —
    /// the agent BOUGHT it. The open-survival claim rests on this channel dominating consumption.
    Bought = 0,
    /// A cold-start seed buffer (the generated bread holdings) or a hearth MINT (the demographic
    /// `food_provision` / producer staple endowment). Retired in the probe, so this channel only
    /// depletes — never refills — after generation, making seed depletion directly visible.
    SeededMinted = 1,
    /// The agent's OWN production — a chain bake or own-use cultivation, booked `produced`.
    SelfProduced = 2,
    /// Own-labor forage. N/A for this probe: bread is never foraged (FORAGE is a distinct,
    /// untracked good), so this channel stays zero — present for completeness/conservation.
    Foraged = 3,
    /// S23e finite rival subsistence commons draw. Distinct from forage and from the G4a
    /// death-estate commons, so the scarce outside-option relief share is directly observable.
    Commons = 4,
}
impl FoodChannel {
    pub(super) const COUNT: usize = 5;
    pub(super) const ALL: [FoodChannel; Self::COUNT] = [
        FoodChannel::Bought,
        FoodChannel::SeededMinted,
        FoodChannel::SelfProduced,
        FoodChannel::Foraged,
        FoodChannel::Commons,
    ];
    pub(super) fn index(self) -> usize {
        self as usize
    }
}
/// S21d.1: the **acquisition-channel ledger** — a sim-side, runtime-only per-agent FIFO balance
/// of the tracked food good (bread), classifying each held unit by the channel it entered through
/// ([`FoodChannel`]). It mirrors the [`BreadProvenance`] readback discipline (post-`society.step()`,
/// never econ-internal hooks) but tracks the FULL stock across all four channels, not just produced
/// origin, so it can answer "what channel did the food survivors eat come from?".
///
/// **Conservation invariant (Codex P2):** EVERY outflow of tracked-food debits the ledger FIFO —
/// consumption, sale/barter transfer, spoilage, estate settlement, and any birth/endowment
/// transfer — and every inflow credits a channel, so `total_held()` stays equal to the tracked
/// food actually held across all living agents (asserted each tick). That equality is what stops
/// "bought food consumed" being overstated by an untracked outflow.
///
/// NOT serialized into `canonical_bytes` (diagnostic/proof state, like `starvation_deaths_total`),
/// so it shifts no digest and every existing golden is byte-identical whether on or off. Maintained
/// only while [`Settlement::acquisition_ledger_active`] holds; the empty default otherwise.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct AcquisitionLedger {
    /// Per-agent FIFO lots, oldest at the front. A credit pushes to the back; a debit pops from
    /// the front (splitting the front lot if it is larger than the debit).
    pub(super) lots: BTreeMap<AgentId, VecDeque<FoodLot>>,
    /// One-time bootstrap latch: the generated seed stock is swept into `SeededMinted` lots on the
    /// first active tick (before any inflow/outflow), so the ledger starts in lockstep with stock.
    pub(super) initialized: bool,
    /// Cumulative tracked-food CONSUMED (eaten), split by the channel it arrived through — the bar
    /// reads this: after warm-up, `Bought` ≫ `SeededMinted` + `Foraged`.
    pub(super) consumed_by_channel: [u64; FoodChannel::COUNT],
    /// Cumulative tracked-food CREDITED (entered stock) per channel — the inflow side, used for the
    /// seed-depletion trace (how much seeded/minted food ever entered vs how much is left).
    pub(super) credited_by_channel: [u64; FoodChannel::COUNT],
    /// Cumulative tracked-food removed by every NON-consume outflow (sale-out, spoilage, estate,
    /// endowment-out), split by channel — completes the conservation picture beside consumption.
    pub(super) other_outflow_by_channel: [u64; FoodChannel::COUNT],
    /// S22a (runtime-only): cumulative tracked food a given agent ever acquired through the
    /// `Bought` channel (a market purchase). Credited on every `Bought` inflow (a fresh credit
    /// or a `transfer_as_bought`), never on an origin-preserving move (inheritance is not
    /// buying). Lets the rolling material-buyer diagnostic count non-cultivating buyers that
    /// actually transact, distinguishing a genuine division-of-labor split from a commune whose
    /// non-cultivators are alive but never buy. Never digested.
    pub(super) bought_credited_by_agent: BTreeMap<AgentId, u64>,
    /// P1.5 (runtime-only): cumulative tracked food a given agent ever acquired through the
    /// `Commons` channel. Mirrors `bought_credited_by_agent` for the substitution diagnostic.
    pub(super) commons_credited_by_agent: BTreeMap<AgentId, u64>,
    /// S23c (runtime-only): cumulative tracked food a given agent ever CONSUMED (ate), across
    /// every channel — the per-agent food-intake denominator. Lets a tenure-tier metric divide
    /// "bought food" (a subset of intake) and "owner-supplied production" by the food a specific
    /// cohort (e.g. non-owners) actually ate, instead of by the whole colony's consumption.
    /// Never digested.
    pub(super) consumed_food_by_agent: BTreeMap<AgentId, u64>,
    /// DH.b (impl-69): whether the burden provenance extension (purchase identity +
    /// construction taint on every lot) is maintained — set at ledger init from
    /// `closure_active()`, so DH.a's force-disable control governs it. `false` everywhere else:
    /// every lot then carries `identity: None, taint: false` and the ledger behaves
    /// byte-identically to the landed one.
    pub(super) burden_provenance: bool,
    /// DH.b (impl-69): live channel violations under the burden extension (a `Foraged`/`Commons`
    /// credit — hard-asserted unreachable on the closed base). The integration suite hard-fails
    /// on any entry; the runtime only records.
    pub(super) burden_violations: Vec<String>,
}
impl AcquisitionLedger {
    /// Credit `qty` of `agent`'s tracked-food as a fresh `channel` lot (an inflow), non-intervention.
    pub(super) fn credit(&mut self, agent: AgentId, channel: FoodChannel, qty: u64) {
        self.credit_lot(agent, channel, qty, false);
    }

    /// C3R.e (impl-67): credit `qty` of `agent`'s tracked-food as a fresh INTERVENTION-ORIGIN lot
    /// (A2's endowment split / B's support mints). Identical to [`Self::credit`] except the lot
    /// carries the origin flag, so the units read as intervention-origin until consumed/sunk.
    pub(super) fn credit_intervention(&mut self, agent: AgentId, channel: FoodChannel, qty: u64) {
        self.credit_lot(agent, channel, qty, true);
    }

    pub(super) fn credit_lot(
        &mut self,
        agent: AgentId,
        channel: FoodChannel,
        qty: u64,
        intervention: bool,
    ) {
        if qty == 0 {
            return;
        }
        // DH.b (impl-69): construction taint — a fresh `SeededMinted` lot is construction stock
        // on the closed base; `Foraged`/`Commons` are unreachable there (recorded as a violation
        // the live suite hard-fails on). Both inert off the burden extension.
        let taint = self.burden_provenance && channel == FoodChannel::SeededMinted;
        if self.burden_provenance && matches!(channel, FoodChannel::Foraged | FoodChannel::Commons)
        {
            self.burden_violations.push(format!(
                "unreachable construction channel {channel:?} credited to {agent:?}"
            ));
        }
        self.lots.entry(agent).or_default().push_back(FoodLot {
            channel,
            qty,
            intervention,
            identity: None,
            taint,
        });
        self.credited_by_channel[channel.index()] += qty;
        // S22a: track per-agent cumulative bought food (market purchases) for the rolling
        // material-buyer diagnostic. `transfer_as_bought` routes the buyer's inflow through
        // here, so every market purchase is counted; inheritance/endowment use
        // `transfer_preserve` (not buying) and are excluded.
        if channel == FoodChannel::Bought {
            *self.bought_credited_by_agent.entry(agent).or_insert(0) += qty;
        } else if channel == FoodChannel::Commons {
            *self.commons_credited_by_agent.entry(agent).or_insert(0) += qty;
        }
    }

    /// Draw up to `qty` of `agent`'s tracked-food FIFO (oldest first), returning the ordered lots
    /// actually drawn. Re-credits NOTHING — the caller decides whether the drawn units are a sink,
    /// a consume, or a transfer.
    pub(super) fn draw_lots(&mut self, agent: AgentId, mut qty: u64) -> Vec<FoodLot> {
        let mut drawn = Vec::new();
        let Some(queue) = self.lots.get_mut(&agent) else {
            return drawn;
        };
        while qty > 0 {
            let Some(front) = queue.front_mut() else {
                break;
            };
            let take = front.qty.min(qty);
            // DH.b: a split preserves BOTH the purchase identity and the taint (R2-7).
            drawn.push(FoodLot {
                qty: take,
                ..*front
            });
            front.qty -= take;
            qty -= take;
            if front.qty == 0 {
                queue.pop_front();
            }
        }
        if queue.is_empty() {
            self.lots.remove(&agent);
        }
        drawn
    }

    /// Draw up to `qty` of `agent`'s tracked-food FIFO (oldest first), returning the per-channel
    /// breakdown of what was actually drawn. The breakdown sums to `min(qty, held)`.
    pub(super) fn draw(&mut self, agent: AgentId, qty: u64) -> [u64; FoodChannel::COUNT] {
        let mut drawn = [0u64; FoodChannel::COUNT];
        for lot in self.draw_lots(agent, qty) {
            drawn[lot.channel.index()] += lot.qty;
        }
        drawn
    }

    /// A CONSUME debit (eaten through the consumption-log readback): draw FIFO and book the drawn
    /// units to `consumed_by_channel` — the headline trace.
    pub(super) fn consume(&mut self, agent: AgentId, qty: u64) {
        let drawn = self.draw(agent, qty);
        let mut total = 0u64;
        for channel in FoodChannel::ALL {
            self.consumed_by_channel[channel.index()] += drawn[channel.index()];
            total += drawn[channel.index()];
        }
        if total > 0 {
            *self.consumed_food_by_agent.entry(agent).or_insert(0) += total;
        }
    }

    /// S23c (runtime-only): cumulative tracked food `agent` has consumed across all channels.
    pub(super) fn consumed_food_of_agent(&self, agent: AgentId) -> u64 {
        self.consumed_food_by_agent
            .get(&agent)
            .copied()
            .unwrap_or(0)
    }

    /// A SINK debit (spoiled / estate→commons): draw FIFO and book the drawn units to the
    /// non-consume outflow tally. The units leave the living population for good.
    pub(super) fn sink(&mut self, agent: AgentId, qty: u64) {
        let drawn = self.draw(agent, qty);
        for channel in FoodChannel::ALL {
            self.other_outflow_by_channel[channel.index()] += drawn[channel.index()];
        }
    }

    /// A MARKET-SALE transfer: the seller's units leave FIFO (booked as non-consume outflow), and
    /// the SAME quantity enters the buyer as fresh `Bought` lots — the buyer's acquisition channel
    /// is, by definition, "bought" regardless of the seller's origin. Conserved (held total
    /// unchanged); the channel mix shifts toward `Bought`, which is exactly the probe's signal.
    ///
    /// C3R.e (impl-67): the retag is ORDER-PRESERVING and origin-flag-preserving. Each drawn lot
    /// maps IN ORIGINAL FIFO ORDER to a fresh `Bought` lot that KEEPS its `intervention` origin flag
    /// (the sale changes the channel, never the origin), coalescing only ADJACENT equal-origin lots.
    /// A channel-partition would reorder a mixed FIFO and corrupt later exhaustion/birth attribution
    /// — so a single intervention loaf sold and resold still reads as intervention-origin. With every
    /// drawn lot equal-origin (the un-flagged default) the run collapses to one `Bought` lot of the
    /// full quantity, exactly the pre-flag credit, so off the intervention path this is inert.
    pub(super) fn transfer_as_bought(
        &mut self,
        from: AgentId,
        to: AgentId,
        qty: u64,
    ) -> [u64; FoodChannel::COUNT] {
        self.transfer_as_bought_identified(from, to, qty, None).0
    }

    /// DH.b (impl-69): [`Self::transfer_as_bought`] with the resale rule of the R2-7 lifecycle
    /// table — every fresh `Bought` lot's purchase identity is OVERWRITTEN with the settling
    /// trade's id while its taint (and intervention origin) is PRESERVED. Coalescing now
    /// requires the FULL equality set: channel (uniformly `Bought` here), intervention, taint,
    /// and identity (constant per call). With `identity: None` and all-false taints (every
    /// non-closed run) the run key degrades to the landed intervention-only rule, so this is
    /// byte-behaviour-identical off the extension. Also returns the fresh `Bought` total — the
    /// purchase-credit-seam aggregate the R4-1 validation compares against the trade record.
    pub(super) fn transfer_as_bought_identified(
        &mut self,
        from: AgentId,
        to: AgentId,
        qty: u64,
        identity: Option<u64>,
    ) -> ([u64; FoodChannel::COUNT], u64) {
        // DH.b: under the extension EVERY fresh `Bought` lot must carry a purchase identity —
        // an identity-less credit (e.g. a barter bread trade, unreachable on the closed base)
        // is instrumentation corruption, recorded at ITS creation seam so it cannot be eaten
        // away before an audit sees it. The live suite hard-fails on the record.
        if self.burden_provenance && identity.is_none() && qty > 0 {
            self.burden_violations.push(format!(
                "Bought credit without purchase identity ({from:?} → {to:?}, qty {qty})"
            ));
        }
        let drawn = self.draw_lots(from, qty);
        let mut breakdown = [0u64; FoodChannel::COUNT];
        let mut run: Vec<FoodLot> = Vec::new();
        for lot in drawn {
            breakdown[lot.channel.index()] += lot.qty;
            self.other_outflow_by_channel[lot.channel.index()] += lot.qty;
            match run.last_mut() {
                Some(last) if last.intervention == lot.intervention && last.taint == lot.taint => {
                    last.qty += lot.qty
                }
                _ => run.push(FoodLot {
                    channel: FoodChannel::Bought,
                    qty: lot.qty,
                    intervention: lot.intervention,
                    identity,
                    taint: lot.taint,
                }),
            }
        }
        let total: u64 = breakdown.iter().sum();
        if total > 0 {
            self.lots.entry(to).or_default().extend(run);
            self.credited_by_channel[FoodChannel::Bought.index()] += total;
            *self.bought_credited_by_agent.entry(to).or_insert(0) += total;
        }
        (breakdown, total)
    }

    /// An ORIGIN-PRESERVING transfer (birth endowment / estate→heir): draw FIFO from `from` and
    /// re-credit the SAME channels (and origin flags) to `to`, so an inherited seeded loaf stays
    /// seeded and an intervention loaf stays intervention-origin. A pure internal move — it touches
    /// neither the consume nor the outflow tally, and (unlike `credit`) does not double-count the
    /// inflow counters. Returns the exact drawn lots so a caller (e.g. the birth-funding site) can
    /// attribute funding by channel and origin.
    pub(super) fn transfer_preserve(
        &mut self,
        from: AgentId,
        to: AgentId,
        qty: u64,
    ) -> Vec<FoodLot> {
        let drawn = self.draw_lots(from, qty);
        if !drawn.is_empty() {
            self.lots
                .entry(to)
                .or_default()
                .extend(drawn.iter().copied());
        }
        drawn
    }

    /// C3R.e (impl-67): like [`Self::transfer_preserve`] but STAMPS every moved lot as
    /// intervention-origin — A1's one-shot injection moves ordinary donor bread and re-flags it as
    /// intervention (the channel is preserved; only the origin flag is set). Returns the moved lots.
    pub(super) fn transfer_preserve_as_intervention(
        &mut self,
        from: AgentId,
        to: AgentId,
        qty: u64,
    ) -> Vec<FoodLot> {
        let mut drawn = self.draw_lots(from, qty);
        for lot in &mut drawn {
            lot.intervention = true;
        }
        if !drawn.is_empty() {
            self.lots
                .entry(to)
                .or_default()
                .extend(drawn.iter().copied());
        }
        drawn
    }

    /// Move up to `qty` of the sender's self-produced tracked-food channel to `to`, leaving
    /// older seeded/bought lots in place. This mirrors the in-kind wage advance's
    /// producer-filtered bread-provenance transfer at the coarser acquisition-channel level.
    pub(super) fn transfer_self_produced(&mut self, from: AgentId, to: AgentId, qty: u64) -> u64 {
        if qty == 0 {
            return 0;
        }
        let mut remaining = qty;
        let mut moved = VecDeque::new();
        let queue_empty = {
            let Some(queue) = self.lots.get_mut(&from) else {
                return 0;
            };
            let mut kept = VecDeque::new();
            while let Some(mut lot) = queue.pop_front() {
                if lot.channel == FoodChannel::SelfProduced && remaining > 0 {
                    let take = lot.qty.min(remaining);
                    moved.push_back(FoodLot { qty: take, ..lot });
                    lot.qty -= take;
                    remaining -= take;
                }
                if lot.qty > 0 {
                    kept.push_back(lot);
                }
            }
            *queue = kept;
            queue.is_empty()
        };
        if queue_empty {
            self.lots.remove(&from);
        }
        let moved_qty: u64 = moved.iter().map(|lot| lot.qty).sum();
        if moved_qty > 0 {
            self.lots.entry(to).or_default().extend(moved);
        }
        moved_qty
    }

    /// Move the newest self-produced tracked-food lots from `from` to `to`. This is the
    /// acquisition-channel twin of `BreadProvenance::transfer_recent_self_produced` for the
    /// C1N crop split, which runs immediately after the worker's conversion credit.
    pub(super) fn transfer_recent_self_produced(
        &mut self,
        from: AgentId,
        to: AgentId,
        qty: u64,
    ) -> u64 {
        if qty == 0 {
            return 0;
        }
        let mut remaining = qty;
        let mut moved = Vec::new();
        let queue_empty = {
            let Some(queue) = self.lots.get_mut(&from) else {
                return 0;
            };
            while remaining > 0 {
                let pop_empty = {
                    let Some(back) = queue.back_mut() else {
                        break;
                    };
                    if back.channel != FoodChannel::SelfProduced {
                        break;
                    }
                    let take = back.qty.min(remaining);
                    moved.push(FoodLot { qty: take, ..*back });
                    back.qty -= take;
                    remaining -= take;
                    back.qty == 0
                };
                if pop_empty {
                    queue.pop_back();
                }
            }
            queue.is_empty()
        };
        if queue_empty {
            self.lots.remove(&from);
        }
        let moved_qty: u64 = moved.iter().map(|lot| lot.qty).sum();
        if moved_qty > 0 {
            let queue = self.lots.entry(to).or_default();
            for lot in moved.into_iter().rev() {
                queue.push_back(lot);
            }
        }
        moved_qty
    }

    /// Drop every lot a removed agent still holds to the sink (the estate→commons exit when its
    /// bread could not be routed to a living heir). Books the drained units as outflow. Returns
    /// the residual units dropped, so the caller can assert the estate routing accounted for
    /// every tracked-food unit (mirroring [`BreadProvenance::drop_to_sink`]).
    pub(super) fn drop_to_sink(&mut self, agent: AgentId) -> u64 {
        let mut dropped = 0;
        if let Some(queue) = self.lots.remove(&agent) {
            for lot in queue {
                self.other_outflow_by_channel[lot.channel.index()] += lot.qty;
                dropped += lot.qty;
            }
        }
        dropped
    }

    /// Total tracked-food held across all living agents (the conservation left-hand side).
    pub(super) fn total_held(&self) -> u64 {
        self.lots
            .values()
            .flat_map(|q| q.iter())
            .map(|lot| lot.qty)
            .sum()
    }

    /// Tracked-food currently held per channel — the seed-depletion read (`SeededMinted` falling
    /// toward zero) and the live channel mix.
    pub(super) fn held_by_channel(&self) -> [u64; FoodChannel::COUNT] {
        let mut held = [0u64; FoodChannel::COUNT];
        for lot in self.lots.values().flat_map(|q| q.iter()) {
            held[lot.channel.index()] += lot.qty;
        }
        held
    }

    /// Tracked-food currently held by one agent, split by channel.
    pub(super) fn held_by_agent(&self, agent: AgentId) -> [u64; FoodChannel::COUNT] {
        let mut held = [0u64; FoodChannel::COUNT];
        if let Some(queue) = self.lots.get(&agent) {
            for lot in queue {
                held[lot.channel.index()] += lot.qty;
            }
        }
        held
    }

    /// C3R.e (impl-67): total INTERVENTION-ORIGIN tracked food held across ALL living agents,
    /// every channel — the GLOBAL exhaustion read (criterion ii is `== 0`). Resale-proof: the
    /// origin flag survives the market retag, so a laundered ignition loaf still counts here.
    pub(super) fn intervention_held(&self) -> u64 {
        self.lots
            .values()
            .flat_map(|q| q.iter())
            .filter(|lot| lot.intervention)
            .map(|lot| lot.qty)
            .sum()
    }

    /// C3R.e (impl-67): intervention-origin tracked food held by the given cohort of agents (the
    /// producer-cohort exhaustion read). A subset of [`Self::intervention_held`].
    pub(super) fn intervention_held_by(&self, agents: &BTreeSet<AgentId>) -> u64 {
        agents
            .iter()
            .filter_map(|id| self.lots.get(id))
            .flat_map(|q| q.iter())
            .filter(|lot| lot.intervention)
            .map(|lot| lot.qty)
            .sum()
    }

    /// Tracked-food currently held by one agent in one channel.
    pub(super) fn held_by_agent_channel(&self, agent: AgentId, channel: FoodChannel) -> u64 {
        self.lots
            .get(&agent)
            .map(|queue| {
                queue
                    .iter()
                    .filter(|lot| lot.channel == channel)
                    .map(|lot| lot.qty)
                    .sum()
            })
            .unwrap_or(0)
    }

    /// S22a: cumulative `Bought`-channel tracked food the agent ever acquired (the rolling
    /// material-buyer signal). `0` for an agent that never bought.
    pub(super) fn bought_credited_of(&self, agent: AgentId) -> u64 {
        self.bought_credited_by_agent
            .get(&agent)
            .copied()
            .unwrap_or(0)
    }

    /// P1.5: cumulative `Commons`-channel tracked food the agent ever acquired.
    pub(super) fn commons_credited_of(&self, agent: AgentId) -> u64 {
        self.commons_credited_by_agent
            .get(&agent)
            .copied()
            .unwrap_or(0)
    }
}
