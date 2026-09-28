"use client";

import { BTN_MD_SKY } from "@/lib/ui-classes";

/**
 * 视图布局编辑器（E6）：站点级「种子列表列 / 详情页段落」显隐。
 * 只裁显隐不改顺序（列序是油猴脚本的 DOM 契约）；title 列后端锁定不可隐藏。
 * 保存走 PUT /api/v1/admin/view-hidden（白名单校验），空选择 = 全恢复默认。
 * 键清单由后端单源下发（GET /api/v1/view-layout，含 columns/sections 键集）。
 */

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

type Kind = "columns" | "sections";

const CHIP_CLS = "rounded-[var(--r-sm)] border px-3 py-1.5 text-xs";
const CHIP_LOCKED =
  "cursor-not-allowed border-line bg-cloud text-sub opacity-60";
const CHIP_OFF = "border-line bg-cloud text-sub line-through";
const CHIP_ON = "border-sky bg-sky/10 text-ink";
const RESET_CLS = "rounded-[var(--r-sm)] border border-line px-3 py-1.5";

export function ViewHiddenEditor() {
  const { dict } = useI18n();
  const t = dict.viewLayout;
  // 键全集（后端单源）+ 当前隐藏集
  const [keys, setKeys] = useState<Record<Kind, string[]> | null>(null);
  const [hidden, setHidden] = useState<Record<Kind, string[]>>({
    columns: [],
    sections: [],
  });
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    Promise.all([
      api.get<Record<Kind, string[]>>("/api/v1/view-layout"),
      api.get<{ view_hidden?: Record<Kind, string[]> }>(
        "/api/v1/site-profile",
      ),
    ])
      .then(([def, prof]) => {
        setKeys(def);
        setHidden({
          columns: prof.view_hidden?.columns ?? [],
          sections: prof.view_hidden?.sections ?? [],
        });
      })
      .catch(() => setKeys({ columns: [], sections: [] }));
  }, []);

  function toggle(kind: Kind, key: string) {
    setHidden((prev) => {
      const cur = prev[kind];
      return {
        ...prev,
        [kind]: cur.includes(key)
          ? cur.filter((k) => k !== key)
          : [...cur, key],
      };
    });
  }

  async function save() {
    if (busy) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.put("/api/v1/admin/view-hidden", hidden);
      setMsg(t.saved);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  if (!keys) return <p className="text-sm text-sub">{dict.my.loading}</p>;

  const groups: [Kind, string][] = [
    ["columns", t.columnsTitle],
    ["sections", t.sectionsTitle],
  ];

  return (
    <div className="flex flex-col gap-4">
      <p className="text-xs text-sub">{t.hint}</p>
      {groups.map(([kind, title]) => (
        <div key={kind}>
          <p className="mb-2 text-sm font-medium">{title}</p>
          <div className="flex flex-wrap gap-2">
            {(keys[kind] ?? []).map((k) => {
              const off = hidden[kind].includes(k);
              const locked = kind === "columns" && k === "title";
              return (
                <button
                  key={k}
                  type="button"
                  disabled={locked}
                  onClick={() => toggle(kind, k)}
                  className={`${CHIP_CLS} ${
                    locked
                      ? CHIP_LOCKED
                      : off
                        ? CHIP_OFF
                        : CHIP_ON
                  }`}
                  title={locked ? t.titleLocked : undefined}
                >
                  {(t.names as Record<string, string>)[k] ?? k}
                  {locked ? ` (${t.locked})` : ""}
                </button>
              );
            })}
          </div>
        </div>
      ))}
      <div className="flex items-center gap-3">
        <button
          type="button"
          className={BTN_MD_SKY}
          onClick={save}
          disabled={busy}
        >
          {t.save}
        </button>
        <button
          type="button"
          className={RESET_CLS}
          onClick={() => setHidden({ columns: [], sections: [] })}
        >
          {t.reset}
        </button>
        {msg && <span className="text-xs text-sub">{msg}</span>}
      </div>
    </div>
  );
}
