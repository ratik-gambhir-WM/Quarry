use helix_db::dsl::prelude::*;

use crate::core::nodes::deal_node::DealNode;

pub const DEAL_LABEL: &str = "Deal";

pub fn add_deal(deal: DealNode) -> Result<DynamicQueryRequest, String> {
    validate_deal_id(&deal.deal_id)?;
    Ok(add_deal_mutation(
        deal.deal_id,
        deal.deal_name,
        deal.status,
        deal.start_date,
        deal.close_date,
        deal.transaction_type,
        deal.target_company,
        deal.primary_buyer,
        deal.deal_sponsor,
    ))
}

#[allow(clippy::too_many_arguments)]
#[register]
fn add_deal_mutation(
    deal_id: String,
    deal_name: String,
    status: String,
    start_date: String,
    close_date: String,
    transaction_type: String,
    target_company: String,
    primary_buyer: String,
    deal_sponsor: String,
) -> WriteBatch {
    let _ = (
        &deal_id,
        &deal_name,
        &status,
        &start_date,
        &close_date,
        &transaction_type,
        &target_company,
        &primary_buyer,
        &deal_sponsor,
    );
    write_batch()
        .var_as(
            "existing_deal",
            g().n_with_label(DEAL_LABEL)
                .where_(Predicate::eq_param("deal_id", "deal_id")),
        )
        .var_as_if(
            "updated_deal",
            BatchCondition::VarNotEmpty("existing_deal".to_string()),
            g().n(NodeRef::var("existing_deal"))
                .set_property("deal_id", PropertyInput::param("deal_id"))
                .set_property("deal_name", PropertyInput::param("deal_name"))
                .set_property("status", PropertyInput::param("status"))
                .set_property("start_date", PropertyInput::param("start_date"))
                .set_property("close_date", PropertyInput::param("close_date"))
                .set_property("transaction_type", PropertyInput::param("transaction_type"))
                .set_property("target_company", PropertyInput::param("target_company"))
                .set_property("primary_buyer", PropertyInput::param("primary_buyer"))
                .set_property("deal_sponsor", PropertyInput::param("deal_sponsor"))
                .project(deal_projection()),
        )
        .var_as_if(
            "created_deal",
            BatchCondition::VarEmpty("existing_deal".to_string()),
            g().add_n(
                DEAL_LABEL,
                vec![
                    ("deal_id", PropertyInput::param("deal_id")),
                    ("deal_name", PropertyInput::param("deal_name")),
                    ("status", PropertyInput::param("status")),
                    ("start_date", PropertyInput::param("start_date")),
                    ("close_date", PropertyInput::param("close_date")),
                    ("transaction_type", PropertyInput::param("transaction_type")),
                    ("target_company", PropertyInput::param("target_company")),
                    ("primary_buyer", PropertyInput::param("primary_buyer")),
                    ("deal_sponsor", PropertyInput::param("deal_sponsor")),
                ],
            )
            .project(deal_projection()),
        )
        .returning(["updated_deal", "created_deal"])
}

pub fn get_deal_by_id(deal_id: String) -> Result<DynamicQueryRequest, String> {
    validate_deal_id(&deal_id)?;
    Ok(get_deal_by_id_query(deal_id))
}

#[register]
fn get_deal_by_id_query(deal_id: String) -> ReadBatch {
    let _ = &deal_id;
    read_batch()
        .var_as(
            "deal",
            g().n_with_label(DEAL_LABEL)
                .where_(Predicate::eq_param("deal_id", "deal_id"))
                .limit(1)
                .project(deal_projection()),
        )
        .returning(["deal"])
}

#[register]
pub fn create_deal_indexes() -> WriteBatch {
    write_batch()
        .var_as(
            "deal_id_unique",
            g().create_index_if_not_exists(IndexSpec::node_unique_equality(DEAL_LABEL, "deal_id")),
        )
        .var_as(
            "deal_transaction_type",
            g().create_index_if_not_exists(IndexSpec::node_equality(
                DEAL_LABEL,
                "transaction_type",
            )),
        )
        .var_as(
            "deal_close_date",
            g().create_index_if_not_exists(IndexSpec::node_equality(DEAL_LABEL, "close_date")),
        )
}

fn deal_projection() -> Vec<PropertyProjection> {
    vec![
        PropertyProjection::renamed("$id", "helix_id"),
        PropertyProjection::new("deal_id"),
        PropertyProjection::new("deal_name"),
        PropertyProjection::new("status"),
        PropertyProjection::new("start_date"),
        PropertyProjection::new("close_date"),
        PropertyProjection::new("transaction_type"),
        PropertyProjection::new("target_company"),
        PropertyProjection::new("primary_buyer"),
        PropertyProjection::new("deal_sponsor"),
    ]
}

fn validate_deal_id(id: &str) -> Result<(), String> {
    if !id.starts_with("DEAL-") {
        Err("deal id must start with DEAL-".to_string())
    } else {
        Ok(())
    }
}
