"use client";
import { useEffect, useRef, useState } from "react";
import Link from "next/link";
import { api, setSessionCookie, hasSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import {
  formatBytes,
  formatRatio,
  avatarFrameStyle,
  FrameImageOverlay,
} from "@/lib/format";
import { UserTools } from "@/components/user-tools";
import { UserCheckin } from "@/components/user-checkin";
import { SparkChip } from "@/components/spark-chip";
import { useSparkSync } from "@/lib/spark-event";
import { MedalIcon } from "@/components/medal-icon";
import { isStaffClass } from "@/lib/domain/user-class";

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

// SSE unread 事件帧体（/me/messages/stream data 段）
type SseUnread = { count?: number };
/**
 * 头像弹窗（同类站 同类站布局口径，0147）：头像按钮弹宽下拉——摘要区
 * （大头像+用户名+勋章+等级+{magic}+退出）/ 数据条 / 快捷链接 / 信箱
 * 工具行（UserTools）。未登录回落登录链接；点外部/ESC 收起。
 */
export function UserMenu({ loginLabel }: { loginLabel: string }) {
  const { dict, currency } = useI18n();
  const [me, setMe] = useState<MeInfo | null>(null);
  const [meFailed, setMeFailed] = useState(false);
  // 模块开关（G2-7）：快捷链接按模块过滤（medals/tasks/jixiao）
  const [mods, setMods] = useState<Record<string, boolean>>({});
  // 顶栏魔力 chip 开关（W-31 / IA-2 ①）：site_settings.topbar_spark_chip，
  // 后端归一为布尔（缺省 true）；取档失败回落 true（非关键路径）
  const [chipOn, setChipOn] = useState(true);
  useEffect(() => {
    fetch("/api/v1/site-profile")
      .then((r) => r.json())
      .then(
        (b: {
          data?: {
            modules?: Record<string, boolean>;
            topbar_spark_chip?: boolean;
          };
        }) => {
          setMods(b?.data?.modules ?? {});
          setChipOn(b?.data?.topbar_spark_chip !== false);
        },
      )
      .catch(() => setMods({}));
  }, []);  const mod = (k: string) => mods[k] !== false;
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
        api
          .get<{ balance: number }>("/api/v1/me/spark")
          .then((b) => setSpark(b.balance))
          .catch(() => {});
      })
      .catch(() => {
        // P-611：取数失败不等于会话失效（2001 已由 api-client 统一跳登录）。
        // 旧写法在这里清本地凭证，网络抖动一次就把已登录用户演成游客。
        setMeFailed(true);
      });
  }, []);
  // 未读 SSE：连上即推+15s 轮询变化才推；onerror 静默重连；会话失效
  // close 防僵尸连接（SseUnread 谓词与 /me unread_messages 同源）
  useEffect(() => {
    if (!hasSessionCookie()) return;
    const es = new EventSource("/api/v1/me/messages/stream");
    es.addEventListener("unread", (ev) => {
      try {
        const n = (JSON.parse((ev as MessageEvent).data) as SseUnread).count;
        if (n === undefined) return;
        setMe((m) => (m ? { ...m, unread_messages: n } : m));
      } catch {
        /* 帧体异常：忽略本帧 */
      }
    });
    es.onerror = () => {
      if (!hasSessionCookie()) es.close();
    };
    return () => es.close();
  }, []);
  useEffect(() => {
    if (spark !== null) setSparkText(Number(spark).toLocaleString());
  }, [spark]);
  // 签到回调刷新（chip 与弹窗摘要共用同一份 sparkText）
  const refreshSpark = () =>
    api
      .get<{ balance: number }>("/api/v1/me/spark")
      .then((b) => setSpark(b.balance))
      .catch(() => {});
  useSparkSync(refreshSpark);

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
    // 后端撤销失败也照常清理本地凭证
    await api.post("/api/v1/auth/logout", {}).catch(() => {});
    localStorage.removeItem("flux.user");
    // 三轮遗留：登出清 SW 页面缓存（含登录态 RSC 快照，防共用设备离线回看）
    navigator.serviceWorker?.controller?.postMessage("CLEAR_PAGES");
    setSessionCookie(false);
    location.href = "/login";
  }

  if (!me && meFailed && hasSessionCookie()) return null;
  if (!me)
    return (
      <a href="/login" className="mainmenu-link usermenu-login">
        {loginLabel}
      </a>
    );

  const ratio = formatRatio(me.uploaded, me.downloaded);
  const t = dict.my;
  // 头像两处渲染（触发钮/摘要区）共用：框图优先否则 CSS 描边；relative
  // 供 FrameImageOverlay 定位。
  const avatar = (cls: string) => (
    <span
      className={`relative ${cls}`}
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
  );
  return (
    <div className="usermenu" ref={rootRef}>
      {/* IA-2 ①（W-31）：头像旁魔力 chip 常显余额，点击进 /my-spark；
          开关 site_settings.topbar_spark_chip（默认开） */}
      {chipOn && <SparkChip balance={sparkText} />}
      <button
        type="button"
        className="usermenu__trigger"
        aria-expanded={open}
        aria-haspopup="true"
        aria-label={me.username}
        onClick={() => setOpen((v) => !v)}
      >
        {avatar("usermenu__avatar")}
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
              className="usermenu__avatar-lg"
              title={me.username}
            >
              {avatar("usermenu__avatar-lg")}
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
                <UserCheckin onDone={refreshSpark} />
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
          {/* 数据条五卡同构 → 数组驱动（渲染口径单一源） */}
          <div className="usermenu__stats">
            {[
              { href: "/my", k: t.ratio, v: ratio },
              {
                href: "/my",
                k: t.uploaded,
                v: formatBytes(me.uploaded),
                cls: "seed-arrow",
              },
              {
                href: "/my",
                k: t.downloaded,
                v: formatBytes(me.downloaded),
                cls: "leech-arrow",
              },
              { href: "/my/torrentlist", k: t.seedingCount, v: me.seeding },
              { href: "/my/torrentlist", k: t.leechingLabel, v: me.leeching },
            ].map((s) => (
              <a key={s.k} href={s.href} className="userstat">
                <span>{s.k}</span>
                <strong className={`num ${s.cls ?? ""}`}>{s.v}</strong>
              </a>
            ))}
          </div>

          {/* ── 快捷链接行（一行收纳高频入口） ── */}
          <nav className="usermenu__links" aria-label={t.center}>
            <a href="/notifications">{dict.notifications.title}</a>
            <a href="/my">{t.center}</a>
            <a href="/me/subscriptions?tab=bookmarks">{t.bookmarksCount}</a>
            <a href="/my/torrentlist">{dict.mytl.title}</a>
            {mod("medals") && <a href="/medals">{dict.nav.medals}</a>}
            {mod("tasks") && <a href="/tasks">{dict.nav.tasks}</a>}
            {mod("invites") && <a href="/invites">{dict.nav.invites}</a>}
            {mod("jixiao") && <a href="/jixiao">{dict.nav.jixiao}</a>}
            {isStaffClass(me.class_id) && (
              <Link
                href="/admin"
                className="usermenu__staff font-bold"
              >
                {dict.admin.panelTitle}
              </Link>
            )}
          </nav>
          {/* ── 信箱工具行（收件箱未读角标/发件箱/作弊者/举报/管理组/社交/RSS） ── */}
          <div className="usermenu__tools">
            <UserTools unread={me.unread_messages ?? 0} mods={mods} />
          </div>
        </div>
      )}
    </div>
  );
}
