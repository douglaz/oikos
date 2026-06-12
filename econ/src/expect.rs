//! Adaptive price beliefs used only to shade posted orders.

use crate::good::Gold;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PriceBelief {
    pub expected: Gold,
    pub step: Gold,
    pub last_seen: u64,
}

impl PriceBelief {
    pub fn new(expected: Gold, step: Gold) -> Self {
        Self {
            expected,
            step,
            last_seen: 0,
        }
    }

    pub fn observe(&mut self, price: Gold, tick: u64) {
        self.expected = move_toward(self.expected, price, self.step);
        self.last_seen = tick;
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
    }

    pub fn nudge_unfilled_ask(&mut self, reservation: Gold, tick: u64) {
        if self.expected > reservation {
            self.expected = self.expected.saturating_sub(self.step).max(reservation);
        }
        self.last_seen = tick;
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
