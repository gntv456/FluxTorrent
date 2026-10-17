import Link from "next/link";
import { getMembers, getClassOptions } from "@/lib/data";
import { getDict } from "@/i18n/server";
import { dateLocale, fmt, fmtCur } from "@/i18n/config";
import { formatBytes, formatRatio } from "@/lib/format";
import { PageHeader } from "@/components/ui/page-header";
import { FilterLinks } from "@/components/ui/filter-links";
import { Pager } from "@/components/ui/pager";
import { ReadState } from "@/components/ui/read-state";
import {
  BTN_SM_BOLD,
  INPUT_CARD,
  INPUT_GROW,
} from "@/lib/ui-classes";

export const dynamic = "force-dynamic";

const PER = 25;
const SORTS = ["created", "class", "uploaded", "last_seen"] as const;

function num(n: number) {
  return n.toLocaleString();
}

/** 会员列表（样图 § users）。
 *
 *  此前 `/users` 直接 302 到排行榜，注释写着「那是全站唯一的用户列表
 *  语义页」——排行榜只有前若干名，翻不到的人永远找不到。后端补了
 *  `GET /users`（登录可见、不含邮箱），这里才是那张表。
 *
 *  筛选全走 GET 表单 + 链接：不写一行客户端状态，禁用 JS 也能筛。 */
export default async function UsersPage({
  searchParams,
}: {
  searchParams: Promise<{
    q?: string;
    class_id?: string;
    sort?: string;
    page?: string;
  }>;
}) {
  const sp = await searchParams;
  const { dict, locale, currency } = await getDict();
  const t = dict.users;
  const q = (sp.q ?? "").trim().slice(0, 64);
  const classNum = Number(sp.class_id);
  const classId =
    Number.isFinite(classNum) && classNum > 0 ? classNum : undefined;
  const sort = (SORTS as readonly string[]).includes(sp.sort ?? "")
    ? (sp.sort as string)
    : "created";
  const pageNum = Number(sp.page);
  const page =
    Number.isFinite(pageNum) && pageNum > 1 ? Math.floor(pageNum) : 1;
  const [data, classes] = await Promise.all([
    getMembers({ q: q || undefined, classId, sort, page }),
    getClassOptions(),
  ]);

  // 换筛选条件一律回到第 1 页；page 只在翻页那条链上出现
  const qs = (over: Record<string, string | undefined>) => {
    const p = new URLSearchParams();
    const merged = {
      q: q || undefined,
      class_id: classId ? String(classId) : undefined,
      sort,
      ...over,
    };
    for (const [k, v] of Object.entries(merged)) if (v) p.set(k, v);
    const s = p.toString();
    return s ? `?${s}` : "";
  };
  const sortHref = (v: string) => `/users${qs({ sort: v })}`;
  const pages = data ? Math.max(1, Math.ceil(data.total / PER)) : 1;
  const hrefs = Array.from({ length: pages }, (_, i) =>
    `/users${qs({ page: i > 0 ? String(i + 1) : undefined })}`,
  );

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow={t.eyebrow}
        title={t.title}
        sub={t.note}
        aside={
          data ? (
            <span className="sticker">
              {fmt(t.sub, { n: num(data.total) })}
            </span>
          ) : undefined
        }
      />

      {/* 筛选：搜索 + 等级 + 排序，一个 GET 表单搞定 */}
      <form
        action="/users"
        method="get"
        className="flex flex-wrap items-center gap-2"
      >
        <input
          name="q"
          defaultValue={q}
          placeholder={t.searchPh}
          className={`${INPUT_GROW} text-ink placeholder:text-sub`}
        />
        <select
          name="class_id"
          defaultValue={classId ? String(classId) : ""}
          className={`${INPUT_CARD} px-3 text-sm text-ink`}
        >
          <option value="">{t.classAll}</option>
          {classes.map((c) => (
            <option key={c.id} value={String(c.id)}>
              {c.name}
            </option>
          ))}
        </select>
        <button
          type="submit"
          className={BTN_SM_BOLD}
        >
          {t.searchBtn}
        </button>
      </form>

      <FilterLinks
        label={t.sortLabel}
        value={sort}
        options={[
          { value: "created", label: t.sortNewest, href: sortHref("created") },
          { value: "class", label: t.sortClass, href: sortHref("class") },
          {
            value: "uploaded",
            label: t.sortUpload,
            href: sortHref("uploaded"),
          },
          {
            value: "last_seen",
            label: t.sortLastSeen,
            href: sortHref("last_seen"),
          },
        ]}
      />

      <ReadState
        failed={data === null}
        empty={(data?.rows.length ?? 0) === 0}
        dict={dict}
        emptyTitle={t.empty}
        emptyDesc={t.emptyHint}
        emptyIcon="users"
      >
        <div className="baozi-panel overflow-x-auto">
          <table className="nexus-table">
            <thead>
              <tr>
                <td className="colhead">{t.colUser}</td>
                <td className="colhead w-24">{t.colClass}</td>
                <td className="colhead w-24 text-right">{t.colUp}</td>
                <td className="colhead w-24 text-right">{t.colDown}</td>
                <td className="colhead w-16 text-right">{t.colRatio}</td>
                <td className="colhead w-16 text-right">{t.colSeeding}</td>
                <td className="colhead w-24 text-right">
                  {fmtCur(t.colSpark, {}, currency)}
                </td>
                <td className="colhead w-24">{t.colJoined}</td>
                <td className="colhead w-24">{t.colLastSeen}</td>
                <td className="colhead w-20 text-right">{t.colAction}</td>
              </tr>
            </thead>
            <tbody>
              {(data?.rows ?? []).map((m) => (
                <tr key={m.id}>
                  <td>
                    <Link
                      href={`/users/${m.id}`}
                      className="font-bold text-ink hover:text-[var(--sky)]"
                    >
                      {m.username}
                    </Link>
                    <div className="text-xs text-faint">
                      {t.uid} {m.id}
                    </div>
                  </td>
                  <td className="text-xs text-sub">
                    {m.class_name ?? m.class_id}
                  </td>
                  <td className="num text-right text-up">
                    {formatBytes(m.uploaded)}
                  </td>
                  <td className="num text-right text-down">
                    {formatBytes(m.downloaded)}
                  </td>
                  <td className="num text-right">
                    {formatRatio(m.uploaded, m.downloaded)}
                  </td>
                  <td className="num text-right">{num(m.seeding)}</td>
                  <td className="num text-right">{num(m.spark_balance)}</td>
                  <td className="num text-xs text-sub">
                    {new Date(m.created_at).toLocaleDateString(
                      dateLocale(locale),
                    )}
                  </td>
                  <td className="num text-xs text-sub">
                    {m.last_seen_at
                      ? new Date(m.last_seen_at).toLocaleDateString(
                          dateLocale(locale),
                        )
                      : "—"}
                  </td>
                  <td className="text-right">
                    <Link
                      href={`/users/${m.id}`}
                      className="text-xs font-bold text-[var(--sky)]"
                    >
                      {t.viewHome}
                    </Link>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </ReadState>

      {data && pages > 1 && (
        <Pager
          page={data.page}
          pages={pages}
          total={data.total}
          hrefs={hrefs}
        />
      )}
    </div>
  );
}
