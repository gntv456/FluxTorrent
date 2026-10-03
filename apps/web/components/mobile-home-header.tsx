"use client";

import { useI18n } from "@/i18n/client";

/** 首页移动问候条 + 数据条（M3，方案 §5）：<md 显示。
 *  问候条：品牌 + 时段问候；数据条：上传/下载/分享率/魔力 横向四格。
 *  数据来自 /home 的 site_data（总口径）+ /me/overview（个人口径优先）——
 *  由调用方 RSC 注入，组件只做展示。 */

function greet(h: number, t: { m: string; a: string; e: string; n: string }) {
  if (h < 6) return t.n;
  if (h < 12) return t.m;
  if (h < 18) return t.a;
  return t.e;
}

/** 站点时区（UTC+8）下的当前小时。
 *
 * ZT2（2026-10-03）修水合不匹配：此前直接 `new Date().getHours()` —— web 容器跑
 * UTC（本地 01:07 时容器是 17:07 → 渲染「下午好」），浏览器跑 UTC+8（渲染「夜深了」），
 * SSR 与首次客户端渲染文本不一致 → React 每次进首页报 #418（text mismatch）并在
 * 首屏闪一下。改成两端都按**站点时区**取小时，结果一致，且对站内用户更符合预期。
 * 取不到（ICU 异常）时返回 -1，由调用方回落到中性问候，绝不渲染 NaN。 */
function siteHour(): number {
  try {
    const s = new Intl.DateTimeFormat("en-GB", {
      timeZone: "Asia/Shanghai",
      hour: "2-digit",
      hour12: false,
    }).format(new Date());
    const h = Number(s) % 24;
    return Number.isFinite(h) ? h : -1;
  } catch {
    return -1;
  }
}

export function MobileHomeHeader({
  brand,
  me,
}: {
  brand: string;
  me: {
    uploaded: string;
    downloaded: string;
    ratio: string;
    spark: string;
  } | null;
}) {
  const { dict } = useI18n();
  const t = dict.mhome;
  const h = siteHour();
  // -1（ICU 异常）→ 中性问候，避免 SSR/CSR 分歧又避免出现 NaN
  const hello = h < 0 ? t.a : greet(h, t);
  return (
    <div className="mhome-head md:hidden">
      <p className="mhome-head__hello">
        {hello}，<b>{brand}</b>
      </p>
      {me && (
        /* 2026-10-03：此前四个数据格只有 ↑/↓/∑/★ 符号没有文字，
           新用户无法判断 ∑ 是分享率、★ 是魔力余额。补标签。 */
        <dl className="mhome-head__stats num">
          <div>
            <dt>{t.up}</dt>
            <dd>
              <i>↑ {me.uploaded}</i>
            </dd>
          </div>
          <div>
            <dt>{t.down}</dt>
            <dd>
              <i>↓ {me.downloaded}</i>
            </dd>
          </div>
          <div>
            <dt>{t.ratio}</dt>
            <dd>
              <i>∑ {me.ratio}</i>
            </dd>
          </div>
          <div>
            <dt>{t.spark}</dt>
            <dd>
              <i>★ {me.spark}</i>
            </dd>
          </div>
        </dl>
      )}
    </div>
  );
}
