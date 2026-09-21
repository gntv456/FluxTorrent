"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 摘要区直发签到（0148）：点击即 POST /attendance/checkin，不跳控制面板；
 *  成功后 ✓ 态 + 结果文案 + 回调刷新 {magic} 余额。后端业务错误（如
 *  「今天已经签到过啦」）透出原文案并把按钮置已签态，不再误报网络失败。 */
export function UserCheckin({ onDone }: { onDone: () => void }) {
  const { dict, currency } = useI18n();
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  // 初始态对齐：今天已签（如首页/别处签过）直接置 ✓，避免点开就报错
  useEffect(() => {
    api
      .get<{ checked_today: boolean }>("/api/v1/attendance")
      .then((s) => setChecked(s.checked_today))
      .catch(() => {});
  }, []);

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
    } catch (e) {
      if (e instanceof ApiError) {
        // 业务错误：透出后端文案；已签类错误直接收敛为已签态
        setMsg(e.message);
        if (e.message.includes("已经签到")) setChecked(true);
      } else {
        setMsg(dict.common.networkError);
      }
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
