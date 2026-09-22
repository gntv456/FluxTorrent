"use client";

import { useCallback, useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";

/**
 * 列表页批量下载（阶段三「最强」：UNIT3D 都没有的能力）。
 *
 * 状态不引状态库：复选框把选中态留在 DOM，操作条订阅 `batch-change` 事件
 * 读 `input.batch-ck:checked` —— 与 hotkeys 同一套「server 渲染 + DOM 协作」思路，
 * 避免为一次多选引入 context/全局 store。
 *
 * 下载走同源 `/api/v1/...`（Next rewrites 代理，HttpOnly cookie 自动携带），
 * 拿到 zip 二进制后用 object URL 触发浏览器下载。
 */

/** 每行的选择框（值 = 种子 id），供 TorrentTr 渲染 */
export function BatchCheckbox({ id }: { id: number }) {
  return (
    <input
      type="checkbox"
      className="batch-ck"
      data-batch-id={id}
      aria-label={`select-${id}`}
      onChange={() =>
        document.dispatchEvent(new CustomEvent("batch-change"))
      }
    />
  );
}

/** 表头全选：作用于当前页全部 `.batch-ck` */
export function BatchCheckAll() {
  const { dict } = useI18n();
  return (
    <input
      type="checkbox"
      className="batch-all"
      aria-label={dict.torrents.batchSelectAll}
      title={dict.torrents.batchSelectAll}
      onChange={(e) => {
        const on = e.currentTarget.checked;
        for (const el of document.querySelectorAll<HTMLInputElement>(
          "input.batch-ck",
        )) {
          el.checked = on;
        }
        document.dispatchEvent(new CustomEvent("batch-change"));
      }}
    />
  );
}

/** 批量操作条：仅在有选中项时出现 */
export function TorrentBatchBar() {
  const { dict } = useI18n();
  const t = dict.torrents;
  const [ids, setIds] = useState<number[]>([]);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  const sync = useCallback(() => {
    const checked = document.querySelectorAll<HTMLInputElement>(
      "input.batch-ck:checked",
    );
    setIds(
      Array.from(checked)
        .map((el) => Number(el.dataset.batchId))
        .filter((n) => Number.isFinite(n) && n > 0),
    );
  }, []);

  useEffect(() => {
    document.addEventListener("batch-change", sync);
    // 表格由 RSC 渲染，可能晚于本组件挂载 → 挂载后再同步一次
    sync();
    return () => document.removeEventListener("batch-change", sync);
  }, [sync]);

  const clear = () => {
    for (const el of document.querySelectorAll<HTMLInputElement>(
      "input.batch-ck",
    )) {
      el.checked = false;
    }
    const all =
      document.querySelector<HTMLInputElement>("input.batch-all");
    if (all) all.checked = false;
    document.dispatchEvent(new CustomEvent("batch-change"));
  };

  async function download() {
    if (!ids.length || busy) return;
    setBusy(true);
    setMsg(null);
    try {
      const res = await fetch("/api/v1/torrents/batch-download", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ ids }),
      });
      if (!res.ok) throw new Error(String(res.status));
      const skipped = Number(res.headers.get("X-Batch-Skipped") ?? "0");
      const blob = await res.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `fluxtorrent-${ids.length}items.zip`;
      a.click();
      URL.revokeObjectURL(url);
      if (skipped > 0) {
        setMsg(t.batchSkipped.replace("{n}", String(skipped)));
      }
      clear();
    } catch {
      setMsg(dict.common.loadFailed);
    } finally {
      setBusy(false);
    }
  }

  if (ids.length === 0) return null;
  return (
    <div className="tsb-batch" role="status">
      <span className="num tsb-batch__n">
        {t.batchSelected.replace("{n}", String(ids.length))}
      </span>
      <button
        type="button"
        className="tsb-batch__go"
        disabled={busy}
        onClick={download}
      >
        {busy ? "…" : t.batchDownload}
      </button>
      <button type="button" className="tsb-batch__clear" onClick={clear}>
        {t.batchClear}
      </button>
      {msg && <span className="tsb-batch__msg">{msg}</span>}
    </div>
  );
}
