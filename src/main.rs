use axum::{
    extract::Json,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::{env, net::SocketAddr, path::PathBuf};
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Deserialize)]
struct CheckoutItem {
    name: String,
    // prix unitaire en centimes, ex 600 pour 6,00 €
    unit_amount: i64,
    quantity: u32,
}

#[derive(Debug, Deserialize)]
struct CheckoutRequest {
    items: Vec<CheckoutItem>,
}

#[derive(Debug, Serialize)]
struct CheckoutResponse {
    url: Option<String>,
    error: Option<String>,
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status":"ok","service":"oturkish-kebab","version": env!("CARGO_PKG_VERSION")}))
}

async fn config() -> impl IntoResponse {
    let publishable = env::var("STRIPE_PUBLISHABLE_KEY").unwrap_or_default();
    let has_stripe = env::var("STRIPE_SECRET_KEY").map(|k| !k.trim().is_empty()).unwrap_or(false);
    Json(serde_json::json!({
        "has_stripe": has_stripe,
        "publishable_key": if publishable.trim().is_empty() { serde_json::Value::Null } else { serde_json::Value::String(publishable) }
    }))
}

async fn create_checkout(headers: HeaderMap, Json(payload): Json<CheckoutRequest>) -> impl IntoResponse {
    if payload.items.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(CheckoutResponse { url: None, error: Some("Panier vide".into()) }),
        );
    }

    // Validation basique
    for it in &payload.items {
        if it.quantity == 0 || it.unit_amount <= 0 {
            return (
                StatusCode::BAD_REQUEST,
                Json(CheckoutResponse { url: None, error: Some(format!("Article invalide: {}", it.name)) }),
            );
        }
        if it.name.trim().is_empty() || it.name.len() > 120 {
            return (
                StatusCode::BAD_REQUEST,
                Json(CheckoutResponse { url: None, error: Some("Nom d'article invalide".into()) }),
            );
        }
    }

    // Si pas de clé Stripe -> mode mock / erreur explicite
    let secret = match env::var("STRIPE_SECRET_KEY") {
        Ok(k) if !k.trim().is_empty() => k,
        _ => {
            // Mode dev sans Stripe : on renvoie une erreur claire côté front qui propose le téléphone
            // On pourrait aussi générer une URL fake, mais mieux d'être explicite
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(CheckoutResponse {
                    url: None,
                    error: Some(
                        "Paiement en ligne non configuré (STRIPE_SECRET_KEY manquant). Configurez .env puis relancez.".into(),
                    ),
                }),
            );
        }
    };

    // Détermine l'origin pour success/cancel_url (depuis header Origin/Referer ou env)
    let origin = headers
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .or_else(|| headers.get("referer").and_then(|v| v.to_str().ok().map(|s| {
            // referer = http://127.0.0.1:8080/#menu -> on coupe au /
            s.split('#').next().unwrap_or(s).trim_end_matches('/')
        })))
        .unwrap_or("http://127.0.0.1:8080")
        .trim_end_matches('/')
        .to_string();

    // Fallback env si défini
    let success_url = env::var("STRIPE_SUCCESS_URL").unwrap_or_else(|_| format!("{}/?success=1", origin));
    let cancel_url = env::var("STRIPE_CANCEL_URL").unwrap_or_else(|_| format!("{}/?canceled=1", origin));

    // Construit le form-urlencoded pour Stripe
    // Doc Stripe: POST https://api.stripe.com/v1/checkout/sessions
    let mut params: Vec<(String, String)> = vec![
        ("mode".into(), "payment".into()),
        ("success_url".into(), success_url),
        ("cancel_url".into(), cancel_url),
    ];

    // Stripe attend line_items[0][price_data][currency], etc.
    for (i, item) in payload.items.iter().enumerate() {
        let prefix = format!("line_items[{}]", i);
        params.push((format!("{}[price_data][currency]", prefix), "eur".into()));
        params.push((format!("{}[price_data][product_data][name]", prefix), item.name.clone()));
        params.push((format!("{}[price_data][unit_amount]", prefix), item.unit_amount.to_string()));
        params.push((format!("{}[quantity]", prefix), item.quantity.to_string()));
    }

    // Optionnel: permettre le téléphone / adresse si besoin
    // params.push(("phone_number_collection[enabled]".into(), "true".into()));

    let client = reqwest::Client::new();
    let res = client
        .post("https://api.stripe.com/v1/checkout/sessions")
        .basic_auth::<String, String>(secret, None) // Stripe utilise Bearer via -u sk_test:
        // En fait basic_auth avec username=secret et password="" = -u sk_test:...
        // Equivalent à header Authorization: Bearer sk_test
        .form(&params)
        .send()
        .await;

    match res {
        Ok(r) => {
            let status = r.status();
            let body = r.text().await.unwrap_or_default();
            if !status.is_success() {
                tracing::error!("Stripe error {}: {}", status, body);
                // On tente de parser le message Stripe
                let msg = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|v| v.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()).map(|s| s.to_string()))
                    .unwrap_or(body);
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(CheckoutResponse { url: None, error: Some(format!("Stripe: {}", msg)) }),
                );
            }
            // Parse url
            let v: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::json!({}));
            let url = v.get("url").and_then(|u| u.as_str()).map(|s| s.to_string());
            if let Some(u) = url {
                (StatusCode::OK, Json(CheckoutResponse { url: Some(u), error: None }))
            } else {
                tracing::error!("Stripe no url in response: {}", body);
                (
                    StatusCode::BAD_GATEWAY,
                    Json(CheckoutResponse { url: None, error: Some("Réponse Stripe sans URL".into()) }),
                )
            }
        }
        Err(e) => {
            tracing::error!("reqwest error: {}", e);
            (
                StatusCode::BAD_GATEWAY,
                Json(CheckoutResponse { url: None, error: Some(format!("Erreur réseau Stripe: {}", e)) }),
            )
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    // Dossier statique = dossier du binaire / ou env STATIC_DIR
    let static_dir = env::var("STATIC_DIR").unwrap_or_else(|_| ".".into());
    let static_path = PathBuf::from(&static_dir);
    tracing::info!("STATIC_DIR = {}", static_path.display());

    // Vérifie que index.html existe
    if !static_path.join("index.html").exists() {
        tracing::warn!("index.html introuvable dans {}", static_path.display());
    }

    let api = Router::new()
        .route("/health", get(health))
        .route("/config", get(config))
        .route("/checkout", post(create_checkout));

    let static_for_fallback = static_path.clone();
    let fallback = ServeDir::new(&static_path)
        .append_index_html_on_directories(true)
        .not_found_service(tower::service_fn(move |_req: axum::http::Request<axum::body::Body>| {
            let index_path = static_for_fallback.join("index.html");
            async move {
                let html = tokio::fs::read_to_string(&index_path)
                    .await
                    .unwrap_or_else(|_| "<h1>index.html manquant</h1>".into());
                Ok::<_, std::convert::Infallible>(Html(html).into_response())
            }
        }));

    let app = Router::new()
        .nest("/api", api)
        .fallback_service(fallback)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("O'Turkish Kebab Rust en ligne : http://{}", addr);
    tracing::info!("API checkout : POST http://{}/api/checkout", addr);
    if env::var("STRIPE_SECRET_KEY").is_err() {
        tracing::warn!("STRIPE_SECRET_KEY non défini -> /api/checkout renverra 503 (mode mock). Créez .env à partir de .env.example");
    }

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
