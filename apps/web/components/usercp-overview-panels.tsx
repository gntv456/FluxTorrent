"use client";

/**
 * 账户概览子面板（从 components/usercp-overview.tsx 按域拆出，295 行门禁）：
 * SummaryRow 摘要行（等级进度/最近登录/邀请/勋章/登录活动）
 * + ActivityPanel 当前活动。明细两张表在 usercp-overview-details.tsx。
 */

import Link from "next/link";
import { useI18n } from "@/i18n/client";
import type { LevelProgress } from "@/lib/usercp-level";
import type { Overview } from "@/components/usercp";

/** 概览子面板共用的字典入参（details 面板同引此定义） */
export interface DictProp {
  dict: ReturnType<typeof useI18n>["dict"];
}

/** 摘要行：等级进度 / 最近登录 / 邀请配额 / 成就勋章 / 登录活动 */
export function SummaryRow({
  ov,
  t,
  dict,
  level,
}: {
  ov: Overview;
  t: Record<string, string>;
  level: LevelProgress | null;
} & DictProp) {
  return (
    // 2026-10-17 对齐样图集 § overview-grid：原先这 5 张摘要卡被一个
    // `.uc-summary-row` 容器（auto-fit minmax(190px)）包着，宽屏下挤成
    // 一整排 5 列，与样图的规整 2 列网格不符。去掉容器、加
    // `uc-summary-card`，让它们成为 .usercp-dashboard 的直接子项。
    <>
      <section className="uc-summary-card uc-level-card">
        <h3>{t.levelTitle}</h3>
        <strong>{ov.class_name ?? "—"}</strong>
        <div>
          <i style={{ width: `${level?.pct ?? 100}%` }} />
        </div>
        {/* P-516：旧版这里写「做种积分 700 / 50.0G」——分子是「在做种数×100」
            的假积分，分母却是下一等级的上传量门槛（字节），两个量纲相除。
            现在显示四项权威条件里达标率最低的那一项（其余三项再快也不升职）。 */}
        <p>
          {level ? (
            <>
              {level.label} {level.text}，{t.nextClass}{" "}
              {ov.next_class?.name} {t.still}
              {level.gap}
            </>
          ) : (
            dict.userProfile2.maxLevel
          )}
        </p>
      </section>
      <section className="uc-summary-card">
        <h3>{t.lastLoginTitle}</h3>
        <strong>
          {ov.last_login
            ? ov.last_login.replace("T", " ").slice(0, 19)
            : "—"}
        </strong>
        <p>Web</p>
        <Link href="/my?tab=security">{t.checkSecurity} ›</Link>
      </section>
      <section className="uc-summary-card">
        <h3>{t.inviteTitle}</h3>
        <strong>
          {ov.invites_pending} {t.inviteUnit}
        </strong>
        <p>
          {t.invited} {ov.invites_pending + ov.invites_used} ·{" "}
          {t.registered}
          {ov.invites_used}
        </p>
        <Link href="/invites">{t.manageInvites} ›</Link>
      </section>
      <section className="uc-summary-card">
        <h3>{t.medalTitle}</h3>
        <strong>{ov.medals} {t.medalUnit}</strong>
        <p>{ov.medals > 0 ? "" : t.noMedal}</p>
        <Link href="/medals">{t.viewMedals} ›</Link>
      </section>
      <section className="uc-summary-card">
        <h3>{t.loginActivityTitle}</h3>
        <strong>{ov.login_total_30d} {t.times}</strong>
        <p>
          {t.last30d} · {ov.totp_enabled ? t.twofaOn : t.twofaOff} ·{" "}
          {dict.usercp.security.passkeyLabel} {ov.passkey_masked}••••
        </p>
        <Link href="/my?tab=security">{t.accountSecurity} ›</Link>
      </section>
    </>
  );
}

/** 当前活动（样图 § ucp 概览第二张面板：做种 / 下载 / 累计发布）。
 *  uploads 此前只在 Overview 类型里躺着，全仓零渲染。 */
export function ActivityPanel({
  ov,
  dict,
}: {
  ov: Overview;
} & DictProp) {
  const u = dict.usercp;
  return (
    <section className="uc-summary-card">
      <h3>{u.activityTitle}</h3>
      <strong className="num">{ov.seeding}</strong>
      <p>
        {u.actLeeching} {ov.leeching} · {u.actUploads} {ov.uploads}
      </p>
      <Link href="/my/torrentlist">{dict.mytl.title} ›</Link>
    </section>
  );
}
