//! Adaptive price beliefs used only to shade posted orders.

use crate::good::Gold;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PriceBelief {
    pub expected: Gold,
    pub step: Gold,
    pub last_seen: u64,
    /// S11: whether this belief has ever been UPDATED from a market observation
    /// (a trade the agent took part in / watched, or an unfilled live quote).
    /// Distinct from `last_seen == 0`, which is ambiguous between "never observed"
    /// and "observed at tick 0" — an entrepreneurial forecast must ground itself in
    /// the belief ONLY once the agent has actually seen the good, else fall back to
    /// the public realized price (see `forecast_price_for`). Set the moment the
    /// belief first updates and never cleared.
    pub observed: bool,
}

impl PriceBelief {
    pub fn new(expected: Gold, step: Gold) -> Self {
        Self {
            expected,
            step,
            last_seen: 0,
            observed: false,
        }
    }

    pub fn observe(&mut self, price: Gold, tick: u64) {
        self.expected = move_toward(self.expected, price, self.step);
        self.last_seen = tick;
        self.observed = true;
    }

    pub fn shade_bid(self, reservation: Gold) -> Gold {
        self.expected.saturating_add(self.step).min(reservation)
    }

    pub fn shade_ask(self, reservation: Gold) -> Gold {
        self.expected.saturating_sub(self.step).max(reservation)
    }

    pub fn nudge_unfilled_bid(&mut self, reservation: Gold, tick: u64) {
        if self.expected < reservation {
            self.expected = self.expected.saturating_add(self.step).min(reservation);
        }
        self.last_seen = tick;
        self.observed = true;
    }

    pub fn nudge_unfilled_ask(&mut self, reservation: Gold, tick: u64) {
        if self.expected > reservation {
            self.expected = self.expected.saturating_sub(self.step).max(reservation);
        }
        self.last_seen = tick;
        self.observed = true;
    }
}

fn move_toward(current: Gold, target: Gold, step: Gold) -> Gold {
    if current < target {
        current.saturating_add(step).min(target)
    } else if current > target {
        current.saturating_sub(step).max(target)
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::PriceBelief;
    use crate::good::Gold;

    #[test]
    fn price_belief_shades_inside_reservation() {
        let belief = PriceBelief::new(Gold(5), Gold(2));

        assert_eq!(belief.shade_bid(Gold(6)), Gold(6));
        assert_eq!(belief.shade_bid(Gold(9)), Gold(7));
        assert_eq!(belief.shade_ask(Gold(4)), Gold(4));
        assert_eq!(belief.shade_ask(Gold(1)), Gold(3));
    }

    #[test]
    fn observed_distinguishes_never_seen_from_a_tick_zero_observation() {
        // S11: a fresh belief is NOT observed even though its `last_seen` is 0; observing
        // at tick 0 sets the flag, so the two are distinguishable (the forecast-grounding
        // contract `last_seen == 0` cannot express).
        let mut never = PriceBelief::new(Gold(2), Gold(1));
        assert!(!never.observed);
        assert_eq!(never.last_seen, 0);

        let mut at_tick_zero = PriceBelief::new(Gold(2), Gold(1));
        at_tick_zero.observe(Gold(2), 0);
        assert!(at_tick_zero.observed);
        assert_eq!(at_tick_zero.last_seen, 0);
        // Same expected/step/last_seen, different `observed`.
        assert_ne!(never, at_tick_zero);

        // An unfilled live quote also counts as an observation.
        never.nudge_unfilled_bid(Gold(5), 3);
        assert!(never.observed);
    }

    #[test]
    fn price_belief_nudge_converges_toward_reservation_without_crossing() {
        let mut bid = PriceBelief::new(Gold(1), Gold(2));
        bid.nudge_unfilled_bid(Gold(5), 1);
        bid.nudge_unfilled_bid(Gold(5), 2);
        bid.nudge_unfilled_bid(Gold(5), 3);
        assert_eq!(bid.expected, Gold(5));

        let mut ask = PriceBelief::new(Gold(9), Gold(3));
        ask.nudge_unfilled_ask(Gold(4), 1);
        ask.nudge_unfilled_ask(Gold(4), 2);
        assert_eq!(ask.expected, Gold(4));
    }
}
