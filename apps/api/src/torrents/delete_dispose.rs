//! 种子删除的站方处置（NP 管理删除表单口径）：弹窗取数、火花回收、
//! 封禁发种权限、操作台账。
//!
//! 原先 DELETE /torrents/{id} 只做软删、不收任何理由与处置，站方裁决
//! 无从留痕。SQL 住在本域而不是 handler：`layer_guard` 对**新建**的
//! `*_http/` 文件是 sqlx 零容忍，取数写在这里既过门也不改分层方向。
//!
//! 处置对象永远是**发种人**；作者删自己的未过审种没有可处置对象，
//! 由 handler 侧的 `can_dispose` 先拒掉（见 torrent_http/torrent_delete.rs）。

use sqlx::PgPool;

use crate::authz;
use crate::economy_http;
use crate::errors::{DomainError, DomainResult};

/// 待回收火花的幂等键前缀（同种同人同档只扣一次，重放安全）
const SPARK_IDEM: &str = "del-spark";

/// 删除弹窗的一次性取数（提交时后端重算火花，不信任前端回传值）
pub struct DeleteView {
    pub owner_id: Option<i64>,
    pub owner_name: String,
    pub seeders: i32,
    pub leechers: i32,
    pub approval_status: i16,
    /// 发种人从**这一颗种**实际入袋的火花（付费出售等正向流水求和）
    pub spark: i64,
    /// 发种人当前是否已被显式禁发（user_permissions 覆盖为 false）
    pub upload_banned: bool,
}

impl DeleteView {
    /// 台账/响应用的对外形状
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "owner_id": self.owner_id,
            "owner_name": self.owner_name,
            "seeders": self.seeders,
            "leechers": self.leechers,
            "peers": self.seeders + self.leechers,
            "approval_status": self.approval_status,
            "spark": self.spark,
            "upload_banned": self.upload_banned,
        })
    }
}

/// 删除前取数：种不存在时报 NotFound（与软删口同口径）。
pub async fn delete_view(
    db: &PgPool,
    torrent_id: i64,
) -> DomainResult<DeleteView> {
    let row: Option<(Option<i64>, Option<String>, i32, i32, i16)> =
        sqlx::query_as(
            "SELECT t.owner_id, u.username, t.seeders, t.leechers, \
                t.approval_status \
         FROM torrents t LEFT JOIN users u ON u.id = t.owner_id \
         WHERE t.id = $1",
        )
        .bind(torrent_id)
        .fetch_optional(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner_id, owner_name, seeders, leechers, approval_status)) = row
    else {
        return Err(DomainError::NotFound(torrent_id));
    };
    let spark = spark_earned(db, torrent_id, owner_id).await?;
    let upload_banned = match owner_id {
        Some(o) => is_upload_banned(db, o).await?,
        None => false,
    };
    Ok(DeleteView {
        owner_id,
        owner_name: owner_name.unwrap_or_default(),
        seeders,
        leechers,
        approval_status,
        spark,
        upload_banned,
    })
}

/// 发种人从该种入袋的火花 = `spark_ledger` 里 ref 指向该种、收款人为发种人
/// 的**正向**流水之和。发布当下不给火花（upload.rs 零火花语句），所以这笔
/// 钱目前只可能是付费种出售净额（kind=torrent_sell，charge.rs 写入）；
/// 按 ref 求和而不是按 kind 求和，将来新增「按种入账」的 kind 不必再改这里。
pub async fn spark_earned(
    db: &PgPool,
    torrent_id: i64,
    owner_id: Option<i64>,
) -> DomainResult<i64> {
    let Some(owner_id) = owner_id else {
        return Ok(0);
    };
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(sum(amount), 0)::bigint FROM spark_ledger \
         WHERE ref_type = 'torrent' AND ref_id = $1 AND user_id = $2 \
           AND amount > 0",
    )
    .bind(torrent_id)
    .bind(owner_id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

/// 发种权限是否已被用户级覆盖显式拒掉（未覆盖=跟随角色判定，不算禁发）
async fn is_upload_banned(db: &PgPool, owner_id: i64) -> DomainResult<bool> {
    let granted: Option<bool> = sqlx::query_scalar(
        "SELECT granted FROM user_permissions WHERE user_id = $1 \
         AND permission_key = $2",
    )
    .bind(owner_id)
    .bind(authz::perm::TORRENT_UPLOAD)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(granted == Some(false))
}

/// 一次删除携带的处置意图（handler 校验过形状后才传进来）
pub struct Disposal {
    pub reason: Option<String>,
    pub note: Option<String>,
    pub deduct_spark: bool,
    pub double_spark: bool,
    pub ban_upload: bool,
}

impl Disposal {
    /// 无请求体（兼容旧调用方）＝只删不处置、理由留空
    pub fn plain() -> Self {
        Self {
            reason: None,
            note: None,
            deduct_spark: false,
            double_spark: false,
            ban_upload: false,
        }
    }
}

/// 处置结果：账面数 + **必须出声**的失败清单
#[derive(Default)]
pub struct DisposalOutcome {
    /// 本次可回收的火花基数（0＝该种没有入账，跳过扣除）
    pub spark_base: i64,
    /// 实扣火花（含双倍后的总额）；幂等命中时为 0＝账面没再动
    pub spark_deducted: i64,
    /// 幂等键已存在（同种同人同档重复提交）：没有二次扣款
    pub spark_replayed: bool,
    pub upload_banned: bool,
    pub errors: Vec<String>,
}

/// 执行处置。删除已经落库，这里任何一步失败都**不回滚删除**、也不
/// `let _ =` 吞掉：失败原样收进 errors，由 handler 回给界面点名。
/// 每步都可重放（火花靠幂等键、禁发靠 upsert），操作员重试不会双扣。
pub async fn apply_disposal(
    db: &PgPool,
    torrent_id: i64,
    actor: i64,
    owner_id: Option<i64>,
    d: &Disposal,
) -> DisposalOutcome {
    let mut o = DisposalOutcome::default();
    if !d.deduct_spark && !d.ban_upload {
        return o;
    }
    let Some(owner) = owner_id else {
        o.errors.push("该种无发种人，处置未执行".into());
        return o;
    };
    let base = match spark_earned(db, torrent_id, owner_id).await {
        Ok(v) => v,
        Err(e) => {
            o.errors.push(format!("待回收火花读取失败：{e}"));
            return o;
        }
    };
    if d.deduct_spark {
        o.spark_base = base;
        let amount = if d.double_spark {
            base.checked_mul(2)
        } else {
            Some(base)
        };
        match amount {
            // 基数为 0 不是失败：没有入袋的火花可回收
            None => o.errors.push("双倍扣除数额超出 int64：已跳过".into()),
            Some(a) if a <= 0 => {}
            Some(a) => {
                let idem = format!(
                    "{SPARK_IDEM}:{}:{}:{}",
                    torrent_id,
                    actor,
                    if d.double_spark { "x2" } else { "x1" }
                );
                let r = economy_http::spend_spark(
                    db, owner, a, "admin", &idem, "torrent", torrent_id,
                )
                .await;
                use economy_http::SpendOutcome as SO;
                match r {
                    // 幂等键命中＝没再扣第二次，账面必须写 0
                    Ok(SO::Spent) => o.spark_deducted = a,
                    Ok(SO::Replayed) => o.spark_replayed = true,
                    Err(e) => o.errors.push(format!("火花回收失败：{e}")),
                }
            }
        }
    }
    if d.ban_upload {
        let note = format!(
            "种子 #{torrent_id} 删除处置（{}）",
            d.reason.as_deref().unwrap_or("unspecified")
        );
        let r = authz::set_user_permission(
            db,
            owner,
            authz::perm::TORRENT_UPLOAD,
            Some(false),
            actor,
            Some(&note),
        )
        .await;
        match r {
            Ok(()) => o.upload_banned = true,
            Err(e) => o.errors.push(format!("封禁发种权限失败：{e}")),
        }
    }
    o
}

/// 删除台账：理由/说明/处置账面/失败清单进 `torrent_operation_logs.detail`
/// （jsonb，不加列）。写失败也必须出声——这条是唯一的裁决留痕。
pub async fn write_delete_log(
    db: &PgPool,
    torrent_id: i64,
    actor: i64,
    d: &Disposal,
    o: &DisposalOutcome,
) -> Result<(), String> {
    let detail = serde_json::json!({
        "reason": d.reason,
        "note": d.note,
        "spark_base": o.spark_base,
        "spark_deducted": o.spark_deducted,
        "spark_replayed": o.spark_replayed,
        "double_spark": d.double_spark,
        "ban_upload": d.ban_upload,
        "upload_banned": o.upload_banned,
        "errors": o.errors,
    });
    sqlx::query(
        "INSERT INTO torrent_operation_logs \
         (torrent_id, operator_id, action, detail) \
         VALUES ($1, $2, 'staff_delete', $3)",
    )
    .bind(torrent_id)
    .bind(actor)
    .bind(detail)
    .execute(db)
    .await
    .map(|_| ())
    .map_err(|e| format!("删除台账写入失败：{e}"))
}

/// 处置响应的 data 载荷：删除已成事实，sanctions 里报账面与失败。
pub fn disposal_json(
    torrent_id: i64,
    o: &DisposalOutcome,
) -> serde_json::Value {
    serde_json::json!({
        "deleted": torrent_id,
        "sanctions": {
            "spark_base": o.spark_base,
            "spark_deducted": o.spark_deducted,
        "spark_replayed": o.spark_replayed,
            "upload_banned": o.upload_banned,
            "errors": o.errors,
        },
    })
}
