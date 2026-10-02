import type { getMenuItems } from "@/lib/data";

/** 导航配置单源（移动端方案 M2）：桌面 MainMenu / 移动抽屉 / 底 Tab「更多」
 *  三处共用同一份构建逻辑，杜绝三处命名漂移（管理端 i18n 三处不一致的教训）。
 *
 *  类型与分组构建从 components/layout.tsx 抽出，原逻辑一字未改——
 *  layout.tsx 与 nav-drawer 都消费本模块。自定义菜单（topbar）生效时
 *  一级/分组同样以这里为准。 */

export type NavItem = { href: string; label: string };
export type NavGroup = { group: string; items: NavItem[] };
export type MenuItem = Awaited<ReturnType<typeof getMenuItems>>[number];

/** 模块开关视图（U1 §6.2）：缺键视为开（T3 缺省=现状） */
export type ModuleGate = (key: string) => boolean;

export type NavConfigInput = {
  /** dict.nav / dict.tabbar（调用方从 getDict 取） */
  nav: Record<string, string>;
  tabbar: Record<string, string>;
  /** {magic} 占位符替换后的站点货币名上下文：nav.spark 等模板需要 */
  currency: string;
  modules: ModuleGate;
  customItems: MenuItem[];
};

/** 自定义菜单归一：一级项 + 分组（带子项的一级项其子项收独立分组，
 *  nexusphp-menu 生产口径；空 = 开关关闭/未配置 → 调用方回退默认） */
export function customNav(items: MenuItem[]): {
  active: boolean;
  primary: NavItem[];
  groups: NavGroup[];
} {
  const tops = items.filter((m) => m.parent_id === 0);
  const active = tops.length > 0;
  const primary = active
    ? tops.map((m) => ({ href: m.url, label: m.label }))
    : [];
  const groups = active
    ? tops
        .filter((m) => items.some((c) => c.parent_id === m.id))
        .map((m) => ({
          group: m.label,
          items: items
            .filter((c) => c.parent_id === m.id)
            .map((c) => ({ href: c.url, label: c.label })),
        }))
    : [];
  return { active, primary, groups };
}

/** 默认导航（customNav 不生效时的全站口径）。
 *  分组顺序：发现 / 魔力经济 / 成长荣誉 / 娱乐 / 更多。
 *  条目按 module 开关过滤；forums 关闭一级入口消失（三审 C-3 与 footer 同口径）。 */
export function defaultNav({
  nav,
  tabbar,
  currency,
  modules,
}: NavConfigInput): {
  primary: NavItem[];
  groups: NavGroup[];
} {
  const t = (tpl: string) => tpl.replace("{magic}", currency);
  const socialOn = modules("social");
  const primary: NavItem[] = [
    { href: "/", label: nav.home },
    { href: "/torrents", label: nav.library },
    ...(modules("forums") ? [{ href: "/forums", label: nav.forums }] : []),
    { href: "/top", label: nav.top },
    { href: "/upload", label: nav.upload },
  ];
  const groups: NavGroup[] = [
    {
      group: nav.discover,
      items: [
        { href: "/torrents?official=1", label: nav.official },
        ...(modules("requests")
          ? [{ href: "/requests", label: nav.requests }]
          : []),
        ...(modules("offers") ? [{ href: "/offers", label: nav.offers }] : []),
        ...(modules("subtitles")
          ? [{ href: "/subtitles", label: nav.subtitles }]
          : []),
        ...(modules("preserve")
          ? [{ href: "/preserve", label: nav.preserve }]
          : []),
        ...(socialOn
          ? [
              { href: "/endangered", label: nav.endangered },
              { href: "/teams", label: nav.teams },
            ]
          : []),
        ...(modules("textbooks")
          ? [{ href: "/textbooks", label: nav.textbooks }]
          : []),
      ],
    },
    {
      group: t(nav.spark),
      items: [
        ...(modules("shop") ? [{ href: "/shop", label: tabbar.shop }] : []),
        ...(modules("bank") ? [{ href: "/bank", label: nav.bank }] : []),
        ...(modules("magic_pool")
          ? [{ href: "/magic-pool", label: nav.magicPool }]
          : []),
        ...(modules("tasks") ? [{ href: "/tasks", label: nav.tasks }] : []),
        { href: "/my-spark", label: t(nav.spark) },
        // C5（0226）：魔力明细（触点 #11 积分透明化）
        { href: "/me/sparks", label: nav.sparkLedger },
      ],
    },
    {
      group: nav.growth,
      items: [
        // P2（0226）：等级要求公开页（触点 #2）
        { href: "/classes", label: nav.classes },
        ...(modules("exams") ? [{ href: "/me/exams", label: nav.exams }] : []),
        { href: "/me/achievements", label: nav.achievements },
        ...(modules("resurrections")
          ? [{ href: "/resurrections", label: nav.resurrections }]
          : []),
        ...(modules("medals")
          ? [{ href: "/medal-wall", label: nav.medalWall }]
          : []),
        ...(modules("dressup")
          ? [{ href: "/avatar-frames", label: nav.frames }]
          : []),
        ...(modules("medals") ? [{ href: "/medals", label: nav.medals }] : []),
        ...(modules("jixiao")
          ? [{ href: "/jixiao", label: nav.jixiao }]
          : []),
        ...(modules("invites")
          ? [{ href: "/invites", label: nav.invites }]
          : []),
      ],
    },
    {
      group: nav.fun,
      items: [
        ...(modules("games") ? [{ href: "/games", label: nav.games }] : []),
        ...(modules("gacha") ? [{ href: "/gacha", label: nav.gacha }] : []),
        ...(modules("farm") ? [{ href: "/farm", label: nav.farm }] : []),
        ...(modules("gomoku")
          ? [{ href: "/gomoku", label: nav.gomoku }]
          : []),
        ...(modules("contests")
          ? [{ href: "/contests", label: nav.contests }]
          : []),
        ...(modules("friends")
          ? [{ href: "/friends", label: nav.friends }]
          : []),
      ],
    },
    {
      group: nav.more,
      items: [
        ...(modules("messages")
          ? [{ href: "/messages", label: nav.messages }]
          : []),
        // H&R 入口恒可见：hr_enforce 执法独立于 exams 模块，入口随 exams
        // 关闭会形成「执法在跑、记录不可见」断链（见 /me/hr 网关注释）
        { href: "/myhr", label: nav.myhr },
        { href: "/faq", label: nav.faq },
        ...(modules("magic_pool")
          ? [{ href: "/donate", label: nav.donate }]
          : []),
      ],
    },
  ];
  return { primary, groups };
}

/** 统一入口：自定义菜单优先，回退默认。Header / NavDrawer / 校验脚本共用。 */
export function buildNav(input: NavConfigInput): {
  primary: NavItem[];
  groups: NavGroup[];
} {
  const custom = customNav(input.customItems);
  if (custom.active) return { primary: custom.primary, groups: custom.groups };
  return defaultNav(input);
}
