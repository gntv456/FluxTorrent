import type { ReactNode } from "react";

import { Icon } from "@/components/icons";

/** 空态语气（DS-PLAN-01 §3.2「补 tone/图标协议」）：
 *  neutral=纯展示型空态（默认，现状不变）；brand=引导型（去发第一贴/
 *  传第一颗种）；success=积极结果（如审核通过后的空收件箱）；warn=警示
 *  （无权限/被过滤）。只改图标圆底的颜色族，不动布局——四枚 tone 令牌
 *  base.css 已有，零新增令牌。 */
export type EmptyTone = "neutral" | "brand" | "success" | "warn";

/** 空态通用件（2026-10-03 新建，TIDE 设计系统第一层补齐）。
 *
 *  为什么需要：项目 i18n 字典里早有 `noComments` / `empty` 等文案，
 *  但页面侧只把它当**裸文字**渲染（居中一行，没有容器、没有引导），
 *  出现 5 处：/torrents 列表底 300px 空白、/admin 趋势图无数据、
 *  /torrent「还没有评论」、/forums 空版块、/my 等级进度「—」。
 *  有内容时好看、没内容时垮掉，是通用系统最典型的短板。
 *
 *  设计规格（三件套：图标 + 文案 + 引导动作）：
 *  - 图标 28px 描边、置于 56px 圆形浅底内，继承 currentColor；
 *  - 标题 14px/500，次要说明 12px，最多两行；
 *  - 有 action 时给主按钮，无 action 时不占位（不留空按钮框）。
 *  - tone（W-03 补）：图标圆底语气色，缺省 neutral=旧视觉，存量
 *    4 处引用零改动（error/not-found/torrent-detail/torrent-list）。
 *
 *  纯展示组件（无 hook / 无事件），故**不加 "use client"**——
 *  这样 RSC 与 client 组件都能用。
 */
export function EmptyState({
  icon = "info",
  tone,
  title,
  desc,
  action,
  compact = false,
}: {
  /** Icon 组件的图标名，默认 info（纯展示型空态） */
  icon?: string;
  /** 语气色（只影响图标圆底），缺省沿用旧视觉（neutral），向后兼容 */
  tone?: EmptyTone;
  /** 主文案，必填 —— 空态必须有明确说明，不能只有图标 */
  title: string;
  /** 次要说明，可选 */
  desc?: string;
  /** 引导动作（按钮/链接），可选 */
  action?: ReactNode;
  /** 紧凑模式：用于卡片内部 / 表格旁，缩小上下留白与图标尺寸 */
  compact?: boolean;
}) {
  return (
    <div
      className={`ui-empty${compact ? " ui-empty--compact" : ""}${
        tone && tone !== "neutral" ? ` ui-empty--${tone}` : ""
      }`}
    >
      <span className="ui-empty__icon" aria-hidden="true">
        <Icon name={icon} size={compact ? 18 : 26} />
      </span>
      <p className="ui-empty__title">{title}</p>
      {desc ? <p className="ui-empty__desc">{desc}</p> : null}
      {action ? <div className="ui-empty__action">{action}</div> : null}
    </div>
  );
}
