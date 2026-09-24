//! 种子互动（M03）：评论/感谢/收藏。
//! 从 torrents.rs 按域拆出。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 种子编辑（NP takeedit.php 作者口径）：name/small_descr/descr/anonymous/category/medium/grade/edition
/// 普通作者修改后回退到待审核（approval_status=0），走审核流重新过审；
/// 0170 例外：staff 管理通道（编辑他人）与 ≥ 免审等级的作者本人
/// （upload_auto_approve_class，缺省 92 论坛版主）编辑后保持原审核状态
/// ——「版主以上免审核」同样覆盖编辑口。返回 true = 编辑后未回退待审。
pub struct TorrentEdit<'a> {
    pub name: Option<&'a str>,
    pub small_descr: Option<&'a str>,
    pub descr: Option<&'a str>,
    pub anonymous: Option<bool>,
    pub category_id: Option<i32>,
    pub medium_id: Option<i32>,
    pub grade_id: Option<i32>,
    pub edition_id: Option<i32>,
    /// IMDB id（0150：后台/编辑表单直填；TT+7~8 位数字，空串清空）
    pub imdb_id: Option<&'a str>,
    /// 封面外链（0173 编辑同步发布能力）：None=不动，Some("")=清除，Some(url)=写入
    pub poster: Option<&'a str>,
    /// MediaInfo 全文（0173 编辑同步发布能力）：None=不动，Some("")=清除，Some(text)=写入
    pub mediainfo: Option<&'a str>,
}

pub async fn edit_torrent(
    db: &PgPool,
    torrent_id: i64,
    editor: (i64, i16), // (user_id, class_id)
    e: &TorrentEdit<'_>,
) -> DomainResult<bool> {
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
    // 0159 阈值化：作者本人恒可编辑自己的；管他人的按站点设定
    // torrent_edit_class（缺省 94 管理员）
    if owner_id != editor.0 {
        let threshold = super::manage_perm::edit_threshold(db).await;
        if editor.1 < threshold {
            return Err(DomainError::Forbidden);
        }
    }
    // 0170：编辑他人（staff 管理通道，能走到这里即已过阈值校验）或
    // 作者本人等级 ≥ 免审阈值 → 保持原审核状态；其余回退待审。
    // 被拒（2）种子被高权限编辑时保持 2 —— 状态流转统一走审核台 approve/reject。
    let keep_status = if owner_id != editor.0 {
        true
    } else {
        editor.1 >= super::manage_perm::auto_approve_threshold(db).await
    };
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
            imdb_id = COALESCE($10, imdb_id),
            approval_status = CASE WHEN $11::bool THEN approval_status ELSE 0 END,
            media_info = CASE
                WHEN $12::text IS NULL AND $13::text IS NULL THEN media_info
                ELSE COALESCE(media_info, '{}'::jsonb)
                     || CASE WHEN $12::text IS NULL THEN '{}'::jsonb
                             WHEN $12::text = ''
                                 THEN jsonb_build_object('poster', NULL)
                             ELSE jsonb_build_object('poster', $12::text) END
                     || CASE WHEN $13::text IS NULL THEN '{}'::jsonb
                             WHEN $13::text = ''
                                 THEN jsonb_build_object('mediainfo', NULL)
                             ELSE jsonb_build_object('mediainfo', $13::text) END
            END,
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
    .bind(e.imdb_id)
    .bind(keep_status)
    .bind(e.poster)
    .bind(e.mediainfo)
    .execute(db)
    .await
    .map_err(|err| DomainError::Internal(err.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(torrent_id));
    }
    Ok(keep_status)
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
    let is_owner = owner_id == Some(actor.0);
    // 0159 阈值化：删他人的按站点设定 torrent_delete_class（缺省 94 管理员）；
    // 作者只能删自己未过审（pending/rejected）的种子
    let can_manage = super::manage_perm::delete_threshold(db).await;
    let staff_ok = actor.1 >= can_manage;
    if !staff_ok && !(is_owner && approval != 1) {
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
