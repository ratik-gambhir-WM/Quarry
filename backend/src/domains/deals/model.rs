//! Typed engagement, transaction, and participation records.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EngagementType {
    BuySide,
    SellSide,
    Other,
}
impl EngagementType {
    pub fn code(self) -> &'static str {
        match self {
            Self::BuySide => "buy_side",
            Self::SellSide => "sell_side",
            Self::Other => "other",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::BuySide => "Buy-side",
            Self::SellSide => "Sell-side",
            Self::Other => "Other",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    PlatformAcquisition,
    AddOnAcquisition,
    StrategicAcquisition,
    CarveOut,
    Merger,
    Divestiture,
    Recapitalization,
    GrowthEquity,
    Other,
    Unknown,
}
impl TransactionType {
    pub fn code(self) -> &'static str {
        match self {
            Self::PlatformAcquisition => "platform_acquisition",
            Self::AddOnAcquisition => "add_on_acquisition",
            Self::StrategicAcquisition => "strategic_acquisition",
            Self::CarveOut => "carve_out",
            Self::Merger => "merger",
            Self::Divestiture => "divestiture",
            Self::Recapitalization => "recapitalization",
            Self::GrowthEquity => "growth_equity",
            Self::Other => "other",
            Self::Unknown => "unknown",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::PlatformAcquisition => "Platform acquisition",
            Self::AddOnAcquisition => "Add-on acquisition",
            Self::StrategicAcquisition => "Strategic acquisition",
            Self::CarveOut => "Carve out",
            Self::Merger => "Merger",
            Self::Divestiture => "Divestiture",
            Self::Recapitalization => "Recapitalization",
            Self::GrowthEquity => "Growth equity",
            Self::Other => "Other",
            Self::Unknown => "Unknown",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DealStatus {
    Pipeline,
    Active,
    Completed,
    OnHold,
    Cancelled,
}
impl DealStatus {
    pub fn code(self) -> &'static str {
        match self {
            Self::Pipeline => "pipeline",
            Self::Active => "active",
            Self::Completed => "completed",
            Self::OnHold => "on_hold",
            Self::Cancelled => "cancelled",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PartyRole {
    Target,
    Buyer,
    Seller,
    PeSponsor,
    PortfolioCompany,
    ParentCompany,
    Client,
    Other,
}
impl PartyRole {
    pub fn code(self) -> &'static str {
        match self {
            Self::Target => "target",
            Self::Buyer => "buyer",
            Self::Seller => "seller",
            Self::PeSponsor => "pe_sponsor",
            Self::PortfolioCompany => "portfolio_company",
            Self::ParentCompany => "parent_company",
            Self::Client => "client",
            Self::Other => "other",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Target => "Target",
            Self::Buyer => "Buyer",
            Self::Seller => "Seller",
            Self::PeSponsor => "PE sponsor",
            Self::PortfolioCompany => "Portfolio company",
            Self::ParentCompany => "Parent company",
            Self::Client => "Client",
            Self::Other => "Other",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiligenceScope {
    ProductArchitecture,
    CorporateIt,
    Cybersecurity,
    ApplicationSecurity,
    Sdlc,
    ItOrganization,
    ItFinancials,
}
impl DiligenceScope {
    pub fn code(self) -> &'static str {
        match self {
            Self::ProductArchitecture => "product_architecture",
            Self::CorporateIt => "corporate_it",
            Self::Cybersecurity => "cybersecurity",
            Self::ApplicationSecurity => "application_security",
            Self::Sdlc => "sdlc",
            Self::ItOrganization => "it_organization",
            Self::ItFinancials => "it_financials",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::ProductArchitecture => "Product architecture",
            Self::CorporateIt => "Corporate IT",
            Self::Cybersecurity => "Cybersecurity",
            Self::ApplicationSecurity => "Application security",
            Self::Sdlc => "SDLC",
            Self::ItOrganization => "IT organization",
            Self::ItFinancials => "IT financials",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartyInput {
    pub company_id: String,
    pub role: PartyRole,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DealFields {
    pub deal_name: String,
    #[serde(deserialize_with = "required_nullable")]
    pub deal_code: Option<String>,
    pub engagement_type: EngagementType,
    pub transaction_type: TransactionType,
    pub status: DealStatus,
    #[serde(deserialize_with = "required_nullable")]
    pub start_date: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub expected_close_date: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub actual_close_date: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub engagement_completed_date: Option<String>,
    pub parties: Vec<PartyInput>,
    pub scopes: Vec<DiligenceScope>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealParty {
    pub deal_party_id: String,
    pub company_id: String,
    pub role: PartyRole,
    pub notes: Option<String>,
    pub company: crate::domains::companies::Company,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deal {
    pub deal_id: String,
    pub user_id: i64,
    pub deal_name: String,
    pub deal_code: Option<String>,
    pub engagement_type: EngagementType,
    pub transaction_type: TransactionType,
    pub status: DealStatus,
    pub start_date: Option<String>,
    pub expected_close_date: Option<String>,
    pub actual_close_date: Option<String>,
    pub engagement_completed_date: Option<String>,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub parties: Vec<DealParty>,
    pub scopes: Vec<DiligenceScope>,
}

fn required_nullable<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}
