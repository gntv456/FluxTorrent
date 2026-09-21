//! 种子互动（M03）：评论/感谢/收藏。
//! 从 torrents.rs 按域拆出。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 种子编辑（NP takeedit.php 作者口径）：name/small_descr/descr/anonymous/category/medium/grade/edition
/// 修改后回退到待审核（approval_status=0），走审核流重新过审。
pub struct TorrentEdit<'a> {
    pub name: Option<&'a str>,
    pub small_descr: Option<&'a str>,
    pub descr: Option<&'a str>,
    pub anonymous: Option<bool>,
    pub category_id: Option<i32>,
    pub medium_id: Option<i32>,
    pub grade_id: Option<i32>,
    pub edition_id: Option<i32>,
}

pub async fn edit_torrent(
    db: &PgPool,
    torrent_id: i64,
    editor: (i64, i16), // (user_id, class_id)：作者本人或 staff（>=90）
    e: &TorrentEdit<'_>,
) -> DomainResult<()> {
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(torrent_id)
            .fetch_optional(db)
            .await
            .map_err(|err| DomainError::Internal(err.into()))?
            .flatten();
    let Some(owner_id) = owner else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if editor.1 < 90 && owner_id != editor.0 {
        return Err(DomainError::Forbidden);
    }
    let n = sqlx::query(
        r#"
        UPDATE torrents SET
            name = COALESCE($2, name),
            small_descr = COALESCE($3, small_descr),
            descr = COALESCE($4, descr),
            anonymous = COALESCE($5, anonymous),
            category_id = COALESCE($6, category_id),
            medium_id = COALESCE($7, medium_id),
            grade_id = COALESCE($8, grade_id),
            edition_id = COALESCE($9, edition_id),
            approval_status = 0,
            mtime = now()
        WHERE id = $1
        "#,
    )
    .bind(torrent_id)
    .bind(e.name)
    .bind(e.small_descr)
    .bind(e.descr)
    .bind(e.anonymous)
    .bind(e.category_id)
    .bind(e.medium_id)
    .bind(e.grade_id)
    .bind(e.edition_id)
    .execute(db)
    .await
    .map_err(|err| DomainError::Internal(err.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(torrent_id));
    }
    Ok(())
}

/// 种子软删除（NP delete.php 口径）：staff 或作者本人（未过审的可直接删；已过审的作者删除需 staff）
pub async fn delete_torrent(
    db: &PgPool,
    torrent_id: i64,
    actor: (i64, i16),
) -> DomainResult<()> {
    let row: Option<(Option<i64>, i16)> = sqlx::query_as(
        "SELECT owner_id, approval_status FROM torrents WHERE id = $1",
    )
    .bind(torrent_id)
    .fetch_optional(db)
    .await
    .map_err(|err| DomainError::Internal(err.into()))?;
    let Some((owner_id, approval)) = row else {
        return Err(DomainError::NotFound(torrent_id));
    };
    let is_staff = actor.1 >= 90;
    let is_owner = owner_id == Some(actor.0);
    // staff 任意删；作者只能删自己未过审（pending/rejected）的种子
    if !is_staff && !(is_owner && approval != 1) {
        return Err(DomainError::Forbidden);
    }
    // 软删 + 清理关联促销（审计修复：促销残留会被计费/H&R 豁免回查误命中）
    let mut tx = db
        .begin()
        .await
        .map_err(|err| DomainError::Internal(err.into()))?;
    sqlx::query(
        "UPDATE torrents SET approval_status = 3, mtime = now() WHERE id = $1",
    )
    .bind(torrent_id)
    .execute(&mut *tx)
    .await
    .map_err(|err| DomainError::Internal(err.into()))?;
    sqlx::query("DELETE FROM promotions WHERE torrent_id = $1")
        .bind(torrent_id)
        .execute(&mut *tx)
        .await
        .map_err(|err| DomainError::Internal(err.into()))?;
    tx.commit()
        .await
        .map_err(|err| DomainError::Internal(err.into()))?;
    Ok(())
}

/// 恢复软删种子（approval_status 3 → 0 待审）：此前误删后只能直连数据库手工修数。
pub async fn restore_torrent(db: &PgPool, torrent_id: i64) -> DomainResult<()> {
    let n = sqlx::query(
        "UPDATE torrents SET approval_status = 0, \
         mtime = now() WHERE id = $1 AND approval_status = 3",
    )
    .bind(torrent_id)
    .execute(db)
    .await
    .map_err(|err| DomainError::Internal(err.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(torrent_id));
    }
    Ok(())
}
