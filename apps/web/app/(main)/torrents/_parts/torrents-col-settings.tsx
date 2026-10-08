"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/**
 * 用户级列设置（0316）：勾选要隐藏的列，写进 users.list_prefs。
 *
 * 站点级列隐藏（site_settings.view_hidden）是全体生效的；本组件是**个人覆盖**，
 * 叠加在站点级之上（见列表页 hiddenCols 合并口径）。title/选择/行为列恒显，
 * 不在此处，故不出现在可选项里。
 *
 * 保存后整页刷新（`location.reload`）：列显隐影响 SSR 产出的表头/单元格，
 * 客户端状态重渲反而会闪——直接重取最稳。
 */

/** 可隐藏列（与后端 COL_KEYS 白名单严格一致，顺序即 UI 顺序） */
const COL_KEYS = [
  "cat",
  "cover",
  "comments",
  "alive",
  "size",
  "seeders",
  "leechers",
  "completed",
] as const;

export function TorrentsColSettings({
  initialHidden,
}: {
  initialHidden: string[];
}) {
  const { dict } = useI18n();
  const t = dict.torrents;
  const [open, setOpen] = useState(false);
  const [hidden, setHidden] = useState<Set<string>>(
    new Set(initialHidden),
  );
  const [saving, setSaving] = useState(false);

  // 站点级已隐藏的列，用户不该在个人层「取消」——置灰提示
  const siteHidden = new Set(initialHidden);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);

  const toggle = (k: string) => {
    setHidden((prev) => {
      const next = new Set(prev);
      if (next.has(k)) next.delete(k);
      else next.add(k);
      return next;
    });
  };

  const save = async () => {
    setSaving(true);
    try {
      await api.post("/api/v1/me/list-prefs", {
        key: "hidden_cols",
        value: [...hidden],
      });
      window.location.reload();
    } catch {
      setSaving(false);
    }
  };

  return (
    <div className="torrents-colset">
      <button
        type="button"
        className="torrents-colset__btn"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        ⚙ {t.colSettings}
      </button>
      {open && (
        <div className="torrents-colset__panel" role="dialog">
          <p className="torrents-colset__hint">{t.colSettingsHint}</p>
          <ul className="torrents-colset__list">
            {COL_KEYS.map((k) => (
              <li key={k}>
                <label className="torrents-colset__item">
                  <input
                    type="checkbox"
                    checked={!hidden.has(k)}
                    disabled={siteHidden.has(k)}
                    onChange={() => toggle(k)}
                  />
                  <span>{t.colNames[k] ?? k}</span>
                </label>
              </li>
            ))}
          </ul>
          <div className="torrents-colset__ops">
            <button
              type="button"
              className="torrents-colset__save"
              disabled={saving}
              onClick={save}
            >
              {saving ? t.colSaving : t.colSave}
            </button>
            <button
              type="button"
              className="torrents-colset__cancel"
              onClick={() => setOpen(false)}
            >
              {dict.common.cancel}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
