"use client";

/** rowhead/rowfollow 经典表格行（从 usercp.tsx 拆出，300 行门禁）：
 *  usercp-personal / usercp-tracker(-browse) / usercp-forum /
 *  usercp-security 各设定面板共用。 */

export function Row({
  head,
  children,
}: {
  head: string;
  children: React.ReactNode;
}) {
  return (
    <tr>
      <td className="rowhead nowrap">{head}</td>
      <td className="rowfollow">{children}</td>
    </tr>
  );
}
