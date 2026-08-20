use helix_db::dsl::prelude::*;

use crate::core::{helix_queries::user::persistence::USER_LABEL, nodes::deal_node::DealNode};

pub const DEAL_LABEL: &str = "Deal";
pub const USER_HAS_DEAL_LABEL: &str = "HAS_DEAL";

pub fn add_deal(deal: DealNode, user_id: i64) -> Result<DynamicQueryRequest, String> {
    let DealNode {
        id,
        deal_name,
        main_data_room_folder,
        deal_type,
        pe_firm,
        status,
        target_company,
        buyer_or_platform_company,
        parent_or_seller_company,
        carve_out_business,
        created_at,
        updated_at,
    } = deal;

    validate_deal_id(id)?;
    validate_user_id(user_id)?;

    Ok(add_deal_mutation(
        id,
        user_id,
        deal_name,
        main_data_room_folder,
        deal_type,
        pe_firm,
        status,
        optional_string_property(target_company),
        optional_string_property(buyer_or_platform_company),
        optional_string_property(parent_or_seller_company),
        optional_string_property(carve_out_business),
        created_at,
        updated_at,
    ))
}

#[allow(clippy::too_many_arguments)]
#[register]
fn add_deal_mutation(
    id: i64,
    user_id: i64,
    deal_name: String,
    main_data_room_folder: String,
    deal_type: String,
    pe_firm: String,
    status: String,
    target_company: PropertyValue,
    buyer_or_platform_company: PropertyValue,
    parent_or_seller_company: PropertyValue,
    carve_out_business: PropertyValue,
    created_at: String,
    updated_at: String,
) -> WriteBatch {
    let _ = (
        &id,
        &user_id,
        &deal_name,
        &main_data_room_folder,
        &deal_type,
        &pe_firm,
        &status,
        &target_company,
        &buyer_or_platform_company,
        &parent_or_seller_company,
        &carve_out_business,
        &created_at,
        &updated_at,
    );

    write_batch()
        .var_as(
            "existing_deal",
            g().n_with_label(DEAL_LABEL)
                .where_(Predicate::eq_param("id", "id")),
        )
        .var_as_if(
            "updated_deal",
            BatchCondition::VarNotEmpty("existing_deal".to_string()),
            g().n(NodeRef::var("existing_deal"))
                .set_property("id", PropertyInput::param("id"))
                .set_property("deal_name", PropertyInput::param("deal_name"))
                .set_property(
                    "main_data_room_folder",
                    PropertyInput::param("main_data_room_folder"),
                )
                .set_property("deal_type", PropertyInput::param("deal_type"))
                .set_property("pe_firm", PropertyInput::param("pe_firm"))
                .set_property("status", PropertyInput::param("status"))
                .set_property("target_company", PropertyInput::param("target_company"))
                .set_property(
                    "buyer_or_platform_company",
                    PropertyInput::param("buyer_or_platform_company"),
                )
                .set_property(
                    "parent_or_seller_company",
                    PropertyInput::param("parent_or_seller_company"),
                )
                .set_property(
                    "carve_out_business",
                    PropertyInput::param("carve_out_business"),
                )
                .set_property("created_at", PropertyInput::param("created_at"))
                .set_property("updated_at", PropertyInput::param("updated_at"))
                .project(deal_projection()),
        )
        .var_as_if(
            "created_deal",
            BatchCondition::VarEmpty("existing_deal".to_string()),
            g().add_n(
                DEAL_LABEL,
                vec![
                    ("id", PropertyInput::param("id")),
                    ("deal_name", PropertyInput::param("deal_name")),
                    (
                        "main_data_room_folder",
                        PropertyInput::param("main_data_room_folder"),
                    ),
                    ("deal_type", PropertyInput::param("deal_type")),
                    ("pe_firm", PropertyInput::param("pe_firm")),
                    ("status", PropertyInput::param("status")),
                    ("target_company", PropertyInput::param("target_company")),
                    (
                        "buyer_or_platform_company",
                        PropertyInput::param("buyer_or_platform_company"),
                    ),
                    (
                        "parent_or_seller_company",
                        PropertyInput::param("parent_or_seller_company"),
                    ),
                    (
                        "carve_out_business",
                        PropertyInput::param("carve_out_business"),
                    ),
                    ("created_at", PropertyInput::param("created_at")),
                    ("updated_at", PropertyInput::param("updated_at")),
                ],
            )
            .project(deal_projection()),
        )
        .var_as(
            "deal",
            g().n_with_label(DEAL_LABEL)
                .where_(Predicate::eq_param("id", "id")),
        )
        .var_as(
            "user",
            g().n_with_label(USER_LABEL)
                .where_(Predicate::eq_param("id", "user_id")),
        )
        .var_as(
            "existing_user_deal",
            g().e_with_label(USER_HAS_DEAL_LABEL)
                .where_(Predicate::eq_param("user_id", "user_id"))
                .where_(Predicate::eq_param("deal_id", "id")),
        )
        .var_as_if(
            "user_has_deal",
            BatchCondition::VarEmpty("existing_user_deal".to_string()),
            g().n(NodeRef::var("user")).add_e(
                USER_HAS_DEAL_LABEL,
                NodeRef::var("deal"),
                vec![
                    ("user_id", PropertyInput::param("user_id")),
                    ("deal_id", PropertyInput::param("id")),
                ],
            ),
        )
        .returning([
            "updated_deal",
            "created_deal",
            "existing_user_deal",
            "user_has_deal",
        ])
}

pub fn get_deal_by_id(deal_id: i64) -> Result<DynamicQueryRequest, String> {
    validate_deal_id(deal_id)?;
    Ok(get_deal_by_id_query(deal_id))
}

#[register]
fn get_deal_by_id_query(id: i64) -> ReadBatch {
    let _ = &id;
    read_batch()
        .var_as(
            "deal",
            g().n_with_label(DEAL_LABEL)
                .where_(Predicate::eq_param("id", "id"))
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
            g().create_index_if_not_exists(IndexSpec::node_unique_equality(DEAL_LABEL, "id")),
        )
        .var_as(
            "deal_type",
            g().create_index_if_not_exists(IndexSpec::node_equality(DEAL_LABEL, "deal_type")),
        )
        .var_as(
            "deal_pe_firm",
            g().create_index_if_not_exists(IndexSpec::node_equality(DEAL_LABEL, "pe_firm")),
        )
        .var_as(
            "deal_updated_at",
            g().create_index_if_not_exists(IndexSpec::node_equality(DEAL_LABEL, "updated_at")),
        )
        .var_as(
            "user_has_deal_user_id",
            g().create_index_if_not_exists(IndexSpec::edge_equality(
                USER_HAS_DEAL_LABEL,
                "user_id",
            )),
        )
        .var_as(
            "user_has_deal_deal_id",
            g().create_index_if_not_exists(IndexSpec::edge_equality(
                USER_HAS_DEAL_LABEL,
                "deal_id",
            )),
        )
}

fn deal_projection() -> Vec<PropertyProjection> {
    vec![
        PropertyProjection::renamed("$id", "helix_id"),
        PropertyProjection::new("id"),
        PropertyProjection::new("deal_name"),
        PropertyProjection::new("main_data_room_folder"),
        PropertyProjection::new("deal_type"),
        PropertyProjection::new("pe_firm"),
        PropertyProjection::new("status"),
        PropertyProjection::new("target_company"),
        PropertyProjection::new("buyer_or_platform_company"),
        PropertyProjection::new("parent_or_seller_company"),
        PropertyProjection::new("carve_out_business"),
        PropertyProjection::new("created_at"),
        PropertyProjection::new("updated_at"),
    ]
}

fn optional_string_property(value: Option<String>) -> PropertyValue {
    value.map_or(PropertyValue::Null, PropertyValue::String)
}

fn validate_deal_id(id: i64) -> Result<(), String> {
    if id <= 0 {
        Err("deal id must be greater than zero".to_string())
    } else {
        Ok(())
    }
}

fn validate_user_id(id: i64) -> Result<(), String> {
    if id <= 0 {
        Err("user id must be greater than zero".to_string())
    } else {
        Ok(())
    }
}
