"use client";

import { BTN_SM_SKY } from "@/lib/ui-classes";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { AdminOpsJobs } from "@/components/admin-ops-jobs";

/** 后台运维三件套（0078，U3D 口径）：
 *  ① 版本页 GET /admin/version（git 版本 + 组件运行信息）
 *  ② 备份面板 GET /admin/backups + POST /admin/backups/run
 *  ③ 任务调度（0218 G7：拆至 admin-ops-jobs.tsx，目录/最近运行/入队触发） */

interface VersionInfo {
  app: string;
  batch: string;
  git: string;
  db: { users: number; torrents: number; active_peers: number };
  latest_migration: string;
}

/** /admin/update_check 响应（0276 检查更新） */
interface UpdateInfo {
  current: string;
  latest: string;
  up_to_date: boolean;
  release_name: string | null;
  published_at: string | null;
  url: string | null;
  notes: string | null;
}

interface BackupsInfo {
  dir: string;
  files: [string, number][];
}

function fmtBytes(n: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(2)} ${units[i]}`;
}

export function AdminOpsPanel() {
  const { dict } = useI18n();
  const t = dict.adminops;
  const [ver, setVer] = useState<VersionInfo | null>(null);
  const [upd, setUpd] = useState<UpdateInfo | null>(null);
  const [backups, setBackups] = useState<BackupsInfo | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [v, b] = await Promise.all([
        api.get<VersionInfo>("/api/v1/admin/version").catch(() => null),
        api.get<BackupsInfo>("/api/v1/admin/backups").catch(() => null),
      ]);
      setVer(v);
      setBackups(b);
    } catch {
      /* 面板级静默 */
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  async function updateCheck() {
    setBusy("update");
    setUpd(null);
    try {
      const r = await api.get<UpdateInfo>("/api/v1/admin/update_check");
      setUpd(r);
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.updateFailed);
    } finally {
      setBusy(null);
    }
  }

  async function backupRun() {
    setBusy("backup");
    setMsg(null);
    try {
      const r = await api.post<{ file: string; bytes: number }>(
        "/api/v1/admin/backups/run",
        {},
      );
      setMsg(
        t.backupDone
          .replace("{file}", r.file)
          .replace("{size}", fmtBytes(r.bytes)),
      );
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : t.runFailed);
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="flex flex-col gap-4">
      {/* ① 版本信息卡（含检查更新，0276） */}
      <div className="baozi-panel flex flex-col gap-2 p-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h2 className="text-base font-bold">🧭 {t.versionTitle}</h2>
          <button
            type="button"
            disabled={busy === "update"}
            onClick={updateCheck}
            className={BTN_SM_SKY}
          >
            {busy === "update" ? t.updateChecking : t.updateCheck}
          </button>
        </div>
        {upd && (
          <p
            className={
              upd.up_to_date
                ? "rounded-[var(--r-md)] bg-sky-soft p-2 text-xs text-ink"
                : "rounded-[var(--r-md)] p-2 text-xs text-ink"
            }
            style={
              upd.up_to_date
                ? undefined
                : { background: "var(--warning-soft)" }
            }
            role="status"
          >
            {upd.up_to_date
              ? fmt(t.updateToDate, { current: upd.current })
              : fmt(t.updateAvailable, {
                  latest: upd.latest,
                  current: upd.current,
                })}
            {upd.published_at && (
              <>
                {" · "}
                {t.updateReleasedAt} {upd.published_at.slice(0, 10)}
              </>
            )}
            {upd.url && !upd.up_to_date && (
              <>
                {" · "}
                <a
                  className="text-link"
                  href={upd.url}
                  target="_blank"
                  rel="noreferrer"
                >
                  {upd.release_name || upd.latest}
                </a>
              </>
            )}
          </p>
        )}
        {ver ? (
          <>
            <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm md:grid-cols-3">
              <div>
                <dt className="text-xs text-sub">{t.versionBatch}</dt>
                <dd className="font-bold">
                  {ver.app} · {ver.batch}
                </dd>
              </div>
              <div className="md:col-span-2">
                <dt className="text-xs text-sub">{t.versionGit}</dt>
                <dd className="font-mono text-xs">{ver.git || "—"}</dd>
              </div>
              <div className="md:col-span-2">
                <dt className="text-xs text-sub">{t.versionMigration}</dt>
                <dd className="font-mono text-xs">
                  {ver.latest_migration || "—"}
                </dd>
              </div>
            </dl>
            <div className="flex flex-wrap gap-x-5 text-xs text-sub">
              <span>
                {t.versionDb}：{t.versionUsers}{" "}
                <b className="num text-ink">{ver.db.users.toLocaleString()}</b>
              </span>
              <span>
                {t.versionTorrents}{" "}
                <b className="num text-ink">
                  {ver.db.torrents.toLocaleString()}
                </b>
              </span>
              <span>
                {t.versionPeers}{" "}
                <b className="num text-ink">
                  {ver.db.active_peers.toLocaleString()}
                </b>
              </span>
            </div>
          </>
        ) : (
          <p className="py-2 text-xs text-sub">…</p>
        )}
      </div>

      {/* ② 备份面板 */}
      <div className="baozi-panel flex flex-col gap-2 p-4">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h2 className="text-base font-bold">💾 {t.backupsTitle}</h2>
          <button
            type="button"
            disabled={busy === "backup"}
            onClick={backupRun}
            className={BTN_SM_SKY}
          >
            {busy === "backup" ? "…" : t.backupRun}
          </button>
        </div>
        {backups && (
          <p className="text-xs text-sub">
            {t.backupsDir}：<code className="font-mono">{backups.dir}</code>
          </p>
        )}
        <div className="baozi-wide-table-scroll">
          <table className="nexus-table text-xs">
            <thead>
              <tr>
                <td className="colhead">{t.backupsFile}</td>
                <td className="colhead w-28">{t.backupsSize}</td>
              </tr>
            </thead>
            <tbody>
              {(backups?.files ?? []).map(([name, size]) => (
                <tr key={name}>
                  <td className="font-mono">{name}</td>
                  <td className="num">{fmtBytes(size)}</td>
                </tr>
              ))}
              {backups !== null && backups.files.length === 0 && (
                <tr>
                  <td colSpan={2} className="py-4 text-center text-sub">
                    {t.backupsEmpty}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* ③ 任务调度（0218 G7：目录/最近运行/入队触发，见 admin-ops-jobs.tsx） */}
      <AdminOpsJobs />

      {msg && (
        <p
          className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink"
          role="status"
        >
          {fmt(msg, {})}
        </p>
      )}
    </section>
  );
}
