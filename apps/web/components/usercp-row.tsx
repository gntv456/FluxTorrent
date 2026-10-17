"use client";

/** rowhead/rowfollow 经典表格行（从 usercp.tsx 拆出，300 行门禁）：
 *  usercp-personal / usercp-tracker(-browse) / usercp-forum /
 *  usercp-security 各设定面板共用。 */

export function Row({
  head,
  children,
  id,
  className,
}: {
  head: string;
  children: React.ReactNode;
  /** 锚点直达用（账户概览的「密钥 → 重置」带 #passkey 跳进来） */
  id?: string;
  className?: string;
}) {
  return (
    <tr id={id} className={className}>
      <td className="rowhead nowrap">{head}</td>
      <td className="rowfollow">{children}</td>
    </tr>
  );
}
