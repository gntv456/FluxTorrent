"use client";

/**
 * 注册页·闸门组件集（0372 M5 前端）：
 *   RegisterCaptcha —— 算术验证码 / 第三方组件挂载点（原 register/page.tsx
 *     内联 JSX 的等价搬移，顺带把 300 行门禁压力从页面挪走）；
 *   RegisterQuiz    —— 站规答题（后端 /auth/quiz-challenge 一次性票据）；
 *   RegisterPow     —— 自托管工作量证明（captcha_provider=pow 时求解）。
 *
 * 三者都是「注册体里的一个凭据」，语义对齐后端
 * （quiz_ticket/quiz_answer、pow_token/pow_solution）。
 */

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { DRIVER_WIDGET_ID } from "@/lib/captcha-widget";
import { FormField } from "@/components/ui/form-field";
import { INPUT_LG } from "@/lib/ui-classes";

interface Captcha {
  captcha_id: string;
  question: string;
}

/** 算术验证码块（driver='none' 时渲染；第三方驱动渲染组件挂载点） */
export function RegisterCaptcha({
  driver,
  siteKey,
  captcha,
  answer,
  onAnswer,
  onRefresh,
  error,
}: {
  driver: string;
  siteKey: string;
  captcha: Captcha | null;
  answer: string;
  onAnswer: (v: string) => void;
  onRefresh: () => void;
  error?: string;
}) {
  const { dict } = useI18n();
  if (driver !== "none") {
    return siteKey ? (
      <div id={DRIVER_WIDGET_ID[driver]} className="min-h-[64px]" />
    ) : null;
  }
  return (
    <FormField
      name="captcha"
      label={dict.register.captcha}
      required
      error={error}
    >
      {/* a11y：aria-* 只许落在表单控件上——Lighthouse aria-allowed-attr
       * 拦过 wrapper div 带 aria-required 的形态（axe：非控件元素不得
       * 持有控件态）。input 从 FormField 继承关联，wrapper 是纯布局。 */}
      <div className="flex items-center gap-2">
        <span
          className="num min-h-[44px] flex items-center
            rounded-[var(--r-sm)] border border-line bg-cloud px-3
            font-bold"
        >
          {captcha?.question ?? "…"}
        </span>
        <input
          type="text"
          inputMode="numeric"
          value={answer}
          onChange={(e) => onAnswer(e.target.value.replace(/\D/g, ""))}
          required
          placeholder={dict.register.captchaAnswer}
          className={`${INPUT_LG} min-w-0 flex-1`}
        />
        <button
          type="button"
          onClick={onRefresh}
          aria-label={dict.register.captchaRefresh}
          className="min-h-[44px] shrink-0 rounded-full border border-line
            px-3 text-sm text-sub active:scale-[0.97]"
        >
          ↻
        </button>
      </div>
    </FormField>
  );
}

interface QuizData {
  ticket: string;
  question: string;
  options: string[];
  ttl_minutes: number;
}

/** 站规答题块：自取题、自持有票据；选择即上报 (ticket, 下标)。 */
export function RegisterQuiz({
  hard,
  onChange,
  error,
}: {
  /** true = 答错即拒（reg_quiz_pass_required），文案区别于「只记录」 */
  hard: boolean;
  onChange: (ticket: string, answer: number) => void;
  error?: string;
}) {
  const { dict } = useI18n();
  const [data, setData] = useState<QuizData | null>(null);
  const [pick, setPick] = useState<number | null>(null);
  const [failed, setFailed] = useState(false);

  const draw = useCallback(() => {
    setData(null);
    setPick(null);
    setFailed(false);
    api
      .get<QuizData>("/api/v1/auth/quiz-challenge")
      .then(setData)
      .catch(() => setFailed(true));
  }, []);

  useEffect(() => {
    draw();
  }, [draw]);

  const t = dict.register;
  return (
    <FormField
      name="quiz"
      label={t.quizTitle}
      required={hard}
      error={error}
      hint={hard ? t.quizHintHard : t.quizHintSoft}
    >
      <div className="flex flex-col gap-2">
        {failed ? (
          <p className="text-sm text-danger">{t.quizFailed}</p>
        ) : (
          <p className="text-sm font-bold">{data?.question ?? "…"}</p>
        )}
        <ul className="flex flex-col gap-1">
          {(data?.options ?? []).map((o, i) => (
            <li key={`${i}-${o.slice(0, 12)}`}>
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="radio"
                  name="quiz-option"
                  checked={pick === i}
                  onChange={() => {
                    if (!data) return;
                    setPick(i);
                    onChange(data.ticket, i);
                  }}
                />
                {o}
              </label>
            </li>
          ))}
        </ul>
        <button
          type="button"
          onClick={draw}
          className="self-start text-xs text-sub underline"
        >
          {t.quizRefresh}
        </button>
      </div>
    </FormField>
  );
}

interface PowData {
  pow_token: string;
  pow_challenge: string;
  pow_difficulty: number;
}

/** SHA-256("<challenge>:<solution>") 的 hex（与后端 pow_hash 同口径）。 */
async function powHash(challenge: string, solution: string) {
  const buf = new TextEncoder().encode(`${challenge}:${solution}`);
  const d = await crypto.subtle.digest("SHA-256", buf);
  return Array.from(new Uint8Array(d))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

/** 前 bits 个 bit 是否全 0（与后端 meets_difficulty 同口径）。 */
function meets(hex: string, bits: number) {
  const full = Math.floor(bits / 4);
  for (let i = 0; i < full; i++) if (hex[i] !== "0") return false;
  const rem = bits % 4;
  if (rem === 0) return true;
  const v = parseInt(hex[full], 16);
  return Number.isFinite(v) && v <= 0xf >> rem;
}

/** 自托管 PoW 块：取挑战 → 浏览器求解（crypto.subtle）→ 上报凭据。 */
export function RegisterPow({
  onSolved,
  error,
}: {
  onSolved: (token: string, solution: string) => void;
  error?: string;
}) {
  const { dict } = useI18n();
  const t = dict.register;
  const [state, setState] = useState<"idle" | "solving" | "done" | "fail">(
    "idle",
  );

  useEffect(() => {
    let stop = false;
    void (async () => {
      setState("solving");
      try {
        const c = await api.get<PowData>(
          "/api/v1/auth/pow-challenge?scope=register",
        );
        for (let n = 0; n < 20_000_000; n++) {
          if (stop) return;
          const sol = n.toString();
          // 每 512 次让出一次事件循环，避免长任务卡住首屏
          if (n % 512 === 0) await new Promise((r) => setTimeout(r, 0));
          if (meets(await powHash(c.pow_challenge, sol), c.pow_difficulty)) {
            setState("done");
            onSolved(c.pow_token, sol);
            return;
          }
        }
        setState("fail");
      } catch {
        setState("fail");
      }
    })();
    return () => {
      stop = true;
    };
  }, [onSolved]);

  return (
    <FormField name="pow" label={t.powTitle} required error={error}>
      <p className="text-sm text-sub">
        {state === "solving"
          ? t.powSolving
          : state === "done"
            ? t.powDone
            : t.powFailed}
      </p>
    </FormField>
  );
}
