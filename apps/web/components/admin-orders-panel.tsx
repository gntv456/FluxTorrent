"use client";

/**
 * 后台道具管理·订单子表（商城审计 P2-1）：GET /admin/shop-orders 的
 * 浏览面——按用户名/UID/商品/未发货筛选。回收动作不在本表：背包道具走
 * /admin/users/{id}/revoke-item/{order_id}（用户背包行内按钮），券走
 * /admin/user-vouchers/void，此处只解决「找到那笔订单」。
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface AdminOrderRow {
  id: number;
  user_id: number;
  username: string | null;
  item_id: number;
  item_name: string | null;
  kind: string;
  price: number;
  effect_applied: boolean;
  idempotency_key: string;
  created_at: string;
}

interface OrdersEnvelope {
  rows: AdminOrderRow[];
  total: number;
  page: number;
  per_page: number;
}

const FILTER_CLS =
  "min-h-[32px] rounded-full border border-line px-3 text-xs";

export function OrdersPanel() {
  const { dict, locale, currency } = useI18n();
  const at = dict.adminProps.orders;
  const [data, setData] = useState<OrdersEnvelope | null>(null);
  const [q, setQ] = useState("");
  const [uid, setUid] = useState("");
  const [pending, setPending] = useState(false);
  const [page, setPage] = useState(1);
  const [err, setErr] = useState<string | null>(null);

  const load = useCallback(async () => {
    const sp = new URLSearchParams({ page: String(page) });
    if (q.trim()) sp.set("q", q.trim());
    if (uid.trim() && Number(uid) > 0) sp.set("uid", uid.trim());
    if (pending) sp.set("pending", "true");
    try {
      setData(
        await api.get<OrdersEnvelope>(
          `/api/v1/admin/shop-orders?${sp.toString()}`,
        ),
      );
      setErr(null);
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : dict.common.loadFailed);
    }
  }, [q, uid, pending, page, dict]);
  useEffect(() => {
    void load();
  }, [load]);

  const pages = data ? Math.max(1, Math.ceil(data.total / data.per_page)) : 1;
  const d = (s: string) => s.slice(0, 19).replace("T", " ");

  return (
    <section className="baozi-panel p-4">
      <div className="mb-2 flex flex-wrap items-end gap-2">
        <h3 className="text-sm font-bold">{at.title}</h3>
        <input
          value={q}
          onChange={(e) => {
            setQ(e.target.value);
            setPage(1);
          }}
          placeholder={at.qUser}
          className={FILTER_CLS + " w-40"}
        />
        <input
          value={uid}
          onChange={(e) => {
            setUid(e.target.value);
            setPage(1);
          }}
          placeholder={at.qUid}
          className={FILTER_CLS + " w-28"}
          inputMode="numeric"
        />
        <label className="flex items-center gap-1 text-xs text-sub">
          <input
            type="checkbox"
            checked={pending}
            onChange={(e) => {
              setPending(e.target.checked);
              setPage(1);
            }}
          />
          {at.onlyPending}
        </label>
      </div>
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table">
          <thead>
            <tr>
              <th>#</th>
              <th>{at.colUser}</th>
              <th>{at.colItem}</th>
              <th>{at.colPrice}</th>
              <th>{at.colStatus}</th>
              <th>{at.colAt}</th>
            </tr>
          </thead>
          <tbody>
            {(data?.rows ?? []).map((r) => (
              <tr key={r.id}>
                <td className="num">{r.id}</td>
                <td>
                  {r.username ?? "—"}
                  <span className="text-xs text-sub"> #{r.user_id}</span>
                </td>
                <td>
                  {r.item_name ?? `#${r.item_id}`}
                  <span className="text-xs text-sub"> {r.kind}</span>
                </td>
                <td className="num">
                  {r.price.toLocaleString()} {currency}
                </td>
                <td>
                  <span
                    className={
                      r.effect_applied ? "text-success" : "text-danger"
                    }
                  >
                    {r.effect_applied ? at.applied : at.pending}
                  </span>
                </td>
                <td className="text-xs text-sub">{d(r.created_at)}</td>
              </tr>
            ))}
            {(data?.rows ?? []).length === 0 && (
              <tr>
                <td colSpan={6} className="py-6 text-center text-sub">
                  {err ?? at.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      {pages > 1 && (
        <div className="flex items-center justify-center gap-3 p-2 text-sm">
          <button
            type="button"
            className="btn btn-sm btn-ghost"
            disabled={page <= 1}
            onClick={() => setPage((p) => p - 1)}
          >
            ‹
          </button>
          <span className="text-xs text-sub">
            {page} / {pages}
          </span>
          <button
            type="button"
            className="btn btn-sm btn-ghost"
            disabled={page >= pages}
            onClick={() => setPage((p) => p + 1)}
          >
            ›
          </button>
        </div>
      )}
    </section>
  );
}
