//! Multi-agent M2 capital projects and project output lots.

use crate::agent::AgentId;
use crate::good::{Gold, GoodId, Stock, FOOD};
use crate::project::Tick;
pub use crate::purpose::{M2ProjectId, ProjectPlanId};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectLineId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum M2ProjectState {
    Forming,
    Waiting,
    Mature,
    Sold,
    Abandoned,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectLine {
    pub id: ProjectLineId,
    pub name: &'static str,
    pub input_goods: Vec<(GoodId, u32)>,
    pub required_labor: u32,
    pub period_len: u8,
    pub output_good: GoodId,
    pub output_qty: u32,
    pub salvage_bps: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct M2Project {
    pub id: M2ProjectId,
    pub owner: AgentId,
    pub line: ProjectLineId,
    pub state: M2ProjectState,
    pub started_at: Tick,
    pub maturity: Tick,
    pub labor_advanced: u32,
    pub input_goods_committed: Vec<(GoodId, u32)>,
    pub input_cost_basis: Gold,
    pub advanced_gold: Gold,
    pub expected_revenue: Gold,
    pub output_good: GoodId,
    pub output_qty: u32,
    pub salvage_bps: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectOutputLot {
    pub project: M2ProjectId,
    pub owner: AgentId,
    pub good: GoodId,
    pub qty_remaining: u32,
    pub proceeds: Gold,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectFundingPlan {
    pub id: ProjectPlanId,
    pub owner: AgentId,
    pub line: ProjectLineId,
    pub created_tick: Tick,
    pub expires_tick: Tick,
    pub expected_revenue: Gold,
    pub input_cost_basis: Gold,
    pub required_labor: u32,
    pub funding_horizon: u32,
    pub borrowed_gold: Gold,
    pub future_due_committed: Gold,
    pub reserved_gold: Gold,
    pub started_project: Option<M2ProjectId>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct M2CapitalLoss {
    pub labor_consumed: u32,
    pub goods_consumed: u32,
    pub gold_loss: Gold,
}

pub fn dry_fish_short_line() -> ProjectLine {
    ProjectLine {
        id: ProjectLineId(1),
        name: "DryFishShort",
        input_goods: Vec::new(),
        required_labor: 1,
        period_len: 1,
        output_good: FOOD,
        output_qty: 2,
        salvage_bps: 8000,
    }
}

pub fn build_fish_trap_line() -> ProjectLine {
    ProjectLine {
        id: ProjectLineId(2),
        name: "BuildFishTrap",
        input_goods: Vec::new(),
        required_labor: 3,
        period_len: 4,
        output_good: FOOD,
        output_qty: 9,
        salvage_bps: 5000,
    }
}

pub fn borrow_to_build_line() -> ProjectLine {
    ProjectLine {
        id: ProjectLineId(3),
        name: "BorrowToBuildTrap",
        input_goods: Vec::new(),
        required_labor: 1,
        period_len: 2,
        output_good: FOOD,
        output_qty: 4,
        salvage_bps: 5000,
    }
}

pub fn credit_boom_long_line() -> ProjectLine {
    ProjectLine {
        id: ProjectLineId(4),
        name: "CreditBoomLongTrap",
        input_goods: Vec::new(),
        required_labor: 6,
        period_len: 8,
        output_good: FOOD,
        output_qty: 20,
        salvage_bps: 1000,
    }
}

pub fn builtin_project_lines() -> Vec<ProjectLine> {
    vec![dry_fish_short_line(), build_fish_trap_line()]
}

pub fn find_line(lines: &[ProjectLine], id: ProjectLineId) -> Option<&ProjectLine> {
    lines.iter().find(|line| line.id == id)
}

pub fn start_project(
    owner: AgentId,
    line: &ProjectLine,
    id: M2ProjectId,
    tick: Tick,
    expected_revenue: Gold,
    input_cost_basis: Gold,
) -> M2Project {
    M2Project {
        id,
        owner,
        line: line.id,
        state: M2ProjectState::Forming,
        started_at: tick,
        maturity: Tick(tick.0.saturating_add(u64::from(line.period_len))),
        labor_advanced: 0,
        input_goods_committed: aggregate_input_goods(&line.input_goods),
        input_cost_basis,
        advanced_gold: Gold::ZERO,
        expected_revenue,
        output_good: line.output_good,
        output_qty: line.output_qty,
        salvage_bps: line.salvage_bps,
    }
}

pub(crate) fn aggregate_input_goods(input_goods: &[(GoodId, u32)]) -> Vec<(GoodId, u32)> {
    // First-appearance order is part of the contract (input debits follow
    // it); the index map only replaces the O(N) scan per entry.
    let mut required: Vec<(GoodId, u32)> = Vec::new();
    let mut index: BTreeMap<GoodId, usize> = BTreeMap::new();
    for (good, qty) in input_goods {
        match index.get(good) {
            Some(&at) => {
                let (_, total) = &mut required[at];
                *total = total.saturating_add(*qty);
            }
            None => {
                index.insert(*good, required.len());
                required.push((*good, *qty));
            }
        }
    }
    required
}

pub(crate) fn committed_input_goods(line: &ProjectLine) -> Vec<(GoodId, u32)> {
    aggregate_input_goods(&line.input_goods)
}

pub fn advance_project(
    project: &mut M2Project,
    line: &ProjectLine,
    payment: Gold,
    qty: u32,
) -> bool {
    if qty == 0
        || project.state != M2ProjectState::Forming
        || project.labor_advanced >= line.required_labor
    {
        return false;
    }
    let remaining = line.required_labor.saturating_sub(project.labor_advanced);
    project.labor_advanced = project.labor_advanced.saturating_add(qty.min(remaining));
    project.advanced_gold = project.advanced_gold.saturating_add(payment);
    if project.labor_advanced >= line.required_labor {
        project.state = M2ProjectState::Waiting;
    }
    true
}

pub fn mature_project(project: &mut M2Project, tick: Tick) -> Option<ProjectOutputLot> {
    if project.state != M2ProjectState::Waiting || project.maturity > tick {
        return None;
    }
    project.state = M2ProjectState::Mature;
    Some(ProjectOutputLot {
        project: project.id,
        owner: project.owner,
        good: project.output_good,
        qty_remaining: project.output_qty,
        proceeds: Gold::ZERO,
    })
}

pub fn record_project_sale(
    lots: &mut [ProjectOutputLot],
    projects: &mut [M2Project],
    owner: AgentId,
    good: GoodId,
    qty: u32,
    proceeds: Gold,
) -> Gold {
    if qty == 0 {
        return Gold::ZERO;
    }
    let mut qty_left = qty;
    let mut proceeds_left = proceeds;
    let mut attributed = Gold::ZERO;
    for lot in lots.iter_mut() {
        if qty_left == 0 || lot.owner != owner || lot.good != good || lot.qty_remaining == 0 {
            continue;
        }
        let take = lot.qty_remaining.min(qty_left);
        let lot_proceeds = if qty_left == take {
            proceeds_left
        } else {
            let prorated =
                u128::from(proceeds.0).saturating_mul(u128::from(take)) / u128::from(qty);
            Gold(u64::try_from(prorated).unwrap_or(u64::MAX))
        };
        lot.qty_remaining -= take;
        qty_left -= take;
        proceeds_left = proceeds_left.saturating_sub(lot_proceeds);
        lot.proceeds = lot.proceeds.saturating_add(lot_proceeds);
        attributed = attributed.saturating_add(lot_proceeds);
        if lot.qty_remaining == 0 {
            if let Some(project) = projects
                .iter_mut()
                .find(|project| project.id == lot.project)
            {
                if project.state == M2ProjectState::Mature {
                    project.state = M2ProjectState::Sold;
                }
            }
        }
    }
    attributed
}

pub fn abandon_project(project: &mut M2Project, stock: &mut Stock) -> M2CapitalLoss {
    if !matches!(
        project.state,
        M2ProjectState::Forming | M2ProjectState::Waiting
    ) {
        return M2CapitalLoss::default();
    }
    project.state = M2ProjectState::Abandoned;
    let salvage_bps = project.salvage_bps.min(10_000);
    let mut goods_consumed = 0u32;
    // Invariant: every committed input was removed from the owner's stock when
    // the project started. Abandonment may only return salvage from that debited
    // stock; callers must not seed committed inputs without the matching debit.
    for (good, qty) in aggregate_input_goods(&project.input_goods_committed) {
        let salvage = (u64::from(qty) * u64::from(salvage_bps) / 10_000) as u32;
        if salvage > 0 {
            stock.add(good, salvage);
        }
        goods_consumed = goods_consumed.saturating_add(qty.saturating_sub(salvage));
    }
    M2CapitalLoss {
        labor_consumed: project.labor_advanced,
        goods_consumed,
        gold_loss: project.advanced_gold,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        abandon_project, advance_project, build_fish_trap_line, mature_project,
        record_project_sale, start_project, M2ProjectId, M2ProjectState,
    };
    use crate::agent::AgentId;
    use crate::good::{Gold, Stock, FOOD, WOOD};
    use crate::project::Tick;

    #[test]
    fn labor_trade_advances_project_once() {
        let line = build_fish_trap_line();
        let mut project = start_project(
            AgentId(1),
            &line,
            M2ProjectId(1),
            Tick(0),
            Gold(9),
            Gold::ZERO,
        );

        assert!(advance_project(&mut project, &line, Gold(1), 1));

        assert_eq!(project.labor_advanced, 1);
        assert_eq!(project.advanced_gold, Gold(1));
        assert_eq!(project.state, M2ProjectState::Forming);
    }

    #[test]
    fn project_matures_only_after_period_len() {
        let line = build_fish_trap_line();
        let mut project = start_project(
            AgentId(1),
            &line,
            M2ProjectId(1),
            Tick(0),
            Gold(9),
            Gold::ZERO,
        );
        for _ in 0..line.required_labor {
            advance_project(&mut project, &line, Gold(1), 1);
        }

        assert!(mature_project(&mut project, Tick(3)).is_none());
        let lot = mature_project(&mut project, Tick(4)).unwrap();

        assert_eq!(project.state, M2ProjectState::Mature);
        assert_eq!(lot.good, FOOD);
        assert_eq!(lot.qty_remaining, 9);
    }

    #[test]
    fn project_output_sale_records_realized_revenue() {
        let line = build_fish_trap_line();
        let mut project = start_project(
            AgentId(1),
            &line,
            M2ProjectId(1),
            Tick(0),
            Gold(9),
            Gold::ZERO,
        );
        for _ in 0..line.required_labor {
            advance_project(&mut project, &line, Gold(1), 1);
        }
        let lot = mature_project(&mut project, Tick(4)).unwrap();
        let mut projects = vec![project];
        let mut lots = vec![lot];

        let attributed =
            record_project_sale(&mut lots, &mut projects, AgentId(1), FOOD, 9, Gold(18));

        assert_eq!(attributed, Gold(18));
        assert_eq!(lots[0].proceeds, Gold(18));
        assert_eq!(projects[0].state, M2ProjectState::Sold);
    }

    #[test]
    fn start_project_aggregates_duplicate_input_goods() {
        let mut line = build_fish_trap_line();
        line.input_goods = vec![(WOOD, 1), (WOOD, 2)];

        let project = start_project(
            AgentId(1),
            &line,
            M2ProjectId(1),
            Tick(0),
            Gold(9),
            Gold::ZERO,
        );

        assert_eq!(project.input_goods_committed, vec![(WOOD, 3)]);
    }

    #[test]
    fn abandonment_returns_m2_input_salvage() {
        let mut line = build_fish_trap_line();
        line.input_goods = vec![(WOOD, 1), (WOOD, 1)];
        line.salvage_bps = 5000;
        let mut project = start_project(
            AgentId(1),
            &line,
            M2ProjectId(1),
            Tick(0),
            Gold(9),
            Gold::ZERO,
        );
        project.labor_advanced = 1;
        project.advanced_gold = Gold(2);
        let mut stock = Stock::new(3);

        let loss = abandon_project(&mut project, &mut stock);

        assert_eq!(project.state, M2ProjectState::Abandoned);
        assert_eq!(stock.get(WOOD), 1);
        assert_eq!(loss.goods_consumed, 1);
        assert_eq!(loss.labor_consumed, 1);
        assert_eq!(loss.gold_loss, Gold(2));
    }

    #[test]
    fn project_sale_proration_uses_wide_math() {
        let line = build_fish_trap_line();
        let mut project = start_project(
            AgentId(1),
            &line,
            M2ProjectId(1),
            Tick(0),
            Gold(9),
            Gold::ZERO,
        );
        for _ in 0..line.required_labor {
            advance_project(&mut project, &line, Gold(1), 1);
        }
        let mut lot = mature_project(&mut project, Tick(4)).unwrap();
        lot.qty_remaining = 2;
        let mut projects = vec![project];
        let mut lots = vec![lot];

        let attributed = record_project_sale(
            &mut lots,
            &mut projects,
            AgentId(1),
            FOOD,
            3,
            Gold(u64::MAX),
        );

        assert_eq!(attributed, Gold((u128::from(u64::MAX) * 2 / 3) as u64));
    }
}
