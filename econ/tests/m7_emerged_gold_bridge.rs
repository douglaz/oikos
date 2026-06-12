use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::expect::PriceBelief;
use econ::good::{Gold, GoodId, Horizon, Stock, GOLD, ORE, SALT};
use econ::ledger::MoneySystem;
use econ::money::Regime;
use econ::record::V2Phase;
use econ::scenario::{
    builtin_market_scenario, emerged_gold_bridge_agents, EventKind, ScenarioKind, ScenarioName,
};
use econ::society::{run_m3_with_shadow_without_metrics, Society};

#[derive(Clone, Debug, PartialEq, Eq)]
struct SeedRow {
    agent: AgentId,
    gold: Gold,
    stock: Vec<(GoodId, u32)>,
    scale: Vec<(WantKind, Horizon, u32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Purchase {
    buyer: AgentId,
    seller: AgentId,
    good: GoodId,
    qty: u32,
}

#[test]
fn bridge_seed_matches_menger_gold_promotion_state() {
    let mut society =
        Society::from_scenario(builtin_market_scenario(ScenarioName::MengerGoldMoney));

    loop {
        society.step();
        if society
            .v2_records
            .last()
            .is_some_and(|record| record.promoted_this_tick)
        {
            break;
        }
    }

    let actual = seed_rows(&society.agents);
    let expected = seed_rows(&emerged_gold_bridge_agents());

    assert_eq!(actual, expected);
    assert_eq!(total_agent_gold(&society.agents), Gold(16));
    assert!(society
        .agents
        .iter()
        .all(|agent| agent.stock.get(GOLD) == 0));
    assert!(emerged_gold_bridge_agents()
        .iter()
        .all(|agent| agent.stock.get(GOLD) == 0));
}

#[test]
fn emerged_gold_sound_control_initializes_m3_ledgers_from_emerged_gold() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldSoundControl);
    assert_eq!(scenario.scenario.kind(), ScenarioKind::MarketM3);

    let society = Society::from_scenario(scenario);
    assert_eq!(society.current_money_good(), Some(GOLD));
    assert!(society.banks.is_empty());
    assert!(society.issuers.is_empty());

    let snapshot = society
        .money_system
        .as_ref()
        .expect("M3 money system")
        .snapshot();
    assert_eq!(snapshot.public_specie, Gold(16));
    assert_eq!(snapshot.public_fiat, Gold::ZERO);
    assert_eq!(snapshot.demand_claims, Gold::ZERO);
    assert_eq!(snapshot.fiduciary, Gold::ZERO);
    assert_eq!(snapshot.tms(), Gold(16));
}

#[test]
fn emerged_gold_sound_control_spends_specie_without_fiat() {
    let mut society = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldSoundControl,
    ));

    assert_eq!(public_specie(&society, AgentId(7)), Gold(4));
    assert_eq!(public_specie(&society, AgentId(8)), Gold(4));

    let first_tick = step_until_first_trading_tick(&mut society);

    assert_eq!(first_tick, 0);
    assert!(public_specie(&society, AgentId(7)) < Gold(4));
    assert!(public_specie(&society, AgentId(8)) < Gold(4));
    assert_eq!(public_fiat(&society, AgentId(7)), Gold::ZERO);
    assert_eq!(public_fiat(&society, AgentId(8)), Gold::ZERO);
}

#[test]
fn emerged_gold_fiat_displacement_starts_from_sound_gold_then_switches_to_fiat() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldFiatDisplacement);
    assert_eq!(scenario.scenario.kind(), ScenarioKind::MarketM3);
    assert_eq!(scenario.scenario.regime(), Regime::SoundGold);
    assert_eq!(scenario.events.len(), 3);
    assert!(scenario.events.iter().all(|event| event.tick.0 == 0));
    assert!(matches!(
        &scenario.events[0].kind,
        EventKind::SetRegime(Regime::Fiat)
    ));
    assert!(matches!(
        &scenario.events[1].kind,
        EventKind::SetIssuerPolicy { policy, .. }
            if policy.fiscal_enabled
                && !policy.credit_enabled
                && policy.max_fiscal_issue_per_tick >= Gold(8)
    ));
    assert!(matches!(
        &scenario.events[2].kind,
        EventKind::FiatPrint { amount, .. } if *amount == Gold(8)
    ));

    let mut society = Society::from_scenario(scenario);
    assert_eq!(society.regime(), Regime::SoundGold);
    assert!(society.banks.is_empty());
    assert_eq!(society.issuers.len(), 1);

    society.step();

    let first = society.m3_records.first().expect("tick 0 M3 record");
    assert_eq!(first.m2.tick, 0);
    assert_eq!(first.regime, Regime::Fiat);
    assert_eq!(first.fiat_fiscal_issued, Gold(8));
    assert_eq!(first.public_fiat, Gold(8));
    assert_eq!(first.public_specie, Gold(16));
    assert_eq!(first.tms, Gold(24));
}

#[test]
fn fiat_first_receivers_spend_fiat_before_specie() {
    let mut displacement = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldFiatDisplacement,
    ));
    let mut control = Society::from_scenario(builtin_market_scenario(
        ScenarioName::EmergedGoldSoundControl,
    ));

    assert_eq!(public_specie(&displacement, AgentId(7)), Gold(4));
    assert_eq!(public_specie(&displacement, AgentId(8)), Gold(4));
    assert_eq!(public_fiat(&displacement, AgentId(7)), Gold::ZERO);
    assert_eq!(public_fiat(&displacement, AgentId(8)), Gold::ZERO);

    let displacement_tick = step_until_first_trading_tick(&mut displacement);
    let control_tick = step_until_first_trading_tick(&mut control);

    assert_eq!(displacement_tick, 0);
    assert_eq!(control_tick, displacement_tick);
    assert_eq!(
        first_tick_receipt_amount(&displacement, AgentId(7)),
        Gold(4)
    );
    assert_eq!(
        first_tick_receipt_amount(&displacement, AgentId(8)),
        Gold(4)
    );
    assert_eq!(
        purchases_by_first_receivers(&displacement, displacement_tick),
        purchases_by_first_receivers(&control, control_tick)
    );

    assert_eq!(public_specie(&displacement, AgentId(7)), Gold(4));
    assert_eq!(public_specie(&displacement, AgentId(8)), Gold(4));
    assert!(public_fiat(&displacement, AgentId(7)) < Gold(4));
    assert!(public_fiat(&displacement, AgentId(8)) < Gold(4));

    assert!(public_specie(&control, AgentId(7)) < Gold(4));
    assert!(public_specie(&control, AgentId(8)) < Gold(4));
    assert_eq!(public_fiat(&control, AgentId(7)), Gold::ZERO);
    assert_eq!(public_fiat(&control, AgentId(8)), Gold::ZERO);
}

#[test]
fn fiat_displacement_uses_existing_money_transfer_order() {
    let agents = vec![ledger_agent(1, Gold(3)), ledger_agent(2, Gold::ZERO)];
    let mut money = MoneySystem::from_agents(&agents);
    money
        .credit_fiat(AgentId(1), Gold(2))
        .expect("fiat credit succeeds");

    let composition = money
        .transfer_spendable(AgentId(1), AgentId(2), Gold(1))
        .expect("payer has spendable funds");

    assert_eq!(composition.fiat, Gold(1));
    assert!(composition.claims.is_empty());
    assert_eq!(composition.specie, Gold::ZERO);
    assert_eq!(
        money
            .balance_snapshot(AgentId(1))
            .expect("payer balance")
            .public_specie,
        Gold(3)
    );
}

#[test]
fn fiat_displacement_shadow_removes_fiat_print() {
    let scenario = builtin_market_scenario(ScenarioName::EmergedGoldFiatDisplacement);
    let society = run_m3_with_shadow_without_metrics(scenario);

    assert!(society.m3_shadow_attached());
    assert!(society
        .m3_records
        .iter()
        .any(|record| record.fiat_fiscal_issued == Gold(8)));
    assert!(society.m3_records.iter().all(|record| {
        record.bank_credit_issued == Gold::ZERO
            && record.fiat_credit_issued == Gold::ZERO
            && record.bank_loan_trades == 0
            && record.fiat_loan_trades == 0
    }));
}

#[test]
fn m6_gold_and_salt_pins_remain_stable() {
    let gold = run_full(ScenarioName::MengerGoldMoney);
    let salt = run_full(ScenarioName::MengerSaltMoney);

    let gold_promotion = promotion_record(&gold);
    let salt_promotion = promotion_record(&salt);

    assert_eq!(gold_promotion.tick, 3);
    assert_eq!(gold_promotion.money_good, Some(GOLD));
    assert_eq!(gold_promotion.total_money_units, Gold(16));
    assert_eq!(gold_promotion.phase, V2Phase::Barter);

    assert_eq!(salt_promotion.tick, 3);
    assert_eq!(salt_promotion.money_good, Some(SALT));
    assert_eq!(salt_promotion.total_money_units, Gold(16));
    assert_eq!(salt_promotion.phase, V2Phase::Barter);
}

fn seed_rows(agents: &[Agent]) -> Vec<SeedRow> {
    let mut rows = agents
        .iter()
        .map(|agent| SeedRow {
            agent: agent.id,
            gold: agent.gold,
            stock: positive_stock(&agent.stock),
            scale: scale_signature(&agent.scale),
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

fn total_agent_gold(agents: &[Agent]) -> Gold {
    agents
        .iter()
        .fold(Gold::ZERO, |total, agent| total.saturating_add(agent.gold))
}

fn step_until_first_trading_tick(society: &mut Society) -> u64 {
    for _ in 0..8 {
        society.step();
        let record = society.m3_records.last().expect("M3 record after step");
        if record.m2.spot_trades > 0 {
            return record.m2.tick;
        }
    }
    panic!("scenario should clear deterministic early spot trades");
}

fn public_specie(society: &Society, agent: AgentId) -> Gold {
    society
        .money_system
        .as_ref()
        .and_then(|money| money.balance_snapshot(agent))
        .map(|balance| balance.public_specie)
        .unwrap_or(Gold::ZERO)
}

fn public_fiat(society: &Society, agent: AgentId) -> Gold {
    society
        .money_system
        .as_ref()
        .and_then(|money| money.balance_snapshot(agent))
        .map(|balance| balance.public_fiat)
        .unwrap_or(Gold::ZERO)
}

fn first_tick_receipt_amount(society: &Society, agent: AgentId) -> Gold {
    society
        .cantillon_receipts
        .iter()
        .filter(|receipt| receipt.tick.0 == 0 && receipt.agent == agent)
        .fold(Gold::ZERO, |total, receipt| {
            total.saturating_add(receipt.amount)
        })
}

fn purchases_by_first_receivers(society: &Society, tick: u64) -> Vec<Purchase> {
    let mut purchases = society
        .trades
        .iter()
        .filter(|trade| trade.tick == tick && matches!(trade.buyer, AgentId(7) | AgentId(8)))
        .map(|trade| Purchase {
            buyer: trade.buyer,
            seller: trade.seller,
            good: trade.good,
            qty: trade.qty,
        })
        .collect::<Vec<_>>();
    purchases.sort();
    purchases
}

fn ledger_agent(id: u32, gold: Gold) -> Agent {
    Agent {
        id: AgentId(id),
        scale: Vec::new(),
        stock: Stock::new(ORE.0),
        gold,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Trader],
        expect: vec![PriceBelief::new(Gold(1), Gold(1)); usize::from(ORE.0) + 1],
    }
}

fn run_full(name: ScenarioName) -> Society {
    let scenario = builtin_market_scenario(name);
    let periods = scenario.periods;
    let mut society = Society::from_scenario(scenario);
    society.run(periods);
    society
}

fn promotion_record(society: &Society) -> &econ::record::V2Record {
    society
        .v2_records
        .iter()
        .find(|record| record.promoted_this_tick)
        .expect("scenario promotes")
}
