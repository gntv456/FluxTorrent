import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

import {
  sfxEnabled,
  setSfxEnabled,
  playSfx,
} from "@/lib/sfx";
import { useSfxToggle } from "@/components/game/use-sfx";
import { GameShell } from "@/components/game/game-kit";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 娱乐屋音效系统（手感批 2026-11-13）。
 * 口径：默认静音（localStorage.arcade_sfx 缺省 off）；开关落偏好持久化；
 * 静音时 playSfx 不触碰 AudioContext（jsdom 无实现，碰到即炸，正好当断言）。
 */

function Harness() {
  const { on, toggle } = useSfxToggle();
  return (
    <button type="button" onClick={toggle} aria-pressed={on}>
      sfx-{on ? "on" : "off"}
    </button>
  );
}

function shell() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <GameShell
        title={zhCN.games.bigsmall.title}
        icon="🎲"
        balance={1000}
        todayNet={0}
        limitLeft={10}
        stage={<p>stage</p>}
      />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  window.localStorage.clear();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("sfx preference", () => {
  it("默认静音：未设置过偏好时 sfxEnabled() 为 false", () => {
    expect(sfxEnabled()).toBe(false);
  });

  it("setSfxEnabled(true) 落 localStorage，重读为开", () => {
    setSfxEnabled(true);
    expect(window.localStorage.getItem("arcade_sfx")).toBe("on");
    expect(sfxEnabled()).toBe(true);
    setSfxEnabled(false);
    expect(sfxEnabled()).toBe(false);
  });

  it("静音态 playSfx 不触碰 AudioContext（jsdom 无实现即断言）", () => {
    // 不 stub AudioContext：若 playSfx 在静音态仍尝试建上下文会直接抛
    expect(() => playSfx("win")).not.toThrow();
  });

  it("GameShell 头部开关钮：点击后偏好持久化且 aria-pressed 翻转", () => {
    // jsdom 无 AudioContext：stub 一个最小壳供手势内 resume 调用
    const resume = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal(
      "AudioContext",
      class {
        state = "suspended";
        currentTime = 0;
        resume = resume;
        createOscillator() {
          return {
            type: "",
            frequency: { value: 0 },
            connect: vi.fn(() => ({ connect: vi.fn() })),
            start: vi.fn(),
            stop: vi.fn(),
            onended: null,
          };
        }
        createGain() {
          return {
            gain: {
              setValueAtTime: vi.fn(),
              exponentialRampToValueAtTime: vi.fn(),
            },
            connect: vi.fn(),
          };
        }
        destination = {};
      } as unknown as typeof AudioContext,
    );
    shell();
    const btn = screen.getByRole("button", {
      name: `${zhCN.games.sfx.label}：${zhCN.games.sfx.off}`,
    });
    expect(btn.getAttribute("aria-pressed")).toBe("false");
    fireEvent.click(btn);
    expect(btn.getAttribute("aria-pressed")).toBe("true");
    expect(window.localStorage.getItem("arcade_sfx")).toBe("on");
    expect(resume).toHaveBeenCalled();
  });
});

describe("sfx toggle hook", () => {
  it("toggle 翻转并持久化", () => {
    const resume = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal(
      "AudioContext",
      class {
        state = "suspended";
        currentTime = 0;
        resume = resume;
      } as unknown as typeof AudioContext,
    );
    render(<Harness />);
    const btn = screen.getByRole("button");
    expect(screen.getByText("sfx-off")).toBeTruthy();
    fireEvent.click(btn);
    expect(screen.getByText("sfx-on")).toBeTruthy();
    expect(window.localStorage.getItem("arcade_sfx")).toBe("on");
  });
});
