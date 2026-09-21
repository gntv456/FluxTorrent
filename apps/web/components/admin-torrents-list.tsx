"use client";

import { BTN_SM_GHOST } from "@/lib/ui-classes";

/**
 * 后台种子管理·批量工作台（从 components/admin-torrents.tsx 按域拆出）：
 * TorrentList 筛选条件 + 批量工具条（置顶/优惠/推荐/标签/H&R/改分类/删除）
 * + 种子列表 + 分页。好学站 torrent/torrents 批量动作口径。
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import {
  APPROVAL,
  actionLabel,
  PROMO_LABEL,
  fmtBytes,
  type AdminTorrentRow,
  type CatRow,
  type DenyReason,
  type TagRow,
} from "./admin-torrents-shared";
import { BatchBar } from "./admin-torrents-batch-bar";
import { TorrentTable } from "./admin-torrents-table";

/** 搜索按钮（实底天蓝） */
const SKY_BTN_CLS =
  "min-h-[36px] rounded-full bg-sky px-4 text-xs font-bold text-white";
/** 分页描边小按钮 */
const PAGE_BTN_CLS = BTN_SM_GHOST;
/** 种子名搜索输入框 */
const Q_INPUT_CLS =
  "min-h-[36px] w-40 rounded-[var(--r-sm)] border border-line px-2";
const sel_input =
  "min-h-[36px] rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-2 text-xs";

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


  const decide = async (id: number, approve: boolean) => {
    let deny_reason_id: number | undefined;
    let reason = "";
    if (!approve) {
      const reasons: DenyReason[] = await api.get("/api/v1/admin/deny-reasons");
      const list = reasons.map((r) => `${r.id}. ${r.reason}`).join("\\n");
      const choice = prompt(
        `拒绝原因（输入编号，可留空后手填理由）：\\n${list}\\n` +
          "或直接输入自定义理由文字",
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
            className={Q_INPUT_CLS}
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
          className={SKY_BTN_CLS}
        >
          搜索
        </button>
      </section>

      {/* 批量工具条（拆至 ./admin-torrents-batch-bar.tsx） */}
      <BatchBar
        busy={busy}
        selCount={sel.size}
        tags={tags}
        cats={cats}
        posUntil={posUntil}
        setPosUntil={setPosUntil}
        promoKind={promoKind}
        setPromoKind={setPromoKind}
        promoHours={promoHours}
        setPromoHours={setPromoHours}
        pickType={pickType}
        setPickType={setPickType}
        tagIds={tagIds}
        setTagIds={setTagIds}
        batchCat={batchCat}
        setBatchCat={setBatchCat}
        batch={batch}
      />

      {/* 列表（表格拆至 ./admin-torrents-table.tsx） */}
      <TorrentTable data={data} sel={sel} setSel={setSel} decide={decide} />
      <div className="flex items-center justify-between text-sm text-sub">
        <span>共 {total} 条</span>
        <div className="flex gap-2">
          <button
            disabled={page <= 1}
            onClick={() => setPage(page - 1)}
            className={PAGE_BTN_CLS}
          >
            上一页
          </button>
          <span>第 {data?.page ?? 1} 页</span>
          <button
            disabled={!data || data.rows.length < 20}
            onClick={() => setPage(page + 1)}
            className={PAGE_BTN_CLS}
          >
            下一页
          </button>
        </div>
      </div>
    </div>
  );
}
