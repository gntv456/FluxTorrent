"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useI18n } from "@/i18n/client";
import type { HomeMeView } from "@/components/mobile-home-header";

/** 跳过状态只存本地（不新增 API / 不改用户设置）；异常环境静默降级为不记忆。 */
const SKIP_KEY = "flux-onboarding-skipped";

/** 新手 5 分钟上手卡：三步勾选 + 跳过引导。
 *  只在 me 非空时渲染；跳过后再也不自动出现，驾驶舱的入口可手动重开。
 *  SSR 安全：localStorage 只在挂载后的 effect 里读，首帧恒为隐藏态。 */
export function OnboardingCard({
  me,
  openSignal = 0,
}: {
  me: HomeMeView | null;
  openSignal?: number;
}) {
  const { dict } = useI18n();
  const [visible, setVisible] = useState(false);
  const [skipped, setSkipped] = useState(false);
  const [done, setDone] = useState<boolean[]>([false, false, false]);

  useEffect(() => {
    let wasSkipped = false;
    try {
      wasSkipped = window.localStorage.getItem(SKIP_KEY) === "1";
    } catch {
      /* 隐私模式：不记忆，本次会话仍可正常显示 */
    }
    setSkipped(wasSkipped);
    if (!wasSkipped) setVisible(true);
  }, []);

  // 驾驶舱入口：openSignal 每次自增都强制重开（即使此前跳过）
  useEffect(() => {
    if (openSignal <= 0) return;
    setVisible(true);
  }, [openSignal]);

  if (!me || !visible) return null;
  const u = dict.uxOpt;
  const steps: [string, string][] = [
    [u.onboardingStep1, u.onboardingStep1Desc],
    [u.onboardingStep2, u.onboardingStep2Desc],
    [u.onboardingStep3, u.onboardingStep3Desc],
  ];
  const skip = () => {
    try {
      window.localStorage.setItem(SKIP_KEY, "1");
    } catch {
      /* 写不进去也先收起，避免反复打断 */
    }
    setSkipped(true);
    setVisible(false);
  };

  return (
    <section className="ux-onboard" aria-labelledby="ux-onboard-title">
      <header className="ux-onboard__head">
        <h2 id="ux-onboard-title" className="ux-onboard__title">
          {u.onboardingTitle}
        </h2>
        <button type="button" className="ux-onboard__skip" onClick={skip}>
          {u.onboardingSkip}
        </button>
      </header>
      {skipped ? (
        <p className="ux-onboard__note">{u.onboardingSkipped}</p>
      ) : null}
      <ol className="ux-onboard__steps">
        {steps.map(([title, desc], i) => (
          <li
            key={title}
            className={
              done[i] ? "ux-onboard__step is-done" : "ux-onboard__step"
            }
          >
            <label className="ux-onboard__label">
              <input
                type="checkbox"
                checked={done[i]}
                onChange={() =>
                  setDone((prev) => prev.map((v, j) => (j === i ? !v : v)))
                }
              />
              <span className="ux-onboard__box" aria-hidden="true">
                {done[i] ? "✓" : ""}
              </span>
              <span className="ux-onboard__text">
                <b>{title}</b>
                <i>{desc}</i>
              </span>
            </label>
          </li>
        ))}
      </ol>
      <Link
        href="/help#survival"
        className="ux-onboard__why"
        data-ux-event="guide_open"
        data-ux-label="onboarding"
      >
        {u.guideRuleLink} ›
      </Link>
    </section>
  );
}
