"use client";

import { useState } from "react";

/** 第五轮 P2：置顶促销 / 自定义菜单 / 消息模板（好学站 Other 组口径）。
 *  四个子面板分别拆出（300 门禁）：admin-p2-tools-promos.tsx（置顶促销）、
 *  admin-p2-tools-menus.tsx（自定义菜单）、admin-p2-tools-templates.tsx
 *  （消息模板）、admin-p2-tools-claims.tsx（保种认领）。 */
import { StickyPromos } from "@/components/admin-p2-tools-promos";
import { MenuItems } from "@/components/admin-p2-tools-menus";
import { MsgTemplates } from "@/components/admin-p2-tools-templates";
import { Claims } from "@/components/admin-p2-tools-claims";

function useFlash() {
  const [msg, setMsg] = useState<string | null>(null);
  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };
  const node = msg ? (
    <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>
  ) : null;
  return { flash, node };
}

export function AdminP2Tools() {
  const [sub, setSub] = useState<"promos" | "menus" | "templates" | "claims">("promos");
  const { flash, node } = useFlash();
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-2" role="tablist">
        {([
          ["promos", "置顶促销"],
          ["menus", "自定义菜单"],
          ["templates", "消息模板"],
          ["claims", "保种认领"],
        ] as [typeof sub, string][]).map(([k, label]) => (
          <button
            key={k}
            role="tab"
            aria-selected={sub === k}
            onClick={() => setSub(k)}
            className={`min-h-[40px] rounded-full px-4 text-sm font-bold ${sub === k ? "bg-sky text-white" : "border border-line bg-[var(--surface-card)] text-sub"}`}
          >
            {label}
          </button>
        ))}
      </div>
      {node}
      {sub === "promos" && <StickyPromos flash={flash} />}
      {sub === "menus" && <MenuItems flash={flash} />}
      {sub === "templates" && <MsgTemplates flash={flash} />}
      {sub === "claims" && <Claims flash={flash} />}
    </div>
  );
}
