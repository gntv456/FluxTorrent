"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 详情页策展口（批 C1 合集 / 批 C2 版本组）。
 *
 *  两个后端写口早已存在且权限齐：
 *   - `POST /collections/{id}/items` {torrent_id}（staff 或种主，幂等）
 *   - `POST /torrents/{id}/group` {name, descr?}（组名 1-100 字，幂等归组）
 *  此前前端只有 GET（/collections 列表页、订阅按钮），没有任何录入口，
 *  于是详情页的「合集」与「同组版本」两个板块在现网恒空（0 行 / 2 行）。
 *
 *  刻意不猜权限：能不能建合集由后端判（class>=90），失败就把它回给你的
 *  那句中文原样显示——不静默吞、也不在前端复刻一套等级表。
 */

type Col = { id: number; name: string; kind: string };

export function TorrentCurate({
  torrentId,
  can,
}: {
  torrentId: number;
  /** 种主或职员：普通访客不给看这排控件（后端仍会二次判） */
  can: boolean;
}) {
  const { dict } = useI18n();
  const t = dict.torrentCurate;
  const [cols, setCols] = useState<Col[]>([]);
  const [picked, setPicked] = useState("");
  const [newCol, setNewCol] = useState("");
  const [groupName, setGroupName] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => {
    if (!can) return;
    api
      .get<{ items?: Col[] } | Col[]>("/api/v1/collections")
      .then((r) => setCols(Array.isArray(r) ? r : (r.items ?? [])))
      // 读不到必须出声：静默回落成空数组会演成「这站一个合集都没有」，
      // 用户以为该收的没收，实际是列表请求失败了。
      .catch((e) => {
        setCols([]);
        setMsg(e instanceof ApiError ? e.message : t.failed);
      });
    // t 是字典快照（整页语言切换会重挂），不必进依赖
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [can]);

  if (!can) return null;

  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    setMsg(null);
    try {
      await fn();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.failed);
    } finally {
      setBusy(false);
    }
  };

  const addItem = (cid: number) =>
    api.post(`/api/v1/collections/${cid}/items`, { torrent_id: torrentId });

  const onCollect = () =>
    run(async () => {
      let cid = Number(picked);
      const nm = newCol.trim();
      if (!cid && nm) {
        const made = await api.post<{ id?: number } | number>(
          "/api/v1/collections",
          { name: nm },
        );
        cid = typeof made === "number" ? made : (made?.id ?? 0);
      }
      if (!cid) {
        setMsg(t.pickFirst);
        return;
      }
      await addItem(cid);
      setMsg(t.collected);
      setNewCol("");
    });

  const onGroup = () =>
    run(async () => {
      const nm = groupName.trim();
      if (!nm) {
        setMsg(t.groupNameFirst);
        return;
      }
      await api.post(`/api/v1/torrents/${torrentId}/group`, { name: nm });
      setMsg(t.grouped);
      setGroupName("");
    });

  const FLD =
    "rounded-[var(--r-sm)] border border-line px-2 py-1 text-xs";
  return (
    <div className="td-curate">
      <p className="td-curate__head">{t.title}</p>
      <div className="td-curate__row">
        <select
          className={FLD}
          value={picked}
          onChange={(e) => setPicked(e.target.value)}
          aria-label={t.collectionAria}
        >
          <option value="">
            {cols.length === 0 ? t.noCollections : t.pickCollection}
          </option>
          {cols.map((c) => (
            <option key={c.id} value={c.id}>
              {c.kind === "series" ? `★ ${c.name}` : c.name}
            </option>
          ))}
        </select>
        <input
          className={FLD}
          value={newCol}
          placeholder={t.newCollection}
          onChange={(e) => setNewCol(e.target.value)}
        />
        <button
          type="button"
          className="td-curate__btn"
          disabled={busy}
          onClick={() => void onCollect()}
        >
          {t.collect}
        </button>
      </div>
      <div className="td-curate__row">
        <input
          className={FLD}
          value={groupName}
          placeholder={t.groupPlaceholder}
          onChange={(e) => setGroupName(e.target.value)}
        />
        <button
          type="button"
          className="td-curate__btn"
          disabled={busy}
          onClick={() => void onGroup()}
        >
          {t.group}
        </button>
      </div>
      {msg && <p className="td-curate__msg">{msg}</p>}
      <p className="td-curate__hint">{t.reloadHint}</p>
    </div>
  );
}
