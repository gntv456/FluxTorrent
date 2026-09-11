import Link from "next/link";
import { getMenuItems, type MenuItem } from "@/lib/data";

/** 单个自定义菜单项：内链（以 / 开头）走 <Link>，外链新窗口打开。 */
export function MenuItemLink({
  item,
  className,
}: {
  item: MenuItem;
  className: string;
}) {
  if (item.url.startsWith("/")) {
    return (
      <Link href={item.url} className={className}>
        {item.label}
      </Link>
    );
  }
  return (
    <a
      href={item.url}
      target="_blank"
      rel="noopener noreferrer"
      className={className}
    >
      {item.label}
    </a>
  );
}

/**
 * 自定义菜单（admin menu_items → 前台渲染层）：
 * 按位置拉取生效项（sidebar/footer/topbar），接口失败或为空时不渲染任何内容。
 */
export async function CustomMenu({
  location,
  className,
  linkClassName,
}: {
  location: "topbar" | "footer" | "sidebar";
  className: string;
  linkClassName: string;
}) {
  const items = await getMenuItems(location);
  if (items.length === 0) return null;
  return (
    <ul className={className}>
      {items.map((m) => (
        <li key={m.id}>
          <MenuItemLink item={m} className={linkClassName} />
        </li>
      ))}
    </ul>
  );
}
