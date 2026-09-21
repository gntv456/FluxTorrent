"use client";

/**
 * 管理后台促销状态面板（从 app/(main)/admin/page.tsx 按域拆出，
 * 后并回 ./_parts/admin-tool-panels.tsx 家族）：FreeleechPanel 对应
 * /admin/freeleech 的增删查。清除缓存面板同文件承载。
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import type { PromoRow } from "./admin-shared";

const PROMO_KINDS: [string, string][] = [
  ["free", "免费下载"],
  ["x2", "双倍上传"],
  ["x2free", "免费 + 双倍"],
  ["half", "半价下载"],
  ["x2half", "半价 + 双倍"],
  ["p30", "30% 下载"],
];

const PROMO_SCOPES: [string, string][] = [
  ["global", "全站"],
  ["official", "官方种"],
  ["non_official", "非官方种"],
  ["category", "指定分类"],
];

/** 促销状态（freeleech）管理：对应 /admin/freeleech 的增删查 */
export function FreeleechPanel() {
  const { dict, locale } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const [rows, setRows] = useState<PromoRow[]>([]);
  const [kind, setKind] = useState("free");
  const [scope, setScope] = useState("global");
  const [hours, setHours] = useState(24);
  const [categoryId, setCategoryId] = useState("1");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setRows(await api.get<PromoRow[]>("/api/v1/admin/freeleech"));
    } catch {
      setRows([]);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  async function create() {
    setBusy(true);
    try {
      await api.post("/api/v1/admin/freeleech", {
        kind,
        scope,
        hours,
        ...(scope === "category" ? { category_id: Number(categoryId) } : {}),
      });
      setMsg("促销已生效");
      load();
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : a.actionFailed,
      );
    } finally {
      setBusy(false);
    }
  }

  async function remove(id: number) {
    setBusy(true);
    try {
      await api.del(`/api/v1/admin/freeleech/${id}`);
      setMsg("已取消");
      load();
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : a.actionFailed,
      );
    } finally {
      setBusy(false);
    }
  }

  const inputCls =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky";

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-2 text-base font-bold">促销状态</h2>
      <p className="mb-3 text-xs text-sub">
        对全站或指定范围种子设置免费 / 双倍 / 半价状态，到期自动失效。
      </p>
      {msg && (
        <p className="mb-3 rounded-[var(--r-md)] bg-sky-soft p-2 text-xs">
          {msg}
        </p>
      )}
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead w-24">类型</td>
              <td className="colhead w-28">范围</td>
              <td className="colhead">开始</td>
              <td className="colhead">结束</td>
              <td className="colhead w-20" />
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.id}>
                <td className="rowfollow">
                  {PROMO_KINDS.find(([k]) => k === r.kind)?.[1] ?? r.kind}
                </td>
                <td className="rowfollow">
                  {PROMO_SCOPES.find(([s]) => s === r.scope)?.[1] ?? r.scope}
                  {r.category_name ? ` · ${r.category_name}` : ""}
                </td>
                <td className="rowfollow text-xs text-sub">
                  {new Date(r.starts_at).toLocaleString(dateLocale(locale))}
                </td>
                <td className="rowfollow text-xs text-sub">
                  {new Date(r.ends_at).toLocaleString(dateLocale(locale))}
                </td>
                <td className="rowfollow">
                  <button
                    disabled={busy}
                    onClick={() => remove(r.id)}
                    className="min-h-[28px] rounded-full border border-line px-3 font-bold text-danger disabled:opacity-50"
                  >
                    取消
                  </button>
                </td>
              </tr>
            ))}
            {rows.length === 0 && (
              <tr>
                <td colSpan={5} className="py-4 text-center text-sub">
                  当前没有进行中的促销
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="mt-4 flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">类型</span>
          <select
            value={kind}
            onChange={(e) => setKind(e.target.value)}
            className={inputCls}
          >
            {PROMO_KINDS.map(([k, label]) => (
              <option key={k} value={k}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">范围</span>
          <select
            value={scope}
            onChange={(e) => setScope(e.target.value)}
            className={inputCls}
          >
            {PROMO_SCOPES.map(([s, label]) => (
              <option key={s} value={s}>
                {label}
              </option>
            ))}
          </select>
        </label>
        {scope === "category" && (
          <label className="flex flex-col gap-1">
            <span className="text-xs text-sub">分类 ID</span>
            <input
              type="number"
              value={categoryId}
              onChange={(e) => setCategoryId(e.target.value)}
              className={`${inputCls} w-24`}
            />
          </label>
        )}
        <label className="flex flex-col gap-1">
          <span className="text-xs text-sub">时长（小时）</span>
          <input
            type="number"
            min={1}
            max={720}
            value={hours}
            onChange={(e) => setHours(Number(e.target.value))}
            className={`${inputCls} w-28`}
          />
        </label>
        <button
          disabled={busy}
          onClick={create}
          className="min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold text-white disabled:opacity-50"
        >
          生效
        </button>
      </div>
    </section>
  );
}

/** 清除缓存：对应 POST /admin/clearcache（清除限流等运行期缓存键） */
export function ClearCachePanel() {
  const { dict } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function run() {
    setBusy(true);
    try {
      const r = await api.post<{ cleared: number }>(
        "/api/v1/admin/clearcache",
        {},
      );
      setMsg(`已清除 ${r.cleared} 个缓存键`);
    } catch (e) {
      setMsg(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : a.actionFailed,
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-2 text-base font-bold">清除缓存</h2>
      <p className="mb-3 text-xs text-sub">
        清除运行期缓存键（限流计数等）。不影响数据库数据，站点会自动重建缓存。
      </p>
      {msg && (
        <p className="mb-3 rounded-[var(--r-md)] bg-sky-soft p-2 text-xs">
          {msg}
        </p>
      )}
      <button
        disabled={busy}
        onClick={run}
        className="min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold text-white disabled:opacity-50"
      >
        立即清除
      </button>
    </section>
  );
}
