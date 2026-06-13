//! Shared IDs and purpose tags for project-funded credit.

use std::fmt;

use crate::agent::AgentId;
use crate::ledger::{BankId, IssuerId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectPlanId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct M2ProjectId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreditSource {
    Commodity,
    BankFiduciary(BankId),
    FiatFiscal(IssuerId),
    FiatCredit(IssuerId),
    /// A tax liability owed to the named issuer. The contract carries
    /// `principal = Gold::ZERO` (no loan was made); only the `due` levy is
    /// owed. Settlement routes through `settle_tax_debt_m3`, never the credit
    /// paths, so tax receipts never touch `fiat_credit_outstanding`.
    Tax(IssuerId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CreditLender {
    Agent(AgentId),
    Bank(BankId),
    Issuer(IssuerId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoanPurpose {
    Consumption,
    ProjectFunding(ProjectPlanId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebtPurpose {
    Consumption,
    ProjectFunding {
        plan: ProjectPlanId,
        project: Option<M2ProjectId>,
    },
    /// A levied tax liability (M21). Carries no project linkage.
    TaxLiability,
}

impl LoanPurpose {
    pub fn project_plan(&self) -> Option<ProjectPlanId> {
        match self {
            LoanPurpose::Consumption => None,
            LoanPurpose::ProjectFunding(plan) => Some(*plan),
        }
    }
}

impl DebtPurpose {
    pub fn project_plan(&self) -> Option<ProjectPlanId> {
        match self {
            DebtPurpose::Consumption | DebtPurpose::TaxLiability => None,
            DebtPurpose::ProjectFunding { plan, .. } => Some(*plan),
        }
    }

    pub fn linked_project(&self) -> Option<M2ProjectId> {
        match self {
            DebtPurpose::Consumption | DebtPurpose::TaxLiability => None,
            DebtPurpose::ProjectFunding { project, .. } => *project,
        }
    }
}

impl CreditLender {
    pub fn agent(self) -> Option<AgentId> {
        match self {
            CreditLender::Agent(agent) => Some(agent),
            CreditLender::Bank(_) | CreditLender::Issuer(_) => None,
        }
    }
}

impl fmt::Display for CreditSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CreditSource::Commodity => f.write_str("Commodity"),
            CreditSource::BankFiduciary(bank) => write!(f, "BankFiduciary({})", bank.0),
            CreditSource::FiatFiscal(issuer) => write!(f, "FiatFiscal({})", issuer.0),
            CreditSource::FiatCredit(issuer) => write!(f, "FiatCredit({})", issuer.0),
            CreditSource::Tax(issuer) => write!(f, "Tax({})", issuer.0),
        }
    }
}

impl fmt::Display for CreditLender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CreditLender::Agent(agent) => write!(f, "Agent({agent})"),
            CreditLender::Bank(bank) => write!(f, "Bank({})", bank.0),
            CreditLender::Issuer(issuer) => write!(f, "Issuer({})", issuer.0),
        }
    }
}

impl fmt::Display for LoanPurpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoanPurpose::Consumption => f.write_str("Consumption"),
            LoanPurpose::ProjectFunding(plan) => write!(f, "ProjectFunding(plan={})", plan.0),
        }
    }
}
