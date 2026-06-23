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

use sim::{EconTickReport, EraDetector, GoodId, Region, RegionConfig, Settlement, Vocation};

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
         convergence gap over time (G2c). The `roads` and `roads-control` scenarios (run\n\
         only) add a road built from community labor: it cuts the route transit on\n\
         completion, so the gap converges faster than the no-road control (G7). The\n\
         price/colonist inspectors apply to the single-settlement scenarios only.\n",
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
        // G7: the road region and its no-road control (both run a caravan; the road
        // is the only difference). Run-only, like the other region scenarios.
        "roads" => Some(RegionConfig::roads()),
        "roads-control" => Some(RegionConfig::roads_control()),
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

    // G6a/G8c-1: classify the settlement's measured institutional era as the run
    // advances. Read-only — the detector observes (`&settlement`) and never changes the
    // run. Surfaced for the emergent path (barter→money→…) and the finance path
    // (…→credit→modern, the G8c-1 cycle), both of which climb the measured ladder.
    let mut detector = (settlement.is_emergent() || settlement.is_cycle()).then(EraDetector::new);

    let mut rows = Vec::with_capacity(ticks as usize);
    for _ in 0..ticks {
        let report = settlement.econ_tick();
        let era = detector
            .as_mut()
            .map(|d| d.observe(&settlement).label().to_string())
            .unwrap_or_default();
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
        // G5a emergence surfacing: the barter/money phase, the saleability leader,
        // and the emerged money good (with the promotion tick flagged). The columns
        // are hidden for non-emergent settlements.
        let good_name = |g: GoodId| settlement.society().good_name(g).to_string();
        let money_good = settlement.current_money_good();
        let phase = if money_good.is_some() {
            "money"
        } else {
            "barter"
        }
        .to_string();
        let saleability_leader = settlement
            .saleability_leader()
            .map_or("—".to_string(), good_name);
        let money_label = money_good.map_or("—".to_string(), good_name);
        let promoted_this_tick = settlement.promoted_at_tick() == Some(report.econ_tick);
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
            phase,
            saleability_leader,
            money_good: money_label,
            promoted_this_tick,
            era,
            // G6b research surfacing: the accumulated Knowledge (after this tick), the
            // Knowledge produced this tick (the non-conserved line), and the tier.
            knowledge: settlement.knowledge(),
            knowledge_produced: report.knowledge_produced(),
            tier: settlement.current_tier(),
            // G8c-1 cycle surfacing: the regime rung this tick (the ladder descent) and
            // the shadow gap (filled after the run, once the shadow replay is known).
            // Empty/None for a non-cycle settlement, so those dashboards are unchanged.
            regime: if settlement.is_cycle() {
                settlement.regime_label().to_string()
            } else {
                String::new()
            },
            gap_bps: None,
        });
    }

    // G8c-1: fill the per-tick shadow gap (shadow natural rate − market rate). The
    // shadow replay needs the whole run, so it is computed once here and back-filled
    // onto the rows. A no-op for a non-cycle settlement.
    if let Some(gaps) = settlement.shadow_gap_bps() {
        for (row, gap) in rows.iter_mut().zip(gaps) {
            row.gap_bps = gap;
        }
    }

    // The era banner: the current era and the tick each rung was first reached.
    let era_summary = detector.as_ref().map(|d| render::EraSummary {
        current: d.current_era().label().to_string(),
        timeline: d
            .timeline()
            .into_iter()
            .map(|(era, tick)| (era.label().to_string(), tick))
            .collect(),
    });

    // G6b research banner: the earned Knowledge, the current tier, and the unlock tick.
    let research_summary = settlement.is_research().then(|| render::ResearchSummary {
        knowledge: settlement.knowledge(),
        threshold: settlement.tier2_threshold(),
        tier: settlement.current_tier(),
        unlocked_at: settlement.tier2_unlocked_at(),
    });

    // G8a/G8b money banner: the M3 ledger composition (specie/fiat/claims/reserves/
    // fiduciary). `None` for a closed-GOLD M1 settlement, so non-M3 dashboards are
    // unchanged. A G8b bank fills in claims/reserves/fiduciary; G8a is pure specie.
    let money_summary = settlement
        .money_composition()
        .map(|c| render::MoneySummary {
            specie: c.public_specie.0,
            fiat: c.public_fiat.0,
            claims: c.demand_claims.0,
            reserves: c.bank_reserves.0,
            fiduciary: c.fiduciary.0,
        });

    // G8b bank banner: the chartered bank's balance sheet. `None` for a bank-free
    // settlement (G8a and earlier), so those dashboards are unchanged.
    let bank_summary = settlement.bank().map(|bank| render::BankSummary {
        name: bank.name.to_string(),
        reserves: bank.reserves.0,
        demand_deposits: bank.demand_deposits.0,
        fiduciary_issued: bank.fiduciary_issued.0,
        reserve_ratio_bps: bank.reserve_ratio_bps.0,
    });

    // G8c-1 credit-cycle banner: the regime rung, the measured shadow gap, the
    // boom/bust indicators, the capital consumed, and the fiat base. `None` for a
    // non-cycle settlement, so those dashboards are unchanged.
    let cycle_summary = settlement.cycle_kind().map(|kind| render::CycleSummary {
        kind: match kind {
            sim::CycleKind::CreditCycle => "credit-cycle",
            sim::CycleKind::SoundMoney => "sound-money",
        },
        regime: settlement.regime_label(),
        max_gap_bps: settlement.max_shadow_gap_bps(),
        boom_projects: settlement.boom_projects_started(),
        bust_abandoned: settlement.bust_abandoned_projects(),
        capital_consumed: settlement.capital_consumed(),
        fiat_base: settlement.fiat_base().0,
        // G8c-2: the wage tender is the cycle's transmission valve; the outcome is
        // whether the cycle fired, is still transmitting before the bust, is pending
        // before credit reaches the real economy, or is truly inert under wage refusal.
        wage_tender: sim::labor_wage_tender_name(settlement.labor_wage_tender()),
        outcome: cycle_outcome_label(&settlement, kind),
    });

    // G8c-2 tender banner: the active media-acceptance levers and (for a bench) the
    // demonstrated surface's settled fiat/specie split. Shown for any finance
    // settlement (the cycle and the spot/debt benches); `None` otherwise, so every
    // non-finance dashboard is unchanged.
    let tender_summary = if settlement.is_cycle() || settlement.is_tender_bench() {
        let (
            bench_surface,
            bench_fiat_settled,
            bench_claims_settled,
            bench_specie_settled,
            bench_credit_retired,
        ) = match settlement.bench_surface() {
            Some(sim::BenchSurface::Spot) => (
                Some("spot"),
                settlement.spot_fiat_settled().0,
                0,
                settlement.spot_specie_settled().0,
                0,
            ),
            Some(sim::BenchSurface::Debt) => (
                Some("debt"),
                settlement.debt_fiat_settled().0,
                0,
                settlement.debt_specie_settled().0,
                0,
            ),
            Some(sim::BenchSurface::BankRepayment) => (
                Some("bank-repayment"),
                settlement.bank_repayment_fiat_settled().0,
                settlement.bank_repayment_claims_settled().0,
                settlement.bank_repayment_specie_settled().0,
                settlement.bank_repayment_credit_retired().0,
            ),
            Some(sim::BenchSurface::IssuerRepayment) => (
                Some("issuer-repayment"),
                settlement.issuer_repayment_fiat_settled().0,
                0,
                settlement.issuer_repayment_specie_settled().0,
                settlement.issuer_repayment_credit_retired().0,
            ),
            None => (None, 0, 0, 0, 0),
        };
        Some(render::TenderSummary {
            spot: sim::public_spot_tender_name(settlement.public_spot_tender()),
            wage: sim::labor_wage_tender_name(settlement.labor_wage_tender()),
            debt: sim::public_debt_tender_name(settlement.public_debt_tender()),
            bank_repayment: sim::bank_repayment_tender_name(settlement.bank_repayment_tender()),
            issuer_repayment: sim::issuer_repayment_tender_name(
                settlement.issuer_repayment_tender(),
            ),
            bench_surface,
            bench_fiat_settled,
            bench_claims_settled,
            bench_specie_settled,
            bench_credit_retired,
            broad_money: settlement.total_broad_money().0,
        })
    } else {
        None
    };

    // G8c-3 tax banner: the active tax receivability (the chartalist counter-lever) and
    // the levy/receipt/default split. Shown only for a tax settlement (`tax-in-fiat` /
    // `tax-in-specie`); `None` otherwise, so every non-tax dashboard is unchanged.
    let tax_summary = if settlement.is_tax() {
        Some(render::TaxSummary {
            receivability: sim::tax_receivability_name(settlement.tax_receivability()),
            levied: settlement.taxes_levied().0,
            receipts_fiat: settlement.tax_receipts_fiat().0,
            receipts_specie: settlement.tax_receipts_specie().0,
            defaulted: settlement.taxes_defaulted().0,
        })
    } else {
        None
    };

    // S8.0 emergence-probe banner: the read-only co-emergence diagnostics (promotion
    // tick, per-candidate barter saleability, the bread-for-SALT leg, producer working
    // capital, pre-promotion hunger trough). Shown only for an emergent settlement, so
    // every designated-money dashboard is unchanged.
    let probe_summary = settlement.is_emergent().then(|| {
        let good_name = |g: GoodId| settlement.society().good_name(g).to_string();
        let candidates = settlement
            .emergence_acceptances()
            .into_iter()
            .map(|c| (good_name(c.good), c.acceptances, c.acceptor_agents))
            .collect();
        // Fold per-producer working capital into (role, count, total free gold) rows.
        let mut producer_cash: Vec<(&'static str, usize, u64)> = Vec::new();
        for cash in settlement.producer_cash() {
            let label = match cash.role {
                sim::ProducerRole::Miller => "Miller",
                sim::ProducerRole::Baker => "Baker",
                sim::ProducerRole::LatentMiller => "latent-Miller",
                sim::ProducerRole::LatentBaker => "latent-Baker",
            };
            match producer_cash.iter_mut().find(|(role, ..)| *role == label) {
                Some(entry) => {
                    entry.1 += 1;
                    entry.2 = entry.2.saturating_add(cash.free_gold);
                }
                None => producer_cash.push((label, 1, cash.free_gold)),
            }
        }
        render::EmergenceProbeSummary {
            promoted_at_tick: settlement.promoted_at_tick(),
            bread_for_salt_volume: settlement.bread_for_salt_volume(),
            peak_pre_promotion_hunger: settlement.peak_pre_promotion_hunger(),
            critical_ticks: settlement.critical_ticks_before_promotion(),
            candidates,
            producer_cash,
        }
    });

    let banners = render::DashboardBanners {
        era: era_summary.as_ref(),
        research: research_summary.as_ref(),
        money: money_summary.as_ref(),
        bank: bank_summary.as_ref(),
        cycle: cycle_summary.as_ref(),
        tender: tender_summary.as_ref(),
        tax: tax_summary.as_ref(),
        probe: probe_summary.as_ref(),
    };
    Ok(render::format_dashboard(
        &settlement,
        scenario,
        seed,
        ticks,
        &population_label(&settlement),
        &banners,
        &rows,
    ))
}

fn cycle_outcome_label(settlement: &Settlement, kind: sim::CycleKind) -> &'static str {
    if settlement.cycle_fired() {
        return "fired";
    }
    if settlement.boom_projects_started() > 0 || settlement.structure_rose_above_shadow() {
        return "transmitting";
    }
    // G8c-2: `inert` is the wage-refusal outcome (the M17 control) — credit was issued
    // but the wage surface refuses fiat, so it can never reach the real economy. A
    // legal/par wage tender whose boom simply hasn't started yet (a short-horizon
    // dashboard, before any project breaks ground) is `pending`, not inert; gating on
    // the wage tender's accepted media keeps fiat-legal cycles from being mislabeled.
    let wage_refuses_fiat = !settlement.labor_wage_tender().accepted_media().fiat;
    if wage_refuses_fiat && settlement.credit_ever_circulated() {
        return "inert";
    }
    match kind {
        sim::CycleKind::CreditCycle => "pending",
        sim::CycleKind::SoundMoney => "no-credit",
    }
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
    // G6b: scholars (research) and confectioners (tier-2 production).
    let scholars = settlement.living_count(Vocation::Scholar);
    if scholars > 0 {
        parts.push(format!("{scholars} scholars"));
    }
    let confectioners = settlement.living_count(Vocation::Confectioner);
    if confectioners > 0 {
        parts.push(format!("{confectioners} confectioners"));
    }
    let cycle_a = settlement.living_count(Vocation::CycleA);
    if cycle_a > 0 {
        parts.push(format!("{cycle_a} cycle-A"));
    }
    let cycle_b = settlement.living_count(Vocation::CycleB);
    if cycle_b > 0 {
        parts.push(format!("{cycle_b} cycle-B"));
    }
    let cycle_c = settlement.living_count(Vocation::CycleC);
    if cycle_c > 0 {
        parts.push(format!("{cycle_c} cycle-C"));
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
        // G7 road surfacing: build progress (labor/cost while building, built@tick
        // once complete) and the current route transit (which the road cuts).
        let road = if !region.has_road() {
            "—".to_string()
        } else if region.road_complete() {
            format!(
                "built@{}",
                region.road_completed_at().unwrap_or(report.econ_tick)
            )
        } else {
            format!(
                "{}/{}",
                region.road_labor_advanced().unwrap_or(0),
                region.road_labor_cost().unwrap_or(0)
            )
        };
        rows.push(render::RegionDashboardRow {
            econ_tick: report.econ_tick,
            price_a: region.realized_price(0, good),
            price_b: region.realized_price(1, good),
            gap: region.price_gap(good),
            conserves: report.conserves(),
            escrow_good: u64::from(region.escrow_good()),
            escrow_gold: region.escrow_gold(),
            transit_ticks: region.route_transit_ticks(),
            road,
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
/// Uses the G3a generalized invariant (production accounted) plus the G5a
/// promotion sink, so it agrees with [`EconTickReport::conserves`] term-for-term
/// — without the `promoted` term it would misattribute the promotion tick (where
/// the winning good's stock leaves the physical ledger 1-for-1 into money) as a
/// violation of that good. For a plain settlement the extra terms are zero and it
/// reduces to G2b's `before + regen − consumed`.
fn first_offending_good(report: &EconTickReport, goods: &[GoodId]) -> Option<GoodId> {
    goods.iter().copied().find(|&good| {
        let before = i128::from(report.whole_system_before_of(good));
        let after = i128::from(report.whole_system_after_of(good));
        let regen = i128::from(report.regen_of(good));
        let endowment = i128::from(report.endowment_of(good));
        let produced = i128::from(report.produced_of(good));
        let consumed_as_input = i128::from(report.consumed_as_input_of(good));
        let consumed = i128::from(report.consumed_of(good));
        let promoted = i128::from(report.promoted_of(good));
        after != before + regen + endowment + produced - consumed_as_input - consumed - promoted
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
