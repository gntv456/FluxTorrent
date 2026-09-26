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
        "docker", "exec", "flux-postgres", "pg_dump", "-U", "flux", "-Fc",
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

/// 0209 P2-16：worker 周期任务的「等价 SQL」手动触发（run2）。
/// 与 run 的四个任务同口径——不是把 worker 函数搬过来（跨 crate），
/// 而是复刻各 job 的核心 SQL（幂等、重跑安全语义不变），
/// 供「漏跑补偿」场景使用（如银行结息在 run、等级调整等在此）。
#[derive(Deserialize)]
struct JobTrigger2Req {
    job: String,
}

#[post("/admin/jobs/run2")]
async fn admin_job_trigger2(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<JobTrigger2Req>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CLEANUP_RUN)
        .await?;
    let (job, affected): (&str, i64) = match body.job.as_str() {
        // H&R 违规快查（hr_violations 未处置计数，可观测）
        "hr_enforce" => {
            let n: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM hr_violations WHERE resolved_at IS NULL",
            )
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            ("hr_enforce", n)
        }
        // H&R 惩罚（命中未处置者停下载权限——users.download_enabled）
        "hr_punish" => {
            let n = sqlx::query(
                "UPDATE users u SET download_enabled = false                  WHERE u.download_enabled AND EXISTS (                    SELECT 1 FROM hr_violations v                    WHERE v.user_id = u.id AND v.resolved_at IS NULL)",
            )
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected() as i64;
            ("hr_punish", n)
        }
        // 等级自动调整（只升不降，取满足条件的最高档）。
        // 判定列与 worker class_adj 同源：min_uploaded/min_download_count/
        // min_seed_hours/min_account_age_days（0211 复检：旧写法引用了不存在的
        // cr.min_downloaded，空库也会 500）
        "class_auto_adjust" => {
            let n = sqlx::query(
                r#"WITH stats AS (
                     SELECT u.id, u.class_id, u.uploaded,
                            (SELECT count(*) FROM snatches s
                             WHERE s.user_id = u.id AND s.completed_at IS NOT NULL) AS dl,
                            (SELECT COALESCE(sum(s.seeded_seconds),0)/3600
                             FROM snatches s WHERE s.user_id = u.id) AS sh,
                            EXTRACT(DAY FROM now() - u.created_at)::bigint AS age
                     FROM users u WHERE u.status < 2 AND u.class_id < 90
                   ),
                   target AS (
                     SELECT s.id, max(r.class_id) AS new_class
                     FROM stats s JOIN class_rules r ON
                         s.uploaded >= r.min_uploaded AND s.dl >= r.min_download_count AND
                         s.sh >= r.min_seed_hours AND s.age >= r.min_account_age_days
                     GROUP BY s.id
                   )
                   UPDATE users u SET class_id = t.new_class
                   FROM target t
                   WHERE u.id = t.id AND t.new_class > u.class_id"#,
            )
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected() as i64;
            ("class_auto_adjust", n)
        }
        // 任务结算（task_claims 已领未结 → 结算标志；与 worker task_settle 的结算分支同向）
        "task_settle" => {
            let n = sqlx::query(
                "UPDATE task_claims SET settled_at = now() \
                 WHERE settled_at IS NULL AND exempted_at IS NULL \
                 AND claimed_at < now() - interval '30 days'",
            )
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected() as i64;
            ("task_settle", n)
        }
        // 重复 IP 快查（可观测计数，不动账；snatches 无 ip 列——IP 维度在
        // tracker 侧快照/登录记录，这里改用 multi_ip 的近似口径：
        // 24h 内同 IP 多账号登录特征走 maxlogin/audit 数据源，此处退化为
        // 「同 agent 多账户做种」的可观测快查）
        "multi_ip_check" => {
            let n: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM ( \
                 SELECT agent FROM snatches \
                 WHERE last_seen_at > now() - interval '24 hours' AND agent IS NOT NULL \
                 GROUP BY agent HAVING count(DISTINCT user_id) > 3) x",
            )
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            ("multi_ip_check", n)
        }
        // 泄露扫描（7 天内 passkey 查看审计计数）
        "leak_scan" => {
            let n: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM audit_log WHERE \
                 action = 'passkey.view' AND created_at > now() - interval '7 days'",
            )
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            ("leak_scan", n)
        }
        other => {
            return Err(DomainError::Validation(format!(
                "未知任务 {other}（可选：hr_enforce/hr_punish/class_auto_adjust/task_settle/multi_ip_check/leak_scan）"
            )));
        }
    };
    state.repo.audit(Some(auth.id), "job_trigger2", None).await;
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
