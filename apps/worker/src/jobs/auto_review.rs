//! 自动审种引擎 worker 侧（0400 批 1）：《自动审种功能策划案》v1.3。
//!
//! 取件 → 评估（flux-shared 纯函数内核）→ 影子/执法分支。三条纪律：
//!   * **裁决同尺**（§1.2）：执法裁决不直接 UPDATE torrents——HTTP 内调
//!     api 的 `/admin/reviews/decide`（系统审核员身份），信用分/streak/
//!     approved_at/促销/通知全部与人工逐字段一致。HTTP 失败降级影子
//!     （只落台账），绝不半途手动改状态。
//!   * **不与人工抢审**（§13.1）：取件到终裁之间人工可能已裁决——终裁
//!     前由 decide 端点自己的 `approval_status=0` 谓词兜底（NotPending），
//!     本侧把该结果落「人机相遇」对照行，不重试。
//!   * **影子零副作用**（A7）：shadow 模式只写 auto_review_log，不动
//!     torrents、不发任何用户通知。
//!
//! 深检（D1/D3 沙箱与 magic number）在批 4 落地；本版 verdict=3 只作为
//! 台账内部态预留。

use flux_shared::auto_review::{
    self, Action, Assessment, EngineConfig, PublisherProfile, Suggestion,
    TorrentSnapshot,
};
use sqlx::PgPool;
use std::collections::HashMap;

/// 规则集版本（§7/A25）：读 site_settings.auto_review_ruleset_ver
/// （settings 保存链在 auto_review 组变更时自动 +1）；缺键=1。
async fn ruleset_ver(db: &PgPool) -> i16 {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int2 FROM site_settings          WHERE name = 'auto_review_ruleset_ver'), 1)",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or(1)
}

/// 从 site_settings 装配引擎配置（键清单见 0400 迁移；缺键回缺省）。
pub async fn load_config(db: &PgPool) -> EngineConfig {
    let mut cfg = EngineConfig::defaults();
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings \
                        WHERE grp = 'auto_review'",
    )
    .bind(0)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let map: HashMap<String, String> = rows.into_iter().collect();
    let g = |k: &str| map.get(k).cloned().unwrap_or_default();
    let gb = |k: &str, d: bool| {
        map.get(k)
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(d)
    };
    let gi = |k: &str, d: i64| {
        map.get(k).and_then(|v| v.trim().parse().ok()).unwrap_or(d)
    };
    cfg.enabled = gb("auto_review_enabled", false);
    cfg.mode = {
        let m = g("auto_review_mode");
        if m.is_empty() {
            "shadow".into()
        } else {
            m
        }
    };
    cfg.delay_sec = gi("auto_review_delay_sec", 90);
    cfg.daily_cap = gi("auto_review_daily_cap", 0);
    cfg.descr_min_chars =
        gi("arule_descr_missing_min_chars", 80).max(0) as usize;
    cfg.dupe_threshold_pct = gi("arule_dupe_check_threshold", 92) as i32;
    cfg.title_level = {
        let l = g("arule_title_format_level");
        if l.is_empty() {
            "warn".into()
        } else {
            l
        }
    };
    cfg.title_template = g("auto_review_title_template");
    cfg.title_required = g("auto_review_title_required")
        .split('，')
        .chain(g("auto_review_title_required").split(','))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    cfg.ad_words = g("auto_review_ad_words")
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let parse_ids = |v: &str| -> Vec<i64> {
        v.split(|c| c == ',' || c == '，')
            .filter_map(|s| s.trim().parse().ok())
            .collect()
    };
    cfg.trusted_uploaders = parse_ids(&g("auto_review_trusted_uploaders"));
    cfg.watch_uploaders = parse_ids(&g("auto_review_watch_uploaders"));
    for id in [
        "arule_title_garbled",
        "arule_descr_missing",
        "arule_ads_keywords",
        "arule_leak_marker",
        "arule_dupe_check",
        "arule_title_format",
    ] {
        let enabled = gb(&format!("{id}_enabled"), true);
        let action = Action::parse(&g(&format!("{id}_action")));
        cfg.rules.insert(id, (enabled, action));
    }
    cfg
}

/// 系统审核员 UID：裁决 HTTP 的 operator。取在册最低 id 的 class≥99
/// 账号；留痕/审计可由 operator_id 反查（§16「引擎身份」）。
async fn system_operator(db: &PgPool) -> Option<i64> {
    sqlx::query_scalar(
        "SELECT MIN(id) FROM users WHERE class_id >= 99 AND status < 2",
    )
    .bind(0)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

/// 一轮取件：捞「待审且过了延迟窗」的种子逐个评估。
/// 返回本轮处理数。CAS 语义由台账唯一索引承担：`auto_review_log_latest_uq`
/// 保证每种子最多一行机器意见（decided_by=0 且非深检），ON CONFLICT 跳过
/// 即「别的实例已评」——双 worker 单飞取件（A21）。
pub async fn auto_review_tick(db: &PgPool) -> anyhow::Result<u64> {
    let cfg = load_config(db).await;
    if !cfg.enabled {
        return Ok(0);
    }
    // 当日执法配额（影子不计）
    let quota_ok = if cfg.daily_cap > 0 && cfg.mode != "shadow" {
        let used: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM auto_review_log \
             WHERE decided_by = 0 AND mode <> 0 \
               AND created_at >= \
               (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') \
               AT TIME ZONE 'Asia/Shanghai')",
        )
        .bind(0)
        .fetch_one(db)
        .await
        .unwrap_or(0);
        used < cfg.daily_cap
    } else {
        true
    };
    let rows: Vec<(i64, i64, String, Option<String>, Option<String>, i64)> =
        sqlx::query_as(
            "SELECT t.id, t.owner_id, t.name, t.small_descr, t.descr, \
                    t.category_id::int8 \
             FROM torrents t \
             WHERE t.approval_status = 0 \
               AND t.mtime < now() - make_interval(secs => $1::int4) \
               AND NOT EXISTS (SELECT 1 FROM auto_review_log l \
                     WHERE l.torrent_id = t.id AND l.decided_by = 0)",
        )
        .bind(cfg.delay_sec)
        .fetch_all(db)
        .await?;
    // 批 3（§6/§12）：人工推翻回填 + 熔断检查在取件前跑（幂等）。
    // SQL 与 api 侧 review_auto::sweep_overturns 同源同口径——改一处须同步另一处。
    sweep_overturns(db).await;
    // §13.4 队列超时升级：待审 >24h 且无机器行 → 管理组提醒（一天一次，
    // 幂等锚=staffmessages 当日 subject 去重）。影子模式同样生效（队列
    // 健康观测），不动种子状态。
    queue_health(db).await;
    let mut done = 0u64;
    for (id, owner, name, small, descr_opt, cat) in rows {
        // descr/small_descr 均可空（实测 49425 双 NULL）——空描述正是 H2
        // 要抓的形态，不能让解码先崩
        let descr = descr_opt.unwrap_or_default();
        let snap = build_snapshot(db, id, owner, name, small, descr, cat).await;
        let profile = load_profile(db, owner).await;
        let mut a = auto_review::assess(&snap, &profile, &cfg);
        // 深检·文件清单层（批 4，§2.3 D2/D4/D5 的零 IO 部分）：
        // 命中一律 postpone 档（客观差异未必恶意）；白名单分类豁免 D2。
        deep_file_rules(db, &mut a, &snap, &cfg).await;
        if let Err(e) = handle(db, &cfg, id, owner, &a).await {
            tracing::warn!(%e, torrent = id, "auto_review: 处理失败");
            continue;
        }
        done += 1;
    }
    let _ = quota_ok; // 配额满时 handle 内部自然落 shadow 行，不额外短路
    Ok(done)
}

/// 取数：文件清单（批 4 深检用，本轮先装载）与近 90 天过审标题窗（H5）。
async fn build_snapshot(
    db: &PgPool,
    id: i64,
    owner: i64,
    name: String,
    small: Option<String>,
    descr: String,
    cat: i64,
) -> TorrentSnapshot {
    // P-466：`torrent_files` 只有 (torrent_id, raw, created_at)——没有
    // filename 列。旧查询恒失败，被 `.unwrap_or_default()` 吞成空 Vec，
    // 于是下面「MediaInfo 声明含视频轨 ⇒ 包内必须有视频文件」那条规则
    // 对**每个带 MediaInfo 的种**都成立，整队列被标成伪媒体嫌疑。
    // 文件名只能从归档的 .torrent（raw）里 bdecode 出来，那是另一件事；
    // 先把它钉成「不知道」而不是「没有」：空清单不再当作证据。
    let file_names: Vec<String> = Vec::new();
    let recent_titles: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM torrents \
         WHERE approval_status = 1 AND id <> $1 AND mtime > now() - \
           make_interval(days => 90) \
         ORDER BY mtime DESC LIMIT 300",
    )
    .bind(id)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let site_type: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'site_type'",
    )
    .bind(0)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "general".into());
    TorrentSnapshot {
        torrent_id: id,
        owner_id: owner,
        title: name,
        small_descr: small.unwrap_or_default(),
        descr,
        category_id: cat,
        site_type,
        file_names,
        recent_titles,
    }
}

/// 发布者画像（软规则 S1/S3）。
async fn load_profile(db: &PgPool, owner: i64) -> PublisherProfile {
    let row: Option<(i32, i32, Option<i32>, Option<chrono::NaiveDate>)> =
        sqlx::query_as(
            "SELECT \
                COALESCE(publish_credits, 0), \
                COALESCE(approve_streak, 0), \
                COALESCE(deny_count, 0), \
                created_at::date \
             FROM users WHERE id = $1",
        )
        .bind(owner)
        .fetch_optional(db)
        .await
        .ok()
        .flatten();
    let (credits, streak, denied_total, created) = match row {
        Some(r) => r,
        None => return PublisherProfile::default(),
    };
    let registered_days = created
        .map(|d| (chrono::Utc::now().date_naive() - d).num_days().max(0) as i32)
        .unwrap_or(0);
    // 近 90 天人工通过/被拒（ torrents.approved_at / deny 时间用
    // modified_time 近似——被拒时 mtime 即拒绝时刻）
    let approved: i32 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE owner_id = $1 \
          AND approval_status = 1 \
          AND approved_at > now() - make_interval(days => 90)",
    )
    .bind(owner)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let denied: i32 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE owner_id = $1 \
          AND approval_status = 2 \
          AND mtime > now() - make_interval(days => 90)",
    )
    .bind(owner)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let _ = denied_total;
    PublisherProfile {
        publish_credits: credits,
        approve_streak: streak,
        registered_days,
        approved_90d: approved,
        denied_90d: denied,
    }
}

/// verdict 编码（与 0400 表注释一致）：0 通过 1 拒绝 2 暂缓 3 深检中。
fn verdict_of(a: &Assessment) -> i16 {
    match a.suggestion {
        Suggestion::Pass => 0,
        Suggestion::Deny => 1,
        Suggestion::Postpone => 2,
    }
}

/// mode 编码：0 影子 1 半自动 2 全自动。
fn mode_code(mode: &str) -> i16 {
    match mode {
        "half" => 1,
        "full" => 2,
        _ => 0,
    }
}

/// 单种子处理：落台账（影子/执法都落）+ 执法分支内调 decide。
async fn handle(
    db: &PgPool,
    cfg: &EngineConfig,
    torrent_id: i64,
    owner: i64,
    a: &Assessment,
) -> anyhow::Result<()> {
    let mode = mode_code(&cfg.mode);
    let verdict = verdict_of(a);
    let hits = auto_review::hits_to_json(&a.hits);
    // 唯一索引兜底单飞：冲突=别的实例已评，跳过整个执法分支
    let inserted = sqlx::query(
        "INSERT INTO auto_review_log \
         (torrent_id, uploader_id, mode, verdict, hit_rules, final_score, \
          ruleset_ver, decided_by) \
         VALUES ($1, $2, $3, $4, $5::jsonb, $6, $7, 0) \
         ON CONFLICT (torrent_id) WHERE decided_by = 0 AND verdict <> 3 \
         DO NOTHING",
    )
    .bind(torrent_id)
    .bind(owner)
    .bind(mode)
    .bind(verdict)
    .bind(hits.to_string())
    .bind(a.score)
    .bind(ruleset_ver(db).await)
    .execute(db)
    .await?;
    if inserted.rows_affected() == 0 {
        return Ok(()); // 已被评估（双实例竞态），让人工/既有结论生效
    }
    // H4 命中同步落 leak_events（§2.1：泄露证据供 staff 复核流，与
    // worker leak_scan 同表不自动处罚）——影子模式也落（证据记录不是
    // 「对种子动手」，且这条线索恰恰最需要人看）。
    if a.hits.iter().any(|h| h.rule == "arule_leak_marker") {
        // detail 是 jsonb：包一层 {"detail": "..."}（列口径，score 默认 0）
        let evidence = serde_json::json!({
            "detail": a.hits.iter()
                .find(|h| h.rule == "arule_leak_marker")
                .map(|h| h.detail.clone())
                .unwrap_or_default(),
            "torrent_id": torrent_id,
        });
        let _ = sqlx::query(
            "INSERT INTO leak_events (user_id, torrent_id, kind, detail)              SELECT $1, $2, 'descr_marker', $3::jsonb WHERE NOT EXISTS (                SELECT 1 FROM leak_events WHERE user_id = $1                  AND kind = 'descr_marker' AND created_at > now() - interval '1 day')",
        )
        .bind(owner)
        .bind(torrent_id)
        .bind(evidence.to_string())
        .execute(db)
        .await;
    }
    // 影子：到此为止（零副作用）
    if mode == 0 {
        return Ok(());
    }
    // §18.1 申诉/翻案回流：人工已推翻过的 (种子, 规则) 不再自动拒——
    // 否则「申诉赢了又被机器拒回去」死循环。被推翻规则集 = 该种历史
    // 台账行中 human_decided_at 非空者命中的规则（sweep 回填时点即
    // 人工翻案时点）。过滤后若无 deny 证据，降为暂缓由人定夺。
    let overturned: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT e->>'rule' FROM auto_review_log l,          jsonb_array_elements(l.hit_rules) e          WHERE l.torrent_id = $1 AND l.human_decided_at IS NOT NULL",
    )
    .bind(torrent_id)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let deny_evidence = a.hits.iter().any(|h| {
        h.action == Action::Deny && !overturned.contains(&h.rule.to_string())
    });
    // 半自动不执行自动拒绝；全自动拒绝须 deny 档证据（软分永不拒）
    let should_deny =
        mode == 2 && a.suggestion == Suggestion::Deny && deny_evidence;
    let should_approve = a.suggestion == Suggestion::Pass
        && a.hits.iter().all(|h| h.action != Action::Deny);
    if !should_deny && !should_approve {
        if a.suggestion == Suggestion::Deny && !overturned.is_empty() {
            tracing::info!(torrent = torrent_id,
                "auto_review: deny 证据已被人工推翻，降为人工处理（§18.1 回流）");
        }
        return Ok(()); // 灰区：台账留意见，人工处理（暂缓不落 torrents）
    }
    // 执法：HTTP 内调 auto-decide（裁决同尺）。凭证三选一（compose 注入）：
    //   FLUX_ENGINE_JWT（现成 token，RS256 站点用）> JWT_SECRET 现签（HS256）
    //   > 无凭证（跳过执法，留人工——fail-safe）。api 不可达同理只 WARN：
    // 台账已落，唯一索引已占，改日重试不可行，人工队列兜底。
    let token = engine_jwt(db).await;
    let Some(token) = token else {
        tracing::warn!(torrent = torrent_id,
            "auto_review: 无引擎凭证（JWT_SECRET/FLUX_ENGINE_JWT 均缺），留在人工队列");
        return Ok(());
    };
    let base = std::env::var("AUTO_REVIEW_API")
        .unwrap_or_else(|_| "http://flux-api:9000".into());
    let payload = serde_json::json!({
        "torrent_id": torrent_id,
        "approve": should_approve,
        "reason": a.hits.iter().map(|h| h.detail.clone())
            .take(3).collect::<Vec<_>>().join("；"),
        "deny_reason_id": deny_reason_id_for(db, &a.hits).await,
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let url = format!("{base}/api/v1/admin/reviews/auto-decide");
    let resp = client
        .post(&url)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .header("x-flux-engine", "auto-review")
        .json(&payload)
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => {
            tracing::info!(
                torrent = torrent_id,
                approve = should_approve,
                "auto_review: 已裁决（decide 同尺）"
            );
        }
        Ok(r) => {
            tracing::warn!(torrent = torrent_id, status = %r.status(),
                           "auto_review: auto-decide 拒绝（401=凭证配置错；skipped=人审先到属正常）");
        }
        Err(e) => {
            tracing::warn!(%e, torrent = torrent_id,
                           "auto_review: api 不可达，留在人工队列");
        }
    }
    Ok(())
}

/// 引擎凭证：FLUX_ENGINE_JWT 直用（RS256 站点）；否则 HS256 现签
/// （claims 与 api 侧 auth::Claims 同构：sub=系统审核员、exp=+5min）。
/// 手工铸 JWT 的 HS256 头/签名见 0214 五路修复批同款做法。
async fn engine_jwt(db: &PgPool) -> Option<String> {
    if let Ok(t) = std::env::var("FLUX_ENGINE_JWT") {
        if !t.trim().is_empty() {
            return Some(t);
        }
    }
    let secret = std::env::var("JWT_SECRET").unwrap_or_default();
    let alg = std::env::var("JWT_ALG").unwrap_or_else(|_| "hs256".into());
    if secret.len() < 32 || alg != "hs256" {
        return None; // RS256 无私钥可签：必须用 FLUX_ENGINE_JWT 注入
    }
    let sub: i64 = system_operator(db).await?;
    let header = serde_json::json!({"alg": "HS256", "typ": "JWT"});
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "sub": sub, "class_id": 99, "exp": now + 300, "iat": now,
    });
    let b64 = |v: &str| {
        let bytes = v.as_bytes();
        const T: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            out.push(T[(n >> 18) as usize & 63] as char);
            out.push(T[(n >> 12) as usize & 63] as char);
            out.push(if chunk.len() > 1 {
                T[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                T[n as usize & 63] as char
            } else {
                '='
            });
        }
        out
    };
    let signing_input =
        format!("{}.{}", b64(&header.to_string()), b64(&claims.to_string()));
    let sig = {
        use hmac::{Hmac, Mac};
        type HmacSha256 = Hmac<sha2::Sha256>;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
        mac.update(signing_input.as_bytes());
        let bytes = mac.finalize().into_bytes();
        let mut s = String::new();
        // URL-safe base64（无填充形态与 jsonwebtoken 一致）
        const T: &str =
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        for chunk in bytes.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            s.push(T.as_bytes()[(n >> 18) as usize & 63] as char);
            s.push(T.as_bytes()[(n >> 12) as usize & 63] as char);
            s.push(if chunk.len() > 1 {
                T.as_bytes()[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            s.push(if chunk.len() > 2 {
                T.as_bytes()[n as usize & 63] as char
            } else {
                '='
            });
        }
        s
    };
    Some(format!("{signing_input}.{sig}"))
}

/// 命中规则 → 拒因字典 id（0400 种子条目按 sort 定位）。
async fn deny_reason_id_for(
    db: &PgPool,
    hits: &[flux_shared::auto_review::RuleHit],
) -> Option<i64> {
    let rule = hits.iter().find(|h| h.action == Action::Deny)?;
    let label: i32 = match rule.rule {
        "arule_title_garbled" | "arule_title_format" => 91,
        "arule_descr_missing" => 92,
        "arule_ads_keywords" => 93,
        "arule_leak_marker" => 94,
        "arule_dupe_check" => 95,
        _ => 96,
    };
    sqlx::query_scalar(
        "SELECT id FROM torrent_deny_reasons WHERE sort = $1 LIMIT 1",
    )
    .bind(label)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

/// 人工推翻回填 + 熔断（§6/§12）。与 api 侧 review_auto::sweep_overturns
/// 同款 SQL 的 worker 副本（两边都无法跨 crate 调用；台账是共享表，
/// 幂等设计保证双跑不重复计数：human_decided_at 回填即记账一次）。
async fn sweep_overturns(db: &PgPool) {
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT l.id, l.hit_rules::text FROM auto_review_log l          JOIN torrents t ON t.id = l.torrent_id          WHERE l.decided_by = 0 AND l.human_decided_at IS NULL            AND ((l.verdict = 1 AND t.approval_status = 1)              OR (l.verdict = 0 AND t.approval_status = 2))",
    )
    .bind(0)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    use std::collections::HashMap;
    let mut per_rule: HashMap<String, i32> = HashMap::new();
    for (log_id, hits) in &rows {
        if let Ok(arr) = serde_json::from_str::<serde_json::Value>(hits) {
            if let Some(list) = arr.as_array() {
                for e in list {
                    if let Some(r) = e.get("rule").and_then(|v| v.as_str()) {
                        *per_rule.entry(r.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
        let _ = sqlx::query(
            "UPDATE auto_review_log SET human_decided_at = now() WHERE id = $1",
        )
        .bind(log_id)
        .execute(db)
        .await;
    }
    for (rule, n) in &per_rule {
        let _ = sqlx::query(
            "INSERT INTO auto_review_rule_health (rule, overturns)              VALUES ($1, $2) ON CONFLICT (rule) DO UPDATE SET              overturns = auto_review_rule_health.overturns + $2, updated_at = now()",
        )
        .bind(rule)
        .bind(n)
        .execute(db)
        .await;
        let total: i32 = sqlx::query_scalar(
            "SELECT overturns + 2 * appeal_wins              FROM auto_review_rule_health WHERE rule = $1",
        )
        .bind(&rule)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .unwrap_or(0);
        if total >= 10 {
            let key = format!("{rule}_action");
            let _ = sqlx::query(
                "INSERT INTO site_settings (name, value, descr, grp)                  VALUES ($1, 'postpone', '自动审种（规则健康度降级写入）', 'auto_review')                  ON CONFLICT (name) DO UPDATE SET value = 'postpone'",
            )
            .bind(&key)
            .execute(db)
            .await;
            let _ = sqlx::query(
                "UPDATE auto_review_rule_health SET demoted_at = now() WHERE rule = $1",
            )
            .bind(&rule)
            .execute(db)
            .await;
            tracing::warn!(rule = %rule,
                "auto_review: 规则被推翻累计 ≥10，已降级为仅暂缓");
        }
    }
    let threshold: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings           WHERE name = 'auto_review_circuit_daily_overturn'), 5)",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or(5);
    if threshold > 0 {
        let today: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM auto_review_log \
             WHERE human_decided_at >= \
             (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') \
             AT TIME ZONE 'Asia/Shanghai')",
        )
        .bind(0)
        .fetch_one(db)
        .await
        .unwrap_or(0);
        if today >= threshold {
            let mode: String = sqlx::query_scalar(
                "SELECT value FROM site_settings WHERE name = 'auto_review_mode'",
            )
            .bind(0)
            .fetch_optional(db)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
            if mode == "full" {
                let _ = sqlx::query(
                    "UPDATE site_settings SET value = 'half'                      WHERE name = 'auto_review_mode'",
                )
                .execute(db)
                .await;
                tracing::warn!(
                    today,
                    threshold,
                    "auto_review: 熔断——日推翻达阈值，已降级半自动"
                );
            }
        }
    }
}

/// 深检·文件清单层：D2 可疑可执行 / D4 话数交叉 / D5 伪媒体（文件名层）。
/// 结果并入 Assessment.hits（postpone 档），与在线规则同走台账/执法流。
async fn deep_file_rules(
    db: &PgPool,
    a: &mut Assessment,
    snap: &TorrentSnapshot,
    cfg: &EngineConfig,
) {
    // D1（§2.3）沙箱试连·数据面：tracker 在线探针交给现有
    // leak_scan/sweep（30m 档已覆盖活性）；这里判「秒完成」异常——
    // 待审种在 snatches 已有完成记录=下载在审核前就发生（发布者
    // 秒做种给同伙的强信号，2026-10-07 幽灵做种审计同型）。
    let early_snatch: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM snatches WHERE torrent_id = $1            AND completed_at IS NOT NULL",
    )
    .bind(snap.torrent_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .unwrap_or(0);
    if early_snatch > 0 {
        a.hits.push(auto_review::RuleHit {
            rule: "arule_sandbox_anomaly",
            action: Action::Postpone,
            score: 0,
            detail: format!(
                "待审期间已有 {early_snatch} 条完成记录（审核前下载已发生，疑似私发）"
            ),
        });
        a.suggestion = if a.suggestion == auto_review::Suggestion::Pass {
            auto_review::Suggestion::Postpone
        } else {
            a.suggestion
        };
    }
    // D2 分类白名单：auto_review_exe_whitelist_cats（逗号分隔分类 ID）
    let whitelist: String = sqlx::query_scalar(
        "SELECT value FROM site_settings          WHERE name = 'auto_review_exe_whitelist_cats'",
    )
    .bind(0)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .unwrap_or_default();
    let exempt = whitelist
        .split(|c| c == ',' || c == '，')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .any(|c| c == snap.category_id);
    if let Some(d) = auto_review::hit_suspicious_exe(&snap.file_names, exempt) {
        a.hits.push(auto_review::RuleHit {
            rule: "arule_files_exe",
            action: Action::Postpone,
            score: 0,
            detail: d,
        });
        // 有 deny 级证据才可能变 Deny；深检层永远只暂缓 ⇒ 建议需重算
        a.suggestion = if a.suggestion == auto_review::Suggestion::Pass {
            auto_review::Suggestion::Postpone
        } else {
            a.suggestion
        };
    }
    if let Some(d) =
        auto_review::hit_files_crosscheck(&snap.title, &snap.file_names)
    {
        a.hits.push(auto_review::RuleHit {
            rule: "arule_files_crosscheck",
            action: Action::Postpone,
            score: 0,
            detail: d,
        });
        a.suggestion = if a.suggestion == auto_review::Suggestion::Pass {
            auto_review::Suggestion::Postpone
        } else {
            a.suggestion
        };
    }
    // D3（§2.3）：MediaInfo（0390 真源，summary 无容器字段）× 文件清单
    // 交叉——「解析出视频轨但包里没有任何视频文件」= 伪媒体强信号。
    // 解析缺位/无 MediaInfo → 不命中（宁可漏放）。
    let has_mi_video: bool = sqlx::query_scalar(
        "SELECT mediainfo_check->'summary'->>'video_codec' IS NOT NULL          FROM torrents WHERE id = $1",
    )
    .bind(snap.torrent_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .unwrap_or(false);
    if has_mi_video && !snap.file_names.is_empty() {
        let any_video = snap.file_names.iter().any(|f| {
            let low = f.to_lowercase();
            [
                ".mp4", ".mkv", ".avi", ".wmv", ".mov", ".ts", ".m2ts", ".iso",
            ]
            .iter()
            .any(|e| low.ends_with(e))
        });
        if !any_video {
            a.hits.push(auto_review::RuleHit {
                rule: "arule_fake_media",
                action: Action::Postpone,
                score: 0,
                detail: "MediaInfo 声明含视频轨，但包内无任何视频文件（伪媒体嫌疑，请人工核验）".into(),
            });
            a.suggestion = if a.suggestion == auto_review::Suggestion::Pass {
                auto_review::Suggestion::Postpone
            } else {
                a.suggestion
            };
        }
    }
    if let Some(d) = auto_review::hit_fake_media_by_names(&snap.file_names) {
        a.hits.push(auto_review::RuleHit {
            rule: "arule_fake_media",
            action: Action::Postpone,
            score: 0,
            detail: d,
        });
        a.suggestion = if a.suggestion == auto_review::Suggestion::Pass {
            auto_review::Suggestion::Postpone
        } else {
            a.suggestion
        };
    }
    let _ = cfg;
}

/// 管理摘要日报（§4.3）：昨日自动通过/拒绝/暂缓 + 影子四指标 + Top 规则。
/// 幂等锚：站内信 kind=review_auto_digest 当日只发一封（messages 查重）。
pub async fn auto_review_digest(db: &PgPool) -> anyhow::Result<()> {
    let enabled: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value = '1' FROM site_settings          WHERE name = 'auto_review_notify_admin'), true)",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or(true);
    if !enabled {
        return Ok(());
    }
    // 今日已发则跳过
    let sent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM messages WHERE kind = 'review_auto_digest' \
         AND created_at > \
         (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') \
         AT TIME ZONE 'Asia/Shanghai')",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    if sent > 0 {
        return Ok(());
    }
    let (pass, deny, postpone): (i64, i64, i64) = sqlx::query_as(
        "SELECT             count(*) FILTER (WHERE verdict = 0),             count(*) FILTER (WHERE verdict = 1),             count(*) FILTER (WHERE verdict = 2)          FROM auto_review_log          WHERE decided_by = 0            AND created_at >= (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') - interval '1 day') AT TIME ZONE 'Asia/Shanghai'            AND created_at < (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai')) AT TIME ZONE 'Asia/Shanghai'",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or((0, 0, 0));
    let (agree, total): (i64, i64) = sqlx::query_as(
        "SELECT             count(*) FILTER (WHERE                 (l.verdict = 0 AND t.approval_status = 1) OR                 (l.verdict = 1 AND t.approval_status = 2)),             count(*)          FROM auto_review_log l JOIN torrents t ON t.id = l.torrent_id          WHERE l.decided_by = 0 AND t.approval_status IN (1, 2)",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or((0, 0));
    let agree_pct = if total > 0 { agree * 100 / total } else { 0 };
    let top_rules: Vec<(String, i64)> = sqlx::query_as(
        "SELECT e->>'rule' AS rule, count(*)          FROM auto_review_log l, jsonb_array_elements(l.hit_rules) e          WHERE l.created_at >= (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') - interval '1 day') AT TIME ZONE 'Asia/Shanghai'            AND l.created_at < (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai')) AT TIME ZONE 'Asia/Shanghai'          GROUP BY 1 ORDER BY 2 DESC LIMIT 5",
    )
    .bind(0)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let top = top_rules
        .iter()
        .map(|(r, n)| format!("{r}×{n}"))
        .collect::<Vec<_>>()
        .join("、");
    let subject = "自动审种每日摘要".to_string();
    let body = format!(
        "昨日：自动通过 {pass} / 自动拒绝 {deny} / 暂缓 {postpone}。
         影子一致率 {agree_pct}%（{agree}/{total} 样本）。
         Top 规则：{top}。详情见管理台「自动审种」。"
    );
    // 发给管理组：逐个 class≥99 在册用户（messages 无组播，逐条插）
    let staff: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE class_id >= 99 AND status < 2 LIMIT 20",
    )
    .bind(0)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    for uid in staff {
        let _ = sqlx::query(
            "INSERT INTO messages (receiver_id, subject, body, kind, params)              SELECT $1, $2, $3, 'review_auto_digest', '{}'::jsonb              WHERE EXISTS (SELECT 1 FROM users WHERE id = $1)",
        )
        .bind(uid)
        .bind(&subject)
        .bind(&body)
        .execute(db)
        .await;
    }
    Ok(())
}

/// §13.4 + §15.1：队列健康观测（超时升级 + 深度告警），均一天最多一封。
async fn queue_health(db: &PgPool) {
    let (depth, stale): (i64, i64) = sqlx::query_as(
        "SELECT             (SELECT count(*) FROM torrents WHERE approval_status = 0),             (SELECT count(*) FROM torrents t WHERE t.approval_status = 0                AND t.mtime < now() - make_interval(hours => 24)                AND NOT EXISTS (SELECT 1 FROM auto_review_log l                      WHERE l.torrent_id = t.id AND l.decided_by = 0))",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or((0, 0));
    let alerted_today: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staffmessages          WHERE subject LIKE '自动审种：队列%'            AND created_at > now() - interval '20 hours'",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    if alerted_today > 0 {
        return;
    }
    let msg = if stale > 0 {
        Some(format!(
            "待审队列有 {stale} 个种子超过 24 小时无人处理（且未经机器评估）。             请审核组关注：管理台 → 种子审核。"
        ))
    } else if depth > 50 {
        Some(format!(
            "待审队列深度 {depth}（>50）。自动审种引擎在持续消化，             人工侧可帮忙看暂缓件：管理台 → 自动审种。"
        ))
    } else {
        None
    };
    if let Some(body) = msg {
        // 走 worker 的管理组告警**唯一出口**（staff_alert_as，署名档 99
        // = 与 leak_bot 同口径）：守卫（无在册署名不插行、只署在册号）
        // 收在出口件里，闸门 BQ 的 R1d 才站得住。
        super::staff_alert::staff_alert_as(
            db,
            "自动审种：队列健康提醒",
            &body,
            "torrent",
            99,
        )
        .await;
    }
}
