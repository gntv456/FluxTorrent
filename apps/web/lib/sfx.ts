/**
 * 娱乐屋音效引擎（零音频资产）：全部线索用 Web Audio 振荡器合成，
 * 不引 .mp3/.wav、不装音频库 —— 口径见 .zcode/skills/arcade-audio-feedback。
 *
 * 铁律：
 * - 默认静音（localStorage.arcade_sfx 缺省 "off"），开关在 GameShell 头部
 * - AudioContext 惰性创建；播放须在用户手势之后（浏览器自动解锁策略）
 * - voice 上限：并发的短音超出上限直接丢弃，不给音频栈堆积
 * - 节点 onended 后即弃，防泄漏
 */

/** 线索集：只确认意图与结果，不做常噪（养成玩法日常动作刻意无音） */
export type SfxCue =
  | "tick" // 选注/下注确认
  | "pop" // 揭开涂层/开箱
  | "win" // 普通赢（上行双音）
  | "jackpot" // 大奖（四音号角）
  | "lose"; // 输（低柔闷响，短且轻）

const STORAGE_KEY = "arcade_sfx";
/** 并发 voice 上限：短音场景 6 足够，超出即丢弃 */
const VOICE_CAP = 6;

type Ctx = AudioContext & { __ftVoices?: number };

let ctx: Ctx | null = null;

function ensureCtx(): Ctx | null {
  if (typeof window === "undefined") return null;
  const AC =
    window.AudioContext ??
    (window as unknown as { webkitAudioContext?: typeof AudioContext })
      .webkitAudioContext;
  if (!AC) return null;
  if (!ctx) ctx = new AC() as Ctx;
  return ctx;
}

/** 读用户偏好：缺省 off（默认静音口径） */
export function sfxEnabled(): boolean {
  try {
    return window.localStorage.getItem(STORAGE_KEY) === "on";
  } catch {
    return false;
  }
}

/** 写偏好；on 时顺带在当前手势里解锁 AudioContext */
export function setSfxEnabled(on: boolean): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, on ? "on" : "off");
  } catch {
    /* 隐私模式等 localStorage 不可用：静默降级为本次会话内不记忆 */
  }
  if (on) {
    const c = ensureCtx();
    // resume 必须发生在用户手势调用栈里，这里正是手势（点开关）
    void c?.resume().catch(() => {});
  }
}

interface Note {
  /** 频率 Hz */
  f: number;
  /** 起始时刻（相对线索起点，秒） */
  t: number;
  /** 时长（秒） */
  d: number;
  /** 音量 0-1 */
  v: number;
  /** 波形 */
  w?: OscillatorType;
}

/** 线索谱面：克制优先——输的音最短最轻，jackpot 是唯一隆重时刻 */
const CUES: Record<SfxCue, Note[]> = {
  tick: [{ f: 880, t: 0, d: 0.05, v: 0.25, w: "square" }],
  pop: [{ f: 520, t: 0, d: 0.09, v: 0.3, w: "sine" }],
  win: [
    { f: 523, t: 0, d: 0.12, v: 0.3, w: "triangle" }, // C5
    { f: 784, t: 0.1, d: 0.18, v: 0.3, w: "triangle" }, // G5
  ],
  jackpot: [
    { f: 523, t: 0, d: 0.14, v: 0.32, w: "triangle" }, // C5
    { f: 659, t: 0.12, d: 0.14, v: 0.32, w: "triangle" }, // E5
    { f: 784, t: 0.24, d: 0.14, v: 0.32, w: "triangle" }, // G5
    { f: 1046, t: 0.36, d: 0.3, v: 0.34, w: "triangle" }, // C6
  ],
  lose: [{ f: 196, t: 0, d: 0.16, v: 0.18, w: "sine" }], // G3 低柔
};

/** 播一条线索。静音/无手势解锁/超 voice 上限/无 AudioContext 时安静返回。 */
export function playSfx(cue: SfxCue): void {
  if (!sfxEnabled()) return;
  const c = ensureCtx();
  if (!c) return;
  if (c.state === "suspended") {
    // 非手势栈场景（如 setTimeout 回调）尝试唤醒；resume 是异步的，
    // 本条线索放弃（下一个手势触发的线索自然就绪），不排队
    void c.resume().catch(() => {});
    return;
  }
  const voices = c.__ftVoices ?? 0;
  if (voices >= VOICE_CAP) return;

  const now = c.currentTime;
  for (const n of CUES[cue]) {
    const osc = c.createOscillator();
    const gain = c.createGain();
    osc.type = n.w ?? "sine";
    osc.frequency.value = n.f;
    const t0 = now + n.t;
    // 快 attack / 指数 release，避免咔哒声
    gain.gain.setValueAtTime(0.0001, t0);
    gain.gain.exponentialRampToValueAtTime(n.v, t0 + 0.012);
    gain.gain.exponentialRampToValueAtTime(0.0001, t0 + n.d);
    osc.connect(gain).connect(c.destination);
    osc.start(t0);
    osc.stop(t0 + n.d + 0.02);
    c.__ftVoices = voices + 1;
    osc.onended = () => {
      c.__ftVoices = Math.max(0, (c.__ftVoices ?? 1) - 1);
    };
  }
}
