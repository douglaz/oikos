//! Recipes, project templates, project lifecycle, and physical loss accounting.

use crate::good::{GoodId, Stock, FOOD, NET, WOOD};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecipeId {
    GatherFood,
    CutWood,
    FishWithNet,
    /// G3a production-chain: mill grain into flour (a content recipe, applied by
    /// the `sim` producer phase, never by the lab planner).
    Mill,
    /// G3a production-chain: bake flour into bread (a content recipe).
    Bake,
    /// G6b research: a scholar turns a conserved good input + labor into
    /// **Knowledge** (a content recipe, applied by the `sim` scholar phase, never
    /// by the lab planner). Knowledge is an accumulator, not a tradeable good —
    /// `sim` drains the recipe's output into a per-settlement counter.
    Research,
    /// G6b tech tier 2: a tier-gated higher-order recipe (starts `enabled: false`,
    /// flipped `true` by the `sim` unlock once Knowledge crosses the threshold). A
    /// content recipe, applied by the `sim` producer phase, never by the lab planner.
    Confect,
    /// S15 own-use cultivation: a **no-tool** grain → bread recipe a hungry colonist
    /// runs by its OWN labor (the more-roundabout, more-laborious alternative to
    /// foraging), applied by the `sim` own-use cultivation phase, never by the lab
    /// planner or the producer phase. Its conversion is booked `produced`/
    /// `consumed_as_input` and the bread is eaten at home (own-use) — not traded. A
    /// content recipe carried only by the gated cultivation content set, so every
    /// other config is byte-identical.
    Cultivate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipe {
    pub id: RecipeId,
    pub name: &'static str,
    pub labor: u32,
    pub input_good: Option<(GoodId, u32)>,
    pub required_tool: Option<GoodId>,
    pub output_good: GoodId,
    pub output_qty: u32,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    NearTerm,
    Distant,
}

pub fn recipe_reach(recipe: &Recipe) -> Reach {
    if recipe.required_tool.is_some() {
        Reach::Distant
    } else {
        Reach::NearTerm
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectTemplateId {
    BuildNet,
    /// G7 roads: a **public-works** project — community labor contributed to an
    /// inter-settlement road until a labor cost is met, at which point the `sim`
    /// `Region` cuts the route's transit cost. Reuses the existing project-labor
    /// lifecycle ([`start_project`]/[`advance_project`]/[`complete_project_if_ready`]);
    /// it is built only by the game (`sim`), never by the lab planner, so it is kept
    /// out of [`builtin_project_templates`] and the conformance goldens are
    /// byte-identical.
    BuildRoad,
    /// S7 producible capital: a **per-agent** project that mints a **mill** (a durable
    /// `required_tool` for the Mill recipe) from saved WOOD + labor. Like [`BuildNet`]
    /// it outputs a real tool — proof a project can mint production capital — but it is
    /// driven by the `sim` per-builder capital-formation phase (one builder, its own
    /// WOOD via [`start_project`], its own labor), never by the lab planner, so it is
    /// kept out of [`builtin_project_templates`] and the conformance goldens are
    /// byte-identical.
    BuildMill,
    /// S7 producible capital: the per-agent project that mints an **oven** (the Bake
    /// recipe's `required_tool`) from saved WOOD + labor. The baker-side twin of
    /// [`BuildMill`]; game-only, absent from [`builtin_project_templates`].
    BuildOven,
}

#[derive(Clone, Debug)]
pub struct ProjectTemplate {
    pub id: ProjectTemplateId,
    pub name: &'static str,
    pub input_goods: Vec<(GoodId, u32)>,
    pub required_labor: u32,
    pub output_good: GoodId,
    pub output_qty: u32,
    pub salvage_bps: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectState {
    Forming,
    Complete,
    Abandoned,
}

#[derive(Clone, Debug)]
pub struct Project {
    pub id: ProjectId,
    pub template: ProjectTemplateId,
    pub state: ProjectState,
    pub started_at: Tick,
    pub labor_advanced: u32,
    pub input_goods_committed: Vec<(GoodId, u32)>,
    pub output_good: GoodId,
    pub output_qty: u32,
    pub salvage_bps: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CapitalLoss {
    pub labor_consumed: u32,
    pub goods_consumed: u32,
}

pub fn builtin_recipes() -> Vec<Recipe> {
    vec![
        Recipe {
            id: RecipeId::GatherFood,
            name: "GatherFood",
            labor: 1,
            input_good: None,
            required_tool: None,
            output_good: FOOD,
            output_qty: 2,
            enabled: true,
        },
        Recipe {
            id: RecipeId::CutWood,
            name: "CutWood",
            labor: 1,
            input_good: None,
            required_tool: None,
            output_good: WOOD,
            output_qty: 1,
            enabled: true,
        },
        Recipe {
            id: RecipeId::FishWithNet,
            name: "FishWithNet",
            labor: 1,
            input_good: None,
            required_tool: Some(NET),
            output_good: FOOD,
            output_qty: 5,
            enabled: true,
        },
    ]
}

pub fn build_net_template() -> ProjectTemplate {
    ProjectTemplate {
        id: ProjectTemplateId::BuildNet,
        name: "BuildNet",
        input_goods: vec![(WOOD, 2)],
        required_labor: 3,
        output_good: NET,
        output_qty: 1,
        salvage_bps: 5000,
    }
}

/// The G7 **road** public-works project template: a pure-labor project of
/// `required_labor` units and no created good.
///
/// A road is **community labor** that, once enough has been contributed, cuts a
/// route's transit cost — it does not *produce a good*, so `output_qty` is `0`
/// (the completion `stock.add(_, 0)` is a no-op; `output_good` is therefore a
/// don't-care placeholder, kept as a real `GoodId` only because the field is not
/// optional). The optional **conserved material cost** is NOT modelled here as
/// committed `input_goods` (which `start_project` would consume all at once from a
/// single stock); the `sim` `Region` consumes road materials *incrementally* from
/// its own road fund as labor is contributed, and accounts them as
/// `consumed_as_input`, so the build conserves across its whole duration rather
/// than only at the start tick. This template therefore drives just the labor
/// lifecycle (the reused project-labor path); materials are the `Region`'s concern.
///
/// Game-only: built by `sim`, never by the lab planner, and absent from
/// [`builtin_project_templates`] — so adding it leaves every conformance golden
/// byte-identical.
pub fn build_road_template(output_good: GoodId, required_labor: u32) -> ProjectTemplate {
    ProjectTemplate {
        id: ProjectTemplateId::BuildRoad,
        name: "BuildRoad",
        input_goods: Vec::new(),
        required_labor,
        output_good,
        output_qty: 0,
        salvage_bps: 0,
    }
}

/// S7 producible capital: a **tool-minting** per-agent project template — `wood_qty`
/// units of saved WOOD plus `required_labor` labor produce one durable `tool`
/// (a mill or an oven), with **no partial-build salvage** (`salvage_bps: 0`).
///
/// `salvage_bps` is `0` deliberately: the conserved WOOD is committed up front by
/// [`start_project`] (the `sim` capital-formation phase books it to
/// `consumed_as_input` at the START tick) and the built tool is booked to `produced`
/// at completion, so the build conserves end-to-end with no work-in-progress source
/// to account; an abandoned build simply forfeits its committed WOOD (already
/// consumed). The tool itself is a `required_tool` for its recipe (the Mill needs a
/// mill, the Bake an oven), exactly like [`build_net_template`]'s NET — proof a
/// project mints production capital.
///
/// Game-only: built by the `sim` per-builder phase, never by the lab planner, and
/// absent from [`builtin_project_templates`], so adding it leaves every conformance
/// golden byte-identical.
pub fn build_tool_template(
    id: ProjectTemplateId,
    name: &'static str,
    tool: GoodId,
    wood_qty: u32,
    required_labor: u32,
) -> ProjectTemplate {
    ProjectTemplate {
        id,
        name,
        input_goods: vec![(WOOD, wood_qty)],
        required_labor,
        output_good: tool,
        output_qty: 1,
        salvage_bps: 0,
    }
}

/// The S7 [`ProjectTemplateId::BuildMill`] template — a [`build_tool_template`] that
/// mints the given `mill` tool from `wood_qty` WOOD + `required_labor` labor.
pub fn build_mill_template(mill: GoodId, wood_qty: u32, required_labor: u32) -> ProjectTemplate {
    build_tool_template(
        ProjectTemplateId::BuildMill,
        "BuildMill",
        mill,
        wood_qty,
        required_labor,
    )
}

/// The S7 [`ProjectTemplateId::BuildOven`] template — a [`build_tool_template`] that
/// mints the given `oven` tool from `wood_qty` WOOD + `required_labor` labor.
pub fn build_oven_template(oven: GoodId, wood_qty: u32, required_labor: u32) -> ProjectTemplate {
    build_tool_template(
        ProjectTemplateId::BuildOven,
        "BuildOven",
        oven,
        wood_qty,
        required_labor,
    )
}

pub fn builtin_project_templates() -> Vec<ProjectTemplate> {
    vec![build_net_template()]
}

pub fn find_template(
    templates: &[ProjectTemplate],
    id: ProjectTemplateId,
) -> Option<&ProjectTemplate> {
    templates.iter().find(|template| template.id == id)
}

pub fn start_project(
    template: &ProjectTemplate,
    stock: &mut Stock,
    id: ProjectId,
    tick: Tick,
) -> Option<Project> {
    let mut required: Vec<(GoodId, u32)> = Vec::new();
    for (good, qty) in &template.input_goods {
        if let Some((_, total)) = required
            .iter_mut()
            .find(|(required_good, _)| required_good == good)
        {
            *total = total.saturating_add(*qty);
        } else {
            required.push((*good, *qty));
        }
    }

    for (good, qty) in &required {
        if !stock.can_remove(*good, *qty) {
            return None;
        }
    }

    for (good, qty) in &required {
        if !stock.remove(*good, *qty) {
            return None;
        }
    }

    Some(Project {
        id,
        template: template.id,
        state: ProjectState::Forming,
        started_at: tick,
        labor_advanced: 0,
        input_goods_committed: required,
        output_good: template.output_good,
        output_qty: template.output_qty,
        salvage_bps: template.salvage_bps,
    })
}

pub fn advance_project(project: &mut Project) -> bool {
    advance_project_by(project, 1)
}

/// Contribute `labor` units to a forming project in one step — the bulk equivalent
/// of calling [`advance_project`] `labor` times, for callers that pool a whole
/// tick's labor at once (the `sim` G7 road public works contributes the community's
/// per-tick labor in a single call instead of looping unit-by-unit, which a large
/// accepted config could otherwise spin for billions of iterations). Adds the labor
/// and returns `true` iff the project is `Forming`; a no-op returning `false` on a
/// finished project, exactly like [`advance_project`]. Saturating, so an oversized
/// contribution clamps rather than wraps. Additive: the lab planner only ever
/// advances one unit at a time, so the conformance goldens are byte-identical.
pub fn advance_project_by(project: &mut Project, labor: u32) -> bool {
    if project.state != ProjectState::Forming {
        return false;
    }
    project.labor_advanced = project.labor_advanced.saturating_add(labor);
    true
}

pub fn complete_project_if_ready(
    project: &mut Project,
    template: &ProjectTemplate,
    stock: &mut Stock,
) -> bool {
    if project.state == ProjectState::Forming && project.labor_advanced >= template.required_labor {
        project.state = ProjectState::Complete;
        stock.add(project.output_good, project.output_qty);
        return true;
    }
    false
}

pub fn abandon_project(project: &mut Project, stock: &mut Stock) -> CapitalLoss {
    if project.state != ProjectState::Forming {
        return CapitalLoss::default();
    }

    project.state = ProjectState::Abandoned;
    let mut goods_consumed = 0;

    let salvage_bps = project.salvage_bps.min(10_000);
    let mut committed: Vec<(GoodId, u32)> = Vec::new();
    for (good, qty) in &project.input_goods_committed {
        if let Some((_, total)) = committed
            .iter_mut()
            .find(|(committed_good, _)| committed_good == good)
        {
            *total = total.saturating_add(*qty);
        } else {
            committed.push((*good, *qty));
        }
    }

    for (good, qty) in committed {
        let salvage = (u64::from(qty) * u64::from(salvage_bps) / 10_000) as u32;
        if salvage > 0 {
            stock.add(good, salvage);
        }
        goods_consumed += qty.saturating_sub(salvage);
    }

    CapitalLoss {
        labor_consumed: project.labor_advanced,
        goods_consumed,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        abandon_project, advance_project, advance_project_by, build_mill_template,
        build_net_template, build_oven_template, build_road_template, builtin_project_templates,
        complete_project_if_ready, start_project, CapitalLoss, ProjectId, ProjectState,
        ProjectTemplateId, Tick,
    };
    use crate::good::{GoodId, Stock, WOOD};

    #[test]
    fn advance_project_by_equals_looping_advance_project() {
        // The bulk contribution must end in exactly the same state as looping the
        // unit advance, so a caller can pool a whole tick's labor in one call (the
        // G7 road) without changing the deterministic completion tick.
        let template = build_road_template(WOOD, 100);
        let mut fund_loop = Stock::new(8);
        let mut fund_bulk = Stock::new(8);
        let mut looped = start_project(&template, &mut fund_loop, ProjectId(1), Tick(0)).unwrap();
        let mut bulk = start_project(&template, &mut fund_bulk, ProjectId(1), Tick(0)).unwrap();

        for _ in 0..37 {
            advance_project(&mut looped);
        }
        assert!(advance_project_by(&mut bulk, 37));
        assert_eq!(looped.labor_advanced, bulk.labor_advanced);
        assert_eq!(looped.state, bulk.state);

        // One-way: a finished project rejects a bulk contribution, like the unit form.
        bulk.state = ProjectState::Complete;
        assert!(!advance_project_by(&mut bulk, 5));
        assert_eq!(bulk.labor_advanced, 37, "a finished project gained labor");

        // Saturating: an oversized contribution clamps rather than wraps.
        let mut huge = start_project(&template, &mut Stock::new(8), ProjectId(2), Tick(0)).unwrap();
        huge.labor_advanced = u32::MAX - 1;
        assert!(advance_project_by(&mut huge, u32::MAX));
        assert_eq!(huge.labor_advanced, u32::MAX);
    }

    #[test]
    fn road_template_drives_the_labor_lifecycle_and_creates_no_good() {
        // A pure-labor public-works project: it accumulates contributed labor and
        // completes at the cost, producing NO good (output_qty 0 → completion adds
        // nothing). The reused project-labor path the G7 road runs on.
        let template = build_road_template(WOOD, 3);
        assert_eq!(template.id, ProjectTemplateId::BuildRoad);
        let mut fund = Stock::new(8);
        let mut project = start_project(&template, &mut fund, ProjectId(1), Tick(0))
            .expect("no inputs to commit");
        assert_eq!(project.state, ProjectState::Forming);

        // Under the cost the project does not complete...
        for _ in 0..2 {
            assert!(advance_project(&mut project));
            assert!(!complete_project_if_ready(
                &mut project,
                &template,
                &mut fund
            ));
        }
        assert_eq!(project.labor_advanced, 2);
        assert_eq!(project.state, ProjectState::Forming);

        // ...the labor unit that meets the cost completes it, and no good is created.
        assert!(advance_project(&mut project));
        assert!(complete_project_if_ready(
            &mut project,
            &template,
            &mut fund
        ));
        assert_eq!(project.state, ProjectState::Complete);
        assert_eq!(fund.get(WOOD), 0, "a road creates no good on completion");

        // One-way: a completed project never advances again.
        assert!(!advance_project(&mut project));
    }

    #[test]
    fn road_template_is_game_only_not_a_builtin() {
        // The lab planner only ever sees BuildNet; the road template is `sim`-only,
        // so the conformance goldens stay byte-identical.
        assert!(builtin_project_templates()
            .iter()
            .all(|t| t.id == ProjectTemplateId::BuildNet));
    }

    #[test]
    fn tool_templates_are_game_only_not_builtins() {
        // S7: the mill/oven capital-build templates are driven by the `sim` per-agent
        // capital-formation phase, never the lab planner, so they must stay OUT of the
        // builtin set — the conformance goldens (produced_of(mill)==0 in the lab
        // scenarios) stay byte-identical only because no lab project ever mints a tool.
        assert!(builtin_project_templates()
            .iter()
            .all(|t| t.id == ProjectTemplateId::BuildNet));
    }

    #[test]
    fn tool_template_mints_the_tool_consuming_saved_wood() {
        // S7 conservation contract in miniature: a mill build commits its WOOD up front
        // (the sim phase books it to consumed_as_input there), advances over several
        // ticks of labor, and on completion mints exactly one durable tool (the sim
        // phase books it to produced). salvage_bps is 0: the input is fully committed.
        let mill = GoodId(42);
        let template = build_mill_template(mill, 5, 3);
        assert_eq!(template.id, ProjectTemplateId::BuildMill);
        assert_eq!(template.input_goods, vec![(WOOD, 5)]);
        assert_eq!(template.output_good, mill);
        assert_eq!(template.output_qty, 1);
        assert_eq!(template.salvage_bps, 0);

        let mut stock = Stock::new(WOOD.0);
        stock.add(WOOD, 5);
        let mut project = start_project(&template, &mut stock, ProjectId(1), Tick(0))
            .expect("the saved WOOD funds the build");
        // start_project committed the WOOD up front (the consumed_as_input the sim
        // phase books at the start tick); no tool exists yet.
        assert_eq!(stock.get(WOOD), 0);
        assert_eq!(stock.get(mill), 0);

        // Under the labor cost the build does not complete...
        for _ in 0..2 {
            assert!(advance_project(&mut project));
            assert!(!complete_project_if_ready(
                &mut project,
                &template,
                &mut stock
            ));
            assert_eq!(stock.get(mill), 0);
        }
        // ...the labor unit that meets the cost mints exactly one mill.
        assert!(advance_project(&mut project));
        assert!(complete_project_if_ready(
            &mut project,
            &template,
            &mut stock
        ));
        assert_eq!(project.state, ProjectState::Complete);
        assert_eq!(
            stock.get(mill),
            1,
            "the completed build mints one durable tool"
        );

        // The oven twin mints its own tool from its own WOOD.
        let oven = GoodId(43);
        let oven_template = build_oven_template(oven, 4, 1);
        assert_eq!(oven_template.id, ProjectTemplateId::BuildOven);
        let mut oven_stock = Stock::new(oven.0);
        oven_stock.add(WOOD, 4);
        let mut oven_project =
            start_project(&oven_template, &mut oven_stock, ProjectId(2), Tick(0))
                .expect("the saved WOOD funds the oven build");
        assert!(advance_project(&mut oven_project));
        assert!(complete_project_if_ready(
            &mut oven_project,
            &oven_template,
            &mut oven_stock
        ));
        assert_eq!(oven_stock.get(oven), 1);
    }

    #[test]
    fn abandoned_tool_build_forfeits_its_committed_wood() {
        // salvage_bps 0: abandoning a partial tool build returns no WOOD — the
        // committed input is forfeit (already consumed at the start tick), so there is
        // no work-in-progress source to account and the sim build conserves end-to-end.
        let mill = GoodId(42);
        let template = build_mill_template(mill, 6, 4);
        let mut stock = Stock::new(mill.0);
        stock.add(WOOD, 6);
        let mut project = start_project(&template, &mut stock, ProjectId(1), Tick(0)).unwrap();
        advance_project(&mut project);
        let loss = abandon_project(&mut project, &mut stock);
        assert_eq!(stock.get(WOOD), 0, "no salvage is returned");
        assert_eq!(loss.goods_consumed, 6);
        assert_eq!(loss.labor_consumed, 1);
    }

    #[test]
    fn capital_project_requires_saved_inputs() {
        let template = build_net_template();
        let mut stock = Stock::new(3);
        stock.add(WOOD, 1);

        let project = start_project(&template, &mut stock, ProjectId(1), Tick(0));

        assert!(project.is_none());
        assert_eq!(stock.get(WOOD), 1);
    }

    #[test]
    fn abandonment_returns_only_salvage() {
        let template = build_net_template();
        let mut stock = Stock::new(3);
        stock.add(WOOD, 2);
        let mut project = start_project(&template, &mut stock, ProjectId(1), Tick(0)).unwrap();
        project.labor_advanced = 2;

        let loss = abandon_project(&mut project, &mut stock);

        assert_eq!(stock.get(WOOD), 1);
        assert_eq!(
            loss,
            CapitalLoss {
                labor_consumed: 2,
                goods_consumed: 1,
            }
        );
    }

    #[test]
    fn abandonment_clamps_invalid_salvage() {
        let template = build_net_template();
        let mut stock = Stock::new(3);
        stock.add(WOOD, 2);
        let mut project = start_project(&template, &mut stock, ProjectId(1), Tick(0)).unwrap();
        project.salvage_bps = 20_000;

        let loss = abandon_project(&mut project, &mut stock);

        assert_eq!(stock.get(WOOD), 2);
        assert_eq!(loss.goods_consumed, 0);
    }

    #[test]
    fn abandonment_salvage_aggregates_duplicate_committed_inputs() {
        let mut template = build_net_template();
        template.input_goods = vec![(WOOD, 1), (WOOD, 1)];
        let mut stock = Stock::new(3);
        stock.add(WOOD, 2);
        let mut project = start_project(&template, &mut stock, ProjectId(1), Tick(0)).unwrap();

        let loss = abandon_project(&mut project, &mut stock);

        assert_eq!(stock.get(WOOD), 1);
        assert_eq!(loss.goods_consumed, 1);
    }

    #[test]
    fn duplicate_project_inputs_are_checked_cumulatively() {
        let mut template = build_net_template();
        template.input_goods = vec![(WOOD, 2), (WOOD, 2)];
        let mut stock = Stock::new(3);
        stock.add(WOOD, 2);

        let project = start_project(&template, &mut stock, ProjectId(1), Tick(0));

        assert!(project.is_none());
        assert_eq!(stock.get(WOOD), 2);
    }
}
