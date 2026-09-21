"use client";

/** 后台用户管理·分页条（从 components/admin-users.tsx 按域拆出）。 */

// 分页按钮
const PAGE_BTN =
  "min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40";

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
    <div className="flex items-center justify-between text-sm text-sub">
      <span>共 {data?.total ?? 0} 条</span>
      <div className="flex items-center gap-2">
        <button
          disabled={page <= 1}
          onClick={() => setPage(page - 1)}
          className={PAGE_BTN}
        >
          上一页
        </button>
        <span>
          {data?.page ?? 1} / {totalPages}
        </span>
        <button
          disabled={page >= totalPages}
          onClick={() => setPage(page + 1)}
          className={PAGE_BTN}
        >
          下一页
        </button>
      </div>
    </div>
  );
}
