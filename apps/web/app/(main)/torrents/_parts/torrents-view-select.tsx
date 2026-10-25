"use client";

import { useRouter } from "next/navigation";

/** 手机端把「列表形态 / 每页条数」从两排药丸收成两个原生下拉：
 *  列表页首屏让位给种子本身。选项的 value 直接是服务端拼好的目标 URL，
 *  选中即 router.push（与药丸同为整页链接语义，可回退/可分享）。
 *  无 JS 时 select 不可用，但桌面药丸与高级面板不受影响（<md 专用）。 */
export function TorrentsViewSelect({
  label,
  current,
  options,
}: {
  label: string;
  /** 当前项的 URL，用于选中态（与 options[].href 严格同源） */
  current: string;
  options: { href: string; label: string }[];
}) {
  const router = useRouter();
  return (
    <select
      className="torrents-view-select tsb-select"
      aria-label={label}
      value={current}
      onChange={(e) => router.push(e.target.value)}
    >
      {options.map((o) => (
        <option key={o.href} value={o.href}>
          {o.label}
        </option>
      ))}
    </select>
  );
}
