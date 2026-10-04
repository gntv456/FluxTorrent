// FluxTorrent 匿名使用统计收集端（0276）
//
// 部署：Cloudflare Workers 免费版（10 万请求/天，绰绰有余）
//   wrangler deploy scripts/stats-collector/worker.js --name flux-stats
//   并把站点设定 stats_report_url 指到 https://flux-stats.<你的子域>.workers.dev/ping
//
// 数据落 KV：key = site:<site_id>，value = { version, users, torrents,
// active_peers, first_seen, last_seen, days }。看板就是 /admin 端点的 JSON
// （或导出到本地自己统计）。只存计数，永不存在任何可识别信息——上报端
// （apps/worker/src/jobs/usage_stats.rs）已经匿名化。
//
// 接口：
//   POST /ping   —— 站点上报（site_id 16 hex + 版本 + 三项计数）
//   GET  /admin  —— 总览 JSON：{ sites: N, by_version: {...}, recent: [...] }
//                   ?token= 与环境变量 ADMIN_TOKEN 比对（没设则只允许本机回环看）

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname === "/ping" && request.method === "POST") {
      let body;
      try {
        body = await request.json();
      } catch {
        return json({ error: "bad json" }, 400);
      }
      if (!/^[0-9a-f]{16}$/.test(String(body.site_id || ""))) {
        return json({ error: "bad site_id" }, 400);
      }
      const key = "site:" + body.site_id;
      const prev = (await env.STATS.get(key, "json")) || {};
      const rec = {
        version: String(body.version || "?").slice(0, 32),
        users: clampInt(body.users),
        torrents: clampInt(body.torrents),
        active_peers: clampInt(body.active_peers),
        first_seen: prev.first_seen || new Date().toISOString(),
        last_seen: new Date().toISOString(),
        days: (prev.days || 0) + 1, // 每 site 每天最多 +1（worker 侧 24h 节流）
      };
      await env.STATS.put(key, JSON.stringify(rec));
      return json({ ok: true });
    }

    if (url.pathname === "/admin" && request.method === "GET") {
      const want = env.ADMIN_TOKEN;
      if (want && url.searchParams.get("token") !== want) {
        return json({ error: "forbidden" }, 403);
      }
      const list = await env.STATS.list({ prefix: "site:" });
      const sites = [];
      for (const k of list.keys) {
        const v = await env.STATS.get(k.name, "json");
        if (v) sites.push({ site_id: k.name.slice(5), ...v });
      }
      const by_version = {};
      for (const s of sites) {
        by_version[s.version] = (by_version[s.version] || 0) + 1;
      }
      const month_ago = Date.now() - 30 * 86400 * 1000;
      const active30 = sites.filter((s) => +new Date(s.last_seen) > month_ago);
      return json({
        sites: sites.length,
        active_30d: active30.length,
        by_version,
        sites: sites.sort((a, b) => (a.last_seen < b.last_seen ? 1 : -1)),
      });
    }

    return json({ error: "not found" }, 404);
  },
};

function json(obj, status = 200) {
  return new Response(JSON.stringify(obj, null, 2), {
    status,
    headers: {
      "content-type": "application/json",
      "access-control-allow-origin": "*",
    },
  });
}

function clampInt(v) {
  const n = Number(v);
  return Number.isFinite(n) ? Math.min(Math.max(Math.trunc(n), 0), 1e9) : 0;
}
