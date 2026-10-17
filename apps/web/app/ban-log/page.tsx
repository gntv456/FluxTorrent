"use client";

import { BTN_LG_SKY, INPUT_LG } from "@/lib/ui-classes";
import { BanLogList } from "@/components/ban-log-list";
import { PageHeader } from "@/components/ui/page-header";

import { useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";

interface BanLog {
  status: number;
  changed_at: string | null;
}

/** 封禁记录（NexusPHP user-ban-log.php 口径，公开）：
 *  ①按用户名查当前状态与最近变更时间（匿名可用、防枚举、限频 5 次/分）；
 *  ②登录成员看整份公示名单（BanLogList，名字后端已打码）。
 *
 *  2026-10-17 对齐样图 § ban-log：原先只有 ①，且页头是「大图标 + 居中大标题」
 *  的散页排法；现在页头走 PageHeader，点查收进一张面板，名单面板在其下方。
 *  两件事各有存在理由——被封的人登不进来，只能靠 ① 自助确认状态。 */
export default function BanLogPage() {
  const { dict, locale } = useI18n();
  const [username, setUsername] = useState("");
  const [result, setResult] = useState<BanLog | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    setResult(null);
    try {
      setResult(
        await api.get<BanLog>(
          `/api/v1/auth/ban-log?username=${encodeURIComponent(username.trim())}`,
        ),
      );
    } catch (err) {
      setMsg(
        err instanceof ApiError
          ? (apiErrorMessage(dict, err))
          : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  const statusLabel = (s: number) =>
    s === 2
      ? dict.banlog.banned
      : s === 1
        ? dict.banlog.muted
        : dict.banlog.normal;

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-4">
      <PageHeader
        eyebrow={dict.banlog.eyebrow}
        title={dict.banlog.title}
        sub={dict.banlog.listSub}
      />

      <section className="baozi-panel">
        <div className="baozi-panel__head">
          <h2>{dict.banlog.lookupTitle}</h2>
          <span className="text-xs text-sub">{dict.banlog.subtitle}</span>
        </div>
        <form onSubmit={submit} className="flex flex-col gap-3 px-4 pb-4">
          <label className="flex flex-col gap-1">
            <span className="text-sm text-sub">{dict.banlog.username}</span>
            <input
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              required
              maxLength={24}
              className={INPUT_LG}
            />
          </label>
          {msg && (
            <p role="alert" className="text-sm text-danger">
              {msg}
            </p>
          )}
          <button type="submit" disabled={busy} className={BTN_LG_SKY}>
            {busy ? dict.banlog.busy : dict.banlog.submit}
          </button>
        </form>
        {result && (
          <div
            role="status"
            className="mx-4 mb-4 rounded-[var(--r-lg)] border border-line bg-[var(--surface-card)] p-4 text-sm"
          >
            <p className="flex justify-between">
              <span className="text-sub">{dict.banlog.currentStatus}</span>
              <span
                className={`font-bold ${
                  result.status === 2 ? "text-danger" : "text-mint"
                }`}
              >
                {statusLabel(result.status)}
              </span>
            </p>
            {result.changed_at && (
              <p className="mt-1 flex justify-between">
                <span className="text-sub">{dict.banlog.changedAt}</span>
                <span className="num">
                  {new Date(result.changed_at).toLocaleString(
                    dateLocale(locale),
                  )}
                </span>
              </p>
            )}
            {result.status === 2 && (
              <p className="mt-2 text-xs text-sub">
                {dict.banlog.appealHint}{" "}
                <Link href="/appeals" className="font-bold text-sky">
                  {dict.login.appealLink}
                </Link>
              </p>
            )}
          </div>
        )}
        <p className="px-4 pb-3 text-xs text-sub">{dict.banlog.note}</p>
      </section>

      <BanLogList />
    </div>
  );
}
