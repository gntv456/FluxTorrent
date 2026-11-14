"use client";

import Link from "next/link";
import type { Topic } from "@fluxtorrent/domain-types";
import { TypeBadge, TagChip, MarkBadge } from "@/components/forum-bits";
import { Icon } from "@/components/icons";
import { topicStamp } from "@/lib/topic-stamp";

/** 论坛话题行——首页话题流与版块页主题列表**共用**的行组件。
 *
 *  为什么收编成一份：此前首页是 fh-topic 头像行、版块页是 nexus-table
 *  表格，两处样式各写各的（用户点名：只是展现的内容不一样，样式不该
 *  两样）。样式真源 = 本组件 + forums-home.css 的 .fh-topic 系；两处
 *  只差内容槽位，由调用方按页装配（见 ForumRowSlots）。
 *
 *  客户端组件：版块页 can_mod 的勾选态要在这里持有。首页话题流（RSC
 *  页面）import 本 client 组件作元素渲染是合法用法（rsc_boundary 口径）。
 */

/** 头像渐变色板（样图 avColor 的五条，按用户名散列）；色源 --fhue-* */
const AV_GRADIENTS = [
  "linear-gradient(135deg,var(--fhue-sky),var(--fhue-indigo))",
  "linear-gradient(135deg,var(--fhue-mint),var(--fhue-sky))",
  "linear-gradient(135deg,var(--fhue-coral),var(--fhue-sun))",
  "linear-gradient(135deg,var(--fhue-indigo),var(--fhue-candy))",
  "linear-gradient(135deg,var(--fhue-coral),var(--fhue-indigo))",
];

const hash = (s: string) =>
  [...s].reduce((h, c) => (h * 31 + c.codePointAt(0)!) % 100000, 7);

const avatarBg = (name: string) =>
  AV_GRADIENTS[hash(name) % AV_GRADIENTS.length];

export function TopicAvatar({ name }: { name: string }) {
  return (
    <span
      className="fh-topic__av"
      style={{ background: avatarBg(name) }}
      aria-hidden="true"
    >
      {name.slice(0, 1).toUpperCase()}
    </span>
  );
}

/** 行内标记：公告 > 置顶 > 精华（文字徽章，用户要求不再要图钉/星形
 *  图标），多个并存就都显示；锁定单独出锁图标。 */
function RowMarks({
  topic,
  labels,
}: {
  topic: Topic;
  labels: { notice: string; pin: string; ess: string; locked: string };
}) {
  return (
    <>
      {topic.notice && <MarkBadge kind="notice" label={labels.notice} />}
      {topic.sticky && <MarkBadge kind="pin" label={labels.pin} />}
      {topic.digest && <MarkBadge kind="ess" label={labels.ess} />}
      {topic.locked && (
        <Icon
          name="lock"
          size={13}
          className="mr-1 inline align-[-2px]"
          title={labels.locked}
        />
      )}
    </>
  );
}

export interface ForumRowSlots {
  /** 标题行前缀（版块徽章：首页话题流用；版块页省略——已在自己的版块里） */
  board?: React.ReactNode;
  /** 标题行后缀（热度标：首页话题流用） */
  afterTitle?: React.ReactNode;
  /** meta 行时间文本（发布时间；版块页接口不回 created_at 时传 last_post_at） */
  metaTime?: React.ReactNode;
  /** meta 行扩展（类型/标签徽章：版块页用；首页无） */
  metaExtra?: React.ReactNode;
  /** 右列内容：首页=最后回复人+时间；版块页=回复/浏览数+最后回复时间 */
  right: React.ReactNode;
}

export function ForumTopicRow({
  topic,
  picked,
  onPick,
  pickLabel,
  slots,
  marks,
}: {
  topic: Topic;
  /** 版主勾选态（onPick 缺省 = 不渲染勾选框，普通用户视图） */
  picked?: boolean;
  onPick?: () => void;
  /** 勾选框的无障碍名（「主题：<标题>」） */
  pickLabel?: string;
  slots: ForumRowSlots;
  marks: { notice: string; pin: string; ess: string; locked: string };
}) {
  // 行底色三档（优先级 公告 > 置顶 > 精华），样式在 .fh-topic--* 系
  const rowTone = topic.notice
    ? " fh-topic--notice"
    : topic.sticky
      ? " fh-topic--pin"
      : topic.digest
        ? " fh-topic--ess"
        : "";
  const author = topic.username;
  return (
    <div className={`fh-topic${rowTone}`} data-topic-id={topic.id}>
      {/* 版主勾选：行首占位（.fh-topic__pick 与头像同宽，保持对齐） */}
      {onPick && (
        <input
          type="checkbox"
          className="fh-topic__pick"
          checked={!!picked}
          onChange={onPick}
          aria-label={pickLabel}
        />
      )}
      <TopicAvatar name={author ?? "?"} />
      <div className="fh-topic__main">
        <div className="fh-topic__title">
          <RowMarks topic={topic} labels={marks} />
          <Link href={`/forums/topic/${topic.id}`} className="fh-topic__t">
            {topic.title}
          </Link>
          {slots.afterTitle}
        </div>
        <div className="fh-topic__meta">
          {slots.board}
          <b>{author ?? "—"}</b>
          <span className="fh-topic__sep">·</span>
          {/* meta = **发布时间**（与右列最后回复时间是两个值）。容器跑
              UTC、浏览器在本地时区，文本必然不同——有意的差异，抑制
              hydration 告警。版块页接口不回 created_at ⇒ 调用方传
              last_post_at 兜底（见 slots.metaTime 注释）。 */}
          <span suppressHydrationWarning>{slots.metaTime ?? "—"}</span>
          {slots.metaExtra}
        </div>
      </div>
      <div className="fh-topic__stats">{slots.right}</div>
    </div>
  );
}
