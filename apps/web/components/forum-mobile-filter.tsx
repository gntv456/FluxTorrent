"use client";

import Link from "next/link";
import { usePathname, useSearchParams } from "next/navigation";
import { useI18n } from "@/i18n/client";

/** 手机端「筛选」弹层（论坛首页/版块页共用一份）。
 *
 *  链接与按钮文案都按当前 URL 现算：布局在切版块时保持挂载，
 *  服务端拿不到新的 pathname/searchParams，只有客户端能跟上。
 *  版块页的 sort/tag 参数原样保留，只改 type 并回到第 1 页。 */

const OPTIONS = [
  [undefined, "filterAll", "bg-sky"],
  ["notice", "filterNotice", "bg-coral"],
  ["sticky", "filterSticky", "bg-indigo"],
  ["digest", "filterDigest", "bg-sun"],
] as const;

type FilterType = "notice" | "sticky" | "digest";

function typeOf(raw: string | null): FilterType | undefined {
  return raw === "notice" || raw === "sticky" || raw === "digest"
    ? raw
    : undefined;
}

export function ForumMobileFilterLabel() {
  const { dict } = useI18n();
  const type = typeOf(useSearchParams().get("type"));
  const label =
    type === "notice"
      ? dict.forums.filterNotice
      : type === "sticky"
        ? dict.forums.filterSticky
        : type === "digest"
          ? dict.forums.filterDigest
          : dict.forums.filterAll;
  return <>{label}</>;
}

export function ForumMobileFilter() {
  const { dict } = useI18n();
  const pathname = usePathname() ?? "/forums";
  const params = useSearchParams();
  const type = typeOf(params.get("type"));
  const hrefFor = (value: string | undefined) => {
    const next = new URLSearchParams(params.toString());
    next.delete("page");
    if (value) next.set("type", value);
    else next.delete("type");
    const s = next.toString();
    return s ? `${pathname}?${s}` : pathname;
  };
  return (
    <div className="fh-pick">
      {OPTIONS.map(([value, key, dot]) => (
        <Link
          key={value ?? "all"}
          href={hrefFor(value)}
          className={"fh-pick__row" + (type === value ? " on" : undefined)}
          aria-current={type === value ? "true" : undefined}
        >
          <span
            className={"fh-pick__dot " + dot}
            aria-hidden="true"
          />
          {dict.forums[key]}
        </Link>
      ))}
    </div>
  );
}
