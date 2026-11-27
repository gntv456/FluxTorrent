/** 保种区一级下拉（2026-11-27 站长定稿）：一级「保种区」挂两子菜单——
 *  保种区（发布 ≥3 天 + 做种 1-4 人的温带种免费区）与拯救区（死种复活，
 *  /resurrections，从「成长荣誉」组随迁至此，组里不再重复）。
 *  resurrections 模块关时下拉只剩自身。
 *
 *  拆件原因：nav-menu.ts 贴 300 行门禁顶，本件照 nav-edu.ts 先例独立。 */
import type { NavItem } from "./nav-menu";

export function preservePrimary(
  nav: Record<string, string>,
  rescueOn: boolean,
): NavItem {
  return {
    href: "/preserve",
    label: nav.preserve,
    sub: [
      {
        label: nav.preserveZone ?? nav.preserve,
        href: "/preserve",
        items: [],
      },
      ...(rescueOn
        ? [
            {
              label: nav.rescueZone ?? nav.resurrections,
              href: "/resurrections",
              items: [],
            },
          ]
        : []),
    ],
  };
}
