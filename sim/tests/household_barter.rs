//! S21f — **endogenous pre-money household production-for-barter**.
//!
//! S21e proved a finite *seeded* pre-promotion bread supply is sufficient for SALT to
//! monetize under market-financed survival. S21f makes that supply ENDOGENOUS: lineage
//! households *cultivate* bread by their own labor (no forage substrate), eat what they
//! need, and barter the surplus *before money exists* — so SALT emerges from barter over
//! genuinely produced (`SelfProduced`) surplus (the Mengerian / regression-theorem
//! bootstrap). The decisive new bar vs S21e: the pre-promotion bread sold for SALT is
//! `SelfProduced` (cultivated), NOT `SeededMinted` (seeded) — and with every bread buffer
//! zeroed, NO `SeededMinted` bread ever enters.
//!
//! This is recurring production from a real depleting grain commons (NOT a one-time seed),
//! so there is no "exhaustion" framing: the honesty guard is that produced bread is bounded
//! by real grain input and that money rests on `SelfProduced` supply (the grain-flow sweep
//! brackets the promotion window and shows produced bread tracks the grain consumed).

use econ::barter::BarterReason;
use econ::good::{GoodId, SALT, WOOD};
use sim::{Settlement, SettlementConfig};

const PROBE_TICKS: u64 = 1_600;
/// The headline SALT-mediated share bar (reused from S21e): of the bread/WOOD trade
/// volume, the share that flows THROUGH SALT must dominate the residual direct barter.
const HEADLINE_MIN_SALT_SHARE_BPS: u64 = 9_000;

fn bread_good(s: &Settlement) -> GoodId {
    s.bread_good()
        .expect("the household-barter chain carries a bread good")
}

fn grain_good(s: &Settlement) -> GoodId {
    s.content()
        .expect("the household-barter chain carries content")
        .grain()
}

/// The accumulated cold-start / non-vacuity signals collected over a full run, plus the
/// settlement at the end (for the post-hoc provenance/promotion accessors).
struct Run {
    s: Settlement,
    /// Some living lineage member was BOTH spatial (a world agent) AND `cultivating`.
    spatial_lineage_cultivator: bool,
    /// A `cultivating` lineage member held hauled grain in its own stock.
    cultivator_hauled_grain: bool,
    /// The first tick any living colonist was `cultivating` (None ⇒ cultivation never fired).
    first_cultivate_tick: Option<u64>,
    /// A produced-bread surplus was ever held above reserves (the offerable surplus).
    surplus_ever: bool,
    /// A produced surplus held at tick `t-1` was matched by a live OR freshly-cleared
    /// `bread → SALT IndirectFor{WOOD}` lane at tick `t` (the post-market→next-tick path).
    cross_tick_sale: bool,
    produced_bread: u64,
    consumed_grain: u64,
    /// The max bread minted in any single tick's endowment term (must stay 0 — no mint).
    bread_minted_max: u64,
    conserved_every_tick: bool,
}

fn lane_is_bread_salt_for_wood(
    give: GoodId,
    receive: GoodId,
    reason: BarterReason,
    bread: GoodId,
) -> bool {
    give == bread
        && receive == SALT
        && matches!(reason, BarterReason::IndirectFor { target } if target == WOOD)
}

fn run_full(seed: u64, cfg: &SettlementConfig, ticks: u64) -> Run {
    let mut s = Settlement::generate(seed, cfg);
    let bread = bread_good(&s);
    let grain = grain_good(&s);
    let mut spatial_lineage_cultivator = false;
    let mut cultivator_hauled_grain = false;
    let mut first_cultivate_tick = None;
    let mut surplus_ever = false;
    let mut cross_tick_sale = false;
    let mut produced_bread = 0u64;
    let mut consumed_grain = 0u64;
    let mut bread_minted_max = 0u64;
    let mut conserved_every_tick = true;
    let mut had_surplus_prev = false;
    for t in 0..ticks {
        let report = s.econ_tick();
        conserved_every_tick &= report.conserves();
        produced_bread += report.produced_of(bread);
        consumed_grain += report.consumed_as_input_of(grain);
        bread_minted_max = bread_minted_max.max(report.endowment_of(bread));
        // The cold-start non-vacuity chain: a spatial lineage member becomes `cultivating`
        // and hauls grain into its own stock.
        let mut cultivating_now = false;
        for i in 0..s.population() {
            if !s.is_alive(i) || !s.is_cultivating(i) {
                continue;
            }
            cultivating_now = true;
            if s.household_of(i).is_some() {
                if let Some(id) = s.colonist_id(i) {
                    if s.world().agent_pos(id).is_some() {
                        spatial_lineage_cultivator = true;
                    }
                }
                if s.stock_of(i, grain) > 0 {
                    cultivator_hauled_grain = true;
                }
            }
        }
        if cultivating_now && first_cultivate_tick.is_none() {
            first_cultivate_tick = Some(t);
        }
        // Cross-tick: a produced surplus held at the END of the previous tick, matched by a
        // live (or freshly cleared THIS tick) bread→SALT IndirectFor{WOOD} lane. The cultivate
        // phase is POST-market, so a fresh surplus can only be offered the NEXT tick.
        let lane_live = s
            .society()
            .live_barter_offers()
            .iter()
            .any(|o| lane_is_bread_salt_for_wood(o.give_good, o.receive_good, o.reason, bread));
        let lane_cleared_now = s.society().barter_trades.iter().any(|tr| {
            tr.tick == t
                && (lane_is_bread_salt_for_wood(tr.a_gives, tr.b_gives, tr.a_reason, bread)
                    || lane_is_bread_salt_for_wood(tr.b_gives, tr.a_gives, tr.b_reason, bread))
        });
        if had_surplus_prev && (lane_live || lane_cleared_now) {
            cross_tick_sale = true;
        }
        let surplus = s.produced_bread_held() > 0;
        surplus_ever |= surplus;
        had_surplus_prev = surplus;
    }
    Run {
        s,
        spatial_lineage_cultivator,
        cultivator_hauled_grain,
        first_cultivate_tick,
        surplus_ever,
        cross_tick_sale,
        produced_bread,
        consumed_grain,
        bread_minted_max,
        conserved_every_tick,
    }
}

fn run(seed: u64, cfg: &SettlementConfig) -> Run {
    run_full(seed, cfg, PROBE_TICKS)
}

/// The cumulative pre-promotion bread→SALT `IndirectFor{WOOD}` cleared volume (the medium
/// lane the bar requires). Reads the public barter tape directly.
fn pre_promotion_bread_salt_wood_cleared(s: &Settlement, bread: GoodId) -> u64 {
    let promo = s.promoted_at_tick().unwrap_or(u64::MAX);
    s.society()
        .barter_trades
        .iter()
        .filter(|tr| tr.tick < promo)
        .map(|tr| {
            if lane_is_bread_salt_for_wood(tr.a_gives, tr.b_gives, tr.a_reason, bread)
                || lane_is_bread_salt_for_wood(tr.b_gives, tr.a_gives, tr.b_reason, bread)
            {
                u64::from(tr.qty)
            } else {
                0
            }
        })
        .sum()
}

/// The SALT-mediated share of the bread+WOOD trade volume, in basis points: of the bread/WOOD
/// volume that moves THROUGH SALT (barter legs + post-promotion spot) vs the residual direct
/// bread↔WOOD barter.
fn salt_share_bps(s: &Settlement, bread: GoodId) -> u64 {
    let mediated_barter: u64 = s
        .society()
        .barter_trades
        .iter()
        .filter(|t| {
            (t.a_gives == SALT && (t.b_gives == bread || t.b_gives == WOOD))
                || (t.b_gives == SALT && (t.a_gives == bread || t.a_gives == WOOD))
        })
        .map(|t| u64::from(t.qty))
        .sum();
    let mediated_spot: u64 = if s.current_money_good() == Some(SALT) {
        s.society()
            .trades
            .iter()
            .filter(|t| t.good == bread || t.good == WOOD)
            .map(|t| u64::from(t.qty))
            .sum()
    } else {
        0
    };
    let mediated = mediated_barter + mediated_spot;
    let direct: u64 = s
        .society()
        .barter_trades
        .iter()
        .filter(|t| {
            (t.a_gives == bread && t.b_gives == WOOD) || (t.a_gives == WOOD && t.b_gives == bread)
        })
        .map(|t| u64::from(t.qty))
        .sum();
    mediated
        .saturating_mul(10_000)
        .checked_div(mediated + direct)
        .unwrap_or(0)
}

#[derive(Debug, PartialEq, Eq)]
enum ColdStart {
    /// Cultivation never started (no spatial cultivator / nobody `cultivating`): a bad probe
    /// / activation failure — fix the seam or buffers, NOT an economic finding.
    NeverStarts,
    /// Cultivation started but produced no offerable surplus (all bread eaten): a
    /// production-flow finding (grain flow / `cultivate_consume` too tight).
    NoSurplus,
    /// Surplus existed but no cleared `bread → SALT IndirectFor{WOOD}` lane: a barter/topology
    /// finding (the WOOD-target / role separation did not compose).
    NoClearedLane,
    /// The full chain through to ≥1 cleared pre-promotion `bread → SALT IndirectFor{WOOD}`
    /// lane whose bread is `SelfProduced` (and SALT promoted on it).
    Success,
}

fn classify(r: &Run) -> ColdStart {
    let bread = bread_good(&r.s);
    if !r.spatial_lineage_cultivator || r.first_cultivate_tick.is_none() {
        return ColdStart::NeverStarts;
    }
    if !r.surplus_ever {
        return ColdStart::NoSurplus;
    }
    if pre_promotion_bread_salt_wood_cleared(&r.s, bread) == 0 {
        return ColdStart::NoClearedLane;
    }
    ColdStart::Success
}

// ---- the determinism / golden contract ------------------------------------

#[test]
fn canonical_bytes_include_household_barter_cultivation() {
    // OFF (the open-survival base, no cultivation seam).
    let off = Settlement::generate(7, &SettlementConfig::frontier_open_survival());

    // An explicit `household_barter_cultivation = false` on the same base must keep the
    // default byte stream identical (the flag is canonicalized ON-only).
    let mut explicit_off_cfg = SettlementConfig::frontier_open_survival();
    explicit_off_cfg
        .chain
        .as_mut()
        .expect("chain")
        .household_barter_cultivation = false;
    let explicit_off = Settlement::generate(7, &explicit_off_cfg);

    // ON (the household-barter scenario) must SPLIT the digest (it changes production).
    let on = Settlement::generate(7, &SettlementConfig::frontier_household_barter());

    assert_eq!(
        off.canonical_bytes(),
        explicit_off.canonical_bytes(),
        "explicit default household_barter_cultivation=false must keep default bytes identical"
    );
    assert_ne!(
        off.canonical_bytes(),
        on.canonical_bytes(),
        "the household-barter cultivation seam must split the canonical digest when on"
    );
}

#[test]
fn goldens_unchanged() {
    // The S21f field + scenario are additive and the flag is canonicalized ON-only, so the
    // cross-history demographic + emergence goldens stay BYTE-IDENTICAL (the `lineages` and
    // `frontier` digests are the key tripwires; the values are the ones pinned in
    // tests/mortality.rs and tests/spatial_households.rs).
    let digest = |cfg: &SettlementConfig, ticks: u64| {
        let mut s = Settlement::generate(1, cfg);
        s.run(ticks);
        s.digest()
    };
    assert_eq!(
        digest(&SettlementConfig::lineages(), 300),
        0x2335e13c809749fc,
        "the `lineages` demographic golden must be byte-identical"
    );
    assert_eq!(
        digest(&SettlementConfig::frontier(), 300),
        0xcc83bf2669f0980d,
        "the S5-S13 frontier golden must be byte-identical"
    );
    assert_eq!(
        digest(&SettlementConfig::frontier_spatial_households(), 300),
        0xf30e3ce9345a73b3,
        "the S13 spatial-households golden must be byte-identical"
    );
    let mut viable = Settlement::generate(0xC0FFEE, &SettlementConfig::viable());
    viable.run(60);
    assert_eq!(
        viable.digest(),
        0xa174_8567_db1c_4341,
        "the g4a viable no-death digest must be byte-identical"
    );
}

#[test]
fn household_barter_run_is_deterministic() {
    let cfg = SettlementConfig::frontier_household_barter();
    let mut a = Settlement::generate(7, &cfg);
    let mut b = Settlement::generate(7, &cfg);
    a.run(PROBE_TICKS);
    b.run(PROBE_TICKS);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "the household-barter run must be byte-identical for the same (seed, config)"
    );
    assert_eq!(
        a.promoted_at_tick(),
        b.promoted_at_tick(),
        "the promotion tick must be deterministic"
    );
}

// ---- non-vacuity + the cold-start classification --------------------------

#[test]
fn frontier_household_barter_non_vacuity_chain() {
    // The mandatory non-vacuity gate: at least one lineage member is SPATIAL, becomes
    // `cultivating`, HAULS grain, and produces `SelfProduced` bread — then the chain runs
    // through to a cleared pre-promotion `bread → SALT IndirectFor{WOOD}` lane.
    let r = run(7, &SettlementConfig::frontier_household_barter());
    assert!(r.conserved_every_tick, "conservation must hold every tick");
    assert!(
        r.spatial_lineage_cultivator,
        "a lineage member must be spatial AND cultivating (the cold-start trigger)"
    );
    assert!(
        r.cultivator_hauled_grain,
        "a cultivating lineage member must haul grain into its own stock"
    );
    assert!(
        r.s.acquisition_credited_by_channel().self_produced > 0,
        "cultivation must produce SelfProduced bread"
    );
    // The 3-way classification must land on Success (not a bad probe / production-flow /
    // barter-topology finding).
    assert_eq!(
        classify(&r),
        ColdStart::Success,
        "the cold-start chain must reach a cleared pre-promotion bread→SALT IndirectFor{{WOOD}} lane"
    );
}

#[test]
fn cross_tick_surplus_sells_next_tick() {
    // The cross-tick non-vacuity test: a cultivator produces SelfProduced bread at tick `t`
    // (the cultivate phase is POST-market), and at tick `t+1` its above-reserve bread is
    // visible as a live or cleared `bread → SALT IndirectFor{WOOD}` lane — proving the
    // post-market→next-tick sale path works.
    let r = run(7, &SettlementConfig::frontier_household_barter());
    assert!(
        r.cross_tick_sale,
        "a produced surplus must be offered as a bread→SALT IndirectFor{{WOOD}} lane the next tick"
    );
}

// ---- the headline: endogenous SelfProduced supply monetizes SALT ----------

#[test]
fn frontier_household_barter_classified_seed7_1600() {
    let r = run(7, &SettlementConfig::frontier_household_barter());
    let s = &r.s;
    let bread = bread_good(s);

    // Cultivation fired and conserved.
    assert!(r.conserved_every_tick, "conservation must hold every tick");
    assert_eq!(
        classify(&r),
        ColdStart::Success,
        "the headline run must succeed"
    );

    // SALT promotes as the medium leader.
    assert_eq!(
        s.current_money_good(),
        Some(SALT),
        "SALT must promote on the cultivated supply"
    );
    assert!(
        s.promoted_at_tick().is_some(),
        "SALT must have a promotion tick"
    );

    // Indirect breadth includes the non-food WOOD target (the two-sided {bread, WOOD} bar).
    let targets = s.indirect_target_goods(SALT);
    assert!(
        targets.contains(&WOOD),
        "SALT's indirect breadth must include the non-food WOOD target, got {targets:?}"
    );
    assert!(
        targets.contains(&bread),
        "SALT's indirect breadth must include the bread target, got {targets:?}"
    );

    // The SALT-mediated share dominates the residual direct barter.
    let share = salt_share_bps(s, bread);
    assert!(
        share >= HEADLINE_MIN_SALT_SHARE_BPS,
        "the SALT-mediated share {share} bps must clear the headline bar {HEADLINE_MIN_SALT_SHARE_BPS}"
    );

    // The means role completes (SALT round-trips, not hoarded).
    let (spent, accepted) = s.salt_round_trip();
    assert!(
        accepted > 0,
        "SALT must be accepted as a means (IndirectFor)"
    );
    assert!(
        spent > 0,
        "the means role must complete — SALT is spent on its target"
    );

    // THE CORE CLAIM — the pre-promotion bread sold for SALT is SelfProduced, NOT minted,
    // and NO SeededMinted bread ever entered.
    let (pp_produced, pp_minted) = s.pre_promotion_bread_for_salt_by_provenance();
    assert!(
        pp_produced > 0,
        "pre-promotion bread sold for SALT must be SelfProduced (produced > 0)"
    );
    assert_eq!(
        pp_minted, 0,
        "no minted bread may be sold for SALT pre-promotion (the supply is endogenous)"
    );
    let credited = s.acquisition_credited_by_channel();
    assert_eq!(
        credited.seeded_minted, 0,
        "NO SeededMinted bread may ever enter (every bread buffer + starting_food zeroed)"
    );
    assert!(
        pre_promotion_bread_salt_wood_cleared(s, bread) > 0,
        "at least one pre-promotion bread→SALT IndirectFor{{WOOD}} lane must clear"
    );

    // Self-sustaining: food eaten is SelfProduced/Bought, never seeded/foraged.
    let consumed = s.acquisition_consumed_by_channel();
    assert_eq!(
        consumed.seeded_minted, 0,
        "no SeededMinted food is ever eaten"
    );
    assert_eq!(
        consumed.foraged, 0,
        "no foraged food (no forage good is interned)"
    );
    assert!(
        consumed.self_produced > 0 && consumed.bought > 0,
        "the colony eats SelfProduced AND Bought food (a real market for survival)"
    );
}

#[test]
fn production_is_grain_bounded_not_minted() {
    // The honesty guard: produced bread is grain-derived (recurring from the real commons),
    // never minted. Bread is NEVER minted (the endowment term is 0 every tick), no
    // SeededMinted/Foraged bread enters, and production consumes the real grain commons.
    let r = run(7, &SettlementConfig::frontier_household_barter());
    assert_eq!(
        r.bread_minted_max, 0,
        "no bread may be minted in any tick (the food mints are retired)"
    );
    assert!(
        r.produced_bread > 0,
        "bread must be produced (cultivation + chain)"
    );
    assert!(
        r.consumed_grain > 0,
        "production must consume the real grain commons (grain-bounded, recurring)"
    );
    let credited = r.s.acquisition_credited_by_channel();
    assert_eq!(
        credited.seeded_minted, 0,
        "no SeededMinted bread ever enters"
    );
    assert_eq!(credited.foraged, 0, "no foraged bread ever enters");
    assert!(
        credited.self_produced > 0,
        "the bread that enters is SelfProduced (cultivated + baked)"
    );
}

// ---- the control matrix (classify, never tune) ----------------------------

#[test]
fn household_barter_control_matrix() {
    // CONTROL 1 — cultivation OFF (`household_barter_cultivation = false`): without the
    // activation seam cultivation requires the absent forage substrate, so NOTHING is
    // cultivated, no bread enters, and the market collapses to the S21d zero supply.
    let mut c = SettlementConfig::frontier_household_barter();
    c.chain
        .as_mut()
        .expect("chain")
        .household_barter_cultivation = false;
    let off = run(7, &c);
    assert_eq!(
        off.first_cultivate_tick, None,
        "no colonist cultivates with the seam off"
    );
    assert_eq!(
        off.produced_bread, 0,
        "no bread is produced with the seam off"
    );
    assert!(
        off.s.promoted_at_tick().is_none(),
        "SALT must NOT promote with no cultivated supply (the S21d collapse)"
    );

    // CONTROL 2 — no WOOD-poor lineage (restore lineage WOOD): a lineage that is warm from
    // its own WOOD has no unsatisfied WOOD target, so it never posts the `bread → SALT
    // IndirectFor{WOOD}` medium lane — and SALT does not promote.
    let mut c = SettlementConfig::frontier_household_barter();
    for h in &mut c.demography.as_mut().expect("demography").households {
        h.starting_wood = 6;
        h.wood_provision = 3;
    }
    let no_wood_poor = run(7, &c);
    assert!(
        no_wood_poor.first_cultivate_tick.is_some(),
        "cultivation still fires (the control isolates the medium lane, not cultivation)"
    );
    assert!(
        no_wood_poor.s.promoted_at_tick().is_none(),
        "without an unsatisfied WOOD target the medium lane never forms, so SALT must NOT promote"
    );

    // CONTROL 3 — two-layer saleability OFF: without the two-layer eligibility floor +
    // medium-share ranking, SALT does not promote.
    let mut c = SettlementConfig::frontier_household_barter();
    c.barter
        .as_mut()
        .expect("barter")
        .menger
        .two_layer_saleability = false;
    let two_layer_off = run(7, &c);
    assert!(
        two_layer_off.s.promoted_at_tick().is_none(),
        "SALT must NOT promote with two-layer saleability off"
    );

    // CONTROL 4 — multi-offer medium OFF: without the two-lane medium book SALT does not
    // promote.
    let mut c = SettlementConfig::frontier_household_barter();
    c.barter.as_mut().expect("barter").menger.multi_offer_medium = false;
    let multi_off = run(7, &c);
    assert!(
        multi_off.s.promoted_at_tick().is_none(),
        "SALT must NOT promote with the multi-offer medium book off"
    );

    // CONTROL 5 — buy/sell split OFF (`cultivation_sells_surplus = false`): classified, not
    // a clean negative. With the split off the eligibility is `household.is_none() ||
    // spatial`, so the SALT-rich consumers AND the woodcutters ALSO self-cultivate — the
    // division of labor that scopes the bread supply to lineages is LOST, so much more bread
    // is produced. In this WOOD-rich topology SALT still monetizes (the WOOD↔bread
    // cross-demand mediated by the SALT-rich consumers survives), so the split is NOT
    // load-bearing for monetization — only for the clean lineage-sourced household-production
    // story. (Reported, never tuned.)
    let mut c = SettlementConfig::frontier_household_barter();
    c.chain.as_mut().expect("chain").cultivation_sells_surplus = false;
    let split_off = run(7, &c);
    let headline = run(7, &SettlementConfig::frontier_household_barter());
    assert!(
        split_off.produced_bread > 2 * headline.produced_bread,
        "with the buy/sell split off the supply is no longer lineage-scoped: non-lineage \
         colonists also cultivate, so far more bread is produced ({} vs {})",
        split_off.produced_bread,
        headline.produced_bread
    );
    // Provenance stays clean either way: no SeededMinted bread can enter.
    assert_eq!(
        split_off.s.acquisition_credited_by_channel().seeded_minted,
        0,
        "no SeededMinted bread enters even with the split off (every buffer is zeroed)"
    );
}

// ---- positive control: the S21e seeded-surplus sibling --------------------

#[test]
fn seeded_surplus_positive_control_still_monetizes() {
    // S21e (seeded supply) is the positive control: a finite seeded bread supply DOES
    // monetize SALT. S21f matches it with CULTIVATED supply. (The seeded sibling is the
    // SeededMinted analogue — its pre-promotion supply is minted, the S21f bar inverts it.)
    let mut s = Settlement::generate(7, &SettlementConfig::frontier_seeded_surplus());
    s.run(PROBE_TICKS);
    assert_eq!(
        s.current_money_good(),
        Some(SALT),
        "the S21e seeded-surplus positive control must still promote SALT"
    );
}

// ---- the grain-flow sweep -------------------------------------------------

fn with_grain_flow(stock: u32, regen: u32, cap: u32) -> SettlementConfig {
    let mut cfg = SettlementConfig::frontier_household_barter();
    let grain = cfg.chain.as_ref().expect("chain").content.grain();
    for node in cfg.nodes.iter_mut() {
        if node.good == grain {
            node.stock = stock;
            node.regen = regen;
            node.cap = cap;
        }
    }
    cfg
}

#[test]
fn grain_flow_sweep_brackets_promotion() {
    // Zero grain flow → NO produced bread, NO promotion (the supply is flow-starved).
    let zero = run(7, &with_grain_flow(0, 0, 0));
    assert_eq!(zero.produced_bread, 0, "zero grain flow produces no bread");
    assert_eq!(zero.consumed_grain, 0, "zero grain flow consumes no grain");
    assert!(
        zero.s.promoted_at_tick().is_none(),
        "zero grain flow must NOT promote (no supply)"
    );

    // A finite grain STOCK with NO regen → cultivation produces bread ONCE (bounded EXACTLY
    // by the grain consumed — the grain-bounded identity in the cultivation-only regime),
    // then the commons runs dry. A one-time finite supply is INSUFFICIENT to monetize: this
    // is the proof that promotion needs a RECURRING flow, NOT seed exhaustion.
    let finite = run(7, &with_grain_flow(240, 0, 240));
    assert!(
        finite.produced_bread > 0,
        "a finite grain stock produces some bread"
    );
    assert_eq!(
        finite.produced_bread, finite.consumed_grain,
        "with only cultivation running, produced[bread] == consumed_as_input[grain] (grain-bounded)"
    );
    assert!(
        finite.produced_bread <= 240,
        "a finite stock of 240 grain can yield at most 240 cultivated bread (no recurring mint)"
    );
    assert!(
        finite.s.promoted_at_tick().is_none(),
        "a one-time finite grain stock is insufficient — SALT must NOT promote (recurring flow needed)"
    );

    // A real RECURRING flow → SALT promotes on the SelfProduced supply, and produced bread
    // tracks the grain consumed (recurring production from the real commons).
    let flow = run(7, &with_grain_flow(480, 24, 960));
    assert_eq!(
        flow.s.current_money_good(),
        Some(SALT),
        "a recurring grain flow must promote SALT on the cultivated supply"
    );
    assert!(
        flow.consumed_grain > finite.consumed_grain,
        "the recurring flow consumes far more grain than the one-time stock ({} vs {})",
        flow.consumed_grain,
        finite.consumed_grain
    );
    assert_eq!(
        flow.s.pre_promotion_bread_for_salt_by_provenance().1,
        0,
        "the promotion rests on SelfProduced bread (zero minted) at any flow"
    );
}

// ---- cross-seed robustness ------------------------------------------------

#[test]
fn household_barter_holds_across_seeds() {
    for seed in [3u64, 7, 11, 19, 23] {
        let r = run(seed, &SettlementConfig::frontier_household_barter());
        let s = &r.s;
        let bread = bread_good(s);
        assert!(
            r.conserved_every_tick,
            "seed {seed}: conservation must hold every tick"
        );
        assert_eq!(
            classify(&r),
            ColdStart::Success,
            "seed {seed}: the cold-start chain must succeed"
        );
        assert_eq!(
            s.current_money_good(),
            Some(SALT),
            "seed {seed}: SALT must promote on the cultivated supply"
        );
        assert!(
            s.indirect_target_goods(SALT).contains(&WOOD),
            "seed {seed}: the indirect breadth must include the WOOD target"
        );
        assert_eq!(
            s.acquisition_credited_by_channel().seeded_minted,
            0,
            "seed {seed}: NO SeededMinted bread may ever enter"
        );
        assert_eq!(
            s.pre_promotion_bread_for_salt_by_provenance().1,
            0,
            "seed {seed}: no minted bread is sold for SALT pre-promotion"
        );
        assert!(
            salt_share_bps(s, bread) >= HEADLINE_MIN_SALT_SHARE_BPS,
            "seed {seed}: the SALT-mediated share must clear the headline bar"
        );
    }
}
