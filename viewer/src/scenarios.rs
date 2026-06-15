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
];

fn build_near() -> SettlementConfig {
    SettlementConfig::price_probe().with_food_node_distance(NEAR_DISTANCE)
}

fn build_far() -> SettlementConfig {
    SettlementConfig::price_probe().with_food_node_distance(FAR_DISTANCE)
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
    out.push_str("\nAliases: near-node = near, far-node = far\n");
    out
}
