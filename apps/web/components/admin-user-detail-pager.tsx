"use client";

/** 后台用户详情·关联 tab 分页条（从 admin-user-detail-panels.tsx 拆出，
 *  保 300 行门禁）：火花流水 / 登录记录两个 tab 共用。 */

import { Pager } from "@/components/ui/pager";
import { PER_PAGE } from "./admin-user-detail-shared";

export function PagedBar({
  total,
  cur,
  onPage,
}: {
  total: number;
  cur: number;
  onPage: (v: number) => void;
}) {
  return (
    <Pager
      page={cur}
      pages={Math.max(1, Math.ceil(total / PER_PAGE))}
      total={total}
      onStep={onPage}
    />
  );
}
