"use client";

import { useEffect, useState } from "react";
import { api, setSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { formatBytes, formatRatio } from "@/lib/format";
import { UserTools } from "@/components/user-tools";

/** /me 返回口径（http.rs me handler）+ spark_balance */
interface MeInfo {
  username: string;
  class_id?: number;
  class_name?: string | null;
  uploaded: number;
  downloaded: number;
  seeding: number;
  leeching: number;
}

/**
 * 参考站用户信息条（userbar）：头像 + 欢迎回来 {用户} 退出 + 魔力胶囊
 * + 快捷入口（控制面板/收藏/勋章/任务/邀请）+ 票券统计条
 * （分享率/上传/下载/做种/下载中）。未登录回落为登录链接。
 * 服务端 Header 无法读 localStorage 的 JWT，状态由客户端补齐。
 */
export function UserBox({ loginLabel }: { loginLabel: string }) {
  const { dict, currency } = useI18n();
  const [me, setMe] = useState<MeInfo | null>(null);
  const [spark, setSpark] = useState<number | null>(null);

  useEffect(() => {
    if (!localStorage.getItem("flux.token")) return;
    api
      .get<MeInfo & { spark_balance?: number }>("/api/v1/me")
      .then((r) => {
        setMe(r);
        setSpark(r.spark_balance ?? null);
        // 魔力值单独取（economy /me/spark），失败不打扰
        return api
          .get<{ balance: number }>("/api/v1/me/spark")
          .then((b) => setSpark(b.balance))
          .catch(() => {});
      })
      .catch(() => {
        // token 失效：清掉本地凭证，回落到登录链接
        localStorage.removeItem("flux.token");
        localStorage.removeItem("flux.user");
        setSessionCookie(null);
      });
  }, []);

  async function logout() {
    try {
      await api.post("/api/v1/auth/logout", {});
    } catch {
      // 后端撤销失败也照常清理本地凭证
    }
    localStorage.removeItem("flux.token");
    localStorage.removeItem("flux.user");
    setSessionCookie(null);
    location.href = "/login";
  }

  if (!me) {
    return (
      <div className="userbar__identity">
        <a href="/login" className="mainmenu-link">
          {loginLabel}
        </a>
      </div>
    );
  }

  const ratio = formatRatio(me.uploaded, me.downloaded);
  return (
    <>
      <div className="userbar__identity">
        {/* 装饰性头像（用户名链接已在旁侧）：aria-hidden 时须移出 tab 序列 */}
        <a href="/my" aria-hidden tabIndex={-1} className="userbar__avatar" title={me.username}>
          {me.username.slice(0, 1).toUpperCase()}
        </a>
        <div className="min-w-0">
          <div className="userbar__welcome">
            <span>{dict.my.welcomeBack}</span>
            <a href="/my" className="userbar__name">
              {me.username}
            </a>
            {me.class_name && <span className="sticker">{me.class_name}</span>}
            <button type="button" onClick={logout} className="text-xs text-sub hover:text-sky">
              {dict.my.logout}
            </button>
          </div>
          <div className="userbar__meta">
            <span className="bonus-pill">
              <span>{currency}：</span>
              <b className="num">{spark !== null ? Number(spark).toLocaleString() : "…"}</b>
              <a href="/my" className="bonus-hint">
                [{dict.my.dailyCheckin}]
              </a>
            </span>
            <nav className="userbar__shortcuts" aria-label={dict.my.center}>
              <a href="/my">{dict.my.center}</a>
              <a href="/my?tab=bookmarks">{dict.my.bookmarksCount}</a>
              <a href="/medals">{dict.nav.medals}</a>
              <a href="/tasks">{dict.nav.tasks}</a>
              <a href="/invites">{dict.nav.invites}</a>
              <a href="/jixiao">{dict.nav.jixiao}</a>
              {me.class_id !== undefined && me.class_id >= 90 && (
                  <a href="/admin" className="font-bold text-[var(--baozi-orange-dark)]">
                    {dict.admin.panelTitle}
                  </a>
                )}
            </nav>
          </div>
        </div>
      </div>
      <div className="userbar__stats">
        <div className="userstat">
          <span>{dict.my.ratio}</span>
          <strong className="num">{ratio}</strong>
        </div>
        <div className="userstat">
          <span>{dict.my.uploaded}</span>
          <strong className="num seed-arrow">{formatBytes(me.uploaded)}</strong>
        </div>
        <div className="userstat">
          <span>{dict.my.downloaded}</span>
          <strong className="num leech-arrow">{formatBytes(me.downloaded)}</strong>
        </div>
        <div className="userstat">
          <span>{dict.my.seedingCount}</span>
          <strong className="num">{me.seeding}</strong>
        </div>
        <div className="userstat">
          <span>{dict.my.leechingLabel}</span>
          <strong className="num">{me.leeching}</strong>
        </div>
      </div>
      <UserTools />
    </>
  );
}
