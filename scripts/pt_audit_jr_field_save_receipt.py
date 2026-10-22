#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""P-572 常驻闸门：字段保存错误必须点名，usercp 回执必须读 skipped。

现网红证（2026-10-22，root 账号，探针字段已删）：
  · select 选项由 [a,b] 收紧到 [b] 后，旧值 a 保存 →
    400「参数校验失败: 值不在字段选项集内」，整句没有字段名，
    用户面对一屏字段不知道是哪一格拦的；
  · 未知/已下线字段保存 → 200 code=0 {saved:0, skipped:1}，
    而 usercp-fields.tsx 不看回执，永远提示「已保存」。

后端修法：三处写口统一用 crate::fields::named(label, msg) 拼 422。
本闸门是源码回归；后端活体绿证待 api 镜像重建后再补。
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from pt_audit_lib import ok, summary  # noqa: E402


def text(rel):
    return (ROOT / rel).read_text(encoding="utf-8")


# 旧形状（负控输入）：必须被对应判据判成违例，否则判据不承重。
SELF_OLD = "            .map_err(DomainError::Validation)?;"
UI_OLD = 'await api.put("/api/v1/me/fields", { values: vals });'


def self_named_ok(src):
    return "DomainError::Validation(named(&_label, &e))" in src


def admin_named_ok(src):
    return "DomainError::Validation(named(&label, &e))" in src


def reg_named_ok(src):
    return "crate::fields::named(label, &e)" in src


def ui_receipt_ok(src):
    return (
        "api.put<{ saved: number; skipped: number }>" in src
        and "r.skipped > 0" in src
        and "t.savedPartial" in src
    )


fields = text("apps/api/src/fields.rs")
ok(
    "JR1 fields::named 是唯一点名出口",
    "pub fn named(" in fields and "「{label}」{msg}" in fields,
)

user_fields = text("apps/api/src/auth_http/user_fields.rs")
ok("JR2 本人写口校验错误带字段名", self_named_ok(user_fields))
ok("JR2a 负控：旧的无名错误形状必须判违例", not self_named_ok(SELF_OLD))

admin = text("apps/api/src/admin_http/user_fields_admin.rs")
ok("JR3 后台代改校验错误带字段名", admin_named_ok(admin))

register = text("apps/api/src/auth_http/register.rs")
ok("JR4 注册口径校验错误带字段名", reg_named_ok(register))

ui = text("apps/web/components/usercp-fields.tsx")
ok("JR5 usercp 按 saved/skipped 如实回话", ui_receipt_ok(ui))
ok("JR5a 负控：无条件「已保存」的旧形状必须判违例", not ui_receipt_ok(UI_OLD))

for name in ("zh-CN", "zh-TW", "en"):
    d = text("apps/web/i18n/%s.ts" % name)
    ok("JR6 %s 字典含 savedPartial 且唯一" % name,
       d.count("savedPartial:") == 1)

t = text("apps/web/tests/usercp-fields-skipped-receipt.test.tsx")
ok(
    "JR7 用例覆盖 skipped=1 与 skipped=0 两档",
    "skipped: 1" in t and "skipped: 0" in t,
)

summary()
