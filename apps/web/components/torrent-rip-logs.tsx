"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
import { Badge } from "@/components/ui/badge";
import { formatBytes } from "@/lib/format";
import { Fold } from "@/components/torrent-detail-parts";

/**
 * 抓轨日志面板（0312 音乐站 Logchecker；0337 加人工改判）。
 *
 * 清单来自详情页 aggregate 的 `logs` 段（不含正文）；正文按碟懒加载
 * ——单份 EAC 日志可上百 KiB，全内联会把首屏与共享段缓存一起打爆。
 * 分数与问题都以后端解析结果为准，前端不重算（两套口径必然漂移）。
 * 0337 起展示**有效分** = adjusted_score ?? log_score；改判按钮常显，
 * 非版主由后端 403 兜底（与删评按钮同一范式）。
 */

export interface RipLog {
  ordinal: number;
  filename: string;
  engine: string;
  /** 0–100；null = 无法定分（不是 0 分） */
  log_score: number | null;
  /** 人工改判分（0337）；null/缺省 = 未改判 */
  adjusted_score?: number | null;
  adjust_reason?: string | null;
  tracks: number;
  issues: { code: string; msg: string }[];
  size: number;
}

interface RipLogBody extends RipLog {
  torrent_id: number;
  body: string;
}

export function TorrentRipLogs({
  torrentId,
  logs,
}: {
  torrentId: number;
  logs: RipLog[];
}) {
  const { dict } = useI18n();
  const d = dict.tdetail;
  const router = useRouter();
  const [open, setOpen] = useState<number | null>(null);
  const [body, setBody] = useState<Record<number, string>>({});
  const [failed, setFailed] = useState<number | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  // P2 清偿：人工改判输入状态机（prompt 退役）
  const ask = useConfirm();

  const toggle = async (ordinal: number) => {
    if (open === ordinal) {
      setOpen(null);
      return;
    }
    setOpen(ordinal);
    if (body[ordinal] !== undefined) return;
    try {
      const r = await api.get<RipLogBody>(
        `/api/v1/torrents/${torrentId}/logs/${ordinal}`,
      );
      setBody((prev) => ({ ...prev, [ordinal]: r.body }));
      setFailed(null);
    } catch {
      setFailed(ordinal);
    }
  };

  /** 人工改判（0337）：空输入 = 撤销改判。
   *  P2 清偿：window.prompt → ConfirmDialog 输入槽（回填当前分）。 */
  const askAdjust = (l: RipLog) => {
    const cur = l.adjusted_score ?? l.log_score ?? "";
    ask.open({
      title: d.adjustLog,
      label: d.adjustHint,
      initial: cur === "" ? "" : String(cur),
      run: async (raw) => {
        const trimmed = raw.trim();
        let score: number | null = null;
        if (trimmed !== "") {
          const v = Number(trimmed);
          if (!Number.isFinite(v) || v < 0 || v > 100) {
            setMsg(d.adjustFail);
            return;
          }
          score = Math.round(v);
        }
        try {
          await api.post(
            `/api/v1/admin/torrents/${torrentId}/logs/${l.ordinal}/adjust`,
            { score },
          );
          setMsg(d.adjustDone);
          router.refresh();
        } catch (e) {
          setMsg(e instanceof ApiError ? e.message : d.adjustFail);
        }
      },
    });
  };

  // 无日志行整段不渲染（与 mediainfo/nfo 同口径）；段落显隐门在调用方
  if (logs.length === 0) return null;
  return (
    <Fold title={d.ripLogs} count={logs.length}>
      {msg && <p className="mb-1 text-xs text-sub">{msg}</p>}
      <ul className="td-riplogs">
        {logs.map((l) => {
          const eff = l.adjusted_score ?? l.log_score;
          const adjusted =
            l.adjusted_score !== null && l.adjusted_score !== undefined;
          return (
            <li key={l.ordinal} className="td-riplogs__item">
              <div className="td-riplogs__head">
                <span className="td-riplogs__disc">
                  {d.ripDisc.replace("{n}", `${l.ordinal + 1}`)}
                </span>
                <Badge {...scoreTone(eff)}>
                  {eff === null ? d.ripUnscored : `${eff}% · ${l.engine}`}
                </Badge>
                {adjusted && (
                  <Badge tone="brand" variant="solid">
                    {d.adjustLog}
                  </Badge>
                )}
                <span className="td-riplogs__file">{l.filename}</span>
                <span className="td-riplogs__meta">
                  {d.ripTracks.replace("{n}", `${l.tracks}`)} ·{" "}
                  {formatBytes(l.size)}
                </span>
                <button
                  type="button"
                  className="td-riplogs__toggle"
                  onClick={() => toggle(l.ordinal)}
                >
                  {d.ripViewLog}
                </button>
                <button
                  type="button"
                  className="td-riplogs__toggle"
                  onClick={() => askAdjust(l)}
                >
                  {d.adjustLog}
                </button>
              </div>
              {l.issues.length > 0 && (
                <ul className="td-riplogs__issues">
                  {l.issues.map((i) => (
                    <li key={i.code} className="text-sub">
                      {i.msg}
                    </li>
                  ))}
                </ul>
              )}
              {open === l.ordinal && (
                <pre className="td-nfo">
                  {body[l.ordinal] ??
                    (failed === l.ordinal ? d.ripLoadFailed : d.ripLoading)}
                </pre>
              )}
            </li>
          );
        })}
      </ul>
      <AskDialog {...ask} />
    </Fold>
  );
}

/** 分数→色档：满分最常见、未定分要看得出、低分必须显眼 */
/** 有效性评分徽章三态（2026-10-16 S2 归 Badge：未评分=brand 实底、
 *  ≥100%=warn 浅底、<100%=brand 浅底）。原实现直接返回 Tailwind 色类
 *  串（bg-indigo text-white 等），把配色写死在计算里。 */
function scoreTone(score: number | null): {
  tone: "brand" | "warn";
  variant: "soft" | "solid";
} {
  if (score === null) return { tone: "brand", variant: "solid" };
  if (score >= 100) return { tone: "warn", variant: "soft" };
  return { tone: "brand", variant: "soft" };
}
