"use client";

/** 关联数据三 tab（火花流水 / 登录记录 / 做种列表）的渲染壳。
 *  从 admin-user-detail.tsx 按域外移（300 行门禁）——外移的同时补上
 *  「读失败 ≠ 没有数据」：原先三个 catch 一律回落成空数组/null，
 *  接口 500 时面板安静地显示「暂无」，版主当成「这用户真没记录」。
 */

import { ErrorPanel } from "@/components/ui/error-panel";
import { useDetailTabs } from "./admin-user-detail-hooks";
import {
  LoginsPanel,
  SeedingPanel,
  SparkPanel,
} from "./admin-user-detail-panels";

export function DetailTabsBody({
  tabs,
  dt,
}: {
  tabs: ReturnType<typeof useDetailTabs>;
  dt: (s: string | null) => string;
}) {
  const fail = tabs.tabErr !== null;
  return (
    <>
      {fail && (
        <ErrorPanel error={tabs.tabErr} onRetry={tabs.reloadTab} />
      )}

      {tabs.tab === "spark" && !fail && (
        <SparkPanel
          spark={tabs.spark}
          page={tabs.sparkPage}
          setPage={tabs.setSparkPage}
          dt={dt}
        />
      )}

      {tabs.tab === "logins" && !fail && (
        <LoginsPanel
          logins={tabs.logins}
          page={tabs.loginsPage}
          setPage={tabs.setLoginsPage}
          dt={dt}
        />
      )}

      {tabs.tab === "seeding" && !fail && (
        <SeedingPanel seeds={tabs.seeds} />
      )}
    </>
  );
}
