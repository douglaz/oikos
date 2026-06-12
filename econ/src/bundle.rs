//! Ordinal appraisal for a whole borrow-build-sell-repay project bundle.

use crate::agent::{AgentId, Want, WantKind};
use crate::agio::{want_provisioned_temporally_for_money, TemporalEndowment};
use crate::capital::ProjectLineId;
use crate::good::{Gold, GoodId, Horizon, Stock, GOLD};
use crate::project::Tick;
use crate::purpose::{CreditLender, CreditSource, DebtPurpose, LoanPurpose, ProjectPlanId};
use crate::timemarket::{DebtContract, DebtId, DebtState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectBundleCandidate {
    pub owner: AgentId,
    pub line: ProjectLineId,
    pub present_advance: Gold,
    pub expected_revenue: Gold,
    pub input_cost_basis: Gold,
    pub required_labor: u32,
    pub project_period: u32,
    pub loan_horizon: u32,
    pub input_goods: Vec<(GoodId, u32)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectBundleQuote {
    pub purpose: LoanPurpose,
    pub present_advance: Gold,
    pub max_future_due: Gold,
    pub loan_horizon: u32,
    pub target_rank: usize,
}

pub struct ProjectBundleEndowment<'a> {
    pub scale: &'a [Want],
    pub stock: &'a Stock,
    pub gold: Gold,
    pub receivables: &'a [DebtContract],
    pub payables: &'a [DebtContract],
    pub tick: Tick,
}

pub fn appraise_project_bundle(
    endowment: &ProjectBundleEndowment<'_>,
    candidate: &ProjectBundleCandidate,
    plan: ProjectPlanId,
) -> Option<ProjectBundleQuote> {
    appraise_project_bundle_for_money(endowment, candidate, plan, GOLD)
}

pub fn appraise_project_bundle_for_money(
    endowment: &ProjectBundleEndowment<'_>,
    candidate: &ProjectBundleCandidate,
    plan: ProjectPlanId,
    money_good: GoodId,
) -> Option<ProjectBundleQuote> {
    if candidate.owner.0 == 0
        || candidate.present_advance == Gold::ZERO
        || candidate.expected_revenue == Gold::ZERO
        || candidate.project_period == 0
        || candidate.loan_horizon < candidate.project_period
    {
        return None;
    }
    if candidate.expected_revenue < candidate.present_advance {
        return None;
    }
    let bundle_stock = stock_after_inputs(endowment.stock, &candidate.input_goods)?;
    let first_due = candidate.present_advance.0;
    let upper_due = candidate.expected_revenue.0;
    let (_, first_target) =
        bundle_accepts_due(endowment, candidate, &bundle_stock, first_due, money_good)?;

    let mut low = first_due;
    let mut high = upper_due;
    let mut target_rank = first_target;
    while low < high {
        let mid = low + (high - low).div_ceil(2);
        if let Some((_, target)) =
            bundle_accepts_due(endowment, candidate, &bundle_stock, mid, money_good)
        {
            low = mid;
            target_rank = target;
        } else {
            high = mid.saturating_sub(1);
        }
    }

    Some(ProjectBundleQuote {
        purpose: LoanPurpose::ProjectFunding(plan),
        present_advance: candidate.present_advance,
        max_future_due: Gold(low),
        loan_horizon: candidate.loan_horizon,
        target_rank,
    })
}

fn bundle_accepts_due(
    endowment: &ProjectBundleEndowment<'_>,
    candidate: &ProjectBundleCandidate,
    bundle_stock: &Stock,
    future_due: u64,
    money_good: GoodId,
) -> Option<(Gold, usize)> {
    let future_due = Gold(future_due);
    if future_due >= candidate.expected_revenue {
        return None;
    }

    let baseline = TemporalEndowment {
        stock: endowment.stock,
        gold: endowment.gold,
        receivables: endowment.receivables,
        payables: endowment.payables,
        tick: endowment.tick,
    };
    let baseline_provided = provisioning_bitmap(endowment.scale, &baseline, money_good);

    let project_receivable = hypothetical_contract(
        candidate.owner,
        endowment.tick,
        candidate.project_period,
        candidate.expected_revenue,
    );
    let project_payable = hypothetical_contract(
        candidate.owner,
        endowment.tick,
        candidate.loan_horizon,
        future_due,
    );
    let mut receivables = Vec::with_capacity(endowment.receivables.len().saturating_add(1));
    receivables.extend_from_slice(endowment.receivables);
    receivables.push(project_receivable);
    let mut payables = Vec::with_capacity(endowment.payables.len().saturating_add(1));
    payables.extend_from_slice(endowment.payables);
    payables.push(project_payable);
    let bundle = TemporalEndowment {
        stock: bundle_stock,
        gold: endowment.gold,
        receivables: &receivables,
        payables: &payables,
        tick: endowment.tick,
    };
    let bundle_provided = provisioning_bitmap(endowment.scale, &bundle, money_good);

    endowment
        .scale
        .iter()
        .enumerate()
        .find_map(|(index, want)| {
            let future_money_want = want.kind == WantKind::Good(money_good)
                && matches!(want.horizon, Horizon::Later(later) if u32::from(later) >= candidate.loan_horizon);
            if future_money_want
                && !baseline_provided.get(index).copied().unwrap_or(false)
                && bundle_provided.get(index).copied().unwrap_or(false)
                && preserved_above_target(&baseline_provided, &bundle_provided, index)
            {
                Some((future_due, index))
            } else {
                None
            }
        })
}

fn provisioning_bitmap(
    scale: &[Want],
    endowment: &TemporalEndowment<'_>,
    money_good: GoodId,
) -> Vec<bool> {
    (0..scale.len())
        .map(|index| want_provisioned_temporally_for_money(scale, index, endowment, money_good))
        .collect()
}

fn stock_after_inputs(stock: &Stock, input_goods: &[(GoodId, u32)]) -> Option<Stock> {
    let mut after = stock.clone();
    for (good, qty) in aggregate(input_goods) {
        if !after.remove(good, qty) {
            return None;
        }
    }
    Some(after)
}

fn aggregate(input_goods: &[(GoodId, u32)]) -> Vec<(GoodId, u32)> {
    let mut aggregated = Vec::<(GoodId, u32)>::new();
    for (good, qty) in input_goods {
        if let Some((_, total)) = aggregated.iter_mut().find(|(entry, _)| entry == good) {
            *total = total.saturating_add(*qty);
        } else {
            aggregated.push((*good, *qty));
        }
    }
    aggregated
}

fn hypothetical_contract(owner: AgentId, tick: Tick, horizon: u32, due: Gold) -> DebtContract {
    DebtContract {
        id: DebtId(0),
        lender: CreditLender::Agent(AgentId(0)),
        borrower: owner,
        opened_tick: tick,
        due_tick: Tick(tick.0.saturating_add(u64::from(horizon))),
        principal: Gold::ZERO,
        due,
        paid: Gold::ZERO,
        state: DebtState::Open,
        purpose: DebtPurpose::Consumption,
        funding: CreditSource::Commodity,
    }
}

fn preserved_above_target(before: &[bool], after: &[bool], target: usize) -> bool {
    before
        .iter()
        .zip(after)
        .take(target)
        .all(|(was, now)| !*was || *now)
}

#[cfg(test)]
mod tests {
    use super::{appraise_project_bundle, ProjectBundleCandidate, ProjectBundleEndowment};
    use crate::agent::{AgentId, Want, WantKind};
    use crate::capital::ProjectLineId;
    use crate::good::{Gold, Horizon, Stock, GOLD, WOOD};
    use crate::project::Tick;
    use crate::purpose::{CreditLender, CreditSource, DebtPurpose, LoanPurpose, ProjectPlanId};
    use crate::timemarket::{DebtContract, DebtId, DebtState};

    fn want(kind: WantKind, horizon: Horizon) -> Want {
        Want {
            kind,
            horizon,
            qty: 1,
            satisfied: false,
        }
    }

    fn repeat(scale: &mut Vec<Want>, kind: WantKind, horizon: Horizon, n: usize) {
        for _ in 0..n {
            scale.push(want(kind, horizon));
        }
    }

    fn candidate(expected_revenue: Gold) -> ProjectBundleCandidate {
        ProjectBundleCandidate {
            owner: AgentId(1),
            line: ProjectLineId(1),
            present_advance: Gold(1),
            expected_revenue,
            input_cost_basis: Gold::ZERO,
            required_labor: 1,
            project_period: 2,
            loan_horizon: 4,
            input_goods: Vec::new(),
        }
    }

    fn contract(due_tick: u64, due: Gold) -> DebtContract {
        DebtContract {
            id: DebtId(1),
            lender: CreditLender::Agent(AgentId(9)),
            borrower: AgentId(1),
            opened_tick: Tick(0),
            due_tick: Tick(due_tick),
            principal: Gold::ZERO,
            due,
            paid: Gold::ZERO,
            state: DebtState::Open,
            purpose: DebtPurpose::Consumption,
            funding: CreditSource::Commodity,
        }
    }

    #[test]
    fn bundle_appraisal_accepts_project_backed_borrowing() {
        let mut scale = Vec::new();
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Later(4), 3);
        let stock = Stock::new(3);
        let endowment = ProjectBundleEndowment {
            scale: &scale,
            stock: &stock,
            gold: Gold::ZERO,
            receivables: &[],
            payables: &[],
            tick: Tick(0),
        };

        let quote =
            appraise_project_bundle(&endowment, &candidate(Gold(5)), ProjectPlanId(7)).unwrap();

        assert_eq!(quote.purpose, LoanPurpose::ProjectFunding(ProjectPlanId(7)));
        assert_eq!(quote.present_advance, Gold(1));
        assert!(quote.max_future_due >= Gold(1));
    }

    #[test]
    fn bundle_appraisal_rejects_due_that_breaks_higher_ranked_want() {
        let mut scale = Vec::new();
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Later(4), 4);
        let stock = Stock::new(3);
        let receivables = vec![contract(4, Gold(3))];
        let endowment = ProjectBundleEndowment {
            scale: &scale,
            stock: &stock,
            gold: Gold::ZERO,
            receivables: &receivables,
            payables: &[],
            tick: Tick(0),
        };

        let quote =
            appraise_project_bundle(&endowment, &candidate(Gold(3)), ProjectPlanId(8)).unwrap();

        assert_eq!(quote.max_future_due, Gold(2));
        assert_eq!(quote.target_rank, 3);
    }

    #[test]
    fn bundle_appraisal_rejects_project_with_no_expected_sale() {
        let mut scale = Vec::new();
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Later(4), 1);
        let stock = Stock::new(3);
        let endowment = ProjectBundleEndowment {
            scale: &scale,
            stock: &stock,
            gold: Gold::ZERO,
            receivables: &[],
            payables: &[],
            tick: Tick(0),
        };

        assert_eq!(
            appraise_project_bundle(&endowment, &candidate(Gold::ZERO), ProjectPlanId(9)),
            None
        );
    }

    #[test]
    fn bundle_appraisal_works_without_lending_quotes() {
        let mut scale = Vec::new();
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Later(4), 2);
        let stock = Stock::new(3);
        let endowment = ProjectBundleEndowment {
            scale: &scale,
            stock: &stock,
            gold: Gold::ZERO,
            receivables: &[],
            payables: &[],
            tick: Tick(0),
        };

        assert!(
            appraise_project_bundle(&endowment, &candidate(Gold(4)), ProjectPlanId(10)).is_some()
        );
    }

    #[test]
    fn bundle_appraisal_requires_input_goods_on_hand() {
        let mut scale = Vec::new();
        repeat(&mut scale, WantKind::Good(GOLD), Horizon::Later(4), 2);
        let stock = Stock::new(3);
        let mut candidate = candidate(Gold(4));
        candidate.input_goods = vec![(WOOD, 1)];
        let endowment = ProjectBundleEndowment {
            scale: &scale,
            stock: &stock,
            gold: Gold::ZERO,
            receivables: &[],
            payables: &[],
            tick: Tick(0),
        };

        assert!(appraise_project_bundle(&endowment, &candidate, ProjectPlanId(11)).is_none());
    }
}
