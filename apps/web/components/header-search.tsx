"use client";

/** 顶栏全局搜索直显框（DS-PLAN-01 W-30 / IA-1）。
 *
 *  背景：六站横评 + 同类站 对标——顶栏是全站最贵的不动产，搜索这种
 *  「看一眼就走」的高频操作不该藏在弹窗按钮后面。IA-1 把 /top 从一级
 *  让位给本框（一级 5 项 → 4 项 + 搜索，与 同类站 同构）。
 *
 *  提交走 /torrents?search=（种子域）；论坛域/全站域仍走旁边的
 *  GlobalSearchDialog 按钮（两档各司其职，不重复建设）。
 *  `/` 快捷键聚焦属阶段五 W-39，本件先落形态。
 */

import { useState } from "react";
import { useRouter } from "next/navigation";

import { Icon } from "@/components/icons";

export function HeaderSearch({
  placeholder,
  label,
}: {
  placeholder: string;
  /** 无障碍名（图标钮无文本时用） */
  label: string;
}) {
  const router = useRouter();
  const [q, setQ] = useState("");
  return (
    <form
      role="search"
      className="ui-hsearch"
      onSubmit={(e) => {
        e.preventDefault();
        const kw = q.trim();
        if (kw) router.push(`/torrents?search=${encodeURIComponent(kw)}`);
      }}
    >
      <Icon name="search" size={15} />
      <input
        type="search"
        value={q}
        onChange={(e) => setQ(e.target.value)}
        placeholder={placeholder}
        aria-label={label}
        className="ui-hsearch__input"
      />
    </form>
  );
}
