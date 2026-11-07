use super::*;

#[test]
fn acquisition_target_rule_depends_on_engagement_status_and_transaction_structure() {
    use crate::domains::deals::model::{DealStatus, PartyInput, PartyRole, TransactionType};
    for transaction in [
        TransactionType::PlatformAcquisition,
        TransactionType::AddOnAcquisition,
        TransactionType::StrategicAcquisition,
    ] {
        let mut fields: DealFields = serde_json::from_value(serde_json::json!({"dealName":"Acquisition","dealCode":null,"startDate":null,"expectedCloseDate":null,"actualCloseDate":null,"engagementCompletedDate":null,"engagementType":"buy_side","transactionType":"unknown","status":"pipeline","parties":[],"scopes":[]})).unwrap();
        fields.transaction_type = transaction;
        for status in [DealStatus::Pipeline, DealStatus::PencilsDown] {
            fields.status = status;
            fields.parties.clear();
            assert!(validate_target_rule(&fields).is_ok());
        }
        for status in [
            DealStatus::Prepare,
            DealStatus::ReviewPlanDiscovery,
            DealStatus::InterviewsSiteVisit,
            DealStatus::DebriefFlashReport,
            DealStatus::FollowUpsAnalysis,
            DealStatus::QaReadoutCloseout,
            DealStatus::Closed,
        ] {
            fields.status = status;
            fields.parties.clear();
            assert!(validate_target_rule(&fields).is_err());
            fields.parties.push(PartyInput {
                company_id: uuid::Uuid::new_v4().to_string(),
                role: PartyRole::Target,
                notes: None,
            });
            assert!(validate_target_rule(&fields).is_ok());
            fields.parties.push(PartyInput {
                company_id: uuid::Uuid::new_v4().to_string(),
                role: PartyRole::Target,
                notes: None,
            });
            assert!(validate_target_rule(&fields).is_err());
            fields.transaction_type = TransactionType::CarveOut;
            assert!(validate_target_rule(&fields).is_ok());
            fields.transaction_type = transaction;
        }
    }
}
