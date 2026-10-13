"use client";

/** 登录 PoW（sha256-pow-v1）客户端：取题 → Web Crypto 循环 nonce → 交卷。
 *  从 app/login/login-form.tsx 拆出（300 行门禁）；口径与后端
 *  gaps_http/pow.rs 一致：SHA-256("<challenge>:<nonce>") 前导零 bit 数
 *  达标即解。18bit ≈ 平均 26 万次哈希，主线程分片防 UI 冻住。 */

import { api } from "@/lib/api-client";

/** 2012 错误信封 data 里的挑战四件套（后端 gaps_http/pow.rs） */
export interface PowChallenge {
  pow_token: string;
  pow_challenge: string;
  pow_difficulty: number;
  pow_expires_at: string;
}

/** hex 串前 bits 个 bit 是否全 0（与后端 meets_difficulty 同口径） */
function meetsDifficulty(hex: string, bits: number): boolean {
  const full = Math.floor(bits / 4);
  if (hex.slice(0, full).split("").some((c) => c !== "0")) return false;
  const rem = bits % 4;
  if (rem === 0) return true;
  const v = parseInt(hex[full], 16);
  return v <= (0xf >> rem);
}

/** 取挑战 → Web Crypto 循环 nonce 求 solution。
 *  18bit ≈ 平均 26 万次哈希：主线程分片（每 2000 次让出一帧），避免卡 UI。
 *  返回 null = 取题失败或 10s 内未解出（难度被站长调到 24 时如实报失败）。 */
export async function solvePow(
  username: string,
): Promise<{ token: string; solution: string } | null> {
  let ch: PowChallenge;
  try {
    ch = await api.get<PowChallenge>(
      `/api/v1/auth/pow-challenge?username=${encodeURIComponent(username)}`,
    );
  } catch {
    return null;
  }
  const { pow_challenge: challenge, pow_difficulty: bits } = ch;
  const data = new TextEncoder();
  const t0 = Date.now();
  for (let nonce = 0; Date.now() - t0 < 10_000; nonce++) {
    const buf = await crypto.subtle.digest(
      "SHA-256",
      data.encode(`${challenge}:${nonce}`),
    );
    const hex = Array.from(new Uint8Array(buf))
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");
    if (meetsDifficulty(hex, bits)) {
      return { token: ch.pow_token, solution: String(nonce) };
    }
    // 分片让出主线程：每 2000 次哈希交还一帧，避免 UI 冻住
    if (nonce % 2000 === 1999) await new Promise((r) => setTimeout(r));
  }
  return null;
}

/** 从 2012 错误 data 直接取挑战求解（省一次取题往返） */
export async function powFromErrorData(
  data: unknown,
): Promise<{ token: string; solution: string } | null> {
  if (typeof data !== "object" || data === null) return null;
  const d = data as Partial<PowChallenge>;
  if (
    typeof d.pow_token !== "string" ||
    typeof d.pow_challenge !== "string" ||
    typeof d.pow_difficulty !== "number"
  ) {
    return null;
  }
  const ch: PowChallenge = {
    pow_token: d.pow_token,
    pow_challenge: d.pow_challenge,
    pow_difficulty: d.pow_difficulty,
    pow_expires_at: d.pow_expires_at ?? "",
  };
  const dataEnc = new TextEncoder();
  const t0 = Date.now();
  for (let nonce = 0; Date.now() - t0 < 10_000; nonce++) {
    const buf = await crypto.subtle.digest(
      "SHA-256",
      dataEnc.encode(`${ch.pow_challenge}:${nonce}`),
    );
    const hex = Array.from(new Uint8Array(buf))
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");
    if (meetsDifficulty(hex, ch.pow_difficulty)) {
      return { token: ch.pow_token, solution: String(nonce) };
    }
    if (nonce % 2000 === 1999)
      await new Promise((r) => setTimeout(r));
  }
  return null;
}
