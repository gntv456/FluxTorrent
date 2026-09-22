"use client";

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { AdminShell, type PanelEntry } from "@/components/admin-shell";

/**
 * 独立管理页（`/admin/forums`、`/admin/settings`）共用的外壳加载器。
 *
 * 为什么单独抽一层：这两个页面是独立路由而非 `/admin?tool=` 的页签，需要各自
 * 拉一次 `/admin/staffpanel` 才能渲染左侧导航 —— 原先 `/admin/forums` 内联了
 * 一份，`/admin/settings` 干脆没有，导致进设置页之后无法用导航跳到别的工具。
 *
 * 导航动作一律**整页跳转**：独立页与主面板之间不存在可共享的客户端路由状态，
 * 让 URL 保持唯一真相，避免「本地 tool 状态」和地址栏脱节。
 */
export function AdminPageShell({
  active,
  badges,
  children,
}: {
  /** 当前高亮的 tab_key（如 "forums" / "settings"） */
  active: string;
  badges?: Record<string, number>;
  children: React.ReactNode;
}) {
  const [entries, setEntries] = useState<PanelEntry[]>([]);
  const [role, setRole] = useState("");
  const [classId, setClassId] = useState<number | undefined>(undefined);

  useEffect(() => {
    let alive = true;
    api
      .get<{ entries: PanelEntry[]; role: string; class_id?: number }>(
        "/api/v1/admin/staffpanel",
      )
      .then((pnl) => {
        if (!alive) return;
        setEntries(pnl.entries);
        setRole(pnl.role);
        setClassId(pnl.class_id);
      })
      .catch(() => {
        // 导航拉取失败不阻塞页面主体；面板自身会给出权限/加载提示
        if (alive) setEntries([]);
      });
    return () => {
      alive = false;
    };
  }, []);

  const onTool = useCallback(
    (t: string) => {
      const e = entries.find((x) => x.tab_key === t);
      // 条目缺失（导航尚未加载完）时回落到标准查询串，不至于点不动
      if (!e || !e.url.startsWith("/")) {
        window.location.href = `/admin?tool=${encodeURIComponent(t)}`;
        return;
      }
      window.location.href = e.url;
    },
    [entries],
  );

  return (
    <AdminShell
      entries={entries}
      tool={active}
      onTool={onTool}
      badges={badges ?? {}}
      role={role}
      classId={classId}
    >
      {children}
    </AdminShell>
  );
}
