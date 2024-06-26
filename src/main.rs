mod contributions;
use axum::http::request::Parts;
use axum::http::HeaderValue;
use axum::routing::get;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::{info, level_filters::LevelFilter};
use tracing::{instrument, warn};

use crate::contributions::contributions;

use axum::Router;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::{fmt, prelude::*, Registry};

#[derive(Debug)]
pub struct TheState {
    pub github_token: String,
    pub user: Option<String>,
    pub contributions_cache: Arc<RwLock<Option<Vec<contributions::ContributionDay>>>>,
    pub contributions_last_cache_time_ms: Arc<RwLock<i64>>,
}

impl TheState {
    pub fn new(github_token: String, user: Option<String>) -> Self {
        Self {
            github_token,
            user,
            contributions_cache: Arc::new(None.into()),
            contributions_last_cache_time_ms: Arc::new(0.into()),
        }
    }
}

pub type SharedState = Arc<TheState>;

#[tokio::main]
#[instrument]
async fn main() {
    let env = std::env::var("ENV").unwrap_or("production".into());
    if env == "development" {
        tracing_subscriber::fmt().without_time().init();
    } else {
        let env_filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::DEBUG.into())
            .from_env()
            .expect("Failed to create env filter invalid RUST_LOG env var");

        let registry = Registry::default().with(env_filter).with(fmt::layer());

        if let Ok(_) = std::env::var("AXIOM_TOKEN") {
            let axiom_layer = tracing_axiom::builder()
                .with_service_name("gh")
                .with_tags(&[(
                    &"deployment_id",
                    &std::env::var("RAILWAY_DEPLOYMENT_ID")
                        .map(|s| {
                            s + "-"
                                + std::env::var("RAILWAY_DEPLOYMENT_ID")
                                    .unwrap_or("unknown_replica".into())
                                    .as_str()
                        })
                        .unwrap_or("unknown_deployment".into()),
                )])
                .with_tags(&[(&"service.name", "gh".into())])
                .layer()
                .expect("Axiom layer failed to initialize");

            registry
                .with(axiom_layer)
                .try_init()
                .expect("Failed to initialize tracing with axiom");
            info!("Initialized tracing with axiom");
        } else {
            registry.try_init().expect("Failed to initialize tracing");
        }
    };

    let auth_token = std::env::var("GITHUB_TOKEN").expect("AUTH_TOKEN env var set");
    let state = Arc::new(TheState::new(auth_token, std::env::var("GITHUB_URL").ok()));

    let app = Router::new()
        .route("/contributions/:user", get(contributions))
        .layer(CorsLayer::new().allow_origin(AllowOrigin::predicate(
            |origin: &HeaderValue, _request_parts: &Parts| {
                if let Ok(host) = origin.to_str() {
                    return [
                        "https://finndore.dev",
                        "finnnn.vercel.app",
                        "http://localhost:3000",
                    ]
                    .into_iter()
                    .any(|allowed_origin| host.ends_with(allowed_origin));
                }
                warn!(?origin, "Cors layer failed to parse origin header");
                false
            },
        )))
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or("3002".to_string());
    let host = format!("0.0.0.0:{}", port);
    info!("Running server on {}", host);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(host).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
