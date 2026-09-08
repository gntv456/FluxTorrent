//! FluxTorrent API 服务入口。
//! 启动：迁移 schema → 连接池/Redis → HTTP 服务（信封 + CORS + tracing）。

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
mod http;
mod ops_http;
mod repo;
mod state;
mod torrents;

use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer};

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
            .wrap(
                Cors::permissive(), // 生产由 Pingora 网关收敛来源（§5.1 边缘层）
            )
            .configure(community_http::configure)
            .default_service(web::to(|| async {
                actix_web::HttpResponse::NotFound().json(serde_json::json!({
                    "code": 1004, "message": "接口不存在", "data": null,
                    "request_id": uuid::Uuid::new_v4().to_string()
                }))
            }))
    })
    .bind(&bind)?
    .run()
    .await?;
    Ok(())
}
