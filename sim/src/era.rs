//! G6a — the **era detector**: a read-only classification of a settlement's
//! institutional era from **measured** quantities, with hysteresis.
//!
//! This is game-spec pillar 2 — *"eras are earned, not timed"* — and the
//! lab-inherited *"phase is measured, never set"* doctrine: the era is a
//! **derived statistic**, never a state the engine sets or a timer advances. The
//! detector reads only `sim`'s existing read-only accessors (vocations, the money
//! good, the barter volume, the population) and:
//!
//! - mutates **nothing** in the settlement — [`EraDetector::observe`] takes
//!   `&Settlement`;
//! - draws **no** randomness and holds **no** `HashMap` (integer counters, a fixed
//!   array), so the era timeline is a pure function of the run;
//! - is read by **no decision path** — like econ's `metrics` module, the era is a
//!   measurement layer no behavior may import (the source-gate in
//!   `sim/tests/g6a_eras.rs` enforces it). Running a settlement with vs without an
//!   `EraDetector` observing it is byte-identical.
//!
//! ## The era ladder (measured triggers + hysteresis)
//!
//! ```text
//! Forager     — no sustained exchange (negligible barter volume)
//! Barter      — sustained reciprocal exchange (cumulative barter trade volume)
//! Money       — a money good has been promoted (current_money_good is Some)
//! Specialist  — a sustained division of labor (producer-role share ≥ a floor
//!               over a window)
//! Capital     — sustained roundabout production (a produced intermediate is itself
//!               consumed as a recipe input — both chain stages staffed — over a window)
//! ```
//!
//! The **Credit** and **Modern** eras (chartered banks, state money) are
//! **deferred to G8**: they need finance machinery that does not exist in the game
//! yet, and G6a does not invent placeholder finance to reach them.
//!
//! ### Why two trigger shapes
//!
//! Barter and Money are **institutional milestones**: once a camp has bartered a
//! sustained *volume*, or once a money good *has been* promoted, the fact does not
//! un-happen (barter even stops after promotion, freezing its cumulative count).
//! Their triggers are therefore monotonic — `barter_trade_count() ≥ a floor`, and
//! `current_money_good().is_some()`. Specialist and Capital describe an **ongoing
//! structure** — a division of labor, a roundabout chain — that can genuinely
//! collapse, so their triggers are the live producer roster (`producer-role share
//! above a floor`, `both stages are staffed`), and the hysteresis window protects
//! them from flapping on a single-tick dip while still letting a *sustained*
//! collapse regress the era.
//!
//! **Hysteresis** is the load-bearing anti-flap rule. An era is *entered* only when
//! the next rung's trigger holds for a sustained `window` of consecutive ticks, and
//! is not abandoned on a single-tick dip: the reached era only regresses when the
//! current rung's trigger **fails for a sustained window**. The ladder is climbed
//! one rung at a time; [`EraDetector::first_tick`] records the first tick each rung
//! was ever reached (never cleared by a later regression), and
//! [`EraDetector::current_era`] is the rung the settlement holds now.

use crate::settlement::{Settlement, Vocation};

/// The default hysteresis window (consecutive ticks a trigger must hold to enter an
/// era, or fail to regress from it). A small mechanism knob — not a magnitude the
/// acceptance suite asserts; the tests pin the ordered progression and the anti-flap
/// sign, never a tuned tick.
pub const DEFAULT_ERA_WINDOW: u64 = 3;

/// The default cumulative-barter-volume floor the [`Era::Barter`] trigger reads — a
/// handful of realized barter clearings, enough to be "sustained reciprocal
/// exchange" rather than a one-off swap. A mechanism knob (like the Mengerian
/// thresholds), not an asserted magnitude.
pub const DEFAULT_MIN_BARTER_VOLUME: usize = 4;

/// The default minimum live-population share, in basis points, that the
/// [`Era::Specialist`] trigger reads. `2_000` = 20% of the living settlement working
/// producer roles. A mechanism knob, not a tuned acceptance magnitude.
pub const DEFAULT_MIN_PRODUCER_SHARE_BPS: u16 = 2_000;

const BASIS_POINTS: u128 = 10_000;

/// A settlement's institutional era — an **ordered** ladder. Derived `Ord` ranks the
/// variants in declaration order (`Forager` lowest, `Capital` highest), which is
/// exactly the institutional ordering the detector climbs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Era {
    /// No sustained exchange — colonists gather/haul/consume, but negligible trade
    /// has cleared (the floor; every run starts here).
    Forager,
    /// Sustained reciprocal exchange — goods-for-goods barter has cleared a sustained
    /// cumulative volume (a thick barter book), but no money good has emerged.
    Barter,
    /// A money good has been promoted from realized barter
    /// ([`Settlement::current_money_good`] is `Some`) — the economy is money-priced.
    Money,
    /// A sustained division of labor — the live producer-role share reaches a floor
    /// over a window (colonists have adopted milling/baking from the money price spreads).
    Specialist,
    /// Sustained roundabout production — a produced **intermediate** good is itself
    /// consumed as a recipe input over a window (both chain stages are staffed: grain
    /// → flour → bread, where flour is milled *and* baked).
    Capital,
}

impl Era {
    /// Number of concrete G6a era rungs (Credit/Modern are deliberately deferred to G8).
    pub const COUNT: usize = 5;

    /// Every era, lowest rung first — the canonical order for the timeline and the
    /// `first_tick` array.
    pub const ALL: [Era; Self::COUNT] = [
        Era::Forager,
        Era::Barter,
        Era::Money,
        Era::Specialist,
        Era::Capital,
    ];

    /// The era's ladder rank (`Forager` = 0 … `Capital` = 4) — the array index for
    /// the first-tick record and the trigger array. Integer, no map.
    pub fn rank(self) -> usize {
        match self {
            Era::Forager => 0,
            Era::Barter => 1,
            Era::Money => 2,
            Era::Specialist => 3,
            Era::Capital => 4,
        }
    }

    /// The era at ladder rank `rank`, or `None` past the top rung.
    pub fn from_rank(rank: usize) -> Option<Era> {
        Era::ALL.get(rank).copied()
    }

    /// The next rung up, or `None` at the top ([`Era::Capital`]).
    pub fn next(self) -> Option<Era> {
        Era::from_rank(self.rank() + 1)
    }

    /// The rung below, or `None` at the floor ([`Era::Forager`]).
    pub fn prev(self) -> Option<Era> {
        self.rank().checked_sub(1).and_then(Era::from_rank)
    }

    /// A stable lowercase label for rendering and logs.
    pub fn label(self) -> &'static str {
        match self {
            Era::Forager => "forager",
            Era::Barter => "barter",
            Era::Money => "money",
            Era::Specialist => "specialist",
            Era::Capital => "capital",
        }
    }
}

/// The read-only era detector. It is fed one [`EraDetector::observe`] per completed
/// econ tick (right after [`Settlement::econ_tick`]); it reads the settlement's
/// measured accessors, updates its own integer hysteresis state, and reports the
/// reached era and the first tick each rung was reached.
///
/// **It mutates only itself** — the settlement is borrowed `&` — so a run observed by
/// a detector is byte-identical to one that is not. It is deterministic: integer
/// counters and a fixed array, no `HashMap`, no RNG.
#[derive(Clone, Debug)]
pub struct EraDetector {
    /// Consecutive ticks a trigger must sustain (to enter) or fail (to regress).
    window: u64,
    /// The cumulative barter-trade-volume floor the [`Era::Barter`] trigger reads.
    min_barter_volume: usize,
    /// The producer-role share floor, in basis points, the [`Era::Specialist`]
    /// trigger reads.
    min_producer_share_bps: u16,
    /// The era the settlement currently holds (the reached rung; it can regress on a
    /// sustained failure, but `first_tick` keeps the historical record).
    reached: Era,
    /// The first econ tick each era (by rank) was ever reached — never cleared by a
    /// later regression. `Forager` is stamped on the first observation.
    first_tick: [Option<u64>; Era::COUNT],
    /// Consecutive ticks the **next** rung's trigger has held (the enter streak).
    advance_streak: u64,
    /// Consecutive ticks the **current** rung's trigger has failed (the regress
    /// streak). A single-tick dip leaves this at 1 (below a window > 1), then the next
    /// holding tick resets it — so a dip never regresses the era.
    regress_streak: u64,
}

impl Default for EraDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl EraDetector {
    /// A detector with the default hysteresis window ([`DEFAULT_ERA_WINDOW`]),
    /// barter-volume floor ([`DEFAULT_MIN_BARTER_VOLUME`]), and producer-share floor
    /// ([`DEFAULT_MIN_PRODUCER_SHARE_BPS`]), reached at the [`Era::Forager`] floor.
    /// Observe it every econ tick from generation.
    pub fn new() -> Self {
        Self::with_window(DEFAULT_ERA_WINDOW)
    }

    /// A detector with an explicit hysteresis `window` (clamped to at least 1 — a zero
    /// window would enter/regress on a single tick, defeating the anti-flap rule), the
    /// default barter-volume floor, and the default producer-share floor.
    pub fn with_window(window: u64) -> Self {
        Self {
            window: window.max(1),
            min_barter_volume: DEFAULT_MIN_BARTER_VOLUME,
            min_producer_share_bps: DEFAULT_MIN_PRODUCER_SHARE_BPS,
            reached: Era::Forager,
            first_tick: [None; Era::COUNT],
            advance_streak: 0,
            regress_streak: 0,
        }
    }

    /// Set the cumulative-barter-volume floor the [`Era::Barter`] trigger reads
    /// (builder style).
    pub fn with_min_barter_volume(mut self, min_barter_volume: usize) -> Self {
        self.min_barter_volume = min_barter_volume;
        self
    }

    /// Set the producer-role share floor, in basis points, the [`Era::Specialist`]
    /// trigger reads (builder style). Values above 100% are clamped to 10_000 bps.
    pub fn with_min_producer_share_bps(mut self, min_producer_share_bps: u16) -> Self {
        self.min_producer_share_bps = min_producer_share_bps.min(BASIS_POINTS as u16);
        self
    }

    /// The hysteresis window in ticks.
    pub fn window(&self) -> u64 {
        self.window
    }

    /// The era the settlement currently holds.
    pub fn current_era(&self) -> Era {
        self.reached
    }

    /// The highest era ever reached over the run (the peak rung) — distinct from
    /// [`EraDetector::current_era`] only if the era has regressed.
    pub fn peak_era(&self) -> Era {
        Era::ALL
            .into_iter()
            .rev()
            .find(|era| self.first_tick[era.rank()].is_some())
            .unwrap_or(Era::Forager)
    }

    /// The first econ tick `era` was reached, or `None` if it never was. Monotonic —
    /// a later regression never clears it.
    pub fn first_tick(&self, era: Era) -> Option<u64> {
        self.first_tick[era.rank()]
    }

    /// The reached eras and their first ticks, lowest rung first — the era timeline
    /// (only rungs actually reached are included).
    pub fn timeline(&self) -> Vec<(Era, u64)> {
        Era::ALL
            .into_iter()
            .filter_map(|era| self.first_tick(era).map(|tick| (era, tick)))
            .collect()
    }

    /// The per-rung **measured** trigger booleans for `settlement` this tick (index =
    /// [`Era::rank`]). A pure read of the existing accessors — it measures nothing new
    /// in econ. Exposed so the source-gate test can confirm the measurement reads only
    /// read-only accessors, and the hysteresis can be driven independently.
    pub fn measured_triggers(&self, settlement: &Settlement) -> [bool; Era::COUNT] {
        let roster = producer_roster(settlement);
        [
            // Forager — the floor: always reachable.
            true,
            // Barter — sustained reciprocal exchange: a cumulative barter volume floor
            // (monotonic — barter even stops after promotion, freezing the count).
            settlement.barter_trade_count() >= self.min_barter_volume,
            // Money — a money good has been promoted (Some after promotion).
            settlement.current_money_good().is_some(),
            // Specialist — a division of labor: the live producer-role share reaches
            // the floor over the hysteresis window.
            producer_share_at_least(roster, self.min_producer_share_bps),
            // Capital — roundabout production: both chain stages are staffed, so a
            // produced intermediate (flour) is milled AND baked — a produced good
            // feeding a further production stage.
            roster.both_stages_staffed(),
        ]
    }

    /// Observe the settlement at the end of an econ tick: read its measured signals,
    /// update the hysteresis state, and advance/regress the reached era. Returns the
    /// (possibly updated) current era. **Read-only on the settlement.**
    ///
    /// Call once per completed econ tick, in tick order, from generation.
    pub fn observe(&mut self, settlement: &Settlement) -> Era {
        let tick = settlement.last_report().econ_tick;
        let triggers = self.measured_triggers(settlement);
        self.apply_triggers(tick, triggers)
    }

    /// Drive one tick of the hysteresis state machine from the per-rung trigger
    /// booleans (index = [`Era::rank`]; `triggers[0]` is the always-true `Forager`
    /// floor). [`EraDetector::observe`] computes `triggers` from the settlement and
    /// calls this; exposing the pure core lets the acceptance suite drive the
    /// anti-flap machine with controlled signals (a single-tick dip vs a sustained
    /// failure) deterministically. Returns the updated current era.
    pub fn apply_triggers(&mut self, tick: u64, triggers: [bool; Era::COUNT]) -> Era {
        // Stamp the Forager floor on the first observation.
        if self.first_tick[Era::Forager.rank()].is_none() {
            self.first_tick[Era::Forager.rank()] = Some(tick);
        }

        let next = self.reached.next();
        let next_holds = next.is_some_and(|era| triggers[era.rank()]);
        let current_holds = triggers[self.reached.rank()];

        // Enter streak: consecutive ticks the next rung's trigger has held.
        self.advance_streak = if next_holds {
            self.advance_streak + 1
        } else {
            0
        };
        // Regress streak: consecutive ticks the current rung's trigger has failed. A
        // single-tick dip leaves this at 1 (< window unless window == 1), then the next
        // holding tick resets it — so a dip never regresses the era.
        self.regress_streak = if current_holds {
            0
        } else {
            self.regress_streak + 1
        };

        // Advance is evaluated BEFORE regress, so on the tick a promotion both starts
        // the next-rung enter streak and (later) stops the current rung, the climb wins
        // — the era never regresses on the same tick it earns the next rung.
        if let Some(next_era) = next {
            if self.advance_streak >= self.window {
                self.reached = next_era;
                if self.first_tick[next_era.rank()].is_none() {
                    self.first_tick[next_era.rank()] = Some(tick);
                }
                self.advance_streak = 0;
                self.regress_streak = 0;
                return self.reached;
            }
        }
        if self.regress_streak >= self.window {
            if let Some(prev_era) = self.reached.prev() {
                self.reached = prev_era;
                self.advance_streak = 0;
                self.regress_streak = 0;
            }
        }

        self.reached
    }
}

/// The live producer roster the Specialist/Capital rungs read. Counts are collected
/// once per observation and reused by both triggers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ProducerRoster {
    living: usize,
    millers: usize,
    bakers: usize,
}

impl ProducerRoster {
    fn producer_count(self) -> usize {
        self.millers + self.bakers
    }

    fn both_stages_staffed(self) -> bool {
        self.millers > 0 && self.bakers > 0
    }
}

fn producer_roster(settlement: &Settlement) -> ProducerRoster {
    ProducerRoster {
        living: settlement.living_total(),
        millers: settlement.living_count(Vocation::Miller),
        bakers: settlement.living_count(Vocation::Baker),
    }
}

/// Whether the producer count reaches the measured live-population share floor. Uses
/// cross multiplication over integers: no floats, no rounding drift, no RNG.
fn producer_share_at_least(roster: ProducerRoster, min_bps: u16) -> bool {
    let producers = roster.producer_count();
    roster.living > 0
        && producers > 0
        && (producers as u128) * BASIS_POINTS >= (roster.living as u128) * u128::from(min_bps)
}

#[cfg(test)]
fn producer_share_bps(roster: ProducerRoster) -> u16 {
    if roster.living == 0 {
        return 0;
    }
    (((roster.producer_count() as u128) * BASIS_POINTS) / (roster.living as u128)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trigger arrays for the synthetic hysteresis tests, indexed by [`Era::rank`].
    fn triggers(barter: bool, money: bool, specialist: bool, capital: bool) -> [bool; Era::COUNT] {
        [true, barter, money, specialist, capital]
    }

    #[test]
    fn era_ladder_is_ordered() {
        assert!(Era::Forager < Era::Barter);
        assert!(Era::Barter < Era::Money);
        assert!(Era::Money < Era::Specialist);
        assert!(Era::Specialist < Era::Capital);
        for (i, era) in Era::ALL.into_iter().enumerate() {
            assert_eq!(era.rank(), i);
            assert_eq!(Era::from_rank(i), Some(era));
        }
        assert_eq!(Era::from_rank(Era::COUNT), None);
        assert_eq!(Era::Forager.prev(), None);
        assert_eq!(Era::Capital.next(), None);
        assert_eq!(Era::Money.next(), Some(Era::Specialist));
        assert_eq!(Era::Money.prev(), Some(Era::Barter));
    }

    #[test]
    fn fresh_detector_is_forager() {
        let detector = EraDetector::new();
        assert_eq!(detector.current_era(), Era::Forager);
        assert_eq!(detector.peak_era(), Era::Forager);
        assert_eq!(detector.first_tick(Era::Forager), None);
        assert!(detector.timeline().is_empty());
        assert_eq!(detector.window(), DEFAULT_ERA_WINDOW);
    }

    #[test]
    fn specialist_trigger_reads_a_producer_share() {
        let too_thin = ProducerRoster {
            living: 100,
            millers: 1,
            bakers: 0,
        };
        assert_eq!(producer_share_bps(too_thin), 100);
        assert!(!producer_share_at_least(
            too_thin,
            DEFAULT_MIN_PRODUCER_SHARE_BPS
        ));

        let enough = ProducerRoster {
            living: 10,
            millers: 1,
            bakers: 1,
        };
        assert_eq!(producer_share_bps(enough), DEFAULT_MIN_PRODUCER_SHARE_BPS);
        assert!(producer_share_at_least(
            enough,
            DEFAULT_MIN_PRODUCER_SHARE_BPS
        ));
        assert!(enough.both_stages_staffed());

        let no_population = ProducerRoster {
            living: 0,
            millers: 1,
            bakers: 1,
        };
        assert_eq!(producer_share_bps(no_population), 0);
        assert!(!producer_share_at_least(no_population, 0));
    }

    #[test]
    fn window_is_clamped_to_at_least_one() {
        assert_eq!(EraDetector::with_window(0).window(), 1);
        assert_eq!(EraDetector::with_window(5).window(), 5);
    }

    #[test]
    fn enter_requires_a_sustained_window() {
        // A next-rung trigger that holds for fewer than `window` ticks never enters.
        let mut d = EraDetector::with_window(3);
        d.apply_triggers(0, triggers(true, false, false, false));
        d.apply_triggers(1, triggers(true, false, false, false));
        // Two ticks of Barter support — one short of the window.
        assert_eq!(d.current_era(), Era::Forager);
        // The third sustained tick enters Barter.
        d.apply_triggers(2, triggers(true, false, false, false));
        assert_eq!(d.current_era(), Era::Barter);
        assert_eq!(d.first_tick(Era::Barter), Some(2));
    }

    #[test]
    fn enter_streak_resets_on_a_gap() {
        // A gap in the climbing signal resets the enter streak — entering needs a
        // *consecutive* sustained window.
        let mut d = EraDetector::with_window(3);
        d.apply_triggers(0, triggers(true, false, false, false));
        d.apply_triggers(1, triggers(true, false, false, false));
        d.apply_triggers(2, triggers(false, false, false, false)); // gap
        d.apply_triggers(3, triggers(true, false, false, false));
        d.apply_triggers(4, triggers(true, false, false, false));
        assert_eq!(d.current_era(), Era::Forager); // streak restarted at tick 3
        d.apply_triggers(5, triggers(true, false, false, false));
        assert_eq!(d.current_era(), Era::Barter); // ticks 3,4,5 sustained
    }
}
