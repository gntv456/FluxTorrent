// OpenAPI 注解覆盖门禁的域数据（从 openapi_cov_guard.mjs 拆出以守住
// 300 行门禁）：这里只放「域清单」与「注解 ↔ route-manifest 对拍」的
// 采集活，判定逻辑留在主门禁。
//
// 新增域注解化时：① 本文件补源文件；② 主门禁 DOMAIN_ROUTES 补路由；
// ③ 跑 node scripts/openapi_cov_guard.mjs，少一条立刻红。

export const ANNOTATED_FILES = [
  "apps/api/src/openapi_http/open.rs",
  "apps/api/src/openapi_http/detail_open.rs",
  "apps/api/src/openapi_http/me.rs",
  "apps/api/src/openapi_http/me_torrents.rs",
  "apps/api/src/openapi_http/upload.rs",
  "apps/api/src/openapi_http/tokens.rs",
  "apps/api/src/compat_http/nexusphp.rs",
  "apps/api/src/compat_http/aliases.rs",
  "apps/api/src/compat_http/ptpp.rs",
  "apps/api/src/compat_http/download_keys.rs",
  "apps/api/src/economy_http/recon.rs",
  "apps/api/src/rss_http.rs",
  // 种子域（F4 2026-10-16：38 条路由注解化，脚本生成）
  "apps/api/src/torrent_http/list.rs",
  "apps/api/src/torrent_http/detail.rs",
  "apps/api/src/torrent_http/interact.rs",
  "apps/api/src/torrent_http/manage.rs",
  "apps/api/src/torrent_http/magnet.rs",
  "apps/api/src/torrent_http/peers.rs",
  "apps/api/src/torrent_http/preview.rs",
  "apps/api/src/torrent_http/trump.rs",
  "apps/api/src/torrent_http/trump_admin.rs",
  "apps/api/src/torrent_http/aggregate.rs",
  "apps/api/src/torrent_http/batch.rs",
  "apps/api/src/torrent_http/cue.rs",
  "apps/api/src/torrent_http/file_conflict.rs",
  "apps/api/src/torrent_http/logs.rs",
  "apps/api/src/torrent_http/related.rs",
  "apps/api/src/torrent_http/suggest.rs",
  // 社区域（F4 2026-10-16 第三批：85 条路由注解化）
  "apps/api/src/community_http/artists.rs",
  "apps/api/src/community_http/bounty.rs",
  "apps/api/src/community_http/embed_rules.rs",
  "apps/api/src/community_http/feed.rs",
  "apps/api/src/community_http/follow.rs",
  "apps/api/src/community_http/follow_read.rs",
  "apps/api/src/community_http/forum_util.rs",
  "apps/api/src/community_http/forums.rs",
  "apps/api/src/community_http/forums_boards.rs",
  "apps/api/src/community_http/friend.rs",
  "apps/api/src/community_http/friend_action.rs",
  "apps/api/src/community_http/interact.rs",
  "apps/api/src/community_http/leak_bot.rs",
  "apps/api/src/community_http/like.rs",
  "apps/api/src/community_http/list_prefs.rs",
  "apps/api/src/community_http/lottery.rs",
  "apps/api/src/community_http/medal.rs",
  "apps/api/src/community_http/medal_buy.rs",
  "apps/api/src/community_http/medal_gift.rs",
  "apps/api/src/community_http/message.rs",
  "apps/api/src/community_http/message_box.rs",
  "apps/api/src/community_http/message_folders.rs",
  "apps/api/src/community_http/message_send.rs",
  "apps/api/src/community_http/message_stream.rs",
  "apps/api/src/community_http/network_subs.rs",
  "apps/api/src/community_http/networks.rs",
  "apps/api/src/community_http/notice.rs",
  "apps/api/src/community_http/post_edit.rs",
  "apps/api/src/community_http/post_read.rs",
  "apps/api/src/community_http/post_topic_delete.rs",
  "apps/api/src/community_http/post_write.rs",
  "apps/api/src/community_http/seasonal.rs",
  "apps/api/src/community_http/shoutbox.rs",
  "apps/api/src/community_http/sport_matches.rs",
  "apps/api/src/community_http/sport_match_detail.rs",
  "apps/api/src/community_http/staff_answer.rs",
  "apps/api/src/community_http/staffbox.rs",
  "apps/api/src/community_http/subs_calendar.rs",
  "apps/api/src/community_http/ticket.rs",
  "apps/api/src/community_http/tip.rs",
  "apps/api/src/community_http/topic.rs",
  "apps/api/src/community_http/topic_admin.rs",
  "apps/api/src/community_http/topic_create.rs",
];


/**
 * 社区 85 条路由 ↔ #[utoipa::path] 注解对拍。
 *
 * 口径：route-manifest 里 file 落在 apps/api/src/community_http/ 的路由
 * 就是本域事实源；每一条都必须在某个 #[utoipa::path] 注解里出现。这样
 * 社区域再新增路由时，忘了注解立刻红，而不是静悄悄漏出 spec。
 *
 * @param {(p: string) => string} read 读文件回调
 * @param {object} manifest route-manifest 解析结果
 * @returns {number} 失败数（0 = 全绿）
 */
export function checkCommunityCoverage(read, manifest) {
  const expected = manifest.routes.filter((r) =>
    r.file.startsWith("apps/api/src/community_http/"),
  );
  let fails = 0;
  if (expected.length !== 85) {
    console.log(
      `FAIL  route-manifest 社区路由数为 ${expected.length}，预期 85` +
        `（新增社区路由请同步 open_api.rs 聚合并更新本判据）`,
    );
    fails++;
  }

  // 收集注解里的 (method, path) 二元组。
  // 注解两种写法都要吃：
  //   #[utoipa::path(get, path = "/api/v1/x", ...)]      —— 宏式，方法小写裸写
  //   #[utoipa::path(method = "GET", path = "/api/v1/x")] —— 字面式
  //
  // 注意：源文件是 CRLF，不能用 \n\s*\)\] 这种「闭合括号独占一行」的
  // 形态去切，会静悄悄 0 匹配；这里按 )] 直接收，块内本来就只有
  // responses 的元组括号，不会误切。
  const METHOD_ALT =
    "\\b(GET|POST|PUT|DELETE|PATCH|get|post|put|delete|patch)\\b";
  const ops = new Set();
  for (const f of ANNOTATED_FILES) {
    const src = read(f);
    for (const b of src.matchAll(/#\[utoipa::path\(([\s\S]*?)\)\]/g)) {
      const body = b[1];
      const pm = body.match(/path\s*=\s*"\/api\/v1([^"]*)"/);
      if (!pm) continue;
      const literal = body.match(/method\s*=\s*"([^"]+)"/);
      const method = literal
        ? literal[1]
        : (body.match(METHOD_ALT) || [])[1];
      if (method) ops.add(`${method.toUpperCase()} ${pm[1]}`);
    }
  }

  const miss = expected.filter((r) => !ops.has(`${r.method} ${r.path}`));
  if (miss.length) {
    for (const r of miss.slice(0, 20)) {
      console.log(
        `FAIL  社区路由 ${r.method} ${r.path}（${r.file}）没有 #[utoipa::path] 注解`,
      );
    }
    if (miss.length > 20) {
      console.log(`FAIL  …… 另有 ${miss.length - 20} 条未列出`);
    }
    fails++;
  } else {
    console.log(`OK  社区 ${expected.length}/${expected.length} 条全部已注解`);
  }
  return fails;
}
