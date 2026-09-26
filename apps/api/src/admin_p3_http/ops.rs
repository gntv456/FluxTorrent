//! 后台运维三件（0078，U3D 口径，v3 §27-18）：备份面板 / 任务调度 / 版本页。
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 后台运维三件（0078，U3D 口径，v3 §27-18） ============
// ① backup 面板：备份文件列表 + 触发即时备份（容器内 pg_dump）
// ② 任务调度：全量 job 目录/节奏/最近运行 + 手动触发（入队，worker 本体执行）
// ③ 版本页：git 版本 + 各组件运行信息

/// ① 备份面板：列 backups 目录（缺省 ./backups，与 scripts/backup.sh 同目录约定）
#[get("/admin/backups")]
async fn admin_backups_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let dir =
        std::env::var("FLUX_BACKUP_DIR").unwrap_or_else(|_| "./backups".into());
    let mut files: Vec<(String, u64)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("fluxtorrent-") && name.ends_with(".dump") {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                files.push((name, size));
            }
        }
    }
    files.sort();
    files.reverse();
    // 恢复 runbook 提示（审计修复 P2：备份有备份无恢复）：恢复是高危整库替换操作，
    // 不开 HTTP 端点（防误触/防越权），指引走 scripts/restore.sh 的 drill→force 两段流程。
    Ok(ok(serde_json::json!({
        "dir": dir,
        "files": files,
        "restore_runbook": "scripts/restore.sh <dump> --drill  # 先临时库校验；确认后 --force 整库恢复（自动做安全备份）",
    })))
}

/// ① 触发即时备份（同步执行 pg_dump；万级种子约秒级，可接受）
#[post("/admin/backups/run")]
async fn admin_backup_run(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let stamp = chrono::Utc::now().format("%Y-%m-%d-%H%M%S");
    let dir =
        std::env::var("FLUX_BACKUP_DIR").unwrap_or_else(|_| "./backups".into());
    std::fs::create_dir_all(&dir)
        .map_err(|e| DomainError::Internal(e.into()))?;
    let out = format!("{dir}/fluxtorrent-manual-{stamp}.dump");
    // 0209 P2-19：备份命令可配置（site_settings.backup_docker_exec）——
    // 默认 docker exec 容器内 pg_dump（compose 部署）；裸机部署改为本地
    // pg_dump 全路径（如 pg_dump -U flux -Fc fluxtorrent）。
    // 配置按空白切分 argv，输出恒重定向到备份目录文件（防误用吞盘）。
    let cfg_cmd: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'backup_docker_exec'), '')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_default();
    let default_args = [
        "docker",
        "exec",
        "flux-postgres",
        "pg_dump",
        "-U",
        "flux",
        "-Fc",
        "fluxtorrent",
    ];
    let argv: Vec<String> = if cfg_cmd.trim().is_empty() {
        default_args.iter().map(|s| s.to_string()).collect()
    } else {
        cfg_cmd.split_whitespace().map(String::from).collect()
    };
    let st = tokio::process::Command::new(&argv[0])
        .args(&argv[1..])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !st.status.success() {
        return Err(DomainError::Internal(
            anyhow::anyhow!(
                "pg_dump 失败: {}",
                String::from_utf8_lossy(&st.stderr)
            )
            .into(),
        ));
    }
    let n = st.stdout.len() as u64;
    tokio::fs::write(&out, &st.stdout)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "backup_run", None).await;
    Ok(ok(serde_json::json!({ "file": out, "bytes": n })))
}

// ============ ② 任务调度（0218 G7） ============
// 旧版 /admin/jobs/run + /run2 是「与 worker 同款 SQL 的复刻」（run2 自述如此）：
// job 演进后复刻口径会静默漂移，且只覆盖 10/40+ 个任务。现改为**入队**——
// POST 写 job_triggers，worker 在 60s tick 里用本体函数执行（同一把 advisory
// 锁，与定时触发互斥），结果写回该行；面板展示 = job_status（目录+最近运行）。
// 该跑什么、怎么跑，只有 worker 一个真相源。

#[derive(serde::Serialize, sqlx::FromRow)]
struct JobRow {
    job: String,
    cadence: String,
    last_started_at: Option<chrono::DateTime<chrono::Utc>>,
    last_finished_at: Option<chrono::DateTime<chrono::Utc>>,
    last_ok: Option<bool>,
    last_result: Option<String>,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct TriggerRow {
    id: i64,
    job: String,
    status: String,
    requested_at: chrono::DateTime<chrono::Utc>,
    finished_at: Option<chrono::DateTime<chrono::Utc>>,
    ok: Option<bool>,
    result: Option<String>,
}

#[get("/admin/jobs")]
async fn admin_jobs_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CLEANUP_RUN)
        .await?;
    let jobs: Vec<JobRow> = sqlx::query_as(
        "SELECT job, cadence, last_started_at, last_finished_at, last_ok, \
         last_result FROM job_status ORDER BY cadence, job",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let triggers: Vec<TriggerRow> = sqlx::query_as(
        "SELECT id, job, status, requested_at, finished_at, ok, result \
         FROM job_triggers ORDER BY id DESC LIMIT 15",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "jobs": jobs, "triggers": triggers }),
    ))
}

#[derive(Deserialize)]
struct JobTriggerReq {
    job: String,
}

#[post("/admin/jobs/run")]
async fn admin_job_run(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<JobTriggerReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CLEANUP_RUN)
        .await?;
    let job = body.job.trim();
    // 目录校验：只有 worker 注册过的任务名可入队（防手填错名永远 pending）
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM job_status WHERE job = $1)",
    )
    .bind(job)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !exists {
        return Err(DomainError::Validation(
            "未知任务（worker 未注册该任务名）".to_string(),
        ));
    }
    // 连点合并：同任务已有待执行行则直接复用（多实例部署下也不重复排队）
    let pending: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM job_triggers WHERE job = $1 AND status = 'pending' \
         ORDER BY id LIMIT 1",
    )
    .bind(job)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(id) = pending {
        return Ok(ok(
            serde_json::json!({ "id": id, "job": job, "coalesced": true }),
        ));
    }
    // 队列上限：worker 每 tick 只认领 3 条，积压过多说明 worker 没在跑
    let queue: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM job_triggers \
         WHERE status IN ('pending', 'running')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    if queue >= 20 {
        return Err(DomainError::Validation(
            "触发队列已满（20 条待执行）——worker 可能未运行".to_string(),
        ));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO job_triggers (job, requested_by) VALUES ($1, $2) \
         RETURNING id",
    )
    .bind(job)
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "job_trigger", None).await;
    Ok(ok(
        serde_json::json!({ "id": id, "job": job, "coalesced": false }),
    ))
}

/// ③ 版本页：构建信息 + git 版本 + 组件健康（U3D 版本页口径）
#[get("/admin/version")]
async fn admin_version(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_PANEL)
        .await?;
    let (users, torrents, peers): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM users), (SELECT count(*) FROM torrents), \
                (SELECT count(*) FROM snatches WHERE seeding OR leeching)",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let latest_migration: String = sqlx::query_scalar(
        "SELECT description FROM _sqlx_migrations ORDER BY version \
         DESC LIMIT 1",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_default();
    let git = tokio::process::Command::new("git")
        .args(["log", "-1", "--format=%h %cs %s"])
        .current_dir(".")
        .output()
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    Ok(ok(serde_json::json!({
        "app": "FluxTorrent",
        "batch": "0078 longtail",
        "git": git,
        "db": { "users": users, "torrents": torrents, "active_peers": peers },
        "latest_migration": latest_migration,
    })))
}
