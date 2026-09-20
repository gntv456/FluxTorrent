"use client";

import { useEffect, useState } from "react";
import { api, ApiError, setSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/**
 * 安装向导（U3 §8.3）：三步 —— ① 选站型 ② 站名+管理员登录 ③ 合规勾选完成。
 * 消费 GET /api/v1/setup/status 与 POST /api/v1/setup（后端已就绪）。
 * setup_done 已置位时显示已完成态（可重复访问，POST 幂等）。
 */

interface Pack {
  code: string;
  name: string;
  description: string;
}
interface Status {
  done: boolean;
  has_admin: boolean;
  packs: Pack[];
}

export default function SetupWizard() {
  const { dict } = useI18n();
  // setup 域文案（三语字典 setup 键；缺键回落中文兜底——向导页面在 i18n 域注册前可用）
  const setupDict = (dict as unknown as Record<string, Record<string, string> | undefined>).setup;
  const t = (k: string, fallback: string) => setupDict?.[k] ?? fallback;

  const [status, setStatus] = useState<Status | null>(null);
  const [step, setStep] = useState(1);
  const [pack, setPack] = useState("");
  const [siteName, setSiteName] = useState("");
  const [ack, setAck] = useState(false);
  // 管理员登录（后端 POST /setup 需 SITEPACKS_MANAGE 权限）
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [result, setResult] = useState<string>("");

  useEffect(() => {
    api
      .get<Status>("/api/v1/setup/status")
      .then((s) => {
        setStatus(s);
        if (s.done) setStep(3);
      })
      .catch(() => setError(t("loadFailed", "无法加载向导状态，请确认 API 可达")));
  }, []);

  async function finish() {
    setBusy(true);
    setError("");
    try {
      // 需先登录拿 token（向导完成端点要求 SITEPACKS_MANAGE）；
      // api-client 的 token 从 localStorage 读——登录后立即写入再调完成端点
      const login = await api.post<{ token: string }>("/api/v1/auth/login", {
        username,
        password,
      });
      setSessionCookie(true);
      const res = await api.post<{ purged: [string, number][]; extras: [string, number][] }>(
        "/api/v1/setup",
        { pack, site_name: siteName, games_compliance_ack: ack },
      );
      const purged = res.purged?.map(([k, n]) => `${k}:${n}`).join(" ") ?? "";
      setResult(t("done", "安装完成") + (purged ? `（清理 ${purged}）` : ""));
      setStatus((s) => (s ? { ...s, done: true } : s));
    } catch (e) {
      setError(e instanceof ApiError ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  const inputCls =
    "w-full rounded-md border border-line bg-[var(--field-bg)] px-3 py-2 text-sm outline-none focus:border-accent";
  const btn =
    "rounded-md px-4 py-2 text-sm font-medium transition-colors disabled:opacity-50 bg-[var(--accent)] text-[var(--accent-contrast)]";

  if (!status && !error) return null;

  return (
    <div className="mx-auto max-w-2xl px-4 py-12">
      <h1 className="text-xl font-semibold">🚀 {t("title", "站点安装向导")}</h1>
      <p className="mt-2 text-sm text-muted">
        {t("subtitle", "三步完成建站：选择站型 → 站点信息 → 合规确认")}
      </p>

      {/* 步骤条 */}
      <ol className="mt-6 flex items-center gap-2 text-xs">
        {[1, 2, 3].map((n) => (
          <li
            key={n}
            className={`flex items-center gap-1 rounded-full px-3 py-1 ${
              step === n ? "bg-[var(--accent)] text-[var(--accent-contrast)]" : "bg-[var(--panel)] text-muted"
            }`}
          >
            {n}. {t(`step${n}`, ["选择站型", "站点信息", "合规确认"][n - 1])}
          </li>
        ))}
      </ol>

      {error && (
        <p className="mt-4 rounded-md border border-danger/40 bg-danger/10 px-3 py-2 text-sm text-danger">
          {error}
        </p>
      )}

      {step === 1 && status && (
        <div className="mt-6 grid gap-3 sm:grid-cols-2">
          {status.packs.map((p) => (
            <button
              key={p.code}
              onClick={() => {
                setPack(p.code);
                setStep(2);
              }}
              className={`rounded-lg border p-4 text-left transition-colors hover:border-accent ${
                pack === p.code ? "border-accent" : "border-line"
              }`}
            >
              <div className="font-medium">{p.name}</div>
              <div className="mt-1 text-xs text-muted">{p.description}</div>
              <div className="mt-2 font-mono text-[10px] text-muted">{p.code}</div>
            </button>
          ))}
        </div>
      )}

      {step === 2 && (
        <div className="mt-6 space-y-4">
          <label className="block text-sm">
            <span className="text-muted">{t("siteName", "站点名称")}</span>
            <input
              className={`mt-1 ${inputCls}`}
              value={siteName}
              onChange={(e) => setSiteName(e.target.value)}
              placeholder="例：枫叶 PT"
            />
          </label>
          {!status?.has_admin && (
            <p className="rounded-md border border-warn/40 bg-warn/10 px-3 py-2 text-xs">
              {t("noAdmin", "检测到尚无管理员账号：请先用引导 SQL 建立 root（class 99）后再完成向导，或直接在下方登录已有管理员。")}
            </p>
          )}
          <label className="block text-sm">
            <span className="text-muted">{t("adminUser", "管理员用户名")}</span>
            <input className={`mt-1 ${inputCls}`} value={username} onChange={(e) => setUsername(e.target.value)} autoComplete="username" />
          </label>
          <label className="block text-sm">
            <span className="text-muted">{t("adminPass", "管理员密码")}</span>
            <input type="password" className={`mt-1 ${inputCls}`} value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" />
          </label>
          <div className="flex gap-2">
            <button className={`${btn}`} onClick={() => setStep(1)}>
              {t("prev", "上一步")}
            </button>
            <button className={btn} disabled={!username || !password} onClick={() => setStep(3)}>
              {t("next", "下一步")}
            </button>
          </div>
        </div>
      )}

      {step === 3 && (
        <div className="mt-6 space-y-4">
          {status?.done && !result && (
            <p className="rounded-md border border-success/40 bg-success/10 px-3 py-2 text-sm">
              {t("alreadyDone", "本站已完成安装向导（重复完成幂等）。")}
            </p>
          )}
          <label className="flex items-start gap-2 rounded-md border border-line p-3 text-sm">
            <input
              type="checkbox"
              className="mt-0.5"
              checked={ack}
              onChange={(e) => setAck(e.target.checked)}
              disabled={status?.done}
            />
            <span>
              {t(
                "ackText",
                "我确认已了解：本系统内置的娱乐玩法（刮刮乐/猜大小等机会类游戏）使用站内虚拟货币，启用前我已自行确认并遵守所在司法域关于机会类游戏的法律法规。",
              )}
            </span>
          </label>
          {result && (
            <p className="rounded-md border border-success/40 bg-success/10 px-3 py-2 text-sm">✅ {result}</p>
          )}
          <div className="flex gap-2">
            <button className={btn} onClick={() => setStep(2)} disabled={status?.done}>
              {t("prev", "上一步")}
            </button>
            <button className={btn} disabled={!ack || busy || status?.done} onClick={finish}>
              {busy ? t("working", "执行中…") : t("finish", "完成安装")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
