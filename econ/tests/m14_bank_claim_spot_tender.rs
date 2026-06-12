use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::expect::PriceBelief;
use econ::good::{Gold, GoodId, Horizon, Stock, FOOD, GOLD};
use econ::ledger::BankId;
use econ::money::{DesignatedMoney, MarketMoneyConfig, PublicSpotTender};
use econ::project::Tick;
use econ::record::{
    BankAuditRecord, M3Record, PaymentAuditRecord, PaymentKind, RedemptionAuditRecord,
    RedemptionOutcome,
};
use econ::scenario::{
    builtin_market_scenario, emerged_gold_bridge_agents, Event, EventKind, MarketScenario,
    ScenarioName,
};
use econ::shadow::credit_disabled_scenario;
use econ::society::Society;

const BRIDGE_GOLD: Gold = Gold(16);
const CLAIM_SPOT_TICK: u64 = 4;
const CLAIM_BANK: BankId = BankId(1);
const CLAIM_BUYER: AgentId = AgentId(121);
const FIXTURE_SELLER: AgentId = AgentId(400);

#[derive(Clone, Debug, PartialEq, Eq)]
struct SeedRow {
    agent: AgentId,
    gold: Gold,
    stock: Vec<(GoodId, u32)>,
    scale: Vec<(WantKind, Horizon, u32)>,
    expect: Vec<PriceBelief>,
}

#[test]
fn m14_scenarios_preserve_m6_bridge_prefix() {
    let bridge = emerged_gold_bridge_agents();
    let expected = seed_rows(&bridge);

    assert_eq!(bridge_gold(&bridge), BRIDGE_GOLD);
    assert!(bridge.iter().all(|agent| agent.stock.get(GOLD) == 0));

    for name in m14_scenarios() {
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
        assert!(scenario
            .agents
            .iter()
            .skip(bridge.len())
            .all(|agent| agent.id.0 > 10));
        assert!(scenario
            .agents
            .iter()
            .any(|agent| agent.id == FIXTURE_SELLER));
        assert_unique_agent_ids(&scenario.agents);
    }
}

#[test]
fn m14_scenarios_match_suspended_redemption_prefix_until_tick_4() {
    let base = run_with_audits(ScenarioName::EmergedGoldSuspendedRedemption);

    for name in m14_scenarios() {
        let society = run_with_audits(name);

        assert_eq!(
            m3_prefix(&base.m3_records, CLAIM_SPOT_TICK),
            m3_prefix(&society.m3_records, CLAIM_SPOT_TICK),
            "{name:?} M3 prefix"
        );
        assert_eq!(
            bank_prefix(&base.bank_audit, CLAIM_SPOT_TICK),
            bank_prefix(&society.bank_audit, CLAIM_SPOT_TICK),
            "{name:?} bank prefix"
        );
        assert!(society.m3_records.iter().all(|record| {
            record.public_fiat == Gold::ZERO
                && record.fiat_credit_issued == Gold::ZERO
                && record.fiat_fiscal_issued == Gold::ZERO
        }));
    }
}

#[test]
fn redemption_is_suspended_before_spot_proof() {
    for name in m14_scenarios() {
        let society = run_with_audits(name);
        let final_bank = bank_at(&society.bank_audit, CLAIM_SPOT_TICK);

        assert_eq!(sum_requested(&society.redemption_audit), Gold(4));
        assert_eq!(sum_honored(&society.redemption_audit), Gold::ZERO);
        assert_eq!(sum_failed(&society.redemption_audit), Gold(4));
        assert!(society
            .redemption_audit
            .iter()
            .all(|row| row.outcome == RedemptionOutcome::Suspended));
        assert_eq!(final_bank.reserves, Gold(2));
        assert!(!final_bank.convertible);
    }
}

#[test]
fn spot_refusal_has_no_claim_payment_from_claim_holder() {
    let society = run(ScenarioName::EmergedGoldBankClaimSpotRefusalControl);

    assert!(proof_payment(&society.payment_audit).is_none());
    assert_eq!(spot_demand_claim_volume(&society.payment_audit), Gold::ZERO);
    assert!(society
        .payment_audit
        .iter()
        .all(|row| row.tender == PublicSpotTender::SpecieOnly));
}

#[test]
fn spot_legal_tender_spends_unredeemable_claim() {
    let society = run(ScenarioName::EmergedGoldBankClaimSpotLegalTender);
    let row = proof_payment(&society.payment_audit).expect("seeded M14 spot payment");

    assert_eq!(row.tender, PublicSpotTender::BankClaimsAndSpecie);
    assert_eq!(row.demand_claims, Gold(1));
    assert_eq!(row.public_fiat, Gold::ZERO);
    assert_eq!(row.public_specie, Gold::ZERO);
    assert!(society.trades.iter().any(|trade| {
        trade.tick == CLAIM_SPOT_TICK
            && trade.good == FOOD
            && trade.buyer == CLAIM_BUYER
            && trade.seller == FIXTURE_SELLER
            && trade.price == Gold(1)
            && trade.qty == 1
    }));
}

#[test]
fn claim_spot_tender_transfers_claim_without_redemption() {
    let base = run_with_audits(ScenarioName::EmergedGoldSuspendedRedemption);
    let legal = run_with_audits(ScenarioName::EmergedGoldBankClaimSpotLegalTender);
    let base_tick = record_at(&base.m3_records, CLAIM_SPOT_TICK);
    let legal_tick = record_at(&legal.m3_records, CLAIM_SPOT_TICK);
    let legal_bank = bank_at(&legal.bank_audit, CLAIM_SPOT_TICK);
    let base_money = base.money_system.as_ref().expect("base M3 money system");
    let legal_money = legal.money_system.as_ref().expect("legal M3 money system");
    let buyer = agent(&legal.agents, CLAIM_BUYER);

    assert_eq!(legal_tick.demand_claims, Gold(4));
    assert_eq!(legal_tick.bank_reserves, Gold(2));
    assert_eq!(legal_bank.reserves, Gold(2));
    assert_eq!(legal_tick.public_fiat, Gold::ZERO);
    assert_eq!(
        legal_tick.tms,
        legal_tick
            .public_specie
            .saturating_add(legal_tick.demand_claims)
    );
    assert_eq!(legal_tick.demand_claims, base_tick.demand_claims);
    assert_eq!(legal_tick.bank_reserves, base_tick.bank_reserves);
    assert_eq!(legal_tick.public_specie, base_tick.public_specie);
    assert_eq!(legal_tick.tms, base_tick.tms);
    assert_eq!(base_money.demand_claim_on(CLAIM_BUYER, CLAIM_BANK), Gold(1));
    assert_eq!(
        legal_money.demand_claim_on(CLAIM_BUYER, CLAIM_BANK),
        Gold::ZERO
    );
    assert_eq!(
        base_money.demand_claim_on(FIXTURE_SELLER, CLAIM_BANK),
        Gold::ZERO
    );
    assert_eq!(
        legal_money.demand_claim_on(FIXTURE_SELLER, CLAIM_BANK),
        Gold(1)
    );
    assert_eq!(buyer.stock.get(FOOD), 1);
}

#[test]
fn seed_stock_event_is_goods_only() {
    let mut scenario = one_agent_m3_scenario();
    scenario.events = vec![
        Event {
            tick: Tick(0),
            kind: EventKind::SeedStock {
                agent: AgentId(1),
                good: FOOD,
                qty: 2,
            },
        },
        Event {
            tick: Tick(0),
            kind: EventKind::SeedStock {
                agent: AgentId(999),
                good: FOOD,
                qty: 1,
            },
        },
        Event {
            tick: Tick(0),
            kind: EventKind::SeedStock {
                agent: AgentId(1),
                good: GOLD,
                qty: 5,
            },
        },
    ];
    let mut society = Society::from_scenario(scenario);
    let before_money = society.money_system.clone();
    let before_debts = society.debts.clone();
    let before_loans = society.loan_trades.clone();
    let before_payments = society.payment_audit.clone();
    let before_tms = money_stock(&society).tms();

    society.step();

    assert_eq!(agent(&society.agents, AgentId(1)).stock.get(FOOD), 2);
    assert_eq!(agent(&society.agents, AgentId(1)).stock.get(GOLD), 0);
    assert_eq!(society.money_system, before_money);
    assert_eq!(society.debts, before_debts);
    assert_eq!(society.loan_trades, before_loans);
    assert_eq!(society.payment_audit, before_payments);
    assert_eq!(money_stock(&society).tms(), before_tms);
}

#[test]
fn m14_shadow_preserves_policy_but_has_no_claim_to_spend() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldBankClaimSpotLegalTender);
    let shadow = credit_disabled_scenario(&scenario);

    assert!(shadow.events.iter().any(|event| matches!(
        event.kind,
        EventKind::SetPublicSpotTender(PublicSpotTender::BankClaimsAndSpecie)
    )));
    assert_eq!(seed_stock(&shadow.events), seed_stock(&scenario.events));

    let periods = shadow.periods;
    let mut society = Society::from_scenario(shadow);
    society.run(periods);

    assert!(society
        .m3_records
        .iter()
        .all(|record| record.demand_claims == Gold::ZERO));
    assert_eq!(spot_demand_claim_volume(&society.payment_audit), Gold::ZERO);
}

fn m14_scenarios() -> [ScenarioName; 2] {
    [
        ScenarioName::EmergedGoldBankClaimSpotRefusalControl,
        ScenarioName::EmergedGoldBankClaimSpotLegalTender,
    ]
}

fn run(name: ScenarioName) -> Society {
    let scenario = builtin_market_scenario(name);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    society
}

fn run_with_audits(name: ScenarioName) -> Society {
    let scenario = builtin_market_scenario(name);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.enable_bank_audit();
    for tick in 0..periods {
        society.step();
        assert!(
            society.money_ledgers_reconcile(),
            "{name:?} ledgers failed to reconcile at tick {tick}"
        );
    }
    society
}

fn proof_payment(records: &[PaymentAuditRecord]) -> Option<&PaymentAuditRecord> {
    records.iter().find(|row| {
        row.tick == CLAIM_SPOT_TICK
            && row.kind == PaymentKind::Spot
            && row.from == CLAIM_BUYER
            && row.to == FIXTURE_SELLER
            && row.amount == Gold(1)
    })
}

fn spot_demand_claim_volume(records: &[PaymentAuditRecord]) -> Gold {
    records
        .iter()
        .filter(|row| row.kind == PaymentKind::Spot)
        .fold(Gold::ZERO, |sum, row| sum.saturating_add(row.demand_claims))
}

fn seed_stock(events: &[Event]) -> (AgentId, GoodId, u32) {
    events
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::SeedStock { agent, good, qty } => Some((*agent, *good, *qty)),
            _ => None,
        })
        .expect("seed stock event")
}

fn m3_prefix(records: &[M3Record], before_tick: u64) -> Vec<M3Record> {
    records
        .iter()
        .filter(|record| record.m2.tick < before_tick)
        .cloned()
        .collect()
}

fn bank_prefix(records: &[BankAuditRecord], before_tick: u64) -> Vec<BankAuditRecord> {
    records
        .iter()
        .filter(|record| record.tick < before_tick)
        .cloned()
        .collect()
}

fn record_at(records: &[M3Record], tick: u64) -> &M3Record {
    records
        .iter()
        .find(|record| record.m2.tick == tick)
        .expect("M3 record for tick")
}

fn bank_at(records: &[BankAuditRecord], tick: u64) -> &BankAuditRecord {
    records
        .iter()
        .find(|record| record.tick == tick && record.bank == CLAIM_BANK)
        .expect("bank record for tick")
}

fn sum_requested(records: &[RedemptionAuditRecord]) -> Gold {
    records.iter().fold(Gold::ZERO, |sum, record| {
        sum.saturating_add(record.requested)
    })
}

fn sum_honored(records: &[RedemptionAuditRecord]) -> Gold {
    records
        .iter()
        .fold(Gold::ZERO, |sum, record| sum.saturating_add(record.honored))
}

fn sum_failed(records: &[RedemptionAuditRecord]) -> Gold {
    records
        .iter()
        .fold(Gold::ZERO, |sum, record| sum.saturating_add(record.failed))
}

fn seed_rows(agents: &[Agent]) -> Vec<SeedRow> {
    let mut rows = agents
        .iter()
        .map(|agent| SeedRow {
            agent: agent.id,
            gold: agent.gold,
            stock: positive_stock(&agent.stock),
            scale: scale_signature(&agent.scale),
            expect: agent.expect.clone(),
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.agent);
    rows
}

fn scale_signature(scale: &[Want]) -> Vec<(WantKind, Horizon, u32)> {
    scale
        .iter()
        .map(|want| (want.kind, want.horizon, want.qty))
        .collect()
}

fn positive_stock(stock: &Stock) -> Vec<(GoodId, u32)> {
    stock
        .positive_goods()
        .map(|good| (good, stock.get(good)))
        .collect()
}

fn bridge_gold(agents: &[Agent]) -> Gold {
    agents
        .iter()
        .filter(|agent| agent.id.0 <= 10)
        .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold))
}

fn assert_unique_agent_ids(agents: &[Agent]) {
    let mut ids = agents.iter().map(|agent| agent.id).collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), agents.len());
}

fn agent(agents: &[Agent], id: AgentId) -> &Agent {
    agents
        .iter()
        .find(|agent| agent.id == id)
        .expect("agent exists")
}

fn money_stock(society: &Society) -> econ::ledger::MoneyStock {
    society
        .money_system
        .as_ref()
        .expect("M3 money system")
        .snapshot()
}

fn one_agent_m3_scenario() -> MarketScenario {
    MarketScenario {
        name: "seed-stock-test",
        scenario: ScenarioName::CommodityCreditNeutral,
        seed: 1,
        periods: 1,
        agents: vec![Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Next,
                qty: 2,
                satisfied: false,
            }],
            stock: Stock::new(3),
            gold: Gold(3),
            labor_capacity: 0,
            hunger_deficit: 0,
            roles: vec![Role::Household],
            expect: Vec::new(),
        }],
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}
