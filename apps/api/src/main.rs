//! FluxTorrent API 服务入口。
//! 启动：迁移 schema → 连接池/Redis → HTTP 服务（信封 + CORS + tracing）。

mod admin_http;
mod admin_p2_http;
mod admin_p3_http;
mod archive;
mod attachment_http;
mod attachment_video;
mod attachment_video_upload;
mod auth;
mod auth_http;
mod authz;
mod bencode;
mod community_http;
mod compat_http;
mod config;
mod content_http;
mod domain;
mod dto;
mod economy;
mod economy_http;
mod errors;
mod games;
mod games_http;
mod gaps_http;
mod geo;
mod http;
mod i18n;
mod invite_http;
mod mailer;
mod modules;
mod openapi_http;
mod ops_http;
mod payment;
mod plugins;
mod publish_http;
mod push_http;
mod repo;
mod request_id;
mod adapter_http;
mod adapter_seed;
mod adapter_runtime;
mod rules_engine;
mod rss_http;
mod settings_http;
mod setup_http;
mod social_http;
mod staff_http;
mod state;
mod storage;
mod torrent_http;
mod torrents;
mod twofa_http;
mod v4_http;

use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer};

/// CORS：CORS_ORIGINS 逗号分隔白名单（生产必填）；未配置时仅开发态（FLUX_DEV=1）退化为宽松并打警告
fn build_cors() -> actix_cors::Cors {
    let origins = std::env::var("CORS_ORIGINS").unwrap_or_default();
    let list: Vec<&str> = origins
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if list.is_empty() {
        let dev = std::env::var("FLUX_DEV").unwrap_or_default() == "1";
        if !dev {
            panic!("CORS_ORIGINS 未配置：生产环境禁止宽松 CORS（设 FLUX_DEV=1 跳过开发态检查）");
        }
        tracing::warn!(
            "CORS_ORIGINS 未配置，使用宽松 CORS（仅限开发态；生产由网关收敛）"
        );
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
    let state =
        web::Data::new(std::sync::Arc::new(state::AppState::new(cfg).await?));

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

    // 演示账号防线（审计 P0）：0018 迁移自带 12 个口令为 password123 的演示账号
    //（argon2 哈希公开在迁移文件里，任何拿到源码的人都能直接登录——含 class 6 高权限）。
    // 生产态（非 FLUX_DEV=1）启动时把仍持有该公开哈希的账号口令随机化（幂等：中性化后
    // 哈希不再匹配，下次启动零行）；演示数据本体建议随后执行 0108 迁移清理逻辑删除。
    if std::env::var("FLUX_DEV").unwrap_or_default() != "1" {
        const DEMO_PUBLIC_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$XAMwi8WTuzejCBPhdilR6w$xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c";
        use rand::Rng;
        let rnd: String = (0..43)
            .map(|_| {
                rand::thread_rng().sample(rand::distributions::Alphanumeric)
                    as char
            })
            .collect();
        let new_hash = crate::domain::hash_password(&rnd);
        match sqlx::query_scalar::<_, i64>(
            "WITH neutralized AS (\
               UPDATE users SET pass_hash = $1 WHERE pass_hash = $2 RETURNING 1\
             ) SELECT count(*) FROM neutralized",
        )
        .bind(new_hash.as_deref().unwrap_or(""))
        .bind(DEMO_PUBLIC_HASH)
        .fetch_one(&state.repo.db)
        .await
        {
            Ok(n) if n > 0 => {
                tracing::error!(
                    count = n,
                    "检测到 {n} 个账号仍使用 0018 演示数据公开口令（password123），已随机化其口令；请运行 0108 清理逻辑删除演示数据"
                );
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("演示账号防线检查失败（不阻塞启动）: {e}");
            }
        }
    }

    // 内置自营适配器上架（二审 R10-3：破「adapters 零行空货架」——douban
    // wasm 随核心发版，幂等落库、默认停用、编译预检失败仅告警）
    {
        let st = state.clone();
        crate::adapter_seed::ensure_builtin_adapters(&st.repo.db).await;
    }

    tracing::info!("flux-api listening on {bind}");
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            // malformed JSON 等载荷解析错误统一走信封（原为 actix 原生 text/plain 400，
            // 前端 api-client 按 content-type 判非 JSON 会误报"服务异常"）
            .app_data(actix_web::web::JsonConfig::default().error_handler(|err, _req| {
                let locale = i18n::current();
                let body = serde_json::json!({
                    "code": 1002,
                    "message": format!("{}: {}", crate::i18n::localized_message(1002, locale), err),
                    "data": null,
                    "request_id": crate::request_id::current()
                });
                // error_handler 需返回 actix_web::Error；InternalError 是标准包裹方式
                actix_web::error::InternalError::from_response(
                    err.to_string(),
                    actix_web::HttpResponse::BadRequest().json(body),
                )
                .into()
            }))
            // 访问日志（P2 防凭据泄漏）：默认 %r 含完整 query——compat 下载走 ?passkey=、
            // 凭证下载走 ?token=、开放 API 走 ?apikey=，等价把长期/短期凭据写进 access log。
            // 自定义格式以 %r 换成 %m %U（method + 不含 query 的 path）；其余与 default 对齐。
            .wrap(Logger::new("%a \"%m %U\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T").exclude("/api/v1/health"))
            .wrap(build_cors()) // 来源白名单（CORS_ORIGINS）；空则开发态宽松 + 警告
            // 安全响应头基线（§5.7）：nosniff / 防点击劫持 / 引用策略
            // （HSTS 由 TLS 终结的反代统一注入；完整 CSP 需 nonce 基建，web 侧已配基础头）
            .wrap(
                actix_web::middleware::DefaultHeaders::new()
                    .add(("X-Content-Type-Options", "nosniff"))
                    .add(("X-Frame-Options", "DENY"))
                    .add(("Referrer-Policy", "strict-origin-when-cross-origin")),
            )
            .wrap(actix_web::middleware::from_fn(request_id::request_id_mw)) // request_id 贯穿（信封/响应头/日志同源）
            .wrap(actix_web::middleware::from_fn(i18n::locale_mw)) // Accept-Language → task-local（错误消息三语）
            .wrap(actix_web::middleware::from_fn(setup_http::setup_gate_mw)) // U3 安装向导封锁（setup_done 未置位拦业务 API）
            .wrap(actix_web::middleware::from_fn(modules::module_gate_mw)) // U1 模块网关：可选域 fail-close（4101）
            .wrap(actix_web::middleware::from_fn(v4_http::metrics_mw)) // G3：请求/5xx 计数（/metrics 出口）
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
                    "request_id": crate::request_id::current()
                }))
            }))
    })
    .bind(&bind)?
    .run()
    .await?;
    Ok(())
}
