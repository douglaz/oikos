use econ::record::Record;
use econ::scenario::{builtin_scenario, ScenarioName};
use econ::sim::World;

fn run(name: ScenarioName) -> Vec<Record> {
    let scenario = builtin_scenario(name);
    let periods = scenario.periods;
    let mut world = World::from_scenario(scenario);
    world.run(periods);
    world.records
}

fn sum(records: &[Record], field: fn(&Record) -> u32) -> u32 {
    records.iter().map(field).sum()
}

#[test]
fn crusoe_survival_has_no_projects() {
    let records = run(ScenarioName::CrusoeSurvival);

    assert_eq!(sum(&records, |r| r.completed_projects), 0);
    assert_eq!(sum(&records, |r| r.abandoned_projects), 0);
    assert_eq!(sum(&records, |r| r.capital_labor_consumed), 0);
    assert_eq!(sum(&records, |r| r.capital_goods_consumed), 0);
}

#[test]
fn crusoe_capital_completes_net() {
    let records = run(ScenarioName::CrusoeCapital);
    let final_record = records.last().unwrap();

    assert!(final_record.nets >= 1);
    assert!(records.iter().any(|record| record.completed_projects > 0));
    assert!(sum(&records, |r| r.fish_actions) > 0);
    assert_eq!(sum(&records, |r| r.capital_labor_consumed), 0);
    assert_eq!(sum(&records, |r| r.capital_goods_consumed), 0);
}

#[test]
fn crusoe_capital_raises_productivity() {
    let records = run(ScenarioName::CrusoeCapital);
    let first_net = records
        .iter()
        .position(|record| record.nets > 0)
        .expect("net completion");

    let (pre_output, pre_labor) = food_output_and_labor(&records[..first_net]);
    let (post_output, post_labor) = food_output_and_labor(&records[first_net..]);

    assert!(pre_labor > 0);
    assert!(post_labor > 0);
    assert!(post_output * pre_labor > pre_output * post_labor);
}

#[test]
fn crusoe_abandon_consumes_capital() {
    let records = run(ScenarioName::CrusoeAbandon);
    let final_record = records.last().unwrap();

    assert!(sum(&records, |r| r.abandoned_projects) > 0);
    assert_eq!(final_record.nets, 0);
    assert!(sum(&records, |r| r.capital_labor_consumed) > 0);
    assert!(sum(&records, |r| r.capital_goods_consumed) > 0);
}

#[test]
fn scenario_runs_are_deterministic() {
    let a = run(ScenarioName::CrusoeCapital);
    let b = run(ScenarioName::CrusoeCapital);

    assert_eq!(a, b);
}

#[test]
fn golden_m0_series_is_stable() {
    let records = run(ScenarioName::CrusoeCapital);

    // Golden M0 series hash; update only for intentional simulator behavior changes.
    assert_eq!(fnv1a_records(&records), 10_593_929_640_577_161_589);
}

fn food_output_and_labor(records: &[Record]) -> (u32, u32) {
    let output = records
        .iter()
        .map(|record| record.gather_actions * 2 + record.fish_actions * 5)
        .sum();
    let labor = records
        .iter()
        .map(|record| record.gather_actions + record.fish_actions)
        .sum();
    (output, labor)
}

fn fnv1a_records(records: &[Record]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for record in records {
        hash_u64(&mut hash, record.tick);
        hash_u32(&mut hash, record.food);
        hash_u32(&mut hash, record.wood);
        hash_u32(&mut hash, record.nets);
        hash_u32(&mut hash, record.labor_used);
        hash_u32(&mut hash, record.leisure_taken);
        hash_u32(&mut hash, record.food_consumed);
        hash_u32(&mut hash, record.hunger_deficit);
        hash_u32(&mut hash, record.active_projects);
        hash_u32(&mut hash, record.completed_projects);
        hash_u32(&mut hash, record.abandoned_projects);
        hash_u32(&mut hash, record.capital_labor_consumed);
        hash_u32(&mut hash, record.capital_goods_consumed);
        hash_u32(&mut hash, record.gather_actions);
        hash_u32(&mut hash, record.cut_wood_actions);
        hash_u32(&mut hash, record.fish_actions);
        hash_u32(&mut hash, record.project_actions);
    }
    hash
}

fn hash_u32(hash: &mut u64, value: u32) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn hash_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}
