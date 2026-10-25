//! 外部条目 ID 归一化（IMDb / TMDB / 豆瓣）——发种时落纯编号，供作品聚合归堆。
//!
//! 为什么是 media_info 而不是新列：这里本来就是发种元数据的地盘
//! （`imdb`/`tmdb` 外链早已在里面，见 ptgen::media_info_of），而作品聚合
//! 本身就是「全表聚合扫描」端点，不需要 ID 索引。等真要 `?tmdb=` 过滤、
//! Radarr 取数或跨站匹配时，再把 `tmdb_id`/`douban_id` 提成列（一次迁移），
//! 读取侧改 `COALESCE(NULLIF(col,''), media_info->>'tmdb_id')` 即可平滑过渡。
//!
//! 口径（与 upload::extract_imdb 同一套「填了就必须解析得出」纪律）：
//!   imdb   tt1234567 / tt12345678（大小写不敏感，统一大写）
//!   tmdb   https://www.themoviedb.org/movie/603 或裸数字 1~8 位
//!   douban https://movie.douban.com/subject/1292052 或裸数字 5~10 位
//!
//! 裸数字只在自己那一栏里认（TMDB 栏填 "603" 就是 TMDB），不跨栏猜——
//! 豆瓣与 TMDB 的编号区间重叠，猜错的代价是把两部不相干的片合成一部作品。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

use super::ptgen::UploadForm;

/// 发种前校验：三栏填了却解析不出编号 ⇒ 当场 1002，别静默收下。
pub(super) fn validate(form: &UploadForm) -> DomainResult<()> {
    let imdb = raw(&form.imdb);
    if !imdb.is_empty() && super::extract_imdb_pub(imdb).is_none() {
        return Err(DomainError::Validation(super::IMDB_HINT.into()));
    }
    let tmdb = raw(&form.tmdb);
    if !tmdb.is_empty() && extract_tmdb(tmdb).is_none() {
        return Err(DomainError::Validation(TMDB_HINT.into()));
    }
    let douban = raw(&form.douban);
    if !douban.is_empty() && extract_douban(douban).is_none() {
        return Err(DomainError::Validation(DOUBAN_HINT.into()));
    }
    Ok(())
}

/// 与发种校验同一句提示（前端也有一份同文案的字典，两处别漂）。
pub(super) const TMDB_HINT: &str =
    "TMDB 编号格式无效：需 1~8 位数字，或直接贴 themoviedb.org 条目链接";
pub(super) const DOUBAN_HINT: &str =
    "豆瓣编号格式无效：需 5~10 位数字，或直接贴 movie.douban.com/subject/ 链接";

/// 取字段原文（去空白；None → ""）
fn raw(v: &Option<String>) -> &str {
    v.as_deref().map(str::trim).unwrap_or("")
}

fn extract_tmdb(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?i)themoviedb\.org/(?:movie|tv)/(\d{1,8})")
        .ok()?;
    if let Some(c) = re.captures(text) {
        return Some(c[1].to_string());
    }
    let t = text.trim();
    if t.len() <= 8 && !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) {
        return Some(t.to_string());
    }
    None
}

fn extract_douban(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"douban\.com/subject/(\d{5,10})").ok()?;
    if let Some(c) = re.captures(text) {
        return Some(c[1].to_string());
    }
    let t = text.trim();
    if (5..=10).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_digit()) {
        return Some(t.to_string());
    }
    None
}

/// 落库（发种/复活两路共用一次）：只写**能解析出的**键，不覆盖已有值。
///
/// `form` 三栏优先，其次从正文/片名里找（老用户的习惯：豆瓣链接直接粘在简介）。
/// 用 `||` 顶层合并而不是 `jsonb_set`：中间层不存在时 jsonb_set 会静默不建，
/// 顶层合并没有这个坑（0328 事故的教训）。
pub(super) async fn persist(
    db: &PgPool,
    id: i64,
    form: &UploadForm,
    name: &str,
) -> DomainResult<()> {
    let descr = form.descr.as_deref().unwrap_or("");
    let imdb = form
        .imdb
        .as_deref()
        .and_then(super::extract_imdb_pub)
        .or_else(|| super::extract_imdb_pub(descr))
        .or_else(|| super::extract_imdb_pub(name));
    let tmdb = form
        .tmdb
        .as_deref()
        .and_then(extract_tmdb)
        .or_else(|| extract_tmdb(descr));
    // TMDB 条目有 movie/tv 两条路径：编号本身分不出来，能拿到原始链接就原样
    // 存（作品页直接用它跳转）；只给裸编号时按 movie 落——填链接才有精确跳转
    let tmdb_url = form
        .tmdb
        .as_deref()
        .map(str::trim)
        .filter(|t| t.starts_with("http"))
        .map(str::to_string)
        .or_else(|| {
            tmdb.as_ref()
                .map(|id| format!("https://www.themoviedb.org/movie/{id}"))
        });
    let douban = form
        .douban
        .as_deref()
        .and_then(extract_douban)
        .or_else(|| extract_douban(descr))
        .or_else(|| extract_douban(name));
    // 一个都没解析出来：不必白跑一次 UPDATE
    if imdb.is_none() && tmdb.is_none() && douban.is_none() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE torrents SET \
           imdb_id = COALESCE(imdb_id, $2), \
           media_info = COALESCE(media_info, '{}'::jsonb) || \
             jsonb_strip_nulls( \
             jsonb_build_object('imdb_id', $2::text, \
                                'tmdb_id', $3::text, \
                                'tmdb_url', $5::text, \
                                'douban_id', $4::text)) \
         WHERE id = $1",
    )
    .bind(id)
    .bind(&imdb)
    .bind(&tmdb)
    .bind(&douban)
    .bind(&tmdb_url)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 发种后的异步补全（MoviePilot 口径：有 ID 就回查，元数据是识别+缓存出来的）。
/// 失败只记日志：种子已入库，元数据可以后补（auto-meta 端点可手动重试）。
pub(super) fn maybe_enrich(
    state: std::sync::Arc<crate::state::AppState>,
    id: i64,
) {
    actix_web::rt::spawn(async move {
        if let Err(e) = super::auto_meta::enrich_by_any_id(&state, id).await {
            tracing::warn!("auto-enrich torrent {id}: {e}");
        }
    });
}
