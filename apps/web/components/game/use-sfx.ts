"use client";

import { useEffect, useState } from "react";
import {
  playSfx,
  sfxEnabled,
  setSfxEnabled,
  type SfxCue,
} from "@/lib/sfx";

/**
 * 音效开关 hook（GameShell 头部开关钮用）。
 * SSR 首帧固定 false，挂载后读 localStorage 真值——避免水合不匹配。
 */
export function useSfxToggle() {
  const [on, setOn] = useState(false);

  useEffect(() => {
    setOn(sfxEnabled());
  }, []);

  const toggle = () => {
    const next = !on;
    setOn(next);
    setSfxEnabled(next); // 打开时正是在手势里解锁 AudioContext
    if (next) playSfx("tick"); // 开通即给一声确认，用户立刻听到效果
  };

  return { on, toggle };
}

/** 播放一条线索的便捷转发（组件里 `const { play } = useSfx()`） */
export function useSfx() {
  return { play: (cue: SfxCue) => playSfx(cue) };
}
