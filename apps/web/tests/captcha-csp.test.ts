import { describe, expect, it } from "vitest";

import nextConfig from "../next.config";

/**
 * 第三方真人验证（Turnstile / reCAPTCHA / hCaptcha）靠注册页注入厂商
 * 脚本 + iframe 工作。CSP 是构建期静态头，若不放行厂商域，选任何一家
 * 都会出现「页面没有验证码、提交必报请完成验证码」——开放注册直接
 * 被打停（2026-10-30 现网实测：script-src 只有 'self' 'unsafe-inline'，
 * Turnstile 脚本被浏览器拦掉）。本用例钉住三个必需指令的白名单。
 */

function directive(csp: string, name: string): string {
  return (
    csp
      .split(";")
      .map((part) => part.trim())
      .find((part) => part.startsWith(`${name} `)) ?? ""
  );
}

describe("CSP 放行第三方真人验证", () => {
  it("script-src / frame-src / connect-src 含三家厂商域", async () => {
    const groups = (await nextConfig.headers?.()) ?? [];
    const headers = groups.flatMap((group) => group.headers);
    const csp = headers.find(
      (header) => header.key === "Content-Security-Policy",
    )?.value;
    expect(csp).toBeTruthy();

    const script = directive(csp ?? "", "script-src");
    const frame = directive(csp ?? "", "frame-src");
    const connect = directive(csp ?? "", "connect-src");
    for (const domain of [
      "https://challenges.cloudflare.com",
      "https://www.google.com",
      "https://www.gstatic.com",
      "https://js.hcaptcha.com",
    ]) {
      expect(script).toContain(domain);
    }
    for (const domain of [
      "https://challenges.cloudflare.com",
      "https://www.google.com",
      "https://recaptcha.google.com",
      "https://newassets.hcaptcha.com",
    ]) {
      expect(frame).toContain(domain);
    }
    expect(connect).toContain("https://challenges.cloudflare.com");
  });
});
