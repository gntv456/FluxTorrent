"use client";

/**
 * 后台种子管理·批量工作台（从 components/admin-torrents.tsx 按域拆出）：
 * TorrentList 筛选条件 + 批量工具条（置顶/优惠/推荐/标签/H&R/改分类/删除）
 * + 种子列表 + 分页。好学站 torrent/torrents 批量动作口径。
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import {
  APPROVAL,
  PROMO_LABEL,
  fmtBytes,
  type AdminTorrentRow,
  type CatRow,
  type DenyReason,
  type TagRow,
} from "./admin-torrents-shared";

const sel_input =
  "min-h-[36px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2 text-xs";

export function TorrentList({ flash }: { flash: (m: string) => void }) {
  const [q, setQ] = useState("");
  const [status, setStatus] = useState("");
  const [owner, setOwner] = useState("");
  const [pos, setPos] = useState("");
  const [promo, setPromo] = useState("");
  const [pick, setPick] = useState("");
  const [hr, setHr] = useState("");
  const [page, setPage] = useState(1);
  const [total, setTotal] = useState(0);
  const [data, setData] = useState<{
    rows: AdminTorrentRow[];
    page: number;
    per_page: number;
  } | null>(null);
  const [sel, setSel] = useState<Set<number>>(new Set());
  const [tags, setTags] = useState<TagRow[]>([]);
  const [cats, setCats] = useState<CatRow[]>([]);
  const [busy, setBusy] = useState(false);
  // 批量参数
  const [posUntil, setPosUntil] = useState("");
  const [promoKind, setPromoKind] = useState("free");
  const [promoHours, setPromoHours] = useState("48");
  const [pickType, setPickType] = useState("0");
  const [tagIds, setTagIds] = useState<Set<number>>(new Set());
  const [batchCat, setBatchCat] = useState("");

  const load = useCallback(async () => {
    const params = new URLSearchParams({ page: String(page), per_page: "20" });
    if (q.trim()) params.set("q", q.trim());
    if (status) params.set("status", status);
    if (owner.trim()) params.set("owner", owner.trim());
    if (pos !== "") params.set("pos_state", pos);
    if (promo) params.set("promo", promo);
    if (pick) params.set("pick_type", pick);
    if (hr) params.set("hr", hr);
    try {
      const r = await api.get<{
        rows: AdminTorrentRow[];
        page: number;
        per_page: number;
        total: number;
      }>(`/api/v1/admin/torrents?${params.toString()}`);
      setData(r);
      setTotal(r.total ?? 0);
      setSel(new Set());
    } catch {
      setData(null);
    }
  }, [q, status, owner, pos, promo, pick, hr, page]);

  useEffect(() => {
    load();
  }, [load]);
  useEffect(() => {
    api
      .get<TagRow[]>("/api/v1/admin/tags-dict")
      .then(setTags)
      .catch(() => {});
    api
      .get<CatRow[]>("/api/v1/admin/categories")
      .then(setCats)
      .catch(() => {});
  }, []);

  const ids = [...sel];
  async function batch(action: string, extra: Record<string, unknown> = {}) {
    if (ids.length === 0) {
      flash("请先勾选种子");
      return;
    }
    setBusy(true);
    try {
      const r = await api.post<{ affected: number }>(
        "/api/v1/admin/torrents/batch",
        { action, ids, ...extra },
      );
      flash(`已${actionLabel(action)} ${r.affected} 个种子`);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  function actionLabel(a: string): string {
    return (
      {
        sticky: "置顶",
        promo: "设置促销",
        recommend: "推荐",
        set_tags: "打标",
        clear_tags: "清除标签",
        hr: "标记H&R",
        unhr: "取消H&R",
        change_category: "改分类",
        delete: "删除",
      }[a] ?? a
    );
  }

  const decide = async (id: number, approve: boolean) => {
    let deny_reason_id: number | undefined;
    let reason = "";
    if (!approve) {
      const reasons: DenyReason[] = await api.get("/api/v1/admin/deny-reasons");
      const choice = prompt(
        `拒绝原因（输入编号，可留空后手填理由）：\n${reasons.map((r) => `${r.id}. ${r.reason}`).join("\n")}\n或直接输入自定义理由文字`,
      );
      if (!choice) return;
      const asNum = Number(choice);
      if (asNum > 0 && reasons.some((r) => r.id === asNum))
        deny_reason_id = asNum;
      else reason = choice;
    }
    try {
      await api.post("/api/v1/admin/reviews/decide", {
        torrent_id: id,
        approve,
        reason,
        deny_reason_id,
      });
      flash(approve ? `已通过种子 #${id}` : `已拒绝种子 #${id}`);
      load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    }
  };

  return (
    <div className="flex flex-col gap-3">
      {/* 筛选条件 */}
      <section className="baozi-panel flex flex-wrap items-end gap-2 p-3">
        <label className="flex flex-col gap-1 text-xs">
          名称
          <input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="种子名"
            className="min-h-[36px] w-40 rounded-[var(--r-sm)] border border-line px-2"
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          发布者
          <input
            value={owner}
            onChange={(e) => setOwner(e.target.value)}
            placeholder="UID"
            className={`${sel_input} w-20`}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs">
          状态
          <select
            value={status}
            onChange={(e) => setStatus(e.target.value)}
            className={sel_input}
          >
            <option value="">全部</option>
            <option value="1">待审</option>
            <option value="2">通过</option>
            <option value="3">拒绝</option>
            <option value="4">死种</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          置顶
          <select
            value={pos}
            onChange={(e) => setPos(e.target.value)}
            className={sel_input}
          >
            <option value="">全部</option>
            <option value="1">置顶中</option>
            <option value="0">未置顶</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          促销
          <select
            value={promo}
            onChange={(e) => setPromo(e.target.value)}
            className={sel_input}
          >
            <option value="">全部</option>
            <option value="yes">促销中</option>
            <option value="no">无促销</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          推荐
          <select
            value={pick}
            onChange={(e) => setPick(e.target.value)}
            className={sel_input}
          >
            <option value="">全部</option>
            <option value="1">推荐</option>
            <option value="2">经典</option>
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          H&R
          <select
            value={hr}
            onChange={(e) => setHr(e.target.value)}
            className={sel_input}
          >
            <option value="">全部</option>
            <option value="yes">标记</option>
            <option value="no">未标记</option>
          </select>
        </label>
        <button
          onClick={() => {
            setPage(1);
            load();
          }}
          className="min-h-[36px] rounded-full bg-sky px-4 text-xs font-bold text-white"
        >
          搜索
        </button>
      </section>

      {/* 批量工具条 */}
      <section className="baozi-panel flex flex-wrap items-end gap-3 p-3">
        <p className="w-full text-xs font-bold text-sub">
          批量操作（已选 {sel.size} 个，勾选下方列表后执行；单批最多 500）
        </p>
        <label className="flex flex-col gap-1 text-xs">
          置顶截止
          <input
            type="datetime-local"
            value={posUntil}
            onChange={(e) => setPosUntil(e.target.value)}
            className={sel_input}
          />
        </label>
        <button
          disabled={busy}
          onClick={() =>
            batch("sticky", {
              pos_state: 1,
              pos_state_until: posUntil
                ? new Date(posUntil).toISOString()
                : null,
            })
          }
          className="min-h-[36px] rounded-full bg-sky px-4 text-xs font-bold text-white disabled:opacity-50"
        >
          置顶
        </button>
        <button
          disabled={busy}
          onClick={() => batch("sticky", { pos_state: 0 })}
          className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-50"
        >
          取消置顶
        </button>
        <label className="flex flex-col gap-1 text-xs">
          促销类型
          <select
            value={promoKind}
            onChange={(e) => setPromoKind(e.target.value)}
            className={sel_input}
          >
            {Object.entries(PROMO_LABEL).map(([k, l]) => (
              <option key={k} value={k}>
                {l}
              </option>
            ))}
          </select>
        </label>
        <label className="flex flex-col gap-1 text-xs">
          时长(h)
          <input
            type="number"
            value={promoHours}
            onChange={(e) => setPromoHours(e.target.value)}
            className={`${sel_input} w-16`}
          />
        </label>
        <button
          disabled={busy}
          onClick={() =>
            batch("promo", {
              promo_kind: promoKind,
              promo_until: new Date(
                Date.now() + (Number(promoHours) || 48) * 3600e3,
              ).toISOString(),
            })
          }
          className="min-h-[36px] rounded-full bg-sky px-4 text-xs font-bold text-white disabled:opacity-50"
        >
          设促销
        </button>
        <label className="flex flex-col gap-1 text-xs">
          推荐
          <select
            value={pickType}
            onChange={(e) => setPickType(e.target.value)}
            className={sel_input}
          >
            <option value="0">取消</option>
            <option value="1">推荐</option>
            <option value="2">经典</option>
          </select>
        </label>
        <button
          disabled={busy}
          onClick={() => batch("recommend", { pick_type: Number(pickType) })}
          className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-50"
        >
          设推荐
        </button>
        <label className="flex flex-wrap items-center gap-1 pb-1 text-xs">
          标签
          {tags
            .filter((t) => t.enabled)
            .map((t) => (
              <label key={t.id} className="flex items-center gap-0.5">
                <input
                  type="checkbox"
                  checked={tagIds.has(t.id)}
                  onChange={(e) =>
                    setTagIds((prev) => {
                      const n = new Set(prev);
                      if (e.target.checked) n.add(t.id);
                      else n.delete(t.id);
                      return n;
                    })
                  }
                />
                {t.name}
              </label>
            ))}
        </label>
        <button
          disabled={busy || tagIds.size === 0}
          onClick={() => batch("set_tags", { tag_ids: [...tagIds] })}
          className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-50"
        >
          设置标签
        </button>
        <button
          disabled={busy}
          onClick={() => batch("clear_tags", { tag_ids: [...tagIds] })}
          className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-50"
        >
          清除标签
        </button>
        <button
          disabled={busy}
          onClick={() => batch("hr")}
          className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-50"
        >
          标记H&R
        </button>
        <button
          disabled={busy}
          onClick={() => batch("unhr")}
          className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-50"
        >
          取消H&R
        </button>
        <label className="flex flex-col gap-1 text-xs">
          改分类
          <select
            value={batchCat}
            onChange={(e) => setBatchCat(e.target.value)}
            className={sel_input}
          >
            <option value="">（不改）</option>
            {cats.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </label>
        <button
          disabled={busy || !batchCat}
          onClick={() =>
            batch("change_category", { category_id: Number(batchCat) })
          }
          className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-50"
        >
          改分类
        </button>
        <button
          disabled={busy}
          onClick={() => {
            if (window.confirm(`确认删除所选 ${sel.size} 个种子？（软删除）`))
              batch("delete");
          }}
          className="min-h-[36px] rounded-full bg-coral px-4 text-xs font-bold text-white disabled:opacity-50"
        >
          删除已选
        </button>
      </section>

      {/* 列表 */}
      <table className="nexus-table">
        <thead>
          <tr>
            <td className="colhead w-10"></td>
            <td className="colhead">ID</td>
            <td className="colhead">名称</td>
            <td className="colhead">发布者</td>
            <td className="colhead">大小</td>
            <td className="colhead">做种/下载</td>
            <td className="colhead">状态</td>
            <td className="colhead">置顶</td>
            <td className="colhead">促销</td>
            <td className="colhead">推荐</td>
            <td className="colhead">H&R</td>
            <td className="colhead">操作</td>
          </tr>
        </thead>
        <tbody>
          {data?.rows.map((t) => (
            <tr key={t.id}>
              <td>
                <input
                  type="checkbox"
                  checked={sel.has(t.id)}
                  onChange={(e) =>
                    setSel((prev) => {
                      const n = new Set(prev);
                      if (e.target.checked) n.add(t.id);
                      else n.delete(t.id);
                      return n;
                    })
                  }
                />
              </td>
              <td>{t.id}</td>
              <td className="max-w-[240px] truncate">
                <a
                  className="font-bold text-link"
                  href={`/torrents?id=${t.id}`}
                >
                  {t.name}
                </a>
                {t.deny_reason && (
                  <span className="ml-1 text-xs text-danger">
                    {t.deny_reason}
                  </span>
                )}
              </td>
              <td className="text-xs">{t.owner_name ?? "—"}</td>
              <td className="text-xs">{fmtBytes(t.size)}</td>
              <td>
                {t.seeders} / {t.leechers}
              </td>
              <td>{APPROVAL[t.approval_status] ?? t.approval_status}</td>
              <td>
                {t.pos_state === 1 ? (
                  <span className="text-sky">
                    置顶
                    {t.pos_state_until
                      ? `·${new Date(t.pos_state_until).toLocaleDateString()}`
                      : ""}
                  </span>
                ) : (
                  "—"
                )}
              </td>
              <td>
                {t.promotion ? (
                  <span className="text-mint">
                    {PROMO_LABEL[t.promotion] ?? t.promotion}
                    {t.promotion_ends_at
                      ? `·${new Date(t.promotion_ends_at).toLocaleDateString()}`
                      : ""}
                  </span>
                ) : (
                  "—"
                )}
              </td>
              <td>
                {t.pick_type === 1 ? (
                  <span className="text-danger">推荐</span>
                ) : t.pick_type === 2 ? (
                  <span className="text-sun">经典</span>
                ) : (
                  "—"
                )}
              </td>
              <td>{t.hr ? <span className="text-danger">H&R</span> : "—"}</td>
              <td>
                {t.approval_status === 0 && (
                  <>
                    <button
                      className="cmgmt-act"
                      onClick={() => decide(t.id, true)}
                    >
                      通过
                    </button>
                    <button
                      className="cmgmt-act cmgmt-act--danger"
                      onClick={() => decide(t.id, false)}
                    >
                      拒绝
                    </button>
                  </>
                )}
              </td>
            </tr>
          ))}
          {data?.rows.length === 0 && (
            <tr>
              <td colSpan={12} className="py-6 text-center text-sub">
                没有匹配的种子
              </td>
            </tr>
          )}
        </tbody>
      </table>
      <div className="flex items-center justify-between text-sm text-sub">
        <span>共 {total} 条</span>
        <div className="flex gap-2">
          <button
            disabled={page <= 1}
            onClick={() => setPage(page - 1)}
            className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40"
          >
            上一页
          </button>
          <span>第 {data?.page ?? 1} 页</span>
          <button
            disabled={!data || data.rows.length < 20}
            onClick={() => setPage(page + 1)}
            className="min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40"
          >
            下一页
          </button>
        </div>
      </div>
    </div>
  );
}
