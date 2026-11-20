//! Versioned diligence taxonomy: the single source for wire IDs, labels, decision
//! boundaries, Decisions choice descriptions and result validation.
use serde::{Deserialize, Serialize};

/// Bump when an ID, label, mapping or decision boundary changes.
pub const TAXONOMY_VERSION: i64 = 1;
/// Choice sentinel for "no supported primary category"; never persisted as a category.
pub const UNCLASSIFIED: &str = "unclassified";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Workstream {
    ProductEngineering,
    CorporateTechnologySecurity,
    DataAi,
    BusinessCommercial,
    TransactionDiligenceManagement,
}

impl Workstream {
    pub const ALL: [Self; 5] = [
        Self::ProductEngineering,
        Self::CorporateTechnologySecurity,
        Self::DataAi,
        Self::BusinessCommercial,
        Self::TransactionDiligenceManagement,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::ProductEngineering => "product_engineering",
            Self::CorporateTechnologySecurity => "corporate_technology_security",
            Self::DataAi => "data_ai",
            Self::BusinessCommercial => "business_commercial",
            Self::TransactionDiligenceManagement => "transaction_diligence_management",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::ProductEngineering => "Product & Engineering",
            Self::CorporateTechnologySecurity => "Corporate Technology & Security",
            Self::DataAi => "Data & AI",
            Self::BusinessCommercial => "Business & Commercial",
            Self::TransactionDiligenceManagement => "Transaction & Diligence Management",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    ProductArchitecture,
    ProductInfrastructureScalability,
    ProductSecurity,
    Sdlc,
    ProductPortfolioRoadmap,
    TechnicalDebtModernization,
    CorporateItInfrastructure,
    Cybersecurity,
    ItOperations,
    BusinessContinuityDisasterRecovery,
    CompliancePrivacy,
    TechnologyVendorsLicensing,
    DataArchitectureInfrastructure,
    DataEngineeringIntegration,
    DataGovernanceQuality,
    BusinessIntelligenceAnalytics,
    AiMachineLearning,
    AiStrategyAdoptionGovernance,
    CompanyOverviewStrategy,
    CustomersCommercialOperations,
    FinancialsTechnologySpend,
    MaHistoryIntegration,
    OrganizationTalent,
    LegalIntellectualProperty,
    DealAdministrationScope,
    ManagementInterviewsEvidence,
    DiligenceFindingsAssessments,
    RemediationInvestmentPlanning,
}

impl Category {
    pub const ALL: [Self; 28] = [
        Self::ProductArchitecture,
        Self::ProductInfrastructureScalability,
        Self::ProductSecurity,
        Self::Sdlc,
        Self::ProductPortfolioRoadmap,
        Self::TechnicalDebtModernization,
        Self::CorporateItInfrastructure,
        Self::Cybersecurity,
        Self::ItOperations,
        Self::BusinessContinuityDisasterRecovery,
        Self::CompliancePrivacy,
        Self::TechnologyVendorsLicensing,
        Self::DataArchitectureInfrastructure,
        Self::DataEngineeringIntegration,
        Self::DataGovernanceQuality,
        Self::BusinessIntelligenceAnalytics,
        Self::AiMachineLearning,
        Self::AiStrategyAdoptionGovernance,
        Self::CompanyOverviewStrategy,
        Self::CustomersCommercialOperations,
        Self::FinancialsTechnologySpend,
        Self::MaHistoryIntegration,
        Self::OrganizationTalent,
        Self::LegalIntellectualProperty,
        Self::DealAdministrationScope,
        Self::ManagementInterviewsEvidence,
        Self::DiligenceFindingsAssessments,
        Self::RemediationInvestmentPlanning,
    ];

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|category| category.id() == id)
    }

    /// Total hierarchy: every category belongs to exactly one workstream.
    pub const fn workstream(self) -> Workstream {
        use Category::*;
        match self {
            ProductArchitecture
            | ProductInfrastructureScalability
            | ProductSecurity
            | Sdlc
            | ProductPortfolioRoadmap
            | TechnicalDebtModernization => Workstream::ProductEngineering,
            CorporateItInfrastructure
            | Cybersecurity
            | ItOperations
            | BusinessContinuityDisasterRecovery
            | CompliancePrivacy
            | TechnologyVendorsLicensing => Workstream::CorporateTechnologySecurity,
            DataArchitectureInfrastructure
            | DataEngineeringIntegration
            | DataGovernanceQuality
            | BusinessIntelligenceAnalytics
            | AiMachineLearning
            | AiStrategyAdoptionGovernance => Workstream::DataAi,
            CompanyOverviewStrategy
            | CustomersCommercialOperations
            | FinancialsTechnologySpend
            | MaHistoryIntegration
            | OrganizationTalent
            | LegalIntellectualProperty => Workstream::BusinessCommercial,
            DealAdministrationScope
            | ManagementInterviewsEvidence
            | DiligenceFindingsAssessments
            | RemediationInvestmentPlanning => Workstream::TransactionDiligenceManagement,
        }
    }

    pub const fn id(self) -> &'static str {
        use Category::*;
        match self {
            ProductArchitecture => "product_architecture",
            ProductInfrastructureScalability => "product_infrastructure_scalability",
            ProductSecurity => "product_security",
            Sdlc => "sdlc",
            ProductPortfolioRoadmap => "product_portfolio_roadmap",
            TechnicalDebtModernization => "technical_debt_modernization",
            CorporateItInfrastructure => "corporate_it_infrastructure",
            Cybersecurity => "cybersecurity",
            ItOperations => "it_operations",
            BusinessContinuityDisasterRecovery => "business_continuity_disaster_recovery",
            CompliancePrivacy => "compliance_privacy",
            TechnologyVendorsLicensing => "technology_vendors_licensing",
            DataArchitectureInfrastructure => "data_architecture_infrastructure",
            DataEngineeringIntegration => "data_engineering_integration",
            DataGovernanceQuality => "data_governance_quality",
            BusinessIntelligenceAnalytics => "business_intelligence_analytics",
            AiMachineLearning => "ai_machine_learning",
            AiStrategyAdoptionGovernance => "ai_strategy_adoption_governance",
            CompanyOverviewStrategy => "company_overview_strategy",
            CustomersCommercialOperations => "customers_commercial_operations",
            FinancialsTechnologySpend => "financials_technology_spend",
            MaHistoryIntegration => "ma_history_integration",
            OrganizationTalent => "organization_talent",
            LegalIntellectualProperty => "legal_intellectual_property",
            DealAdministrationScope => "deal_administration_scope",
            ManagementInterviewsEvidence => "management_interviews_evidence",
            DiligenceFindingsAssessments => "diligence_findings_assessments",
            RemediationInvestmentPlanning => "remediation_investment_planning",
        }
    }

    pub const fn label(self) -> &'static str {
        use Category::*;
        match self {
            ProductArchitecture => "Product Architecture",
            ProductInfrastructureScalability => "Product Infrastructure & Scalability",
            ProductSecurity => "Product Security",
            Sdlc => "Software Development Lifecycle (SDLC)",
            ProductPortfolioRoadmap => "Product Portfolio & Roadmap",
            TechnicalDebtModernization => "Technical Debt & Modernization",
            CorporateItInfrastructure => "Corporate IT Infrastructure",
            Cybersecurity => "Cybersecurity",
            ItOperations => "IT Operations",
            BusinessContinuityDisasterRecovery => "Business Continuity & Disaster Recovery",
            CompliancePrivacy => "Compliance & Privacy",
            TechnologyVendorsLicensing => "Technology Vendors & Licensing",
            DataArchitectureInfrastructure => "Data Architecture & Infrastructure",
            DataEngineeringIntegration => "Data Engineering & Integration",
            DataGovernanceQuality => "Data Governance & Quality",
            BusinessIntelligenceAnalytics => "Business Intelligence & Analytics",
            AiMachineLearning => "AI & Machine Learning",
            AiStrategyAdoptionGovernance => "AI Strategy, Adoption & Governance",
            CompanyOverviewStrategy => "Company Overview & Strategy",
            CustomersCommercialOperations => "Customers & Commercial Operations",
            FinancialsTechnologySpend => "Financials & Technology Spend",
            MaHistoryIntegration => "M&A History & Integration",
            OrganizationTalent => "Organization & Talent",
            LegalIntellectualProperty => "Legal & Intellectual Property",
            DealAdministrationScope => "Deal Administration & Scope",
            ManagementInterviewsEvidence => "Management Interviews & Evidence",
            DiligenceFindingsAssessments => "Diligence Findings & Assessments",
            RemediationInvestmentPlanning => "Remediation Projects & Investment Planning",
        }
    }

    /// Concise decision boundary used in choice descriptions and coverage predicates.
    pub const fn boundary(self) -> &'static str {
        use Category::*;
        match self {
            ProductArchitecture => "application components, runtime design, dependencies and interfaces of the sold product; not analytical or data-platform topology",
            ProductInfrastructureScalability => "hosting, capacity, performance and reliability of the sold product",
            ProductSecurity => "security of product code, application design, customer-facing services and secure development, such as application threat models; not the enterprise security program",
            Sdlc => "development process, CI/CD, testing, release and change practices",
            ProductPortfolioRoadmap => "product offerings, capabilities, roadmap and prioritization",
            TechnicalDebtModernization => "obsolete product technologies, maintainability and routine modernization plans; not transaction-driven remediation investment plans",
            CorporateItInfrastructure => "employee endpoints, internal networks, identity and enterprise systems",
            Cybersecurity => "enterprise security program, SOC, threat and vulnerability management and corporate controls; not product application security",
            ItOperations => "service desk, ITSM, internal technology support and operations",
            BusinessContinuityDisasterRecovery => "recovery objectives, continuity plans and recovery testing",
            CompliancePrivacy => "regulatory and control evidence, privacy, audits and data protection obligations",
            TechnologyVendorsLicensing => "supplier arrangements, license inventories and obligations",
            DataArchitectureInfrastructure => "warehouses, lakes, data platforms, data models and storage topology; an incidental database mention or application service diagram is not enough",
            DataEngineeringIntegration => "pipelines, ETL/ELT, ingestion, transformations and data interfaces",
            DataGovernanceQuality => "stewardship, lineage, catalogs, quality controls and data policies",
            BusinessIntelligenceAnalytics => "reporting, dashboards, analytical practices and business metrics",
            AiMachineLearning => "models, training, inference, evaluation and MLOps implementation",
            AiStrategyAdoptionGovernance => "AI portfolio and adoption, policy, risk oversight and governance",
            CompanyOverviewStrategy => "company operating model, positioning and strategy",
            CustomersCommercialOperations => "customer base, sales, commercial contract terms and delivery",
            FinancialsTechnologySpend => "budgets, financial performance, technology costs and allocations",
            MaHistoryIntegration => "acquisitions, integration history, integration operating models and synergies",
            OrganizationTalent => "org structures, headcount, skills, leadership and staffing",
            LegalIntellectualProperty => "ownership, patents, legal rights and IP obligations",
            DealAdministrationScope => "transaction logistics, diligence scope, request lists and schedules",
            ManagementInterviewsEvidence => "interview transcripts or notes and evidence collection records; substantive technical subjects may be secondary",
            DiligenceFindingsAssessments => "synthesized diligence judgments, findings, risks and assessments, even when they discuss security or architecture",
            RemediationInvestmentPlanning => "diligence-driven remediation programs, estimates and investment plans",
        }
    }

    /// Choice description: workstream context plus the decision boundary.
    pub fn choice_description(self) -> String {
        format!(
            "{} - {}: {}.",
            self.workstream().label(),
            self.label(),
            self.boundary()
        )
    }

    /// Stable predicate question name.
    pub fn coverage_question(self) -> String {
        format!("covers_{}", self.id())
    }

    pub fn coverage_instructions(self) -> String {
        format!(
            "Does the evidence substantively and independently discuss {} ({})? \
             Require real treatment of the topic; a heading alone, generic boilerplate, \
             a passing mention or an uncertain alternative is not enough.",
            self.label(),
            self.boundary()
        )
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/classification/taxonomy_tests.rs"]
mod tests;
