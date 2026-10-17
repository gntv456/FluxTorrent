"use client";

/**
 * 账户概览「明细」子面板（从 usercp-overview-panels.tsx 按域再拆，300 行
 * 门禁）：MoreInfoTable 更多账户信息 + RecentTopics 最近阅读主题。
 */

import { useState } from "react";
import Link from "next/link";
import { useI18n } from "@/i18n/client";
import { dateLocale, type Locale } from "@/i18n/config";
import { toast } from "@/components/ui/toast";
import { BTN_XS_GHOST } from "@/lib/ui-classes";
import type { Overview } from "@/components/usercp";
import type { DictProp } from "@/components/usercp-overview-panels";

function daysSince(iso: string | null): number {
  if (!iso) return 0;
  const ms = Date.now() - new Date(iso).getTime();
  return Math.max(0, Math.floor(ms / 86400000));
}

/** 更多账户信息（经典 rowhead/rowfollow 表格） */
export function MoreInfoTable({
  ov,
  t,
  dict,
  currency,
  locale,
}: {
  ov: Overview;
  t: Record<string, string>;
  currency: string;
  locale: Locale;
} & DictProp) {
  const [copied, setCopied] = useState(false);
  async function copyPk() {
    try {
      await navigator.clipboard.writeText(ov.passkey);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // 非安全上下文/无权限时 clipboard 直接 reject：把值打回提示里让人手抄
      toast.error(dict.common.networkError);
    }
  }
  return (
    <details className="uc-more" open>
      <summary>{t.moreTitle}</summary>
      <div className="brand-wide-table-scroll">
      <table className="nexus-table nexus-form">
        <tbody>
          <tr>
            <td className="rowhead">{t.joinDate}</td>
            <td className="rowfollow">
              {new Date(ov.created_at ?? "").toLocaleString(
                dateLocale(locale),
              )}（
              {daysSince(ov.created_at)}
              {dict.usercp.days2}）
            </td>
          </tr>
          <tr>
            <td className="rowhead">{t.email}</td>
            <td className="rowfollow">{ov.email}</td>
          </tr>
          <tr>
            <td className="rowhead">{t.ip}</td>
            <td className="rowfollow">
              <span className="uc-hidden-text">
                {ov.passkey ? "127.0.0.1" : "—"}
              </span>
            </td>
          </tr>
          <tr>
            <td className="rowhead">{t.passkeyRow}</td>
            <td className="rowfollow">
              <span className="uc-hidden-text">{ov.passkey}</span>{" "}
              {/* 复制放这儿（passkey 要贴进下载端，此前只能选中打码文本手抄）；
                  重置不在概览里直接发 POST —— 那是不可逆操作，guarded 流程
                  （确认框 + 新钥回显）住在安全面板，这里用锚点跳过去。 */}
              <button
                type="button"
                className={BTN_XS_GHOST}
                onClick={copyPk}
              >
                {copied ? dict.ui.copied : dict.usercp.copy}
              </button>{" "}
              <Link href="/my?tab=security#passkey" className={BTN_XS_GHOST}>
                {dict.usercp.reset}
              </Link>
            </td>
          </tr>
          <tr>
            <td className="rowhead">{t.inviteRow}</td>
            <td className="rowfollow">
              {ov.invites_pending} [<Link href="/invites">{t.send}</Link>]
            </td>
          </tr>
          <tr>
            <td className="rowhead">
              {dict.my.balance.replace("{magic}", currency)}
            </td>
            <td className="rowfollow">
              {ov.spark_balance} [<Link href="/shop">{t.use}</Link>]
            </td>
          </tr>
          <tr>
            <td className="rowhead">{t.commentsRow}</td>
            <td className="rowfollow">
              {ov.comments}[
              <Link href={`/users/${ov.id}#comments`}>{t.view}</Link>]
            </td>
          </tr>
          <tr>
            <td className="rowhead">{t.tokenRow}</td>
            <td className="rowfollow">
              {/* #tokens 锚点：安全面板平铺十余项，直跳 API 令牌那一行
                  （见 usercp-security.tsx 的 hash 滚动） */}
              <Link href="/my?tab=security#tokens" className="sticker">
                {t.create}
              </Link>
            </td>
          </tr>
        </tbody>
      </table>
      </div>
    </details>
  );
}

/** 最近阅读主题（样图 § ucp 概览末尾那张表）。
 *  此前只渲染 4 个 colhead、没有数据行——表头俱备、内容恒空。
 *  数据是 GET /me/overview 的 recent_topics（topic_reads 按阅读时间倒序 5 条，
 *  SQL 侧已按 minclassread 过滤：读不到的版块不会在这里露头）。 */
export function RecentTopics({
  rows,
  t,
  dict,
  locale,
}: {
  rows: Overview["recent_topics"];
  t: Record<string, string>;
  locale: Locale;
} & DictProp) {
  const u = dict.usercp;
  const list = rows ?? [];
  return (
    <section className="uc-recent-topics">
      <h2>{t.recentTopics}</h2>
      <div className="uc-recent-topics__wrap">
        <div className="brand-wide-table-scroll">
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead" style={{ width: "56%" }}>
                {t.colTopic}
              </td>
              <td className="colhead">{u.rtColBoard}</td>
              <td className="colhead" style={{ textAlign: "center" }}>
                {u.rtColReplies}
              </td>
              <td
                className="colhead"
                style={{ width: "20%", textAlign: "center" }}
              >
                {u.rtColRead}
              </td>
            </tr>
            {list.length === 0 && (
              <tr>
                <td colSpan={4} className="py-4 text-center text-sub">
                  {u.rtEmpty}
                </td>
              </tr>
            )}
            {list.map((r) => (
              <tr key={r.topic_id}>
                <td className="max-w-[360px] truncate">
                  <Link
                    href={`/forums/topic/${r.topic_id}`}
                    className="text-[var(--sky-deep)] hover:underline"
                    title={r.title}
                  >
                    {r.title}
                  </Link>
                </td>
                <td>
                  <Link
                    href={`/forums/${r.forum_id}`}
                    className="text-sub hover:underline"
                  >
                    {r.forum_name}
                  </Link>
                </td>
                <td className="num text-center">{r.replies}</td>
                <td className="text-center text-xs text-sub">
                  {new Date(r.read_at).toLocaleString(dateLocale(locale))}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        </div>
      </div>
    </section>
  );
}
