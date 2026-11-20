/** 申诉队列面板（从 admin-queue-panels.tsx 拆出，300 行门禁）。 */
import { PANEL_LG } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import type { AppealRow } from "./admin-shared";

/** 待审时长（0286）：旧版取了 created_at 却从不渲染，压了三天看上去还是三天。 */
export function waitingText(iso: string): string {
  const mins = Math.max(
    0,
    Math.floor((Date.now() - new Date(iso).getTime()) / 60000),
  );
  if (mins < 60) return `${mins}m`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 48) return `${hrs}h`;
  return `${Math.floor(hrs / 24)}d`;
}

export function AppealsPanel({
  appeals,
  onHandle,
}: {
  appeals: AppealRow[];
  onHandle: (id: number, accept: boolean) => void;
}) {
  const { dict } = useI18n();
  const a = dict.admin as unknown as Record<string, string>;
  // 申诉类型四值（后端 appeals.rs CHECK），与用户侧 /appeals 同一组文案
  const KIND: Record<string, string> = {
    ban: dict.appeals.kindBan,
    hr: dict.appeals.kindHr,
    warn: dict.appeals.kindWarn,
    other: dict.appeals.kindOther,
  };
  return (
    <section className={PANEL_LG}>
      <ul className="flex flex-col divide-y divide-line">
        {appeals.map((ap) => (
          <li
              key={ap.id}
              className="flex flex-wrap items-center gap-2 py-2 sm:gap-3"
            >
            <div className="w-full min-w-0 sm:w-auto sm:flex-1">
              <p className="text-sm">
                <span className="rounded-full bg-sun/30 px-2 py-0.5 text-xs">
                  {KIND[ap.kind] ?? ap.kind}
                </span>{" "}
                <b>{ap.username}</b>
                {ap.ref_id !== null && (
                  <span className="text-xs text-sub"> · #{ap.ref_id}</span>
                )}
                {ap.status !== "open" && (
                  <span
                    className={`ml-1 rounded-full px-2 py-0.5 text-xs ${
                      ap.status === "accepted"
                        ? "bg-mint/30"
                        : "bg-coral/20 text-[var(--danger-text)]"
                    }`}
                  >
                    {ap.status === "accepted"
                      ? a.appealAccepted
                      : a.appealRejected}
                  </span>
                )}
              </p>
              <p className="text-xs text-sub">
                {ap.body}
                {ap.result_note
                  ? ` · ${a.appealNoteLabel}: ${ap.result_note}`
                  : ""}
              </p>
            </div>
            {ap.status === "open" && (
              <span className="ux-review-actions">
                <button
                  onClick={() => onHandle(ap.id, true)}
                  className="ux-tap rounded-full bg-mint px-4 text-xs
                    font-bold text-[var(--text-on-accent)]"
                >
                  {a.appealAccept}
                </button>
                <button
                  onClick={() => onHandle(ap.id, false)}
                  className="ux-tap rounded-full border border-line px-4 text-xs
                    font-bold text-[var(--danger-text)]"
                >
                  {a.appealReject}
                </button>
              </span>
            )}
          </li>
        ))}
        {appeals.length === 0 && (
          <li className="py-6 text-center text-sub">{a.appealEmpty}</li>
        )}
      </ul>
    </section>
  );
}
