"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { useIsCompact } from "@/lib/hooks/use-media";
import { HomeNewsModal } from "@/components/home-news-modal";
import { HomeNewsPanel } from "@/components/home-news-panel";
import { ShoutBox } from "@/components/shout-box";
import { FunBox } from "@/components/fun-box";
import { BottomSheet } from "@/components/bottom-sheet";
import { AttendanceSheet, AttendanceStrip } from "@/components/home-attendance";
import { HomeCockpit, type CockpitMe } from "@/components/home-cockpit";
import { OnboardingCard } from "@/components/onboarding-card";
import { SiteDataCard } from "@/components/home-site-data";
import { ResourceStatsPanel } from "@/components/home-resource-stats";
import { LuckyDrawPanel, LinksPanel } from "@/components/home-panels";
import { MobileHomeHeader } from "@/components/mobile-home-header";
import { MobileNewsTop } from "@/components/mobile-home-news-top";
import { MobileQuickGrid } from "@/components/mobile-home-quick-grid";
import { effectiveSpan, parseHomeLayout } from "@/components/home-layout";
import type { HomeData } from "@/components/home-data";

/** 首页板块（复刻参考站 index.php）：新鲜事 · 签到 · 资源统计 · 站点数据 · 转盘 · 友链。
 *  拆出（300 门禁）：排版解析 home-layout.tsx、数据契约 home-data.ts、签到三件套
 *  home-attendance*.tsx、站点数据、资源统计、转盘/友链、公告面板。
 *  2026-11-18：桌面签到改紧凑带 + 日历抽屉；新增驾驶舱与新手引导（均在排版前）。
 *  手机端单一 DOM + CSS order 重排；桌面板块窄屏由 CSS 隐去，水合不分支。 */

export function HomeSections({
  latest,
  initialData = null,
  initialMods = {},
  me = null,
  brand = "",
}: {
  /** 海报墙节点：RSC 取好数据，位置/占宽由排版清单决定 */
  latest?: React.ReactNode;
  /** 首页数据（SSR 直出）：RSC 预取避免"加载中"壳；客户端仍刷新交互态 */
  initialData?: HomeData | null;
  /** 模块开关（SSR 直出）：同样由 RSC 预取，避免首屏板块可见性抖动 */
  initialMods?: Record<string, boolean>;
  /** 个人数据（/me/overview 摘要，由 RSC 注入；驾驶舱多读 username/seeding/hr） */
  me?: CockpitMe | null;
  /** 品牌名（问候语用） */
  brand?: string;
} = {}) {
  const { dict, currency } = useI18n();
  const t = dict.home2;
  const compact = useIsCompact();
  // 模块开关：按 games/attendance/shoutbox 过滤，缺键视为开；SSR 初值由 RSC 传入。
  const [mods, setMods] = useState<Record<string, boolean>>(initialMods);
  useEffect(() => {
    if (Object.keys(initialMods).length > 0) return;
    fetch("/api/v1/site-profile")
      .then((r) => r.json())
      .then((b: { data?: { modules?: Record<string, boolean> } }) =>
        setMods(b?.data?.modules ?? {}),
      )
      .catch(() => setMods({}));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const mod = (k: string) => mods[k] !== false;
  // SSR：初始值来自 RSC 预取的 HomeData，首屏直出真内容
  const [data, setData] = useState<HomeData | null>(initialData);
  const [err, setErr] = useState(false);
  const [modal, setModal] = useState<number | null>(null);
  const [checkinBusy, setCheckinBusy] = useState(false);
  const [checkinMsg, setCheckinMsg] = useState<string | null>(null);
  const [chatOpen, setChatOpen] = useState(false);
  const [calOpen, setCalOpen] = useState(false);
  const [onboardingSignal, setOnboardingSignal] = useState(0);

  const load = () => {
    api
      .get<HomeData>("/api/v1/home")
      .then(setData)
      // 已有 SSR 直出的数据时，后台刷新失败不应把首屏内容替换成错误页
      .catch(() => setErr((prev) => (data === null ? true : prev)));
  };
  useEffect(load, []);

  async function checkin() {
    if (checkinBusy) return;
    setCheckinBusy(true);
    try {
      const r = await api.post<{ reward: number; streak: number }>(
        "/api/v1/attendance/checkin",
      );
      setCheckinMsg(
        dict.my.checkinOk
          .replace("{reward}", String(r.reward))
          .replace("{streak}", String(r.streak))
          .replace("{magic}", currency),
      );
      // 余额同步不在这里做：所有写口过 api-client 的统一广播（见 lib/spark-event）
      // 乐观更新：弱网下 load() 挂起不能让按钮回弹成「未签到」误报。
      const today = new Date(Date.now() + 8 * 3600 * 1000)
        .toISOString()
        .slice(0, 10);
      setData((prev) => {
        if (!prev) return prev;
        const a = prev.attendance as
          | (HomeData["attendance"] & { off?: boolean })
          | undefined;
        if (!a || a.off || a.checked_today) return prev;
        return {
          ...prev,
          attendance: {
            ...a,
            checked_today: true,
            streak: r.streak,
            total_days: (a.total_days ?? 0) + 1,
            calendar: (a.calendar ?? []).map((d) =>
              d.date === today ? { ...d, done: true, reward: r.reward } : d,
            ),
          },
        };
      });
      // 后台对账刷新（失败不打扰——乐观态已在，下次进页自然校正）
      load();
    } catch {
      setCheckinMsg(t.checkinFail);
    } finally {
      setCheckinBusy(false);
    }
  }

  if (err) return <p className="text-sm text-sub">{dict.common.loadFailed}</p>;
  if (!data) return <p className="text-sm text-sub">{dict.my.loading}</p>;

  // 模块关闭的板块从排版中剔除（含站长自定义 home_layout 里显式排进的）
  const layout = parseHomeLayout(
    data.home_layout,
    data.home_sections ?? [],
  ).filter((i) => {
    if (i.key === "attendance") return mod("attendance");
    if (i.key === "shoutbox") return mod("shoutbox");
    if (i.key === "funbox" || i.key === "lucky_draw") return mod("games");
    return true;
  });
  // 闭包内 TS 判窄失效：固化非空引用供 renderSection 使用
  const home = data;

  /** 单板块渲染（0089 排版）：key 与后端 HOME_SECTION_KEYS 一一对应 */
  function renderSection(key: string): React.ReactNode {
    switch (key) {
      case "news":
        return (
          <HomeNewsPanel
            items={home.news}
            onOpen={setModal}
            brand={brand}
          />
        );
      case "attendance":
        // 桌面与手机共用同一条紧凑签到带；整月日历统一收进 AttendanceSheet
        return (
          <div id="attendance" key="attendance-anchor">
            <AttendanceStrip
              home={home}
              t={t}
              checkin={checkin}
              checkinBusy={checkinBusy}
              onOpenCalendar={() => setCalOpen(true)}
              variant="desktop"
              calendarLabel={dict.uxOpt.viewCalendar}
            />
            {checkinMsg ? <p className="attendance-msg">{checkinMsg}</p> : null}
          </div>
        );
      case "shoutbox":
        // 手机端这块由 CSS 隐去（内容进抽屉），窄屏停轮询
        return <ShoutBox active={!compact} />;
      case "funbox":
        return <FunBox embedded />;
      case "resource_stats":
        return <ResourceStatsPanel series={home.resource_stats} t={t} />;
      case "site_data":
        return <SiteDataCard home={home} t={t} dict={dict} />;
      case "lucky_draw":
        return (
          <LuckyDrawPanel
            items={home.lucky_draw}
            title={t.luckyTitle}
            goDraw={t.goDraw}
            got={t.got}
            sparkLabel={dict.common.spark.replace("{magic}", currency)}
            empty={t.luckyEmpty}
          />
        );
      case "links":
        return (
          <LinksPanel
            links={home.friend_links}
            disclaimerTitle={t.disclaimerTitle}
            disclaimerBody={t.disclaimerBody}
            linksTitle={t.linksTitle}
            applyLink={t.applyLink}
          />
        );
      case "latest":
        // 海报墙：内容由首页 RSC 取好传入，这里只决定它在排版中的位置与占宽
        return latest ?? null;
      default:
        return null;
    }
  }

  const isCustom = home.home_layout?.trim() ? true : false;
  return (
    <div
      className={
        isCustom
          ? "home-stack home-stack--custom home-stack--mobile"
          : "home-stack home-stack--mobile"
      }
    >
      {/* ==== 手机端首屏构件（桌面由 md:hidden 隐去；次序靠 CSS order） ==== */}
      <MobileHomeHeader
        brand={brand}
        me={me}
        streak={home.attendance?.streak}
        totalDays={home.attendance?.total_days}
        threshold={home.ratio_watch_threshold ?? null}
      />
      <MobileNewsTop news={home.news} onOpen={setModal} />
      <AttendanceStrip
        home={home}
        t={t}
        checkin={checkin}
        checkinBusy={checkinBusy}
        onOpenCalendar={() => setCalOpen(true)}
      />
      <MobileQuickGrid onChat={() => setChatOpen(true)} />

      {/* ==== 个人驾驶舱 + 新手引导：恒在排版板块之前 ==== */}
      <HomeCockpit
        me={me}
        home={home}
        threshold={home.ratio_watch_threshold}
        currency={currency}
        brand={brand}
        checkin={checkin}
        checkinBusy={checkinBusy}
        checkinMsg={checkinMsg}
        onOpenCalendar={() => setCalOpen(true)}
        onReopenOnboarding={() => setOnboardingSignal((n) => n + 1)}
      />
      <OnboardingCard me={me} openSignal={onboardingSignal} />

      {/* ==== 排版驱动（0089）：按站长配置顺序/宽度渲染；空配置 = 默认布局 ==== */}
      {layout.map((item) => {
        const node = renderSection(item.key);
        if (!node) return null;
        return (
          <div
            key={item.key}
            data-sec={item.key}
            className={`home-cell home-cell--span-${effectiveSpan(
              item,
              home.home_sections ?? [],
            )}`}
          >
            {node}
          </div>
        );
      })}

      {/* ==== 手机端弹层：签到日历（带万年历） / 聊天室 ==== */}
      <AttendanceSheet
        open={calOpen}
        onClose={() => setCalOpen(false)}
        home={home}
        t={t}
        checkin={checkin}
        checkinBusy={checkinBusy}
        checkinMsg={checkinMsg}
      />
      <BottomSheet
        open={chatOpen}
        onClose={() => setChatOpen(false)}
        title={dict.shoutbox.title}
      >
        <ShoutBox />
      </BottomSheet>

      {/* ==== 公告弹窗（原生 dialog + showModal 收在子组件里）==== */}
      {modal !== null && (
        <HomeNewsModal
          item={
            data.news.find((n) => n.id === modal) ?? { title: "", body: "" }
          }
          onClose={() => setModal(null)}
        />
      )}
    </div>
  );
}
