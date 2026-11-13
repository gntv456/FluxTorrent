import type { ReactNode } from "react";

/**
 * 数据表原语（§5.4 P1）：统一 <th scope="col"> + sr-only caption +
 * 可选 aria-sort + 空态 + overflow-x-auto 横滚。
 *
 * 用法：
 *   <DataTable
 *     caption={t.title}
 *     columns={[{ key: "id", label: "ID" }]}
 *     emptyWhen={rows.length === 0}
 *     empty={t.empty}
 *     className="nexus-table"
 *   >
 *     {rows.map((r) => <tr key={r.id}>…</tr>)}
 *   </DataTable>
 *
 * 行内容仍由调用方写（各表单元格差异大），本件只负责表头语义、
 * 无障碍与横滚容器，抹平各 admin-* 各写 colhead 的差异。
 * 表头样式由 .nexus-table th 提供，无需再挂 colhead 类。
 */

export interface DataTableColumn {
  key: string;
  label: ReactNode;
  /** aria-sort：仅当前排序列给，其余省略 */
  sort?: "ascending" | "descending" | "none";
  /** 附加类（宽度 / 右对齐等） */
  className?: string;
}

export function DataTable({
  caption,
  columns,
  className,
  emptyWhen = false,
  empty,
  children,
}: {
  /** 读屏用表名（sr-only，必填） */
  caption: string;
  columns: DataTableColumn[];
  /** table 类名（如 nexus-table） */
  className?: string;
  /** 为空时渲染空态行 */
  emptyWhen?: boolean;
  empty?: ReactNode;
  /** 有数据时的 <tr> 行 */
  children?: ReactNode;
}) {
  return (
    <div className="overflow-x-auto">
      <table className={className}>
        <caption className="sr-only">{caption}</caption>
        <thead>
          <tr>
            {columns.map((c) => (
              <th
                key={c.key}
                scope="col"
                aria-sort={c.sort}
                className={c.className}
              >
                {c.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {emptyWhen && empty !== undefined ? (
            <tr>
              <td
                colSpan={columns.length}
                className="py-6 text-center text-sub"
              >
                {empty}
              </td>
            </tr>
          ) : (
            children
          )}
        </tbody>
      </table>
    </div>
  );
}
