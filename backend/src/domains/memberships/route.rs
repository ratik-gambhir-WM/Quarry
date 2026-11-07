use super::{handler::*, service::DealAccessService};
use axum::{
    routing::{get, patch, post},
    Router,
};
use std::sync::Arc;

pub(crate) fn routes(service: Arc<DealAccessService>) -> Router {
    Router::new()
        .route("/deals/{deal_id}/members", get(members).post(add_member))
        .route(
            "/deals/{deal_id}/members/{user_id}",
            patch(change_role).delete(remove_member),
        )
        .route("/deals/{deal_id}/ownership-transfer", post(transfer))
        .route("/deals/{deal_id}/teams", get(teams).post(create_team))
        .route(
            "/deals/{deal_id}/teams/{team_id}",
            patch(rename_team).delete(delete_team),
        )
        .route("/deals/{deal_id}/teams/{team_id}/members", post(assign))
        .route(
            "/deals/{deal_id}/teams/{team_id}/members/{user_id}",
            axum::routing::delete(unassign),
        )
        .route("/deals/{deal_id}/activity", get(activity))
        .with_state(service)
}
