//! The deterministic text renderers — the dashboard and the two inspectors.
//!
//! Every renderer returns a `String` (never writes stdout) so it is unit-
//! testable, and draws **no** randomness: it reads only from `sim`'s read-only
//! accessors over a settlement that was advanced by a seeded run, so the same
//! `(scenario, ticks, seed)` yields byte-identical output. Formatting is std
//! only — column-aligned plain text, no TUI / color / graphics (that is G9).

use std::fmt::Write as _;

use sim::{EstateDestination, Gold, GoodId, Horizon, Settlement, WantKind};

use crate::scenarios::description_for;

/// Column alignment for [`render_table`].
enum Align {
    Left,
    Right,
}

/// Render a column-aligned table: a header row, a dashed separator, then the
/// data rows. Column widths are the max of the header and every cell, so the
/// layout is a pure function of the content — deterministic. Trailing padding is
/// trimmed so no line carries stray spaces.
fn render_table(headers: &[&str], rows: &[Vec<String>], aligns: &[Align]) -> String {
    let cols = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }

    let mut out = String::new();

    let mut header_line = String::new();
    for (i, header) in headers.iter().enumerate() {
        if i > 0 {
            header_line.push_str(" | ");
        }
        header_line.push_str(&pad(header, widths[i], &aligns[i]));
    }
    out.push_str(header_line.trim_end());
    out.push('\n');

    let mut sep = String::new();
    for (i, width) in widths.iter().enumerate().take(cols) {
        if i > 0 {
            sep.push_str("-+-");
        }
        sep.push_str(&"-".repeat(*width));
    }
    out.push_str(&sep);
    out.push('\n');

    for row in rows {
        let mut line = String::new();
        for (i, cell) in row.iter().enumerate() {
            if i > 0 {
                line.push_str(" | ");
            }
            line.push_str(&pad(cell, widths[i], &aligns[i]));
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }

    out
}

/// Pad `s` to `width` columns on the side dictated by `align`.
fn pad(s: &str, width: usize, align: &Align) -> String {
    let len = s.chars().count();
    if len >= width {
        return s.to_string();
    }
    let fill = " ".repeat(width - len);
    match align {
        Align::Left => format!("{s}{fill}"),
        Align::Right => format!("{fill}{s}"),
    }
}

/// A realized-price cell: the price, or an em dash when no trade in the good has
/// cleared yet.
fn price_cell(price: Option<Gold>) -> String {
    match price {
        Some(p) => p.0.to_string(),
        None => "—".to_string(),
    }
}

/// The most recent econ tick at or before `at_tick` on which a trade in `good`
/// cleared — i.e. the tick that set the carried-over realized price the price
/// inspector reports when the inspected tick itself cleared no trade. The tape is
/// read-only and appended in tick order; `None` when no such trade exists.
fn last_cleared_tick(settlement: &Settlement, good: GoodId, at_tick: u64) -> Option<u64> {
    settlement
        .society()
        .trades
        .iter()
        .filter(|t| t.good == good && t.tick <= at_tick)
        .map(|t| t.tick)
        .max()
}

/// A want's kind as a label — the good's registry name, or `leisure`.
fn want_kind_label(settlement: &Settlement, kind: WantKind) -> String {
    match kind {
        WantKind::Good(good) => settlement.society().good_name(good).to_string(),
        WantKind::Leisure => "leisure".to_string(),
    }
}

/// A want's planning horizon as a label.
fn horizon_label(horizon: Horizon) -> String {
    match horizon {
        Horizon::Now => "Now".to_string(),
        Horizon::Next => "Next".to_string(),
        Horizon::Later(ticks) => format!("Later({ticks})"),
    }
}

/// One captured row of the `run` dashboard — the per-econ-tick snapshot the
/// loop records right after [`Settlement::econ_tick`], in tracked-good order.
pub struct DashboardRow {
    pub econ_tick: u64,
    pub living_gatherers: usize,
    pub living_consumers: usize,
    /// Living colonists currently of each producer role — for a chain settlement
    /// only (rendered behind `show_production`). In the G3b emergent scenario these
    /// rise from zero as latent colonists *adopt* milling/baking from the spread (and
    /// `living_unassigned` is the latent pool not yet producing); in G3a they are the
    /// constant seeded roster.
    pub living_millers: usize,
    pub living_bakers: usize,
    pub living_unassigned: usize,
    /// Realized price per tracked good (good order), `None` if none cleared yet.
    pub prices: Vec<Option<Gold>>,
    /// Units transferred world→econ this tick, per tracked good.
    pub transferred: Vec<u64>,
    /// Units produced by recipes this tick, per tracked good.
    pub produced: Vec<u64>,
    /// Units consumed as recipe inputs this tick, per tracked good.
    pub consumed_as_input: Vec<u64>,
    /// Units consumed this tick, per tracked good.
    pub consumed: Vec<u64>,
    pub conserves: bool,
    /// The first good whose ledger failed, if conservation broke (loud column).
    pub offending_good: Option<GoodId>,
    /// Highest hunger among living colonists, `None` if none are living.
    pub hunger_max: Option<u16>,
    /// Sum of living colonists' hunger and the living count (for the mean).
    pub hunger_sum: u64,
    pub living: u64,
    /// G4b demography surfacing (rendered behind `show_demography`): lifetime births
    /// and old-age deaths so far, and the living count + accumulated gold of each
    /// lineage. Empty / zero for a non-demography settlement.
    pub births_total: u64,
    pub old_age_deaths_total: u64,
    pub lineage_living: Vec<usize>,
    pub lineage_gold: Vec<u64>,
    /// G5a emergence surfacing (rendered behind `show_emergence`): the phase
    /// (`barter` until a money good emerges, then `money`), the current saleability
    /// leader the barter routes through (good name, or `—`), and the emerged money
    /// good once promoted (or `—` while still in barter). `*` marks the promotion
    /// tick. Empty for a non-emergent (designated-money) settlement.
    pub phase: String,
    pub saleability_leader: String,
    pub money_good: String,
    pub promoted_this_tick: bool,
    /// G6a/G8c-1 era surfacing (rendered behind the era banner): the measured
    /// institutional era at this tick
    /// (`forager`/`barter`/`money`/`specialist`/`capital`/`credit`/`modern`). Empty for
    /// a settlement whose path the era ladder does not classify (a designated-money
    /// non-finance settlement).
    pub era: String,
    /// G6b research surfacing (rendered behind `show_research`): the accumulated
    /// Knowledge this tick, the current tech tier, and the Knowledge produced this
    /// tick (the non-conserved accumulator line). Zero for a non-research settlement.
    pub knowledge: u64,
    pub knowledge_produced: u64,
    pub tier: u8,
    /// G8c-1 cycle surfacing (rendered behind `show_cycle`): the regime rung this tick
    /// (the ladder descent — `sound-gold`/`fractional`/`suspended`/`fiat`) and the
    /// measured shadow gap (shadow natural rate − market rate, bps; `None` if no rate
    /// cleared on one side). Empty/`None` for a non-cycle settlement.
    pub regime: String,
    pub gap_bps: Option<i64>,
}

/// G6b research-banner summary: the settlement's final Knowledge level, current tech
/// tier, and the tick tier 2 unlocked (or that it is still locked). A read-only digest
/// rendered above the dashboard for a research settlement. `None` for any other.
pub struct ResearchSummary {
    pub knowledge: u64,
    pub threshold: u64,
    pub tier: u8,
    pub unlocked_at: Option<u64>,
}

/// The optional milestone banners rendered above the dashboard table: the G6a era
/// summary, the G6b research summary, the G8a M3 money composition, and the G8b bank
/// balance sheet. Each is `None` unless its overlay is present (era for an emergent
/// settlement, research for a research settlement, money for an M3 ledger settlement,
/// bank for a chartered-bank settlement). Grouped so the renderer takes one banner
/// argument rather than many.
#[derive(Default)]
pub struct DashboardBanners<'a> {
    pub era: Option<&'a EraSummary>,
    pub research: Option<&'a ResearchSummary>,
    pub money: Option<&'a MoneySummary>,
    pub bank: Option<&'a BankSummary>,
    pub cycle: Option<&'a CycleSummary>,
    pub tender: Option<&'a TenderSummary>,
    pub tax: Option<&'a TaxSummary>,
    pub probe: Option<&'a EmergenceProbeSummary>,
}

/// S8.0 emergence-probe summary: the read-only diagnostics that separate a
/// *principled* co-emergence failure from a tuning one — the promotion tick, the
/// per-candidate barter saleability (acceptances + acceptor breadth), the
/// bread-for-SALT leg that monetizes SALT, the chain producers' working capital
/// (free gold by role, the Tension-B trace), and the pre-promotion hunger trough
/// (Tension A). A read-only digest rendered above the dashboard for an emergent
/// settlement; `None` for a designated-money settlement.
pub struct EmergenceProbeSummary {
    pub promoted_at_tick: Option<u64>,
    pub bread_for_salt_volume: u64,
    pub peak_pre_promotion_hunger: u16,
    pub critical_ticks: u64,
    /// `(good name, acceptances, distinct acceptors)` per money candidate.
    pub candidates: Vec<(String, u64, usize)>,
    /// `(role label, count, total free gold)` per chain-producer role present.
    pub producer_cash: Vec<(&'static str, usize, u64)>,
}

/// G8c-1 credit-cycle summary: the demonstration kind, the final regime rung, and the
/// MEASURED cycle signals — the largest positive shadow gap, the boom project starts,
/// the bust abandonments, the capital consumed, and the fiat base (issued − retired). A
/// read-only digest rendered above the dashboard for a finance (cycle) settlement.
/// `None` for any other. The credit cycle shows a positive gap and nonzero
/// boom/bust/capital; the sound-money control shows all zeros (the falsification twin).
pub struct CycleSummary {
    pub kind: &'static str,
    pub regime: &'static str,
    pub max_gap_bps: i64,
    pub boom_projects: u32,
    pub bust_abandoned: u32,
    pub capital_consumed: u64,
    pub fiat_base: u64,
    /// G8c-2: the active labor-wage tender — the cycle's transmission valve. Under
    /// `fiat-and-specie`/`par-all` the fiat credit reaches workers and the cycle
    /// fires; under `specie-only` the same credit is inert.
    pub wage_tender: &'static str,
    /// G8c-2: the cycle outcome under the active wage tender — `"fired"` (boom→bust
    /// transmitted), `"transmitting"` (boom active before the bust), `"pending"` (the
    /// credit cycle has not reached the real economy yet), `"inert"` (credit issued but
    /// refused at wages), or `"no-credit"` (the sound-money control).
    pub outcome: &'static str,
}

/// G8c-2 tender-policy summary: the active media-acceptance levers on a finance
/// settlement, and — for a tender bench — which surface it demonstrates and the
/// fiat/claim/specie split that surface settled in. A read-only digest rendered above
/// the dashboard for a finance settlement (the cycle and the tender benches). `None`
/// for a non-finance settlement. Tender gates *composition* (which medium settles a
/// surface), never creates money; repayment benches additionally surface the credit
/// retired by the accepted repayment medium.
pub struct TenderSummary {
    /// The active public-spot tender label (e.g. `specie-only`, `fiat-and-specie`).
    pub spot: &'static str,
    /// The active labor-wage tender label.
    pub wage: &'static str,
    /// The active public-debt tender label.
    pub debt: &'static str,
    /// The active bank-repayment tender label.
    pub bank_repayment: &'static str,
    /// The active issuer-repayment tender label.
    pub issuer_repayment: &'static str,
    /// Which bench surface, or `None` for the cycle.
    pub bench_surface: Option<&'static str>,
    /// Fiat that settled the bench's surface (the demonstrated composition split).
    pub bench_fiat_settled: u64,
    /// Demand claims that settled the bench's surface.
    pub bench_claims_settled: u64,
    /// Specie that settled the bench's surface.
    pub bench_specie_settled: u64,
    /// Credit retired by the bench's repayment surface.
    pub bench_credit_retired: u64,
    /// The settlement's total broad money after the run.
    pub broad_money: u64,
}

/// G8c-3 tax-receivability summary: the state's counter-lever to G8c-2. The active tax
/// receivability (the chartalist gate), the total levied, the fiat vs specie receipts,
/// and the defaults. A read-only digest rendered above the dashboard for a tax
/// settlement (the `tax-in-fiat` headline and its `tax-in-specie` control); `None` for
/// every settlement that levies no tax. The fiat-receivable tax compels fiat through the
/// fiscal channel (`receipts_fiat > 0`) where the labor market refused it; the
/// specie-receivable control compels none (`receipts_fiat == 0`, `receipts_specie > 0`).
/// A default is a levy unmet by rule (the holder lacks the receivable medium), conserved.
pub struct TaxSummary {
    /// The active tax receivability label (`fiat-only`, `specie-only`, `fiat-and-specie`).
    pub receivability: &'static str,
    /// Total tax levied (the zero-principal liabilities the state raised).
    pub levied: u64,
    /// Tax settled in fiat — the chartalist headline signal (positive iff fiat-receivable).
    pub receipts_fiat: u64,
    /// Tax settled in specie.
    pub receipts_specie: u64,
    /// Tax defaulted — a levy unmet by rule (not a leak; conserved).
    pub defaulted: u64,
}

/// G8a/G8b M3 money-composition summary: the settlement's M3 ledger money broken into
/// its components. A read-only digest of the [`sim::Settlement::money_composition`]
/// snapshot, rendered above the dashboard for an M3 settlement. `None` for a closed-GOLD
/// M1 settlement. In G8a (no bank) the money is pure **specie**: `fiat`, `claims`,
/// `reserves`, and `fiduciary` are all zero. In G8b a fractional bank makes `claims`,
/// `reserves`, and `fiduciary` (claims beyond reserves) all nonzero, while the
/// 100%-reserve control keeps `fiduciary` zero. `fiat` stays zero until G8c.
pub struct MoneySummary {
    pub specie: u64,
    pub fiat: u64,
    pub claims: u64,
    pub reserves: u64,
    pub fiduciary: u64,
}

/// G8b bank balance-sheet summary: the chartered bank's reserves, demand-deposit
/// liabilities, fiduciary credit issued, and reserve ratio. A read-only digest of the
/// reused econ [`Bank`](econ::bank::Bank) the sim charters, rendered above the dashboard
/// for a banked settlement. `None` without a bank charter (G8a and earlier). The
/// reserve ratio is shown in percent; `fiduciary_issued` is the credit the bank created
/// beyond its reserves (zero for the 100%-reserve control).
pub struct BankSummary {
    pub name: String,
    pub reserves: u64,
    pub demand_deposits: u64,
    pub fiduciary_issued: u64,
    pub reserve_ratio_bps: u16,
}

/// G6a era-banner summary: the current institutional era and the timeline of the
/// tick each rung was first reached. A read-only digest of the [`sim::EraDetector`]
/// the dashboard ran alongside the settlement. `None` for a non-emergent settlement
/// (the era ladder classifies the emergent barter→money→specialist→capital path).
pub struct EraSummary {
    /// The current era's label.
    pub current: String,
    /// `(era label, first tick)` for each rung reached, lowest rung first.
    pub timeline: Vec<(String, u64)>,
}

/// Format a reserve ratio (basis points) as a percent for the bank banner, using
/// integer math only. `2_000 → "20%"`, `10_000 → "100%"`, `1_550 → "15.5%"`.
fn format_reserve_ratio(bps: u16) -> String {
    let whole = bps / 100;
    let frac = bps % 100;
    if frac == 0 {
        format!("{whole}%")
    } else if frac.is_multiple_of(10) {
        // `u16::is_multiple_of` is a stabilized std inherent method (no external trait,
        // no `num_integer`); clippy's `manual_is_multiple_of` mandates it over `% 10 == 0`.
        format!("{whole}.{}%", frac / 10)
    } else {
        format!("{whole}.{frac:02}%")
    }
}

/// Format a fixed-point mean (one decimal) from an integer sum and count using
/// integer math only — no float, so it is bit-stable. `None` count → `-`.
fn mean_one_decimal(sum: u64, count: u64) -> String {
    if count == 0 {
        return "-".to_string();
    }
    let tenths = (sum * 10) / count;
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// Render the `run` dashboard: a header block then one table row per econ tick.
pub fn format_dashboard(
    settlement: &Settlement,
    scenario: &str,
    seed: u64,
    ticks: u64,
    population_label: &str,
    banners: &DashboardBanners,
    rows: &[DashboardRow],
) -> String {
    let goods = settlement.tracked_goods();
    let good_names: Vec<String> = goods
        .iter()
        .map(|&g| settlement.society().good_name(g).to_string())
        .collect();

    let mut out = String::new();
    let _ = writeln!(
        out,
        "oikos run — scenario {scenario:?}: {}",
        description_for(scenario)
    );
    let _ = writeln!(
        out,
        "seed {seed} · {ticks} econ ticks · population {population_label}"
    );
    let _ = writeln!(out, "tracked goods: {}", good_names.join(", "));
    // G6a era banner: the measured era reached and the tick each rung was earned —
    // "eras are earned, not timed". Shown only for an emergent settlement (the ladder
    // classifies the barter→money→specialist→capital path).
    if let Some(era) = banners.era {
        let timeline: Vec<String> = era
            .timeline
            .iter()
            .map(|(label, tick)| format!("{label}@{tick}"))
            .collect();
        let trail = if timeline.is_empty() {
            String::new()
        } else {
            format!(" — {}", timeline.join(" → "))
        };
        let _ = writeln!(out, "era: {}{trail}", era.current);
    }
    // S8.0 emergence-probe banner: the diagnostics that tell a principled co-emergence
    // failure from a tuning one — the promotion tick, the per-candidate barter
    // saleability, the bread-for-SALT monetizing leg, the producers' working capital
    // (Tension B), and the pre-promotion hunger trough (Tension A). Shown only for an
    // emergent settlement, so non-emergent dashboards are unchanged.
    if let Some(probe) = banners.probe {
        let promo = probe
            .promoted_at_tick
            .map_or("—".to_string(), |t| format!("@{t}"));
        let _ = writeln!(
            out,
            "emergence-probe: promoted {promo} · bread-for-salt {} · peak-hunger {} (critical {} ticks)",
            probe.bread_for_salt_volume, probe.peak_pre_promotion_hunger, probe.critical_ticks
        );
        if !probe.candidates.is_empty() {
            let candidates: Vec<String> = probe
                .candidates
                .iter()
                .map(|(name, acc, acceptors)| format!("{name} {acc}×({acceptors} acc)"))
                .collect();
            let _ = writeln!(out, "  candidates: {}", candidates.join(" · "));
        }
        if !probe.producer_cash.is_empty() {
            let cash: Vec<String> = probe
                .producer_cash
                .iter()
                .map(|(role, count, free)| format!("{role} {count}×=free {free}"))
                .collect();
            let _ = writeln!(out, "  producer capital: {}", cash.join(" · "));
        }
    }
    // G6b research banner: the earned Knowledge, the current tech tier, and the tick
    // tier 2 unlocked — "capabilities are earned by research, not unlocked by a timer".
    if let Some(research) = banners.research {
        let unlock = match research.unlocked_at {
            Some(tick) => format!("tier 2 unlocked at tick {tick}"),
            None => format!(
                "tier 2 locked ({}/{} knowledge)",
                research.knowledge, research.threshold
            ),
        };
        let _ = writeln!(
            out,
            "research: knowledge {} · tier {} · {unlock}",
            research.knowledge, research.tier
        );
    }
    // G8a/G8b money banner: the M3 ledger composition — "money is ledger-accounted
    // now". In G8a it is pure specie; a G8b bank makes claims/reserves/fiduciary
    // nonzero (the 100%-reserve control keeps fiduciary zero). Shown only for an M3
    // settlement (the closed-GOLD M1 path surfaces none).
    if let Some(money) = banners.money {
        let _ = writeln!(
            out,
            "money: M3 ledger — specie {} · fiat {} · claims {} · reserves {} · fiduciary {}",
            money.specie, money.fiat, money.claims, money.reserves, money.fiduciary
        );
    }
    // G8b bank banner: the chartered bank's balance sheet — deposits taken in as
    // reserves and the fiduciary credit lent beyond them, gated by the reserve ratio.
    // Shown only for a banked settlement.
    if let Some(bank) = banners.bank {
        let _ = writeln!(
            out,
            "bank: {} — reserves {} · deposits {} · fiduciary issued {} · reserve ratio {}",
            bank.name,
            bank.reserves,
            bank.demand_deposits,
            bank.fiduciary_issued,
            format_reserve_ratio(bank.reserve_ratio_bps),
        );
    }
    // G8c-1 cycle banner: the regime rung, the measured shadow gap, and the boom/bust /
    // capital-consumed / fiat-base signals. The credit cycle shows a positive gap and
    // nonzero boom/bust/capital; the sound-money control shows all zeros. Shown only for
    // a finance (cycle) settlement.
    if let Some(cycle) = banners.cycle {
        let _ = writeln!(
            out,
            "cycle: {} — regime {} · shadow gap(max) {} bps · boom {} · bust {} · \
             capital consumed {} · fiat base {} · wages {} · {}",
            cycle.kind,
            cycle.regime,
            cycle.max_gap_bps,
            cycle.boom_projects,
            cycle.bust_abandoned,
            cycle.capital_consumed,
            cycle.fiat_base,
            cycle.wage_tender,
            cycle.outcome,
        );
    }
    // G8c-2 tender banner: the active media-acceptance levers and, for a tender bench,
    // the demonstrated surface's fiat/claim/specie settlement split. Repayment benches
    // also show the credit retired by the accepted repayment medium.
    // Shown for any finance settlement.
    if let Some(tender) = banners.tender {
        let _ = write!(
            out,
            "tender: spot {} · wage {} · debt {} · bank-repayment {} · issuer-repayment {}",
            tender.spot, tender.wage, tender.debt, tender.bank_repayment, tender.issuer_repayment,
        );
        if let Some(surface) = tender.bench_surface {
            if tender.bench_claims_settled > 0 || surface == "bank-repayment" {
                let _ = write!(
                    out,
                    " · {surface} settled fiat {} / claims {} / specie {}",
                    tender.bench_fiat_settled,
                    tender.bench_claims_settled,
                    tender.bench_specie_settled,
                );
            } else {
                let _ = write!(
                    out,
                    " · {surface} settled fiat {} / specie {}",
                    tender.bench_fiat_settled, tender.bench_specie_settled,
                );
            }
            if tender.bench_credit_retired > 0 || surface.ends_with("repayment") {
                let _ = write!(out, " · credit retired {}", tender.bench_credit_retired);
            }
        }
        let _ = writeln!(out, " · broad money {}", tender.broad_money);
    }
    // G8c-3 tax banner: the active tax receivability (the chartalist gate) and the
    // levy/receipt/default split. The fiat-receivable headline shows receipts in fiat
    // (the fiscal channel) where wages refused it; the specie-receivable control shows
    // receipts in specie and none in fiat. Shown only for a tax settlement.
    if let Some(tax) = banners.tax {
        let _ = writeln!(
            out,
            "tax: receivability {} · levied {} · receipts fiat {} / specie {} · defaulted {}",
            tax.receivability, tax.levied, tax.receipts_fiat, tax.receipts_specie, tax.defaulted,
        );
    }
    out.push('\n');

    // Headers: the per-tick fixed columns, then price/volume columns per tracked
    // good. Chain settlements include the production receipt columns too; plain
    // G2 dashboards keep their original shape.
    let show_production = settlement.content().is_some();
    let mut headers: Vec<String> = vec!["tick".to_string(), "gath".to_string(), "cons".to_string()];
    let mut aligns: Vec<Align> = vec![Align::Right, Align::Right, Align::Right];
    // Chain settlements show the producer roles (G3b: the count that has *adopted*
    // milling/baking from the spread, plus the latent pool still idle).
    if show_production {
        headers.push("mill".to_string());
        headers.push("bake".to_string());
        headers.push("idle".to_string());
        aligns.push(Align::Right);
        aligns.push(Align::Right);
        aligns.push(Align::Right);
    }
    // G6b research columns: the accumulated Knowledge, the Knowledge produced this tick
    // (the non-conserved accumulator line), and the current tech tier. Shown only for a
    // research settlement.
    let show_research = banners.research.is_some();
    if show_research {
        headers.push("know".to_string());
        headers.push("k.tick".to_string());
        headers.push("tier".to_string());
        aligns.push(Align::Right);
        aligns.push(Align::Right);
        aligns.push(Align::Right);
    }
    // G5a emergence columns: the barter/money phase, the saleability leader the
    // barter routes through, and the emerged money good (with the promotion tick
    // marked `*`). Shown only for a barter-start (emergent-money) settlement.
    let show_emergence = settlement.is_emergent();
    if show_emergence {
        headers.push("phase".to_string());
        headers.push("leader".to_string());
        headers.push("money".to_string());
        aligns.push(Align::Left);
        aligns.push(Align::Left);
        aligns.push(Align::Left);
    }
    // G6a era column: the measured institutional era this tick — the headline
    // surfacing alongside the banner. Shown when an era summary is provided (an
    // emergent settlement).
    let show_era = banners.era.is_some();
    if show_era {
        headers.push("era".to_string());
        aligns.push(Align::Left);
    }
    // G8c-1 cycle columns: the regime rung this tick (the ladder descent) and the
    // measured shadow gap (shadow natural rate − market rate, bps). Shown only for a
    // finance (cycle) settlement.
    let show_cycle = banners.cycle.is_some();
    if show_cycle {
        headers.push("regime".to_string());
        headers.push("gap.bps".to_string());
        aligns.push(Align::Left);
        aligns.push(Align::Right);
    }
    headers.push("consv".to_string());
    headers.push("hung.max".to_string());
    headers.push("hung.mean".to_string());
    aligns.push(Align::Left);
    aligns.push(Align::Right);
    aligns.push(Align::Right);
    // G4b demography columns: total population, lifetime births and old-age deaths,
    // and per-lineage living count + accumulated gold (the patient/present-biased
    // wealth surfacing). Shown only for a demography settlement.
    let show_demography = settlement.is_demographic();
    let household_count = settlement.household_count();
    if show_demography {
        headers.push("pop".to_string());
        headers.push("born".to_string());
        headers.push("died".to_string());
        aligns.push(Align::Right);
        aligns.push(Align::Right);
        aligns.push(Align::Right);
        for h in 0..household_count {
            headers.push(format!("L{h}.n"));
            headers.push(format!("L{h}.gold"));
            aligns.push(Align::Right);
            aligns.push(Align::Right);
        }
    }
    for name in &good_names {
        headers.push(format!("{name}.px"));
        headers.push(format!("{name}.xfer"));
        if show_production {
            headers.push(format!("{name}.made"));
            headers.push(format!("{name}.input"));
        }
        headers.push(format!("{name}.eaten"));
        aligns.push(Align::Right);
        aligns.push(Align::Right);
        if show_production {
            aligns.push(Align::Right);
            aligns.push(Align::Right);
        }
        aligns.push(Align::Right);
    }

    let mut table_rows: Vec<Vec<String>> = Vec::with_capacity(rows.len());
    for row in rows {
        let consv = if row.conserves {
            "OK".to_string()
        } else {
            match row.offending_good {
                Some(good) => format!("VIOLATED:{}", settlement.society().good_name(good)),
                None => "VIOLATED".to_string(),
            }
        };
        let hunger_max = match row.hunger_max {
            Some(h) => h.to_string(),
            None => "-".to_string(),
        };
        let mut cells = vec![
            row.econ_tick.to_string(),
            row.living_gatherers.to_string(),
            row.living_consumers.to_string(),
        ];
        if show_production {
            cells.push(row.living_millers.to_string());
            cells.push(row.living_bakers.to_string());
            cells.push(row.living_unassigned.to_string());
        }
        if show_research {
            cells.push(row.knowledge.to_string());
            cells.push(row.knowledge_produced.to_string());
            cells.push(row.tier.to_string());
        }
        if show_emergence {
            cells.push(row.phase.clone());
            cells.push(row.saleability_leader.clone());
            let money = if row.promoted_this_tick {
                format!("{}*", row.money_good)
            } else {
                row.money_good.clone()
            };
            cells.push(money);
        }
        if show_era {
            cells.push(row.era.clone());
        }
        if show_cycle {
            cells.push(row.regime.clone());
            cells.push(match row.gap_bps {
                Some(gap) => gap.to_string(),
                None => "-".to_string(),
            });
        }
        cells.push(consv);
        cells.push(hunger_max);
        cells.push(mean_one_decimal(row.hunger_sum, row.living));
        if show_demography {
            cells.push(row.living.to_string());
            cells.push(row.births_total.to_string());
            cells.push(row.old_age_deaths_total.to_string());
            for h in 0..household_count {
                cells.push(row.lineage_living.get(h).copied().unwrap_or(0).to_string());
                cells.push(row.lineage_gold.get(h).copied().unwrap_or(0).to_string());
            }
        }
        for good_index in 0..goods.len() {
            cells.push(price_cell(row.prices[good_index]));
            cells.push(row.transferred[good_index].to_string());
            if show_production {
                cells.push(row.produced[good_index].to_string());
                cells.push(row.consumed_as_input[good_index].to_string());
            }
            cells.push(row.consumed[good_index].to_string());
        }
        table_rows.push(cells);
    }

    let header_refs: Vec<&str> = headers.iter().map(String::as_str).collect();
    out.push_str(&render_table(&header_refs, &table_rows, &aligns));
    out
}

/// One captured row of the `region` dashboard (G2c) — the per-econ-tick snapshot
/// of the two settlements' realized FOOD price, their convergence gap, the
/// region-wide conservation flag, and the in-transit caravan escrow.
pub struct RegionDashboardRow {
    pub econ_tick: u64,
    /// Realized price of the traded good at settlement A / B, `None` if uncleared.
    pub price_a: Option<Gold>,
    pub price_b: Option<Gold>,
    /// `|price_A − price_B|`, `None` until both have cleared — the convergence
    /// observable.
    pub gap: Option<u64>,
    pub conserves: bool,
    /// Units of the traded good / gold in route escrow (mid-transit) this tick.
    pub escrow_good: u64,
    pub escrow_gold: u64,
    /// G7: the route's CURRENT transit cost (econ ticks per leg) — drops when the
    /// road completes, the mechanism behind the faster convergence.
    pub transit_ticks: u32,
    /// G7: the road's build progress this tick — `"—"` for a no-road region,
    /// `"<labor>/<cost>"` while building, or `"built@<tick>"` once complete.
    pub road: String,
}

/// Render the `region` dashboard (G2c): a header block then one table row per econ
/// tick — the realized price of the traded good at settlement A and B, their gap
/// (the convergence observable), the region-wide conservation flag, and the
/// in-transit caravan escrow. Read-only, deterministic, std formatting only.
pub fn format_region_dashboard(
    scenario: &str,
    seed: u64,
    ticks: u64,
    good_label: &str,
    caravans_enabled: bool,
    rows: &[RegionDashboardRow],
) -> String {
    let mut out = String::new();
    let has_road = rows.iter().any(|row| row.road != "—");
    let mode = if !caravans_enabled {
        "no-caravan control (the falsification twin)"
    } else if has_road {
        "caravan + road public works (community labor cuts the route transit)"
    } else {
        "caravan active (buys cheap at A, sells at B)"
    };
    let _ = writeln!(
        out,
        "oikos run — region {scenario:?}: two settlements, one route — {mode}"
    );
    let _ = writeln!(
        out,
        "seed {seed} · {ticks} econ ticks · traded good {good_label:?} · A = cheap/near, B = dear/far"
    );
    if has_road {
        let _ = writeln!(
            out,
            "the road is built from contributed labor; once built the route transit drops, so caravans cycle faster and the {good_label} gap |A−B| converges faster than the no-road control (sign only)"
        );
    } else {
        let _ = writeln!(
            out,
            "the {good_label} price gap |A−B| narrows over time only with the caravan (sign only)"
        );
    }
    out.push('\n');

    // The road columns are shown only for a region that actually has a road (the G7
    // `roads` scenario); the G2c `region`/`region-control` dashboards omit them, so
    // their output is unchanged. A region "has a road" iff any captured row reports a
    // non-`—` road cell (`has_road`, computed once above).
    let mut headers = vec![
        "tick".to_string(),
        format!("{good_label}@A"),
        format!("{good_label}@B"),
        "gap|A-B|".to_string(),
        "consv".to_string(),
        format!("esc.{good_label}"),
        "esc.gold".to_string(),
    ];
    let mut aligns = vec![
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Left,
        Align::Right,
        Align::Right,
    ];
    if has_road {
        headers.push("transit".to_string());
        headers.push("road".to_string());
        aligns.push(Align::Right);
        aligns.push(Align::Left);
    }
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            let gap = match row.gap {
                Some(g) => g.to_string(),
                None => "—".to_string(),
            };
            let mut cells = vec![
                row.econ_tick.to_string(),
                price_cell(row.price_a),
                price_cell(row.price_b),
                gap,
                if row.conserves { "OK" } else { "VIOLATED" }.to_string(),
                row.escrow_good.to_string(),
                row.escrow_gold.to_string(),
            ];
            if has_road {
                cells.push(row.transit_ticks.to_string());
                cells.push(row.road.clone());
            }
            cells
        })
        .collect();

    let header_refs: Vec<&str> = headers.iter().map(String::as_str).collect();
    out.push_str(&render_table(&header_refs, &table_rows, &aligns));
    out
}

/// Render the **price → trades** inspector: the realized price for `good` and
/// exactly the trades in `society().trades` for that good at `at_tick`.
pub fn format_price(
    settlement: &Settlement,
    scenario: &str,
    seed: u64,
    good: GoodId,
    good_label: &str,
    at_tick: u64,
    ticks: u64,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "oikos inspect price");
    let _ = writeln!(
        out,
        "scenario {scenario:?} · seed {seed} · good {good_label:?} (id {}) · at-tick {at_tick} · {ticks} econ ticks",
        good.0
    );
    out.push('\n');

    // Exactly the trades on the tape for this good at this tick, in tape order.
    // Computed first so the realized-price line can be made coherent with them:
    // `realized_price` is the *most recent* clearing price and is carried forward
    // across quiet ticks, so on a tick that itself cleared no trade it would
    // otherwise read as "the price behind these (zero) trades" — the price→trades
    // contradiction the inspector exists to avoid.
    let trades: Vec<_> = settlement
        .society()
        .trades
        .iter()
        .filter(|t| t.good == good && t.tick == at_tick)
        .collect();

    let realized = settlement.realized_price(good);
    match (realized, trades.is_empty()) {
        (Some(price), false) => {
            // A trade cleared at the inspected tick: the realized price is the
            // price behind the trades listed below.
            let _ = writeln!(out, "realized price ({good_label}): {}", price.0);
        }
        (Some(price), true) => {
            // No trade cleared at the inspected tick: the realized price is
            // carried over from the most recent earlier tick that did clear. Say
            // so plainly so it is never read as the price behind these (zero)
            // trades.
            match last_cleared_tick(settlement, good, at_tick) {
                Some(src) => {
                    let _ = writeln!(
                        out,
                        "realized price ({good_label}): {} (carried from tick {src}; no {good_label} trade cleared at tick {at_tick})",
                        price.0
                    );
                }
                None => {
                    let _ = writeln!(
                        out,
                        "realized price ({good_label}): {} (carried from an earlier tick; no {good_label} trade cleared at tick {at_tick})",
                        price.0
                    );
                }
            }
        }
        (None, _) => {
            let _ = writeln!(
                out,
                "realized price ({good_label}): — (no {good_label} trade has cleared yet)"
            );
        }
    }

    let _ = writeln!(
        out,
        "trades in {good_label} at tick {at_tick}: {}",
        trades.len()
    );
    out.push('\n');

    if trades.is_empty() {
        out.push_str("(no trades in this good cleared at this tick)\n");
        return out;
    }

    let headers = ["tick", "good", "buyer", "seller", "price", "qty"];
    let aligns = [
        Align::Right,
        Align::Left,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
    ];
    let rows: Vec<Vec<String>> = trades
        .iter()
        .map(|t| {
            vec![
                t.tick.to_string(),
                settlement.society().good_name(t.good).to_string(),
                t.buyer.to_string(),
                t.seller.to_string(),
                t.price.0.to_string(),
                t.qty.to_string(),
            ]
        })
        .collect();
    out.push_str(&render_table(&headers, &rows, &aligns));
    out
}

/// Render the **colonist → scale / why** inspector: the colonist's ranked value
/// scale, needs, vocation, alive/dead, carried inventory, and gold.
pub fn format_colonist(
    settlement: &Settlement,
    scenario: &str,
    seed: u64,
    index: usize,
    at_tick: u64,
    ticks: u64,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "oikos inspect colonist");
    let _ = writeln!(
        out,
        "scenario {scenario:?} · seed {seed} · id {index} · at-tick {at_tick} · {ticks} econ ticks"
    );
    out.push('\n');

    let id = settlement
        .colonist_id(index)
        .expect("the id was validated against the population before rendering");
    let vocation = match settlement.vocation_of(index) {
        Some(sim::Vocation::Gatherer) => "gatherer",
        Some(sim::Vocation::Consumer) => "consumer",
        Some(sim::Vocation::Miller) => "miller",
        Some(sim::Vocation::Baker) => "baker",
        // G3b: a latent producer that has not (yet) adopted from the spread.
        Some(sim::Vocation::Unassigned) => "unassigned",
        // G6b: a scholar (research → Knowledge) / a confectioner (tier-2 producer).
        Some(sim::Vocation::Scholar) => "scholar",
        Some(sim::Vocation::Confectioner) => "confectioner",
        Some(sim::Vocation::CycleA) => "cycle-A",
        Some(sim::Vocation::CycleB) => "cycle-B",
        Some(sim::Vocation::CycleC) => "cycle-C",
        None => "unknown",
    };
    let alive = settlement.is_alive(index);
    let liveness = if alive { "ALIVE" } else { "DEAD" };
    let _ = writeln!(out, "colonist {index} (agent {id}): {vocation}, {liveness}");

    if let Some(need) = settlement.need_of(index) {
        let _ = writeln!(
            out,
            "needs: hunger {}, warmth {}, rest {}",
            need.hunger, need.warmth, need.rest
        );
    }

    let goods = settlement.tracked_goods();
    let carry: Vec<String> = goods
        .iter()
        .map(|&g| {
            format!(
                "{} {}",
                settlement.society().good_name(g),
                settlement.carry_of(index, g)
            )
        })
        .collect();
    let _ = writeln!(out, "carry (world delivery escrow): {}", carry.join(", "));

    let agent = settlement.society().agents.get(id);
    if let Some(agent) = agent {
        let _ = writeln!(out, "gold: {}", agent.gold.0);
    }
    out.push('\n');

    let scale = agent.map(|a| a.scale.as_slice()).unwrap_or(&[]);
    if scale.is_empty() {
        if alive {
            out.push_str("value scale: (empty)\n");
        } else {
            match settlement.estate_destination_of(index) {
                Some(EstateDestination::Household { household, heir }) => {
                    let _ = writeln!(
                        out,
                        "value scale: (none — colonist has died; estate settled to household {household} heirs via agent {heir})"
                    );
                }
                Some(EstateDestination::Commons) => {
                    out.push_str(
                        "value scale: (none — colonist has died; estate settled to the commons)\n",
                    );
                }
                None => {
                    out.push_str(
                        "value scale: (none — colonist has died; estate destination unavailable)\n",
                    );
                }
            }
        }
        return out;
    }

    let _ = writeln!(
        out,
        "value scale (ranked, most urgent first): {} wants",
        scale.len()
    );
    out.push('\n');

    let headers = ["rank", "want", "horizon", "qty", "satisfied"];
    let aligns = [
        Align::Right,
        Align::Left,
        Align::Left,
        Align::Right,
        Align::Left,
    ];
    let rows: Vec<Vec<String>> = scale
        .iter()
        .enumerate()
        .map(|(rank, want)| {
            vec![
                (rank + 1).to_string(),
                want_kind_label(settlement, want.kind),
                horizon_label(want.horizon),
                want.qty.to_string(),
                if want.satisfied { "yes" } else { "no" }.to_string(),
            ]
        })
        .collect();
    out.push_str(&render_table(&headers, &rows, &aligns));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_one_decimal_is_integer_fixed_point() {
        assert_eq!(mean_one_decimal(12, 4), "3.0");
        assert_eq!(mean_one_decimal(10, 4), "2.5");
        assert_eq!(mean_one_decimal(7, 3), "2.3"); // 70/3 = 23 tenths (floored)
        assert_eq!(mean_one_decimal(0, 4), "0.0");
        assert_eq!(mean_one_decimal(5, 0), "-"); // no living colonist
    }

    #[test]
    fn format_reserve_ratio_keeps_fractional_basis_points() {
        assert_eq!(format_reserve_ratio(2_000), "20%");
        assert_eq!(format_reserve_ratio(10_000), "100%");
        assert_eq!(format_reserve_ratio(1_550), "15.5%");
        assert_eq!(format_reserve_ratio(1_505), "15.05%");
    }

    #[test]
    fn dashboard_renders_m3_money_and_bank_banners() {
        let settlement = Settlement::generate(1, &sim::SettlementConfig::viable());
        let money = MoneySummary {
            specie: 48,
            fiat: 0,
            claims: 240,
            reserves: 48,
            fiduciary: 192,
        };
        let bank = BankSummary {
            name: "settlement bank".to_string(),
            reserves: 48,
            demand_deposits: 240,
            fiduciary_issued: 192,
            reserve_ratio_bps: 2_000,
        };
        let banners = DashboardBanners {
            money: Some(&money),
            bank: Some(&bank),
            ..DashboardBanners::default()
        };

        let rendered = format_dashboard(&settlement, "bank", 1, 40, "0 living", &banners, &[]);

        assert!(rendered.contains(
            "money: M3 ledger — specie 48 · fiat 0 · claims 240 · reserves 48 · fiduciary 192"
        ));
        assert!(rendered.contains(
            "bank: settlement bank — reserves 48 · deposits 240 · fiduciary issued 192 · reserve ratio 20%"
        ));
    }

    #[test]
    fn price_cell_uses_em_dash_for_no_trade() {
        assert_eq!(price_cell(Some(Gold(7))), "7");
        assert_eq!(price_cell(None), "—");
    }

    #[test]
    fn horizon_label_covers_every_variant() {
        assert_eq!(horizon_label(Horizon::Now), "Now");
        assert_eq!(horizon_label(Horizon::Next), "Next");
        assert_eq!(horizon_label(Horizon::Later(4)), "Later(4)");
    }

    #[test]
    fn render_table_aligns_columns_and_trims_trailing_space() {
        let headers = ["n", "label"];
        let rows = vec![
            vec!["1".to_string(), "a".to_string()],
            vec!["100".to_string(), "bb".to_string()],
        ];
        let aligns = [Align::Right, Align::Left];
        let table = render_table(&headers, &rows, &aligns);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines[0], "  n | label");
        assert_eq!(lines[1], "----+------");
        assert_eq!(lines[2], "  1 | a"); // left-aligned cell's trailing pad trimmed
        assert_eq!(lines[3], "100 | bb");
        // No line carries trailing whitespace.
        assert!(table.lines().all(|line| line == line.trim_end()));
    }
}
