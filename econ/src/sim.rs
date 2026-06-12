//! The world, tick loop, and ordinal action planner.

use crate::agent::{Agent, TickProvisions, WantKind};
use crate::good::{GoodId, Horizon, FOOD, NET, WOOD};
use crate::project::{
    abandon_project, advance_project, complete_project_if_ready, find_template, recipe_reach,
    start_project, Project, ProjectId, ProjectState, ProjectTemplate, ProjectTemplateId, Reach,
    Recipe, RecipeId, Tick,
};
use crate::record::Record;
use crate::rng::Rng;
use crate::scenario::{Event, EventKind, Scenario};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateAction {
    Rest,
    GatherFood,
    CutWoodForProject(ProjectTemplateId),
    StartProject(ProjectTemplateId),
    AdvanceProject(ProjectId),
    FishWithNet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankedCandidate {
    pub action: CandidateAction,
    pub rank: usize,
    pub tie_break: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectRecipeAction {
    GatherFood,
    CutWood,
    FishWithNet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirectRecipeCandidate {
    pub(crate) action: DirectRecipeAction,
    pub(crate) rank: usize,
    pub(crate) tie_break: u8,
}

const TIE_DIRECT_CAPITAL_OUTPUT: u8 = 0;
const TIE_DIRECT_HAND_OUTPUT: u8 = 1;
const TIE_CONTINUE_CAPITAL: u8 = 2;
const TIE_START_CAPITAL: u8 = 3;
const TIE_PREREQUISITE_INPUT: u8 = 4;
const TIE_REST: u8 = 5;

#[derive(Clone, Debug, Default)]
struct StepStats {
    labor_used: u32,
    leisure_taken: u32,
    food_consumed: u32,
    hunger_deficit: u32,
    completed_projects: u32,
    abandoned_projects: u32,
    capital_labor_consumed: u32,
    capital_goods_consumed: u32,
    gather_actions: u32,
    cut_wood_actions: u32,
    fish_actions: u32,
    project_actions: u32,
}

pub struct World {
    pub tick: Tick,
    pub agent: Agent,
    pub recipes: Vec<Recipe>,
    pub project_templates: Vec<ProjectTemplate>,
    pub projects: Vec<Project>,
    pub records: Vec<Record>,
    #[allow(dead_code)]
    rng: Rng,
    events: Vec<Event>,
    next_project_id: Option<u32>,
}

impl World {
    pub fn from_scenario(scenario: Scenario) -> Self {
        let next_project_id = next_project_id_after_existing(&scenario.initial_projects);

        Self {
            tick: Tick(0),
            agent: scenario.agent,
            recipes: scenario.recipes,
            project_templates: scenario.project_templates,
            projects: scenario.initial_projects,
            records: Vec::new(),
            rng: Rng::new(scenario.seed),
            events: scenario.events,
            next_project_id,
        }
    }

    pub fn run(&mut self, periods: u64) {
        for _ in 0..periods {
            self.step();
        }
    }

    pub fn step(&mut self) {
        let mut stats = StepStats::default();

        self.apply_events();
        // Satisfaction is tick-local. Clear the previous tick before
        // abandonment, which compares forming projects against current leisure.
        self.clear_tick_satisfaction();
        self.abandon_unvalued_projects(&mut stats);

        // `recompute_satisfaction` deliberately clears stale flags before
        // consumption. Its pre-consumption `Next` marks are recomputed below
        // after `Now` food has been consumed.
        self.agent.recompute_satisfaction();
        let (consumption, mut provisions) = self.agent.consume_now_wants_with_provisions();
        stats.food_consumed = consumption.food_consumed;
        stats.hunger_deficit = consumption.hunger_deficit;
        self.complete_ready_projects(&mut stats);
        self.agent
            .recompute_satisfaction_with_provisions(&provisions);

        self.allocate_labor(&mut stats, &mut provisions);
        self.complete_ready_projects(&mut stats);
        self.agent
            .recompute_satisfaction_with_provisions(&provisions);
        self.records.push(self.build_record(&stats));
        self.prune_inactive_projects();
        self.tick.0 += 1;
    }

    fn apply_events(&mut self) {
        let tick = self.tick;
        let mut index = 0;
        while index < self.events.len() {
            if self.events[index].tick <= tick {
                let event = self.events.remove(index);
                self.apply_event_kind(event.kind);
            } else {
                index += 1;
            }
        }
    }

    fn apply_event_kind(&mut self, kind: EventKind) {
        match kind {
            EventKind::DisableRecipe(recipe_id) => {
                if let Some(recipe) = self
                    .recipes
                    .iter_mut()
                    .find(|recipe| recipe.id == recipe_id)
                {
                    recipe.enabled = false;
                }
            }
            EventKind::SetReserveRatio { .. }
            | EventKind::SetBankConvertibility { .. }
            | EventKind::SetBankCreditPolicy { .. }
            | EventKind::StopBankCredit { .. }
            | EventKind::RedeemDemandClaims { .. }
            | EventKind::SetRegime(_)
            | EventKind::ResetPublicSpotBook
            | EventKind::SetPublicSpotTender(_)
            | EventKind::SetPublicDebtTender(_)
            | EventKind::SetBankRepaymentTender(_)
            | EventKind::SetIssuerRepaymentTender(_)
            | EventKind::SetLaborWageTender(_)
            | EventKind::SetDebtDueTick { .. }
            | EventKind::SeedCommodityDebt { .. }
            | EventKind::SeedStock { .. }
            | EventKind::FiatPrint { .. }
            | EventKind::SetIssuerPolicy { .. }
            | EventKind::StopIssuerCredit { .. } => {}
            EventKind::SetTaxReceivability(_) | EventKind::LevyTax { .. } => {
                // Tax receivability is an M3-only policy surface; M0 keeps the
                // events shadow-preserved but intentionally inert.
            }
        }
    }

    fn clear_tick_satisfaction(&mut self) {
        for want in &mut self.agent.scale {
            want.satisfied = false;
        }
    }

    fn abandon_unvalued_projects(&mut self, stats: &mut StepStats) {
        let leisure_rank = self.agent.first_unsatisfied_leisure_rank();

        for project in &mut self.projects {
            if project.state != ProjectState::Forming {
                continue;
            }

            let rank = instrumental_rank_for_project(&self.agent, &self.recipes, project);
            let should_abandon = match rank {
                None => true,
                Some(work_rank) => leisure_rank
                    .map(|rest_rank| work_rank > rest_rank)
                    .unwrap_or(false),
            };

            if should_abandon {
                let loss = abandon_project(project, &mut self.agent.stock);
                stats.abandoned_projects += 1;
                stats.capital_labor_consumed += loss.labor_consumed;
                stats.capital_goods_consumed += loss.goods_consumed;
            }
        }
    }

    fn allocate_labor(&mut self, stats: &mut StepStats, provisions: &mut TickProvisions) {
        let mut remaining = self.agent.labor_capacity;

        while remaining > 0 {
            self.agent
                .recompute_satisfaction_with_provisions(provisions);
            let rest_candidate = self.rest_candidate();
            let mut worked = false;
            let mut rest_wins = false;

            for candidate in self.work_candidates(remaining, provisions) {
                if rest_candidate
                    .as_ref()
                    .map(|rest| candidate.rank >= rest.rank)
                    .unwrap_or(false)
                {
                    rest_wins = true;
                    break;
                }

                if let Some(labor_used) = self.execute(candidate, stats, provisions) {
                    stats.labor_used += labor_used;
                    remaining -= labor_used;
                    worked = true;
                    break;
                }
            }

            if rest_wins || !worked {
                self.rest_for_remaining(rest_candidate, remaining, stats, provisions);
                break;
            }
        }
    }

    fn rest_candidate(&self) -> Option<RankedCandidate> {
        self.agent
            .first_unsatisfied_leisure_rank()
            .map(|rank| RankedCandidate {
                action: CandidateAction::Rest,
                rank,
                tie_break: TIE_REST,
            })
    }

    fn rest_for_remaining(
        &mut self,
        rest_candidate: Option<RankedCandidate>,
        remaining: u32,
        stats: &mut StepStats,
        provisions: &mut TickProvisions,
    ) {
        if matches!(
            rest_candidate,
            Some(RankedCandidate {
                action: CandidateAction::Rest,
                ..
            })
        ) {
            self.take_leisure(remaining, provisions);
        }
        stats.leisure_taken += remaining;
    }

    fn work_candidates(
        &self,
        remaining_labor: u32,
        provisions: &TickProvisions,
    ) -> Vec<RankedCandidate> {
        let mut candidates = Vec::new();

        for candidate in direct_recipe_candidates(
            &self.agent,
            &self.recipes,
            remaining_labor,
            provisions,
            false,
        ) {
            let action = match candidate.action {
                DirectRecipeAction::GatherFood => CandidateAction::GatherFood,
                DirectRecipeAction::FishWithNet => CandidateAction::FishWithNet,
                DirectRecipeAction::CutWood => continue,
            };
            candidates.push(RankedCandidate {
                action,
                rank: candidate.rank,
                tie_break: candidate.tie_break,
            });
        }

        for project in &self.projects {
            if project.state != ProjectState::Forming {
                continue;
            }
            let Some(rank) = instrumental_rank_for_project(&self.agent, &self.recipes, project)
            else {
                continue;
            };
            let Some(template) = find_template(&self.project_templates, project.template) else {
                continue;
            };
            if project.labor_advanced >= template.required_labor {
                continue;
            }
            candidates.push(RankedCandidate {
                action: CandidateAction::AdvanceProject(project.id),
                rank,
                tie_break: TIE_CONTINUE_CAPITAL,
            });
        }

        if self.next_project_id.is_some() {
            for template in &self.project_templates {
                if self.has_forming_project(template.id)
                    || self
                        .agent
                        .stock
                        .can_remove(template.output_good, template.output_qty)
                {
                    continue;
                }
                let Some(rank) =
                    instrumental_rank_for_template(&self.agent, &self.recipes, template)
                else {
                    continue;
                };
                if has_required_inputs_for_rank(&self.agent, template, provisions, rank) {
                    candidates.push(RankedCandidate {
                        action: CandidateAction::StartProject(template.id),
                        rank,
                        tie_break: TIE_START_CAPITAL,
                    });
                } else if self
                    .recipe_can_run_base(RecipeId::CutWood, remaining_labor)
                    .filter(|recipe| {
                        template_needs_input_good_for_rank(
                            &self.agent,
                            template,
                            provisions,
                            rank,
                            recipe.output_good,
                        ) && recipe_inputs_available_for_rank(
                            &self.agent,
                            recipe,
                            provisions,
                            rank,
                            None,
                        )
                    })
                    .is_some()
                {
                    candidates.push(RankedCandidate {
                        action: CandidateAction::CutWoodForProject(template.id),
                        rank,
                        tie_break: TIE_PREREQUISITE_INPUT,
                    });
                }
            }
        }

        candidates.sort_by_key(|candidate| (candidate.rank, candidate.tie_break));
        candidates
    }

    fn execute(
        &mut self,
        candidate: RankedCandidate,
        stats: &mut StepStats,
        provisions: &mut TickProvisions,
    ) -> Option<u32> {
        match candidate.action {
            CandidateAction::Rest => None,
            CandidateAction::GatherFood => {
                let labor =
                    self.execute_recipe(RecipeId::GatherFood, candidate.rank, provisions, true)?;
                stats.gather_actions += 1;
                Some(labor)
            }
            CandidateAction::CutWoodForProject(template_id) => {
                let template = find_template(&self.project_templates, template_id)?;
                let output_good = self
                    .recipes
                    .iter()
                    .find(|recipe| recipe.id == RecipeId::CutWood)
                    .map(|recipe| recipe.output_good)?;
                if !template_needs_input_good_for_rank(
                    &self.agent,
                    template,
                    provisions,
                    candidate.rank,
                    output_good,
                ) {
                    return None;
                }
                let labor =
                    self.execute_recipe(RecipeId::CutWood, candidate.rank, provisions, false)?;
                stats.cut_wood_actions += 1;
                Some(labor)
            }
            CandidateAction::StartProject(template_id) => {
                let template = find_template(&self.project_templates, template_id)?;
                if self.has_forming_project(template_id)
                    || self
                        .agent
                        .stock
                        .can_remove(template.output_good, template.output_qty)
                {
                    return None;
                }
                if !has_required_inputs_for_rank(&self.agent, template, provisions, candidate.rank)
                {
                    return None;
                }
                let id = ProjectId(self.next_project_id?);
                let mut project = start_project(template, &mut self.agent.stock, id, self.tick)?;
                advance_project(&mut project);
                stats.project_actions += 1;
                self.projects.push(project);
                self.next_project_id = id.0.checked_add(1);
                self.complete_ready_projects(stats);
                Some(1)
            }
            CandidateAction::AdvanceProject(project_id) => {
                let project = self
                    .projects
                    .iter_mut()
                    .find(|project| project.id == project_id)?;
                if !advance_project(project) {
                    return None;
                }
                stats.project_actions += 1;
                self.complete_ready_projects(stats);
                Some(1)
            }
            CandidateAction::FishWithNet => {
                let labor =
                    self.execute_recipe(RecipeId::FishWithNet, candidate.rank, provisions, true)?;
                stats.fish_actions += 1;
                Some(labor)
            }
        }
    }

    fn execute_recipe(
        &mut self,
        recipe_id: RecipeId,
        rank: usize,
        provisions: &mut TickProvisions,
        provide_to_wants: bool,
    ) -> Option<u32> {
        let recipe = self
            .recipes
            .iter()
            .find(|recipe| recipe.id == recipe_id)
            .cloned()?;
        if !recipe.enabled {
            return None;
        }
        if let Some(tool) = recipe.required_tool {
            if !self.agent.stock.can_remove(tool, 1) {
                return None;
            }
        }
        if let Some((good, qty)) = recipe.input_good {
            if available_after_current_provisions_before_rank(&self.agent, provisions, good, rank)
                < qty
            {
                return None;
            }
            if !self.agent.stock.remove(good, qty) {
                return None;
            }
        }

        self.agent.stock.add(recipe.output_good, recipe.output_qty);
        if provide_to_wants {
            provide_output(
                &mut self.agent,
                recipe.output_good,
                recipe.output_qty,
                recipe_reach(&recipe),
                provisions,
            );
        }
        Some(recipe.labor)
    }

    fn take_leisure(&mut self, mut hours: u32, provisions: &mut TickProvisions) {
        for (index, want) in self.agent.scale.iter_mut().enumerate() {
            if hours == 0 {
                break;
            }
            if want.satisfied || want.kind != WantKind::Leisure {
                continue;
            }

            let needed = provisions.remaining_for(index, want.qty);
            if needed == 0 {
                want.satisfied = true;
                provisions.mark(index);
                continue;
            }

            let used = hours.min(needed);
            provisions.allocate(index, used);
            hours -= used;
            if used < needed {
                break;
            }
            want.satisfied = true;
            provisions.mark(index);
        }
    }

    fn complete_ready_projects(&mut self, stats: &mut StepStats) {
        for project in &mut self.projects {
            if project.state != ProjectState::Forming {
                continue;
            }
            let Some(template) = find_template(&self.project_templates, project.template) else {
                continue;
            };
            if complete_project_if_ready(project, template, &mut self.agent.stock) {
                stats.completed_projects += 1;
            }
        }
    }

    fn build_record(&self, stats: &StepStats) -> Record {
        Record {
            tick: self.tick.0,
            food: self.agent.stock.get(FOOD),
            wood: self.agent.stock.get(WOOD),
            nets: self.agent.stock.get(NET),
            labor_used: stats.labor_used,
            leisure_taken: stats.leisure_taken,
            food_consumed: stats.food_consumed,
            hunger_deficit: stats.hunger_deficit,
            active_projects: self
                .projects
                .iter()
                .filter(|project| project.state == ProjectState::Forming)
                .count() as u32,
            completed_projects: stats.completed_projects,
            abandoned_projects: stats.abandoned_projects,
            capital_labor_consumed: stats.capital_labor_consumed,
            capital_goods_consumed: stats.capital_goods_consumed,
            gather_actions: stats.gather_actions,
            cut_wood_actions: stats.cut_wood_actions,
            fish_actions: stats.fish_actions,
            project_actions: stats.project_actions,
        }
    }

    fn recipe_can_run_base(&self, recipe_id: RecipeId, remaining_labor: u32) -> Option<&Recipe> {
        recipe_can_run_base_for(&self.agent, &self.recipes, recipe_id, remaining_labor)
    }

    fn has_forming_project(&self, template_id: ProjectTemplateId) -> bool {
        self.projects.iter().any(|project| {
            project.state == ProjectState::Forming && project.template == template_id
        })
    }

    fn prune_inactive_projects(&mut self) {
        self.projects
            .retain(|project| project.state == ProjectState::Forming);
    }
}

pub(crate) fn direct_recipe_candidates(
    agent: &Agent,
    recipes: &[Recipe],
    remaining_labor: u32,
    provisions: &TickProvisions,
    include_cut_wood: bool,
) -> Vec<DirectRecipeCandidate> {
    direct_recipe_candidates_with_options(
        agent,
        recipes,
        remaining_labor,
        provisions,
        include_cut_wood,
        None,
        None,
    )
}

pub(crate) fn direct_recipe_candidates_for_money(
    agent: &Agent,
    recipes: &[Recipe],
    remaining_labor: u32,
    provisions: &TickProvisions,
    include_cut_wood: bool,
    money_good: GoodId,
) -> Vec<DirectRecipeCandidate> {
    direct_recipe_candidates_with_options(
        agent,
        recipes,
        remaining_labor,
        provisions,
        include_cut_wood,
        Some(money_good),
        None,
    )
}

pub(crate) fn direct_recipe_candidates_excluding_good(
    agent: &Agent,
    recipes: &[Recipe],
    remaining_labor: u32,
    provisions: &TickProvisions,
    include_cut_wood: bool,
    excluded_good: GoodId,
) -> Vec<DirectRecipeCandidate> {
    direct_recipe_candidates_with_options(
        agent,
        recipes,
        remaining_labor,
        provisions,
        include_cut_wood,
        None,
        Some(excluded_good),
    )
}

fn direct_recipe_candidates_with_options(
    agent: &Agent,
    recipes: &[Recipe],
    remaining_labor: u32,
    provisions: &TickProvisions,
    include_cut_wood: bool,
    money_good: Option<GoodId>,
    excluded_good: Option<GoodId>,
) -> Vec<DirectRecipeCandidate> {
    let mut candidates = Vec::new();
    for (recipe_id, action, tie_break) in [
        (
            RecipeId::GatherFood,
            DirectRecipeAction::GatherFood,
            TIE_DIRECT_HAND_OUTPUT,
        ),
        (
            RecipeId::FishWithNet,
            DirectRecipeAction::FishWithNet,
            TIE_DIRECT_CAPITAL_OUTPUT,
        ),
        (
            RecipeId::CutWood,
            DirectRecipeAction::CutWood,
            TIE_DIRECT_HAND_OUTPUT,
        ),
    ] {
        if !include_cut_wood && recipe_id == RecipeId::CutWood {
            continue;
        }
        let Some(recipe) = recipe_can_run_base_for(agent, recipes, recipe_id, remaining_labor)
        else {
            continue;
        };
        if recipe_touches_excluded_good(recipe, excluded_good) {
            continue;
        }
        let reach = if money_good == Some(recipe.output_good) {
            Reach::Distant
        } else {
            recipe_reach(recipe)
        };
        if let Some(rank) = first_provisionable_good_in_reach(
            agent,
            recipe.output_good,
            reach,
            recipe.output_qty,
            provisions,
        )
        .filter(|rank| {
            recipe_inputs_available_for_rank(agent, recipe, provisions, *rank, money_good)
        }) {
            candidates.push(DirectRecipeCandidate {
                action,
                rank,
                tie_break,
            });
        }
    }
    candidates.sort_by_key(|candidate| (candidate.rank, candidate.tie_break));
    candidates
}

pub(crate) fn execute_direct_recipe_for_agent(
    agent: &mut Agent,
    recipes: &[Recipe],
    recipe_id: RecipeId,
    remaining_labor: u32,
    rank: usize,
    provisions: &mut TickProvisions,
) -> Option<u32> {
    execute_direct_recipe_for_agent_with_money(
        agent,
        recipes,
        recipe_id,
        remaining_labor,
        rank,
        provisions,
        None,
    )
}

pub(crate) fn execute_direct_recipe_for_agent_for_money(
    agent: &mut Agent,
    recipes: &[Recipe],
    recipe_id: RecipeId,
    remaining_labor: u32,
    rank: usize,
    provisions: &mut TickProvisions,
    money_good: GoodId,
) -> Option<u32> {
    execute_direct_recipe_for_agent_with_money(
        agent,
        recipes,
        recipe_id,
        remaining_labor,
        rank,
        provisions,
        Some(money_good),
    )
}

fn execute_direct_recipe_for_agent_with_money(
    agent: &mut Agent,
    recipes: &[Recipe],
    recipe_id: RecipeId,
    remaining_labor: u32,
    rank: usize,
    provisions: &mut TickProvisions,
    _money_good: Option<GoodId>,
) -> Option<u32> {
    let recipe = recipe_can_run_base_for(agent, recipes, recipe_id, remaining_labor)?.clone();
    if let Some((good, qty)) = recipe.input_good {
        if available_after_current_provisions_before_rank(agent, provisions, good, rank) < qty {
            return None;
        }
        if !agent.stock.remove(good, qty) {
            return None;
        }
    }

    let reach = agent
        .scale
        .get(rank)
        .filter(|want| {
            want.kind == WantKind::Good(recipe.output_good)
                && matches!(want.horizon, Horizon::Later(_))
        })
        .map(|_| Reach::Distant)
        .unwrap_or_else(|| recipe_reach(&recipe));

    agent.stock.add(recipe.output_good, recipe.output_qty);
    provide_output(
        agent,
        recipe.output_good,
        recipe.output_qty,
        reach,
        provisions,
    );
    Some(recipe.labor)
}

fn recipe_can_run_base_for<'a>(
    agent: &Agent,
    recipes: &'a [Recipe],
    recipe_id: RecipeId,
    remaining_labor: u32,
) -> Option<&'a Recipe> {
    let recipe = recipes.iter().find(|recipe| recipe.id == recipe_id)?;
    if !recipe.enabled || recipe.labor == 0 || recipe.labor > remaining_labor {
        return None;
    }
    if let Some(tool) = recipe.required_tool {
        if !agent.stock.can_remove(tool, 1) {
            return None;
        }
    }
    Some(recipe)
}

fn recipe_touches_excluded_good(recipe: &Recipe, excluded_good: Option<GoodId>) -> bool {
    let Some(excluded_good) = excluded_good else {
        return false;
    };
    recipe.output_good == excluded_good
        || recipe
            .input_good
            .is_some_and(|(good, _)| good == excluded_good)
        || recipe.required_tool == Some(excluded_good)
}

pub fn instrumental_rank_for_net(agent: &Agent, recipes: &[Recipe]) -> Option<usize> {
    if agent.stock.can_remove(NET, 1) {
        return None;
    }
    instrumental_rank_for_tool(agent, recipes, NET)
}

fn instrumental_rank_for_project(
    agent: &Agent,
    recipes: &[Recipe],
    project: &Project,
) -> Option<usize> {
    if project.output_qty == 0
        || agent
            .stock
            .can_remove(project.output_good, project.output_qty)
    {
        return None;
    }
    instrumental_rank_for_tool(agent, recipes, project.output_good)
}

fn instrumental_rank_for_template(
    agent: &Agent,
    recipes: &[Recipe],
    template: &ProjectTemplate,
) -> Option<usize> {
    if template.output_qty == 0
        || agent
            .stock
            .can_remove(template.output_good, template.output_qty)
    {
        return None;
    }
    instrumental_rank_for_tool(agent, recipes, template.output_good)
}

fn instrumental_rank_for_tool(agent: &Agent, recipes: &[Recipe], tool: GoodId) -> Option<usize> {
    let recipe = capital_using_recipe_for_tool(recipes, tool)?;
    agent.scale.iter().position(|want| {
        !want.satisfied
            && want.kind == WantKind::Good(recipe.output_good)
            && matches!(want.horizon, Horizon::Later(_))
    })
}

fn capital_using_recipe_for_tool(recipes: &[Recipe], tool: GoodId) -> Option<&Recipe> {
    recipes.iter().find(|recipe| {
        recipe.enabled
            && recipe.required_tool == Some(tool)
            && recipe.output_qty > 0
            && recipe_reach(recipe) == Reach::Distant
    })
}

fn first_provisionable_good_in_reach(
    agent: &Agent,
    good: GoodId,
    reach: Reach,
    output_qty: u32,
    provisions: &TickProvisions,
) -> Option<usize> {
    if output_qty == 0 {
        return None;
    }

    for (index, want) in agent.scale.iter().enumerate() {
        if want.satisfied || want.kind != WantKind::Good(good) {
            continue;
        }
        if provisions.provided.get(index).copied().unwrap_or(false) {
            continue;
        }
        let in_reach = match reach {
            Reach::NearTerm => matches!(want.horizon, Horizon::Now | Horizon::Next),
            Reach::Distant => true,
        };
        if in_reach && !provisions.is_fully_allocated(index, want.qty) {
            return Some(index);
        }
    }
    None
}

fn available_after_current_provisions_before_rank(
    agent: &Agent,
    provisions: &TickProvisions,
    good: GoodId,
    before_rank: usize,
) -> u32 {
    let mut available = agent
        .stock
        .get(good)
        .saturating_sub(reserved_qty(&provisions.reserved, good));
    let mut blocked = false;
    for (index, want) in agent.scale.iter().enumerate() {
        if index >= before_rank {
            break;
        }
        if provisions.provided.get(index).copied().unwrap_or(false) {
            continue;
        }
        if let WantKind::Good(want_good) = want.kind {
            if want_good != good || !matches!(want.horizon, Horizon::Next) || blocked {
                continue;
            }
            let needed = provisions.remaining_for(index, want.qty);
            if needed == 0 {
                continue;
            }
            if available >= needed {
                available -= needed;
            } else {
                available = 0;
                blocked = true;
            }
        }
    }

    available
}

fn recipe_inputs_available_for_rank(
    agent: &Agent,
    recipe: &Recipe,
    provisions: &TickProvisions,
    rank: usize,
    _money_good: Option<GoodId>,
) -> bool {
    let Some((good, qty)) = recipe.input_good else {
        return true;
    };
    available_after_current_provisions_before_rank(agent, provisions, good, rank) >= qty
}

fn reserved_qty(reservations: &[(GoodId, u32)], good: GoodId) -> u32 {
    reservations
        .iter()
        .filter(|(reserved_good, _)| *reserved_good == good)
        .map(|(_, qty)| *qty)
        .sum()
}

fn reserve_qty(reservations: &mut Vec<(GoodId, u32)>, good: GoodId, qty: u32) {
    if qty == 0 {
        return;
    }
    if let Some((_, reserved)) = reservations
        .iter_mut()
        .find(|(reserved_good, _)| *reserved_good == good)
    {
        *reserved = reserved.saturating_add(qty);
    } else {
        reservations.push((good, qty));
    }
}

fn provide_output(
    agent: &mut Agent,
    good: GoodId,
    qty: u32,
    reach: Reach,
    provisions: &mut TickProvisions,
) {
    let mut remaining = qty;
    let mut allocated = 0;
    for (index, want) in agent.scale.iter_mut().enumerate() {
        if remaining == 0 {
            break;
        }
        if want.satisfied || want.kind != WantKind::Good(good) {
            continue;
        }
        if provisions.provided.get(index).copied().unwrap_or(false) {
            continue;
        }

        let in_reach = match reach {
            Reach::NearTerm => matches!(want.horizon, Horizon::Now | Horizon::Next),
            Reach::Distant => true,
        };
        if in_reach {
            let needed = provisions.remaining_for(index, want.qty);
            if needed == 0 {
                continue;
            }
            let used = remaining.min(needed);
            provisions.allocate(index, used);
            remaining -= used;
            allocated += used;

            if !provisions.is_fully_allocated(index, want.qty) {
                break;
            }
            if matches!(want.horizon, Horizon::Now) {
                continue;
            } else {
                want.satisfied = true;
                provisions.mark(index);
            }
        }
    }
    if allocated > 0 {
        provisions.reserve(good, allocated);
    }
}

fn has_required_inputs_for_rank(
    agent: &Agent,
    template: &ProjectTemplate,
    provisions: &TickProvisions,
    rank: usize,
) -> bool {
    let mut required = Vec::new();
    for (good, qty) in &template.input_goods {
        reserve_qty(&mut required, *good, *qty);
    }

    for (good, qty) in required {
        if available_after_current_provisions_before_rank(agent, provisions, good, rank) < qty {
            return false;
        }
    }
    true
}

fn template_needs_input_good_for_rank(
    agent: &Agent,
    template: &ProjectTemplate,
    provisions: &TickProvisions,
    rank: usize,
    good: GoodId,
) -> bool {
    let required = template
        .input_goods
        .iter()
        .filter(|(input_good, _)| *input_good == good)
        .map(|(_, qty)| *qty)
        .sum::<u32>();
    required > 0
        && available_after_current_provisions_before_rank(agent, provisions, good, rank) < required
}

fn next_project_id_after_existing(projects: &[Project]) -> Option<u32> {
    projects
        .iter()
        .map(|project| project.id.0)
        .max()
        .map_or(Some(1), |id| id.checked_add(1))
}

#[cfg(test)]
mod tests {
    use super::World;
    use crate::agent::{Agent, AgentId, Role, Want, WantKind};
    use crate::good::{Gold, Horizon, Stock, FOOD, NET, WOOD};
    use crate::project::{
        build_net_template, builtin_project_templates, builtin_recipes, Project, ProjectId,
        ProjectState, RecipeId, Tick,
    };
    use crate::scenario::{Event, EventKind, Scenario};

    #[test]
    fn direct_recipe_execution_can_provision_selected_later_output() {
        let mut agent = Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Later(1),
                qty: 1,
                satisfied: false,
            }],
            stock: Stock::new(3),
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: Gold::ZERO,
            roles: vec![Role::Household],
            expect: Vec::new(),
        };
        let recipes = builtin_recipes();
        let (_, mut provisions) = agent.consume_now_wants_with_provisions();

        let labor = super::execute_direct_recipe_for_agent(
            &mut agent,
            &recipes,
            RecipeId::GatherFood,
            1,
            0,
            &mut provisions,
        );

        assert_eq!(labor, Some(1));
        assert!(provisions.provided[0]);
        assert!(agent.scale[0].satisfied);
    }

    #[test]
    fn direct_recipe_execution_respects_remaining_labor() {
        let mut agent = Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Now,
                qty: 1,
                satisfied: false,
            }],
            stock: Stock::new(3),
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: Gold::ZERO,
            roles: vec![Role::Household],
            expect: Vec::new(),
        };
        let recipes = builtin_recipes();
        let (_, mut provisions) = agent.consume_now_wants_with_provisions();

        let labor = super::execute_direct_recipe_for_agent(
            &mut agent,
            &recipes,
            RecipeId::GatherFood,
            0,
            0,
            &mut provisions,
        );

        assert_eq!(labor, None);
        assert_eq!(agent.stock.get(FOOD), 0);
        assert!(!provisions.provided[0]);
    }

    #[test]
    fn labor_stops_when_leisure_outranks_work() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 1);
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Leisure,
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Leisure,
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Leisure,
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 3,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].gather_actions, 0);
        assert_eq!(world.records[0].labor_used, 0);
        assert_eq!(world.records[0].leisure_taken, 3);
    }

    #[test]
    fn rest_stops_tick_even_if_lower_ranked_work_remains() {
        let mut stock = Stock::new(3);
        stock.add(NET, 1);
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Later(3),
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Leisure,
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Later(6),
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Leisure,
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 3,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let mut recipes = builtin_recipes();
        recipes
            .iter_mut()
            .find(|recipe| recipe.id == RecipeId::FishWithNet)
            .unwrap()
            .output_qty = 1;
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes,
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].fish_actions, 1);
        assert_eq!(world.records[0].leisure_taken, 2);
        assert!(world.agent.scale[0].satisfied);
        assert!(world.agent.scale[1].satisfied);
        assert!(!world.agent.scale[2].satisfied);
        assert!(world.agent.scale[3].satisfied);
    }

    #[test]
    fn recipe_labor_cost_limits_executions() {
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock: Stock::new(3),
            labor_capacity: 3,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let mut recipes = builtin_recipes();
        recipes
            .iter_mut()
            .find(|recipe| recipe.id == RecipeId::GatherFood)
            .unwrap()
            .labor = 2;
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes,
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].gather_actions, 1);
        assert_eq!(world.records[0].labor_used, 2);
        assert_eq!(world.records[0].leisure_taken, 1);
    }

    #[test]
    fn same_tick_output_does_not_count_as_consumption() {
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Leisure,
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock: Stock::new(3),
            labor_capacity: 2,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].hunger_deficit, 1);
        assert_eq!(world.records[0].food_consumed, 0);
        assert_eq!(world.records[0].gather_actions, 1);
        assert_eq!(world.records[0].leisure_taken, 1);
        assert!(!world.agent.scale[0].satisfied);
    }

    #[test]
    fn current_hunger_ranks_near_term_production_once() {
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Leisure,
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock: Stock::new(3),
            labor_capacity: 2,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].hunger_deficit, 1);
        assert_eq!(world.records[0].gather_actions, 1);
        assert_eq!(world.records[0].labor_used, 1);
        assert_eq!(world.records[0].leisure_taken, 1);
        assert_eq!(world.records[0].food, 2);
    }

    #[test]
    fn large_now_want_can_drive_multiple_gathers_same_tick() {
        let agent = Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Now,
                qty: 4,
                satisfied: false,
            }],
            stock: Stock::new(3),
            labor_capacity: 2,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].hunger_deficit, 4);
        assert_eq!(world.records[0].gather_actions, 2);
        assert_eq!(world.records[0].labor_used, 2);
        assert_eq!(world.records[0].food, 4);
        assert!(!world.agent.scale[0].satisfied);
    }

    #[test]
    fn partial_now_consumption_reduces_remaining_production_need() {
        let mut stock = Stock::new(3);
        stock.add(FOOD, 2);
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Now,
                    qty: 4,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Leisure,
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 2,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].food_consumed, 2);
        assert_eq!(world.records[0].hunger_deficit, 2);
        assert_eq!(world.records[0].gather_actions, 1);
        assert_eq!(world.records[0].labor_used, 1);
        assert_eq!(world.records[0].leisure_taken, 1);
        assert_eq!(world.records[0].food, 2);
    }

    #[test]
    fn same_tick_now_output_is_not_reserved_twice() {
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock: Stock::new(3),
            labor_capacity: 2,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let mut recipes = builtin_recipes();
        recipes
            .iter_mut()
            .find(|recipe| recipe.id == RecipeId::GatherFood)
            .unwrap()
            .output_qty = 1;
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes,
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].gather_actions, 2);
        assert!(!world.agent.scale[0].satisfied);
        assert!(world.agent.scale[1].satisfied);
    }

    #[test]
    fn partial_output_serves_large_higher_ranked_want() {
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 3,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock: Stock::new(3),
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].gather_actions, 1);
        assert_eq!(world.records[0].labor_used, 1);
        assert_eq!(world.records[0].leisure_taken, 0);
        assert_eq!(world.records[0].food, 2);
        assert!(!world.agent.scale[0].satisfied);
        assert!(!world.agent.scale[1].satisfied);
    }

    #[test]
    fn output_remainder_does_not_skip_blocked_higher_ranked_want() {
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 3,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock: Stock::new(3),
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].gather_actions, 1);
        assert!(world.agent.scale[0].satisfied);
        assert!(!world.agent.scale[1].satisfied);
        assert!(!world.agent.scale[2].satisfied);
    }

    #[test]
    fn partial_output_to_higher_ranked_want_outranks_leisure() {
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 3,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Leisure,
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock: Stock::new(3),
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].gather_actions, 1);
        assert_eq!(world.records[0].labor_used, 1);
        assert_eq!(world.records[0].leisure_taken, 0);
        assert!(!world.agent.scale[0].satisfied);
        assert!(!world.agent.scale[1].satisfied);
        assert!(!world.agent.scale[2].satisfied);
    }

    #[test]
    fn completed_net_wins_direct_production_tie() {
        let mut stock = Stock::new(3);
        stock.add(NET, 1);
        let agent = Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Next,
                qty: 1,
                satisfied: false,
            }],
            stock,
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].fish_actions, 1);
        assert_eq!(world.records[0].gather_actions, 0);
        assert_eq!(world.records[0].food, 5);
    }

    #[test]
    fn project_start_does_not_spend_reserved_same_tick_output() {
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(WOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Later(3),
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock: Stock::new(3),
            labor_capacity: 3,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].cut_wood_actions, 3);
        assert_eq!(world.records[0].project_actions, 0);
        assert_eq!(world.records[0].active_projects, 0);
        assert_eq!(world.records[0].wood, 3);
    }

    #[test]
    fn project_start_does_not_spend_stock_provisioning_higher_ranked_want() {
        let mut stock = Stock::new(3);
        stock.add(WOOD, 2);
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(WOOD),
                    horizon: Horizon::Next,
                    qty: 2,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Later(3),
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].project_actions, 0);
        assert_eq!(world.records[0].cut_wood_actions, 1);
        assert_eq!(world.records[0].active_projects, 0);
        assert_eq!(world.records[0].wood, 3);
    }

    #[test]
    fn project_start_does_not_spend_partial_higher_ranked_stock_want() {
        let mut stock = Stock::new(3);
        stock.add(WOOD, 2);
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(WOOD),
                    horizon: Horizon::Next,
                    qty: 3,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Later(3),
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].project_actions, 0);
        assert_eq!(world.records[0].active_projects, 0);
        assert_eq!(world.records[0].wood, 3);
    }

    #[test]
    fn lower_ranked_stock_want_does_not_block_higher_ranked_project() {
        let mut stock = Stock::new(3);
        stock.add(WOOD, 2);
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Later(3),
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(WOOD),
                    horizon: Horizon::Next,
                    qty: 2,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].project_actions, 1);
        assert_eq!(world.records[0].cut_wood_actions, 0);
        assert_eq!(world.records[0].active_projects, 1);
        assert_eq!(world.records[0].wood, 0);
    }

    #[test]
    fn lower_ranked_stock_want_does_not_block_higher_ranked_recipe_input() {
        let mut stock = Stock::new(3);
        stock.add(WOOD, 1);
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(WOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let mut recipes = builtin_recipes();
        let gather = recipes
            .iter_mut()
            .find(|recipe| recipe.id == RecipeId::GatherFood)
            .unwrap();
        gather.input_good = Some((WOOD, 1));
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes,
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].gather_actions, 1);
        assert_eq!(world.records[0].wood, 0);
        assert!(world.agent.scale[0].satisfied);
        assert!(!world.agent.scale[1].satisfied);
    }

    #[test]
    fn higher_ranked_stock_want_blocks_lower_ranked_recipe_input() {
        let mut stock = Stock::new(3);
        stock.add(WOOD, 1);
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(WOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let mut recipes = builtin_recipes();
        let gather = recipes
            .iter_mut()
            .find(|recipe| recipe.id == RecipeId::GatherFood)
            .unwrap();
        gather.input_good = Some((WOOD, 1));
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes,
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].gather_actions, 0);
        assert_eq!(world.records[0].wood, 1);
        assert!(world.agent.scale[0].satisfied);
        assert!(!world.agent.scale[1].satisfied);
    }

    #[test]
    fn cut_wood_for_project_does_not_provision_lower_ranked_wood_want() {
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Later(3),
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(WOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock: Stock::new(3),
            labor_capacity: 3,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].cut_wood_actions, 2);
        assert_eq!(world.records[0].project_actions, 1);
        assert_eq!(world.records[0].active_projects, 1);
        assert_eq!(world.records[0].wood, 0);
        assert!(!world.agent.scale[1].satisfied);
    }

    #[test]
    fn project_start_recomputes_stock_backed_wants() {
        let mut stock = Stock::new(3);
        stock.add(WOOD, 2);
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Later(3),
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(WOOD),
                    horizon: Horizon::Next,
                    qty: 2,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].project_actions, 1);
        assert_eq!(world.records[0].wood, 0);
        assert!(!world.agent.scale[1].satisfied);
    }

    #[test]
    fn recipe_output_is_not_counted_twice_across_horizons() {
        let mut stock = Stock::new(3);
        stock.add(NET, 1);
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Later(3),
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Leisure,
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 2,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let mut recipes = builtin_recipes();
        recipes
            .iter_mut()
            .find(|recipe| recipe.id == RecipeId::GatherFood)
            .unwrap()
            .enabled = false;
        recipes
            .iter_mut()
            .find(|recipe| recipe.id == RecipeId::FishWithNet)
            .unwrap()
            .output_qty = 1;
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes,
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].fish_actions, 2);
        assert!(world.agent.scale[0].satisfied);
        assert!(world.agent.scale[1].satisfied);
    }

    #[test]
    fn hand_gathering_does_not_provide_later_food() {
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Later(4),
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock: Stock::new(3),
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let mut recipes = builtin_recipes();
        recipes
            .iter_mut()
            .find(|recipe| recipe.id == RecipeId::FishWithNet)
            .unwrap()
            .enabled = false;
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes,
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };
        let mut world = World::from_scenario(scenario);

        world.step();

        assert_eq!(world.records[0].gather_actions, 1);
        assert!(world.agent.scale[0].satisfied);
        assert!(!world.agent.scale[1].satisfied);
    }

    #[test]
    fn max_initial_project_id_prevents_reuse_after_overflow() {
        let template = build_net_template();
        let mut stock = Stock::new(3);
        stock.add(WOOD, 2);
        let agent = Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Later(3),
                qty: 1,
                satisfied: false,
            }],
            stock,
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: vec![Project {
                id: ProjectId(u32::MAX),
                template: template.id,
                state: ProjectState::Complete,
                started_at: Tick(0),
                labor_advanced: template.required_labor,
                input_goods_committed: template.input_goods.clone(),
                output_good: template.output_good,
                output_qty: template.output_qty,
                salvage_bps: template.salvage_bps,
            }],
            events: Vec::new(),
        };

        let mut world = World::from_scenario(scenario);
        world.step();

        assert_eq!(world.records[0].project_actions, 0);
        assert_eq!(world.records[0].active_projects, 0);
        assert!(world.projects.is_empty());
    }

    #[test]
    fn unavailable_project_id_does_not_skip_direct_work() {
        let template = build_net_template();
        let mut stock = Stock::new(3);
        stock.add(WOOD, 2);
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Later(3),
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock,
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: vec![Project {
                id: ProjectId(u32::MAX),
                template: template.id,
                state: ProjectState::Complete,
                started_at: Tick(0),
                labor_advanced: template.required_labor,
                input_goods_committed: template.input_goods.clone(),
                output_good: template.output_good,
                output_qty: template.output_qty,
                salvage_bps: template.salvage_bps,
            }],
            events: Vec::new(),
        };

        let mut world = World::from_scenario(scenario);
        world.step();

        assert_eq!(world.records[0].project_actions, 0);
        assert_eq!(world.records[0].gather_actions, 1);
        assert_eq!(world.records[0].labor_used, 1);
        assert_eq!(world.records[0].food, 2);
    }

    #[test]
    fn same_tick_disabled_recipe_abandons_ready_project_before_completion() {
        let template = build_net_template();
        let agent = Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Later(3),
                qty: 1,
                satisfied: false,
            }],
            stock: Stock::new(3),
            labor_capacity: 0,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: vec![Project {
                id: ProjectId(1),
                template: template.id,
                state: ProjectState::Forming,
                started_at: Tick(0),
                labor_advanced: template.required_labor,
                input_goods_committed: template.input_goods.clone(),
                output_good: template.output_good,
                output_qty: template.output_qty,
                salvage_bps: template.salvage_bps,
            }],
            events: vec![Event {
                tick: Tick(0),
                kind: EventKind::DisableRecipe(RecipeId::FishWithNet),
            }],
        };

        let mut world = World::from_scenario(scenario);
        world.step();

        assert_eq!(world.records[0].completed_projects, 0);
        assert_eq!(world.records[0].abandoned_projects, 1);
        assert_eq!(world.records[0].nets, 0);
        assert_eq!(world.records[0].wood, 1);
        assert_eq!(
            world.records[0].capital_labor_consumed,
            template.required_labor
        );
        assert_eq!(world.records[0].capital_goods_consumed, 1);
    }

    #[test]
    fn overdue_disable_event_applies_instead_of_being_dropped() {
        let template = build_net_template();
        let agent = Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Later(3),
                qty: 1,
                satisfied: false,
            }],
            stock: Stock::new(3),
            labor_capacity: 0,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: vec![Project {
                id: ProjectId(1),
                template: template.id,
                state: ProjectState::Forming,
                started_at: Tick(0),
                labor_advanced: 1,
                input_goods_committed: template.input_goods.clone(),
                output_good: template.output_good,
                output_qty: template.output_qty,
                salvage_bps: template.salvage_bps,
            }],
            events: vec![Event {
                tick: Tick(0),
                kind: EventKind::DisableRecipe(RecipeId::FishWithNet),
            }],
        };

        let mut world = World::from_scenario(scenario);
        world.tick = Tick(1);
        world.step();

        assert_eq!(world.records[0].abandoned_projects, 1);
        assert_eq!(world.records[0].active_projects, 0);
        assert_eq!(world.records[0].nets, 0);
        assert_eq!(world.records[0].wood, 1);
        assert_eq!(world.records[0].capital_labor_consumed, 1);
        assert_eq!(world.records[0].capital_goods_consumed, 1);
    }

    #[test]
    fn ready_initial_project_completes_without_extra_project_labor() {
        let template = build_net_template();
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Later(3),
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Leisure,
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock: Stock::new(3),
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes: builtin_recipes(),
            project_templates: builtin_project_templates(),
            initial_projects: vec![Project {
                id: ProjectId(1),
                template: template.id,
                state: ProjectState::Forming,
                started_at: Tick(0),
                labor_advanced: template.required_labor,
                input_goods_committed: template.input_goods.clone(),
                output_good: template.output_good,
                output_qty: template.output_qty,
                salvage_bps: template.salvage_bps,
            }],
            events: Vec::new(),
        };

        let mut world = World::from_scenario(scenario);
        world.step();

        assert_eq!(world.records[0].completed_projects, 1);
        assert_eq!(world.records[0].project_actions, 0);
        assert_eq!(world.records[0].fish_actions, 1);
        assert_eq!(world.records[0].labor_used, 1);
        assert_eq!(world.records[0].leisure_taken, 0);
        assert_eq!(world.records[0].active_projects, 0);
        assert_eq!(world.records[0].nets, 1);
        assert!(world.agent.scale[0].satisfied);
        assert!(!world.agent.scale[1].satisfied);
    }

    #[test]
    fn net_instrumental_rank_uses_later_food_only() {
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Next,
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Later(3),
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock: Stock::new(3),
            labor_capacity: 0,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };

        assert_eq!(
            super::instrumental_rank_for_net(&agent, &builtin_recipes()),
            Some(1)
        );
    }

    #[test]
    fn fish_recipe_without_net_tool_cannot_provide_later_food() {
        let agent = Agent {
            id: AgentId(1),
            scale: vec![
                Want {
                    kind: WantKind::Good(FOOD),
                    horizon: Horizon::Later(3),
                    qty: 1,
                    satisfied: false,
                },
                Want {
                    kind: WantKind::Leisure,
                    horizon: Horizon::Now,
                    qty: 1,
                    satisfied: false,
                },
            ],
            stock: Stock::new(3),
            labor_capacity: 1,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let mut recipes = builtin_recipes();
        recipes
            .iter_mut()
            .find(|recipe| recipe.id == RecipeId::FishWithNet)
            .unwrap()
            .required_tool = None;
        let scenario = Scenario {
            name: "test",
            seed: 99,
            periods: 1,
            agent,
            recipes,
            project_templates: builtin_project_templates(),
            initial_projects: Vec::new(),
            events: Vec::new(),
        };

        let mut world = World::from_scenario(scenario);
        world.step();

        assert_eq!(world.records[0].fish_actions, 0);
        assert_eq!(world.records[0].cut_wood_actions, 0);
        assert_eq!(world.records[0].project_actions, 0);
        assert_eq!(world.records[0].leisure_taken, 1);
        assert!(!world.agent.scale[0].satisfied);
    }

    #[test]
    fn net_instrumental_rank_requires_recipe_that_uses_net() {
        let agent = Agent {
            id: AgentId(1),
            scale: vec![Want {
                kind: WantKind::Good(FOOD),
                horizon: Horizon::Later(3),
                qty: 1,
                satisfied: false,
            }],
            stock: Stock::new(3),
            labor_capacity: 0,
            hunger_deficit: 0,
            gold: crate::good::Gold::ZERO,
            roles: vec![crate::agent::Role::Household],
            expect: Vec::new(),
        };
        let mut recipes = builtin_recipes();
        recipes
            .iter_mut()
            .find(|recipe| recipe.id == RecipeId::FishWithNet)
            .unwrap()
            .required_tool = None;

        assert_eq!(super::instrumental_rank_for_net(&agent, &recipes), None);
    }
}
