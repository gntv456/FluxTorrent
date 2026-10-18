"use client";

import { useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { BTN_LG_SKY } from "@/lib/ui-classes";
import { AuthCanvas } from "@/components/auth-canvas";

/** P2 触点 #24：自助解封（首次宽恕，0226）。
 *  被封用户先登录（登录口对被封账号放行到有限面），再到本页一键解封一次；
 *  用掉后走申诉通道。前提：站长已设 self_unban_cooldown_hours > 0。
 *
 *  2026-10-18 对齐样图 § 账户与认证：改用认证画布；顺带换掉一批已不存在的
 *  旧令牌类（text-muted / bg-[var(--panel)] / bg-[var(--accent)]）——
 *  那些类在当前样式表里没有定义，主按钮此前渲染成透明底。 */

export default function SelfUnbanPage() {
  const { dict } = useI18n();
  const [msg, setMsg] = useState("");
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);

  async function unban() {
    setBusy(true);
    setMsg("");
    try {
      await api.post("/api/v1/auth/self-unban", {});
      setDone(true);
      setMsg(dict.selfUnban.done);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <AuthCanvas
      title={dict.selfUnban.title}
      heroTag={dict.selfUnban.heroTag}
      heroDesc={dict.selfUnban.heroDesc}
      badge={dict.selfUnban.badge}
      notice={dict.selfUnban.note}
      foot={dict.selfUnban.closedHint}
    >
      <div className="flex flex-col gap-3">
        {msg && (
          <p
            role={done ? "status" : "alert"}
            className={`rounded-[var(--r-md)] px-3 py-2 text-sm ${
              done
                ? "bg-mint/10 text-mint-deep"
                : "bg-danger/5 text-danger"
            }`}
          >
            {msg}
          </p>
        )}
        {!done && (
          <button
            type="button"
            onClick={() => void unban()}
            disabled={busy}
            className={BTN_LG_SKY}
          >
            {busy ? dict.selfUnban.busy : dict.selfUnban.btn}
          </button>
        )}
      </div>
      <div className="auth-card__links">
        <Link href="/login">{dict.forgot.backToLogin}</Link>
      </div>
    </AuthCanvas>
  );
}
