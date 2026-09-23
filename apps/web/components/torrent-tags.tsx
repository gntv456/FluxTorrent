"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n, apiErrorMessage } from "@/i18n/client";

interface TagDictRow {
  id: number;
  name: string;
  kind: string;
  /** 0159 P1：字典样式列（详情页已选标签消费；旧元组形态缺省回落轮换色） */
  bg_color?: string;
  color?: string;
}

/** 兼容 API 历史形态：dict 行可能序列化为 [id, name, kind] 元组 */
export function normTagRow(r: TagDictRow | [number, string, string]): TagDictRow {
  return Array.isArray(r) ? { id: r[0], name: r[1], kind: r[2] } : r;
}

/** /torrents/{id}/aggregate 内嵌的标签负载（批次三：RSC 预取后免一次水合请求） */
export interface TagPayload {
  dict: (TagDictRow | [number, string, string])[];
  mine: number[];
}

/** 种子标签（T-04）：作者/staff 打标，官种/官方标签仅 staff；点击切换 */
export function TorrentTags({
  torrentId,
  initial,
}: {
  torrentId: number;
  /** 详情页 aggregate 已带回时直接使用，不再水合后二次请求 */
  initial?: TagPayload;
}) {
  const { dict } = useI18n();
  const t = dict.torrTags2 ?? {
    title: "标签",
    needStaff: "官方标签仅管理组可打",
    addTag: "+ 加标签",
  };
  const [dictRows, setDictRows] = useState<TagDictRow[]>(() =>
    initial ? initial.dict.map(normTagRow) : [],
  );
  const [mine, setMine] = useState<number[]>(() => initial?.mine ?? []);
  const [msg, setMsg] = useState<string | null>(null);
  // 0159：默认只显示已选中的标签（此前全字典铺开，未选的也被渲染出来）。
  // 「全部标签」展开打标视图。hook 必须在任何 early return 之前（rules-of-hooks）。
  const [expanded, setExpanded] = useState(false);
  // 三态（方案 P0-6）：此前加载中/失败都走 `return null` 静默消失，用户以为「这个种子本来没标签」
  const [state, setState] = useState<"loading" | "ready" | "error">(
    initial ? "ready" : "loading",
  );

  const load = useCallback(async () => {
    setState("loading");
    try {
      const r = await api.get<{
        dict: (TagDictRow | [number, string, string])[];
        mine: number[];
      }>(`/api/v1/torrents/${torrentId}/tags`);
      setDictRows(r.dict.map(normTagRow));
      setMine(r.mine);
      setState("ready");
    } catch {
      setState("error");
    }
  }, [torrentId]);

  useEffect(() => {
    if (!initial) load();
    // initial 只在挂载时判定一次（重试按钮仍走 load 重新拉取）
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function toggle(tagId: number, on: boolean) {
    setMsg(null);
    try {
      await api.put(`/api/v1/torrents/${torrentId}/tags`, {
        tag_id: tagId,
        on: !on,
      });
      setMine((m) => (on ? m.filter((x) => x !== tagId) : [...m, tagId]));
    } catch (e) {
      setMsg(apiErrorMessage(dict, e));
    }
  }

  if (state === "error") {
    return (
      <div className="td-tagcloud" role="status">
        <span className="text-[11px] text-sub">{dict.common.loadFailed}</span>
        <button
          type="button"
          className="td-tag td-tag--off"
          onClick={() => load()}
        >
          {dict.common.retry}
        </button>
      </div>
    );
  }
  if (state === "loading") {
    return (
      <div className="td-tagcloud" aria-busy="true">
        {[0, 1, 2].map((i) => (
          <span
            key={i}
            className="td-tag td-tag--off"
            style={{ width: 68, opacity: 0.45 }}
            aria-hidden="true"
          >
            &nbsp;
          </span>
        ))}
      </div>
    );
  }
  if (dictRows.length === 0) return null;
  const selected = dictRows.filter((d) => mine.includes(d.id));
  const rows = expanded ? dictRows : selected;
  if (selected.length === 0 && !expanded) {
    return (
      <div className="td-tagcloud">
        <button
          type="button"
          className="td-tag td-tag--off"
          onClick={() => setExpanded(true)}
        >
          {t.addTag ?? t.showAll}
        </button>
        {msg && (
          <span className="text-[11px] text-sub" role="status">
            {msg}
          </span>
        )}
      </div>
    );
  }

  return (
    <div className="td-tagcloud">
      {rows.map((d, i) => {
        const on = mine.includes(d.id);
        const official = d.kind === "official";
        // 0159 P1：渲染统一——字典配了 bg_color 时用字典色（与论坛 TagChip 同口径），
        // 未配色回落既有列表口径的彩色轮换（官方类固定靛蓝，普通标签按位轮换品牌色）
        const palette = [
          "torrents-tag--sky",
          "torrents-tag--mint",
          "torrents-tag--sun",
          "torrents-tag--candy",
        ];
        const colorCls = official
          ? "torrents-tag--official"
          : palette[i % palette.length];
        return (
          <button
            key={d.id}
            type="button"
            onClick={() => toggle(d.id, on)}
            title={official ? t.needStaff : d.name}
            className={`td-tag ${colorCls} ${on ? "" : "td-tag--off"}`}
            style={
              d.bg_color
                ? {
                    background: on ? d.bg_color : undefined,
                    color: on ? d.color || undefined : undefined,
                  }
                : undefined
            }
          >
            {d.name}
          </button>
        );
      })}
      {!expanded ? (
        <button
          type="button"
          className="td-tag td-tag--off"
          onClick={() => setExpanded(true)}
        >
          {t.showAll}
        </button>
      ) : (
        <button
          type="button"
          className="td-tag td-tag--off"
          onClick={() => setExpanded(false)}
        >
          {t.hideAll}
        </button>
      )}
      {msg && (
        <span className="text-[11px] text-sub" role="status">
          {msg}
        </span>
      )}
    </div>
  );
}
