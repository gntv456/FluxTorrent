/** 标签字典共享类型与常量（从 components/admin-tagdict.tsx 按域拆出）。 */

export interface TagRow {
  id: number;
  name: string;
  kind: string;
  /** 作用域（0138）：torrent=种子域 / forum=论坛域 */
  scope?: string;
  bg_color: string;
  color: string;
  font_size: string;
  margin: string;
  padding: string;
  border_radius: string;
  sort: number;
  enabled: boolean;
  mode_id: number | null;
  /** 分组（0160 P2）：attribute=属性类 / content=内容类 */
  tag_group?: string;
  /** 层级（0160 P2）：global=通用层（跨站型）/ pack=站型层 */
  scope_layer?: string;
  /** 使用计数（0159 P1 治理）：种子引用数（后端子查询，缺省 0 兼容旧形态） */
  torrent_usage?: number;
  /** 使用计数（0159 P1 治理）：论坛主题引用数 */
  forum_usage?: number;
}

export interface ModeRow {
  id: number;
  name: string;
}

export const EMPTY: Omit<TagRow, "id"> = {
  name: "",
  kind: "plain",
  scope: "torrent",
  // W-36：新建标签默认底色走品牌蓝令牌（UI 缺省值，非内容数据）
  bg_color: "var(--sky)",
  // design-cov-exempt: 标签字典的 color 是存库 CSS 字符串（用户数据
  // 格式），标签正文惯例白字——令牌写法会让预览与存库值失真
  color: "#ffffff", // design-cov-exempt
  font_size: "12px",
  margin: "0 4px 0 0",
  padding: "1px 4px",
  border_radius: "2px",
  sort: 0,
  enabled: true,
  mode_id: null,
  tag_group: "attribute",
  scope_layer: "pack",
};

/** 预览底色：标签透明背景时衬一块中性底，否则暗色主题下白字标签贴着暗底看不见 */
export const PREVIEW_SHELL =
  "inline-block rounded-[var(--r-sm)] bg-[var(--surface-card)] " + "px-2 py-1";
