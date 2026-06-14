//! `oikos` — the G2d read-only debug viewer over a `sim::Settlement`.
//!
//! This is the workspace's **first runnable artifact** and the legibility layer
//! the game-spec (pillar 3, §8) makes central: a headless, deterministic text
//! CLI that runs a settlement and renders its state, plus the two inspectors the
//! G2 roadmap mandates — **price → the trades behind it** and **colonist → its
//! value scale and why**.
//!
//! It is strictly **read-only**: every renderer draws from `sim`'s existing
//! read-only accessors (and `sim`'s re-exports of the `econ`/`life` types) over a
//! settlement advanced by a *seeded* run. The viewer itself draws no randomness,
//! so the same `(scenario, ticks, seed)` produces byte-identical output — the
//! determinism contract the acceptance suite's tripwire pins. Nothing here
//! changes any `econ`/`world`/`life`/`sim` behavior: the conformance goldens and
//! the G1/G2a/G2b suites stay green and byte-identical.
//!
//! The crate is both a library (so `viewer/tests/` can drive the renderers and
//! assert on their `String` output) and the `oikos` binary (`src/main.rs`, which
//! only parses args, calls [`run`], and prints the result).
//!
//! Errors are **loud**: an unknown scenario, an unknown flag, or a missing
//! required argument yields a message plus the usage block — never a silent
//! default or a panic.

mod cli;
mod render;
mod scenarios;

use sim::{EconTickReport, GoodId, Region, RegionConfig, Settlement, Vocation};

pub use scenarios::{config_for, scenarios_text};

/// Default seed when `--seed` is omitted.
pub const DEFAULT_SEED: u64 = 1;
/// Default econ-tick count for `run` when `--ticks` is omitted.
pub const DEFAULT_RUN_TICKS: u64 = 20;
/// Default econ-tick count for the inspectors when neither `--ticks` nor
/// `--at-tick` is given (one settlement "year").
pub const DEFAULT_INSPECT_TICKS: u64 = sim::ECON_TICKS_PER_YEAR;

/// The CLI entry point: dispatch `args` (the program arguments, *excluding*
/// `argv[0]`) and return the text to print on success, or an error message that
/// already includes the usage block on failure.
pub fn run(args: &[String]) -> Result<String, String> {
    cli::dispatch(args)
}

/// The short usage block, shown by `help` and appended to every error.
pub fn usage() -> String {
    "\
USAGE:
    oikos run <scenario> [--ticks N] [--seed S]
    oikos inspect price <scenario> --good NAME [--at-tick T] [--ticks N] [--seed S]
    oikos inspect colonist <scenario> --id N [--at-tick T] [--ticks N] [--seed S]
    oikos scenarios
    oikos help
"
    .to_string()
}

/// The `oikos help` text: a one-line summary, the usage block, and the command
/// descriptions.
pub fn help_text() -> String {
    let mut out = String::new();
    out.push_str(
        "oikos — a read-only debug viewer for an oikos settlement (game milestone G2d)\n\n",
    );
    out.push_str(&usage());
    out.push_str(
        "\nCOMMANDS:\n\
    \x20   run        run a scenario for N econ ticks; print a per-econ-tick dashboard\n\
    \x20   inspect    price → the trades behind it, or colonist → its value scale and why\n\
    \x20   scenarios  list the available scenarios\n\
    \x20   help       show this message\n",
    );
    out.push_str(
        "\nThe `region` and `region-control` scenarios (run only) advance a two-settlement\n\
         Region with / without a caravan and render the per-settlement prices and the\n\
         convergence gap over time (G2c). The price/colonist inspectors apply to the\n\
         single-settlement scenarios only.\n",
    );
    out.push_str(
        "\nThe viewer is deterministic: the same (scenario, ticks, seed) prints byte-\n\
         identical output. It renders from read-only accessors and changes no simulation\n\
         behavior. Defaults: seed 1; run ticks 20; inspect ticks 12.\n",
    );
    out
}

/// The region scenario name → [`RegionConfig`] lookup (the G2c multi-settlement
/// dashboards). Returns `None` for a non-region name, so the settlement path and
/// the inspectors keep handling the single-settlement scenarios. Read-only: it
/// authors no economics, only selects the `sim` constructors.
fn region_config_for(name: &str) -> Option<RegionConfig> {
    match name {
        "region" => Some(RegionConfig::two_settlements()),
        "region-control" => Some(RegionConfig::two_settlements_control()),
        _ => None,
    }
}

/// Run the `run` dashboard: advance `scenario` for `ticks` econ ticks from
/// `seed`, capturing a per-tick row, then render the dashboard. A `region` /
/// `region-control` scenario advances a two-settlement [`Region`] and renders the
/// per-settlement prices + convergence gap instead (G2c).
pub fn run_dashboard(scenario: &str, ticks: u64, seed: u64) -> Result<String, String> {
    if ticks == 0 {
        return Err("--ticks must be >= 1".to_string());
    }
    if let Some(region_config) = region_config_for(scenario) {
        return Ok(run_region_dashboard(scenario, &region_config, ticks, seed));
    }
    let config = config_for(scenario)?;
    let mut settlement = Settlement::generate(seed, &config);
    let goods: Vec<GoodId> = settlement.tracked_goods().to_vec();

    let mut rows = Vec::with_capacity(ticks as usize);
    for _ in 0..ticks {
        let report = settlement.econ_tick();
        let prices = goods
            .iter()
            .map(|&g| settlement.realized_price(g))
            .collect();
        let transferred = goods.iter().map(|&g| report.transferred_of(g)).collect();
        let produced = goods.iter().map(|&g| report.produced_of(g)).collect();
        let consumed_as_input = goods
            .iter()
            .map(|&g| report.consumed_as_input_of(g))
            .collect();
        let consumed = goods.iter().map(|&g| report.consumed_of(g)).collect();
        let conserves = report.conserves();
        let offending_good = if conserves {
            None
        } else {
            first_offending_good(&report, &goods)
        };
        let (hunger_max, hunger_sum, living) = living_hunger_stats(&settlement);
        let lineage_stats = settlement.lineage_stats();
        let lineage_living = lineage_stats.iter().map(|stats| stats.living).collect();
        let lineage_gold = lineage_stats.iter().map(|stats| stats.gold).collect();
        rows.push(render::DashboardRow {
            econ_tick: report.econ_tick,
            living_gatherers: settlement.living_count(Vocation::Gatherer),
            living_consumers: settlement.living_count(Vocation::Consumer),
            living_millers: settlement.living_count(Vocation::Miller),
            living_bakers: settlement.living_count(Vocation::Baker),
            living_unassigned: settlement.living_count(Vocation::Unassigned),
            prices,
            transferred,
            produced,
            consumed_as_input,
            consumed,
            conserves,
            offending_good,
            hunger_max,
            hunger_sum,
            living,
            births_total: settlement.births_total(),
            old_age_deaths_total: settlement.old_age_deaths_total(),
            lineage_living,
            lineage_gold,
        });
    }

    Ok(render::format_dashboard(
        &settlement,
        scenario,
        seed,
        ticks,
        &population_label(&settlement),
        &rows,
    ))
}

/// The dashboard's population line: the **living** total plus the living
/// per-vocation roster. A plain settlement with no deaths reads
/// `"12 (8 gatherers, 4 consumers)"` exactly as G2b; a chain settlement appends its
/// active producer roles and unassigned latent pool. The leading number is
/// [`Settlement::living_total`] (not the total-ever [`Settlement::population`]) so it
/// always equals the sum of the per-vocation counts below — once a colonist starves
/// (e.g. `starved-hauler`, or a long `emergent-chain` run) a total-ever prefix would
/// no longer add up to the living sub-counts.
fn population_label(settlement: &Settlement) -> String {
    // A demography settlement (G4b) has no vocational division of labor — its colony
    // is households (lineages), so the label is the living total broken down per
    // lineage with that lineage's accumulated gold.
    if settlement.is_demographic() {
        let lineage_stats = settlement.lineage_stats();
        let lineages: Vec<String> = (0..settlement.household_count())
            .map(|h| {
                let stats = lineage_stats.get(h).copied().unwrap_or_default();
                format!("lineage {h}: {} alive, {} gold", stats.living, stats.gold)
            })
            .collect();
        return format!("{} ({})", settlement.living_total(), lineages.join("; "));
    }
    let mut parts = vec![
        format!("{} gatherers", settlement.living_count(Vocation::Gatherer)),
        format!("{} consumers", settlement.living_count(Vocation::Consumer)),
    ];
    let millers = settlement.living_count(Vocation::Miller);
    if millers > 0 {
        parts.push(format!("{millers} millers"));
    }
    let bakers = settlement.living_count(Vocation::Baker);
    if bakers > 0 {
        parts.push(format!("{bakers} bakers"));
    }
    // G3b: the latent pool not (currently) producing.
    let unassigned = settlement.living_count(Vocation::Unassigned);
    if unassigned > 0 {
        parts.push(format!("{unassigned} unassigned"));
    }
    format!("{} ({})", settlement.living_total(), parts.join(", "))
}

/// Advance a two-settlement [`Region`] for `ticks` econ ticks from `seed`,
/// capturing a per-tick row, then render the region dashboard: the realized FOOD
/// price at each settlement, their convergence gap, the conservation flag, and the
/// in-transit caravan escrow. Read-only — it draws only from the `Region`'s
/// accessors over a seeded run, so the same `(scenario, ticks, seed)` is
/// byte-identical.
fn run_region_dashboard(scenario: &str, config: &RegionConfig, ticks: u64, seed: u64) -> String {
    let mut region = Region::generate(seed, config);
    let good = region.traded_good();
    let good_label = region
        .settlement(0)
        .map(|s| s.society().good_name(good).to_string())
        .unwrap_or_else(|| "good".to_string());

    let mut rows = Vec::with_capacity(ticks as usize);
    for _ in 0..ticks {
        let report = region.econ_tick();
        rows.push(render::RegionDashboardRow {
            econ_tick: report.econ_tick,
            price_a: region.realized_price(0, good),
            price_b: region.realized_price(1, good),
            gap: region.price_gap(good),
            conserves: report.conserves(),
            escrow_good: u64::from(region.escrow_good()),
            escrow_gold: region.escrow_gold(),
        });
    }

    render::format_region_dashboard(
        scenario,
        seed,
        ticks,
        &good_label,
        region.caravans_enabled(),
        &rows,
    )
}

/// Run the price → trades inspector for `good` at `at_tick` over a run of
/// `ticks` econ ticks (both derived by [`resolve_window`] when omitted).
pub fn run_price(
    scenario: &str,
    good: &str,
    at_tick: Option<u64>,
    ticks: Option<u64>,
    seed: u64,
) -> Result<String, String> {
    let (ticks, at_tick) = resolve_window(at_tick, ticks, DEFAULT_INSPECT_TICKS)?;
    let config = config_for(scenario)?;
    let mut settlement = Settlement::generate(seed, &config);
    settlement.run(ticks);
    let good_id = resolve_good(&settlement, good)?;
    let label = settlement.society().good_name(good_id).to_string();
    Ok(render::format_price(
        &settlement,
        scenario,
        seed,
        good_id,
        &label,
        at_tick,
        ticks,
    ))
}

/// Run the colonist → scale / why inspector for colonist `id` after a run of
/// `ticks` econ ticks (derived by [`resolve_window`] when omitted).
pub fn run_colonist(
    scenario: &str,
    id: usize,
    at_tick: Option<u64>,
    ticks: Option<u64>,
    seed: u64,
) -> Result<String, String> {
    let (ticks, at_tick) = resolve_window(at_tick, ticks, DEFAULT_INSPECT_TICKS)?;
    let config = config_for(scenario)?;
    let mut settlement = Settlement::generate(seed, &config);
    settlement.run(ticks);
    let population = settlement.population();
    if id >= population {
        return Err(format!(
            "colonist id {id} is out of range (this scenario has {population} colonists, ids 0..={})",
            population.saturating_sub(1)
        ));
    }
    Ok(render::format_colonist(
        &settlement,
        scenario,
        seed,
        id,
        at_tick,
        ticks,
    ))
}

/// Derive `(ticks, at_tick)` from the optionally-supplied flags. `at_tick` is
/// the inspected snapshot boundary: when it is supplied, the settlement is run
/// just long enough to finish that econ tick (`at_tick + 1` ticks), so the
/// realized price, trade tape, needs, carry, and value scale all describe the
/// same tick. If `--ticks` is also supplied it is treated as a reachability
/// bound and must include `at_tick`; extra ticks after the inspected one are not
/// advanced. When `at_tick` is omitted, `ticks` (or the default) decides the run
/// length and the last tick is inspected.
fn resolve_window(
    at_tick: Option<u64>,
    ticks: Option<u64>,
    default_ticks: u64,
) -> Result<(u64, u64), String> {
    if let Some(t) = at_tick {
        if let Some(n) = ticks {
            if n == 0 {
                return Err("--ticks must be >= 1".to_string());
            }
            if t >= n {
                return Err(format!(
                    "--at-tick {t} is beyond the run of {n} econ ticks (the last tick is {})",
                    n - 1
                ));
            }
        }
        let run_ticks = t
            .checked_add(1)
            .ok_or_else(|| "--at-tick is too large to run".to_string())?;
        return Ok((run_ticks, t));
    }

    let run_ticks = ticks.unwrap_or(default_ticks);
    if run_ticks == 0 {
        return Err("--ticks must be >= 1".to_string());
    }
    Ok((run_ticks, run_ticks - 1))
}

/// Resolve a good name (case-insensitive) against the scenario's tracked goods.
fn resolve_good(settlement: &Settlement, name: &str) -> Result<GoodId, String> {
    for &good in settlement.tracked_goods() {
        if settlement
            .society()
            .good_name(good)
            .eq_ignore_ascii_case(name)
        {
            return Ok(good);
        }
    }
    let available: Vec<&str> = settlement
        .tracked_goods()
        .iter()
        .map(|&g| settlement.society().good_name(g))
        .collect();
    Err(format!(
        "unknown good: {name:?} (this scenario trades: {})",
        available.join(", ")
    ))
}

/// The first tracked good whose whole-system ledger failed to balance this tick,
/// for the dashboard's loud `VIOLATED:<good>` cell. `None` when all balance.
/// Uses the G3a generalized invariant (production accounted), so it agrees with
/// [`EconTickReport::conserves`]; for a plain settlement the extra terms are zero
/// and it reduces to G2b's `before + regen − consumed`.
fn first_offending_good(report: &EconTickReport, goods: &[GoodId]) -> Option<GoodId> {
    goods.iter().copied().find(|&good| {
        let before = i128::from(report.whole_system_before_of(good));
        let after = i128::from(report.whole_system_after_of(good));
        let regen = i128::from(report.regen_of(good));
        let endowment = i128::from(report.endowment_of(good));
        let produced = i128::from(report.produced_of(good));
        let consumed_as_input = i128::from(report.consumed_as_input_of(good));
        let consumed = i128::from(report.consumed_of(good));
        after != before + regen + endowment + produced - consumed_as_input - consumed
    })
}

/// `(max, sum, count)` of hunger over the living colonists — the needs summary
/// the dashboard renders (max and a fixed-point mean). `max` is `None` when none
/// are living.
fn living_hunger_stats(settlement: &Settlement) -> (Option<u16>, u64, u64) {
    let mut max: Option<u16> = None;
    let mut sum = 0u64;
    let mut count = 0u64;
    for index in 0..settlement.population() {
        if !settlement.is_alive(index) {
            continue;
        }
        if let Some(need) = settlement.need_of(index) {
            max = Some(max.map_or(need.hunger, |m| m.max(need.hunger)));
            sum += u64::from(need.hunger);
            count += 1;
        }
    }
    (max, sum, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_window_derives_the_other_bound() {
        // Both given: at-tick is authoritative, ticks only proves reachability.
        assert_eq!(resolve_window(Some(3), Some(10), 12), Ok((4, 3)));
        assert_eq!(resolve_window(Some(3), Some(4), 12), Ok((4, 3)));
        // Only ticks: at-tick defaults to the last tick.
        assert_eq!(resolve_window(None, Some(10), 12), Ok((10, 9)));
        // Only at-tick: ticks is just long enough to reach it.
        assert_eq!(resolve_window(Some(5), None, 12), Ok((6, 5)));
        // Neither: the default run, inspecting its last tick.
        assert_eq!(resolve_window(None, None, 12), Ok((12, 11)));
    }

    #[test]
    fn resolve_window_rejects_at_tick_past_the_run() {
        let err = resolve_window(Some(10), Some(10), 12).unwrap_err();
        assert!(err.contains("--at-tick 10 is beyond the run"));
        assert!(resolve_window(Some(0), None, 12).is_ok()); // at-tick 0 → 1 tick
        assert!(resolve_window(None, None, 0).is_err()); // a zero-length default
    }
}
