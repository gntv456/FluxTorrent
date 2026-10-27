/** 旧 URL 兼容：历史 ?tool= 值与现 tab_key 命名不一致，映射后旧书签不失效。
 *
 *  中性模块（非 "use client"）：后台 catch-all 路由 `admin/[...slug]` 是
 *  server component，从 client 模块读这个对象只会拿到 undefined——
 *  旧书签静默落到未知名（2026-10-27 实测 /admin/reset 未映射成 resetpass）。
 */
export const LEGACY_TOOL: Record<string, string> = {
  reset: "resetpass",
  deletedisabled: "deldisabled",
  bannedemails: "emailbans",
  allowedemails: "emailbans",
  docleanup: "cleanup",
  admanage: "ads",
  allagents: "agents",
  polloverview: "polls",
  amountbonus: "incrementbulk",
  bonus: "incrementbulk",
  amountupload: "incrementbulk",
  upload: "incrementbulk",
  notconnectable: "notconnect",
  location: "locations",
  faqmanage: "faq",
  modrules: "rules",
  catmanage: "cats",
  mysql_stats: "dbstats",
  bitbucketlog: "syslog",
  // 邮件群发的 DB url 曾写作 ?tool=massmail，与 tab_key `mail` 不一致（0156 已改）。
  // 这里兜住仍然存在的老书签：没有这条会落进 panelEmpty 空面板。
  massmail: "mail",
  // 促销公告（promo）面板早被 FreeleechPanel 取代（同打 /admin/freeleech），
  // 旧入口指向「免费/促销状态」。
  promo: "freeleech",
};
