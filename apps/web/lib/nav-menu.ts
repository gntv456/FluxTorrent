import type { getMenuItems } from "@/lib/data";
import { eduPrimary, hasEduCat } from "./nav-edu";

/** 导航配置单源（M2）：桌面 MainMenu / 移动抽屉 / 底 Tab 共用同一份构建
 *  逻辑，杜绝命名漂移。自定义菜单（topbar）生效时一级/分组同样以这里为准。 */

export type NavItem = {
  href: string;
  label: string;
  /** 教育资源一级项标记（0473）：侧栏「浏览学段」按标记取项，
   *  不能按首个带 sub 的一级项猜——资源库同样带 sub 且排在前面。 */
  edu?: boolean;
  /** 一级下拉（2026-10-31 资源库）：两级结构 [{label, href, items}]——
   *  items 为三级叶子。目前仅资源库用（全部/资源/官种 × 站型分类）。 */
  sub?: { label: string; href: string; items: NavItem[] }[];
  /** 第四级（0473 教育资源）：年级叶子的科目直达（MainMenu 行内
   *  科目 chips / 图鉴侧栏折叠子树各自消费） */
  children?: NavItem[];
};
export type NavGroup = { group: string; items: NavItem[] };
export type MenuItem = Awaited<ReturnType<typeof getMenuItems>>[number];

/** 顶级分类（无 parent 的）：资源库下拉的三级叶子数据源。
 *  上限防长尾站型把下拉撑爆（超出走「全部分类」进列表页筛）。
 *  14 = 内置最大包（综合站 13 类，0455 加了短剧/有声书/教育资源）+ 1 余量：
 *  内置分类被自己的上限截掉、只能去列表页筛，是产品缺角不是降级设计。
 *  这条由 scripts/pt_audit_m_cat_key_audit.py 的 M5 判据钉住（包分类数
 *  超过上限即红），别再靠人记。 */
export type LibraryCat = { id: number; name: string };
export const LIB_CAT_CAP = 14;

/** 资源库下拉（站长定调 2026-10-31）：二级=全部/资源/官种，
 *  三级=全部 + 站型顶级分类。资源=非官种（official=0），官种=official=1，
 *  全部=不过滤——与列表 API 的 official 三态谓词一一对应。 */
export function librarySub(
  nav: Record<string, string>,
  cats: LibraryCat[],
): NonNullable<NavItem["sub"]> {
  const all = cats.filter((c) => c.name);
  const top = all.slice(0, LIB_CAT_CAP);
  /* extra 是查询串（official=1 这种）——叶子首项必须自带 ?（无 category_id
     兜底，缺问号整段算路径必 404，2026-11-01 站长报）。超上限分类不能只
     是「看不见」：merge 站点历史分类几十条，cap 后导航无入口=「综合站还
     缺三类」，补「更多分类…」落回列表页全量筛选。 */
  const overflow =
    all.length > LIB_CAT_CAP
      ? [{ href: "/torrents", label: nav.moreCats ?? "" }]
      : [];
  const leaf = (extra: string) => [
    { href: extra ? `/torrents?${extra}` : "/torrents", label: nav.libAll },
    ...top.map((c) => ({
      href: extra
        ? `/torrents?category_id=${c.id}&${extra}`
        : `/torrents?category_id=${c.id}`,
      label: c.name,
    })),
    ...overflow.map((o) => ({
      href: extra ? `/torrents?${extra}` : o.href,
      label: o.label,
    })),
  ];
  return [
    { label: nav.libAll, href: "/torrents", items: leaf("") },
    {
      label: nav.libRes,
      href: "/torrents?official=0",
      items: leaf("official=0"),
    },
    {
      label: nav.official,
      href: "/torrents?official=1",
      items: leaf("official=1"),
    },
  ];
}

/** 模块开关视图（U1 §6.2）：缺键视为开（T3 缺省=现状） */
export type ModuleGate = (key: string) => boolean;

/** 维度开关视图（0330）：条目依赖某个 section 维度是否存在时用。
 *  例：出品方页只在站型挂了 `network` 维度（纪录片站）才有意义——
 *  在音乐站给一个空页面是「假成熟」，宁可整条不显示。 */
export type DimGate = (kind: string) => boolean;

export type NavConfigInput = {
  /** dict.nav / dict.tabbar（调用方从 getDict 取） */
  nav: Record<string, string>;
  tabbar: Record<string, string>;
  /** {magic} 占位符替换后的站点货币名上下文：nav.spark 等模板需要 */
  currency: string;
  modules: ModuleGate;
  /** 维度开关（可选）：缺省视为全闭，避免调用方漏传时凭空多出条目 */
  dims?: DimGate;
  customItems: MenuItem[];
  /** 顶级分类（可选，2026-10-31 资源库下拉）：传了才给资源库挂子菜单 */
  libraryCats?: LibraryCat[];
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
 *  分组顺序：资源发现 / 字幕与保种 / 魔力经济 / 成长荣誉 / 娱乐 / 更多。
 *  条目按 module 开关过滤；forums 关闭一级入口消失（三审 C-3 与 footer 同口径）。
 *  IA-1（DS-PLAN-01 W-30，2026-10-13）：发现组 12 项拆为「资源发现 /
 *  字幕与保种」两组降一层（五组杂烩在一层是「全都重要=全都不重要」的
 *  病症，六站横评支持分层诚实）；/top 从一级让位给顶栏搜索直显框
 *  （一级 5 项 → 4 项 + 搜索，同类站 同构），入口一个不删。 */
export function defaultNav({
  nav,
  tabbar,
  currency,
  modules,
  dims,
  libraryCats,
}: NavConfigInput): {
  primary: NavItem[];
  groups: NavGroup[];
} {
  const t = (tpl: string) => tpl.replace("{magic}", currency);
  const socialOn = modules("social");
  // 维度开关缺省全闭：调用方漏传 dims 时条目消失（宁可少一条，不可空页）
  const dimOn = (k: string) => dims?.(k) ?? false;
  const primary: NavItem[] = [
    { href: "/", label: nav.home },
    {
      href: "/torrents",
      label: nav.library,
      ...(libraryCats?.length
        ? { sub: librarySub(nav, libraryCats) }
        : {}),
    },
    // 教育资源四级导航（0473）：教育板块站点（分类含 13）才挂出
    ...(libraryCats && hasEduCat(libraryCats)
      ? [eduPrimary(nav.eduRes, nav)]
      : []),
    ...(modules("forums") ? [{ href: "/forums", label: nav.forums }] : []),
    { href: "/upload", label: nav.upload },
  ];
  const groups: NavGroup[] = [
    {
      group: nav.discoverRes,
      items: [
        { href: "/top", label: nav.top },
        { href: "/torrents?official=1", label: nav.official },
        ...(modules("requests")
          ? [{ href: "/requests", label: nav.requests }]
          : []),
        ...(modules("offers") ? [{ href: "/offers", label: nav.offers }] : []),
        // 出品方聚合页（0330）：仅当站型挂了 network 维度（纪录片站）才出现
        ...(dimOn("network")
          ? [{ href: "/networks", label: nav.networks }]
          : []),
        // 艺人聚合页（0327）：同理，仅音乐站挂 artist 维度时出现
        ...(dimOn("artist")
          ? [{ href: "/artists", label: nav.artists }]
          : []),
        // 当季新番（0334 anime）：仅挂 air_season 维度的站型出现
        ...(dimOn("air_season")
          ? [{ href: "/seasonal", label: nav.seasonal }]
          : []),
      ],
    },
    {
      group: nav.discoverSubs,
      items: [
        ...(modules("subtitles")
          ? [{ href: "/subtitles", label: nav.subtitles }]
          : []),
        // 字幕组聚合（0334 anime）：挂 subtitle_group 维度的站型出现
        ...(dimOn("subtitle_group")
          ? [{ href: "/subtitle-groups", label: nav.subtitleGroups }]
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
        // 追更中心（0328 前端补齐）：我在追的剧集进度 + 更新日历。
        // 无订阅时是空态，不算「假成熟」——与 /me/achievements 同类。
        { href: "/me/subscriptions", label: nav.subsCenter },
        { href: "/faq", label: nav.faq },
        // 客户端指南（0423 P0-1）：此前全站唯一入口在 /help 底部——新手
        // 「装好 qB 默认开 DHT → 私站泄漏/触发作弊判定」是注册三天被封的
        // 头号路径，防御知识不能藏在第四层。恒可见（核心做种纪律非模块）
        { href: "/clients", label: nav.clients },
        // Wiki（0359）：多页文档（BBCode + 版本史），module_wiki 开关控制显隐
        ...(modules("wiki") ? [{ href: "/wiki", label: nav.wiki }] : []),
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
