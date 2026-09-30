/**
 * 娱乐屋玩法注册表（单一来源）。
 *
 * 大厅卡片、分组、模块开关、角标来源全部从这里读 —— 新增玩法 = 加一条 +
 * 一条路由 + 一个页面，大厅页代码零改动。
 *
 * 纯数据、无 hooks、无 "use client"：服务端大厅与客户端导航共用同一份。
 */

/** 卡面主色（对应 arcade.css 的 .gc-tone-* 渐变，全部走 TIDE 令牌）。 */
export type GameTone =
  | "sky"
  | "coral"
  | "mint"
  | "sun"
  | "candy"
  | "indigo"
  | "violet"
  | "teal";

/** 分组：即时开奖（快）／养成经营（慢）。 */
export type GameGroup = "instant" | "session";

export interface GameEntry {
  /** 字典键：dict.games.cards.<key>；也是前台稳定标识 */
  key: string;
  href: string;
  /** 卡面主图标（emoji，零资源依赖） */
  icon: string;
  tone: GameTone;
  group: GameGroup;
  /** 站点模块开关名（site profile modules）；缺省 "games" */
  module?: string;
  /** 是否已上线：false → 卡片置灰标注「即将开放」，不可点 */
  live: boolean;
  /** 角标数据来源（/api/v1/games 概览字段）；缺省无角标 */
  badge?:
    | "scratch"
    | "jgg"
    | "bigsmall"
    | "farm"
    | "capsule"
    | "wheel"
    | "fishing";
  /** 大厅首屏「热门」标记 */
  hot?: boolean;
}

/** 九款玩法：4 即时（猜大小/刮刮乐/九宫格）+ 2 台面（扭蛋/大转盘）
 *  + 4 养成（农场/抽卡/宠物/钓鱼）。顺序即大厅展示顺序。 */
export const GAMES: GameEntry[] = [
  {
    key: "bigsmall",
    href: "/games/bigsmall",
    icon: "🎯",
    tone: "sky",
    group: "instant",
    live: true,
    badge: "bigsmall",
    hot: true,
  },
  {
    key: "scratch",
    href: "/games/scratch",
    icon: "🎫",
    tone: "coral",
    group: "instant",
    live: true,
    badge: "scratch",
    hot: true,
  },
  {
    key: "jgg",
    href: "/games/jgg",
    icon: "🎰",
    tone: "candy",
    group: "instant",
    live: true,
    badge: "jgg",
  },
  {
    key: "capsule",
    href: "/games/capsule",
    icon: "🥚",
    tone: "violet",
    group: "instant",
    live: true,
    badge: "capsule",
  },
  {
    key: "wheel",
    href: "/games/wheel",
    icon: "🎡",
    tone: "sun",
    group: "instant",
    live: true,
    badge: "wheel",
    hot: true,
  },
  {
    key: "farm",
    href: "/games/farm",
    icon: "🌾",
    tone: "mint",
    group: "session",
    module: "farm",
    live: true,
    badge: "farm",
  },
  {
    key: "gacha",
    href: "/gacha",
    icon: "🃏",
    tone: "indigo",
    group: "session",
    module: "gacha",
    live: true,
  },
  {
    key: "pet",
    href: "/games/pet",
    icon: "🐾",
    tone: "teal",
    group: "session",
    live: true,
  },
  {
    key: "fishing",
    href: "/games/fishing",
    icon: "🎣",
    tone: "sky",
    group: "session",
    live: true,
    badge: "fishing",
  },
];
