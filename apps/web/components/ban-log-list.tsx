"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import { FilterLinks } from "@/components/ui/filter-links";
import { Pager } from "@/components/ui/pager";
import { EmptyState } from "@/components/ui/empty-state";
import { SkeletonRows } from "@/components/ui/skeleton";
import { INPUT_GROW, BTN_SM_BOLD } from "@/lib/ui-classes";

/** 封禁公示名单（样图 § 信息与辅助 / 封禁记录 那张六列表）。
 *
 *  本站此前的 /ban-log 只有「按用户名点查」（公开、防枚举、5 次/分），
 *  没有名单——而 admin_http/user_status.rs:177 的注释早就写着「用户只能从
 *  公开 ban-log 猜状态」。这里补的是名单面，点查那一栏原样保留在同页上方。
 *
 *  四态分开说（读失败 ≠ 没数据 ≠ 没登录）：
 *  · 401/2001 → 「需登录」，不是「本站没有封禁记录」；
 *  · 其它失败 → 读口失败文案；
 *  · 200 零行 → 空态 + 一句「这不等于从未封禁过」。
 *  用户名由后端打码后下发，这一层拿不到原名。 */

interface Row {
  uid: number;
  name_masked: string;
  status: number;
  reason: string;
  acted_at: string | null;
  warned_until: string | null;
  cheat_banned_until: string | null;
}

interface Envelope {
  rows: Row[];
  total: number;
  page: number;
  per: number;
}

const PER = 20;

export function BanLogList() {
  const { dict, locale } = useI18n();
  const t = dict.banlog;
  const [data, setData] = useState<Envelope | null>(null);
  const [page, setPage] = useState(1);
  const [kind, setKind] = useState("");
  const [qInput, setQInput] = useState("");
  const [q, setQ] = useState("");
  const [failed, setFailed] = useState(false);
  const [needLogin, setNeedLogin] = useState(false);

  const load = useCallback(() => {
    const sp = new URLSearchParams();
    if (q) sp.set("q", q);
    if (kind) sp.set("kind", kind);
    sp.set("page", String(page));
    sp.set("per", String(PER));
    setData(null);
    setFailed(false);
    setNeedLogin(false);
    api
      .get<Envelope>(`/api/v1/ban-log?${sp.toString()}`)
      .then(setData)
      .catch((e) => {
        // 未登录与后端故障是两句话：前者给登录入口，后者才说取档失败
        if (e instanceof ApiError && (e.status === 401 || e.code === 2001)) {
          setNeedLogin(true);
        } else {
          setFailed(true);
        }
      });
  }, [q, kind, page]);
  useEffect(load, [load]);

  const rows = data?.rows ?? [];
  const pages = Math.ceil((data?.total ?? 0) / PER);
  const term = (r: Row): string => {
    const until = r.cheat_banned_until ?? r.warned_until;
    if (until) return fmt(t.termUntil, { at: until.slice(0, 10) });
    if (r.status === 2) return t.termPermanent;
    return t.termUnknown;
  };

  return (
    <section className="brand-panel overflow-hidden">
      <div className="brand-panel__head">
        <h2>{t.listTitle}</h2>
        {data && (
          <span className="text-xs text-sub">
            {fmt(t.headCount, { n: data.total })}
          </span>
        )}
      </div>

      <div className="flex flex-wrap items-center gap-2 px-4 pt-3">
        <input
          value={qInput}
          onChange={(e) => setQInput(e.target.value)}
          placeholder={t.searchPh}
          aria-label={t.searchPh}
          maxLength={64}
          className={INPUT_GROW}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              setPage(1);
              setQ(qInput.trim());
            }
          }}
        />
        <button
          type="button"
          className={BTN_SM_BOLD}
          onClick={() => {
            setPage(1);
            setQ(qInput.trim());
          }}
        >
          {t.submit}
        </button>
        <FilterLinks
          label={t.filterLabel}
          value={kind || undefined}
          onChange={(v) => {
            setPage(1);
            setKind(v ?? "");
          }}
          options={[
            { value: undefined, label: t.kindAll },
            { value: "ban", label: t.banned },
            { value: "mute", label: t.muted },
          ]}
        />
        {q !== "" && (
          <button
            type="button"
            className="text-xs text-sub underline"
            onClick={() => {
              setQ("");
              setQInput("");
              setPage(1);
            }}
          >
            {t.clear}
          </button>
        )}
      </div>

      {data === null && !failed && !needLogin && <SkeletonRows />}

      {needLogin && (
        <p className="px-4 py-6 text-center text-sm text-sub">
          {t.needLogin}{" "}
          <Link
            href="/login"
            className="font-bold text-[var(--sky)] underline"
          >
            {dict.forgot.backToLogin}
          </Link>
        </p>
      )}

      {failed && (
        <p className="px-4 py-6 text-center text-sm text-sub" role="alert">
          {dict.readState.failTitle}：{dict.readState.failDesc}
        </p>
      )}

      {data && rows.length === 0 && (
        <EmptyState
          icon="shield"
          title={t.listEmpty}
          desc={t.listEmptyHint}
        />
      )}

      {data && rows.length > 0 && (
        <div className="overflow-x-auto">
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">{t.colUser}</td>
                <td className="colhead w-20">{t.colKind}</td>
                <td className="colhead">{t.colReason}</td>
                <td className="colhead w-36">{t.colDate}</td>
                <td className="colhead w-24">{t.colTerm}</td>
                <td className="colhead w-16 text-right">{t.colAction}</td>
              </tr>
              {rows.map((r) => (
                <tr key={r.uid}>
                  <td className="max-w-[200px] truncate font-bold">
                    {/* 打码名字不给链接：/users/{uid} 会把原名带回去 */}
                    <span title={t.whyMasked}>{r.name_masked}</span>
                  </td>
                  <td className="text-xs">
                    {r.status === 2 ? t.banned : t.muted}
                  </td>
                  <td className="max-w-[300px] text-sm">
                    {r.reason || <span className="text-sub">{t.noReason}</span>}
                  </td>
                  <td className="text-xs text-sub">
                    {r.acted_at
                      ? new Date(r.acted_at).toLocaleString(
                          dateLocale(locale),
                        )
                      : t.dateUnknown}
                  </td>
                  <td className="text-xs">{term(r)}</td>
                  <td className="text-right">
                    <Link
                      href="/appeals"
                      className="text-xs font-bold text-[var(--sky)] underline"
                    >
                      {t.appeal}
                    </Link>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="px-4 py-2 text-xs text-sub">{t.listNote}</p>
          {pages > 1 && (
            <div className="px-4 pb-3">
              <Pager
                layout="compact"
                page={page}
                pages={pages}
                posLabel={`${fmt(dict.common.pageX, { n: page })} / ${pages}`}
                onStep={setPage}
              />
            </div>
          )}
        </div>
      )}
    </section>
  );
}
