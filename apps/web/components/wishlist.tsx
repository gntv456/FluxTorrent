"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

/** 教材心愿单（0074，U3D WishList 教育化）
 *  键词型：后端按 keyword 存（非 torrent_id），命中靠发布时的关键词匹配。
 *  GET /wishlist 列表 / POST /wishlist {keyword} / POST /wishlist/remove {id} */

interface WishRow {
  id: number;
  keyword: string;
  category_id: number | null;
  created_at: string;
}

/** 命中历史行（0284 P2-10：{items,hits} 响应的 hits 段） */
interface WishHit {
  torrent_id: number;
  torrent_name: string;
  keyword: string;
  created_at: string;
}

interface WishResp {
  items: WishRow[];
  hits: WishHit[];
}

/** 兼容读取：旧裸数组 / 新 {items,hits} 双形态 */
function readItems(d: WishRow[] | WishResp): WishRow[] {
  return Array.isArray(d) ? d : d.items;
}

export function WishlistButton({ keyword }: { keyword: string }) {
  const { dict } = useI18n();
  const t = dict.wishlist;
  const kw = keyword.trim().slice(0, 100);
  const [added, setAdded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => {
    // 已在单判断：本地列表里命中同名关键词即视为已加（后端按 (user, keyword) 唯一）
    if (!kw) return;
    api
      .get<WishRow[] | WishResp>("/api/v1/wishlist")
      .then((d) => setAdded(readItems(d).some((r) => r.keyword === kw)))
      .catch(() => setAdded(false));
  }, [kw]);

  async function toggle() {
    if (!kw || busy) return;
    setBusy(true);
    setMsg(null);
    try {
      if (added) {
        const d = await api.get<WishRow[] | WishResp>("/api/v1/wishlist");
        const hit = readItems(d).find((r) => r.keyword === kw);
        if (hit) await api.post("/api/v1/wishlist/remove", { id: hit.id });
        setAdded(false);
        setMsg(t.removed);
      } else {
        await api.post("/api/v1/wishlist", { keyword: kw });
        setAdded(true);
        setMsg(t.addedOk);
      }
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.addFailed);
    } finally {
      setBusy(false);
    }
  }

  return (
    <span className="inline-flex flex-col items-start">
      <button
        type="button"
        onClick={toggle}
        disabled={busy || !kw}
        title={t.note}
        className={`min-h-[36px] rounded-full px-4 text-xs font-bold transition-transform active:scale-[0.97] disabled:opacity-50 ${
          added
            ? "border border-[var(--baozi-orange)] text-[var(--baozi-orange-dark)]"
            : "bg-[linear-gradient(135deg,var(--baozi-orange-bright),var(--baozi-orange))] text-white"
        }`}
      >
        {added ? `★ ${t.added}` : `☆ ${t.addBtn}`}
      </button>
      {msg && (
        <span className="mt-1 text-xs text-sub" role="status">
          {msg}
        </span>
      )}
    </span>
  );
}

/** /my 心愿单 tab：关键词列表 + 移除 + 新增输入 */
export function WishlistPanel() {
  const { dict, locale } = useI18n();
  const t = dict.wishlist;
  const [rows, setRows] = useState<WishRow[] | null>(null);
  const [hits, setHits] = useState<WishHit[]>([]);
  const [kw, setKw] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const d = await api.get<WishRow[] | WishResp>("/api/v1/wishlist");
      setRows(readItems(d));
      setHits(Array.isArray(d) ? [] : d.hits ?? []);
    } catch {
      setRows([]);
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  const count = useMemo(() => rows?.length ?? 0, [rows]);

  async function add() {
    if (!kw.trim() || busy) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/wishlist", { keyword: kw.trim() });
      setKw("");
      setMsg(t.addedOk);
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.addFailed);
    } finally {
      setBusy(false);
    }
  }

  async function remove(id: number) {
    setBusy(true);
    try {
      await api.post("/api/v1/wishlist/remove", { id });
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-3">
      <h2 className="font-display text-lg">{t.title}</h2>
      <p className="text-xs text-sub">{t.note}</p>
      <form
        className="flex flex-wrap gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <input
          type="text"
          value={kw}
          onChange={(e) => setKw(e.target.value)}
          maxLength={100}
          placeholder={t.keywordPh}
          className="min-h-[44px] min-w-0 flex-1 rounded-full border border-line bg-cloud px-4 text-sm outline-none focus:border-sky"
        />
        <button
          type="submit"
          disabled={busy || !kw.trim()}
          className="baozi-button disabled:opacity-50"
        >
          {t.addBtn}
        </button>
      </form>
      {msg && (
        <p className="text-xs text-sub" role="status">
          {msg}
        </p>
      )}
      {rows !== null && rows.length > 0 && (
        <div className="baozi-wide-table-scroll">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">#{t.title}</td>
              <td className="colhead w-40">{dict.messages.timeCol}</td>
              <td className="colhead w-24" />
            </tr>
            {rows.map((r) => (
              <tr key={r.id}>
                <td className="rowfollow font-bold">{r.keyword}</td>
                <td className="rowfollow text-xs text-sub">
                  {new Date(r.created_at).toLocaleString(dateLocale(locale))}
                </td>
                <td className="rowfollow text-right">
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => remove(r.id)}
                    className="min-h-[32px] rounded-full border border-line px-3 text-xs font-bold text-danger disabled:opacity-50"
                  >
                    {t.removeBtn}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        </div>
      )}
      {rows !== null && rows.length === 0 && (
        <p className="py-6 text-center text-sub">{t.empty}</p>
      )}
      {rows !== null && (
        <p className="text-right text-xs text-sub">
          {t.hit.replace("{n}", String(count))}
        </p>
      )}
      {/* 命中历史（0284 P2-10）：通知之外的可回看记录 */}
      {hits.length > 0 && (
        <div className="baozi-wide-table-scroll">
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">{t.hitsTitle}</td>
                <td className="colhead">{t.hitsKeyword}</td>
                <td className="colhead">{t.hitsAt}</td>
              </tr>
              {hits.map((h) => (
                <tr key={`${h.torrent_id}-${h.keyword}`}>
                  <td className="rowfollow">
                    <a
                      href={`/torrent/${h.torrent_id}`}
                      className="text-link"
                    >
                      {h.torrent_name}
                    </a>
                  </td>
                  <td className="rowfollow text-xs">{h.keyword}</td>
                  <td className="rowfollow text-xs text-sub">
                    {new Date(h.created_at).toLocaleString(dateLocale(locale))}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
