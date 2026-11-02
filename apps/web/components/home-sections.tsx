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
import {
  AttendanceCard,
  AttendanceSheet,
  AttendanceStrip,
} from "@/components/home-attendance";
import { SiteDataCard } from "@/components/home-site-data";
import { ResourceStatsPanel } from "@/components/home-resource-stats";
import { LuckyDrawPanel, LinksPanel } from "@/components/home-panels";
import {
  MobileHomeHeader,
  type HomeMeView,
} from "@/components/mobile-home-header";
import { MobileNewsTop } from "@/components/mobile-home-news-top";
import { MobileQuickGrid } from "@/components/mobile-home-quick-grid";
import { effectiveSpan, parseHomeLayout } from "@/components/home-layout";
import type { HomeData } from "@/components/home-data";

/** 首页板块（复刻参考站 index.php）：
 *  社区新鲜事（头条+列表+公告弹窗）· 签到得魔力日历 · 新增资源统计（30 天堆叠柱状图）
 *  · 站点数据三列 · 幸运大转盘流水 · 免责条款 + 友情链接。
 *  拆出（300 门禁）：排版解析 home-layout.tsx、数据契约 home-data.ts、
 *  签到日历 home-attendance.tsx、站点数据 home-site-data.tsx、
 *  资源统计 home-resource-stats.tsx、补签弹层 home-resub-button.tsx、
 *  转盘/友链 home-panels.tsx、公告面板 home-news-panel.tsx。
 *
 *  2026-11-02 手机端重设计：单一 DOM + CSS order 重排（不用 SSR 分支，避免
 *  水合不一致）。桌面那几块（公告面板/签到卡/聊天室）在窄屏由 CSS 隐去，
 *  换成首屏的仪表盘 / 公告头条卡 / 签到条 / 快捷宫格；聊天与日历走抽屉。 */

export function HomeSections({
  latest,
  initialData = null,
  initialMods = {},
  me = null,
  brand = "",
}: {
  /** 海报墙节点：数据由首页 RSC 取（服务端渲染不退化），
   *  但**位置与占宽由排版清单决定**——四审 L6 前它没有渲染分支，
   *  站长的排序/span 对它完全无效，只在页面末尾硬渲染。 */
  latest?: React.ReactNode;
  /** 首页数据（SSR 直出）：由首页 RSC 预取并传入，首屏即渲染真内容，
   *  不再退化到"加载中…"壳（伤首屏与 SEO）。客户端 useEffect 仍会
   *  重新拉一次以保证签到/公告等交互后的新鲜度。 */
  initialData?: HomeData | null;
  /** 模块开关（SSR 直出）：同样由 RSC 预取，避免首屏板块可见性抖动 */
  initialMods?: Record<string, boolean>;
  /** 手机端仪表盘的个人数据（/me/overview 摘要，由 RSC 注入） */
  me?: HomeMeView | null;
  /** 品牌名（问候语用） */
  brand?: string;
} = {}) {
  const { dict, currency } = useI18n();
  const t = dict.home2;
  const compact = useIsCompact();
  // 模块开关（二审 G2-1 修复）：attendance/shoutbox/funbox/lucky_draw 板块
  // 按对应模块过滤——games/attendance/shoutbox 关闭的站首页不再出现对应板块。
  // 缺键视为开（与 requireModule/Header mod() 同口径）。
  // SSR：初始值由 RSC 传入；无初值（客户端独用时）再回落到 fetch。
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
        return <HomeNewsPanel items={home.news} onOpen={setModal} />;
      case "attendance":
        return (
          <div id="attendance" key="attendance-anchor">
            <AttendanceCard
              home={home}
              t={t}
              dict={dict}
              checkin={checkin}
              checkinBusy={checkinBusy}
              checkinMsg={checkinMsg}
            />
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
