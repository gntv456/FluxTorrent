"use client";

/**
 * 自动成就进度面板（0382 的前端面）。
 *
 * 设计要点：
 *   1. **未达成的排前面**——用户打开这一页关心的是「还差什么」。
 *      后端已排好序，这里不再二次排。
 *   2. **`current: null` 显式说「口径未接上」**，而不是画一条 0% 的
 *      进度条让人以为自己没努力。对应后端返回的 `unwired`。
 *   3. **进度不外露时（progress_visible=false）不给数字**——有些成就
 *      刻意保密，避免「还差 3 小时」这种压力。
 *   4. 样式走语义变量（--text-* / --border-* / --accent），不写死色值
 *      （ui_design_guard_hex 六位 hex 禁令）。
 */

import { useCallback, useEffect, useState } from "react";

import { api } from "@/lib/api-client";
import { StatePanel } from "@/components/ui/state-panel";

interface ProgressItem {
  id: number;
  name: string;
  description?: string | null;
  rarity?: string | null;
  rule: string;
  unit: string;
  /** null = 取数口径尚未接线（**不是**「进度为 0」） */
  current: number | null;
  target: number;
  ratio: number;
  text: string;
  awarded: boolean;
  awarded_at?: string | null;
  wearing: boolean;
  progress_visible: boolean;
  sort_order: number;
}

interface ProgressEnvelope {
  items: ProgressItem[];
  stats: {
    uploaded_gb: number;
    seed_hours: number;
    uploads: number;
    registered_days: number;
  };
  /** 取数口径未接上的规则名 */
  unwired: string[];
}

export function MedalProgress() {
  const [data, setData] = useState<ProgressEnvelope | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setData(await api.get<ProgressEnvelope>("/api/v1/medals/progress"));
      setErr(null);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <StatePanel
      loading={data === null && err === null}
      error={err}
      empty={data !== null && data.items.length === 0}
      onRetry={() => void load()}
    >
      {data && (
        <div className="flex flex-col gap-4">
          <div className="flex items-baseline justify-between gap-3">
            <span className="text-sm text-[var(--text-muted)]">
              {data.items.filter((i) => i.awarded).length} /{" "}
              {data.items.length}
            </span>
            {data.unwired.length > 0 && (
              // 口径未接上是**站点侧**的问题，不是用户的错，故不用错误色
              <span className="text-xs text-[var(--text-faint)]">
                {data.unwired.length}
              </span>
            )}
          </div>

          <ul className="flex flex-col gap-3">
            {data.items.map((it) => (
              <li
                key={it.id}
                className="rounded-lg border
                           border-[var(--border-soft)] p-3"
              >
                <div className="flex items-center justify-between gap-3">
                  <span
                    className={
                      it.awarded
                        ? "font-medium text-[var(--text-strong)]"
                        : "text-[var(--text-body)]"
                    }
                  >
                    {it.name}
                  </span>
                  {it.awarded && (
                    <span className="text-xs text-[var(--accent)]">✓</span>
                  )}
                </div>

                {it.description && (
                  <p className="mt-1 text-sm text-[var(--text-muted)]">
                    {it.description}
                  </p>
                )}

                {it.current === null ? (
                  // 口径未接上：明说「暂无数据」，不画 0% 进度条
                  <span className="mt-2 block text-sm
                                   text-[var(--text-fainter)]">
                    —
                  </span>
                ) : it.awarded ? null : it.progress_visible ? (
                  <div className="mt-2 flex flex-col gap-1">
                    <div
                      className="h-1.5 w-full overflow-hidden
                                 rounded-full
                                 bg-[var(--border-soft)]"
                      role="progressbar"
                      aria-valuenow={Math.round(it.ratio * 100)}
                      aria-valuemin={0}
                      aria-valuemax={100}
                      aria-label={it.name}
                    >
                      <div
                        className="h-full rounded-full
                                   bg-[var(--accent)]"
                        style={{
                          width: `${Math.round(it.ratio * 100)}%`,
                        }}
                      />
                    </div>
                    <span className="text-xs text-[var(--text-muted)]">
                      {it.text}
                    </span>
                  </div>
                ) : (
                  // 刻意不外露进度的成就：只给一个未达成记号，不给数字
                  <span className="mt-2 block text-sm
                                     text-[var(--text-faint)]">
                    ○
                  </span>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
    </StatePanel>
  );
}
