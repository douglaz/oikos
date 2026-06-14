//! G3b acceptance suite — production roles **emerge** from realized price spreads.
//!
//! G3a proved the grain→flour→bread chain *operates* with **seeded** producer
//! roles. G3b removes the seeding: a pool of colonists hold latent production
//! capital (a mill or an oven) and start [`Vocation::Unassigned`], and each econ
//! tick re-appraise — ordinally, reusing econ's M2.5 entrepreneurial appraisal
//! ([`sim::recipe_adoption_pays`] → `econ::bundle::appraise_project_bundle_for_money`)
//! against the realized prices they can observe and their *own* value scale —
//! whether running their recipe pays. When the spread pays they adopt
//! [`Vocation::Miller`]/[`Vocation::Baker`]; when it collapses they revert.
//!
//! These pin the milestone's DoD as a **mechanism + falsification control**: the
//! chain forms from prices in `emergent-chain` (test 2) and does **not** form in
//! the no-spread control (test 3) — the spread is what creates the roles. The
//! choice is ordinal (test 4: no scalar profit threshold; an unprovisioned future
//! want is what the spread must satisfy), roles track the spread (test 5: a miller
//! re-appraises and stops when its flour−grain spread collapses), conservation
//! still holds exactly under emergent roles (test 6), and `econ` market behaviour is
//! unchanged (test 7). The multi-seed robustness *study* (the "≥X% of N worlds"
//! gate) is deferred — this is the mechanism, not the robustness number.

use econ::agent::{Agent, AgentId, Role, Want, WantKind};
use econ::good::{Gold, GoodId, Horizon, Stock, GOLD, NET};
use econ::project::Recipe;
use sim::{recipe_adoption_pays, ContentSet, Settlement, SettlementConfig, Vocation};

/// The G3b emergent-chain config (no seeded producer roles).
fn emergent_config() -> SettlementConfig {
    SettlementConfig::emergent_chain()
}

/// The G3b no-spread falsification control (bread demand removed).
fn control_config() -> SettlementConfig {
    SettlementConfig::emergent_chain_control()
}

struct ChainGoods {
    flour: GoodId,
    bread: GoodId,
}

fn chain_goods(s: &Settlement) -> ChainGoods {
    let content = s.content().expect("a chain settlement has content");
    ChainGoods {
        flour: content.flour(),
        bread: content.bread(),
    }
}

/// 1. `emergent_chain_run_is_deterministic` — same `(seed, config)` →
///    byte-identical run. The role-choice phase draws no randomness (it reads
///    realized prices and the agent's own scale, both integer state), so two runs
///    stay in lockstep tick by tick; a different seed (different drawn cultures)
///    diverges.
#[test]
fn emergent_chain_run_is_deterministic() {
    let config = emergent_config();

    let mut a = Settlement::generate(0xC0FFEE, &config);
    let mut b = Settlement::generate(0xC0FFEE, &config);
    a.run(40);
    b.run(40);
    assert_eq!(
        a.canonical_bytes(),
        b.canonical_bytes(),
        "same seed diverged"
    );
    assert_eq!(a.digest(), b.digest());

    // Tick-by-tick lockstep: nothing is drawn in the loop or the role-choice phase,
    // so the digest matches at every econ tick.
    let mut x = Settlement::generate(7, &config);
    let mut y = Settlement::generate(7, &config);
    for tick in 0..40 {
        x.econ_tick();
        y.econ_tick();
        assert_eq!(x.digest(), y.digest(), "drifted at econ tick {tick}");
    }

    // A different seed yields a different run (generation actually uses the Rng to
    // draw cultures, which steer adoption timing).
    let mut c = Settlement::generate(0xBADF00D, &config);
    c.run(40);
    assert_ne!(a.digest(), c.digest(), "the seed did not matter");
}

/// 2. `roles_emerge_from_the_spread` — in `emergent-chain` (no seeded roles), over
///    a run at least one colonist adopts milling and at least one adopts baking,
///    and bread is produced and consumed — the chain forms from prices alone. No
///    role is hand-placed: every producer here *chose* its vocation from the spread.
#[test]
fn roles_emerge_from_the_spread() {
    let mut s = Settlement::generate(2_026, &emergent_config());
    let g = chain_goods(&s);

    // No producer roles are seeded — everyone starts unassigned (latent) or as a
    // gatherer/consumer.
    assert_eq!(
        s.vocation_count(Vocation::Miller),
        0,
        "a miller was seeded — G3b must not hand-place producers"
    );
    assert_eq!(
        s.vocation_count(Vocation::Baker),
        0,
        "a baker was seeded — G3b must not hand-place producers"
    );
    assert!(
        s.vocation_count(Vocation::Unassigned) > 0,
        "the emergent config must seed a latent producer pool"
    );

    let (mut max_millers, mut max_bakers) = (0usize, 0usize);
    let (mut bread_made, mut bread_eaten) = (0u64, 0u64);
    for _ in 0..24 {
        let report = s.econ_tick();
        max_millers = max_millers.max(s.living_count(Vocation::Miller));
        max_bakers = max_bakers.max(s.living_count(Vocation::Baker));
        bread_made += report.produced_of(g.bread);
        bread_eaten += report.consumed_of(g.bread);
    }

    // Roles emerged: at least one colonist took up each producer vocation.
    assert!(
        max_millers >= 1,
        "no colonist ever adopted milling — the chain did not form from the spread"
    );
    assert!(
        max_bakers >= 1,
        "no colonist ever adopted baking — the chain did not form from the spread"
    );
    // The chain flowed: bread was produced (by adopted bakers) and consumed.
    assert!(bread_made > 0, "no bread was produced under emergent roles");
    assert!(
        bread_eaten > 0,
        "no bread was consumed under emergent roles"
    );
    // And flour really moved through the middle of the chain (so milling fed baking,
    // not just seeded buffers).
    assert!(
        s.realized_price(g.flour).is_some(),
        "flour never realized a price — the middle of the chain never traded"
    );
}

/// 3. `no_spread_no_roles` — the falsification control. With the spread removed
///    (bread is not the staple, so bread demand is absent and bread/flour never
///    realize a price), the **same** role-choice appraisal — run over the **same**
///    latent pool every tick — adopts NO production vocation, and no flour or bread
///    is ever produced. Paired with test 2 this shows the spread is what creates the
///    roles: if both formed roles, the mechanism would be reading something other
///    than the spread.
#[test]
fn no_spread_no_roles() {
    let mut s = Settlement::generate(2_026, &control_config());
    let g = chain_goods(&s);

    // Same latent pool as the emergent config (so the role-choice code genuinely
    // runs and declines — the control is not vacuous).
    let latent_pool = s.living_count(Vocation::Unassigned);
    assert!(
        latent_pool > 0,
        "the control must seed the same latent producer pool"
    );

    for tick in 0..30 {
        let report = s.econ_tick();
        assert_eq!(
            s.living_count(Vocation::Unassigned),
            latent_pool,
            "the control's latent pool stopped being live at tick {tick}"
        );
        // No colonist ever adopts a production vocation.
        assert_eq!(
            s.living_count(Vocation::Miller),
            0,
            "a colonist adopted milling with no spread (tick {tick})"
        );
        assert_eq!(
            s.living_count(Vocation::Baker),
            0,
            "a colonist adopted baking with no spread (tick {tick})"
        );
        // And so nothing is ever produced.
        assert_eq!(
            report.produced_of(g.flour),
            0,
            "flour was produced with no spread (tick {tick})"
        );
        assert_eq!(
            report.produced_of(g.bread),
            0,
            "bread was produced with no spread (tick {tick})"
        );
    }

    // Bread and flour never even realized a price (no demand → no spread to read).
    assert!(
        s.realized_price(g.bread).is_none(),
        "bread realized a price in the no-demand control"
    );
    assert!(
        s.realized_price(g.flour).is_none(),
        "flour realized a price in the no-demand control"
    );
}

/// A standalone appraiser agent: a patient colonist's scale (several unprovisioned
/// `Later` savings wants) plus `gold` on hand. With `gold = 0` the savings wants are
/// unprovisioned (the spread can satisfy one); with `gold` ≥ the savings count they
/// are all provisioned (nothing left for a spread to provision).
fn appraiser_with_id(id: AgentId, gold: u64) -> Agent {
    let mut scale = Vec::new();
    // A present survival want (realistic; provisioned/irrelevant to the gold logic).
    scale.push(Want {
        kind: WantKind::Leisure,
        horizon: Horizon::Now,
        qty: 1,
        satisfied: false,
    });
    // The patient savings ladder the entrepreneurial appraisal targets.
    for _ in 0..6 {
        scale.push(Want {
            kind: WantKind::Good(GOLD),
            horizon: Horizon::Later(4),
            qty: 1,
            satisfied: false,
        });
    }
    Agent {
        id,
        scale,
        stock: Stock::new(NET.0),
        gold: Gold(gold),
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: Vec::new(),
    }
}

fn appraiser(gold: u64) -> Agent {
    appraiser_with_id(AgentId(1), gold)
}

/// 4. `role_choice_is_ordinal_not_scalar` — the adoption decision routes through the
///    ordinal appraisal (reused `appraise_project_bundle`), not a scalar profit
///    threshold. A colonist declines a vocation whose output does not outrank its
///    costs on its scale, and adopts when it does — and, the ordinal tell, a colonist
///    whose savings are already sated declines **even with a fat spread**, because
///    the appraisal asks whether the spread newly provisions a *future want*, not
///    whether a profit number is positive.
#[test]
fn role_choice_is_ordinal_not_scalar() {
    let content = ContentSet::grain_flour_bread();
    let mill: &Recipe = content.mill_recipe();
    let operating_cost = 1;

    // (a) The output does NOT outrank its costs: flour is cheap, grain dear, so the
    //     flour the recipe yields buys less than the grain + cost it takes. Decline.
    let poor = appraiser(0);
    assert!(
        !recipe_adoption_pays(
            &poor,
            mill,
            Some(Gold(1)),
            Some(Gold(10)),
            0,
            operating_cost
        ),
        "adopted a vocation whose output does not outrank its costs"
    );

    // (b) The output DOES outrank its costs (a real spread): flour dear, grain cheap.
    //     With an unprovisioned savings want for that spread to satisfy, adopt.
    assert!(
        recipe_adoption_pays(&poor, mill, Some(Gold(5)), Some(Gold(1)), 0, operating_cost),
        "declined a vocation whose output clearly outranks its costs"
    );
    let first_colonist = appraiser_with_id(AgentId(0), 0);
    assert!(
        recipe_adoption_pays(
            &first_colonist,
            mill,
            Some(Gold(5)),
            Some(Gold(1)),
            0,
            operating_cost
        ),
        "AgentId(0) must appraise the same ordinal bundle as every other colonist"
    );

    // (c) The ordinal tell: a gold-sated colonist (every savings want already
    //     provisioned) declines the SAME fat spread — there is no future want left
    //     for the spread to provision. A scalar profit-maximizer would still take a
    //     positive-profit project; the ordinal appraisal does not.
    let sated = appraiser(100);
    assert!(
        !recipe_adoption_pays(&sated, mill, Some(Gold(5)), Some(Gold(1)), 0, operating_cost),
        "a gold-sated colonist still adopted — the choice is reading a scalar profit, not its scale"
    );

    // And a colonist with no future (savings) want at all never appraises a vocation
    // — there is no future provisioning to weigh ordinally.
    let mut no_savings = appraiser(0);
    no_savings.scale.retain(|w| w.kind != WantKind::Good(GOLD));
    assert!(
        !recipe_adoption_pays(
            &no_savings,
            mill,
            Some(Gold(5)),
            Some(Gold(1)),
            0,
            operating_cost
        ),
        "a colonist with no future want adopted from a scalar profit"
    );

    // No observable output price ⇒ no spread to read ⇒ decline (the control's gate).
    assert!(
        !recipe_adoption_pays(&poor, mill, None, Some(Gold(1)), 0, operating_cost),
        "adopted with no realized output price to sell into"
    );
}

/// 5. `role_reverts_when_spread_collapses` — a miller whose flour−grain spread
///    collapses re-appraises and stops milling (roles track the spread). The
///    role-choice phase calls exactly [`recipe_adoption_pays`] every tick, so the
///    reversion is precisely this returning `false` once the spread is gone. Using a
///    **real active miller** from a mid-run emergent settlement: with the spread it
///    is living on it stays a miller, but flip its input (grain) cost above the flour
///    it would yield — the spread collapses — and the same re-appraisal stops it.
#[test]
fn role_reverts_when_spread_collapses() {
    let config = emergent_config();
    let operating_cost = config
        .chain
        .as_ref()
        .expect("emergent config has a chain")
        .operating_cost;
    let mut s = Settlement::generate(2_026, &config);
    // Run until milling has emerged.
    let mut adopted_tick = None;
    for tick in 0..12 {
        s.econ_tick();
        if s.living_count(Vocation::Miller) > 0 {
            adopted_tick = Some(tick);
            break;
        }
    }
    assert!(
        adopted_tick.is_some(),
        "milling never emerged, so reversion cannot be exercised"
    );

    let content = s.content().expect("chain content").clone();
    let mill: &Recipe = content.mill_recipe();
    let grain = content.grain();
    let flour = content.flour();

    let miller_slot = (0..s.population())
        .find(|&index| s.vocation_of(index) == Some(Vocation::Miller))
        .expect("an active miller exists");
    let miller_id = s.colonist_id(miller_slot).expect("miller has an id");
    let flour_price = s.realized_price(flour).expect("flour realized a price");
    let grain_price = s.realized_price(grain).expect("grain realized a price");
    let tick = s.econ_tick_count();
    let agent = s
        .society()
        .agents
        .get(miller_id)
        .expect("the miller resolves in the society");

    // With the spread it adopted on, the re-appraisal keeps it milling.
    assert!(
        recipe_adoption_pays(agent, mill, Some(flour_price), Some(grain_price), tick, operating_cost),
        "the miller's own spread no longer supports milling — the setup is not exercising a live role"
    );

    // Collapse the flour−grain spread: grain (the input it must acquire) becomes as
    // dear as all the flour the recipe would yield, so the spread no longer clears
    // the operating cost. The same re-appraisal now reverts the miller.
    let collapsed_grain = Gold(flour_price.0 * u64::from(mill.output_qty) + 1);
    assert!(
        !recipe_adoption_pays(agent, mill, Some(flour_price), Some(collapsed_grain), tick, operating_cost),
        "the miller kept milling after its flour−grain spread collapsed — the role does not track the spread"
    );
}

/// 6. `emergent_chain_conserves` — transformation conservation (G3a) still holds
///    exactly under emergent roles. For every tracked good the whole-system total
///    moves by EXACTLY `+regen +produced −consumed_as_input −consumed` every econ
///    tick, and the proof is non-vacuous (adopted producers really transformed
///    goods). Role adoption/reversion mutates only vocations, never the conserved
///    physical ledger.
#[test]
fn emergent_chain_conserves() {
    let mut s = Settlement::generate(99, &emergent_config());
    let goods: Vec<GoodId> = s.tracked_goods().to_vec();

    let mut prev: Vec<u64> = goods
        .iter()
        .map(|&good| s.whole_system_total(good))
        .collect();
    let (mut any_produced, mut any_input, mut any_eaten) = (0u64, 0u64, 0u64);

    for tick in 0..40 {
        let report = s.econ_tick();
        assert!(
            report.conserves(),
            "report ledger unbalanced at tick {tick}"
        );
        for (i, &good) in goods.iter().enumerate() {
            let after = s.whole_system_total(good);
            let before = prev[i];
            let regen = report.regen_of(good) as i128;
            let produced = report.produced_of(good) as i128;
            let consumed_as_input = report.consumed_as_input_of(good) as i128;
            let consumed = report.consumed_of(good) as i128;
            assert_eq!(
                after as i128,
                before as i128 + regen + produced - consumed_as_input - consumed,
                "{good:?} conservation broke at tick {tick}: before={before} after={after} \
                 regen={regen} produced={produced} consumed_as_input={consumed_as_input} \
                 consumed={consumed}"
            );
            prev[i] = after;
            any_produced += report.produced_of(good);
            any_input += report.consumed_as_input_of(good);
            any_eaten += report.consumed_of(good);
        }
    }

    assert!(
        any_produced > 0,
        "nothing was ever produced (vacuous proof)"
    );
    assert!(any_input > 0, "nothing was ever consumed as a recipe input");
    assert!(any_eaten > 0, "nothing was ever eaten");
}

/// 7. `econ_unchanged` — the engine's conformance scenarios still replay
///    byte-identically and conserve gold (the six econ goldens are untouched by the
///    additive G3b reuse — the role-choice appraisal reuses `econ`'s existing
///    `appraise_project_bundle_for_money`, adding no `econ` edit), and a plain G2b
///    settlement is byte-identical with or without the (defaulted-`None`) chain
///    field. The full `cargo test --workspace`, `cargo clippy -- -D warnings`,
///    `cargo fmt --check`, and the praxeology source-gate run outside this test.
#[test]
fn econ_unchanged() {
    use econ::scenario::{builtin_market_scenario, ScenarioName};
    use econ::society::Society;

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

    // A plain settlement is byte-identical to one with an explicitly-absent chain —
    // the additive G3b state never moves a non-chain digest.
    let plain = Settlement::generate(7, &SettlementConfig::viable());
    let mut explicit = SettlementConfig::viable();
    explicit.chain = None;
    let explicit = Settlement::generate(7, &explicit);
    assert_eq!(plain.digest(), explicit.digest());
}

/// Unit: the control is the emergent config with bread demand removed — same latent
/// pool and recipes, only the staple (and thus the demand that prices the chain)
/// differs. This is what makes the test-2/test-3 pair a clean falsification.
#[test]
fn control_is_the_emergent_world_without_bread_demand() {
    let emergent = SettlementConfig::emergent_chain();
    let control = SettlementConfig::emergent_chain_control();
    let ec = emergent.chain.as_ref().expect("emergent chain");
    let cc = control.chain.as_ref().expect("control chain");

    // The chain content, raw input supply, and latent pool are identical.
    assert_eq!(ec.content, cc.content, "the chain content must match");
    assert_eq!(emergent.nodes, control.nodes, "the raw node must match");
    assert_eq!(ec.latent_millers, cc.latent_millers);
    assert_eq!(ec.latent_bakers, cc.latent_bakers);
    assert_eq!(ec.millers, 0, "no seeded millers in the emergent config");
    assert_eq!(ec.bakers, 0, "no seeded bakers in the emergent config");

    // The causal difference is demand: bread is the staple in the emergent config,
    // not in the control.
    assert!(ec.bread_is_staple, "emergent config: bread is the staple");
    assert!(!cc.bread_is_staple, "control: bread demand is removed");
}
