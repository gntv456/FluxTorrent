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
    // P-17（2026-10-25 站长八轮）：默认命令原样照抄 0208 迁移的
    // `docker exec flux-postgres pg_dump …`——compose 里 api 容器没有
    // docker 二进制（也无 socket），执行必炸 ENOENT 500，「备份」按钮
    // 从未成功过。api 与 postgres 同网络（DATABASE_URL 直连 postgres:5432），
    // 默认改为容器内 pg_dump 远连；站点仍可用 backup_docker_exec 覆盖。
    let default_args = [
        "pg_dump",
        "-h",
        "postgres",
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
    // P2（2026-10-06 安全审计）：argv[0] 可执行白名单——backup_docker_exec
    // 来自站长配置，settings 写权限与备份权限若分离即成 RCE 通道。只放行
    // 备份语义的可执行（pg_dump / docker / podman / sudo），其余当场拒绝。
    const BACKUP_EXE_ALLOW: &[&str] = &[
        "pg_dump",
        "docker",
        "podman",
        "sudo",
        "/usr/bin/pg_dump",
        "/usr/bin/docker",
        "/usr/local/bin/docker",
        "/usr/bin/sudo",
        "/usr/local/bin/pg_dump",
    ];
    // 裸名配置（pg_dump）按 basename 判定；带路径的配置须精确命中白名单全路径
    let basename = argv[0].rsplit('/').next().unwrap_or(&argv[0]).to_string();
    let allowed = BACKUP_EXE_ALLOW.iter().any(|a| {
        *a == argv[0].as_str()
            || (a.rsplit('/').next().unwrap_or(a) == basename
                && !argv[0].contains('/'))
    });
    if !allowed {
        return Err(DomainError::Validation(format!(
            "backup_docker_exec 可执行 {:?} 不在白名单（pg_dump/docker/podman/sudo）",
            argv[0]
        )));
    }
    let st = tokio::process::Command::new(&argv[0])
        .args(&argv[1..])
        // 直连 postgres 时口令走 PGPASSWORD（DATABASE_URL 里已有；
        // 单独拆出来免得出现在 argv/错误正文里）
        .env(
            "PGPASSWORD",
            std::env::var("DB_PASSWORD").unwrap_or_default(),
        )
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
    // 二轮遗留（2026-10-07）：备份含全库敏感数据，落盘后收紧权限 600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(
            &out,
            std::fs::Permissions::from_mode(0o600),
        );
    }
    // P-227：备份动作的审计要留下文件名——「有人跑过一次备份」而不知道
    // 产出的是哪个文件，等于没有审计。
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "backup_run",
            None,
            None,
            Some(serde_json::json!({"file": out.clone()})),
        )
        .await;
    Ok(ok(serde_json::json!({ "file": out, "bytes": n })))
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
    .map_err(|e| DomainError::Internal(e.into()))?;
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
