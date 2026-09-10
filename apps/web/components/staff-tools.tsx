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

type ToolTab = "faq" | "rules" | "cats" | "bans" | "mail" | "promo" | "staffmess" | "adduser" | "bonus" | "warned" | "ipcheck" | "maxlogin";

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
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.loadFailed);
    }
  }, [dict]);
  useEffect(() => { load(); }, [load]);

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
    </div>
  );
}
