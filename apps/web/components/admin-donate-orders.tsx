"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import { STAFF_CLASS_MIN } from "@/lib/domain/user-class";

/** 捐赠订单后台管理（0209 P1-6）：列表/筛选/手工补单。
 *  网关掉单时站长此前只能进数据库——这里是运营侧出口。 */

interface OrderRow {
  id: number;
  order_no: string;
  user_id: number;
  username: string | null;
  amount_usd: number;
  amount_paid: number | null;
  channel: string;
  status: string;
  trade_no: string | null;
  created_at: string;
  paid_at: string | null;
}

interface PanelEntryRow {
  id: number;
  section: string;
  name: string;
  url: string;
  info: string;
  sort: number;
  tab_key: string;
  min_class: number;
  module_key: string | null;
  perm_key: string | null;
}

export function AdminDonateOrders() {
  const { dict, locale } = useI18n();
  const t = dict.adminDonate;
  const c = dict.common;
  const [rows, setRows] = useState<OrderRow[]>([]);
  const [total, setTotal] = useState(0);
  const [paidSum, setPaidSum] = useState(0);
  const [status, setStatus] = useState("");
  const [uid, setUid] = useState("");
  const [page, setPage] = useState(1);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    const p = new URLSearchParams({ page: String(page), per_page: "20" });
    if (status) p.set("status", status);
    if (uid.trim()) p.set("uid", uid.trim());
    try {
      const r = await api.get<{
        rows: OrderRow[];
        total: number;
        paid_sum_usd: number;
      }>(`/api/v1/admin/payment-orders?${p}`);
      setRows(r.rows);
      setTotal(r.total);
      setPaidSum(r.paid_sum_usd);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : c.networkError);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status, uid, page]);

  useEffect(() => {
    load();
  }, [load]);

  async function complete(row: OrderRow) {
    if (
      !window.confirm(
        t.confirmComplete.replace("{no}", row.order_no).replace(
          "{usd}",
          row.amount_usd.toFixed(2),
        ),
      )
    )
      return;
    setBusy(true);
    try {
      const r = await api.post<{ result: string }>(
        "/api/v1/admin/payment-orders/complete",
        { order_no: row.order_no },
      );
      setMsg(r.result === "duplicate" ? t.duplicate : t.completed);
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : c.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <section className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          {t.fStatus}
          <select
            value={status}
            onChange={(e) => {
              setStatus(e.target.value);
              setPage(1);
            }}
            className="min-h-[36px] rounded-md border border-line bg-cloud px-2 text-sm"
          >
            <option value="">{t.optAll}</option>
            <option value="pending">{t.stPending}</option>
            <option value="paid">{t.stPaid}</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          {t.fUid}
          <input
            value={uid}
            onChange={(e) => {
              setUid(e.target.value);
              setPage(1);
            }}
            inputMode="numeric"
            placeholder={t.optAll}
            className="min-h-[36px] w-32 rounded-md border border-line bg-cloud px-2 text-sm"
          />
        </label>
        <span className="ml-auto text-xs text-sub">
          {t.paidSum.replace("{n}", paidSum.toFixed(2))}
        </span>
      </section>
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">{t.thOrder}</td>
              <td className="colhead">{t.thUser}</td>
              <td className="colhead">{t.thAmount}</td>
              <td className="colhead">{t.thStatus}</td>
              <td className="colhead">{t.thTrade}</td>
              <td className="colhead">{t.thCreated}</td>
              <td className="colhead">{t.thAction}</td>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.id}>
                <td className="font-mono">{r.order_no.slice(0, 16)}…</td>
                <td>
                  <a
                    href={`/admin/users/${r.user_id}`}
                    className="text-link"
                  >
                    {r.username ?? `#${r.user_id}`}
                  </a>
                </td>
                <td className="num">
                  ${(r.amount_paid ?? r.amount_usd).toFixed(2)}
                </td>
                <td>
                  <span
                    className={`sticker ${
                      r.status === "paid"
                        ? "bg-mint/30 text-ink"
                        : "bg-sun text-ink"
                    }`}
                  >
                    {r.status === "paid" ? t.stPaid : t.stPending}
                  </span>
                </td>
                <td className="text-sub">{r.trade_no ?? "—"}</td>
                <td className="text-sub">
                  {new Date(r.created_at).toLocaleDateString(
                    dateLocale(locale),
                  )}
                </td>
                <td>
                  {r.status === "pending" && (
                    <button
                      disabled={busy}
                      onClick={() => void complete(r)}
                      className="text-sky-deep hover:underline"
                    >
                      {t.completeBtn}
                    </button>
                  )}
                </td>
              </tr>
            ))}
            {rows.length === 0 && (
              <tr>
                <td colSpan={7} className="py-6 text-center text-sub">
                  {t.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <div className="flex items-center justify-between text-sm text-sub">
        <span>{c.totalItems.replace("{n}", String(total))}</span>
        <div className="flex gap-2">
          <button
            disabled={page <= 1}
            onClick={() => setPage(page - 1)}
            className="min-h-[32px] rounded-full border border-line px-3 text-xs disabled:opacity-40"
          >
            {c.prevPage}
          </button>
          <span>{c.pageX.replace("{n}", String(page))}</span>
          <button
            disabled={rows.length < 20}
            onClick={() => setPage(page + 1)}
            className="min-h-[32px] rounded-full border border-line px-3 text-xs disabled:opacity-40"
          >
            {c.nextPage}
          </button>
        </div>
      </div>
    </div>
  );
}

/** 面板条目管理（0209 P1-7）：后台导航条目 CRUD，不再写 SQL。 */
export function AdminPanelEntries() {
  const { dict } = useI18n();
  const t = dict.adminPanel;
  const c = dict.common;
  const [rows, setRows] = useState<PanelEntryRow[] | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [edit, setEdit] = useState<Partial<PanelEntryRow> | null>(null);

  const load = useCallback(async () => {
    try {
      setRows(await api.get<PanelEntryRow[]>("/api/v1/admin/staffpanel-entries"));
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : c.networkError);
      setRows([]);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  async function save() {
    if (!edit?.name?.trim() || !edit.url?.trim()) return;
    setBusy(true);
    try {
      const payload = {
        section: edit.section?.trim() || "system",
        name: edit.name.trim(),
        url: edit.url.trim(),
        info: edit.info?.trim() ?? "",
        sort: edit.sort ?? 0,
        tab_key: edit.tab_key ?? "",
        min_class: edit.min_class ?? STAFF_CLASS_MIN,
        module_key: edit.module_key || null,
        perm_key: edit.perm_key || null,
      };
      if (edit.id != null) {
        await api.put(`/api/v1/admin/staffpanel-entries/${edit.id}`, payload);
      } else {
        await api.post("/api/v1/admin/staffpanel-entries", payload);
      }
      setEdit(null);
      setMsg(t.saved);
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : c.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function del(id: number) {
    if (!window.confirm(t.confirmDel)) return;
    setBusy(true);
    try {
      await api.post("/api/v1/admin/staffpanel-entries/delete", { id });
      setMsg(t.deleted);
      await load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : c.networkError);
    } finally {
      setBusy(false);
    }
  }

  const inp =
    "min-h-[36px] rounded-md border border-line bg-cloud px-2 text-sm";

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      {/* 编辑/新增表单 */}
      {edit && (
        <section className="baozi-panel flex flex-wrap items-end gap-2 p-3">
          <label className="flex flex-col gap-1 text-xs">
            {t.fSection}
            <input
              value={edit.section ?? ""}
              onChange={(e) => setEdit({ ...edit, section: e.target.value })}
              className={`${inp} w-28`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            {t.fName}
            <input
              value={edit.name ?? ""}
              onChange={(e) => setEdit({ ...edit, name: e.target.value })}
              className={`${inp} w-36`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            URL
            <input
              value={edit.url ?? ""}
              onChange={(e) => setEdit({ ...edit, url: e.target.value })}
              placeholder="/admin?tool=xxx"
              className={`${inp} w-52`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            {t.fInfo}
            <input
              value={edit.info ?? ""}
              onChange={(e) => setEdit({ ...edit, info: e.target.value })}
              className={`${inp} w-44`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            tab_key
            <input
              value={edit.tab_key ?? ""}
              onChange={(e) => setEdit({ ...edit, tab_key: e.target.value })}
              className={`${inp} w-28`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            {t.fMinClass}
            <input
              type="number"
              value={edit.min_class ?? STAFF_CLASS_MIN}
              onChange={(e) =>
                setEdit({ ...edit, min_class: Number(e.target.value) })
              }
              className={`${inp} w-20`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            perm_key
            <input
              value={edit.perm_key ?? ""}
              onChange={(e) => setEdit({ ...edit, perm_key: e.target.value })}
              placeholder={t.phPerm}
              className={`${inp} w-36`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            module_key
            <input
              value={edit.module_key ?? ""}
              onChange={(e) => setEdit({ ...edit, module_key: e.target.value })}
              placeholder={t.phModule}
              className={`${inp} w-28`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            {t.fSort}
            <input
              type="number"
              value={edit.sort ?? 0}
              onChange={(e) =>
                setEdit({ ...edit, sort: Number(e.target.value) })
              }
              className={`${inp} w-20`}
            />
          </label>
          <button
            disabled={busy}
            onClick={() => void save()}
            className="min-h-[36px] rounded-full bg-sky-deep px-4 text-sm text-white disabled:opacity-50"
          >
            {c.save}
          </button>
          <button
            onClick={() => setEdit(null)}
            className="min-h-[36px] rounded-full border border-line px-4 text-sm text-sub"
          >
            {c.cancel}
          </button>
        </section>
      )}
      <div className="flex items-center justify-between">
        <h2 className="text-base font-bold">{t.title}</h2>
        <button
          onClick={() =>
            setEdit({
              section: "system",
              name: "",
              url: "",
              info: "",
              sort: 0,
              tab_key: "",
              min_class: STAFF_CLASS_MIN,
            })
          }
          className="min-h-[36px] rounded-full border border-line px-4 text-sm text-sky-deep"
        >
          + {t.add}
        </button>
      </div>
      <div className="baozi-wide-table-scroll">
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">ID</td>
              <td className="colhead">{t.fSection}</td>
              <td className="colhead">{t.fName}</td>
              <td className="colhead">URL</td>
              <td className="colhead">tab_key</td>
              <td className="colhead">min_class</td>
              <td className="colhead">perm_key</td>
              <td className="colhead">module_key</td>
              <td className="colhead">{t.thAction}</td>
            </tr>
          </thead>
          <tbody>
            {(rows ?? []).map((r) => (
              <tr key={r.id}>
                <td className="num">{r.id}</td>
                <td>{r.section}</td>
                <td>{r.name}</td>
                <td className="font-mono">{r.url}</td>
                <td>{r.tab_key || "—"}</td>
                <td className="num">{r.min_class}</td>
                <td>{r.perm_key ?? "—"}</td>
                <td>{r.module_key ?? "—"}</td>
                <td>
                  <div className="flex gap-2">
                    <button
                      onClick={() => setEdit(r)}
                      className="text-sky-deep hover:underline"
                    >
                      {t.editBtn}
                    </button>
                    <button
                      disabled={busy}
                      onClick={() => void del(r.id)}
                      className="text-tomato hover:underline"
                    >
                      {t.delBtn}
                    </button>
                  </div>
                </td>
              </tr>
            ))}
            {rows?.length === 0 && (
              <tr>
                <td colSpan={9} className="py-6 text-center text-sub">
                  {t.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <p className="text-xs text-sub">{t.permHint}</p>
    </div>
  );
}
