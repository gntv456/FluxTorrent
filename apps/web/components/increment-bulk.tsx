"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 第八轮 0065：批量发放（好学 increment-bulk.php 口径）
 *  合并原「魔力增减 / 上传量增减」两个页签：火花/上传量/邀请/补签卡
 *  × 等级多选 / 职务多选 / 指定用户，支持负数（减）与 PM 通知。 */

interface RoleDef { key: string; name: string }
interface BulkResult { affected: number; targets: number; kind: string; amount: number }

/** 货币名动态化：火花档标签跟随站点 currency_name（默认「魔力」） */
function kindsOf(currency: string): [string, string, string][] {
  return [
    ["spark", currency, "正加负减，单次 ±1,000,000"],
    ["uploaded", "上传量 (GB)", "正加负减，单次 ±10TB"],
    ["invite", "邀请", "正数增发 / 负数回收配额，单次 ±50；可填临时邀请天数直发 N 天码"],
    ["resub_card", "补签卡", "入背包待用户使用，单次 1-50"],
  ];
}

export function IncrementBulk() {
  const { dict, currency } = useI18n();
  const KINDS = kindsOf(currency);
  const classList = (dict.admin as unknown as { classList: [number, string][] }).classList;
  const [kind, setKind] = useState("spark");
  const [amount, setAmount] = useState("100");
  const [days, setDays] = useState(""); // 临时邀请：N 天有效直发邀请码（kind=invite 时可选）
  const [classes, setClasses] = useState<Set<number>>(new Set());
  const [roles, setRoles] = useState<string[]>([]);
  const [roleDefs, setRoleDefs] = useState<RoleDef[]>([]);
  const [userIds, setUserIds] = useState("");
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [sender, setSender] = useState("self");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => { setMsg(m); setTimeout(() => setMsg(null), 5000); };

  useEffect(() => {
    api.get<RoleDef[]>("/api/v1/admin/roles").then(setRoleDefs).catch(() => setRoleDefs([]));
  }, []);

  const kindHint = KINDS.find(([k]) => k === kind)?.[2] ?? "";

  async function run() {
    if (!window.confirm(`确认执行批量发放？将影响所选用户，不可撤销。`)) return;
    setBusy(true);
    try {
      const payload: Record<string, unknown> = {
        kind,
        amount: Number(amount),
        classes: [...classes],
        roles,
        user_ids: userIds.split(/[,，\s]+/).map(Number).filter((n) => n > 0),
      };
      if (days.trim()) payload.days = Number(days);
      if (subject.trim()) payload.subject = subject.trim();
      if (body.trim()) payload.body = body.trim();
      payload.sender = sender;
      const r = await api.post<BulkResult>("/api/v1/admin/increment-bulk", payload);
      flash(`已发放：${KINDS.find(([k]) => k === kind)?.[1]} ${amount} → ${r.targets} 个用户（${r.affected} 条生效${subject.trim() ? "，PM 已发送" : ""}）`);
      setClasses(new Set());
      setRoles([]);
      setUserIds("");
      setSubject("");
      setBody("");
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  const inp = "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky";

  return (
    <div className="flex flex-col gap-3">
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}
      <section className="baozi-panel p-4">
        <h2 className="mb-1 text-base font-bold">批量发放</h2>
        <p className="mb-3 text-xs text-sub">
          合并原「魔力增减 / 上传量增减」：按等级、职务或指定用户批量增减四类资源，完成后可群发 PM 通知（好学 increment-bulk 口径）。
        </p>
        <table className="nexus-table nexus-form">
          <tbody>
            <tr>
              <td className="rowhead w-28">类型</td>
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
              <td className="rowhead">数量</td>
              <td className="rowfollow">
                <input type="number" value={amount} onChange={(e) => setAmount(e.target.value)} className={`${inp} w-32`} />
                <span className="ml-2 text-xs text-sub">正数增加 / 负数减少（下限 0）</span>
              </td>
            </tr>
            {kind === "invite" && (
              <tr>
                <td className="rowhead">临时邀请</td>
                <td className="rowfollow">
                  <input type="number" min={1} max={365} value={days} onChange={(e) => setDays(e.target.value)} placeholder="留空" className={`${inp} w-24`} />
                  <span className="ml-2 text-xs text-sub">
                    填有效期天数（1-365）＝好学「临时邀请」：直接生成 N 天到期的邀请码；留空则按普通邀请加/回收配额
                  </span>
                </td>
              </tr>
            )}
            <tr>
              <td className="rowhead align-top">用户等级</td>
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
                <td className="rowhead align-top">职务</td>
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
              <td className="rowhead">指定用户</td>
              <td className="rowfollow">
                <input value={userIds} onChange={(e) => setUserIds(e.target.value)} placeholder="UID 逗号/空格分隔（优先生效，单批 ≤500）" className={`${inp} w-96`} />
              </td>
            </tr>
            <tr>
              <td className="rowhead">私信主题</td>
              <td className="rowfollow">
                <input value={subject} onChange={(e) => setSubject(e.target.value)} placeholder="留空则不发 PM" className={`${inp} w-96`} />
              </td>
            </tr>
            <tr>
              <td className="rowhead align-top">私信内容</td>
              <td className="rowfollow">
                <textarea value={body} onChange={(e) => setBody(e.target.value)} rows={4} className={`${inp} w-96`} placeholder="支持说明发放原因与数量" />
              </td>
            </tr>
            <tr>
              <td className="rowhead">操作者</td>
              <td className="rowfollow">
                <label className="mr-4 inline-flex items-center gap-1.5 text-sm">
                  <input type="radio" name="bulk-sender" checked={sender === "self"} onChange={() => setSender("self")} />以我的名义
                </label>
                <label className="inline-flex items-center gap-1.5 text-sm">
                  <input type="radio" name="bulk-sender" checked={sender === "system"} onChange={() => setSender("system")} />System 系统私信
                </label>
              </td>
            </tr>
            <tr>
              <td className="rowfollow" colSpan={2}>
                <div className="flex flex-wrap items-center gap-3">
                  <button
                    disabled={busy || !amount || Number(amount) === 0 || (classes.size === 0 && roles.length === 0 && !userIds.trim())}
                    onClick={run}
                    className="min-h-[38px] rounded-full bg-sky px-6 text-sm font-bold text-white disabled:opacity-50"
                  >
                    {busy ? "执行中…" : "提交"}
                  </button>
                  <span className="text-xs text-sub">
                    已选：等级 {classes.size} 项 · 职务 {roles.length} 项 · 指定 {userIds.trim() ? userIds.split(/[,，\s]+/).filter(Boolean).length : 0} 人
                  </span>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </section>
    </div>
  );
}
