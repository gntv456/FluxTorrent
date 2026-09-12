"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface RssInfo {
  urls: { label: string; url: string }[];
  passkey: string;
}

/** 线性图标（24×24 stroke，仿好学站 userbar 图标语义） */
function Icon({ d, extra }: { d: string; extra?: string }) {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.9"
      strokeLinecap="round"
      strokeLinejoin="round"
      className="userbar-tool__icon"
    >
      <path d={d} />
      {extra && <path d={extra} />}
    </svg>
  );
}

const ICONS = {
  inbox: { d: "M3 12 L7 6 H17 L21 12 V19 H3 Z M3 12 H9 A3 3 0 0 0 15 12 H21" },
  sent: { d: "M21 3 L10 14 M21 3 L14 21 L10 14 L3 10 Z" },
  cheaters: { d: "M8 11 A3.5 3.5 0 1 0 8 4 A3.5 3.5 0 1 0 8 11 M2.5 20 A5.5 5.5 0 0 1 13.5 20", extra: "M15.5 8.5 L21.5 14.5 M21.5 8.5 L15.5 14.5" },
  flag: { d: "M5 21 V4 M5 4 C8 2.5 11 5.5 14.5 4.5 C16.5 4 18 3.5 19 4 V12.5 C18 13 16.5 13.5 14.5 14 C11 15 8 12 5 13.5" },
  staff: { d: "M3 6 H21 V18 H3 Z M3 7 L12 13 L21 7 M8 18 L4.5 21.5 M16 18 L19.5 21.5" },
  social: { d: "M9 11 A3.5 3.5 0 1 0 9 4 A3.5 3.5 0 1 0 9 11 M2.5 20 A6.5 6.5 0 0 1 15.5 20 M16 4.5 A3.2 3.2 0 1 1 16.5 10.5 M17.5 14.5 A5.8 5.8 0 0 1 21.5 19.8" },
  rss: { d: "M5 11 A8.5 8.5 0 0 1 13.5 19.5 M5 5.5 A14 14 0 0 1 19 19.5" },
};

/** 快捷工具条（好学站 userbar 口径）：收件箱/发件箱/作弊者/举报信箱/管理组信箱/社交/RSS。
 *  仅图标按钮，4+3 上下两行；悬停 title 提示。 */
export function UserTools() {
  const { dict } = useI18n();
  const t = dict.usertools;
  const [rss, setRss] = useState<RssInfo | null>(null);
  const [showRss, setShowRss] = useState(false);
  const [reportOpen, setReportOpen] = useState(false);
  const [refType, setRefType] = useState("torrent");
  const [refId, setRefId] = useState("");
  const [reason, setReason] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (showRss && !rss) {
      api
        .get<RssInfo>("/api/v1/rss-info")
        .then(setRss)
        .catch(() => setRss(null));
    }
  }, [showRss, rss]);

  async function submitReport() {
    if (!refId || !reason.trim()) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/reports", {
        ref_type: refType,
        ref_id: Number(refId),
        reason: reason.trim(),
      });
      setReportOpen(false);
      setRefId("");
      setReason("");
      setMsg(t.reportOk);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="usertools usertools--grid">
      <a className="userbar-tool userbar-tool--icon" href="/messages" title={t.inbox} aria-label={t.inbox}>
        <Icon {...ICONS.inbox} />
      </a>
      <a className="userbar-tool userbar-tool--icon" href="/messages?box=sent" title={t.sentbox} aria-label={t.sentbox}>
        <Icon {...ICONS.sent} />
      </a>
      <a className="userbar-tool userbar-tool--icon" href="/admin?tool=cheaters" title={t.cheaters} aria-label={t.cheaters}>
        <Icon {...ICONS.cheaters} />
      </a>
      <button
        type="button"
        className="userbar-tool userbar-tool--icon"
        title={t.reportBox}
        aria-label={t.reportBox}
        onClick={() => {
          setReportOpen((v) => !v);
          setShowRss(false);
        }}
      >
        <Icon {...ICONS.flag} />
      </button>
      <a className="userbar-tool userbar-tool--icon" href="/contactstaff" title={t.staffBox} aria-label={t.staffBox}>
        <Icon {...ICONS.staff} />
      </a>
      <a className="userbar-tool userbar-tool--icon" href="/friends" title={t.socialList} aria-label={t.socialList}>
        <Icon {...ICONS.social} />
      </a>
      <button
        type="button"
        className="userbar-tool userbar-tool--icon"
        title={t.getRss}
        aria-label={t.getRss}
        onClick={() => {
          setShowRss((v) => !v);
          setReportOpen(false);
        }}
      >
        <Icon {...ICONS.rss} />
      </button>
      {msg && <p className="usertools__msg">{msg}</p>}

      {/* 举报信箱弹层 */}
      {reportOpen && (
        <div className="usertools-popover">
          <h3>{t.reportBox}</h3>
          <div className="usertools-form">
            <label>
              {t.reportType}
              <select value={refType} onChange={(e) => setRefType(e.target.value)}>
                <option value="torrent">{t.rtTorrent}</option>
                <option value="comment">{t.rtComment}</option>
                <option value="user">{t.rtUser}</option>
                <option value="subtitle">{t.rtSubtitle}</option>
                <option value="forum">{t.rtForum}</option>
              </select>
            </label>
            <label>
              {t.reportId}
              <input
                type="text"
                inputMode="numeric"
                value={refId}
                onChange={(e) => setRefId(e.target.value.replace(/\D/g, ""))}
                placeholder="#"
              />
            </label>
            <label>
              {t.reportReason}
              <textarea
                rows={4}
                value={reason}
                onChange={(e) => setReason(e.target.value)}
                maxLength={500}
              />
            </label>
            <button
              type="button"
              className="baozi-button"
              disabled={busy || !refId || !reason.trim()}
              onClick={submitReport}
            >
              {t.reportSubmit}
            </button>
          </div>
        </div>
      )}

      {/* RSS 弹层 */}
      {showRss && (
        <div className="usertools-popover">
          <h3>{t.getRss}</h3>
          {rss ? (
            <div className="usertools-rss">
              <p className="usertools-rss__note">{t.rssNote}</p>
              {rss.urls.map((u) => (
                <label key={u.url}>
                  <span>{u.label}</span>
                  <input type="password" readOnly value={u.url} />
                </label>
              ))}
            </div>
          ) : (
            <p className="usertools-rss__note">{dict.my.loading}</p>
          )}
        </div>
      )}
    </div>
  );
}
