//! The settlement test suite (moved out of `mod.rs`; same module path
//! `settlement::tests`, so every `super::*` item — public or private —
//! resolves exactly as before).

use super::*;
use econ::society::NoQuoteReason;

#[test]
fn saving_join_uses_a_pass_start_bid_even_when_the_attempt_cancels_it() {
    let member = AgentId(1);
    let seller = AgentId(2);
    let winner = AgentId(3);
    let staple = GoodId(4);
    let records = vec![
        AllocationRecord::PassStart { tick: 1 },
        AllocationRecord::BookSnapshot {
            tick: 1,
            side: OrderSide::Bid,
            agent: member,
            good: staple,
            limit: Gold(5),
            seq: 10,
        },
        AllocationRecord::BookSnapshot {
            tick: 1,
            side: OrderSide::Ask,
            agent: seller,
            good: staple,
            limit: Gold(4),
            seq: 20,
        },
        AllocationRecord::QuoteExit {
            tick: 1,
            agent: member,
            good: staple,
            side: OrderSide::Bid,
            seq: 10,
        },
        AllocationRecord::QuoteAttempt {
            tick: 1,
            order_pos: 1,
            agent: member,
            good: staple,
            side: OrderSide::Bid,
            outcome: QuoteOutcome::NoQuote(NoQuoteReason::ClampZero),
        },
        AllocationRecord::QuoteAttempt {
            tick: 1,
            order_pos: 2,
            agent: winner,
            good: staple,
            side: OrderSide::Bid,
            outcome: QuoteOutcome::Posted {
                seq: 30,
                limit: Gold(6),
                reservation: Gold(6),
            },
        },
        AllocationRecord::Execution {
            tick: 1,
            incoming_seq: 30,
            resting_seq: 20,
            incoming_side: OrderSide::Bid,
            good: staple,
            buyer: winner,
            seller,
            price: Gold(4),
            qty: 1,
            bid_limit: Gold(6),
            ask_limit: Gold(4),
            status: AllocationExecutionStatus::Succeeded,
        },
        AllocationRecord::PassEnd { tick: 1 },
    ];

    assert_eq!(
        classify_saving_opportunity(&records, &BTreeMap::new(), staple, member, &BTreeSet::new(),),
        SavingLossOutcome::CompetitiveLoss {
            basis: PriorityBasis::PostExitConsumption,
            winner_intent: WinnerIntent::Other,
        }
    );
}

#[test]
fn saving_join_matches_consumed_asks_by_sequence() {
    let member = AgentId(1);
    let seller = AgentId(2);
    let early_winner = AgentId(3);
    let later_winner = AgentId(4);
    let staple = GoodId(5);
    let records = vec![
        AllocationRecord::PassStart { tick: 1 },
        AllocationRecord::BookSnapshot {
            tick: 1,
            side: OrderSide::Ask,
            agent: seller,
            good: staple,
            limit: Gold(6),
            seq: 20,
        },
        AllocationRecord::QuoteAttempt {
            tick: 1,
            order_pos: 0,
            agent: early_winner,
            good: staple,
            side: OrderSide::Bid,
            outcome: QuoteOutcome::Posted {
                seq: 30,
                limit: Gold(7),
                reservation: Gold(7),
            },
        },
        AllocationRecord::Execution {
            tick: 1,
            incoming_seq: 30,
            resting_seq: 20,
            incoming_side: OrderSide::Bid,
            good: staple,
            buyer: early_winner,
            seller,
            price: Gold(6),
            qty: 1,
            bid_limit: Gold(7),
            ask_limit: Gold(6),
            status: AllocationExecutionStatus::Succeeded,
        },
        AllocationRecord::QuoteAttempt {
            tick: 1,
            order_pos: 1,
            agent: member,
            good: staple,
            side: OrderSide::Bid,
            outcome: QuoteOutcome::Posted {
                seq: 10,
                limit: Gold(5),
                reservation: Gold(5),
            },
        },
        AllocationRecord::QuoteAttempt {
            tick: 1,
            order_pos: 2,
            agent: seller,
            good: staple,
            side: OrderSide::Ask,
            outcome: QuoteOutcome::Posted {
                seq: 21,
                limit: Gold(4),
                reservation: Gold(4),
            },
        },
        AllocationRecord::QuoteAttempt {
            tick: 1,
            order_pos: 3,
            agent: later_winner,
            good: staple,
            side: OrderSide::Bid,
            outcome: QuoteOutcome::Posted {
                seq: 31,
                limit: Gold(7),
                reservation: Gold(7),
            },
        },
        AllocationRecord::Execution {
            tick: 1,
            incoming_seq: 31,
            resting_seq: 21,
            incoming_side: OrderSide::Bid,
            good: staple,
            buyer: later_winner,
            seller,
            price: Gold(4),
            qty: 1,
            bid_limit: Gold(7),
            ask_limit: Gold(4),
            status: AllocationExecutionStatus::Succeeded,
        },
        AllocationRecord::PassEnd { tick: 1 },
    ];

    assert_eq!(
        classify_saving_opportunity(&records, &BTreeMap::new(), staple, member, &BTreeSet::new(),),
        SavingLossOutcome::CompetitiveLoss {
            basis: PriorityBasis::HigherLimit,
            winner_intent: WinnerIntent::Other,
        }
    );
}

#[test]
fn saving_join_routes_a_lower_limit_winner_while_live_to_residual() {
    let member = AgentId(1);
    let seller = AgentId(2);
    let winner = AgentId(3);
    let staple = GoodId(6);
    let records = vec![
        AllocationRecord::PassStart { tick: 1 },
        AllocationRecord::BookSnapshot {
            tick: 1,
            side: OrderSide::Ask,
            agent: seller,
            good: staple,
            limit: Gold(4),
            seq: 20,
        },
        AllocationRecord::QuoteAttempt {
            tick: 1,
            order_pos: 0,
            agent: member,
            good: staple,
            side: OrderSide::Bid,
            outcome: QuoteOutcome::Posted {
                seq: 10,
                limit: Gold(5),
                reservation: Gold(5),
            },
        },
        AllocationRecord::QuoteAttempt {
            tick: 1,
            order_pos: 1,
            agent: winner,
            good: staple,
            side: OrderSide::Bid,
            outcome: QuoteOutcome::Posted {
                seq: 30,
                limit: Gold(4),
                reservation: Gold(4),
            },
        },
        AllocationRecord::Execution {
            tick: 1,
            incoming_seq: 30,
            resting_seq: 20,
            incoming_side: OrderSide::Bid,
            good: staple,
            buyer: winner,
            seller,
            price: Gold(4),
            qty: 1,
            bid_limit: Gold(4),
            ask_limit: Gold(4),
            status: AllocationExecutionStatus::Succeeded,
        },
        AllocationRecord::PassEnd { tick: 1 },
    ];

    assert_eq!(
        classify_saving_opportunity(&records, &BTreeMap::new(), staple, member, &BTreeSet::new(),),
        SavingLossOutcome::ExecutionResidual
    );
}

// The carried seq-10 bid (limit 7) is recognized as LIVE at the execution — that is
// why the join reaches the equal-limit branch at all, instead of reading only the
// seq-11 requote (limit 5) and reporting `AllAsksAboveLimit`. But the equal-limit
// winner arrived LATER (seq 30 > 10), so the member's carried bid held queue
// priority and should have won by arrival order; the loss therefore came from a
// settlement rejection, not arrival order, and routes to `ExecutionResidual` (never
// `EqualLimitEarlierSeq`, which would spuriously inflate the Microstructure family).
#[test]
fn saving_join_uses_the_bid_interval_live_before_a_requote() {
    let member = AgentId(1);
    let seller = AgentId(2);
    let winner = AgentId(3);
    let staple = GoodId(7);
    let records = vec![
        AllocationRecord::PassStart { tick: 1 },
        AllocationRecord::BookSnapshot {
            tick: 1,
            side: OrderSide::Bid,
            agent: member,
            good: staple,
            limit: Gold(7),
            seq: 10,
        },
        AllocationRecord::BookSnapshot {
            tick: 1,
            side: OrderSide::Ask,
            agent: seller,
            good: staple,
            limit: Gold(6),
            seq: 20,
        },
        AllocationRecord::Execution {
            tick: 1,
            incoming_seq: 30,
            resting_seq: 20,
            incoming_side: OrderSide::Bid,
            good: staple,
            buyer: winner,
            seller,
            price: Gold(6),
            qty: 1,
            bid_limit: Gold(7),
            ask_limit: Gold(6),
            status: AllocationExecutionStatus::Succeeded,
        },
        AllocationRecord::QuoteExit {
            tick: 1,
            agent: member,
            good: staple,
            side: OrderSide::Bid,
            seq: 10,
        },
        AllocationRecord::QuoteAttempt {
            tick: 1,
            order_pos: 1,
            agent: member,
            good: staple,
            side: OrderSide::Bid,
            outcome: QuoteOutcome::Posted {
                seq: 11,
                limit: Gold(5),
                reservation: Gold(5),
            },
        },
        AllocationRecord::PassEnd { tick: 1 },
    ];

    assert_eq!(
        classify_saving_opportunity(&records, &BTreeMap::new(), staple, member, &BTreeSet::new(),),
        SavingLossOutcome::ExecutionResidual
    );
}

// The coherent sibling of the later-winner case above: a genuine arrival-order
// (Microstructure) loss. An EARLIER equal-limit resting bid (the winner, seq 5) is
// crossed by an incoming ask before the member's own equal-limit bid (seq 10). The
// winner's seq is earlier than the member's, so arrival order truly decided the
// loss — the only shape under which `EqualLimitEarlierSeq` is emitted.
#[test]
fn saving_join_reports_an_equal_limit_earlier_winner_as_microstructure() {
    let member = AgentId(1);
    let seller = AgentId(2);
    let winner = AgentId(3);
    let staple = GoodId(8);
    let records = vec![
        AllocationRecord::PassStart { tick: 1 },
        AllocationRecord::BookSnapshot {
            tick: 1,
            side: OrderSide::Bid,
            agent: winner,
            good: staple,
            limit: Gold(7),
            seq: 5,
        },
        AllocationRecord::BookSnapshot {
            tick: 1,
            side: OrderSide::Bid,
            agent: member,
            good: staple,
            limit: Gold(7),
            seq: 10,
        },
        AllocationRecord::QuoteAttempt {
            tick: 1,
            order_pos: 0,
            agent: member,
            good: staple,
            side: OrderSide::Bid,
            outcome: QuoteOutcome::RestingUnchanged {
                seq: 10,
                limit: Gold(7),
                reservation: Gold(7),
            },
        },
        AllocationRecord::QuoteAttempt {
            tick: 1,
            order_pos: 1,
            agent: seller,
            good: staple,
            side: OrderSide::Ask,
            outcome: QuoteOutcome::Posted {
                seq: 20,
                limit: Gold(6),
                reservation: Gold(6),
            },
        },
        AllocationRecord::Execution {
            tick: 1,
            incoming_seq: 20,
            resting_seq: 5,
            incoming_side: OrderSide::Ask,
            good: staple,
            buyer: winner,
            seller,
            price: Gold(7),
            qty: 1,
            bid_limit: Gold(7),
            ask_limit: Gold(6),
            status: AllocationExecutionStatus::Succeeded,
        },
        AllocationRecord::PassEnd { tick: 1 },
    ];

    assert_eq!(
        classify_saving_opportunity(&records, &BTreeMap::new(), staple, member, &BTreeSet::new(),),
        SavingLossOutcome::CompetitiveLoss {
            basis: PriorityBasis::EqualLimitEarlierSeq,
            winner_intent: WinnerIntent::Other,
        }
    );
}

#[test]
fn saving_join_keeps_a_rejected_saving_cross_residual_after_later_consumption() {
    let member = AgentId(1);
    let seller = AgentId(2);
    let winner = AgentId(3);
    let staple = GoodId(8);
    let records = vec![
        AllocationRecord::PassStart { tick: 1 },
        AllocationRecord::BookSnapshot {
            tick: 1,
            side: OrderSide::Ask,
            agent: seller,
            good: staple,
            limit: Gold(4),
            seq: 20,
        },
        AllocationRecord::Execution {
            tick: 1,
            incoming_seq: 10,
            resting_seq: 20,
            incoming_side: OrderSide::Bid,
            good: staple,
            buyer: member,
            seller,
            price: Gold(4),
            qty: 1,
            bid_limit: Gold(5),
            ask_limit: Gold(4),
            status: AllocationExecutionStatus::Rejected,
        },
        AllocationRecord::QuoteAttempt {
            tick: 1,
            order_pos: 0,
            agent: member,
            good: staple,
            side: OrderSide::Bid,
            outcome: QuoteOutcome::Posted {
                seq: 10,
                limit: Gold(5),
                reservation: Gold(5),
            },
        },
        AllocationRecord::QuoteAttempt {
            tick: 1,
            order_pos: 1,
            agent: winner,
            good: staple,
            side: OrderSide::Bid,
            outcome: QuoteOutcome::Posted {
                seq: 30,
                limit: Gold(6),
                reservation: Gold(6),
            },
        },
        AllocationRecord::Execution {
            tick: 1,
            incoming_seq: 30,
            resting_seq: 20,
            incoming_side: OrderSide::Bid,
            good: staple,
            buyer: winner,
            seller,
            price: Gold(4),
            qty: 1,
            bid_limit: Gold(6),
            ask_limit: Gold(4),
            status: AllocationExecutionStatus::Succeeded,
        },
        AllocationRecord::PassEnd { tick: 1 },
    ];

    assert_eq!(
        classify_saving_opportunity(&records, &BTreeMap::new(), staple, member, &BTreeSet::new(),),
        SavingLossOutcome::ExecutionResidual
    );
}

// Table-driven coverage of `classify_saving_opportunity`'s §2 outcome space (impl-66
// repair §2): one synthetic pass per outcome, asserting the pinned precedence maps each
// shape to the right family. `member` is always `AgentId(1)`; distinct staple ids keep
// the rows independent. The intent map is populated only for the payload-selection row.
#[test]
fn classify_saving_opportunity_covers_every_outcome_branch() {
    struct Case {
        name: &'static str,
        records: Vec<AllocationRecord>,
        intent: BTreeMap<u64, TracedWant>,
        expected: SavingLossOutcome,
    }

    let member = AgentId(1);
    let seller = AgentId(2);
    let seller2 = AgentId(5);
    let winner = AgentId(3);
    let winner2 = AgentId(4);

    let cases = vec![
        // A staple bid attempt exists but no live bid results (reservation None) — no
        // bid interval → GoldReservationBind.
        Case {
            name: "NoBidPosted",
            records: vec![
                AllocationRecord::PassStart { tick: 1 },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 0,
                    agent: member,
                    good: GoodId(10),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::NoQuote(NoQuoteReason::ReservationNone),
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 1,
                    agent: seller,
                    good: GoodId(10),
                    side: OrderSide::Ask,
                    outcome: QuoteOutcome::Posted {
                        seq: 20,
                        limit: Gold(3),
                        reservation: Gold(3),
                    },
                },
                AllocationRecord::PassEnd { tick: 1 },
            ],
            intent: BTreeMap::new(),
            expected: SavingLossOutcome::NoBidPosted,
        },
        // A live bid plus one ask, but the ask is the member's own — SelfAskOnly precedes
        // NoExecutableAskInWindow.
        Case {
            name: "SelfAskOnly",
            records: vec![
                AllocationRecord::PassStart { tick: 1 },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 0,
                    agent: member,
                    good: GoodId(11),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::Posted {
                        seq: 10,
                        limit: Gold(5),
                        reservation: Gold(5),
                    },
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 1,
                    agent: member,
                    good: GoodId(11),
                    side: OrderSide::Ask,
                    outcome: QuoteOutcome::Posted {
                        seq: 11,
                        limit: Gold(4),
                        reservation: Gold(4),
                    },
                },
                AllocationRecord::PassEnd { tick: 1 },
            ],
            intent: BTreeMap::new(),
            expected: SavingLossOutcome::SelfAskOnly,
        },
        // A live bid but no ask exists anywhere in the window.
        Case {
            name: "NoExecutableAskInWindow",
            records: vec![
                AllocationRecord::PassStart { tick: 1 },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 0,
                    agent: member,
                    good: GoodId(12),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::Posted {
                        seq: 10,
                        limit: Gold(5),
                        reservation: Gold(5),
                    },
                },
                AllocationRecord::PassEnd { tick: 1 },
            ],
            intent: BTreeMap::new(),
            expected: SavingLossOutcome::NoExecutableAskInWindow,
        },
        // A non-self ask exists but its limit exceeds the saving bid's — PricedOut.
        Case {
            name: "AllAsksAboveLimit",
            records: vec![
                AllocationRecord::PassStart { tick: 1 },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 0,
                    agent: member,
                    good: GoodId(13),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::Posted {
                        seq: 10,
                        limit: Gold(5),
                        reservation: Gold(5),
                    },
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 1,
                    agent: seller,
                    good: GoodId(13),
                    side: OrderSide::Ask,
                    outcome: QuoteOutcome::Posted {
                        seq: 20,
                        limit: Gold(6),
                        reservation: Gold(6),
                    },
                },
                AllocationRecord::PassEnd { tick: 1 },
            ],
            intent: BTreeMap::new(),
            expected: SavingLossOutcome::AllAsksAboveLimit,
        },
        // The compatible ask is consumed BEFORE the saving bid enters the book (the bid's
        // QuoteAttempt is ordered after the winner's execution). The winner's price is
        // irrelevant here — arrival order alone decides → Microstructure.
        Case {
            name: "CompetitiveLoss/PreEntryOrder",
            records: vec![
                AllocationRecord::PassStart { tick: 1 },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 0,
                    agent: seller,
                    good: GoodId(14),
                    side: OrderSide::Ask,
                    outcome: QuoteOutcome::Posted {
                        seq: 20,
                        limit: Gold(4),
                        reservation: Gold(4),
                    },
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 1,
                    agent: winner,
                    good: GoodId(14),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::Posted {
                        seq: 30,
                        limit: Gold(6),
                        reservation: Gold(6),
                    },
                },
                AllocationRecord::Execution {
                    tick: 1,
                    incoming_seq: 30,
                    resting_seq: 20,
                    incoming_side: OrderSide::Bid,
                    good: GoodId(14),
                    buyer: winner,
                    seller,
                    price: Gold(4),
                    qty: 1,
                    bid_limit: Gold(6),
                    ask_limit: Gold(4),
                    status: AllocationExecutionStatus::Succeeded,
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 2,
                    agent: member,
                    good: GoodId(14),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::Posted {
                        seq: 10,
                        limit: Gold(5),
                        reservation: Gold(5),
                    },
                },
                AllocationRecord::PassEnd { tick: 1 },
            ],
            intent: BTreeMap::new(),
            expected: SavingLossOutcome::CompetitiveLoss {
                basis: PriorityBasis::PreEntryOrder,
                winner_intent: WinnerIntent::Other,
            },
        },
        // The compatible ask is consumed while the saving bid is LIVE by a strictly
        // higher-limit winner — a genuine price contest → AllocationPriority.
        Case {
            name: "CompetitiveLoss/HigherLimit",
            records: vec![
                AllocationRecord::PassStart { tick: 1 },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 0,
                    agent: member,
                    good: GoodId(15),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::Posted {
                        seq: 10,
                        limit: Gold(5),
                        reservation: Gold(5),
                    },
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 1,
                    agent: seller,
                    good: GoodId(15),
                    side: OrderSide::Ask,
                    outcome: QuoteOutcome::Posted {
                        seq: 20,
                        limit: Gold(4),
                        reservation: Gold(4),
                    },
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 2,
                    agent: winner,
                    good: GoodId(15),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::Posted {
                        seq: 30,
                        limit: Gold(7),
                        reservation: Gold(7),
                    },
                },
                AllocationRecord::Execution {
                    tick: 1,
                    incoming_seq: 30,
                    resting_seq: 20,
                    incoming_side: OrderSide::Bid,
                    good: GoodId(15),
                    buyer: winner,
                    seller,
                    price: Gold(4),
                    qty: 1,
                    bid_limit: Gold(7),
                    ask_limit: Gold(4),
                    status: AllocationExecutionStatus::Succeeded,
                },
                AllocationRecord::PassEnd { tick: 1 },
            ],
            intent: BTreeMap::new(),
            expected: SavingLossOutcome::CompetitiveLoss {
                basis: PriorityBasis::HigherLimit,
                winner_intent: WinnerIntent::Other,
            },
        },
        // Consumed while live, equal limits, an EARLIER-seq resting winner crossed first —
        // arrival order decided → Microstructure.
        Case {
            name: "CompetitiveLoss/EqualLimitEarlierSeq",
            records: vec![
                AllocationRecord::PassStart { tick: 1 },
                AllocationRecord::BookSnapshot {
                    tick: 1,
                    side: OrderSide::Bid,
                    agent: winner,
                    good: GoodId(16),
                    limit: Gold(7),
                    seq: 5,
                },
                AllocationRecord::BookSnapshot {
                    tick: 1,
                    side: OrderSide::Bid,
                    agent: member,
                    good: GoodId(16),
                    limit: Gold(7),
                    seq: 10,
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 0,
                    agent: member,
                    good: GoodId(16),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::RestingUnchanged {
                        seq: 10,
                        limit: Gold(7),
                        reservation: Gold(7),
                    },
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 1,
                    agent: seller,
                    good: GoodId(16),
                    side: OrderSide::Ask,
                    outcome: QuoteOutcome::Posted {
                        seq: 20,
                        limit: Gold(6),
                        reservation: Gold(6),
                    },
                },
                AllocationRecord::Execution {
                    tick: 1,
                    incoming_seq: 20,
                    resting_seq: 5,
                    incoming_side: OrderSide::Ask,
                    good: GoodId(16),
                    buyer: winner,
                    seller,
                    price: Gold(7),
                    qty: 1,
                    bid_limit: Gold(7),
                    ask_limit: Gold(6),
                    status: AllocationExecutionStatus::Succeeded,
                },
                AllocationRecord::PassEnd { tick: 1 },
            ],
            intent: BTreeMap::new(),
            expected: SavingLossOutcome::CompetitiveLoss {
                basis: PriorityBasis::EqualLimitEarlierSeq,
                winner_intent: WinnerIntent::Other,
            },
        },
        // The saving bid is cancelled intra-pass (a QuoteExit) and the compatible ask is
        // consumed AFTER the exit — neither pre-entry nor while-live → Residual.
        Case {
            name: "CompetitiveLoss/PostExitConsumption",
            records: vec![
                AllocationRecord::PassStart { tick: 1 },
                AllocationRecord::BookSnapshot {
                    tick: 1,
                    side: OrderSide::Bid,
                    agent: member,
                    good: GoodId(17),
                    limit: Gold(5),
                    seq: 10,
                },
                AllocationRecord::BookSnapshot {
                    tick: 1,
                    side: OrderSide::Ask,
                    agent: seller,
                    good: GoodId(17),
                    limit: Gold(4),
                    seq: 20,
                },
                AllocationRecord::QuoteExit {
                    tick: 1,
                    agent: member,
                    good: GoodId(17),
                    side: OrderSide::Bid,
                    seq: 10,
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 0,
                    agent: member,
                    good: GoodId(17),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::NoQuote(NoQuoteReason::ClampZero),
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 1,
                    agent: winner,
                    good: GoodId(17),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::Posted {
                        seq: 30,
                        limit: Gold(6),
                        reservation: Gold(6),
                    },
                },
                AllocationRecord::Execution {
                    tick: 1,
                    incoming_seq: 30,
                    resting_seq: 20,
                    incoming_side: OrderSide::Bid,
                    good: GoodId(17),
                    buyer: winner,
                    seller,
                    price: Gold(4),
                    qty: 1,
                    bid_limit: Gold(6),
                    ask_limit: Gold(4),
                    status: AllocationExecutionStatus::Succeeded,
                },
                AllocationRecord::PassEnd { tick: 1 },
            ],
            intent: BTreeMap::new(),
            expected: SavingLossOutcome::CompetitiveLoss {
                basis: PriorityBasis::PostExitConsumption,
                winner_intent: WinnerIntent::Other,
            },
        },
        // The member's own cross reached the ask and was REJECTED at settlement; a later
        // buyer consuming it does not turn that failure into a priority loss → Residual.
        Case {
            name: "ExecutionResidual/market-rejection",
            records: vec![
                AllocationRecord::PassStart { tick: 1 },
                AllocationRecord::BookSnapshot {
                    tick: 1,
                    side: OrderSide::Ask,
                    agent: seller,
                    good: GoodId(18),
                    limit: Gold(4),
                    seq: 20,
                },
                AllocationRecord::Execution {
                    tick: 1,
                    incoming_seq: 10,
                    resting_seq: 20,
                    incoming_side: OrderSide::Bid,
                    good: GoodId(18),
                    buyer: member,
                    seller,
                    price: Gold(4),
                    qty: 1,
                    bid_limit: Gold(5),
                    ask_limit: Gold(4),
                    status: AllocationExecutionStatus::Rejected,
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 0,
                    agent: member,
                    good: GoodId(18),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::Posted {
                        seq: 10,
                        limit: Gold(5),
                        reservation: Gold(5),
                    },
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 1,
                    agent: winner,
                    good: GoodId(18),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::Posted {
                        seq: 30,
                        limit: Gold(6),
                        reservation: Gold(6),
                    },
                },
                AllocationRecord::Execution {
                    tick: 1,
                    incoming_seq: 30,
                    resting_seq: 20,
                    incoming_side: OrderSide::Bid,
                    good: GoodId(18),
                    buyer: winner,
                    seller,
                    price: Gold(4),
                    qty: 1,
                    bid_limit: Gold(6),
                    ask_limit: Gold(4),
                    status: AllocationExecutionStatus::Succeeded,
                },
                AllocationRecord::PassEnd { tick: 1 },
            ],
            intent: BTreeMap::new(),
            expected: SavingLossOutcome::ExecutionResidual,
        },
        // The member never posted a staple bid QuoteAttempt at all — an unreconciled drop,
        // folded into Residual (never silently absorbed).
        Case {
            name: "ExecutionResidual/no-bid-attempt-drop",
            records: vec![
                AllocationRecord::PassStart { tick: 1 },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 0,
                    agent: seller,
                    good: GoodId(19),
                    side: OrderSide::Ask,
                    outcome: QuoteOutcome::Posted {
                        seq: 20,
                        limit: Gold(4),
                        reservation: Gold(4),
                    },
                },
                AllocationRecord::PassEnd { tick: 1 },
            ],
            intent: BTreeMap::new(),
            expected: SavingLossOutcome::ExecutionResidual,
        },
        // PAYLOAD RULE: two compatible asks are each lost to a higher-limit winner with a
        // DISTINCT winner intent. askA (limit 3, seq 20) precedes askB (limit 4, seq 21)
        // in (limit, seq) order, so the payload is askA's `SavingNext` — even though askB's
        // execution comes FIRST in record order. This pins first-(limit, seq), not
        // first-in-trace.
        Case {
            name: "CompetitiveLoss/payload-first-by-limit-seq",
            records: vec![
                AllocationRecord::PassStart { tick: 1 },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 0,
                    agent: member,
                    good: GoodId(20),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::Posted {
                        seq: 10,
                        limit: Gold(5),
                        reservation: Gold(5),
                    },
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 1,
                    agent: seller,
                    good: GoodId(20),
                    side: OrderSide::Ask,
                    outcome: QuoteOutcome::Posted {
                        seq: 20,
                        limit: Gold(3),
                        reservation: Gold(3),
                    },
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 2,
                    agent: seller2,
                    good: GoodId(20),
                    side: OrderSide::Ask,
                    outcome: QuoteOutcome::Posted {
                        seq: 21,
                        limit: Gold(4),
                        reservation: Gold(4),
                    },
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 3,
                    agent: winner2,
                    good: GoodId(20),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::Posted {
                        seq: 31,
                        limit: Gold(8),
                        reservation: Gold(8),
                    },
                },
                // askB (seq 21) consumed FIRST in record order.
                AllocationRecord::Execution {
                    tick: 1,
                    incoming_seq: 31,
                    resting_seq: 21,
                    incoming_side: OrderSide::Bid,
                    good: GoodId(20),
                    buyer: winner2,
                    seller: seller2,
                    price: Gold(4),
                    qty: 1,
                    bid_limit: Gold(8),
                    ask_limit: Gold(4),
                    status: AllocationExecutionStatus::Succeeded,
                },
                AllocationRecord::QuoteAttempt {
                    tick: 1,
                    order_pos: 4,
                    agent: winner,
                    good: GoodId(20),
                    side: OrderSide::Bid,
                    outcome: QuoteOutcome::Posted {
                        seq: 30,
                        limit: Gold(7),
                        reservation: Gold(7),
                    },
                },
                // askA (seq 20) consumed LATER in record order.
                AllocationRecord::Execution {
                    tick: 1,
                    incoming_seq: 30,
                    resting_seq: 20,
                    incoming_side: OrderSide::Bid,
                    good: GoodId(20),
                    buyer: winner,
                    seller,
                    price: Gold(3),
                    qty: 1,
                    bid_limit: Gold(7),
                    ask_limit: Gold(3),
                    status: AllocationExecutionStatus::Succeeded,
                },
                AllocationRecord::PassEnd { tick: 1 },
            ],
            intent: BTreeMap::from([
                (
                    30,
                    TracedWant {
                        kind: WantKind::Good(GoodId(20)),
                        horizon: Horizon::Next,
                    },
                ),
                (
                    31,
                    TracedWant {
                        kind: WantKind::Good(GoodId(20)),
                        horizon: Horizon::Now,
                    },
                ),
            ]),
            expected: SavingLossOutcome::CompetitiveLoss {
                basis: PriorityBasis::HigherLimit,
                winner_intent: WinnerIntent::SavingNext,
            },
        },
    ];

    for case in &cases {
        // Every row uses a distinct staple id; the first good-bearing record fixes it.
        let staple = case
            .records
            .iter()
            .find_map(|record| match record {
                AllocationRecord::QuoteAttempt { good, .. } => Some(*good),
                AllocationRecord::BookSnapshot { good, .. } => Some(*good),
                _ => None,
            })
            .expect("each case has a staple-bearing record");
        assert_eq!(
            classify_saving_opportunity(
                &case.records,
                &case.intent,
                staple,
                member,
                &BTreeSet::new(),
            ),
            case.expected,
            "case {}",
            case.name,
        );
    }
}

#[test]
fn return_window_counts_last_completed_tick() {
    let window = VecDeque::from([
        ReturnTick {
            tick: 98,
            cultivation_proceeds: 3,
            outside_proceeds: 5,
        },
        ReturnTick {
            tick: 99,
            cultivation_proceeds: 7,
            outside_proceeds: 0,
        },
        ReturnTick {
            tick: 100,
            cultivation_proceeds: 0,
            outside_proceeds: 11,
        },
    ]);

    assert_eq!(
        window_return_sums(&window, 101, 0),
        (0, 0, 0, 0),
        "a zero configured return window must remain empty"
    );
    assert_eq!(
        window_return_sums(&window, 101, 1),
        (0, 0, 11, 1),
        "a one-tick return window must include the last completed tick"
    );
    assert_eq!(
        window_return_sums(&window, 101, 2),
        (7, 1, 11, 1),
        "the inclusive cutoff must count exactly the configured completed ticks"
    );
}

#[test]
fn external_econ_canonical_tags_are_pinned() {
    assert_eq!(cycle_kind_tag(CycleKind::CreditCycle), 0);
    assert_eq!(cycle_kind_tag(CycleKind::SoundMoney), 1);

    let scenarios = [
        ScenarioName::CrusoeSurvival,
        ScenarioName::CrusoeCapital,
        ScenarioName::CrusoeAbandon,
        ScenarioName::MarketBarterishGold,
        ScenarioName::MarketPriceDiscovery,
        ScenarioName::MarketNoMutualBenefit,
        ScenarioName::TimeMarketBasic,
        ScenarioName::RoundaboutCapital,
        ScenarioName::BorrowToBuild,
        ScenarioName::SoundMoney100Pct,
        ScenarioName::CommodityCreditNeutral,
        ScenarioName::FractionalReserve,
        ScenarioName::SuspensionOfConvertibility,
        ScenarioName::FiatCreditExpansion,
        ScenarioName::FiatFiscalCantillon,
        ScenarioName::CantillonIsolation,
        ScenarioName::EmergedGoldSoundControl,
        ScenarioName::EmergedGoldFiatDisplacement,
        ScenarioName::EmergedGoldFiatRefusalControl,
        ScenarioName::EmergedGoldFiatLegalTender,
        ScenarioName::EmergedGoldFiatDebtRefusalControl,
        ScenarioName::EmergedGoldFiatDebtLegalTender,
        ScenarioName::EmergedGoldBankClaimDebtRefusalControl,
        ScenarioName::EmergedGoldBankClaimDebtLegalTender,
        ScenarioName::EmergedGoldBankClaimSpotRefusalControl,
        ScenarioName::EmergedGoldBankClaimSpotLegalTender,
        ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl,
        ScenarioName::EmergedGoldBankLoanRepaymentClaimTender,
        ScenarioName::EmergedGoldFractionalReserve,
        ScenarioName::EmergedGoldFiatCreditExpansion,
        ScenarioName::EmergedGoldFiatWageRefusalControl,
        ScenarioName::EmergedGoldFiatWageLegalTender,
        ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl,
        ScenarioName::EmergedGoldIssuerRepaymentFiatTender,
        ScenarioName::EmergedGoldReserveLeashControl,
        ScenarioName::EmergedGoldSuspensionOfConvertibility,
        ScenarioName::EmergedGoldRedemptionRun,
        ScenarioName::EmergedGoldSuspendedRedemption,
        ScenarioName::EmergedGoldTaxSpecieControl,
        ScenarioName::EmergedGoldTaxFiatUnpayableDefaults,
        ScenarioName::EmergedGoldTaxDrivesFiatLabor,
        ScenarioName::EmergedGoldNoTaxIdleControl,
        ScenarioName::MengerSaltMoney,
        ScenarioName::MengerGoldMoney,
        ScenarioName::MengerMarketabilityDurability,
        ScenarioName::MengerTwoLayerSaleability,
    ];
    for (expected, scenario) in scenarios.into_iter().enumerate() {
        assert_eq!(scenario_name_tag(scenario), expected as u8);
    }

    assert_eq!(recipe_id_tag(RecipeId::GatherFood), 0);
    assert_eq!(recipe_id_tag(RecipeId::CutWood), 1);
    assert_eq!(recipe_id_tag(RecipeId::FishWithNet), 2);
    assert_eq!(recipe_id_tag(RecipeId::Mill), 3);
    assert_eq!(recipe_id_tag(RecipeId::Bake), 4);
    assert_eq!(recipe_id_tag(RecipeId::Research), 5);
    assert_eq!(recipe_id_tag(RecipeId::Confect), 6);
    assert_eq!(recipe_id_tag(RecipeId::Cultivate), 7);
    assert_eq!(recipe_id_tag(RecipeId::CycleA), 8);
    assert_eq!(recipe_id_tag(RecipeId::CycleB), 9);
    assert_eq!(recipe_id_tag(RecipeId::CycleC), 10);

    assert_eq!(cantillon_sector_tag(CantillonSector::Capitalists), 0);
    assert_eq!(cantillon_sector_tag(CantillonSector::Households), 1);
    assert_eq!(cantillon_sector_tag(CantillonSector::Workers), 2);
    assert_eq!(cantillon_sector_tag(CantillonSector::Consumers), 3);

    assert_eq!(public_debt_tender_tag(PublicDebtTender::ParAll), 0);
    assert_eq!(public_debt_tender_tag(PublicDebtTender::SpecieOnly), 1);
    assert_eq!(public_debt_tender_tag(PublicDebtTender::FiatAndSpecie), 2);
    assert_eq!(
        public_debt_tender_tag(PublicDebtTender::BankClaimsAndSpecie),
        3
    );

    assert_eq!(bank_repayment_tender_tag(BankRepaymentTender::ParAll), 0);
    assert_eq!(
        bank_repayment_tender_tag(BankRepaymentTender::SpecieOnly),
        1
    );
    assert_eq!(
        bank_repayment_tender_tag(BankRepaymentTender::FiatAndSpecie),
        2
    );
    assert_eq!(
        bank_repayment_tender_tag(BankRepaymentTender::BankClaimsAndSpecie),
        3
    );

    assert_eq!(
        issuer_repayment_tender_tag(IssuerRepaymentTender::FiatOnly),
        0
    );
    assert_eq!(
        issuer_repayment_tender_tag(IssuerRepaymentTender::FiatRefused),
        1
    );

    assert_eq!(labor_wage_tender_tag(LaborWageTender::ParAll), 0);
    assert_eq!(labor_wage_tender_tag(LaborWageTender::SpecieOnly), 1);
    assert_eq!(labor_wage_tender_tag(LaborWageTender::FiatAndSpecie), 2);

    assert_eq!(tax_receivability_tag(TaxReceivability::SpecieOnly), 0);
    assert_eq!(tax_receivability_tag(TaxReceivability::FiatOnly), 1);
    assert_eq!(tax_receivability_tag(TaxReceivability::FiatAndSpecie), 2);

    assert_eq!(regime_tag(Regime::SoundGold), 0);
    assert_eq!(regime_tag(Regime::FractionalConvertible), 1);
    assert_eq!(regime_tag(Regime::SuspendedConvertibility), 2);
    assert_eq!(regime_tag(Regime::Fiat), 3);

    assert_eq!(public_spot_tender_tag(PublicSpotTender::ParAll), 0);
    assert_eq!(public_spot_tender_tag(PublicSpotTender::SpecieOnly), 1);
    assert_eq!(public_spot_tender_tag(PublicSpotTender::FiatAndSpecie), 2);
    assert_eq!(
        public_spot_tender_tag(PublicSpotTender::BankClaimsAndSpecie),
        3
    );

    assert_eq!(bench_surface_tag(BenchSurface::Spot), 0);
    assert_eq!(bench_surface_tag(BenchSurface::Debt), 1);
    assert_eq!(bench_surface_tag(BenchSurface::BankRepayment), 2);
    assert_eq!(bench_surface_tag(BenchSurface::IssuerRepayment), 3);

    assert_eq!(project_template_id_tag(ProjectTemplateId::BuildNet), 0);
    assert_eq!(project_template_id_tag(ProjectTemplateId::BuildRoad), 1);
    assert_eq!(project_template_id_tag(ProjectTemplateId::BuildMill), 2);
    assert_eq!(project_template_id_tag(ProjectTemplateId::BuildOven), 3);
    assert_eq!(
        project_template_id_tag(ProjectTemplateId::BuildCultivationTool),
        4
    );
}

/// The G8c-2 tender policy emits a `SetXTender` event **only** for a knob that
/// differs from econ's default — so a default policy contributes zero events
/// (keeping the G8c-1 finance bytes byte-identical), and each non-default knob
/// emits exactly its surface's event at `Tick(0)`.
#[test]
fn tender_events_emit_only_non_default_knobs() {
    // The default policy is inert: no events at all.
    assert!(TenderPolicy::default().tender_events().is_empty());

    // A single non-default knob (the wage refusal) emits exactly one wage event.
    let wage_only = TenderPolicy {
        wage: LaborWageTender::SpecieOnly,
        ..TenderPolicy::default()
    };
    let events = wage_only.tender_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].tick, Tick(0));
    assert!(matches!(
        events[0].kind,
        EventKind::SetLaborWageTender(LaborWageTender::SpecieOnly)
    ));

    // Every surface set to a non-default emits one event per surface, in the fixed
    // order spot, debt, bank-repayment, issuer-repayment, wage.
    let all = TenderPolicy {
        spot: PublicSpotTender::SpecieOnly,
        wage: LaborWageTender::FiatAndSpecie,
        debt: PublicDebtTender::SpecieOnly,
        bank_repayment: BankRepaymentTender::SpecieOnly,
        issuer_repayment: IssuerRepaymentTender::FiatRefused,
    };
    let kinds: Vec<_> = all.tender_events().into_iter().map(|e| e.kind).collect();
    assert!(matches!(kinds[0], EventKind::SetPublicSpotTender(_)));
    assert!(matches!(kinds[1], EventKind::SetPublicDebtTender(_)));
    assert!(matches!(kinds[2], EventKind::SetBankRepaymentTender(_)));
    assert!(matches!(kinds[3], EventKind::SetIssuerRepaymentTender(_)));
    assert!(matches!(kinds[4], EventKind::SetLaborWageTender(_)));
    assert_eq!(kinds.len(), 5);
}

/// The default `TenderPolicy` equals econ's per-surface defaults, so a default
/// cycle is byte-identical to the policy-free G8c-1 cycle (the finance-bytes
/// tripwire).
#[test]
fn default_tender_policy_matches_econ_defaults() {
    let default = TenderPolicy::default();
    assert_eq!(default.spot, PublicSpotTender::ParAll);
    assert_eq!(default.wage, LaborWageTender::ParAll);
    assert_eq!(default.debt, PublicDebtTender::ParAll);
    assert_eq!(default.bank_repayment, BankRepaymentTender::ParAll);
    assert_eq!(default.issuer_repayment, IssuerRepaymentTender::FiatOnly);
}

#[test]
fn medium_scale_extension_inserts_near_wants_below_survival() {
    // A scale with a present (Now) survival want and a future (Later) savings
    // want; the medium wants must land between them (survival first, then the
    // medium, then savings) and be `Horizon::Next` good wants for the medium.
    let mut scale = vec![
        Want {
            kind: WantKind::Good(FOOD),
            horizon: Horizon::Now,
            qty: 1,
            satisfied: false,
        },
        Want {
            kind: WantKind::Good(SALT),
            horizon: Horizon::Later(4),
            qty: 1,
            satisfied: false,
        },
    ];
    medium_scale_extension(&mut scale, WOOD, 2);
    assert_eq!(scale.len(), 4, "two medium wants were added");
    // Survival (the Now want) stays first.
    assert!(matches!(scale[0].horizon, Horizon::Now));
    // The two medium wants follow, before the Later savings want.
    assert_eq!(scale[1].kind, WantKind::Good(WOOD));
    assert_eq!(scale[1].horizon, Horizon::Next);
    assert_eq!(scale[2].kind, WantKind::Good(WOOD));
    assert_eq!(scale[2].horizon, Horizon::Next);
    assert!(matches!(scale[3].horizon, Horizon::Later(_)));

    // Zero qty is a no-op.
    let mut empty = scale.clone();
    let before = empty.clone();
    medium_scale_extension(&mut empty, WOOD, 0);
    assert_eq!(empty, before);
}

#[test]
fn direct_use_scale_extension_inserts_now_consumption_wants_below_survival() {
    // S9: the heterogeneous direct use is a `Horizon::Now` CONSUMPTION want (not
    // a `Horizon::Next` savings want like the medium). It lands between the
    // survival present wants and the savings ladder, exactly like the medium
    // block, but tagged `Now` so the consume arm eats it into the `consumed`
    // bucket.
    let mut scale = vec![
        Want {
            kind: WantKind::Good(FOOD),
            horizon: Horizon::Now,
            qty: 1,
            satisfied: false,
        },
        Want {
            kind: WantKind::Good(GOLD),
            horizon: Horizon::Later(4),
            qty: 1,
            satisfied: false,
        },
    ];
    direct_use_scale_extension(&mut scale, SALT, 2);
    assert_eq!(scale.len(), 4, "two direct-use wants were added");
    // Survival (the Now food want) stays first.
    assert_eq!(scale[0].kind, WantKind::Good(FOOD));
    assert!(matches!(scale[0].horizon, Horizon::Now));
    // The two SALT direct-use wants follow as single-unit `Now` consumption
    // wants, before the Later savings want.
    assert_eq!(scale[1].kind, WantKind::Good(SALT));
    assert_eq!(scale[1].horizon, Horizon::Now);
    assert_eq!(scale[1].qty, 1);
    assert_eq!(scale[2].kind, WantKind::Good(SALT));
    assert_eq!(scale[2].horizon, Horizon::Now);
    assert!(matches!(scale[3].horizon, Horizon::Later(_)));

    // Zero qty is a no-op.
    let mut empty = scale.clone();
    let before = empty.clone();
    direct_use_scale_extension(&mut empty, SALT, 0);
    assert_eq!(empty, before);
}

#[test]
fn canonical_bytes_include_salt_direct_use() {
    // S9: the heterogeneous direct-use seed steers which colonists barter for
    // SALT pre-promotion (and thus the saleability the promotion reads), so both
    // knobs are part of the determinism identity before the first tick.
    let base = SettlementConfig::frontier_coemergent();

    let mut with_qty = SettlementConfig::frontier_coemergent();
    let b = with_qty.barter.as_mut().expect("barter overlay");
    b.salt_direct_use_qty = 1;
    b.salt_direct_use_period = 8;

    let mut other_period = SettlementConfig::frontier_coemergent();
    let b = other_period.barter.as_mut().expect("barter overlay");
    b.salt_direct_use_qty = 1;
    b.salt_direct_use_period = 4;

    let base = Settlement::generate(7, &base);
    let with_qty = Settlement::generate(7, &with_qty);
    let other_period = Settlement::generate(7, &other_period);

    assert_ne!(
        base.canonical_bytes(),
        with_qty.canonical_bytes(),
        "salt_direct_use_qty/period must be part of the barter config identity"
    );
    assert_ne!(
        with_qty.canonical_bytes(),
        other_period.canonical_bytes(),
        "the heterogeneity period must be part of the barter config identity"
    );
}

#[test]
fn report_conserves_accounts_the_promotion_sink() {
    // A tick that converts 5 units of SALT to money (a promotion): the physical
    // ledger drops by exactly the promoted units, and `conserves` accepts it
    // only when the `promoted` term balances the drop.
    let mut report = EconTickReport::default();
    report.whole_system_before.insert(SALT, 5);
    report.whole_system_after.insert(SALT, 0);
    report.promoted.insert(SALT, 5);
    assert!(
        report.conserves(),
        "the promotion sink must balance the drop"
    );

    // Without the promoted term the same drop is a conservation violation.
    report.promoted.clear();
    assert!(
        !report.conserves(),
        "an unaccounted physical drop must fail conservation"
    );
}

#[test]
fn generate_places_one_world_agent_per_colonist_at_the_exchange() {
    let config = SettlementConfig::viable();
    let s = Settlement::generate(1, &config);
    assert_eq!(
        s.population(),
        usize::from(config.consumers) + usize::from(config.gatherers)
    );
    // Consumers take the lower ids, gatherers the higher.
    for index in 0..s.population() {
        let expected = if index < usize::from(config.consumers) {
            Vocation::Consumer
        } else {
            Vocation::Gatherer
        };
        assert_eq!(s.vocation_of(index), Some(expected));
        assert_eq!(s.colonist_id(index), Some(AgentId(index as u64)));
    }
    // Everyone starts on the exchange tile.
    for index in 0..s.population() {
        let id = s.colonist_id(index).unwrap();
        assert_eq!(s.world().agent_pos(id), Some(config.exchange));
    }
}

#[test]
fn tracked_goods_are_food_and_wood_only() {
    let s = Settlement::generate(1, &SettlementConfig::viable());
    assert_eq!(s.tracked_goods(), &[FOOD, WOOD]);
}

#[test]
fn resident_traders_take_the_lowest_ids_and_start_idle() {
    let config = SettlementConfig::viable().with_resident_traders(vec![TraderEndowment {
        gold: 500,
        stock: Vec::new(),
    }]);
    let s = Settlement::generate(1, &config);
    let population = usize::from(config.consumers) + usize::from(config.gatherers);

    // The trader takes id 0 (a price-setting maker, processed first) and is NOT
    // a colonist; colonists shift up to ids 1..=population.
    assert_eq!(s.population(), population, "traders are not colonists");
    assert_eq!(s.resident_trader_ids(), &[AgentId(0)]);
    assert_eq!(
        s.colonist_id(0),
        Some(AgentId(1)),
        "colonists shift up by one"
    );

    // It is a real econ agent: present in the arena with its endowment, an
    // empty (idle) scale, the Trader role, and a parked world agent at the
    // exchange (so world/econ ids stay coincident for the colonists).
    let trader = s
        .society()
        .agents
        .get(AgentId(0))
        .expect("trader resolves in the arena");
    assert_eq!(trader.gold.0, 500);
    assert!(trader.scale.is_empty(), "a fresh trader posts no orders");
    assert_eq!(trader.roles, vec![Role::Trader]);
    assert_eq!(
        s.world().agent_pos(AgentId(0)),
        Some(config.exchange),
        "a trader parks at the exchange, never tasked"
    );
}

#[test]
fn no_resident_traders_is_byte_identical_to_a_plain_settlement() {
    // The additive field must not move a trader-less settlement's digest — the
    // G2b determinism tripwire and the econ goldens depend on this.
    let plain = Settlement::generate(7, &SettlementConfig::viable());
    let explicit_empty = Settlement::generate(
        7,
        &SettlementConfig::viable().with_resident_traders(Vec::new()),
    );
    assert_eq!(plain.digest(), explicit_empty.digest());
}

#[test]
fn bank_phase_respects_tight_fiduciary_tick_cap() {
    let mut s = Settlement::generate(7, &SettlementConfig::bank());
    s.run_bank_phase();
    let borrower = s
        .live_colonist_slots
        .iter()
        .find_map(|&slot| {
            (s.colonists[slot].vocation == Vocation::Gatherer).then_some(s.colonists[slot].id)
        })
        .expect("banked settlement has a gatherer borrower");
    let depositors = s
        .live_colonist_slots
        .iter()
        .filter_map(|&slot| {
            (s.colonists[slot].vocation == Vocation::Consumer).then_some(s.colonists[slot].id)
        })
        .collect::<Vec<_>>();
    {
        let money_system = s
            .society
            .money_system
            .as_mut()
            .expect("banked settlement runs on the M3 ledger");
        for depositor in depositors {
            let claim = money_system.demand_claim_on(depositor, BANK_ID);
            if claim > Gold::ZERO {
                money_system
                    .transfer_spendable(depositor, borrower, claim)
                    .expect("test claim transfer is funded by the depositor's demand claim");
            }
        }
        money_system.reconcile_agent_cache(s.society.agents.as_mut_slice());
    }
    let before = s
        .bank()
        .expect("banked settlement charters a bank")
        .fiduciary_issued;
    s.society
        .banks
        .iter_mut()
        .find(|bank| bank.id == BANK_ID)
        .expect("banked settlement charters a bank")
        .policy
        .max_new_fiduciary_per_tick = Gold(3);

    s.run_bank_phase();

    let bank = s.bank().expect("banked settlement charters a bank");
    assert_eq!(
        bank.fiduciary_issued
            .checked_sub(before)
            .expect("fiduciary issuance is monotone"),
        Gold(3),
        "direct G8b lending must stop at the bank's per-tick fiduciary cap"
    );
}

#[test]
fn demography_provisions_report_only_credited_headroom() {
    let mut config = SettlementConfig::lineages();
    config.demography = Some(DemographyConfig {
        households: vec![crate::demography::HouseholdSpec {
            founders: 1,
            time_preference_base_bps: 500,
            food_provision: 7,
            wood_provision: 7,
            starting_gold: 0,
            starting_food: u32::MAX,
            starting_wood: u32::MAX - 1,
        }],
        birth_interval: 100,
        ..DemographyConfig::lineages()
    });
    let mut s = Settlement::generate(1, &config);
    let id = s.colonist_id(0).unwrap();
    let mut report = EconTickReport::default();

    s.deliver_demography_provisions(&mut report);

    let agent = s.society.agents.get(id).unwrap();
    assert_eq!(agent.stock.get(FOOD), u32::MAX);
    assert_eq!(agent.stock.get(WOOD), u32::MAX);
    assert_eq!(
        report.endowment_of(FOOD),
        0,
        "saturated FOOD stock must not report uncredited provision"
    );
    assert_eq!(
        report.endowment_of(WOOD),
        1,
        "only WOOD headroom should be reported as provisioned"
    );
}

#[test]
fn estate_to_heir_overflow_routes_remainder_to_commons() {
    // A death's estate that would push a living heir's stock past `u32::MAX` must
    // not silently saturate-and-drop the overflow: the heir takes only its headroom
    // and the uncreditable remainder routes to the commons, so whole-system
    // conservation holds even at the ceiling. (The saturating `Stock::add` would
    // otherwise vanish the overflow — this pins the headroom clamp.)
    let mut config = SettlementConfig::lineages();
    config.demography = Some(DemographyConfig {
        households: vec![crate::demography::HouseholdSpec {
            founders: 2,
            time_preference_base_bps: 500,
            food_provision: 0,
            wood_provision: 0,
            starting_gold: 0,
            starting_food: u32::MAX - 1,
            starting_wood: 0,
        }],
        ..DemographyConfig::lineages()
    });
    // Settle directly post-generate (no tick, no provision, no consumption), so each
    // founder holds exactly `starting_food` and the heir's headroom is a single unit.
    let mut s = Settlement::generate(1, &config);
    let deceased = s.colonist_id(0).unwrap();
    let heir = s.colonist_id(1).unwrap();
    assert_eq!(
        s.society.agents.get(heir).unwrap().stock.get(FOOD),
        u32::MAX - 1
    );

    let before = s.whole_system_total(FOOD);

    // Mirror the real caller: mark the dying member dead, then settle to heirs.
    let slot = s.slot_for_id(deceased).unwrap();
    s.mark_colonist_dead(slot);
    let mut report = EconTickReport::default();
    let mut wage_labor_used = Vec::new();
    s.settle_estate_to_heirs(deceased, &mut report, &mut wage_labor_used);

    // The heir saturates at the ceiling, the remainder (the deceased's stock minus
    // the heir's single unit of headroom) lands in the commons, and total FOOD is
    // unchanged — nothing minted, nothing lost.
    assert_eq!(
        s.society.agents.get(heir).unwrap().stock.get(FOOD),
        u32::MAX
    );
    assert_eq!(s.commons_stock_of(FOOD), u64::from(u32::MAX - 2));
    assert_eq!(
        s.whole_system_total(FOOD),
        before,
        "estate overflow to the commons must conserve total FOOD"
    );
}

#[test]
fn closure_estate_observer_records_the_actual_heir_commons_split() {
    let mut config = SettlementConfig::frontier_closed_circulation();
    config
        .demography
        .as_mut()
        .expect("closed circulation has demography")
        .households[0]
        .founders = 2;
    let mut s = Settlement::generate(1, &config);
    let household_members: Vec<AgentId> = s
        .live_colonist_slots
        .iter()
        .filter_map(|&slot| {
            (s.colonists[slot].household == Some(0)).then_some(s.colonists[slot].id)
        })
        .collect();
    let deceased = household_members[0];
    let heir = household_members[1];

    let estate_good = s
        .society
        .agents
        .get(deceased)
        .unwrap()
        .stock
        .positive_goods()
        .find(|&good| s.society.agents.get(deceased).unwrap().stock.get(good) > 1)
        .expect("the producer estate must contain a splittable physical holding");
    let heir_held = s.society.agents.get(heir).unwrap().stock.get(estate_good);
    let top_up = (u32::MAX - 1) - heir_held;
    assert!(s.society.credit_stock(heir, estate_good, top_up));
    // Mirror the manual out-of-band real top-up into the closure shadow inventory. The pre-P1-1
    // phase diff absorbed any stock change automatically; the seam-based ledger (P1-1) tracks
    // only real mutation seams, so this test injects the matching shadow credit directly to keep
    // the physical-invariant reconcile `closure_observe_estates` now asserts satisfied.
    s.closure.record(
        0,
        closure::ClosureEventKind::GatherDeposit {
            agent: heir,
            good: estate_good,
            qty: top_up,
        },
    );
    let deceased_qty = s
        .society
        .agents
        .get(deceased)
        .unwrap()
        .stock
        .get(estate_good);

    let slot = s.slot_for_id(deceased).unwrap();
    s.mark_colonist_dead(slot);
    let mut report = EconTickReport::default();
    let mut wage_labor_used = Vec::new();
    assert!(s.settle_estate_to_heirs(deceased, &mut report, &mut wage_labor_used));
    s.closure_observe_estates();

    let mut heir_qty = 0;
    let mut commons_qty = 0;
    for event in s.closure.tape.iter().filter(|event| event.tick == 0) {
        match event.kind {
            closure::ClosureEventKind::HouseholdTransfer {
                from,
                to,
                good,
                qty,
            } if from == deceased && to == heir && good == estate_good => heir_qty += qty,
            closure::ClosureEventKind::EstateToCommons { agent, good, qty }
                if agent == deceased && good == estate_good =>
            {
                commons_qty += qty
            }
            _ => {}
        }
    }
    assert_eq!(heir_qty, 1, "only the heir's real headroom is transferred");
    assert_eq!(commons_qty, deceased_qty - 1);
    assert_eq!(
        s.closure.cur.commons_goods_drain,
        u64::from(deceased_qty - 1),
        "the closure observer records the gross unplaceable remainder"
    );
}

#[test]
fn closure_estate_observer_records_failed_heir_gold_credit_as_commons_drain() {
    let mut config = SettlementConfig::frontier_closed_circulation();
    config
        .demography
        .as_mut()
        .expect("closed circulation has demography")
        .households[0]
        .founders = 2;
    let mut s = Settlement::generate(1, &config);
    let household_members: Vec<AgentId> = s
        .live_colonist_slots
        .iter()
        .filter_map(|&slot| {
            (s.colonists[slot].household == Some(0)).then_some(s.colonists[slot].id)
        })
        .collect();
    let deceased = household_members[0];
    let heir = household_members[1];
    let estate_gold = s.society.agents.get(deceased).unwrap().gold;
    assert!(
        estate_gold > Gold::ZERO,
        "the estate must exercise gold routing"
    );

    s.society.agents.get_mut(heir).unwrap().gold = Gold(u64::MAX);
    let heir_shadow = s.closure.gold.entry(heir).or_default();
    heir_shadow.earned = Gold::ZERO;
    heir_shadow.endowed = Gold(u64::MAX);
    let commons_before = s.commons_gold();

    let slot = s.slot_for_id(deceased).unwrap();
    s.mark_colonist_dead(slot);
    let mut report = EconTickReport::default();
    let mut wage_labor_used = Vec::new();
    assert!(s.settle_estate_to_heirs(deceased, &mut report, &mut wage_labor_used));
    assert_eq!(
        s.commons_gold(),
        commons_before.saturating_add(estate_gold),
        "the real estate route falls back to commons when the heir would overflow"
    );

    s.closure_observe_estates();
    assert_eq!(
        s.closure.cur.commons_drain, estate_gold.0,
        "the closure observer must record the actual gold fallback, independently of goods"
    );
}

#[test]
fn closure_estate_observer_replays_chained_estates_in_causal_order() {
    let mut config = SettlementConfig::frontier_closed_circulation();
    config
        .demography
        .as_mut()
        .expect("closed circulation has demography")
        .households[0]
        .founders = 2;
    let mut s = Settlement::generate(1, &config);
    let mut household_members: Vec<AgentId> = s
        .live_colonist_slots
        .iter()
        .filter_map(|&slot| {
            (s.colonists[slot].household == Some(0)).then_some(s.colonists[slot].id)
        })
        .collect();
    household_members.sort();
    let heir = household_members[0];
    let donor = *household_members.last().expect("household member");
    assert!(donor > heir, "the donor must sort after its heir");
    for &id in &household_members[1..household_members.len() - 1] {
        let slot = s.slot_for_id(id).expect("intermediate member slot");
        s.mark_colonist_dead(slot);
    }
    for id in [heir, donor] {
        s.society.agents.get_mut(id).expect("live member").gold = Gold(5);
        let buckets = s.closure.gold.entry(id).or_default();
        buckets.earned = Gold::ZERO;
        buckets.endowed = Gold(5);
    }
    let expected_drain = 10;

    let mut report = EconTickReport::default();
    let mut wage_labor_used = Vec::new();
    let donor_slot = s.slot_for_id(donor).expect("donor slot");
    s.mark_colonist_dead(donor_slot);
    assert!(s.settle_estate_to_heirs(donor, &mut report, &mut wage_labor_used));
    assert_eq!(s.heir_for(donor), Some(heir));

    let heir_slot = s.slot_for_id(heir).expect("heir slot");
    s.mark_colonist_dead(heir_slot);
    assert!(s.settle_estate_to_heirs(heir, &mut report, &mut wage_labor_used));
    assert_eq!(s.heir_for(heir), None);

    s.closure_observe_estates();
    assert_eq!(
        s.closure.cur.commons_drain, expected_drain,
        "the donor's gold must enter the heir's buckets before the heir's estate drains"
    );
}

#[test]
fn frontier_estate_gold_inherits_after_emergent_promotion() {
    // After G5a promotion the frontier's money balances live in `Agent.gold` even
    // though the money regime is still `Emergent(SALT)`. The public econ
    // `credit_gold` half-move correctly rejects that regime, but household
    // inheritance must still route an already-collected estate to the heir instead
    // of diverting it to the commons.
    let mut s = Settlement::generate(2_026, &SettlementConfig::frontier());

    let mut victim_slot = None;
    for tick in 0..120 {
        let report = s.econ_tick();
        assert!(report.conserves(), "frontier ledger broke at tick {tick}");
        if s.current_money_good() != Some(SALT) {
            continue;
        }
        victim_slot = s.live_colonist_slots.iter().copied().find(|&slot| {
            let colonist = &s.colonists[slot];
            let Some(household) = colonist.household else {
                return false;
            };
            let has_gold = s
                .society
                .agents
                .get(colonist.id)
                .is_some_and(|agent| agent.gold > Gold::ZERO);
            let has_heir = s
                .live_colonist_slots
                .iter()
                .any(|&other| other != slot && s.colonists[other].household == Some(household));
            has_gold && has_heir
        });
        if victim_slot.is_some() {
            break;
        }
    }

    let slot = victim_slot.expect("a promoted frontier household member holds money");
    let victim = s.colonists[slot].id;
    let household = s.colonists[slot].household.expect("household member");
    let estate_gold = s.society.agents.get(victim).expect("live victim").gold;
    assert!(
        estate_gold > Gold::ZERO,
        "the estate must exercise gold routing"
    );
    assert_eq!(
        s.current_money_good(),
        Some(SALT),
        "the test must run in the post-promotion emergent-money phase"
    );

    s.mark_colonist_dead(slot);
    let heir = s.heir_for(victim).expect("same-household heir");
    let heir_gold_before = s.society.agents.get(heir).expect("live heir").gold;
    let total_gold_before = s.total_gold();
    let commons_gold_before = s.commons_gold();

    assert!(
        !s.society.credit_gold(heir, estate_gold),
        "the external gold accessor must still reject emergent-money societies"
    );
    let mut report = EconTickReport::default();
    let mut wage_labor_used = Vec::new();
    assert!(s.settle_estate_to_heirs(victim, &mut report, &mut wage_labor_used));

    let heir_gold_after = s.society.agents.get(heir).expect("live heir").gold;
    assert_eq!(
        heir_gold_after,
        heir_gold_before
            .checked_add(estate_gold)
            .expect("small frontier estate fits"),
        "the heir must inherit the post-promotion money balance"
    );
    assert_eq!(
        s.commons_gold(),
        commons_gold_before,
        "household-routed money must not be diverted to commons"
    );
    assert_eq!(
        s.total_gold(),
        total_gold_before,
        "estate settlement must conserve total money"
    );
    assert_eq!(
        s.estate_destination_of(slot),
        Some(EstateDestination::Household { household, heir })
    );
}

#[test]
fn birth_gold_endowment_uses_only_unreserved_parent_balance() {
    let mut config = SettlementConfig::lineages();
    config.demography = Some(DemographyConfig {
        households: vec![crate::demography::HouseholdSpec {
            founders: 1,
            time_preference_base_bps: 500,
            food_provision: 0,
            wood_provision: 0,
            starting_gold: 5,
            starting_food: 8,
            starting_wood: 0,
        }],
        birth_interval: 0,
        max_household_size: 2,
        child_food_endowment: 4,
        child_gold_endowment: 5,
        ..DemographyConfig::lineages()
    });
    let mut s = Settlement::generate(1, &config);
    let parent = s.colonist_id(0).unwrap();
    let bid = econ::market::Order {
        agent: parent,
        side: econ::market::OrderSide::Bid,
        good: FOOD,
        limit: Gold(1),
        qty: 4,
        seq: 1,
        expires_tick: 10,
    };
    assert!(s
        .society
        .reservations
        .reserve_order(&s.society.agents, &bid));
    assert_eq!(s.society.reservations.reserved_gold(parent), Gold(4));

    assert_eq!(s.run_births(), 1);

    let child = s.colonist_id(1).unwrap();
    assert_eq!(
        s.society.agents.get(child).unwrap().gold,
        Gold(1),
        "the newborn gets only the parent's unreserved gold"
    );
    let parent_agent = s.society.agents.get(parent).unwrap();
    assert_eq!(parent_agent.gold, Gold(4));
    assert!(
        s.society.reservations.reserved_gold(parent) <= parent_agent.gold,
        "birth must not leave reserved gold above the parent's balance"
    );
}

#[test]
fn settle_estate_drains_a_stranded_pending_deposit_to_the_commons() {
    // A gatherer can deliver units to the exchange whose econ credit is still
    // pending when it dies. Estate settlement must drain that stranded escrow to
    // the commons (a conserved world-exchange → commons transfer) and drop the
    // attribution — never orphan the units in the exchange or leak the entry.
    // Drive the deposit phase WITHOUT the transfer to strand a pending entry,
    // then settle the depositor directly and check the drain.
    let mut s = Settlement::generate(1, &SettlementConfig::viable());

    // Accumulate a real pending deposit (deposit phase only — no transfer, so it
    // is never credited and stays attributed in `pending_deposits`).
    for _ in 0..8 {
        let fast = s.run_fast_loop();
        s.record_pending_deposits(fast.deposited);
        if !s.pending_deposits.is_empty() {
            break;
        }
    }
    let &(depositor, good) = s
        .pending_deposits
        .keys()
        .next()
        .expect("a gatherer must have a stranded pending deposit");
    let pending_qty = s.pending_deposits[&(depositor, good)];
    assert!(pending_qty > 0, "the stranded pending deposit is non-empty");

    // Mark the depositor dead (mirroring the real caller) and snapshot the
    // conserved totals + the exchange contents before settling.
    let index = s
        .colonists
        .iter()
        .position(|c| c.id == depositor)
        .expect("the depositor is a colonist");
    s.colonists[index].alive = false;
    let goods = s.goods.clone();
    let before: Vec<u64> = goods.iter().map(|&g| s.whole_system_total(g)).collect();
    let exchange_before = s.world.stockpile_get(s.exchange, good);
    let commons_before = s.commons_stock_of(good);

    let mut report = EconTickReport::default();
    let mut wage_labor_used = Vec::new();
    s.settle_estate_to_commons(depositor, &mut report, &mut wage_labor_used);

    // The attribution is gone, exactly the stranded units left the exchange for
    // the commons, and every good's whole-system total is unchanged.
    assert!(
        s.pending_deposits.keys().all(|(a, _)| *a != depositor),
        "the dead depositor's pending attribution must be drained"
    );
    assert_eq!(
        s.world.stockpile_get(s.exchange, good),
        exchange_before - pending_qty,
        "exactly the stranded pending units leave the exchange"
    );
    assert!(
        s.commons_stock_of(good) >= commons_before + u64::from(pending_qty),
        "the stranded pending units settle to the commons"
    );
    for (i, &g) in goods.iter().enumerate() {
        assert_eq!(
            s.whole_system_total(g),
            before[i],
            "estate settlement broke whole-system conservation"
        );
    }
}

#[test]
fn canonical_bytes_capture_a_nonempty_commons() {
    // The commons is omitted from the canonical bytes while empty — so a no-death
    // run matches the pre-G4a layout (the test-7 tripwire) — but joins the digest
    // once a death settles an estate, so two states that differ only in their
    // settled commons no longer collide.
    let config = SettlementConfig::viable();
    let baseline = Settlement::generate(1, &config);
    let empty_len = baseline.canonical_bytes().len();

    // An empty commons adds nothing: a clone with an untouched commons is byte-
    // identical (the inertness the no-death goldens depend on).
    let mut settled_gold = Settlement::generate(1, &config);
    assert_eq!(
        settled_gold.canonical_bytes(),
        baseline.canonical_bytes(),
        "an empty commons must not perturb the canonical bytes"
    );

    // Settling gold to the commons changes the bytes and lengthens them.
    settled_gold.commons_gold = Gold(7);
    let with_gold = settled_gold.canonical_bytes();
    assert!(
        with_gold.len() > empty_len,
        "a non-empty commons extends the digest"
    );
    assert_ne!(with_gold, baseline.canonical_bytes());

    // Two commons that differ only in their settled balance digest differently —
    // the post-death collision the digest would otherwise miss is closed.
    let mut more_gold = Settlement::generate(1, &config);
    more_gold.commons_gold = Gold(8);
    assert_ne!(
        settled_gold.digest(),
        more_gold.digest(),
        "distinct settled commons balances must not digest equal"
    );

    // Commons stock alone (a settled estate of goods, no gold) registers too.
    let mut settled_stock = Settlement::generate(1, &config);
    settled_stock.commons_stock.insert(FOOD, 3);
    assert_ne!(
        settled_stock.canonical_bytes(),
        baseline.canonical_bytes(),
        "settled commons stock must enter the canonical bytes"
    );
}

#[test]
fn canonical_bytes_include_m3_ledger_money_runtime() {
    // M3 starts with the same public money quantities as the M1 viable economy,
    // but its future stepping is ledger-backed. The canonical state must encode
    // that regime and the ledger rows, or generation-time M1/M3 twins collide.
    let m1 = Settlement::generate(7, &SettlementConfig::viable());
    let m3 = Settlement::generate(7, &SettlementConfig::m3_settlement());

    assert!(
        !m1.is_m3() && m3.is_m3(),
        "the twins must differ only by money regime"
    );
    assert_ne!(
        m1.canonical_bytes(),
        m3.canonical_bytes(),
        "M1 and M3 settlements must not serialize identically"
    );
    assert_ne!(
        m1.digest(),
        m3.digest(),
        "M1 and M3 settlements must not digest identically"
    );

    let mut expected = vec![1];
    push_money_system_bytes(
        &mut expected,
        m3.society.money_system.as_ref().expect("M3 money system"),
    );
    let bytes = m3.canonical_bytes();
    assert!(
        bytes
            .windows(expected.len())
            .any(|window| window == expected.as_slice()),
        "the M3 ledger snapshot is missing from canonical bytes"
    );
}

#[test]
#[should_panic(expected = "cannot be endowed with the money good")]
fn resident_trader_rejects_gold_stock() {
    let config = SettlementConfig::viable().with_resident_traders(vec![TraderEndowment {
        gold: 0,
        stock: vec![(GOLD, 10)],
    }]);
    let _ = Settlement::generate(1, &config);
}

#[test]
#[should_panic(expected = "cannot harvest the money good")]
fn generate_rejects_a_money_good_resource_node() {
    // GOLD is excluded from `self.goods`, so a GOLD node would be harvested
    // and deposited by the fast loop yet never transferred or conserved — a
    // silent world-side money leak. `generate` must reject it at the seam.
    let mut config = SettlementConfig::viable();
    config.nodes[0].good = GOLD;
    let _ = Settlement::generate(1, &config);
}

#[test]
#[should_panic(expected = "emergent medium cannot be GOLD")]
fn generate_rejects_gold_emergent_medium() {
    // GOLD is the money ledger, not a physical good: it never enters
    // `self.goods`, the deposit attribution, the transfer, or the conservation
    // report. A GOLD medium with a positive endowment would mint stock the
    // digest and whole-system ledger never track — `generate` rejects it at the
    // seam rather than ship a silent money leak.
    let mut config = SettlementConfig::barter_camp();
    let barter = config.barter.as_mut().expect("barter overlay");
    barter.medium_good = GOLD;
    barter.menger.candidate_goods = vec![FOOD, WOOD, GOLD];
    let _ = Settlement::generate(1, &config);
}

#[test]
#[should_panic(expected = "must define at least one resource node")]
fn generate_rejects_gatherers_without_nodes() {
    let mut config = SettlementConfig::viable();
    config.nodes.clear();
    let _ = Settlement::generate(1, &config);
}

#[test]
#[should_panic(expected = "active multigood_money requires a WOOD resource node")]
fn generate_rejects_active_multigood_without_wood_node() {
    let mut config = SettlementConfig::frontier_multigood();
    config.nodes.retain(|spec| spec.good != WOOD);
    let _ = Settlement::generate(1, &config);
}

#[test]
fn demography_provisions_the_hunger_staple_not_just_food() {
    // G5b generalizes the G4b household hearth to provision the settlement's hunger
    // staple ([`KnownGoods::hunger`]). On a `lineages` colony that is FOOD (byte-
    // identical to G4b); composed with a `bread_is_staple` chain it becomes bread,
    // so householders are endowed and provisioned in the very good they eat. The
    // composition the pre-G5b FOOD-only guard used to reject is now supported.
    let mut config = SettlementConfig::lineages();
    let mut chain = ChainConfig::grain_flour_bread();
    // No spatial producers — just the demography colony plus the chain's staple
    // mapping (bread), so the test isolates the provision good.
    chain.millers = 0;
    chain.bakers = 0;
    config.chain = Some(chain);
    let mut s = Settlement::generate(1, &config);
    let bread = s.content().expect("chain content").bread();

    // A founder starts with its staple buffer in bread, never FOOD.
    let founder = s.colonist_id(0).expect("a founder");
    let stock = &s
        .society
        .agents
        .get(founder)
        .expect("founder resolves")
        .stock;
    assert!(
        stock.get(bread) > 0,
        "the founder holds a bread staple buffer"
    );
    assert_eq!(stock.get(FOOD), 0, "FOOD is no longer the household staple");

    // The provision phase mints bread (the staple), recorded as a conserved source.
    let mut report = EconTickReport::default();
    s.deliver_demography_provisions(&mut report);
    assert!(
        report.endowment_of(bread) > 0,
        "the staple bread is provisioned"
    );
    assert_eq!(
        report.endowment_of(FOOD),
        0,
        "FOOD is no longer the provisioned staple"
    );
}

#[test]
fn barter_chain_without_bread_staple_saves_the_medium() {
    // A barter overlay composed with a chain whose bread is NOT the staple is a
    // coherent (if unshipped) camp: hunger stays FOOD, yet the emergent medium is
    // still endowed and circulated (`build_agent` always adds `medium_good` under a
    // barter overlay; the post-promotion market runs `step_rejecting_v2_*`). The
    // savings want must therefore name the medium, not the lab-default GOLD —
    // otherwise colonists would save GOLD while the market clears SALT, and
    // `run_role_choice`'s `soonest_savings_horizon(money_good)` would find no
    // matching want and never adopt a role. Guards the generation arm.
    let mut config = SettlementConfig::frontier();
    config
        .chain
        .as_mut()
        .expect("frontier ships a chain")
        .bread_is_staple = false;
    let s = Settlement::generate(7, &config);

    assert_eq!(
        s.known.savings, SALT,
        "a barter-start chain saves the emergent medium even when bread is not staple"
    );
    assert_eq!(
        s.known.hunger, FOOD,
        "with bread not the staple, hunger stays FOOD"
    );

    // The retargeted savings want is exactly what role-choice looks for: at least
    // one (patient) colonist carries a future `Good(SALT)` savings want, and no
    // colonist saves GOLD (the lab-default fallthrough the fix removes).
    let mut saw_salt_savings = false;
    for index in 0..s.population() {
        let id = s.colonist_id(index).expect("colonist id");
        let scale = &s
            .society
            .agents
            .get(id)
            .expect("living colonist resolves")
            .scale;
        for want in scale {
            if let WantKind::Good(good) = want.kind {
                assert_ne!(good, GOLD, "no colonist saves GOLD under a barter overlay");
                if good == SALT && matches!(want.horizon, Horizon::Later(_)) {
                    saw_salt_savings = true;
                }
            }
        }
    }
    assert!(
        saw_salt_savings,
        "a patient colonist carries a future SALT savings want the appraisal can target"
    );
}

#[test]
fn canonical_bytes_include_value_scale_contents() {
    let config = SettlementConfig::viable();
    let a = Settlement::generate(1, &config);
    let mut b = Settlement::generate(1, &config);

    let agent = b
        .society
        .agents
        .get_mut(AgentId(0))
        .expect("generated consumer resolves");
    assert!(
        !agent.scale.is_empty(),
        "generated agents have value scales"
    );
    agent.scale[0].qty = agent.scale[0].qty.saturating_add(1);

    assert_ne!(a.canonical_bytes(), b.canonical_bytes());
    assert_ne!(a.digest(), b.digest());
}

#[test]
fn first_econ_tick_transfers_some_food_and_conserves() {
    let config = SettlementConfig::viable().with_food_node_distance(3);
    let mut s = Settlement::generate(1, &config);
    let report = s.econ_tick();
    // A near node delivers FOOD within the first interval.
    assert!(
        report.transferred_of(FOOD) > 0,
        "no FOOD reached the market"
    );
    // No WOOD is ever hauled (it never enters the world).
    assert_eq!(report.transferred_of(WOOD), 0);
    assert_eq!(s.world().total_goods_of(WOOD), 0);
    assert!(report.conserves(), "first tick broke conservation");
}

#[test]
fn emergent_config_seeds_a_latent_pool_not_seeded_roles() {
    // G3b: the emergent config hand-places NO producer; instead it seeds a pool
    // of `Unassigned` colonists carrying a latent recipe (and the tool for it),
    // following the gatherers/consumers in id order.
    let config = SettlementConfig::emergent_chain();
    let s = Settlement::generate(1, &config);
    let content = s.content().expect("emergent config has chain content");

    let (mut latent_millers, mut latent_bakers) = (0, 0);
    for colonist in &s.colonists {
        match colonist.latent {
            Some(RecipeId::Mill) => {
                assert_eq!(colonist.vocation, Vocation::Unassigned);
                // A latent miller holds its mill (latent capital) — never seeded
                // as an active producer.
                let stock = &s.society.agents.get(colonist.id).unwrap().stock;
                assert_eq!(stock.get(content.mill()), 1, "latent miller holds a mill");
                latent_millers += 1;
            }
            Some(RecipeId::Bake) => {
                assert_eq!(colonist.vocation, Vocation::Unassigned);
                let stock = &s.society.agents.get(colonist.id).unwrap().stock;
                assert_eq!(stock.get(content.oven()), 1, "latent baker holds an oven");
                latent_bakers += 1;
            }
            Some(_) => panic!("only the chain recipes are latent specialties"),
            None => assert_ne!(
                colonist.vocation,
                Vocation::Unassigned,
                "a non-latent colonist is never Unassigned"
            ),
        }
    }
    assert!(
        latent_millers > 0 && latent_bakers > 0,
        "both latent stages seeded"
    );
    // No producer role is hand-placed at generation.
    assert_eq!(s.vocation_count(Vocation::Miller), 0);
    assert_eq!(s.vocation_count(Vocation::Baker), 0);
}

#[test]
fn canonical_bytes_include_operating_cost_and_latent() {
    // Two emergent configs differing only in the operating cost must digest
    // differently — it steers the role-choice appraisal, so it is part of the
    // settlement's future-behaviour identity (the determinism tripwire stays
    // honest for non-equivalent chain configs).
    let base = SettlementConfig::emergent_chain();
    let mut dearer = SettlementConfig::emergent_chain();
    dearer.chain.as_mut().expect("chain").operating_cost += 1;
    let base = Settlement::generate(7, &base);
    let dearer = Settlement::generate(7, &dearer);
    assert_ne!(
        base.canonical_bytes(),
        dearer.canonical_bytes(),
        "operating cost must be part of the chain config identity"
    );
}

#[test]
fn seeded_chain_digest_ignores_unused_operating_cost() {
    // A seeded G3a chain has no latent pool, so role-choice is a no-op and the
    // operating cost can never steer a future tick. Two such chains differing
    // only in it behave identically, so they must digest identically — the
    // determinism tripwire's "byte-identical iff future behaviour identical"
    // contract. (Contrast `canonical_bytes_include_operating_cost_and_latent`,
    // where a latent pool makes the same knob load-bearing.)
    let base = SettlementConfig::grain_flour_bread_chain();
    assert_eq!(
        base.chain.as_ref().expect("chain").latent_millers,
        0,
        "the seeded G3a chain must have no latent pool for this contract"
    );
    let mut dearer = SettlementConfig::grain_flour_bread_chain();
    dearer.chain.as_mut().expect("chain").operating_cost += 1;
    let base = Settlement::generate(7, &base);
    let dearer = Settlement::generate(7, &dearer);
    assert_eq!(
        base.canonical_bytes(),
        dearer.canonical_bytes(),
        "an operating cost no latent pool can read must not split the digest"
    );
}

#[test]
fn canonical_bytes_include_tool_acquisition_eligibility() {
    // S7.1: the tool-acquisition eligibility gate relaxes role-choice and adds the
    // acquired-tool scale anchor, steering future ticks for any chain — so two
    // chains differing only in it must digest apart.
    let mut off = SettlementConfig::frontier_endogenous();
    off.chain
        .as_mut()
        .expect("chain")
        .tool_acquisition_eligibility = false;
    let mut on = SettlementConfig::frontier_endogenous();
    on.chain
        .as_mut()
        .expect("chain")
        .tool_acquisition_eligibility = true;
    assert_ne!(
        Settlement::generate(7, &off).canonical_bytes(),
        Settlement::generate(7, &on).canonical_bytes(),
        "the tool-acquisition eligibility gate must be part of the chain config identity"
    );

    // The widened role-choice gate: even a SEEDED chain with no latent pool now
    // serializes the operating cost when eligibility is on (role-choice can act on
    // a tool-holder), so a chain that the latent-pool gate alone would have left
    // operating-cost-blind splits on the operating cost under eligibility.
    let mut elig = SettlementConfig::grain_flour_bread_chain();
    elig.chain
        .as_mut()
        .expect("chain")
        .tool_acquisition_eligibility = true;
    let mut elig_dearer = elig.clone();
    elig_dearer.chain.as_mut().expect("chain").operating_cost += 1;
    assert_ne!(
        Settlement::generate(7, &elig).canonical_bytes(),
        Settlement::generate(7, &elig_dearer).canonical_bytes(),
        "with eligibility on, the operating cost must steer the digest even with no latent pool"
    );

    // Tripwire: with eligibility OFF, the same seeded chain stays
    // operating-cost-blind (the pre-S7 contract) — proven by
    // `seeded_chain_digest_ignores_unused_operating_cost`, re-checked here against
    // the eligibility-on twin so the widening is the ONLY thing that flips it.
    let base = SettlementConfig::grain_flour_bread_chain();
    let mut base_dearer = base.clone();
    base_dearer.chain.as_mut().expect("chain").operating_cost += 1;
    assert_eq!(
        Settlement::generate(7, &base).canonical_bytes(),
        Settlement::generate(7, &base_dearer).canonical_bytes(),
        "with eligibility off, the seeded chain must stay operating-cost-blind"
    );
}

#[test]
fn tool_acquisition_admits_a_non_latent_tool_holder() {
    // S7.1 (the keystone): a colonist that is NOT seeded latent but is handed a
    // mill mid-run is admitted to the adoption appraisal, DOES NOT sell the mill on
    // the market step, adopts Miller, and actually produces flour. With the gate
    // OFF the same handed mill changes nothing — a non-latent colonist holding a
    // mill is not eligible, never adopts, and never mills.
    let mut on = SettlementConfig::frontier_endogenous();
    on.chain
        .as_mut()
        .expect("chain")
        .tool_acquisition_eligibility = true;
    let off = SettlementConfig::frontier_endogenous();
    let mill = on.chain.as_ref().expect("chain").content.mill();
    let flour = on.chain.as_ref().expect("chain").content.flour();

    // The first spatial, non-latent, non-producer colonist (a gatherer/consumer).
    // Deterministic, so the same index is picked across runs of the same config.
    let pick = |s: &Settlement| -> usize {
        (0..s.population())
            .find(|&i| {
                s.is_alive(i)
                    && !s.is_tool_acquisition_eligible(i)
                    && matches!(
                        s.vocation_of(i),
                        Some(Vocation::Gatherer) | Some(Vocation::Consumer)
                    )
            })
            .expect("a non-latent, non-producer spatial colonist")
    };

    // Gate ON: hand the mill once prices have formed, then run.
    let mut s = Settlement::generate(42, &on);
    s.run(400);
    let idx = pick(&s);
    let id = s.colonist_id(idx).expect("a living colonist id");
    let mill_before = s.whole_system_total(mill);
    assert!(s.society_mut().credit_stock(id, mill, 1), "mill credited");
    // It is now eligible the very next appraisal — the gate relaxation, not a relabel.
    assert!(
        s.is_tool_acquisition_eligible(idx),
        "holding the mill must make the non-latent colonist eligible"
    );
    let mut flour_made = 0u64;
    for _ in 0..200 {
        let report = s.econ_tick();
        flour_made += report.produced_of(flour);
    }
    assert_eq!(
        s.vocation_of(idx),
        Some(Vocation::Miller),
        "the eligible tool-holder must adopt Miller"
    );
    assert!(
        s.society().agents.get(id).expect("agent").stock.get(mill) >= 1,
        "the eligible tool-holder must still hold its mill (not sold before adoption)"
    );
    assert!(
        s.whole_system_total(mill) > mill_before,
        "the handed mill must remain in the whole system (the tool count did not drop)"
    );
    assert!(
        flour_made > 0,
        "the adopted tool-holder must actually produce flour, got {flour_made}"
    );

    // Gate OFF: the same handed mill at the same point changes nothing — the colonist
    // is not eligible, never adopts, and never mills.
    let mut s_off = Settlement::generate(42, &off);
    s_off.run(400);
    let off_idx = pick(&s_off);
    let off_id = s_off.colonist_id(off_idx).expect("id");
    let voc_before = s_off.vocation_of(off_idx);
    assert!(s_off.society_mut().credit_stock(off_id, mill, 1));
    assert!(
        !s_off.is_tool_acquisition_eligible(off_idx),
        "with the gate off, holding a mill must not make a non-latent colonist eligible"
    );
    s_off.run(200);
    assert_eq!(
        s_off.vocation_of(off_idx),
        voc_before,
        "with the gate off a handed mill must not turn a non-latent colonist into a producer"
    );
}

#[test]
fn tool_acquisition_waits_for_gatherer_spatial_state_to_settle() {
    // A non-latent gatherer handed a tool must finish any world-side haul before
    // switching to Miller/Baker. Otherwise its later deposit would not be attributed
    // by the fast loop, because deposits are tracked only for current gatherers.
    let mut cfg = SettlementConfig::frontier_endogenous();
    cfg.chain
        .as_mut()
        .expect("chain")
        .tool_acquisition_eligibility = true;
    let mill = cfg.chain.as_ref().expect("chain").content.mill();

    let mut s = Settlement::generate(42, &cfg);
    s.run(400);
    let idx = (0..s.population())
        .find(|&i| {
            s.is_alive(i)
                && !s.is_tool_acquisition_eligible(i)
                && s.vocation_of(i) == Some(Vocation::Gatherer)
                && s.node_of(i).is_some()
        })
        .expect("a non-latent gatherer");
    let id = s.colonist_id(idx).expect("id");
    let node = s.node_of(idx).expect("gatherer node");
    let carried_good = s.world().node(node).expect("node").good;

    assert!(s.world.assign_task(id, Task::GoHarvest(node, s.carry_cap)));
    for _ in 0..64 {
        s.world.tick();
        if s.world.agent_carry_total(id) > 0 {
            break;
        }
    }
    assert!(
        s.world.agent_carry_total(id) > 0,
        "test setup must put a real harvested load in carry"
    );
    assert!(s.world.assign_task(id, Task::GoTo(Pos::new(48, 0))));
    for _ in 0..64 {
        s.world.tick();
        if s.world.agent_pos(id) == Some(Pos::new(48, 0)) {
            break;
        }
    }
    assert_eq!(
        s.world.agent_pos(id),
        Some(Pos::new(48, 0)),
        "test setup must park the loaded gatherer far from the exchange"
    );
    assert!(s.world.assign_task(id, Task::GoDeposit(s.exchange)));
    assert_eq!(s.world.agent_status(id), Some(AgentStatus::Moving));
    assert!(s.society_mut().credit_stock(id, mill, 1));

    let mut saw_unsettled_wait = false;
    let mut adopted = false;
    for tick in 0..200u64 {
        let report = s.econ_tick();
        assert!(report.conserves(), "tick {tick} must conserve");
        assert_eq!(
            s.world().stockpile_get(s.exchange(), carried_good),
            0,
            "each deposit must be attributed and transferred at tick {tick}"
        );
        let unsettled = s.world().agent_status(id) != Some(AgentStatus::Idle)
            || s.world().agent_carry_total(id) > 0;
        if unsettled {
            saw_unsettled_wait = true;
            assert_eq!(
                s.vocation_of(idx),
                Some(Vocation::Gatherer),
                "a gatherer must not adopt while its haul is unsettled at tick {tick}"
            );
        }
        if s.vocation_of(idx) == Some(Vocation::Miller) {
            adopted = true;
            break;
        }
    }
    assert!(
        saw_unsettled_wait,
        "the regression must exercise a tool-holder with unsettled spatial state"
    );
    assert!(
        adopted,
        "the settled tool-holder must still adopt once the appraisal pays"
    );
}

#[test]
fn reentry_does_not_revert_an_adopted_tool_holder() {
    // A non-latent spatial colonist that adopts from a held tool keeps its producer
    // role through the later same-tick re-entry phase. Its home role remains spatial,
    // but active tool producers are outside the S6 re-entry path.
    let mut cfg = SettlementConfig::frontier_endogenous_scaling();
    {
        let chain = cfg.chain.as_mut().expect("chain");
        chain.tool_acquisition_eligibility = true;
        chain.producible_capital = false;
    }
    let mill = cfg.chain.as_ref().expect("chain").content.mill();
    let mut s = Settlement::generate(42, &cfg);
    s.run(400);
    let idx = (0..s.population())
        .find(|&i| {
            s.is_alive(i)
                && !s.is_tool_acquisition_eligible(i)
                && matches!(
                    s.vocation_of(i),
                    Some(Vocation::Gatherer) | Some(Vocation::Consumer)
                )
        })
        .expect("a non-latent spatial colonist");
    let id = s.colonist_id(idx).expect("id");
    assert!(s.society_mut().credit_stock(id, mill, 1));

    let slot = s.slot_for_id(id).expect("slot");
    let mut adopted = false;
    for _ in 0..200 {
        s.colonists[slot].need.hunger = 0;
        s.econ_tick();
        if s.vocation_of(idx) == Some(Vocation::Miller) {
            adopted = true;
            break;
        }
    }
    assert!(
        adopted,
        "re-entry must not revert an active tool-holder back to its spatial home role"
    );
}

#[test]
fn tool_acquisition_off_is_byte_identical() {
    // S7.1 inertness: with the gate OFF, flipping the (unused) eligibility flag in
    // isolation is a no-op — the gate never fires without a non-latent tool-holder,
    // and generation is untouched, so a fresh run is byte-identical. (The autonomous
    // path that would create such a holder is S7.2, gated separately.)
    let off = SettlementConfig::frontier_endogenous();
    let mut a = Settlement::generate(0xC0FFEE, &off);
    let mut b = Settlement::generate(0xC0FFEE, &off);
    a.run(600);
    b.run(600);
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    assert_eq!(a.digest(), b.digest());
}

/// A capital economy for the S7.2 mechanism tests: the scaling economy with both S7
/// gates on and a larger colony, so bread demand genuinely outruns the seeded chain
/// and the per-builder phase has real demand to respond to. Self-contained (does not
/// depend on the S7.3 `frontier_capital` scenario).
fn capital_test_config() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_endogenous_scaling();
    {
        let c = cfg.chain.as_mut().expect("chain");
        c.tool_acquisition_eligibility = true;
        c.producible_capital = true;
        c.capital_payback_cycles = 16;
        c.tool_build_wood = 6;
        c.tool_build_labor = 4;
        c.capital_build_hunger_max = 4;
    }
    cfg.consumers = 44;
    cfg.gatherers = 24;
    cfg
}

#[test]
fn canonical_bytes_include_producible_capital() {
    // S7.2: producible_capital and its appraisal knobs steer future ticks (whether
    // and when a tool is built), so two chains differing only in one must digest
    // apart — and with the phase OFF the unused knobs must NOT split the digest.
    let mut off = SettlementConfig::frontier_endogenous();
    off.chain
        .as_mut()
        .expect("chain")
        .tool_acquisition_eligibility = true;
    let mut on = off.clone();
    on.chain.as_mut().expect("chain").producible_capital = true;
    let off_bytes = Settlement::generate(7, &off).canonical_bytes();
    assert_ne!(
        off_bytes,
        Settlement::generate(7, &on).canonical_bytes(),
        "the producible-capital phase gate must be part of the chain config identity"
    );

    // Phase ON: each appraisal knob must split the digest.
    for mutate in [
        (|c: &mut ChainConfig| c.capital_payback_cycles += 1) as fn(&mut ChainConfig),
        |c: &mut ChainConfig| c.tool_build_wood += 1,
        |c: &mut ChainConfig| c.tool_build_labor += 1,
        |c: &mut ChainConfig| c.capital_build_hunger_max += 1,
    ] {
        let mut tweaked = on.clone();
        mutate(tweaked.chain.as_mut().expect("chain"));
        assert_ne!(
            Settlement::generate(7, &on).canonical_bytes(),
            Settlement::generate(7, &tweaked).canonical_bytes(),
            "with producible capital on, every appraisal knob must steer the digest"
        );
    }

    // Phase OFF: the same (unused) knobs must NOT split the digest, or the tripwire
    // would call two behaviour-identical configs unequal.
    let mut off_tweaked = off.clone();
    {
        let c = off_tweaked.chain.as_mut().expect("chain");
        c.capital_payback_cycles += 5;
        c.tool_build_wood += 5;
        c.tool_build_labor += 5;
        c.capital_build_hunger_max += 5;
    }
    assert_eq!(
        off_bytes,
        Settlement::generate(7, &off_tweaked).canonical_bytes(),
        "with producible capital off, the unused build knobs must not steer the digest"
    );

    let id_a = Settlement::generate(7, &on);
    let mut id_b = Settlement::generate(7, &on);
    id_b.next_capital_project_id = id_b.next_capital_project_id.saturating_add(1);
    assert_ne!(
        id_a.canonical_bytes(),
        id_b.canonical_bytes(),
        "the next capital project id steers future project ids and must be serialized"
    );

    let build_cfg = capital_test_config();
    let mut build_state = Settlement::generate(7, &build_cfg);
    for _ in 0..900 {
        build_state.econ_tick();
        if !build_state.capital_builds.is_empty() {
            break;
        }
    }
    assert!(
        !build_state.capital_builds.is_empty(),
        "the capital config should produce an in-flight build for the digest check"
    );
    let before = build_state.canonical_bytes();
    build_state.capital_builds[0].project.id =
        ProjectId(build_state.capital_builds[0].project.id.0.saturating_add(1));
    assert_ne!(
        before,
        build_state.canonical_bytes(),
        "an in-flight capital project's id must be serialized"
    );
}

#[test]
fn canonical_bytes_include_ignition_withdrawal() {
    // C3R.e (impl-67): tag 33 is ONE fixed injective record (a presence-bit byte + present
    // fields in fixed order). With every knob off it emits nothing (byte-identical to the
    // base); each knob splits the digest; and the presence byte keeps the record injective —
    // the same numeric value under a DIFFERENT knob must digest apart (no partition collision).
    let base = SettlementConfig::frontier_mortal_producers_earned();
    let base_bytes = Settlement::generate(7, &base).canonical_bytes();

    // Off is byte-identical: the C3R.e knobs are all off by default, so an explicit all-off
    // clone emits no tag-33 byte and digests exactly as the base.
    let mut all_off = base.clone();
    {
        let c = all_off.chain.as_mut().expect("chain");
        c.birth_stock_ignition_at = None;
        c.producer_house_starting_staple = 0;
        c.producer_support_until_tick = None;
    }
    assert_eq!(
        base_bytes,
        Settlement::generate(7, &all_off).canonical_bytes(),
        "with every C3R.e knob off, tag 33 emits nothing — the base stays byte-identical"
    );

    // Each knob splits the digest (it steers deterministic behavior at its gated tick).
    for mutate in [
        (|c: &mut ChainConfig| c.birth_stock_ignition_at = Some(50)) as fn(&mut ChainConfig),
        |c: &mut ChainConfig| c.producer_house_starting_staple = 4,
        |c: &mut ChainConfig| c.producer_support_until_tick = Some(400),
    ] {
        let mut on = base.clone();
        mutate(on.chain.as_mut().expect("chain"));
        assert_ne!(
            base_bytes,
            Settlement::generate(7, &on).canonical_bytes(),
            "each C3R.e knob must be part of the chain-config identity"
        );
    }

    // Injectivity via the presence byte: the SAME numeric value under two DIFFERENT knobs must
    // digest apart — a partition of the value bytes alone would collide these.
    let mut ignite_50 = base.clone();
    ignite_50
        .chain
        .as_mut()
        .expect("chain")
        .birth_stock_ignition_at = Some(50);
    let mut support_50 = base.clone();
    support_50
        .chain
        .as_mut()
        .expect("chain")
        .producer_support_until_tick = Some(50);
    assert_ne!(
        Settlement::generate(7, &ignite_50).canonical_bytes(),
        Settlement::generate(7, &support_50).canonical_bytes(),
        "the tag-33 presence byte must keep the record injective across knobs"
    );
}

#[test]
fn canonical_bytes_include_per_agent_capital() {
    // S10: the per_agent_capital flag steers every future tick (it replaces the S7
    // build planner with a per-colonist ordinal decision), so it is part of the chain
    // config identity. And in per-agent mode capital_payback_cycles is behaviour-INERT
    // — two per-agent configs differing only in it must NOT digest apart (no false
    // split for a behaviour-inert knob).
    let mut heuristic = capital_test_config();
    heuristic.chain.as_mut().expect("chain").per_agent_capital = false;
    let mut per_agent = heuristic.clone();
    per_agent.chain.as_mut().expect("chain").per_agent_capital = true;

    // per-agent ON vs the S7 heuristic must digest apart (the gate is in the identity).
    assert_ne!(
        Settlement::generate(7, &heuristic).canonical_bytes(),
        Settlement::generate(7, &per_agent).canonical_bytes(),
        "per_agent_capital must be part of the chain config identity"
    );

    // In per-agent mode capital_payback_cycles is inert — including across a live run
    // (the per-agent decision never reads it) — so the digest must not split on it.
    let mut per_agent_other = per_agent.clone();
    per_agent_other
        .chain
        .as_mut()
        .expect("chain")
        .capital_payback_cycles += 9;
    let mut a = Settlement::generate(7, &per_agent);
    let mut b = Settlement::generate(7, &per_agent_other);
    a.run(120);
    b.run(120);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "capital_payback_cycles is inert in per-agent mode and must not split the digest"
    );
    assert_eq!(a.digest(), b.digest());

    // The still-active build knobs DO steer per-agent builds, so each must split.
    for mutate in [
        (|c: &mut ChainConfig| c.tool_build_wood += 1) as fn(&mut ChainConfig),
        |c: &mut ChainConfig| c.tool_build_labor += 1,
        |c: &mut ChainConfig| c.capital_build_hunger_max += 1,
    ] {
        let mut tweaked = per_agent.clone();
        mutate(tweaked.chain.as_mut().expect("chain"));
        assert_ne!(
            Settlement::generate(7, &per_agent).canonical_bytes(),
            Settlement::generate(7, &tweaked).canonical_bytes(),
            "with per-agent capital on, the active build knobs must steer the digest"
        );
    }
}

#[test]
fn forecast_output_price_grounds_on_belief_then_realized() {
    // S11: the grounded fallible forecast — belief.expected when observed (× bias),
    // else the public realized price (× bias), else None. The bias is a standing
    // multiplier, so a biased agent systematically over/under-shoots.
    let mut agent = Agent {
        id: AgentId(1),
        scale: Vec::new(),
        stock: Stock::new(NET.0),
        gold: Gold::ZERO,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: belief_vec(),
    };
    // FOOD: an OBSERVED belief grounds the forecast on `expected`, IGNORING realized.
    agent.expect[usize::from(FOOD.0)] = PriceBelief {
        expected: Gold(10),
        step: Gold(1),
        last_seen: 0,
        observed: true,
    };
    assert_eq!(
        forecast_output_price(&agent, FOOD, Some(Gold(5)), 10_000),
        Some(Gold(10)),
        "neutral bias on an observed belief forecasts the belief level, not realized"
    );
    assert_eq!(
        forecast_output_price(&agent, FOOD, Some(Gold(5)), 20_000),
        Some(Gold(20)),
        "an optimist over-shoots its belief by ×2"
    );
    assert_eq!(
        forecast_output_price(&agent, FOOD, None, 5_000),
        Some(Gold(5)),
        "a pessimist under-shoots its belief by ×0.5 (realized absent is irrelevant)"
    );
    // WOOD: an UN-observed belief falls back to the public realized price.
    assert!(!agent_belief(&agent, WOOD).observed);
    assert_eq!(
        forecast_output_price(&agent, WOOD, Some(Gold(6)), 20_000),
        Some(Gold(12)),
        "an un-observed good grounds on realized × bias"
    );
    // No belief AND no realized price → no forecast (the decision is skipped).
    assert_eq!(forecast_output_price(&agent, WOOD, None, 10_000), None);
}

#[test]
fn project_input_bid_limit_anchors_forecast_bid_to_observed_input_price() {
    // S11: the forecast-inflated reservation can make a producer willing to buy input,
    // but the posted limit stays at the observed input price when one exists. That
    // keeps a resting producer bid from setting a higher input price solely because its
    // output forecast was optimistic.
    assert_eq!(
        project_input_bid_limit(Gold(9), Some(Gold(4)), true),
        Gold(4),
        "an optimistic reservation is capped at the observed input price"
    );
    assert_eq!(
        project_input_bid_limit(Gold(3), Some(Gold(4)), true),
        Gold(3),
        "the cap never raises a conservative reservation"
    );
    assert_eq!(
        project_input_bid_limit(Gold(9), None, true),
        Gold(9),
        "without an observed input price, the first discovery bid keeps its reservation"
    );
    assert_eq!(
        project_input_bid_limit(Gold(9), Some(Gold(4)), false),
        Gold(9),
        "with forecasts off, the legacy reservation-as-limit path is byte-identical"
    );
}

#[test]
fn labor_hire_appraisal_accepts_receipt_that_restores_debited_savings() {
    let owner = Agent {
        id: AgentId(1),
        scale: vec![Want {
            kind: WantKind::Good(SALT),
            horizon: Horizon::Later(1),
            qty: 10,
            satisfied: false,
        }],
        stock: Stock::new(NET.0),
        gold: Gold(10),
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: Vec::new(),
    };

    assert!(
        appraise_labor_hire_for_money(&owner, Gold(4), Gold(4), 0, SALT),
        "future proceeds that restore a savings want after the wage debit must appraise"
    );
    assert!(
        !appraise_labor_hire_for_money(&owner, Gold(3), Gold(4), 0, SALT),
        "a receipt that leaves the post-wage savings want unprovisioned must still decline"
    );
}

#[test]
fn wage_hire_payment_prices_the_whole_bundle_not_per_unit() {
    // The Cultivate recipe hires `CULTIVATE_LABOR` (= 2) units in one contract, so `worker_ask`
    // is the TOTAL reservation for the 2-unit bundle. The employer ceiling is also a total
    // amount, and the escrowed payment is the total ask — never `ask * labor_qty`.
    assert_eq!(
        wage_hire_payment(Gold(5), Gold(6)),
        Some(Gold(5)),
        "affordable hire pays the worker's total ask, not ask * labor_qty"
    );
    // A total ceiling too low to cover the total ask cannot hire.
    assert_eq!(
        wage_hire_payment(Gold(7), Gold(6)),
        None,
        "total ceiling 6 below the total ask 7 must not match"
    );
    // A ceiling exactly meeting the ask still pays only the ask.
    assert_eq!(
        wage_hire_payment(Gold(6), Gold(6)),
        Some(Gold(6)),
        "ceiling exactly meeting the ask pays the ask"
    );
    assert_eq!(wage_hire_payment(Gold(4), Gold(4)), Some(Gold(4)));
    assert_eq!(wage_hire_payment(Gold(5), Gold(4)), None);
}

#[test]
fn labor_hire_appraisal_uses_total_wage_ceiling_without_unit_floor() {
    let owner = Agent {
        id: AgentId(1),
        scale: vec![Want {
            kind: WantKind::Good(SALT),
            horizon: Horizon::Later(1),
            qty: 1,
            satisfied: false,
        }],
        stock: Stock::new(NET.0),
        gold: Gold(1),
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: Vec::new(),
    };

    assert_eq!(
        highest_appraised_labor_total_wage(&owner, Gold(1), Gold(1), 0, SALT),
        Some(Gold(1)),
        "a 1-SALT total wage remains affordable for a multi-labor bundle"
    );
}

#[test]
fn highest_wage_ceiling_finds_the_band_above_a_declined_low_wage() {
    // The appraisal clears on a band, not from the bottom: an owner holding gold ABOVE its
    // soonest future-money savings threshold declines a 1-SALT wage (the want is still
    // provisioned by present gold, so the proceeds are not pivotal) yet accepts a larger wage
    // that newly un-provisions the want the proceeds then restore. Here the threshold is 5, the
    // owner holds 7 (surplus 2), and the expected proceeds are 3, so the clearing band is
    // `(2, 5]`. The ceiling must be the HIGHEST clearing wage — never `None` from a `pays(1)`
    // early-out that assumes monotonicity.
    let owner = Agent {
        id: AgentId(1),
        scale: vec![Want {
            kind: WantKind::Good(SALT),
            horizon: Horizon::Later(1),
            qty: 5,
            satisfied: false,
        }],
        stock: Stock::new(NET.0),
        gold: Gold(7),
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: Vec::new(),
    };

    // The low wage is genuinely declined — this is the state the old early-out mis-read.
    assert!(!appraise_labor_hire_for_money(
        &owner,
        Gold(3),
        Gold(1),
        0,
        SALT
    ));
    assert!(!appraise_labor_hire_for_money(
        &owner,
        Gold(3),
        Gold(2),
        0,
        SALT
    ));
    assert!(appraise_labor_hire_for_money(
        &owner,
        Gold(3),
        Gold(3),
        0,
        SALT
    ));
    // Capped by the affordability ceiling: the highest clearing wage within `[1, 3]` is 3, even
    // though `pays(1)` and `pays(2)` are both false.
    assert_eq!(
        highest_appraised_labor_total_wage(&owner, Gold(3), Gold(3), 0, SALT),
        Some(Gold(3)),
        "the ceiling is the highest clearing wage in the band, not None from a pays(1) gate"
    );
    // With headroom to reach the top of the band, the ceiling is the band's upper edge (5),
    // where the proceeds (3) still just restore the threshold (5) at post-wage gold (2).
    assert_eq!(
        highest_appraised_labor_total_wage(&owner, Gold(3), Gold(10), 0, SALT),
        Some(Gold(5)),
        "an ample affordability cap resolves to the top of the clearing band"
    );
}

fn wage_labor_test_config(mode: WageLaborMode) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_mortal_landowner_demography();
    let chain = cfg.chain.as_mut().expect("frontier base carries a chain");
    chain.rival_subsistence_commons = true;
    chain.rival_subsistence_commons_phi_bps = RIVAL_COMMONS_PHI_MARGINAL_BPS;
    chain.wage_labor = true;
    chain.wage_labor_mode = mode;
    cfg
}

fn single_term_forecast_member() -> (Settlement, AgentId, GoodId) {
    let mut s = Settlement::generate(3, &wage_labor_test_config(WageLaborMode::Voluntary));
    let bread = s.provenance_bread_good().expect("commons base has bread");
    let threshold = s
        .chain
        .as_ref()
        .expect("commons base has a chain")
        .emergency_hunger_threshold;
    assert!(threshold > 2, "test needs room below the emergency trigger");
    let target_slot = *s
        .live_colonist_slots
        .first()
        .expect("commons base has a live colonist");
    let slots = s.live_colonist_slots.clone();
    for slot in slots {
        s.colonists[slot].vocation = Vocation::Unassigned;
    }
    let worker = s.colonists[target_slot].id;
    s.colonists[target_slot].vocation = Vocation::Consumer;
    s.colonists[target_slot].household = None;
    s.colonists[target_slot].need.hunger = threshold - 2;
    let agent = s.society.agents.get_mut(worker).expect("worker is live");
    let held = agent.stock.get(bread);
    if held > 0 {
        assert!(agent.stock.remove(bread, held));
    }
    s.subsistence_commons_stock = 0;
    s.subsistence_commons_regen = 0;
    s.subsistence_commons_cap = 0;
    (s, worker, bread)
}

#[test]
fn term_forecast_advances_hunger_after_simulated_held_bread() {
    let (mut s, worker, bread) = single_term_forecast_member();
    s.society
        .agents
        .get_mut(worker)
        .expect("worker is live")
        .stock
        .add(bread, 1);

    assert_eq!(
        s.forecast_term_need_unmet(worker, bread, 2),
        0,
        "held bread eaten in the first simulated tick must lower hunger before the next one"
    );
}

#[test]
fn term_forecast_advances_hunger_after_simulated_commons_draw() {
    let (mut s, worker, bread) = single_term_forecast_member();
    s.subsistence_commons_stock = 1;
    s.subsistence_commons_cap = 1;

    assert_eq!(
        s.forecast_term_need_unmet(worker, bread, 2),
        0,
        "commons bread drawn in the first simulated tick must lower hunger before the next one"
    );
}

fn same_household_pair(s: &Settlement) -> (usize, AgentId) {
    s.live_colonist_slots
        .iter()
        .copied()
        .find_map(|slot| {
            let colonist = &s.colonists[slot];
            let household = colonist.household?;
            let heir = s
                .live_colonist_slots
                .iter()
                .copied()
                .find(|&other| other != slot && s.colonists[other].household == Some(household))
                .map(|other| s.colonists[other].id)?;
            Some((slot, heir))
        })
        .expect("wage labor test config has a same-household pair")
}

fn add_due_wage_escrow(
    s: &mut Settlement,
    employer: AgentId,
    worker: AgentId,
    recipe: &Recipe,
    amount: Gold,
) {
    if let Some((input, input_qty)) = recipe.input_good {
        s.society
            .agents
            .get_mut(employer)
            .expect("employer exists")
            .stock
            .add(input, input_qty);
    }
    s.wage_escrow_gold = s.wage_escrow_gold.saturating_add(amount);
    s.wage_escrows.push(WageEscrow {
        id: 1,
        employer,
        worker,
        amount,
        wage: amount,
        retained_funded: amount,
        endowment_funded: Gold::ZERO,
        qty: recipe.labor.max(1),
        opened_tick: s.econ_tick.saturating_sub(1),
        release_tick: s.econ_tick,
        recipe: recipe.id,
        output_good: recipe.output_good,
        output_qty: recipe.output_qty,
        input: recipe.input_good,
        delivered: 0,
    });
}

#[test]
fn fiat_wage_quotes_are_pinned_not_voluntary_asks() {
    let mut s = Settlement::generate(3, &wage_labor_test_config(WageLaborMode::FiatWage));
    let bread = s.provenance_bread_good().expect("wage base has bread");
    let threshold = s
        .chain
        .as_ref()
        .expect("wage base has a chain")
        .emergency_hunger_threshold;
    assert!(threshold > 0);
    s.subsistence_commons_stock = 0;
    s.subsistence_commons_regen = 0;
    s.subsistence_commons_cap = 0;

    let slot = s
        .live_colonist_slots
        .iter()
        .copied()
        .find(|&slot| !s.private_land_agent_holds_any_plot(s.colonists[slot].id))
        .expect("wage base has a live non-owner");
    let worker = s.colonists[slot].id;
    s.colonists[slot].vocation = Vocation::Consumer;
    s.colonists[slot].household = None;
    s.colonists[slot].need.hunger = threshold;
    let agent = s.society.agents.get_mut(worker).expect("worker is live");
    let held = agent.stock.get(bread);
    assert!(agent.stock.remove(bread, held));
    agent.gold = Gold::ZERO;
    agent.scale = vec![
        Want {
            kind: WantKind::Good(SALT),
            horizon: Horizon::Later(1),
            qty: 5,
            satisfied: false,
        },
        Want {
            kind: WantKind::Leisure,
            horizon: Horizon::Now,
            qty: 1,
            satisfied: false,
        },
    ];

    assert_eq!(
        s.worker_labor_ask_for_salt(worker, 2),
        Some(Gold(5)),
        "test setup must give the worker a voluntary ask above the fiat pin"
    );
    let quote = s
        .wage_worker_quotes(bread, 2)
        .into_iter()
        .find(|quote| quote.worker == worker)
        .expect("hungry non-owner must quote labor under the fiat control");
    assert_eq!(
        quote.ask,
        Gold(1),
        "fiat_wage is a forced-employment control and must override the voluntary ask"
    );
}

#[test]
fn due_wage_escrow_releases_when_worker_dies_on_release_tick() {
    let mut s = Settlement::generate(3, &wage_labor_test_config(WageLaborMode::Voluntary));
    let recipe = s
        .wage_labor_recipe()
        .expect("wage base has a cultivate recipe");
    let (worker_slot, heir) = same_household_pair(&s);
    let worker = s.colonists[worker_slot].id;
    let employer = s
        .live_colonist_slots
        .iter()
        .map(|&slot| s.colonists[slot].id)
        .find(|&id| id != worker && id != heir)
        .expect("wage base has a distinct employer");
    let amount = Gold(7);
    s.society.agents.get_mut(worker).expect("worker").gold = Gold::ZERO;
    let heir_gold_before = s.society.agents.get(heir).expect("heir").gold;
    let employer_output_before = s
        .society
        .agents
        .get(employer)
        .expect("employer")
        .stock
        .get(recipe.output_good);
    add_due_wage_escrow(&mut s, employer, worker, &recipe, amount);

    s.mark_colonist_dead(worker_slot);
    let mut report = EconTickReport::default();
    let mut wage_labor_used = Vec::new();
    assert!(s.settle_estate_to_heirs(worker, &mut report, &mut wage_labor_used));

    assert_eq!(s.wage_escrow_gold, Gold::ZERO);
    assert!(s.wage_escrows.is_empty());
    assert_eq!(
        s.society.agents.get(heir).expect("heir").gold,
        heir_gold_before
            .checked_add(amount)
            .expect("test wage fits heir gold"),
        "a due wage earned before worker death must route through the worker estate"
    );
    assert_eq!(
        s.society
            .agents
            .get(employer)
            .expect("employer")
            .stock
            .get(recipe.output_good),
        employer_output_before + recipe.output_qty,
        "the employer keeps output from labor delivered before the worker death"
    );
    assert_eq!(
        report.produced_of(recipe.output_good),
        u64::from(recipe.output_qty)
    );
    assert!(
        !s.wage_proceeds_buckets.contains_key(&worker),
        "wage-spend attribution is not inherited by a dead worker"
    );
    assert!(
        wage_labor_used.is_empty(),
        "dead workers do not need a next-tick labor readback"
    );
}

#[test]
fn due_wage_escrow_releases_when_employer_dies_on_release_tick() {
    let mut s = Settlement::generate(3, &wage_labor_test_config(WageLaborMode::Voluntary));
    let recipe = s
        .wage_labor_recipe()
        .expect("wage base has a cultivate recipe");
    let (employer_slot, heir) = same_household_pair(&s);
    let employer = s.colonists[employer_slot].id;
    let worker = s
        .live_colonist_slots
        .iter()
        .map(|&slot| s.colonists[slot].id)
        .find(|&id| id != employer && id != heir)
        .expect("wage base has a distinct worker");
    let amount = Gold(9);
    let worker_gold_before = s.society.agents.get(worker).expect("worker").gold;
    {
        let heir_stock = &mut s.society.agents.get_mut(heir).expect("heir").stock;
        let held = heir_stock.get(recipe.output_good);
        assert!(heir_stock.remove(recipe.output_good, held));
    }
    {
        let employer_stock = &mut s.society.agents.get_mut(employer).expect("employer").stock;
        let held = employer_stock.get(recipe.output_good);
        assert!(employer_stock.remove(recipe.output_good, held));
    }
    add_due_wage_escrow(&mut s, employer, worker, &recipe, amount);

    s.mark_colonist_dead(employer_slot);
    let mut report = EconTickReport::default();
    let mut wage_labor_used = Vec::new();
    assert!(s.settle_estate_to_heirs(employer, &mut report, &mut wage_labor_used));

    assert_eq!(s.wage_escrow_gold, Gold::ZERO);
    assert!(s.wage_escrows.is_empty());
    assert_eq!(
        s.society.agents.get(worker).expect("worker").gold,
        worker_gold_before
            .checked_add(amount)
            .expect("test wage fits worker gold"),
        "a due wage must release to the live worker when the employer dies"
    );
    assert_eq!(
        s.society
            .agents
            .get(heir)
            .expect("heir")
            .stock
            .get(recipe.output_good),
        recipe.output_qty,
        "output credited before employer removal must route through the employer estate"
    );
    assert_eq!(
        report.produced_of(recipe.output_good),
        u64::from(recipe.output_qty)
    );
    assert_eq!(
        s.wage_retained_earnings.get(&employer),
        None,
        "dead employers do not keep or restore wage-eligible retained earnings"
    );
    assert_eq!(wage_labor_used, vec![(worker, recipe.labor.max(1))]);
}

#[test]
fn open_wage_escrow_workers_are_not_assigned_world_tasks() {
    let mut s = Settlement::generate(3, &wage_labor_test_config(WageLaborMode::Voluntary));
    let recipe = s
        .wage_labor_recipe()
        .expect("wage base has a cultivate recipe");
    let (slot, node) = s
        .live_colonist_slots
        .iter()
        .find_map(|&slot| {
            let colonist = &s.colonists[slot];
            (colonist.vocation == Vocation::Gatherer
                && !s.private_land_agent_holds_any_plot(colonist.id))
            .then_some((slot, colonist.node?))
        })
        .expect("wage base has a non-owner gatherer");
    let worker = s.colonists[slot].id;
    let employer = s
        .live_colonist_slots
        .iter()
        .map(|&slot| s.colonists[slot].id)
        .find(|&id| id != worker)
        .expect("wage base has another live agent");

    assert!(s
        .world
        .assign_task(worker, Task::GoHarvest(node, s.carry_cap)));
    assert_eq!(s.world.agent_status(worker), Some(AgentStatus::Moving));
    s.wage_escrows.push(WageEscrow {
        id: 1,
        employer,
        worker,
        amount: Gold(1),
        wage: Gold(1),
        retained_funded: Gold(1),
        endowment_funded: Gold::ZERO,
        qty: recipe.labor.max(1),
        opened_tick: s.econ_tick,
        release_tick: s.econ_tick.saturating_add(1),
        recipe: recipe.id,
        output_good: recipe.output_good,
        output_qty: recipe.output_qty,
        input: recipe.input_good,
        delivered: 0,
    });

    s.idle_open_wage_workers();
    assert_eq!(
        s.world.agent_status(worker),
        Some(AgentStatus::Idle),
        "an escrowed worker's prior gather/deposit task must be cleared"
    );
    s.assign_idle_gatherer_tasks();
    assert_eq!(
        s.world.agent_status(worker),
        Some(AgentStatus::Idle),
        "an escrowed worker must not be reassigned to normal gathering"
    );
}

#[test]
fn canonical_bytes_include_forecast_bias() {
    // S11: under entrepreneurial forecasts the per-colonist forecast bias steers every
    // appraisal, so two configs whose forecast-bias base differs — and thus whose drawn
    // per-colonist biases differ — digest apart.
    let base = SettlementConfig::frontier_coemergent_strong_entrepreneurial();
    let mut tilted = base.clone();
    tilted.forecast_bias_base_bps = 15_000;
    assert_ne!(
        Settlement::generate(7, &base).canonical_bytes(),
        Settlement::generate(7, &tilted).canonical_bytes(),
        "forecast_bias must be part of the entrepreneurial identity"
    );

    // With the flag OFF the forecast bias is never serialized, so the SAME base change
    // is invisible (byte-identical) — the additivity anchor.
    let off = SettlementConfig::frontier_coemergent_strong_originary();
    let mut off_tilted = off.clone();
    off_tilted.forecast_bias_base_bps = 15_000;
    assert_eq!(
        Settlement::generate(7, &off).canonical_bytes(),
        Settlement::generate(7, &off_tilted).canonical_bytes(),
        "forecast bias must be invisible to the digest with forecasts off"
    );
}

#[test]
fn canonical_bytes_include_entrepreneurial_flag_and_belief_observed() {
    // S11: the entrepreneurial_forecasts flag is part of the chain config identity (it
    // flips every appraisal from realized price to a per-agent forecast), so the
    // flagship and the originary base it derives from digest apart at generation.
    let on = SettlementConfig::frontier_coemergent_strong_entrepreneurial();
    let off = SettlementConfig::frontier_coemergent_strong_originary();
    assert_ne!(
        Settlement::generate(7, &on).canonical_bytes(),
        Settlement::generate(7, &off).canonical_bytes(),
        "the entrepreneurial_forecasts flag must be part of the identity"
    );

    // The per-belief `observed` flag is in the digest under the flag (it steers the
    // belief-vs-realized grounding) and is NOT derivable from `last_seen`. Flip one
    // belief's `observed` and the digest must move under the flag…
    let a = Settlement::generate(7, &on);
    let mut b = Settlement::generate(7, &on);
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    b.society.agents.as_mut_slice()[0].expect[usize::from(FOOD.0)].observed = true;
    assert_ne!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the per-belief `observed` flag must be part of the entrepreneurial identity"
    );

    // …and must stay invisible with the flag off (byte-identical).
    let c = Settlement::generate(7, &off);
    let mut d = Settlement::generate(7, &off);
    d.society.agents.as_mut_slice()[0].expect[usize::from(FOOD.0)].observed = true;
    assert_eq!(
        c.canonical_bytes(),
        d.canonical_bytes(),
        "the belief `observed` flag must be invisible to the digest with forecasts off"
    );
}

#[test]
fn per_agent_capital_builds_by_appraisal_with_a_visible_decliner() {
    // S10.1: on a simple designated-GOLD capital config with per_agent_capital ON, an
    // individual colonist builds via its OWN ordinal appraisal, and the per-tick
    // decision diagnostic shows at least one tick where an EARLIER-eligible colonist
    // declined on its own scale while a LATER one accepted — proving the builder is
    // chosen by its own appraisal, not slot-order-first. (The flagship emergence
    // variant is covered by the integration suite; this isolates the gated core.)
    let mut cfg = capital_test_config();
    cfg.chain.as_mut().expect("chain").per_agent_capital = true;
    // A leaner roster than the full capital config keeps this isolated core test fast
    // while still leaving bread demand the seeded chain cannot meet (so builds fire).
    cfg.consumers = 20;
    cfg.gatherers = 12;
    let mut s = Settlement::generate(1, &cfg);

    let mut saw_later_accept_after_earlier_own_decline = false;
    for _ in 0..500u64 {
        s.econ_tick();
        let decisions = s.last_capital_decisions();
        let earliest_own_decline = decisions
            .iter()
            .filter(|d| {
                !d.accepted
                    && matches!(
                        d.reason,
                        CapitalDeclineReason::NoFutureProvision
                            | CapitalDeclineReason::PresentCostOutranks
                    )
            })
            .map(|d| d.slot)
            .min();
        if let Some(decline_slot) = earliest_own_decline {
            if decisions
                .iter()
                .any(|d| d.accepted && d.slot > decline_slot)
            {
                saw_later_accept_after_earlier_own_decline = true;
            }
        }
    }

    assert!(
        s.tools_built() > 0,
        "an individual colonist must build via its own appraisal with per-agent on"
    );
    assert!(
        saw_later_accept_after_earlier_own_decline,
        "the diagnostic must show an earlier-eligible colonist declining on its own \
             scale while a later one accepts (per-agent, not slot-order-first)"
    );

    // Flag OFF (the S7 heuristic) records no per-agent decision diagnostic at all, and
    // the multi-horizon savings ladder never activates — byte-identical to S7.
    let mut off = capital_test_config();
    off.chain.as_mut().expect("chain").per_agent_capital = false;
    let mut t = Settlement::generate(1, &off);
    t.run(200);
    assert!(
        t.last_capital_decisions().is_empty(),
        "the per-agent diagnostic must be empty on the S7 heuristic path"
    );
}

#[test]
fn per_agent_capital_ignores_stale_output_prices() {
    let mut cfg = capital_test_config();
    {
        let c = cfg.chain.as_mut().expect("chain");
        c.per_agent_capital = true;
        c.tool_build_labor = 1;
    }
    cfg.consumers = 20;
    cfg.gatherers = 12;
    let chain = cfg.chain.as_ref().expect("chain");
    let bread = chain.content.bread();
    let flour = chain.content.flour();
    let grain = chain.content.grain();
    let mill = chain.content.mill();
    let oven = chain.content.oven();
    let wood_qty = chain.tool_build_wood;
    let mut s = Settlement::generate(1, &cfg);

    for _ in 0..500u64 {
        s.econ_tick();
        if s.society.tick.0 > CAPITAL_BUILD_RECENCY + 2
            && s.realized_price(bread).is_some()
            && s.realized_price(flour).is_some()
            && s.realized_price(grain).is_some()
        {
            break;
        }
    }
    assert!(
        s.realized_price(bread).is_some()
            && s.realized_price(flour).is_some()
            && s.realized_price(grain).is_some(),
        "test setup must establish realized recipe prices"
    );

    let old_tick = s.society.tick.0.saturating_sub(CAPITAL_BUILD_RECENCY + 2);
    for trade in &mut s.society.trades {
        trade.tick = old_tick;
    }
    assert!(!s.good_traded_within(bread, CAPITAL_BUILD_RECENCY));
    assert!(!s.good_traded_within(flour, CAPITAL_BUILD_RECENCY));

    let mut eligible = 0u32;
    for &slot in &s.live_colonist_slots {
        let colonist = &mut s.colonists[slot];
        if colonist.latent.is_some()
            || !matches!(
                colonist.vocation,
                Vocation::Gatherer | Vocation::Consumer | Vocation::Unassigned
            )
        {
            continue;
        }
        colonist.need.hunger = 0;
        colonist.need.warmth = 0;
        colonist.need.rest = 0;
        let Some(agent) = s.society.agents.get_mut(colonist.id) else {
            continue;
        };
        if agent.stock.get(mill) != 0 || agent.stock.get(oven) != 0 {
            continue;
        }
        let held = agent.stock.get(WOOD);
        if held < wood_qty {
            agent.stock.add(WOOD, wood_qty - held);
        }
        eligible += 1;
    }
    assert!(eligible > 0, "test setup must leave eligible builders");

    let built_before = s.tools_built();
    s.econ_tick();
    let decisions = s.last_capital_decisions();
    assert!(
        !decisions.is_empty(),
        "test setup must exercise per-agent appraisals"
    );
    assert!(
        decisions
            .iter()
            .all(|d| !d.accepted && d.reason == CapitalDeclineReason::NoPrices),
        "stale output prices must not support per-agent builds: {decisions:?}"
    );
    assert_eq!(
        s.tools_built(),
        built_before,
        "no tool may be built from stale realized output prices"
    );
}

#[test]
fn per_agent_capital_clears_decisions_on_completion_ticks() {
    let mut cfg = capital_test_config();
    cfg.chain.as_mut().expect("chain").per_agent_capital = true;
    cfg.consumers = 20;
    cfg.gatherers = 12;
    let mill = cfg.chain.as_ref().expect("chain").content.mill();
    let oven = cfg.chain.as_ref().expect("chain").content.oven();
    let mut s = Settlement::generate(1, &cfg);

    let mut saw_started_build = false;
    for _ in 0..700u64 {
        s.econ_tick();
        if s.active_capital_builds() > 0 && s.last_capital_decisions().iter().any(|d| d.accepted) {
            saw_started_build = true;
            break;
        }
    }
    assert!(
        saw_started_build,
        "test setup must start an in-flight per-agent build"
    );

    for _ in 0..16u64 {
        let report = s.econ_tick();
        if report.produced_of(mill) + report.produced_of(oven) > 0 {
            assert!(
                s.last_capital_decisions().is_empty(),
                "a completion-only tick must not expose stale per-agent decisions"
            );
            return;
        }
    }
    panic!("test setup did not reach a capital completion tick");
}

#[test]
#[should_panic(
    expected = "per-agent capital requires tool_build_labor below the deepest savings horizon"
)]
fn per_agent_capital_build_labor_must_fit_the_savings_horizon() {
    let mut cfg = capital_test_config();
    {
        let c = cfg.chain.as_mut().expect("chain");
        c.per_agent_capital = true;
        c.tool_build_labor = u32::try_from(max_savings_ladder_horizon()).unwrap_or(u32::MAX);
    }
    let _ = Settlement::generate(1, &cfg);
}

#[test]
fn capital_capacity_counts_only_live_tool_holders() {
    let cfg = capital_test_config();
    let mill = cfg.chain.as_ref().expect("chain").content.mill();
    let mut s = Settlement::generate(1, &cfg);
    let holders_before = s.live_colonist_holder_count(mill);
    let whole_before = s.whole_system_total(mill);

    // Commons tools are conserved (whole-system) but inaccessible — never usable
    // capital capacity.
    s.commons_stock.insert(mill, 10);
    assert_eq!(
        s.live_colonist_holder_count(mill),
        holders_before,
        "commons tools are conserved but not usable capital capacity"
    );
    assert_eq!(
        s.whole_system_total(mill),
        whole_before + 10,
        "whole-system conservation still includes commons tools"
    );

    // Concentration cannot overstate capacity: stacking a SECOND mill on a colonist
    // that already holds one (an inherited/transferred estate) adds a conserved unit
    // but no capacity — the holder still runs one vocation, one throughput, so the
    // bottleneck/idle-tool guards must count holders, not raw units.
    let stacked = s
        .live_colonist_slots
        .iter()
        .map(|&slot| s.colonists[slot].id)
        .find(|&id| {
            s.society
                .agents
                .get(id)
                .is_some_and(|a| a.stock.get(mill) > 0)
        })
        .expect("the seeded latent pool holds at least one mill");
    s.society
        .agents
        .get_mut(stacked)
        .expect("the stacked holder resolves")
        .stock
        .add(mill, 1);
    assert_eq!(
        s.live_colonist_holder_count(mill),
        holders_before,
        "a second tool stacked on one holder must not raise usable capacity"
    );
    assert_eq!(
        s.whole_system_total(mill),
        whole_before + 11,
        "the stacked unit is still conserved in the whole-system total"
    );
}

#[test]
fn one_labor_capital_build_completes_on_start_and_records_labor() {
    let mut cfg = capital_test_config();
    cfg.chain.as_mut().expect("chain").tool_build_labor = 1;
    let mill = cfg.chain.as_ref().expect("chain").content.mill();
    let oven = cfg.chain.as_ref().expect("chain").content.oven();
    let mut s = Settlement::generate(1, &cfg);

    let mut builder = None;
    for tick in 0..900u64 {
        let report = s.econ_tick();
        let tool_produced = report.produced_of(mill) + report.produced_of(oven);
        if report.consumed_as_input_of(WOOD) > 0 {
            assert!(
                tool_produced > 0,
                "a one-labor capital build must complete on its start tick, failed at {tick}"
            );
            assert_eq!(
                s.active_capital_builds(),
                0,
                "a completion tick must not immediately start the next capital build"
            );
            builder = (0..s.population())
                .find(|&i| s.acquired_tool_of(i))
                .and_then(|i| s.colonist_id(i).map(|id| (i, id)));
            break;
        }
    }
    let (builder_index, builder_id) = builder.expect("a one-labor build completed");
    assert!(
        s.society
            .labor_used_last_tick()
            .iter()
            .any(|&(id, labor)| id == builder_id && labor > 0),
        "capital build labor must be recorded in the society labor log"
    );

    let builder_slot = s.slot_for_id(builder_id).expect("builder slot");
    s.colonists[builder_slot].need.rest = 0;
    s.econ_tick();
    assert!(
        s.need_of(builder_index).expect("builder need").rest > 0,
        "recorded capital labor must feed the next needs update"
    );
}

#[test]
fn capital_formation_emits_input_and_output_at_the_mutation_seams() {
    let mut cfg = capital_test_config();
    {
        let chain = cfg.chain.as_mut().expect("chain");
        chain.producible_capital = false;
        chain.tool_build_labor = 1;
    }
    let mut s = Settlement::generate(1, &cfg);

    for _ in 0..900u64 {
        s.econ_tick();
        s.chain.as_mut().expect("chain").producible_capital = true;
        s.closed_circulation = true;
        let mut report = EconTickReport::default();
        let mut labor_used = Vec::new();
        let completed = s.run_capital_formation(&mut report, &mut labor_used);
        if report.consumed_as_input_of(WOOD) > 0 {
            assert!(completed, "a one-labor build completes at its start seam");
            let capital_events: Vec<_> = s
                .closure
                .tape
                .iter()
                .filter_map(|event| match event.kind {
                    closure::ClosureEventKind::CapitalFormation {
                        agent,
                        input,
                        input_qty,
                        tool,
                        tool_qty,
                    } => Some((agent, input, input_qty, tool, tool_qty)),
                    _ => None,
                })
                .collect();
            assert_eq!(
                capital_events.len(),
                2,
                "start and completion are distinct seams"
            );
            assert_eq!(capital_events[0].1, WOOD);
            assert!(capital_events[0].2 > 0);
            assert_eq!(capital_events[0].4, 0);
            assert_eq!(capital_events[1].0, capital_events[0].0);
            assert_eq!(capital_events[1].1, WOOD);
            assert_eq!(capital_events[1].2, 0);
            assert_eq!(capital_events[1].3, capital_events[0].3);
            assert!(capital_events[1].4 > 0);
            return;
        }
        s.closed_circulation = false;
        s.chain.as_mut().expect("chain").producible_capital = false;
    }
    panic!("test setup did not reach a capital start seam");
}

#[test]
#[should_panic(expected = "producible capital (S7.2) requires tool-acquisition eligibility")]
fn producible_capital_requires_eligibility() {
    let mut cfg = SettlementConfig::frontier_endogenous();
    {
        let c = cfg.chain.as_mut().expect("chain");
        c.tool_acquisition_eligibility = false;
        c.producible_capital = true;
    }
    let _ = Settlement::generate(7, &cfg);
}

#[test]
fn capital_is_built_under_demand_and_conserves() {
    // S7.2 (the headline): under the scaling economy's unmet bread demand a builder
    // commits its OWN WOOD, completes a BuildMill/BuildOven, the whole-system tool
    // count rises, produced_of(tool) > 0, WOOD is booked to consumed_as_input, and
    // conservation holds EVERY tick across the build. The builder then adopts and
    // becomes a producer (a formerly-non-latent colonist with a produced tool).
    let cfg = capital_test_config();
    let mill = cfg.chain.as_ref().expect("chain").content.mill();
    let oven = cfg.chain.as_ref().expect("chain").content.oven();
    let mut s = Settlement::generate(1, &cfg);
    let tools_before = s.whole_system_total(mill) + s.whole_system_total(oven);

    let mut wood_consumed_as_input = 0u64;
    let mut tool_produced = 0u64;
    // A formerly-non-latent colonist that built a tool and adopted the trade — sampled
    // across the run, since adoption fluctuates tick to tick in the emergent chain.
    let mut built_adopter = false;
    for tick in 0..1200u64 {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "whole-system conservation must hold every tick across a tool build, broke at {tick}"
        );
        // WOOD is consumed_as_input ONLY by a capital build (no recipe consumes it).
        wood_consumed_as_input += report.consumed_as_input_of(WOOD);
        tool_produced += report.produced_of(mill) + report.produced_of(oven);
        if !built_adopter {
            built_adopter = (0..s.population()).any(|i| {
                s.acquired_tool_of(i)
                    && matches!(
                        s.vocation_of(i),
                        Some(Vocation::Miller) | Some(Vocation::Baker)
                    )
            });
        }
    }

    assert!(
        s.tools_built() > 0,
        "a builder must complete at least one tool under unmet demand, got {}",
        s.tools_built()
    );
    assert!(
        tool_produced > 0,
        "produced_of(tool) must be > 0, got {tool_produced}"
    );
    assert!(
        wood_consumed_as_input > 0,
        "the build must book its WOOD to consumed_as_input, got {wood_consumed_as_input}"
    );
    let tools_after = s.whole_system_total(mill) + s.whole_system_total(oven);
    assert!(
        tools_after > tools_before,
        "whole-system tool count must rise ({tools_before} -> {tools_after})"
    );
    assert!(
        built_adopter,
        "a formerly-non-latent builder must have adopted a producer role with its built tool"
    );
}

#[test]
fn no_capital_built_when_the_appraisal_declines() {
    // S7.2 overinvestment guard: a payback horizon of 0 puts the amortized margin
    // (margin × 0 = 0) below any positive build cost, so the appraisal always
    // declines — no tool is ever built, however strong the demand. Proves building
    // is the appraisal's decision, not blind: the per-run margin must clear the
    // payback bar (the demand-driven version is the acceptance suite's test 7).
    let mut cfg = capital_test_config();
    cfg.chain.as_mut().expect("chain").capital_payback_cycles = 0;
    let mut s = Settlement::generate(1, &cfg);
    for _ in 0..1200u64 {
        s.econ_tick();
    }
    assert_eq!(
        s.tools_built(),
        0,
        "no tool may be built when the amortized margin is below the payback bar"
    );
    assert_eq!(
        s.active_capital_builds(),
        0,
        "no build may be left in flight"
    );
}

#[test]
fn canonical_bytes_include_producer_subsistence() {
    // `producer_subsistence` mints a local staple/WOOD floor for the chain's
    // producers every tick — it steers future behaviour, yet it is a pure
    // runtime knob that never shows up in the generated holdings. Two chains
    // differing only in it must therefore digest differently, or the
    // determinism tripwire would call two non-equivalent configs equal.
    let mut base = SettlementConfig::emergent_chain();
    base.chain.as_mut().expect("chain").producer_subsistence = 0;
    let mut fed = SettlementConfig::emergent_chain();
    fed.chain.as_mut().expect("chain").producer_subsistence = 4;
    let base = Settlement::generate(7, &base);
    let fed = Settlement::generate(7, &fed);
    assert_ne!(
        base.canonical_bytes(),
        fed.canonical_bytes(),
        "the producer-subsistence floor must be part of the chain config identity"
    );
}

#[test]
fn canonical_bytes_include_project_input_bids() {
    // `project_input_bids` switches input acquisition from the generic spot bid
    // to the project-aware imputed market bid — a runtime knob that steers
    // future ticks without changing generation, so it too must split the digest.
    let mut base = SettlementConfig::emergent_chain();
    base.chain.as_mut().expect("chain").project_input_bids = false;
    let mut bidding = SettlementConfig::emergent_chain();
    bidding.chain.as_mut().expect("chain").project_input_bids = true;
    let base = Settlement::generate(7, &base);
    let bidding = Settlement::generate(7, &bidding);
    assert_ne!(
        base.canonical_bytes(),
        bidding.canonical_bytes(),
        "the project-aware input bid flag must be part of the chain config identity"
    );
}

#[test]
fn canonical_bytes_include_recurring_motive() {
    // `recurring_motive` keeps an owner-operator adopted while the recipe stays
    // profitable — a runtime knob that steers future role-choice ticks without
    // changing generation, so two chains differing only in it must digest apart.
    let mut base = SettlementConfig::emergent_chain();
    base.chain.as_mut().expect("chain").recurring_motive = false;
    let mut motivated = SettlementConfig::emergent_chain();
    motivated.chain.as_mut().expect("chain").recurring_motive = true;
    let base = Settlement::generate(7, &base);
    let motivated = Settlement::generate(7, &motivated);
    assert_ne!(
        base.canonical_bytes(),
        motivated.canonical_bytes(),
        "the recurring-motive flag must be part of the chain config identity"
    );
}

#[test]
fn canonical_bytes_include_subsistence_on_grain() {
    // `subsistence_on_grain` is realised as `known.subsistence`, a directly
    // edible staple fallback that steers the future needs/scale phase yet leaves
    // generation untouched, so two chains differing only in it must digest apart.
    let mut base = SettlementConfig::emergent_chain();
    base.chain.as_mut().expect("chain").subsistence_on_grain = false;
    let mut edible = SettlementConfig::emergent_chain();
    edible.chain.as_mut().expect("chain").subsistence_on_grain = true;
    let base = Settlement::generate(7, &base);
    let edible = Settlement::generate(7, &edible);
    assert_ne!(
        base.canonical_bytes(),
        edible.canonical_bytes(),
        "the edible-grain subsistence fallback must be part of the chain config identity"
    );
}

#[test]
fn canonical_bytes_include_own_labor_subsistence() {
    // S12: the own-labor gate retires the food mints, wires FORAGE as the
    // subsistence good, and steers the forage phase + yield — so the provisioned
    // flagship (own-labor ON) must digest apart from the entrepreneurial base
    // (own-labor OFF, no forage good).
    let base = Settlement::generate(
        7,
        &SettlementConfig::frontier_coemergent_strong_entrepreneurial(),
    );
    let provisioned = Settlement::generate(
        7,
        &SettlementConfig::frontier_coemergent_strong_provisioned(),
    );
    assert_ne!(
        base.canonical_bytes(),
        provisioned.canonical_bytes(),
        "own-labor subsistence (the FORAGE good + retired mints) must be part of the identity"
    );

    // ON: a different forage yield or hysteresis threshold steers the forage phase,
    // so each must split the digest.
    let mut y_a = SettlementConfig::frontier_coemergent_strong_provisioned();
    y_a.chain.as_mut().expect("chain").forage_yield = 2;
    let mut y_b = SettlementConfig::frontier_coemergent_strong_provisioned();
    y_b.chain.as_mut().expect("chain").forage_yield = 5;
    assert_ne!(
        Settlement::generate(7, &y_a).canonical_bytes(),
        Settlement::generate(7, &y_b).canonical_bytes(),
        "with own-labor on, the forage yield must be part of the digest"
    );
    let mut h = SettlementConfig::frontier_coemergent_strong_provisioned();
    h.chain.as_mut().expect("chain").forage_hunger_in = 9;
    assert_ne!(
        provisioned.canonical_bytes(),
        Settlement::generate(7, &h).canonical_bytes(),
        "with own-labor on, the forage entry threshold must be part of the digest"
    );

    // OFF: the (unused) forage knobs must NOT split a flag-off chain's digest, or
    // the tripwire would call two behaviour-identical configs unequal.
    let off_bytes = base.canonical_bytes();
    let mut off_knobs = SettlementConfig::frontier_coemergent_strong_entrepreneurial();
    {
        let c = off_knobs.chain.as_mut().expect("chain");
        c.forage_yield = 9;
        c.forage_hunger_in = 11;
        c.forage_hunger_out = 1;
    }
    assert_eq!(
        off_bytes,
        Settlement::generate(7, &off_knobs).canonical_bytes(),
        "with own-labor off, the unused forage knobs must not steer the digest"
    );
}

#[test]
fn canonical_bytes_include_forage_commons() {
    // S14: the FORAGE-commons mode switches the forage path from the S12 fixed
    // credit (a `0/0/0` marker) to a real depleting node + the haul cycle, so a
    // commons config must digest apart from the marker-mode flagship. Both the
    // node's stock/regen/cap (via `world.canonical_bytes`) and the behavior marker
    // contribute.
    let marker = Settlement::generate(
        7,
        &SettlementConfig::frontier_coemergent_strong_provisioned(),
    );
    let mut commons_cfg = SettlementConfig::frontier_coemergent_strong_provisioned();
    commons_cfg.chain.as_mut().expect("chain").forage_commons = Some(ForageCommons {
        stock: 40,
        regen: 3,
        cap: 80,
    });
    let commons = Settlement::generate(7, &commons_cfg);
    assert_ne!(
        marker.canonical_bytes(),
        commons.canonical_bytes(),
        "the FORAGE-commons mode must be part of the chain config identity"
    );

    // In commons mode, the S12 fixed-credit yield is retired: varying it cannot
    // change execution and therefore must not split canonical bytes.
    let mut commons_yield = commons_cfg.clone();
    commons_yield.chain.as_mut().expect("chain").forage_yield = 99;
    assert_eq!(
        commons.canonical_bytes(),
        Settlement::generate(7, &commons_yield).canonical_bytes(),
        "commons mode must ignore the inactive fixed-credit forage_yield"
    );

    // The behavior marker is load-bearing on its OWN: even a degenerate `0/0/0`
    // commons (node bytes coincide with the marker) must still digest apart, because
    // it routes foragers through the depleting harvest cycle + the FORAGE endowment.
    let mut degenerate = SettlementConfig::frontier_coemergent_strong_provisioned();
    degenerate.chain.as_mut().expect("chain").forage_commons = Some(ForageCommons {
        stock: 0,
        regen: 0,
        cap: 0,
    });
    assert_ne!(
        marker.canonical_bytes(),
        Settlement::generate(7, &degenerate).canonical_bytes(),
        "the commons behavior marker must split the digest even with a 0/0/0 node"
    );

    // OFF (no own-labor path): the unused commons must NOT split a flag-off chain's
    // digest, or the tripwire would call two behaviour-identical configs unequal.
    let off = Settlement::generate(7, &SettlementConfig::emergent_chain());
    let mut off_commons = SettlementConfig::emergent_chain();
    off_commons.chain.as_mut().expect("chain").forage_commons = Some(ForageCommons {
        stock: 99,
        regen: 9,
        cap: 99,
    });
    assert_eq!(
        off.canonical_bytes(),
        Settlement::generate(7, &off_commons).canonical_bytes(),
        "with own-labor off, an unused commons must not steer the digest"
    );
}

#[test]
fn canonical_bytes_include_birth_block_counters() {
    // S14: the birth-block diagnostic counters are live run state, serialized ONLY
    // on the forage-commons path — so on it they split the digest, and off it (a
    // plain demography golden) the unused counters never do.
    let mut on = Settlement::generate(1, &SettlementConfig::frontier_forage_capacity());
    let before = on.canonical_bytes();
    on.birth_block_hunger_ceiling = on.birth_block_hunger_ceiling.wrapping_add(1);
    assert_ne!(
        before,
        on.canonical_bytes(),
        "on the forage-commons path the birth-block counters must be part of the digest"
    );

    let mut off = Settlement::generate(1, &SettlementConfig::lineages());
    let off_before = off.canonical_bytes();
    off.birth_block_hunger_ceiling = off.birth_block_hunger_ceiling.wrapping_add(7);
    off.birth_block_size_cap = off.birth_block_size_cap.wrapping_add(3);
    assert_eq!(
        off_before,
        off.canonical_bytes(),
        "off the forage-commons path the unused birth-block counters must not steer the digest"
    );
}

#[test]
fn canonical_bytes_exclude_starvation_deaths_total() {
    // S17 (the P1 tripwire): the starvation-death counter is a runtime-only
    // diagnostic — it must NEVER enter canonical_bytes, or it would shift the
    // digest of every live-starvation config (`g4a_death`, `starved_hauler`) and
    // break their goldens. Mutating it leaves the bytes byte-identical, on a
    // live-starvation config AND on the demographic mortality scenario.
    let mut s = Settlement::generate(1, &SettlementConfig::starved_hauler());
    s.run(40); // exercise a real starvation death so the counter is non-zero
    assert!(
        s.starvation_deaths_total() > 0,
        "the starved hauler must record a starvation death"
    );
    let before = s.canonical_bytes();
    s.starvation_deaths_total = s.starvation_deaths_total.wrapping_add(1);
    assert_eq!(
        before,
        s.canonical_bytes(),
        "starvation_deaths_total must NOT enter canonical_bytes (the digest tripwire)"
    );

    let mut d = Settlement::generate(1, &SettlementConfig::frontier_mortality());
    d.run(50);
    let d_before = d.canonical_bytes();
    d.starvation_deaths_total = d.starvation_deaths_total.wrapping_add(9);
    assert_eq!(
        d_before,
        d.canonical_bytes(),
        "starvation_deaths_total must not steer the digest on the mortality scenario"
    );
}

#[test]
fn birth_food_selector_seeds_founders_from_forage_not_bread() {
    // S14.2: on the forage-commons path the birth-food selector routes the founder
    // seed (and the child endowment) to the FORAGE subsistence good, NOT the bread
    // staple — so a lineage feeds and reproduces on forage. `known.hunger` (bread)
    // is left untouched, so founders hold zero bread.
    let cfg = SettlementConfig::frontier_forage_capacity();
    let bread = cfg.chain.as_ref().expect("chain").content.bread();
    let forage = cfg
        .chain
        .as_ref()
        .expect("chain")
        .content
        .forage()
        .expect("forage good");
    let s = Settlement::generate(1, &cfg);
    let mut founders_seen = 0usize;
    for colonist in &s.colonists {
        if colonist.household.is_none() {
            continue;
        }
        founders_seen += 1;
        let agent = s.society.agents.get(colonist.id).expect("founder agent");
        assert_eq!(
            agent.stock.get(bread),
            0,
            "a forage-path founder holds no bread (the staple is untouched)"
        );
        assert!(
            agent.stock.get(forage) > 0,
            "a forage-path founder is seeded with FORAGE (the birth-food selector)"
        );
    }
    assert!(founders_seen > 0, "the config must seed lineage founders");
}

#[test]
fn canonical_bytes_include_foraging() {
    // S12: the per-colonist `foraging` flag steers the next fast loop (forage the
    // FORAGE node vs harvest WOOD), so two own-labor states differing only in it must
    // digest apart — and a flag-off chain must NOT serialize it (byte-identical).
    let mut on = Settlement::generate(
        7,
        &SettlementConfig::frontier_coemergent_strong_provisioned(),
    );
    let before = on.canonical_bytes();
    on.colonists[0].foraging = !on.colonists[0].foraging;
    assert_ne!(
        before,
        on.canonical_bytes(),
        "with own-labor on, a colonist's foraging flag must be part of the digest"
    );

    let mut off = Settlement::generate(
        7,
        &SettlementConfig::frontier_coemergent_strong_entrepreneurial(),
    );
    let off_before = off.canonical_bytes();
    off.colonists[0].foraging = !off.colonists[0].foraging;
    assert_eq!(
        off_before,
        off.canonical_bytes(),
        "with own-labor off, the unused foraging flag must not steer the digest"
    );
}

#[test]
fn canonical_bytes_include_own_use_cultivation() {
    // S15: the own-use cultivation gate + its thresholds steer who escalates to
    // cultivation and how much bread is eaten, so a cultivation config must digest
    // apart from the same config with the flag off — and an OFF config must NOT
    // serialize the (unused) thresholds.
    let on = Settlement::generate(7, &SettlementConfig::frontier_cultivation());
    let mut off_cfg = SettlementConfig::frontier_cultivation();
    off_cfg.chain.as_mut().expect("chain").own_use_cultivation = false;
    let off = Settlement::generate(7, &off_cfg);
    assert_ne!(
        on.canonical_bytes(),
        off.canonical_bytes(),
        "the own_use_cultivation flag + thresholds must be part of the identity"
    );

    // With cultivation off, the unused thresholds cannot steer a future tick, so
    // varying one must NOT split the digest.
    let mut off2 = off_cfg.clone();
    off2.chain.as_mut().expect("chain").cultivate_consume = 99;
    off2.chain.as_mut().expect("chain").cultivate_patience = 17;
    assert_eq!(
        off.canonical_bytes(),
        Settlement::generate(7, &off2).canonical_bytes(),
        "with cultivation off the unused thresholds must not steer the digest"
    );

    // A non-cultivation chain (no Cultivate recipe) likewise ignores the thresholds.
    let base = Settlement::generate(7, &SettlementConfig::frontier_forage_capacity());
    let mut base_cfg = SettlementConfig::frontier_forage_capacity();
    base_cfg.chain.as_mut().expect("chain").cultivate_consume = 42;
    assert_eq!(
        base.canonical_bytes(),
        Settlement::generate(7, &base_cfg).canonical_bytes(),
        "off the cultivation path the cultivate thresholds must not steer the digest"
    );
}

#[test]
fn canonical_bytes_include_cultivation_sells_surplus() {
    // S16: the money-from-produced-bread gate steers the buy/sell split (who
    // forages/cultivates) and the provenance ledger, so a flag-on config must digest
    // apart from the same config with the flag off.
    let on = Settlement::generate(7, &SettlementConfig::frontier_money_from_cultivation());
    let mut off_cfg = SettlementConfig::frontier_money_from_cultivation();
    off_cfg
        .chain
        .as_mut()
        .expect("chain")
        .cultivation_sells_surplus = false;
    let off = Settlement::generate(7, &off_cfg);
    assert_ne!(
        on.canonical_bytes(),
        off.canonical_bytes(),
        "the cultivation_sells_surplus flag must be part of the identity"
    );

    // Off the cultivation path (no Cultivate recipe) the flag is inert: it composes on
    // own-use cultivation, so toggling it on a forage-only config must NOT split the
    // digest — preserving the pre-S16 layout for every existing config.
    let base = Settlement::generate(7, &SettlementConfig::frontier_forage_capacity());
    let mut base_cfg = SettlementConfig::frontier_forage_capacity();
    base_cfg
        .chain
        .as_mut()
        .expect("chain")
        .cultivation_sells_surplus = true;
    assert_eq!(
        base.canonical_bytes(),
        Settlement::generate(7, &base_cfg).canonical_bytes(),
        "off the cultivation path the cultivation_sells_surplus flag must not steer the digest"
    );
}

#[test]
fn canonical_bytes_include_multigood_money() {
    // S18: the multi-good money gate routes the non-lineage gatherers (the woodcutters) to
    // the WOOD node instead of round-robin, so a flag-on config must digest apart from the
    // same config with the flag off.
    let on = Settlement::generate(7, &SettlementConfig::frontier_multigood());
    let mut off_cfg = SettlementConfig::frontier_multigood();
    off_cfg.chain.as_mut().expect("chain").multigood_money = false;
    let off = Settlement::generate(7, &off_cfg);
    assert_ne!(
        on.canonical_bytes(),
        off.canonical_bytes(),
        "the multigood_money flag must be part of the identity (the woodcutter routing)"
    );

    // Off the money-from-produced-bread path the flag is inert: it composes on
    // `cultivation_sells_surplus`, so toggling it on a forage-only config must NOT split
    // the digest — preserving the pre-S18 layout for every existing config.
    let base = Settlement::generate(7, &SettlementConfig::frontier_forage_capacity());
    let mut base_cfg = SettlementConfig::frontier_forage_capacity();
    base_cfg.chain.as_mut().expect("chain").multigood_money = true;
    assert_eq!(
        base.canonical_bytes(),
        Settlement::generate(7, &base_cfg).canonical_bytes(),
        "off the money-from-produced-bread path the multigood_money flag must not steer the digest"
    );
}

#[test]
fn canonical_bytes_exclude_multigood_instrumentation() {
    // S18 (the digest tripwire): the multi-good money instrumentation (the WOOD source
    // bound + the pending-indirect-SALT round-trip ledger) is a runtime-only diagnostic —
    // it must NEVER enter canonical_bytes, or it would shift the digest of any barter
    // scenario the ledger runs on (it now traces ALL emergent economies) and break their
    // goldens. Mutating it leaves the bytes byte-identical.
    let mut s = Settlement::generate(1, &SettlementConfig::frontier_multigood());
    s.run(80); // exercise the WOOD haul + barter so the counters are non-zero
    assert!(
        s.wood_gathered_total() > 0,
        "the woodcutters must have gathered WOOD so the tripwire is non-vacuous"
    );
    let before = s.canonical_bytes();
    s.multigood.wood_gathered = s.multigood.wood_gathered.wrapping_add(1);
    s.multigood.wood_for_salt = s.multigood.wood_for_salt.wrapping_add(7);
    s.multigood.indirect_accepted = s.multigood.indirect_accepted.wrapping_add(3);
    s.multigood.indirect_spent_on_target = s.multigood.indirect_spent_on_target.wrapping_add(2);
    s.multigood.pending.insert((AgentId(0), WOOD), 5);
    assert_eq!(
        before,
        s.canonical_bytes(),
        "the multi-good instrumentation must NOT enter canonical_bytes (the digest tripwire)"
    );

    // It must not steer the digest on an existing emergence scenario either (the round-trip
    // ledger runs there too).
    let mut e = Settlement::generate(2, &SettlementConfig::frontier_coemergent_strong());
    e.run(60);
    let e_before = e.canonical_bytes();
    e.multigood.indirect_accepted = e.multigood.indirect_accepted.wrapping_add(9);
    assert_eq!(
        e_before,
        e.canonical_bytes(),
        "the round-trip ledger must not steer the digest on the strong-bar scenario"
    );
}

#[test]
fn round_trip_ledger_credits_decrements_and_detects_hoarding() {
    // S18: the pending-indirect-SALT round-trip ledger's core rule. A credit (accept the
    // medium IndirectFor{target}) raises pending + the accept-side total; a spend on that
    // target draws the lesser of the spend and the pending (only the means role completes),
    // raising the spent total; a spend with nothing earmarked is inert; and the fraction is
    // `spent / accepted` — `0` with nothing accepted (no division by zero), and the
    // HOARDING signature (accepted > 0, spent ≈ 0) reads as fraction 0.
    let a = AgentId(1);
    let mut m = MultigoodMoney::default();

    // Hoarding: credit without ever spending on the target.
    m.credit_indirect(a, WOOD, 10);
    assert_eq!(m.pending_of(a, WOOD), 10);
    assert_eq!(m.indirect_accepted, 10);
    assert_eq!(m.indirect_spent_on_target, 0);
    assert_eq!(
        m.round_trip_fraction_bps(),
        0,
        "accepted but not spent on target => round-trip 0 (the hoarding signature)"
    );

    // A spend on the WRONG target is inert (nothing earmarked there).
    m.spend_on_target(a, SALT, 4);
    assert_eq!(m.indirect_spent_on_target, 0, "wrong-target spend is inert");
    assert_eq!(m.pending_of(a, WOOD), 10);

    // A spend on the earmarked target draws the round-trip leg.
    m.spend_on_target(a, WOOD, 4);
    assert_eq!(m.indirect_spent_on_target, 4);
    assert_eq!(m.pending_of(a, WOOD), 6);
    assert_eq!(m.round_trip_fraction_bps(), 4000, "4 of 10 spent => 40%");

    // A spend beyond the standing pending caps at it (the excess is ordinary spending).
    m.spend_on_target(a, WOOD, 99);
    assert_eq!(m.indirect_spent_on_target, 10);
    assert_eq!(m.pending_of(a, WOOD), 0);
    assert_eq!(
        m.round_trip_fraction_bps(),
        10_000,
        "fully round-tripped => 100%"
    );

    // Nothing accepted at all => fraction 0, no division by zero.
    let empty = MultigoodMoney::default();
    assert_eq!(empty.round_trip_fraction_bps(), 0);
}

#[test]
fn bread_provenance_draws_produced_first_and_preserves_origin() {
    // The ledger's core rule: a debit draws produced-origin FIRST, a transfer moves the
    // drawn produced units to the receiver (so a resold PRODUCED loaf stays produced —
    // the resold-bought-bread case), a sink draws to `produced_sunk`, and the whole-run
    // identity `credited == sunk + held` holds throughout.
    let mut bp = BreadProvenance::default();
    let drawn = |lots: &[ProducedLot]| -> u64 { lots.iter().map(|lot| lot.qty).sum() };
    let (a, b, c) = (AgentId(1), AgentId(2), AgentId(3));
    bp.credit_produced(a, 5, true);
    assert_eq!(bp.produced_credited, 5);
    assert_eq!(bp.total_held(), 5);
    // a SELLS 3 produced loaves to b (a transfer): origin preserved at the buyer.
    assert_eq!(drawn(&bp.transfer(a, b, 3)), 3);
    assert_eq!(bp.produced.get(&a).copied().unwrap_or(0), 2);
    assert_eq!(bp.produced.get(&b).copied().unwrap_or(0), 3);
    // b RESELLS the 3 bought loaves to c — produced origin is preserved (not minted).
    assert_eq!(drawn(&bp.transfer(b, c, 3)), 3);
    assert_eq!(bp.produced.get(&c).copied().unwrap_or(0), 3);
    // c eats 2 (a sink) — produced-first to the sunk counter.
    assert_eq!(bp.sink(c, 2), 2);
    assert_eq!(bp.produced_sunk, 2);
    assert_eq!(bp.produced_credited, bp.produced_sunk + bp.total_held());
    // S22a: the produced lots mirror the flat balance exactly through every move.
    assert_eq!(bp.produced.get(&c).copied().unwrap_or(0), 1);
    assert_eq!(
        bp.produced_lots
            .get(&c)
            .map(|q| q.iter().map(|lot| lot.qty).sum::<u64>())
            .unwrap_or(0),
        1,
        "the class-tagged lots stay in lockstep with the flat produced balance"
    );
}

#[test]
fn bread_provenance_transfer_self_produced_filters_other_producers() {
    let mut bp = BreadProvenance::default();
    let (owner, other, worker) = (AgentId(1), AgentId(2), AgentId(3));

    bp.credit_produced(owner, 3, true);
    bp.credit_produced(other, 4, false);
    bp.transfer(other, owner, 4);

    assert_eq!(bp.produced.get(&owner).copied(), Some(7));
    assert_eq!(bp.transfer_self_produced(owner, worker, 2), 2);
    assert_eq!(bp.produced.get(&worker).copied(), Some(2));
    assert_eq!(bp.produced.get(&owner).copied(), Some(5));

    assert_eq!(
        bp.transfer_self_produced(owner, worker, 5),
        1,
        "only the remaining lot whose producer is the owner may fund the advance"
    );
    assert_eq!(bp.produced.get(&worker).copied(), Some(3));
    assert_eq!(bp.produced.get(&owner).copied(), Some(4));
    let owner_other_produced: u64 = bp
        .produced_lots
        .get(&owner)
        .expect("owner still holds the other producer's lots")
        .iter()
        .filter(|lot| lot.producer == other)
        .map(|lot| lot.qty)
        .sum();
    assert_eq!(owner_other_produced, 4);
    assert_eq!(bp.produced_credited, bp.produced_sunk + bp.total_held());
}

#[test]
fn acquisition_transfer_preserve_keeps_fifo_lot_order() {
    // Origin-preserving moves (birth, inheritance) must keep the donor's actual FIFO order.
    // If the recipient consumes only part of a mixed transfer, older seeded food must be
    // debited before newer bought food.
    let mut ledger = AcquisitionLedger::default();
    let (donor, recipient) = (AgentId(1), AgentId(2));

    ledger.credit(donor, FoodChannel::SeededMinted, 2);
    ledger.credit(donor, FoodChannel::Bought, 3);
    ledger.transfer_preserve(donor, recipient, 4);
    ledger.consume(recipient, 2);

    assert_eq!(
        ledger.consumed_by_channel[FoodChannel::SeededMinted.index()],
        2,
        "the recipient must eat the older seeded lot first"
    );
    assert_eq!(
        ledger.consumed_by_channel[FoodChannel::Bought.index()],
        0,
        "bought food must not jump ahead of older seeded food"
    );

    ledger.consume(recipient, 1);
    assert_eq!(
        ledger.consumed_by_channel[FoodChannel::Bought.index()],
        1,
        "after the seeded lot is exhausted, the transferred bought lot is next"
    );
}

#[test]
fn acquisition_intervention_flag_survives_partial_draw_in_fifo_order() {
    // C3R.e: a MIXED-ORIGIN FIFO — an intervention-flagged SeededMinted lot ahead of a plain
    // Bought lot. A partial origin-preserving transfer (birth/inheritance) draws oldest-first
    // and KEEPS each lot's intervention flag; the global intervention-held read tracks exactly
    // the flagged units as they move.
    let mut ledger = AcquisitionLedger::default();
    let (donor, heir) = (AgentId(1), AgentId(2));
    ledger.credit_intervention(donor, FoodChannel::SeededMinted, 3);
    ledger.credit(donor, FoodChannel::Bought, 4);
    assert_eq!(ledger.intervention_held(), 3);

    // The heir inherits 4 units oldest-first: all 3 intervention + 1 plain Bought.
    let drawn = ledger.transfer_preserve(donor, heir, 4);
    assert_eq!(
        drawn,
        vec![
            FoodLot {
                channel: FoodChannel::SeededMinted,
                qty: 3,
                intervention: true,
                identity: None,
                taint: false,
            },
            FoodLot {
                channel: FoodChannel::Bought,
                qty: 1,
                intervention: false,
                identity: None,
                taint: false,
            },
        ],
        "the drawn breakdown preserves FIFO order and each lot's origin flag"
    );
    // The flag moved with the units — total unchanged, now held by the heir, not the donor.
    assert_eq!(ledger.intervention_held(), 3);
    assert_eq!(ledger.intervention_held_by(&BTreeSet::from([heir])), 3);
    assert_eq!(ledger.intervention_held_by(&BTreeSet::from([donor])), 0);
}

#[test]
fn acquisition_market_retag_preserves_order_and_intervention_flag() {
    // C3R.e: the P0 laundering guard for the market-sale retag. A seller holds a MIXED FIFO —
    // two adjacent PLAIN lots, then an intervention lot. Selling all of it retags every unit to
    // Bought but must KEEP each unit's intervention flag, mapping lots in original FIFO order
    // and coalescing ONLY adjacent equal-origin lots: the two plains merge, the intervention
    // run stays separate. A channel-partition would fold the flagged units into the plain run
    // and launder the origin.
    let mut ledger = AcquisitionLedger::default();
    let (seller, buyer) = (AgentId(1), AgentId(2));
    ledger.credit(seller, FoodChannel::SeededMinted, 2);
    ledger.credit(seller, FoodChannel::Bought, 2);
    ledger.credit_intervention(seller, FoodChannel::SeededMinted, 3);
    assert_eq!(ledger.intervention_held(), 3);

    let drawn = ledger.transfer_as_bought(seller, buyer, 7);
    // The per-channel breakdown of what left the seller (the pre-flag contract, unchanged).
    assert_eq!(drawn[FoodChannel::SeededMinted.index()], 5);
    assert_eq!(drawn[FoodChannel::Bought.index()], 2);

    // The buyer's lots: all Bought channel; the two adjacent PLAIN lots coalesce to one, the
    // intervention run stays SEPARATE and in FIFO order — never one laundered Bought lot.
    assert_eq!(
        ledger.lots.get(&buyer),
        Some(&VecDeque::from([
            FoodLot {
                channel: FoodChannel::Bought,
                qty: 4,
                intervention: false,
                identity: None,
                taint: false,
            },
            FoodLot {
                channel: FoodChannel::Bought,
                qty: 3,
                intervention: true,
                identity: None,
                taint: false,
            },
        ])),
        "the retag preserves FIFO order + the origin flag, coalescing only adjacent equal-origin"
    );
    // Resale-proof: the intervention units are still tracked after leaving the cohort as Bought.
    assert_eq!(ledger.intervention_held(), 3);
    assert_eq!(ledger.intervention_held_by(&BTreeSet::from([buyer])), 3);

    // A mixed partial CONSUME still eats oldest-first across the retagged run: the 4 plain
    // first, then one unit into the intervention run.
    ledger.consume(buyer, 5);
    assert_eq!(
        ledger.intervention_held(),
        2,
        "after the 4 plain units, one intervention unit is consumed"
    );
}

#[test]
fn acquisition_transfer_self_produced_skips_older_channels() {
    let mut ledger = AcquisitionLedger::default();
    let (donor, recipient) = (AgentId(1), AgentId(2));

    ledger.credit(donor, FoodChannel::SeededMinted, 2);
    ledger.credit(donor, FoodChannel::Bought, 3);
    ledger.credit(donor, FoodChannel::SelfProduced, 4);

    assert_eq!(ledger.transfer_self_produced(donor, recipient, 3), 3);
    assert_eq!(
        ledger.held_by_agent_channel(donor, FoodChannel::SeededMinted),
        2
    );
    assert_eq!(ledger.held_by_agent_channel(donor, FoodChannel::Bought), 3);
    assert_eq!(
        ledger.held_by_agent_channel(donor, FoodChannel::SelfProduced),
        1
    );
    assert_eq!(
        ledger.held_by_agent_channel(recipient, FoodChannel::SelfProduced),
        3,
        "an in-kind advance must not record older seeded/bought lots as the wage"
    );
    assert_eq!(
        ledger.held_by_agent_channel(recipient, FoodChannel::SeededMinted),
        0
    );
    assert_eq!(
        ledger.held_by_agent_channel(recipient, FoodChannel::Bought),
        0
    );
}

#[test]
fn bootstrap_buy_eat_bid_predicate_requires_current_eat_after_prior_buy() {
    let producer = AgentId(1);
    let mut trace = BootstrapTrace::default();

    trace.observe_food_eat(producer, 10);
    trace.observe_food_buy(producer, 10);
    assert!(
        !trace.bought_then_ate_on_tick(producer, 11, false),
        "a buy followed by no current-tick eat is not buy -> eat -> bid"
    );

    trace.observe_food_buy(producer, 11);
    assert!(
        !trace.bought_then_ate_on_tick(producer, 11, true),
        "an eat before a same-tick buy must not count as buy -> eat -> bid"
    );

    assert!(
        trace.bought_then_ate_on_tick(producer, 12, true),
        "a prior-tick buy plus current-tick eat is the bootstrap leg"
    );
}

#[test]
fn bread_provenance_does_not_misattribute_minted_bread() {
    // A minted/buffer loaf is NEVER credited produced, so it sits in the residual
    // other-origin pool: drawing it (sale or resale) yields ZERO produced — attributed
    // minted, never mis-attributed produced. A mixed holder draws produced-first.
    let mut bp = BreadProvenance::default();
    let drawn = |lots: &[ProducedLot]| -> u64 { lots.iter().map(|lot| lot.qty).sum() };
    let (holder, buyer, third) = (AgentId(1), AgentId(2), AgentId(3));
    // holder sells 4 MINTED loaves (none ever credited produced): 0 produced drawn.
    assert_eq!(drawn(&bp.transfer(holder, buyer, 4)), 0);
    assert_eq!(bp.produced.get(&buyer).copied().unwrap_or(0), 0);
    // the buyer reselling that minted bread is likewise not mis-attributed.
    assert_eq!(drawn(&bp.transfer(buyer, third, 4)), 0);
    // a MIXED holder (2 produced + minted residual): a 5-loaf sale draws the 2 produced.
    bp.credit_produced(holder, 2, true);
    assert_eq!(drawn(&bp.transfer(holder, buyer, 5)), 2);
    assert_eq!(bp.produced.get(&holder).copied().unwrap_or(0), 0);
    assert_eq!(bp.produced.get(&buyer).copied().unwrap_or(0), 2);
    // estate→commons drops a dead agent's produced bread to the sink (conserved exit).
    bp.credit_produced(third, 3, true);
    assert_eq!(bp.drop_to_sink(third), 3);
    assert_eq!(bp.produced_credited, bp.produced_sunk + bp.total_held());
}

#[test]
fn bread_provenance_tracks_spot_bread_sales() {
    // Post-promotion V2 money trades live on the spot tape, not the barter tape. The
    // provenance pass must still transfer produced-origin bread from seller to buyer
    // after the market has moved the physical stock.
    let mut s = Settlement::generate(7, &SettlementConfig::frontier_money_from_cultivation());
    let bread = s.provenance_bread_good().expect("chain bread");
    let agents = s
        .society
        .agents
        .iter()
        .map(|agent| agent.id)
        .take(2)
        .collect::<Vec<_>>();
    let seller = agents[0];
    let buyer = agents[1];

    s.bread_provenance = BreadProvenance::default();
    s.bread_provenance.credit_produced(seller, 3, true);
    s.society
        .agents
        .get_mut(seller)
        .expect("seller")
        .stock
        .add(bread, 3);

    let barter_start = s.society.barter_trades.len();
    let spot_start = s.society.trades.len();

    // Mirror a filled spot ask for two bread: stock has already moved, and the spot
    // tape is the only record the provenance pass can read.
    assert!(
        s.society
            .agents
            .get_mut(seller)
            .expect("seller")
            .stock
            .remove(bread, 2),
        "seller starts with enough bread"
    );
    s.society
        .agents
        .get_mut(buyer)
        .expect("buyer")
        .stock
        .add(bread, 2);
    s.society.trades.push(econ::market::Trade {
        tick: s.econ_tick,
        good: bread,
        buyer,
        seller,
        price: Gold(1),
        qty: 2,
    });

    s.run_bread_provenance_market(barter_start, spot_start, false);

    assert_eq!(s.bread_provenance.produced.get(&seller).copied(), Some(1));
    assert_eq!(s.bread_provenance.produced.get(&buyer).copied(), Some(2));
    assert!(
        s.bread_provenance_conserves(),
        "spot bread sales must preserve produced-origin conservation"
    );
}

#[test]
fn bread_provenance_attributes_medium_sale_split() {
    // The bread→medium attribution arm: a cleared sale splits produced vs minted by the
    // produced units the seller's debit drew, accumulates the pre-promotion-only split
    // (the causality probe), and latches the first produced bread→medium tick exactly
    // once. The shipped finding is principled-failure (SALT never promotes), so no live
    // promotion reaches this accumulation on the spot tape — `bread_provenance_tracks_-
    // spot_bread_sales` covers the spot-tape transfer wiring; this exercises the
    // produced/minted accumulation arm directly so it never runs untested.
    let mut bp = BreadProvenance::default();
    // A pre-promotion 5-loaf sale that drew 3 produced (a lineage producer's lot): 3
    // produced, 2 minted; pre-promotion split mirrors the run total, the first-produced
    // latch fires at tick 10. S22a: the produced 3 attribute to the LINEAGE class.
    let lineage_producer = AgentId(7);
    bp.attribute_medium_sale(
        &[ProducedLot {
            producer: lineage_producer,
            lineage: true,
            qty: 3,
        }],
        5,
        true,
        10,
    );
    assert_eq!((bp.salt_volume_produced, bp.salt_volume_minted), (3, 2));
    assert_eq!(
        (
            bp.pre_promotion_salt_volume_produced,
            bp.pre_promotion_salt_volume_minted
        ),
        (3, 2)
    );
    assert_eq!(bp.first_produced_bread_for_salt_tick, Some(10));
    // S22a: the produced volume + distinct seller attribute to the lineage class only.
    assert_eq!(
        (
            bp.salt_volume_produced_lineage,
            bp.salt_volume_produced_nonlineage
        ),
        (3, 0)
    );
    assert!(bp.lineage_salt_producers.contains(&lineage_producer));
    assert!(bp.nonlineage_salt_producers.is_empty());
    // A later, post-promotion 4-loaf sale: 1 produced by a NON-lineage entrant, 3 minted.
    // The whole-run produced/minted updates, the pre-promotion split is FROZEN, the
    // first-produced latch does not move, and the non-lineage class + seller is recorded.
    let nonlineage_producer = AgentId(9);
    bp.attribute_medium_sale(
        &[ProducedLot {
            producer: nonlineage_producer,
            lineage: false,
            qty: 1,
        }],
        4,
        false,
        20,
    );
    assert_eq!((bp.salt_volume_produced, bp.salt_volume_minted), (4, 5));
    assert_eq!(
        (
            bp.pre_promotion_salt_volume_produced,
            bp.pre_promotion_salt_volume_minted
        ),
        (3, 2),
        "the pre-promotion causality split freezes after promotion"
    );
    assert_eq!(
        bp.first_produced_bread_for_salt_tick,
        Some(10),
        "the first produced bread→medium latch fires exactly once"
    );
    // S22a: the post-promotion non-lineage produced unit attributes to the non-lineage
    // class + seller; the FROZEN pre-promotion class split stays lineage-only.
    assert_eq!(
        (
            bp.salt_volume_produced_lineage,
            bp.salt_volume_produced_nonlineage
        ),
        (3, 1)
    );
    assert_eq!(
        (
            bp.pre_promotion_salt_volume_produced_lineage,
            bp.pre_promotion_salt_volume_produced_nonlineage
        ),
        (3, 0),
        "the pre-promotion entrant-class split freezes after promotion"
    );
    assert!(bp.nonlineage_salt_producers.contains(&nonlineage_producer));
    assert_eq!(bp.lineage_salt_producers.len(), 1);
}

#[test]
fn intra_household_sale_reattaches_earned_gold_lots_to_seller() {
    let cfg = SettlementConfig::frontier_mortal_producers_earned();
    let mut s = Settlement::generate(7, &cfg);
    let bread = s.provenance_bread_good().expect("chain bread");
    let household = s
        .producer_household_start()
        .expect("earned base has producer households");
    let seller = s
        .live_colonist_slots
        .iter()
        .find_map(|&slot| {
            (s.colonists[slot].household == Some(household)).then_some(s.colonists[slot].id)
        })
        .expect("producer household has a founder");
    let buyer = s
        .society
        .agents
        .iter()
        .map(|agent| agent.id)
        .find(|&id| id != seller)
        .expect("earned base has another agent");
    let buyer_slot = s.slot_for_id(buyer).expect("buyer is a colonist");
    s.colonists[buyer_slot].household = Some(household);

    s.earned_provisioning.buckets.remove(&buyer);
    s.earned_provisioning.buckets.remove(&seller);
    s.credit_earned_provisioning_lot(
        buyer,
        EarnedGoldLot {
            source: EarnedGoldSource::Earned,
            amount: Gold(2),
        },
    );
    s.credit_earned_provisioning_lot(
        buyer,
        EarnedGoldLot {
            source: EarnedGoldSource::Endowed,
            amount: Gold(3),
        },
    );
    let spot_start = s.society.trades.len();
    s.society.trades.push(econ::market::Trade {
        tick: s.econ_tick,
        good: bread,
        buyer,
        seller,
        price: Gold(5),
        qty: 1,
    });

    s.run_earned_provisioning_market_attribution(spot_start);

    let (lots, untracked) = s.debit_earned_provisioning_lots(seller, Gold(5));
    assert_eq!(
        lots,
        vec![
            EarnedGoldLot {
                source: EarnedGoldSource::Earned,
                amount: Gold(2),
            },
            EarnedGoldLot {
                source: EarnedGoldSource::Endowed,
                amount: Gold(3),
            },
        ],
        "an intra-household sale must preserve the buyer's earned/endowed labels"
    );
    assert_eq!(untracked, Gold::ZERO);
    assert_eq!(s.earned_provisioning.stats.intra_household_sales, Gold(5));
    assert_eq!(
        s.earned_provisioning.stats.external_earned_revenue,
        Gold::ZERO
    );
    assert_eq!(
        s.earned_provisioning.stats.genuine_external_revenue,
        Gold::ZERO
    );
}

// C3R.e-obs Slice 0 (D1): the two non-bread (flour) earned-provisioning counter
// assertions carried forward as verified test debt. A hand-pushed spot tape moves NO
// actual gold, so "credited/debited" here means the earned-provisioning PROVENANCE lots:
// the FIFO ledger the attribution pass debits from the buyer and re-credits (as a fresh
// `Earned` lot) to a producer-house seller. Bread stock and every bread-only earned stat
// must stay untouched — a flour sale is class-tracked outside the bread split.
fn assert_earned_bread_stats_unchanged(
    before: &EarnedProvisioningStats,
    after: &EarnedProvisioningStats,
) {
    // The ONLY stats a flour sale may move are the two non-bread earned counters; project
    // `before` forward on exactly those and require byte-for-byte equality elsewhere, so
    // any leak into bread revenue, the bread-trade counts, or any other earned stat fails.
    let mut expected = *before;
    expected.non_bread_external_earned = after.non_bread_external_earned;
    expected.non_bread_producer_class_earned = after.non_bread_producer_class_earned;
    assert_eq!(
        expected, *after,
        "a flour sale must move only the non-bread earned counters"
    );
}

#[test]
fn flour_sale_to_external_buyer_credits_non_bread_external_earned() {
    let cfg = SettlementConfig::frontier_mortal_producers_earned();
    let mut s = Settlement::generate(7, &cfg);
    let flour = s.chain.as_ref().expect("chain").content.flour();
    let bread = s.provenance_bread_good().expect("chain bread");
    let household = s
        .producer_household_start()
        .expect("earned base has producer households");
    let seller = s
        .live_colonist_slots
        .iter()
        .find_map(|&slot| {
            (s.colonists[slot].household == Some(household)).then_some(s.colonists[slot].id)
        })
        .expect("producer household has a founder");
    let buyer = s
        .society
        .agents
        .iter()
        .map(|agent| agent.id)
        .find(|&id| id != seller)
        .expect("earned base has another agent");
    // A genuinely external buyer: not in any producer household.
    let buyer_slot = s.slot_for_id(buyer).expect("buyer is a colonist");
    s.colonists[buyer_slot].household = None;

    s.earned_provisioning.buckets.remove(&buyer);
    s.earned_provisioning.buckets.remove(&seller);
    s.credit_earned_provisioning_lot(
        buyer,
        EarnedGoldLot {
            source: EarnedGoldSource::Earned,
            amount: Gold(2),
        },
    );
    s.credit_earned_provisioning_lot(
        buyer,
        EarnedGoldLot {
            source: EarnedGoldSource::Endowed,
            amount: Gold(3),
        },
    );

    let bread_stock_before: u64 = s
        .society
        .agents
        .iter()
        .map(|agent| u64::from(agent.stock.get(bread)))
        .sum();
    let bread_stats_before = s.earned_provisioning.stats;

    let spot_start = s.society.trades.len();
    s.society.trades.push(econ::market::Trade {
        tick: s.econ_tick,
        good: flour,
        buyer,
        seller,
        price: Gold(5),
        qty: 1,
    });
    s.run_earned_provisioning_market_attribution(spot_start);

    // The external non-bread counter takes the full sale; its producer-class sibling
    // stays put.
    assert_eq!(
        s.earned_provisioning.stats.non_bread_external_earned,
        Gold(5)
    );
    assert_eq!(
        s.earned_provisioning.stats.non_bread_producer_class_earned,
        Gold::ZERO
    );
    // The seller is re-credited a single fresh `Earned` lot of the sale value.
    let (seller_lots, seller_untracked) = s.debit_earned_provisioning_lots(seller, Gold(5));
    assert_eq!(
        seller_lots,
        vec![EarnedGoldLot {
            source: EarnedGoldSource::Earned,
            amount: Gold(5),
        }],
        "a flour sale credits the producer-house seller a fresh Earned lot"
    );
    assert_eq!(seller_untracked, Gold::ZERO);
    // The buyer's lots are FIFO-debited to empty by the sale.
    let (buyer_lots, buyer_untracked) = s.debit_earned_provisioning_lots(buyer, Gold(5));
    assert!(
        buyer_lots.is_empty(),
        "the buyer's earned-provisioning lots are fully debited by the flour sale"
    );
    assert_eq!(buyer_untracked, Gold(5));
    // Nothing bread moved: neither stock nor any bread-only earned statistic.
    let bread_stock_after: u64 = s
        .society
        .agents
        .iter()
        .map(|agent| u64::from(agent.stock.get(bread)))
        .sum();
    assert_eq!(
        bread_stock_before, bread_stock_after,
        "bread stock is untouched"
    );
    assert_earned_bread_stats_unchanged(&bread_stats_before, &s.earned_provisioning.stats);
}

#[test]
fn flour_sale_to_producer_class_buyer_credits_non_bread_producer_class_earned() {
    let cfg = SettlementConfig::frontier_mortal_producers_earned();
    let mut s = Settlement::generate(7, &cfg);
    let flour = s.chain.as_ref().expect("chain").content.flour();
    let bread = s.provenance_bread_good().expect("chain bread");
    let seller_household = s
        .producer_household_start()
        .expect("earned base has producer households");
    let seller = s
        .live_colonist_slots
        .iter()
        .find_map(|&slot| {
            (s.colonists[slot].household == Some(seller_household)).then_some(s.colonists[slot].id)
        })
        .expect("producer household has a founder");
    let buyer = s
        .society
        .agents
        .iter()
        .map(|agent| agent.id)
        .find(|&id| id != seller)
        .expect("earned base has another agent");
    // A producer-CLASS buyer: a member of ANOTHER producer household.
    let buyer_slot = s.slot_for_id(buyer).expect("buyer is a colonist");
    s.colonists[buyer_slot].household = Some(seller_household + 1);
    assert!(s.is_producer_household(seller_household + 1));

    s.earned_provisioning.buckets.remove(&buyer);
    s.earned_provisioning.buckets.remove(&seller);
    s.credit_earned_provisioning_lot(
        buyer,
        EarnedGoldLot {
            source: EarnedGoldSource::Earned,
            amount: Gold(2),
        },
    );
    s.credit_earned_provisioning_lot(
        buyer,
        EarnedGoldLot {
            source: EarnedGoldSource::Endowed,
            amount: Gold(3),
        },
    );

    let bread_stock_before: u64 = s
        .society
        .agents
        .iter()
        .map(|agent| u64::from(agent.stock.get(bread)))
        .sum();
    let bread_stats_before = s.earned_provisioning.stats;

    let spot_start = s.society.trades.len();
    s.society.trades.push(econ::market::Trade {
        tick: s.econ_tick,
        good: flour,
        buyer,
        seller,
        price: Gold(5),
        qty: 1,
    });
    s.run_earned_provisioning_market_attribution(spot_start);

    // The producer-class non-bread counter takes the full sale; the external sibling
    // stays put.
    assert_eq!(
        s.earned_provisioning.stats.non_bread_producer_class_earned,
        Gold(5)
    );
    assert_eq!(
        s.earned_provisioning.stats.non_bread_external_earned,
        Gold::ZERO
    );
    let (seller_lots, seller_untracked) = s.debit_earned_provisioning_lots(seller, Gold(5));
    assert_eq!(
        seller_lots,
        vec![EarnedGoldLot {
            source: EarnedGoldSource::Earned,
            amount: Gold(5),
        }],
        "a producer-class flour sale still credits the seller a fresh Earned lot"
    );
    assert_eq!(seller_untracked, Gold::ZERO);
    let (buyer_lots, buyer_untracked) = s.debit_earned_provisioning_lots(buyer, Gold(5));
    assert!(
        buyer_lots.is_empty(),
        "the producer-class buyer's lots are fully debited by the flour sale"
    );
    assert_eq!(buyer_untracked, Gold(5));
    let bread_stock_after: u64 = s
        .society
        .agents
        .iter()
        .map(|agent| u64::from(agent.stock.get(bread)))
        .sum();
    assert_eq!(
        bread_stock_before, bread_stock_after,
        "bread stock is untouched"
    );
    assert_earned_bread_stats_unchanged(&bread_stats_before, &s.earned_provisioning.stats);
}

#[test]
#[should_panic(expected = "birth-stock motive and sufficiency control must be mutually exclusive")]
fn birth_stock_modes_reject_coactivation() {
    let mut cfg = SettlementConfig::frontier_mortal_producers_saving();
    cfg.chain.as_mut().expect("chain").birth_stock_saving_mode =
        BirthStockSavingMode::SufficiencyControl;
    let _ = Settlement::generate(7, &cfg);
}

#[test]
fn birth_stock_motive_emits_the_full_target_below_cap() {
    let cfg = SettlementConfig::frontier_mortal_producers_saving();
    let mut s = Settlement::generate(7, &cfg);
    s.regenerate_scales();
    let staple = s.known.hunger;
    let target = s
        .demography
        .as_ref()
        .expect("demography")
        .child_food_endowment;
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
        assert_eq!(wants, target as usize, "the full target stays reserved");
    }
    assert!(eligible > 0);
    assert_eq!(
        s.birth_stock_wants_emitted,
        (eligible as u64) * u64::from(target)
    );
}

#[test]
fn birth_stock_motive_emits_nothing_at_the_household_cap() {
    let mut cfg = SettlementConfig::frontier_mortal_producers_saving();
    cfg.chain.as_mut().expect("chain").producer_house_cap = 1;
    let mut s = Settlement::generate(7, &cfg);
    s.regenerate_scales();
    let staple = s.known.hunger;
    let mut producer_members = 0;
    for &slot in &s.live_colonist_slots {
        let colonist = &s.colonists[slot];
        if !colonist
            .household
            .is_some_and(|household| s.is_producer_household(household))
        {
            continue;
        }
        producer_members += 1;
        assert!(!s
            .society
            .agents
            .get(colonist.id)
            .expect("agent")
            .scale
            .iter()
            .any(|want| {
                want.kind == WantKind::Good(staple) && matches!(want.horizon, Horizon::Next)
            }));
    }
    assert!(producer_members > 0);
    assert_eq!(s.birth_stock_wants_emitted, 0);
}

#[test]
fn attribution_counts_next_bread_buys_but_not_unprovided_now_buys() {
    let mut s = Settlement::generate(7, &SettlementConfig::frontier_mortal_producers_saving());
    let staple = s.known.hunger;
    let mut producer_ids = s
        .live_colonist_slots
        .iter()
        .map(|&slot| &s.colonists[slot])
        .filter(|colonist| {
            colonist
                .household
                .is_some_and(|household| s.is_producer_household(household))
        })
        .map(|colonist| colonist.id);
    let next_buyer = producer_ids.next().expect("first producer-house buyer");
    let now_buyer = producer_ids.next().expect("second producer-house buyer");
    let seller = s
        .society
        .agents
        .iter()
        .map(|agent| agent.id)
        .find(|&id| id != next_buyer && id != now_buyer)
        .expect("seller");
    for (buyer, horizon) in [(next_buyer, Horizon::Next), (now_buyer, Horizon::Now)] {
        let held = s.stock_of_id(buyer, staple);
        assert!(s.society.debit_stock(
            buyer,
            staple,
            u32::try_from(held).expect("test stock fits u32")
        ));
        s.society.agents.get_mut(buyer).expect("buyer").scale = vec![Want {
            kind: WantKind::Good(staple),
            horizon,
            qty: 1,
            satisfied: false,
        }];
    }
    let snapshot = s.birth_stock_attribution_snapshot();
    assert!(snapshot.contains(&next_buyer));
    assert!(!snapshot.contains(&now_buyer));
    let trade_start = s.society.trades.len();
    for buyer in [next_buyer, now_buyer] {
        s.society.trades.push(econ::market::Trade {
            tick: s.econ_tick,
            good: staple,
            buyer,
            seller,
            price: Gold(1),
            qty: 1,
        });
    }
    s.record_birth_stock_attributable_purchases(trade_start, &snapshot);
    assert_eq!(s.birth_stock_attributable_purchases, 1);
}

#[test]
fn reached_four_excludes_seeded_stock_until_it_is_reaccumulated() {
    let mut s = Settlement::generate(7, &SettlementConfig::frontier_mortal_producers_saving());
    let staple = s.known.hunger;
    let target = s
        .demography
        .as_ref()
        .expect("demography")
        .child_food_endowment;
    let producer = s
        .live_colonist_slots
        .iter()
        .find_map(|&slot| {
            let colonist = &s.colonists[slot];
            colonist
                .household
                .is_some_and(|household| s.is_producer_household(household))
                .then_some(colonist.id)
        })
        .expect("producer household member");

    s.observe_birth_stock_holdings();
    assert_eq!(
        s.birth_stock_reached_agents.len(),
        0,
        "the founders' seeded bread is not accumulated birth stock"
    );
    assert!(
        s.birth_stock_held_max() < target,
        "seeded above-target stock must not inflate held_max"
    );

    let held = s
        .society
        .agents
        .get(producer)
        .expect("producer agent")
        .stock
        .get(staple);
    assert!(held >= target);
    assert!(s.society.debit_stock(producer, staple, held - target + 1));
    s.observe_birth_stock_holdings();
    assert!(s.society.credit_stock(producer, staple, 1));
    s.observe_birth_stock_holdings();
    assert_eq!(s.birth_stock_reached_agents.len(), 1);
    assert_eq!(
        s.birth_stock_held_max(),
        target,
        "held_max reflects the stock reaccumulated from below, not seeded stock"
    );
}

#[test]
fn held_at_death_excludes_seeded_stock_until_it_is_reaccumulated() {
    let mut s = Settlement::generate(7, &SettlementConfig::frontier_mortal_producers_saving());
    let staple = s.known.hunger;
    let target = s
        .demography
        .as_ref()
        .expect("demography")
        .child_food_endowment;
    let producer = s
        .live_colonist_slots
        .iter()
        .find_map(|&slot| {
            let colonist = &s.colonists[slot];
            colonist
                .household
                .is_some_and(|household| s.is_producer_household(household))
                .then_some(colonist.id)
        })
        .expect("producer household member");

    // A founder holding its seeded endowment (at/above the target) that has
    // never accumulated from below must not be attributed to the motive.
    let seeded = s.society.free_stock_after_all_reserves(producer, staple);
    assert!(seeded >= target);
    s.record_producer_house_death(producer);
    assert_eq!(
        s.birth_stock_held_at_death, 0,
        "seeded stock is not saved birth stock at death"
    );

    // Once the member drops below the target and is observed there, a later
    // death records the stock it genuinely holds.
    assert!(s
        .society
        .debit_stock(producer, staple, seeded - (target - 1)));
    s.observe_birth_stock_holdings();
    s.record_producer_house_death(producer);
    assert_eq!(s.birth_stock_held_at_death, target - 1);
}

#[test]
fn project_input_override_leaves_birth_stock_bids_for_both_active_stages() {
    let cfg = SettlementConfig::frontier_mortal_producers_saving();
    let mut s = Settlement::generate(7, &cfg);
    let chain = s.chain.as_ref().expect("chain");
    let (grain, flour, bread) = (
        chain.content.grain(),
        chain.content.flour(),
        chain.content.bread(),
    );
    fn seed_realized_price(s: &mut Settlement, good: GoodId, seller: AgentId, buyer: AgentId) {
        let ids = s
            .society
            .agents
            .iter()
            .map(|agent| agent.id)
            .collect::<Vec<_>>();
        for &id in &ids {
            s.society.agents.get_mut(id).expect("agent").scale = vec![Want {
                kind: WantKind::Leisure,
                horizon: Horizon::Now,
                qty: 1,
                satisfied: false,
            }];
        }
        let seller_agent = s.society.agents.get_mut(seller).expect("seller");
        let held = seller_agent.stock.get(good);
        assert!(seller_agent.stock.remove(good, held));
        seller_agent.stock.add(good, 1);
        seller_agent.gold = Gold::ZERO;
        seller_agent.scale = vec![Want {
            kind: WantKind::Good(GOLD),
            horizon: Horizon::Later(4),
            qty: 1,
            satisfied: false,
        }];
        let buyer_agent = s.society.agents.get_mut(buyer).expect("buyer");
        let held = buyer_agent.stock.get(good);
        assert!(buyer_agent.stock.remove(good, held));
        buyer_agent.gold = Gold(10);
        buyer_agent.scale = vec![Want {
            kind: WantKind::Good(good),
            horizon: Horizon::Next,
            qty: 1,
            satisfied: false,
        }];
        s.society.cancel_changed_live_quotes_for_agents(&ids);
        s.society.step();
        assert!(s.society.realized_price(good).is_some());
    }

    let price_agents = s
        .society
        .agents
        .iter()
        .map(|agent| agent.id)
        .take(2)
        .collect::<Vec<_>>();
    seed_realized_price(&mut s, flour, price_agents[0], price_agents[1]);
    seed_realized_price(&mut s, bread, price_agents[0], price_agents[1]);
    let miller_slot = s
        .live_colonist_slots
        .iter()
        .copied()
        .find(|&slot| s.colonists[slot].latent == Some(RecipeId::Mill))
        .expect("latent Miller");
    let baker_slot = s
        .live_colonist_slots
        .iter()
        .copied()
        .find(|&slot| s.colonists[slot].latent == Some(RecipeId::Bake))
        .expect("latent Baker");
    s.colonists[miller_slot].vocation = Vocation::Miller;
    s.colonists[baker_slot].vocation = Vocation::Baker;
    let (miller, baker) = (s.colonists[miller_slot].id, s.colonists[baker_slot].id);
    s.chain.as_mut().expect("chain").producer_house_cap = u8::MAX;
    for id in [miller, baker] {
        let slot = s.slot_for_id(id).expect("active producer is a colonist");
        s.colonists[slot].need.hunger = 0;
        let agent = s.society.agents.get_mut(id).expect("active producer");
        let held_bread = agent.stock.get(bread);
        assert!(agent.stock.remove(bread, held_bread));
        // Three input units reserve first at the seeded unit price, leaving only
        // one gold for the lower-ranked birth-stock block. This makes the test
        // sensitive to the real input-first reservation ordering.
        agent.gold = Gold(4);
    }
    let miller_flour = s
        .society
        .agents
        .get(miller)
        .expect("miller")
        .stock
        .get(flour);
    assert!(s.society.debit_stock(miller, flour, miller_flour));
    let miller_grain = s
        .society
        .agents
        .get(miller)
        .expect("miller")
        .stock
        .get(grain);
    assert!(s.society.debit_stock(miller, grain, miller_grain));
    let baker_flour = s.society.agents.get(baker).expect("baker").stock.get(flour);
    assert!(s.society.debit_stock(baker, flour, baker_flour));
    let baker_bread = s.society.agents.get(baker).expect("baker").stock.get(bread);
    assert!(s.society.debit_stock(baker, bread, baker_bread));
    s.regenerate_scales();
    s.set_project_input_bid_overrides();
    let trade_start = s.society.trades.len();
    s.society.step();
    let posted_or_filled = |id, good| {
        s.society.has_live_spot_bid(id, good)
            || s.society.trades[trade_start..]
                .iter()
                .any(|trade| trade.buyer == id && trade.good == good)
    };
    assert!(
        posted_or_filled(miller, grain),
        "Miller input override must post"
    );
    assert!(
        posted_or_filled(baker, flour),
        "Baker input override must post"
    );
    assert!(
        posted_or_filled(miller, bread),
        "an active Miller must retain a birth-stock bid after input reservation"
    );
    assert!(
        posted_or_filled(baker, bread),
        "an active Baker must retain a birth-stock bid after input reservation"
    );
}

#[test]
fn sufficiency_control_conserves_stock_and_records_the_immediate_birth() {
    let mut cfg = SettlementConfig::frontier_mortal_producers_earned();
    cfg.chain.as_mut().expect("chain").birth_stock_saving_mode =
        BirthStockSavingMode::SufficiencyControl;
    let mut s = Settlement::generate(7, &cfg);
    let demo = s.demography.clone().expect("demography");
    let household = s.producer_household_start().expect("producer houses");
    s.econ_tick = demo.birth_interval;
    for &slot in &s.live_colonist_slots.clone() {
        s.colonists[slot].need.hunger = 0;
        if s.colonists[slot].household == Some(household) {
            let id = s.colonists[slot].id;
            let held = s
                .society
                .agents
                .get(id)
                .expect("agent")
                .stock
                .get(s.known.hunger);
            assert!(s.society.debit_stock(id, s.known.hunger, held));
        }
    }
    let before = s.whole_system_total(s.known.hunger);
    let injected = s.run_birth_stock_sufficiency_control();
    let after = s.whole_system_total(s.known.hunger);
    assert_eq!(before, after, "the control must move existing bread only");
    assert_eq!(injected, vec![household]);
    assert_eq!(s.birth_stock_eligible_opportunities, 1);
    assert_eq!(s.birth_stock_injections_completed, 1);
    let births = s.run_births();
    s.record_birth_stock_control_results(&injected);
    assert!(births > 0);
    assert_eq!(s.households[household].last_birth_tick, Some(s.econ_tick));
    assert_eq!(
        s.birth_stock_injection_records,
        vec![BirthStockInjectionRecord {
            tick: s.econ_tick,
            household,
            birth_succeeded: true,
        }]
    );
}

#[test]
fn birth_stock_transfer_preserves_both_bread_provenance_ledgers() {
    let mut cfg = SettlementConfig::frontier_mortal_producers_earned();
    cfg.chain.as_mut().expect("chain").birth_stock_saving_mode =
        BirthStockSavingMode::SufficiencyControl;
    let mut s = Settlement::generate(7, &cfg);
    let staple = s.known.hunger;
    let household = s.producer_household_start().expect("producer houses");
    let recipient = s
        .live_colonist_slots
        .iter()
        .map(|&slot| &s.colonists[slot])
        .find(|colonist| colonist.household == Some(household))
        .expect("producer-house recipient")
        .id;
    let donor = s
        .live_colonist_slots
        .iter()
        .map(|&slot| &s.colonists[slot])
        .find(|colonist| colonist.household != Some(household))
        .expect("outside donor")
        .id;
    for id in [donor, recipient] {
        let held = s.stock_of_id(id, staple);
        assert!(s.society.debit_stock(
            id,
            staple,
            u32::try_from(held).expect("test stock fits u32")
        ));
    }
    assert!(s.society.credit_stock(donor, staple, 4));
    s.bread_provenance = BreadProvenance::default();
    s.bread_provenance.credit_produced(donor, 4, true);
    s.acquisition = AcquisitionLedger::default();
    s.acquisition.credit(donor, FoodChannel::Bought, 4);

    assert!(s.transfer_birth_stock(donor, recipient, staple, 4, false));

    assert_eq!(s.stock_of_id(donor, staple), 0);
    assert_eq!(s.stock_of_id(recipient, staple), 4);
    assert_eq!(s.bread_provenance.produced.get(&donor), None);
    assert_eq!(s.bread_provenance.produced.get(&recipient), Some(&4));
    assert!(!s.acquisition.lots.contains_key(&donor));
    assert_eq!(
        s.acquisition.lots.get(&recipient),
        Some(&VecDeque::from([FoodLot {
            channel: FoodChannel::Bought,
            qty: 4,
            intervention: false,
            identity: None,
            taint: false,
        }]))
    );
}

#[test]
fn failed_birth_stock_credit_rolls_back_the_donor() {
    let mut cfg = SettlementConfig::frontier_mortal_producers_earned();
    cfg.chain.as_mut().expect("chain").birth_stock_saving_mode =
        BirthStockSavingMode::SufficiencyControl;
    let mut s = Settlement::generate(7, &cfg);
    let staple = s.known.hunger;
    let donor = s
        .society
        .agents
        .iter()
        .map(|agent| agent.id)
        .next()
        .expect("donor");
    let recipient = AgentId(u64::MAX);
    let held = s
        .society
        .agents
        .get(donor)
        .expect("agent")
        .stock
        .get(staple);
    assert!(s.society.debit_stock(donor, staple, held));
    assert!(s.society.credit_stock(donor, staple, 4));
    assert!(!s.transfer_birth_stock(donor, recipient, staple, 4, false));
    assert_eq!(s.stock_of_id(donor, staple), 4);
}

#[test]
fn control_does_not_reuse_an_earlier_injection_as_a_later_donation() {
    let mut cfg = SettlementConfig::frontier_mortal_producers_earned();
    cfg.chain.as_mut().expect("chain").birth_stock_saving_mode =
        BirthStockSavingMode::SufficiencyControl;
    let mut s = Settlement::generate(7, &cfg);
    let demo = s.demography.clone().expect("demography");
    let staple = s.known.hunger;
    s.econ_tick = demo.birth_interval;
    for &slot in &s.live_colonist_slots.clone() {
        s.colonists[slot].need.hunger = 0;
        let id = s.colonists[slot].id;
        let held = s.stock_of_id(id, staple);
        assert!(s.society.debit_stock(
            id,
            staple,
            u32::try_from(held).expect("test stock fits u32")
        ));
    }
    let donor = s
        .live_colonist_slots
        .iter()
        .map(|&slot| &s.colonists[slot])
        .find(|colonist| {
            !colonist
                .household
                .is_some_and(|household| s.is_producer_household(household))
        })
        .expect("non-producer donor")
        .id;
    assert!(s.society.credit_stock(donor, staple, 4));

    let injected = s.run_birth_stock_sufficiency_control();

    assert_eq!(injected.len(), 1);
    assert_eq!(s.birth_stock_injections_completed, 1);
    assert!(s.birth_stock_source_shortfalls > 0);
    let injected_household = injected[0];
    assert!(s.live_colonist_slots.iter().any(|&slot| {
        s.colonists[slot].household == Some(injected_household)
            && s.stock_of_id(s.colonists[slot].id, staple) >= 4
    }));
}

#[test]
fn subsistence_advance_provenance_follows_in_kind_bread() {
    // The in-kind subsistence advance moves the bread staple donor→producer BEFORE the
    // market. When the staple is the tracked bread the provenance ledger must follow the
    // physical loaf (the fix): without the mirror the produced origin strands on the
    // donor and the per-agent invariant (`produced ≤ stock`) breaks once the producer
    // holds the advanced bread.
    let mut s = Settlement::generate(7, &SettlementConfig::frontier_money_from_cultivation());
    let bread = s.provenance_bread_good().expect("chain bread");
    let ids = s
        .society
        .agents
        .iter()
        .map(|agent| agent.id)
        .take(2)
        .collect::<Vec<_>>();
    let (donor, producer) = (ids[0], ids[1]);

    // Isolate the move: clear both agents' bread, give the donor 3 produced loaves.
    s.bread_provenance = BreadProvenance::default();
    for id in [donor, producer] {
        let stock = &mut s.society.agents.get_mut(id).expect("agent").stock;
        let held = stock.get(bread);
        assert!(stock.remove(bread, held));
    }
    s.society
        .agents
        .get_mut(donor)
        .expect("donor")
        .stock
        .add(bread, 3);
    s.bread_provenance.credit_produced(donor, 3, true);

    // The advance's conserved in-kind move of 2 loaves donor→producer. The PHYSICAL move
    // alone strands the produced origin: the donor's produced (3) now exceeds its bread
    // stock (1) — the invariant the fix restores is provably broken here.
    assert!(s.society.debit_stock(donor, bread, 2));
    assert!(s.society.credit_stock(producer, bread, 2));
    assert!(
        !s.bread_provenance_conserves(),
        "the physical in-kind move alone strands the produced origin on the donor"
    );

    // The fix mirrors the in-kind move in the ledger: the produced origin follows the
    // loaf donor→producer, so a produced loaf the producer later sells stays produced and
    // the per-agent invariant is restored.
    s.bread_provenance.transfer(donor, producer, 2);
    assert_eq!(s.bread_provenance.produced.get(&donor).copied(), Some(1));
    assert_eq!(s.bread_provenance.produced.get(&producer).copied(), Some(2));
    assert!(
        s.bread_provenance_conserves(),
        "mirroring the in-kind staple advance keeps the provenance ledger conserved"
    );
}

#[test]
fn estate_provenance_follows_heir_headroom_split() {
    // Physical estate settlement credits the heir first, then sends any bread that
    // cannot fit at the heir to the commons. Produced-origin bread must follow that
    // exact split instead of moving the dead agent's whole counter to the heir.
    let mut s = Settlement::generate(7, &SettlementConfig::frontier_money_from_cultivation());
    let bread = s.provenance_bread_good().expect("chain bread");
    let household = s
        .live_colonist_slots
        .iter()
        .filter_map(|&slot| s.colonists[slot].household)
        .find(|&household| {
            s.live_colonist_slots
                .iter()
                .filter(|&&slot| s.colonists[slot].household == Some(household))
                .count()
                >= 2
        })
        .expect("a lineage with an heir");
    let members = s
        .live_colonist_slots
        .iter()
        .copied()
        .filter(|&slot| s.colonists[slot].household == Some(household))
        .take(2)
        .collect::<Vec<_>>();
    assert!(members.len() >= 2, "test needs a same-household heir");
    let deceased_slot = members[0];
    let deceased = s.colonists[deceased_slot].id;
    s.mark_colonist_dead(deceased_slot);
    let heir = s.heir_for(deceased).expect("same-household heir");

    for id in [deceased, heir] {
        let stock = &mut s.society.agents.get_mut(id).expect("agent").stock;
        let held = stock.get(bread);
        assert!(stock.remove(bread, held));
    }
    s.society
        .agents
        .get_mut(heir)
        .expect("heir")
        .stock
        .add(bread, u32::MAX - 1);
    s.society
        .agents
        .get_mut(deceased)
        .expect("deceased")
        .stock
        .add(bread, 4);
    s.bread_provenance = BreadProvenance::default();
    s.bread_provenance.credit_produced(deceased, 4, true);

    let commons_before = s.commons_stock_of(bread);
    let mut report = EconTickReport::default();
    let mut wage_labor_used = Vec::new();
    assert!(s.settle_estate_to_heirs(deceased, &mut report, &mut wage_labor_used));

    assert_eq!(
        s.society.agents.get(heir).expect("heir").stock.get(bread),
        u32::MAX,
        "the heir receives only its one unit of bread headroom"
    );
    assert_eq!(
        s.commons_stock_of(bread),
        commons_before + 3,
        "the estate bread that cannot fit at the heir routes to commons"
    );
    assert_eq!(s.bread_provenance.produced.get(&heir).copied(), Some(1));
    assert_eq!(s.bread_provenance.produced_sunk, 3);
    assert!(
        s.bread_provenance_conserves(),
        "estate provenance must conserve after an heir/commons split"
    );
}

#[test]
fn canonical_bytes_include_bread_provenance() {
    // S16: the per-agent produced-bread counters steer the future origin attribution of
    // every bread→medium trade, so two settlements differing only in a produced balance
    // must digest apart — and an off-path settlement must NOT serialize them.
    let mut on = Settlement::generate(7, &SettlementConfig::frontier_money_from_cultivation());
    let before = on.canonical_bytes();
    let id = on.society.agents.iter().next().expect("an agent exists").id;
    *on.bread_provenance.produced.entry(id).or_insert(0) += 1;
    assert_ne!(
        before,
        on.canonical_bytes(),
        "with the ledger active a per-agent produced balance must be in the digest"
    );

    // Off the money-from-produced-bread path the ledger is the empty default and is
    // never serialized: poking it must not steer the digest (byte-identical to pre-S16).
    let mut off = Settlement::generate(7, &SettlementConfig::frontier_cultivation());
    let off_before = off.canonical_bytes();
    let off_id = off
        .society
        .agents
        .iter()
        .next()
        .expect("an agent exists")
        .id;
    *off.bread_provenance.produced.entry(off_id).or_insert(0) += 7;
    assert_eq!(
        off_before,
        off.canonical_bytes(),
        "off the path the provenance ledger must not steer the digest"
    );
}

#[test]
fn canonical_bytes_include_cultivating_state() {
    // S15: the per-colonist `cultivating` flag, `cultivate_pressure` streak, and
    // pending-stock latch steer the next world task / drain, so two cultivation
    // states differing only in them must digest apart — and an off-cultivation chain
    // must NOT serialize them (byte-identical).
    let mut on = Settlement::generate(7, &SettlementConfig::frontier_cultivation());
    let before = on.canonical_bytes();
    on.colonists[0].cultivating = !on.colonists[0].cultivating;
    assert_ne!(
        before,
        on.canonical_bytes(),
        "with cultivation on, a colonist's cultivating flag must be in the digest"
    );
    let before = on.canonical_bytes();
    on.colonists[0].cultivate_pressure = on.colonists[0].cultivate_pressure.wrapping_add(1);
    assert_ne!(
        before,
        on.canonical_bytes(),
        "with cultivation on, the cultivate-pressure streak must be in the digest"
    );
    let before = on.canonical_bytes();
    on.colonists[0].cultivation_stock_pending = !on.colonists[0].cultivation_stock_pending;
    assert_ne!(
        before,
        on.canonical_bytes(),
        "with cultivation on, the pending cultivation-stock latch must be in the digest"
    );

    let mut off = Settlement::generate(7, &SettlementConfig::frontier_forage_capacity());
    let off_before = off.canonical_bytes();
    off.colonists[0].cultivating = !off.colonists[0].cultivating;
    off.colonists[0].cultivate_pressure = off.colonists[0].cultivate_pressure.wrapping_add(5);
    off.colonists[0].cultivation_stock_pending = !off.colonists[0].cultivation_stock_pending;
    assert_eq!(
        off_before,
        off.canonical_bytes(),
        "with cultivation off, the unused cultivating state must not steer the digest"
    );
}

#[test]
fn canonical_bytes_include_cultivation_skill() {
    // S22b: the per-colonist `cultivation_skill` scalar steers the next grain trip's haul
    // capacity, so on the skill path two states differing only in it must digest apart — and
    // off the path (S22a, flag off) the unused skill field must NOT serialize (byte-identical).
    let mut on = Settlement::generate(7, &SettlementConfig::frontier_occupational_stickiness());
    let before = on.canonical_bytes();
    on.colonists[0].cultivation_skill = on.colonists[0].cultivation_skill.wrapping_add(50);
    assert_ne!(
        before,
        on.canonical_bytes(),
        "with cultivation skill on, a colonist's skill scalar must be in the digest"
    );

    // The skill parameters enter the digest (tag 8) only on the active path: two configs
    // differing only in a skill magnitude generate apart.
    let base = Settlement::generate(7, &SettlementConfig::frontier_occupational_stickiness());
    let mut tweaked = SettlementConfig::frontier_occupational_stickiness();
    tweaked.chain.as_mut().expect("chain").skill_haul_ceiling = 4;
    assert_ne!(
        base.canonical_bytes(),
        Settlement::generate(7, &tweaked).canonical_bytes(),
        "the skill haul ceiling must steer the digest on the active path"
    );

    // Off the skill path (S22a) the cultivation_skill field is never serialized.
    let mut off = Settlement::generate(7, &SettlementConfig::frontier_endogenous_cultivation());
    let off_before = off.canonical_bytes();
    off.colonists[0].cultivation_skill = off.colonists[0].cultivation_skill.wrapping_add(123);
    assert_eq!(
        off_before,
        off.canonical_bytes(),
        "with cultivation skill off, the unused skill scalar must not steer the digest"
    );
}

#[test]
fn cultivation_skill_reverted_is_byte_identical_to_s22a() {
    // S22b is one additive, default-off ON-only gate: reverting `cultivation_skill` to false
    // makes `frontier_occupational_stickiness` byte-identical to `frontier_endogenous_cultivation`
    // (the S22a stream) across a long horizon, and the ON config splits the digest.
    let on = Settlement::generate(7, &SettlementConfig::frontier_occupational_stickiness());
    let s22a = Settlement::generate(7, &SettlementConfig::frontier_endogenous_cultivation());
    assert_ne!(
        on.canonical_bytes(),
        s22a.canonical_bytes(),
        "the cultivation_skill gate must split the digest vs the S22a base"
    );

    let mut reverted = SettlementConfig::frontier_occupational_stickiness();
    reverted.chain.as_mut().expect("chain").cultivation_skill = false;
    let mut a = Settlement::generate(7, &reverted);
    let mut b = Settlement::generate(7, &SettlementConfig::frontier_endogenous_cultivation());
    a.run(1_000);
    b.run(1_000);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "reverting cultivation_skill must equal frontier_endogenous_cultivation byte-for-byte"
    );
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn cultivation_haul_scales_with_skill_bounded() {
    // S22b: the per-trip haul is carry_cap at skill 0 / cap 0 / ceiling ≤ 1, rises linearly,
    // and saturates at ceiling × carry_cap at full skill. Never exceeds the ceiling.
    assert_eq!(cultivation_haul(6, 0, 1000, 2), 6, "skill 0 ⇒ carry_cap");
    assert_eq!(cultivation_haul(6, 1000, 1000, 2), 12, "full skill ⇒ 2×");
    assert_eq!(cultivation_haul(6, 500, 1000, 2), 9, "half skill ⇒ +50%");
    assert_eq!(cultivation_haul(6, 1000, 1000, 1), 6, "ceiling 1 ⇒ no-op");
    assert_eq!(cultivation_haul(6, 1000, 0, 2), 6, "cap 0 ⇒ no-op");
    assert_eq!(cultivation_haul(6, 1000, 1000, 4), 24, "ceiling 4 ⇒ 4×");
    assert_eq!(
        cultivation_haul(6, 5000, 1000, 2),
        12,
        "skill above cap still saturates at the ceiling"
    );
}

#[test]
fn birth_food_options_broadens_only_on_cultivation() {
    // S15: off the cultivation path the birth-food rule is the single S14 selector
    // (FORAGE on the commons path); on it, it BROADENS to bread-then-forage so a
    // cultivator's own bread can endow children (else births stall on a forage
    // shortage and the plateau cannot rise).
    let forage_cfg = SettlementConfig::frontier_forage_capacity();
    let forage = forage_cfg.chain.as_ref().unwrap().content.forage().unwrap();
    let s = Settlement::generate(1, &forage_cfg);
    let mut buf = [s.known.hunger; 2];
    assert_eq!(
        s.birth_food_options(&mut buf),
        [forage].as_slice(),
        "off cultivation the birth-food rule is the single S14 (forage) selector"
    );

    let cult_cfg = SettlementConfig::frontier_cultivation();
    let bread = cult_cfg.chain.as_ref().unwrap().content.bread();
    let forage = cult_cfg.chain.as_ref().unwrap().content.forage().unwrap();
    let s = Settlement::generate(1, &cult_cfg);
    let mut buf = [s.known.hunger; 2];
    assert_eq!(
        s.birth_food_options(&mut buf),
        [bread, forage].as_slice(),
        "on cultivation the rule broadens to bread first, then forage"
    );
}

#[test]
fn own_labor_credit_requires_completed_forage_task() {
    // A stale `foraging` decision is not enough to create FORAGE. The agent must
    // actually complete `Task::GoForage` in the preceding fast loop; a hungry
    // colonist busy walking somewhere else keeps the flag for the next assignment
    // but produces nothing this tick.
    let cfg = SettlementConfig::frontier_coemergent_strong_provisioned();
    let forage = cfg
        .chain
        .as_ref()
        .expect("chain")
        .content
        .forage()
        .expect("forage good");
    let mut s = Settlement::generate(7, &cfg);
    let slot = s
        .live_colonist_slots
        .iter()
        .copied()
        .find(|&slot| s.colonists[slot].household.is_none())
        .expect("a spatial non-lineage colonist");
    let id = s.colonists[slot].id;
    s.colonists[slot].need.hunger = 12;
    s.colonists[slot].foraging = true;
    assert!(s.world.assign_task(id, Task::GoTo(Pos::new(63, 0))));
    assert_eq!(s.world.agent_status(id), Some(AgentStatus::Moving));

    let report = s.econ_tick();
    assert!(report.conserves());
    assert_eq!(
        report.produced_of(forage),
        0,
        "FORAGE credit must be gated on a completed GoForage task, not just the flag"
    );
    assert!(
        s.colonists[slot].foraging,
        "the hungry colonist should still be marked to forage once its current task settles"
    );
}

#[test]
fn canonical_bytes_include_reentry_flags() {
    // S6: `productive_reentry` gates the re-entry phase that flips spatial
    // colonists' vocations/nodes. It steers future ticks while leaving generation
    // untouched only when raw grain is edible, so two active chains differing only
    // in it must digest apart. The two hysteresis thresholds steer behaviour only
    // while the phase runs, so they join the digest when (and only when) re-entry
    // is active: two active chains differing in a threshold split, but inactive
    // chains differing only in a threshold stay byte-identical.
    let mut off = SettlementConfig::frontier_endogenous();
    off.chain.as_mut().expect("chain").productive_reentry = false;
    let mut on = SettlementConfig::frontier_endogenous();
    on.chain.as_mut().expect("chain").productive_reentry = true;
    let off_bytes = Settlement::generate(7, &off).canonical_bytes();
    assert_ne!(
        off_bytes,
        Settlement::generate(7, &on).canonical_bytes(),
        "the re-entry phase gate must be part of the chain config identity"
    );

    // Phase ON: a different entry OR exit threshold must split the digest.
    let mut on_hi = on.clone();
    on_hi.chain.as_mut().expect("chain").reentry_hunger_in = 6;
    assert_ne!(
        Settlement::generate(7, &on).canonical_bytes(),
        Settlement::generate(7, &on_hi).canonical_bytes(),
        "with re-entry on, the entry threshold must be part of the digest"
    );
    let mut on_lo = on.clone();
    on_lo.chain.as_mut().expect("chain").reentry_hunger_out = 2;
    assert_ne!(
        Settlement::generate(7, &on).canonical_bytes(),
        Settlement::generate(7, &on_lo).canonical_bytes(),
        "with re-entry on, the exit threshold must be part of the digest"
    );

    // Phase OFF: the (unused) thresholds must NOT split the digest, or the
    // tripwire would call two behaviour-identical configs unequal.
    let mut off_thresholds = off.clone();
    {
        let c = off_thresholds.chain.as_mut().expect("chain");
        c.reentry_hunger_in = 6;
        c.reentry_hunger_out = 2;
    }
    assert_eq!(
        off_bytes,
        Settlement::generate(7, &off_thresholds).canonical_bytes(),
        "with re-entry off, the unused thresholds must not steer the digest"
    );

    // Phase ON but no edible-grain fallback: the runtime phase exits before
    // mutating, so the flag and thresholds must serialize as a no-op.
    let inert = SettlementConfig::grain_flour_bread_chain();
    assert!(
        !inert.chain.as_ref().expect("chain").subsistence_on_grain,
        "the seeded chain does not make raw grain directly edible"
    );
    let mut inert_on = inert.clone();
    {
        let c = inert_on.chain.as_mut().expect("chain");
        c.productive_reentry = true;
        c.reentry_hunger_in = 6;
        c.reentry_hunger_out = 2;
    }
    assert_eq!(
        Settlement::generate(7, &inert).canonical_bytes(),
        Settlement::generate(7, &inert_on).canonical_bytes(),
        "without edible grain, re-entry is behavior-identical and must not split the digest"
    );
}

#[test]
fn canonical_bytes_include_reentry_home() {
    // S6: with re-entry ON, a colonist's HOME vocation+node decide whether and where
    // a displaced re-entrant reverts once fed (`run_productive_reentry`). Two states
    // with identical CURRENT vocation/node but different homes diverge on the revert
    // path, so the home is part of the future-behaviour identity — `canonical_bytes`
    // must read it. With re-entry OFF the home is never consulted, so it must NOT
    // steer the digest (the `endogenous` byte-identity tripwire).
    let mut on = SettlementConfig::frontier_endogenous();
    on.chain.as_mut().expect("chain").productive_reentry = true;
    let on_bytes = Settlement::generate(7, &on).canonical_bytes();

    // Re-entry ON: perturbing a colonist's home NODE must split the digest.
    let mut on_node = Settlement::generate(7, &on);
    let node_slot = on_node
        .colonists
        .iter()
        .position(|c| c.home_node.is_some())
        .expect("a spatial gatherer with a home node");
    on_node.colonists[node_slot].home_node = None;
    assert_ne!(
        on_bytes,
        on_node.canonical_bytes(),
        "with re-entry on, the home node must be part of the digest"
    );

    // Re-entry ON: perturbing a colonist's home VOCATION must split the digest.
    let mut on_voc = Settlement::generate(7, &on);
    let voc_slot = on_voc
        .colonists
        .iter()
        .position(|c| c.home_vocation == Vocation::Consumer)
        .expect("a non-lineage consumer with a Consumer home");
    on_voc.colonists[voc_slot].home_vocation = Vocation::Gatherer;
    assert_ne!(
        on_bytes,
        on_voc.canonical_bytes(),
        "with re-entry on, the home vocation must be part of the digest"
    );

    // Re-entry OFF: the same home perturbation must NOT split the digest, or the
    // pre-S6 per-colonist layout (and the `endogenous` byte-identity) would break.
    let off = SettlementConfig::frontier_endogenous();
    let off_bytes = Settlement::generate(7, &off).canonical_bytes();
    let mut off_node = Settlement::generate(7, &off);
    let off_slot = off_node
        .colonists
        .iter()
        .position(|c| c.home_node.is_some())
        .expect("a spatial gatherer with a home node");
    off_node.colonists[off_slot].home_node = None;
    assert_eq!(
        off_bytes,
        off_node.canonical_bytes(),
        "with re-entry off, the home must not steer the digest"
    );

    // Re-entry ON but raw grain not edible: the phase is inert, so the home also
    // must not steer the digest.
    let mut inert = SettlementConfig::grain_flour_bread_chain();
    inert.chain.as_mut().expect("chain").productive_reentry = true;
    let inert_bytes = Settlement::generate(7, &inert).canonical_bytes();
    let mut inert_node = Settlement::generate(7, &inert);
    let inert_slot = inert_node
        .colonists
        .iter()
        .position(|c| c.home_node.is_some())
        .expect("a spatial gatherer with a home node");
    inert_node.colonists[inert_slot].home_node = None;
    assert_eq!(
        inert_bytes,
        inert_node.canonical_bytes(),
        "without edible grain, re-entry home state must not steer the digest"
    );
}

#[test]
fn canonical_bytes_include_group_payoff_home_anchor() {
    let on = SettlementConfig::frontier_group_payoff_imitation();
    let on_bytes = Settlement::generate(7, &on).canonical_bytes();

    let mut on_node = Settlement::generate(7, &on);
    let node_slot = on_node
        .colonists
        .iter()
        .position(|c| c.home_node.is_some())
        .expect("a spatial gatherer with a home node");
    on_node.colonists[node_slot].home_node = None;
    assert_ne!(
        on_bytes,
        on_node.canonical_bytes(),
        "S24c group anchors read home_node, so it must split the digest"
    );

    let off = SettlementConfig::frontier_abandonable_norm();
    let off_bytes = Settlement::generate(7, &off).canonical_bytes();
    let mut off_node = Settlement::generate(7, &off);
    let off_slot = off_node
        .colonists
        .iter()
        .position(|c| c.home_node.is_some())
        .expect("a spatial gatherer with a home node");
    off_node.colonists[off_slot].home_node = None;
    assert_eq!(
        off_bytes,
        off_node.canonical_bytes(),
        "with S24c off, home_node must not perturb the S24b digest"
    );
}

#[test]
#[should_panic(expected = "re-entry hysteresis requires reentry_hunger_out < reentry_hunger_in")]
fn active_reentry_rejects_invalid_hysteresis() {
    let mut cfg = SettlementConfig::frontier_endogenous();
    {
        let c = cfg.chain.as_mut().expect("chain");
        c.productive_reentry = true;
        c.reentry_hunger_in = 4;
        c.reentry_hunger_out = 4;
    }
    let _ = Settlement::generate(7, &cfg);
}

#[test]
#[should_panic(
    expected = "own-labor subsistence hysteresis requires forage_hunger_out < forage_hunger_in"
)]
fn active_own_labor_subsistence_rejects_invalid_hysteresis() {
    let mut cfg = SettlementConfig::frontier_coemergent_strong_provisioned();
    {
        let c = cfg.chain.as_mut().expect("chain");
        c.forage_hunger_in = 4;
        c.forage_hunger_out = 4;
    }
    let _ = Settlement::generate(7, &cfg);
}

#[test]
#[should_panic(
    expected = "own-use cultivation requires cultivate_hunger_out < cultivate_hunger_in"
)]
fn active_own_use_cultivation_rejects_invalid_hysteresis() {
    let mut cfg = SettlementConfig::frontier_cultivation();
    {
        let c = cfg.chain.as_mut().expect("chain");
        c.cultivate_hunger_in = 4;
        c.cultivate_hunger_out = 4;
    }
    let _ = Settlement::generate(7, &cfg);
}

#[test]
#[should_panic(expected = "own-use cultivation requires cultivate_patience > 0")]
fn active_own_use_cultivation_rejects_zero_patience() {
    // A 0 patience would satisfy `pressure >= cultivate_patience` even with a 0
    // pressure streak (hunger below the threshold), escalating fed colonists to
    // cultivation — the scarcity gate must reject it at config time.
    let mut cfg = SettlementConfig::frontier_cultivation();
    cfg.chain.as_mut().expect("chain").cultivate_patience = 0;
    let _ = Settlement::generate(7, &cfg);
}

#[test]
#[should_panic(expected = "own-use cultivation requires cultivate_consume > 0")]
fn active_own_use_cultivation_rejects_zero_consume() {
    // A 0 draw never eats the cultivated bread through the readback, so the escape
    // valve never relieves hunger and bread silently hoards — rejected at config time.
    let mut cfg = SettlementConfig::frontier_cultivation();
    cfg.chain.as_mut().expect("chain").cultivate_consume = 0;
    let _ = Settlement::generate(7, &cfg);
}

#[test]
#[should_panic(expected = "own-use cultivation requires cultivated bread to be the hunger good")]
fn active_own_use_cultivation_rejects_non_staple_bread() {
    // The own-use phase consumes `content.bread()` and the need readback only feeds
    // hunger from known edible goods. If bread is not the hunger staple, cultivated
    // bread would be debited/logged without relieving hunger.
    let mut cfg = SettlementConfig::frontier_cultivation();
    cfg.chain.as_mut().expect("chain").bread_is_staple = false;
    let _ = Settlement::generate(7, &cfg);
}

#[test]
#[should_panic(expected = "own-use cultivation requires a grain resource node")]
fn active_own_use_cultivation_rejects_missing_grain_node() {
    // Cultivators are steered to GoHarvest the grain node before applying the
    // recipe. Without one they would stop foraging but never receive input.
    let mut cfg = SettlementConfig::frontier_cultivation();
    let grain = cfg.chain.as_ref().expect("chain").content.grain();
    cfg.nodes.retain(|spec| spec.good != grain);
    let _ = Settlement::generate(7, &cfg);
}

#[test]
fn cultivation_holds_in_flight_grain_then_drains_settled_stock() {
    let cfg = SettlementConfig::frontier_cultivation();
    let grain = cfg.chain.as_ref().expect("chain").content.grain();
    let bread = cfg.chain.as_ref().expect("chain").content.bread();
    let mut s = Settlement::generate(7, &cfg);
    let slot = s.live_colonist_slots[0];
    let id = s.colonists[slot].id;
    s.colonists[slot].need.hunger = 0;
    s.colonists[slot].foraging = false;
    s.colonists[slot].cultivating = true;
    s.colonists[slot].cultivate_pressure = 0;
    s.pending_deposits.insert((id, grain), 1);

    let mut report = EconTickReport::default();
    s.run_own_labor_subsistence(&BTreeSet::new(), &mut report);
    assert!(
        s.colonists[slot].cultivating,
        "cultivation must stay latched while harvested grain is still in flight"
    );
    assert!(
        !s.colonists[slot].foraging,
        "a grain-draining cultivator must not forage in the same tick"
    );

    s.pending_deposits.clear();
    s.society
        .agents
        .get_mut(id)
        .expect("agent exists")
        .stock
        .add(grain, 1);
    s.run_own_labor_subsistence(&BTreeSet::new(), &mut report);
    assert!(
        !s.colonists[slot].cultivating,
        "cultivation may clear once hunger is low and grain is settled in stock"
    );
    s.run_own_use_cultivation(&mut report);
    assert_eq!(
        report.produced_of(bread),
        1,
        "settled grain must still be drained into bread after the flag clears"
    );
    assert_eq!(
        report.consumed_as_input_of(grain),
        1,
        "draining settled grain must book the conserved input"
    );
}

#[test]
fn share_split_ignores_worker_owned_grain_after_renewal_settlement() {
    let cfg = SettlementConfig::frontier_cultivation();
    let grain = cfg.chain.as_ref().expect("chain").content.grain();
    let bread = cfg.chain.as_ref().expect("chain").content.bread();
    let mut s = Settlement::generate(7, &cfg);
    let owner_slot = s.live_colonist_slots[0];
    let worker_slot = s.live_colonist_slots[1];
    let owner = s.colonists[owner_slot].id;
    let worker = s.colonists[worker_slot].id;
    let node = s.grain_node().expect("cultivation config has grain");

    s.colonists[worker_slot].need.hunger = 0;
    s.colonists[worker_slot].cultivating = true;
    s.share_contracts.push(ShareContract {
        id: 1,
        owner,
        worker,
        node,
        share_bps: SHARE_TENANCY_BPS_DEFAULT,
        term: SHARE_TENANCY_TERM_DEFAULT,
        opened_tick: s.econ_tick,
        renewals: 1,
        cap_at_start: 0,
        grain_in_stock: 0,
        split_remainder_bps: 0,
    });

    s.society
        .agents
        .get_mut(worker)
        .expect("worker exists")
        .stock
        .add(grain, 2);
    let mut report = EconTickReport::default();
    s.run_own_use_cultivation(&mut report);
    assert_eq!(report.produced_of(bread), 2);
    assert_eq!(
        s.society
            .agents
            .get(owner)
            .expect("owner exists")
            .stock
            .get(bread),
        0,
        "worker-owned grain kept from a prior settlement must not be split again"
    );
    assert_eq!(s.share_owner_bread_income, 0);
    assert_eq!(s.share_worker_bread_income, 0);

    s.society
        .agents
        .get_mut(worker)
        .expect("worker exists")
        .stock
        .add(grain, 2);
    s.share_contracts[0].grain_in_stock = 2;
    let mut report = EconTickReport::default();
    s.run_own_use_cultivation(&mut report);
    assert_eq!(report.produced_of(bread), 2);
    assert_eq!(
        s.society
            .agents
            .get(owner)
            .expect("owner exists")
            .stock
            .get(bread),
        1,
        "contract-sourced grain must still pay the owner share"
    );
    assert_eq!(s.share_owner_bread_income, 1);
    assert_eq!(s.share_worker_bread_income, 1);
    assert_eq!(s.share_contracts[0].grain_in_stock, 0);
}

#[test]
fn in_kind_split_transfers_full_contract_output_to_employer() {
    let mut cfg = SettlementConfig::frontier_cultivation();
    if let Some(chain) = cfg.chain.as_mut() {
        chain.cultivation_sells_surplus = true;
        chain.acquisition_ledger = true;
    }
    let grain = cfg.chain.as_ref().expect("chain").content.grain();
    let bread = cfg.chain.as_ref().expect("chain").content.bread();
    let mut s = Settlement::generate(7, &cfg);
    s.chain.as_mut().expect("chain").cultivate_consume = 0;
    let employer_slot = s.live_colonist_slots[0];
    let worker_slot = s.live_colonist_slots[1];
    let employer = s.colonists[employer_slot].id;
    let worker = s.colonists[worker_slot].id;
    let node = s.grain_node().expect("cultivation config has grain");

    s.colonists[worker_slot].need.hunger = 0;
    s.colonists[worker_slot].cultivating = true;
    s.in_kind_contracts.push(InKindWageContract {
        id: 1,
        employer,
        worker,
        node,
        wage_bread: 2,
        term: SHARE_TENANCY_TERM_DEFAULT,
        opened_tick: s.econ_tick,
        grain_in_stock: 2,
        split_remainder_bps: 0,
    });
    assert!(s.society.credit_stock(employer, bread, 2));
    s.bread_provenance.credit_produced(employer, 2, true);
    s.acquisition.credit(employer, FoodChannel::SelfProduced, 2);
    assert!(s.society.debit_stock(employer, bread, 2));
    assert!(s.society.credit_stock(worker, bread, 2));
    assert_eq!(
        s.bread_provenance
            .transfer_self_produced(employer, worker, 2),
        2
    );
    s.acquisition.transfer_preserve(employer, worker, 2);
    s.acquisition.credit(worker, FoodChannel::Bought, 1);
    s.society
        .agents
        .get_mut(worker)
        .expect("worker exists")
        .stock
        .add(grain, 2);
    s.society
        .agents
        .get_mut(worker)
        .expect("worker exists")
        .stock
        .add(bread, 1);

    let mut report = EconTickReport::default();
    s.run_own_use_cultivation(&mut report);

    assert_eq!(report.produced_of(bread), 2);
    assert_eq!(report.consumed_as_input_of(grain), 2);
    assert_eq!(
        s.society
            .agents
            .get(employer)
            .expect("employer exists")
            .stock
            .get(bread),
        2,
        "C1N product goes 100% to the employer"
    );
    assert_eq!(
        s.society
            .agents
            .get(worker)
            .expect("worker exists")
            .stock
            .get(bread),
        3,
        "the worker keeps the old advance and bought bread, but no product share"
    );
    let employer_product_lots: u64 = s
        .bread_provenance
        .produced_lots
        .get(&employer)
        .expect("employer receives the worker-produced crop")
        .iter()
        .filter(|lot| lot.producer == worker)
        .map(|lot| lot.qty)
        .sum();
    assert_eq!(
        employer_product_lots, 2,
        "the employer must receive the newly produced crop provenance, not its own advance back"
    );
    let worker_advance_lots: u64 = s
        .bread_provenance
        .produced_lots
        .get(&worker)
        .expect("worker keeps the old wage-advance provenance")
        .iter()
        .filter(|lot| lot.producer == employer)
        .map(|lot| lot.qty)
        .sum();
    assert_eq!(
        worker_advance_lots, 2,
        "the generic FIFO transfer would return this advance provenance to the employer"
    );
    let employer_acquired_self_produced: u64 = s
        .acquisition
        .lots
        .get(&employer)
        .expect("employer receives the crop acquisition lot")
        .iter()
        .filter(|lot| lot.channel == FoodChannel::SelfProduced)
        .map(|lot| lot.qty)
        .sum();
    assert_eq!(employer_acquired_self_produced, 2);
    let worker_bought_lots: u64 = s
        .acquisition
        .lots
        .get(&worker)
        .expect("worker keeps the older non-product lots")
        .iter()
        .filter(|lot| lot.channel == FoodChannel::Bought)
        .map(|lot| lot.qty)
        .sum();
    assert_eq!(
        worker_bought_lots, 1,
        "the crop split must not transfer older bought bread channels"
    );
    assert_eq!(s.in_kind_employer_bread_income, 2);
    assert_eq!(s.in_kind_contracts[0].grain_in_stock, 0);
}

fn harvest_one_carried_grain(s: &mut Settlement, worker: AgentId, node: NodeId, grain: GoodId) {
    assert!(s.world.assign_task(worker, Task::GoHarvest(node, 1)));
    for _ in 0..64 {
        s.world.tick();
        if s.world.agent_carry(worker, grain) > 0 {
            return;
        }
    }
    panic!("worker never harvested carried grain");
}

#[test]
fn in_kind_dissolution_settles_carried_contract_grain() {
    let cfg = SettlementConfig::frontier_cultivation();
    let grain = cfg.chain.as_ref().expect("chain").content.grain();
    let mut s = Settlement::generate(7, &cfg);
    let employer_slot = s.live_colonist_slots[0];
    let worker_slot = s.live_colonist_slots[1];
    let employer = s.colonists[employer_slot].id;
    let worker = s.colonists[worker_slot].id;
    let node = s.grain_node().expect("cultivation config has grain");

    harvest_one_carried_grain(&mut s, worker, node, grain);
    s.colonists[worker_slot].carried_grain_source = Some(node);
    s.colonists[worker_slot].carried_in_kind_contract_id = Some(7);
    let employer_before = s
        .society
        .agents
        .get(employer)
        .expect("employer exists")
        .stock
        .get(grain);
    let contract = InKindWageContract {
        id: 7,
        employer,
        worker,
        node,
        wage_bread: 1,
        term: SHARE_TENANCY_TERM_DEFAULT,
        opened_tick: s.econ_tick,
        grain_in_stock: 0,
        split_remainder_bps: 0,
    };

    s.settle_in_kind_contract_grain(&contract);

    assert_eq!(s.world.agent_carry(worker, grain), 0);
    assert_eq!(
        s.society
            .agents
            .get(employer)
            .expect("employer exists")
            .stock
            .get(grain),
        employer_before + 1,
        "carried contract grain must settle to the employer before the contract closes"
    );
    assert_eq!(s.in_kind_employer_grain_settled, 1);
    assert_eq!(s.colonists[worker_slot].carried_grain_source, None);
    assert_eq!(s.colonists[worker_slot].carried_in_kind_contract_id, None);
}

#[test]
fn in_kind_dissolution_settles_pending_contract_grain() {
    let cfg = SettlementConfig::frontier_cultivation();
    let grain = cfg.chain.as_ref().expect("chain").content.grain();
    let mut s = Settlement::generate(7, &cfg);
    let employer_slot = s.live_colonist_slots[0];
    let worker_slot = s.live_colonist_slots[1];
    let employer = s.colonists[employer_slot].id;
    let worker = s.colonists[worker_slot].id;
    let node = s.grain_node().expect("cultivation config has grain");

    harvest_one_carried_grain(&mut s, worker, node, grain);
    assert!(s.world.assign_task(worker, Task::GoDeposit(s.exchange)));
    for _ in 0..64 {
        s.world.tick();
        if s.world.agent_carry(worker, grain) == 0 {
            break;
        }
    }
    assert_eq!(s.world.agent_carry(worker, grain), 0);
    assert_eq!(s.world.stockpile_get(s.exchange, grain), 1);
    s.pending_deposits.insert((worker, grain), 1);
    s.colonists[worker_slot].carried_grain_source = Some(node);
    s.colonists[worker_slot].carried_in_kind_contract_id = Some(8);
    let employer_before = s
        .society
        .agents
        .get(employer)
        .expect("employer exists")
        .stock
        .get(grain);
    let contract = InKindWageContract {
        id: 8,
        employer,
        worker,
        node,
        wage_bread: 1,
        term: SHARE_TENANCY_TERM_DEFAULT,
        opened_tick: s.econ_tick,
        grain_in_stock: 0,
        split_remainder_bps: 0,
    };

    s.settle_in_kind_contract_grain(&contract);

    assert_eq!(s.world.stockpile_get(s.exchange, grain), 0);
    assert!(
        !s.pending_deposits.contains_key(&(worker, grain)),
        "pending contract grain must not survive contract dissolution"
    );
    assert_eq!(
        s.society
            .agents
            .get(employer)
            .expect("employer exists")
            .stock
            .get(grain),
        employer_before + 1,
        "pending contract grain must settle to the employer before the contract closes"
    );
    assert_eq!(s.in_kind_employer_grain_settled, 1);
    assert_eq!(s.colonists[worker_slot].carried_grain_source, None);
    assert_eq!(s.colonists[worker_slot].carried_in_kind_contract_id, None);
}

#[test]
fn in_kind_candidates_ignore_share_residual_bps() {
    let mut cfg = SettlementConfig::frontier_mortal_landowner_demography();
    let chain = cfg.chain.as_mut().expect("chain");
    chain.share_tenancy = true;
    chain.share_bps = SHARE_TENANCY_BPS_DEFAULT;
    let bread = chain.content.bread();
    let mut s = Settlement::generate(7, &cfg);
    let mut share_candidates = Vec::new();
    for _ in 0..300 {
        share_candidates = s.share_owner_candidate_plots(bread);
        if !share_candidates.is_empty() {
            break;
        }
        let _ = s.econ_tick();
    }
    assert!(
        !share_candidates.is_empty(),
        "test setup needs at least one cap-waste candidate"
    );

    s.chain.as_mut().expect("chain").share_bps = 10_000;
    assert!(
        s.share_owner_candidate_plots(bread).is_empty(),
        "a 100% worker share still disables ordinary share-tenancy offers"
    );
    assert_eq!(
        s.in_kind_owner_candidate_plots(),
        share_candidates,
        "fixed-wage candidates reuse plot constraints, not share residual economics"
    );
}

#[test]
fn share_dissolution_settles_reserved_contract_grain() {
    let cfg = SettlementConfig::frontier_cultivation();
    let grain = cfg.chain.as_ref().expect("chain").content.grain();
    let mut s = Settlement::generate(7, &cfg);
    let owner_slot = s.live_colonist_slots[0];
    let worker_slot = s.live_colonist_slots[1];
    let owner = s.colonists[owner_slot].id;
    let worker = s.colonists[worker_slot].id;
    let node = s.grain_node().expect("cultivation config has grain");

    s.society
        .agents
        .get_mut(worker)
        .expect("worker exists")
        .stock
        .add(grain, 4);
    let ask = econ::market::Order {
        agent: worker,
        side: econ::market::OrderSide::Ask,
        good: grain,
        limit: Gold(1),
        qty: 4,
        seq: 1,
        expires_tick: 99,
    };
    assert!(s
        .society
        .reservations
        .reserve_order(&s.society.agents, &ask));
    assert_eq!(
        s.society.free_stock_after_all_reserves(worker, grain),
        0,
        "the regression requires all contract grain to be reserved"
    );

    let owner_before = s
        .society
        .agents
        .get(owner)
        .expect("owner exists")
        .stock
        .get(grain);
    let worker_before = s
        .society
        .agents
        .get(worker)
        .expect("worker exists")
        .stock
        .get(grain);
    let contract = ShareContract {
        id: 1,
        owner,
        worker,
        node,
        share_bps: SHARE_TENANCY_BPS_DEFAULT,
        term: SHARE_TENANCY_TERM_DEFAULT,
        opened_tick: s.econ_tick,
        renewals: 0,
        cap_at_start: 0,
        grain_in_stock: 4,
        split_remainder_bps: 0,
    };

    s.settle_share_contract_grain(&contract);

    assert_eq!(
        s.society
            .agents
            .get(owner)
            .expect("owner exists")
            .stock
            .get(grain),
        owner_before + 2,
        "the owner share must be paid even when the grain was reserved"
    );
    assert_eq!(
        s.society
            .agents
            .get(worker)
            .expect("worker exists")
            .stock
            .get(grain),
        worker_before - 2,
        "the worker keeps the exact floor share"
    );
    assert_eq!(s.share_owner_grain_settled, 2);
}

#[test]
fn share_contract_settles_on_owner_old_age_death() {
    let cfg = SettlementConfig::frontier_mortal_landowner_demography();
    let grain = cfg.chain.as_ref().expect("chain").content.grain();
    let mut s = Settlement::generate(7, &cfg);
    let owner_slot = s
        .live_colonist_slots
        .iter()
        .copied()
        .find(|&slot| s.colonists[slot].lifespan.is_some())
        .expect("demography config has mortal householders");
    let owner = s.colonists[owner_slot].id;
    let worker = s
        .live_colonist_slots
        .iter()
        .copied()
        .map(|slot| s.colonists[slot].id)
        .find(|&id| id != owner)
        .expect("a second colonist exists");
    let node = s.grain_node().expect("cultivation config has grain");

    s.society
        .agents
        .get_mut(worker)
        .expect("worker exists")
        .stock
        .add(grain, 4);
    let worker_before = s
        .society
        .agents
        .get(worker)
        .expect("worker exists")
        .stock
        .get(grain);
    s.share_contracts.push(ShareContract {
        id: 1,
        owner,
        worker,
        node,
        share_bps: SHARE_TENANCY_BPS_DEFAULT,
        term: SHARE_TENANCY_TERM_DEFAULT,
        opened_tick: s.econ_tick,
        renewals: 0,
        cap_at_start: 0,
        grain_in_stock: 4,
        split_remainder_bps: 0,
    });
    let lifespan = s.colonists[owner_slot].lifespan.expect("owner is mortal");
    s.colonists[owner_slot].age = lifespan;

    let mut report = EconTickReport::default();
    let mut wage_labor_used = Vec::new();
    let deaths = s.age_and_remove_elderly(&mut report, &mut wage_labor_used);

    assert!(deaths >= 1, "the aged owner must die of old age");
    assert!(
        s.share_contracts.is_empty(),
        "the dead owner's contract must dissolve at the old-age seam"
    );
    assert_eq!(
        s.share_owner_grain_settled, 2,
        "the (1 - s) pending-grain share must settle at the old-age seam, not lapse to the worker"
    );
    assert_eq!(
        s.society
            .agents
            .get(worker)
            .expect("worker exists")
            .stock
            .get(grain),
        worker_before - 2,
        "the worker keeps exactly the floor share"
    );
}

/// Build a live share contract on the mortal-landowner base whose owner is a mortal
/// with a resolvable, re-consenting heir and a re-accepting worker — the exact
/// precondition for a voluntary owner-death succession. Shared by the single-death
/// transfer test and the same-batch multi-owner-death regression (review R1/R2), which
/// differ only in whether the plot is already transferred when the owner is settled.
fn setup_owner_death_succession(
    seed: u64,
) -> (Settlement, NodeId, usize, AgentId, AgentId, AgentId, u64) {
    let mut cfg = SettlementConfig::frontier_mortal_landowner_demography();
    let chain = cfg.chain.as_mut().expect("chain");
    chain.rival_subsistence_commons = true;
    chain.rival_subsistence_commons_phi_bps = RIVAL_COMMONS_PHI_MARGINAL_BPS;
    chain.acquisition_ledger = true;
    chain.share_forward_provisioning = true;
    chain.share_bps = SHARE_TENANCY_BPS_DEFAULT;
    chain.share_term = SHARE_TENANCY_TERM_DEFAULT;
    let bread = chain.content.bread();
    let mut s = Settlement::generate(seed, &cfg);

    let mut setup = None;
    for _ in 0..300 {
        let candidates = s.share_owner_candidate_plots(bread);
        'candidate: for candidate in candidates {
            let owner = candidate.owner;
            let Some(owner_slot) = s.slot_for_id(owner) else {
                continue;
            };
            if s.colonists[owner_slot].lifespan.is_none() {
                continue;
            }
            for heir_slot in s.live_colonist_slots.clone() {
                let heir = s.colonists[heir_slot].id;
                if heir == owner || !s.private_land_heir_eligible(heir) {
                    continue;
                }
                for heir_plot in s.land_plots.keys().copied().collect::<Vec<_>>() {
                    if heir_plot == candidate.node || !s.private_land_plot_has_stock(heir_plot) {
                        continue;
                    }
                    let original_parent = s.colonists[heir_slot].parent;
                    let original_household = s.colonists[heir_slot].household;
                    let original_record = s
                        .land_plots
                        .get(&heir_plot)
                        .expect("candidate plot exists")
                        .clone();
                    let original_candidate_record = s
                        .land_plots
                        .get(&candidate.node)
                        .expect("contract plot exists")
                        .clone();
                    s.colonists[heir_slot].parent = Some(owner);
                    s.colonists[heir_slot].household = s.colonists[owner_slot].household;
                    if let Some(record) = s.land_plots.get_mut(&heir_plot) {
                        record.owner = Some(heir);
                        record.reserved_for = None;
                        record.shares.clear();
                        record.stranded_regen = 0;
                        record.stranded_cap = 0;
                    }
                    if let Some(record) = s.land_plots.get_mut(&candidate.node) {
                        record.owner = Some(heir);
                        record.reserved_for = None;
                    }
                    let heir_stays_heir = s.secure_land_universal_heir_for(owner) == Some(heir);
                    let heir_accepts_after_transfer = s
                        .share_owner_candidate_plots(bread)
                        .into_iter()
                        .any(|post| post.owner == heir && post.node == candidate.node);
                    s.land_plots
                        .insert(candidate.node, original_candidate_record);
                    if !heir_stays_heir || !heir_accepts_after_transfer {
                        s.colonists[heir_slot].parent = original_parent;
                        s.colonists[heir_slot].household = original_household;
                        s.land_plots.insert(heir_plot, original_record);
                        continue;
                    }
                    let worker = s.live_colonist_slots.iter().copied().find_map(|slot| {
                        let worker = s.colonists[slot].id;
                        (worker != owner
                            && worker != heir
                            && !s.private_land_agent_holds_any_plot(worker)
                            && s.share_worker_accepts_bread(worker, bread, candidate.node))
                        .then_some(worker)
                    });
                    let Some(worker) = worker else {
                        s.colonists[heir_slot].parent = original_parent;
                        s.colonists[heir_slot].household = original_household;
                        s.land_plots.insert(heir_plot, original_record);
                        continue;
                    };
                    setup = Some((candidate, owner_slot, owner, heir, worker));
                    break 'candidate;
                }
            }
        }
        if setup.is_some() {
            break;
        }
        let _ = s.econ_tick();
    }

    let (candidate, owner_slot, owner, heir, worker) =
        setup.expect("test setup finds a voluntary succession candidate");
    {
        let chain = s.chain.as_mut().expect("chain");
        chain.share_tenancy = true;
        chain.share_forward_provisioning = true;
        chain.share_contract_succession = true;
    }
    let contract_id = 77;
    if let Some(record) = s.land_plots.get_mut(&candidate.node) {
        record.owner = Some(owner);
        record.reserved_for = Some(worker);
    }
    s.share_contracts.push(ShareContract {
        id: contract_id,
        owner,
        worker,
        node: candidate.node,
        share_bps: SHARE_TENANCY_BPS_DEFAULT,
        term: SHARE_TENANCY_TERM_DEFAULT,
        opened_tick: s.econ_tick,
        renewals: 0,
        cap_at_start: candidate.cap_at_start,
        grain_in_stock: 0,
        split_remainder_bps: 0,
    });
    s.next_share_contract_id = contract_id + 1;

    (
        s,
        candidate.node,
        owner_slot,
        owner,
        heir,
        worker,
        contract_id,
    )
}

/// Slice A DoD: a single owner-death succession conserves (goods + provenance) and
/// leaves a live contract owned by the heir with the worker admitted. Here the plot is
/// transferred AFTER the dying owner is settled (the ordinary lone-death ordering).
#[test]
fn owner_death_succession_retains_finalizes_and_conserves() {
    let (mut s, node, owner_slot, owner, heir, worker, contract_id) =
        setup_owner_death_succession(7);

    let before_goods: Vec<_> = s
        .goods
        .iter()
        .map(|&good| (good, s.whole_system_total(good)))
        .collect();
    assert!(s.bread_provenance_conserves());
    s.mark_colonist_dead(owner_slot);
    let pending = s.settle_share_tenancy_for_death(owner);
    assert_eq!(
        pending.len(),
        1,
        "owner death should tentatively retain the contract"
    );
    assert_eq!(
        s.land_plots
            .get(&node)
            .and_then(|record| record.reserved_for),
        Some(worker),
        "tentative retain must not clear the worker reservation before land transfer"
    );
    s.transfer_private_land_on_death(owner);
    assert_eq!(
        s.land_plots.get(&node).and_then(|record| record.owner),
        Some(heir),
        "the staged heir must inherit the contracted plot"
    );
    assert_eq!(
        s.land_plots
            .get(&node)
            .and_then(|record| record.reserved_for),
        None,
        "the land transfer wipes the reservation before succession finalizes"
    );
    s.finalize_share_contract_successions(pending);

    let stats = s.share_tenancy_stats();
    assert_eq!(stats.successions_total, 1);
    assert_eq!(stats.heir_declined, 0);
    assert_eq!(stats.worker_re_declined, 0);
    assert_eq!(stats.final_open_succeeded, 1);
    assert_eq!(stats.owner_grain_settled, 0);
    let contract = s
        .share_contracts
        .iter()
        .find(|contract| contract.id == contract_id)
        .expect("succeeded contract stays live under the same id");
    assert_eq!(contract.owner, heir);
    assert_eq!(contract.worker, worker);
    assert_eq!(contract.node, node);
    let record = s
        .land_plots
        .get(&node)
        .expect("succeeded plot remains registered");
    assert_eq!(record.owner, Some(heir));
    assert_eq!(record.reserved_for, Some(worker));
    assert!(s.share_worker_admitted_to(worker, node, record));
    assert!(s.private_land_registry_invariant_holds());
    assert!(s.share_succession_registry_invariant_holds());
    assert!(s.bread_provenance_conserves());
    for (good, before) in before_goods {
        assert_eq!(
            s.whole_system_total(good),
            before,
            "succession must not move or mint goods"
        );
    }
}

/// Same-batch multi-owner-death regression (review R1/R2): when several share-contract
/// owners die in one death batch, an earlier-processed death's
/// `transfer_private_land_on_death` bulk-reassigns EVERY currently-dead owner's plot to
/// its heir before this owner's own `settle_share_tenancy_for_death` runs — so the plot
/// already reads `owner == heir` when its contract is settled. Emulate that by
/// transferring before settling: the succession must still stage and finalize (not
/// dissolve), so successions are never silently capped at one per death batch (which
/// would under-count exactly under the clustered scarce-phi die-offs where standing
/// tenure is most plausible).
#[test]
fn same_batch_owner_death_before_transfer_still_succeeds() {
    let (mut s, node, owner_slot, owner, heir, worker, contract_id) =
        setup_owner_death_succession(7);

    let before_goods: Vec<_> = s
        .goods
        .iter()
        .map(|&good| (good, s.whole_system_total(good)))
        .collect();
    s.mark_colonist_dead(owner_slot);
    // Emulate the earlier-death bulk transfer already moving THIS owner's plot to the
    // heir before its own contract is settled (the same-batch case).
    s.transfer_private_land_on_death(owner);
    assert_eq!(
        s.land_plots.get(&node).and_then(|record| record.owner),
        Some(heir),
        "the emulated earlier-death transfer moves the plot to the heir first"
    );
    let pending = s.settle_share_tenancy_for_death(owner);
    assert_eq!(
        pending.len(),
        1,
        "a pre-transferred plot (owner already == heir) must still stage succession \
             rather than dissolve — succession is not capped at one per death batch"
    );
    s.finalize_share_contract_successions(pending);

    let stats = s.share_tenancy_stats();
    assert_eq!(
        stats.successions_total, 1,
        "the same-batch succession finalizes exactly as a lone owner death"
    );
    assert_eq!(stats.heir_declined, 0);
    assert_eq!(stats.worker_re_declined, 0);
    assert_eq!(stats.owner_grain_settled, 0);
    let contract = s
        .share_contracts
        .iter()
        .find(|contract| contract.id == contract_id)
        .expect("succeeded contract stays live under the same id");
    assert_eq!(contract.owner, heir);
    assert_eq!(contract.worker, worker);
    assert_eq!(contract.node, node);
    let record = s
        .land_plots
        .get(&node)
        .expect("succeeded plot remains registered");
    assert_eq!(record.owner, Some(heir));
    assert_eq!(record.reserved_for, Some(worker));
    assert!(s.share_worker_admitted_to(worker, node, record));
    assert!(s.private_land_registry_invariant_holds());
    assert!(s.share_succession_registry_invariant_holds());
    assert!(s.bread_provenance_conserves());
    for (good, before) in before_goods {
        assert_eq!(
            s.whole_system_total(good),
            before,
            "succession must not move or mint goods"
        );
    }
}

#[test]
fn share_deposit_does_not_attach_expired_carry_to_same_plot_renewal() {
    let cfg = SettlementConfig::frontier_cultivation();
    let mut s = Settlement::generate(7, &cfg);
    let owner_slot = s.live_colonist_slots[0];
    let worker_slot = s.live_colonist_slots[1];
    let owner = s.colonists[owner_slot].id;
    let worker = s.colonists[worker_slot].id;
    let node = s.grain_node().expect("cultivation config has grain");

    s.colonists[worker_slot].carried_grain_source = Some(node);
    s.colonists[worker_slot].carried_share_contract_id = Some(1);
    s.share_contracts.push(ShareContract {
        id: 2,
        owner,
        worker,
        node,
        share_bps: SHARE_TENANCY_BPS_DEFAULT,
        term: SHARE_TENANCY_TERM_DEFAULT,
        opened_tick: s.econ_tick,
        renewals: 1,
        cap_at_start: 0,
        grain_in_stock: 0,
        split_remainder_bps: 0,
    });

    s.credit_share_contract_grain(worker, 3);
    assert_eq!(
        s.share_contracts[0].grain_in_stock, 0,
        "carry from an expired term on the same plot stays worker-owned"
    );

    s.colonists[worker_slot].carried_share_contract_id = Some(2);
    s.credit_share_contract_grain(worker, 3);
    assert_eq!(
        s.share_contracts[0].grain_in_stock, 3,
        "current-term carry is still attributed to the live contract"
    );
}

#[test]
fn own_use_cultivation_ignores_grain_held_outside_cultivation_path() {
    let cfg = SettlementConfig::frontier_cultivation();
    let grain = cfg.chain.as_ref().expect("chain").content.grain();
    let bread = cfg.chain.as_ref().expect("chain").content.bread();
    let mut s = Settlement::generate(7, &cfg);
    let slot = s.live_colonist_slots[0];
    let id = s.colonists[slot].id;
    s.colonists[slot].vocation = Vocation::Miller;
    s.colonists[slot].foraging = false;
    s.colonists[slot].cultivating = false;
    s.colonists[slot].cultivation_stock_pending = false;
    s.society
        .agents
        .get_mut(id)
        .expect("agent exists")
        .stock
        .add(grain, 2);

    let mut report = EconTickReport::default();
    s.run_own_use_cultivation(&mut report);

    let agent = s.society.agents.get(id).expect("agent exists");
    assert_eq!(
        report.produced_of(bread),
        0,
        "non-cultivation grain must not produce own-use bread"
    );
    assert_eq!(
        report.consumed_as_input_of(grain),
        0,
        "non-cultivation grain must not be consumed by own-use cultivation"
    );
    assert_eq!(
        agent.stock.get(grain),
        2,
        "a grain-holding producer must keep its recipe input"
    );
    assert_eq!(
        agent.stock.get(bread),
        0,
        "own-use cultivation must not credit bread to a non-cultivator"
    );
}

#[test]
fn own_use_cultivation_spends_a_bounded_labor_budget() {
    let cfg = SettlementConfig::frontier_cultivation();
    let chain = cfg.chain.as_ref().expect("chain");
    let recipe = chain.content.cultivate_recipe().expect("cultivate recipe");
    let grain = chain.content.grain();
    let bread = chain.content.bread();
    let budgeted_runs = OWN_USE_CULTIVATION_LABOR_BUDGET / recipe.labor;
    assert!(budgeted_runs > 0, "test setup needs at least one run");

    let mut s = Settlement::generate(7, &cfg);
    let slot = s.live_colonist_slots[0];
    let id = s.colonists[slot].id;
    s.colonists[slot].cultivating = true;
    let starting_grain = budgeted_runs + 2;
    s.society
        .agents
        .get_mut(id)
        .expect("agent exists")
        .stock
        .add(grain, starting_grain);

    let mut report = EconTickReport::default();
    s.run_own_use_cultivation(&mut report);

    assert_eq!(
        report.produced_of(bread),
        u64::from(budgeted_runs * recipe.output_qty),
        "cultivation must stop when the own-labor budget is spent"
    );
    assert_eq!(
        report.consumed_as_input_of(grain),
        u64::from(budgeted_runs * recipe.input_good.expect("input").1),
        "only budgeted grain applications may be consumed"
    );
    assert_eq!(
        s.society
            .labor_used_last_tick()
            .iter()
            .find(|(agent, _)| *agent == id)
            .map(|(_, labor)| *labor),
        Some(budgeted_runs * recipe.labor),
        "the bounded cultivation labor must be recorded for the readback"
    );
    let agent = s.society.agents.get(id).expect("agent exists");
    assert_eq!(
        agent.stock.get(grain),
        starting_grain - budgeted_runs,
        "grain past the labor budget must remain for a later cultivation tick"
    );
}

#[test]
fn own_use_cultivation_nets_out_food_eaten_this_tick() {
    // P2: a cultivator that already ate food in THIS tick's market consume pass must
    // not double-feed through the own-use seam. The need readback advances hunger from
    // the same tick-local log the seam records into, so food already logged this tick
    // is netted out of the own-use draw — else the seam over-eats and drains the
    // child-endowment reserve.
    let cfg = SettlementConfig::frontier_cultivation();
    let bread = cfg.chain.as_ref().expect("chain").content.bread();

    // Run the CONSUME step with `pre_eaten` staple units already logged this tick, and
    // return how much bread the own-use seam then eats. A generous draw + plenty of
    // bread + a low hunger keep the draw bounded by the hunger target, so netting moves
    // it 1:1.
    let eat_with = |pre_eaten: u32| -> u64 {
        let mut s = Settlement::generate(7, &cfg);
        s.chain.as_mut().expect("chain").cultivate_consume = 1_000;
        let slot = s.live_colonist_slots[0];
        let id = s.colonists[slot].id;
        let staple = s.known.hunger;
        s.colonists[slot].cultivating = true;
        s.colonists[slot].need.hunger = 10;
        s.society
            .agents
            .get_mut(id)
            .expect("agent")
            .stock
            .add(bread, 1_000);
        let held_before = s.society.agents.get(id).expect("agent").stock.get(bread);
        if pre_eaten > 0 {
            // Stand in for the market consume pass having already fed this agent.
            s.society.record_own_use_consumption(id, staple, pre_eaten);
        }
        let mut report = EconTickReport::default();
        s.run_own_use_cultivation(&mut report);
        let held_after = s.society.agents.get(id).expect("agent").stock.get(bread);
        // Net out any bread produced this tick, leaving purely the eaten amount.
        u64::from(held_before) + report.produced_of(bread) - u64::from(held_after)
    };

    let baseline = eat_with(0);
    assert!(
        baseline > 0,
        "the baseline own-use draw must be positive and hunger-bound"
    );
    assert_eq!(
        eat_with(1),
        baseline - 1,
        "one unit of food eaten this tick must net one unit off the own-use draw"
    );
    assert_eq!(
        eat_with(baseline as u32),
        0,
        "food already covering the hunger target leaves nothing for the own-use draw"
    );
}

#[test]
fn canonical_bytes_include_phase_gating_flags() {
    // capital_advance / subsistence_advance / input_advance / perishable_decay_bps
    // each gate a future settlement phase that runs for any chain, so a config
    // differing only in one steers later ticks while generating identically — the
    // determinism digest must split them or it would call two non-equivalent
    // configs equal. Flip each in isolation from a common base.
    let base = SettlementConfig::emergent_chain();
    let base_bytes = Settlement::generate(7, &base).canonical_bytes();
    let flip = |mutate: &dyn Fn(&mut ChainConfig)| {
        let mut cfg = SettlementConfig::emergent_chain();
        mutate(cfg.chain.as_mut().expect("chain"));
        Settlement::generate(7, &cfg).canonical_bytes()
    };
    assert_ne!(
        base_bytes,
        flip(&|c| c.capital_advance = !c.capital_advance),
        "the capital-advance flag must be part of the chain config identity"
    );
    assert_ne!(
        base_bytes,
        flip(&|c| c.subsistence_advance = !c.subsistence_advance),
        "the in-kind subsistence-advance flag must be part of the chain config identity"
    );
    assert_ne!(
        base_bytes,
        flip(&|c| c.input_advance = !c.input_advance),
        "the in-kind input-advance flag must be part of the chain config identity"
    );
    assert_ne!(
        base_bytes,
        flip(&|c| c.perishable_decay_bps = c.perishable_decay_bps.wrapping_add(50)),
        "the spoilage decay rate must be part of the chain config identity"
    );
}

#[test]
fn canonical_bytes_include_staple_mapping() {
    // Same physical generated state, different need→good mapping: future scale
    // regeneration will diverge, so the canonical bytes must diverge too.
    let config = SettlementConfig::emergent_chain();
    let a = Settlement::generate(7, &config);
    let mut b = Settlement::generate(7, &config);
    b.known.hunger = FOOD;

    assert_ne!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the staple mapping must be part of the chain config identity"
    );
}

#[test]
fn canonical_bytes_include_barter_config() {
    // Same generated physical state, different barter overlay: future scale
    // regeneration / promotion checks will diverge, so emergent configs must not
    // collide in the determinism digest before the first tick.
    let base = SettlementConfig::barter_camp();

    let mut stronger_medium_want = SettlementConfig::barter_camp();
    stronger_medium_want
        .barter
        .as_mut()
        .expect("barter overlay")
        .medium_want_qty += 1;

    let mut stricter_promotion = SettlementConfig::barter_camp();
    stricter_promotion
        .barter
        .as_mut()
        .expect("barter overlay")
        .menger
        .min_total_acceptances += 1;

    let base = Settlement::generate(7, &base);
    let stronger_medium_want = Settlement::generate(7, &stronger_medium_want);
    let stricter_promotion = Settlement::generate(7, &stricter_promotion);

    assert_ne!(
        base.canonical_bytes(),
        stronger_medium_want.canonical_bytes(),
        "medium_want_qty must be part of the barter config identity"
    );
    assert_ne!(
        base.canonical_bytes(),
        stricter_promotion.canonical_bytes(),
        "Mengerian thresholds must be part of the barter config identity"
    );
}

#[test]
fn canonical_bytes_include_emergence_runtime() {
    // A barter camp run into the barter phase accumulates saleability state (the
    // per-candidate acceptance counts plus the DISTINCT acceptor/counterpart
    // members and the stability latch) that steers the FUTURE promotion tick.
    // That state must ride in the canonical digest — otherwise two barter states
    // with equal holdings but different tracker progress would collide and then
    // promote on different ticks. Reconstruct the runtime bytes from econ's
    // accessors and assert they appear verbatim in the digest input.
    let mut s = Settlement::generate(2_026, &SettlementConfig::barter_camp());
    // Advance into barter but stop before promotion so the tracker is live.
    for _ in 0..3 {
        s.econ_tick();
    }
    assert!(
        s.in_barter_phase(),
        "the run must still be bartering so the tracker is live"
    );
    let emergence = s
        .society
        .emergence()
        .expect("a barter camp runs econ's emergence");
    assert!(
        emergence.tracker().total_acceptances() > 0,
        "the test is vacuous — no barter was observed"
    );

    let mut expected = Vec::new();
    push_emergence_runtime_bytes(&mut expected, emergence);
    let bytes = s.canonical_bytes();
    assert!(
        bytes
            .windows(expected.len())
            .any(|window| window == expected.as_slice()),
        "the accumulated emergence runtime is missing from the canonical bytes"
    );
}

/// A barter config with the heterogeneous SALT direct use + the indirect-breadth
/// gate armed (the strong-bar shape), derived inline from `frontier_coemergent`
/// so the S9.2 digest regressions do not depend on the S9.3 builder.
#[cfg(test)]
fn strong_bar_barter_config() -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_coemergent();
    let barter = cfg.barter.as_mut().expect("barter overlay");
    barter.medium_want_qty = 0;
    barter.salt_direct_use_qty = 1;
    barter.salt_direct_use_period = 8;
    barter.menger.min_indirect_acceptances = 12;
    barter.menger.min_indirect_acceptor_agents = 6;
    barter.menger.min_indirect_target_goods = 1;
    cfg
}

#[test]
fn canonical_bytes_include_indirect_breadth_gate() {
    // S9: each strong-bar gate knob steers the future promotion decision, so all
    // four ride in the determinism identity before the first tick.
    let base = SettlementConfig::frontier_coemergent();

    let knobs: [fn(&mut MengerianConfig); 4] = [
        |m| m.min_indirect_acceptances += 1,
        |m| m.min_indirect_acceptor_agents += 1,
        |m| m.min_indirect_target_goods += 1,
        |m| m.allow_indirect_acceptance = false,
    ];
    let base_bytes = Settlement::generate(7, &base).canonical_bytes();
    for knob in knobs {
        let mut cfg = SettlementConfig::frontier_coemergent();
        knob(&mut cfg.barter.as_mut().expect("barter overlay").menger);
        assert_ne!(
            base_bytes,
            Settlement::generate(7, &cfg).canonical_bytes(),
            "an indirect-breadth gate knob must be part of the Mengerian config identity"
        );
    }
}

#[test]
fn canonical_bytes_include_indirect_acceptance_runtime() {
    // S9: a strong-bar run accumulates per-candidate INDIRECT breadth (the
    // distinct indirect acceptors/targets behind the gate) that steers the future
    // promotion tick. Reconstruct the runtime bytes from econ's accessors and
    // assert they appear verbatim in the digest input.
    let mut s = Settlement::generate(1, &strong_bar_barter_config());
    // Advance into barter far enough that indirect acceptance has accrued but
    // stop before promotion so the tracker is still live.
    for _ in 0..120 {
        if !s.in_barter_phase() {
            break;
        }
        s.econ_tick();
    }
    assert!(
        s.in_barter_phase(),
        "the run must still be bartering so the tracker is live"
    );
    let emergence = s
        .society
        .emergence()
        .expect("a strong-bar run uses econ's emergence");
    let salt_indirect = emergence
        .tracker()
        .candidate_saleability()
        .find(|c| c.good == SALT)
        .map(|c| c.indirect_acceptances)
        .unwrap_or(0);
    assert!(
        salt_indirect > 0,
        "the test is vacuous — no indirect acceptance was observed"
    );

    let mut expected = Vec::new();
    push_emergence_runtime_bytes(&mut expected, emergence);
    let bytes = s.canonical_bytes();
    assert!(
        bytes
            .windows(expected.len())
            .any(|window| window == expected.as_slice()),
        "the accumulated indirect-acceptance runtime is missing from the canonical bytes"
    );
}

#[test]
#[should_panic(expected = "operating_cost must be at least 1")]
fn generate_rejects_zero_chain_operating_cost() {
    let mut config = SettlementConfig::emergent_chain();
    config.chain.as_mut().expect("chain").operating_cost = 0;
    let _ = Settlement::generate(7, &config);
}

#[test]
#[should_panic(expected = "exceeds the sanity bound")]
fn generate_rejects_absurd_chain_throughput() {
    // An unbounded throughput would let a config append arbitrarily many input
    // wants to every producer's value scale (an OOM at the extreme); generation
    // rejects it at the seam, like a zero operating cost.
    let mut config = SettlementConfig::emergent_chain();
    config.chain.as_mut().expect("chain").throughput = MAX_CHAIN_THROUGHPUT + 1;
    let _ = Settlement::generate(7, &config);
}

#[test]
fn role_choice_uses_fresh_scales_and_refreshes_changed_roles() {
    let mut s = Settlement::generate(2_026, &SettlementConfig::emergent_chain());

    let mut miller_slot = None;
    for _ in 0..12 {
        s.econ_tick();
        miller_slot =
            (0..s.population()).find(|&index| s.vocation_of(index) == Some(Vocation::Miller));
        if miller_slot.is_some() {
            break;
        }
    }
    let miller_slot = miller_slot.expect("milling emerged");
    let miller_id = s.colonist_id(miller_slot).expect("miller id");
    let content = s.content().expect("chain").clone();

    // Poison the live econ scale. If role-choice reads the stale scale before
    // SCALES, the miller sees no future savings want and incorrectly reverts.
    s.society
        .agents
        .get_mut(miller_id)
        .expect("miller resolves")
        .scale
        .clear();

    s.econ_tick();

    assert_eq!(
        s.vocation_of(miller_slot),
        Some(Vocation::Miller),
        "role-choice used the stale pre-regeneration scale"
    );
    let scale = &s
        .society
        .agents
        .get(miller_id)
        .expect("miller resolves")
        .scale;
    assert!(
        scale
            .iter()
            .any(|want| want.kind == WantKind::Good(content.grain())),
        "the post-adoption scale must be refreshed with active input wants"
    );
}

#[test]
fn latent_producer_anchors_its_tool_but_posts_no_input_bid() {
    // A latent (Unassigned) producer reserves only its tool — it never bids for
    // its recipe input, so it creates no autonomous demand for the intermediate
    // good (the property the no-spread control relies on). An adopted producer
    // does bid for input.
    let content = ContentSet::grain_flour_bread();
    let mut latent = vec![Want {
        kind: WantKind::Good(content.bread()),
        horizon: Horizon::Now,
        qty: 1,
        satisfied: false,
    }];
    producer_scale_extension(&mut latent, content.mill(), content.grain(), 0);
    assert!(
        !latent
            .iter()
            .any(|w| w.kind == WantKind::Good(content.grain())),
        "a latent producer must not post an input want"
    );
    assert!(
        latent
            .iter()
            .any(|w| w.kind == WantKind::Good(content.mill())),
        "a latent producer still anchors its tool (never sells its capital)"
    );

    let mut active = vec![Want {
        kind: WantKind::Good(content.bread()),
        horizon: Horizon::Now,
        qty: 1,
        satisfied: false,
    }];
    producer_scale_extension(&mut active, content.mill(), content.grain(), 2);
    assert_eq!(
        active
            .iter()
            .filter(|w| w.kind == WantKind::Good(content.grain()))
            .count(),
        2,
        "an active producer bids throughput units of its input"
    );
}

#[test]
fn recipe_adoption_pays_appraises_an_input_less_recipe() {
    // The reused G3a `Recipe` carries at most one input; an input-less recipe
    // (`input_good: None`) is NOT special-cased away — its input cost is zero, so
    // the appraisal reduces to the output spread against the operating cost alone.
    // The chain recipes (Mill, Bake) always carry an input, so this only
    // generalizes the input-less case rather than declining it outright.
    let content = ContentSet::grain_flour_bread();
    let free_recipe = Recipe {
        id: RecipeId::GatherFood,
        name: "Forage",
        labor: 1,
        input_good: None,
        required_tool: None,
        output_good: content.bread(),
        output_qty: 2,
        enabled: true,
    };
    let mut patient = Agent {
        id: AgentId(1),
        scale: vec![Want {
            kind: WantKind::Good(GOLD),
            horizon: Horizon::Later(4),
            qty: 1,
            satisfied: false,
        }],
        stock: Stock::new(NET.0),
        gold: Gold(0),
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: Vec::new(),
    };
    // An observable output price with an unprovisioned savings want and no input
    // cost still appraises (the input-less recipe is weighed, not auto-declined).
    assert!(
        recipe_adoption_pays(&patient, &free_recipe, Some(Gold(5)), None, 0, 1),
        "an input-less recipe with an output spread must still be appraised"
    );
    // Still ordinal: a gold-sated colonist declines the same spread.
    patient.gold = Gold(100);
    assert!(
        !recipe_adoption_pays(&patient, &free_recipe, Some(Gold(5)), None, 0, 1),
        "a sated colonist declines even an input-less spread (ordinal, not scalar)"
    );
}

// ---- S13.1: founders spatial at generation -------------------------------------

#[test]
fn spatial_households_flag_makes_founders_spatial() {
    // Flag off: `lineages` founders are econ-only — the world holds no colonist
    // agents (no gatherers/consumers/traders in this config), the pre-S13 model.
    let off = Settlement::generate(1, &SettlementConfig::lineages());
    assert!(
        off.world().agent_ids().is_empty(),
        "flag off: founders stay non-spatial (no world agents)"
    );

    // Flag on: every founder has a world agent at its EXACT econ id (world_id ==
    // econ_id by construction via add_agent_with_id).
    let mut cfg = SettlementConfig::lineages();
    cfg.demography.as_mut().unwrap().spatial_households = true;
    let on = Settlement::generate(1, &cfg);
    let founders = cfg.demography.as_ref().unwrap().founder_count();
    assert_eq!(
        on.world().agent_ids().len(),
        founders,
        "flag on: one world agent per founder"
    );
    for i in 0..on.population() {
        if on.is_alive(i) {
            let id = on.colonist_id(i).unwrap();
            assert!(
                on.world().agent_pos(id).is_some(),
                "every living founder has a world agent at world_id == econ_id"
            );
        }
    }
}

#[test]
fn spatial_households_founders_feed_and_conserve_unchanged() {
    // Flag on but no forage scarcity (`lineages` keeps its food hearth): the
    // spatial founders are inert (Idle world agents, never tasked), so feeding,
    // demography, and whole-system conservation match the non-spatial baseline
    // tick for tick — the milestone adds capability, not a behavior change.
    let base_cfg = SettlementConfig::lineages();
    let mut spatial_cfg = base_cfg.clone();
    spatial_cfg.demography.as_mut().unwrap().spatial_households = true;

    let mut base = Settlement::generate(7, &base_cfg);
    let mut spatial = Settlement::generate(7, &spatial_cfg);
    for tick in 0..200u64 {
        base.econ_tick();
        let report = spatial.econ_tick();
        assert!(
            report.conserves(),
            "spatial founders must conserve at tick {tick}"
        );
        assert_eq!(
            base.population(),
            spatial.population(),
            "population diverged at tick {tick}"
        );
        for i in 0..base.population() {
            assert_eq!(
                base.is_alive(i),
                spatial.is_alive(i),
                "liveness at {i}/{tick}"
            );
            assert_eq!(
                base.need_of(i).map(|n| (n.hunger, n.warmth, n.rest)),
                spatial.need_of(i).map(|n| (n.hunger, n.warmth, n.rest)),
                "feeding diverged for colonist {i} at tick {tick}"
            );
        }
    }
}

// ---- S13.2: newborns spatial at birth ------------------------------------------

#[test]
fn spatial_newborn_mirrors_a_reused_slot_after_a_death() {
    // A long `lineages` run with spatial households: every newborn — including those
    // born AFTER a death recycled an arena slot (a reused `slot#gen` id) — gets a
    // world agent at its EXACT econ id, world_id == econ_id mid-run. The slot's prior
    // world occupant was removed on death (`collect_estate`), so there is no leak: the
    // world's live agent count never exceeds the living colonist roster (lineages has
    // no traders/roster, so every world agent is a lineage member).
    let mut cfg = SettlementConfig::lineages();
    cfg.demography.as_mut().unwrap().spatial_households = true;
    let mut s = Settlement::generate(7, &cfg);

    let mut saw_reused_newborn = false;
    for tick in 0..600u64 {
        s.econ_tick();
        let mut living = 0usize;
        for i in 0..s.population() {
            if !s.is_alive(i) {
                continue;
            }
            living += 1;
            let id = s.colonist_id(i).unwrap();
            assert!(
                s.world().agent_pos(id).is_some(),
                "living colonist {id} lacks a world agent at its exact id (tick {tick})"
            );
            if id.generation() >= 1 {
                // Born into a reused arena slot (a death freed it first) — the crux.
                saw_reused_newborn = true;
            }
        }
        assert_eq!(
            s.world().agent_ids().len(),
            living,
            "world-agent leak: live world agents must equal living colonists (tick {tick})"
        );
    }
    assert!(
        saw_reused_newborn,
        "the run must exercise a newborn born at a reused slot#gen (birth after death)"
    );
}

#[test]
fn canonical_bytes_include_spatial_households() {
    // The flag (and its founder world agents) must register in the determinism
    // surface, so two settlements differing only in it never digest equal; with the
    // flag off the byte layout is the exact pre-S13 stream (the `lineages` golden).
    let off = Settlement::generate(1, &SettlementConfig::lineages());
    let mut cfg = SettlementConfig::lineages();
    cfg.demography.as_mut().unwrap().spatial_households = true;
    let on = Settlement::generate(1, &cfg);
    assert_ne!(
        off.canonical_bytes(),
        on.canonical_bytes(),
        "the spatial-households flag must register in canonical_bytes"
    );
    assert_ne!(off.digest(), on.digest());
}
