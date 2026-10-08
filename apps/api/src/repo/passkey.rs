//! passkey 轮换（0308 ①，自 repo/mod.rs 拆出以守 300 行门禁）。

use crate::domain::{new_passkey, passkey_grace_hours};
use crate::errors::{DomainError, DomainResult};

use super::Repo;

impl Repo {
    /// 换 passkey。`grace` 决定旧钥的去留，两种语义必须分开：
    /// · `grace = true`：**改密顺带轮换**（10-07 P1-4）。passkey 烤在用户已下载的
    ///   每一个 .torrent 里，旧钥立刻失效 = 手上所有种子集体停种，所以给宽限窗。
    /// · `grace = false`：**显式「重置密钥」**（自助 `/me/passkey/rotate` 与后台代为
    ///   重置）。那是安全处置动作——密钥泄露/外流后才按它，留 7 天宽限等于
    ///   「撤销了但攻击者还能接着做种 7 天」，即撤销不生效（五轮 P2-6）。
    ///   主流同口径：UNIT3D 的 `User/PasskeyController` 换钥即 `cache()->forget(旧钥)`
    ///   并把旧钥送进只读的历史表，announce 只解析 `users.passkey`；
    ///   而且它的「改密」与「重置 passkey」是两个互不相干的控制器。
    pub async fn update_passkey(
        &self,
        user_id: i64,
        grace: bool,
    ) -> DomainResult<String> {
        let pk = new_passkey();
        sqlx::query(
            "UPDATE users SET passkey = $2, \
                passkey_prev = CASE WHEN $3::boolean THEN passkey END, \
                passkey_prev_until = CASE WHEN $3::boolean \
                     THEN now() + ($4::bigint * interval '1 hour') END \
             WHERE id = $1",
        )
        .bind(user_id)
        .bind(&pk)
        .bind(grace)
        .bind(passkey_grace_hours())
        .execute(&self.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(pk)
    }
}
