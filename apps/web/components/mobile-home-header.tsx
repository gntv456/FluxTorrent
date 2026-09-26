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
  const h = new Date().getHours();
  const t = dict.mhome;
  const hello = greet(h, t);
  return (
    <div className="mhome-head md:hidden">
      <p className="mhome-head__hello">
        {hello}，<b>{brand}</b>
      </p>
      {me && (
        <div className="mhome-head__stats num">
          <span>
            <i>↑ {me.uploaded}</i>
          </span>
          <span>
            <i>↓ {me.downloaded}</i>
          </span>
          <span>
            <i>∑ {me.ratio}</i>
          </span>
          <span>
            <i>★ {me.spark}</i>
          </span>
        </div>
      )}
    </div>
  );
}
