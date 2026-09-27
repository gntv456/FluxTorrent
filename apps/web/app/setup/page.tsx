"use client";

import { useEffect, useState } from "react";
import { api, ApiError, setSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { SetupStepFinish } from "./_parts/step-finish";
import { SetupStepSite, type SiteDraft } from "./_parts/step-site";
import type { Status } from "./_parts/setup-types";

/**
 * 安装向导（U3 §8.3）：三步 —— ① 选站型 ② 站名+管理员登录 ③ 合规勾选完成。
 * 消费 GET /api/v1/setup/status 与 POST /api/v1/setup（后端已就绪）。
 * setup_done 已置位时显示已完成态（可重复访问，POST 幂等）。
 * 第二/三步拆至 ./_parts/step-site.tsx、step-finish.tsx（300 行门禁）。
 */

export default function SetupWizard() {
  const { dict } = useI18n();
  // setup 域文案（三语字典 setup 键；缺键回落中文兜底——向导页面在 i18n 域注册前可用）
  const setupDict = (
    dict as unknown as Record<string, Record<string, string> | undefined>
  ).setup;
  const t = (k: string, fallback: string) => setupDict?.[k] ?? fallback;

  const [status, setStatus] = useState<Status | null>(null);
  const [step, setStep] = useState(1);
  const [pack, setPack] = useState("");
  // 站点信息草稿（G12）：第二步由 step-site 采集，含临时密码就地改密后
  // 的新口令；第三步 finish 用它重登（改密会撤销全部旧 token）
  const [draft, setDraft] = useState<SiteDraft>({
    siteName: "",
    username: "",
    password: "",
    announceUrl: "",
  });
  const [ack, setAck] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [result, setResult] = useState<string>("");
  // 首个邀请码（P0-2.2）：向导完成时后端自动发的一枚，展示 + 一键复制
  const [firstInvite, setFirstInvite] = useState<string | null>(null);

  useEffect(() => {
    api
      .get<Status>("/api/v1/setup/status")
      .then((s) => {
        setStatus(s);
        if (s.done) setStep(3);
      })
      .catch(() =>
        setError(t("loadFailed", "无法加载向导状态，请确认 API 可达")),
      );
  }, []);

  async function finish() {
    setBusy(true);
    setError("");
    try {
      // 需先登录拿 token（向导完成端点要求 SITEPACKS_MANAGE）：登录响应
      // Set-Cookie 下发 HttpOnly flux_token，浏览器随后的同源请求自动携带
      await api.post("/api/v1/auth/login", {
        username: draft.username,
        password: draft.password,
      });
      setSessionCookie(true);
      const res = await api.post<{
        purged: [string, number][];
        extras: [string, number][];
        first_invite?: string | null;
      }>("/api/v1/setup", {
        pack,
        site_name: draft.siteName,
        announce_url: draft.announceUrl,
        games_compliance_ack: ack,
      });
      const purged = res.purged?.map(([k, n]) => `${k}:${n}`).join(" ") ?? "";
      setResult(t("done", "安装完成") + (purged ? `（清理 ${purged}）` : ""));
      setFirstInvite(res.first_invite ?? null);
      setStatus((s) => (s ? { ...s, done: true } : s));
    } catch (e) {
      setError(e instanceof ApiError ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

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
              step === n
                ? "bg-[var(--accent)] text-[var(--accent-contrast)]"
                : "bg-[var(--panel)] text-muted"
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
        <div className="mt-6">
          <div className="grid gap-3 sm:grid-cols-2">
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
                <div className="mt-1 text-xs text-muted">
                  {p.description}
                </div>
                <div className="mt-2 font-mono text-[10px] text-muted">
                  {p.code}
                </div>
              </button>
            ))}
          </div>
          {/* G12：站型可显式跳过——产品定位是「通用建站」，站长可能要从零搭；
              也修掉「包列表为空时卡在第一步」的死路 */}
          <div className="mt-4 text-center">
            <button
              className="rounded-md border border-line px-4 py-2 text-sm
                text-muted transition-colors hover:border-accent"
              onClick={() => {
                setPack("");
                setStep(2);
              }}
            >
              {t("skipPack", "跳过：暂不预设站型，用空站起步")}
            </button>
            <p className="mt-2 text-xs text-muted">
              {t(
                "skipPackHint",
                "跳过 = 不应用任何站型包（分类 / 模块保持系统默认）；之后可在后台「站点设定」页的「内容包」里随时导入。",
              )}
            </p>
          </div>
        </div>
      )}

      {step === 2 && (
        <SetupStepSite
          t={t}
          btn={btn}
          status={status}
          initial={draft}
          onBack={() => setStep(1)}
          onNext={(d) => {
            setDraft(d);
            setStep(3);
          }}
        />
      )}

      {step === 3 && (
        <SetupStepFinish
          t={t}
          btn={btn}
          status={status}
          ack={ack}
          setAck={setAck}
          result={result}
          firstInvite={firstInvite}
          busy={busy}
          onFinish={finish}
          onPrev={() => setStep(2)}
        />
      )}

    </div>
  );
}
