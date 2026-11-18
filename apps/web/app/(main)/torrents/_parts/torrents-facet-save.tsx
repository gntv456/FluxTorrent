"use client";
/**
 * 分面筛选的保存 / 一键恢复（A2）：URL 是唯一筛选真源，localStorage 只存
 * 一条可分享的 /torrents 链接；恢复走原生导航，保证可后退、可再分享。
 * 文案由服务端注入 dict.uxOpt，本件不写任何语言字面量。
 */
import { useEffect, useState } from "react";
import type { TorrentsSP } from "./torrents-utils";
import { ActionFeedback } from "@/components/ui/action-feedback";

const KEY = "flux-saved-filters";
/** 游标 / 分页不进快照：恢复时不能把旧数据集的游标带回来 */
const DROP = new Set(["cursor", "p", "dir"]);

function currentHref(sp: TorrentsSP): string {
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(sp)) {
    if (v && !DROP.has(k)) qs.set(k, v);
  }
  const s = qs.toString();
  return s ? `/torrents?${s}` : "/torrents";
}

/** 只读自己写过的链接：防手工篡改 localStorage 造成开放重定向 */
function readHref(raw: string): string | null {
  try {
    const parsed = JSON.parse(raw) as { href?: unknown };
    return typeof parsed.href === "string" &&
      parsed.href.startsWith("/torrents")
      ? parsed.href
      : null;
  } catch {
    return null;
  }
}

export function FacetSaveButton({
  sp,
  saveLabel,
  savedLabel,
}: {
  sp: TorrentsSP;
  saveLabel: string;
  savedLabel: string;
}) {
  const [hasSaved, setHasSaved] = useState(false);
  const [justSaved, setJustSaved] = useState(false);

  useEffect(() => {
    try {
      const raw = localStorage.getItem(KEY);
      setHasSaved(Boolean(raw && readHref(raw)));
    } catch {
      /* 隐私模式不可用：静默降级，不影响筛选本身 */
    }
  }, []);

  const save = () => {
    try {
      localStorage.setItem(
        KEY,
        JSON.stringify({ href: currentHref(sp), at: Date.now() }),
      );
      setHasSaved(true);
      setJustSaved(true);
    } catch {
      /* 同上 */
    }
  };

  const restore = () => {
    try {
      const raw = localStorage.getItem(KEY);
      const href = raw ? readHref(raw) : null;
      if (href) window.location.assign(href);
    } catch {
      /* 同上 */
    }
  };

  return (
    <div className="ux-facets__save">
      <button
        type="button"
        className="ux-facets__save-btn"
        data-ux-event="filter_save"
        onClick={save}
      >
        {saveLabel}
      </button>
      {hasSaved && (
        <button
          type="button"
          className="ux-facets__save-btn"
          onClick={restore}
        >
          {savedLabel}
        </button>
      )}
      <ActionFeedback
        state={justSaved ? "ok" : "idle"}
        okLabel={savedLabel}
      />
    </div>
  );
}
