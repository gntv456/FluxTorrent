import type { Dict } from "@/i18n/zh-CN";

/** 首页数据契约与共享工具（从 home-sections.tsx 按域拆出，300 门禁）：
 *  HomeData 接口 + 字节格式化；各板块部件文件共用，避免运行时循环依赖。 */

export interface HomeData {
  news: {
    id: number;
    title: string;
    body: string;
    badge: string;
    date: string;
  }[];
  attendance: {
    month: string;
    streak: number;
    total_days: number;
    checked_today: boolean;
    calendar: { date: string; day: number; done: boolean; reward: number }[];
    makeup_cards?: number;
  };
  resource_stats: {
    today: number;
    avg7: number;
    total30: number;
    series: {
      date: string;
      ordinary: number;
      official: number;
      total: number;
    }[];
  };
  site_data: {
    users: number;
    torrents: number;
    peers: number;
    seeders: number;
    leechers: number;
    warned: number;
    banned: number;
    unverified: number;
    total_upload: number;
    total_download: number;
    total_size: number;
  };
  lucky_draw: { user: string; kind: string; amount: number }[];
  friend_links: { name: string; url: string; title: string | null }[];
  /** 首页排版（0089）：JSON 数组字符串或空串（空 = 默认布局） */
  home_layout?: string;
}

/** 首页板块文案字典段（原文件里 ReturnType<typeof useI18n> 的等价类型） */
export type Home2T = Dict["home2"];
export type DictT = Dict;

export function fmtBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(3)} ${units[i]}`;
}
