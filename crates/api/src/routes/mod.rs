pub mod analysis_reports;
pub mod health;
pub mod holdings;
pub mod llm_keys;
pub mod manager_universe;
pub mod managers;
pub mod order_plans;
pub mod paper_orders;
pub mod scenarios;
pub mod schedule_api;
pub mod symbols;
pub mod trades;

use axum::{middleware, routing::get, Router};

use crate::auth::auto_auth_middleware;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .nest("/llm-keys", llm_keys::router())
        .merge(manager_universe::router())
        .nest("/managers", managers::router())
        .nest("/managers/:manager_id/scenarios", scenarios::router())
        .nest("/managers/:manager_id/holdings", holdings::router())
        .nest("/managers/:manager_id/trades", trades::router())
        .nest("/managers/:manager_id/schedule", schedule_api::router())
        .nest(
            "/managers/:manager_id/analysis-reports",
            analysis_reports::router(),
        )
        .nest("/managers/:manager_id/order-plans", order_plans::router())
        .nest("/paper/orders", paper_orders::router())
        .nest("/symbols", symbols::router())
        .layer(middleware::from_fn(auto_auth_middleware));

    Router::new()
        .route("/health", get(health::health))
        .nest("/api", protected)
        .with_state(state)
}
