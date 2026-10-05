"use client";

import { INPUT_CLOUD } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { kindsOf } from "./increment-bulk-kinds";
import { BulkNotifyRow } from "./increment-bulk-notify";

/** 第八轮 0065：批量发放（好学 increment-bulk.php 口径）
 *  合并原「魔力增减 / 上传量增减」两个页签：火花/上传量/邀请/补签卡
 *  × 等级多选 / 职务多选 / 指定用户，支持负数（减）与 PM 通知。 */

interface RoleDef { key: string; name: string }
interface BulkResult {
  affected: number;
  targets: number;
  kind: string;
  amount: number;
  batch_id?: string;
  target_ids?: number[];
}
interface MedalDef { id: number; name: string }
interface ItemDef { id: number; name: string; kind: string }

export function IncrementBulk() {
  const { dict, currency } = useI18n();
  const t = dict.adminBulk;
  const KINDS = kindsOf(currency, t);
  const classList = (dict.admin as unknown as { classList: [number, string][] }).classList;
  const [kind, setKind] = useState("spark");
  const [amount, setAmount] = useState("100");
  const [days, setDays] = useState(""); // 临时邀请：N 天有效直发邀请码（kind=invite 时可选）
  const [medalId, setMedalId] = useState(""); // kind=medal（0204）
  const [itemId, setItemId] = useState(""); // kind=item（0204）
  const [medals, setMedals] = useState<MedalDef[]>([]);
  const [items, setItems] = useState<ItemDef[]>([]);
  const [classes, setClasses] = useState<Set<number>>(new Set());
  const [roles, setRoles] = useState<string[]>([]);
  const [roleDefs, setRoleDefs] = useState<RoleDef[]>([]);
  const [userIds, setUserIds] = useState("");
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [sender, setSender] = useState("self");
  const [email, setEmail] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = useCallback((m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 5000);
  }, []);

  useEffect(() => {
    api.get<RoleDef[]>("/api/v1/admin/roles").then(setRoleDefs).catch(() => setRoleDefs([]));
    // 勋章/道具下拉（0204）：懒加载；/medals 已信封化（双形状兼容）
    api.get<MedalDef[] | { items: MedalDef[] }>("/api/v1/medals")
      .then((r) => setMedals(Array.isArray(r) ? r : ((r as { items?: MedalDef[] }).items ?? [])))
      .catch(() => setMedals([]));
    api.get<ItemDef[]>("/api/v1/shop/items").then(setItems).catch(() => setItems([]));
  }, []);

  const kindHint = KINDS.find(([k]) => k === kind)?.[2] ?? "";

  function buildPayload(): Record<string, unknown> {
    const payload: Record<string, unknown> = {
      kind,
      amount: Number(amount),
      classes: [...classes],
      roles,
      user_ids: userIds.split(/[,，\s]+/).map(Number).filter((n) => n > 0),
    };
    if (days.trim()) payload.days = Number(days);
    if (kind === "medal" && medalId) payload.medal_id = Number(medalId);
    if (kind === "item" && itemId) payload.item_id = Number(itemId);
    if (subject.trim()) payload.subject = subject.trim();
    if (body.trim()) payload.body = body.trim();
    payload.sender = sender;
    if (email) payload.email = true;
    return payload;
  }

  // 提交可用性（0286）：两个按钮共用——数量/勋章/道具/受众齐备
  const canSubmit =
    !!amount &&
    Number(amount) !== 0 &&
    !(kind === "medal" && !medalId) &&
    !(kind === "item" && !itemId) &&
    (classes.size > 0 || roles.length > 0 || !!userIds.trim());

  /** dry（0286）：true = 试运行（只返回命中清单不发） */
  async function submit(dry: boolean) {
    if (!dry && !window.confirm(t.confirmRun)) return;
    setBusy(true);
    try {
      const r = await api.post<BulkResult>("/api/v1/admin/increment-bulk", {
        ...buildPayload(),
        ...(dry ? { dry_run: true } : {}),
      });
      if (dry) {
        flash(t.dryOk.replace("{n}", String(r.targets)).replace("{ids}", (r.target_ids ?? []).join(", ")));
        return;
      }
      const label = KINDS.find(([k]) => k === kind)?.[1] ?? kind;
      flash(
        t.done
          .replace("{kind}", label)
          .replace("{amount}", amount)
          .replace("{n}", String(r.targets))
          .replace("{m}", String(r.affected))
          .replace("{pm}", subject.trim() ? t.donePm : ""),
      );
      setClasses(new Set());
      setRoles([]);
      setUserIds("");
      setSubject("");
      setBody("");
    } catch (e) {
      flash(e instanceof ApiError ? e.message : t.fail);
    } finally {
      setBusy(false);
    }
  }

  const inp = INPUT_CLOUD;
  const picked = userIds.trim() ? userIds.split(/[,，\s]+/).filter(Boolean).length : 0;

  return (
    <div className="flex flex-col gap-3">
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}
      <section className="baozi-panel p-4">
        <h2 className="mb-1 text-base font-bold">{t.title}</h2>
        <p className="mb-3 text-xs text-sub">{t.intro}</p>
        <div className="baozi-wide-table-scroll">
        <table className="nexus-table nexus-form">
          <tbody>
            <tr>
              <td className="rowhead w-28">{t.kind}</td>
              <td className="rowfollow">
                <div className="flex flex-wrap gap-3">
                  {KINDS.map(([k, label]) => (
                    <label key={k} className="flex cursor-pointer items-center gap-1.5 text-sm">
                      <input type="radio" name="bulk-kind" checked={kind === k} onChange={() => setKind(k)} className="accent-sky" />
                      {label}
                    </label>
                  ))}
                </div>
                <p className="mt-1 text-xs text-sub">{kindHint}</p>
              </td>
            </tr>
            <tr>
              <td className="rowhead">{t.amount}</td>
              <td className="rowfollow">
                <input type="number" value={amount} onChange={(e) => setAmount(e.target.value)} className={`${inp} w-32`} />
                <span className="ml-2 text-xs text-sub">{t.amountHint}</span>
              </td>
            </tr>
            {kind === "invite" && (
              <tr>
                <td className="rowhead">{t.tempInvite}</td>
                <td className="rowfollow">
                  <input type="number" min={1} max={365} value={days} onChange={(e) => setDays(e.target.value)} className={`${inp} w-24`} />
                  <span className="ml-2 text-xs text-sub">{t.tempInviteHint}</span>
                </td>
              </tr>
            )}
            {kind === "medal" && (
              <tr>
                <td className="rowhead">{t.medalLabel}</td>
                <td className="rowfollow">
                  <select value={medalId} onChange={(e) => setMedalId(e.target.value)} className={`${inp} w-64`}>
                    <option value="">{t.medalPick}</option>
                    {medals.map((m) => (
                      <option key={m.id} value={m.id}>#{m.id} {m.name}</option>
                    ))}
                  </select>
                  <span className="ml-2 text-xs text-sub">{t.medalHint}</span>
                </td>
              </tr>
            )}
            {kind === "item" && (
              <tr>
                <td className="rowhead">{t.itemLabel}</td>
                <td className="rowfollow">
                  <select value={itemId} onChange={(e) => setItemId(e.target.value)} className={`${inp} w-64`}>
                    <option value="">{t.itemPick}</option>
                    {items.map((it) => (
                      <option key={it.id} value={it.id}>#{it.id} {it.name}</option>
                    ))}
                  </select>
                  <span className="ml-2 text-xs text-sub">{t.itemHint}</span>
                </td>
              </tr>
            )}
            <tr>
              <td className="rowhead align-top">{t.classes}</td>
              <td className="rowfollow">
                <div className="grid grid-cols-2 gap-1 md:grid-cols-4">
                  {classList.filter(([id]) => id < 90).map(([id, label]) => (
                    <label key={id} className="flex items-center gap-1.5 text-sm">
                      <input
                        type="checkbox"
                        checked={classes.has(id)}
                        onChange={(e) => setClasses((prev) => {
                          const n = new Set(prev);
                          if (e.target.checked) n.add(id); else n.delete(id);
                          return n;
                        })}
                      />
                      {label}
                    </label>
                  ))}
                </div>
              </td>
            </tr>
            {roleDefs.length > 0 && (
              <tr>
                <td className="rowhead align-top">{t.roles}</td>
                <td className="rowfollow">
                  <div className="flex flex-wrap gap-3">
                    {roleDefs.map((r) => (
                      <label key={r.key} className="flex items-center gap-1.5 text-sm">
                        <input
                          type="checkbox"
                          checked={roles.includes(r.key)}
                          onChange={(e) => setRoles((prev) =>
                            e.target.checked ? [...prev, r.key] : prev.filter((k) => k !== r.key),
                          )}
                        />
                        {r.name}
                      </label>
                    ))}
                  </div>
                </td>
              </tr>
            )}
            <tr>
              <td className="rowhead">{t.userIds}</td>
              <td className="rowfollow">
                <input value={userIds} onChange={(e) => setUserIds(e.target.value)} placeholder={t.userIdsPh} className={`${inp} w-96`} />
              </td>
            </tr>
            <tr>
              <td className="rowhead">{t.pmSubject}</td>
              <td className="rowfollow">
                <input value={subject} onChange={(e) => setSubject(e.target.value)} placeholder={t.pmSubjectPh} className={`${inp} w-96`} />
              </td>
            </tr>
            <tr>
              <td className="rowhead align-top">{t.pmBody}</td>
              <td className="rowfollow">
                <textarea value={body} onChange={(e) => setBody(e.target.value)} rows={4} className={`${inp} w-96`} placeholder={t.pmBodyPh} />
              </td>
            </tr>
            <BulkNotifyRow t={t} email={email} setEmail={setEmail} />
            <tr>
              <td className="rowhead">{t.operator}</td>
              <td className="rowfollow">
                <label className="mr-4 inline-flex items-center gap-1.5 text-sm">
                  <input type="radio" name="bulk-sender" checked={sender === "self"} onChange={() => setSender("self")} />{t.asSelf}
                </label>
                <label className="inline-flex items-center gap-1.5 text-sm">
                  <input type="radio" name="bulk-sender" checked={sender === "system"} onChange={() => setSender("system")} />{t.asSystem}
                </label>
              </td>
            </tr>
            <tr>
              <td className="rowfollow" colSpan={2}>
                <div className="flex flex-wrap items-center gap-3">
                  <button
                    disabled={busy || !canSubmit}
                    onClick={() => void submit(false)}
                    className="min-h-[38px] rounded-full bg-sky px-6 text-sm font-bold text-white disabled:opacity-50"
                  >
                    {busy ? t.running : t.submit}
                  </button>
                  {/* 试运行（0286） */}
                  <button
                    disabled={busy || !canSubmit}
                    onClick={() => void submit(true)}
                    className="min-h-[38px] rounded-full border border-sky px-5
                      text-sm font-bold text-sky-deep disabled:opacity-50"
                  >
                    {t.dryRun}
                  </button>
                  <span className="text-xs text-sub">
                    {t.selected.replace("{c}", String(classes.size)).replace("{r}", String(roles.length)).replace("{u}", String(picked))}
                  </span>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
        </div>
      </section>
    </div>
  );
}
