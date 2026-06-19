//! Designated commodity money and monetary-regime markers.

use crate::good::GoodId;
use crate::menger::MengerianEmergence;

pub trait MoneyRegime {
    fn current_money_good(&self) -> Option<GoodId>;

    fn is_money_good(&self, good: GoodId) -> bool {
        self.current_money_good() == Some(good)
    }

    fn saleability_bps(&self, _good: GoodId) -> Option<u16> {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DesignatedMoney {
    pub good: GoodId,
}

impl DesignatedMoney {
    pub fn money_good(&self) -> GoodId {
        self.good
    }
}

impl MoneyRegime for DesignatedMoney {
    fn current_money_good(&self) -> Option<GoodId> {
        Some(self.good)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarketMoneyConfig {
    Designated(DesignatedMoney),
    /// Phase A configuration for commodity-money emergence.
    ///
    /// This is intentionally inert in the existing runner until the Phase B V2
    /// barter/promotion loop is wired.
    Emergent(MengerianConfig),
}

impl MoneyRegime for MarketMoneyConfig {
    fn current_money_good(&self) -> Option<GoodId> {
        match self {
            Self::Designated(money) => money.current_money_good(),
            Self::Emergent(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarketMoneyState {
    Designated(DesignatedMoney),
    /// Runtime emergence state for the Phase B V2 runner.
    ///
    /// Phase A constructs this state for sizing and pure tracker tests only.
    Emergent(MengerianEmergence),
}

impl MarketMoneyState {
    pub fn from_config(config: MarketMoneyConfig) -> Self {
        match config {
            MarketMoneyConfig::Designated(money) => Self::Designated(money),
            MarketMoneyConfig::Emergent(config) => Self::Emergent(MengerianEmergence::new(config)),
        }
    }
}

impl MoneyRegime for MarketMoneyState {
    fn current_money_good(&self) -> Option<GoodId> {
        match self {
            Self::Designated(money) => money.current_money_good(),
            Self::Emergent(emergence) => emergence.current_money_good(),
        }
    }

    fn saleability_bps(&self, good: GoodId) -> Option<u16> {
        match self {
            Self::Designated(money) => money.saleability_bps(good),
            Self::Emergent(emergence) => emergence.saleability_bps(good),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MengerianConfig {
    pub candidate_goods: Vec<GoodId>,
    pub min_total_acceptances: u32,
    pub promotion_threshold_bps: u16,
    pub lead_margin_bps: u16,
    pub min_acceptor_agents: u16,
    pub min_counterpart_goods: u16,
    pub stability_ticks: u32,
    pub indirect_min_acceptance_share_bps: u16,
    /// S9 strong-bar gate: the minimum INDIRECT acceptances (a good taken
    /// instrumentally, `IndirectFor`, to re-trade) a candidate must accrue before it
    /// can promote. `0` (default) is inert — a pre-S9 config promotes on total
    /// acceptances/share/breadth alone, exactly as before. The strong scenario sets
    /// it so a good monetizes only after REAL indirect exchange, not direct-want
    /// churn.
    pub min_indirect_acceptances: u32,
    /// S9 strong-bar gate: the minimum DISTINCT agents that must have accepted the
    /// candidate indirectly — breadth of who re-trades it, not just how often (a raw
    /// count is gameable by a few agents churning one pair). `0` (default) inert.
    pub min_indirect_acceptor_agents: u16,
    /// S9 strong-bar gate: the minimum DISTINCT final target goods the indirect
    /// acceptors were pursuing — proof the good is taken as a GENERAL medium toward
    /// many ends, not one repeated purpose. `0` (default) inert.
    pub min_indirect_target_goods: u16,
    /// S9 control knob: whether agents may post INDIRECT barter offers for the
    /// provisional leader at all. `true` (default) keeps the existing
    /// indirect-acceptance machinery on. A gated `false` is the clean
    /// no-indirect-acceptance control — the leader still leads and still trades
    /// directly, but no indirect volume can accrue, so under a positive indirect gate
    /// it cannot monetize. Does NOT lower the leader floor (that would disable
    /// leadership itself).
    pub allow_indirect_acceptance: bool,
}

impl Default for MengerianConfig {
    fn default() -> Self {
        Self {
            candidate_goods: Vec::new(),
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
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Regime {
    #[default]
    SoundGold,
    FractionalConvertible,
    SuspendedConvertibility,
    Fiat,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PublicSpotTender {
    #[default]
    ParAll,
    SpecieOnly,
    FiatAndSpecie,
    BankClaimsAndSpecie,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LaborWageTender {
    #[default]
    ParAll,
    SpecieOnly,
    FiatAndSpecie,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcceptedMedia {
    pub fiat: bool,
    pub bank_claims: bool,
    pub specie: bool,
}

/// Media the state names as acceptable to discharge a levied tax liability
/// (M21). `SpecieOnly` is the default: a commodity-money state taxes in the
/// money of the realm, so every existing scenario — none of which levies a
/// tax — keeps the default inert. Bank claims are never accepted for taxes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaxReceivability {
    #[default]
    SpecieOnly,
    FiatOnly,
    FiatAndSpecie,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PublicDebtTender {
    #[default]
    ParAll,
    SpecieOnly,
    FiatAndSpecie,
    BankClaimsAndSpecie,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BankRepaymentTender {
    #[default]
    ParAll,
    SpecieOnly,
    FiatAndSpecie,
    BankClaimsAndSpecie,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IssuerRepaymentTender {
    #[default]
    FiatOnly,
    FiatRefused,
}

impl PublicSpotTender {
    pub fn accepted_media(self) -> AcceptedMedia {
        match self {
            Self::ParAll => AcceptedMedia {
                fiat: true,
                bank_claims: true,
                specie: true,
            },
            Self::SpecieOnly => AcceptedMedia {
                fiat: false,
                bank_claims: false,
                specie: true,
            },
            Self::FiatAndSpecie => AcceptedMedia {
                fiat: true,
                bank_claims: false,
                specie: true,
            },
            Self::BankClaimsAndSpecie => AcceptedMedia {
                fiat: false,
                bank_claims: true,
                specie: true,
            },
        }
    }
}

impl LaborWageTender {
    pub fn accepted_media(self) -> AcceptedMedia {
        match self {
            Self::ParAll => AcceptedMedia {
                fiat: true,
                bank_claims: true,
                specie: true,
            },
            Self::SpecieOnly => AcceptedMedia {
                fiat: false,
                bank_claims: false,
                specie: true,
            },
            Self::FiatAndSpecie => AcceptedMedia {
                fiat: true,
                bank_claims: false,
                specie: true,
            },
        }
    }
}

impl PublicDebtTender {
    pub fn accepted_media(self) -> AcceptedMedia {
        match self {
            Self::ParAll => AcceptedMedia {
                fiat: true,
                bank_claims: true,
                specie: true,
            },
            Self::SpecieOnly => AcceptedMedia {
                fiat: false,
                bank_claims: false,
                specie: true,
            },
            Self::FiatAndSpecie => AcceptedMedia {
                fiat: true,
                bank_claims: false,
                specie: true,
            },
            Self::BankClaimsAndSpecie => AcceptedMedia {
                fiat: false,
                bank_claims: true,
                specie: true,
            },
        }
    }
}

impl BankRepaymentTender {
    pub fn accepted_media(self) -> AcceptedMedia {
        match self {
            Self::ParAll => AcceptedMedia {
                fiat: true,
                bank_claims: true,
                specie: true,
            },
            Self::SpecieOnly => AcceptedMedia {
                fiat: false,
                bank_claims: false,
                specie: true,
            },
            Self::FiatAndSpecie => AcceptedMedia {
                fiat: true,
                bank_claims: false,
                specie: true,
            },
            Self::BankClaimsAndSpecie => AcceptedMedia {
                fiat: false,
                bank_claims: true,
                specie: true,
            },
        }
    }
}

impl IssuerRepaymentTender {
    pub fn accepts_fiat(self) -> bool {
        matches!(self, Self::FiatOnly)
    }
}

impl TaxReceivability {
    pub fn accepted_media(self) -> AcceptedMedia {
        match self {
            Self::SpecieOnly => AcceptedMedia {
                fiat: false,
                bank_claims: false,
                specie: true,
            },
            Self::FiatOnly => AcceptedMedia {
                fiat: true,
                bank_claims: false,
                specie: false,
            },
            Self::FiatAndSpecie => AcceptedMedia {
                fiat: true,
                bank_claims: false,
                specie: true,
            },
        }
    }
}

pub fn public_spot_tender_name(tender: PublicSpotTender) -> &'static str {
    match tender {
        PublicSpotTender::ParAll => "par-all",
        PublicSpotTender::SpecieOnly => "specie-only",
        PublicSpotTender::FiatAndSpecie => "fiat-and-specie",
        PublicSpotTender::BankClaimsAndSpecie => "bank-claims-and-specie",
    }
}

pub fn labor_wage_tender_name(tender: LaborWageTender) -> &'static str {
    match tender {
        LaborWageTender::ParAll => "par-all",
        LaborWageTender::SpecieOnly => "specie-only",
        LaborWageTender::FiatAndSpecie => "fiat-and-specie",
    }
}

pub fn public_debt_tender_name(tender: PublicDebtTender) -> &'static str {
    match tender {
        PublicDebtTender::ParAll => "par-all",
        PublicDebtTender::SpecieOnly => "specie-only",
        PublicDebtTender::FiatAndSpecie => "fiat-and-specie",
        PublicDebtTender::BankClaimsAndSpecie => "bank-claims-and-specie",
    }
}

pub fn bank_repayment_tender_name(tender: BankRepaymentTender) -> &'static str {
    match tender {
        BankRepaymentTender::ParAll => "par-all",
        BankRepaymentTender::SpecieOnly => "specie-only",
        BankRepaymentTender::FiatAndSpecie => "fiat-and-specie",
        BankRepaymentTender::BankClaimsAndSpecie => "bank-claims-and-specie",
    }
}

pub fn issuer_repayment_tender_name(tender: IssuerRepaymentTender) -> &'static str {
    match tender {
        IssuerRepaymentTender::FiatOnly => "fiat-only",
        IssuerRepaymentTender::FiatRefused => "fiat-refused",
    }
}

pub fn tax_receivability_name(receivability: TaxReceivability) -> &'static str {
    match receivability {
        TaxReceivability::SpecieOnly => "specie-only",
        TaxReceivability::FiatOnly => "fiat-only",
        TaxReceivability::FiatAndSpecie => "fiat-and-specie",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        bank_repayment_tender_name, issuer_repayment_tender_name, labor_wage_tender_name,
        public_debt_tender_name, public_spot_tender_name, tax_receivability_name,
        BankRepaymentTender, IssuerRepaymentTender, LaborWageTender, PublicDebtTender,
        PublicSpotTender, TaxReceivability,
    };

    #[test]
    fn debt_and_spot_tender_media_names_match_by_policy() {
        for (spot, debt) in [
            (PublicSpotTender::ParAll, PublicDebtTender::ParAll),
            (PublicSpotTender::SpecieOnly, PublicDebtTender::SpecieOnly),
            (
                PublicSpotTender::FiatAndSpecie,
                PublicDebtTender::FiatAndSpecie,
            ),
            (
                PublicSpotTender::BankClaimsAndSpecie,
                PublicDebtTender::BankClaimsAndSpecie,
            ),
        ] {
            assert_eq!(spot.accepted_media(), debt.accepted_media());
            assert_eq!(public_spot_tender_name(spot), public_debt_tender_name(debt));
        }
    }

    #[test]
    fn spot_bank_claims_and_specie_accepts_claims_and_specie_not_fiat() {
        let media = PublicSpotTender::BankClaimsAndSpecie.accepted_media();

        assert!(!media.fiat);
        assert!(media.bank_claims);
        assert!(media.specie);
    }

    #[test]
    fn labor_wage_tender_default_names_and_media_match_shipped_spot_variants() {
        assert_eq!(LaborWageTender::default(), LaborWageTender::ParAll);

        for (labor, spot, name) in [
            (LaborWageTender::ParAll, PublicSpotTender::ParAll, "par-all"),
            (
                LaborWageTender::SpecieOnly,
                PublicSpotTender::SpecieOnly,
                "specie-only",
            ),
            (
                LaborWageTender::FiatAndSpecie,
                PublicSpotTender::FiatAndSpecie,
                "fiat-and-specie",
            ),
        ] {
            assert_eq!(labor.accepted_media(), spot.accepted_media());
            assert_eq!(labor_wage_tender_name(labor), name);
        }
    }

    #[test]
    fn bank_repayment_par_all_matches_legacy_media() {
        let media = BankRepaymentTender::ParAll.accepted_media();

        assert!(media.fiat);
        assert!(media.bank_claims);
        assert!(media.specie);
        assert_eq!(media, PublicDebtTender::ParAll.accepted_media());
        assert_eq!(
            bank_repayment_tender_name(BankRepaymentTender::ParAll),
            "par-all"
        );
    }

    #[test]
    fn bank_repayment_specie_only_accepts_only_specie() {
        let media = BankRepaymentTender::SpecieOnly.accepted_media();

        assert!(!media.fiat);
        assert!(!media.bank_claims);
        assert!(media.specie);
        assert_eq!(
            bank_repayment_tender_name(BankRepaymentTender::SpecieOnly),
            "specie-only"
        );
    }

    #[test]
    fn bank_repayment_bank_claims_and_specie_rejects_fiat() {
        let media = BankRepaymentTender::BankClaimsAndSpecie.accepted_media();

        assert!(!media.fiat);
        assert!(media.bank_claims);
        assert!(media.specie);
        assert_eq!(
            bank_repayment_tender_name(BankRepaymentTender::BankClaimsAndSpecie),
            "bank-claims-and-specie"
        );
    }

    #[test]
    fn issuer_repayment_fiat_only_is_default_and_formats() {
        assert_eq!(
            IssuerRepaymentTender::default(),
            IssuerRepaymentTender::FiatOnly
        );
        assert!(IssuerRepaymentTender::FiatOnly.accepts_fiat());
        assert_eq!(
            issuer_repayment_tender_name(IssuerRepaymentTender::FiatOnly),
            "fiat-only"
        );
    }

    #[test]
    fn issuer_repayment_fiat_refused_formats() {
        assert!(!IssuerRepaymentTender::FiatRefused.accepts_fiat());
        assert_eq!(
            issuer_repayment_tender_name(IssuerRepaymentTender::FiatRefused),
            "fiat-refused"
        );
    }

    #[test]
    fn tax_receivability_default_is_specie_only() {
        assert_eq!(TaxReceivability::default(), TaxReceivability::SpecieOnly);
    }

    #[test]
    fn tax_receivability_names_match_policy() {
        for (receivability, name) in [
            (TaxReceivability::SpecieOnly, "specie-only"),
            (TaxReceivability::FiatOnly, "fiat-only"),
            (TaxReceivability::FiatAndSpecie, "fiat-and-specie"),
        ] {
            assert_eq!(tax_receivability_name(receivability), name);
        }
    }

    #[test]
    fn tax_receivability_media_table_never_accepts_bank_claims() {
        let specie = TaxReceivability::SpecieOnly.accepted_media();
        assert!(!specie.fiat);
        assert!(!specie.bank_claims);
        assert!(specie.specie);

        let fiat = TaxReceivability::FiatOnly.accepted_media();
        assert!(fiat.fiat);
        assert!(!fiat.bank_claims);
        assert!(!fiat.specie);

        let both = TaxReceivability::FiatAndSpecie.accepted_media();
        assert!(both.fiat);
        assert!(!both.bank_claims);
        assert!(both.specie);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReserveRatioBps(pub u16);

impl ReserveRatioBps {
    pub const FULL: Self = Self(10_000);
}
