# -*- coding: utf-8 -*-
"""常驻闸门 JP：推送测试的读数必须诚实（P-570）。

会员视角：设置页那枚「发送测试」按钮的唯一用途就是验证
「订阅→投递→收到」整条链。旧实现把 `PushResult::ServiceError`
（推送服务 4xx/5xx、网络错、出网闸门拒绝、加密失败）与 Delivered
一起计进 `delivered`，前端又不读响应体、直接 flash「测试消息已发送」
——一次根本没送达的测试也报成功。`Gone`（订阅已失效）同样被前端忽略，
用户看着「已发送」，服务端却已把这枚订阅标成过期。

判据：
  JP1 后端三档分开计数（delivered / failed / expired_cleaned）
  JP2 前端按读数回话，不再无条件 flash(testSent)
  JP3 失效读数要能落到「重新开启推送」这句人话
  JP4 三本字典都有四句文案
  JP5 正控：修前的旧形状必须被同一套判据判为违例
  JP6 推送面板的范围说明必须把 H&R 也写上（订阅里一直含 hr）
"""
import io
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
API = os.path.join(ROOT, "apps", "api", "src", "push_http.rs")
UI = os.path.join(
    ROOT, "apps", "web", "components", "push-settings.tsx")
LIB = os.path.join(ROOT, "apps", "web", "lib", "push-test-result.ts")
DICTS = {
    "zh-CN": os.path.join(ROOT, "apps", "web", "i18n", "zh-CN.ts"),
    "en": os.path.join(ROOT, "apps", "web", "i18n", "en.ts"),
    "zh-TW": os.path.join(ROOT, "apps", "web", "i18n", "zh-TW.ts"),
}

OLD_API = ("            PushResult::Delivered | "
           "PushResult::ServiceError => delivered += 1,")
OLD_UI = ('      await api.post("/api/v1/push/test", {});\n'
          '      flash(t.testSent);')

# 修前旧形状（仅用于 JP5 正控，不进判据正文）
OLD_UI_RE = (r'api\.post(?:<[^>]+>)?\("/api/v1/push/test"'
             r'[\s\S]{0,160}?flash\(t\.testSent\)')


def body(path):
    """读源码并剥整行注释：注释里复述旧形状不许把真洞洗白。"""
    raw = io.open(path, encoding="utf-8", errors="replace").read()
    keep = []
    for line in raw.splitlines():
        s = line.strip()
        if s.startswith("//") or s.startswith("*"):
            continue
        keep.append(line)
    return "\n".join(keep)


def api_bad(text):
    bad = []
    if "PushResult::Delivered | PushResult::ServiceError" in text:
        bad.append("失败仍并进 delivered")
    if "PushResult::ServiceError => failed += 1" not in text:
        bad.append("缺少 failed 计数")
    if '"failed": failed' not in text:
        bad.append("响应缺少 failed")
    return bad


def ui_bad(text):
    bad = []
    if re.search(OLD_UI_RE, text):
        bad.append("仍无条件 flash(testSent)")
    if "pushTestMessage(" not in text:
        bad.append("没有按读数回话")
    return bad


def main():
    api = body(API)
    ui = body(UI)
    lib = body(LIB)

    b = api_bad(api)
    ok("JP1 后端把送达/失败/失效分开计数", not b, "违例=%s" % b)
    b = ui_bad(ui)
    ok("JP2 前端按 /push/test 的读数回话", not b, "违例=%s" % b)
    ok("JP3 失效读数落到「重新开启推送」那句",
       "testExpired" in lib and "expired_cleaned" in lib)
    miss = []
    for name, p in DICTS.items():
        d = body(p)
        for k in ("testSent", "testPartial", "testFailed", "testExpired"):
            if (k + ":") not in d:
                miss.append("%s/%s" % (name, k))
    ok("JP4 三本字典都有四句测试结果文案", not miss, "缺=%s" % miss)
    ok("JP5 正控：修前旧形状必须被同一套判据判为违例",
       bool(api_bad(OLD_API)) and bool(ui_bad(OLD_UI)),
       "旧后端=%s 旧前端=%s" % (api_bad(OLD_API), ui_bad(OLD_UI)))
    no_hr = [n for n, p in DICTS.items()
             if "H&R" not in _topic_line(body(p))]
    ok("JP6 推送范围说明写明 H&R（订阅里一直含 hr 话题）",
       not no_hr, "缺 H&R=%s" % no_hr)
    summary()


def _topic_line(text):
    for line in text.splitlines():
        if "topicsNote:" in line:
            return line
    return ""


if __name__ == "__main__":
    main()
