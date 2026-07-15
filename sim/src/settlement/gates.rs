//! Chain-runtime gate predicates.
//!
//! These `chain_runtime_*` free functions answer "is milestone/feature X active for
//! this `ChainRuntime`?" — the composition gates that keep every off-by-default
//! milestone byte-identical. Extracted verbatim from `mod.rs` (pure code motion); the
//! `pub(crate) use gates::*` re-export in the parent keeps all call sites unchanged.

use super::*;

pub(super) fn chain_runtime_own_labor_subsistence_can_run(chain: &ChainRuntime) -> bool {
    own_labor_subsistence_fields_active(
        chain.own_labor_subsistence,
        chain.content.forage().is_some(),
    )
}
pub(super) fn chain_runtime_forage_commons_active(chain: &ChainRuntime) -> bool {
    forage_commons_fields_active(
        chain.own_labor_subsistence,
        chain.content.forage().is_some(),
        chain.forage_commons.is_some(),
    )
}
/// S15: own-use cultivation is active iff the flag is on, the `Cultivate` recipe is
/// present, and the own-labor/forage path it composes on can run (so the foraging
/// eligibility + FORAGE node exist). Off (every existing config) it is `false`, so the
/// cultivation steering/phase/digest surface never engages and the run is byte-identical.
pub(super) fn chain_runtime_own_use_cultivation_active(chain: &ChainRuntime) -> bool {
    chain.own_use_cultivation
        && chain.content.cultivate_recipe().is_some()
        && (chain_runtime_own_labor_subsistence_can_run(chain)
            || chain.household_barter_cultivation)
}
/// S21f: the household-barter cultivation seam is active this tick iff the flag is on AND
/// the S15 own-use cultivation knobs (the gate + the `Cultivate` recipe) are present — the
/// alternative substrate to the own-labor/forage path. Off (every existing config) it is
/// `false`, so cultivation still requires the forage substrate and the run is
/// byte-identical.
pub(super) fn chain_runtime_household_barter_cultivation_active(chain: &ChainRuntime) -> bool {
    chain.household_barter_cultivation
        && chain.own_use_cultivation
        && chain.content.cultivate_recipe().is_some()
}
/// S22a: endogenous cultivation entry is active iff the flag is on AND the
/// money-from-produced-bread path is active (it composes strictly on the S16 path whose
/// buy/sell eligibility branch it overrides). Off (every existing config) it is `false`, so
/// the eligibility relaxation never engages and the run is byte-identical.
pub(super) fn chain_runtime_endogenous_cultivation_entry_active(chain: &ChainRuntime) -> bool {
    chain.endogenous_cultivation_entry && chain_runtime_cultivation_sells_surplus_active(chain)
}
/// S23a: private land tenure is active iff the gate is on AND S22a endogenous cultivation
/// entry is active. A manually toggled flag on an older substrate is inert and therefore
/// preserves the older run byte-for-byte.
pub(super) fn chain_runtime_private_land_tenure_active(chain: &ChainRuntime) -> bool {
    (chain.private_land_tenure || chain.secure_land_tenure)
        && chain_runtime_endogenous_cultivation_entry_active(chain)
}
pub(super) fn chain_runtime_secure_land_tenure_active(chain: &ChainRuntime) -> bool {
    chain.secure_land_tenure && chain_runtime_endogenous_cultivation_entry_active(chain)
}
pub(super) fn chain_runtime_land_market_active(chain: &ChainRuntime) -> bool {
    chain.land_market && chain_runtime_private_land_tenure_active(chain)
}
pub(super) fn chain_runtime_mortal_landowner_demography_active(chain: &ChainRuntime) -> bool {
    chain.mortal_landowner_demography
        && chain_runtime_secure_land_tenure_active(chain)
        && !chain_runtime_land_market_active(chain)
}
pub(super) fn chain_runtime_mortal_chain_producers_active(chain: &ChainRuntime) -> bool {
    chain.mortal_chain_producers
}
pub(super) fn chain_runtime_mortal_producer_inheritance_active(chain: &ChainRuntime) -> bool {
    chain.mortal_producer_inheritance && chain_runtime_mortal_chain_producers_active(chain)
}
pub(super) fn chain_runtime_earned_provisioning_active(chain: &ChainRuntime) -> bool {
    chain.earned_provisioning && chain_runtime_mortal_producer_inheritance_active(chain)
}
pub(super) fn chain_runtime_birth_stock_saving_active(chain: &ChainRuntime) -> bool {
    chain.birth_stock_saving
        && chain.birth_stock_saving_mode == BirthStockSavingMode::Motive
        && chain_runtime_earned_provisioning_active(chain)
}
pub(super) fn chain_runtime_birth_stock_control_active(chain: &ChainRuntime) -> bool {
    !chain.birth_stock_saving
        && chain.birth_stock_saving_mode == BirthStockSavingMode::SufficiencyControl
        && chain_runtime_earned_provisioning_active(chain)
}
pub(super) fn chain_runtime_saving_allocation_obs_active(chain: &ChainRuntime) -> bool {
    // The motive (mode 1) must be on to observe its bids.
    chain.saving_allocation_obs && chain_runtime_birth_stock_saving_active(chain)
}
pub(super) fn chain_runtime_producer_stock_provisioning_control_active(
    chain: &ChainRuntime,
) -> bool {
    chain.producer_stock_provisioning_control
        && chain_runtime_mortal_producer_inheritance_active(chain)
}
pub(super) fn chain_runtime_rival_subsistence_commons_active(chain: &ChainRuntime) -> bool {
    chain.rival_subsistence_commons
        && chain_runtime_mortal_landowner_demography_active(chain)
        && chain.emergency_hunger_threshold > 0
}
pub(super) fn chain_runtime_wage_labor_active(chain: &ChainRuntime) -> bool {
    chain.wage_labor && chain_runtime_rival_subsistence_commons_active(chain)
}
pub(super) fn chain_runtime_share_tenancy_active(chain: &ChainRuntime) -> bool {
    chain.share_tenancy && chain_runtime_rival_subsistence_commons_active(chain)
}
pub(super) fn chain_runtime_in_kind_wage_active(chain: &ChainRuntime) -> bool {
    chain.in_kind_wage && chain_runtime_share_tenancy_active(chain)
}
pub(super) fn chain_runtime_share_forward_provisioning_active(chain: &ChainRuntime) -> bool {
    chain.share_forward_provisioning && chain_runtime_share_tenancy_active(chain)
}
pub(super) fn chain_runtime_share_contract_succession_active(chain: &ChainRuntime) -> bool {
    chain.share_contract_succession && chain_runtime_share_tenancy_active(chain)
}
/// S22b: bounded cultivation skill is active iff the flag is on AND the S22a
/// endogenous-cultivation-entry path is active (it composes strictly on it). Off (every existing
/// config) it is `false`, so the grain-haul lever, the skill accumulate/decay, and the ON-only
/// digest surface never engage and the run is byte-identical.
pub(super) fn chain_runtime_cultivation_skill_active(chain: &ChainRuntime) -> bool {
    chain.cultivation_skill && chain_runtime_endogenous_cultivation_entry_active(chain)
}
/// S22c: profit-driven cultivation retention is active iff the flag is on AND the S22a
/// endogenous-cultivation-entry path is active (it composes strictly on it; orthogonal to
/// `cultivation_skill`). Off (every existing config) it is `false`, so the per-agent return
/// window, the profit-stay exit modulation, and the ON-only digest surface never engage and the
/// run is byte-identical.
pub(super) fn chain_runtime_profit_driven_retention_active(chain: &ChainRuntime) -> bool {
    chain.profit_driven_retention && chain_runtime_endogenous_cultivation_entry_active(chain)
}
/// S22d: durable role-specific cultivation capital is active iff the flag is on, the S22c
/// profit-driven-retention path is active (it composes strictly on it — the durable advantage
/// works THROUGH the profit-stay exit, and S22c itself requires the S22a endogenous-entry path),
/// and the content set carries the cultivation-tool good. The content check keeps a manually
/// toggled flag without the plow content from changing canonical state while the feature itself
/// would no-op. Off (every existing config) it is `false`, so the cultivation-capital phase, the
/// owner-haul boost, the per-agent tenure counter, and the ON-only digest surface never engage and
/// the run is byte-identical.
pub(super) fn chain_runtime_durable_cultivation_tool_active(chain: &ChainRuntime) -> bool {
    chain.durable_cultivation_tool
        && chain_runtime_profit_driven_retention_active(chain)
        && chain.content.cultivation_tool().is_some()
}
/// S22f: the voluntary fixed-term cultivation commitment is active iff the flag is on AND the S22c
/// profit-driven-retention path is active (it composes strictly on it — the entry signal IS the S22c
/// realized return, and S22c itself requires the S22a endogenous-entry path). Off (every existing
/// config) it is `false`, so the commitment entry/binding/expiry seam and the ON-only digest surface
/// never engage and the run is byte-identical. (The post-money inertness — no commit can form before
/// SALT promotes — is enforced separately at the entry seam via `current_money_good()`.)
pub(super) fn chain_runtime_voluntary_cultivation_commitment_active(chain: &ChainRuntime) -> bool {
    chain.voluntary_cultivation_commitment && chain_runtime_profit_driven_retention_active(chain)
}
pub(super) fn chain_runtime_commitment_norm_spread_active(chain: &ChainRuntime) -> bool {
    chain.fixed_commitment_norm_prevalence.is_none()
        && chain.commitment_norm_spread
        && chain_runtime_voluntary_cultivation_commitment_active(chain)
}
pub(super) fn chain_runtime_abandonable_norm_active(chain: &ChainRuntime) -> bool {
    chain.abandonable_norm && chain_runtime_commitment_norm_spread_active(chain)
}
pub(super) fn chain_runtime_group_payoff_imitation_active(chain: &ChainRuntime) -> bool {
    chain.group_payoff_imitation && chain_runtime_abandonable_norm_active(chain)
}
/// S16: money-from-produced-bread is active iff the flag is on AND own-use cultivation is
/// active (it composes strictly on the S15 path). Off (every existing config) it is
/// `false`, so the buy/sell split and the provenance ledger never engage and the run is
/// byte-identical.
pub(super) fn chain_runtime_cultivation_sells_surplus_active(chain: &ChainRuntime) -> bool {
    chain.cultivation_sells_surplus && chain_runtime_own_use_cultivation_active(chain)
}
/// S18: money-from-a-multi-good-economy is active iff the flag is on AND the
/// money-from-produced-bread path is active (it composes strictly on the S16 path). Off
/// (every existing config) it is `false`, so the woodcutter routing and the runtime-only
/// instrumentation never engage and the run is byte-identical.
pub(super) fn chain_runtime_multigood_money_active(chain: &ChainRuntime) -> bool {
    chain.multigood_money && chain_runtime_cultivation_sells_surplus_active(chain)
}
