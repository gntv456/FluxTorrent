"use client";

import { useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface RssInfo {
  urls: { label: string; url: string }[];
  passkey: string;
}

/** 快捷工具条（包子站 userbar__tools 复刻）：
 *  举报信箱 / 管理组信箱（PM 管理组）/ 社交名单 / 获取RSS */
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
    <div className="usertools">
      <button
        type="button"
        className="userbar-tool"
        title={t.reportBox}
        onClick={() => {
          setReportOpen((v) => !v);
          setShowRss(false);
        }}
      >
        <span aria-hidden="true">♡</span>
        <b>0</b>
        <span className="userbar-tool__label">{t.reportBox}</span>
      </button>
      <a className="userbar-tool" href="/contactstaff" title={t.staffBox}>
        <span aria-hidden="true">♡</span>
        <b>0</b>
        <span className="userbar-tool__label">{t.staffBox}</span>
      </a>
      <a className="userbar-tool" href="/friends" title={t.socialList}>
        <span aria-hidden="true">♡</span>
        <span className="userbar-tool__label">{t.socialList}</span>
      </a>
      <button
        type="button"
        className="userbar-tool"
        title={t.getRss}
        onClick={() => {
          setShowRss((v) => !v);
          setReportOpen(false);
        }}
      >
        <span aria-hidden="true">📡</span>
        <span className="userbar-tool__label">{t.getRss}</span>
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
