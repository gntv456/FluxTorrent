"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 摘要区签到入口（2026-11-22 调整）：点击跳签到日历页 /checkin——旧的
 *  弹窗内直发 POST 退役，连签进度/里程碑/补签卡都在日历页，导流过去更顺。
 *  已签态只换文案不锁点击：点进去还能看日历/补签。 */
export function UserCheckin() {
  const { dict } = useI18n();
  const [checked, setChecked] = useState(false);

  // 初始态对齐：今天已签（如首页/别处签过）显示 ✓，避免口径打架
  useEffect(() => {
    api
      .get<{ checked_today: boolean }>("/api/v1/attendance")
      .then((s) => setChecked(s.checked_today))
      .catch(() => {});
  }, []);

  return (
    <Link href="/checkin" className={`usermenu__checkin${checked ? " is-checked" : ""}`}>
      {checked ? `✓ ${dict.my.checked}` : `[${dict.my.dailyCheckin}]`}
    </Link>
  );
}
