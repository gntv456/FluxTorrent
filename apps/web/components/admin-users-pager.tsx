"use client";

/** 后台用户管理·分页条（从 components/admin-users.tsx 按域拆出）。 */

import { Pager } from "@/components/ui/pager";

interface UsersPage {
  rows: unknown[];
  total: number;
  page: number;
  per_page: number;
}

export function UsersPager({
  data,
  page,
  totalPages,
  setPage,
}: {
  data: UsersPage | null;
  page: number;
  totalPages: number;
  setPage: (p: number) => void;
}) {
  return (
    <Pager
      page={page}
      pages={totalPages}
      total={data?.total ?? 0}
      onStep={setPage}
    />
  );
}
