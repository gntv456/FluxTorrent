//! 发种（M04）：POST /torrents 主上传链路。
//! 从 publish_http.rs 按域拆出；NFO 解码助手在 ptgen.rs。
use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{require_auth, AuthUser};
use crate::state::AppState;

use super::descr_image::descr_images;
use super::ptgen::{build_media_info, decode_nfo};
use super::{
    upload_credits as credits, upload_precheck as precheck, upload_revive,
};

/// 发种结果。Token 化发种（`/open/torrents`）要把「重复」当成功返回（幂等：
/// 工具重试同一 .torrent 不该报错），网页发种必须报 3004 让用户看到——
/// 故核心层返回枚举，由两个入口各自翻译。
pub(crate) enum UploadOutcome {
    Created(serde_json::Value),
    Duplicate {
        id: Option<i64>,
        info_hash: String,
        pieces_hash: String,
    },
}
#[post("/torrents")]
pub async fn upload(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    payload: actix_multipart::Multipart,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    match upload_core(&state, &auth, req.query_string(), payload).await? {
        UploadOutcome::Created(v) => Ok(ok(v)),
        UploadOutcome::Duplicate { .. } => Err(DomainError::TorrentDuplicate),
    }
}

/// Token 化发种入口（0267，`POST /open/torrents`）：与网页上传走同一条
/// 核心链路（同一套过审/扣费/权限/幂等规则），差别只有两处：
/// ①鉴权来自 API Token（外部合成 AuthUser）；
/// ②元数据来自原始 query string（无需把 UploadForm 提升为 crate 可见）。
pub(crate) async fn upload_token(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &AuthUser,
    query_string: &str,
    payload: actix_multipart::Multipart,
) -> DomainResult<UploadOutcome> {
    upload_core(state, auth, query_string, payload).await
}

/// 发种核心：网页表单与开放 API 共用。调用方负责鉴权与表单来源。
async fn upload_core(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &AuthUser,
    query_string: &str,
    mut payload: actix_multipart::Multipart,
) -> DomainResult<UploadOutcome> {
    // 发种基础权限（默认配给全体用户 class 1；可用于限制上传资格）
    crate::authz::require_perm(state, auth, crate::authz::perm::TORRENT_UPLOAD)
        .await?;
    // 0388 阶梯第 3 档起禁发新种：worker 把 block_upload 写进
    // hr_warn_states，但此前全仓没有读点——「禁传」只是库里的装饰。
    // 与禁下同一口径按当前态执行，到期由 worker restore_due 清位。
    if crate::repo::hr::warn_blocks(&state.repo, auth.id).await?.0 {
        return Err(DomainError::Validation(
            "H&R 警告已达禁传档：暂不能发布新种，警告到期且违规处理完毕后自动恢复"
                .into(),
        ));
    }
    // 体积上限、multipart 读取与元数据装配都在 upload_body.rs
    let body =
        super::upload_body::read_body(&state.repo.db, &mut payload).await?;
    // 更新链挂链（0323）：parent_torrent_id 是自由字段，不进 UploadForm
    let mp_artifact_parent: Option<String> = body
        .fields
        .iter()
        .find(|(k, _)| k == "parent_torrent_id")
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| !v.is_empty());
    // 对阵挂链（0330 sports）：match_id 自由字段
    let mp_match_id: Option<i64> = body
        .fields
        .iter()
        .find(|(k, _)| k == "match_id")
        .and_then(|(_, v)| v.trim().parse().ok());
    let form = super::upload_fields::build_form(query_string, body.fields)?;
    let parsed = crate::bencode::parse_torrent(&body.torrent)
        .map_err(DomainError::TorrentInvalid)?;
    // 结构校验（P0-6 实测：无 `pieces`、非 20 倍数、piece length=0、总大小 0、
    // 空标题、`..\..\` 路径穿越等畸形 .torrent 全部 200 入库；这类种子在客户端
    // 必然校验失败，变成 0 做种死种，工单全回审核员面前）
    crate::bencode::validate_for_upload(&parsed)
        .map_err(DomainError::TorrentInvalid)?;
    // 站外取源键（HTTP seed / DHT 提示）入库这一侧直接拒收；下载侧另有剥离
    crate::bencode::reject_off_tracker_sources(&body.torrent)
        .map_err(DomainError::TorrentInvalid)?;
    // 重复检测（M04：info_hash 唯一）。同一作者的墓碑（status=3）不算重复，
    // 走复活路径——否则「删了再发」被唯一索引永久锁死（P0-4 实测）。
    let existing: Option<(i64, Option<i64>, i16)> = sqlx::query_as(
        "SELECT id, owner_id, approval_status FROM torrents \
         WHERE info_hash = $1 OR raw_info_hash = $2 ORDER BY id LIMIT 1",
    )
    .bind(&parsed.info_hash_hex)
    .bind(&parsed.raw_info_hash_hex)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let revive_id: Option<i64> = match existing {
        Some((id, owner, status)) if status == 3 && owner == Some(auth.id) => {
            Some(id)
        }
        // 幂等：把已存在那一枚的 id 一并回给调用方（Token 发种据此判重）
        Some((id, _, _)) => {
            return Ok(UploadOutcome::Duplicate {
                id: Some(id),
                info_hash: parsed.info_hash_hex.clone(),
                pieces_hash: parsed.pieces_hash_hex.clone(),
            });
        }
        None => None,
    };
    // 异主墓碑（P2-7）：内容仍被软删锁着，回「种子重复」等于死路——落一条
    // staff 待办（reports 队列），站务恢复后即可重发；(ref_type, ref_id,
    // reporter_id) 开件唯一约束保证不刷屏。
    if let Some((id, owner, status)) = &existing {
        if *status == 3 && *owner != Some(auth.id) {
            let _ = sqlx::query(
                "INSERT INTO reports (reporter_id, ref_type, ref_id, reason) \
                 SELECT $1, 'tombstone_reupload', $2, \
                 '异主墓碑重发：内容被他人删除后同 hash 重发被拦，待站务恢复' \
                 WHERE NOT EXISTS (SELECT 1 FROM reports \
                 WHERE ref_type = 'tombstone_reupload' AND ref_id = $2 \
                   AND status IN (0, 2))",
            )
            .bind(auth.id)
            .bind(id)
            .execute(&state.repo.db)
            .await;
        }
    }
    let name = form
        .name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or(parsed.name.clone());
    // 封面外链 + IMDb 链接 → media_info（JSONB 键合并；搜索区 4 按 imdb 键命中）
    let media_info: Option<serde_json::Value> = build_media_info(&form);
    // 命中「自动过审」分类同样免审（categories.auto_approve）
    let cat_auto: bool = sqlx::query_scalar(
        "SELECT COALESCE(bool_or(auto_approve), FALSE) FROM categories \
         WHERE id = $1",
    )
    .bind(form.category_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    // 0077 被拒禁发：累计被拒达阈值直接拦（键统一读设置页的
    // upload_deny_approval_deny_count——P1-2 前 old 键两不相认恒不生效）。
    // 0355：顺带取信用分
    let (deny_count, streak, credits): (i32, i32, i32) = sqlx::query_as(
        "SELECT deny_count, approve_streak, publish_credits \
         FROM users WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let deny_limit: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE \
         name = 'upload_deny_approval_deny_count'), 2)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(2);
    if deny_count >= deny_limit {
        return Err(DomainError::Validation(
            "因多次发布被拒，上传资格已暂停；请先通过『联系我们』申诉".into(),
        ));
    }
    // 0077 免审（NP offer_skip_approved_count）：连续**人工过审** ≥5 免审。
    // P1-8：连击只由审核员累加，免审通道不给自己加分（防自激）。
    let streak_skip = streak >= precheck::STREAK_SKIP_MIN;
    // 0170 等级免审：class ≥ upload_auto_approve_class（缺省 92 论坛版主）
    let auto_class =
        crate::torrents::manage_perm::auto_approve_threshold(&state.repo.db)
            .await;
    // 0355 信用分免审（第五条）：credits ≥ threshold（缺省 5，累积规则见
    // 迁移 0355 注释），与连击并列独立——被拒打断连击后信用分仍有残留
    let credit_threshold: i32 =
        credits::credits_threshold(&state.repo.db).await;
    let credits_skip = credits >= credit_threshold;
    let auto_approve = cat_auto
        || streak_skip
        || credits_skip
        || auth.class_id >= i32::from(auto_class)
        || crate::authz::can(
            state,
            auth,
            crate::authz::perm::TORRENT_APPROVAL_AUTO,
        )
        .await;
    // 免审判定审计（0355 顺手补）：命中维度随 info 日志留痕（可 grep 聚合）
    if auto_approve {
        let class_hit = auth.class_id >= i32::from(auto_class);
        let dims =
            credits::audit_dims(cat_auto, streak_skip, credits_skip, class_hit);
        tracing::info!(user = auth.id, dims, credits, "发种免审命中");
    }
    let approval_status: i16 = if auto_approve { 1 } else { 0 };
    // 聚合组（0069）：显式传入的 group_id 必须存在（防悬挂引用）
    if let Some(gid) = form.group_id {
        let g: Option<i64> =
            sqlx::query_scalar("SELECT id FROM torrent_groups WHERE id = $1")
                .bind(gid)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        if g.is_none() {
            return Err(DomainError::Validation("聚合组不存在".into()));
        }
    }
    let nfo_text: Option<String> = body
        .nfo
        .as_deref()
        .map(decode_nfo)
        .filter(|s| !s.trim().is_empty());
    // ---- 以下全部在 INSERT 之前（P0-4：过去 tags / 推荐位 / 价格越界都在入库后
    //      才报错，留下待审残种 + 同一 .torrent 永久「种子重复」）----
    precheck::check_lengths(&form)?;
    let price = precheck::check_price(&state.repo.db, form.price).await?;
    precheck::check_tags(&state.repo.db, &form, auth).await?;
    let promo = precheck::check_promo(&form, auth)?;
    // 0148 C1：IMDb 编号落 `imdb_id` 列（详情页/编辑页/审核台/`?imdb=` 并集
    // 都以它为准）；三栏（IMDb/TMDB/豆瓣）填了却解析不出编号 ⇒ 当场 1002。
    let imdb_id: Option<String> = form
        .imdb
        .as_deref()
        .and_then(extract_imdb)
        .or_else(|| form.descr.as_deref().and_then(extract_imdb))
        .or_else(|| extract_imdb(&name));
    super::upload_ids::validate(&form)?;
    // 简介里的图 → screenshots 列（该列过去全仓无写入，审核队列的「截图数」恒 0）
    let shots = descr_images(form.descr.as_deref());
    precheck::check_quality(&state.repo.db, &form, &parsed, shots.len())
        .await?;
    let rip = super::upload_logcheck::run(&body.logcheck, &body.logs)?;
    // sections 先整体校验再落种子：原先校验在 INSERT 之后、且与写入交织，
    // 报错时种子已入库，重试同一 .torrent 永远撞 TorrentDuplicate
    super::upload_sections::parse_sections(
        &state.repo.db,
        form.sections.as_ref(),
    )
    .await?;
    // 重复发布策略（suggest/block/group）：pieces_hash 命中同内容时的处置
    let dup_group: Option<i64> =
        match precheck::dup_policy(&state.repo.db, &parsed.pieces_hash_hex)
            .await?
        {
            precheck::DupVerdict::AutoGroup(gid) => Some(gid),
            _ => None,
        };
    let group_id = form.group_id.or(dup_group);
    // 复活路径（同一作者删过的同内容）：复用原 id，内容整体覆盖，重回审核流
    let id: i64 = match revive_id {
        Some(old) => {
            upload_revive::revive_tombstone(
                &state.repo.db,
                old,
                &form,
                &name,
                &parsed,
                &media_info,
                &nfo_text,
                price,
                &imdb_id,
                group_id,
                approval_status,
                &shots,
            )
            .await?
        }
        None => {
            let insert_res: Result<i64, sqlx::Error> = sqlx::query_scalar(
                "INSERT INTO torrents (info_hash, raw_info_hash, pieces_hash, \
                 group_id, name, small_descr, descr, category_id, medium_id, \
                 grade_id, edition_id, owner_id, anonymous, size, numfiles, \
                 approval_status, approved_at, media_info, nfo, price, imdb_id, \
                 screenshots) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, \
                 $13, $14, $15, $16, CASE WHEN $16::smallint = 1 THEN now() ELSE NULL END, \
                 $17, $18, $19, $20, \
                 $21::jsonb) RETURNING id",
            )
            .bind(&parsed.info_hash_hex)
            .bind(&parsed.raw_info_hash_hex)
            .bind(&parsed.pieces_hash_hex)
            .bind(group_id)
            .bind(&name)
            .bind(&form.small_descr)
            .bind(&form.descr)
            .bind(form.category_id)
            .bind(form.medium_id)
            .bind(form.grade_id)
            .bind(form.edition_id)
            .bind(auth.id)
            .bind(form.anonymous)
            .bind(parsed.size)
            .bind(parsed.numfiles)
            .bind(approval_status)
            .bind(media_info)
            .bind(nfo_text)
            .bind(price)
            .bind(imdb_id)
            .bind(serde_json::json!(shots))
            .fetch_one(&state.repo.db)
            .await;
            match insert_res {
                Ok(v) => v,
                Err(e) => {
                    // 并发上传同一 .torrent：判重与 INSERT 之间的窗口由唯一约束
                    // 兜底，映射为语义化重复结果而非裸 500（0081：raw_info_hash
                    // 只有普通索引）
                    if e.to_string().contains("torrents_info_hash_key")
                        || e.to_string().contains("duplicate key")
                    {
                        let existing: Option<i64> = sqlx::query_scalar(
                            "SELECT id FROM torrents WHERE info_hash = $1 \
                             ORDER BY id LIMIT 1",
                        )
                        .bind(&parsed.info_hash_hex)
                        .fetch_optional(&state.repo.db)
                        .await
                        .unwrap_or(None);
                        return Ok(UploadOutcome::Duplicate {
                            id: existing,
                            info_hash: parsed.info_hash_hex.clone(),
                            pieces_hash: parsed.pieces_hash_hex.clone(),
                        });
                    }
                    // 分类/媒介/学段/版本不存在 → 外键违规，输入问题非服务器故障
                    return Err(crate::errors::db_to_domain(
                        e,
                        "分类/媒介/学段/版本",
                    ));
                }
            }
        }
    };
    // 外部条目 ID 归一化（imdb_id/tmdb_id/douban_id 落 media_info）：
    // 作品聚合据此把同片多版本归一部作品，见 torrent_http/works_sql.rs
    super::upload_ids::persist(&state.repo.db, id, &form, &name).await?;
    // MoviePilot 口径：有 ID 就回查补全元数据（异步，不阻塞发种回包）
    super::upload_ids::maybe_enrich(std::sync::Arc::clone(state.get_ref()), id);
    // P-346：发布带 MediaInfo 跑一致性判定（与编辑口同口径：提示不闸门，
    // 解析失败不拦发布回包）。
    if let Some(t) = form
        .mediainfo
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let _ = crate::mediainfo_service::check_and_store(
            &state,
            id,
            &name,
            Some(t),
        )
        .await;
    }
    // 0374 outbox：种子行与 uploaded 事件同事务落库（事务外 fire-and-forget
    // 会丢事件、出幽灵置顶）。dedupe_key=info_hash：并发发同一种只留一条
    // 未消费事件；幂等索引只覆盖未消费行，删种复活后重发不会被历史行挡住。
    {
        let mut tx_up = state
            .repo
            .db
            .begin()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let payload = serde_json::json!({
            "torrent_id": id,
            "owner_id": auth.id,
            "name": name,
            "approval_status": approval_status,
            "group_id": group_id,
            "info_hash": parsed.info_hash_hex,
        });
        let up_key = format!("torrent.uploaded:{}", parsed.info_hash_hex);
        crate::domain_events::emit_typed(
            &mut tx_up,
            crate::domain_events::events::TORRENT_UPLOADED,
            Some("torrent"),
            Some(id),
            &payload,
            Some(&up_key),
        )
        .await?;
        tx_up
            .commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    // 多维属性（第八轮 Section）与标签：外提至 sections_store::store_sections_tags
    // （标签已在 INSERT 前按同一判据校验过，这里只会因写库失败而错）
    super::upload_sections::store_sections_tags(state, &form, auth, id).await?;
    // 新种子进列表：推进列表缓存代际，否则首屏 45s 内看不到刚发的种
    let app: &AppState = state;
    crate::torrent_http::bump_list_cache_gen(app).await;
    // 免审种（approval_status=1）即刻在 tracker 白名单内，待审种本就装载；
    // 这里统一 bump 一次，免审种不必等下一个 60s 周期才能做种（P2-2）
    crate::http::bump_guard_ver(state.get_ref()).await;
    // 存原始 .torrent 字节（下载时重新注入 announce，M05）
    sqlx::query("INSERT INTO torrent_files (torrent_id, raw) VALUES ($1, $2)")
        .bind(id)
        .bind(&parsed.raw)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 文件清单/自动促销/推荐位：外提至 upload_files_promo::store_files_promo
    // （推荐位的权限与取值已在 INSERT 前校验，这里只负责写）
    super::upload_files_promo::store_files_promo(
        state, &parsed, id, auth, promo,
    )
    .await?;
    super::upload_logcheck::store(&state.repo.db, id, &rip).await?;
    // 判分维度回流（0314 补全）：log_score/haslog 从权威 torrent_logs 自动
    // 同步进筛选维度。降级不阻断：种子已入库，500 会让重发撞「种子重复」
    if let Err(e) = crate::torrents::sync_log_dims(&state.repo.db, id).await {
        tracing::warn!(error = ?e, torrent = id, "判分维度回流失败（不阻断发种）");
    }
    // 对阵挂链（0330）：match_id → torrents.match_id；悬空 id 静默降级
    if let Some(mid) = mp_match_id {
        let _ = sqlx::query(
            "UPDATE torrents SET match_id = $2 WHERE id = $1 AND EXISTS              (SELECT 1 FROM sport_matches WHERE id = $2)",
        )
        .bind(id)
        .bind(mid)
        .execute(&state.repo.db)
        .await;
    }
    // 种子工件（0323）：校验和/更新日志落库；更新链 parent 走表单字段
    // parent_torrent_id（上面在 build_form 消费前抓出）
    let artifact_parent: Option<i64> =
        mp_artifact_parent.as_deref().and_then(|v| v.parse().ok());
    super::upload_artifacts::store(
        &state.repo.db,
        id,
        artifact_parent,
        &body.artifacts,
    )
    .await?;
    // M28 插件 Hook：发布成功后分发（异步、失败不影响主流程）
    state.plugins.dispatch_upload(state.get_ref(), id, auth.id);
    // 发种奖励（P-4）：免审种此刻就是「发布完成」，与人工过审同一函数
    // 发放（幂等键锚种子，见 review_side_effects::pay_upload_reward）；
    // 待审种的奖励在 review_decide 过审时发。失败只 warn（种子已入库）。
    if auto_approve {
        crate::admin_http::review_side_effects::pay_upload_reward(
            &state.repo.db,
            id,
        )
        .await;
    }
    // 聚合组推荐（同名候选 / 同源命中）：外提至 upload_suggest.rs
    let group_suggest = super::upload_suggest::group_suggest(
        &state.repo.db,
        &parsed.pieces_hash_hex,
        &name,
        id,
        group_id,
    )
    .await;
    Ok(UploadOutcome::Created(serde_json::json!({
        "id": id,
        "approval_status": approval_status,
        "auto_approved": auto_approve,
        "group_suggest": group_suggest,
        "pieces_hash": parsed.pieces_hash_hex,
        "info_hash": parsed.info_hash_hex,
    })))
}

/// descr/名称里的 IMDB id 提取（0148 C1）：tt1234567 / tt12345678，
/// 大小写不敏感；统一大写存储（TT…）。manage.rs 编辑 descr 后复用。
pub(crate) fn extract_imdb_pub(text: &str) -> Option<String> {
    extract_imdb(text)
}
/// IMDb 输入校验提示（发种与编辑两个入口共用同一句，防两处规则漂移）。
pub(crate) const IMDB_HINT: &str =
    "IMDb 编号格式无效：需 tt 加 7~8 位数字，或直接贴 IMDb 链接";
fn extract_imdb(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?i)\b(tt[0-9]{7,8})\b").ok()?;
    re.captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_ascii_uppercase())
}
/// 简介首图提取的 crate 出口（manage.rs 编辑 descr 后同步回落 poster 用）。
pub(crate) fn first_descr_image_pub(descr: Option<&str>) -> Option<String> {
    super::descr_image::first_descr_image(descr)
}
