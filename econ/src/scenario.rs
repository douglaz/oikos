//! Built-in scenarios.

use crate::agent::{Agent, AgentId, Role, Want, WantKind};
use crate::bank::BankPolicy;
use crate::cantillon::CantillonRoute;
use crate::expect::PriceBelief;
use crate::good::{Gold, GoodId, Horizon, Stock, CLOTH, FOOD, GOLD, NET, ORE, SALT, WOOD};
use crate::issuer::IssuerPolicy;
use crate::ledger::{BankId, IssuerId};
use crate::marketability::{GoodMarketability, MarketabilityConfig};
use crate::money::{
    BankRepaymentTender, DesignatedMoney, IssuerRepaymentTender, LaborWageTender,
    MarketMoneyConfig, MengerianConfig, PublicDebtTender, PublicSpotTender, Regime,
    ReserveRatioBps, TaxReceivability,
};
use crate::project::{
    build_net_template, builtin_project_templates, builtin_recipes, Project, ProjectId,
    ProjectState, ProjectTemplate, Recipe, RecipeId, Tick,
};
use crate::purpose::DebtPurpose;
use crate::timemarket::DebtId;

const EMERGED_GOLD_FIRST_RECEIVERS: [AgentId; 2] = [AgentId(7), AgentId(8)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScenarioName {
    CrusoeSurvival,
    CrusoeCapital,
    CrusoeAbandon,
    MarketBarterishGold,
    MarketPriceDiscovery,
    MarketNoMutualBenefit,
    TimeMarketBasic,
    RoundaboutCapital,
    BorrowToBuild,
    SoundMoney100Pct,
    CommodityCreditNeutral,
    FractionalReserve,
    SuspensionOfConvertibility,
    FiatCreditExpansion,
    FiatFiscalCantillon,
    CantillonIsolation,
    EmergedGoldSoundControl,
    EmergedGoldFiatDisplacement,
    EmergedGoldFiatRefusalControl,
    EmergedGoldFiatLegalTender,
    EmergedGoldFiatDebtRefusalControl,
    EmergedGoldFiatDebtLegalTender,
    EmergedGoldBankClaimDebtRefusalControl,
    EmergedGoldBankClaimDebtLegalTender,
    EmergedGoldBankClaimSpotRefusalControl,
    EmergedGoldBankClaimSpotLegalTender,
    EmergedGoldBankLoanRepaymentRefusalControl,
    EmergedGoldBankLoanRepaymentClaimTender,
    EmergedGoldFractionalReserve,
    EmergedGoldFiatCreditExpansion,
    EmergedGoldFiatWageRefusalControl,
    EmergedGoldFiatWageLegalTender,
    EmergedGoldIssuerRepaymentFiatRefusalControl,
    EmergedGoldIssuerRepaymentFiatTender,
    EmergedGoldReserveLeashControl,
    EmergedGoldSuspensionOfConvertibility,
    EmergedGoldRedemptionRun,
    EmergedGoldSuspendedRedemption,
    EmergedGoldTaxSpecieControl,
    EmergedGoldTaxFiatUnpayableDefaults,
    EmergedGoldTaxDrivesFiatLabor,
    EmergedGoldNoTaxIdleControl,
    MengerSaltMoney,
    MengerGoldMoney,
    MengerMarketabilityDurability,
    MengerTwoLayerSaleability,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScenarioKind {
    AutarkyM0,
    MarketM1,
    MarketM2,
    MarketM3,
    MarketV2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScenarioProjectLines {
    None,
    Builtin,
    BorrowToBuild,
    CreditBoomLong,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScenarioInfo {
    pub name: &'static str,
    pub scenario: ScenarioName,
    pub description: &'static str,
}

pub const BUILTIN_SCENARIOS: &[ScenarioInfo] = &[
    ScenarioInfo {
        name: "crusoe-survival",
        scenario: ScenarioName::CrusoeSurvival,
        description: "direct production, consumption, and rest",
    },
    ScenarioInfo {
        name: "crusoe-capital",
        scenario: ScenarioName::CrusoeCapital,
        description: "saving, net building, and higher food output",
    },
    ScenarioInfo {
        name: "crusoe-abandon",
        scenario: ScenarioName::CrusoeAbandon,
        description: "abandonment and physical capital loss",
    },
    ScenarioInfo {
        name: "market-barterish-gold",
        scenario: ScenarioName::MarketBarterishGold,
        description: "designated-gold exchange across food and wood",
    },
    ScenarioInfo {
        name: "market-price-discovery",
        scenario: ScenarioName::MarketPriceDiscovery,
        description: "CDA price discovery with no-fill belief updates",
    },
    ScenarioInfo {
        name: "market-no-mutual-benefit",
        scenario: ScenarioName::MarketNoMutualBenefit,
        description: "no trade without compatible marginal wants",
    },
    ScenarioInfo {
        name: "time-market-basic",
        scenario: ScenarioName::TimeMarketBasic,
        description: "present gold for future gold and later settlement",
    },
    ScenarioInfo {
        name: "roundabout-capital",
        scenario: ScenarioName::RoundaboutCapital,
        description:
            "saver-capitalist self-funds roundabout output; separate borrower uses commodity credit",
    },
    ScenarioInfo {
        name: "borrow-to-build",
        scenario: ScenarioName::BorrowToBuild,
        description: "cash-poor capitalist uses project-purpose commodity credit to build",
    },
    ScenarioInfo {
        name: "sound-money-100pct",
        scenario: ScenarioName::SoundMoney100Pct,
        description: "commodity-credit control scenario with no money-stock drift",
    },
    ScenarioInfo {
        name: "commodity-credit-neutral",
        scenario: ScenarioName::CommodityCreditNeutral,
        description: "commodity-credit control with public specie only",
    },
    ScenarioInfo {
        name: "fractional-reserve",
        scenario: ScenarioName::FractionalReserve,
        description: "fractional bank demand-claim credit through crossed loan orders",
    },
    ScenarioInfo {
        name: "suspension-of-convertibility",
        scenario: ScenarioName::SuspensionOfConvertibility,
        description: "fractional banking followed by suspended convertibility and wider credit",
    },
    ScenarioInfo {
        name: "fiat-credit-expansion",
        scenario: ScenarioName::FiatCreditExpansion,
        description: "fiat credit boom into long projects, stop, abandonment, and bad debt",
    },
    ScenarioInfo {
        name: "fiat-fiscal-cantillon",
        scenario: ScenarioName::FiatFiscalCantillon,
        description: "fiscal fiat routed to first receivers who spend through spot markets",
    },
    ScenarioInfo {
        name: "cantillon-isolation",
        scenario: ScenarioName::CantillonIsolation,
        description: "single narrow fiat injection showing first-receiver redistribution",
    },
    ScenarioInfo {
        name: "emerged-gold-sound-control",
        scenario: ScenarioName::EmergedGoldSoundControl,
        description: "M3 sound-gold ledger bridge from the M6 emerged-gold endowment",
    },
    ScenarioInfo {
        name: "emerged-gold-fiat-displacement",
        scenario: ScenarioName::EmergedGoldFiatDisplacement,
        description: "tick-0 fiat fiscal issue spent before retained emerged-gold specie",
    },
    ScenarioInfo {
        name: "emerged-gold-fiat-refusal-control",
        scenario: ScenarioName::EmergedGoldFiatRefusalControl,
        description: "emerged-gold fiat issue with specie-only spot acceptance",
    },
    ScenarioInfo {
        name: "emerged-gold-fiat-legal-tender",
        scenario: ScenarioName::EmergedGoldFiatLegalTender,
        description: "emerged-gold fiat issue with fiat legal-tender spot acceptance",
    },
    ScenarioInfo {
        name: "emerged-gold-fiat-debt-refusal-control",
        scenario: ScenarioName::EmergedGoldFiatDebtRefusalControl,
        description: "emerged-gold fiat debt with specie-only discharge",
    },
    ScenarioInfo {
        name: "emerged-gold-fiat-debt-legal-tender",
        scenario: ScenarioName::EmergedGoldFiatDebtLegalTender,
        description: "emerged-gold fiat debt with fiat-and-specie legal-tender discharge",
    },
    ScenarioInfo {
        name: "emerged-gold-bank-claim-debt-refusal-control",
        scenario: ScenarioName::EmergedGoldBankClaimDebtRefusalControl,
        description: "emerged-gold suspended bank claim refused for commodity-debt discharge",
    },
    ScenarioInfo {
        name: "emerged-gold-bank-claim-debt-legal-tender",
        scenario: ScenarioName::EmergedGoldBankClaimDebtLegalTender,
        description: "emerged-gold suspended bank claim discharges commodity debt at par",
    },
    ScenarioInfo {
        name: "emerged-gold-bank-claim-spot-refusal-control",
        scenario: ScenarioName::EmergedGoldBankClaimSpotRefusalControl,
        description: "emerged-gold suspended bank claim refused for spot purchase",
    },
    ScenarioInfo {
        name: "emerged-gold-bank-claim-spot-legal-tender",
        scenario: ScenarioName::EmergedGoldBankClaimSpotLegalTender,
        description: "emerged-gold suspended bank claim buys a spot good at par",
    },
    ScenarioInfo {
        name: "emerged-gold-bank-loan-repayment-refusal-control",
        scenario: ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl,
        description: "emerged-gold suspended bank loan repayment refuses the unredeemable claim",
    },
    ScenarioInfo {
        name: "emerged-gold-bank-loan-repayment-claim-tender",
        scenario: ScenarioName::EmergedGoldBankLoanRepaymentClaimTender,
        description: "emerged-gold suspended bank accepts its own claim in loan repayment",
    },
    ScenarioInfo {
        name: "emerged-gold-fractional-reserve",
        scenario: ScenarioName::EmergedGoldFractionalReserve,
        description: "emerged-gold bridge with convertible bank fiduciary credit",
    },
    ScenarioInfo {
        name: "emerged-gold-fiat-credit-expansion",
        scenario: ScenarioName::EmergedGoldFiatCreditExpansion,
        description:
            "emerged-gold bridge with fiat credit boom, stop, bust, and capital consumption",
    },
    ScenarioInfo {
        name: "emerged-gold-fiat-wage-refusal-control",
        scenario: ScenarioName::EmergedGoldFiatWageRefusalControl,
        description: "emerged-gold fiat-credit boom with specie-only labor-wage tender",
    },
    ScenarioInfo {
        name: "emerged-gold-fiat-wage-legal-tender",
        scenario: ScenarioName::EmergedGoldFiatWageLegalTender,
        description: "emerged-gold fiat-credit boom with fiat-and-specie labor-wage tender",
    },
    ScenarioInfo {
        name: "emerged-gold-issuer-repayment-fiat-refusal-control",
        scenario: ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl,
        description: "emerged-gold issuer refuses fiat repayment, credit overhang remains",
    },
    ScenarioInfo {
        name: "emerged-gold-issuer-repayment-fiat-tender",
        scenario: ScenarioName::EmergedGoldIssuerRepaymentFiatTender,
        description: "emerged-gold issuer accepts returned fiat, retiring credit",
    },
    ScenarioInfo {
        name: "emerged-gold-reserve-leash-control",
        scenario: ScenarioName::EmergedGoldReserveLeashControl,
        description: "emerged-gold bridge with a reserve-ratio-constrained convertible bank",
    },
    ScenarioInfo {
        name: "emerged-gold-suspension-of-convertibility",
        scenario: ScenarioName::EmergedGoldSuspensionOfConvertibility,
        description: "emerged-gold bridge where suspended redemption widens bank credit",
    },
    ScenarioInfo {
        name: "emerged-gold-redemption-run",
        scenario: ScenarioName::EmergedGoldRedemptionRun,
        description: "emerged-gold bank run where demand-claim redemption exhausts reserves",
    },
    ScenarioInfo {
        name: "emerged-gold-suspended-redemption",
        scenario: ScenarioName::EmergedGoldSuspendedRedemption,
        description: "emerged-gold suspended redemption refuses demand-claim conversion",
    },
    ScenarioInfo {
        name: "emerged-gold-tax-specie-control",
        scenario: ScenarioName::EmergedGoldTaxSpecieControl,
        description: "emerged-gold specie-only tax settles into the issuer vault",
    },
    ScenarioInfo {
        name: "emerged-gold-tax-fiat-unpayable-defaults",
        scenario: ScenarioName::EmergedGoldTaxFiatUnpayableDefaults,
        description: "fiat-only tax defaults on a specie-rich, fiat-less agent",
    },
    ScenarioInfo {
        name: "emerged-gold-tax-drives-fiat-labor",
        scenario: ScenarioName::EmergedGoldTaxDrivesFiatLabor,
        description: "a fiat-only tax pulls a leisured worker into fiat-wage labor",
    },
    ScenarioInfo {
        name: "emerged-gold-no-tax-idle-control",
        scenario: ScenarioName::EmergedGoldNoTaxIdleControl,
        description: "the tax-free falsification twin: the worker stays idle",
    },
    ScenarioInfo {
        name: "menger-salt-money",
        scenario: ScenarioName::MengerSaltMoney,
        description:
            "barter society where salt becomes commodity money through realized saleability",
    },
    ScenarioInfo {
        name: "menger-gold-money",
        scenario: ScenarioName::MengerGoldMoney,
        description: "barter society where physical gold emerges as commodity money",
    },
    ScenarioInfo {
        name: "menger-marketability-durability",
        scenario: ScenarioName::MengerMarketabilityDurability,
        description:
            "barter society isolating physical durability and carry cost in indirect acceptance",
    },
    ScenarioInfo {
        name: "menger-two-layer-saleability",
        scenario: ScenarioName::MengerTwoLayerSaleability,
        description: "two-layer saleability: direct-use eligibility with medium-share leadership",
    },
];

#[derive(Clone, Debug)]
pub struct Scenario {
    pub name: &'static str,
    pub seed: u64,
    pub periods: u64,
    pub agent: Agent,
    pub recipes: Vec<Recipe>,
    pub project_templates: Vec<ProjectTemplate>,
    pub initial_projects: Vec<Project>,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug)]
pub struct MarketScenario {
    pub name: &'static str,
    pub scenario: ScenarioName,
    pub seed: u64,
    pub periods: u64,
    pub agents: Vec<Agent>,
    pub recipes: Vec<Recipe>,
    pub events: Vec<Event>,
    pub money: MarketMoneyConfig,
}

#[derive(Clone, Debug)]
pub struct Event {
    pub tick: Tick,
    pub kind: EventKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RedemptionRoute {
    Agents(Vec<AgentId>),
    AllClaimHolders,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    DisableRecipe(RecipeId),
    SetRegime(Regime),
    SetReserveRatio {
        bank: BankId,
        ratio: ReserveRatioBps,
    },
    SetBankConvertibility {
        bank: BankId,
        convertible: bool,
    },
    SetBankCreditPolicy {
        bank: BankId,
        policy: BankPolicy,
    },
    StopBankCredit {
        bank: BankId,
    },
    RedeemDemandClaims {
        bank: BankId,
        route: RedemptionRoute,
        max_per_agent: Option<Gold>,
    },
    FiatPrint {
        issuer: IssuerId,
        amount: Gold,
        route: CantillonRoute,
    },
    ResetPublicSpotBook,
    SetPublicSpotTender(PublicSpotTender),
    SetPublicDebtTender(PublicDebtTender),
    SetBankRepaymentTender(BankRepaymentTender),
    SetIssuerRepaymentTender(IssuerRepaymentTender),
    SetLaborWageTender(LaborWageTender),
    SetTaxReceivability(TaxReceivability),
    LevyTax {
        agent: AgentId,
        amount: Gold,
        due_tick: Tick,
    },
    SetDebtDueTick {
        debt: DebtId,
        due_tick: Tick,
    },
    SeedCommodityDebt {
        lender: AgentId,
        borrower: AgentId,
        principal: Gold,
        due: Gold,
        due_tick: Tick,
        purpose: DebtPurpose,
    },
    SeedStock {
        agent: AgentId,
        good: GoodId,
        qty: u32,
    },
    SetIssuerPolicy {
        issuer: IssuerId,
        policy: IssuerPolicy,
    },
    StopIssuerCredit {
        issuer: IssuerId,
    },
}

impl ScenarioName {
    pub fn parse(name: &str) -> Option<Self> {
        BUILTIN_SCENARIOS
            .iter()
            .find(|info| info.name == name)
            .map(|info| info.scenario)
    }

    pub fn agent_order_priority(self) -> &'static [AgentId] {
        match self {
            ScenarioName::EmergedGoldBankClaimSpotRefusalControl
            | ScenarioName::EmergedGoldBankClaimSpotLegalTender => &[AgentId(400)],
            _ => &[],
        }
    }

    pub fn kind(self) -> ScenarioKind {
        match self {
            ScenarioName::CrusoeSurvival
            | ScenarioName::CrusoeCapital
            | ScenarioName::CrusoeAbandon => ScenarioKind::AutarkyM0,
            ScenarioName::MarketBarterishGold
            | ScenarioName::MarketPriceDiscovery
            | ScenarioName::MarketNoMutualBenefit => ScenarioKind::MarketM1,
            ScenarioName::TimeMarketBasic
            | ScenarioName::RoundaboutCapital
            | ScenarioName::BorrowToBuild
            | ScenarioName::SoundMoney100Pct => ScenarioKind::MarketM2,
            ScenarioName::CommodityCreditNeutral
            | ScenarioName::FractionalReserve
            | ScenarioName::SuspensionOfConvertibility
            | ScenarioName::FiatCreditExpansion
            | ScenarioName::FiatFiscalCantillon
            | ScenarioName::CantillonIsolation
            | ScenarioName::EmergedGoldSoundControl
            | ScenarioName::EmergedGoldFiatDisplacement
            | ScenarioName::EmergedGoldFiatRefusalControl
            | ScenarioName::EmergedGoldFiatLegalTender
            | ScenarioName::EmergedGoldFiatDebtRefusalControl
            | ScenarioName::EmergedGoldFiatDebtLegalTender
            | ScenarioName::EmergedGoldBankClaimDebtRefusalControl
            | ScenarioName::EmergedGoldBankClaimDebtLegalTender
            | ScenarioName::EmergedGoldBankClaimSpotRefusalControl
            | ScenarioName::EmergedGoldBankClaimSpotLegalTender
            | ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl
            | ScenarioName::EmergedGoldBankLoanRepaymentClaimTender
            | ScenarioName::EmergedGoldFractionalReserve
            | ScenarioName::EmergedGoldFiatCreditExpansion
            | ScenarioName::EmergedGoldFiatWageRefusalControl
            | ScenarioName::EmergedGoldFiatWageLegalTender
            | ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl
            | ScenarioName::EmergedGoldIssuerRepaymentFiatTender
            | ScenarioName::EmergedGoldReserveLeashControl
            | ScenarioName::EmergedGoldSuspensionOfConvertibility
            | ScenarioName::EmergedGoldRedemptionRun
            | ScenarioName::EmergedGoldSuspendedRedemption
            | ScenarioName::EmergedGoldTaxSpecieControl
            | ScenarioName::EmergedGoldTaxFiatUnpayableDefaults
            | ScenarioName::EmergedGoldTaxDrivesFiatLabor
            | ScenarioName::EmergedGoldNoTaxIdleControl => ScenarioKind::MarketM3,
            ScenarioName::MengerSaltMoney
            | ScenarioName::MengerGoldMoney
            | ScenarioName::MengerMarketabilityDurability
            | ScenarioName::MengerTwoLayerSaleability => ScenarioKind::MarketV2,
        }
    }

    pub fn regime(self) -> Regime {
        match self {
            ScenarioName::FractionalReserve => Regime::FractionalConvertible,
            ScenarioName::SuspensionOfConvertibility => Regime::FractionalConvertible,
            ScenarioName::FiatCreditExpansion
            | ScenarioName::FiatFiscalCantillon
            | ScenarioName::CantillonIsolation
            | ScenarioName::EmergedGoldTaxFiatUnpayableDefaults
            | ScenarioName::EmergedGoldTaxDrivesFiatLabor
            | ScenarioName::EmergedGoldNoTaxIdleControl => Regime::Fiat,
            ScenarioName::CrusoeSurvival
            | ScenarioName::CrusoeCapital
            | ScenarioName::CrusoeAbandon
            | ScenarioName::MarketBarterishGold
            | ScenarioName::MarketPriceDiscovery
            | ScenarioName::MarketNoMutualBenefit
            | ScenarioName::TimeMarketBasic
            | ScenarioName::RoundaboutCapital
            | ScenarioName::BorrowToBuild
            | ScenarioName::SoundMoney100Pct
            | ScenarioName::CommodityCreditNeutral
            | ScenarioName::EmergedGoldSoundControl
            | ScenarioName::EmergedGoldFiatDisplacement
            | ScenarioName::EmergedGoldFiatRefusalControl
            | ScenarioName::EmergedGoldFiatLegalTender
            | ScenarioName::EmergedGoldFiatDebtRefusalControl
            | ScenarioName::EmergedGoldFiatDebtLegalTender
            | ScenarioName::EmergedGoldBankClaimDebtRefusalControl
            | ScenarioName::EmergedGoldBankClaimDebtLegalTender
            | ScenarioName::EmergedGoldBankClaimSpotRefusalControl
            | ScenarioName::EmergedGoldBankClaimSpotLegalTender
            | ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl
            | ScenarioName::EmergedGoldBankLoanRepaymentClaimTender
            | ScenarioName::EmergedGoldFractionalReserve
            | ScenarioName::EmergedGoldFiatCreditExpansion
            | ScenarioName::EmergedGoldFiatWageRefusalControl
            | ScenarioName::EmergedGoldFiatWageLegalTender
            | ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl
            | ScenarioName::EmergedGoldIssuerRepaymentFiatTender
            | ScenarioName::EmergedGoldReserveLeashControl
            | ScenarioName::EmergedGoldSuspensionOfConvertibility
            | ScenarioName::EmergedGoldRedemptionRun
            | ScenarioName::EmergedGoldSuspendedRedemption
            | ScenarioName::EmergedGoldTaxSpecieControl
            | ScenarioName::MengerSaltMoney
            | ScenarioName::MengerGoldMoney
            | ScenarioName::MengerMarketabilityDurability
            | ScenarioName::MengerTwoLayerSaleability => Regime::SoundGold,
        }
    }

    pub(crate) fn starts_with_fiat_issuer(self) -> bool {
        matches!(
            self,
            ScenarioName::FiatCreditExpansion
                | ScenarioName::FiatFiscalCantillon
                | ScenarioName::CantillonIsolation
                | ScenarioName::EmergedGoldFiatDisplacement
                | ScenarioName::EmergedGoldFiatRefusalControl
                | ScenarioName::EmergedGoldFiatLegalTender
                | ScenarioName::EmergedGoldFiatDebtRefusalControl
                | ScenarioName::EmergedGoldFiatDebtLegalTender
                | ScenarioName::EmergedGoldFiatCreditExpansion
                | ScenarioName::EmergedGoldFiatWageRefusalControl
                | ScenarioName::EmergedGoldFiatWageLegalTender
                | ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl
                | ScenarioName::EmergedGoldIssuerRepaymentFiatTender
                | ScenarioName::EmergedGoldTaxSpecieControl
                | ScenarioName::EmergedGoldTaxFiatUnpayableDefaults
                | ScenarioName::EmergedGoldTaxDrivesFiatLabor
                | ScenarioName::EmergedGoldNoTaxIdleControl
        )
    }

    pub(crate) fn project_lines(self) -> ScenarioProjectLines {
        match self {
            ScenarioName::EmergedGoldSoundControl
            | ScenarioName::EmergedGoldFiatDisplacement
            | ScenarioName::EmergedGoldFiatRefusalControl
            | ScenarioName::EmergedGoldFiatLegalTender
            | ScenarioName::EmergedGoldFiatDebtRefusalControl
            | ScenarioName::EmergedGoldFiatDebtLegalTender
            | ScenarioName::EmergedGoldTaxSpecieControl
            | ScenarioName::EmergedGoldTaxFiatUnpayableDefaults => ScenarioProjectLines::None,
            ScenarioName::EmergedGoldTaxDrivesFiatLabor
            | ScenarioName::EmergedGoldNoTaxIdleControl => ScenarioProjectLines::BorrowToBuild,
            ScenarioName::BorrowToBuild
            | ScenarioName::FractionalReserve
            | ScenarioName::SuspensionOfConvertibility
            | ScenarioName::EmergedGoldFractionalReserve
            | ScenarioName::EmergedGoldReserveLeashControl
            | ScenarioName::EmergedGoldSuspensionOfConvertibility
            | ScenarioName::EmergedGoldRedemptionRun
            | ScenarioName::EmergedGoldSuspendedRedemption
            | ScenarioName::EmergedGoldBankClaimDebtRefusalControl
            | ScenarioName::EmergedGoldBankClaimDebtLegalTender
            | ScenarioName::EmergedGoldBankClaimSpotRefusalControl
            | ScenarioName::EmergedGoldBankClaimSpotLegalTender
            | ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl
            | ScenarioName::EmergedGoldBankLoanRepaymentClaimTender => {
                ScenarioProjectLines::BorrowToBuild
            }
            ScenarioName::FiatCreditExpansion
            | ScenarioName::EmergedGoldFiatCreditExpansion
            | ScenarioName::EmergedGoldFiatWageRefusalControl
            | ScenarioName::EmergedGoldFiatWageLegalTender
            | ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl
            | ScenarioName::EmergedGoldIssuerRepaymentFiatTender => {
                ScenarioProjectLines::CreditBoomLong
            }
            _ => ScenarioProjectLines::Builtin,
        }
    }
}

pub fn scenario_infos() -> &'static [ScenarioInfo] {
    BUILTIN_SCENARIOS
}

pub fn scenario_names() -> Vec<&'static str> {
    BUILTIN_SCENARIOS.iter().map(|info| info.name).collect()
}

pub fn scenario_description(name: ScenarioName) -> &'static str {
    scenario_info(name).description
}

pub fn builtin_scenario(name: ScenarioName) -> Scenario {
    match name {
        ScenarioName::CrusoeSurvival => crusoe_survival(),
        ScenarioName::CrusoeCapital => crusoe_capital(),
        ScenarioName::CrusoeAbandon => crusoe_abandon(),
        ScenarioName::MarketBarterishGold
        | ScenarioName::MarketPriceDiscovery
        | ScenarioName::MarketNoMutualBenefit
        | ScenarioName::TimeMarketBasic
        | ScenarioName::RoundaboutCapital
        | ScenarioName::BorrowToBuild
        | ScenarioName::SoundMoney100Pct
        | ScenarioName::CommodityCreditNeutral
        | ScenarioName::FractionalReserve
        | ScenarioName::SuspensionOfConvertibility
        | ScenarioName::FiatCreditExpansion
        | ScenarioName::FiatFiscalCantillon
        | ScenarioName::CantillonIsolation
        | ScenarioName::EmergedGoldSoundControl
        | ScenarioName::EmergedGoldFiatDisplacement
        | ScenarioName::EmergedGoldFiatRefusalControl
        | ScenarioName::EmergedGoldFiatLegalTender
        | ScenarioName::EmergedGoldFiatDebtRefusalControl
        | ScenarioName::EmergedGoldFiatDebtLegalTender
        | ScenarioName::EmergedGoldBankClaimDebtRefusalControl
        | ScenarioName::EmergedGoldBankClaimDebtLegalTender
        | ScenarioName::EmergedGoldBankClaimSpotRefusalControl
        | ScenarioName::EmergedGoldBankClaimSpotLegalTender
        | ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl
        | ScenarioName::EmergedGoldBankLoanRepaymentClaimTender
        | ScenarioName::EmergedGoldFractionalReserve
        | ScenarioName::EmergedGoldFiatCreditExpansion
        | ScenarioName::EmergedGoldFiatWageRefusalControl
        | ScenarioName::EmergedGoldFiatWageLegalTender
        | ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl
        | ScenarioName::EmergedGoldIssuerRepaymentFiatTender
        | ScenarioName::EmergedGoldReserveLeashControl
        | ScenarioName::EmergedGoldSuspensionOfConvertibility
        | ScenarioName::EmergedGoldRedemptionRun
        | ScenarioName::EmergedGoldSuspendedRedemption
        | ScenarioName::EmergedGoldTaxSpecieControl
        | ScenarioName::EmergedGoldTaxFiatUnpayableDefaults
        | ScenarioName::EmergedGoldTaxDrivesFiatLabor
        | ScenarioName::EmergedGoldNoTaxIdleControl
        | ScenarioName::MengerSaltMoney
        | ScenarioName::MengerGoldMoney
        | ScenarioName::MengerMarketabilityDurability
        | ScenarioName::MengerTwoLayerSaleability => {
            panic!("market scenarios use builtin_market_scenario")
        }
    }
}

pub fn builtin_market_scenario(name: ScenarioName) -> MarketScenario {
    match name {
        ScenarioName::MarketBarterishGold => market_barterish_gold(),
        ScenarioName::MarketPriceDiscovery => market_price_discovery(),
        ScenarioName::MarketNoMutualBenefit => market_no_mutual_benefit(),
        ScenarioName::TimeMarketBasic => time_market_basic(),
        ScenarioName::RoundaboutCapital => roundabout_capital(),
        ScenarioName::BorrowToBuild => borrow_to_build(),
        ScenarioName::SoundMoney100Pct => sound_money_100pct(),
        ScenarioName::CommodityCreditNeutral => commodity_credit_neutral(),
        ScenarioName::SuspensionOfConvertibility => suspension_of_convertibility(),
        ScenarioName::FiatCreditExpansion => fiat_credit_expansion(),
        ScenarioName::FiatFiscalCantillon => fiat_fiscal_cantillon(),
        ScenarioName::CantillonIsolation => cantillon_isolation(),
        ScenarioName::EmergedGoldSoundControl => emerged_gold_sound_control(),
        ScenarioName::EmergedGoldFiatDisplacement => emerged_gold_fiat_displacement(),
        ScenarioName::EmergedGoldFiatRefusalControl => emerged_gold_fiat_refusal_control(),
        ScenarioName::EmergedGoldFiatLegalTender => emerged_gold_fiat_legal_tender(),
        ScenarioName::EmergedGoldFiatDebtRefusalControl => emerged_gold_fiat_debt_refusal_control(),
        ScenarioName::EmergedGoldFiatDebtLegalTender => emerged_gold_fiat_debt_legal_tender(),
        ScenarioName::EmergedGoldBankClaimDebtRefusalControl => {
            emerged_gold_bank_claim_debt_refusal_control()
        }
        ScenarioName::EmergedGoldBankClaimDebtLegalTender => {
            emerged_gold_bank_claim_debt_legal_tender()
        }
        ScenarioName::EmergedGoldBankClaimSpotRefusalControl => {
            emerged_gold_bank_claim_spot_refusal_control()
        }
        ScenarioName::EmergedGoldBankClaimSpotLegalTender => {
            emerged_gold_bank_claim_spot_legal_tender()
        }
        ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl => {
            emerged_gold_bank_loan_repayment_refusal_control()
        }
        ScenarioName::EmergedGoldBankLoanRepaymentClaimTender => {
            emerged_gold_bank_loan_repayment_claim_tender()
        }
        ScenarioName::EmergedGoldFractionalReserve => emerged_gold_fractional_reserve(),
        ScenarioName::EmergedGoldFiatCreditExpansion => emerged_gold_fiat_credit_expansion(),
        ScenarioName::EmergedGoldFiatWageRefusalControl => emerged_gold_fiat_wage_refusal_control(),
        ScenarioName::EmergedGoldFiatWageLegalTender => emerged_gold_fiat_wage_legal_tender(),
        ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl => {
            emerged_gold_issuer_repayment_fiat_refusal_control()
        }
        ScenarioName::EmergedGoldIssuerRepaymentFiatTender => {
            emerged_gold_issuer_repayment_fiat_tender()
        }
        ScenarioName::EmergedGoldReserveLeashControl => emerged_gold_reserve_leash_control(),
        ScenarioName::EmergedGoldSuspensionOfConvertibility => {
            emerged_gold_suspension_of_convertibility()
        }
        ScenarioName::EmergedGoldRedemptionRun => emerged_gold_redemption_run(),
        ScenarioName::EmergedGoldSuspendedRedemption => emerged_gold_suspended_redemption(),
        ScenarioName::EmergedGoldTaxSpecieControl => emerged_gold_tax_specie_control(),
        ScenarioName::EmergedGoldTaxFiatUnpayableDefaults => {
            emerged_gold_tax_fiat_unpayable_defaults()
        }
        ScenarioName::EmergedGoldTaxDrivesFiatLabor => emerged_gold_tax_drives_fiat_labor(),
        ScenarioName::EmergedGoldNoTaxIdleControl => emerged_gold_no_tax_idle_control(),
        ScenarioName::FractionalReserve => fractional_reserve(),
        ScenarioName::MengerSaltMoney => menger_salt_money(),
        ScenarioName::MengerGoldMoney => menger_gold_money(),
        ScenarioName::MengerMarketabilityDurability => menger_marketability_durability(),
        ScenarioName::MengerTwoLayerSaleability => menger_two_layer_saleability(),
        ScenarioName::CrusoeSurvival
        | ScenarioName::CrusoeCapital
        | ScenarioName::CrusoeAbandon => {
            panic!("M0 scenarios use builtin_scenario")
        }
    }
}

fn scenario_info(name: ScenarioName) -> &'static ScenarioInfo {
    BUILTIN_SCENARIOS
        .iter()
        .find(|info| info.scenario == name)
        .expect("all scenario names have metadata")
}

fn stock(food: u32, wood: u32, nets: u32) -> Stock {
    let mut stock = Stock::new(3);
    stock.add(FOOD, food);
    stock.add(WOOD, wood);
    stock.add(NET, nets);
    stock
}

pub fn v2_stock(food: u32, wood: u32, salt: u32, cloth: u32, ore: u32) -> Stock {
    v2_stock_with_gold(0, food, wood, salt, cloth, ore)
}

fn v2_stock_with_net(food: u32, wood: u32, net: u32, salt: u32, cloth: u32, ore: u32) -> Stock {
    let mut stock = v2_stock(food, wood, salt, cloth, ore);
    stock.add(NET, net);
    stock
}

fn v2_stock_with_gold(gold: u32, food: u32, wood: u32, salt: u32, cloth: u32, ore: u32) -> Stock {
    let mut stock = Stock::new(ORE.0);
    stock.add(GOLD, gold);
    stock.add(FOOD, food);
    stock.add(WOOD, wood);
    stock.add(SALT, salt);
    stock.add(CLOTH, cloth);
    stock.add(ORE, ore);
    stock
}

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

fn base_agent(scale: Vec<Want>, stock: Stock, labor_capacity: u32) -> Agent {
    Agent {
        id: AgentId(1),
        scale,
        stock,
        gold: Gold::ZERO,
        labor_capacity,
        hunger_deficit: 0,
        roles: vec![Role::Household],
        expect: Vec::new(),
    }
}

fn crusoe_survival() -> Scenario {
    let mut scale = Vec::new();
    repeat(&mut scale, WantKind::Good(FOOD), Horizon::Now, 1);
    repeat(&mut scale, WantKind::Good(FOOD), Horizon::Next, 1);
    repeat(&mut scale, WantKind::Leisure, Horizon::Now, 2);
    repeat(&mut scale, WantKind::Good(FOOD), Horizon::Later(3), 1);

    Scenario {
        name: scenario_info(ScenarioName::CrusoeSurvival).name,
        seed: 1,
        periods: 12,
        agent: base_agent(scale, stock(0, 0, 0), 3),
        recipes: builtin_recipes(),
        project_templates: builtin_project_templates(),
        initial_projects: Vec::new(),
        events: Vec::new(),
    }
}

fn crusoe_capital() -> Scenario {
    let mut scale = Vec::new();
    repeat(&mut scale, WantKind::Good(FOOD), Horizon::Now, 1);
    repeat(&mut scale, WantKind::Good(FOOD), Horizon::Next, 2);
    repeat(&mut scale, WantKind::Good(FOOD), Horizon::Later(3), 6);
    repeat(&mut scale, WantKind::Leisure, Horizon::Now, 1);
    repeat(&mut scale, WantKind::Good(FOOD), Horizon::Later(6), 3);
    repeat(&mut scale, WantKind::Leisure, Horizon::Now, 4);

    Scenario {
        name: scenario_info(ScenarioName::CrusoeCapital).name,
        seed: 2,
        periods: 20,
        agent: base_agent(scale, stock(1, 0, 0), 4),
        recipes: builtin_recipes(),
        project_templates: builtin_project_templates(),
        initial_projects: Vec::new(),
        events: Vec::new(),
    }
}

fn crusoe_abandon() -> Scenario {
    let mut scale = Vec::new();
    repeat(&mut scale, WantKind::Good(FOOD), Horizon::Now, 1);
    repeat(&mut scale, WantKind::Good(FOOD), Horizon::Later(4), 1);
    repeat(&mut scale, WantKind::Leisure, Horizon::Now, 2);

    let template = build_net_template();
    let project = Project {
        id: ProjectId(1),
        template: template.id,
        state: ProjectState::Forming,
        started_at: Tick(0),
        labor_advanced: 1,
        input_goods_committed: template.input_goods.clone(),
        output_good: template.output_good,
        output_qty: template.output_qty,
        salvage_bps: template.salvage_bps,
    };

    Scenario {
        name: scenario_info(ScenarioName::CrusoeAbandon).name,
        seed: 3,
        periods: 12,
        agent: base_agent(scale, stock(2, 0, 0), 1),
        recipes: builtin_recipes(),
        project_templates: builtin_project_templates(),
        initial_projects: vec![project],
        events: vec![Event {
            tick: Tick(1),
            kind: EventKind::DisableRecipe(RecipeId::FishWithNet),
        }],
    }
}

fn market_barterish_gold() -> MarketScenario {
    let step = Gold(1);
    MarketScenario {
        name: scenario_info(ScenarioName::MarketBarterishGold).name,
        scenario: ScenarioName::MarketBarterishGold,
        seed: 10,
        periods: 12,
        agents: vec![
            market_agent(
                1,
                Gold(10),
                stock(0, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 7),
                    (WantKind::Good(FOOD), Horizon::Next, 2),
                    (WantKind::Leisure, Horizon::Now, 2),
                ]),
                Gold(3),
                Gold(3),
                step,
            ),
            market_agent(
                2,
                Gold(0),
                stock(5, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 8),
                    (WantKind::Leisure, Horizon::Now, 2),
                ]),
                Gold(3),
                Gold(3),
                step,
            ),
            market_agent(
                3,
                Gold(9),
                stock(0, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 6),
                    (WantKind::Good(WOOD), Horizon::Next, 2),
                    (WantKind::Leisure, Horizon::Now, 2),
                ]),
                Gold(3),
                Gold(3),
                step,
            ),
            market_agent(
                4,
                Gold(0),
                stock(0, 6, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 8),
                    (WantKind::Leisure, Horizon::Now, 2),
                ]),
                Gold(3),
                Gold(3),
                step,
            ),
        ],
        recipes: builtin_recipes(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn market_price_discovery() -> MarketScenario {
    let step = Gold(1);
    MarketScenario {
        name: scenario_info(ScenarioName::MarketPriceDiscovery).name,
        scenario: ScenarioName::MarketPriceDiscovery,
        seed: 11,
        periods: 20,
        agents: vec![
            market_agent(
                1,
                Gold(0),
                stock(6, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 8),
                    (WantKind::Leisure, Horizon::Now, 2),
                ]),
                Gold(9),
                Gold(9),
                step,
            ),
            market_agent(
                2,
                Gold(0),
                stock(6, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 8),
                    (WantKind::Leisure, Horizon::Now, 2),
                ]),
                Gold(8),
                Gold(8),
                step,
            ),
            market_agent(
                3,
                Gold(0),
                stock(6, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 8),
                    (WantKind::Leisure, Horizon::Now, 2),
                ]),
                Gold(7),
                Gold(7),
                step,
            ),
            market_agent(
                4,
                Gold(20),
                stock(0, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 4),
                    (WantKind::Good(FOOD), Horizon::Next, 3),
                    (WantKind::Leisure, Horizon::Now, 2),
                ]),
                Gold(1),
                Gold(1),
                step,
            ),
            market_agent(
                5,
                Gold(20),
                stock(0, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 6),
                    (WantKind::Good(FOOD), Horizon::Next, 3),
                    (WantKind::Leisure, Horizon::Now, 2),
                ]),
                Gold(1),
                Gold(1),
                step,
            ),
            market_agent(
                6,
                Gold(20),
                stock(0, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Later(1), 7),
                    (WantKind::Good(FOOD), Horizon::Next, 3),
                    (WantKind::Leisure, Horizon::Now, 2),
                ]),
                Gold(2),
                Gold(2),
                step,
            ),
        ],
        recipes: builtin_recipes(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn market_no_mutual_benefit() -> MarketScenario {
    let step = Gold(1);
    MarketScenario {
        name: scenario_info(ScenarioName::MarketNoMutualBenefit).name,
        scenario: ScenarioName::MarketNoMutualBenefit,
        seed: 12,
        periods: 8,
        agents: vec![
            market_agent(
                1,
                Gold(5),
                stock(3, 0, 0),
                scale(&[
                    (WantKind::Good(FOOD), Horizon::Next, 3),
                    (WantKind::Good(GOLD), Horizon::Later(1), 5),
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                    (WantKind::Leisure, Horizon::Now, 2),
                ]),
                Gold(3),
                Gold(3),
                step,
            ),
            market_agent(
                2,
                Gold(5),
                stock(0, 3, 0),
                scale(&[
                    (WantKind::Good(WOOD), Horizon::Next, 3),
                    (WantKind::Good(GOLD), Horizon::Later(1), 5),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                    (WantKind::Leisure, Horizon::Now, 2),
                ]),
                Gold(3),
                Gold(3),
                step,
            ),
        ],
        recipes: builtin_recipes(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn menger_salt_money() -> MarketScenario {
    MarketScenario {
        name: scenario_info(ScenarioName::MengerSaltMoney).name,
        scenario: ScenarioName::MengerSaltMoney,
        seed: 50,
        periods: 12,
        agents: vec![
            v2_agent(
                1,
                v2_stock(3, 0, 0, 0, 0),
                scale(&[
                    (WantKind::Good(SALT), Horizon::Next, 3),
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                2,
                v2_stock(3, 0, 0, 0, 0),
                scale(&[
                    (WantKind::Good(SALT), Horizon::Next, 3),
                    (WantKind::Good(CLOTH), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                3,
                v2_stock(0, 3, 0, 0, 0),
                scale(&[
                    (WantKind::Good(SALT), Horizon::Next, 3),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                4,
                v2_stock(0, 3, 0, 0, 0),
                scale(&[
                    (WantKind::Good(SALT), Horizon::Next, 3),
                    (WantKind::Good(CLOTH), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                5,
                v2_stock(0, 0, 0, 3, 0),
                scale(&[
                    (WantKind::Good(SALT), Horizon::Next, 3),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                6,
                v2_stock(0, 0, 0, 0, 1),
                scale(&[(WantKind::Good(WOOD), Horizon::Next, 1)]),
            ),
            v2_agent(
                7,
                v2_stock(0, 0, 8, 0, 0),
                scale(&[
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                    (WantKind::Good(ORE), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                    (WantKind::Good(CLOTH), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                8,
                v2_stock(0, 0, 8, 0, 0),
                scale(&[
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                    (WantKind::Good(CLOTH), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                    (WantKind::Good(ORE), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                9,
                v2_stock(2, 2, 0, 1, 0),
                scale(&[(WantKind::Good(SALT), Horizon::Later(4), 6)]),
            ),
            v2_agent(
                10,
                v2_stock(0, 0, 0, 1, 3),
                scale(&[
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                    (WantKind::Good(SALT), Horizon::Later(4), 6),
                ]),
            ),
        ],
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Emergent(MengerianConfig {
            candidate_goods: vec![FOOD, WOOD, SALT, CLOTH, ORE],
            min_total_acceptances: 12,
            promotion_threshold_bps: 4_500,
            lead_margin_bps: 1_500,
            min_acceptor_agents: 3,
            min_counterpart_goods: 2,
            stability_ticks: 2,
            indirect_min_acceptance_share_bps: 3_000,
            min_indirect_acceptances: 0,
            min_indirect_acceptor_agents: 0,
            min_indirect_target_goods: 0,
            allow_indirect_acceptance: true,
            multi_offer_medium: false,
            durability_aware_acceptance: false,
            two_layer_saleability: false,
            min_direct_use_acceptors: 0,
            marketability: Default::default(),
        }),
    }
}

fn menger_marketability_durability() -> MarketScenario {
    let marketability = MarketabilityConfig {
        hold_horizon: 1,
        ..MarketabilityConfig::default()
    }
    .with_good(
        FOOD,
        GoodMarketability {
            decay_bps: 10_000,
            carry_cost: 0,
        },
    )
    .with_good(
        WOOD,
        GoodMarketability {
            decay_bps: 0,
            carry_cost: 1,
        },
    )
    .with_good(
        SALT,
        GoodMarketability {
            decay_bps: 0,
            carry_cost: 0,
        },
    );

    MarketScenario {
        name: scenario_info(ScenarioName::MengerMarketabilityDurability).name,
        scenario: ScenarioName::MengerMarketabilityDurability,
        seed: 52,
        periods: 4,
        agents: vec![
            v2_agent(
                1,
                v2_stock(1, 0, 0, 0, 0),
                scale(&[
                    (WantKind::Good(CLOTH), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                2,
                v2_stock(0, 0, 0, 1, 0),
                scale(&[(WantKind::Good(FOOD), Horizon::Next, 1)]),
            ),
            v2_agent(
                3,
                v2_stock(1, 0, 0, 0, 0),
                scale(&[
                    (WantKind::Good(ORE), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                4,
                v2_stock(0, 0, 0, 0, 1),
                scale(&[(WantKind::Good(FOOD), Horizon::Next, 1)]),
            ),
            v2_agent(
                5,
                v2_stock(1, 0, 0, 0, 0),
                scale(&[
                    (WantKind::Good(CLOTH), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                6,
                v2_stock(0, 0, 0, 1, 0),
                scale(&[(WantKind::Good(FOOD), Horizon::Next, 1)]),
            ),
            v2_agent(
                7,
                v2_stock(1, 0, 0, 0, 0),
                scale(&[
                    (WantKind::Good(ORE), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                8,
                v2_stock(0, 0, 0, 0, 1),
                scale(&[(WantKind::Good(FOOD), Horizon::Next, 1)]),
            ),
            v2_agent(
                9,
                v2_stock(0, 0, 1, 0, 0),
                scale(&[
                    (WantKind::Good(CLOTH), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                10,
                v2_stock(0, 0, 0, 1, 0),
                scale(&[
                    (WantKind::Good(SALT), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                11,
                v2_stock(0, 0, 1, 0, 0),
                scale(&[
                    (WantKind::Good(ORE), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                12,
                v2_stock(0, 0, 0, 0, 1),
                scale(&[
                    (WantKind::Good(SALT), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                13,
                v2_stock(0, 0, 1, 0, 0),
                scale(&[
                    (WantKind::Good(CLOTH), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                14,
                v2_stock(0, 0, 0, 1, 0),
                scale(&[
                    (WantKind::Good(SALT), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                15,
                v2_stock_with_net(1, 0, 0, 0, 0, 0),
                scale(&[
                    (WantKind::Good(NET), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                16,
                v2_stock_with_net(0, 0, 1, 0, 0, 0),
                scale(&[(WantKind::Good(CLOTH), Horizon::Next, 1)]),
            ),
            v2_agent(
                17,
                v2_stock_with_net(1, 0, 0, 0, 0, 0),
                scale(&[
                    (WantKind::Good(NET), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                18,
                v2_stock_with_net(0, 0, 1, 0, 0, 0),
                scale(&[(WantKind::Good(ORE), Horizon::Next, 1)]),
            ),
            v2_agent(
                19,
                v2_stock(0, 1, 0, 0, 0),
                scale(&[
                    (WantKind::Good(CLOTH), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                20,
                v2_stock(0, 0, 0, 1, 0),
                scale(&[
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
        ],
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Emergent(MengerianConfig {
            candidate_goods: vec![FOOD, WOOD, SALT],
            min_total_acceptances: 8,
            promotion_threshold_bps: 9_900,
            lead_margin_bps: 250,
            min_acceptor_agents: 2,
            min_counterpart_goods: 2,
            stability_ticks: 50,
            indirect_min_acceptance_share_bps: 2_000,
            durability_aware_acceptance: true,
            marketability,
            ..MengerianConfig::default()
        }),
    }
}

fn menger_two_layer_saleability() -> MarketScenario {
    let mut scenario = menger_marketability_durability();
    scenario.name = scenario_info(ScenarioName::MengerTwoLayerSaleability).name;
    scenario.scenario = ScenarioName::MengerTwoLayerSaleability;
    scenario.periods = 6;
    scenario.agents.extend([
        v2_agent(
            21,
            v2_stock(1, 0, 0, 0, 0),
            scale(&[(WantKind::Good(CLOTH), Horizon::Next, 1)]),
        ),
        v2_agent(
            22,
            v2_stock(0, 0, 1, 0, 0),
            scale(&[(WantKind::Good(FOOD), Horizon::Next, 1)]),
        ),
    ]);
    let MarketMoneyConfig::Emergent(config) = &mut scenario.money else {
        panic!("marketability scenario must use emergent money");
    };
    config.two_layer_saleability = true;
    config.min_direct_use_acceptors = 2;
    config.multi_offer_medium = true;
    config.stability_ticks = 1;
    config.promotion_threshold_bps = 5_000;
    config.lead_margin_bps = 250;
    config.indirect_min_acceptance_share_bps = 1;
    config.min_indirect_acceptances = 1;
    config.min_indirect_acceptor_agents = 1;
    config.min_indirect_target_goods = 1;
    scenario
}

fn menger_gold_money() -> MarketScenario {
    MarketScenario {
        name: scenario_info(ScenarioName::MengerGoldMoney).name,
        scenario: ScenarioName::MengerGoldMoney,
        seed: 51,
        periods: 12,
        agents: vec![
            v2_agent(
                1,
                v2_stock_with_gold(0, 3, 0, 0, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Next, 3),
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                2,
                v2_stock_with_gold(0, 3, 0, 0, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Next, 3),
                    (WantKind::Good(CLOTH), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                3,
                v2_stock_with_gold(0, 0, 3, 0, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Next, 3),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                4,
                v2_stock_with_gold(0, 0, 3, 0, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Next, 3),
                    (WantKind::Good(CLOTH), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                5,
                v2_stock_with_gold(0, 0, 0, 0, 3, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Next, 3),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                6,
                v2_stock_with_gold(0, 0, 0, 0, 0, 1),
                scale(&[(WantKind::Good(WOOD), Horizon::Next, 1)]),
            ),
            v2_agent(
                7,
                v2_stock_with_gold(8, 0, 0, 0, 0, 0),
                scale(&[
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                    (WantKind::Good(ORE), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                    (WantKind::Good(CLOTH), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                8,
                v2_stock_with_gold(8, 0, 0, 0, 0, 0),
                scale(&[
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                    (WantKind::Good(CLOTH), Horizon::Next, 1),
                    (WantKind::Good(FOOD), Horizon::Next, 1),
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                    (WantKind::Good(ORE), Horizon::Next, 1),
                ]),
            ),
            v2_agent(
                9,
                v2_stock_with_gold(0, 2, 2, 0, 1, 0),
                scale(&[(WantKind::Good(GOLD), Horizon::Later(4), 6)]),
            ),
            v2_agent(
                10,
                v2_stock_with_gold(0, 0, 0, 0, 1, 3),
                scale(&[
                    (WantKind::Good(WOOD), Horizon::Next, 1),
                    (WantKind::Good(GOLD), Horizon::Later(4), 6),
                ]),
            ),
        ],
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Emergent(MengerianConfig {
            candidate_goods: vec![GOLD, FOOD, WOOD, SALT, CLOTH, ORE],
            min_total_acceptances: 12,
            promotion_threshold_bps: 4_500,
            lead_margin_bps: 1_500,
            min_acceptor_agents: 3,
            min_counterpart_goods: 2,
            stability_ticks: 2,
            indirect_min_acceptance_share_bps: 3_000,
            min_indirect_acceptances: 0,
            min_indirect_acceptor_agents: 0,
            min_indirect_target_goods: 0,
            allow_indirect_acceptance: true,
            multi_offer_medium: false,
            durability_aware_acceptance: false,
            two_layer_saleability: false,
            min_direct_use_acceptors: 0,
            marketability: Default::default(),
        }),
    }
}

pub fn emerged_gold_bridge_agents() -> Vec<Agent> {
    // These are the M6 value scales at promotion, including repeated entries
    // that weight the first receivers' demand under the M3 runner.
    vec![
        bridge_agent(
            1,
            2,
            1,
            0,
            0,
            0,
            scale(&[
                (WantKind::Good(GOLD), Horizon::Next, 3),
                (WantKind::Good(WOOD), Horizon::Next, 1),
            ]),
        ),
        bridge_agent(
            2,
            2,
            1,
            0,
            0,
            0,
            scale(&[
                (WantKind::Good(GOLD), Horizon::Next, 3),
                (WantKind::Good(CLOTH), Horizon::Next, 1),
            ]),
        ),
        bridge_agent(
            3,
            2,
            0,
            1,
            0,
            0,
            scale(&[
                (WantKind::Good(GOLD), Horizon::Next, 3),
                (WantKind::Good(FOOD), Horizon::Next, 1),
            ]),
        ),
        bridge_agent(
            4,
            1,
            0,
            2,
            0,
            0,
            scale(&[
                (WantKind::Good(GOLD), Horizon::Next, 3),
                (WantKind::Good(CLOTH), Horizon::Next, 1),
            ]),
        ),
        bridge_agent(
            5,
            1,
            0,
            0,
            2,
            0,
            scale(&[
                (WantKind::Good(GOLD), Horizon::Next, 3),
                (WantKind::Good(FOOD), Horizon::Next, 1),
            ]),
        ),
        bridge_agent(
            6,
            0,
            0,
            1,
            0,
            0,
            scale(&[(WantKind::Good(WOOD), Horizon::Next, 1)]),
        ),
        bridge_agent(
            7,
            4,
            2,
            1,
            0,
            1,
            scale(&[
                (WantKind::Good(FOOD), Horizon::Next, 1),
                (WantKind::Good(WOOD), Horizon::Next, 1),
                (WantKind::Good(ORE), Horizon::Next, 1),
                (WantKind::Good(FOOD), Horizon::Next, 1),
                (WantKind::Good(WOOD), Horizon::Next, 1),
                (WantKind::Good(CLOTH), Horizon::Next, 1),
            ]),
        ),
        bridge_agent(
            8,
            4,
            2,
            1,
            1,
            0,
            scale(&[
                (WantKind::Good(FOOD), Horizon::Next, 1),
                (WantKind::Good(WOOD), Horizon::Next, 1),
                (WantKind::Good(CLOTH), Horizon::Next, 1),
                (WantKind::Good(FOOD), Horizon::Next, 1),
                (WantKind::Good(WOOD), Horizon::Next, 1),
                (WantKind::Good(ORE), Horizon::Next, 1),
            ]),
        ),
        bridge_agent(
            9,
            0,
            2,
            2,
            1,
            0,
            scale(&[(WantKind::Good(GOLD), Horizon::Later(4), 6)]),
        ),
        bridge_agent(
            10,
            0,
            0,
            0,
            1,
            3,
            scale(&[
                (WantKind::Good(WOOD), Horizon::Next, 1),
                (WantKind::Good(GOLD), Horizon::Later(4), 6),
            ]),
        ),
    ]
}

fn bridge_agent(
    id: u32,
    gold: u64,
    food: u32,
    wood: u32,
    cloth: u32,
    ore: u32,
    scale: Vec<Want>,
) -> Agent {
    Agent {
        gold: Gold(gold),
        ..v2_agent(id, emerged_gold_bridge_stock(food, wood, cloth, ore), scale)
    }
}

fn emerged_gold_bridge_stock(food: u32, wood: u32, cloth: u32, ore: u32) -> Stock {
    v2_stock_with_gold(0, food, wood, 0, cloth, ore)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EmergedGoldCreditLadderProfile {
    FractionalReserve,
    FiatCreditExpansion,
}

fn emerged_gold_credit_ladder_agents(profile: EmergedGoldCreditLadderProfile) -> Vec<Agent> {
    let mut agents = emerged_gold_bridge_agents();
    match profile {
        EmergedGoldCreditLadderProfile::FractionalReserve => {
            agents.extend(organic_credit_pair(100));
            agents.extend(project_cluster(120, 6, 7, 6, Gold(2)));
        }
        EmergedGoldCreditLadderProfile::FiatCreditExpansion => {
            agents.extend(organic_credit_pair(100));
            agents.extend(organic_credit_pair_with_horizon(110, 1));
            agents.extend(project_cluster(200, 8, 13, 30, Gold(2)));
            for id in 300..316 {
                agents.push(project_worker(id));
            }
        }
    }
    agents
}

fn standard_bank_credit_policy(max_new_fiduciary_per_tick: Gold) -> BankPolicy {
    BankPolicy {
        max_new_fiduciary_per_tick,
        loan_present: Gold(1),
        loan_horizon: 7,
        loan_future_due: Gold(1),
        enabled: true,
    }
}

fn emerged_gold_sound_control() -> MarketScenario {
    MarketScenario {
        name: scenario_info(ScenarioName::EmergedGoldSoundControl).name,
        scenario: ScenarioName::EmergedGoldSoundControl,
        seed: 52,
        periods: 8,
        agents: emerged_gold_bridge_agents(),
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn emerged_gold_fiat_displacement() -> MarketScenario {
    let mut scenario = emerged_gold_sound_control();
    scenario.name = scenario_info(ScenarioName::EmergedGoldFiatDisplacement).name;
    scenario.scenario = ScenarioName::EmergedGoldFiatDisplacement;
    scenario.events = vec![
        Event {
            tick: Tick(0),
            kind: EventKind::SetRegime(Regime::Fiat),
        },
        Event {
            tick: Tick(0),
            kind: EventKind::SetIssuerPolicy {
                issuer: IssuerId(1),
                policy: fiscal_issuer_policy(Gold(8)),
            },
        },
        Event {
            tick: Tick(0),
            kind: EventKind::FiatPrint {
                issuer: IssuerId(1),
                amount: Gold(8),
                route: CantillonRoute::Agents(EMERGED_GOLD_FIRST_RECEIVERS.to_vec()),
            },
        },
    ];
    scenario
}

fn emerged_gold_fiat_with_tender(
    scenario_name: ScenarioName,
    tender: PublicSpotTender,
) -> MarketScenario {
    let mut scenario = emerged_gold_fiat_displacement();
    scenario.name = scenario_info(scenario_name).name;
    scenario.scenario = scenario_name;
    scenario.events.push(Event {
        tick: Tick(0),
        kind: EventKind::SetPublicSpotTender(tender),
    });
    scenario
}

fn emerged_gold_fiat_refusal_control() -> MarketScenario {
    emerged_gold_fiat_with_tender(
        ScenarioName::EmergedGoldFiatRefusalControl,
        PublicSpotTender::SpecieOnly,
    )
}

fn emerged_gold_fiat_legal_tender() -> MarketScenario {
    emerged_gold_fiat_with_tender(
        ScenarioName::EmergedGoldFiatLegalTender,
        PublicSpotTender::FiatAndSpecie,
    )
}

fn emerged_gold_fiat_debt_with_tender(
    scenario_name: ScenarioName,
    debt_tender: PublicDebtTender,
) -> MarketScenario {
    let mut scenario = emerged_gold_fiat_displacement();
    scenario.name = scenario_info(scenario_name).name;
    scenario.scenario = scenario_name;
    scenario.events.extend([
        Event {
            tick: Tick(0),
            kind: EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly),
        },
        Event {
            tick: Tick(0),
            kind: EventKind::SetPublicDebtTender(debt_tender),
        },
        Event {
            tick: Tick(0),
            kind: EventKind::SeedCommodityDebt {
                lender: AgentId(1),
                borrower: AgentId(7),
                principal: Gold(4),
                due: Gold(4),
                due_tick: Tick(0),
                purpose: DebtPurpose::Consumption,
            },
        },
    ]);
    scenario
}

fn emerged_gold_fiat_debt_refusal_control() -> MarketScenario {
    emerged_gold_fiat_debt_with_tender(
        ScenarioName::EmergedGoldFiatDebtRefusalControl,
        PublicDebtTender::SpecieOnly,
    )
}

fn emerged_gold_fiat_debt_legal_tender() -> MarketScenario {
    emerged_gold_fiat_debt_with_tender(
        ScenarioName::EmergedGoldFiatDebtLegalTender,
        PublicDebtTender::FiatAndSpecie,
    )
}

fn emerged_gold_fractional_reserve() -> MarketScenario {
    MarketScenario {
        name: scenario_info(ScenarioName::EmergedGoldFractionalReserve).name,
        scenario: ScenarioName::EmergedGoldFractionalReserve,
        seed: 53,
        periods: 28,
        agents: emerged_gold_credit_ladder_agents(
            EmergedGoldCreditLadderProfile::FractionalReserve,
        ),
        recipes: Vec::new(),
        events: vec![
            Event {
                tick: Tick(0),
                kind: EventKind::SetRegime(Regime::FractionalConvertible),
            },
            Event {
                tick: Tick(0),
                kind: EventKind::SetReserveRatio {
                    bank: BankId(1),
                    ratio: ReserveRatioBps(0),
                },
            },
            Event {
                tick: Tick(0),
                kind: EventKind::SetBankCreditPolicy {
                    bank: BankId(1),
                    policy: standard_bank_credit_policy(Gold(1)),
                },
            },
        ],
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn emerged_gold_reserve_leash_control() -> MarketScenario {
    MarketScenario {
        name: scenario_info(ScenarioName::EmergedGoldReserveLeashControl).name,
        scenario: ScenarioName::EmergedGoldReserveLeashControl,
        seed: 55,
        // Single-phase M9 horizon: tick 3 binds the reserve leash and tick 4
        // shows the suspended path cutting it, without modeling a later bust.
        periods: 5,
        agents: emerged_gold_credit_ladder_agents(
            EmergedGoldCreditLadderProfile::FractionalReserve,
        ),
        recipes: Vec::new(),
        events: vec![
            Event {
                tick: Tick(0),
                kind: EventKind::SetRegime(Regime::FractionalConvertible),
            },
            Event {
                tick: Tick(0),
                kind: EventKind::SetReserveRatio {
                    bank: BankId(1),
                    ratio: ReserveRatioBps(5_000),
                },
            },
            Event {
                tick: Tick(0),
                kind: EventKind::SetBankCreditPolicy {
                    bank: BankId(1),
                    policy: standard_bank_credit_policy(Gold(1)),
                },
            },
        ],
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn emerged_gold_suspension_of_convertibility() -> MarketScenario {
    let mut scenario = emerged_gold_reserve_leash_control();
    scenario.name = scenario_info(ScenarioName::EmergedGoldSuspensionOfConvertibility).name;
    scenario.scenario = ScenarioName::EmergedGoldSuspensionOfConvertibility;
    scenario.events.extend([
        Event {
            tick: Tick(4),
            kind: EventKind::SetRegime(Regime::SuspendedConvertibility),
        },
        Event {
            tick: Tick(4),
            kind: EventKind::SetBankConvertibility {
                bank: BankId(1),
                convertible: false,
            },
        },
        Event {
            tick: Tick(4),
            kind: EventKind::SetBankCreditPolicy {
                bank: BankId(1),
                policy: standard_bank_credit_policy(Gold(3)),
            },
        },
    ]);
    scenario
}

fn emerged_gold_redemption_run() -> MarketScenario {
    let mut scenario = emerged_gold_reserve_leash_control();
    scenario.name = scenario_info(ScenarioName::EmergedGoldRedemptionRun).name;
    scenario.scenario = ScenarioName::EmergedGoldRedemptionRun;
    scenario.events.push(Event {
        tick: Tick(4),
        kind: EventKind::RedeemDemandClaims {
            bank: BankId(1),
            route: RedemptionRoute::AllClaimHolders,
            max_per_agent: None,
        },
    });
    scenario
}

fn emerged_gold_suspended_redemption() -> MarketScenario {
    let mut scenario = emerged_gold_reserve_leash_control();
    scenario.name = scenario_info(ScenarioName::EmergedGoldSuspendedRedemption).name;
    scenario.scenario = ScenarioName::EmergedGoldSuspendedRedemption;
    scenario.events.extend([
        Event {
            tick: Tick(4),
            kind: EventKind::SetRegime(Regime::SuspendedConvertibility),
        },
        Event {
            tick: Tick(4),
            kind: EventKind::SetBankConvertibility {
                bank: BankId(1),
                convertible: false,
            },
        },
        Event {
            tick: Tick(4),
            kind: EventKind::StopBankCredit { bank: BankId(1) },
        },
        Event {
            tick: Tick(4),
            kind: EventKind::RedeemDemandClaims {
                bank: BankId(1),
                route: RedemptionRoute::AllClaimHolders,
                max_per_agent: None,
            },
        },
    ]);
    scenario
}

fn emerged_gold_bank_claim_debt_with_tender(
    scenario_name: ScenarioName,
    debt_tender: PublicDebtTender,
) -> MarketScenario {
    let mut scenario = emerged_gold_suspended_redemption();
    scenario.name = scenario_info(scenario_name).name;
    scenario.scenario = scenario_name;
    scenario.events.extend([
        Event {
            tick: Tick(4),
            kind: EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly),
        },
        Event {
            tick: Tick(4),
            kind: EventKind::SetPublicDebtTender(debt_tender),
        },
        Event {
            tick: Tick(4),
            kind: EventKind::SeedCommodityDebt {
                lender: AgentId(1),
                borrower: AgentId(121),
                principal: Gold(1),
                due: Gold(1),
                due_tick: Tick(4),
                purpose: DebtPurpose::Consumption,
            },
        },
    ]);
    scenario
}

fn emerged_gold_bank_claim_debt_refusal_control() -> MarketScenario {
    emerged_gold_bank_claim_debt_with_tender(
        ScenarioName::EmergedGoldBankClaimDebtRefusalControl,
        PublicDebtTender::SpecieOnly,
    )
}

fn emerged_gold_bank_claim_debt_legal_tender() -> MarketScenario {
    emerged_gold_bank_claim_debt_with_tender(
        ScenarioName::EmergedGoldBankClaimDebtLegalTender,
        PublicDebtTender::BankClaimsAndSpecie,
    )
}

fn emerged_gold_bank_claim_spot_with_tender(
    scenario_name: ScenarioName,
    spot_tender: PublicSpotTender,
) -> MarketScenario {
    let mut scenario = emerged_gold_suspended_redemption();
    scenario.name = scenario_info(scenario_name).name;
    scenario.scenario = scenario_name;
    if let Some(agent) = scenario
        .agents
        .iter_mut()
        .find(|agent| agent.id == AgentId(121))
    {
        agent.scale = scale(&[
            (WantKind::Good(FOOD), Horizon::Next, 1),
            (WantKind::Good(GOLD), Horizon::Now, 1),
            (WantKind::Leisure, Horizon::Now, 1),
        ]);
        agent.expect[usize::from(FOOD.0)] = PriceBelief::new(Gold::ZERO, Gold(1));
    }
    scenario.agents.push(m2_agent(
        400,
        Gold::ZERO,
        stock(0, 0, 0),
        scale(&[(WantKind::Good(GOLD), Horizon::Later(4), 1)]),
        0,
        vec![Role::Trader],
        Gold(1),
    ));
    scenario.events.extend([
        Event {
            tick: Tick(0),
            kind: EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly),
        },
        Event {
            tick: Tick(4),
            kind: EventKind::ResetPublicSpotBook,
        },
        Event {
            tick: Tick(4),
            kind: EventKind::SetPublicSpotTender(spot_tender),
        },
        Event {
            tick: Tick(4),
            kind: EventKind::SeedStock {
                agent: AgentId(400),
                good: FOOD,
                qty: 1,
            },
        },
    ]);
    scenario
}

fn emerged_gold_bank_claim_spot_refusal_control() -> MarketScenario {
    emerged_gold_bank_claim_spot_with_tender(
        ScenarioName::EmergedGoldBankClaimSpotRefusalControl,
        PublicSpotTender::SpecieOnly,
    )
}

fn emerged_gold_bank_claim_spot_legal_tender() -> MarketScenario {
    emerged_gold_bank_claim_spot_with_tender(
        ScenarioName::EmergedGoldBankClaimSpotLegalTender,
        PublicSpotTender::BankClaimsAndSpecie,
    )
}

fn emerged_gold_bank_loan_repayment_with_tender(
    scenario_name: ScenarioName,
    bank_repayment_tender: BankRepaymentTender,
) -> MarketScenario {
    let mut scenario = emerged_gold_suspended_redemption();
    scenario.name = scenario_info(scenario_name).name;
    scenario.scenario = scenario_name;
    scenario.events.extend([
        Event {
            tick: Tick(4),
            kind: EventKind::SetBankRepaymentTender(bank_repayment_tender),
        },
        Event {
            tick: Tick(4),
            kind: EventKind::SetDebtDueTick {
                debt: DebtId(4),
                due_tick: Tick(4),
            },
        },
    ]);
    scenario
}

fn emerged_gold_bank_loan_repayment_refusal_control() -> MarketScenario {
    emerged_gold_bank_loan_repayment_with_tender(
        ScenarioName::EmergedGoldBankLoanRepaymentRefusalControl,
        BankRepaymentTender::SpecieOnly,
    )
}

fn emerged_gold_bank_loan_repayment_claim_tender() -> MarketScenario {
    emerged_gold_bank_loan_repayment_with_tender(
        ScenarioName::EmergedGoldBankLoanRepaymentClaimTender,
        BankRepaymentTender::BankClaimsAndSpecie,
    )
}

fn emerged_gold_fiat_credit_expansion() -> MarketScenario {
    MarketScenario {
        name: scenario_info(ScenarioName::EmergedGoldFiatCreditExpansion).name,
        scenario: ScenarioName::EmergedGoldFiatCreditExpansion,
        seed: 54,
        periods: 30,
        agents: emerged_gold_credit_ladder_agents(
            EmergedGoldCreditLadderProfile::FiatCreditExpansion,
        ),
        recipes: Vec::new(),
        events: vec![
            Event {
                tick: Tick(0),
                kind: EventKind::SetRegime(Regime::Fiat),
            },
            Event {
                tick: Tick(0),
                kind: EventKind::SetIssuerPolicy {
                    issuer: IssuerId(1),
                    policy: IssuerPolicy {
                        fiscal_enabled: false,
                        credit_enabled: true,
                        max_fiscal_issue_per_tick: Gold::ZERO,
                        max_credit_issue_per_tick: Gold(3),
                        loan_present: Gold(1),
                        loan_horizon: 13,
                        loan_future_due: Gold(1),
                    },
                },
            },
            Event {
                tick: Tick(4),
                kind: EventKind::StopIssuerCredit {
                    issuer: IssuerId(1),
                },
            },
        ],
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn emerged_gold_fiat_wage_with_tender(
    scenario_name: ScenarioName,
    tender: LaborWageTender,
) -> MarketScenario {
    let mut scenario = emerged_gold_fiat_credit_expansion();
    scenario.name = scenario_info(scenario_name).name;
    scenario.scenario = scenario_name;
    scenario.events.push(Event {
        tick: Tick(0),
        kind: EventKind::SetLaborWageTender(tender),
    });
    scenario
}

fn emerged_gold_fiat_wage_refusal_control() -> MarketScenario {
    emerged_gold_fiat_wage_with_tender(
        ScenarioName::EmergedGoldFiatWageRefusalControl,
        LaborWageTender::SpecieOnly,
    )
}

fn emerged_gold_fiat_wage_legal_tender() -> MarketScenario {
    emerged_gold_fiat_wage_with_tender(
        ScenarioName::EmergedGoldFiatWageLegalTender,
        LaborWageTender::FiatAndSpecie,
    )
}

fn emerged_gold_issuer_repayment_with_tender(
    scenario_name: ScenarioName,
    tender: IssuerRepaymentTender,
) -> MarketScenario {
    let mut scenario = emerged_gold_fiat_credit_expansion();
    scenario.name = scenario_info(scenario_name).name;
    scenario.scenario = scenario_name;
    // Even the default FiatOnly policy is emitted so the proof tick records the repayment law.
    scenario.events.extend([
        Event {
            tick: Tick(13),
            kind: EventKind::SetIssuerRepaymentTender(tender),
        },
        Event {
            tick: Tick(13),
            kind: EventKind::SetDebtDueTick {
                debt: DebtId(10),
                due_tick: Tick(13),
            },
        },
    ]);
    if tender != IssuerRepaymentTender::FiatOnly {
        // Scope the refusal control to the proof tick; later inherited issuer debts use the default.
        scenario.events.push(Event {
            tick: Tick(14),
            kind: EventKind::SetIssuerRepaymentTender(IssuerRepaymentTender::FiatOnly),
        });
    }
    scenario
}

fn emerged_gold_issuer_repayment_fiat_refusal_control() -> MarketScenario {
    emerged_gold_issuer_repayment_with_tender(
        ScenarioName::EmergedGoldIssuerRepaymentFiatRefusalControl,
        IssuerRepaymentTender::FiatRefused,
    )
}

fn emerged_gold_issuer_repayment_fiat_tender() -> MarketScenario {
    emerged_gold_issuer_repayment_with_tender(
        ScenarioName::EmergedGoldIssuerRepaymentFiatTender,
        IssuerRepaymentTender::FiatOnly,
    )
}

// --- M21 tax-receivability scenarios -------------------------------------
//
// Known Seam (see README / impl-23.md): the payable-accounting labor pull is
// AMOUNT-based. The headline worker works to cover the levy's amount and is
// paid in whatever the employer's fiat-first debit order yields — fiat, because
// the employer holds nothing else. No media-aware planning is involved; the
// demand-for-fiat is produced by cast construction plus the debit order.

const TAX_PAYER: AgentId = AgentId(50);
const TAX_EMPLOYER: AgentId = AgentId(60);
const TAX_WORKER: AgentId = AgentId(61);

/// A pure money holder: holds `gold` specie and rests on a single leisure want,
/// so it neither trades nor spends — a controlled tax subject.
fn tax_specie_holder(id: AgentId, gold: u64) -> Agent {
    bridge_agent(
        id.index(),
        gold,
        0,
        0,
        0,
        0,
        scale(&[(WantKind::Leisure, Horizon::Now, 1)]),
    )
}

fn emerged_gold_tax_specie_control() -> MarketScenario {
    let mut scenario = emerged_gold_sound_control();
    scenario.name = scenario_info(ScenarioName::EmergedGoldTaxSpecieControl).name;
    scenario.scenario = ScenarioName::EmergedGoldTaxSpecieControl;
    scenario.agents.push(tax_specie_holder(TAX_PAYER, 3));
    scenario.events = vec![
        Event {
            tick: Tick(0),
            kind: EventKind::SetTaxReceivability(TaxReceivability::SpecieOnly),
        },
        Event {
            tick: Tick(0),
            kind: EventKind::LevyTax {
                agent: TAX_PAYER,
                amount: Gold(2),
                due_tick: Tick(2),
            },
        },
    ];
    scenario
}

fn emerged_gold_tax_fiat_unpayable_defaults() -> MarketScenario {
    let mut scenario = emerged_gold_fiat_displacement();
    scenario.name = scenario_info(ScenarioName::EmergedGoldTaxFiatUnpayableDefaults).name;
    scenario.scenario = ScenarioName::EmergedGoldTaxFiatUnpayableDefaults;
    // A specie-rich, fiat-less subject with no fiat income path: spot tender is
    // specie-only, so the printed fiat held by others cannot reach this agent.
    scenario.agents.push(tax_specie_holder(TAX_PAYER, 5));
    scenario.events.extend([
        Event {
            tick: Tick(0),
            kind: EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly),
        },
        Event {
            tick: Tick(0),
            kind: EventKind::SetTaxReceivability(TaxReceivability::FiatOnly),
        },
        Event {
            tick: Tick(0),
            kind: EventKind::LevyTax {
                agent: TAX_PAYER,
                amount: Gold(2),
                due_tick: Tick(2),
            },
        },
    ]);
    scenario
}

/// A capitalist who holds only printed fiat and forms a one-labor project,
/// posting a standing hire each tick (the open hire path).
fn fiat_employer(id: AgentId) -> Agent {
    Agent {
        labor_capacity: 0,
        roles: vec![Role::Capitalist],
        ..v2_agent(
            id.index(),
            emerged_gold_bridge_stock(0, 0, 0, 0),
            // The future-money want exceeds the printed-fiat endowment so the
            // capitalist has unprovided later money and therefore lending
            // quotes — the ordinal valuation the project formation requires. It
            // never reserves the fiat, which stays free to fund wages.
            scale(&[(WantKind::Good(GOLD), Horizon::Later(6), 30)]),
        )
    }
}

/// A worker whose single future-money want is provisioned by one held specie
/// unit, so absent a payable it stays leisured; a levied tax pushes its future
/// money capacity below the want and it posts a labor ask for the levy.
fn leisure_default_worker(id: AgentId) -> Agent {
    Agent {
        gold: Gold(1),
        labor_capacity: 1,
        roles: vec![Role::Household],
        ..v2_agent(
            id.index(),
            emerged_gold_bridge_stock(0, 0, 0, 0),
            scale(&[
                (WantKind::Good(GOLD), Horizon::Later(6), 1),
                (WantKind::Leisure, Horizon::Now, 1),
            ]),
        )
    }
}

fn emerged_gold_tax_labor_scenario(name: ScenarioName, levy_tax: bool) -> MarketScenario {
    let mut agents = emerged_gold_bridge_agents();
    agents.push(fiat_employer(TAX_EMPLOYER));
    agents.push(leisure_default_worker(TAX_WORKER));
    let mut events = vec![
        Event {
            tick: Tick(0),
            kind: EventKind::SetRegime(Regime::Fiat),
        },
        Event {
            tick: Tick(0),
            kind: EventKind::SetIssuerPolicy {
                issuer: IssuerId(1),
                policy: fiscal_issuer_policy(Gold(8)),
            },
        },
        Event {
            tick: Tick(0),
            kind: EventKind::FiatPrint {
                issuer: IssuerId(1),
                amount: Gold(8),
                route: CantillonRoute::Agents(vec![TAX_EMPLOYER]),
            },
        },
        // Spot markets refuse fiat outright; only specie clears a purchase.
        Event {
            tick: Tick(0),
            kind: EventKind::SetPublicSpotTender(PublicSpotTender::SpecieOnly),
        },
        // The wage tender keeps its ParAll default, which admits fiat.
        Event {
            tick: Tick(0),
            kind: EventKind::SetTaxReceivability(TaxReceivability::FiatOnly),
        },
    ];
    if levy_tax {
        events.push(Event {
            tick: Tick(0),
            kind: EventKind::LevyTax {
                agent: TAX_WORKER,
                amount: Gold(2),
                due_tick: Tick(5),
            },
        });
    }
    MarketScenario {
        name: scenario_info(name).name,
        scenario: name,
        seed: 55,
        periods: 8,
        agents,
        recipes: Vec::new(),
        events,
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn emerged_gold_tax_drives_fiat_labor() -> MarketScenario {
    emerged_gold_tax_labor_scenario(ScenarioName::EmergedGoldTaxDrivesFiatLabor, true)
}

fn emerged_gold_no_tax_idle_control() -> MarketScenario {
    emerged_gold_tax_labor_scenario(ScenarioName::EmergedGoldNoTaxIdleControl, false)
}

fn time_market_basic() -> MarketScenario {
    MarketScenario {
        name: scenario_info(ScenarioName::TimeMarketBasic).name,
        scenario: ScenarioName::TimeMarketBasic,
        seed: 20,
        periods: 8,
        agents: vec![
            m2_agent(
                1,
                Gold(5),
                stock(0, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Now, 4),
                    (WantKind::Good(GOLD), Horizon::Later(4), 8),
                ]),
                0,
                vec![Role::Trader],
                Gold(1),
            ),
            m2_agent(
                2,
                Gold(4),
                stock(0, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Now, 6),
                    (WantKind::Good(GOLD), Horizon::Later(4), 3),
                ]),
                0,
                vec![Role::Trader],
                Gold(1),
            ),
        ],
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn roundabout_capital() -> MarketScenario {
    MarketScenario {
        name: scenario_info(ScenarioName::RoundaboutCapital).name,
        scenario: ScenarioName::RoundaboutCapital,
        seed: 21,
        periods: 16,
        agents: roundabout_agents(1),
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn borrow_to_build() -> MarketScenario {
    MarketScenario {
        name: scenario_info(ScenarioName::BorrowToBuild).name,
        scenario: ScenarioName::BorrowToBuild,
        seed: 23,
        periods: 18,
        agents: vec![
            m2_agent(
                1,
                Gold(10),
                stock(0, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Now, 1),
                    (WantKind::Good(GOLD), Horizon::Later(7), 12),
                ]),
                0,
                vec![Role::Trader],
                Gold(2),
            ),
            m2_agent(
                2,
                Gold(10),
                stock(0, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Now, 1),
                    (WantKind::Good(GOLD), Horizon::Later(7), 12),
                ]),
                0,
                vec![Role::Trader],
                Gold(2),
            ),
            m2_agent(
                3,
                Gold::ZERO,
                stock(0, 0, 0),
                scale(&[(WantKind::Good(GOLD), Horizon::Later(7), 6)]),
                0,
                vec![Role::Capitalist],
                Gold(2),
            ),
            m2_agent(
                4,
                Gold::ZERO,
                stock(0, 0, 0),
                scale(&[
                    (WantKind::Good(GOLD), Horizon::Now, 1),
                    (WantKind::Leisure, Horizon::Now, 1),
                ]),
                1,
                vec![Role::Household],
                Gold(2),
            ),
            borrow_to_build_consumer(5),
            borrow_to_build_consumer(6),
            borrow_to_build_consumer(7),
            borrow_to_build_consumer(8),
        ],
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn sound_money_100pct() -> MarketScenario {
    let mut agents = roundabout_agents(1);
    agents.extend(roundabout_agents(20));
    MarketScenario {
        name: scenario_info(ScenarioName::SoundMoney100Pct).name,
        scenario: ScenarioName::SoundMoney100Pct,
        seed: 22,
        periods: 32,
        agents,
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn commodity_credit_neutral() -> MarketScenario {
    let mut scenario = time_market_basic();
    scenario.name = scenario_info(ScenarioName::CommodityCreditNeutral).name;
    scenario.scenario = ScenarioName::CommodityCreditNeutral;
    scenario
}

fn fractional_reserve() -> MarketScenario {
    let mut agents = Vec::new();
    agents.extend(organic_credit_pair(1));
    agents.extend(project_cluster(10, 6, 7, 6, Gold(2)));
    MarketScenario {
        name: scenario_info(ScenarioName::FractionalReserve).name,
        scenario: ScenarioName::FractionalReserve,
        seed: 24,
        periods: 24,
        agents,
        recipes: Vec::new(),
        events: vec![
            Event {
                tick: Tick(0),
                kind: EventKind::SetReserveRatio {
                    bank: BankId(1),
                    ratio: ReserveRatioBps(0),
                },
            },
            Event {
                tick: Tick(0),
                kind: EventKind::SetBankCreditPolicy {
                    bank: BankId(1),
                    policy: BankPolicy {
                        max_new_fiduciary_per_tick: Gold(1),
                        loan_present: Gold(1),
                        loan_horizon: 7,
                        loan_future_due: Gold(1),
                        enabled: true,
                    },
                },
            },
        ],
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn suspension_of_convertibility() -> MarketScenario {
    let mut scenario = fractional_reserve();
    scenario.name = scenario_info(ScenarioName::SuspensionOfConvertibility).name;
    scenario.scenario = ScenarioName::SuspensionOfConvertibility;
    scenario.periods = 28;
    scenario
        .agents
        .extend(project_cluster(80, 8, 7, 6, Gold(2)));
    scenario.events = vec![
        Event {
            tick: Tick(0),
            kind: EventKind::SetReserveRatio {
                bank: BankId(1),
                ratio: ReserveRatioBps(0),
            },
        },
        Event {
            tick: Tick(0),
            kind: EventKind::SetBankCreditPolicy {
                bank: BankId(1),
                policy: BankPolicy {
                    max_new_fiduciary_per_tick: Gold(1),
                    loan_present: Gold(1),
                    loan_horizon: 7,
                    loan_future_due: Gold(1),
                    enabled: true,
                },
            },
        },
        Event {
            tick: Tick(4),
            kind: EventKind::SetRegime(Regime::SuspendedConvertibility),
        },
        Event {
            tick: Tick(4),
            kind: EventKind::SetBankConvertibility {
                bank: BankId(1),
                convertible: false,
            },
        },
        Event {
            tick: Tick(4),
            kind: EventKind::SetBankCreditPolicy {
                bank: BankId(1),
                policy: BankPolicy {
                    max_new_fiduciary_per_tick: Gold(3),
                    loan_present: Gold(1),
                    loan_horizon: 7,
                    loan_future_due: Gold(1),
                    enabled: true,
                },
            },
        },
    ];
    scenario
}

fn fiat_credit_expansion() -> MarketScenario {
    let mut agents = Vec::new();
    agents.extend(organic_credit_pair(1));
    agents.extend(organic_credit_pair_with_horizon(70, 1));
    agents.extend(project_cluster(10, 8, 13, 30, Gold(2)));
    for id in 40..56 {
        agents.push(project_worker(id));
    }
    MarketScenario {
        name: scenario_info(ScenarioName::FiatCreditExpansion).name,
        scenario: ScenarioName::FiatCreditExpansion,
        seed: 31,
        periods: 26,
        agents,
        recipes: Vec::new(),
        events: vec![
            Event {
                tick: Tick(0),
                kind: EventKind::SetIssuerPolicy {
                    issuer: IssuerId(1),
                    policy: IssuerPolicy {
                        fiscal_enabled: false,
                        credit_enabled: true,
                        max_fiscal_issue_per_tick: Gold::ZERO,
                        max_credit_issue_per_tick: Gold(3),
                        loan_present: Gold(1),
                        loan_horizon: 13,
                        loan_future_due: Gold(1),
                    },
                },
            },
            Event {
                tick: Tick(4),
                kind: EventKind::StopIssuerCredit {
                    issuer: IssuerId(1),
                },
            },
        ],
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn fiat_fiscal_cantillon() -> MarketScenario {
    let mut scenario = cantillon_spot_market(ScenarioName::FiatFiscalCantillon);
    scenario.periods = 10;
    scenario.events = vec![
        Event {
            tick: Tick(0),
            kind: EventKind::SetIssuerPolicy {
                issuer: IssuerId(1),
                policy: fiscal_issuer_policy(Gold(10)),
            },
        },
        Event {
            tick: Tick(0),
            kind: EventKind::FiatPrint {
                issuer: IssuerId(1),
                amount: Gold(10),
                route: CantillonRoute::Agents(vec![AgentId(1), AgentId(2)]),
            },
        },
        Event {
            tick: Tick(2),
            kind: EventKind::FiatPrint {
                issuer: IssuerId(1),
                amount: Gold(6),
                route: CantillonRoute::Helicopter,
            },
        },
    ];
    scenario
}

fn cantillon_isolation() -> MarketScenario {
    let mut scenario = cantillon_spot_market(ScenarioName::CantillonIsolation);
    scenario.periods = 8;
    scenario.events = vec![
        Event {
            tick: Tick(0),
            kind: EventKind::SetIssuerPolicy {
                issuer: IssuerId(1),
                policy: fiscal_issuer_policy(Gold(4)),
            },
        },
        Event {
            tick: Tick(0),
            kind: EventKind::FiatPrint {
                issuer: IssuerId(1),
                amount: Gold(4),
                route: CantillonRoute::Agents(vec![AgentId(1)]),
            },
        },
    ];
    scenario
}

fn cantillon_spot_market(name: ScenarioName) -> MarketScenario {
    MarketScenario {
        name: scenario_info(name).name,
        scenario: name,
        seed: 30,
        periods: 8,
        agents: vec![
            cantillon_buyer(1),
            cantillon_buyer(2),
            cantillon_late_buyer(3),
            cantillon_late_buyer(4),
            cantillon_seller(5, 5),
            cantillon_seller(6, 5),
        ],
        recipes: Vec::new(),
        events: Vec::new(),
        money: MarketMoneyConfig::Designated(DesignatedMoney { good: GOLD }),
    }
}

fn fiscal_issuer_policy(max_fiscal_issue_per_tick: Gold) -> IssuerPolicy {
    IssuerPolicy {
        fiscal_enabled: true,
        credit_enabled: false,
        max_fiscal_issue_per_tick,
        max_credit_issue_per_tick: Gold::ZERO,
        loan_present: Gold::ZERO,
        loan_horizon: 0,
        loan_future_due: Gold::ZERO,
    }
}

fn cantillon_buyer(id: u32) -> Agent {
    m2_agent(
        id,
        Gold::ZERO,
        stock(0, 0, 0),
        scale(&[
            (WantKind::Good(FOOD), Horizon::Next, 2),
            (WantKind::Good(GOLD), Horizon::Later(4), 2),
        ]),
        0,
        vec![Role::Consumer],
        Gold(2),
    )
}

fn cantillon_late_buyer(id: u32) -> Agent {
    m2_agent(
        id,
        Gold::ZERO,
        stock(0, 0, 0),
        scale(&[
            (WantKind::Good(FOOD), Horizon::Next, 2),
            (WantKind::Good(GOLD), Horizon::Later(4), 2),
        ]),
        0,
        vec![Role::Consumer],
        Gold(2),
    )
}

fn cantillon_seller(id: u32, food: u32) -> Agent {
    m2_agent(
        id,
        Gold::ZERO,
        stock(food, 0, 0),
        scale(&[(WantKind::Good(GOLD), Horizon::Later(4), 12)]),
        0,
        vec![Role::Trader],
        Gold(1),
    )
}

fn roundabout_agents(offset: u32) -> Vec<Agent> {
    vec![
        m2_agent(
            offset,
            Gold(30),
            stock(0, 0, 0),
            scale(&[
                (WantKind::Good(GOLD), Horizon::Now, 10),
                (WantKind::Good(GOLD), Horizon::Later(4), 36),
            ]),
            0,
            vec![Role::Trader],
            Gold(2),
        ),
        // Patient saver-capitalist: holds capital, so its lending-only
        // present_value exceeds the project cost and it SELF-FUNDS the roundabout
        // project. It does not borrow to fund (that honest story needs the
        // bundle-appraisal milestone — see concerns.md Concern 1). Its `Now` wants
        // are covered by its gold, so it posts no borrow orders.
        m2_agent(
            offset + 1,
            Gold(10),
            stock(0, 0, 0),
            scale(&[
                (WantKind::Good(GOLD), Horizon::Now, 2),
                (WantKind::Good(GOLD), Horizon::Later(4), 20),
            ]),
            0,
            vec![Role::Capitalist],
            Gold(2),
        ),
        m2_agent(
            offset + 2,
            Gold::ZERO,
            stock(0, 0, 0),
            scale(&[
                (WantKind::Good(GOLD), Horizon::Now, 3),
                (WantKind::Leisure, Horizon::Now, 1),
            ]),
            1,
            vec![Role::Household],
            Gold(2),
        ),
        m2_agent(
            offset + 3,
            Gold::ZERO,
            stock(0, 0, 0),
            scale(&[
                (WantKind::Good(GOLD), Horizon::Now, 3),
                (WantKind::Leisure, Horizon::Now, 1),
            ]),
            1,
            vec![Role::Household],
            Gold(2),
        ),
        m2_agent(
            offset + 4,
            Gold::ZERO,
            stock(0, 0, 0),
            scale(&[
                (WantKind::Good(GOLD), Horizon::Now, 3),
                (WantKind::Leisure, Horizon::Now, 1),
            ]),
            1,
            vec![Role::Household],
            Gold(2),
        ),
        roundabout_consumer(offset + 5),
        roundabout_consumer(offset + 6),
        // Separate, non-capitalist borrower: cash-poor and impatient (more
        // present-gold wants than it can cover), so it borrows present gold via
        // commodity credit and settles later from its own holdings (no default).
        // This is where roundabout-capital's loan trades come from — never the
        // capitalist funding its project.
        m2_agent(
            offset + 7,
            Gold(9),
            stock(0, 0, 0),
            scale(&[
                (WantKind::Good(GOLD), Horizon::Now, 10),
                (WantKind::Good(GOLD), Horizon::Later(4), 3),
            ]),
            0,
            vec![Role::Household],
            Gold(2),
        ),
        roundabout_consumer(offset + 8),
        roundabout_consumer(offset + 9),
        roundabout_consumer(offset + 10),
        roundabout_consumer(offset + 11),
        roundabout_consumer(offset + 12),
        roundabout_consumer(offset + 13),
        roundabout_consumer(offset + 14),
        m2_agent(
            offset + 15,
            Gold::ZERO,
            stock(0, 0, 0),
            scale(&[
                (WantKind::Good(GOLD), Horizon::Now, 3),
                (WantKind::Leisure, Horizon::Now, 1),
            ]),
            1,
            vec![Role::Household],
            Gold(2),
        ),
    ]
}

fn roundabout_consumer(id: u32) -> Agent {
    m2_agent(
        id,
        Gold(1),
        stock(0, 0, 0),
        scale(&[(WantKind::Good(FOOD), Horizon::Next, 1)]),
        0,
        vec![Role::Household],
        Gold(1),
    )
}

fn borrow_to_build_consumer(id: u32) -> Agent {
    m2_agent(
        id,
        Gold(2),
        stock(0, 0, 0),
        scale(&[(WantKind::Good(FOOD), Horizon::Next, 1)]),
        0,
        vec![Role::Household],
        Gold(2),
    )
}

fn organic_credit_pair(offset: u32) -> Vec<Agent> {
    organic_credit_pair_with_horizon(offset, 4)
}

fn organic_credit_pair_with_horizon(offset: u32, horizon: u8) -> Vec<Agent> {
    vec![
        m2_agent(
            offset,
            Gold(30),
            stock(0, 0, 0),
            scale(&[
                (WantKind::Good(GOLD), Horizon::Now, 10),
                (WantKind::Good(GOLD), Horizon::Later(horizon), 36),
            ]),
            0,
            vec![Role::Trader],
            Gold(2),
        ),
        m2_agent(
            offset + 1,
            Gold(9),
            stock(0, 0, 0),
            scale(&[
                (WantKind::Good(GOLD), Horizon::Now, 10),
                (WantKind::Good(GOLD), Horizon::Later(horizon), 3),
            ]),
            0,
            vec![Role::Household],
            Gold(2),
        ),
    ]
}

fn project_cluster(
    offset: u32,
    count: u32,
    funding_horizon: u8,
    future_gold_wants: usize,
    food_expected: Gold,
) -> Vec<Agent> {
    let mut agents = Vec::new();
    for pos in 0..count {
        let capitalist = offset + pos.saturating_mul(2);
        agents.push(m2_agent(
            capitalist,
            Gold::ZERO,
            stock(0, 0, 0),
            scale(&[(
                WantKind::Good(GOLD),
                Horizon::Later(funding_horizon),
                future_gold_wants,
            )]),
            0,
            vec![Role::Capitalist],
            food_expected,
        ));
        agents.push(project_worker(capitalist + 1));
    }
    agents
}

fn project_worker(id: u32) -> Agent {
    m2_agent(
        id,
        Gold::ZERO,
        stock(0, 0, 0),
        scale(&[
            (WantKind::Good(GOLD), Horizon::Now, 1),
            (WantKind::Leisure, Horizon::Now, 1),
        ]),
        1,
        vec![Role::Household],
        Gold(2),
    )
}

pub fn scale(entries: &[(WantKind, Horizon, usize)]) -> Vec<Want> {
    let mut scale = Vec::new();
    for (kind, horizon, n) in entries {
        repeat(&mut scale, *kind, *horizon, *n);
    }
    scale
}

fn market_agent(
    id: u32,
    gold: Gold,
    stock: Stock,
    scale: Vec<Want>,
    food_expected: Gold,
    wood_expected: Gold,
    step: Gold,
) -> Agent {
    let belief_slots = [GOLD, FOOD, WOOD, NET]
        .into_iter()
        .map(|good| good.0)
        .max()
        .map(|max| usize::from(max) + 1)
        .unwrap_or(0);
    let mut expect = vec![PriceBelief::new(Gold::ZERO, step); belief_slots];
    expect[usize::from(FOOD.0)] = PriceBelief::new(food_expected, step);
    expect[usize::from(WOOD.0)] = PriceBelief::new(wood_expected, step);

    Agent {
        id: AgentId(u64::from(id)),
        scale,
        stock,
        gold,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Trader],
        expect,
    }
}

pub fn v2_agent(id: u32, stock: Stock, scale: Vec<Want>) -> Agent {
    let belief_slots = [GOLD, FOOD, WOOD, NET, SALT, CLOTH, ORE]
        .into_iter()
        .map(|good| good.0)
        .max()
        .map(|max| usize::from(max) + 1)
        .unwrap_or(0);
    let expect = vec![PriceBelief::new(Gold(2), Gold(1)); belief_slots];

    Agent {
        id: AgentId(u64::from(id)),
        scale,
        stock,
        gold: Gold::ZERO,
        labor_capacity: 0,
        hunger_deficit: 0,
        roles: vec![Role::Trader],
        expect,
    }
}

fn m2_agent(
    id: u32,
    gold: Gold,
    stock: Stock,
    scale: Vec<Want>,
    labor_capacity: u32,
    roles: Vec<Role>,
    food_expected: Gold,
) -> Agent {
    let belief_slots = [GOLD, FOOD, WOOD, NET]
        .into_iter()
        .map(|good| good.0)
        .max()
        .map(|max| usize::from(max) + 1)
        .unwrap_or(0);
    let mut expect = vec![PriceBelief::new(Gold::ZERO, Gold(1)); belief_slots];
    expect[usize::from(FOOD.0)] = PriceBelief::new(food_expected, Gold(1));
    expect[usize::from(WOOD.0)] = PriceBelief::new(Gold(1), Gold(1));

    Agent {
        id: AgentId(u64::from(id)),
        scale,
        stock,
        gold,
        labor_capacity,
        hunger_deficit: 0,
        roles,
        expect,
    }
}
