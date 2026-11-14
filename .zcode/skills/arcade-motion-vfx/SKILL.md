---
name: arcade-motion-vfx
description: 娱乐屋动效与庆祝特效的实施口径。用于加赢局庆祝、滚动/错峰动画、反馈微动效，以及给既有玩法动效做体检。合并自 animation-systems 与 create-game-vfx 方法论，按本仓门禁改写。
---

# 娱乐屋动效与庆祝

动效只为三件事服务：**解释因果、确认动作、奖励时刻**。三者都不沾的动效删掉。

## 庆祝分级（核心口径）

普通赢局也要有庆祝——只有 jackpot 有彩带会让日常赢局显得寒酸。

| 档 | 触发 | 形态 | 时长 |
|---|---|---|---|
| 轻 | lose/tie | 无特效，飘字即止 | — |
| 中 | 普通赢 | 星屑小迸发（WinBurst），粒子数 ~10、尺寸减半 | ~1.2s |
| 全 | jackpot | 26 片彩带（既有 JackpotBurst） | 2.4s |

粒子上限、确定性伪随机（避免每次重渲形状跳变）、`key` 变化即重播、
播完即卸载（cleanup 幂等，reset/连点不泄漏）。

## 动效原语

- 只动 `transform` 与 `opacity`，不碰 width/height/top/left
- 入场 ease-out（120–320ms），离场更快 ease-in；娱乐屋可用轻弹性，
  严肃页不弹
- 错峰（stagger）40–90ms/件：一键全开类批量动作必须错峰，逐格悬念感
  来自延迟而不是随机
- 纯视觉的状态类（掷骰滚动）挂 boolean prop 驱动 CSS class，不在 JS 里
  逐帧驱动

## 本仓门禁对策（最重要的一节）

- `arcade-sweet.css` 已 93% 满（2783/3000），**一行不加**；`arcade.css`
  也不要再长——新动效全部进 `arcade-fx.css`（新文件，layout 加 import）
- 九个游戏 `_inner.tsx` 被行数基线钉死（316–467 行 > 300 上限）：
  只做**净零行改写**（如 `<DieFace n={d} />` → `<DieFace n={d} roll={busy} />`），
  能不动就不动；逻辑落共享件（game-kit-feedback / win-burst）
- 新组件先数行数再收笔：组件 ≤ 90 行，改前先查
  `scripts/line_limit_baseline.json` 有没有该文件的条目

## reduced-motion 与核验

`prefers-reduced-motion`：粒子/彩带降级为一次性透明度闪现或干脆不播，
飘字照旧（文字反馈永不缺失）。核验：连点重播、一键全开错峰、
移动端视口不溢出、DevTools 面板强制 reduced-motion 后全站无眩晕动效、
动静效后 console 无警告。
