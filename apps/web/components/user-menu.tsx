"use client";

import { useEffect, useRef, useState } from "react";
import { api, setSessionCookie, hasSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import {
  formatBytes,
  formatRatio,
  avatarFrameStyle,
  FrameImageOverlay,
} from "@/lib/format";
import { UserTools } from "@/components/user-tools";
import { MedalIcon } from "@/components/medal-icon";

/** /me 返回口径（http.rs me handler）+ spark_balance */
interface MeInfo {
  id: number;
  username: string;
  class_id?: number;
  class_name?: string | null;
  uploaded: number;
  downloaded: number;
  seeding: number;
  leeching: number;
  unread_messages?: number;
  avatar_url?: string | null;
  avatar_frame_css?: string | null;
  avatar_frame_image?: string | null;
  worn_medals?: { name: string; asset_ref?: string | null }[];
}

/**
 * 头像弹窗（好学站 CuteTop 口径，0147）：导航条右侧头像按钮，点击弹出
 * 宽下拉——摘要区（大头像+用户名+勋章+等级+{magic}+退出）/ 数据条（分享率、
 * 上传、下载、做种、下载中）/ 快捷链接行（控制面板/收藏/种子清单/勋章/任务/
 * 邀请/签到）/ 信箱工具行（UserTools）。未登录回落登录链接。
 * 点外部/ESC 收起；替代原 userbar 常驻条。
 */
export function UserMenu({ loginLabel }: { loginLabel: string }) {
  const { dict, currency } = useI18n();
  const [me, setMe] = useState<MeInfo | null>(null);
  const [spark, setSpark] = useState<number | null>(null);
  const [sparkText, setSparkText] = useState<string>("…");
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!hasSessionCookie()) return;
    api
      .get<MeInfo & { spark_balance?: number }>("/api/v1/me")
      .then((r) => {
        setMe(r);
        setSpark(r.spark_balance ?? null);
        return api
          .get<{ balance: number }>("/api/v1/me/spark")
          .then((b) => setSpark(b.balance))
          .catch(() => {});
      })
      .catch(() => {
        localStorage.removeItem("flux.user");
        setSessionCookie(false);
      });
  }, []);

  useEffect(() => {
    if (spark !== null) setSparkText(Number(spark).toLocaleString());
  }, [spark]);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node))
        setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  async function logout() {
    try {
      await api.post("/api/v1/auth/logout", {});
    } catch {
      // 后端撤销失败也照常清理本地凭证
    }
    localStorage.removeItem("flux.user");
    setSessionCookie(false);
    location.href = "/login";
  }

  if (!me) {
    return (
      <a href="/login" className="mainmenu-link usermenu-login">
        {loginLabel}
      </a>
    );
  }

  const ratio = formatRatio(me.uploaded, me.downloaded);
  const t = dict.my;
  return (
    <div className="usermenu" ref={rootRef}>
      <button
        type="button"
        className="usermenu__trigger"
        aria-expanded={open}
        aria-haspopup="true"
        aria-label={me.username}
        onClick={() => setOpen((v) => !v)}
      >
        <span
          className="usermenu__avatar relative"
          style={
            me.avatar_frame_image
              ? undefined
              : avatarFrameStyle(me.avatar_frame_css)
          }
        >
          {me.avatar_url ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img
              src={me.avatar_url}
              alt=""
              className="h-full w-full rounded-full object-cover"
            />
          ) : (
            me.username.slice(0, 1).toUpperCase()
          )}
          <FrameImageOverlay url={me.avatar_frame_image} />
        </span>
        <span className="usermenu__caret" aria-hidden="true">
          ▾
        </span>
        {(me.unread_messages ?? 0) > 0 && (
          <span className="usermenu__dot num" aria-hidden="true">
            {me.unread_messages! > 99 ? "99+" : me.unread_messages}
          </span>
        )}
      </button>

      {open && (
        <div className="usermenu__drop">
          {/* ── 摘要区：大头像 + 用户名/勋章/等级 + {magic} + 退出 ── */}
          <div className="usermenu__summary">
            <a
              href={`/users/${me.id}`}
              aria-hidden
              tabIndex={-1}
              className="usermenu__avatar-lg relative"
              style={
                me.avatar_frame_image
                  ? undefined
                  : avatarFrameStyle(me.avatar_frame_css)
              }
              title={me.username}
            >
              {me.avatar_url ? (
                // eslint-disable-next-line @next/next/no-img-element
                <img
                  src={me.avatar_url}
                  alt=""
                  className="h-full w-full rounded-full object-cover"
                />
              ) : (
                me.username.slice(0, 1).toUpperCase()
              )}
              <FrameImageOverlay url={me.avatar_frame_image} />
            </a>
            <div className="usermenu__summary-main">
              <div className="usermenu__name-row">
                <a href={`/users/${me.id}`} className="usermenu__name">
                  {me.username}
                </a>
                {(me.worn_medals ?? []).slice(0, 5).map((m) => (
                  <span key={m.name} className="medal-chip" title={m.name}>
                    <MedalIcon src={m.asset_ref} size={14} title={m.name} />
                  </span>
                ))}
                {me.class_name && (
                  <span className="sticker">{me.class_name}</span>
                )}
              </div>
              <div className="usermenu__subline">
                <a href="/my-spark" className="bonus-pill">
                  <span>{currency}：</span>
                  <b className="num">{sparkText}</b>
                </a>
                <a href="/my" className="usermenu__checkin">
                  [{t.dailyCheckin}]
                </a>
              </div>
            </div>
            <button
              type="button"
              onClick={logout}
              className="usermenu__logout"
            >
              {t.logout}
            </button>
          </div>

          {/* ── 数据条：分享率 / 上传 / 下载 / 做种 / 下载中 ── */}
          <div className="usermenu__stats">
            <a href="/my" className="userstat">
              <span>{t.ratio}</span>
              <strong className="num">{ratio}</strong>
            </a>
            <a href="/my" className="userstat">
              <span>{t.uploaded}</span>
              <strong className="num seed-arrow">
                {formatBytes(me.uploaded)}
              </strong>
            </a>
            <a href="/my" className="userstat">
              <span>{t.downloaded}</span>
              <strong className="num leech-arrow">
                {formatBytes(me.downloaded)}
              </strong>
            </a>
            <a href="/my/torrentlist" className="userstat">
              <span>{t.seedingCount}</span>
              <strong className="num">{me.seeding}</strong>
            </a>
            <a href="/my/torrentlist" className="userstat">
              <span>{t.leechingLabel}</span>
              <strong className="num">{me.leeching}</strong>
            </a>
          </div>

          {/* ── 快捷链接行（好学 CuteTop 口径：一行收纳高频入口） ── */}
          <nav className="usermenu__links" aria-label={t.center}>
            <a href="/my">{t.center}</a>
            <a href="/my?tab=bookmarks">{t.bookmarksCount}</a>
            <a href="/my/torrentlist">{dict.mytl.title}</a>
            <a href="/medals">{dict.nav.medals}</a>
            <a href="/tasks">{dict.nav.tasks}</a>
            <a href="/invites">{dict.nav.invites}</a>
            <a href="/jixiao">{dict.nav.jixiao}</a>
            {me.class_id !== undefined && me.class_id >= 90 && (
              <a
                href="/admin"
                className="font-bold text-[var(--baozi-orange-dark)]"
              >
                {dict.admin.panelTitle}
              </a>
            )}
          </nav>

          {/* ── 信箱工具行（收件箱未读角标/发件箱/作弊者/举报/管理组/社交/RSS） ── */}
          <div className="usermenu__tools">
            <UserTools unread={me.unread_messages ?? 0} />
          </div>
        </div>
      )}
    </div>
  );
}
