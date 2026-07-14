//! DH.b (impl-69) — the reproductive-burden robustness audit: the runtime succession/birth-funding
//! telemetry, the pure per-cell succession-survival classifier, and the pure cross-grid synthesis.
//!
//! Everything here is OBSERVATION: every runtime hook gates through
//! [`Settlement::closure_active`] — the same predicate the landed DH.a force-disable control flips
//! — so a marker-off (or force-disabled) run records nothing and stays byte-identical. Nothing in
//! this module is serialized into `canonical_bytes` (no digest tag — R1-11: identity is proven by
//! the §5.6 tests, not a new tag). See `docs/impl-reproductive-burden.md`.
//!
//! The pure half (`classify_burden_cell`, `synthesize_burden_grid`, `classify_birth_funding`,
//! `build_burden_windows`) is total over its inputs and carries the exhaustive per-rung payloads;
//! the LIVE integration suite (`sim/tests/reproductive_burden.rs`) treats instrumentation
//! corruption as a hard guard failure, while the pure classifier stays conservative (malformed
//! synthetic funding input → the `Unverifiable` bit, never a panic — R2-6).

use super::closure::ClosureClass;
use super::{FoodChannel, FoodLot, Settlement, Vocation};
use econ::agent::AgentId;
use econ::good::GoodId;
use econ::project::RecipeId;
use std::collections::{BTreeMap, BTreeSet};

// ===========================================================================================
// Pinned constants (§3 — none searched)
// ===========================================================================================

/// W — the scoring window width in ticks: the actual maximum producer lifespan on this base,
/// `(old_age_onset_years + lifespan_span_years) × ticks_per_year = (3+3)×6`.
pub const BURDEN_WINDOW_TICKS: u64 = 36;
/// M — consecutive windows per streak (180 scored ticks).
pub const BURDEN_STREAK_WINDOWS: usize = 5;
/// The landed oracle run length.
pub const BURDEN_RUN_TICKS: u64 = 1_600;
/// The landed seed set, outermost in the sweep order.
pub const BURDEN_SEEDS: [u64; 5] = [3, 7, 11, 19, 23];
/// The burden grid, ascending.
pub const BURDEN_QS: [u32; 6] = [0, 1, 2, 3, 4, 8];
/// The scored producer classes, in Miller-before-Baker payload order.
pub const BURDEN_PRODUCER_CLASSES: [ClosureClass; 2] = [ClosureClass::Miller, ClosureClass::Baker];

/// The per-seed scoring start: `36 × ceil((last_founder_death_tick + 1) / 36)` (§3, R1-4).
pub fn burden_start_tick(last_founder_death_tick: u64) -> u64 {
    BURDEN_WINDOW_TICKS * (last_founder_death_tick + 1).div_ceil(BURDEN_WINDOW_TICKS)
}

/// The two-field saving arm (§2, R1-10): Off = `(birth_stock_saving=false, mode=Off)`,
/// On = `(birth_stock_saving=true, mode=Motive)`. `SufficiencyControl` is unreachable in every
/// cell (asserted by the suite).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BurdenSavingArm {
    Off,
    On,
}

// ===========================================================================================
// Telemetry event vocabulary (pub — the integration suite reads these)
// ===========================================================================================

/// The acquisition channel a funding lot arrived through — the pub mirror of the landed
/// (crate-private) `FoodChannel`, so the pure funding classifier can be driven from the
/// integration suite. `Foraged`/`Commons` are UNREACHABLE on this exact base (hard guard in the
/// live suite; the pure classifier routes them to `Unverifiable`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BurdenChannel {
    Bought,
    SeededMinted,
    SelfProduced,
    Foraged,
    Commons,
}

/// One drawn birth-funding lot: the channel, the quantity, the purchase identity (`Some(trade_id)`
/// only for `Bought` lots, overwritten on resale), and the orthogonal
/// ultimate-construction-endowment taint (set at construction, never cleared — R2-7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BurdenLot {
    pub channel: BurdenChannel,
    pub qty: u64,
    pub identity: Option<u64>,
    pub taint: bool,
}

/// The inheritance-identity succession event, recorded at the real estate seam (a producer
/// subject's tool placed with a LIVING heir).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BurdenToolInherited {
    pub tick: u64,
    pub class: ClosureClass,
    pub deceased: AgentId,
    pub heir: AgentId,
    pub tool: GoodId,
}

/// The re-adoption succession event, recorded at the real role-choice seam (an heir with an
/// inheritance record for the adopted role's tool flips into a producer vocation).
/// `holds_tool` is the continued-possession observation at the adoption instant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BurdenRoleAdopted {
    pub tick: u64,
    pub class: ClosureClass,
    pub heir: AgentId,
    pub tool: GoodId,
    pub role: Vocation,
    pub holds_tool: bool,
}

/// One stage-recipe application (Mill/Bake only), recorded at the real production seam — the
/// successor-execution and staffed-flow evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BurdenStageExecution {
    pub tick: u64,
    pub agent: AgentId,
    pub recipe: RecipeId,
}

/// Stream (a) of the two independent per-birth streams (R2-5): emitted at successful newborn
/// insertion into a fixed Miller/Baker closure class. `child` is the qualifying birth's id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BurdenBirthOccurred {
    pub tick: u64,
    pub class: ClosureClass,
    pub parent: AgentId,
    pub child: AgentId,
}

/// Stream (b): the funding record, emitted after the exact conserved lot transfer at the birth
/// seam — the drawn lots carry their identities and taints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BurdenBirthFunding {
    pub tick: u64,
    pub class: ClosureClass,
    pub parent: AgentId,
    pub child: AgentId,
    pub q: u32,
    pub lots: Vec<BurdenLot>,
}

/// One settled spot trade's record, taken from the DH.a gold split AT EVENT TIME (R4-1):
/// `earned_paid`/`endowed_paid` are the buyer's actual earned-first debit split;
/// `positive_consideration` is `paid > 0`. Downstream funding joins are by `trade_id` ONLY.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BurdenTradeRecord {
    pub trade_id: u64,
    pub buyer: AgentId,
    pub good: GoodId,
    pub quantity: u32,
    pub earned_paid: u64,
    pub endowed_paid: u64,
    pub positive_consideration: bool,
}

/// A purchase-credit-seam fact (R4-1): the fresh `Bought` fragments a settled bread trade
/// credited, captured at the seam and validated against the trade record the same tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PendingPurchaseCredit {
    pub trade_id: u64,
    pub buyer: AgentId,
    pub good: GoodId,
    pub credited: u64,
}

pub(super) fn burden_channel_of(channel: FoodChannel) -> BurdenChannel {
    match channel {
        FoodChannel::Bought => BurdenChannel::Bought,
        FoodChannel::SeededMinted => BurdenChannel::SeededMinted,
        FoodChannel::SelfProduced => BurdenChannel::SelfProduced,
        FoodChannel::Foraged => BurdenChannel::Foraged,
        FoodChannel::Commons => BurdenChannel::Commons,
    }
}

pub(super) fn burden_lot_of(lot: &FoodLot) -> BurdenLot {
    BurdenLot {
        channel: burden_channel_of(lot.channel),
        qty: lot.qty,
        identity: lot.identity,
        taint: lot.taint,
    }
}

// ===========================================================================================
// The runtime telemetry holder (a Settlement field; runtime-only, never digested)
// ===========================================================================================

/// The DH.b runtime telemetry. Maintained only while [`Settlement::closure_active`] holds;
/// the empty default otherwise. Pure observation — feeds no decision path.
#[derive(Clone, Debug, Default)]
pub(crate) struct BurdenTelemetry {
    pub inheritances: Vec<BurdenToolInherited>,
    pub adoptions: Vec<BurdenRoleAdopted>,
    pub executions: Vec<BurdenStageExecution>,
    pub births: Vec<BurdenBirthOccurred>,
    pub funding: Vec<BurdenBirthFunding>,
    pub trades: Vec<BurdenTradeRecord>,
    /// Purchase-credit facts awaiting the same-tick join against the trade records.
    pub pending_purchase_credits: Vec<PendingPurchaseCredit>,
    /// Live instrumentation-corruption findings (purchase-credit-seam mismatches). The suite
    /// hard-fails on any entry (R2-6/R3-4); the runtime only records, never panics.
    pub seam_violations: Vec<String>,
}

impl Settlement {
    /// The recorded inheritance-identity succession events.
    pub fn burden_tool_inheritances(&self) -> &[BurdenToolInherited] {
        &self.burden.inheritances
    }

    /// The recorded re-adoption succession events.
    pub fn burden_role_adoptions(&self) -> &[BurdenRoleAdopted] {
        &self.burden.adoptions
    }

    /// The recorded Mill/Bake stage executions.
    pub fn burden_stage_executions(&self) -> &[BurdenStageExecution] {
        &self.burden.executions
    }

    /// Stream (a): the `BirthOccurred` events.
    pub fn burden_birth_events(&self) -> &[BurdenBirthOccurred] {
        &self.burden.births
    }

    /// Stream (b): the per-birth funding records.
    pub fn burden_birth_funding_records(&self) -> &[BurdenBirthFunding] {
        &self.burden.funding
    }

    /// The settled-trade records (the DH.a gold split at event time).
    pub fn burden_trade_records(&self) -> &[BurdenTradeRecord] {
        &self.burden.trades
    }

    /// Live instrumentation-corruption findings: the purchase-credit-seam mismatches plus the
    /// acquisition-ledger channel violations. The suite hard-fails on any entry.
    pub fn burden_seam_violations(&self) -> Vec<String> {
        let mut out = self.burden.seam_violations.clone();
        out.extend(self.acquisition.burden_violations.iter().cloned());
        out
    }

    /// A per-tick living snapshot for the suite's guards and continuity sampling:
    /// `(id, closure class, has_lifespan)` for every living colonist.
    pub fn burden_living_snapshot(&self) -> Vec<(AgentId, Option<ClosureClass>, bool)> {
        self.live_colonist_slots
            .iter()
            .map(|&slot| {
                let c = &self.colonists[slot];
                (c.id, self.closure_class_of(c.id), c.lifespan.is_some())
            })
            .collect()
    }

    /// The live lot audit (R2-6): scan every held lot for instrumentation corruption — a
    /// `Bought` lot without a purchase identity, an identity that does not join EXACTLY one
    /// trade record, an untainted `SeededMinted` lot (taint is construction-set), a purchase
    /// identity on a non-`Bought` lot, a tainted `SelfProduced` lot (only construction sets
    /// taint — the pinned lifecycle, R3-2), or any `Foraged`/`Commons` lot (unreachable on
    /// this base). Also rejects duplicate trade ids. The suite calls this per tick and
    /// hard-fails on any entry.
    pub fn burden_lot_audit(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.closure_active() {
            return out;
        }
        // Trade ids are the global spot-trade indexes, recorded append-only, so the record
        // stream is strictly increasing unless corrupted — one comparison pass proves BOTH
        // orderedness and uniqueness (the audit runs per tick in the suite, so it stays
        // allocation-free), and lot joins then binary-search the sorted stream.
        let sorted_unique = self
            .burden
            .trades
            .windows(2)
            .all(|w| w[0].trade_id < w[1].trade_id);
        if !sorted_unique {
            out.push(
                "trade record stream is not strictly increasing (duplicate trade id)".to_string(),
            );
        }
        for (agent, lots) in &self.acquisition.lots {
            for lot in lots {
                match lot.channel {
                    FoodChannel::Bought => match lot.identity {
                        None => out.push(format!(
                            "Bought lot without purchase identity held by {agent:?}"
                        )),
                        Some(id) => {
                            let joins = if sorted_unique {
                                usize::from(
                                    self.burden
                                        .trades
                                        .binary_search_by_key(&id, |t| t.trade_id)
                                        .is_ok(),
                                )
                            } else {
                                self.burden
                                    .trades
                                    .iter()
                                    .filter(|t| t.trade_id == id)
                                    .count()
                            };
                            if joins != 1 {
                                out.push(format!(
                                    "purchase identity {id} joins {joins} trade records (want 1)"
                                ));
                            }
                        }
                    },
                    FoodChannel::SeededMinted => {
                        if lot.identity.is_some() {
                            out.push(format!(
                                "SeededMinted lot with purchase identity held by {agent:?}"
                            ));
                        }
                        if !lot.taint {
                            out.push(format!(
                                "SeededMinted lot without construction taint held by {agent:?}"
                            ));
                        }
                    }
                    FoodChannel::SelfProduced => {
                        if lot.identity.is_some() {
                            out.push(format!(
                                "SelfProduced lot with purchase identity held by {agent:?}"
                            ));
                        }
                        if lot.taint {
                            out.push(format!(
                                "SelfProduced lot with construction taint held by {agent:?}"
                            ));
                        }
                    }
                    FoodChannel::Foraged | FoodChannel::Commons => out.push(format!(
                        "unreachable channel {channel:?} held by {agent:?}",
                        channel = lot.channel
                    )),
                }
            }
        }
        out
    }

    /// Validate this tick's purchase-credit-seam facts against the settled-trade records
    /// (R4-1): the fresh `Bought` fragments of a trade must be credited to ITS buyer, carry ITS
    /// good, and aggregate exactly to ITS quantity. A mismatch is recorded as a seam violation
    /// (the live suite hard-fails on it). Downstream records join by identity only.
    pub(crate) fn burden_validate_purchase_credits(&mut self) {
        let pending = std::mem::take(&mut self.burden.pending_purchase_credits);
        for p in pending {
            let joined: Vec<&BurdenTradeRecord> = self
                .burden
                .trades
                .iter()
                .filter(|t| t.trade_id == p.trade_id)
                .collect();
            let violation = match joined.as_slice() {
                [t] => {
                    if t.buyer != p.buyer {
                        Some(format!(
                            "purchase-credit seam: trade {id} credited {got:?}, record buyer {want:?}",
                            id = p.trade_id,
                            got = p.buyer,
                            want = t.buyer
                        ))
                    } else if t.good != p.good {
                        Some(format!(
                            "purchase-credit seam: trade {id} credited good {got:?}, record good {want:?}",
                            id = p.trade_id,
                            got = p.good,
                            want = t.good
                        ))
                    } else if u64::from(t.quantity) != p.credited {
                        Some(format!(
                            "purchase-credit seam: trade {id} credited {got} fresh Bought units, record quantity {want}",
                            id = p.trade_id,
                            got = p.credited,
                            want = t.quantity
                        ))
                    } else {
                        None
                    }
                }
                [] => Some(format!(
                    "purchase-credit seam: trade {id} has no settled-trade record",
                    id = p.trade_id
                )),
                _ => Some(format!(
                    "purchase-credit seam: trade {id} joins multiple settled-trade records",
                    id = p.trade_id
                )),
            };
            if let Some(v) = violation {
                self.burden.seam_violations.push(v);
            }
        }
    }
}

// ===========================================================================================
// The pure per-cell classifier (§3 — total, precedence-ordered, exhaustive payloads)
// ===========================================================================================

/// The six-bit causal-succession diagnostic (R2-8, R3-6). A SET bit names the link that FAILED.
/// The two event-existence bits are independent; the chain bits (`tuple_join` →
/// `strict_ordering` → `possession_at_adoption` → `successor_execution`) are cumulative: a chain
/// bit is set only when its predecessor stage still had surviving candidates — the earliest
/// missing link tells the story without noise from vacuous downstream stages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SuccessionBits {
    /// No class-correct inheritance event (class, class-correct tool, born-in-simulation heir).
    pub inheritance_event: bool,
    /// No class-correct adoption event (requires the class-correct `Vocation` — R3-6).
    pub adoption_event: bool,
    /// Events exist on both sides but no pair shares the SAME `(heir, tool)`.
    pub tuple_join: bool,
    /// Joined pairs exist but none with inheritance strictly before adoption.
    pub strict_ordering: bool,
    /// Ordered pairs exist but none with continued possession at adoption.
    pub possession_at_adoption: bool,
    /// Possessed chains exist but none with a class-correct-recipe execution by that successor
    /// within the streak, strictly after the adoption.
    pub successor_execution: bool,
}

impl SuccessionBits {
    pub fn any(&self) -> bool {
        self.inheritance_event
            || self.adoption_event
            || self.tuple_join
            || self.strict_ordering
            || self.possession_at_adoption
            || self.successor_execution
    }
}

/// The rung-6 funding-failure reason bitset (R2-2), aggregated over the proof streak's
/// qualifying births. Renamed from "EndowmentDependent": `Unverifiable` distinguishes a failed
/// join from proven endowment dependence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FundingBits {
    /// A drawn lot carries the ultimate-construction-endowment taint.
    pub physical_endowment_taint: bool,
    /// A joined trade paid with endowed gold (`endowed_paid > 0`).
    pub endowed_payment: bool,
    /// Both payment buckets positive on a joined trade (additionally to `endowed_payment`).
    pub mixed_payment: bool,
    /// A joined trade settled with zero consideration.
    pub zero_consideration: bool,
    /// A malformed join: a `Bought` lot without identity, an identity joining ≠ 1 trade
    /// records, forbidden identity/taint metadata on a non-`Bought` lot, a trade whose
    /// `positive_consideration` flag contradicts its payment buckets, or an unreachable channel.
    /// Conservative — pure-classifier only (the live suite hard-fails the equivalent conditions
    /// instead — R2-6/R3-4).
    pub unverifiable: bool,
}

impl FundingBits {
    pub fn is_empty(&self) -> bool {
        !(self.physical_endowment_taint
            || self.endowed_payment
            || self.mixed_payment
            || self.zero_consideration
            || self.unverifiable)
    }

    pub fn union(&mut self, other: FundingBits) {
        self.physical_endowment_taint |= other.physical_endowment_taint;
        self.endowed_payment |= other.endowed_payment;
        self.mixed_payment |= other.mixed_payment;
        self.zero_consideration |= other.zero_consideration;
        self.unverifiable |= other.unverifiable;
    }
}

/// Classify one qualifying birth's funding (§3.4). EconomicallyFunded ⟺ the returned bitset is
/// empty: zero ultimate-endowed physical quantity AND every `Bought` unit joins (by identity
/// ONLY) exactly one trade record with `endowed_paid == 0` and positive consideration;
/// `SelfProduced` units qualify. Total: malformed input sets `Unverifiable`, never panics.
pub fn classify_birth_funding(lots: &[BurdenLot], trades: &[BurdenTradeRecord]) -> FundingBits {
    let mut bits = FundingBits::default();
    for lot in lots {
        if lot.qty == 0 {
            continue;
        }
        if lot.taint {
            bits.physical_endowment_taint = true;
        }
        match lot.channel {
            BurdenChannel::Bought => match lot.identity {
                None => bits.unverifiable = true,
                Some(id) => {
                    let joined: Vec<&BurdenTradeRecord> =
                        trades.iter().filter(|t| t.trade_id == id).collect();
                    match joined.as_slice() {
                        [t] => {
                            // A live record satisfies `positive_consideration ⟺
                            // earned_paid + endowed_paid > 0` (the flag is `paid > 0` and
                            // the buckets are the actual debit split of `paid` —
                            // closure.rs). A record contradicting that is malformed
                            // synthetic input: its buckets prove nothing, so it is
                            // Unverifiable, never a payment finding (R2-6).
                            let paid_recorded = t.earned_paid > 0 || t.endowed_paid > 0;
                            if t.positive_consideration != paid_recorded {
                                bits.unverifiable = true;
                            } else if !paid_recorded {
                                bits.zero_consideration = true;
                            } else if t.endowed_paid > 0 {
                                bits.endowed_payment = true;
                                if t.earned_paid > 0 {
                                    bits.mixed_payment = true;
                                }
                            }
                        }
                        _ => bits.unverifiable = true,
                    }
                }
            },
            BurdenChannel::SelfProduced => {
                if lot.identity.is_some() || lot.taint {
                    bits.unverifiable = true;
                }
            }
            // SeededMinted qualifies only through its construction taint (set above); an
            // identity or missing taint is malformed telemetry — conservative.
            BurdenChannel::SeededMinted => {
                if lot.identity.is_some() || !lot.taint {
                    bits.unverifiable = true;
                }
            }
            // Unreachable on this base — conservative in the pure classifier.
            BurdenChannel::Foraged | BurdenChannel::Commons => bits.unverifiable = true,
        }
    }
    bits
}

/// One scored window's per-class observations, `[Miller, Baker]`-indexed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BurdenWindowObs {
    pub start: u64,
    /// Criterion 1: ≥1 living NONFOUNDER class member at every sample in the window.
    pub continuity: [bool; 2],
    /// Criterion 3: the class's stage ran (recipe executions > 0) in the window.
    pub flow: [bool; 2],
}

/// One qualifying birth, with its pre-computed funding classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BurdenBirthObs {
    pub tick: u64,
    pub child: AgentId,
    pub funding: FundingBits,
}

/// The pure classifier's whole-cell input, assembled by the suite from the telemetry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BurdenCellInput {
    pub q: u32,
    /// The class-correct stage tools: Miller ↔ `mill_good`, Baker ↔ `oven_good` (R4-4).
    pub mill_good: GoodId,
    pub oven_good: GoodId,
    /// The complete scored windows, in ascending start order (built by
    /// [`build_burden_windows`]).
    pub windows: Vec<BurdenWindowObs>,
    pub inheritances: Vec<BurdenToolInherited>,
    pub adoptions: Vec<BurdenRoleAdopted>,
    pub executions: Vec<BurdenStageExecution>,
    /// EVERY qualifying birth of the run (not just in-window ones) — rung 2 is "no qualifying
    /// birth EVER".
    pub births: Vec<BurdenBirthObs>,
}

/// The rung-3 payload detail (R3-1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BurdenLineageExtinctDetail {
    /// ≥1 class lacks even a PRIVATE M-window continuity streak (Miller-before-Baker order,
    /// nonempty).
    MissingPrivateStreaks { classes: Vec<ClosureClass> },
    /// Both classes have private streaks but never a common one.
    DisjointPrivateStreaks,
}

/// The per-cell verdict ladder (§3, R1-2) — pure, total, precedence-ordered. Never carried as a
/// display string; always the computed enum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BurdenCellVerdict {
    /// Rung 1 — constructed by the SUITE when a hard guard fails (also fails the suite).
    PreconditionInvalid { guard: String },
    /// Rung 2 — no qualifying birth ever (total for all q, including q=0).
    NoBirth { q: u32 },
    /// Rung 3 — births occur; the common-continuity set C is empty.
    BirthsButLineageExtinct { detail: BurdenLineageExtinctDetail },
    /// Rung 4 — C≠∅, S=∅; per-class bits aggregated on the earliest C-streak.
    LineagePersistsSuccessionAbsent {
        failures: BTreeMap<ClosureClass, SuccessionBits>,
    },
    /// Rung 5 — S≠∅, F=∅; every class whose flow failed on the earliest S-streak.
    FunctionalSuccessionFlowAbsent { classes: Vec<ClosureClass> },
    /// Rung 6 — F≠∅, E=∅; reasons aggregated over the earliest F-streak's qualifying births.
    FunctionalSuccessionEconomicFundingUnproven { reasons: FundingBits },
    /// Rung 7 — E≠∅.
    FunctionalSuccessionEconomicallyFunded,
    /// The q=0 cap (R1-2): a q=0 cell that would reach rung 6/7. No economic rank; excluded
    /// from the economic and motive syntheses.
    CostlessReplacement,
}

impl BurdenCellVerdict {
    /// The ladder rung. `CostlessReplacement` carries NO economic rank (0 — it never passes and
    /// never enters the economic or motive syntheses).
    pub fn rung(&self) -> u8 {
        match self {
            BurdenCellVerdict::PreconditionInvalid { .. } => 1,
            BurdenCellVerdict::NoBirth { .. } => 2,
            BurdenCellVerdict::BirthsButLineageExtinct { .. } => 3,
            BurdenCellVerdict::LineagePersistsSuccessionAbsent { .. } => 4,
            BurdenCellVerdict::FunctionalSuccessionFlowAbsent { .. } => 5,
            BurdenCellVerdict::FunctionalSuccessionEconomicFundingUnproven { .. } => 6,
            BurdenCellVerdict::FunctionalSuccessionEconomicallyFunded => 7,
            BurdenCellVerdict::CostlessReplacement => 0,
        }
    }

    /// Rung 6+/7 — the DH.c-gate passing bar. `CostlessReplacement` never passes (q=0 cap).
    pub fn passes(&self) -> bool {
        matches!(self.rung(), 6 | 7)
    }
}

/// Build the scored windows from per-tick class-continuity samples and the execution stream
/// (pure; shared by the suite and its boundary tests). `continuity_by_tick[t]` is the
/// `[Miller, Baker]` nonfounder-living sample at tick `t`; only COMPLETE windows strictly before
/// `horizon` are enumerated (R1-4).
pub fn build_burden_windows(
    start: u64,
    horizon: u64,
    continuity_by_tick: &[[bool; 2]],
    executions: &[BurdenStageExecution],
) -> Vec<BurdenWindowObs> {
    let mut windows = Vec::new();
    let mut w_start = start;
    while w_start + BURDEN_WINDOW_TICKS <= horizon {
        let mut continuity = [true; 2];
        for t in w_start..w_start + BURDEN_WINDOW_TICKS {
            match continuity_by_tick.get(t as usize) {
                Some(sample) => {
                    for (slot, &alive) in continuity.iter_mut().zip(sample) {
                        *slot &= alive;
                    }
                }
                None => continuity = [false; 2],
            }
        }
        let mut flow = [false; 2];
        for e in executions {
            if e.tick >= w_start && e.tick < w_start + BURDEN_WINDOW_TICKS {
                match e.recipe {
                    RecipeId::Mill => flow[0] = true,
                    RecipeId::Bake => flow[1] = true,
                    _ => {}
                }
            }
        }
        windows.push(BurdenWindowObs {
            start: w_start,
            continuity,
            flow,
        });
        w_start += BURDEN_WINDOW_TICKS;
    }
    windows
}

fn class_tool(input: &BurdenCellInput, class: ClosureClass) -> GoodId {
    match class {
        ClosureClass::Miller => input.mill_good,
        _ => input.oven_good,
    }
}

fn class_vocation(class: ClosureClass) -> Vocation {
    match class {
        ClosureClass::Miller => Vocation::Miller,
        _ => Vocation::Baker,
    }
}

fn class_recipe(class: ClosureClass) -> RecipeId {
    match class {
        ClosureClass::Miller => RecipeId::Mill,
        _ => RecipeId::Bake,
    }
}

/// Evaluate criterion 2 for one class over one streak span `[span_start, span_end)` (R4-3:
/// once per streak; the successor execution must fall WITHIN the streak; the inheritance and
/// adoption events may predate it but must strictly precede that execution).
///
/// Ordering is on the `(tick, seam-phase)` pair: the tick stamp is phase-coarse, and within one
/// econ tick the REAL seams run in the fixed order estate (inheritance) → role-choice
/// (adoption) → production (execution) — so an equal-tick pair is strictly causally ordered by
/// construction, and the tick comparisons below are `<=`/`>=` with the seam phase breaking the
/// tie. A synthetic adoption on a strictly EARLIER tick than its inheritance (or an execution
/// strictly earlier than its adoption) still fails.
fn succession_for_class(
    input: &BurdenCellInput,
    class: ClosureClass,
    span: (u64, u64),
    born: &BTreeSet<AgentId>,
) -> Result<(), SuccessionBits> {
    let tool = class_tool(input, class);
    let vocation = class_vocation(class);
    let recipe = class_recipe(class);
    let inh: Vec<&BurdenToolInherited> = input
        .inheritances
        .iter()
        .filter(|e| e.tick < span.1 && e.class == class && e.tool == tool && born.contains(&e.heir))
        .collect();
    let adopt: Vec<&BurdenRoleAdopted> = input
        .adoptions
        .iter()
        .filter(|e| e.tick < span.1 && e.class == class && e.role == vocation)
        .collect();
    let pairs: Vec<(&BurdenToolInherited, &BurdenRoleAdopted)> = inh
        .iter()
        .flat_map(|i| {
            adopt
                .iter()
                .filter(move |a| a.heir == i.heir && a.tool == i.tool)
                .map(move |a| (*i, *a))
        })
        .collect();
    let ordered: Vec<&(&BurdenToolInherited, &BurdenRoleAdopted)> =
        pairs.iter().filter(|(i, a)| i.tick <= a.tick).collect();
    let possessed: Vec<&&(&BurdenToolInherited, &BurdenRoleAdopted)> =
        ordered.iter().filter(|(_, a)| a.holds_tool).collect();
    let executed = possessed.iter().any(|(_, a)| {
        input.executions.iter().any(|e| {
            e.agent == a.heir
                && e.recipe == recipe
                && e.tick >= span.0
                && e.tick < span.1
                && e.tick >= a.tick
        })
    });
    if executed {
        return Ok(());
    }
    let bits = SuccessionBits {
        inheritance_event: inh.is_empty(),
        adoption_event: adopt.is_empty(),
        tuple_join: !inh.is_empty() && !adopt.is_empty() && pairs.is_empty(),
        strict_ordering: !pairs.is_empty() && ordered.is_empty(),
        possession_at_adoption: !ordered.is_empty() && possessed.is_empty(),
        successor_execution: !possessed.is_empty(),
    };
    debug_assert!(
        bits.any(),
        "an unsatisfied succession chain names a failed link"
    );
    Err(bits)
}

/// The private M-streak starts for one class (continuity only) — the rung-3 detail input.
fn private_streak_starts(windows: &[BurdenWindowObs], class_index: usize) -> Vec<usize> {
    streak_starts(windows, |w| w.continuity[class_index])
}

/// All M-consecutive-window streak start INDEXES where `pred` holds for every window.
fn streak_starts(
    windows: &[BurdenWindowObs],
    pred: impl Fn(&BurdenWindowObs) -> bool,
) -> Vec<usize> {
    if windows.len() < BURDEN_STREAK_WINDOWS {
        return Vec::new();
    }
    (0..=windows.len() - BURDEN_STREAK_WINDOWS)
        .filter(|&i| windows[i..i + BURDEN_STREAK_WINDOWS].iter().all(&pred))
        .collect()
}

/// The tick span `[start, end)` of the streak beginning at window index `i`.
fn streak_span(windows: &[BurdenWindowObs], i: usize) -> (u64, u64) {
    let start = windows[i].start;
    (
        start,
        start + BURDEN_WINDOW_TICKS * BURDEN_STREAK_WINDOWS as u64,
    )
}

/// The pure per-cell classifier (§3): nested common-streak sets C ⊇ S ⊇ F ⊇ E, classified
/// highest-to-lowest, the proof streak = the EARLIEST start within the highest achieved set
/// (R2-1). Total over every input; the q=0 cap replaces rungs 6/7 with `CostlessReplacement`
/// and is asserted (R1-2).
pub fn classify_burden_cell(input: &BurdenCellInput) -> BurdenCellVerdict {
    let verdict = classify_burden_cell_inner(input);
    debug_assert!(
        input.q != 0 || !verdict.passes(),
        "the q=0 cap: a q=0 cell never emits rung 6/7"
    );
    verdict
}

fn classify_burden_cell_inner(input: &BurdenCellInput) -> BurdenCellVerdict {
    if input.births.is_empty() {
        return BurdenCellVerdict::NoBirth { q: input.q };
    }
    let born: BTreeSet<AgentId> = input.births.iter().map(|b| b.child).collect();
    let windows = &input.windows;

    // C — common nonfounder continuity for BOTH classes across the same streak.
    let c: Vec<usize> = streak_starts(windows, |w| w.continuity[0] && w.continuity[1]);
    if c.is_empty() {
        let missing: Vec<ClosureClass> = BURDEN_PRODUCER_CLASSES
            .into_iter()
            .enumerate()
            .filter(|&(i, _)| private_streak_starts(windows, i).is_empty())
            .map(|(_, class)| class)
            .collect();
        let detail = if missing.is_empty() {
            BurdenLineageExtinctDetail::DisjointPrivateStreaks
        } else {
            BurdenLineageExtinctDetail::MissingPrivateStreaks { classes: missing }
        };
        return BurdenCellVerdict::BirthsButLineageExtinct { detail };
    }

    // S ⊆ C — both classes' causal-succession criterion holds over the streak (R4-3).
    let s: Vec<usize> = c
        .iter()
        .copied()
        .filter(|&i| {
            let span = streak_span(windows, i);
            BURDEN_PRODUCER_CLASSES
                .into_iter()
                .all(|class| succession_for_class(input, class, span, &born).is_ok())
        })
        .collect();
    if s.is_empty() {
        let earliest = c[0];
        let span = streak_span(windows, earliest);
        let mut failures = BTreeMap::new();
        for class in BURDEN_PRODUCER_CLASSES {
            if let Err(bits) = succession_for_class(input, class, span, &born) {
                failures.insert(class, bits);
            }
        }
        debug_assert!(
            !failures.is_empty(),
            "S=∅ ⇒ the earliest C-streak names a failure"
        );
        return BurdenCellVerdict::LineagePersistsSuccessionAbsent { failures };
    }

    // F ⊆ S — staffed flow for both classes in every window of the streak.
    let f: Vec<usize> = s
        .iter()
        .copied()
        .filter(|&i| {
            windows[i..i + BURDEN_STREAK_WINDOWS]
                .iter()
                .all(|w| w.flow[0] && w.flow[1])
        })
        .collect();
    if f.is_empty() {
        let earliest = s[0];
        let classes: Vec<ClosureClass> = BURDEN_PRODUCER_CLASSES
            .into_iter()
            .enumerate()
            .filter(|&(ci, _)| {
                windows[earliest..earliest + BURDEN_STREAK_WINDOWS]
                    .iter()
                    .any(|w| !w.flow[ci])
            })
            .map(|(_, class)| class)
            .collect();
        debug_assert!(
            !classes.is_empty(),
            "F=∅ ⇒ the earliest S-streak names a class"
        );
        return BurdenCellVerdict::FunctionalSuccessionFlowAbsent { classes };
    }

    // E ⊆ F — every qualifying birth in the streak is EconomicallyFunded.
    let births_in = |span: (u64, u64)| {
        input
            .births
            .iter()
            .filter(move |b| b.tick >= span.0 && b.tick < span.1)
    };
    let e: Vec<usize> = f
        .iter()
        .copied()
        .filter(|&i| births_in(streak_span(windows, i)).all(|b| b.funding.is_empty()))
        .collect();
    if e.is_empty() {
        if input.q == 0 {
            return BurdenCellVerdict::CostlessReplacement;
        }
        let earliest = f[0];
        let mut reasons = FundingBits::default();
        for b in births_in(streak_span(windows, earliest)) {
            reasons.union(b.funding);
        }
        debug_assert!(
            !reasons.is_empty(),
            "E=∅ ⇒ the earliest F-streak names a reason"
        );
        return BurdenCellVerdict::FunctionalSuccessionEconomicFundingUnproven { reasons };
    }
    if input.q == 0 {
        return BurdenCellVerdict::CostlessReplacement;
    }
    BurdenCellVerdict::FunctionalSuccessionEconomicallyFunded
}

// ===========================================================================================
// The pure cross-grid synthesis (§3 — precedence-ordered, exhaustive; R1-3)
// ===========================================================================================

/// One classified cell of the 60-cell grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BurdenCellResult {
    pub q: u32,
    pub arm: BurdenSavingArm,
    pub seed: u64,
    pub verdict: BurdenCellVerdict,
}

/// The precedence-ordered exhaustive synthesis verdict (§3, R1-3). Evaluated top-down; the
/// first matching row is the verdict. Rows 6/7/8 are disjoint by construction (R2-3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BurdenSynthesis {
    /// Row 1 — any cell `PreconditionInvalid` (the suite has already failed).
    InvalidGrid,
    /// Row 2 — a q=4 arm at rung 6+/7 on 5/5 seeds (authorizes a FUTURE DH.c grand-oracle
    /// rerun; never executed in DH.b).
    CanonicalBurdenSurvives { arms: Vec<BurdenSavingArm> },
    /// Row 3 — some 0<q<4 arm passes 5/5; q=4 does not.
    SubcanonicalSurvives {
        highest_q: u32,
        arms: Vec<(u32, BurdenSavingArm)>,
    },
    /// Row 4 — ONLY q=8 arms pass 5/5 (R2-4).
    StressOnly { arms: Vec<BurdenSavingArm> },
    /// Row 5 — some q>0 cell reaches rung 6/7 but no arm passes 5/5. The table lists every
    /// such cell `(q, arm, seed, rung)`.
    SeedHeterogeneousSurvival {
        table: Vec<(u32, BurdenSavingArm, u64, u8)>,
    },
    /// Row 6 — ≥1 q>0 cell at rung 4 or 5 and NO q>0 cell above rung 5 (R2-3).
    ContinuityWithoutEconomicSuccession {
        best_rung_by_arm: Vec<(u32, BurdenSavingArm, u8)>,
    },
    /// Row 7 — only q=0 reaches `CostlessReplacement`; every q>0 cell at rungs 2–3.
    CostlessOnlyReplacement,
    /// Row 8 — every valid q>0 cell at rungs 2–3 AND q=0 did not reach `CostlessReplacement`.
    RobustExtinction,
}

/// Does the `(q, arm)` arm pass — rung 6+/7 on ALL of its seeds in `cells`?
fn arm_passes(cells: &[BurdenCellResult], q: u32, arm: BurdenSavingArm) -> bool {
    let arm_cells: Vec<&BurdenCellResult> =
        cells.iter().filter(|c| c.q == q && c.arm == arm).collect();
    arm_cells.len() == BURDEN_SEEDS.len()
        && arm_cells.iter().map(|c| c.seed).collect::<BTreeSet<_>>() == BTreeSet::from(BURDEN_SEEDS)
        && arm_cells.iter().all(|c| c.verdict.passes())
}

/// Every passing arm at burden `q`.
fn passing_arms_at(cells: &[BurdenCellResult], q: u32) -> Vec<BurdenSavingArm> {
    [BurdenSavingArm::Off, BurdenSavingArm::On]
        .into_iter()
        .filter(|&arm| arm_passes(cells, q, arm))
        .collect()
}

/// The pure synthesis over the classified grid (§3). Exhaustive: exactly one row matches.
pub fn synthesize_burden_grid(cells: &[BurdenCellResult]) -> BurdenSynthesis {
    debug_assert!(
        cells
            .iter()
            .all(|c| c.q > 0 || c.verdict.rung() != 6 && c.verdict.rung() != 7),
        "the q=0 cap holds upstream"
    );
    // Row 1.
    if cells
        .iter()
        .any(|c| matches!(c.verdict, BurdenCellVerdict::PreconditionInvalid { .. }))
    {
        return BurdenSynthesis::InvalidGrid;
    }
    // Row 2.
    let q4 = passing_arms_at(cells, 4);
    if !q4.is_empty() {
        return BurdenSynthesis::CanonicalBurdenSurvives { arms: q4 };
    }
    // Row 3.
    let sub: Vec<(u32, BurdenSavingArm)> = [1u32, 2, 3]
        .into_iter()
        .flat_map(|q| {
            passing_arms_at(cells, q)
                .into_iter()
                .map(move |arm| (q, arm))
        })
        .collect();
    if !sub.is_empty() {
        let highest_q = sub.iter().map(|&(q, _)| q).max().expect("nonempty");
        return BurdenSynthesis::SubcanonicalSurvives {
            highest_q,
            arms: sub,
        };
    }
    // Row 4 — rows 2/3 exhausted every q<8, so any passing arm here is q=8-only.
    let q8 = passing_arms_at(cells, 8);
    if !q8.is_empty() {
        return BurdenSynthesis::StressOnly { arms: q8 };
    }
    // Row 5.
    let table: Vec<(u32, BurdenSavingArm, u64, u8)> = cells
        .iter()
        .filter(|c| c.q > 0 && c.verdict.passes())
        .map(|c| (c.q, c.arm, c.seed, c.verdict.rung()))
        .collect();
    if !table.is_empty() {
        return BurdenSynthesis::SeedHeterogeneousSurvival { table };
    }
    // Row 6 — no q>0 cell above rung 5 here (row 5 exhausted 6/7).
    if cells
        .iter()
        .any(|c| c.q > 0 && matches!(c.verdict.rung(), 4 | 5))
    {
        let mut best: BTreeMap<(u32, BurdenSavingArm), u8> = BTreeMap::new();
        for c in cells.iter().filter(|c| c.q > 0) {
            let slot = best.entry((c.q, c.arm)).or_insert(0);
            *slot = (*slot).max(c.verdict.rung());
        }
        return BurdenSynthesis::ContinuityWithoutEconomicSuccession {
            best_rung_by_arm: best
                .into_iter()
                .map(|((q, arm), rung)| (q, arm, rung))
                .collect(),
        };
    }
    // Rows 7/8 — every valid q>0 cell is at rungs 2–3 now; discriminate on the q=0 control.
    let costless = cells
        .iter()
        .any(|c| c.q == 0 && c.verdict == BurdenCellVerdict::CostlessReplacement);
    if costless {
        BurdenSynthesis::CostlessOnlyReplacement
    } else {
        BurdenSynthesis::RobustExtinction
    }
}

/// The orthogonal motive-effect payload (always printed, never a verdict): exact matched-pair
/// `(q, seed)` lists where the On rung beats the Off rung and vice versa; q=0 excluded;
/// inversions print, never smoothed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BurdenMotiveEffect {
    pub on_better: Vec<(u32, u64)>,
    pub off_better: Vec<(u32, u64)>,
}

pub fn burden_motive_effect(cells: &[BurdenCellResult]) -> BurdenMotiveEffect {
    let mut out = BurdenMotiveEffect::default();
    for c_on in cells
        .iter()
        .filter(|c| c.arm == BurdenSavingArm::On && c.q != 0)
    {
        let Some(c_off) = cells
            .iter()
            .find(|c| c.arm == BurdenSavingArm::Off && c.q == c_on.q && c.seed == c_on.seed)
        else {
            continue;
        };
        let (on, off) = (c_on.verdict.rung(), c_off.verdict.rung());
        if on > off {
            out.on_better.push((c_on.q, c_on.seed));
        } else if off > on {
            out.off_better.push((c_on.q, c_on.seed));
        }
    }
    out
}

/// The orthogonal pairwise nonmonotonicity payload (R2-4, R3-5, R4-2): every `(saving, q_low,
/// q_high)` with `q_low < q_high`, BOTH drawn from {1,2,3,4,8} (q=0 is capped and excluded),
/// where the `q_high` arm passes 5/5 and the matching `q_low` arm does not. Printed under every
/// synthesis row.
pub fn burden_nonmonotone_pairs(cells: &[BurdenCellResult]) -> Vec<(BurdenSavingArm, u32, u32)> {
    const NONMONOTONE_QS: [u32; 5] = [1, 2, 3, 4, 8];
    let mut out = Vec::new();
    for arm in [BurdenSavingArm::Off, BurdenSavingArm::On] {
        for (i, &q_low) in NONMONOTONE_QS.iter().enumerate() {
            for &q_high in &NONMONOTONE_QS[i + 1..] {
                if arm_passes(cells, q_high, arm) && !arm_passes(cells, q_low, arm) {
                    out.push((arm, q_low, q_high));
                }
            }
        }
    }
    out
}

/// The DH.c gate (§3, R1-1/R4-5): a FUTURE DH.c grand-oracle rerun is authorized only if a q=4
/// arm passes on 5/5 valid seeds at rung 6+/7. Returns the passing q=4 arm(s) — empty means NOT
/// authorized. Reported by the suite, never executed here. q=8 can never authorize (it is not
/// consulted).
pub fn burden_dh_c_gate(cells: &[BurdenCellResult]) -> Vec<BurdenSavingArm> {
    passing_arms_at(cells, 4)
}

// ===========================================================================================
// §5/§6 battery — pure-classifier tables, streak boundaries, funding matrix, synthesis rows
// ===========================================================================================

#[cfg(test)]
mod classifier_tests {
    use super::*;

    const MILL: GoodId = GoodId(101);
    const OVEN: GoodId = GoodId(102);
    const M: ClosureClass = ClosureClass::Miller;
    const B: ClosureClass = ClosureClass::Baker;
    const HEIR_M: AgentId = AgentId(11);
    const HEIR_B: AgentId = AgentId(12);
    const DEAD_M: AgentId = AgentId(1);
    const DEAD_B: AgentId = AgentId(2);

    fn window(start: u64, continuity: [bool; 2], flow: [bool; 2]) -> BurdenWindowObs {
        BurdenWindowObs {
            start,
            continuity,
            flow,
        }
    }

    /// `n` consecutive windows from `start_index` with uniform continuity/flow.
    fn windows(n: usize, continuity: [bool; 2], flow: [bool; 2]) -> Vec<BurdenWindowObs> {
        (0..n)
            .map(|i| window(72 + i as u64 * BURDEN_WINDOW_TICKS, continuity, flow))
            .collect()
    }

    fn birth(tick: u64, child: AgentId, funding: FundingBits) -> BurdenBirthObs {
        BurdenBirthObs {
            tick,
            child,
            funding,
        }
    }

    fn inh(class: ClosureClass, heir: AgentId, tool: GoodId, tick: u64) -> BurdenToolInherited {
        BurdenToolInherited {
            tick,
            class,
            deceased: if class == M { DEAD_M } else { DEAD_B },
            heir,
            tool,
        }
    }

    fn adopt(
        class: ClosureClass,
        heir: AgentId,
        tool: GoodId,
        role: Vocation,
        tick: u64,
        holds_tool: bool,
    ) -> BurdenRoleAdopted {
        BurdenRoleAdopted {
            tick,
            class,
            heir,
            tool,
            role,
            holds_tool,
        }
    }

    fn exec(agent: AgentId, recipe: RecipeId, tick: u64) -> BurdenStageExecution {
        BurdenStageExecution {
            tick,
            agent,
            recipe,
        }
    }

    /// A complete rung-7 input: 5 windows of common continuity + flow, full chains for both
    /// classes (inheritance at 40 < adoption at 50 < executions inside the streak), and one
    /// funded birth per heir.
    fn full_input(q: u32) -> BurdenCellInput {
        BurdenCellInput {
            q,
            mill_good: MILL,
            oven_good: OVEN,
            windows: windows(BURDEN_STREAK_WINDOWS, [true; 2], [true; 2]),
            inheritances: vec![inh(M, HEIR_M, MILL, 40), inh(B, HEIR_B, OVEN, 41)],
            adoptions: vec![
                adopt(M, HEIR_M, MILL, Vocation::Miller, 50, true),
                adopt(B, HEIR_B, OVEN, Vocation::Baker, 51, true),
            ],
            executions: vec![
                exec(HEIR_M, RecipeId::Mill, 80),
                exec(HEIR_B, RecipeId::Bake, 81),
            ],
            births: vec![
                birth(80, HEIR_M, FundingBits::default()),
                birth(81, HEIR_B, FundingBits::default()),
            ],
        }
    }

    #[test]
    fn start_formula_is_the_pinned_ceiling() {
        assert_eq!(burden_start_tick(0), 36);
        assert_eq!(burden_start_tick(35), 36);
        assert_eq!(burden_start_tick(36), 72);
        assert_eq!(burden_start_tick(71), 72);
        assert_eq!(burden_start_tick(72), 108);
    }

    #[test]
    fn rung7_full_chain_is_economically_funded() {
        assert_eq!(
            classify_burden_cell(&full_input(4)),
            BurdenCellVerdict::FunctionalSuccessionEconomicallyFunded
        );
    }

    #[test]
    fn rung2_no_birth_is_total_for_every_q() {
        for q in BURDEN_QS {
            let mut input = full_input(q);
            input.births.clear();
            assert_eq!(
                classify_burden_cell(&input),
                BurdenCellVerdict::NoBirth { q },
                "q={q}"
            );
        }
    }

    #[test]
    fn rung3_missing_private_streak_names_classes_miller_before_baker() {
        // Baker never continuous → Baker lacks even a private streak; Miller has one.
        let mut input = full_input(4);
        input.windows = windows(BURDEN_STREAK_WINDOWS, [true, false], [true; 2]);
        assert_eq!(
            classify_burden_cell(&input),
            BurdenCellVerdict::BirthsButLineageExtinct {
                detail: BurdenLineageExtinctDetail::MissingPrivateStreaks { classes: vec![B] },
            }
        );
        // BOTH lack one — Miller-before-Baker order.
        let mut input = full_input(4);
        input.windows = windows(BURDEN_STREAK_WINDOWS, [false, false], [true; 2]);
        assert_eq!(
            classify_burden_cell(&input),
            BurdenCellVerdict::BirthsButLineageExtinct {
                detail: BurdenLineageExtinctDetail::MissingPrivateStreaks {
                    classes: vec![M, B]
                },
            }
        );
    }

    #[test]
    fn rung3_disjoint_private_streaks_never_common() {
        // Miller continuous in windows 0–4, Baker in windows 5–9: each class has a PRIVATE
        // 5-streak but never a COMMON one — the both-have-private-but-never-common case (R3-1).
        let mut w = Vec::new();
        for i in 0..10usize {
            let miller = i < 5;
            w.push(window(
                72 + i as u64 * BURDEN_WINDOW_TICKS,
                [miller, !miller],
                [true; 2],
            ));
        }
        let mut input = full_input(4);
        input.windows = w;
        assert_eq!(
            classify_burden_cell(&input),
            BurdenCellVerdict::BirthsButLineageExtinct {
                detail: BurdenLineageExtinctDetail::DisjointPrivateStreaks,
            }
        );
    }

    #[test]
    fn streak_boundaries_off_by_one() {
        // Exactly M windows → one streak; M−1 → none (rung drops to 3).
        let mut input = full_input(4);
        input.windows = windows(BURDEN_STREAK_WINDOWS - 1, [true; 2], [true; 2]);
        assert_eq!(classify_burden_cell(&input).rung(), 3);
        // A hole at the LAST window of the only candidate streak kills it.
        let mut input = full_input(4);
        input.windows = windows(BURDEN_STREAK_WINDOWS, [true; 2], [true; 2]);
        input.windows[BURDEN_STREAK_WINDOWS - 1].continuity = [true, false];
        assert_eq!(classify_burden_cell(&input).rung(), 3);
        // Streak at the grid START and at the grid END of a longer grid: a hole at window 5
        // leaves the start streak (windows 0–4); a hole at window 0 leaves streaks from window
        // 1 on. Executions at tick 120 sit inside both candidate proof streaks.
        for hole in [5usize, 0] {
            let mut input = full_input(4);
            input.windows = windows(11, [true; 2], [true; 2]);
            input.windows[hole].continuity = [false, false];
            input.executions = vec![
                exec(HEIR_M, RecipeId::Mill, 120),
                exec(HEIR_B, RecipeId::Bake, 120),
            ];
            assert_eq!(classify_burden_cell(&input).rung(), 7, "hole at {hole}");
        }
    }

    #[test]
    fn rung4_no_events_sets_both_event_bits_for_both_classes() {
        let mut input = full_input(4);
        input.inheritances.clear();
        input.adoptions.clear();
        let expected_bits = SuccessionBits {
            inheritance_event: true,
            adoption_event: true,
            ..Default::default()
        };
        assert_eq!(
            classify_burden_cell(&input),
            BurdenCellVerdict::LineagePersistsSuccessionAbsent {
                failures: BTreeMap::from([(M, expected_bits), (B, expected_bits)]),
            }
        );
    }

    /// The six-bit negative table (R2-8/R3-6): each case perturbs ONE link of the Miller chain
    /// (Baker stays complete) and must name exactly that link.
    #[test]
    fn rung4_bit_table_names_the_failed_link() {
        type Case = (
            &'static str,
            Box<dyn Fn(&mut BurdenCellInput)>,
            SuccessionBits,
        );
        let cases: Vec<Case> = vec![
            (
                "wrong tool on the inheritance (class-correct filter)",
                Box::new(|i: &mut BurdenCellInput| i.inheritances[0].tool = OVEN),
                SuccessionBits {
                    inheritance_event: true,
                    ..Default::default()
                },
            ),
            (
                "non-born heir on the inheritance",
                Box::new(|i: &mut BurdenCellInput| i.inheritances[0].heir = AgentId(99)),
                SuccessionBits {
                    inheritance_event: true,
                    ..Default::default()
                },
            ),
            (
                "wrong class on the inheritance",
                Box::new(|i: &mut BurdenCellInput| i.inheritances[0].class = B),
                SuccessionBits {
                    inheritance_event: true,
                    ..Default::default()
                },
            ),
            (
                "wrong ROLE on the adoption (all preceding tuple fields correct)",
                Box::new(|i: &mut BurdenCellInput| i.adoptions[0].role = Vocation::Baker),
                SuccessionBits {
                    adoption_event: true,
                    ..Default::default()
                },
            ),
            (
                "wrong heir join (both events exist, tuples never meet)",
                Box::new(|i: &mut BurdenCellInput| i.adoptions[0].heir = HEIR_B),
                SuccessionBits {
                    tuple_join: true,
                    ..Default::default()
                },
            ),
            (
                "wrong tool join",
                Box::new(|i: &mut BurdenCellInput| {
                    // Keep the adoption class-correct-ROLE but on the other tool, and give the
                    // heir a second class-correct inheritance for that other... no: perturb the
                    // ADOPTION tool so the (heir, tool) tuples never meet.
                    i.adoptions[0].tool = OVEN;
                }),
                SuccessionBits {
                    tuple_join: true,
                    ..Default::default()
                },
            ),
            (
                "reversed ordering (adoption on a strictly earlier tick)",
                Box::new(|i: &mut BurdenCellInput| i.adoptions[0].tick = 39),
                SuccessionBits {
                    strict_ordering: true,
                    ..Default::default()
                },
            ),
            (
                "lost possession at adoption",
                Box::new(|i: &mut BurdenCellInput| i.adoptions[0].holds_tool = false),
                SuccessionBits {
                    possession_at_adoption: true,
                    ..Default::default()
                },
            ),
            (
                "absent execution in the streak",
                Box::new(|i: &mut BurdenCellInput| {
                    i.executions.remove(0);
                }),
                SuccessionBits {
                    successor_execution: true,
                    ..Default::default()
                },
            ),
            (
                "wrong RECIPE on the execution (all preceding tuple fields correct)",
                Box::new(|i: &mut BurdenCellInput| i.executions[0].recipe = RecipeId::Bake),
                SuccessionBits {
                    successor_execution: true,
                    ..Default::default()
                },
            ),
            (
                "execution on a strictly earlier tick than the adoption",
                Box::new(|i: &mut BurdenCellInput| i.executions[0].tick = 45),
                SuccessionBits {
                    successor_execution: true,
                    ..Default::default()
                },
            ),
            (
                "execution outside the streak",
                Box::new(|i: &mut BurdenCellInput| i.executions[0].tick = 71),
                SuccessionBits {
                    successor_execution: true,
                    ..Default::default()
                },
            ),
            (
                "inheritance at the streak end is unavailable",
                Box::new(|i: &mut BurdenCellInput| i.inheritances[0].tick = 252),
                SuccessionBits {
                    inheritance_event: true,
                    ..Default::default()
                },
            ),
            (
                "adoption at the streak end is unavailable",
                Box::new(|i: &mut BurdenCellInput| i.adoptions[0].tick = 252),
                SuccessionBits {
                    adoption_event: true,
                    ..Default::default()
                },
            ),
        ];
        for (name, perturb, expected) in cases {
            let mut input = full_input(4);
            perturb(&mut input);
            let BurdenCellVerdict::LineagePersistsSuccessionAbsent { failures } =
                classify_burden_cell(&input)
            else {
                panic!("{name}: expected rung 4");
            };
            assert_eq!(
                failures,
                BTreeMap::from([(M, expected)]),
                "{name}: only Miller fails, with exactly the named link"
            );
        }
    }

    #[test]
    fn rung4_disjoint_succession_streaks_do_not_stitch() {
        // Two disjoint C-streaks (a continuity hole between them). Miller's execution lies in
        // the FIRST streak only, Baker's in the SECOND only — no common streak satisfies both,
        // so S=∅ even though each class succeeds somewhere (no stitching of classes/periods).
        let mut input = full_input(4);
        input.windows = windows(11, [true; 2], [true; 2]);
        input.windows[5].continuity = [false, false];
        // Streak A = windows 0..5 → ticks [72, 252). Streak B = windows 6..11 → [288, 468).
        input.executions = vec![
            exec(HEIR_M, RecipeId::Mill, 100),
            exec(HEIR_B, RecipeId::Bake, 300),
        ];
        let BurdenCellVerdict::LineagePersistsSuccessionAbsent { failures } =
            classify_burden_cell(&input)
        else {
            panic!("expected rung 4");
        };
        // The earliest C-streak is streak A, where Baker's execution is missing.
        assert_eq!(
            failures,
            BTreeMap::from([(
                B,
                SuccessionBits {
                    successor_execution: true,
                    ..Default::default()
                }
            )])
        );
    }

    #[test]
    fn rung5_flow_absent_names_classes_on_the_earliest_s_streak() {
        let mut input = full_input(4);
        for w in &mut input.windows {
            w.flow = [true, false];
        }
        assert_eq!(
            classify_burden_cell(&input),
            BurdenCellVerdict::FunctionalSuccessionFlowAbsent { classes: vec![B] }
        );
        let mut input = full_input(4);
        for w in &mut input.windows {
            w.flow = [false, false];
        }
        assert_eq!(
            classify_burden_cell(&input),
            BurdenCellVerdict::FunctionalSuccessionFlowAbsent {
                classes: vec![M, B]
            }
        );
    }

    #[test]
    fn rung5_disjoint_flow_streaks_do_not_stitch() {
        // Continuity + succession hold over a long grid; Miller flow only in the first half's
        // windows, Baker flow only in the second half's — no single streak carries both.
        let mut input = full_input(4);
        input.windows = windows(10, [true; 2], [true; 2]);
        for (i, w) in input.windows.iter_mut().enumerate() {
            w.flow = [i < 5, i >= 5];
        }
        // Executions must also satisfy criterion 2 on every streak — spread them densely.
        input.executions = (0..10)
            .flat_map(|i| {
                let t = 72 + i * BURDEN_WINDOW_TICKS + 1;
                [
                    exec(HEIR_M, RecipeId::Mill, t),
                    exec(HEIR_B, RecipeId::Bake, t),
                ]
            })
            .collect();
        assert_eq!(
            classify_burden_cell(&input),
            BurdenCellVerdict::FunctionalSuccessionFlowAbsent { classes: vec![B] },
            "the earliest S-streak (the first) lacks Baker flow — never stitched with the later \
             Baker-flow windows"
        );
    }

    #[test]
    fn rung6_reasons_aggregate_over_the_earliest_f_streak() {
        let mut input = full_input(4);
        input.births = vec![
            birth(
                80,
                HEIR_M,
                FundingBits {
                    physical_endowment_taint: true,
                    ..Default::default()
                },
            ),
            birth(
                81,
                HEIR_B,
                FundingBits {
                    endowed_payment: true,
                    mixed_payment: true,
                    ..Default::default()
                },
            ),
        ];
        assert_eq!(
            classify_burden_cell(&input),
            BurdenCellVerdict::FunctionalSuccessionEconomicFundingUnproven {
                reasons: FundingBits {
                    physical_endowment_taint: true,
                    endowed_payment: true,
                    mixed_payment: true,
                    ..Default::default()
                },
            }
        );
    }

    #[test]
    fn rung6_disjoint_funded_streaks_do_not_stitch() {
        // Two F-streaks, EACH containing one unfunded birth (different reasons) — E=∅ and the
        // reasons come from the EARLIEST streak only, never a union across streaks.
        let mut input = full_input(4);
        input.windows = windows(11, [true; 2], [true; 2]);
        input.windows[5].continuity = [false, false];
        input.executions = vec![
            exec(HEIR_M, RecipeId::Mill, 100),
            exec(HEIR_B, RecipeId::Bake, 101),
            exec(HEIR_M, RecipeId::Mill, 300),
            exec(HEIR_B, RecipeId::Bake, 301),
        ];
        input.births = vec![
            birth(
                100,
                HEIR_M,
                FundingBits {
                    zero_consideration: true,
                    ..Default::default()
                },
            ),
            birth(
                300,
                HEIR_B,
                FundingBits {
                    unverifiable: true,
                    ..Default::default()
                },
            ),
        ];
        assert_eq!(
            classify_burden_cell(&input),
            BurdenCellVerdict::FunctionalSuccessionEconomicFundingUnproven {
                reasons: FundingBits {
                    zero_consideration: true,
                    ..Default::default()
                },
            },
            "reasons come from the EARLIEST F-streak's births only"
        );
    }

    #[test]
    fn q0_cap_replaces_rung_6_and_7_with_costless_replacement() {
        // Would-be rung 7.
        assert_eq!(
            classify_burden_cell(&full_input(0)),
            BurdenCellVerdict::CostlessReplacement
        );
        // Would-be rung 6 (an unfunded birth inside every streak; both heirs stay born so the
        // succession chains still hold).
        let mut input = full_input(0);
        input.births = vec![
            birth(
                80,
                HEIR_M,
                FundingBits {
                    physical_endowment_taint: true,
                    ..Default::default()
                },
            ),
            birth(81, HEIR_B, FundingBits::default()),
        ];
        assert_eq!(
            classify_burden_cell(&input),
            BurdenCellVerdict::CostlessReplacement
        );
        // Below the cap the ordinary ladder applies to q=0 too.
        let mut input = full_input(0);
        input.executions.clear();
        assert_eq!(classify_burden_cell(&input).rung(), 4);
        // CostlessReplacement never passes the DH.c bar.
        assert!(!BurdenCellVerdict::CostlessReplacement.passes());
    }

    #[test]
    fn build_windows_pins_boundaries_and_complete_windows_only() {
        // start=72, horizon=180: exactly three complete windows [72,108), [108,144), [144,180).
        let mut continuity = vec![[true, true]; 180];
        continuity[143] = [true, false]; // Baker hole at the LAST sample of window 1
        let executions = vec![
            exec(HEIR_M, RecipeId::Mill, 72),  // first sample of window 0
            exec(HEIR_B, RecipeId::Bake, 143), // last sample of window 1
            exec(HEIR_M, RecipeId::Mill, 144), // first sample of window 2 — not window 1
        ];
        let w = build_burden_windows(72, 180, &continuity, &executions);
        assert_eq!(w.len(), 3);
        assert_eq!(w[0].start, 72);
        assert_eq!(w[0].continuity, [true, true]);
        assert_eq!(w[0].flow, [true, false]);
        assert_eq!(
            w[1].continuity,
            [true, false],
            "one bad sample fails the window"
        );
        assert_eq!(w[1].flow, [false, true]);
        assert_eq!(
            w[2].flow,
            [true, false],
            "tick 144 belongs to window 2, not 1"
        );
        // horizon 179: the third window is incomplete and must be dropped.
        let w = build_burden_windows(72, 179, &continuity, &executions);
        assert_eq!(w.len(), 2);
        // Samples past the recorded trace read as discontinuity, never as alive.
        let w = build_burden_windows(72, 216, &vec![[true, true]; 180], &[]);
        assert_eq!(w.len(), 4);
        assert_eq!(w[3].continuity, [false, false]);
    }

    // ---- the funding classification matrix (§3.4) ----

    fn trade(
        trade_id: u64,
        earned_paid: u64,
        endowed_paid: u64,
        positive_consideration: bool,
    ) -> BurdenTradeRecord {
        BurdenTradeRecord {
            trade_id,
            buyer: HEIR_M,
            good: GoodId(7),
            quantity: 1,
            earned_paid,
            endowed_paid,
            positive_consideration,
        }
    }

    fn lot(channel: BurdenChannel, identity: Option<u64>, taint: bool) -> BurdenLot {
        BurdenLot {
            channel,
            qty: 1,
            identity,
            taint,
        }
    }

    #[test]
    fn funding_matrix_every_combination() {
        use BurdenChannel::*;
        let none = FundingBits::default();
        let cases: Vec<(&str, Vec<BurdenLot>, Vec<BurdenTradeRecord>, FundingBits)> = vec![
            ("q=0 empty lots are vacuously funded", vec![], vec![], none),
            (
                "SelfProduced untainted qualifies",
                vec![lot(SelfProduced, None, false)],
                vec![],
                none,
            ),
            (
                "SelfProduced with purchase identity is malformed telemetry",
                vec![lot(SelfProduced, Some(5), false)],
                vec![],
                FundingBits {
                    unverifiable: true,
                    ..none
                },
            ),
            (
                "tainted SelfProduced is malformed telemetry",
                vec![lot(SelfProduced, None, true)],
                vec![],
                FundingBits {
                    physical_endowment_taint: true,
                    unverifiable: true,
                    ..none
                },
            ),
            (
                "zero-qty lots are ignored",
                vec![BurdenLot {
                    channel: Foraged,
                    qty: 0,
                    identity: None,
                    taint: true,
                }],
                vec![],
                none,
            ),
            (
                "SeededMinted carries the construction taint",
                vec![lot(SeededMinted, None, true)],
                vec![],
                FundingBits {
                    physical_endowment_taint: true,
                    ..none
                },
            ),
            (
                "untainted SeededMinted is malformed telemetry",
                vec![lot(SeededMinted, None, false)],
                vec![],
                FundingBits {
                    unverifiable: true,
                    ..none
                },
            ),
            (
                "SeededMinted with purchase identity is malformed telemetry",
                vec![lot(SeededMinted, Some(5), true)],
                vec![],
                FundingBits {
                    physical_endowment_taint: true,
                    unverifiable: true,
                    ..none
                },
            ),
            (
                "Bought + clean earned trade qualifies",
                vec![lot(Bought, Some(5), false)],
                vec![trade(5, 3, 0, true)],
                none,
            ),
            (
                "Bought resale of a construction loaf keeps the taint",
                vec![lot(Bought, Some(5), true)],
                vec![trade(5, 3, 0, true)],
                FundingBits {
                    physical_endowment_taint: true,
                    ..none
                },
            ),
            (
                "endowed-only payment",
                vec![lot(Bought, Some(5), false)],
                vec![trade(5, 0, 3, true)],
                FundingBits {
                    endowed_payment: true,
                    ..none
                },
            ),
            (
                "mixed payment sets BOTH endowed and mixed",
                vec![lot(Bought, Some(5), false)],
                vec![trade(5, 2, 3, true)],
                FundingBits {
                    endowed_payment: true,
                    mixed_payment: true,
                    ..none
                },
            ),
            (
                "zero consideration",
                vec![lot(Bought, Some(5), false)],
                vec![trade(5, 0, 0, false)],
                FundingBits {
                    zero_consideration: true,
                    ..none
                },
            ),
            (
                "positive consideration with both buckets zero is malformed",
                vec![lot(Bought, Some(5), false)],
                vec![trade(5, 0, 0, true)],
                FundingBits {
                    unverifiable: true,
                    ..none
                },
            ),
            (
                "zero-consideration flag with an earned bucket is malformed",
                vec![lot(Bought, Some(5), false)],
                vec![trade(5, 3, 0, false)],
                FundingBits {
                    unverifiable: true,
                    ..none
                },
            ),
            (
                "zero-consideration flag with an endowed bucket is malformed, not EndowedPayment",
                vec![lot(Bought, Some(5), false)],
                vec![trade(5, 0, 3, false)],
                FundingBits {
                    unverifiable: true,
                    ..none
                },
            ),
            (
                "Bought without identity is unverifiable",
                vec![lot(Bought, None, false)],
                vec![trade(5, 3, 0, true)],
                FundingBits {
                    unverifiable: true,
                    ..none
                },
            ),
            (
                "identity joining zero records is unverifiable",
                vec![lot(Bought, Some(9), false)],
                vec![trade(5, 3, 0, true)],
                FundingBits {
                    unverifiable: true,
                    ..none
                },
            ),
            (
                "identity joining two records is unverifiable",
                vec![lot(Bought, Some(5), false)],
                vec![trade(5, 3, 0, true), trade(5, 1, 0, true)],
                FundingBits {
                    unverifiable: true,
                    ..none
                },
            ),
            (
                "Foraged is unreachable-conservative",
                vec![lot(Foraged, None, false)],
                vec![],
                FundingBits {
                    unverifiable: true,
                    ..none
                },
            ),
            (
                "Commons is unreachable-conservative",
                vec![lot(Commons, None, false)],
                vec![],
                FundingBits {
                    unverifiable: true,
                    ..none
                },
            ),
            (
                "multi-lot union",
                vec![
                    lot(SeededMinted, None, true),
                    lot(Bought, Some(5), false),
                    lot(SelfProduced, None, false),
                ],
                vec![trade(5, 1, 2, true)],
                FundingBits {
                    physical_endowment_taint: true,
                    endowed_payment: true,
                    mixed_payment: true,
                    ..none
                },
            ),
        ];
        for (name, lots, trades, expected) in cases {
            assert_eq!(classify_birth_funding(&lots, &trades), expected, "{name}");
        }
    }
}

#[cfg(test)]
mod synthesis_tests {
    use super::*;

    fn cell(q: u32, arm: BurdenSavingArm, seed: u64, rung: u8) -> BurdenCellResult {
        let verdict = match rung {
            1 => BurdenCellVerdict::PreconditionInvalid {
                guard: "test".to_string(),
            },
            2 => BurdenCellVerdict::NoBirth { q },
            3 => BurdenCellVerdict::BirthsButLineageExtinct {
                detail: BurdenLineageExtinctDetail::DisjointPrivateStreaks,
            },
            4 => BurdenCellVerdict::LineagePersistsSuccessionAbsent {
                failures: BTreeMap::from([(
                    ClosureClass::Miller,
                    SuccessionBits {
                        inheritance_event: true,
                        ..Default::default()
                    },
                )]),
            },
            5 => BurdenCellVerdict::FunctionalSuccessionFlowAbsent {
                classes: vec![ClosureClass::Miller],
            },
            6 => BurdenCellVerdict::FunctionalSuccessionEconomicFundingUnproven {
                reasons: FundingBits {
                    physical_endowment_taint: true,
                    ..Default::default()
                },
            },
            7 => BurdenCellVerdict::FunctionalSuccessionEconomicallyFunded,
            0 => BurdenCellVerdict::CostlessReplacement,
            _ => unreachable!(),
        };
        BurdenCellResult {
            q,
            arm,
            seed,
            verdict,
        }
    }

    /// A full 60-cell grid where every cell gets `rung_of(q, arm, seed)`.
    fn grid(rung_of: impl Fn(u32, BurdenSavingArm, u64) -> u8) -> Vec<BurdenCellResult> {
        let mut cells = Vec::new();
        for &seed in &BURDEN_SEEDS {
            for &q in &BURDEN_QS {
                for arm in [BurdenSavingArm::Off, BurdenSavingArm::On] {
                    cells.push(cell(q, arm, seed, rung_of(q, arm, seed)));
                }
            }
        }
        cells
    }

    /// The baseline all-extinct grid: q=0 at rung 3, every q>0 at rung 2/3.
    fn extinct_rung(q: u32, _arm: BurdenSavingArm, _seed: u64) -> u8 {
        if q == 0 {
            3
        } else {
            2
        }
    }

    #[test]
    fn row1_invalid_grid_dominates() {
        let mut cells = grid(|q, a, s| if q == 4 { 7 } else { extinct_rung(q, a, s) });
        cells[0] = cell(0, BurdenSavingArm::Off, 3, 1);
        assert_eq!(synthesize_burden_grid(&cells), BurdenSynthesis::InvalidGrid);
    }

    #[test]
    fn row2_canonical_survives_names_the_arms() {
        let cells = grid(|q, arm, s| {
            if q == 4 && arm == BurdenSavingArm::On {
                7
            } else if q == 2 {
                // A sub-canonical pass must NOT preempt the canonical row.
                6
            } else {
                extinct_rung(q, arm, s)
            }
        });
        assert_eq!(
            synthesize_burden_grid(&cells),
            BurdenSynthesis::CanonicalBurdenSurvives {
                arms: vec![BurdenSavingArm::On],
            }
        );
        assert_eq!(burden_dh_c_gate(&cells), vec![BurdenSavingArm::On]);
    }

    #[test]
    fn five_of_five_requires_each_pinned_seed_exactly_once() {
        let partial = vec![cell(4, BurdenSavingArm::On, BURDEN_SEEDS[0], 7)];
        assert!(burden_dh_c_gate(&partial).is_empty());
        assert!(!matches!(
            synthesize_burden_grid(&partial),
            BurdenSynthesis::CanonicalBurdenSurvives { .. }
        ));

        let mut duplicated: Vec<BurdenCellResult> = BURDEN_SEEDS
            .into_iter()
            .map(|seed| cell(4, BurdenSavingArm::On, seed, 7))
            .collect();
        duplicated.push(cell(4, BurdenSavingArm::On, BURDEN_SEEDS[0], 7));
        assert!(burden_dh_c_gate(&duplicated).is_empty());
        assert!(!matches!(
            synthesize_burden_grid(&duplicated),
            BurdenSynthesis::CanonicalBurdenSurvives { .. }
        ));
    }

    #[test]
    fn row3_subcanonical_names_highest_q_and_arms() {
        let cells = grid(|q, arm, s| {
            if (q == 1 || q == 3) && arm == BurdenSavingArm::Off {
                6
            } else {
                extinct_rung(q, arm, s)
            }
        });
        assert_eq!(
            synthesize_burden_grid(&cells),
            BurdenSynthesis::SubcanonicalSurvives {
                highest_q: 3,
                arms: vec![(1, BurdenSavingArm::Off), (3, BurdenSavingArm::Off)],
            }
        );
        assert_eq!(burden_dh_c_gate(&cells), Vec::<BurdenSavingArm>::new());
    }

    #[test]
    fn row4_stress_only_q8_cannot_authorize_dh_c() {
        let cells = grid(|q, _arm, s| {
            if q == 8 {
                7
            } else {
                extinct_rung(q, BurdenSavingArm::Off, s)
            }
        });
        assert_eq!(
            synthesize_burden_grid(&cells),
            BurdenSynthesis::StressOnly {
                arms: vec![BurdenSavingArm::Off, BurdenSavingArm::On],
            }
        );
        assert_eq!(
            burden_dh_c_gate(&cells),
            Vec::<BurdenSavingArm>::new(),
            "a q=8-only pass NEVER authorizes the DH.c rerun"
        );
    }

    #[test]
    fn row5_heterogeneous_when_no_arm_passes_all_seeds() {
        let cells = grid(|q, arm, seed| {
            if q == 4 && arm == BurdenSavingArm::On && seed == 11 {
                7
            } else {
                extinct_rung(q, arm, seed)
            }
        });
        assert_eq!(
            synthesize_burden_grid(&cells),
            BurdenSynthesis::SeedHeterogeneousSurvival {
                table: vec![(4, BurdenSavingArm::On, 11, 7)],
            }
        );
    }

    #[test]
    fn row6_continuity_without_economic_succession() {
        let cells = grid(|q, arm, s| {
            if q == 2 && arm == BurdenSavingArm::Off {
                5
            } else if q == 1 {
                4
            } else {
                extinct_rung(q, arm, s)
            }
        });
        let BurdenSynthesis::ContinuityWithoutEconomicSuccession { best_rung_by_arm } =
            synthesize_burden_grid(&cells)
        else {
            panic!("expected row 6");
        };
        assert!(best_rung_by_arm.contains(&(2, BurdenSavingArm::Off, 5)));
        assert!(best_rung_by_arm.contains(&(1, BurdenSavingArm::On, 4)));
    }

    #[test]
    fn row6_vs_row8_discrimination_rung3_only_is_not_row6() {
        // R2-3: a rung-3-only grid (q>0 all at 2–3) must fall through to rows 7/8, never row 6.
        let cells = grid(extinct_rung);
        assert_eq!(
            synthesize_burden_grid(&cells),
            BurdenSynthesis::RobustExtinction
        );
        // …and a single rung-4 q>0 cell flips it to row 6.
        let cells = grid(|q, arm, s| {
            if q == 3 && arm == BurdenSavingArm::On && s == 23 {
                4
            } else {
                extinct_rung(q, arm, s)
            }
        });
        assert!(matches!(
            synthesize_burden_grid(&cells),
            BurdenSynthesis::ContinuityWithoutEconomicSuccession { .. }
        ));
    }

    #[test]
    fn row7_costless_only_vs_row8_disjoint_on_the_q0_control() {
        let costless = grid(|q, a, s| if q == 0 { 0 } else { extinct_rung(q, a, s) });
        assert_eq!(
            synthesize_burden_grid(&costless),
            BurdenSynthesis::CostlessOnlyReplacement
        );
        let extinct = grid(extinct_rung);
        assert_eq!(
            synthesize_burden_grid(&extinct),
            BurdenSynthesis::RobustExtinction
        );
        // A q=0 cell at rung 4 is NOT CostlessReplacement — row 8 still matches (rows 6/7/8
        // stay disjoint: row 6 consults q>0 cells only).
        let q0_rung4 = grid(|q, a, s| if q == 0 { 4 } else { extinct_rung(q, a, s) });
        assert_eq!(
            synthesize_burden_grid(&q0_rung4),
            BurdenSynthesis::RobustExtinction
        );
    }

    #[test]
    fn motive_effect_lists_matched_pairs_and_excludes_q0() {
        let cells = grid(|q, arm, seed| match (q, arm, seed) {
            // q=0 On/Off asymmetry must NOT appear in the lists.
            (0, BurdenSavingArm::On, _) => 0,
            (0, BurdenSavingArm::Off, _) => 3,
            (2, BurdenSavingArm::On, 7) => 5,
            (2, BurdenSavingArm::Off, 7) => 3,
            (3, BurdenSavingArm::Off, 11) => 4,
            _ => 2,
        });
        let effect = burden_motive_effect(&cells);
        assert_eq!(effect.on_better, vec![(2, 7)]);
        assert_eq!(effect.off_better, vec![(3, 11)]);
    }

    #[test]
    fn nonmonotone_pairs_catch_non_downward_closed_sets_and_never_q0() {
        // On arm passes at q∈{2,3} but not q=1 (and not 4/8): pairwise tuples (1,2) and (1,3)
        // (the contiguous-but-not-downward-closed {2,3} case), plus (4-, 8-) high-side misses
        // produce nothing.
        let cells = grid(|q, arm, _| {
            if arm == BurdenSavingArm::On && (q == 2 || q == 3) {
                7
            } else if q == 0 {
                0
            } else {
                2
            }
        });
        let pairs = burden_nonmonotone_pairs(&cells);
        assert_eq!(
            pairs,
            vec![(BurdenSavingArm::On, 1, 2), (BurdenSavingArm::On, 1, 3),]
        );
        assert!(
            pairs.iter().all(|&(_, lo, hi)| lo != 0 && hi != 0),
            "no emitted tuple contains q=0 (R4-2)"
        );
        // A q=0 CostlessReplacement beside a passing q never yields a (0, q) tuple.
        let cells = grid(|q, _, _| {
            if q == 0 {
                0
            } else if q == 8 {
                7
            } else {
                2
            }
        });
        let pairs = burden_nonmonotone_pairs(&cells);
        assert!(pairs.iter().all(|&(_, lo, _)| lo != 0));
        assert_eq!(
            pairs,
            vec![
                (BurdenSavingArm::Off, 1, 8),
                (BurdenSavingArm::Off, 2, 8),
                (BurdenSavingArm::Off, 3, 8),
                (BurdenSavingArm::Off, 4, 8),
                (BurdenSavingArm::On, 1, 8),
                (BurdenSavingArm::On, 2, 8),
                (BurdenSavingArm::On, 3, 8),
                (BurdenSavingArm::On, 4, 8),
            ]
        );
    }
}

// ===========================================================================================
// R2-7 lifecycle tests — the landed lot machinery under the burden extension
// ===========================================================================================

#[cfg(test)]
mod lifecycle_tests {
    use super::super::AcquisitionLedger;
    use super::*;

    const SELLER: AgentId = AgentId(1);
    const BUYER: AgentId = AgentId(2);
    const CHILD: AgentId = AgentId(3);

    fn ledger() -> AcquisitionLedger {
        AcquisitionLedger {
            burden_provenance: true,
            ..Default::default()
        }
    }

    #[test]
    fn construction_taint_is_set_for_seeded_minted_and_clear_for_self_produced() {
        let mut l = ledger();
        l.credit(SELLER, FoodChannel::SeededMinted, 2);
        l.credit(SELLER, FoodChannel::SelfProduced, 3);
        let lots = &l.lots[&SELLER];
        assert_eq!((lots[0].taint, lots[0].identity), (true, None));
        assert_eq!((lots[1].taint, lots[1].identity), (false, None));
        // Off the extension the same credits stay untainted — the landed behavior.
        let mut l = AcquisitionLedger::default();
        l.credit(SELLER, FoodChannel::SeededMinted, 2);
        assert!(!l.lots[&SELLER][0].taint);
        assert!(l.burden_violations.is_empty());
    }

    #[test]
    fn foraged_and_commons_credits_are_recorded_violations_under_the_extension() {
        let mut l = ledger();
        l.credit(SELLER, FoodChannel::Foraged, 1);
        l.credit(SELLER, FoodChannel::Commons, 1);
        assert_eq!(l.burden_violations.len(), 2);
        // Off the extension they are ordinary channels (other configs use them).
        let mut l = AcquisitionLedger::default();
        l.credit(SELLER, FoodChannel::Foraged, 1);
        assert!(l.burden_violations.is_empty());
    }

    #[test]
    fn resale_overwrites_identity_and_preserves_taint() {
        let mut l = ledger();
        l.credit(SELLER, FoodChannel::SeededMinted, 4);
        let (_, fresh) = l.transfer_as_bought_identified(SELLER, BUYER, 4, Some(7));
        assert_eq!(fresh, 4);
        assert_eq!(
            l.lots[&BUYER][0],
            FoodLot {
                channel: FoodChannel::Bought,
                qty: 4,
                intervention: false,
                identity: Some(7),
                taint: true,
            },
            "the first sale stamps the trade id and keeps the construction taint"
        );
        // Resale: the identity is OVERWRITTEN with the new trade id; the taint survives.
        l.transfer_as_bought_identified(BUYER, CHILD, 4, Some(9));
        assert_eq!(
            l.lots[&CHILD][0],
            FoodLot {
                channel: FoodChannel::Bought,
                qty: 4,
                intervention: false,
                identity: Some(9),
                taint: true,
            }
        );
    }

    #[test]
    fn split_inheritance_and_birth_transfer_preserve_identity_and_taint() {
        let mut l = ledger();
        l.credit(SELLER, FoodChannel::SeededMinted, 3);
        l.transfer_as_bought_identified(SELLER, BUYER, 3, Some(11));
        // A partial draw SPLITS the lot: both fragments keep identity + taint.
        let drawn = l.transfer_preserve(BUYER, CHILD, 2);
        assert_eq!(
            drawn,
            vec![FoodLot {
                channel: FoodChannel::Bought,
                qty: 2,
                intervention: false,
                identity: Some(11),
                taint: true,
            }]
        );
        assert_eq!(
            l.lots[&BUYER][0],
            FoodLot {
                channel: FoodChannel::Bought,
                qty: 1,
                intervention: false,
                identity: Some(11),
                taint: true,
            },
            "the remainder fragment keeps the same identity and taint"
        );
        assert_eq!(l.lots[&CHILD][0].identity, Some(11));
        assert!(l.lots[&CHILD][0].taint);
    }

    #[test]
    fn intervention_stamp_preserves_identity_and_taint() {
        let mut l = ledger();
        l.credit(SELLER, FoodChannel::SeededMinted, 2);
        l.transfer_as_bought_identified(SELLER, BUYER, 2, Some(13));
        let moved = l.transfer_preserve_as_intervention(BUYER, CHILD, 2);
        assert_eq!(moved.len(), 1);
        assert!(moved[0].intervention);
        assert_eq!(moved[0].identity, Some(13));
        assert!(moved[0].taint);
    }

    #[test]
    fn coalescing_requires_taint_and_identity_equality_beyond_the_landed_key() {
        // Adjacent drawn lots differing ONLY in taint must NOT merge in the retag.
        let mut l = ledger();
        l.credit(SELLER, FoodChannel::SeededMinted, 2); // taint=true
        l.credit(SELLER, FoodChannel::SelfProduced, 3); // taint=false
        l.transfer_as_bought_identified(SELLER, BUYER, 5, Some(21));
        let lots = &l.lots[&BUYER];
        assert_eq!(lots.len(), 2, "a taint boundary blocks coalescing");
        assert_eq!((lots[0].qty, lots[0].taint), (2, true));
        assert_eq!((lots[1].qty, lots[1].taint), (3, false));
        // Two equal-taint lots with DIFFERENT old identities resold in one trade: the resale
        // overwrites both with the SAME new identity, so they coalesce into one lot — the
        // "coalesce requires identity equality" rule composed with the resale-overwrite rule.
        let mut l = ledger();
        l.credit(SELLER, FoodChannel::SelfProduced, 2);
        l.transfer_as_bought_identified(SELLER, BUYER, 2, Some(31));
        l.credit(SELLER, FoodChannel::SelfProduced, 3);
        l.transfer_as_bought_identified(SELLER, BUYER, 3, Some(32));
        assert_eq!(l.lots[&BUYER].len(), 2, "distinct identities sit apart");
        l.transfer_as_bought_identified(BUYER, CHILD, 5, Some(33));
        assert_eq!(
            l.lots[&CHILD].iter().collect::<Vec<_>>(),
            vec![&FoodLot {
                channel: FoodChannel::Bought,
                qty: 5,
                intervention: false,
                identity: Some(33),
                taint: false,
            }]
        );
    }
}

// ===========================================================================================
// Real-seam tests (§3.4/§5.2) — driven through the live closed-base seams, never fixtures
// ===========================================================================================

#[cfg(test)]
mod real_seam_tests {
    use super::super::closure::ClosureEventKind;
    use super::super::{BirthStockSavingMode, Settlement, SettlementConfig};
    use super::*;
    use econ::agent::WantKind;
    use econ::good::Horizon;
    use std::collections::VecDeque;

    fn cell_config(q: u32, saving_on: bool) -> SettlementConfig {
        let mut cfg = SettlementConfig::frontier_closed_circulation();
        cfg.demography
            .as_mut()
            .expect("the closed base carries demography")
            .child_food_endowment = q;
        if saving_on {
            let chain = cfg.chain.as_mut().expect("the closed base carries a chain");
            chain.birth_stock_saving = true;
            chain.birth_stock_saving_mode = BirthStockSavingMode::Motive;
        }
        cfg
    }

    fn mill_oven(s: &Settlement) -> (GoodId, GoodId) {
        let chain = s.chain.as_ref().expect("chain");
        (chain.content.mill(), chain.content.oven())
    }

    /// Credit `qty` of `good` to `id`, mirroring the closure ledger (an own-produced deposit)
    /// and — for the tracked food — the acquisition ledger, so every shadow invariant holds.
    fn mirror_credit(s: &mut Settlement, id: AgentId, good: GoodId, qty: u32) {
        assert!(s.society.credit_stock(id, good, qty), "credit lands");
        let tick = s.econ_tick;
        s.closure.record(
            tick,
            ClosureEventKind::GatherDeposit {
                agent: id,
                good,
                qty,
            },
        );
        if Some(good) == s.acquisition_food_good() {
            s.acquisition
                .credit(id, FoodChannel::SelfProduced, u64::from(qty));
        }
    }

    /// Debit `qty` of `good` from `id`, mirroring the closure ledger (a consumption debit)
    /// and — for the tracked food — the acquisition ledger.
    fn mirror_debit(s: &mut Settlement, id: AgentId, good: GoodId, qty: u32) {
        assert!(s.society.debit_stock(id, good, qty), "debit lands");
        let tick = s.econ_tick;
        s.closure.record(
            tick,
            ClosureEventKind::Consumption {
                agent: id,
                good,
                qty,
            },
        );
        if Some(good) == s.acquisition_food_good() {
            s.acquisition.consume(id, u64::from(qty));
        }
    }

    /// The positive real-seam chain (q=0 reachability control): inheritance → adoption →
    /// execution recorded at the live seams, the two birth streams complete with
    /// multiplicity one, and the seam's structural ordering/possession guarantees.
    #[test]
    fn positive_chain_and_birth_streams_at_the_real_seams() {
        let cfg = cell_config(0, false);
        let mut s = Settlement::generate(3, &cfg);
        for _ in 0..400 {
            s.econ_tick();
        }
        let (mill, oven) = mill_oven(&s);

        // The two independent per-birth streams: equal cardinality, equal ID sets,
        // multiplicity one, and zero drawn lots at q=0.
        let births = s.burden_birth_events();
        let funding = s.burden_birth_funding_records();
        assert!(!births.is_empty(), "q=0 reaches births on this base");
        assert_eq!(births.len(), funding.len());
        let birth_ids: BTreeSet<AgentId> = births.iter().map(|b| b.child).collect();
        let funding_ids: BTreeSet<AgentId> = funding.iter().map(|f| f.child).collect();
        assert_eq!(birth_ids.len(), births.len(), "one BirthOccurred per child");
        assert_eq!(
            funding_ids.len(),
            funding.len(),
            "one funding record per child"
        );
        assert_eq!(birth_ids, funding_ids);
        assert!(funding.iter().all(|f| f.q == 0 && f.lots.is_empty()));

        // A full class-correct chain exists at the real seams for at least one class.
        let chain_exists = |class: ClosureClass, tool: GoodId, role: Vocation, recipe: RecipeId| {
            s.burden_tool_inheritances().iter().any(|i| {
                i.class == class
                    && i.tool == tool
                    && birth_ids.contains(&i.heir)
                    && s.burden_role_adoptions().iter().any(|a| {
                        a.class == class
                            && a.heir == i.heir
                            && a.tool == i.tool
                            && a.role == role
                            && i.tick <= a.tick
                            && a.holds_tool
                            && s.burden_stage_executions().iter().any(|e| {
                                e.agent == i.heir && e.recipe == recipe && e.tick >= a.tick
                            })
                    })
            })
        };
        assert!(
            chain_exists(ClosureClass::Miller, mill, Vocation::Miller, RecipeId::Mill)
                || chain_exists(ClosureClass::Baker, oven, Vocation::Baker, RecipeId::Bake),
            "a complete inheritance→adoption→execution chain forms at the real seams"
        );

        // The seam's structural guarantees: every adoption event strictly follows a matching
        // inheritance for the same (heir, tool) — reversed ordering is unreachable at the live
        // seam (the strict_ordering bit is exercised by the pure classifier table) — and
        // possession holds at every adoption (candidates derive from current holdings).
        for a in s.burden_role_adoptions() {
            assert!(
                s.burden_tool_inheritances()
                    .iter()
                    .any(|i| i.heir == a.heir && i.tool == a.tool && i.tick <= a.tick),
                "adoption without a causally-preceding matching inheritance: {a:?}"
            );
            assert!(a.holds_tool, "the live seam adopts from current holdings");
        }

        // No corruption anywhere.
        assert!(s.burden_seam_violations().is_empty());
        assert!(s.burden_lot_audit().is_empty());
    }

    /// Real-seam negatives assembled from live-seam events by SUBSET selection (never
    /// synthetic events): wrong heir (tuple join) and absent execution (span before the first
    /// execution).
    #[test]
    fn wrong_heir_and_absent_execution_negatives_from_live_events() {
        let cfg = cell_config(0, false);
        let mut s = Settlement::generate(3, &cfg);
        for _ in 0..400 {
            s.econ_tick();
        }
        let (mill, oven) = mill_oven(&s);
        let born: BTreeSet<AgentId> = s.burden_birth_events().iter().map(|b| b.child).collect();

        // Two DIFFERENT Miller-class heirs with live-seam events: A's inheritance beside B's
        // adoption never joins (the wrong-heir negative).
        let miller_inh: Vec<BurdenToolInherited> = s
            .burden_tool_inheritances()
            .iter()
            .filter(|i| i.class == ClosureClass::Miller && i.tool == mill)
            .copied()
            .collect();
        let heir_a = miller_inh.first().expect("a Miller inheritance").heir;
        let adoption_b = s
            .burden_role_adoptions()
            .iter()
            .find(|a| {
                a.class == ClosureClass::Miller && a.role == Vocation::Miller && a.heir != heir_a
            })
            .copied()
            .expect("a second Miller heir's adoption");
        let input = BurdenCellInput {
            q: 0,
            mill_good: mill,
            oven_good: oven,
            windows: Vec::new(),
            inheritances: miller_inh
                .iter()
                .filter(|i| i.heir == heir_a)
                .copied()
                .collect(),
            adoptions: vec![adoption_b],
            executions: s.burden_stage_executions().to_vec(),
            births: s
                .burden_birth_events()
                .iter()
                .map(|b| BurdenBirthObs {
                    tick: b.tick,
                    child: b.child,
                    funding: FundingBits::default(),
                })
                .collect(),
        };
        assert_eq!(
            succession_for_class(&input, ClosureClass::Miller, (0, BURDEN_RUN_TICKS), &born),
            Err(SuccessionBits {
                tuple_join: true,
                ..Default::default()
            }),
            "live events from two different heirs never tuple-join"
        );

        // Absent execution: retain the real complete inheritance/adoption chain but omit the
        // Miller execution observations from the evaluated live stream.
        let mut full = BurdenCellInput {
            inheritances: s.burden_tool_inheritances().to_vec(),
            adoptions: s.burden_role_adoptions().to_vec(),
            ..input
        };
        full.executions.retain(|e| e.recipe != RecipeId::Mill);
        assert_eq!(
            succession_for_class(&full, ClosureClass::Miller, (0, BURDEN_RUN_TICKS), &born),
            Err(SuccessionBits {
                successor_execution: true,
                ..Default::default()
            }),
            "the same live chain fails when its class-correct executions are absent"
        );
    }

    /// The remaining isolated criterion-2 negatives, each derived from ONE complete chain
    /// emitted by the live estate → role-choice → production seams. The baseline is kept
    /// byte-for-byte from the live events; each case changes only the observation named by the
    /// case so an earlier failed link cannot mask the intended diagnostic.
    #[test]
    fn isolated_role_recipe_ordering_and_possession_negatives_from_live_chain() {
        let cfg = cell_config(0, false);
        let mut s = Settlement::generate(3, &cfg);
        for _ in 0..400 {
            s.econ_tick();
        }
        let (mill, oven) = mill_oven(&s);
        let born: BTreeSet<AgentId> = s.burden_birth_events().iter().map(|b| b.child).collect();

        let (inheritance, adoption, execution) = s
            .burden_tool_inheritances()
            .iter()
            .filter(|i| i.class == ClosureClass::Miller && i.tool == mill && born.contains(&i.heir))
            .find_map(|&inheritance| {
                s.burden_role_adoptions()
                    .iter()
                    .filter(|a| {
                        a.class == ClosureClass::Miller
                            && a.heir == inheritance.heir
                            && a.tool == inheritance.tool
                            && a.role == Vocation::Miller
                            && inheritance.tick <= a.tick
                            && a.holds_tool
                    })
                    .find_map(|&adoption| {
                        s.burden_stage_executions()
                            .iter()
                            .find(|e| {
                                e.agent == adoption.heir
                                    && e.recipe == RecipeId::Mill
                                    && e.tick >= adoption.tick
                            })
                            .copied()
                            .map(|execution| (inheritance, adoption, execution))
                    })
            })
            .expect("a complete Miller chain is emitted at all three live seams");

        let live_input = BurdenCellInput {
            q: 0,
            mill_good: mill,
            oven_good: oven,
            windows: Vec::new(),
            inheritances: vec![inheritance],
            adoptions: vec![adoption],
            executions: vec![execution],
            births: vec![BurdenBirthObs {
                tick: 0,
                child: inheritance.heir,
                funding: FundingBits::default(),
            }],
        };
        assert_eq!(
            succession_for_class(
                &live_input,
                ClosureClass::Miller,
                (0, BURDEN_RUN_TICKS),
                &born
            ),
            Ok(()),
            "the unmodified live chain is the positive control"
        );

        let mut wrong_role = live_input.clone();
        wrong_role.adoptions[0].role = Vocation::Baker;
        assert_eq!(
            succession_for_class(
                &wrong_role,
                ClosureClass::Miller,
                (0, BURDEN_RUN_TICKS),
                &born
            ),
            Err(SuccessionBits {
                adoption_event: true,
                ..Default::default()
            }),
            "wrong role is isolated: class/heir/tool and the live predecessor are unchanged"
        );

        let mut wrong_recipe = live_input.clone();
        wrong_recipe.executions[0].recipe = RecipeId::Bake;
        assert_eq!(
            succession_for_class(
                &wrong_recipe,
                ClosureClass::Miller,
                (0, BURDEN_RUN_TICKS),
                &born
            ),
            Err(SuccessionBits {
                successor_execution: true,
                ..Default::default()
            }),
            "wrong recipe leaves the complete live inheritance/adoption chain intact"
        );

        let mut reversed = live_input.clone();
        reversed.adoptions[0].tick = inheritance
            .tick
            .checked_sub(1)
            .expect("the live inheritance occurs after tick 0");
        assert_eq!(
            succession_for_class(
                &reversed,
                ClosureClass::Miller,
                (0, BURDEN_RUN_TICKS),
                &born
            ),
            Err(SuccessionBits {
                strict_ordering: true,
                ..Default::default()
            }),
            "only the live adoption's ordering observation is reversed"
        );

        let mut lost_possession = live_input;
        lost_possession.adoptions[0].holds_tool = false;
        assert_eq!(
            succession_for_class(
                &lost_possession,
                ClosureClass::Miller,
                (0, BURDEN_RUN_TICKS),
                &born
            ),
            Err(SuccessionBits {
                possession_at_adoption: true,
                ..Default::default()
            }),
            "only the live adoption's possession observation is removed"
        );
    }

    /// The manufactured wrong-tool / wrong-role negative at the REAL seams: a Miller-class
    /// producer dies holding ONLY an oven (the wrong stage tool for its class), the heir
    /// inherits it through the live estate seam and — holding only that oven — adopts BAKER
    /// through the live role-choice seam. The recorded events carry class=Miller with
    /// tool=oven / role=Baker, and the class-correct filters reject both.
    #[test]
    fn manufactured_wrong_tool_inheritance_and_wrong_role_adoption() {
        let cfg = cell_config(0, false);
        let mut s = Settlement::generate(3, &cfg);
        for _ in 0..120 {
            s.econ_tick();
        }
        let (mill, oven) = mill_oven(&s);

        // Pick a LIVING adopted Miller with a living household co-member (its future heir).
        let target = s
            .live_colonist_slots
            .iter()
            .copied()
            .find(|&slot| {
                let c = &s.colonists[slot];
                c.vocation == Vocation::Miller
                    && c.lifespan.is_some()
                    && s.society
                        .agents
                        .get(c.id)
                        .is_some_and(|a| a.stock.get(mill) > 0)
                    && c.household.is_some_and(|h| {
                        s.live_colonist_slots
                            .iter()
                            .any(|&other| other != slot && s.colonists[other].household == Some(h))
                    })
            })
            .expect("a living adopted Miller with an heir");
        let target_id = s.colonists[target].id;
        assert_eq!(s.closure_class_of(target_id), Some(ClosureClass::Miller));

        // Manufacture the estate: strip its mill(s), hand it an oven, and age it to death.
        let held_mill = s
            .society
            .agents
            .get(target_id)
            .expect("agent")
            .stock
            .get(mill);
        mirror_debit(&mut s, target_id, mill, held_mill);
        mirror_credit(&mut s, target_id, oven, 1);
        let lifespan = s.colonists[target].lifespan.expect("mortal producer");
        s.colonists[target].age = lifespan;

        let inh_before = s.burden_tool_inheritances().len();
        let mut wrong_inh = None;
        let mut wrong_adopt = None;
        for _ in 0..200 {
            s.econ_tick();
            if wrong_inh.is_none() {
                wrong_inh = s.burden_tool_inheritances()[inh_before..]
                    .iter()
                    .find(|i| i.deceased == target_id && i.tool == oven)
                    .copied();
            }
            if let Some(inh) = wrong_inh {
                wrong_adopt = s
                    .burden_role_adoptions()
                    .iter()
                    .find(|a| a.heir == inh.heir && a.role == Vocation::Baker)
                    .copied();
                if wrong_adopt.is_some() {
                    break;
                }
            }
        }
        let inh = wrong_inh.expect("the oven passed through the live estate seam");
        assert_eq!(
            inh.class,
            ClosureClass::Miller,
            "the event carries the heir's fixed class, not the tool's stage"
        );
        let adopt = wrong_adopt.expect("the heir adopted Baker at the live role-choice seam");
        assert_eq!(adopt.class, ClosureClass::Miller);
        assert_eq!(adopt.tool, oven);
        assert!(adopt.holds_tool);

        // The class-correct filters reject the whole heir history: its only inheritance is the
        // WRONG TOOL and its only producer adoption the WRONG ROLE.
        let born: BTreeSet<AgentId> = s.burden_birth_events().iter().map(|b| b.child).collect();
        assert!(born.contains(&inh.heir), "the heir is born in simulation");
        let input = BurdenCellInput {
            q: 0,
            mill_good: mill,
            oven_good: oven,
            windows: Vec::new(),
            inheritances: s
                .burden_tool_inheritances()
                .iter()
                .filter(|i| i.heir == inh.heir)
                .copied()
                .collect(),
            adoptions: s
                .burden_role_adoptions()
                .iter()
                .filter(|a| a.heir == inh.heir)
                .copied()
                .collect(),
            executions: Vec::new(),
            births: vec![BurdenBirthObs {
                tick: 0,
                child: inh.heir,
                funding: FundingBits::default(),
            }],
        };
        assert_eq!(
            succession_for_class(&input, ClosureClass::Miller, (0, BURDEN_RUN_TICKS), &born),
            Err(SuccessionBits {
                inheritance_event: true,
                adoption_event: true,
                ..Default::default()
            }),
            "a Miller-class heir inheriting/adopting the wrong stage satisfies nothing (R3-6)"
        );
    }

    /// q=1 funding records: each drawn-lot set sums exactly to q, the two streams stay
    /// complete, and a positive live Bought-funded birth joins exactly one settled trade. The
    /// child's post-birth holding proves purchase identity + taint survived the REAL conserved
    /// birth-transfer seam.
    #[test]
    fn q1_funding_records_sum_to_q_and_join_by_identity() {
        let cfg = cell_config(1, false);
        let mut s = Settlement::generate(7, &cfg);
        let mut bought_funding = None;
        for _ in 0..600 {
            s.econ_tick();
            bought_funding = s
                .burden_birth_funding_records()
                .iter()
                .find(|f| {
                    f.lots
                        .iter()
                        .any(|lot| lot.channel == BurdenChannel::Bought && lot.qty > 0)
                })
                .cloned();
            if bought_funding.is_some() {
                break;
            }
        }
        let births = s.burden_birth_events();
        let funding = s.burden_birth_funding_records();
        assert!(!funding.is_empty(), "q=1 reaches births on this base");
        assert_eq!(births.len(), funding.len());
        for f in funding {
            assert_eq!(f.q, 1);
            assert_eq!(f.lots.iter().map(|l| l.qty).sum::<u64>(), 1);
            let bits = classify_birth_funding(&f.lots, s.burden_trade_records());
            assert!(
                !bits.unverifiable,
                "every live funding lot joins cleanly: {f:?}"
            );
        }
        let bought_funding = bought_funding.expect("seed 7 q=1 produces a Bought-funded birth");
        let birth = births
            .iter()
            .find(|b| b.child == bought_funding.child)
            .expect("the independent BirthOccurred stream contains the Bought-funded child");
        assert_eq!(
            (birth.tick, birth.class, birth.parent),
            (
                bought_funding.tick,
                bought_funding.class,
                bought_funding.parent
            )
        );
        let bought_lots: Vec<&BurdenLot> = bought_funding
            .lots
            .iter()
            .filter(|lot| lot.channel == BurdenChannel::Bought && lot.qty > 0)
            .collect();
        assert!(
            bought_lots.iter().map(|lot| lot.qty).sum::<u64>() > 0,
            "the positive is non-vacuous"
        );
        let child_lots = s
            .acquisition
            .lots
            .get(&bought_funding.child)
            .expect("the newborn holds the conserved birth transfer after the live seam");
        for lot in bought_lots {
            let trade_id = lot.identity.expect("every live Bought lot has an identity");
            let joined: Vec<&BurdenTradeRecord> = s
                .burden_trade_records()
                .iter()
                .filter(|trade| trade.trade_id == trade_id)
                .collect();
            assert_eq!(joined.len(), 1, "purchase identity joins exactly one trade");
            // Deliberately NO buyer comparison here: downstream split/inheritance/birth
            // fragments join by purchase identity ONLY (R4-1) — an inherited Bought lot
            // legitimately reaches a parent who is not the original buyer. Buyer/good/quantity
            // validation lives at the purchase-credit seam.
            assert!(
                child_lots.iter().any(|child_lot| {
                    child_lot.channel == FoodChannel::Bought
                        && child_lot.qty == lot.qty
                        && child_lot.identity == lot.identity
                        && child_lot.taint == lot.taint
                }),
                "the REAL birth transfer preserves quantity, identity, and taint: {lot:?}"
            );
        }
        assert!(s.burden_seam_violations().is_empty());
        assert!(s.burden_lot_audit().is_empty());
    }

    /// R4-1 purchase-credit-seam negatives at the validation seam: wrong buyer, wrong good,
    /// wrong aggregate quantity, a missing record, and a duplicate record each hard-fail.
    #[test]
    fn purchase_credit_seam_validation_negatives() {
        let cfg = cell_config(4, false);
        let mut s = Settlement::generate(3, &cfg);
        s.econ_tick();
        let bread = s.acquisition_food_good().expect("tracked food");
        let record = BurdenTradeRecord {
            trade_id: 900,
            buyer: AgentId(5),
            good: bread,
            quantity: 3,
            earned_paid: 3,
            endowed_paid: 0,
            positive_consideration: true,
        };
        s.burden.trades.push(record);
        let base = PendingPurchaseCredit {
            trade_id: 900,
            buyer: AgentId(5),
            good: bread,
            credited: 3,
        };
        let cases: Vec<(&str, PendingPurchaseCredit, usize)> = vec![
            ("clean fact validates", base, 0),
            (
                "wrong buyer",
                PendingPurchaseCredit {
                    buyer: AgentId(6),
                    ..base
                },
                1,
            ),
            (
                "wrong good",
                PendingPurchaseCredit {
                    good: GoodId(9999),
                    ..base
                },
                1,
            ),
            (
                "wrong aggregate quantity",
                PendingPurchaseCredit {
                    credited: 2,
                    ..base
                },
                1,
            ),
            (
                "missing record",
                PendingPurchaseCredit {
                    trade_id: 901,
                    ..base
                },
                1,
            ),
        ];
        for (name, fact, want) in cases {
            s.burden.seam_violations.clear();
            s.burden.pending_purchase_credits.push(fact);
            s.burden_validate_purchase_credits();
            assert_eq!(s.burden.seam_violations.len(), want, "{name}");
        }
        // A duplicate trade record makes even the clean fact ambiguous.
        s.burden.seam_violations.clear();
        s.burden.trades.push(record);
        s.burden.pending_purchase_credits.push(base);
        s.burden_validate_purchase_credits();
        assert_eq!(s.burden.seam_violations.len(), 1, "duplicate record");
        // …and the lot audit independently rejects the duplicated trade id.
        assert!(s
            .burden_lot_audit()
            .iter()
            .any(|v| v.contains("duplicate trade id")));
    }

    /// R2-6/R3-4 lot-audit negatives: every pinned-lifecycle metadata violation — a `Bought`
    /// lot without identity, a purchase identity on a `SeededMinted` or `SelfProduced` lot,
    /// an untainted `SeededMinted` lot, a tainted `SelfProduced` lot — is a named audit entry
    /// (which the suite turns into a hard guard failure).
    #[test]
    fn lot_audit_rejects_forbidden_channel_metadata() {
        let cfg = cell_config(4, false);
        let mut s = Settlement::generate(3, &cfg);
        s.econ_tick();
        assert!(s.burden_lot_audit().is_empty(), "clean live state");
        let planted = AgentId(u64::MAX);
        let corrupt = |channel, identity, taint| FoodLot {
            channel,
            qty: 1,
            intervention: false,
            identity,
            taint,
        };
        let cases = [
            (
                corrupt(FoodChannel::Bought, None, false),
                "Bought lot without purchase identity",
            ),
            (
                corrupt(FoodChannel::SeededMinted, Some(999), true),
                "SeededMinted lot with purchase identity",
            ),
            (
                corrupt(FoodChannel::SeededMinted, None, false),
                "SeededMinted lot without construction taint",
            ),
            (
                corrupt(FoodChannel::SelfProduced, Some(999), false),
                "SelfProduced lot with purchase identity",
            ),
            (
                corrupt(FoodChannel::SelfProduced, None, true),
                "SelfProduced lot with construction taint",
            ),
        ];
        for (lot, want) in cases {
            s.acquisition.lots.insert(planted, VecDeque::from([lot]));
            let audit = s.burden_lot_audit();
            assert!(
                audit.iter().any(|v| v.contains(want)),
                "{want}: audit reported {audit:?}"
            );
            s.acquisition.lots.remove(&planted);
        }
        assert!(s.burden_lot_audit().is_empty(), "clean after removal");
    }

    /// §2/§6: the saving TARGET auto-derives from `child_food_endowment` across ALL SIX q
    /// values (q=0 is a no-op), the Off arm emits nothing, and `SufficiencyControl` is
    /// unreachable in every cell.
    #[test]
    fn saving_target_equals_q_across_all_six_burdens() {
        for q in BURDEN_QS {
            let cfg = cell_config(q, true);
            assert_ne!(
                cfg.chain.as_ref().expect("chain").birth_stock_saving_mode,
                BirthStockSavingMode::SufficiencyControl,
                "SufficiencyControl is unreachable in every cell"
            );
            let mut s = Settlement::generate(7, &cfg);
            assert!(!s.birth_stock_control_active());
            s.regenerate_scales();
            let staple = s.known.hunger;
            let mut eligible = 0usize;
            for &slot in &s.live_colonist_slots {
                let colonist = &s.colonists[slot];
                if !colonist
                    .household
                    .is_some_and(|household| s.is_producer_household(household))
                {
                    continue;
                }
                eligible += 1;
                let wants = s
                    .society
                    .agents
                    .get(colonist.id)
                    .expect("agent")
                    .scale
                    .iter()
                    .filter(|want| {
                        want.kind == WantKind::Good(staple) && matches!(want.horizon, Horizon::Next)
                    })
                    .count();
                assert_eq!(wants, q as usize, "q={q}: the saving target IS the burden");
            }
            assert!(eligible > 0);
            assert_eq!(
                s.birth_stock_wants_emitted,
                (eligible as u64) * u64::from(q)
            );

            // The Off arm emits nothing at the same burden.
            let mut off = Settlement::generate(7, &cell_config(q, false));
            off.regenerate_scales();
            assert_eq!(off.birth_stock_wants_emitted, 0, "q={q} Off arm");
        }
    }
}
