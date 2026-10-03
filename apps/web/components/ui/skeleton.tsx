/** 加载骨架通用件（2026-10-03 新建）。
 *
 *  补的是「有内容时好看、没内容/加载时垮掉」这条短板：全站多处加载
 *  态是一个裸 `…`（如 /my/torrentlist 的 rows===null 分支），用户
 *  不知道是加载中还是真的没有数据。骨架给出形状暗示，比转圈更省事。
 *
 *  设计规格：
 *  - 用 currentColor 的低透明度做单色块，不用彩色 shimmer（不刺眼）；
 *  - 默认 6 行模拟表格行，行高 34px 与真实行高对齐，避免加载完跳动；
 *  - 尊重 prefers-reduced-motion：关掉脉冲（见 ui.css 兜底）。
 *
 *  纯展示组件（无 hook / 无事件），故**不加 "use client"**。
 */
export function SkeletonRows({
  rows = 6,
  height = 34,
}: {
  /** 骨架行数，默认 6 */
  rows?: number;
  /** 单行高度 px，默认 34（与表格行高对齐） */
  height?: number;
}) {
  return (
    <div className="ui-skeleton" role="status" aria-busy="true">
      <span className="sr-only">加载中</span>
      {Array.from({ length: rows }, (_, i) => (
        <span
          key={i}
          className="ui-skeleton__row"
          style={{ height }}
          // 逐行错开 60ms，形成自上而下的加载波（视觉暗示进度方向）
          aria-hidden="true"
        />
      ))}
    </div>
  );
}

/** 图表/面板骨架：一个大占位块（用于 /admin 趋势图这类暂无数据的框）。 */
export function SkeletonBlock({
  height = 220,
  label,
}: {
  /** 占位块高度 px，默认 220 */
  height?: number;
  /** 无障碍标签，如「趋势图加载中」 */
  label?: string;
}) {
  return (
    <div
      className="ui-skeleton__block"
      style={{ height }}
      role="status"
      aria-busy="true"
    >
      <span className="sr-only">{label ?? "加载中"}</span>
    </div>
  );
}
