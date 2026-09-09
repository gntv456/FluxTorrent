//! FluxTorrent API 服务入口。
//! 启动：迁移 schema → 连接池/Redis → HTTP 服务（信封 + CORS + tracing）。

mod admin_http;
mod auth;
mod bencode;
mod community_http;
mod config;
mod content_http;
mod domain;
mod dto;
mod economy;
mod economy_http;
mod errors;
mod games;
mod games_http;
mod http;
mod i18n;
mod openapi_http;
mod ops_http;
mod plugins;
mod push_http;
mod repo;
mod state;
mod torrents;

use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer};

/// CORS：CORS_ORIGINS 逗号分隔白名单（生产必填）；未配置时退化为宽松并打警告
fn build_cors() -> actix_cors::Cors {
    let origins = std::env::var("CORS_ORIGINS").unwrap_or_default();
    let list: Vec<&str> = origins
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if list.is_empty() {
        tracing::warn!("CORS_ORIGINS 未配置，使用宽松 CORS（仅限开发态；生产由网关收敛）");
        return Cors::permissive();
    }
    let mut cors = Cors::default()
        .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
        .allowed_headers(vec![
            actix_web::http::header::AUTHORIZATION,
            actix_web::http::header::CONTENT_TYPE,
        ])
        .max_age(3600);
    for o in &list {
        cors = cors.allowed_origin(o);
    }
    cors
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .init();

    let cfg = config::AppConfig::from_env()?;
    let bind = cfg.bind.clone();
    let state = web::Data::new(std::sync::Arc::new(state::AppState::new(cfg).await?));

    // 迁移（幂等）。路径解析相对 crate 根，兼容从仓库根或 apps/api 目录启动。
    {
        let st = state.clone();
        let migrations_dir = ["./migrations", "apps/api/migrations"]
            .into_iter()
            .find(|p| std::path::Path::new(p).exists())
            .unwrap_or("./migrations");
        sqlx::migrate::Migrator::new(std::path::Path::new(migrations_dir))
            .await?
            .run(&st.repo.db)
            .await?;
    }

    tracing::info!("flux-api listening on {bind}");
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(Logger::default().exclude("/api/v1/health"))
            .wrap(build_cors()) // 来源白名单（CORS_ORIGINS）；空则开发态宽松 + 警告
            .wrap(actix_web::middleware::from_fn(i18n::locale_mw)) // Accept-Language → task-local（错误消息三语）
            .configure(community_http::configure)
            .default_service(web::to(|req: actix_web::HttpRequest| async move {
                let locale = req
                    .headers()
                    .get(actix_web::http::header::ACCEPT_LANGUAGE)
                    .and_then(|v| v.to_str().ok())
                    .map(i18n::from_accept_language)
                    .unwrap_or(i18n::Locale::ZhCn);
                actix_web::HttpResponse::NotFound().json(serde_json::json!({
                    "code": 1004, "message": i18n::endpoint_not_found(locale), "data": null,
                    "request_id": uuid::Uuid::new_v4().to_string()
                }))
            }))
    })
    .bind(&bind)?
    .run()
    .await?;
    Ok(())
}
