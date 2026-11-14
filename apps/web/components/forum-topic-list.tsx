"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import type { BoardBrief, Topic } from "@fluxtorrent/domain-types";
import { TypeBadge, TagChip } from "@/components/forum-bits";
import { useI18n } from "@/i18n/client";
import { ForumTopicRow } from "@/components/forum-topic-row";
import { topicStamp } from "@/lib/topic-stamp";
import { ForumModBar } from "./forum-mod-bar";

/** 版块页主题列表（客户端）。
 *
 *  为什么必须是客户端组件：`can_mod` 时列表要承载**勾选态**（批量管理的输入）。
 *
 *  行样式已收编到 forum-topic-row.tsx——与首页话题流同一套 fh-topic
 *  行式（用户口径：两处只是内容不同，样式不该两样）。本列表装配版块页
 *  特有内容：类型/标签徽章 + 回复/浏览数字右列；版主勾选占行首。
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
  const { dict } = useI18n();
  const router = useRouter();
  const [picked, setPicked] = useState<number[]>([]);
  const pageIds = topics.map((t) => t.id);

  function toggle(id: number) {
    setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p, id]));
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

      {topics.length === 0 ? (
        <p className="fh-stream__empty">{dict.forums.noTopics}</p>
      ) : (
        <div className="fh-stream__list">
          {topics.map((t) => (
            <ForumTopicRow
              key={t.id}
              topic={t}
              picked={picked.includes(t.id)}
              onPick={canMod ? () => toggle(t.id) : undefined}
              pickLabel={`${dict.forums.topicTitleFallback}: ${t.title}`}
              marks={{
                notice: dict.forums.markNotice,
                pin: dict.forums.markPin,
                ess: dict.forums.markEss,
                locked: dict.forums.locked,
              }}
              slots={{
                // 版块页接口不回 created_at，meta 时间以最后发言兜底
                metaTime: topicStamp(t.last_post_at),
                metaExtra: (
                  <>
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
                  </>
                ),
                right: (
                  <div className="fh-topic__nums">
                    {/* 右列数字块：回复 / 浏览上下两行，列头文案取
                        「回复 {n}」剥掉占位符后的裸词（与旧表格列头同源） */}
                    <span>
                      <b className="num">{Math.max(t.replies, 0)}</b>{" "}
                      {dict.forums.replies.replace("{n}", "").trim() ||
                        dict.forums.replies}
                    </span>
                    <span>
                      <b className="num">{t.views}</b>{" "}
                      {dict.forums.views.replace("{n}", "").trim() ||
                        dict.forums.views}
                    </span>
                  </div>
                ),
              }}
            />
          ))}
        </div>
      )}
    </div>
  );
}
