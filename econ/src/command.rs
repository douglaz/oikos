//! Command result/error semantics — player input cannot silently no-op.
//!
//! G0b migration (game-spec §7): authored scenario *events* may silently
//! tolerate missing targets (an unknown debt id, a no-issuer levy), and that
//! tolerance is load-bearing for scripted scenarios. Player *commands* must
//! not — they have to report `Applied | Rejected(reason)`.
//!
//! These types are **additive**: [`crate::society::Society::apply_command`]
//! wraps the very same mutation logic the event path runs, but returns the
//! result instead of discarding it. The event path is untouched (see
//! `docs/engine-divergence.md`). Nothing in `econ` calls `apply_command` yet
//! besides tests — it is plumbing for the sim crate's future command queue.

/// Why a command did nothing — one named reason per currently-silent event
/// no-op.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejectReason {
    /// The referenced agent is not in the society.
    UnknownAgent,
    /// The referenced debt id does not exist.
    UnknownDebt,
    /// The referenced bank does not exist.
    UnknownBank,
    /// The referenced issuer does not exist.
    UnknownIssuer,
    /// The referenced recipe does not exist.
    UnknownRecipe,
    /// A levy needs exactly one issuer; the society has zero or several.
    NoIssuer,
    /// The event has no meaning for this kernel (e.g. redemption without a
    /// money system in an M0 autarky).
    NotApplicableToKernel,
    /// The event is well-formed but its preconditions reject it (zero amount,
    /// empty route, exhausted issue cap, …).
    Ineligible,
}

/// A rejection: the reason plus a short human-readable detail (never serialized
/// into a golden — commands have no tape surface in G0b).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandRejection {
    pub reason: RejectReason,
    pub detail: String,
}

/// The outcome of applying a command. `Applied` means the same mutation the
/// event path performs ran; `Rejected` names why nothing changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandResult {
    Applied,
    Rejected(CommandRejection),
}

impl CommandResult {
    /// Build a `Rejected` result.
    pub fn rejected(reason: RejectReason, detail: impl Into<String>) -> Self {
        CommandResult::Rejected(CommandRejection {
            reason,
            detail: detail.into(),
        })
    }

    /// Whether the command applied.
    pub fn is_applied(&self) -> bool {
        matches!(self, CommandResult::Applied)
    }

    /// Whether the command was rejected.
    pub fn is_rejected(&self) -> bool {
        matches!(self, CommandResult::Rejected(_))
    }

    /// The rejection, if any.
    pub fn rejection(&self) -> Option<&CommandRejection> {
        match self {
            CommandResult::Applied => None,
            CommandResult::Rejected(rejection) => Some(rejection),
        }
    }

    /// The rejection reason, if any.
    pub fn reason(&self) -> Option<RejectReason> {
        self.rejection().map(|rejection| rejection.reason)
    }
}
