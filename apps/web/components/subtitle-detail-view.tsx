"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import {
  fmtKB,
  langLabelOf,
  loadLangDict,
} from "@/components/subtitle-board-format";
import { SubtitleEditDialog } from "@/components/subtitle-edit-dialog";

/** 字幕详情页主体（0150 缺口1）：下载统计卡 + 全元数据 + 修订版链。
 *  数据 GET /api/v1/subtitles/{id}/detail；编辑入口（缺口2）复用
 *  SubtitleEditDialog（本人/staff 可见）。 */

interface SubDetail {
  id: number;
  torrent_id: number | null;
  user_id: number;
  username: string | null;
  title: string;
  lang: string | null;
  downloads: number;
  size: number;
  ext: string | null;
  fps: number | null;
  machine_translated: boolean;
  hearing_impaired: boolean;
  foreign_parts_only: boolean;
  source: string | null;
  producer: string | null;
  proofreader: string | null;
  author_name: string | null;
  release_name: string | null;
  anon: boolean;
  verified: boolean;
  parent_id: number | null;
  imdb_id: string | null;
  rating: number | null;
  rating_count: number;
  bad_reports: number;
  created_at: string;
  ai_state: string;
  cert_tier: string | null;
  my_vote: number | null;
  parents: [number, string, string | null, string][];
  children: [number, string, string | null, string][];
  can_modify: boolean;
}

export function SubtitleDetailView({ sid }: { sid: number }) {
  const { dict } = useI18n();
  const t = dict.subtitles;
  const [d, setD] = useState<SubDetail | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);

  const load = useCallback(async () => {
    try {
      await loadLangDict();
      setD(
        await api.get<SubDetail>(`/api/v1/subtitles/${sid}/detail`),
      );
    } catch (e) {
      setErr(e instanceof Error ? e.message : "load failed");
    }
  }, [sid]);

  useEffect(() => {
    void load();
  }, [load]);

  if (err) {
    return <p className="py-8 text-center text-coral">{err}</p>;
  }
  if (!d) {
    return <p className="py-8 text-center text-sub">…</p>;
  }
  const dt = dict.subtitleDetail;
  const metaRows: [string, React.ReactNode][] = [
    [dt.lang, langLabelOf(d.lang)],
    [dt.size, fmtKB(d.size)],
    [dt.ext, d.ext ?? "—"],
    [dt.fps, d.fps ? String(d.fps) : "—"],
    [
      dt.aiState,
      d.ai_state === "ai"
        ? (t.aiBadgePure ?? "MT")
        : d.ai_state === "ai_proofread"
          ? (t.aiBadgeProof ?? "MT✓")
          : (dt.aiHuman ?? "human"),
    ],
    [dt.rating, d.rating ? `★ ${d.rating} (${d.rating_count})` : "—"],
    [dt.downloads, d.downloads],
    [dt.source, d.source ?? "—"],
    [dt.producer, d.producer ?? "—"],
    [dt.proofreader, d.proofreader ?? "—"],
    [dt.author, d.author_name ?? (d.username ?? "—")],
    [dt.release, d.release_name ?? "—"],
    [dt.imdb, d.imdb_id ?? "—"],
    [dt.badReports, d.bad_reports],
    [dt.created, new Date(d.created_at).toLocaleString()],
  ];
  return (
    <div className="subtitles-wrap">
      <header className="subtitles-head">
        <strong>{d.title}</strong>
        <small>
          {d.verified && <span className="text-mint">✓ </span>}
          #{d.id}
          {d.cert_tier && (
            <span
              className={
                d.cert_tier === "gold"
                  ? "ml-1 text-amber-500"
                  : "ml-1 text-mint"
              }
            >
              {d.cert_tier === "gold" ? "✎★" : "✎"}
            </span>
          )}
        </small>
      </header>

      <div className="mb-3 flex flex-wrap gap-2">
        <a
          href={`/api/v1/subtitles/${d.id}/download`}
          target="_blank"
          rel="noreferrer"
          className="btn"
        >
          {dt.download}
        </a>
        {d.torrent_id && (
          <Link href={`/torrent/${d.torrent_id}`} className="btn2">
            {dt.torrent}
          </Link>
        )}
        {d.can_modify && (
          <button
            type="button"
            className="btn2"
            onClick={() => setEditing(true)}
          >
            {t.editLabel}
          </button>
        )}
      </div>

      <section className="subtitles-rules">
        <h2>{dt.metaTitle}</h2>
        <div className="baozi-wide-table-scroll">
        <table className="nexus-table">
          <tbody>
            {metaRows.map(([k, v]) => (
              <tr key={k}>
                <td className="rowhead w-32">{k}</td>
                <td>{v}</td>
              </tr>
            ))}
          </tbody>
        </table>
        </div>
      </section>

      {(d.parents.length > 0 || d.children.length > 0) && (
        <section className="subtitles-rules">
          <h2>{dt.versions}</h2>
          <div className="baozi-wide-table-scroll">
          <table className="nexus-table">
            <tbody>
              {d.parents.map(([id, title, lang, at]) => (
                <tr key={`p-${id}`}>
                  <td className="w-20 text-center text-sub">
                    {dt.parentTag}
                  </td>
                  <td>
                    <Link href={`/subtitles/${id}`} className="font-bold">
                      {title}
                    </Link>
                    <span className="ml-2 text-xs text-sub">
                      {langLabelOf(lang)} ·{" "}
                      {new Date(at).toLocaleDateString()}
                    </span>
                  </td>
                </tr>
              ))}
              <tr>
                <td className="w-20 text-center font-bold">
                  {dt.currentTag}
                </td>
                <td className="font-bold">{d.title}</td>
              </tr>
              {d.children.map(([id, title, lang, at]) => (
                <tr key={`c-${id}`}>
                  <td className="w-20 text-center text-sub">
                    {dt.childTag}
                  </td>
                  <td>
                    <Link href={`/subtitles/${id}`} className="font-bold">
                      {title}
                    </Link>
                    <span className="ml-2 text-xs text-sub">
                      {langLabelOf(lang)} ·{" "}
                      {new Date(at).toLocaleDateString()}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          </div>
        </section>
      )}

      {editing && (
        <SubtitleEditDialog
          sid={d.id}
          initial={{
            title: d.title,
            fps: d.fps ? String(d.fps) : "",
            source: d.source ?? "",
            producer: d.producer ?? "",
            proofreader: d.proofreader ?? "",
            author_name: d.author_name ?? "",
            machine_translated: d.machine_translated,
            lang: d.lang ?? "",
          }}
          onClose={() => setEditing(false)}
          onSaved={() => {
            setEditing(false);
            void load();
          }}
        />
      )}
    </div>
  );
}
