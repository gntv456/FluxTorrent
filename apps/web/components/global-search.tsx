"use client";

import { useEffect, useRef, useState } from "react";

import { BusyLabel } from "@/components/ui/busy-label";
import { EmptyState } from "@/components/ui/empty-state";

/**
 * 头栏全局搜索（0283 P1-6 → W-25 四态化）：
 * 聚合联想——种子（/torrents/suggest）+ 论坛主题（/forums/search）双路，
 * 回车跳种子列表常规搜索；点候选项直达详情/帖子。
 *
 * W-25 三态补齐（原实现只有「静默吞错 + 空 recovery」两态）：
 *  - 搜索中：BusyLabel spinner（350ms 防抖后请求在途时）；
 *  - 无结果：EmptyState compact（tipEmpty 文案 + 图标圆底）；
 *  - 请求失败：EmptyState warn + 重试按钮（重发当前词）。
 *  fetch 失败不再静默——用户打了一堆字下拉却空白，比报错更迷惑。
 */

interface TorrentHit {
  id: number;
  name: string;
  seeders: number;
}
interface TopicHit {
  topic_id: number;
  title: string;
}
interface Hits {
  torrents: TorrentHit[];
  topics: TopicHit[];
}

const EMPTY: Hits = { torrents: [], topics: [] };

export function GlobalSearch({
  placeholder,
  tipTorrents,
  tipTopics,
  tipEmpty,
  tipMore,
  tipSearching,
  tipError,
  retryLabel,
}: {
  placeholder: string;
  tipTorrents: string;
  tipTopics: string;
  tipEmpty: string;
  tipMore: string;
  /** W-25 三态文案（搜索中 / 失败 / 重试钮） */
  tipSearching: string;
  tipError: string;
  retryLabel: string;
}) {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<Hits>(EMPTY);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const [nonce, setNonce] = useState(0); // 失败后「重试」重发当前词
  const boxRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const query = q.trim();
    if (query.length < 2) {
      setHits(EMPTY);
      setBusy(false);
      setFailed(false);
      return;
    }
    let aborted = false;
    setBusy(true);
    setFailed(false);
    const timer = setTimeout(async () => {
      try {
        const enc = encodeURIComponent(query);
        // 双路各自降级：单路失败不拖垮另一路（null = 该路无结果）
        const [tRes, fRes] = await Promise.all([
          fetch(`/api/v1/torrents/suggest?q=${enc}`).catch(() => null),
          fetch(`/api/v1/forums/search?q=${enc}&limit=3`).catch(() => null),
        ]);
        if (aborted) return;
        // 两路全挂才算失败（W-25：单路静默、双路才提示可重试）
        if (!tRes && !fRes) {
          setFailed(true);
          setHits(EMPTY);
          return;
        }
        const torrents: TorrentHit[] =
          (await tRes?.json())
            ?.data?.items?.slice(0, 5)
            ?.map((x: TorrentHit) => x) ?? [];
        const rawTopics = (await fRes?.json())?.data;
        const topics: TopicHit[] = (Array.isArray(rawTopics)
          ? rawTopics
          : rawTopics?.items ?? []
        ).slice(0, 3);
        setHits({ torrents, topics });
        setOpen(true);
      } catch {
        if (!aborted) {
          setFailed(true);
          setHits(EMPTY);
        }
      } finally {
        if (!aborted) setBusy(false);
      }
    }, 350);
    return () => {
      aborted = true;
      clearTimeout(timer);
      setBusy(false);
    };
  }, [q, nonce]);

  useEffect(() => {
    const onDoc = (e: MouseEvent) => {
      if (boxRef.current && !boxRef.current.contains(e.target as Node))
        setOpen(false);
    };
    document.addEventListener("click", onDoc);
    return () => document.removeEventListener("click", onDoc);
  }, []);

  const typed = q.trim().length >= 2;
  const hasHits = hits.torrents.length + hits.topics.length > 0;
  return (
    <div ref={boxRef} className="gsearch">
      <form
        action="/torrents"
        method="get"
        className="gsearch__form"
        onSubmit={() => setOpen(false)}
      >
        <input
          type="search"
          name="search"
          className="gsearch__input"
          placeholder={placeholder}
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onFocus={() => typed && hasHits && setOpen(true)}
          autoComplete="off"
          aria-label={placeholder}
        />
      </form>
      {open && typed && (
        <div className="gsearch__drop" role="listbox">
          {busy ? (
            <div className="gsearch__state">
              <BusyLabel label={tipSearching} />
            </div>
          ) : failed ? (
            <div className="gsearch__state">
              <EmptyState compact icon="warning" tone="warn" title={tipError} />
              <button
                type="button"
                className="gsearch__retry"
                onClick={() => setNonce((n) => n + 1)}
              >
                {retryLabel}
              </button>
            </div>
          ) : (
            <>
              {!hasHits && (
                <EmptyState compact icon="search" title={tipEmpty} />
              )}
              {hits.torrents.length > 0 && (
                <>
                  <div className="gsearch__cap">{tipTorrents}</div>
                  {hits.torrents.map((t) => (
                    <a
                      key={t.id}
                      href={`/torrent/${t.id}`}
                      className="gsearch__item"
                    >
                      <span className="gsearch__name">{t.name}</span>
                      {t.seeders > 0 && (
                        <span className="gsearch__meta">{t.seeders} ↑</span>
                      )}
                    </a>
                  ))}
                  <a
                    href={`/torrents?search=${encodeURIComponent(q.trim())}`}
                    className="gsearch__item gsearch__item--more"
                  >
                    {tipMore}
                  </a>
                </>
              )}
              {hits.topics.length > 0 && (
                <>
                  <div className="gsearch__cap">{tipTopics}</div>
                  {hits.topics.map((t) => (
                    <a
                      key={t.topic_id}
                      href={`/forums/topic/${t.topic_id}`}
                      className="gsearch__item"
                    >
                      <span className="gsearch__name">{t.title}</span>
                    </a>
                  ))}
                </>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
