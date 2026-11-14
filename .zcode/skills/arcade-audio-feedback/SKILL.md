---
name: arcade-audio-feedback
description: 娱乐屋/游戏音效反馈的设计与实施口径。用于给玩法页加声音线索、Web Audio 合成音效、静音开关、移动端解锁与可访问性核验。零音频资产是硬约束。
---

# 娱乐屋音效反馈

用声音确认玩家意图与结果，不做常噪背景音。本站娱乐屋**零音频资产**是铁口径：
所有线索用 Web Audio 振荡器合成，不引 `.mp3/.wav`，不装音频库。

## 线索映射（克制优先）

| 事件 | 线索 | 口径 |
|---|---|---|
| 下注/选注确认 | `tick` | 短促方波点击，确认意图 |
| 揭开/开箱 | `pop` | 短正弦泡音 |
| 普通赢 | `win` | 上行双音，轻快不吵 |
| 大奖 | `jackpot` | 四音号角，唯一的隆重时刻 |
| 输 | `lose` | 低柔闷响，短且轻，无惩罚感 |

养成玩法（农场/宠物）日常动作**不加音**——动作高频，加了就是常噪。

## 浏览器实现铁律

- **默认静音**（`localStorage.arcade_sfx` 缺省 off）：办公/公共场合友好；开关在
  GameShell 头部，偏好持久化
- AudioContext 惰性创建，必须在用户手势里 `resume()`
- voice 上限 ~6：超出的播放请求直接丢弃，不给音频栈堆积
- 节点 `onended` 即断引用；oscillator/gain 用完即弃，防泄漏
- 声音只确认已有视觉反馈的事件，不承载独占信息——静音时功能零缺失

## 核验

首次交互（解锁）、连点（voice 上限）、静音态、`prefers-reduced-motion`
（音效不属 motion，但开关仍须可触达）、移动端 Safari（须手势内 resume）。
一局游玩后看 console 无 AudioContext 警告、无节点泄漏。

## 本仓门禁对策

- 新代码进新文件（`lib/sfx.ts` + `components/game/use-sfx.ts`），不往被行数
  基线钉死的 `_inner.tsx` 里加行
- 接线走共享件（`game-kit-feedback.tsx` 的 ResultFlash 已覆盖六款下注玩法）
- 开关文案走 `dict.games.sfx.*` 三语键，**ja 不加**（ja 是段级回落，
  加半截 games 段会吞掉 zh-CN 整段回落键）
