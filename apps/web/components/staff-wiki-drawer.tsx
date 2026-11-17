"use client";

/** Wiki 管理面板子件（0495 从 staff-wiki.tsx 拆出——section 字段把面板
 *  顶过 300 行门禁）：版本历史抽屉（只读，回滚不做）。
 *  类型 WikiRevision 从 staff-wiki-shared.ts 复用，不另立口径。 */

import { EmptyState } from "@/components/ui/empty-state";
import type { WikiRevision } from "@/components/staff-wiki-shared";

export function WikiRevDrawer({
  revs,
  dict,
  onClose,
}: {
  revs: WikiRevision[] | null;
  dict: {
    adminWiki: { revisionsTitle: string; noRevisions: string };
    common: { cancel: string };
  };
  onClose: () => void;
}) {
  return (
    <div className="mb-4 rounded-[var(--r-md)] border border-line p-3">
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-sm font-bold">{dict.adminWiki.revisionsTitle}</h3>
        <button type="button" className="text-xs underline" onClick={onClose}>
          {dict.common.cancel}
        </button>
      </div>
      <ul className="flex max-h-72 flex-col gap-2 overflow-y-auto text-xs">
        {(revs ?? []).map((r) => (
          <li
            key={r.id}
            className="rounded-[var(--r-sm)] border border-line p-2"
          >
            <div className="flex flex-wrap items-center gap-2">
              <span className="font-bold">{r.title}</span>
              <span className="text-sub">
                {r.edited_by_name ?? "-"} ·{" "}
                {r.created_at.slice(0, 16).replace("T", " ")}
              </span>
            </div>
            <pre
              className="mt-1 max-h-24 overflow-y-auto whitespace-pre-wrap \
font-mono text-xs text-sub"
            >
              {r.body.slice(0, 500)}
            </pre>
          </li>
        ))}
        {revs !== null && revs.length === 0 && (
          <li>
            <EmptyState
              icon="info"
              compact
              title={dict.adminWiki.noRevisions}
            />
          </li>
        )}
      </ul>
    </div>
  );
}
