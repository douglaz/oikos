//! `NeedState` and its pure, integer per-tick dynamics.
//!
//! A colonist's physiological state is three integer deficits — `hunger`,
//! `warmth`, `rest` — where `0` means fully satisfied and higher means more
//! depleted, up to a per-need ceiling. Everything here is integer and
//! deterministic: depletion is a fixed per-tick step, replenishment comes from
//! realized consumption (FOOD → hunger, WOOD → warmth) and from taking leisure
//! (idleness → rest), and hunger held at its critical ceiling for the death
//! window is what kills (real removal, applied by the [`crate::camp::Camp`]).

/// A colonist's depletion levels. `0` = fully satisfied; higher = more depleted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NeedState {
    pub hunger: u16,
    pub warmth: u16,
    pub rest: u16,
}

impl NeedState {
    /// A freshly provisioned colonist: every need fully satisfied.
    pub const fn rested() -> Self {
        Self {
            hunger: 0,
            warmth: 0,
            rest: 0,
        }
    }

    /// Construct an explicit state (clamped to the ceiling by the caller's
    /// dynamics when advanced). Useful for property tests over the scale.
    pub const fn new(hunger: u16, warmth: u16, rest: u16) -> Self {
        Self {
            hunger,
            warmth,
            rest,
        }
    }
}

/// What a colonist actually took in over the tick just completed — the realized
/// figures the camp reads back from the econ market. Drives replenishment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NeedIntake {
    pub food_consumed: u32,
    pub wood_consumed: u32,
    pub labor_used: u32,
}

/// The integer constants governing the per-tick need dynamics. These are
/// mechanism knobs (a viable camp must keep needs bounded), not balance
/// targets — G1 asserts boundedness, never a specific level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeedDynamics {
    /// Per-need ceiling; deficits clamp here.
    pub need_max: u16,
    /// Hunger added each tick before consumption.
    pub hunger_deplete: u16,
    /// Warmth added each tick before consumption.
    pub warmth_deplete: u16,
    /// Hunger removed per FOOD unit consumed.
    pub hunger_per_food: u16,
    /// Warmth removed per WOOD unit consumed.
    pub warmth_per_wood: u16,
    /// Rest added per labor unit supplied (work tires).
    pub rest_per_labor: u16,
    /// Rest removed when the colonist supplied no labor (leisure recovers).
    pub rest_recover: u16,
    /// Hunger at or above this is "critical".
    pub hunger_critical: u16,
    /// Consecutive critical ticks that kill.
    pub death_window: u16,
}

impl NeedDynamics {
    /// Lab-default dynamics: a colonist that eats a unit of food and burns a
    /// unit of fuel each tick stays comfortably bounded; one cut off from food
    /// crosses the critical ceiling within a handful of ticks and then dies
    /// after the death window. Mechanism, not balance.
    pub const fn lab_default() -> Self {
        Self {
            need_max: 12,
            hunger_deplete: 2,
            warmth_deplete: 1,
            hunger_per_food: 3,
            warmth_per_wood: 2,
            rest_per_labor: 2,
            rest_recover: 3,
            hunger_critical: 12,
            death_window: 3,
        }
    }
}

impl Default for NeedDynamics {
    fn default() -> Self {
        Self::lab_default()
    }
}

impl NeedState {
    /// Advance one tick: apply baseline depletion, then the realized intake.
    ///
    /// - hunger/warmth always deplete by a fixed step, then drop by what was
    ///   consumed (FOOD → hunger, WOOD → warmth);
    /// - rest rises when the colonist worked (any labor supplied) and falls when
    ///   it stayed idle (took leisure);
    /// - every deficit is clamped into `0..=need_max`.
    ///
    /// Pure and integer: same `(state, dynamics, intake)` → same next state.
    pub fn advance(&mut self, dynamics: &NeedDynamics, intake: NeedIntake) {
        self.hunger = step_consumable(
            self.hunger,
            dynamics.hunger_deplete,
            intake.food_consumed,
            dynamics.hunger_per_food,
            dynamics.need_max,
        );
        self.warmth = step_consumable(
            self.warmth,
            dynamics.warmth_deplete,
            intake.wood_consumed,
            dynamics.warmth_per_wood,
            dynamics.need_max,
        );
        self.rest = if intake.labor_used > 0 {
            let tire = u32::from(dynamics.rest_per_labor).saturating_mul(intake.labor_used);
            let tired = u32::from(self.rest).saturating_add(tire);
            u16::try_from(tired.min(u32::from(dynamics.need_max))).unwrap_or(dynamics.need_max)
        } else {
            // The `.min` is load-bearing, not redundant: `NeedState::new` accepts
            // an over-ceiling `rest`, and an idle tick must clamp it back into
            // range (see `idle_rest_clamps_to_the_ceiling`). In the sim path rest
            // is always already clamped, so this is a no-op there.
            self.rest
                .saturating_sub(dynamics.rest_recover)
                .min(dynamics.need_max)
        };
    }

    /// Whether hunger is at or above the critical ceiling this tick.
    pub fn is_critical(&self, dynamics: &NeedDynamics) -> bool {
        self.hunger >= dynamics.hunger_critical
    }
}

/// Deplete by a fixed step, replenish by `consumed * per_unit`, clamp to ceiling.
fn step_consumable(level: u16, deplete: u16, consumed: u32, per_unit: u16, ceiling: u16) -> u16 {
    let replenish = u32::from(per_unit).saturating_mul(consumed);
    let raised = u32::from(level).saturating_add(u32::from(deplete));
    let lowered = raised.saturating_sub(replenish);
    u16::try_from(lowered.min(u32::from(ceiling))).unwrap_or(ceiling)
}

#[cfg(test)]
mod tests {
    use super::{NeedDynamics, NeedIntake, NeedState};

    fn intake(food: u32, wood: u32, labor: u32) -> NeedIntake {
        NeedIntake {
            food_consumed: food,
            wood_consumed: wood,
            labor_used: labor,
        }
    }

    #[test]
    fn hunger_depletes_without_food_and_recovers_with_it() {
        let dynamics = NeedDynamics::lab_default();
        let mut state = NeedState::rested();

        state.advance(&dynamics, intake(0, 0, 0));
        assert_eq!(state.hunger, dynamics.hunger_deplete);

        // From 2: one food unit (worth 3) against the 2/tick depletion → 2+2-3=1.
        state.advance(&dynamics, intake(1, 0, 0));
        assert_eq!(state.hunger, 1);

        // Two food units more than cover depletion, flooring hunger at 0.
        state.advance(&dynamics, intake(2, 0, 0));
        assert_eq!(state.hunger, 0);
    }

    #[test]
    fn deficits_clamp_at_the_ceiling() {
        let dynamics = NeedDynamics::lab_default();
        let mut state = NeedState::rested();
        for _ in 0..100 {
            state.advance(&dynamics, intake(0, 0, 0));
        }
        assert_eq!(state.hunger, dynamics.need_max);
        assert_eq!(state.warmth, dynamics.need_max);
        assert!(state.is_critical(&dynamics));
    }

    #[test]
    fn rest_rises_with_work_and_falls_with_idleness() {
        let dynamics = NeedDynamics::lab_default();
        let mut state = NeedState::rested();

        state.advance(&dynamics, intake(2, 2, 3));
        assert_eq!(state.rest, dynamics.rest_per_labor * 3);

        state.advance(&dynamics, intake(2, 2, 0));
        assert_eq!(
            state.rest,
            (dynamics.rest_per_labor * 3).saturating_sub(dynamics.rest_recover)
        );
    }

    #[test]
    fn idle_rest_clamps_to_the_ceiling() {
        let dynamics = NeedDynamics::lab_default();
        let mut state = NeedState::new(0, 0, dynamics.need_max + 10);

        state.advance(&dynamics, intake(1, 1, 0));

        assert_eq!(state.rest, dynamics.need_max);
    }

    #[test]
    fn warmth_recovers_with_fuel() {
        let dynamics = NeedDynamics::lab_default();
        let mut state = NeedState::new(6, 6, 0);
        state.advance(&dynamics, intake(0, 4, 0));
        // +1 depletion, -2*4 replenishment, clamped at 0.
        assert_eq!(state.warmth, 0);
    }

    #[test]
    fn advance_is_deterministic() {
        let dynamics = NeedDynamics::lab_default();
        let mut a = NeedState::new(4, 5, 6);
        let mut b = NeedState::new(4, 5, 6);
        let intake = intake(1, 1, 2);
        a.advance(&dynamics, intake);
        b.advance(&dynamics, intake);
        assert_eq!(a, b);
    }
}
