"use client";

import { useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import type { BoardBrief, Topic } from "@fluxtorrent/domain-types";
import { TypeBadge, TagChip } from "@/components/forum-bits";
import { Icon } from "@/components/icons";
import { dateLocale } from "@/i18n/config";
import { useI18n } from "@/i18n/client";
import { useIsCompact } from "@/lib/hooks/use-media";
import { ForumModBar } from "./forum-mod-bar";

/** 版块页主题列表（客户端）。
 *
 *  为什么必须是客户端组件：`can_mod` 时列表要承载**勾选态**（批量管理的输入），
 *  而 RSC 的 `<table>` 无法持有交互状态。展示内容与改造前的 SSR 版本一致。
 *
 *  行内勾选框只在 `canMod` 时渲染 —— 普通用户看到的表格与从前完全一样。
 */
export function ForumTopicList({
  topics,
  canMod,
  forums,
}: {
  topics: Topic[];
  canMod: boolean;
  /** can_mod 时才有值：批量「移动到」下拉的数据源（全站可读版块） */
  forums: BoardBrief[];
}) {
  const { dict, locale } = useI18n();
  const compact = useIsCompact();
  const router = useRouter();
  const [picked, setPicked] = useState<number[]>([]);
  const pageIds = topics.map((t) => t.id);

  function toggle(id: number) {
    setPicked((p) =>
      p.includes(id) ? p.filter((x) => x !== id) : [...p, id],
    );
  }

  return (
    <div>
      {canMod && (
        <ForumModBar
          ids={picked}
          pageIds={pageIds}
          forums={forums}
          onPickedAll={setPicked}
          onDone={() => router.refresh()}
        />
      )}

      {compact ? (
        <div className="ftopic-cards">
          {topics.map((t) => (
            <a
              key={t.id}
              href={`/forums/topic/${t.id}`}
              className="ftopic-card"
            >
              <p className="ftopic-card__title">
                {t.sticky ? "📌 " : ""}
                {t.digest ? "💎 " : ""}
                {t.title}
              </p>
              <p className="ftopic-card__meta">
                <span>{t.username ?? dict.torrent.anonymous}</span>
                <span className="ftopic-card__nums">
                  💬 {Math.max(0, t.replies)} · 👁 {t.views}
                </span>
              </p>
            </a>
          ))}
        </div>
      ) : (
      <div className="baozi-wide-table-scroll">
      <table className="nexus-table">
        <thead>
          <tr>
            {canMod && <td className="colhead w-8" />}
            <td className="colhead">{dict.forums.topicTitleFallback}</td>
            <td className="colhead w-32">{dict.forums.author}</td>
            <td className="colhead w-16 text-right">
              {dict.forums.replies.replace("{n}", "").trim() ||
                dict.forums.replies}
            </td>
            <td className="colhead hidden w-20 text-right sm:table-cell">
              {dict.forums.views.replace("{n}", "").trim() ||
                dict.forums.views}
            </td>
            <td className="colhead hidden w-28 text-right sm:table-cell">
              {dict.forums.lastPost}
            </td>
          </tr>
        </thead>
        <tbody>
          {topics.map((t) => (
            <tr key={t.id}>
              {canMod && (
                <td>
                  <input
                    type="checkbox"
                    checked={picked.includes(t.id)}
                    onChange={() => toggle(t.id)}
                    aria-label={`${dict.forums.topicTitleFallback}: ${t.title}`}
                  />
                </td>
              )}
              <td>
                {t.digest && (
                  <span
                    aria-label="精华"
                    className="mr-1 text-[var(--baozi-orange-dark)]"
                  >
                    ⭐
                  </span>
                )}
                {t.sticky && (
                  <Icon
                    name="pin"
                    size={13}
                    className="mr-1 inline align-[-2px] font-bold text-coral"
                    title="置顶"
                  />
                )}
                {t.locked && (
                  <Icon
                    name="lock"
                    size={13}
                    className="mr-1 inline align-[-2px]"
                    title="已锁定"
                  />
                )}
                <TypeBadge
                  type={t.topic_type}
                  label={
                    t.topic_type
                      ? (dict.forums.types as Record<string, string>)[
                          t.topic_type
                        ]
                      : undefined
                  }
                  className="mr-1 align-middle"
                />
                {t.tags?.map((tg) => (
                  <TagChip
                    key={tg.id}
                    tag={tg}
                    className="mr-1 align-middle"
                  />
                ))}
                <Link
                  href={`/forums/topic/${t.id}`}
                  className="font-bold text-ink hover:text-sky"
                >
                  {t.title}
                </Link>
              </td>
              <td className="text-sub">{t.username ?? "—"}</td>
              <td className="num text-right">{Math.max(t.replies, 0)}</td>
              <td className="num hidden text-right sm:table-cell">
                {t.views}
              </td>
              {/* 日期按**访问者本地时区**渲染，容器（UTC）与浏览器必然渲染出
                  不同文本 —— 这是有意的差异，非 bug。抑制 hydration 告警
                  （React 对时间戳场景的官方建议）。 */}
              <td
                className="num hidden text-right text-sub sm:table-cell"
                suppressHydrationWarning
              >
                {t.last_post_at
                  ? new Date(t.last_post_at).toLocaleDateString(
                      dateLocale(locale),
                    )
                  : "—"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      </div>
      )}
    </div>
  );
}
