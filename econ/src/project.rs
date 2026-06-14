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
    if project.state != ProjectState::Forming {
        return false;
    }
    project.labor_advanced += 1;
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
    use super::{abandon_project, build_net_template, start_project, CapitalLoss, ProjectId, Tick};
    use crate::good::{Stock, WOOD};

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
