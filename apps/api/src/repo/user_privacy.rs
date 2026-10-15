//! `users.privacy` 点查（`auth_http/profile_privacy.rs` 的 SQL 唯一居所）。
//! 2026-10-15 下沉：FRAME §2「H 不 import sqlx」——该点查写在 H 层
//! profile_privacy.rs 里，属批八十三（P-293）新增时带进来的 SQL，
//! 母文件是当轮新建，故按「新文件零容忍」处理而非存量基线登记。

use super::Repo;

/// 被查看者的隐私等级（`normal` / `low` / `strong` / NULL）。
/// 读失败按「无记录」处理（None）：隐私不该把公开主页拖倒，
/// 且判定侧本就按 `normal` 解释 None，方向一致。
pub async fn privacy_level(repo: &Repo, user_id: i64) -> Option<String> {
    sqlx::query_scalar("SELECT privacy FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&repo.read_db)
        .await
        .unwrap_or(None)
}
