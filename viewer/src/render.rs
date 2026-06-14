//! The deterministic text renderers — the dashboard and the two inspectors.
//!
//! Every renderer returns a `String` (never writes stdout) so it is unit-
//! testable, and draws **no** randomness: it reads only from `sim`'s read-only
//! accessors over a settlement that was advanced by a seeded run, so the same
//! `(scenario, ticks, seed)` yields byte-identical output. Formatting is std
//! only — column-aligned plain text, no TUI / color / graphics (that is G9).

use std::fmt::Write as _;

use sim::{Gold, GoodId, Horizon, Settlement, WantKind};

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
    /// Realized price per tracked good (good order), `None` if none cleared yet.
    pub prices: Vec<Option<Gold>>,
    /// Units transferred world→econ this tick, per tracked good.
    pub transferred: Vec<u64>,
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
    gatherers: u16,
    consumers: u16,
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
        "seed {seed} · {ticks} econ ticks · population {} ({gatherers} gatherers, {consumers} consumers)",
        u64::from(gatherers) + u64::from(consumers)
    );
    let _ = writeln!(out, "tracked goods: {}", good_names.join(", "));
    out.push('\n');

    // Headers: the per-tick fixed columns, then a price/xfer/eaten triple per
    // tracked good.
    let mut headers: Vec<String> = vec![
        "tick".to_string(),
        "gath".to_string(),
        "cons".to_string(),
        "consv".to_string(),
        "hung.max".to_string(),
        "hung.mean".to_string(),
    ];
    let mut aligns: Vec<Align> = vec![
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Left,
        Align::Right,
        Align::Right,
    ];
    for name in &good_names {
        headers.push(format!("{name}.px"));
        headers.push(format!("{name}.xfer"));
        headers.push(format!("{name}.eaten"));
        aligns.push(Align::Right);
        aligns.push(Align::Right);
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
            consv,
            hunger_max,
            mean_one_decimal(row.hunger_sum, row.living),
        ];
        for good_index in 0..goods.len() {
            cells.push(price_cell(row.prices[good_index]));
            cells.push(row.transferred[good_index].to_string());
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
    let mode = if caravans_enabled {
        "caravan active (buys cheap at A, sells at B)"
    } else {
        "no-caravan control (the falsification twin)"
    };
    let _ = writeln!(
        out,
        "oikos run — region {scenario:?}: two settlements, one route — {mode}"
    );
    let _ = writeln!(
        out,
        "seed {seed} · {ticks} econ ticks · traded good {good_label:?} · A = cheap/near, B = dear/far"
    );
    let _ = writeln!(
        out,
        "the {good_label} price gap |A−B| narrows over time only with the caravan (sign only)"
    );
    out.push('\n');

    let headers = [
        "tick".to_string(),
        format!("{good_label}@A"),
        format!("{good_label}@B"),
        "gap|A-B|".to_string(),
        "consv".to_string(),
        format!("esc.{good_label}"),
        "esc.gold".to_string(),
    ];
    let aligns = [
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Right,
        Align::Left,
        Align::Right,
        Align::Right,
    ];
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            let gap = match row.gap {
                Some(g) => g.to_string(),
                None => "—".to_string(),
            };
            vec![
                row.econ_tick.to_string(),
                price_cell(row.price_a),
                price_cell(row.price_b),
                gap,
                if row.conserves { "OK" } else { "VIOLATED" }.to_string(),
                row.escrow_good.to_string(),
                row.escrow_gold.to_string(),
            ]
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
            out.push_str("value scale: (empty — colonist is tombstoned)\n");
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
