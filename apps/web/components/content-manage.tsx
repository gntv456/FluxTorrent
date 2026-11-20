"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { FunTab, LinksTab } from "@/components/content-manage-tabs";
import { NewsTab, MgmtTabs } from "@/components/content-manage-news";
import type { AdminNewsRow } from "@/components/content-manage-news";

/** 内容管理（管理后台扩展）：公告 / 趣味盒 / 友情链接 的发布·编辑·删除·禁止·审核。
 *  趣味盒与友链两个 tab 拆出 content-manage-tabs.tsx、公告 tab 拆出
 *  content-manage-news.tsx（0505，300 门禁）。 */

interface FunItem {
  id: number;
  username: string | null;
  title: string;
  body: string | null;
  status: string;
  added: string;
  fun_votes: number | null;
  dull_votes: number | null;
  my_vote: string | null;
}

interface LinkItem {
  id: number;
  name: string;
  url: string;
  title: string | null;
  status: string;
  admin_name: string | null;
  email: string | null;
  reason: string | null;
}

type MgmtTab = "news" | "fun" | "links";

export function ContentManage({ initialTab }: { initialTab?: MgmtTab }) {
  const { dict } = useI18n();
  const [tab, setTab] = useState<MgmtTab>(initialTab ?? "news");
  const [news, setNews] = useState<AdminNewsRow[]>([]);
  const [fun, setFun] = useState<FunItem[]>([]);
  const [links, setLinks] = useState<LinkItem[]>([]);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      // 0505：改拉后台专用列表（含未到点定时待发；/home 只出已到点，
      // 此前定时公告发布后后台不可见不可改）
      const rows = await api.get<AdminNewsRow[]>("/api/v1/admin/news");
      setNews(rows ?? []);
      setFun(await api.get<FunItem[]>("/api/v1/fun/items?status=all"));
      setLinks(await api.get<LinkItem[]>("/api/v1/admin/links"));
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.loadFailed);
    }
  }, [dict]);
  useEffect(() => {
    load();
  }, [load]);

  const TABS: [MgmtTab, string][] = [
    ["news", dict.cmgmt.tabNews],
    ["fun", dict.cmgmt.tabFun],
    ["links", dict.cmgmt.tabLinks],
  ];

  return (
    <div className="flex flex-col gap-3">
      <MgmtTabs tab={tab} setTab={setTab} tabs={TABS} />
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}

      {/* 公告管理 */}
      {tab === "news" && <NewsTab news={news} load={load} />}

      {/* 趣味盒管理 */}
      {tab === "fun" && <FunTab fun={fun} load={load} t={dict.cmgmt} />}

      {/* 友情链接管理 */}
      {tab === "links" && <LinksTab links={links} load={load} t={dict.cmgmt} />}
    </div>
  );
}
