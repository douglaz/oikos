//! DH.a (impl-68) — the closed circulation: the whole-population gold/physical provenance ledger,
//! the raw `ClosurePhysicalEvent` audit tape, the two independent reducers, and the pure
//! `classify_closure` verdict. All of it is pure OBSERVATION — active only under the
//! `closed_circulation` marker and altering no settlement (the DH.a inertness test proves it).
//!
//! See `docs/impl-closed-circulation.md`. The tape is PHYSICAL only (no raw gold events ride it);
//! the gold buckets are maintained by the production ledger alone (the sale-split rule driven by the
//! tape's physical decompositions), guarded by the `earned + endowed == agent.gold` invariant.

use econ::agent::AgentId;
use econ::good::{Gold, GoodId};
use std::collections::BTreeMap;

// ===========================================================================================
// Public vocabulary
// ===========================================================================================

/// The total, stable accounting class of a colonist (§3.1) — NOT its momentary vocation. Every
/// non-household colonist maps to `Gatherer`; every household member maps to its household's fixed
/// latent recipe (`Miller`/`Baker`), immutable and inherited by descendants. The ordinal order
/// (`Gatherer < Miller < Baker`) is the "lowest class ordinal" tie-break in `classify_closure`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClosureClass {
    Gatherer,
    Miller,
    Baker,
}

impl ClosureClass {
    /// The fixed required-class set (§3.3), in ascending ordinal order.
    pub const REQUIRED: [ClosureClass; 3] = [
        ClosureClass::Gatherer,
        ClosureClass::Miller,
        ClosureClass::Baker,
    ];

    /// The 0-based ordinal (`Gatherer = 0`), used to index the per-class arrays.
    pub fn index(self) -> usize {
        match self {
            ClosureClass::Gatherer => 0,
            ClosureClass::Miller => 1,
            ClosureClass::Baker => 2,
        }
    }
}

/// The event families that can post an endowed PHYSICAL debit (§3.2, CC2). Kept distinct so the
/// recount can compare `endowed_physical_debits` "by class AND event family" (R6-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClosureDebitFamily {
    Sale,
    Consumption,
    RecipeInput,
    CapitalInput,
    Spoilage,
}

/// One RAW physical event on the audit tape (§3.2): a `tick`/`order` stamp plus a variant-specific
/// signed committed leg. Carries NO origin buckets, NO decompositions, NO aggregates — origin
/// classification is a REDUCER's job. `SettledSpotTrade` carries the price only so the reducer can
/// price own-production sale consideration via `Gold::mul_qty`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosurePhysicalEvent {
    pub tick: u64,
    pub order: u32,
    pub kind: ClosureEventKind,
}

/// The variant-specific signed committed legs of a [`ClosurePhysicalEvent`] (§3.2 event table).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosureEventKind {
    /// The FINAL generated per-agent per-good holding MINUS the A2 component → `endowed`.
    InitialHolding {
        agent: AgentId,
        good: GoodId,
        qty: u32,
    },
    /// Exactly the A2 component of the generated holding (disjoint from `InitialHolding`) → `endowed`.
    A2FrontLoad {
        agent: AgentId,
        good: GoodId,
        qty: u32,
    },
    /// A Closed B arm's runtime support delivery → `endowed` (support is not production).
    BSupportCredit {
        agent: AgentId,
        good: GoodId,
        qty: u32,
    },
    /// A settled gathered-node deposit → `own_produced`.
    GatherDeposit {
        agent: AgentId,
        good: GoodId,
        qty: u32,
    },
    /// BOTH legs of one settled trade: seller stock debit (decomposed) AND buyer `acquired` credit.
    SettledSpotTrade {
        seller: AgentId,
        buyer: AgentId,
        good: GoodId,
        qty: u32,
        price: Gold,
        trade_id: u64,
    },
    /// Recipe application: input consumed (debit order) → output `own_produced`.
    RecipeProduction {
        agent: AgentId,
        input: GoodId,
        input_qty: u32,
        output: GoodId,
        output_qty: u32,
    },
    /// WOOD in → mill/oven out: input debited (order), tool credited `own_produced`.
    CapitalFormation {
        agent: AgentId,
        input: GoodId,
        input_qty: u32,
        tool: GoodId,
        tool_qty: u32,
    },
    /// Eating/warmth debit (debit order).
    Consumption {
        agent: AgentId,
        good: GoodId,
        qty: u32,
    },
    /// Per-agent perishable decay debit (a recorded sink; commons spoilage is NOT a per-agent event).
    Spoilage {
        agent: AgentId,
        good: GoodId,
        qty: u32,
    },
    /// Bucket-preserving physical transfer: birth staple, A1 `transfer_birth_stock`, estate-to-heir.
    HouseholdTransfer {
        from: AgentId,
        to: AgentId,
        good: GoodId,
        qty: u32,
    },
    /// Direct commons estates + unplaceable heir remainders: buckets removed; gross qty is the
    /// window's `commons_goods_drain`.
    EstateToCommons {
        agent: AgentId,
        good: GoodId,
        qty: u32,
    },
}

// ===========================================================================================
// Origin buckets (physical) and gold buckets
// ===========================================================================================

/// The three physical-origin buckets of a per-agent per-good inventory (§3.2). Every physical debit
/// consumes them in the FIXED anti-false-green order `endowed → acquired → own_produced`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct OriginBuckets {
    pub endowed: u32,
    pub acquired: u32,
    pub own_produced: u32,
}

/// The signed split a physical debit consumed, in the fixed order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DebitSplit {
    pub endowed: u32,
    pub acquired: u32,
    pub own_produced: u32,
}

impl OriginBuckets {
    pub fn total(self) -> u32 {
        self.endowed
            .saturating_add(self.acquired)
            .saturating_add(self.own_produced)
    }

    /// Consume `qty` in the FIXED order endowed → acquired → own_produced, returning the split.
    /// Debiting more than held saturates (the caller's reconciliation invariant catches drift).
    fn debit(&mut self, qty: u32) -> DebitSplit {
        let endowed = self.endowed.min(qty);
        self.endowed -= endowed;
        let mut rest = qty - endowed;
        let acquired = self.acquired.min(rest);
        self.acquired -= acquired;
        rest -= acquired;
        let own_produced = self.own_produced.min(rest);
        self.own_produced -= own_produced;
        DebitSplit {
            endowed,
            acquired,
            own_produced,
        }
    }
}

/// The two GOLD provenance buckets of a per-agent balance (§3.2), invariant `earned + endowed ==
/// agent.gold`. A purchase debits `earned` first, then `endowed`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GoldBuckets {
    pub earned: Gold,
    pub endowed: Gold,
}

impl GoldBuckets {
    pub fn total(self) -> Gold {
        self.earned.saturating_add(self.endowed)
    }

    /// Debit `amount` earned-first, then endowed (matching the producer-house rule). Returns
    /// `(earned_taken, endowed_taken)`; the endowed portion feeds CC2.
    fn debit(&mut self, amount: Gold) -> (Gold, Gold) {
        let earned = self.earned.0.min(amount.0);
        self.earned.0 -= earned;
        let endowed = self.endowed.0.min(amount.0 - earned);
        self.endowed.0 -= endowed;
        (Gold(earned), Gold(endowed))
    }
}

// ===========================================================================================
// classify_closure — the pure verdict
// ===========================================================================================

/// One classified window's per-criterion inputs (§3.3), fed to [`classify_closure`]. Per-class
/// fields are indexed by [`ClosureClass::index`] (`[Gatherer, Miller, Baker]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureWindow {
    pub start: u64,
    /// CC0: the class has ≥1 living member at every post-`econ_tick` sample in the window.
    pub present: [bool; 3],
    /// CC1: window own-production sale consideration (price × own_produced-bucket-qty).
    pub own_sale_consideration: [u64; 3],
    /// CC1: window settled market-purchase consideration (rule-3 debits only).
    pub purchase_consideration: [u64; 3],
    /// CC2: window sum of endowed portions of purchase gold debits.
    pub endowed_purchase_debits: [u64; 3],
    /// CC2: window `endowed_physical_debits` (endowed portion of every non-bucket-preserving debit).
    pub endowed_physical_debits: [u64; 3],
    /// CC3: window gold drained to commons.
    pub commons_drain: u64,
    /// CC3: gross goods routed into commons at the estate seam.
    pub commons_goods_drain: u64,
    /// CC3: wage escrow at the window boundary (must be 0 on the closed regime).
    pub wage_escrow_gold: u64,
    /// CC3: land-fee pool at the window boundary (must be 0 on the closed regime).
    pub land_fee_pool_salt: u64,
}

/// The named criterion a leak fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosureCriterion {
    Cc1,
    Cc2,
    Cc3,
}

/// The closure verdict (§3.3), a pure function of the classified windows. Never carried as a
/// display string — always the computed enum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClosureVerdict {
    /// Non-empty grid, every window passes CC0–CC3.
    ClosureHolds,
    /// The first failing window's failure is CC0 (a required class absent).
    ClosureStructureAbsent {
        first_window: u64,
        class: ClosureClass,
    },
    /// The earliest CC1/CC2/CC3-failing window, when no CC0 failure exists anywhere OR that window
    /// is strictly before the first CC0-failing window. `class = Some(lowest failing ordinal)` for
    /// CC1/CC2; `None` for CC3.
    ClosureLeaks {
        first_window: u64,
        criterion: ClosureCriterion,
        class: Option<ClosureClass>,
    },
    /// Empty grid — a measurement failure; defensive, so no path defaults to Holds.
    ClosureUndeterminedNoWindow,
}

/// Does the window satisfy CC0 (structure)? If not, the lowest failing class ordinal.
fn cc0_failing_class(w: &ClosureWindow) -> Option<ClosureClass> {
    ClosureClass::REQUIRED
        .into_iter()
        .find(|&c| !w.present[c.index()])
}

/// The lowest class ordinal that fails CC1 (own sale > 0 AND purchase > 0), else `None`.
fn cc1_failing_class(w: &ClosureWindow) -> Option<ClosureClass> {
    ClosureClass::REQUIRED.into_iter().find(|&c| {
        let i = c.index();
        w.own_sale_consideration[i] == 0 || w.purchase_consideration[i] == 0
    })
}

/// The lowest class ordinal that fails CC2 (no endowed drawdown, monetary AND physical), else `None`.
fn cc2_failing_class(w: &ClosureWindow) -> Option<ClosureClass> {
    ClosureClass::REQUIRED.into_iter().find(|&c| {
        let i = c.index();
        w.endowed_purchase_debits[i] != 0 || w.endowed_physical_debits[i] != 0
    })
}

/// Does the window fail CC3 (any drain nonzero)?
fn cc3_fails(w: &ClosureWindow) -> bool {
    w.commons_drain != 0
        || w.commons_goods_drain != 0
        || w.wage_escrow_gold != 0
        || w.land_fee_pool_salt != 0
}

/// The earliest CC1/CC2/CC3 leak (evaluation order CC1 → CC2 → CC3 within a window), returning the
/// window start, criterion, and class payload.
fn first_leak(windows: &[ClosureWindow]) -> Option<(u64, ClosureCriterion, Option<ClosureClass>)> {
    for w in windows {
        if let Some(class) = cc1_failing_class(w) {
            return Some((w.start, ClosureCriterion::Cc1, Some(class)));
        }
        if let Some(class) = cc2_failing_class(w) {
            return Some((w.start, ClosureCriterion::Cc2, Some(class)));
        }
        if cc3_fails(w) {
            return Some((w.start, ClosureCriterion::Cc3, None));
        }
    }
    None
}

/// The pure closure verdict over the POST-BOOTSTRAP classified windows (§3.3). Empty input is
/// checked FIRST and returns `ClosureUndeterminedNoWindow`; `ClosureHolds` requires a non-empty grid
/// with every window passing CC0–CC3. Within a window CC0 precedes CC1/CC2/CC3; a leak is reported
/// only when NO CC0 failure exists anywhere OR the leak is STRICTLY BEFORE the first CC0 failure.
pub fn classify_closure(windows: &[ClosureWindow]) -> ClosureVerdict {
    if windows.is_empty() {
        return ClosureVerdict::ClosureUndeterminedNoWindow;
    }

    let first_cc0 = windows
        .iter()
        .find_map(|w| cc0_failing_class(w).map(|class| (w.start, class)));
    let leak = first_leak(windows);

    match (first_cc0, leak) {
        (None, None) => ClosureVerdict::ClosureHolds,
        (None, Some((first_window, criterion, class))) => ClosureVerdict::ClosureLeaks {
            first_window,
            criterion,
            class,
        },
        (Some((cc0_window, cc0_class)), leak) => {
            // A leak strictly before the first CC0 failure takes precedence; otherwise the CC0
            // failure decides (within a window CC0 precedes, so a window both structure-absent and
            // leaking reads as CC0).
            match leak {
                Some((leak_window, criterion, class)) if leak_window < cc0_window => {
                    ClosureVerdict::ClosureLeaks {
                        first_window: leak_window,
                        criterion,
                        class,
                    }
                }
                _ => ClosureVerdict::ClosureStructureAbsent {
                    first_window: cc0_window,
                    class: cc0_class,
                },
            }
        }
    }
}

// ===========================================================================================
// The runtime ledger (data holder; observation wired in the `impl Settlement` block below)
// ===========================================================================================

/// The whole-population gold/physical provenance ledger + raw event tape (§3.2), a field on
/// [`crate::Settlement`]. Maintained only under the `closed_circulation` marker; pure observation.
#[derive(Clone, Debug, Default)]
pub(crate) struct ClosureLedger {
    /// Test-only force-disable (§3.3 inertness proof). When true the whole closure observation is
    /// skipped, so a marker-on run with the ledger disabled is byte-identical to one with it
    /// enabled.
    pub disabled: bool,
    /// The actor → class registry (total; the ONLY auxiliary replay input besides the tape).
    pub registry: BTreeMap<AgentId, ClosureClass>,
    /// The per-household fixed class (Miller/Baker), used to classify newborns.
    pub household_class: BTreeMap<usize, ClosureClass>,
    /// Per-agent gold buckets (production ledger only; NOT tape-derived, NOT in the recount).
    pub gold: BTreeMap<AgentId, GoldBuckets>,
    /// Per-agent per-good physical origin buckets (the production physical reducer's state).
    pub inv: BTreeMap<AgentId, BTreeMap<GoodId, OriginBuckets>>,
    /// The running physical snapshot for phase diffing (real stock, excluding GOLD).
    pub prev_stock: BTreeMap<AgentId, BTreeMap<GoodId, u32>>,
    /// The RAW physical audit tape (all ticks, in mutation order).
    pub tape: Vec<ClosurePhysicalEvent>,
    /// Per-tick production-reducer aggregates (windowed by the oracle / classify caller).
    pub ticks: Vec<ClosureTickAgg>,
    /// The accumulator for the tick currently being observed.
    pub cur: ClosureTickAgg,
    /// The monotonic tape event counter (the `order` stamp), never reset.
    pub order: u32,
    /// Actual estate placements stashed by the `record_estate_destination` hook, consumed by the
    /// post-death estate observation. Each good records `(placed_with_heir, placed_with_commons)`.
    pub pending_estate: BTreeMap<AgentId, ClosureEstateRouting>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ClosureEstateRouting {
    /// The actual gold recipient. This is independent of physical placement because a stale or
    /// overflowing intended heir sends gold to commons even when some goods still fit the heir.
    pub gold_heir: Option<AgentId>,
    /// The intended household heir used for each good's actual heir/commons placement split.
    pub heir: Option<AgentId>,
    pub goods: BTreeMap<GoodId, (u64, u64)>,
}

/// One tick's production-reducer aggregates (§3.2/§3.3). Per-class fields are indexed by
/// [`ClosureClass::index`]. Summed over a 160-tick window to build a [`ClosureWindow`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClosureTickAgg {
    pub tick: u64,
    /// CC0: the class has ≥1 living member at this post-`econ_tick` sample.
    pub living: [bool; 3],
    /// CC1: own-production sale consideration (price × own_produced-bucket-qty).
    pub own_sale_consideration: [u64; 3],
    /// CC1: settled market-purchase consideration (rule-3 debits).
    pub purchase_consideration: [u64; 3],
    /// CC2: endowed portion of purchase gold debits.
    pub endowed_purchase_debits: [u64; 3],
    /// CC2: endowed portion of every non-bucket-preserving physical debit, by class.
    pub endowed_physical_debits: [u64; 3],
    /// CC2 recount detail: endowed physical debits by (class, family), row order per
    /// [`ClosureDebitFamily`] (Sale, Consumption, RecipeInput, CapitalInput, Spoilage).
    pub endowed_physical_debits_by_family: [[u64; 5]; 3],
    /// CC3: gold drained to commons this tick.
    pub commons_drain: u64,
    /// CC3: gross goods routed into commons at the estate seam this tick.
    pub commons_goods_drain: u64,
    /// CC3: wage escrow at this tick's boundary.
    pub wage_escrow_gold: u64,
    /// CC3: land-fee pool at this tick's boundary.
    pub land_fee_pool_salt: u64,
}

impl ClosureDebitFamily {
    /// The 0-based row index (Sale, Consumption, RecipeInput, CapitalInput, Spoilage).
    pub fn index(self) -> usize {
        match self {
            ClosureDebitFamily::Sale => 0,
            ClosureDebitFamily::Consumption => 1,
            ClosureDebitFamily::RecipeInput => 2,
            ClosureDebitFamily::CapitalInput => 3,
            ClosureDebitFamily::Spoilage => 4,
        }
    }
}

#[cfg(test)]
mod classify_tests {
    use super::*;

    /// A window that passes CC0–CC3 for every required class.
    fn pass_window(start: u64) -> ClosureWindow {
        ClosureWindow {
            start,
            present: [true; 3],
            own_sale_consideration: [1, 1, 1],
            purchase_consideration: [1, 1, 1],
            endowed_purchase_debits: [0; 3],
            endowed_physical_debits: [0; 3],
            commons_drain: 0,
            commons_goods_drain: 0,
            wage_escrow_gold: 0,
            land_fee_pool_salt: 0,
        }
    }

    #[test]
    fn empty_grid_is_undetermined_and_never_holds() {
        assert_eq!(
            classify_closure(&[]),
            ClosureVerdict::ClosureUndeterminedNoWindow
        );
    }

    #[test]
    fn all_windows_pass_holds() {
        let windows = [pass_window(160), pass_window(320)];
        assert_eq!(classify_closure(&windows), ClosureVerdict::ClosureHolds);
    }

    #[test]
    fn cc0_absent_reports_lowest_failing_class() {
        // Only Baker (highest ordinal) present-fails → class = Baker.
        let mut w = pass_window(160);
        w.present[ClosureClass::Baker.index()] = false;
        assert_eq!(
            classify_closure(&[w]),
            ClosureVerdict::ClosureStructureAbsent {
                first_window: 160,
                class: ClosureClass::Baker,
            }
        );
        // Gatherer AND Miller present-fail → the LOWEST ordinal (Gatherer) is reported.
        let mut w = pass_window(160);
        w.present[ClosureClass::Gatherer.index()] = false;
        w.present[ClosureClass::Miller.index()] = false;
        assert_eq!(
            classify_closure(&[w]),
            ClosureVerdict::ClosureStructureAbsent {
                first_window: 160,
                class: ClosureClass::Gatherer,
            }
        );
    }

    #[test]
    fn cc1_leak_with_stable_structure() {
        // Baker earns nothing this window (own sale == 0) but structure holds everywhere.
        let mut w = pass_window(160);
        w.own_sale_consideration[ClosureClass::Baker.index()] = 0;
        assert_eq!(
            classify_closure(&[w, pass_window(320)]),
            ClosureVerdict::ClosureLeaks {
                first_window: 160,
                criterion: ClosureCriterion::Cc1,
                class: Some(ClosureClass::Baker),
            }
        );
    }

    #[test]
    fn cc1_leak_on_zero_purchase() {
        let mut w = pass_window(160);
        w.purchase_consideration[ClosureClass::Miller.index()] = 0;
        assert_eq!(
            classify_closure(&[w]),
            ClosureVerdict::ClosureLeaks {
                first_window: 160,
                criterion: ClosureCriterion::Cc1,
                class: Some(ClosureClass::Miller),
            }
        );
    }

    #[test]
    fn cc2_leak_monetary_and_physical() {
        // Monetary endowed drawdown.
        let mut w = pass_window(160);
        w.endowed_purchase_debits[ClosureClass::Gatherer.index()] = 3;
        assert_eq!(
            classify_closure(&[w]),
            ClosureVerdict::ClosureLeaks {
                first_window: 160,
                criterion: ClosureCriterion::Cc2,
                class: Some(ClosureClass::Gatherer),
            }
        );
        // Physical endowed drawdown.
        let mut w = pass_window(160);
        w.endowed_physical_debits[ClosureClass::Baker.index()] = 1;
        assert_eq!(
            classify_closure(&[w]),
            ClosureVerdict::ClosureLeaks {
                first_window: 160,
                criterion: ClosureCriterion::Cc2,
                class: Some(ClosureClass::Baker),
            }
        );
    }

    #[test]
    fn cc1_precedes_cc2_within_a_window() {
        // A window that fails BOTH CC1 (zero own sale) and CC2 (endowed physical) for a class:
        // evaluation order reports CC1.
        let mut w = pass_window(160);
        w.own_sale_consideration[ClosureClass::Gatherer.index()] = 0;
        w.endowed_physical_debits[ClosureClass::Gatherer.index()] = 5;
        assert_eq!(
            classify_closure(&[w]),
            ClosureVerdict::ClosureLeaks {
                first_window: 160,
                criterion: ClosureCriterion::Cc1,
                class: Some(ClosureClass::Gatherer),
            }
        );
    }

    #[test]
    fn cc3_leak_is_a_global_drain() {
        for mutate in [
            |w: &mut ClosureWindow| w.commons_drain = 1,
            |w: &mut ClosureWindow| w.commons_goods_drain = 1,
            |w: &mut ClosureWindow| w.wage_escrow_gold = 1,
            |w: &mut ClosureWindow| w.land_fee_pool_salt = 1,
        ] {
            let mut w = pass_window(160);
            mutate(&mut w);
            assert_eq!(
                classify_closure(&[w]),
                ClosureVerdict::ClosureLeaks {
                    first_window: 160,
                    criterion: ClosureCriterion::Cc3,
                    class: None,
                }
            );
        }
    }

    #[test]
    fn leak_strictly_before_cc0_failure_takes_precedence() {
        let mut leak = pass_window(160);
        leak.own_sale_consideration[ClosureClass::Baker.index()] = 0; // CC1 leak at 160
        let mut absent = pass_window(320);
        absent.present[ClosureClass::Miller.index()] = false; // CC0 at 320
        assert_eq!(
            classify_closure(&[leak, absent]),
            ClosureVerdict::ClosureLeaks {
                first_window: 160,
                criterion: ClosureCriterion::Cc1,
                class: Some(ClosureClass::Baker),
            }
        );
    }

    #[test]
    fn leak_at_or_after_cc0_failure_reads_as_cc0() {
        // CC0 at 160, a later leak at 320 → StructureAbsent (the CC0 window is earliest).
        let mut absent = pass_window(160);
        absent.present[ClosureClass::Miller.index()] = false;
        let mut leak = pass_window(320);
        leak.commons_drain = 7;
        assert_eq!(
            classify_closure(&[absent, leak]),
            ClosureVerdict::ClosureStructureAbsent {
                first_window: 160,
                class: ClosureClass::Miller,
            }
        );
    }

    #[test]
    fn same_window_cc0_and_leak_reads_as_cc0() {
        // One window both structure-absent AND leaking: CC0 precedes within a window.
        let mut w = pass_window(160);
        w.present[ClosureClass::Baker.index()] = false;
        w.commons_goods_drain = 4;
        assert_eq!(
            classify_closure(&[w]),
            ClosureVerdict::ClosureStructureAbsent {
                first_window: 160,
                class: ClosureClass::Baker,
            }
        );
    }

    #[test]
    fn zero_activity_window_fails_cc1_not_cc0() {
        // Structure present but nobody trades: CC1 leak on the lowest class (Gatherer).
        let mut w = pass_window(160);
        w.own_sale_consideration = [0; 3];
        w.purchase_consideration = [0; 3];
        assert_eq!(
            classify_closure(&[w]),
            ClosureVerdict::ClosureLeaks {
                first_window: 160,
                criterion: ClosureCriterion::Cc1,
                class: Some(ClosureClass::Gatherer),
            }
        );
    }
}

// ===========================================================================================
// The reducer (physical) + gold buckets — ClosureLedger methods
// ===========================================================================================

impl ClosureLedger {
    fn class_of(&self, agent: AgentId) -> Option<ClosureClass> {
        self.registry.get(&agent).copied()
    }

    fn inv_credit(&mut self, agent: AgentId, good: GoodId, qty: u32, origin: Origin) {
        if qty == 0 {
            return;
        }
        let bucket = self.inv.entry(agent).or_default().entry(good).or_default();
        match origin {
            Origin::Endowed => bucket.endowed = bucket.endowed.saturating_add(qty),
            Origin::Acquired => bucket.acquired = bucket.acquired.saturating_add(qty),
            Origin::OwnProduced => bucket.own_produced = bucket.own_produced.saturating_add(qty),
        }
    }

    fn inv_debit(&mut self, agent: AgentId, good: GoodId, qty: u32) -> DebitSplit {
        if qty == 0 {
            return DebitSplit::default();
        }
        let bucket = self.inv.entry(agent).or_default().entry(good).or_default();
        bucket.debit(qty)
    }

    /// Record a physical event on the tape and apply it to the physical reducer state (buckets +
    /// physical aggregates). Returns the seller's debit split for a `SettledSpotTrade` (which the
    /// caller uses to split the sale proceeds into gold buckets); `None` otherwise.
    fn record(&mut self, tick: u64, kind: ClosureEventKind) -> Option<DebitSplit> {
        let order = self.order;
        self.order = self.order.saturating_add(1);
        let ev = ClosurePhysicalEvent { tick, order, kind };
        self.tape.push(ev);
        self.apply_physical(kind)
    }

    /// The physical reducer (production side; the reference reducer in the test re-implements this
    /// from the tape + registry). Updates `inv` and the per-tick physical aggregates in `cur`.
    fn apply_physical(&mut self, kind: ClosureEventKind) -> Option<DebitSplit> {
        match kind {
            ClosureEventKind::InitialHolding { agent, good, qty }
            | ClosureEventKind::A2FrontLoad { agent, good, qty }
            | ClosureEventKind::BSupportCredit { agent, good, qty } => {
                self.inv_credit(agent, good, qty, Origin::Endowed);
            }
            ClosureEventKind::GatherDeposit { agent, good, qty } => {
                self.inv_credit(agent, good, qty, Origin::OwnProduced);
            }
            ClosureEventKind::SettledSpotTrade {
                seller,
                buyer,
                good,
                qty,
                price,
                trade_id: _,
            } => {
                let split = self.inv_debit(seller, good, qty);
                self.inv_credit(buyer, good, qty, Origin::Acquired);
                // CC1: own-production sale consideration (price × own_produced-bucket-qty) to the
                // seller's class; settled market-purchase consideration (price × qty) to the buyer's.
                if let Some(seller_class) = self.class_of(seller) {
                    let own = price.mul_qty(split.own_produced).unwrap_or(Gold::ZERO);
                    let slot = &mut self.cur.own_sale_consideration[seller_class.index()];
                    *slot = slot.saturating_add(own.0);
                    self.post_endowed_physical_debit(
                        seller_class,
                        ClosureDebitFamily::Sale,
                        split.endowed,
                    );
                }
                if let Some(buyer_class) = self.class_of(buyer) {
                    let paid = price.mul_qty(qty).unwrap_or(Gold::ZERO);
                    let slot = &mut self.cur.purchase_consideration[buyer_class.index()];
                    *slot = slot.saturating_add(paid.0);
                }
                return Some(split);
            }
            ClosureEventKind::RecipeProduction {
                agent,
                input,
                input_qty,
                output,
                output_qty,
            } => {
                let split = self.inv_debit(agent, input, input_qty);
                self.inv_credit(agent, output, output_qty, Origin::OwnProduced);
                if let Some(class) = self.class_of(agent) {
                    self.post_endowed_physical_debit(
                        class,
                        ClosureDebitFamily::RecipeInput,
                        split.endowed,
                    );
                }
            }
            ClosureEventKind::CapitalFormation {
                agent,
                input,
                input_qty,
                tool,
                tool_qty,
            } => {
                let split = self.inv_debit(agent, input, input_qty);
                self.inv_credit(agent, tool, tool_qty, Origin::OwnProduced);
                if let Some(class) = self.class_of(agent) {
                    self.post_endowed_physical_debit(
                        class,
                        ClosureDebitFamily::CapitalInput,
                        split.endowed,
                    );
                }
            }
            ClosureEventKind::Consumption { agent, good, qty } => {
                let split = self.inv_debit(agent, good, qty);
                if let Some(class) = self.class_of(agent) {
                    self.post_endowed_physical_debit(
                        class,
                        ClosureDebitFamily::Consumption,
                        split.endowed,
                    );
                }
            }
            ClosureEventKind::Spoilage { agent, good, qty } => {
                let split = self.inv_debit(agent, good, qty);
                if let Some(class) = self.class_of(agent) {
                    self.post_endowed_physical_debit(
                        class,
                        ClosureDebitFamily::Spoilage,
                        split.endowed,
                    );
                }
            }
            ClosureEventKind::HouseholdTransfer {
                from,
                to,
                good,
                qty,
            } => {
                // Bucket-preserving: the source's consumed lots re-credit the recipient into the
                // SAME buckets. Excluded from CC1 outflow and CC2 drawdown.
                let split = self.inv_debit(from, good, qty);
                self.inv_credit(to, good, split.endowed, Origin::Endowed);
                self.inv_credit(to, good, split.acquired, Origin::Acquired);
                self.inv_credit(to, good, split.own_produced, Origin::OwnProduced);
            }
            ClosureEventKind::EstateToCommons { agent, good, qty } => {
                // Buckets removed; the gross quantity is the window's commons_goods_drain (CC3), NOT
                // a CC2 endowed physical debit.
                self.inv_debit(agent, good, qty);
                self.cur.commons_goods_drain =
                    self.cur.commons_goods_drain.saturating_add(u64::from(qty));
            }
        }
        None
    }

    fn post_endowed_physical_debit(
        &mut self,
        class: ClosureClass,
        family: ClosureDebitFamily,
        endowed: u32,
    ) {
        if endowed == 0 {
            return;
        }
        let c = class.index();
        let slot = &mut self.cur.endowed_physical_debits[c];
        *slot = slot.saturating_add(u64::from(endowed));
        let fam = &mut self.cur.endowed_physical_debits_by_family[c][family.index()];
        *fam = fam.saturating_add(u64::from(endowed));
    }

    // ---- Gold buckets (production ledger only; not tape-derived, not in the recount) ----

    fn gold_credit(&mut self, agent: AgentId, amount: Gold, earned: bool) {
        if amount == Gold::ZERO {
            return;
        }
        let bucket = self.gold.entry(agent).or_default();
        if earned {
            bucket.earned = bucket.earned.saturating_add(amount);
        } else {
            bucket.endowed = bucket.endowed.saturating_add(amount);
        }
    }

    fn gold_debit(&mut self, agent: AgentId, amount: Gold) -> (Gold, Gold) {
        if amount == Gold::ZERO {
            return (Gold::ZERO, Gold::ZERO);
        }
        self.gold.entry(agent).or_default().debit(amount)
    }

    /// Bucket-preserving gold move (rules 4–6): the source is debited earned-first and the exact
    /// (earned, endowed) split re-credits the recipient. Non-spending; excluded from CC1/CC2.
    fn gold_transfer_preserving(&mut self, from: AgentId, to: AgentId, amount: Gold) {
        let (earned, endowed) = self.gold_debit(from, amount);
        self.gold_credit(to, earned, true);
        self.gold_credit(to, endowed, false);
    }

    /// Rule 3: debit a buyer's purchase earned-first, then endowed. Returns the ENDOWED portion —
    /// the endowed-purchase-debit that feeds CC2 (liquidation-funded demand).
    fn gold_purchase_debit(&mut self, buyer: AgentId, paid: Gold) -> Gold {
        let (_earned, endowed) = self.gold_debit(buyer, paid);
        endowed
    }

    /// Rule 2: credit a seller's sale proceeds split by the sale's physical decomposition — the
    /// pro-rata `endowed`-origin portion (liquidation, not income) → endowed gold; the
    /// `own_produced` + `acquired` portions (production + resale income) → earned gold.
    fn gold_sale_credit(&mut self, seller: AgentId, price: Gold, split: DebitSplit) {
        let endowed_proceeds = price.mul_qty(split.endowed).unwrap_or(Gold::ZERO);
        let earned_qty = split.acquired.saturating_add(split.own_produced);
        let earned_proceeds = price.mul_qty(earned_qty).unwrap_or(Gold::ZERO);
        self.gold_credit(seller, earned_proceeds, true);
        self.gold_credit(seller, endowed_proceeds, false);
    }
}

/// The origin a physical credit carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    Endowed,
    Acquired,
    OwnProduced,
}

// ===========================================================================================
// The live observation — impl Settlement (pure observation; touches no serialized state)
// ===========================================================================================

use super::{Settlement, Vocation};
use econ::good::GOLD;
use econ::project::RecipeId;
use std::collections::BTreeSet;

/// The stock-mutating phases the closure observation diffs (§3.2 event table). Each maps a
/// per-agent per-good positive delta to a credit origin and a negative delta to a debit family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClosurePhase {
    /// Settled gathered-node deposits → `own_produced`.
    Gather,
    /// WOOD in → mill/oven out.
    Capital,
    /// B-arm runtime support deliveries → `endowed`.
    Support,
    /// Recipe application (miller/baker) → input debit, output `own_produced`.
    Production,
    /// Own-use production/consumption (own-labor subsistence, cultivation, emergency floor).
    OwnUse,
    /// Per-agent perishable decay.
    Spoilage,
}

/// A colonist's class from its latent recipe (preferred) or its seeded vocation.
fn class_from_recipe_or_vocation(
    latent: Option<RecipeId>,
    vocation: Vocation,
) -> Option<ClosureClass> {
    match latent {
        Some(RecipeId::Mill) => Some(ClosureClass::Miller),
        Some(RecipeId::Bake) => Some(ClosureClass::Baker),
        _ => match vocation {
            Vocation::Miller => Some(ClosureClass::Miller),
            Vocation::Baker => Some(ClosureClass::Baker),
            _ => None,
        },
    }
}

impl Settlement {
    /// The closure observation is live iff the marker is set AND the test-only force-disable hook is
    /// off. Everything downstream gates on this, so a disabled marker-on run is byte-identical.
    pub(crate) fn closure_active(&self) -> bool {
        self.closed_circulation && !self.closure.disabled
    }

    /// Test-only force-disable hook (§3.3): flip the ledger off so a marker-on run does no
    /// observation. Conservation-safe only when called before the first `econ_tick`.
    pub fn closure_ledger_force_disable_for_test(&mut self) {
        self.closure.disabled = true;
    }

    /// A living agent's stock, excluding GOLD (money is tracked in the gold buckets).
    fn closure_snapshot_stock(&self) -> BTreeMap<AgentId, BTreeMap<GoodId, u32>> {
        let mut out = BTreeMap::new();
        for &slot in &self.live_colonist_slots {
            let id = self.colonists[slot].id;
            if let Some(agent) = self.society.agents.get(id) {
                let goods: BTreeMap<GoodId, u32> = agent
                    .stock
                    .positive_goods()
                    .filter(|&g| g != GOLD)
                    .map(|g| (g, agent.stock.get(g)))
                    .collect();
                out.insert(id, goods);
            }
        }
        out
    }

    fn closure_resync_prev_stock(&mut self) {
        self.closure.prev_stock = self.closure_snapshot_stock();
    }

    /// Build the registry + household_class, seed the endowed gold/physical buckets, emit the
    /// `InitialHolding`/`A2FrontLoad` tape (§3.2), and snapshot the opening inventory. Called once,
    /// at the end of generation (before the first `econ_tick`).
    pub(crate) fn closure_init(&mut self) {
        if !self.closed_circulation {
            return;
        }
        let staple = self.known.hunger;
        let a2 = self
            .chain
            .as_ref()
            .map_or(0, |chain| chain.producer_house_starting_staple);

        // Pass 1: household → fixed class, from the founding latent/seeded producers.
        let mut household_class: BTreeMap<usize, ClosureClass> = BTreeMap::new();
        for &slot in &self.live_colonist_slots {
            let c = &self.colonists[slot];
            if let Some(h) = c.household {
                if let Some(class) = class_from_recipe_or_vocation(c.latent, c.vocation) {
                    household_class.entry(h).or_insert(class);
                }
            }
        }
        // Pass 2: the total registry.
        let mut registry: BTreeMap<AgentId, ClosureClass> = BTreeMap::new();
        for &slot in &self.live_colonist_slots {
            let c = &self.colonists[slot];
            let class = match c.household {
                Some(h) => *household_class
                    .get(&h)
                    .expect("every producer household has a founding class"),
                None => {
                    debug_assert_eq!(
                        c.home_vocation,
                        Vocation::Gatherer,
                        "closed regime: every non-household colonist is a gatherer"
                    );
                    ClosureClass::Gatherer
                }
            };
            registry.insert(c.id, class);
        }
        self.closure.household_class = household_class;
        self.closure.registry = registry;

        // Seed the endowed buckets and emit InitialHolding / A2FrontLoad.
        let subjects: Vec<(AgentId, bool)> = self
            .live_colonist_slots
            .iter()
            .map(|&slot| {
                let id = self.colonists[slot].id;
                (id, self.is_producer_subject_id(id))
            })
            .collect();
        for (id, is_subject) in subjects {
            let (gold, goods) = self.society.agents.get(id).map_or_else(
                || (Gold::ZERO, Vec::new()),
                |agent| {
                    let goods: Vec<(GoodId, u32)> = agent
                        .stock
                        .positive_goods()
                        .filter(|&g| g != GOLD)
                        .map(|g| (g, agent.stock.get(g)))
                        .collect();
                    (agent.gold, goods)
                },
            );
            self.closure.gold_credit(id, gold, false);
            for (good, qty) in goods {
                let a2_qty = if good == staple && is_subject {
                    a2.min(qty)
                } else {
                    0
                };
                let init_qty = qty - a2_qty;
                self.closure.record(
                    0,
                    ClosureEventKind::InitialHolding {
                        agent: id,
                        good,
                        qty: init_qty,
                    },
                );
                if a2_qty > 0 {
                    self.closure.record(
                        0,
                        ClosureEventKind::A2FrontLoad {
                            agent: id,
                            good,
                            qty: a2_qty,
                        },
                    );
                }
            }
        }
        self.closure_resync_prev_stock();
    }

    /// Reset the per-tick aggregate accumulator at the top of `econ_tick`.
    pub(crate) fn closure_begin_tick(&mut self) {
        self.closure.cur = ClosureTickAgg {
            tick: self.econ_tick,
            ..Default::default()
        };
    }

    /// Diff the current stock against `prev_stock`, emit the phase's events, apply them to the
    /// reducer, and advance `prev_stock`. Pure observation.
    pub(crate) fn closure_phase(&mut self, phase: ClosurePhase) {
        let now = self.closure_snapshot_stock();
        let tick = self.econ_tick;
        let mut events: Vec<ClosureEventKind> = Vec::new();
        let ids: BTreeSet<AgentId> = now
            .keys()
            .chain(self.closure.prev_stock.keys())
            .copied()
            .collect();
        for id in ids {
            let prev = self.closure.prev_stock.get(&id);
            let cur = now.get(&id);
            let goods: BTreeSet<GoodId> = prev
                .into_iter()
                .flat_map(|m| m.keys().copied())
                .chain(cur.into_iter().flat_map(|m| m.keys().copied()))
                .collect();
            let mut gains: Vec<(GoodId, u32)> = Vec::new();
            let mut losses: Vec<(GoodId, u32)> = Vec::new();
            for good in goods {
                let p = prev.and_then(|m| m.get(&good)).copied().unwrap_or(0);
                let c = cur.and_then(|m| m.get(&good)).copied().unwrap_or(0);
                if c > p {
                    gains.push((good, c - p));
                } else if p > c {
                    losses.push((good, p - c));
                }
            }
            closure_phase_events(phase, id, gains, losses, &mut events);
        }
        for kind in events {
            self.closure.record(tick, kind);
        }
        self.closure.prev_stock = now;
    }

    /// Observe the market batch (§3.2): replay the authoritative consumption log first, matching
    /// `Society::step_m1`, then process the tick's settled spot trades explicitly (both physical legs
    /// on the tape + the gold sale-split). Reconciles the buckets against reality afterward.
    pub(crate) fn closure_observe_market(&mut self, spot_start: usize) {
        let tick = self.econ_tick;
        let now = self.closure_snapshot_stock();
        let consumptions = self.society.consumption_log_last_tick().to_vec();
        // Snapshot the tick's spot trades.
        let trades: Vec<(AgentId, AgentId, GoodId, u32, Gold)> = self.society.trades[spot_start..]
            .iter()
            .map(|t| (t.seller, t.buyer, t.good, t.qty, t.price))
            .collect();
        for (agent, good, qty) in consumptions {
            self.closure
                .record(tick, ClosureEventKind::Consumption { agent, good, qty });
        }
        for (i, &(seller, buyer, good, qty, price)) in trades.iter().enumerate() {
            let trade_id = spot_start as u64 + i as u64;
            let split = self
                .closure
                .record(
                    tick,
                    ClosureEventKind::SettledSpotTrade {
                        seller,
                        buyer,
                        good,
                        qty,
                        price,
                        trade_id,
                    },
                )
                .unwrap_or_default();
            // Gold: buyer debits earned-first (endowed portion feeds CC2); seller proceeds split by
            // the physical decomposition (endowed-origin proceeds → endowed gold, else → earned).
            let paid = price.mul_qty(qty).unwrap_or(Gold::ZERO);
            let endowed_paid = self.closure.gold_purchase_debit(buyer, paid);
            if let Some(bc) = self.closure.class_of(buyer) {
                let slot = &mut self.closure.cur.endowed_purchase_debits[bc.index()];
                *slot = slot.saturating_add(endowed_paid.0);
            }
            self.closure.gold_sale_credit(seller, price, split);
        }
        self.closure.prev_stock = now;
        self.closure_reconcile("post-market-batch");
    }

    /// Route the estates of agents that died this tick (§3.2 rules 6–7). Dead agents are producers
    /// (gatherers are immortal, non-spatial), so a dead agent's estate is exactly its ledgered
    /// buckets: to a heir (bucket-preserving) or to the commons (removed + drain recorded).
    pub(crate) fn closure_observe_estates(&mut self) {
        let current: BTreeSet<AgentId> = self
            .live_colonist_slots
            .iter()
            .map(|&slot| self.colonists[slot].id)
            .collect();
        let dead: Vec<AgentId> = self
            .closure
            .prev_stock
            .keys()
            .copied()
            .filter(|id| !current.contains(id))
            .collect();
        let tick = self.econ_tick;
        for dead_id in dead {
            let routing = self
                .closure
                .pending_estate
                .remove(&dead_id)
                .expect("every closed-regime death records its actual estate routing");
            // Gold (production ledger only).
            let gold_b = self.closure.gold.remove(&dead_id).unwrap_or_default();
            match routing.gold_heir {
                Some(h) => {
                    self.closure.gold_credit(h, gold_b.earned, true);
                    self.closure.gold_credit(h, gold_b.endowed, false);
                }
                None => {
                    self.closure.cur.commons_drain = self
                        .closure
                        .cur
                        .commons_drain
                        .saturating_add(gold_b.total().0);
                }
            }
            // Physical (tape events; apply_physical moves/removes the buckets).
            let goods: Vec<(GoodId, u32)> = self
                .closure
                .inv
                .get(&dead_id)
                .map(|m| m.iter().map(|(&g, b)| (g, b.total())).collect())
                .unwrap_or_default();
            for (good, qty) in goods {
                if qty == 0 {
                    continue;
                }
                let (heir_qty, commons_qty) = routing
                    .goods
                    .get(&good)
                    .copied()
                    .expect("every ledgered estate holding records its actual placement");
                debug_assert_eq!(
                    heir_qty.saturating_add(commons_qty),
                    u64::from(qty),
                    "closed-regime estate routing must place every ledgered holding exactly once"
                );
                if heir_qty > 0 {
                    let h = routing
                        .heir
                        .expect("goods placed with an heir require an heir destination");
                    self.closure.record(
                        tick,
                        ClosureEventKind::HouseholdTransfer {
                            from: dead_id,
                            to: h,
                            good,
                            qty: u32::try_from(heir_qty)
                                .expect("ledgered heir placement fits a stock balance"),
                        },
                    );
                }
                if commons_qty > 0 {
                    self.closure.record(
                        tick,
                        ClosureEventKind::EstateToCommons {
                            agent: dead_id,
                            good,
                            qty: u32::try_from(commons_qty)
                                .expect("ledgered commons placement fits a stock balance"),
                        },
                    );
                }
            }
            self.closure.inv.remove(&dead_id);
        }
        self.closure_resync_prev_stock();
    }

    // ---- Seam hooks (called from the corresponding mod.rs functions) ----

    /// Rule 4: an earned-provisioning gold transfer (producer → member) — bucket-preserving.
    pub(crate) fn closure_note_gold_transfer(&mut self, from: AgentId, to: AgentId, amount: Gold) {
        if !self.closure_active() {
            return;
        }
        self.closure.gold_transfer_preserving(from, to, amount);
    }

    /// A1 `transfer_birth_stock` / sufficiency control: a conserved staple move between EXISTING
    /// agents — a bucket-preserving `HouseholdTransfer`.
    pub(crate) fn closure_note_staple_transfer(
        &mut self,
        donor: AgentId,
        recipient: AgentId,
        good: GoodId,
        qty: u32,
    ) {
        if !self.closure_active() || qty == 0 {
            return;
        }
        let tick = self.econ_tick;
        self.closure.record(
            tick,
            ClosureEventKind::HouseholdTransfer {
                from: donor,
                to: recipient,
                good,
                qty,
            },
        );
        // Keep the diff baseline consistent for the donor/recipient across this out-of-phase move.
        self.closure_sync_agent_prev(donor);
        self.closure_sync_agent_prev(recipient);
    }

    /// A birth: register the child (household → class), transfer the staple endowment
    /// (bucket-preserving) and the gold gift (bucket-preserving), and extend the diff baseline to
    /// cover the new agent.
    pub(crate) fn closure_note_birth(
        &mut self,
        parent: AgentId,
        child: AgentId,
        household: usize,
        staple: GoodId,
        staple_qty: u32,
        gold: Gold,
    ) {
        if !self.closure_active() {
            return;
        }
        if let Some(&class) = self.closure.household_class.get(&household) {
            self.closure.registry.insert(child, class);
        }
        let tick = self.econ_tick;
        if staple_qty > 0 {
            self.closure.record(
                tick,
                ClosureEventKind::HouseholdTransfer {
                    from: parent,
                    to: child,
                    good: staple,
                    qty: staple_qty,
                },
            );
        }
        self.closure.gold_transfer_preserving(parent, child, gold);
        self.closure_sync_agent_prev(parent);
        self.closure_sync_agent_prev(child);
    }

    /// Refresh a single agent's entry in `prev_stock` to its current real stock (used after an
    /// out-of-phase transfer so the next phase diff does not re-observe it).
    fn closure_sync_agent_prev(&mut self, id: AgentId) {
        if let Some(agent) = self.society.agents.get(id) {
            let goods: BTreeMap<GoodId, u32> = agent
                .stock
                .positive_goods()
                .filter(|&g| g != GOLD)
                .map(|g| (g, agent.stock.get(g)))
                .collect();
            self.closure.prev_stock.insert(id, goods);
        } else {
            self.closure.prev_stock.remove(&id);
        }
    }

    /// Finalize the tick: record CC0 living membership + the CC3 boundary sinks, reconcile
    /// end-of-tick, and push the aggregate.
    pub(crate) fn closure_finalize_tick(&mut self) {
        let mut living = [false; 3];
        for &slot in &self.live_colonist_slots {
            let id = self.colonists[slot].id;
            match self.closure.class_of(id) {
                Some(class) => living[class.index()] = true,
                None => debug_assert!(false, "closed regime: every living agent maps to a class"),
            }
        }
        self.closure.cur.living = living;
        self.closure.cur.wage_escrow_gold = self.wage_escrow_gold.0;
        self.closure.cur.land_fee_pool_salt = self.land_fee_pool_salt.0;
        self.closure_reconcile("end-of-tick");
        let agg = std::mem::take(&mut self.closure.cur);
        self.closure.ticks.push(agg);
    }

    /// Debug-assert the ledger invariants against reality: `earned + endowed == agent.gold`, and
    /// `endowed + acquired + own_produced == agent.stock(good)` for every living agent and good.
    fn closure_reconcile(&self, at: &str) {
        if cfg!(debug_assertions) {
            for &slot in &self.live_colonist_slots {
                let id = self.colonists[slot].id;
                let Some(agent) = self.society.agents.get(id) else {
                    continue;
                };
                let gold = self.closure.gold.get(&id).copied().unwrap_or_default();
                debug_assert_eq!(
                    gold.total(),
                    agent.gold,
                    "closure gold invariant ({at}) broke for {id:?}"
                );
                let empty = BTreeMap::new();
                let buckets = self.closure.inv.get(&id).unwrap_or(&empty);
                let goods: BTreeSet<GoodId> = agent
                    .stock
                    .positive_goods()
                    .filter(|&g| g != GOLD)
                    .chain(buckets.keys().copied())
                    .collect();
                for good in goods {
                    let real = agent.stock.get(good);
                    let shadow = buckets.get(&good).copied().unwrap_or_default().total();
                    debug_assert_eq!(
                        shadow, real,
                        "closure physical invariant ({at}) broke for {id:?} good {good:?}"
                    );
                }
            }
        }
    }

    // ---- Accessors (the raw tape + registry for the reference reducer; per-tick aggregates) ----

    /// The RAW physical audit tape (for the seed-3 reference reducer).
    pub fn closure_event_tape(&self) -> &[ClosurePhysicalEvent] {
        &self.closure.tape
    }

    /// The actor → class registry (the only auxiliary replay input for the reference reducer).
    pub fn closure_registry(&self) -> &BTreeMap<AgentId, ClosureClass> {
        &self.closure.registry
    }

    /// The class of an agent under the §3.1 mapping, or `None` if unregistered.
    pub fn closure_class_of(&self, id: AgentId) -> Option<ClosureClass> {
        self.closure.registry.get(&id).copied()
    }

    /// The per-tick production-reducer aggregates (windowed into [`ClosureWindow`]s by the oracle).
    pub fn closure_tick_aggregates(&self) -> &[ClosureTickAgg] {
        &self.closure.ticks
    }

    /// The production physical reducer's boundary origin inventory: per agent per good with a
    /// positive holding, the `(endowed, acquired, own_produced)` split. Used by the seed-3 recount
    /// to byte-match the independent reference reducer.
    pub fn closure_inventory_snapshot(
        &self,
    ) -> BTreeMap<AgentId, BTreeMap<GoodId, (u32, u32, u32)>> {
        let mut out: BTreeMap<AgentId, BTreeMap<GoodId, (u32, u32, u32)>> = BTreeMap::new();
        for (&agent, goods) in &self.closure.inv {
            for (&good, b) in goods {
                if b.total() > 0 {
                    out.entry(agent)
                        .or_default()
                        .insert(good, (b.endowed, b.acquired, b.own_produced));
                }
            }
        }
        out
    }
}

/// Build the tape events for one agent's per-good gains/losses under a phase's semantics.
fn closure_phase_events(
    phase: ClosurePhase,
    agent: AgentId,
    gains: Vec<(GoodId, u32)>,
    losses: Vec<(GoodId, u32)>,
    out: &mut Vec<ClosureEventKind>,
) {
    match phase {
        ClosurePhase::Gather => {
            for (good, qty) in gains {
                out.push(ClosureEventKind::GatherDeposit { agent, good, qty });
            }
            for (good, qty) in losses {
                out.push(ClosureEventKind::Consumption { agent, good, qty });
            }
        }
        ClosurePhase::Support => {
            for (good, qty) in gains {
                out.push(ClosureEventKind::BSupportCredit { agent, good, qty });
            }
            for (good, qty) in losses {
                out.push(ClosureEventKind::Consumption { agent, good, qty });
            }
        }
        ClosurePhase::Spoilage => {
            for (good, qty) in losses {
                out.push(ClosureEventKind::Spoilage { agent, good, qty });
            }
            for (good, qty) in gains {
                out.push(ClosureEventKind::GatherDeposit { agent, good, qty });
            }
        }
        ClosurePhase::Capital | ClosurePhase::Production | ClosurePhase::OwnUse => {
            let mut gi = gains.into_iter();
            let mut li = losses.into_iter();
            let first_loss = li.next();
            let first_gain = gi.next();
            match (first_loss, first_gain) {
                (Some((ig, iq)), Some((og, oq))) => {
                    if phase == ClosurePhase::Capital {
                        out.push(ClosureEventKind::CapitalFormation {
                            agent,
                            input: ig,
                            input_qty: iq,
                            tool: og,
                            tool_qty: oq,
                        });
                    } else {
                        out.push(ClosureEventKind::RecipeProduction {
                            agent,
                            input: ig,
                            input_qty: iq,
                            output: og,
                            output_qty: oq,
                        });
                    }
                }
                (Some((ig, iq)), None) => {
                    if phase == ClosurePhase::Capital {
                        out.push(ClosureEventKind::CapitalFormation {
                            agent,
                            input: ig,
                            input_qty: iq,
                            tool: ig,
                            tool_qty: 0,
                        });
                    } else {
                        out.push(ClosureEventKind::RecipeProduction {
                            agent,
                            input: ig,
                            input_qty: iq,
                            output: ig,
                            output_qty: 0,
                        });
                    }
                }
                (None, Some((og, oq))) => {
                    if phase == ClosurePhase::Capital {
                        out.push(ClosureEventKind::CapitalFormation {
                            agent,
                            input: og,
                            input_qty: 0,
                            tool: og,
                            tool_qty: oq,
                        });
                    } else {
                        out.push(ClosureEventKind::GatherDeposit {
                            agent,
                            good: og,
                            qty: oq,
                        });
                    }
                }
                (None, None) => {}
            }
            for (good, qty) in li {
                out.push(ClosureEventKind::Consumption { agent, good, qty });
            }
            for (good, qty) in gi {
                out.push(ClosureEventKind::GatherDeposit { agent, good, qty });
            }
        }
    }
}

#[cfg(test)]
mod reducer_tests {
    use super::*;

    const G: ClosureClass = ClosureClass::Gatherer;
    const M: ClosureClass = ClosureClass::Miller;
    const GOOD: GoodId = GoodId(7);

    fn ledger(classes: &[(AgentId, ClosureClass)]) -> ClosureLedger {
        let mut l = ClosureLedger::default();
        for &(id, c) in classes {
            l.registry.insert(id, c);
        }
        l
    }

    #[test]
    fn split_capital_build_legs_stay_in_the_capital_event_family() {
        let agent = AgentId(1);
        let wood = GoodId(2);
        let tool = GoodId(3);

        let mut start = Vec::new();
        closure_phase_events(
            ClosurePhase::Capital,
            agent,
            Vec::new(),
            vec![(wood, 6)],
            &mut start,
        );
        assert_eq!(
            start,
            vec![ClosureEventKind::CapitalFormation {
                agent,
                input: wood,
                input_qty: 6,
                tool: wood,
                tool_qty: 0,
            }],
            "a multi-tick build's committed input must not be reported as recipe production"
        );
        let mut l = ledger(&[(agent, G)]);
        l.inv_credit(agent, wood, 6, Origin::Endowed);
        l.record(0, start[0]);
        assert_eq!(
            l.cur.endowed_physical_debits_by_family[G.index()]
                [ClosureDebitFamily::CapitalInput.index()],
            6
        );
        assert_eq!(
            l.cur.endowed_physical_debits_by_family[G.index()]
                [ClosureDebitFamily::RecipeInput.index()],
            0
        );

        let mut completion = Vec::new();
        closure_phase_events(
            ClosurePhase::Capital,
            agent,
            vec![(tool, 1)],
            Vec::new(),
            &mut completion,
        );
        assert_eq!(
            completion,
            vec![ClosureEventKind::CapitalFormation {
                agent,
                input: tool,
                input_qty: 0,
                tool,
                tool_qty: 1,
            }],
            "a later tool completion must not be reported as a gather deposit"
        );
    }

    // ---- R7-1: the gold reducer ----

    #[test]
    fn gold_purchase_debits_earned_first_below_at_above() {
        let a = AgentId(1);
        // strictly BELOW the earned balance: all earned, endowed portion 0.
        let mut l = ClosureLedger::default();
        l.gold_credit(a, Gold(10), true);
        l.gold_credit(a, Gold(5), false);
        assert_eq!(l.gold_purchase_debit(a, Gold(4)), Gold(0));
        assert_eq!(
            l.gold[&a],
            GoldBuckets {
                earned: Gold(6),
                endowed: Gold(5)
            }
        );
        // EXACTLY the earned balance: earned drained to 0, endowed portion still 0.
        let mut l = ClosureLedger::default();
        l.gold_credit(a, Gold(10), true);
        l.gold_credit(a, Gold(5), false);
        assert_eq!(l.gold_purchase_debit(a, Gold(10)), Gold(0));
        assert_eq!(
            l.gold[&a],
            GoldBuckets {
                earned: Gold(0),
                endowed: Gold(5)
            }
        );
        // strictly ABOVE the earned balance: the overflow debits endowed (feeds CC2).
        let mut l = ClosureLedger::default();
        l.gold_credit(a, Gold(10), true);
        l.gold_credit(a, Gold(5), false);
        assert_eq!(l.gold_purchase_debit(a, Gold(12)), Gold(2));
        assert_eq!(
            l.gold[&a],
            GoldBuckets {
                earned: Gold(0),
                endowed: Gold(3)
            }
        );
    }

    #[test]
    fn gold_sale_credit_is_origin_specific() {
        let s = AgentId(1);
        // Pure endowed-origin sale (liquidation): proceeds → ENDOWED gold, never earned.
        let mut l = ClosureLedger::default();
        l.gold_sale_credit(
            s,
            Gold(3),
            DebitSplit {
                endowed: 4,
                acquired: 0,
                own_produced: 0,
            },
        );
        assert_eq!(
            l.gold[&s],
            GoldBuckets {
                earned: Gold(0),
                endowed: Gold(12)
            }
        );
        // Own-produced + acquired sale: proceeds → EARNED gold.
        let mut l = ClosureLedger::default();
        l.gold_sale_credit(
            s,
            Gold(3),
            DebitSplit {
                endowed: 0,
                acquired: 1,
                own_produced: 2,
            },
        );
        assert_eq!(
            l.gold[&s],
            GoldBuckets {
                earned: Gold(9),
                endowed: Gold(0)
            }
        );
        // Mixed: the pro-rata endowed portion → endowed, the rest → earned.
        let mut l = ClosureLedger::default();
        l.gold_sale_credit(
            s,
            Gold(2),
            DebitSplit {
                endowed: 1,
                acquired: 1,
                own_produced: 1,
            },
        );
        assert_eq!(
            l.gold[&s],
            GoldBuckets {
                earned: Gold(4),
                endowed: Gold(2)
            }
        );
    }

    #[test]
    fn gold_transfer_is_bucket_preserving() {
        let (from, to) = (AgentId(1), AgentId(2));
        let mut l = ClosureLedger::default();
        l.gold_credit(from, Gold(7), true);
        l.gold_credit(from, Gold(3), false);
        // Transfer 9: earned-first drains all 7 earned + 2 endowed; recipient mirrors the split.
        l.gold_transfer_preserving(from, to, Gold(9));
        assert_eq!(
            l.gold[&from],
            GoldBuckets {
                earned: Gold(0),
                endowed: Gold(1)
            }
        );
        assert_eq!(
            l.gold[&to],
            GoldBuckets {
                earned: Gold(7),
                endowed: Gold(2)
            }
        );
    }

    // ---- R5-4: the physical shadow reducer sequence tests ----

    #[test]
    fn endowed_sale_then_production_then_own_sale_laundering() {
        let (seller, buyer) = (AgentId(1), AgentId(2));
        let mut l = ledger(&[(seller, M), (buyer, G)]);
        // Seed: the seller holds endowed stock only.
        l.inv_credit(seller, GOOD, 5, Origin::Endowed);

        // Sale 1 — of ENDOWED stock: increments endowed_physical_debits (Sale family), credits
        // ENDOWED gold (via the split), and contributes NOTHING to own-production consideration.
        let split1 = l
            .record(
                1,
                ClosureEventKind::SettledSpotTrade {
                    seller,
                    buyer,
                    good: GOOD,
                    qty: 3,
                    price: Gold(2),
                    trade_id: 0,
                },
            )
            .expect("trade returns the seller split");
        assert_eq!(
            split1,
            DebitSplit {
                endowed: 3,
                acquired: 0,
                own_produced: 0
            }
        );
        assert_eq!(
            l.cur.own_sale_consideration[M.index()],
            0,
            "endowed sale is not earning"
        );
        assert_eq!(l.cur.endowed_physical_debits[M.index()], 3);
        assert_eq!(
            l.cur.endowed_physical_debits_by_family[M.index()][ClosureDebitFamily::Sale.index()],
            3
        );

        // Later: the seller PRODUCES the same good (own_produced).
        l.record(
            1,
            ClosureEventKind::GatherDeposit {
                agent: seller,
                good: GOOD,
                qty: 4,
            },
        );

        // Sale 2 — of OWN-PRODUCED stock: contributes own-production consideration, adds no endowed
        // physical debit (the remaining endowed stock, 2, is not touched — own_produced sold first?
        // No: debit order is endowed→acquired→own_produced, so the 2 remaining endowed sell first).
        let split2 = l
            .record(
                1,
                ClosureEventKind::SettledSpotTrade {
                    seller,
                    buyer,
                    good: GOOD,
                    qty: 4,
                    price: Gold(2),
                    trade_id: 1,
                },
            )
            .expect("trade split");
        // The 2 leftover endowed sell first (order), then 2 own_produced.
        assert_eq!(
            split2,
            DebitSplit {
                endowed: 2,
                acquired: 0,
                own_produced: 2
            }
        );
        // own-production consideration = price × own_produced-qty = 2 × 2 = 4.
        assert_eq!(l.cur.own_sale_consideration[M.index()], 4);
        // endowed physical debits accrue the 2 leftover endowed (total 3 + 2 = 5).
        assert_eq!(l.cur.endowed_physical_debits[M.index()], 5);
    }

    #[test]
    fn consumption_of_endowed_before_a_later_own_produced_sale() {
        let (seller, buyer) = (AgentId(1), AgentId(2));
        let mut l = ledger(&[(seller, M), (buyer, G)]);
        l.inv_credit(seller, GOOD, 3, Origin::Endowed);
        // Consume endowed stock: posts an endowed physical debit under the Consumption family.
        l.record(
            1,
            ClosureEventKind::Consumption {
                agent: seller,
                good: GOOD,
                qty: 2,
            },
        );
        assert_eq!(l.cur.endowed_physical_debits[M.index()], 2);
        assert_eq!(
            l.cur.endowed_physical_debits_by_family[M.index()]
                [ClosureDebitFamily::Consumption.index()],
            2
        );
        // Produce, then sell own-produced.
        l.record(
            1,
            ClosureEventKind::GatherDeposit {
                agent: seller,
                good: GOOD,
                qty: 5,
            },
        );
        l.record(
            1,
            ClosureEventKind::SettledSpotTrade {
                seller,
                buyer,
                good: GOOD,
                qty: 5,
                price: Gold(1),
                trade_id: 0,
            },
        );
        // The 1 remaining endowed sells first, then 4 own_produced → own consideration = 4.
        assert_eq!(l.cur.own_sale_consideration[M.index()], 4);
    }

    #[test]
    fn household_transfer_is_bucket_preserving_and_not_a_debit() {
        let (from, to) = (AgentId(1), AgentId(2));
        let mut l = ledger(&[(from, M), (to, M)]);
        l.inv_credit(from, GOOD, 2, Origin::Endowed);
        l.inv_credit(from, GOOD, 3, Origin::OwnProduced);
        l.record(
            1,
            ClosureEventKind::HouseholdTransfer {
                from,
                to,
                good: GOOD,
                qty: 4,
            },
        );
        // 4 consumed endowed-first: 2 endowed + 2 own_produced, re-credited to the recipient in the
        // SAME buckets. No endowed_physical_debit (bucket-preserving, excluded from CC2).
        assert_eq!(
            l.inv[&from][&GOOD],
            OriginBuckets {
                endowed: 0,
                acquired: 0,
                own_produced: 1
            }
        );
        assert_eq!(
            l.inv[&to][&GOOD],
            OriginBuckets {
                endowed: 2,
                acquired: 0,
                own_produced: 2
            }
        );
        assert_eq!(l.cur.endowed_physical_debits[M.index()], 0);
    }

    #[test]
    fn estate_to_commons_records_goods_drain_not_a_debit() {
        let dead = AgentId(1);
        let mut l = ledger(&[(dead, M)]);
        l.inv_credit(dead, GOOD, 4, Origin::Endowed);
        l.record(
            1,
            ClosureEventKind::EstateToCommons {
                agent: dead,
                good: GOOD,
                qty: 4,
            },
        );
        assert_eq!(l.cur.commons_goods_drain, 4);
        assert_eq!(
            l.cur.endowed_physical_debits[M.index()],
            0,
            "a drain is CC3, not CC2"
        );
    }
}
