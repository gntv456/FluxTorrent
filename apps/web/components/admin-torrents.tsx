"use client";

import { CELL_CARD_SUB } from "@/lib/ui-classes";

import { useI18n } from "@/i18n/client";

import { useState } from "react";
import { TorrentList } from "./admin-torrents-list";
import { DenyReasons, OpLogs } from "./admin-torrents-records";
import { LoginLogs } from "./admin-torrents-login-logs";
import { RecordQuery } from "./admin-torrents-record-query";
import type { SubTab } from "./admin-torrents-shared";

/** 后台种子管理 + 拒绝原因字典 + 种子操作记录 + 记录查询（好学站口径）
 *  第八轮：批量工作台（置顶/优惠/推荐/标签/H&R/改分类/删除）+ 多维筛选 + 登录记录一键封/解封 IP
 *  批量工作台拆至 ./admin-torrents-list.tsx；拒绝原因/操作记录拆至 ./admin-torrents-records.tsx；
 *  登录记录拆至 ./admin-torrents-login-logs.tsx；记录查询拆至 ./admin-torrents-record-query.tsx；
 *  类型与常量拆至 ./admin-torrents-shared.ts。 */
export function AdminTorrents() {
  const { currency } = useI18n();
  const [sub, setSub] = useState<SubTab>("torrents");
  const [msg, setMsg] = useState<string | null>(null);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-2" role="tablist">
        {(
          [
            ["torrents", "种子管理"],
            ["deny", "拒绝原因"],
            ["ops", "种子操作记录"],
            ["spark", `${currency}记录`],
            ["buys", "种子购买"],
            ["logins", "登录记录"],
          ] as [SubTab, string][]
        ).map(([k, label]) => (
          <button
            key={k}
            role="tab"
            aria-selected={sub === k}
            onClick={() => setSub(k)}
            className={`min-h-[40px] rounded-full px-4 text-sm font-bold ${sub === k ? "bg-sky text-white" : CELL_CARD_SUB}`}
          >
            {label}
          </button>
        ))}
      </div>
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      {sub === "torrents" && <TorrentList flash={flash} />}
      {sub === "deny" && <DenyReasons flash={flash} />}
      {sub === "ops" && <OpLogs />}
      {sub === "spark" && (
        <RecordQuery
          title={`${currency}记录`}
          endpoint="/api/v1/admin/spark-logs"
          columns={[
            ["username", "用户"],
            ["amount", "数额"],
            ["kind", "类型"],
            ["balance_after", "余额"],
            ["created_at", "时间"],
          ]}
        />
      )}
      {sub === "buys" && (
        <RecordQuery
          title="种子购买记录"
          endpoint="/api/v1/admin/torrent-buys"
          columns={[
            ["username", "用户"],
            ["kind", "类型"],
            ["ref_id", "种子ID"],
            ["amount", currency],
            ["created_at", "时间"],
          ]}
        />
      )}
      {sub === "logins" && <LoginLogs />}
    </div>
  );
}
