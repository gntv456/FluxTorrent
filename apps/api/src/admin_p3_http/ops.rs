//! 后台运维三件（0078，U3D 口径，v3 §27-18）：备份面板 / 任务手动触发 / 版本页。
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 后台运维三件（0078，U3D 口径，v3 §27-18） ============
// ① backup 面板：备份文件列表 + 触发即时备份（容器内 pg_dump）
// ② 任务手动触发器：常用 worker 周期任务即时跑一次（HTTP 侧直接调用 job 函数）
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
    let st = tokio::process::Command::new("docker")
        .args([
            "exec",
            "flux-postgres",
            "pg_dump",
            "-U",
            "flux",
            "-Fc",
            "fluxtorrent",
        ])
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

/// ② 任务手动触发器：支持任务名 → 即时执行（worker 侧 job 的 API 直查版本）
#[derive(Deserialize)]
struct JobTriggerReq {
    job: String,
}

#[post("/admin/jobs/run")]
async fn admin_job_trigger(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<JobTriggerReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CLEANUP_RUN)
        .await?;
    // 与 worker 周期同款 SQL 的「读侧快查」版本：手动触发给出可观测的行数结果
    let (job, affected): (&str, i64) = match body.job.as_str() {
        "expire_promotions" => {
            let n = sqlx::query("DELETE FROM promotions WHERE ends_at <= now()")
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .rows_affected() as i64;
            ("expire_promotions", n)
        }
        "sweep_stale_peers" => {
            let n: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM snatches WHERE (seeding \
                 OR leeching) AND last_seen_at < now() - interval '90 minutes'",
            )
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            ("sweep_stale_peers", n)
        }
        "reconcile_snapshots" => {
            sqlx::query(
                "UPDATE users SET uploaded = COALESCE((SELECT sum(delta_up) FROM traffic_ledger WHERE user_id = users.id), 0), \
                 downloaded = COALESCE((SELECT sum(delta_down) FROM traffic_ledger WHERE user_id = users.id), 0)",
            )
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            ("reconcile_snapshots", -1) // 全量纠偏无行数语义
        }
        "funding_settle" => {
            // 审计修复（P1）：旧版只置 status=1，不挂促销不发通知 —— 之后 worker 版
            // WHERE status=0 匹配不到，达标承诺的 free 促销永久丢失。与 worker 同款：
            // 置状态 + 幂等挂 promotions + 给发起人发达标通知。
            let reached: Vec<(i64, i64, i32)> = sqlx::query_as(
                "UPDATE fundings SET status = 1, promoted_at = now() \
                 WHERE status = 0 AND raised >= goal \
                 RETURNING id, torrent_id, hours",
            )
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            for (fid, tid, hours) in &reached {
                let _ = sqlx::query(
                    "INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source) \
                     VALUES ('torrent', $1, 'free', now(), now() + make_interval(hours => $2::int), 'task') \
                     ON CONFLICT DO NOTHING",
                )
                .bind(tid)
                .bind(*hours as i64)
                .execute(&state.repo.db)
                .await;
                let _ = sqlx::query(
                    "INSERT INTO messages (sender_id, receiver_id, subject, body) \
                     SELECT NULL, creator_id, '众筹达标', \
                            '种子 #' || $1 || ' 的众筹已达标，已挂 ' || $2 || ' 小时免费促销。' \
                     FROM fundings WHERE id = $3",
                )
                .bind(tid)
                .bind(*hours as i64)
                .bind(fid)
                .execute(&state.repo.db)
                .await;
            }
            ("funding_settle", reached.len() as i64)
        }
        other => {
            return Err(DomainError::Validation(format!(
                "未知任务 {other}（可选：expire_promotions/sweep_stale_peers/reconcile_snapshots/funding_settle）"
            )));
        }
    };
    state.repo.audit(Some(auth.id), "job_trigger", None).await;
    Ok(ok(serde_json::json!({ "job": job, "affected": affected })))
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
