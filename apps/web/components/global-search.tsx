"use client";

import { useEffect, useRef, useState } from "react";

/**
 * 头栏全局搜索（0283 P1-6）：聚合联想——种子（/torrents/suggest）+
 * 论坛主题（/forums/search）双路，回车跳种子列表常规搜索；
 * 点候选项直达详情/帖子。桌面 md+ 显示（移动端用列表页搜索盒）。
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
}: {
  placeholder: string;
  tipTorrents: string;
  tipTopics: string;
  tipEmpty: string;
  tipMore: string;
}) {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<Hits>(EMPTY);
  const [open, setOpen] = useState(false);
  const boxRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const query = q.trim();
    if (query.length < 2) {
      setHits(EMPTY);
      return;
    }
    let aborted = false;
    const timer = setTimeout(async () => {
      try {
        const enc = encodeURIComponent(query);
        const [tRes, fRes] = await Promise.all([
          fetch(`/api/v1/torrents/suggest?q=${enc}`).catch(() => null),
          fetch(`/api/v1/forums/search?q=${enc}&limit=3`).catch(() => null),
        ]);
        if (aborted) return;
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
        /* 静默 */
      }
    }, 350);
    return () => {
      aborted = true;
      clearTimeout(timer);
    };
  }, [q]);

  useEffect(() => {
    const onDoc = (e: MouseEvent) => {
      if (boxRef.current && !boxRef.current.contains(e.target as Node))
        setOpen(false);
    };
    document.addEventListener("click", onDoc);
    return () => document.removeEventListener("click", onDoc);
  }, []);

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
          onFocus={() => q.trim().length >= 2 && setOpen(true)}
          autoComplete="off"
          aria-label={placeholder}
        />
      </form>
      {open && q.trim().length >= 2 && (
        <div className="gsearch__drop" role="listbox">
          {!hasHits && <div className="gsearch__empty">{tipEmpty}</div>}
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
        </div>
      )}
    </div>
  );
}
