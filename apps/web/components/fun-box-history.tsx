"use client";

import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

import type { FunItem } from "./fun-box";

/** 趣味盒前台·历史列表（从 components/fun-box.tsx 按域拆出）：
 *  「更多」折叠区 + 逐条恢复/禁止/删除。 */

export function FunHistory({
  history,
  showMore,
  setShowMore,
  setStatus,
  remove,
}: {
  history: FunItem[];
  showMore: boolean;
  setShowMore: React.Dispatch<React.SetStateAction<boolean>>;
  setStatus: (id: number, status: "banned" | "normal") => void;
  remove: (id: number) => void;
}) {
  const i18n = useI18n();
  const t = i18n.dict.funbox;

  return (
    <>
      <button
        type="button"
        className="funbox__more-toggle"
        onClick={() => setShowMore((v) => !v)}
      >
        {showMore ? "▴ " + t.collapse : `▾ ${t.more} (${history.length})`}
      </button>
      {showMore && (
        <ul className="funbox__history">
          {history.map((h) => (
            <li
              key={h.id}
              className={h.status === "banned" ? "is-banned" : undefined}
            >
              <div className="funbox__history-main">
                <strong>{h.title}</strong>
                <span className="funbox__history-meta">
                  {h.username ?? "—"} ·{" "}
                  {new Date(h.added).toLocaleDateString(dateLocale(i18n.locale))} · 😂{" "}
                  {h.fun_votes ?? 0} / 😑 {h.dull_votes ?? 0}
                  {h.status === "banned" && ` · ${t.bannedTag}`}
                </span>
              </div>
              <div className="funbox__acts">
                {h.status === "banned" ? (
                  <button
                    type="button"
                    className="cmgmt-act cmgmt-act--ok"
                    onClick={() => setStatus(h.id, "normal")}
                  >
                    {t.restore}
                  </button>
                ) : (
                  <button
                    type="button"
                    className="cmgmt-act cmgmt-act--danger"
                    onClick={() => setStatus(h.id, "banned")}
                  >
                    {t.ban}
                  </button>
                )}
                <button
                  type="button"
                  className="cmgmt-act cmgmt-act--danger"
                  onClick={() => remove(h.id)}
                >
                  {t.delete}
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}
