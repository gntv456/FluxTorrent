"use client";

import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";

/** 后台用户详情·确认对话框组（W-34 从 admin-user-detail.tsx 拆出）：
 *  禁言/封禁理由输入 + 改名/重置密码/删除用户三连确认——
 *  原先散在 hook 里的 window.confirm/prompt 全部收口到这两个对话框。 */

// 状态理由输入框样式（防 JSX 超宽）
const ASK_INPUT =
  "min-h-[36px] rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-3 text-sm text-ink";

export type AskAct = "rename" | "resetPass" | "deleteUser";

export function UserDetailDialogs({
  askStatus,
  setAskStatus,
  statusReason,
  setStatusReason,
  askAct,
  setAskAct,
  username,
  newName,
  uid,
  onStatus,
  onAct,
}: {
  askStatus: 0 | 1 | 2 | null;
  setAskStatus: (v: 0 | 1 | 2 | null) => void;
  statusReason: string;
  setStatusReason: (v: string) => void;
  askAct: AskAct | null;
  setAskAct: (v: AskAct | null) => void;
  username: string;
  newName: string;
  uid: number;
  onStatus: (next: 0 | 1 | 2, reason: string) => void;
  onAct: (act: AskAct) => void;
}) {
  const { dict } = useI18n();
  const ud = dict.userDetail;
  return (
    <>
      <ConfirmDialog
        open={askStatus !== null}
        onClose={() => setAskStatus(null)}
        onConfirm={() => {
          const next = askStatus ?? 0;
          setAskStatus(null);
          onStatus(next, statusReason);
        }}
        tone="danger"
        title={askStatus === 2 ? ud.btnBan : ud.btnMute}
      >
        <label className="mt-2 flex flex-col gap-1 text-xs text-sub">
          {askStatus === 2 ? ud.banReason : ud.muteReason}
          <input
            value={statusReason}
            autoFocus
            onChange={(e) => setStatusReason(e.target.value)}
            className={ASK_INPUT}
          />
        </label>
      </ConfirmDialog>
      <ConfirmDialog
        open={askAct !== null}
        onClose={() => setAskAct(null)}
        onConfirm={() => {
          const act = askAct;
          setAskAct(null);
          if (act !== null) onAct(act);
        }}
        tone={askAct === "deleteUser" ? "danger" : "neutral"}
        title={
          askAct === "rename"
            ? dict.adminrename.btn
            : askAct === "resetPass"
              ? ud.btnResetPass
              : ud.btnDeleteUser
        }
        message={
          askAct === "rename"
            ? dict.adminrename.confirm
                .replace("{old}", username)
                .replace("{new}", newName.trim())
            : askAct === "resetPass"
              ? fmt(ud.resetPwdConfirm, { name: username })
              : fmt(ud.delUserConfirm, { name: username, id: uid })
        }
      />
    </>
  );
}
