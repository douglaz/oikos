use econ::agent::{Agent, AgentId, WantKind};
use econ::cantillon::CantillonRoute;
use econ::good::{Gold, GoodId, Horizon, GOLD};
use econ::money::{PublicSpotTender, Regime};
use econ::record::PaymentAuditRecord;
use econ::scenario::{
    builtin_market_scenario, emerged_gold_bridge_agents, EventKind, ScenarioName,
};
use econ::shadow::credit_disabled_scenario;
use econ::society::Society;

const BRIDGE_GOLD: Gold = Gold(16);
const FIRST_RECEIVERS: [AgentId; 2] = [AgentId(7), AgentId(8)];

#[derive(Clone, Debug, PartialEq, Eq)]
struct SeedRow {
    agent: AgentId,
    gold: Gold,
    stock: Vec<(GoodId, u32)>,
    scale: Vec<(WantKind, Horizon, u32)>,
}

#[test]
fn m11_scenarios_preserve_m6_bridge_prefix() {
    let bridge = emerged_gold_bridge_agents();
    let expected = seed_rows(&bridge);

    assert_eq!(bridge_gold(&bridge), BRIDGE_GOLD);
    assert!(bridge.iter().all(|agent| agent.stock.get(GOLD) == 0));

    for name in m11_scenarios() {
        let scenario = builtin_market_scenario(name);
        let prefix = scenario
            .agents
            .iter()
            .take(bridge.len())
            .cloned()
            .collect::<Vec<_>>();

        assert_eq!(seed_rows(&prefix), expected, "{name:?} bridge prefix");
        assert_eq!(bridge_gold(&prefix), BRIDGE_GOLD, "{name:?} bridge gold");
        assert!(prefix.iter().all(|agent| agent.stock.get(GOLD) == 0));
    }
}

#[test]
fn m11_scenarios_have_issuer_no_bank_no_claims() {
    for name in m11_scenarios() {
        let society = run(name);

        assert_eq!(society.issuers.len(), 1);
        assert!(society.banks.is_empty());
        assert!(society.m3_records.iter().all(|record| {
            record.demand_claims == Gold::ZERO
                && record.fiduciary == Gold::ZERO
                && record.bank_reserves == Gold::ZERO
                && record.time_deposits == Gold::ZERO
        }));
    }
}

#[test]
fn m11_scenarios_share_same_fiat_print() {
    let refusal = builtin_market_scenario(ScenarioName::EmergedGoldFiatRefusalControl);
    let legal = builtin_market_scenario(ScenarioName::EmergedGoldFiatLegalTender);

    assert_eq!(fiat_print(&refusal.events), fiat_print(&legal.events));
    assert_eq!(
        fiat_print(&refusal.events),
        (Gold(8), CantillonRoute::Agents(FIRST_RECEIVERS.to_vec()))
    );
    assert!(matches!(
        refusal.events[3].kind,
        EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly)
    ));
    assert!(matches!(
        legal.events[3].kind,
        EventKind::SetPublicSpotTender(PublicSpotTender::FiatAndSpecie)
    ));
}

#[test]
fn refusal_control_prints_fiat_but_uses_no_fiat_in_spot_payments() {
    let society = run(ScenarioName::EmergedGoldFiatRefusalControl);

    assert!(society.m3_records.iter().all(|record| {
        record.public_fiat == Gold(8)
            && record.tms == Gold(24)
            && record.regime == Regime::Fiat
            && record.demand_claims == Gold::ZERO
    }));
    assert!(society
        .payment_audit
        .iter()
        .all(|row| row.public_fiat == Gold::ZERO
            && row.demand_claims == Gold::ZERO
            && row.tender == PublicSpotTender::SpecieOnly));
}

#[test]
fn legal_tender_spends_fiat_before_specie() {
    let mut society = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldFiatLegalTender,
    ));
    society.step();

    assert!(society
        .payment_audit
        .iter()
        .any(|row| row.public_fiat > Gold::ZERO));
    assert!(society.payment_audit.iter().all(|row| {
        row.demand_claims == Gold::ZERO && row.tender == PublicSpotTender::FiatAndSpecie
    }));
    assert!(society.payment_audit.iter().any(|row| {
        FIRST_RECEIVERS.contains(&row.from)
            && row.public_fiat > Gold::ZERO
            && row.public_specie == Gold::ZERO
    }));
    for receiver in FIRST_RECEIVERS {
        assert_eq!(public_specie(&society, receiver), Gold(4));
    }
}

#[test]
fn legal_tender_differs_from_refusal_only_by_acceptance_path() {
    let refusal_scenario = builtin_market_scenario(ScenarioName::EmergedGoldFiatRefusalControl);
    let legal_scenario = builtin_market_scenario(ScenarioName::EmergedGoldFiatLegalTender);
    let bridge_len = emerged_gold_bridge_agents().len();

    assert_eq!(
        seed_rows(&refusal_scenario.agents[..bridge_len]),
        seed_rows(&legal_scenario.agents[..bridge_len])
    );
    assert_eq!(
        fiat_print(&refusal_scenario.events),
        fiat_print(&legal_scenario.events)
    );
    assert_eq!(
        issuer_policy_event(&refusal_scenario.events),
        issuer_policy_event(&legal_scenario.events)
    );

    let refusal = run(ScenarioName::EmergedGoldFiatRefusalControl);
    let legal = run(ScenarioName::EmergedGoldFiatLegalTender);

    assert!(refusal.banks.is_empty());
    assert!(legal.banks.is_empty());
    assert_eq!(sum_fiat_payments(&refusal.payment_audit), Gold::ZERO);
    assert!(sum_fiat_payments(&legal.payment_audit) > Gold::ZERO);
}

#[test]
fn m11_shadow_removes_fiat_print_but_preserves_tender_policy() {
    for (name, tender) in [
        (
            ScenarioName::EmergedGoldFiatRefusalControl,
            PublicSpotTender::SpecieOnly,
        ),
        (
            ScenarioName::EmergedGoldFiatLegalTender,
            PublicSpotTender::FiatAndSpecie,
        ),
    ] {
        let scenario = builtin_market_scenario(name);
        let shadow = credit_disabled_scenario(&scenario);

        assert!(shadow
            .events
            .iter()
            .any(|event| matches!(event.kind, EventKind::SetRegime(Regime::Fiat))));
        assert!(shadow.events.iter().any(
            |event| matches!(event.kind, EventKind::SetPublicSpotTender(policy) if policy == tender)
        ));
        assert!(shadow
            .events
            .iter()
            .all(|event| !matches!(event.kind, EventKind::FiatPrint { .. })));

        let periods = shadow.periods;
        let mut society = Society::from_scenario(shadow);
        society.run(periods);

        assert_eq!(society.public_spot_tender, tender);
        assert!(society
            .m3_records
            .iter()
            .all(|record| record.regime == Regime::Fiat && record.public_fiat == Gold::ZERO));
        assert_eq!(sum_fiat_payments(&society.payment_audit), Gold::ZERO);
    }
}

#[test]
fn displacement_and_legal_tender_share_fiat_issue_and_spend_fiat_before_specie() {
    let displacement = builtin_market_scenario(ScenarioName::EmergedGoldFiatDisplacement);
    let legal = builtin_market_scenario(ScenarioName::EmergedGoldFiatLegalTender);
    let bridge_len = emerged_gold_bridge_agents().len();

    assert_eq!(
        seed_rows(&displacement.agents[..bridge_len]),
        seed_rows(&legal.agents[..bridge_len])
    );
    assert_eq!(fiat_print(&displacement.events), fiat_print(&legal.events));

    for name in [
        ScenarioName::EmergedGoldFiatDisplacement,
        ScenarioName::EmergedGoldFiatLegalTender,
    ] {
        let mut society = Society::from_scenario(builtin_market_scenario(name));
        society.step();

        assert!(society.payment_audit.iter().any(|row| {
            FIRST_RECEIVERS.contains(&row.from)
                && row.public_fiat > Gold::ZERO
                && row.public_specie == Gold::ZERO
        }));
        for receiver in FIRST_RECEIVERS {
            assert_eq!(public_specie(&society, receiver), Gold(4), "{name:?}");
        }
    }
}

fn m11_scenarios() -> [ScenarioName; 2] {
    [
        ScenarioName::EmergedGoldFiatRefusalControl,
        ScenarioName::EmergedGoldFiatLegalTender,
    ]
}

fn run(name: ScenarioName) -> Society {
    let scenario = builtin_market_scenario(name);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    society
}

fn seed_rows(agents: &[Agent]) -> Vec<SeedRow> {
    agents
        .iter()
        .map(|agent| SeedRow {
            agent: agent.id,
            gold: agent.gold,
            stock: agent
                .stock
                .positive_goods()
                .map(|good| (good, agent.stock.get(good)))
                .collect(),
            scale: agent
                .scale
                .iter()
                .map(|want| (want.kind, want.horizon, want.qty))
                .collect(),
        })
        .collect()
}

fn bridge_gold(agents: &[Agent]) -> Gold {
    agents
        .iter()
        .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold))
}

fn fiat_print(events: &[econ::scenario::Event]) -> (Gold, CantillonRoute) {
    events
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::FiatPrint { amount, route, .. } => Some((*amount, route.clone())),
            _ => None,
        })
        .expect("fiat print event")
}

fn issuer_policy_event(events: &[econ::scenario::Event]) -> EventKind {
    events
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::SetIssuerPolicy { .. } => Some(event.kind.clone()),
            _ => None,
        })
        .expect("issuer policy event")
}

fn public_specie(society: &Society, agent: AgentId) -> Gold {
    society
        .money_system
        .as_ref()
        .and_then(|money| money.balance_snapshot(agent))
        .map(|balance| balance.public_specie)
        .unwrap_or(Gold::ZERO)
}

fn sum_fiat_payments(records: &[PaymentAuditRecord]) -> Gold {
    records.iter().fold(Gold::ZERO, |total, record| {
        total.saturating_add(record.public_fiat)
    })
}
