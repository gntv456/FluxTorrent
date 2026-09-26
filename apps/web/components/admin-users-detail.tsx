"use client";

import { BTN_SM_BOLD, INPUT_MD } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import { AdminUserFieldsPanel } from "@/components/admin-user-fields-panel";

/** 用户详情面板（从 admin-users.tsx 按域拆出，300 门禁）：
 *  好学站用户详情页口径——字段全景 + 管理动作（数值调整 / 下载权限 / 挂起）。 */

interface AdminUserRow {
  id: number;
  username: string;
  email: string;
  class_id: number;
  class_name: string | null;
  uploaded: number;
  downloaded: number;
  status: number;
  download_enabled: boolean;
  suspended: boolean;
  created_at: string;
  last_seen_at: string | null;
}

export interface AdminUserDetail extends AdminUserRow {
  passkey: string;
  title: string | null;
  spark_balance: number;
  parked: boolean;
  donor: boolean;
  totp_enabled: boolean;
  invited_by: number | null;
  inviter_name: string | null;
  seeding: number;
  leeching: number;
  uploads: number;
  invites_unused: number;
}

export function fmtBytes(n: number): string {
  if (n >= 1099511627776) return `${(n / 1099511627776).toFixed(2)} TB`;
  if (n >= 1073741824) return `${(n / 1073741824).toFixed(2)} GB`;
  if (n >= 1048576) return `${(n / 1048576).toFixed(2)} MB`;
  return `${(n / 1024).toFixed(2)} KB`;
}

/** 数值调整表单的状态形状（delta 语义，见下方表单） */
export interface AdjustState {
  up: string;
  down: string;
  spark: string;
  invite: string;
  note: string;
}

export function UserDetailPanel({
  detail,
  onClose,
  onAdjust,
  submitAdjust,
  toggleFlag,
  adjust,
  setAdjust,
  busy,
  currency,
}: {
  detail: AdminUserDetail;
  onClose: () => void;
  onAdjust: () => void;
  submitAdjust: () => Promise<void>;
  toggleFlag: (
    userId: number,
    flag: "download_enabled" | "suspended",
    value: boolean,
  ) => Promise<void>;
  adjust: AdjustState | null;
  setAdjust: React.Dispatch<React.SetStateAction<AdjustState | null>>;
  busy: boolean;
  currency: string;
}) {
  const { dict, locale } = useI18n();
  const u = dict.userDetail;
  const a = dict.adminUsers;
  return (
    <section className="baozi-panel flex flex-col gap-3 p-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="text-base font-bold text-ink">
          {fmt(a.detailTitle, { name: detail.username, id: detail.id })}
        </h2>
        <button
          className="min-h-[36px] rounded-full border border-line px-3 text-xs"
          onClick={onClose}
        >
          {a.close}
        </button>
      </div>
      <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm md:grid-cols-3">
        <div>
          <dt className="text-sub">{u.email}</dt>
          <dd>{detail.email}</dd>
        </div>
        <div>
          <dt className="text-sub">Passkey</dt>
          <dd className="font-mono text-xs">{detail.passkey.slice(0, 10)}…</dd>
        </div>
        <div>
          <dt className="text-sub">{a.klass}</dt>
          <dd>{detail.class_name ?? `LV${detail.class_id}`}</dd>
        </div>
        <div>
          <dt className="text-sub">{u.uploaded}</dt>
          <dd>{fmtBytes(detail.uploaded)}</dd>
        </div>
        <div>
          <dt className="text-sub">{u.downloaded}</dt>
          <dd>{fmtBytes(detail.downloaded)}</dd>
        </div>
        <div>
          <dt className="text-sub">{currency}</dt>
          <dd>{detail.spark_balance}</dd>
        </div>
        <div>
          <dt className="text-sub">{u.seedingNow}</dt>
          <dd>{detail.seeding}</dd>
        </div>
        <div>
          <dt className="text-sub">{u.leechingNow}</dt>
          <dd>{detail.leeching}</dd>
        </div>
        <div>
          <dt className="text-sub">{u.uploadsCount}</dt>
          <dd>{detail.uploads}</dd>
        </div>
        <div>
          <dt className="text-sub">{u.invitesUnused}</dt>
          <dd>{detail.invites_unused}</dd>
        </div>
        <div>
          <dt className="text-sub">{u.inviter}</dt>
          <dd>{detail.inviter_name ?? "—"}</dd>
        </div>
        <div>
          <dt className="text-sub">{u.twoFa}</dt>
          <dd>{detail.totp_enabled ? u.totpOn : u.totpOff}</dd>
        </div>
        <div>
          <dt className="text-sub">{u.createdAt}</dt>
          <dd>
            {new Date(detail.created_at).toLocaleString(dateLocale(locale))}
          </dd>
        </div>
        <div>
          <dt className="text-sub">{u.lastSeen}</dt>
          <dd>
            {detail.last_seen_at
              ? new Date(detail.last_seen_at).toLocaleString(dateLocale(locale))
              : "—"}
          </dd>
        </div>
      </dl>

      <div className="flex flex-wrap gap-2">
        <button className="baozi-button" onClick={onAdjust}>
          {a.adjustBtn}
        </button>
        <button
          className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${detail.download_enabled ? "border border-line text-danger" : "bg-mint text-white"}`}
          disabled={busy}
          onClick={() =>
            toggleFlag(detail.id, "download_enabled", !detail.download_enabled)
          }
        >
          {detail.download_enabled ? a.dlDisableBtn : a.dlEnableBtn}
        </button>
        <button
          className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${detail.suspended ? "bg-mint text-white" : "border border-line text-danger"}`}
          disabled={busy}
          onClick={() => toggleFlag(detail.id, "suspended", !detail.suspended)}
        >
          {detail.suspended ? a.unsuspendBtn : a.suspendBtn}
        </button>
      </div>

      {/* 自定义字段值（G4）：查看 + 代改，站长没建字段时组件自身不渲染 */}
      <AdminUserFieldsPanel userId={detail.id} />

      {/* 数值调整表单（delta 语义） */}
      {adjust && (
        <div className="cmgmt-form rounded-[var(--r-md)] border border-line p-3">
          <p className="mb-2 text-xs text-sub">{u.adjustHint}</p>
          <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
            <label className="flex flex-col gap-1 text-xs">
              {u.adjUp}
              <input
                type="number"
                value={adjust.up}
                onChange={(e) => setAdjust({ ...adjust, up: e.target.value })}
                className={INPUT_MD}
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              {u.adjDown}
              <input
                type="number"
                value={adjust.down}
                onChange={(e) => setAdjust({ ...adjust, down: e.target.value })}
                className={INPUT_MD}
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              {fmt(u.adjSpark, { magic: currency })}
              <input
                type="number"
                value={adjust.spark}
                onChange={(e) =>
                  setAdjust({ ...adjust, spark: e.target.value })
                }
                className={INPUT_MD}
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              {u.adjInvite}
              <input
                type="number"
                value={adjust.invite}
                onChange={(e) =>
                  setAdjust({ ...adjust, invite: e.target.value })
                }
                className={INPUT_MD}
              />
            </label>
          </div>
          <label className="mt-2 flex flex-col gap-1 text-xs">
            {u.adjNote}
            <input
              value={adjust.note}
              onChange={(e) => setAdjust({ ...adjust, note: e.target.value })}
              className={INPUT_MD}
            />
          </label>
          <div className="mt-2 flex gap-2">
            <button
              className="baozi-button"
              disabled={busy}
              onClick={submitAdjust}
            >
              {u.adjustSubmit}
            </button>
            <button className={BTN_SM_BOLD} onClick={() => setAdjust(null)}>
              {dict.common.cancel}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
