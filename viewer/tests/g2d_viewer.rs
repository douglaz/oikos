//! G2d acceptance suite — the `oikos` read-only debug viewer + the two
//! inspectors.
//!
//! These pin the milestone's contracts: the viewer is **deterministic** (test 1,
//! the tripwire), the dashboard has the documented shape and never falsely cries
//! conservation (test 2), the price inspector prints **exactly** the trade tape
//! for the good/tick and the matching realized price (test 3), the colonist
//! inspector matches the colonist's `sim` state including a tombstoned colonist's
//! emptied scale (test 4), the distance→price result is now **visible** in the
//! viewer (test 5, sign only), errors are **loud** (test 6), and the viewer is
//! **read-only** — the engine still replays deterministically and conserves from
//! the viewer's workspace (test 7; the full byte-identical-golden / clippy / fmt
//! proof is the workspace gate). They assert shape, exactness, and sign — never a
//! pinned magnitude (the lab discipline).

use sim::{Settlement, SettlementConfig, FOOD};

/// Build the same settlement the viewer builds for `scenario`, advanced `ticks`
/// econ ticks — so a test can compare the rendered text against the engine
/// state directly. Uses the viewer's own scenario registry, so the config is
/// byte-identical to what the renderer ran (determinism does the rest).
fn build(scenario: &str, seed: u64, ticks: u64) -> Settlement {
    let config: SettlementConfig = viewer::config_for(scenario).expect("known scenario");
    let mut settlement = Settlement::generate(seed, &config);
    settlement.run(ticks);
    settlement
}

/// Turn `&[&str]` into the `Vec<String>` the dispatcher takes.
fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

/// The data rows of the (single) table in `output`: the lines after the first
/// dashed separator, each split into trimmed cells. Used to read a rendered
/// table back for exact comparison.
fn table_rows(output: &str) -> Vec<Vec<String>> {
    let lines: Vec<&str> = output.lines().collect();
    let Some(sep) = lines.iter().position(is_separator) else {
        return Vec::new();
    };
    lines[sep + 1..]
        .iter()
        .take_while(|line| !line.is_empty())
        .map(|line| line.split('|').map(|c| c.trim().to_string()).collect())
        .collect()
}

fn table_headers(output: &str) -> Vec<String> {
    output
        .lines()
        .find(|line| line.starts_with("tick"))
        .expect("dashboard prints a table header")
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

fn is_separator(line: &&str) -> bool {
    !line.is_empty() && line.chars().all(|c| matches!(c, '-' | '+' | ' '))
}

/// Parse the integer realized price out of a `realized price (...): N` line, or
/// `None` when the inspector printed the no-trade em dash.
fn parse_realized_price(output: &str) -> Option<u64> {
    let line = output
        .lines()
        .find(|l| l.starts_with("realized price"))
        .expect("the price inspector prints a realized-price line");
    let value = line.split(": ").nth(1).expect("a value after the colon");
    value.split_whitespace().next().and_then(|t| t.parse().ok())
}

// ---- 1. determinism (the tripwire) ---------------------------------------

#[test]
fn run_output_is_deterministic() {
    let args = argv(&["run", "viable", "--ticks", "30", "--seed", "7"]);
    let first = viewer::run(&args).expect("viable runs");
    let second = viewer::run(&args).expect("viable runs again");
    assert_eq!(
        first, second,
        "the dashboard is not byte-identical across runs"
    );

    // A different seed produces a different dashboard (the run really is seeded).
    let other = viewer::run(&argv(&["run", "viable", "--ticks", "30", "--seed", "8"]))
        .expect("a different seed runs");
    assert_ne!(first, other, "the seed did not affect the output");

    // The inspectors are deterministic too.
    let p1 = viewer::run(&argv(&[
        "inspect",
        "price",
        "price-probe",
        "--good",
        "food",
    ]))
    .expect("price inspector runs");
    let p2 = viewer::run(&argv(&[
        "inspect",
        "price",
        "price-probe",
        "--good",
        "food",
    ]))
    .expect("price inspector runs again");
    assert_eq!(p1, p2);
}

// ---- 2. the dashboard's shape --------------------------------------------

#[test]
fn run_dashboard_has_expected_shape() {
    let ticks = 24u64;
    let output = viewer::run_dashboard("viable", ticks, 1).expect("viable dashboard");

    // One table row per econ tick, numbered 0..ticks.
    let rows = table_rows(&output);
    assert_eq!(
        rows.len() as u64,
        ticks,
        "the dashboard must have one row per econ tick"
    );
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row[0], i.to_string(), "rows must be tick-ordered");
    }

    // Population is reported in the header.
    assert!(
        output.contains("population 12 (8 gatherers, 4 consumers)"),
        "the header must report the population by vocation"
    );

    // It reports conservation OK every tick and NEVER prints VIOLATED for a
    // conserving scenario.
    assert!(
        !output.contains("VIOLATED"),
        "a conserving scenario must never print VIOLATED"
    );
    for row in &rows {
        assert_eq!(row[3], "OK", "every tick must report conservation OK");
    }

    // At least one realized price is shown (the food.px column, index 6, is a
    // number on at least one tick — the market actually cleared).
    let saw_price = rows.iter().any(|row| row[6].parse::<u64>().is_ok());
    assert!(
        saw_price,
        "the dashboard must show at least one realized price"
    );
}

#[test]
fn chain_dashboard_shows_production_receipts() {
    let output = viewer::run_dashboard("chain", 12, 1).expect("chain dashboard");
    let headers = table_headers(&output);
    let flour_made = headers
        .iter()
        .position(|header| header == "flour.made")
        .expect("chain dashboard includes produced flour");
    let grain_input = headers
        .iter()
        .position(|header| header == "grain.input")
        .expect("chain dashboard includes milled grain input");
    let bread_made = headers
        .iter()
        .position(|header| header == "bread.made")
        .expect("chain dashboard includes produced bread");
    let rows = table_rows(&output);

    assert!(
        rows.iter()
            .any(|row| row[flour_made].parse::<u64>().unwrap_or(0) > 0),
        "milling is hidden in the chain dashboard"
    );
    assert!(
        rows.iter()
            .any(|row| row[grain_input].parse::<u64>().unwrap_or(0) > 0),
        "recipe input consumption is hidden in the chain dashboard"
    );
    assert!(
        rows.iter()
            .any(|row| row[bread_made].parse::<u64>().unwrap_or(0) > 0),
        "baking is hidden in the chain dashboard"
    );
}

// ---- 3. price → the trade tape -------------------------------------------

#[test]
fn price_inspector_matches_the_trade_tape() {
    let (seed, ticks, at_tick) = (1u64, 15u64, 14u64);

    // The engine's own truth: the FOOD trades on the tape at the inspected tick.
    let settlement = build("price-probe", seed, ticks);
    let expected: Vec<_> = settlement
        .society()
        .trades
        .iter()
        .filter(|t| t.good == FOOD && t.tick == at_tick)
        .collect();
    assert!(
        !expected.is_empty(),
        "the chosen tick must actually have FOOD trades, or the test is vacuous"
    );

    let output = viewer::run_price("price-probe", "food", Some(at_tick), Some(ticks), seed)
        .expect("price inspector runs");

    // The printed trade rows are EXACTLY the tape's, in tape order.
    let rows = table_rows(&output);
    assert_eq!(
        rows.len(),
        expected.len(),
        "the inspector must print exactly the tape's trades for the good/tick"
    );
    for (row, trade) in rows.iter().zip(&expected) {
        assert_eq!(row[0], trade.tick.to_string());
        assert_eq!(row[1], settlement.society().good_name(trade.good));
        assert_eq!(row[2], trade.buyer.to_string());
        assert_eq!(row[3], trade.seller.to_string());
        assert_eq!(row[4], trade.price.0.to_string());
        assert_eq!(row[5], trade.qty.to_string());
    }
    assert!(output.contains(&format!(
        "trades in food at tick {at_tick}: {}",
        expected.len()
    )));

    // The printed realized price matches `realized_price(good)`.
    let realized = settlement
        .realized_price(FOOD)
        .expect("FOOD has cleared by now")
        .0;
    assert_eq!(parse_realized_price(&output), Some(realized));
}

#[test]
fn price_inspector_reports_a_tick_with_no_trades() {
    // Tick 7 of price-probe (seed 1) clears no FOOD trade (the delivery cadence).
    let output = viewer::run_price("price-probe", "food", Some(7), Some(15), 1)
        .expect("price inspector runs");
    assert!(output.contains("trades in food at tick 7: 0"));
    assert!(output.contains("(no trades in this good cleared at this tick)"));

    // The realized price is still shown (it is the engine's most-recent realized
    // price), but it is explicitly labelled as carried over from the earlier tick
    // that produced it, so it is never read as "the price behind these (zero)
    // trades". The inspector runs to the inspected tick (at-tick 7 → 8 ticks), so
    // its realized price is the engine's after that same run.
    let inspected = build("price-probe", 1, 8);
    let carried = inspected
        .realized_price(FOOD)
        .expect("an earlier FOOD trade set a carried price")
        .0;
    let source_tick = inspected
        .society()
        .trades
        .iter()
        .filter(|t| t.good == FOOD && t.tick <= 7)
        .map(|t| t.tick)
        .max()
        .expect("the carried price has a source tick");
    assert!(
        output.contains(&format!(
            "realized price (food): {carried} (carried from tick {source_tick}; no food trade cleared at tick 7)"
        )),
        "the carried-over price must be labelled with its source tick, got: {output}"
    );
    assert_eq!(
        parse_realized_price(&output),
        Some(carried),
        "the carried realized price must still match realized_price(good)"
    );

    // It must match the tape: the tape really has no FOOD trade at tick 7.
    let settlement = build("price-probe", 1, 15);
    let count = settlement
        .society()
        .trades
        .iter()
        .filter(|t| t.good == FOOD && t.tick == 7)
        .count();
    assert_eq!(count, 0);
}

#[test]
fn price_inspector_uses_the_inspected_tick_when_ticks_is_longer() {
    let (seed, supplied_ticks) = (1u64, 20u64);
    let final_price = build("viable", seed, supplied_ticks).realized_price(FOOD);
    let at_tick = (0..supplied_ticks - 1)
        .find(|&tick| {
            let settlement = build("viable", seed, tick + 1);
            settlement.realized_price(FOOD).is_some()
                && settlement.realized_price(FOOD) != final_price
                && settlement
                    .society()
                    .trades
                    .iter()
                    .any(|trade| trade.good == FOOD && trade.tick == tick)
        })
        .expect("the viable scenario should have an inspected tick with a distinct FOOD price");
    let inspected = build("viable", seed, at_tick + 1);
    let expected_price = inspected.realized_price(FOOD).map(|price| price.0);
    assert_ne!(
        expected_price,
        final_price.map(|price| price.0),
        "the regression probe must distinguish the inspected tick from the later run"
    );

    let output = viewer::run_price("viable", "food", Some(at_tick), Some(supplied_ticks), seed)
        .expect("price inspector runs");
    assert!(output.contains(&format!("at-tick {at_tick} · {} econ ticks", at_tick + 1)));
    assert_eq!(
        parse_realized_price(&output),
        expected_price,
        "the realized price must be as of --at-tick, not the later --ticks bound"
    );
}

// ---- 4. colonist → scale / why -------------------------------------------

#[test]
fn colonist_inspector_matches_state() {
    let (seed, supplied_ticks, at_tick, id) = (1u64, 20u64, 2u64, 1usize);
    let inspected_ticks = at_tick + 1;
    let settlement = build("viable", seed, inspected_ticks);
    let agent_id = settlement.colonist_id(id).expect("colonist exists");
    let agent = settlement
        .society()
        .agents
        .get(agent_id)
        .expect("econ agent exists");
    let need = settlement.need_of(id).expect("need state exists");
    let later_need = build("viable", seed, supplied_ticks)
        .need_of(id)
        .expect("need state exists later");
    assert_ne!(
        (need.hunger, need.warmth, need.rest),
        (later_need.hunger, later_need.warmth, later_need.rest),
        "the regression probe must distinguish the inspected tick from the later run"
    );

    let output = viewer::run_colonist("viable", id, Some(at_tick), Some(supplied_ticks), seed)
        .expect("colonist inspector runs");
    assert!(output.contains(&format!("at-tick {at_tick} · {inspected_ticks} econ ticks")));

    // Vocation, liveness.
    assert!(settlement.is_alive(id), "colonist 1 should be alive here");
    assert!(output.contains(&format!(
        "colonist {id} (agent {agent_id}): consumer, ALIVE"
    )));

    // Needs.
    assert!(output.contains(&format!(
        "needs: hunger {}, warmth {}, rest {}",
        need.hunger, need.warmth, need.rest
    )));

    // Carry (delivery escrow), per tracked good.
    for &good in settlement.tracked_goods() {
        assert!(output.contains(&format!(
            "{} {}",
            settlement.society().good_name(good),
            settlement.carry_of(id, good)
        )));
    }

    // Gold.
    assert!(output.contains(&format!("gold: {}", agent.gold.0)));

    // The full value scale: as many rows as the agent's scale, first want
    // matching, ranks starting at 1.
    assert!(output.contains(&format!(
        "value scale (ranked, most urgent first): {} wants",
        agent.scale.len()
    )));
    let rows = table_rows(&output);
    assert_eq!(
        rows.len(),
        agent.scale.len(),
        "every want must be printed, one per row"
    );
    assert_eq!(rows[0][0], "1", "the scale is ranked from 1");
}

#[test]
fn colonist_inspector_shows_a_dead_colonist_with_an_emptied_scale() {
    let (seed, ticks, at_tick, id) = (1u64, 20u64, 19u64, 0usize);

    // The starved hauler dies mid-haul and its scale is emptied (tombstoned).
    let settlement = build("starved-hauler", seed, ticks);
    assert!(
        !settlement.is_alive(id),
        "the hauler must be dead by tick 19"
    );
    let agent_id = settlement.colonist_id(id).unwrap();
    assert!(
        settlement
            .society()
            .agents
            .get(agent_id)
            .unwrap()
            .scale
            .is_empty(),
        "a tombstoned colonist's scale is emptied"
    );

    let output = viewer::run_colonist("starved-hauler", id, Some(at_tick), Some(ticks), seed)
        .expect("colonist inspector runs");
    assert!(output.contains("gatherer, DEAD"));
    assert!(output.contains("(empty — colonist is tombstoned)"));
    // Its frozen carry (delivery escrow) is still shown — conserved, not lost.
    assert!(output.contains(&format!("food {}", settlement.carry_of(id, FOOD))));
}

// ---- 5. the distance→price result, now visible ---------------------------

#[test]
fn distance_contrast_is_visible() {
    // The G2b probe seed and a post-warmup tick: at every tick in the proven
    // window the far run's realized FOOD price exceeds the near run's, so a
    // single deterministic sample suffices here (sign only — no magnitude).
    let (seed, ticks, at_tick) = (20_260_613u64, 84u64, 83u64);

    let near = viewer::run_price("near", "food", Some(at_tick), Some(ticks), seed).unwrap();
    let far = viewer::run_price("far", "food", Some(at_tick), Some(ticks), seed).unwrap();

    let near_price = parse_realized_price(&near).expect("near cleared FOOD");
    let far_price = parse_realized_price(&far).expect("far cleared FOOD");
    assert!(
        far_price > near_price,
        "distance did not raise the realized food price in the viewer: near={near_price} far={far_price}"
    );

    // The `far-node` alias resolves to the same scenario: identical realized
    // price (the header echoes the name the user typed, so it differs only there).
    let far_alias =
        viewer::run_price("far-node", "food", Some(at_tick), Some(ticks), seed).unwrap();
    assert_eq!(
        parse_realized_price(&far_alias),
        Some(far_price),
        "far-node must alias far"
    );
}

// ---- 5b. the G2c region dashboard (multi-settlement + caravans) -----------

/// The `region` run dashboard is deterministic, conserves every tick, and shows
/// the convergence gap shrinking — while the `region-control` twin keeps it. This
/// surfaces the G2c result in the read-only viewer (sign only).
#[test]
fn region_dashboard_shows_convergence_versus_the_control() {
    let ticks = 30u64;
    let caravan = viewer::run_dashboard("region", ticks, 1).expect("region dashboard");
    let control = viewer::run_dashboard("region-control", ticks, 1).expect("control dashboard");

    // Deterministic: byte-identical across runs.
    assert_eq!(
        caravan,
        viewer::run_dashboard("region", ticks, 1).unwrap(),
        "the region dashboard is not byte-identical across runs"
    );

    // The header announces the caravan vs the control twin.
    assert!(caravan.contains("caravan active"));
    assert!(control.contains("no-caravan control"));

    // Conservation holds every tick in both modes (never a VIOLATED cell).
    assert!(
        !caravan.contains("VIOLATED"),
        "the caravan run broke conservation"
    );
    assert!(
        !control.contains("VIOLATED"),
        "the control run broke conservation"
    );

    // One table row per econ tick, gap column present.
    let caravan_rows = table_rows(&caravan);
    assert_eq!(caravan_rows.len() as u64, ticks, "one row per econ tick");

    // The gap (column index 3) at the LAST tick is smaller with the caravan than
    // in the control — the caravan closes what the control leaves open (sign only).
    let last_gap = |rows: &[Vec<String>]| -> u64 {
        rows.last()
            .and_then(|r| r.get(3))
            .and_then(|c| c.parse::<u64>().ok())
            .expect("the last row has a numeric gap")
    };
    let control_rows = table_rows(&control);
    assert!(
        last_gap(&caravan_rows) < last_gap(&control_rows),
        "the caravan did not narrow the gap below the control: caravan={} control={}",
        last_gap(&caravan_rows),
        last_gap(&control_rows)
    );
}

/// The price / colonist inspectors reject the region scenarios: those advance a
/// two-settlement Region, not a single Settlement, so they are `run`-only.
#[test]
fn inspectors_reject_region_scenarios() {
    let err = viewer::run(&argv(&["inspect", "price", "region", "--good", "food"]))
        .expect_err("region is not an inspectable settlement scenario");
    assert!(err.contains("unknown scenario"));
    assert!(err.contains("USAGE:"));
}

// ---- 6. loud errors ------------------------------------------------------

#[test]
fn unknown_scenario_and_flags_error() {
    // Each of these must be an Err (not a panic, not a silent default), and the
    // error must carry the usage block.
    let cases: &[(&[&str], &str)] = &[
        (&["run", "nonsense"], "unknown scenario"),
        (&["run", "viable", "--bogus", "5"], "unknown flag"),
        (&["run"], "missing required <scenario>"),
        (&["inspect", "price", "viable"], "missing required --good"),
        (&["inspect", "colonist", "viable"], "missing required --id"),
        (
            &["inspect", "price", "viable", "--good", "bread"],
            "unknown good",
        ),
        (&["inspect", "wat", "viable"], "unknown inspector"),
        (&["frobnicate"], "unknown command"),
        (&["scenarios", "--bogus"], "unknown flag"),
        (&["help", "extra"], "unexpected argument"),
        (
            &["inspect", "price", "viable", "--good", "--at-tick", "10"],
            "--good requires a value",
        ),
        (
            &["inspect", "colonist", "viable", "--id", "--seed", "1"],
            "--id requires a value",
        ),
        (
            &["run", "viable", "--ticks", "lots"],
            "invalid value for --ticks",
        ),
        (
            &["inspect", "colonist", "viable", "--id", "999"],
            "out of range",
        ),
    ];
    for (args, needle) in cases {
        let result = viewer::run(&argv(args));
        let err = result.expect_err(&format!("{args:?} should be an error"));
        assert!(
            err.contains(needle),
            "error for {args:?} should mention {needle:?}, got: {err}"
        );
        assert!(
            err.contains("USAGE:"),
            "error for {args:?} should include the usage block"
        );
    }
}

#[test]
fn help_and_scenarios_are_not_errors() {
    assert!(viewer::run(&argv(&["help"])).unwrap().contains("USAGE:"));
    assert!(viewer::run(&argv(&[])).unwrap().contains("USAGE:")); // no args → help
    let scenarios = viewer::run(&argv(&["scenarios"])).unwrap();
    for name in ["viable", "price-probe", "near", "far", "starved-hauler"] {
        assert!(scenarios.contains(name), "scenarios must list {name}");
    }
    assert!(scenarios.contains("near-node = near"));
}

// ---- 7. read-only: the engine is unperturbed -----------------------------

#[test]
fn inspectors_are_read_only() {
    use econ::scenario::{builtin_market_scenario, ScenarioName};
    use econ::society::Society;

    // Exercise the viewer's read paths (the dashboard and both inspectors).
    let _ = viewer::run_dashboard("viable", 20, 1).expect("dashboard");
    let _ = viewer::run_price("price-probe", "food", Some(14), Some(15), 1).expect("price");
    let _ = viewer::run_colonist("viable", 1, Some(10), Some(11), 1).expect("colonist");

    // The econ engine still replays deterministically and conserves gold from
    // the viewer's workspace — the viewer changed no lib behavior. (The full
    // byte-identical-golden, clippy, and fmt proof is the workspace gate; this
    // is the in-crate usability/non-perturbation check, mirroring G2b.)
    for name in [
        ScenarioName::MarketBarterishGold,
        ScenarioName::MarketPriceDiscovery,
        ScenarioName::MengerSaltMoney,
    ] {
        let scenario = builtin_market_scenario(name);
        let periods = scenario.periods;

        let mut first = Society::from_scenario(scenario);
        let total_gold = first.total_gold();
        first.run(periods);

        let mut second = Society::from_scenario(builtin_market_scenario(name));
        second.run(periods);

        assert_eq!(
            first.records, second.records,
            "{name:?} did not replay deterministically"
        );
        if matches!(name, ScenarioName::MarketBarterishGold) {
            assert_eq!(
                first.total_gold(),
                total_gold,
                "{name:?} broke gold conservation"
            );
        }
    }
}

// ---- extra unit-level checks ---------------------------------------------

#[test]
fn at_tick_alone_sets_the_run_length() {
    // `--at-tick 10` with no `--ticks` runs exactly 11 econ ticks.
    let output = viewer::run_price("viable", "food", Some(10), None, 1).unwrap();
    assert!(output.contains("at-tick 10 · 11 econ ticks"));
}

#[test]
fn scenario_aliases_resolve_to_the_same_config() {
    assert_eq!(
        viewer::config_for("near").unwrap(),
        viewer::config_for("near-node").unwrap()
    );
    assert_eq!(
        viewer::config_for("far").unwrap(),
        viewer::config_for("far-node").unwrap()
    );
}
