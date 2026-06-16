//! The scenario registry: a name → [`SettlementConfig`] lookup built from the
//! existing `sim` constructors. Read-only — it authors no new economics, only
//! selects among the configs `sim` already exposes (plus the `near`/`far`
//! distance variants the distance→price contrast needs, built with
//! [`SettlementConfig::with_food_node_distance`]).
//!
//! Resolution is a linear scan over a static table, not a `HashMap` (the lab
//! discipline: no `HashMap` in logic), and is order-stable so `oikos scenarios`
//! prints deterministically.

use sim::SettlementConfig;

/// The FOOD-node distance the `near` scenario uses (tiles east of the exchange).
/// Matches the G2b acceptance suite's near probe.
pub const NEAR_DISTANCE: u16 = 8;
/// The FOOD-node distance the `far` scenario uses. Matches the G2b far probe;
/// far enough that the round trip eats the fast-tick budget and fewer FOOD units
/// reach the market per econ tick, so the realized price is strictly higher.
pub const FAR_DISTANCE: u16 = 24;

/// One registered scenario: its canonical name, a one-line description, and the
/// builder that produces its config.
struct Scenario {
    name: &'static str,
    description: &'static str,
    build: fn() -> SettlementConfig,
}

/// The canonical scenarios, in display order. `near-node`/`far-node` are
/// accepted as aliases of `near`/`far` (handled in [`config_for`]).
const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "viable",
        description: "a viable single-FOOD-node settlement (8 gatherers, 4 consumers)",
        build: SettlementConfig::viable,
    },
    Scenario {
        name: "price-probe",
        description: "the distance→price probe (hunger-resilient consumers, larger gold)",
        build: SettlementConfig::price_probe,
    },
    Scenario {
        name: "near",
        description: "price-probe with the FOOD node NEAR the exchange (distance 8)",
        build: build_near,
    },
    Scenario {
        name: "far",
        description: "price-probe with the FOOD node FAR from the exchange (distance 24)",
        build: build_far,
    },
    Scenario {
        name: "starved-hauler",
        description: "a single gatherer on a haul too long to survive (escrow-on-death)",
        build: SettlementConfig::starved_hauler,
    },
    Scenario {
        name: "chain",
        description: "the G3a grain→flour→bread production chain (seeded millers + bakers)",
        build: SettlementConfig::grain_flour_bread_chain,
    },
    Scenario {
        name: "emergent-chain",
        description: "G3b: roles emerge — latent millers/bakers adopt from the price spread",
        build: SettlementConfig::emergent_chain,
    },
    Scenario {
        name: "emergent-chain-control",
        description: "G3b no-spread control: bread demand removed, so no roles form",
        build: SettlementConfig::emergent_chain_control,
    },
    Scenario {
        name: "lineages",
        description:
            "G4b demography: two households age, reproduce, inherit — the patient lineage out-saves",
        build: SettlementConfig::lineages,
    },
    Scenario {
        name: "barter-camp",
        description:
            "G5a: money emerges — a barter camp where SALT is promoted from spatial barter, then trade is money-priced",
        build: SettlementConfig::barter_camp,
    },
    Scenario {
        name: "barter-camp-control",
        description:
            "G5a no-surplus control: no saleability differential, so the camp stays in barter (nothing monetizes)",
        build: SettlementConfig::barter_camp_control,
    },
    Scenario {
        name: "frontier",
        description:
            "G5b: emergence composed — money emerges, then producer roles adopt from the spread, while births/deaths run (one settlement)",
        build: SettlementConfig::frontier,
    },
    Scenario {
        name: "no-consumers",
        description:
            "EXPERIMENT: frontier with the pure-consumer class removed — money held by producing gatherers, not idle consumers (does de-segregating money fix the deadlock?)",
        build: build_no_consumers,
    },
    Scenario {
        name: "millisats-1x",
        description:
            "EXPERIMENT: frontier at coarse (x1) money precision — the unit-starved baseline that freezes (~320 money units)",
        build: build_millisats_1x,
    },
    Scenario {
        name: "millisats",
        description:
            "EXPERIMENT: frontier redenominated into a x1000-finer money unit (Lightning millisats) — same economy, ~320000 units; does circulation survive?",
        build: build_millisats,
    },
    Scenario {
        name: "progress-probe-1x",
        description:
            "EXPERIMENT: frontier with a constant-generous demographic headroom, productive bundle x1 (the carrying-capacity baseline)",
        build: build_progress_probe_1x,
    },
    Scenario {
        name: "progress-probe-2x",
        description:
            "EXPERIMENT: the same colony with the productive bundle (food supply, labor, throughput) x2 — does the equilibrium scale?",
        build: build_progress_probe_2x,
    },
    Scenario {
        name: "progress-probe-4x",
        description:
            "EXPERIMENT: the same colony with the productive bundle x4 — carrying-capacity-bound (scales) or hard-capped (saturates)?",
        build: build_progress_probe_4x,
    },
    Scenario {
        name: "research",
        description:
            "G6b: research & tech tiers — scholars accumulate Knowledge, which unlocks the gated tier-2 recipe (pastry)",
        build: SettlementConfig::research,
    },
    Scenario {
        name: "research-control",
        description:
            "G6b no-scholars control: no Knowledge accrues, so tier 2 never unlocks and pastry is never produced",
        build: SettlementConfig::research_control,
    },
    Scenario {
        name: "m3-settlement",
        description:
            "G8a: the viable settlement run on M3 ledger money (specie — no banks, no fiat); money is ledger-accounted, economically M1",
        build: SettlementConfig::m3_settlement,
    },
    Scenario {
        name: "bank",
        description:
            "G8b: a chartered fractional-reserve bank — deposits become claims, the bank lends fiduciary credit beyond its reserves",
        build: SettlementConfig::bank,
    },
    Scenario {
        name: "bank-full-reserve",
        description:
            "G8b 100%-reserve control: the same bank at a full reserve ratio lends ZERO fiduciary, while deposits still circulate as claims",
        build: SettlementConfig::bank_full_reserve,
    },
    Scenario {
        name: "credit-cycle",
        description:
            "G8c-1: the Austrian business cycle — the regime descends to Fiat, cheap credit opens a shadow gap, the boom over-invests, credit stops, the bust abandons the malinvestment and consumes capital",
        build: SettlementConfig::credit_cycle,
    },
    Scenario {
        name: "sound-money",
        description:
            "G8c-1 sound-money control: SoundGold, no fiat, no credit expansion — gap ≈ 0, no boom, no bust, no capital consumed (the cycle is credit-driven)",
        build: SettlementConfig::sound_money,
    },
    Scenario {
        name: "wage-tender-cycle",
        description:
            "G8c-2 headline: the credit cycle with fiat wages as legal tender — fiat credit reaches workers, so the boom→bust transmits (the cycle fires)",
        build: SettlementConfig::wage_tender_cycle,
    },
    Scenario {
        name: "wage-refusal-cycle",
        description:
            "G8c-2 control: the same credit cycle with specie-only wages — the fiat credit cannot pay wages, so it never transmits (inert: no boom, no bust)",
        build: SettlementConfig::wage_refusal_cycle,
    },
    Scenario {
        name: "tax-in-fiat",
        description:
            "G8c-3 headline: the specie-only-wage cycle with a fiat-receivable state tax — the state compels fiat through the fiscal channel where the labor market refused it (tax receipts in fiat)",
        build: SettlementConfig::tax_in_fiat,
    },
    Scenario {
        name: "tax-in-specie",
        description:
            "G8c-3 control: the same settlement with a specie-receivable tax — no compelled fiat demand (tax receipts in specie, none in fiat); only the receivability differs",
        build: SettlementConfig::tax_in_specie,
    },
    Scenario {
        name: "spot-tender-legal",
        description:
            "G8c-2 spot bench (M11): fiat is legal tender on the spot market, so the printed fiat settles goods trades (composition flips, totals do not)",
        build: build_spot_tender_legal,
    },
    Scenario {
        name: "spot-tender-refusal",
        description:
            "G8c-2 spot bench control: specie-only spot tender refuses the held fiat, so specie settles the same trades (broad money unchanged)",
        build: build_spot_tender_refusal,
    },
    Scenario {
        name: "debt-tender-legal",
        description:
            "G8c-2 debt bench (M12): fiat is legal tender for public debt, so the seeded debt is discharged in fiat (composition flips, totals do not)",
        build: build_debt_tender_legal,
    },
    Scenario {
        name: "debt-tender-refusal",
        description:
            "G8c-2 debt bench control: specie-only debt tender refuses fiat, so the same debt is discharged in specie (broad money unchanged)",
        build: build_debt_tender_refusal,
    },
    Scenario {
        name: "bank-repayment-tender-legal",
        description:
            "G8c-2 bank-repayment bench (M15): bank claims are legal tender for bank-loan repayment, so the claim settles and retires credit",
        build: build_bank_repayment_tender_legal,
    },
    Scenario {
        name: "bank-repayment-tender-refusal",
        description:
            "G8c-2 bank-repayment bench control: specie-only bank repayment refuses the held claim, so the repayment defaults",
        build: build_bank_repayment_tender_refusal,
    },
    Scenario {
        name: "issuer-repayment-tender-legal",
        description:
            "G8c-2 issuer-repayment bench (M16): fiat is accepted for issuer-credit repayment, so the returned fiat retires credit",
        build: build_issuer_repayment_tender_legal,
    },
    Scenario {
        name: "issuer-repayment-tender-refusal",
        description:
            "G8c-2 issuer-repayment bench control: fiat-refused issuer repayment leaves the held fiat unable to retire the credit",
        build: build_issuer_repayment_tender_refusal,
    },
];

fn build_no_consumers() -> SettlementConfig {
    SettlementConfig::frontier_no_consumers()
}

fn build_millisats_1x() -> SettlementConfig {
    SettlementConfig::frontier_millisats(1)
}

fn build_millisats() -> SettlementConfig {
    SettlementConfig::frontier_millisats(1_000)
}

fn build_progress_probe_1x() -> SettlementConfig {
    SettlementConfig::frontier_probe(1)
}

fn build_progress_probe_2x() -> SettlementConfig {
    SettlementConfig::frontier_probe(2)
}

fn build_progress_probe_4x() -> SettlementConfig {
    SettlementConfig::frontier_probe(4)
}

fn build_near() -> SettlementConfig {
    SettlementConfig::price_probe().with_food_node_distance(NEAR_DISTANCE)
}

fn build_far() -> SettlementConfig {
    SettlementConfig::price_probe().with_food_node_distance(FAR_DISTANCE)
}

fn build_spot_tender_legal() -> SettlementConfig {
    SettlementConfig::spot_tender_bench(sim::PublicSpotTender::FiatAndSpecie)
}

fn build_spot_tender_refusal() -> SettlementConfig {
    SettlementConfig::spot_tender_bench(sim::PublicSpotTender::SpecieOnly)
}

fn build_debt_tender_legal() -> SettlementConfig {
    SettlementConfig::debt_tender_bench(sim::PublicDebtTender::FiatAndSpecie)
}

fn build_debt_tender_refusal() -> SettlementConfig {
    SettlementConfig::debt_tender_bench(sim::PublicDebtTender::SpecieOnly)
}

fn build_bank_repayment_tender_legal() -> SettlementConfig {
    SettlementConfig::bank_repayment_tender_bench(sim::BankRepaymentTender::BankClaimsAndSpecie)
}

fn build_bank_repayment_tender_refusal() -> SettlementConfig {
    SettlementConfig::bank_repayment_tender_bench(sim::BankRepaymentTender::SpecieOnly)
}

fn build_issuer_repayment_tender_legal() -> SettlementConfig {
    SettlementConfig::issuer_repayment_tender_bench(sim::IssuerRepaymentTender::FiatOnly)
}

fn build_issuer_repayment_tender_refusal() -> SettlementConfig {
    SettlementConfig::issuer_repayment_tender_bench(sim::IssuerRepaymentTender::FiatRefused)
}

/// Resolve a scenario name (including the `near-node`/`far-node` aliases) to its
/// [`SettlementConfig`]. Returns a loud error listing the available scenarios
/// for an unknown name — never a silent default.
pub fn config_for(name: &str) -> Result<SettlementConfig, String> {
    let canonical = match name {
        "near-node" => "near",
        "far-node" => "far",
        other => other,
    };
    SCENARIOS
        .iter()
        .find(|s| s.name == canonical)
        .map(|s| (s.build)())
        .ok_or_else(|| {
            format!("unknown scenario: {name:?} (run `oikos scenarios` to list the available ones)")
        })
}

/// The one-line description for a (canonical or aliased) scenario name, or a
/// generic label if the name is unknown (the caller validates the name first via
/// [`config_for`], so the fallback is unreachable in practice).
pub fn description_for(name: &str) -> &'static str {
    let canonical = match name {
        "near-node" => "near",
        "far-node" => "far",
        other => other,
    };
    SCENARIOS
        .iter()
        .find(|s| s.name == canonical)
        .map(|s| s.description)
        .unwrap_or("a settlement scenario")
}

/// The rendered `oikos scenarios` listing — every scenario with its description,
/// plus the alias note. Deterministic (the table order is the static table's).
pub fn scenarios_text() -> String {
    let name_width = SCENARIOS
        .iter()
        .map(|s| s.name.len())
        .max()
        .unwrap_or(0)
        .max("scenario".len());

    let name_width = name_width.max("region-control".len());
    let mut out = String::new();
    out.push_str("Available scenarios:\n\n");
    for scenario in SCENARIOS {
        out.push_str(&format!(
            "    {:<width$}  {}\n",
            scenario.name,
            scenario.description,
            width = name_width
        ));
    }
    // The G2c multi-settlement scenarios — for the `run` dashboard only (they
    // advance a two-settlement Region, not a single Settlement, so the price /
    // colonist inspectors do not apply).
    out.push_str(&format!(
        "    {:<width$}  {}\n",
        "region",
        "two settlements + a caravan (run only): the FOOD-price gap narrows",
        width = name_width
    ));
    out.push_str(&format!(
        "    {:<width$}  {}\n",
        "region-control",
        "the no-caravan twin (run only): the gap is kept",
        width = name_width
    ));
    out.push_str(&format!(
        "    {:<width$}  {}\n",
        "roads",
        "G7: a road built from community labor cuts the route transit, so the gap converges faster (run only)",
        width = name_width
    ));
    out.push_str(&format!(
        "    {:<width$}  {}\n",
        "roads-control",
        "G7 no-road twin (run only): same caravan, no road — the gap converges slower",
        width = name_width
    ));
    out.push_str("\nAliases: near-node = near, far-node = far\n");
    out
}
