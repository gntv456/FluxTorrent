"use client";

import { BTN_MD_SKY } from "@/lib/ui-classes";

/**
 * 首页排版编辑器（0089）：站长可视化调整首页板块——顺序（上移/下移）、
 * 宽度档（1/3、2/3、整行/自动）、显示/隐藏（从列表移除后可从「未展示」加回）。
 * 保存走 PUT /api/v1/admin/home-layout（后端强校验白名单/去重/span），
 * 空列表保存 = 恢复默认排版。实时预览：编辑状态即渲染 home-stack--custom 结构。
 */

import { useEffect, useMemo, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import {
  defaultLayout,
  parseHomeLayout,
  type HomeLayoutItem,
  type HomeSectionMeta,
} from "@/components/home-layout";

const SPAN_OPTIONS: [number, string][] = [
  [0, "auto"],
  [1, "1/3"],
  [2, "2/3"],
  [3, "整行"],
];

export function HomeLayoutEditor() {
  const { dict } = useI18n();
  const t = dict.homeLayout;
  const [items, setItems] = useState<HomeLayoutItem[] | null>(null);
  // 板块清单同样只认后端下发的那一份（四审 L6 单源化）
  const [sections, setSections] = useState<HomeSectionMeta[]>([]);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // 初始值 = 当前线上配置（/api/v1/home.home_layout；空/非法回默认排版）
  useEffect(() => {
    const fallback = (s: HomeSectionMeta[]) => defaultLayout(s);
    api
      .get<{
        home_layout?: string;
        home_sections?: HomeSectionMeta[];
      }>("/api/v1/home")
      .then((h) => {
        const secs = h.home_sections ?? [];
        setSections(secs);
        setItems(parseHomeLayout(h.home_layout, secs));
      })
      .catch(() => {
        setSections([]);
        setItems(fallback([]));
      });
  }, []);

  const hiddenKeys = useMemo(
    () =>
      sections
        .filter((s) => !items?.some((i) => i.key === s.key))
        .map((s) => s.key),
    [items, sections],
  );

  function move(idx: number, dir: -1 | 1) {
    setItems((prev) => {
      if (!prev) return prev;
      const next = [...prev];
      const j = idx + dir;
      if (j < 0 || j >= next.length) return prev;
      [next[idx], next[j]] = [next[j], next[idx]];
      return next;
    });
  }

  function setSpan(idx: number, span: number) {
    setItems(
      (prev) =>
        prev?.map((it, i) => (i === idx ? { ...it, span } : it)) ?? prev,
    );
  }

  function remove(key: string) {
    setItems((prev) => prev?.filter((it) => it.key !== key) ?? prev);
  }

  function add(key: string) {
    setItems((prev) => [...(prev ?? []), { key, span: 0 }]);
  }

  async function save() {
    if (!items || busy) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.put("/api/v1/admin/home-layout", items);
      setMsg(t.saved);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function resetDefault() {
    // 「恢复默认」= 后端清单里 in_default 的那批、按清单顺序（不再前端自带副本）
    setItems(defaultLayout(sections).map((x) => ({ ...x })));
    setMsg(t.resetHint);
  }

  if (!items) return <p className="text-sm text-sub">{dict.my.loading}</p>;

  return (
    <div className="flex flex-col gap-3">
      <p className="text-xs text-sub">{t.hint}</p>
      <ol className="flex flex-col gap-2">
        {items.map((it, idx) => (
          <li
            key={it.key}
            className="flex flex-wrap items-center gap-2 rounded-[var(--r-sm)] border border-line bg-cloud px-3 py-2"
          >
            <b className="min-w-20 text-sm">
              {(t.names as Record<string, string>)[it.key] ?? it.key}
              <code className="ml-2 text-[10px] text-sub">{it.key}</code>
            </b>
            <select
              value={it.span}
              onChange={(e) => setSpan(idx, Number(e.target.value))}
              className="min-h-[34px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2 text-xs"
              aria-label={t.spanLabel}
            >
              {SPAN_OPTIONS.map(([v, label]) => (
                <option key={v} value={v}>
                  {label === "auto"
                    ? t.spanAuto
                    : label === "1/3"
                      ? t.span13
                      : label === "2/3"
                        ? t.span23
                        : t.spanFull}
                </option>
              ))}
            </select>
            <span className="ml-auto flex items-center gap-1">
              <button
                type="button"
                onClick={() => move(idx, -1)}
                disabled={idx === 0}
                className="min-h-[34px] rounded-[var(--r-sm)] border border-line px-2 text-xs disabled:opacity-40"
                aria-label={t.up}
              >
                ↑
              </button>
              <button
                type="button"
                onClick={() => move(idx, 1)}
                disabled={idx === items.length - 1}
                className="min-h-[34px] rounded-[var(--r-sm)] border border-line px-2 text-xs disabled:opacity-40"
                aria-label={t.down}
              >
                ↓
              </button>
              <button
                type="button"
                onClick={() => remove(it.key)}
                className="min-h-[34px] rounded-[var(--r-sm)] border border-[var(--baozi-orange)] px-2 text-xs text-[var(--baozi-orange-dark)]"
              >
                {t.hide}
              </button>
            </span>
          </li>
        ))}
      </ol>

      {hiddenKeys.length > 0 && (
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs text-sub">{t.hidden}：</span>
          {hiddenKeys.map((k) => (
            <button
              key={k}
              type="button"
              onClick={() => add(k)}
              className="min-h-[32px] rounded-full border border-line px-3 text-xs"
            >
              + {(t.names as Record<string, string>)[k] ?? k}
            </button>
          ))}
        </div>
      )}

      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onClick={save}
          disabled={busy}
          className={BTN_MD_SKY}
        >
          {busy ? t.saving : t.save}
        </button>
        <button
          type="button"
          onClick={resetDefault}
          className="min-h-[40px] rounded-full border border-line px-4 text-sm"
        >
          {t.reset}
        </button>
        {msg && <span className="text-xs text-sub">{msg}</span>}
      </div>
    </div>
  );
}
