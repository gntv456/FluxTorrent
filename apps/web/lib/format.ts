import type { TorrentListItem, UserPublic } from "@fluxtorrent/domain-types";

/** 体积人性化（§8.2：金额/流量整数最小单位，前端负责格式化） */
export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(1)} ${units[i]}`;
}

/** 分享率：INF 特殊样式（设计稿数据口径） */
export function formatRatio(up: number, down: number): string {
  if (down === 0) return "INF";
  const r = up / down;
  return r >= 100 ? "INF" : r.toFixed(2);
}

/** 学科分类色（设计稿六色系 → 七分类映射） */
const CATEGORY_COLORS: Record<number, string> = {
  1: "#ff8fc7",
  2: "#2fa8ff",
  3: "#2fbf9b",
  4: "#ffc93c",
  5: "#5b6bf5",
  6: "#ff7a59",
  7: "#93a1bc",
};

export function categoryColor(categoryId: number): string {
  return CATEGORY_COLORS[categoryId] ?? "#93a1bc";
}

/** 促销徽章色（§7.3：免费=薄荷绿 / 2x=珊瑚橙） */
export function promotionBadge(
  promotion: TorrentListItem["promotion"],
): { label: string; className: string } | null {
  switch (promotion) {
    case "free":
      return { label: "免费", className: "bg-mint text-white" };
    case "x2":
      return { label: "2x", className: "bg-coral text-white" };
    case "x2free":
      return { label: "2x免费", className: "bg-coral text-white" };
    case "half":
      return { label: "50%", className: "bg-sun text-ink" };
    case "x2half":
      return { label: "2x 50%", className: "bg-sun text-ink" };
    case "p30":
      return { label: "30%", className: "bg-sun text-ink" };
    default:
      return null;
  }
}

/** 用户等级 → 成长豆荚名称（§3.3，数据口径与后端 user_classes 一致） */
export function podName(user: Pick<UserPublic, "class_name">): string {
  return user.class_name;
}
