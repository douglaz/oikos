//! The `Region` — multiple settlements that trade (game milestone G2c).
//!
//! G2c completes the revised G2 by making **several settlement economies exist
//! and trade**. The game-spec frames this as "extract settlement-scoped services
//! from the `Society` monolith"; we reach the same *end* — independent settlement
//! economies that trade — by **composition, not internal surgery**. A [`Region`]
//! holds a `Vec<Settlement>`, each one **unchanged from G2b** (its own `World` +
//! `Society` + colonists + conservation receipt), linked by an abstract
//! inter-settlement [`Route`] (a transit-tick count), with a caravan carrying
//! one good from where it is cheap to where it is dear. Because no `Settlement`
//! and no `Society` internal behaviour changes, the six econ conformance goldens
//! and every G1/G2a/G2b/G2d test stay byte-identical (the unchanged workspace
//! `cargo test` is the proof).
//!
//! ## The caravan as a permanent trader pair (the load-bearing design)
//!
//! Runtime agent-roster mutation (`AgentArena` free/cache reconciliation) is
//! G4-deferred, so a caravan must **never** add or remove an agent from a
//! `Society`. Instead a caravan is a **pair of permanent resident trader agents**
//! — one per linked settlement, created at *generation* (see
//! [`crate::settlement::TraderEndowment`]) — and the `Region` shuttles their
//! **wealth** between them as route escrow, never the agents themselves. Each
//! settlement's agent count is therefore constant for the whole run.
//!
//! One arbitrage cycle (good `X` dearer in B than A; `trader_a` resides in A,
//! `trader_b` in B):
//!
//! 1. **BuyA** — `trader_a` is an active bidder in A's market (it holds gold and
//!    a standing want for `X`); it buys `X` at A's price through the *unchanged*
//!    [`Settlement::econ_tick`] / `Society::step` clearing — no special accessor.
//! 2. **DepartA→B** — the `Region` withdraws the bought `X` from `trader_a` into
//!    **route escrow** (`Society::debit_stock`); `trader_a`'s gold stays home
//!    (it is idled — an empty scale posts nothing — so the gold-poor seller it
//!    feeds keeps a strong sell motivation).
//! 3. **Transit** — each econ tick decrements the route's remaining transit; the
//!    escrow holdings are conserved (counted in the `Region` roll-up, at no
//!    settlement). G2c has no loss/risk: escrow in transit is never destroyed and
//!    is retained intact if a leg never completes.
//! 4. **ArriveB** — the `Region` deposits the escrowed `X` into `trader_b`
//!    (`Society::credit_stock`) and activates it to sell.
//! 5. **SellB** — `trader_b` asks `X` into B's market; it clears at B's price,
//!    accumulating gold.
//! 6. **DepartB→A / ArriveA** — the `Region` escrows `trader_b`'s proceeds (and
//!    any unsold `X`) back along the route and deposits them into `trader_a`
//!    (`Society::credit_gold` / `credit_stock`); the cycle repeats.
//!
//! While a trader is not the active side, its scale is empty, so it places no
//! orders (idle). Every transfer is **net-zero** across the `[Σ societies ∪
//! escrow]` ledger — the additive `econ` accessors MOVE value, never mint or burn
//! it — so the `Region` roll-up is an exact ledger: for each good and for gold,
//! the regional total changes only by accounted node regen (the source) and
//! consumption (the sink). [`Region::econ_tick`] checks this every tick.
//!
//! ## Direction, and what this deliberately is not
//!
//! G2c is **two settlements, one traded good, one caravan**. The buy→sell
//! direction is fixed by construction: settlement 0 (A) is configured cheaper
//! (a nearer FOOD node) and settlement 1 (B) dearer (a farther node), so the
//! realized-price differential's *sign* is persistent; the caravan still gates a
//! fresh haul on the live differential (a `Hold` phase) so it never hauls the
//! wrong way after a convergence overshoot. There is no loss/risk, no
//! roads-as-projects, no `>2` settlements, and no multi-good / dynamic multi-hop
//! routing (all later). Determinism is inherited: integer state, the econ `Rng`
//! consumed only at generation, nothing drawn in the loops, settlement- and
//! id-ordered iteration, `BTreeMap`/`Vec` only — same `(seed, RegionConfig)` →
//! byte-identical run ([`Region::digest`]).

use std::collections::BTreeMap;

use econ::agent::{AgentId, Want, WantKind};
use econ::good::{Gold, GoodId, Horizon, FOOD, GOLD};

use crate::settlement::{Settlement, SettlementConfig, TraderEndowment};

/// An abstract inter-settlement **route**: a transit-tick count, NOT intra-
/// settlement grid movement. The number of econ ticks a caravan spends in transit
/// on each leg. "near"/"far" route lengths give the convergence contrast and the
/// future road idea; G2c uses one route between the two settlements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    /// Econ ticks a caravan is in transit on a leg (each way). Zero means a leg
    /// completes the same tick it departs.
    pub transit_ticks: u32,
}

/// How a region of two settlements is generated: the two settlement recipes, the
/// linking route, the single traded good, the caravan's trader working capital,
/// the dwell lengths, and whether the caravan runs at all (the no-caravan control
/// is `caravans_enabled = false`). Mechanism knobs, not balance targets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionConfig {
    /// Settlement A (index 0, the buyer/cheaper side) recipe.
    pub settlement_a: SettlementConfig,
    /// Settlement B (index 1, the seller/dearer side) recipe.
    pub settlement_b: SettlementConfig,
    /// The abstract route linking A and B.
    pub route: Route,
    /// The single good the caravan trades. Must be a good both settlements
    /// actively **trade** — one with an order book in each settlement's market
    /// (a built-in market good like FOOD/WOOD/NET, or one some agent holds or
    /// wants at generation) and tracked for conservation in both. FOOD is the
    /// canonical choice. Never GOLD (money is not hauled); a node-only tracked
    /// good with no order book is rejected at [`Region::generate`] (it would
    /// conserve but never trade).
    pub good: GoodId,
    /// Working-capital gold the buyer trader (`trader_a`) starts with. The seller
    /// trader (`trader_b`) starts gold-poor (zero) so it keeps a strong incentive
    /// to sell the good it is handed.
    pub trader_gold: u64,
    /// Econ ticks the caravan dwells buying at A before departing.
    pub buy_ticks: u32,
    /// Econ ticks the caravan dwells selling at B before departing.
    pub sell_ticks: u32,
    /// Whether the caravan operates. `false` is the **no-caravan control** (the
    /// falsification twin): the trader pair still exists (agent counts match) but
    /// stays idle, so the settlements never interact and the price gap is kept.
    pub caravans_enabled: bool,
}

impl RegionConfig {
    /// The canonical two-settlement, one-caravan region for the price-convergence
    /// experiment. Settlement A is cheap and supply-rich (a near FOOD node, more
    /// gatherers than consumers); settlement B is dear and supply-poor (a far node,
    /// a small population that starts with no FOOD buffer, so it is scarce — and so
    /// dear — from the first ticks). The caravan buys FOOD cheap at A and sells it
    /// at B, relieving B's scarcity so its realized FOOD price falls toward A's;
    /// the no-caravan control keeps the gap. Both settlements use the G2b
    /// hunger-resilient [`SettlementConfig::price_probe`] colonists (so the dear
    /// market stays demand-heavy and bids up under scarcity instead of dying off).
    /// Mechanism knobs, not balance targets; the proof is sign only.
    pub fn two_settlements() -> Self {
        let mut settlement_a = SettlementConfig::price_probe();
        settlement_a.gatherers = 8;
        settlement_a.consumers = 4;
        let settlement_a = settlement_a.with_food_node_distance(6);

        let mut settlement_b = SettlementConfig::price_probe();
        settlement_b.gatherers = 4;
        settlement_b.consumers = 2;
        // No starting FOOD buffer, so B's consumers are hungry — and so the market
        // is dear — from the opening ticks, giving a positive gap for the caravan
        // to narrow (rather than merely prevent from opening).
        settlement_b.consumer_food_buffer = 0;
        settlement_b.gatherer_food_buffer = 0;
        let settlement_b = settlement_b.with_food_node_distance(18);

        Self {
            settlement_a,
            settlement_b,
            route: Route { transit_ticks: 1 },
            good: FOOD,
            trader_gold: 600,
            buy_ticks: 6,
            sell_ticks: 10,
            caravans_enabled: true,
        }
    }

    /// The no-caravan **control** twin of [`Self::two_settlements`] — identical
    /// settlements and trader pair (so agent counts match), but the caravan is
    /// disabled, so the settlements never interact and the price gap is kept. The
    /// falsification twin that proves the caravan is what closes the gap.
    pub fn two_settlements_control() -> Self {
        Self {
            caravans_enabled: false,
            ..Self::two_settlements()
        }
    }

    /// Number of distinct `Want{Good(GOLD), Later}` units the seller's value scale
    /// carries — its standing money demand. Sized well above the gold a seller
    /// ever holds in one sell phase (it arrives gold-poor and only accrues a
    /// cycle's proceeds before they are swept home), so the seller keeps an
    /// unmet money want and goes on asking the good cheaply. If it were ever too
    /// small the seller would simply stop early and carry the remainder home —
    /// conserved, never lost.
    const SELL_LADDER_UNITS: u16 = 256;
    /// Number of `Want{Good(X), Next}` units the buyer's scale carries — its
    /// standing demand for the traded good. Bounds how much the buyer accumulates
    /// in one buy phase (it buys at most one unit per econ tick); a few times the
    /// buy dwell is ample.
    const BUY_LADDER_UNITS: u16 = 64;
}

/// The per-econ-tick **region-wide** conservation + flow receipt. Sums each
/// settlement's own [`crate::EconTickReport`] and adds the in-transit route
/// escrow, so the invariant it pins is regional: for every tracked good the total
/// over all settlements **plus** route escrow changes by exactly `+regen −
/// consumed`; for gold the regional total is unchanged (gold is a closed regional
/// balance). Every caravan transfer is net-zero and so never appears.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegionTickReport {
    pub econ_tick: u64,
    /// Goods created by node regen across all settlements (the only source).
    pub regen: BTreeMap<GoodId, u64>,
    /// Goods consumed across all settlements (the only sink).
    pub consumed: BTreeMap<GoodId, u64>,
    /// Regional total per good (Σ settlements + route escrow) before this tick.
    pub before: BTreeMap<GoodId, u64>,
    /// Regional total per good (Σ settlements + route escrow) after this tick.
    pub after: BTreeMap<GoodId, u64>,
    /// In-transit route escrow per good at the end of this tick.
    pub escrow: BTreeMap<GoodId, u64>,
    /// Regional gold (Σ settlements + escrow) before / after — must be equal.
    pub gold_before: u64,
    pub gold_after: u64,
    /// In-transit route escrow gold at the end of this tick.
    pub escrow_gold: u64,
}

impl RegionTickReport {
    pub fn regen_of(&self, good: GoodId) -> u64 {
        self.regen.get(&good).copied().unwrap_or(0)
    }
    pub fn consumed_of(&self, good: GoodId) -> u64 {
        self.consumed.get(&good).copied().unwrap_or(0)
    }
    pub fn before_of(&self, good: GoodId) -> u64 {
        self.before.get(&good).copied().unwrap_or(0)
    }
    pub fn after_of(&self, good: GoodId) -> u64 {
        self.after.get(&good).copied().unwrap_or(0)
    }
    pub fn escrow_of(&self, good: GoodId) -> u64 {
        self.escrow.get(&good).copied().unwrap_or(0)
    }

    /// Whether the regional ledger balances: for every tracked good `after ==
    /// before + regen − consumed`, and gold is unchanged. This is the G2c
    /// conservation DoD; [`Region::econ_tick`] also `debug_assert`s it.
    pub fn conserves(&self) -> bool {
        let goods_ok = self.before.keys().all(|good| {
            let before = self.before_of(*good) as i128;
            let after = self.after_of(*good) as i128;
            let regen = self.regen_of(*good) as i128;
            let consumed = self.consumed_of(*good) as i128;
            after == before + regen - consumed
        });
        goods_ok && self.gold_after == self.gold_before
    }
}

/// One arbitrage cycle's phase. The caravan advances one phase boundary at a time,
/// dwelling `buy_ticks` / `sell_ticks` at the endpoints and `route.transit_ticks`
/// on each leg. Integer state only; no randomness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// `trader_a` actively buys the good at settlement A.
    BuyA,
    /// In transit A→B: the bought good rides in route escrow.
    ToB,
    /// `trader_b` actively sells the good at settlement B.
    SellB,
    /// In transit B→A: the proceeds (and any unsold good) ride in route escrow.
    ToA,
    /// Parked at A: the live price differential is unfavourable (A is no longer
    /// cheaper than B), so the caravan waits rather than haul the wrong way.
    Hold,
}

/// The caravan operator: the permanent trader pair, the phase machine, and the
/// in-transit route escrow. `trader_a` resides in settlement 0 (the buyer side),
/// `trader_b` in settlement 1 (the seller side).
struct Caravan {
    trader_a: AgentId,
    trader_b: AgentId,
    phase: Phase,
    /// Econ ticks remaining in the current dwell/transit phase.
    counter: u32,
    /// Good held in route escrow (mid-transit, at no settlement).
    escrow_good: u32,
    /// Gold held in route escrow (mid-transit, at no settlement).
    escrow_gold: u64,
}

/// A region of two composed settlements linked by one caravan over one route.
pub struct Region {
    settlements: Vec<Settlement>,
    caravan: Caravan,
    route: Route,
    good: GoodId,
    caravans_enabled: bool,
    buy_ticks: u32,
    sell_ticks: u32,
    /// The goods tracked for region-wide conservation (union of the settlements'
    /// tracked goods), `GoodId`-ordered. GOLD is excluded (money, tracked apart).
    goods: Vec<GoodId>,
    econ_tick: u64,
    last_report: RegionTickReport,
}

impl Region {
    /// Generate a region from `seed` and a [`RegionConfig`]. Each settlement is
    /// generated with one resident trader (the caravan's pair); all randomness is
    /// drawn inside the settlements' `generate` (per-colonist culture) — the
    /// region loop draws none. Deterministic: same `(seed, RegionConfig)` →
    /// byte-identical region.
    pub fn generate(seed: u64, config: &RegionConfig) -> Self {
        assert_ne!(
            config.good, GOLD,
            "the caravan cannot trade the money good (GOLD)"
        );
        // One resident trader per settlement (the permanent pair). The buyer side
        // (A) carries the working capital; the seller side (B) starts gold-poor so
        // it keeps a standing money demand once it is handed the good to sell.
        let config_a = config
            .settlement_a
            .clone()
            .with_resident_traders(vec![TraderEndowment {
                gold: config.trader_gold,
                stock: Vec::new(),
            }]);
        let config_b = config
            .settlement_b
            .clone()
            .with_resident_traders(vec![TraderEndowment {
                gold: 0,
                stock: Vec::new(),
            }]);

        // Distinct sub-seeds so the two settlements are independently generated
        // (same base config would otherwise produce identical colonist cultures).
        let mut settlement_a = Settlement::generate(seed, &config_a);
        let settlement_b = Settlement::generate(seed ^ 0x9E37_79B9_7F4A_7C15, &config_b);

        let trader_a = settlement_a
            .resident_trader_ids()
            .first()
            .copied()
            .expect("settlement A was generated with one resident trader");
        let trader_b = settlement_b
            .resident_trader_ids()
            .first()
            .copied()
            .expect("settlement B was generated with one resident trader");

        assert!(
            settlement_a.tracked_goods().contains(&config.good)
                && settlement_b.tracked_goods().contains(&config.good),
            "both settlements must track the traded good for region-wide conservation"
        );
        // The good must also be actively tradeable — have an order book — in BOTH
        // settlements, else the caravan would set up its trader scales but post no
        // orders, so the region would conserve yet never trade (a silent no-op). A
        // book exists only for the built-in market goods (FOOD/WOOD/NET) or a good
        // some agent holds or wants at generation; a node-only tracked good (e.g.
        // SALT) has none. Reject it loudly rather than run a dead region.
        assert!(
            settlement_a.society().market_goods().contains(&config.good)
                && settlement_b.society().market_goods().contains(&config.good),
            "the traded good {:?} must have an order book in both settlements' \
             markets (a built-in market good like FOOD/WOOD/NET, or one held or \
             wanted by some agent at generation); a node-only good would conserve \
             but never trade",
            config.good
        );

        // The tracked-goods union (GoodId order) for the conservation roll-up.
        let mut goods: Vec<GoodId> = settlement_a.tracked_goods().to_vec();
        for &g in settlement_b.tracked_goods() {
            if !goods.contains(&g) {
                goods.push(g);
            }
        }
        goods.sort();

        // The caravan opens by buying at A (A is configured the cheaper side), so
        // its buyer scale is active on tick 0. At generation no trade has cleared,
        // so the favourability gate is vacuously true and the caravan opens in
        // `BuyA`; it is applied anyway so a configuration whose realized
        // differential is already inverted (A dearer than B) parks in `Hold`
        // instead of opening with a wrong-way haul. The control (caravans
        // disabled) keeps the same `BuyA`/`buy_ticks` opening — it never steps, so
        // its serialized state is unchanged.
        let opens_buying = config.caravans_enabled
            && haul_is_favourable(
                settlement_a.realized_price(config.good),
                settlement_b.realized_price(config.good),
            );
        let (phase, counter) = if config.caravans_enabled && !opens_buying {
            (Phase::Hold, 0)
        } else {
            (Phase::BuyA, config.buy_ticks)
        };
        if opens_buying {
            set_trader_scale(&mut settlement_a, trader_a, buy_scale(config.good));
        }

        let caravan = Caravan {
            trader_a,
            trader_b,
            phase,
            counter,
            escrow_good: 0,
            escrow_gold: 0,
        };

        Self {
            settlements: vec![settlement_a, settlement_b],
            caravan,
            route: config.route,
            good: config.good,
            caravans_enabled: config.caravans_enabled,
            buy_ticks: config.buy_ticks,
            sell_ticks: config.sell_ticks,
            goods,
            econ_tick: 0,
            last_report: RegionTickReport::default(),
        }
    }

    /// Advance the region by one economic tick: advance each settlement's econ
    /// tick (the unchanged G2b loop), run the caravan step (decide / escrow-move /
    /// (de)activate traders) using the additive transfer accessors, then roll up
    /// the region-wide conservation report. Returns — and stores — the report.
    pub fn econ_tick(&mut self) -> RegionTickReport {
        let mut report = RegionTickReport {
            econ_tick: self.econ_tick,
            ..RegionTickReport::default()
        };

        // ---- before: regional totals (Σ settlements + route escrow).
        for &good in &self.goods {
            report.before.insert(good, self.regional_total(good));
        }
        report.gold_before = self.regional_gold();

        // ---- advance each settlement (unchanged G2b), summing regen / consumed.
        for settlement in &mut self.settlements {
            let tick_report = settlement.econ_tick();
            for (&good, &qty) in &tick_report.regen {
                *report.regen.entry(good).or_insert(0) += qty;
            }
            for (&good, &qty) in &tick_report.consumed {
                *report.consumed.entry(good).or_insert(0) += qty;
            }
        }

        // ---- caravan step (net-zero moves; only runs when enabled).
        if self.caravans_enabled {
            self.caravan_step();
        }

        // ---- after: regional totals + escrow snapshot.
        for &good in &self.goods {
            report.after.insert(good, self.regional_total(good));
        }
        report.gold_after = self.regional_gold();
        report
            .escrow
            .insert(self.good, u64::from(self.caravan.escrow_good));
        report.escrow_gold = self.caravan.escrow_gold;

        debug_assert!(
            report.conserves(),
            "region-wide conservation broke at econ tick {}",
            self.econ_tick
        );

        self.econ_tick += 1;
        self.last_report = report.clone();
        report
    }

    /// Run `ticks` region econ ticks.
    pub fn run(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.econ_tick();
        }
    }

    // ---- the caravan operator ------------------------------------------

    /// One caravan step, run after both settlements have advanced this tick. It
    /// progresses the phase machine: counting down dwells/transits, moving wealth
    /// between a trader and the route escrow at each boundary (always net-zero),
    /// and (de)activating trader scales for the next tick. The buyer's gold stays
    /// home through the A→B leg so the seller it feeds arrives gold-poor and keeps
    /// a strong sell motivation; the proceeds ride back on the B→A leg.
    fn caravan_step(&mut self) {
        let trader_a = self.caravan.trader_a;
        let trader_b = self.caravan.trader_b;
        let good = self.good;

        match self.caravan.phase {
            Phase::BuyA => {
                self.caravan.counter = self.caravan.counter.saturating_sub(1);
                if self.caravan.counter == 0 {
                    // The buy dwell discovers A's price; gate the departure on the
                    // live differential so the caravan never hauls the wrong way
                    // even if the buy phase revealed A is no longer the cheaper
                    // side. Idle the trader either way (an empty scale posts
                    // nothing, and its gold stays home for the A→B leg).
                    idle_trader(&mut self.settlements[0], trader_a);
                    if self.favourable_to_haul() {
                        // Depart A→B: escrow the bought good.
                        let moved = withdraw_all_stock(&mut self.settlements[0], trader_a, good);
                        self.caravan.escrow_good += moved;
                        self.caravan.phase = Phase::ToB;
                        self.caravan.counter = self.route.transit_ticks;
                        // A zero-length transit arrives the same tick.
                        self.maybe_complete_transit();
                    } else {
                        // Unfavourable: park at A rather than haul wrong. The bought
                        // good stays with the idled trader (conserved at A, never
                        // hauled wrong) until the differential is favourable again.
                        self.caravan.phase = Phase::Hold;
                    }
                }
            }
            Phase::ToB => {
                self.advance_transit();
            }
            Phase::SellB => {
                self.caravan.counter = self.caravan.counter.saturating_sub(1);
                if self.caravan.counter == 0 {
                    // Depart B→A: escrow the proceeds (gold) and any unsold good.
                    idle_trader(&mut self.settlements[1], trader_b);
                    let unsold = withdraw_all_stock(&mut self.settlements[1], trader_b, good);
                    let proceeds = withdraw_all_gold(&mut self.settlements[1], trader_b);
                    self.caravan.escrow_good += unsold;
                    self.caravan.escrow_gold += proceeds;
                    self.caravan.phase = Phase::ToA;
                    self.caravan.counter = self.route.transit_ticks;
                    self.maybe_complete_transit();
                }
            }
            Phase::ToA => {
                self.advance_transit();
            }
            Phase::Hold => {
                // Parked at A because the differential is unfavourable; re-check.
                if self.favourable_to_haul() {
                    set_trader_scale(&mut self.settlements[0], trader_a, buy_scale(good));
                    self.caravan.phase = Phase::BuyA;
                    self.caravan.counter = self.buy_ticks;
                }
            }
        }
    }

    /// Decrement the current transit and, when it reaches zero, complete the leg.
    fn advance_transit(&mut self) {
        debug_assert!(matches!(self.caravan.phase, Phase::ToB | Phase::ToA));
        self.caravan.counter = self.caravan.counter.saturating_sub(1);
        self.maybe_complete_transit();
    }

    /// If a transit phase has run out its counter, deposit the escrow into the
    /// destination trader and start the next dwell. Also handles a zero-length
    /// transit (arrival the same tick it departs).
    fn maybe_complete_transit(&mut self) {
        if self.caravan.counter != 0 {
            return;
        }
        let trader_a = self.caravan.trader_a;
        let trader_b = self.caravan.trader_b;
        let good = self.good;
        match self.caravan.phase {
            Phase::ToB => {
                // Arrive at B: hand the good to the seller and activate it.
                let amount = self.caravan.escrow_good;
                self.caravan.escrow_good = 0;
                if amount > 0 {
                    let ok = self.settlements[1]
                        .society_mut()
                        .credit_stock(trader_b, good, amount);
                    assert!(
                        ok,
                        "the resident seller trader must accept the escrowed good"
                    );
                }
                set_trader_scale(&mut self.settlements[1], trader_b, sell_scale());
                self.caravan.phase = Phase::SellB;
                self.caravan.counter = self.sell_ticks;
            }
            Phase::ToA => {
                // Arrive at A: return proceeds (and any unsold good) to the buyer.
                let amount = self.caravan.escrow_good;
                self.caravan.escrow_good = 0;
                if amount > 0 {
                    let ok = self.settlements[0]
                        .society_mut()
                        .credit_stock(trader_a, good, amount);
                    assert!(
                        ok,
                        "the resident buyer trader must accept the returned good"
                    );
                }
                let gold = self.caravan.escrow_gold;
                self.caravan.escrow_gold = 0;
                if gold > 0 {
                    let ok = self.settlements[0]
                        .society_mut()
                        .credit_gold(trader_a, Gold(gold));
                    assert!(
                        ok,
                        "the resident buyer trader must accept the returned gold"
                    );
                }
                // Start a fresh cycle only if the differential is still favourable.
                if self.favourable_to_haul() {
                    set_trader_scale(&mut self.settlements[0], trader_a, buy_scale(good));
                    self.caravan.phase = Phase::BuyA;
                    self.caravan.counter = self.buy_ticks;
                } else {
                    self.caravan.phase = Phase::Hold;
                }
            }
            _ => {}
        }
    }

    /// Whether the live realized-price differential favours the configured
    /// A→B haul: A is no greater than B (or a price is not yet known, in which
    /// case the caravan proceeds to get the market going). Buy-low/sell-high.
    fn favourable_to_haul(&self) -> bool {
        haul_is_favourable(
            self.settlements[0].realized_price(self.good),
            self.settlements[1].realized_price(self.good),
        )
    }

    // ---- conservation roll-up helpers ----------------------------------

    /// The regional total of `good`: Σ over settlements of their whole-system
    /// total, plus any units in route escrow. The conserved regional quantity.
    fn regional_total(&self, good: GoodId) -> u64 {
        let in_settlements: u64 = self
            .settlements
            .iter()
            .map(|s| s.whole_system_total(good))
            .sum();
        let in_escrow = if good == self.good {
            u64::from(self.caravan.escrow_good)
        } else {
            0
        };
        in_settlements + in_escrow
    }

    /// The regional gold total: Σ over settlements of their (closed) gold balance,
    /// plus any gold in route escrow.
    fn regional_gold(&self) -> u64 {
        let in_settlements: u64 = self.settlements.iter().map(|s| s.total_gold().0).sum();
        in_settlements + self.caravan.escrow_gold
    }

    // ---- accessors -----------------------------------------------------

    /// The realized spot price for `good` at the settlement with index
    /// `settlement` (0 = A, 1 = B), or `None` if no trade in that good has
    /// cleared there yet.
    pub fn realized_price(&self, settlement: usize, good: GoodId) -> Option<Gold> {
        self.settlements.get(settlement)?.realized_price(good)
    }

    /// The realized-price gap `|price_A(good) − price_B(good)|`, or `None` if
    /// either settlement has not yet cleared a trade in `good`. The convergence
    /// observable — sign only, no magnitude is pinned.
    pub fn price_gap(&self, good: GoodId) -> Option<u64> {
        let a = self.realized_price(0, good)?.0;
        let b = self.realized_price(1, good)?.0;
        Some(a.abs_diff(b))
    }

    /// The traded good.
    pub fn traded_good(&self) -> GoodId {
        self.good
    }

    /// Whether the caravan operates (the no-caravan control has this `false`).
    pub fn caravans_enabled(&self) -> bool {
        self.caravans_enabled
    }

    /// Units of the traded good currently in route escrow (mid-transit).
    pub fn escrow_good(&self) -> u32 {
        self.caravan.escrow_good
    }

    /// Gold currently in route escrow (mid-transit).
    pub fn escrow_gold(&self) -> u64 {
        self.caravan.escrow_gold
    }

    /// Read-only access to a settlement by index.
    pub fn settlement(&self, index: usize) -> Option<&Settlement> {
        self.settlements.get(index)
    }

    /// The settlements, in index order (0 = A, 1 = B).
    pub fn settlements(&self) -> &[Settlement] {
        &self.settlements
    }

    /// The number of settlements (two in G2c).
    pub fn settlement_count(&self) -> usize {
        self.settlements.len()
    }

    /// The goods tracked for region-wide conservation (`GoodId`-ordered).
    pub fn tracked_goods(&self) -> &[GoodId] {
        &self.goods
    }

    /// The most recent region econ tick's report.
    pub fn last_report(&self) -> &RegionTickReport {
        &self.last_report
    }

    /// Completed region econ ticks.
    pub fn econ_tick_count(&self) -> u64 {
        self.econ_tick
    }

    /// The resident-trader id at settlement `index` (its caravan trader), if any.
    pub fn trader_id(&self, index: usize) -> Option<AgentId> {
        match index {
            0 => Some(self.caravan.trader_a),
            1 => Some(self.caravan.trader_b),
            _ => None,
        }
    }

    // ---- determinism surface -------------------------------------------

    /// A canonical, order-stable byte serialization of the whole region — each
    /// settlement's canonical bytes plus the caravan phase, counter, and route
    /// escrow. Two regions are byte-identical iff these are equal (the determinism
    /// tripwire).
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.econ_tick.to_le_bytes());
        out.extend_from_slice(&(self.settlements.len() as u32).to_le_bytes());
        for settlement in &self.settlements {
            let bytes = settlement.canonical_bytes();
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(&bytes);
        }
        // Config-derived parameters that steer future ticks but are not otherwise
        // captured by the dynamic state below, so non-equivalent fresh regions
        // never digest equal merely because they have not diverged yet.
        out.extend_from_slice(&self.good.0.to_le_bytes());
        out.extend_from_slice(&self.route.transit_ticks.to_le_bytes());
        out.extend_from_slice(&self.buy_ticks.to_le_bytes());
        out.extend_from_slice(&self.sell_ticks.to_le_bytes());
        out.push(u8::from(self.caravans_enabled));

        // The caravan operator state that steers future ticks.
        out.push(match self.caravan.phase {
            Phase::BuyA => 0,
            Phase::ToB => 1,
            Phase::SellB => 2,
            Phase::ToA => 3,
            Phase::Hold => 4,
        });
        out.extend_from_slice(&self.caravan.counter.to_le_bytes());
        out.extend_from_slice(&self.caravan.escrow_good.to_le_bytes());
        out.extend_from_slice(&self.caravan.escrow_gold.to_le_bytes());
        out
    }

    /// A 64-bit FNV-1a digest of [`Region::canonical_bytes`] — a compact
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

/// Set a resident trader's value scale (the `Region`/caravan seam) and release any
/// now-stale resting quotes so its reservations match the new scale. Only ever
/// called on a resident-trader id, never a colonist.
fn set_trader_scale(settlement: &mut Settlement, trader: AgentId, scale: Vec<Want>) {
    let society = settlement.society_mut();
    if let Some(agent) = society.agents.get_mut(trader) {
        agent.scale = scale;
    }
    society.cancel_changed_live_quotes_for_agent(trader);
}

/// Idle a resident trader: an empty scale posts no orders, and cancelling its
/// quotes releases every reservation so its wealth can be withdrawn safely.
fn idle_trader(settlement: &mut Settlement, trader: AgentId) {
    set_trader_scale(settlement, trader, Vec::new());
}

/// Withdraw the trader's entire holding of `good` into the caller (route escrow),
/// returning the amount moved. Net-zero against the escrow credit the caller does.
///
/// G2c hauls **exactly one** good, so a trader only ever bids for `good` (or asks
/// GOLD) and never holds another tracked good. The single-good route escrow has no
/// slot for any other good; were a future multi-good engine to leave one in a
/// trader's inventory, this withdrawal would silently strand it (still conserved at
/// the settlement, but never hauled). The debug-assert pins that invariant so the
/// stranding fails loudly here rather than passing silently — generalizing escrow
/// to many goods is explicitly later work, not G2c.
fn withdraw_all_stock(settlement: &mut Settlement, trader: AgentId, good: GoodId) -> u32 {
    debug_assert!(
        settlement
            .society()
            .agents
            .get(trader)
            .map(|a| settlement
                .tracked_goods()
                .iter()
                .all(|&g| g == good || a.stock.get(g) == 0))
            .unwrap_or(true),
        "a G2c caravan trader held a tracked good other than the single traded good \
         {good:?}; the single-good route escrow would strand it"
    );
    let qty = settlement
        .society()
        .agents
        .get(trader)
        .map_or(0, |a| a.stock.get(good));
    if qty > 0 {
        let ok = settlement.society_mut().debit_stock(trader, good, qty);
        assert!(ok, "an idled resident trader's stock must debit cleanly");
    }
    qty
}

/// Withdraw the trader's entire gold into the caller (route escrow), returning the
/// amount moved. Net-zero against the escrow credit the caller does.
fn withdraw_all_gold(settlement: &mut Settlement, trader: AgentId) -> u64 {
    let amount = settlement
        .society()
        .agents
        .get(trader)
        .map_or(Gold::ZERO, |a| a.gold);
    if amount.0 > 0 {
        let ok = settlement.society_mut().debit_gold(trader, amount);
        assert!(ok, "an idled resident trader's gold must debit cleanly");
    }
    amount.0
}

/// Whether the configured A→B haul is favourable given the two realized prices:
/// A (the buyer side) must be no dearer than B (the seller side). Unknown prices
/// (no trade has cleared in that settlement yet) count as favourable, so the
/// caravan opens its market rather than stall before any price exists. The shared
/// buy-low/sell-high gate used at generation and at every fresh-haul boundary.
fn haul_is_favourable(price_a: Option<Gold>, price_b: Option<Gold>) -> bool {
    match (price_a, price_b) {
        (Some(a), Some(b)) => a <= b,
        _ => true,
    }
}

/// The buyer's value scale: a ladder of `Want{Good(good), Next}` units. `Next`
/// (not `Now`) makes the trader **acquire and hold** the good rather than consume
/// it — it bids for the good while it holds fewer units than the ladder is long,
/// and the held units are reserved (never eaten), so the caravan can carry them.
fn buy_scale(good: GoodId) -> Vec<Want> {
    (0..RegionConfig::BUY_LADDER_UNITS)
        .map(|_| Want {
            kind: WantKind::Good(good),
            horizon: Horizon::Next,
            qty: 1,
            satisfied: false,
        })
        .collect()
}

/// The seller's value scale: a ladder of `Want{Good(GOLD), Later}` units — a
/// standing money demand, exactly the mechanism that makes a gatherer sell its
/// surplus. With unmet money wants and the good in stock the trader asks the good
/// for gold (cheaply, since one coin satisfies the next want), so it sells what
/// the caravan handed it.
fn sell_scale() -> Vec<Want> {
    (0..RegionConfig::SELL_LADDER_UNITS)
        .map(|_| Want {
            kind: WantKind::Good(GOLD),
            horizon: Horizon::Later(4),
            qty: 1,
            satisfied: false,
        })
        .collect()
}
