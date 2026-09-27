/** 第三方人机验证组件装载（0227）：按 provider 注入脚本并渲染 widget。
 *  从注册页拆出（300 行门禁）；token 由组件回调写入调用方 state。 */

export const DRIVER_WIDGET_ID: Record<string, string> = {
  turnstile: "cf-turnstile",
  recaptcha: "g-recaptcha",
  hcaptcha: "h-captcha",
};

const DRIVER_SCRIPT: Record<string, (k: string) => string> = {
  turnstile: (k) =>
    `https://challenges.cloudflare.com/turnstile/v0/api.js` +
      `?render=explicit&sitekey=${k}`,
  recaptcha: (k) =>
    `https://www.google.com/recaptcha/api.js?render=explicit&sitekey=${k}`,
  hcaptcha: (k) =>
    `https://js.hcaptcha.com/1/api.js?render=explicit&sitekey=${k}`,
};

export function mountCaptchaWidget(
  provider: string,
  siteKey: string,
  onToken: (token: string) => void,
) {
  if (provider === "none" || !siteKey) return;
  const w = window as unknown as {
    turnstile?: { render: (el: HTMLElement, o: unknown) => void };
    grecaptcha?: { render: (el: HTMLElement, o: unknown) => void };
    hcaptcha?: { render: (el: HTMLElement, o: unknown) => void };
  };
  const opts = {
    sitekey: siteKey,
    callback: onToken,
    "expired-callback": () => onToken(""),
  };
  const inject = () => {
    const el = document.getElementById(DRIVER_WIDGET_ID[provider]);
    if (!el) return;
    if (provider === "turnstile") w.turnstile?.render(el, opts);
    else if (provider === "recaptcha") w.grecaptcha?.render(el, opts);
    else if (provider === "hcaptcha") w.hcaptcha?.render(el, opts);
  };
  const sc = document.createElement("script");
  sc.src = DRIVER_SCRIPT[provider](siteKey);
  sc.async = true;
  sc.onload = inject;
  document.head.appendChild(sc);
}
