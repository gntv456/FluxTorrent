"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** staffpanel 管理工具落地页：FAQ 管理/规则管理/分类管理/封禁系统/批量邮件
 *  （hxpt faqmanage/modrules/catmanage/bans/massmail 口径，五个工具 tab） */

interface FaqItem { id: number; category: string; question: string; answer: string; sort: number }
interface RuleItem { id: number; title: string; body: string; sort: number }
interface CatItem { id: number; name: string; torrents: number }
interface BanItem { id: number; ip: string; reason: string | null; banned_by: string | null; created_at: string }
interface MailItem { id: number; subject: string; recipients: number; created_at: string; sender: string | null }
interface GlobalPromo { id: number; kind: string; starts_at: string; ends_at: string }
interface WarnedUser { id: number; username: string; warned_until: string | null; warned_reason: string | null }
interface IpCheckRow { ip: string | null; users: number; usernames: string | null; last_seen: string | null }
interface FailedLogin { id: number; username: string | null; ip: string | null; created_at: string }
interface EmailBan { id: number; pattern: string; mode: string; note: string | null; created_by: string | null; created_at: string }
interface AdItem { id: number; title: string; html: string; position: string; enabled: boolean; sort: number }
interface TestIpResult { ip: string; banned: boolean; reason: string | null; by: string | null; seen_users: string[] }
interface SiteStats { users: number; torrents: number; seeding: number; leeching: number; comments: number; messages: number; redis: string; db: string; uptime_secs: number }
interface CleanupResult { expired_promotions: number; expired_warnings: number; old_login_events: number; old_password_resets: number }
interface NotConnectRow { id: number; username: string; torrents: number; last_seen_at: string | null }
interface UploaderRow { id: number; username: string; uploads: number; seeding: number; total_size: number }
interface AgentRow { agent: string; peers: number }
interface PollRow { id: number; question: string; closed: boolean; votes: number; created_at: string }
interface PgConn { state: string; count: number }
interface TableSize { relname: string; total_size: number; row_estimates: number }
interface DbStats { engine: string; database: string; connections: PgConn[]; total_connections: number; database_size: number; slow_transactions: number; dead_tuples: number; tables: TableSize[] }
interface SysLogItem { id: number; actor: string | null; action: string; ref_json: unknown; ip: string | null; created_at: string }
interface SysLogPage { items: SysLogItem[]; total: number; page: number; per_page: number; pages: number }
interface LocationItem { net: string; netmask: number; logins: number; users: number; failed: number; last_seen: string | null }
interface LocationPage { items: LocationItem[]; total: number; page: number; per_page: number; pages: number }

type ToolTab =
  | "faq" | "rules" | "cats" | "bans" | "mail"
  | "promo" | "staffmess" | "adduser" | "bonus" | "warned" | "ipcheck" | "maxlogin"
  | "upload" | "resetpass" | "deldisabled" | "emailbans" | "testip" | "stats"
  | "cleanup" | "ads" | "notconnect" | "uploaders" | "agents" | "polls"
  | "dbstats" | "syslog" | "locations" | "hrpardon" | "plugins";

export function StaffTools({ initialTab }: { initialTab?: ToolTab }) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const [tab, setTab] = useState<ToolTab>(initialTab ?? "faq");
  const [faqs, setFaqs] = useState<FaqItem[]>([]);
  const [rules, setRules] = useState<RuleItem[]>([]);
  const [cats, setCats] = useState<CatItem[]>([]);
  const [bans, setBans] = useState<BanItem[]>([]);
  const [mails, setMails] = useState<MailItem[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // FAQ / 规则编辑器
  const [faqEdit, setFaqEdit] = useState<{ id: number | null; question: string; answer: string }>({ id: null, question: "", answer: "" });
  const [ruleEdit, setRuleEdit] = useState<{ id: number | null; title: string; body: string }>({ id: null, title: "", body: "" });
  const [catName, setCatName] = useState("");
  const [banIp, setBanIp] = useState("");
  const [banReason, setBanReason] = useState("");
  const [mailSubject, setMailSubject] = useState("");
  const [mailBody, setMailBody] = useState("");
  // 运营工具状态
  const [promo, setPromo] = useState<GlobalPromo | null>(null);
  const [promoKind, setPromoKind] = useState("free");
  const [promoHours, setPromoHours] = useState(24);
  const [smSubject, setSmSubject] = useState("");
  const [smBody, setSmBody] = useState("");
  const [smMinClass, setSmMinClass] = useState("");
  const [auName, setAuName] = useState("");
  const [auEmail, setAuEmail] = useState("");
  const [auPass, setAuPass] = useState("");
  const [bonusAmount, setBonusAmount] = useState("1000");
  const [bonusUser, setBonusUser] = useState("");
  const [warned, setWarned] = useState<WarnedUser[]>([]);
  const [warnUser, setWarnUser] = useState("");
  const [warnWeeks, setWarnWeeks] = useState("2");
  const [warnReason, setWarnReason] = useState("");
  const [ipRows, setIpRows] = useState<IpCheckRow[]>([]);
  const [failRows, setFailRows] = useState<FailedLogin[]>([]);
  // 第二批运营工具
  const [upBytes, setUpBytes] = useState("10737418240"); // 10 GiB
  const [upUser, setUpUser] = useState("");
  const [resetId, setResetId] = useState("");
  const [resetPass, setResetPass] = useState<string | null>(null);
  const [emailBans, setEmailBans] = useState<EmailBan[]>([]);
  const [ebPattern, setEbPattern] = useState("");
  const [ebMode, setEbMode] = useState("ban");
  const [ebNote, setEbNote] = useState("");
  const [testIpQuery, setTestIpQuery] = useState("");
  const [testIpResult, setTestIpResult] = useState<TestIpResult | null>(null);
  const [stats, setStats] = useState<SiteStats | null>(null);
  const [cleanupResult, setCleanupResult] = useState<CleanupResult | null>(null);
  const [ads, setAds] = useState<AdItem[]>([]);
  const [adEdit, setAdEdit] = useState<{ id: number | null; title: string; html: string; position: string }>({ id: null, title: "", html: "", position: "header" });
  const [notConnectRows, setNotConnectRows] = useState<NotConnectRow[]>([]);
  const [uploaderRows, setUploaderRows] = useState<UploaderRow[]>([]);
  const [agentRows, setAgentRows] = useState<AgentRow[]>([]);
  const [pollRows, setPollRows] = useState<PollRow[]>([]);
  const [dbStats, setDbStats] = useState<DbStats | null>(null);
  const [logPage, setLogPage] = useState(1);
  const [logQ, setLogQ] = useState("");
  const [logData, setLogData] = useState<SysLogPage | null>(null);
  const [locPage, setLocPage] = useState(1);
  const [locData, setLocData] = useState<LocationPage | null>(null);
  const [hpUser, setHpUser] = useState("");
  const [hpTorrent, setHpTorrent] = useState("");
  const [hpNote, setHpNote] = useState("");
  const [pluginList, setPluginList] = useState<string[] | null>(null);

  const load = useCallback(async () => {
    try {
      const [f, r, b, m] = await Promise.all([
        api.get<FaqItem[]>("/api/v1/faq"),
        api.get<RuleItem[]>("/api/v1/rules-content"),
        api.get<BanItem[]>("/api/v1/admin/bans"),
        api.get<MailItem[]>("/api/v1/admin/massmail"),
      ]);
      setFaqs(f);
      setRules(r);
      setBans(b);
      setMails(m);
      // 分类列表（管理口径，含每分类种子数）
      try {
        const cs = await api.get<CatItem[]>("/api/v1/admin/categories");
        setCats(cs);
      } catch { /* 分类需 sysop，无权限时保持为空 */ }
      // 运营工具数据（促销/警告/重复IP/失败登录，失败静默）
      api.get<GlobalPromo | null>("/api/v1/admin/freeleech").then(setPromo).catch(() => {});
      api.get<WarnedUser[]>("/api/v1/admin/warned").then(setWarned).catch(() => setWarned([]));
      api.get<IpCheckRow[]>("/api/v1/admin/ipcheck").then(setIpRows).catch(() => setIpRows([]));
      api.get<FailedLogin[]>("/api/v1/admin/maxlogin").then(setFailRows).catch(() => setFailRows([]));
      api.get<EmailBan[]>("/api/v1/admin/emailbans").then(setEmailBans).catch(() => setEmailBans([]));
      api.get<SiteStats | null>("/api/v1/admin/stats").then(setStats).catch(() => setStats(null));
      api.get<AdItem[]>("/api/v1/admin/ads").then(setAds).catch(() => setAds([]));
      api.get<NotConnectRow[]>("/api/v1/admin/notconnectable").then(setNotConnectRows).catch(() => setNotConnectRows([]));
      api.get<UploaderRow[]>("/api/v1/admin/uploaders").then(setUploaderRows).catch(() => setUploaderRows([]));
      api.get<AgentRow[]>("/api/v1/admin/allagents").then(setAgentRows).catch(() => setAgentRows([]));
      api.get<PollRow[]>("/api/v1/admin/polloverview").then(setPollRows).catch(() => setPollRows([]));
      api.get<DbStats | null>("/api/v1/admin/dbstats").then(setDbStats).catch(() => setDbStats(null));
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.loadFailed);
    }
  }, [dict]);
  useEffect(() => { load(); }, [load]);
  // 系统日志/位置管理：按需分页加载
  useEffect(() => {
    api.get<SysLogPage>(`/api/v1/admin/syslog?page=${logPage}&q=${encodeURIComponent(logQ)}`)
      .then(setLogData).catch(() => setLogData(null));
  }, [logPage, logQ]);
  useEffect(() => {
    api.get<LocationPage>(`/api/v1/admin/locations?page=${locPage}`)
      .then(setLocData).catch(() => setLocData(null));
  }, [locPage]);

  function flash(m: string) { setMsg(m); setTimeout(() => setMsg(null), 2500); }
  async function guard(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try { await fn(); flash(ok); await load(); }
    catch (e) { flash(e instanceof ApiError ? e.message : dict.common.networkError); }
    finally { setBusy(false); }
  }

  const TABS: [ToolTab, string][] = [
    ["faq", t.tabFaq], ["rules", t.tabRules], ["cats", t.tabCats], ["bans", t.tabBans], ["mail", t.tabMail],
    ["promo", t.tabPromo], ["staffmess", t.tabStaffmess], ["adduser", t.tabAdduser],
    ["bonus", t.tabBonus], ["warned", t.tabWarned], ["ipcheck", t.tabIpcheck], ["maxlogin", t.tabMaxlogin],
    ["upload", t.tabUpload], ["resetpass", t.tabResetpass], ["deldisabled", t.tabDeldisabled],
    ["emailbans", t.tabEmailbans], ["testip", t.tabTestip], ["stats", t.tabStats],
    ["cleanup", t.tabCleanup], ["ads", t.tabAds],
    ["notconnect", t.tabNotconnect], ["uploaders", t.tabUploaders], ["agents", t.tabAgents], ["polls", t.tabPolls],
    ["dbstats", t.tabDbstats], ["syslog", t.tabSyslog], ["locations", t.tabLocations],
    ["hrpardon", t.tabHrpardon],
    ["plugins", t.tabPlugins ?? "插件"],
  ];

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-2" role="tablist">
        {TABS.map(([k, label]) => (
          <button key={k} role="tab" aria-selected={tab === k} onClick={() => setTab(k)}
            className={`min-h-[40px] rounded-full px-4 text-sm font-bold ${tab === k ? "bg-sky text-white" : "border border-line bg-white text-sub"}`}>
            {label}
          </button>
        ))}
      </div>
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}

      {/* FAQ 管理 */}
      {tab === "faq" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{faqEdit.id === null ? t.faqNew : t.faqEdit}</h2>
            <div className="cmgmt-form">
              <label>{t.fldQuestion}<input value={faqEdit.question} onChange={(e) => setFaqEdit({ ...faqEdit, question: e.target.value })} /></label>
              <label>{t.fldAnswer}<textarea rows={4} value={faqEdit.answer} onChange={(e) => setFaqEdit({ ...faqEdit, answer: e.target.value })} /></label>
              <div className="flex gap-2">
                <button className="baozi-button" disabled={busy || !faqEdit.question.trim()}
                  onClick={() => guard(async () => {
                    if (faqEdit.id === null) await api.post("/api/v1/admin/faq", { question: faqEdit.question, answer: faqEdit.answer });
                    else await api.put(`/api/v1/admin/faq/${faqEdit.id}`, { question: faqEdit.question, answer: faqEdit.answer });
                    setFaqEdit({ id: null, question: "", answer: "" });
                  }, t.saved)}>{t.btnSave}</button>
                {faqEdit.id !== null && (
                  <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
                    onClick={() => setFaqEdit({ id: null, question: "", answer: "" })}>{dict.cmgmt.btnCancel}</button>
                )}
              </div>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr><td className="colhead">{t.fldQuestion}</td><td className="colhead text-right">{dict.cmgmt.colActions}</td></tr>
              {faqs.map((f) => (
                <tr key={f.id}>
                  <td>{f.question}</td>
                  <td className="text-right">
                    <button className="cmgmt-act" onClick={() => setFaqEdit({ id: f.id, question: f.question, answer: f.answer })}>{dict.cmgmt.btnEdit}</button>
                    <button className="cmgmt-act cmgmt-act--danger"
                      onClick={() => guard(async () => { await api.del(`/api/v1/admin/faq/${f.id}`); }, t.deleted)}>{dict.cmgmt.btnDelete}</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {/* 规则管理 */}
      {tab === "rules" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{ruleEdit.id === null ? t.ruleNew : t.ruleEdit}</h2>
            <div className="cmgmt-form">
              <label>{dict.cmgmt.fldTitle}<input value={ruleEdit.title} onChange={(e) => setRuleEdit({ ...ruleEdit, title: e.target.value })} /></label>
              <label>{dict.cmgmt.fldBody}<textarea rows={5} value={ruleEdit.body} onChange={(e) => setRuleEdit({ ...ruleEdit, body: e.target.value })} /></label>
              <div className="flex gap-2">
                <button className="baozi-button" disabled={busy || !ruleEdit.title.trim()}
                  onClick={() => guard(async () => {
                    if (ruleEdit.id === null) await api.post("/api/v1/admin/rules", { title: ruleEdit.title, body: ruleEdit.body });
                    else await api.put(`/api/v1/admin/rules/${ruleEdit.id}`, { title: ruleEdit.title, body: ruleEdit.body });
                    setRuleEdit({ id: null, title: "", body: "" });
                  }, t.saved)}>{t.btnSave}</button>
                {ruleEdit.id !== null && (
                  <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
                    onClick={() => setRuleEdit({ id: null, title: "", body: "" })}>{dict.cmgmt.btnCancel}</button>
                )}
              </div>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr><td className="colhead">{dict.cmgmt.fldTitle}</td><td className="colhead text-right">{dict.cmgmt.colActions}</td></tr>
              {rules.map((r) => (
                <tr key={r.id}>
                  <td>{r.title}</td>
                  <td className="text-right">
                    <button className="cmgmt-act" onClick={() => setRuleEdit({ id: r.id, title: r.title, body: r.body })}>{dict.cmgmt.btnEdit}</button>
                    <button className="cmgmt-act cmgmt-act--danger"
                      onClick={() => guard(async () => { await api.del(`/api/v1/admin/rules/${r.id}`); }, t.deleted)}>{dict.cmgmt.btnDelete}</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {/* 分类管理 */}
      {tab === "cats" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.tabCats}</h2>
          <div className="cmgmt-form">
            <label>{t.fldCatName}<input value={catName} onChange={(e) => setCatName(e.target.value)} /></label>
            <button className="baozi-button self-start" disabled={busy || !catName.trim()}
              onClick={() => guard(async () => { await api.post("/api/v1/admin/categories", { name: catName }); setCatName(""); }, t.saved)}>
              {t.btnAddCat}
            </button>
          </div>
          <table className="nexus-table mt-3">
            <tbody>
              <tr><td className="colhead">#</td><td className="colhead">{t.fldCatName}</td><td className="colhead">{t.catTorrents}</td><td className="colhead text-right">{dict.cmgmt.colActions}</td></tr>
              {cats.map((c) => (
                <tr key={c.id}>
                  <td className="num">{c.id}</td>
                  <td>{c.name}</td>
                  <td className="num">{c.torrents}</td>
                  <td className="text-right">
                    <button className="cmgmt-act"
                      onClick={() => {
                        const nn = prompt(t.renamePrompt, c.name);
                        if (nn && nn !== c.name) void guard(async () => { await api.put(`/api/v1/admin/categories/${c.id}`, { name: nn }); }, t.saved);
                      }}>{dict.cmgmt.btnEdit}</button>
                    <button className="cmgmt-act cmgmt-act--danger"
                      onClick={() => guard(async () => { await api.del(`/api/v1/admin/categories/${c.id}`); }, t.deleted)}>{dict.cmgmt.btnDelete}</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {/* 封禁系统 */}
      {tab === "bans" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{t.banNew}</h2>
            <div className="cmgmt-form">
              <label>{t.fldIp}<input value={banIp} onChange={(e) => setBanIp(e.target.value)} placeholder="203.0.113.10" /></label>
              <label>{t.fldReason}<input value={banReason} onChange={(e) => setBanReason(e.target.value)} /></label>
              <button className="baozi-button self-start" disabled={busy || !banIp.trim()}
                onClick={() => guard(async () => { await api.post("/api/v1/admin/bans", { ip: banIp, reason: banReason }); setBanIp(""); setBanReason(""); }, t.banAdded)}>
                {t.btnBan}
              </button>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr><td className="colhead">{t.fldIp}</td><td className="colhead">{t.fldReason}</td><td className="colhead">{t.banBy}</td><td className="colhead text-right">{dict.cmgmt.colActions}</td></tr>
              {bans.map((b) => (
                <tr key={b.id}>
                  <td className="font-mono">{b.ip}</td>
                  <td>{b.reason ?? "—"}</td>
                  <td>{b.banned_by ?? "—"}</td>
                  <td className="text-right">
                    <button className="cmgmt-act cmgmt-act--ok"
                      onClick={() => guard(async () => { await api.del(`/api/v1/admin/bans/${b.id}`); }, t.unbanned)}>{t.btnUnban}</button>
                  </td>
                </tr>
              ))}
              {bans.length === 0 && <tr><td colSpan={4} className="py-6 text-center text-sub">{t.banEmpty}</td></tr>}
            </tbody>
          </table>
        </>
      )}

      {/* 批量邮件 */}
      {tab === "mail" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{t.mailNew}</h2>
            <div className="cmgmt-form">
              <label>{t.fldSubject}<input value={mailSubject} onChange={(e) => setMailSubject(e.target.value)} /></label>
              <label>{t.fldBody}<textarea rows={6} value={mailBody} onChange={(e) => setMailBody(e.target.value)} /></label>
              <button className="baozi-button self-start" disabled={busy || !mailSubject.trim() || !mailBody.trim()}
                onClick={() => guard(async () => {
                  await api.post("/api/v1/admin/massmail", { subject: mailSubject, body: mailBody });
                  setMailSubject(""); setMailBody("");
                }, t.mailQueued)}>
                {t.btnSend}
              </button>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr><td className="colhead">{t.fldSubject}</td><td className="colhead">{t.mailRecipients}</td><td className="colhead">{t.mailSender}</td><td className="colhead">{t.mailAt}</td></tr>
              {mails.map((m) => (
                <tr key={m.id}>
                  <td>{m.subject}</td>
                  <td className="num">{m.recipients}</td>
                  <td>{m.sender ?? "—"}</td>
                  <td className="text-xs text-sub">{new Date(m.created_at).toLocaleString("zh-CN")}</td>
                </tr>
              ))}
              {mails.length === 0 && <tr><td colSpan={4} className="py-6 text-center text-sub">{t.mailEmpty}</td></tr>}
            </tbody>
          </table>
        </>
      )}

      {/* 全站促销（freeleech） */}
      {tab === "promo" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{t.promoNew}</h2>
            <div className="cmgmt-form">
              <label>
                {t.promoKind}
                <select value={promoKind} onChange={(e) => setPromoKind(e.target.value)}>
                  <option value="free">Free 免费</option>
                  <option value="x2">2x 上传</option>
                  <option value="x2free">2x 免费</option>
                  <option value="half">50% 下载</option>
                  <option value="x2half">2x 50%</option>
                  <option value="p30">30% 下载</option>
                </select>
              </label>
              <label>{t.promoHours}<input type="number" min={1} max={720} value={promoHours} onChange={(e) => setPromoHours(Number(e.target.value))} /></label>
              <div className="flex gap-2">
                <button className="baozi-button" disabled={busy || promoHours < 1}
                  onClick={() => guard(async () => {
                    await api.post("/api/v1/admin/freeleech", { kind: promoKind, hours: promoHours });
                  }, t.promoSet)}>{t.promoBtnSet}</button>
                <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold" disabled={busy || !promo}
                  onClick={() => guard(async () => { await api.del("/api/v1/admin/freeleech"); }, t.promoCleared)}>{t.promoBtnClear}</button>
              </div>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr><td className="colhead">{t.promoKind}</td><td className="colhead">{t.promoStart}</td><td className="colhead">{t.promoEnd}</td></tr>
              {promo ? (
                <tr>
                  <td className="font-bold">{promo.kind}</td>
                  <td className="text-xs text-sub">{new Date(promo.starts_at).toLocaleString("zh-CN")}</td>
                  <td className="text-xs text-sub">{new Date(promo.ends_at).toLocaleString("zh-CN")}</td>
                </tr>
              ) : (
                <tr><td colSpan={3} className="py-6 text-center text-sub">{t.promoNone}</td></tr>
              )}
            </tbody>
          </table>
        </>
      )}

      {/* 批量私信（staffmess） */}
      {tab === "staffmess" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.smNew}</h2>
          <div className="cmgmt-form">
            <label>{t.fldSubject}<input value={smSubject} onChange={(e) => setSmSubject(e.target.value)} /></label>
            <label>{t.fldBody}<textarea rows={5} value={smBody} onChange={(e) => setSmBody(e.target.value)} /></label>
            <label>
              {t.smMinClass}
              <select value={smMinClass} onChange={(e) => setSmMinClass(e.target.value)}>
                <option value="">{t.smAllUsers}</option>
                <option value="10">Power User+</option>
                <option value="50">Elite+</option>
                <option value="90">管理组</option>
              </select>
            </label>
            <button className="baozi-button self-start" disabled={busy || !smSubject.trim() || !smBody.trim()}
              onClick={() => guard(async () => {
                await api.post("/api/v1/admin/staffmess", {
                  subject: smSubject, body: smBody,
                  min_class: smMinClass ? Number(smMinClass) : null,
                });
                setSmSubject(""); setSmBody("");
              }, t.smSent)}>
              {t.btnSend}
            </button>
          </div>
        </section>
      )}

      {/* 添加用户（adduser） */}
      {tab === "adduser" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.auNew}</h2>
          <div className="cmgmt-form">
            <label>{t.auUsername}<input value={auName} onChange={(e) => setAuName(e.target.value)} /></label>
            <label>{t.auEmail}<input type="email" value={auEmail} onChange={(e) => setAuEmail(e.target.value)} /></label>
            <label>{t.auPassword}<input type="password" value={auPass} onChange={(e) => setAuPass(e.target.value)} /></label>
            <button className="baozi-button self-start" disabled={busy || !auName.trim() || !auEmail.includes("@") || auPass.length < 8}
              onClick={() => guard(async () => {
                await api.post("/api/v1/admin/adduser", { username: auName, email: auEmail, password: auPass });
                setAuName(""); setAuEmail(""); setAuPass("");
              }, t.auCreated)}>
              {t.auBtnCreate}
            </button>
            <p className="text-xs text-sub">{t.auNote}</p>
          </div>
        </section>
      )}

      {/* 增加魔力（amountbonus） */}
      {tab === "bonus" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.bonusNew}</h2>
          <div className="cmgmt-form">
            <label>{t.bonusAmount}<input value={bonusAmount} onChange={(e) => setBonusAmount(e.target.value)} placeholder="1000 / -500" /></label>
            <label>{t.bonusUser}<input value={bonusUser} onChange={(e) => setBonusUser(e.target.value)} placeholder={t.bonusUserPh} /></label>
            <button className="baozi-button self-start" disabled={busy || !bonusAmount.trim() || Number.isNaN(Number(bonusAmount))}
              onClick={() => guard(async () => {
                await api.post("/api/v1/admin/amountbonus", {
                  amount: Number(bonusAmount),
                  user_id: bonusUser.trim() ? Number(bonusUser) : null,
                });
              }, t.bonusDone)}>
              {t.bonusBtn}
            </button>
            <p className="text-xs text-sub">{t.bonusNote}</p>
          </div>
        </section>
      )}

      {/* 警告用户（warned） */}
      {tab === "warned" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{t.warnNew}</h2>
            <div className="cmgmt-form">
              <label>{t.warnUserId}<input value={warnUser} onChange={(e) => setWarnUser(e.target.value)} placeholder="4" /></label>
              <label>{t.warnWeeks}<input type="number" min={1} max={52} value={warnWeeks} onChange={(e) => setWarnWeeks(e.target.value)} /></label>
              <label>{t.fldReason}<input value={warnReason} onChange={(e) => setWarnReason(e.target.value)} /></label>
              <button className="baozi-button self-start" disabled={busy || !warnUser.trim()}
                onClick={() => guard(async () => {
                  await api.post("/api/v1/admin/warned", {
                    user_id: Number(warnUser), weeks: Number(warnWeeks), reason: warnReason || null,
                  });
                }, t.warnDone)}>
                {t.warnBtn}
              </button>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr><td className="colhead">ID</td><td className="colhead">{t.banBy === "操作人" ? "用户" : t.banBy}</td><td className="colhead">{t.warnUntil}</td><td className="colhead">{t.fldReason}</td><td className="colhead text-right">{dict.cmgmt.colActions}</td></tr>
              {warned.map((w) => (
                <tr key={w.id}>
                  <td className="num">{w.id}</td>
                  <td>{w.username}</td>
                  <td className="text-xs text-sub">{w.warned_until ? new Date(w.warned_until).toLocaleString("zh-CN") : "—"}</td>
                  <td>{w.warned_reason ?? "—"}</td>
                  <td className="text-right">
                    <button className="cmgmt-act cmgmt-act--ok"
                      onClick={() => guard(async () => { await api.del(`/api/v1/admin/warned/${w.id}`); }, t.unwarned)}>{t.unwarnBtn}</button>
                  </td>
                </tr>
              ))}
              {warned.length === 0 && <tr><td colSpan={5} className="py-6 text-center text-sub">{t.warnEmpty}</td></tr>}
            </tbody>
          </table>
        </>
      )}

      {/* 重复 IP 检测（ipcheck） */}
      {tab === "ipcheck" && (
        <table className="nexus-table">
          <tbody>
            <tr><td className="colhead">{t.fldIp}</td><td className="colhead">{t.ipAccounts}</td><td className="colhead">{t.ipUsers}</td><td className="colhead">{t.ipLastSeen}</td></tr>
            {ipRows.map((r, i) => (
              <tr key={i}>
                <td className="font-mono">{r.ip}</td>
                <td className="num">{r.users}</td>
                <td>{r.usernames ?? "—"}</td>
                <td className="text-xs text-sub">{r.last_seen ? new Date(r.last_seen).toLocaleString("zh-CN") : "—"}</td>
              </tr>
            ))}
            {ipRows.length === 0 && <tr><td colSpan={4} className="py-6 text-center text-sub">{t.ipEmpty}</td></tr>}
          </tbody>
        </table>
      )}

      {/* 失败登录（maxlogin） */}
      {tab === "maxlogin" && (
        <table className="nexus-table">
          <tbody>
            <tr><td className="colhead">{t.mlUser}</td><td className="colhead">{t.fldIp}</td><td className="colhead">{t.mailAt}</td></tr>
            {failRows.map((r) => (
              <tr key={r.id}>
                <td>{r.username ?? "（未知用户）"}</td>
                <td className="font-mono">{r.ip ?? "—"}</td>
                <td className="text-xs text-sub">{new Date(r.created_at).toLocaleString("zh-CN")}</td>
              </tr>
            ))}
            {failRows.length === 0 && <tr><td colSpan={3} className="py-6 text-center text-sub">{t.mlEmpty}</td></tr>}
          </tbody>
        </table>
      )}

      {/* 增加上传（amountupload） */}
      {tab === "upload" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.upNew}</h2>
          <div className="cmgmt-form">
            <label>{t.upBytes}<input value={upBytes} onChange={(e) => setUpBytes(e.target.value)} placeholder="10737418240（字节，可负）" /></label>
            <label>{t.bonusUser}<input value={upUser} onChange={(e) => setUpUser(e.target.value)} placeholder={t.bonusUserPh} /></label>
            <button className="baozi-button self-start" disabled={busy || !upBytes.trim() || Number.isNaN(Number(upBytes))}
              onClick={() => guard(async () => {
                await api.post("/api/v1/admin/amountupload", {
                  bytes: Number(upBytes),
                  user_id: upUser.trim() ? Number(upUser) : null,
                });
              }, t.upDone)}>
              {t.upBtn}
            </button>
            <p className="text-xs text-sub">{t.upNote}</p>
          </div>
        </section>
      )}

      {/* 重置用户密码（reset） */}
      {tab === "resetpass" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.rpNew}</h2>
          <div className="cmgmt-form">
            <label>{t.warnUserId}<input value={resetId} onChange={(e) => setResetId(e.target.value)} placeholder="4" /></label>
            <button className="baozi-button self-start" disabled={busy || !resetId.trim()}
              onClick={() => guard(async () => {
                const r = await api.post<{ temp_password: string }>("/api/v1/admin/resetpass", { user_id: Number(resetId) });
                setResetPass(r.temp_password);
              }, t.rpDone)}>
              {t.rpBtn}
            </button>
            <p className="text-xs text-sub">{t.rpNote}</p>
          </div>
          {resetPass && (
            <p className="mt-3 rounded-[var(--r-md)] bg-sky-soft p-3 font-mono text-sm text-ink">
              {t.rpTemp}: {resetPass}
            </p>
          )}
        </section>
      )}

      {/* 删除被禁用户（deletedisabled） */}
      {tab === "deldisabled" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.ddTitle}</h2>
          <p className="mb-3 text-xs text-sub">{t.ddNote}</p>
          <button className="min-h-[40px] rounded-full bg-[var(--baozi-orange-dark)] px-5 text-sm font-bold text-white disabled:opacity-50" disabled={busy}
            onClick={() => {
              if (!window.confirm(t.ddConfirm)) return;
              void guard(async () => {
                const r = await api.post<{ deleted: number }>("/api/v1/admin/deletedisabled");
                flash(t.ddDone.replace("{n}", String(r.deleted)));
              }, "");
            }}>
            {t.ddBtn}
          </button>
        </section>
      )}

      {/* 邮箱黑白名单（bannedemails/allowedemails） */}
      {tab === "emailbans" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{t.ebNew}</h2>
            <div className="cmgmt-form">
              <label>{t.ebPattern}<input value={ebPattern} onChange={(e) => setEbPattern(e.target.value)} placeholder="@spam.example / user@ / a@b.com" /></label>
              <label>
                {t.ebMode}
                <select value={ebMode} onChange={(e) => setEbMode(e.target.value)}>
                  <option value="ban">{t.ebModeBan}</option>
                  <option value="allow">{t.ebModeAllow}</option>
                </select>
              </label>
              <label>{t.ebNote}<input value={ebNote} onChange={(e) => setEbNote(e.target.value)} /></label>
              <button className="baozi-button self-start" disabled={busy || !ebPattern.trim()}
                onClick={() => guard(async () => {
                  await api.post("/api/v1/admin/emailbans", { pattern: ebPattern, mode: ebMode, note: ebNote || null });
                  setEbPattern(""); setEbNote("");
                }, t.saved)}>{t.btnSave}</button>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr><td className="colhead">{t.ebPattern}</td><td className="colhead">{t.ebMode}</td><td className="colhead">{t.ebNote}</td><td className="colhead text-right">{dict.cmgmt.colActions}</td></tr>
              {emailBans.map((e) => (
                <tr key={e.id}>
                  <td className="font-mono">{e.pattern}</td>
                  <td>{e.mode === "ban" ? t.ebModeBan : t.ebModeAllow}</td>
                  <td>{e.note ?? "—"}</td>
                  <td className="text-right">
                    <button className="cmgmt-act cmgmt-act--danger"
                      onClick={() => guard(async () => { await api.del(`/api/v1/admin/emailbans/${e.id}`); }, t.deleted)}>{dict.cmgmt.btnDelete}</button>
                  </td>
                </tr>
              ))}
              {emailBans.length === 0 && <tr><td colSpan={4} className="py-6 text-center text-sub">{t.ebEmpty}</td></tr>}
            </tbody>
          </table>
        </>
      )}

      {/* IP 测试（testip） */}
      {tab === "testip" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.tiTitle}</h2>
          <div className="cmgmt-form">
            <label>{t.fldIp}<input value={testIpQuery} onChange={(e) => setTestIpQuery(e.target.value)} placeholder="203.0.113.10" /></label>
            <button className="baozi-button self-start" disabled={busy || !testIpQuery.trim()}
              onClick={() => void (async () => {
                setBusy(true);
                try {
                  const r = await api.get<TestIpResult>(`/api/v1/admin/testip?ip=${encodeURIComponent(testIpQuery.trim())}`);
                  setTestIpResult(r);
                } catch (e) {
                  flash(e instanceof ApiError ? e.message : dict.common.networkError);
                } finally { setBusy(false); }
              })()}>
              {t.tiBtn}
            </button>
          </div>
          {testIpResult && (
            <table className="nexus-table mt-3">
              <tbody>
                <tr><td className="rowhead">{t.fldIp}</td><td className="rowfollow font-mono">{testIpResult.ip}</td></tr>
                <tr><td className="rowhead">{t.tiBanned}</td><td className="rowfollow">{testIpResult.banned ? `⛔ ${t.tiYes}` : `✅ ${t.tiNo}`}</td></tr>
                {testIpResult.banned && (
                  <>
                    <tr><td className="rowhead">{t.fldReason}</td><td className="rowfollow">{testIpResult.reason ?? "—"}</td></tr>
                    <tr><td className="rowhead">{t.banBy}</td><td className="rowfollow">{testIpResult.by ?? "—"}</td></tr>
                  </>
                )}
                <tr><td className="rowhead">{t.tiSeenUsers}</td><td className="rowfollow">{testIpResult.seen_users.length > 0 ? testIpResult.seen_users.join(", ") : "—"}</td></tr>
              </tbody>
            </table>
          )}
        </section>
      )}

      {/* 统计（stats） */}
      {tab === "stats" && stats && (
        <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
          {([
            [t.stUsers, stats.users], [t.stTorrents, stats.torrents], [t.stSeeding, stats.seeding],
            [t.stLeeching, stats.leeching], [t.stComments, stats.comments], [t.stMessages, stats.messages],
          ] as [string, number][]).map(([label, v]) => (
            <div key={label} className="baozi-panel p-4">
              <p className="text-xs text-sub">{label}</p>
              <p className="num text-2xl font-bold text-ink">{v.toLocaleString("zh-CN")}</p>
            </div>
          ))}
          <div className="baozi-panel p-4">
            <p className="text-xs text-sub">{t.stRedis}</p>
            <p className={`text-lg font-bold ${stats.redis === "up" ? "text-[#2fb26b]" : "text-[#e14d4d]"}`}>{stats.redis === "up" ? "✅ up" : "⛔ down"}</p>
          </div>
          <div className="baozi-panel p-4">
            <p className="text-xs text-sub">{t.stUptime}</p>
            <p className="num text-lg font-bold text-ink">{Math.floor(stats.uptime_secs / 3600)}h {Math.floor((stats.uptime_secs % 3600) / 60)}m</p>
          </div>
        </div>
      )}

      {/* 清除缓存 + 做清理（clearcache/docleanup） */}
      {tab === "cleanup" && (
        <>
          <section className="baozi-panel flex flex-col gap-3 p-4">
            <h2 className="text-base font-bold text-ink">{t.ccTitle}</h2>
            <p className="text-xs text-sub">{t.ccNote}</p>
            <button className="baozi-button self-start" disabled={busy}
              onClick={() => guard(async () => {
                const r = await api.post<{ cleared: number }>("/api/v1/admin/clearcache");
                flash(t.ccDone.replace("{n}", String(r.cleared)));
              }, "")}>{t.ccBtn}</button>
          </section>
          <section className="baozi-panel flex flex-col gap-3 p-4">
            <h2 className="text-base font-bold text-ink">{t.dcuTitle}</h2>
            <p className="text-xs text-sub">{t.dcuNote}</p>
            <button className="baozi-button self-start" disabled={busy}
              onClick={() => guard(async () => {
                const r = await api.post<CleanupResult>("/api/v1/admin/docleanup");
                setCleanupResult(r);
              }, t.dcuDone)}>{t.dcuBtn}</button>
            {cleanupResult && (
              <table className="nexus-table">
                <tbody>
                  <tr><td className="rowhead">{t.dcuPromos}</td><td className="rowfollow num">{cleanupResult.expired_promotions}</td></tr>
                  <tr><td className="rowhead">{t.dcuWarns}</td><td className="rowfollow num">{cleanupResult.expired_warnings}</td></tr>
                  <tr><td className="rowhead">{t.dcuLogins}</td><td className="rowfollow num">{cleanupResult.old_login_events}</td></tr>
                  <tr><td className="rowhead">{t.dcuResets}</td><td className="rowfollow num">{cleanupResult.old_password_resets}</td></tr>
                </tbody>
              </table>
            )}
          </section>
        </>
      )}

      {/* 广告管理（admanage） */}
      {tab === "ads" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{adEdit.id === null ? t.adsNew : t.adsEdit}</h2>
            <div className="cmgmt-form">
              <label>{dict.cmgmt.fldTitle}<input value={adEdit.title} onChange={(e) => setAdEdit({ ...adEdit, title: e.target.value })} /></label>
              <label>{t.adsHtml}<textarea rows={3} value={adEdit.html} onChange={(e) => setAdEdit({ ...adEdit, html: e.target.value })} /></label>
              <label>
                {t.adsPosition}
                <select value={adEdit.position} onChange={(e) => setAdEdit({ ...adEdit, position: e.target.value })}>
                  <option value="header">Header</option>
                  <option value="footer">Footer</option>
                  <option value="sidebar">Sidebar</option>
                </select>
              </label>
              <div className="flex gap-2">
                <button className="baozi-button" disabled={busy || !adEdit.title.trim() || !adEdit.html.trim()}
                  onClick={() => guard(async () => {
                    if (adEdit.id === null) await api.post("/api/v1/admin/ads", { title: adEdit.title, html: adEdit.html, position: adEdit.position });
                    else await api.put(`/api/v1/admin/ads/${adEdit.id}`, { title: adEdit.title, html: adEdit.html, position: adEdit.position });
                    setAdEdit({ id: null, title: "", html: "", position: "header" });
                  }, t.saved)}>{t.btnSave}</button>
                {adEdit.id !== null && (
                  <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
                    onClick={() => setAdEdit({ id: null, title: "", html: "", position: "header" })}>{dict.cmgmt.btnCancel}</button>
                )}
              </div>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr><td className="colhead">{dict.cmgmt.fldTitle}</td><td className="colhead">{t.adsPosition}</td><td className="colhead">{t.adsEnabled}</td><td className="colhead text-right">{dict.cmgmt.colActions}</td></tr>
              {ads.map((a) => (
                <tr key={a.id}>
                  <td>{a.title}</td>
                  <td className="text-xs">{a.position}</td>
                  <td>{a.enabled ? "✅" : "⛔"}</td>
                  <td className="text-right">
                    <button className="cmgmt-act" onClick={() => setAdEdit({ id: a.id, title: a.title, html: a.html, position: a.position })}>{dict.cmgmt.btnEdit}</button>
                    <button className="cmgmt-act" onClick={() => guard(async () => { await api.put(`/api/v1/admin/ads/${a.id}/toggle`); }, t.saved)}>{t.adsToggle}</button>
                    <button className="cmgmt-act cmgmt-act--danger" onClick={() => guard(async () => { await api.del(`/api/v1/admin/ads/${a.id}`); }, t.deleted)}>{dict.cmgmt.btnDelete}</button>
                  </td>
                </tr>
              ))}
              {ads.length === 0 && <tr><td colSpan={4} className="py-6 text-center text-sub">{t.adsEmpty}</td></tr>}
            </tbody>
          </table>
        </>
      )}

      {/* 无法连接的用户（notconnectable） */}
      {tab === "notconnect" && (
        <table className="nexus-table">
          <tbody>
            <tr><td className="colhead">ID</td><td className="colhead">{t.mlUser}</td><td className="colhead">{t.ncTorrents}</td><td className="colhead">{t.ncLastSeen}</td></tr>
            {notConnectRows.map((r) => (
              <tr key={r.id}>
                <td className="num">{r.id}</td>
                <td>{r.username}</td>
                <td className="num">{r.torrents}</td>
                <td className="text-xs text-sub">{r.last_seen_at ? new Date(r.last_seen_at).toLocaleString("zh-CN") : "—"}</td>
              </tr>
            ))}
            {notConnectRows.length === 0 && <tr><td colSpan={4} className="py-6 text-center text-sub">{t.ncEmpty}</td></tr>}
          </tbody>
        </table>
      )}

      {/* 上传者（uploaders） */}
      {tab === "uploaders" && (
        <table className="nexus-table">
          <tbody>
            <tr><td className="colhead">ID</td><td className="colhead">{t.mlUser}</td><td className="colhead">{t.ulpUploads}</td><td className="colhead">{t.stSeeding}</td><td className="colhead">{t.ulpSize}</td></tr>
            {uploaderRows.map((r) => (
              <tr key={r.id}>
                <td className="num">{r.id}</td>
                <td>{r.username}</td>
                <td className="num">{r.uploads}</td>
                <td className="num">{r.seeding}</td>
                <td className="num">{(r.total_size / 1024 ** 3).toFixed(2)} GB</td>
              </tr>
            ))}
            {uploaderRows.length === 0 && <tr><td colSpan={5} className="py-6 text-center text-sub">{t.ulpEmpty}</td></tr>}
          </tbody>
        </table>
      )}

      {/* 全部客户端（allagents） */}
      {tab === "agents" && (
        <table className="nexus-table">
          <tbody>
            <tr><td className="colhead">{t.agAgent}</td><td className="colhead">{t.agPeers}</td></tr>
            {agentRows.map((r) => (
              <tr key={r.agent}>
                <td className="font-mono">{r.agent}</td>
                <td className="num">{r.peers}</td>
              </tr>
            ))}
            {agentRows.length === 0 && <tr><td colSpan={2} className="py-6 text-center text-sub">{t.agEmpty}</td></tr>}
          </tbody>
        </table>
      )}

      {/* 投票总览（polloverview） */}
      {tab === "polls" && (
        <table className="nexus-table">
          <tbody>
            <tr><td className="colhead">ID</td><td className="colhead">{t.plQuestion}</td><td className="colhead">{t.plVotes}</td><td className="colhead">{t.plStatus}</td></tr>
            {pollRows.map((p) => (
              <tr key={p.id}>
                <td className="num">{p.id}</td>
                <td>{p.question}</td>
                <td className="num">{p.votes}</td>
                <td>{p.closed ? t.plClosed : t.plOpen}</td>
              </tr>
            ))}
            {pollRows.length === 0 && <tr><td colSpan={4} className="py-6 text-center text-sub">{t.plEmpty}</td></tr>}
          </tbody>
        </table>
      )}
      {/* 数据库状态（mysql_stats → PostgreSQL） */}
      {tab === "dbstats" && dbStats && (
        <>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
            <div className="baozi-panel p-4">
              <p className="text-xs text-sub">{t.dsEngine}</p>
              <p className="text-base font-bold text-ink">{dbStats.engine} · {dbStats.database}</p>
            </div>
            <div className="baozi-panel p-4">
              <p className="text-xs text-sub">{t.dsConns}</p>
              <p className="num text-2xl font-bold text-ink">{dbStats.total_connections}</p>
            </div>
            <div className="baozi-panel p-4">
              <p className="text-xs text-sub">{t.dsSize}</p>
              <p className="num text-lg font-bold text-ink">{(dbStats.database_size / 1024 ** 2).toFixed(1)} MB</p>
            </div>
            <div className="baozi-panel p-4">
              <p className="text-xs text-sub">{t.dsSlow}</p>
              <p className={`num text-2xl font-bold ${dbStats.slow_transactions > 0 ? "text-[#e14d4d]" : "text-[#2fb26b]"}`}>{dbStats.slow_transactions}</p>
            </div>
          </div>
          <table className="nexus-table">
            <tbody>
              <tr><td className="colhead">{t.dsState}</td><td className="colhead">{t.dsCount}</td></tr>
              {dbStats.connections.map((c) => (
                <tr key={c.state}><td className="font-mono text-xs">{c.state}</td><td className="num">{c.count}</td></tr>
              ))}
              {dbStats.connections.length === 0 && <tr><td colSpan={2} className="py-6 text-center text-sub">{t.dsEmpty}</td></tr>}
            </tbody>
          </table>
          <table className="nexus-table">
            <tbody>
              <tr><td className="colhead">{t.dsTable}</td><td className="colhead">{t.dsRows}</td><td className="colhead text-right">{t.dsTableSize}</td></tr>
              {dbStats.tables.map((tb) => (
                <tr key={tb.relname}>
                  <td className="font-mono text-xs">{tb.relname}</td>
                  <td className="num">{tb.row_estimates}</td>
                  <td className="num text-right">{(tb.total_size / 1024 ** 2).toFixed(2)} MB</td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="text-xs text-sub">{t.dsDead.replace("{n}", dbStats.dead_tuples.toLocaleString("zh-CN"))}</p>
        </>
      )}

      {/* 系统日志（bitbucketlog → 审计日志） */}
      {tab === "syslog" && (
        <>
          <section className="baozi-panel p-4">
            <div className="cmgmt-form">
              <label>{t.slSearch}<input value={logQ} onChange={(e) => { setLogQ(e.target.value); setLogPage(1); }} placeholder="massmail / warn_user / freeleech" /></label>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr><td className="colhead">ID</td><td className="colhead">{t.slActor}</td><td className="colhead">{t.slAction}</td><td className="colhead">{t.fldIp}</td><td className="colhead">{t.mailAt}</td></tr>
              {logData?.items.map((r) => (
                <tr key={r.id}>
                  <td className="num">{r.id}</td>
                  <td>{r.actor ?? "system"}</td>
                  <td className="font-mono text-xs">{r.action}</td>
                  <td className="font-mono text-xs">{r.ip ?? "—"}</td>
                  <td className="text-xs text-sub">{new Date(r.created_at).toLocaleString("zh-CN")}</td>
                </tr>
              ))}
              {(!logData || logData.items.length === 0) && <tr><td colSpan={5} className="py-6 text-center text-sub">{t.slEmpty}</td></tr>}
            </tbody>
          </table>
          {logData && logData.pages > 1 && (
            <div className="flex items-center justify-between">
              <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-40"
                disabled={logPage <= 1} onClick={() => setLogPage((p) => p - 1)}>{dict.common.nextPage}</button>
              <span className="text-xs text-sub">{logData.page} / {logData.pages}（{logData.total}）</span>
              <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-40"
                disabled={logPage >= logData.pages} onClick={() => setLogPage((p) => p + 1)}>{dict.common.nextPage}</button>
            </div>
          )}
        </>
      )}

      {/* 位置管理（location → IP 网段视图） */}
      {tab === "locations" && (
        <>
          <p className="text-xs text-sub">{t.loNote}</p>
          <table className="nexus-table">
            <tbody>
              <tr><td className="colhead">{t.loNet}</td><td className="colhead">{t.loLogins}</td><td className="colhead">{t.ipAccounts}</td><td className="colhead">{t.loFailed}</td><td className="colhead">{t.ipLastSeen}</td></tr>
              {locData?.items.map((r) => (
                <tr key={`${r.net}/${r.netmask}`}>
                  <td className="font-mono">{r.net}/{r.netmask}</td>
                  <td className="num">{r.logins}</td>
                  <td className="num">{r.users}</td>
                  <td className="num">{r.failed > 0 ? <span className="text-[#e14d4d]">{r.failed}</span> : 0}</td>
                  <td className="text-xs text-sub">{r.last_seen ? new Date(r.last_seen).toLocaleString("zh-CN") : "—"}</td>
                </tr>
              ))}
              {(!locData || locData.items.length === 0) && <tr><td colSpan={5} className="py-6 text-center text-sub">{t.loEmpty}</td></tr>}
            </tbody>
          </table>
          {locData && locData.pages > 1 && (
            <div className="flex items-center justify-between">
              <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-40"
                disabled={locPage <= 1} onClick={() => setLocPage((p) => p - 1)}>‹</button>
              <span className="text-xs text-sub">{locData.page} / {locData.pages}（{locData.total}）</span>
              <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-40"
                disabled={locPage >= locData.pages} onClick={() => setLocPage((p) => p + 1)}>›</button>
            </div>
          )}
        </>
      )}
      {/* H&R 赦免（hr/pardon） */}
      {tab === "hrpardon" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.hpTitle}</h2>
          <div className="cmgmt-form">
            <label>{t.hpUser}<input value={hpUser} onChange={(e) => setHpUser(e.target.value.replace(/\D/g, ""))} placeholder="4" /></label>
            <label>{t.hpTorrent}<input value={hpTorrent} onChange={(e) => setHpTorrent(e.target.value.replace(/\D/g, ""))} placeholder="12" /></label>
            <label>{t.fldReason}<input value={hpNote} onChange={(e) => setHpNote(e.target.value)} /></label>
            <button className="baozi-button self-start" disabled={busy || !hpUser || !hpTorrent || !hpNote.trim()}
              onClick={() => guard(async () => {
                await api.post("/api/v1/admin/hr/pardon", {
                  user_id: Number(hpUser), torrent_id: Number(hpTorrent), note: hpNote,
                });
                setHpUser(""); setHpTorrent(""); setHpNote("");
              }, t.hpDone)}>
              {t.hpBtn}
            </button>
            <p className="text-xs text-sub">{t.hpNote}</p>
          </div>
        </section>
      )}
      {/* 插件清单（M28 只读：启停由插件配置决定） */}
      {tab === "plugins" && (
        <section className="baozi-panel p-4">
          <h2 className="mb-3 text-base font-bold text-ink">{t.pluginsTitle ?? "插件清单"}</h2>
          {pluginList === null ? (
            <button className="baozi-button" onClick={async () => {
              try {
                const r = await api.get<{ plugins: string[] }>("/api/v1/admin/plugins");
                setPluginList(r.plugins);
              } catch { setPluginList([]); }
            }}>{t.pluginsLoad ?? "加载"}</button>
          ) : (
            <ul className="flex flex-col gap-1 text-sm">
              {pluginList.map((pl) => (
                <li key={pl} className="flex items-center gap-2">
                  <span className="fun-status fun-status--normal">on</span>
                  <code className="text-xs">{pl}</code>
                </li>
              ))}
              {pluginList.length === 0 && <li className="text-sub">{t.pluginsEmpty ?? "无插件"}</li>}
            </ul>
          )}
          <p className="mt-2 text-xs text-sub">{t.pluginsNote ?? "插件启停由服务端配置决定，此处为只读清单"}</p>
        </section>
      )}
    </div>
  );
}
