//! G0b engine-migrations acceptance tests (`docs/impl-g0b.md`).
//!
//! Three migrations behind a compatibility surface — a dynamic `GoodRegistry`,
//! a generational `AgentId` arena, and additive `Command` result semantics —
//! each proven to leave the lab's observable surface untouched. The series
//! goldens (M0/M1/M2/M3), the M18/M20 emergence goldens, and the M5/M6 anchors
//! live in their own files and pass UNMODIFIED; this file proves the
//! compatibility properties those goldens rest on, plus the new surfaces.

use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::arena::AgentArena;
use econ::cantillon::CantillonRoute;
use econ::command::{CommandResult, RejectReason};
use econ::good::{
    good_name, Gold, GoodId, Horizon, Stock, CLOTH, FOOD, GOLD, NET, ORE, SALT, WOOD,
};
use econ::issuer::IssuerPolicy;
use econ::ledger::{BankId, IssuerId};
use econ::money::{PublicSpotTender, Regime};
use econ::project::Tick;
use econ::purpose::DebtPurpose;
use econ::registry::GoodRegistry;
use econ::scenario::{builtin_market_scenario, Event, EventKind, RedemptionRoute, ScenarioName};
use econ::society::Society;
use econ::timemarket::DebtId;

const LAB_GOODS: [(GoodId, &str); 7] = [
    (GOLD, "gold"),
    (FOOD, "food"),
    (WOOD, "wood"),
    (NET, "net"),
    (SALT, "salt"),
    (CLOTH, "cloth"),
    (ORE, "ore"),
];

fn arena_agent(id: u32) -> Agent {
    Agent {
        id: AgentId(u64::from(id)),
        scale: vec![Want {
            kind: WantKind::Good(FOOD),
            horizon: Horizon::Now,
            qty: 1,
            satisfied: false,
        }],
        stock: Stock::new(6),
        gold: Gold::ZERO,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: Vec::new(),
    }
}

/// Tiny deterministic LCG — pure std, reproducible across runs.
struct Lcg(u64);

impl Lcg {
    fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        (self.0 >> 32) as u32
    }
}

/// Acceptance 1: the lab-default registry reproduces the legacy good constants
/// one by one — names, ids, and count. (The byte-identical goldens themselves
/// are enforced by the dedicated M0–M21 test files, unmodified.)
#[test]
fn goldens_hold_natively() {
    let registry = GoodRegistry::lab_default();
    assert_eq!(registry.len(), LAB_GOODS.len());

    for (expected_id, (good, name)) in LAB_GOODS.iter().enumerate() {
        // Exact id order: the constant's id equals its catalog index.
        assert_eq!(usize::from(good.0), expected_id);
        // Name through the registry matches the legacy `good_name` shim.
        assert_eq!(registry.name(*good), *name);
        assert_eq!(registry.name(*good), good_name(*good));
        // Round trip name -> id.
        assert_eq!(registry.id_of(name), Some(*good));
    }

    // A lab-constructed society carries the same catalog through its
    // registry-aware accessors.
    let society =
        Society::from_scenario(builtin_market_scenario(ScenarioName::MarketPriceDiscovery));
    assert_eq!(society.good_registry().len(), 7);
    assert_eq!(society.good_name(GOLD), "gold");
    assert_eq!(society.good_name(ORE), good_name(ORE));
}

/// Acceptance 2: `AgentId` widens to packed `(generation, index)` without
/// disturbing the lab's generation-0 surface.
#[test]
fn agent_id_packing_is_compatible() {
    // A bare literal still compiles and means index 212, generation 0.
    let id = AgentId(212);
    assert_eq!(id.index(), 212);
    assert_eq!(id.generation(), 0);
    assert_eq!(AgentId::with_generation(212, 0), id);

    // Generation-0 ordering equals u32 ordering.
    let sparse = [400u32, 1, 215, 100, 7, 300];
    let mut ids: Vec<AgentId> = sparse.iter().map(|&n| AgentId(u64::from(n))).collect();
    ids.sort();
    let mut expected = sparse;
    expected.sort_unstable();
    assert_eq!(
        ids.iter().map(|id| id.index()).collect::<Vec<_>>(),
        expected.to_vec()
    );

    // Generation-0 formatting is digit-identical to the bare index — this is
    // the surface every tape and CSV path depends on.
    for n in [0u32, 1, 212, 400, u32::MAX] {
        assert_eq!(AgentId(u64::from(n)).to_string(), n.to_string());
    }

    // A nonzero generation orders AFTER its generation-0 ancestor and formats
    // distinguishably — a brand-new surface no golden can reference.
    let gen0 = AgentId(212);
    let gen1 = AgentId::with_generation(212, 1);
    assert!(gen1 > gen0);
    assert_eq!(gen1.index(), 212);
    assert_eq!(gen1.generation(), 1);
    assert_ne!(gen1.to_string(), gen0.to_string());
    assert_eq!(gen1.to_string(), "212#1");
    // Even a regenerated low id sorts after a higher generation-0 id.
    assert!(AgentId::with_generation(5, 1) > AgentId(300));
}

/// Acceptance 3: the arena matches the legacy `Vec` + id-order construction
/// when nothing dies (the lab case) — iteration order, lookups, and count.
#[test]
fn arena_matches_vec_semantics_when_nothing_dies() {
    let mut lcg = Lcg(0xA11C_E5ED);

    for round in 0..40 {
        let mut ids = Vec::new();
        let count = 1 + (lcg.next_u32() % 24) as usize;
        while ids.len() < count {
            let candidate = lcg.next_u32() % 500;
            if !ids.contains(&candidate) {
                ids.push(candidate);
            }
        }

        // Legacy construction: cast order plus an id-sorted iteration order.
        let cast: Vec<Agent> = ids.iter().map(|&id| arena_agent(id)).collect();
        let mut legacy_order = ids.clone();
        legacy_order.sort_unstable();

        let arena = AgentArena::from_cast(cast.clone());

        assert_eq!(arena.len(), cast.len(), "round {round} count");
        assert_eq!(
            arena.iter().map(|a| a.id.index()).collect::<Vec<_>>(),
            legacy_order,
            "round {round} id-order iteration"
        );

        for &id in &ids {
            let agent_id = AgentId(u64::from(id));
            assert_eq!(arena.get(agent_id).map(|a| a.id), Some(agent_id));
            let position = arena.position_of(agent_id).expect("position resolves");
            assert_eq!(arena.as_slice()[position].id, agent_id);
        }

        // A regenerated id never resolves against a never-freed cast.
        assert!(arena.get(AgentId::with_generation(ids[0], 1)).is_none());
    }
}

/// Acceptance 4: insert/free/insert reuses the slot with a bumped generation;
/// the stale id resolves to `None`, the new id resolves, iteration never yields
/// freed agents.
#[test]
fn arena_reuse_bumps_generation() {
    let mut arena = AgentArena::new();
    arena.insert_with_id(arena_agent(2));
    arena.insert_with_id(arena_agent(5));

    let fresh = arena.insert(arena_agent(0));
    assert_eq!(fresh, AgentId::with_generation(6, 0));

    let freed = arena.free(fresh).expect("fresh id frees");
    assert_eq!(freed.id, fresh);
    assert!(arena.get(fresh).is_none(), "stale id resolves to None");
    assert!(arena.iter().all(|a| a.id != fresh), "iteration drops freed");

    let reused = arena.insert(arena_agent(0));
    assert_eq!(reused, AgentId::with_generation(6, 1));
    assert_eq!(reused.index(), fresh.index());
    assert!(arena.get(reused).is_some(), "new id resolves");
    assert!(
        arena.get(fresh).is_none(),
        "stale id stays None after reuse"
    );
    assert!(arena.free(fresh).is_none(), "freeing a stale id is a no-op");

    assert_eq!(
        arena.iter().map(|a| a.id.index()).collect::<Vec<_>>(),
        vec![2, 5, 6]
    );
}

/// Acceptance 5: interning is stable (existing names keep their id), new names
/// extend, and `len` drives `Stock` sizing equal to the legacy constant-derived
/// size for `lab_default`.
#[test]
fn registry_intern_is_stable() {
    let mut registry = GoodRegistry::lab_default();
    let len_before = registry.len();

    assert_eq!(registry.intern("gold"), GOLD);
    assert_eq!(registry.intern("ore"), ORE);
    assert_eq!(registry.len(), len_before);

    let timber = registry.intern("timber");
    assert_eq!(usize::from(timber.0), len_before);
    assert_eq!(registry.len(), len_before + 1);
    assert_eq!(registry.intern("timber"), timber);

    // `len` is the slot-count source; for `lab_default` it equals the legacy
    // constant-derived size (ORE = 6 → 7 slots).
    let lab = GoodRegistry::lab_default();
    let from_registry = Stock::new((lab.len() - 1) as u16);
    let from_constants = Stock::new(ORE.0);
    assert_eq!(from_registry, from_constants);
}

/// Acceptance 6: commands reject loudly where authored events stay silent. Each
/// known silent no-op returns a named reason through `apply_command`; the event
/// path still tolerates the same no-op silently; an applied command mutates the
/// same field the event path would.
#[test]
fn commands_reject_loudly_where_events_are_silent() {
    // M1 price discovery: no issuers, no money system, no debts — the kernel
    // where each silent no-op is reachable.
    let mut society =
        Society::from_scenario(builtin_market_scenario(ScenarioName::MarketPriceDiscovery));
    assert!(society.issuers.is_empty(), "precondition: no issuers");
    assert!(society.debts.is_empty(), "precondition: no debts");

    // Unknown debt.
    let unknown_debt = EventKind::SetDebtDueTick {
        debt: DebtId(9_999),
        due_tick: Tick(7),
    };
    let result = society.apply_command(unknown_debt.clone());
    assert_eq!(result.reason(), Some(RejectReason::UnknownDebt));
    assert!(society.debts.is_empty(), "rejected command mutated nothing");

    // No-issuer levy.
    let levy = EventKind::LevyTax {
        agent: AgentId(1),
        amount: Gold(5),
        due_tick: Tick(7),
    };
    let result = society.apply_command(levy.clone());
    assert_eq!(result.reason(), Some(RejectReason::NoIssuer));
    assert!(society.debts.is_empty(), "no tax debt seeded on rejection");

    // Unknown commodity-debt parties.
    assert!(society.agents.get(AgentId(1)).is_some(), "agent 1 exists");
    let unknown_lender_debt = EventKind::SeedCommodityDebt {
        lender: AgentId(9_999),
        borrower: AgentId(1),
        principal: Gold(1),
        due: Gold(2),
        due_tick: Tick(7),
        purpose: DebtPurpose::Consumption,
    };
    let result = society.apply_command(unknown_lender_debt);
    assert_eq!(result.reason(), Some(RejectReason::UnknownAgent));
    assert!(society.debts.is_empty(), "unknown lender seeded no debt");

    let unknown_borrower_debt = EventKind::SeedCommodityDebt {
        lender: AgentId(1),
        borrower: AgentId(9_999),
        principal: Gold(1),
        due: Gold(2),
        due_tick: Tick(7),
        purpose: DebtPurpose::Consumption,
    };
    let result = society.apply_command(unknown_borrower_debt);
    assert_eq!(result.reason(), Some(RejectReason::UnknownAgent));
    assert!(society.debts.is_empty(), "unknown borrower seeded no debt");

    // Inapplicable to this kernel: redemption without a money system.
    let redeem = EventKind::RedeemDemandClaims {
        bank: BankId(1),
        route: RedemptionRoute::AllClaimHolders,
        max_per_agent: None,
    };
    let result = society.apply_command(redeem);
    assert_eq!(result.reason(), Some(RejectReason::NotApplicableToKernel));

    // Missing all-claim-holders bank in a money-system society is a rejected
    // command, not an applied empty loop.
    let mut money_society = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldTaxSpecieControl,
    ));
    assert!(
        money_society.money_system.is_some(),
        "precondition: M3 money"
    );
    assert!(money_society.banks.is_empty(), "precondition: no banks");
    let result = money_society.apply_command(EventKind::RedeemDemandClaims {
        bank: BankId(9_999),
        route: RedemptionRoute::AllClaimHolders,
        max_per_agent: None,
    });
    assert_eq!(result.reason(), Some(RejectReason::UnknownBank));
    assert!(
        money_society.redemption_audit.is_empty(),
        "missing all-holders bank emitted no audit rows"
    );

    // Levy with an issuer but a missing target agent rejects instead of
    // creating an unpayable liability.
    let result = money_society.apply_command(EventKind::LevyTax {
        agent: AgentId(9_999),
        amount: Gold(5),
        due_tick: Tick(7),
    });
    assert_eq!(result.reason(), Some(RejectReason::UnknownAgent));
    assert!(
        money_society.debts.is_empty(),
        "unknown tax target seeded no debt"
    );

    // An applicable command returns Applied and performs its mutation.
    let applied =
        society.apply_command(EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly));
    assert_eq!(applied, CommandResult::Applied);
    assert!(applied.is_applied());
    assert_eq!(society.public_spot_tender, PublicSpotTender::SpecieOnly);

    // The EVENT path stays silent on the same no-ops: scheduling them and
    // running tolerates both with no debt and no panic (result discarded).
    let mut scenario = builtin_market_scenario(ScenarioName::MarketPriceDiscovery);
    scenario.events.push(Event {
        tick: Tick(0),
        kind: unknown_debt,
    });
    scenario.events.push(Event {
        tick: Tick(0),
        kind: levy,
    });
    let mut event_society = Society::from_scenario(scenario);
    event_society.run(1);
    assert!(
        event_society.debts.is_empty(),
        "event path silently tolerated both no-ops"
    );

    // Applied command and the scheduled-event path mutate the same field
    // identically — they share one implementation.
    let mut command_society =
        Society::from_scenario(builtin_market_scenario(ScenarioName::MarketPriceDiscovery));
    let mut scheduled = builtin_market_scenario(ScenarioName::MarketPriceDiscovery);
    scheduled.events.push(Event {
        tick: Tick(0),
        kind: EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly),
    });
    let mut event_path_society = Society::from_scenario(scheduled);
    let _ =
        command_society.apply_command(EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly));
    event_path_society.run(1);
    assert_eq!(
        command_society.public_spot_tender,
        event_path_society.public_spot_tender
    );
}

/// A command that names a missing bank rejects loudly *regardless of route* —
/// including the explicit `Agents` route, whose non-empty requester list used to
/// slip past the missing-bank check and return `Applied` after writing
/// `BankMissing` audit rows. The event path keeps that lab audit behavior (see
/// the in-crate `targeted_redemption_route_records_explicit_failures`); the
/// command path must reject and mutate nothing.
#[test]
fn commands_reject_missing_bank_for_explicit_agents_route() {
    let mut money_society = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldTaxSpecieControl,
    ));
    assert!(
        money_society.money_system.is_some(),
        "precondition: M3 money"
    );
    assert!(money_society.banks.is_empty(), "precondition: no banks");

    // The explicit route names a real agent, so the requester list is non-empty
    // — the exact shape that bypassed the old missing-bank guard.
    let target = money_society
        .agents
        .iter()
        .next()
        .expect("scenario has agents")
        .id;
    let result = money_society.apply_command(EventKind::RedeemDemandClaims {
        bank: BankId(9_999),
        route: RedemptionRoute::Agents(vec![target]),
        max_per_agent: None,
    });
    assert_eq!(result.reason(), Some(RejectReason::UnknownBank));
    assert!(
        money_society.redemption_audit.is_empty(),
        "rejected command wrote no BankMissing audit rows"
    );
}

/// Command-mode redemption validates the requester set before touching the
/// redemption audit: empty explicit routes, all-holder routes with no holders,
/// and unknown explicit agents are rejected loudly. Authored events keep their
/// silent/audit semantics in the in-crate redemption tests.
#[test]
fn commands_reject_redemption_routes_without_live_requesters() {
    let mut society = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldRedemptionRun,
    ));
    assert!(
        society.banks.iter().any(|bank| bank.id == BankId(1)),
        "precondition: bank exists"
    );
    assert!(
        society.money_system.is_some(),
        "precondition: money system exists"
    );
    assert!(
        society
            .money_system
            .as_ref()
            .expect("money system")
            .demand_claim_holders(BankId(1))
            .is_empty(),
        "precondition: no initial demand-claim holders"
    );

    let all_holders = society.apply_command(EventKind::RedeemDemandClaims {
        bank: BankId(1),
        route: RedemptionRoute::AllClaimHolders,
        max_per_agent: None,
    });
    assert_eq!(all_holders.reason(), Some(RejectReason::Ineligible));
    assert!(society.redemption_audit.is_empty());

    let empty_agents = society.apply_command(EventKind::RedeemDemandClaims {
        bank: BankId(1),
        route: RedemptionRoute::Agents(Vec::new()),
        max_per_agent: None,
    });
    assert_eq!(empty_agents.reason(), Some(RejectReason::Ineligible));
    assert!(society.redemption_audit.is_empty());

    let unknown_agent = society.apply_command(EventKind::RedeemDemandClaims {
        bank: BankId(1),
        route: RedemptionRoute::Agents(vec![AgentId(9_999)]),
        max_per_agent: None,
    });
    assert_eq!(unknown_agent.reason(), Some(RejectReason::UnknownAgent));
    assert!(society.redemption_audit.is_empty());
}

/// A zero-amount tax levy is a command-only rejection: with exactly one issuer
/// and a live target it is well-formed, but it would seed an open zero-due
/// liability that mutates nothing meaningful. The command path rejects it
/// (`Ineligible`); the authored event path keeps the lab's unconditional seed
/// (asserted below) — the existence/amount precondition is command-only.
#[test]
fn commands_reject_zero_amount_levy() {
    let mut society = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldTaxSpecieControl,
    ));
    let target = society
        .agents
        .iter()
        .next()
        .expect("scenario has agents")
        .id;
    assert_eq!(society.issuers.len(), 1, "precondition: exactly one issuer");

    let debts_before = society.debts.len();
    let result = society.apply_command(EventKind::LevyTax {
        agent: target,
        amount: Gold::ZERO,
        due_tick: Tick(10_000),
    });
    assert_eq!(result.reason(), Some(RejectReason::Ineligible));
    assert_eq!(
        society.debts.len(),
        debts_before,
        "rejected zero levy seeded no debt"
    );

    // The event path keeps the lab's unconditional seed: a scheduled zero levy
    // against the same live target seeds an open zero-due tax debt.
    let mut scenario = builtin_market_scenario(ScenarioName::EmergedGoldTaxSpecieControl);
    scenario.events.push(Event {
        tick: Tick(0),
        kind: EventKind::LevyTax {
            agent: target,
            amount: Gold::ZERO,
            due_tick: Tick(10_000),
        },
    });
    let mut event_society = Society::from_scenario(scenario);
    event_society.run(1);
    assert!(
        event_society
            .debts
            .iter()
            .any(|debt| debt.borrower == target
                && debt.due == Gold::ZERO
                && debt.purpose == DebtPurpose::TaxLiability),
        "event path seeded the zero-due tax debt"
    );
}

/// A zero per-agent redemption cap requests nothing: in command mode it would
/// take the loop's zero-request `continue` for every holder and fall through to
/// `Applied` having mutated nothing. The command path rejects it (`Ineligible`)
/// before building the requester set, regardless of route. The authored event
/// path keeps its silent tolerance (covered by the in-crate redemption tests).
#[test]
fn commands_reject_zero_cap_redemption() {
    let mut society = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldRedemptionRun,
    ));
    assert!(
        society.banks.iter().any(|bank| bank.id == BankId(1)),
        "precondition: bank exists"
    );
    let target = society
        .agents
        .iter()
        .next()
        .expect("scenario has agents")
        .id;

    let all_holders = society.apply_command(EventKind::RedeemDemandClaims {
        bank: BankId(1),
        route: RedemptionRoute::AllClaimHolders,
        max_per_agent: Some(Gold::ZERO),
    });
    assert_eq!(all_holders.reason(), Some(RejectReason::Ineligible));
    assert!(society.redemption_audit.is_empty());

    let explicit = society.apply_command(EventKind::RedeemDemandClaims {
        bank: BankId(1),
        route: RedemptionRoute::Agents(vec![target]),
        max_per_agent: Some(Gold::ZERO),
    });
    assert_eq!(explicit.reason(), Some(RejectReason::Ineligible));
    assert!(
        society.redemption_audit.is_empty(),
        "rejected zero-cap redemption wrote no audit rows"
    );
}

/// Command-mode fiat prints reject explicit agent routes containing unknown
/// recipients before issuing money. The authored event path still uses the
/// Cantillon router's historical filtering behavior.
#[test]
fn commands_reject_agent_routed_fiat_prints_with_unknown_recipients() {
    let mut society = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldTaxSpecieControl,
    ));
    let recipient = society
        .agents
        .iter()
        .next()
        .expect("scenario has agents")
        .id;

    assert_eq!(
        society.apply_command(EventKind::SetRegime(Regime::Fiat)),
        CommandResult::Applied
    );
    assert_eq!(
        society.apply_command(EventKind::SetIssuerPolicy {
            issuer: IssuerId(1),
            policy: IssuerPolicy {
                fiscal_enabled: true,
                credit_enabled: false,
                max_fiscal_issue_per_tick: Gold(8),
                max_credit_issue_per_tick: Gold::ZERO,
                loan_present: Gold::ZERO,
                loan_horizon: 0,
                loan_future_due: Gold::ZERO,
            },
        }),
        CommandResult::Applied
    );

    let mixed_route = society.apply_command(EventKind::FiatPrint {
        issuer: IssuerId(1),
        amount: Gold(8),
        route: CantillonRoute::Agents(vec![recipient, AgentId(9_999)]),
    });
    assert_eq!(mixed_route.reason(), Some(RejectReason::UnknownAgent));
    assert_eq!(society.issuers[0].fiat_issued, Gold::ZERO);
    assert_eq!(
        society
            .money_system
            .as_ref()
            .expect("money system")
            .public_fiat(recipient),
        Gold::ZERO
    );
    assert!(society.cantillon_receipts.is_empty());

    let empty_route = society.apply_command(EventKind::FiatPrint {
        issuer: IssuerId(1),
        amount: Gold(8),
        route: CantillonRoute::Agents(Vec::new()),
    });
    assert_eq!(empty_route.reason(), Some(RejectReason::Ineligible));
    assert_eq!(society.issuers[0].fiat_issued, Gold::ZERO);
    assert!(society.cantillon_receipts.is_empty());
}

/// A `StopBankCredit` / `StopIssuerCredit` command for a target that does not
/// exist rejects *before* the unconditional lender-quote cancel, so a rejected
/// command is side-effect-free. The event path keeps the lab's unconditional
/// cancel.
#[test]
fn commands_reject_stop_credit_for_missing_target() {
    let mut society =
        Society::from_scenario(builtin_market_scenario(ScenarioName::MarketPriceDiscovery));
    assert!(society.banks.is_empty(), "precondition: no banks");
    assert!(society.issuers.is_empty(), "precondition: no issuers");

    let bank = society.apply_command(EventKind::StopBankCredit {
        bank: BankId(9_999),
    });
    assert_eq!(bank.reason(), Some(RejectReason::UnknownBank));

    let issuer = society.apply_command(EventKind::StopIssuerCredit {
        issuer: IssuerId(9_999),
    });
    assert_eq!(issuer.reason(), Some(RejectReason::UnknownIssuer));
}

/// The event path stays byte-for-byte the lab's: it seeds a commodity debt even
/// when neither party is a live agent (the lab's load-bearing silent tolerance),
/// where the command path rejects (`UnknownAgent`, tested above). This is the
/// asymmetry the migration must preserve — the existence precondition is
/// command-only.
#[test]
fn event_path_seeds_debt_against_missing_agent() {
    let mut scenario = builtin_market_scenario(ScenarioName::MarketPriceDiscovery);
    // Far-future due tick: the debt is seeded and simply rests for the run.
    scenario.events.push(Event {
        tick: Tick(0),
        kind: EventKind::SeedCommodityDebt {
            lender: AgentId(9_999),
            borrower: AgentId(9_998),
            principal: Gold(1),
            due: Gold(2),
            due_tick: Tick(10_000),
            purpose: DebtPurpose::Consumption,
        },
    });
    let mut society = Society::from_scenario(scenario);
    society.run(1);
    assert_eq!(
        society.debts.len(),
        1,
        "event path seeded the debt despite missing parties"
    );
    let debt = &society.debts[0];
    assert_eq!(debt.borrower, AgentId(9_998));
    assert_eq!(debt.due, Gold(2));
}

/// Acceptance 7: the conformance suite is green natively and deterministic.
/// Byte-identity of the goldens is enforced by the dedicated M0–M21 files (run
/// by `cargo test`); here we additionally confirm a lab scenario replays
/// bit-for-bit across two independent constructions through the migrated
/// arena/registry engine. `cargo clippy --workspace --all-targets -- -D
/// warnings` and `cargo fmt --check` gate the rest.
#[test]
fn conformance_replays_deterministically() {
    for name in [
        ScenarioName::MarketPriceDiscovery,
        ScenarioName::MarketBarterishGold,
    ] {
        let mut a = Society::from_scenario(builtin_market_scenario(name));
        let mut b = Society::from_scenario(builtin_market_scenario(name));
        let periods = builtin_market_scenario(name).periods;
        a.run(periods);
        b.run(periods);
        assert_eq!(a.records, b.records, "{name:?} replays deterministically");
        assert_eq!(a.total_gold(), b.total_gold());
        // The migrated engine still iterates colonists in id order.
        let ids: Vec<u32> = a.agents.iter().map(|agent| agent.id.index()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted, "{name:?} arena iterates in id order");
    }
}
