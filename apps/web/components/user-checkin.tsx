"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 摘要区直发签到（0148）：点击即 POST /attendance/checkin，不跳控制面板；
 *  成功后 ✓ 态 + 结果文案 + 回调刷新 {magic} 余额。从 user-menu 按域拆出。 */
export function UserCheckin({ onDone }: { onDone: () => void }) {
  const { dict, currency } = useI18n();
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  async function checkin() {
    if (busy || checked) return;
    setBusy(true);
    try {
      const r = await api.post<{ reward: number; streak: number }>(
        "/api/v1/attendance/checkin",
        {},
      );
      setChecked(true);
      setMsg(
        dict.my.checkinOk
          .replace("{reward}", String(r.reward))
          .replace("{streak}", String(r.streak))
          .replace("{magic}", currency),
      );
      onDone();
    } catch {
      setMsg(dict.home2?.checkinFail ?? dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <button
        type="button"
        className="usermenu__checkin"
        onClick={checkin}
        disabled={busy || checked}
      >
        {busy
          ? dict.my.checkinBusy
          : checked
            ? dict.my.checked
            : `[${dict.my.dailyCheckin}]`}
      </button>
      {msg && <p className="usermenu__checkmsg">{msg}</p>}
    </>
  );
}
