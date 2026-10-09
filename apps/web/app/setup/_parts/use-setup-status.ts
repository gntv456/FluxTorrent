"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import type { Status } from "./setup-types";

/**
 * 向导状态加载（从 setup/page.tsx 按域拆出，300 行门禁）。
 *
 * 为什么不能是一次性 fetch：api 进程启动时先跑全部 sqlx 迁移才 bind
 * 端口（main.rs 迁移在 HttpServer::bind 之前），全新装机 / 重建卷之后
 * 这段空窗可达数十秒甚至数分钟。此时 web 已 healthy、/setup 页面能渲染，
 * 但 GET /api/v1/setup/status 必然连不上。旧实现 catch 里直接写死
 * 「无法加载向导状态，请确认 API 可达」——既不重试也不给真实原因，
 * 站长在冷启动窗口打开向导就进不去，只能手动刷页面。
 *
 * 这里补两件事：
 *  1) 失败后退避重试（1→2→4→8→15→30s，上限 12 次约 3 分钟），
 *     覆盖迁移空窗，重试期间页面保持可用；
 *  2) 错误保真——ApiError 的业务码 / HTTP 状态 / 网关 502 全部透出。
 *     1003 是 web→api 网关不通，与浏览器侧网络故障的处置完全不同，
 *     压成一句话等于把诊断线索销毁。
 */

/** 退避序列（秒）：末项 30s 持续到上限，覆盖长迁移空窗。 */
const BACKOFF_S = [1, 2, 4, 8, 15, 30];
/** 探测次数上限（含首次）。超过后转为纯手动重试。 */
const MAX_ATTEMPTS = 12;

export interface SetupStatusState {
  status: Status | null;
  /** 主文案（t("loadFailed")） */
  error: string;
  /** 技术细节：业务码 / HTTP 状态 / 原始异常消息 */
  detail: string;
  /** 处置建议（按错误分类给出） */
  hint: string;
  /** 是否处于自动重试等待中 */
  probing: boolean;
  /** 已探测次数（1 = 首次） */
  attempt: number;

  /** 手动重试（重置退避） */
  reload: () => void;
}

/** 错误分类：把 ApiError 归一为「细节 + 建议键」 */
function classify(
  e: unknown,
): { detail: string; hintKey: string } {
  if (e instanceof ApiError) {
    if (e.code === 1003) {
      return {
        detail: `web → api 网关不通（code 1003）：${e.message}`,
        hintKey: "hintGateway",
      };
    }
    if (e.code === 1000) {
      return {
        detail: `api 返回非 JSON 响应（code 1000）：${e.message}`,
        hintKey: "hintHttp",
      };
    }
    return {
      detail: `api 业务错误 code ${e.code}：${e.message}`,
      hintKey: "",
    };
  }
  // fetch 抛出的 TypeError：DNS / 连接拒绝 / CSP 拦截都落在这里
  const msg = e instanceof Error ? e.message : String(e);
  return { detail: `浏览器请求失败：${msg}`, hintKey: "hintNetwork" };
}

export function useSetupStatus(
  t: (k: string, fallback: string) => string,
): SetupStatusState {
  // t 每次渲染都是新函数（page.tsx 里就地定义），进依赖会无限重跑
  const tRef = useRef(t);
  tRef.current = t;
  const [status, setStatus] = useState<Status | null>(null);
  const [error, setError] = useState("");
  const [detail, setDetail] = useState("");
  const [hint, setHint] = useState("");
  const [probing, setProbing] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const alive = useRef(true);

  const probe = useCallback((n: number) => {
    if (!alive.current) return;
    if (timer.current) clearTimeout(timer.current);
    // 计数对外 1 基（n 是 0 基的尝试序号）
    setAttempt(n + 1);
    setProbing(n > 0);
    api
      .get<Status>("/api/v1/setup/status")
      .then((s) => {
        if (!alive.current) return;
        setStatus(s);
        setError("");
        setDetail("");
        setHint("");
        setProbing(false);
      })
      .catch((e: unknown) => {
        if (!alive.current) return;
        const c = classify(e);
        const tr = tRef.current;
        setError(tr("loadFailed", "无法加载向导状态，请确认 API 可达"));
        setDetail(c.detail);
        setHint(c.hintKey ? tr(c.hintKey, "") : "");
        const next = n + 1;
        // n+1 即本次（含）总探测数，达到上限就不再排下一次
        if (next >= MAX_ATTEMPTS) {
          setProbing(false);
          return;
        }
        const wait = BACKOFF_S[Math.min(n, BACKOFF_S.length - 1)];
        timer.current = setTimeout(() => probe(next), wait * 1000);
      });
  }, []);

  useEffect(() => {
    alive.current = true;
    probe(0);
    return () => {
      alive.current = false;
      if (timer.current) clearTimeout(timer.current);
    };
  }, [probe]);

  const reload = useCallback(() => probe(0), [probe]);

  return { status, error, detail, hint, probing, attempt, reload };
}
