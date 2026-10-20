# -*- coding: utf-8 -*-
"""常驻闸门：绑定生效中的免费券不许被指控「已使用或已过期」（P-459）。

形状：`user_vouchers` 的「生效中」= `used_torrent_id` 已写、`used_at` 仍是 NULL
（worker 计费侧按这个组合匹配，下载量过阈值才核销——见 voucher_use.rs 的注释，
旧版绑定时就写 used_at 会让券永远不被计费侧匹配，那是真资损，已修）。
但 `/me/vouchers` 把 `used_torrent_id` 回给前端，面板的「可用券」过滤器只看
`used_at` + 有效期 ⇒ **绑定生效中的券照样列在可用区、照样给「使用」按钮**；
再点一次，三处拒绝口都说「券不存在、已使用或已过期」——现网 vid=32 实测
`used_at=NULL / 过期=false / 已绑定=40`，三句指控全是假的。管理撤券口更干脆：
对「没有可撤的券」回 404 资源不存在。

修法：判定与文案抽成纯函数（`vouchers.rs::VoucherState / unusable_reason /
state_of / why_unusable`），三处拒绝口共用；面板对绑定券显示
「已对种子 #X 生效，下载量超过 4% 后自动核销」并撤掉使用按钮；撤券口 404 改
成说清范围的话。

判据分层：
  T0 正控   FIX^ 的谎话字面量确实出现 3 次，且现网真有一张 Bound 行
  T1        接线：三处都改走 why_unusable；谎话字面量只剩「读状态失败」兜底一处
  T2        纯函数单测（含「绑定态不许出现已使用/已过期/不存在」那条本体断言）
  T3        面板：绑定分支在 rename_card 之前，且不再有可用的 use 按钮路径
  T4        撤券口不再回 NotFound，文案说明本口管得到哪些券
  T5        现网账面：生效中券的行数（0 行时 T0 的正控前提失效，要出声）
跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_voucher_state_messages.py
      负控：同加 --selftest
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
VS = "apps/api/src/economy_http/vouchers.rs"
VU = "apps/api/src/economy_http/voucher_use.rs"
AV = "apps/api/src/admin_http/user_vouchers.rs"
PN = "apps/web/components/voucher-panel.tsx"
FIX = "9c999257"
LIE = "券不存在、已使用或已过期"


def read(rel, ref=None):
    if ref:
        return subprocess.run(["git", "show", "%s:%s" % (ref, rel)],
                              capture_output=True, text=True,
                              encoding="utf-8", errors="replace").stdout
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read().replace("\r\n", "\n")


def strip(text):
    t = re.sub(r"/\*[\s\S]*?\*/", " ", text)
    return re.sub(r"(?<!:)//[^\n]*", " ", t)


def state_sql():
    src = read(VS)
    i = src.index("SELECT used_torrent_id, used_at, expires_at")
    lit = src[src.rindex('"', 0, i) + 1: src.index('",', i)]
    return re.sub(r"\\\s*\n\s*", "", lit)


def main():
    old_use = strip(read(VU, FIX + "^"))
    new_use = strip(read(VU))
    # ---------- T0 正控 ----------
    # 必须剥注释再数：FIX^ 里除了三处代码文案，还有一句 P-407 的注释在**引用**
    # 这句谎话——按原文数会得到 4，判据就成了自己造出来的假红（10-21 实踩）。
    ok("T0 正控：FIX^ 的谎话字面量在代码里出现 3 次（实=%d）"
       % old_use.count(LIE), old_use.count(LIE) == 3,
       "旧形状数不对＝这条尺看的已经不是这批修复")
    bound = psql_stdin("SELECT id FROM user_vouchers WHERE used_torrent_id "
                       "IS NOT NULL AND used_at IS NULL ORDER BY id LIMIT 1;")
    row = ""
    if bound:
        row = psql_stdin(state_sql().replace("$1", bound).replace(
            "$2", psql_stdin("SELECT user_id FROM user_vouchers "
                             "WHERE id=%s;" % bound)))
    ok("T0b 现网确有一张「生效中」的券（vid=%s，源码 state_of SQL 回读=%s）"
       % (bound or "无", row or "-"), bool(bound) and bool(row),
       "没有这种行了＝T0 的正控前提失效，需自造夹具再验")
    # ---------- T1 接线 ----------
    calls = new_use.count("vouchers::why_unusable(")
    lies = new_use.count(LIE) + strip(read(VS)).count(LIE)
    ok("T1 三处拒绝口都改走 why_unusable（实=%d），谎话字面量全仓只剩兜底"
       "一处（实=%d）" % (calls, lies), calls == 3 and lies == 1,
       "接线被搬回字面量＝又开始假指控")
    ok("T1b 状态机在册：VoucherState / unusable_reason / state_of 都在 "
       "vouchers.rs", all(k in read(VS) for k in (
           "enum VoucherState", "fn unusable_reason", "async fn state_of")),
       "被拆走时这条要红，提醒同步改调用点")
    # ---------- T2 纯函数 ----------
    p = subprocess.run(["cargo", "test", "--bin", "flux-api",
                        "voucher_state_tests"], capture_output=True,
                       text=True, encoding="utf-8", errors="replace",
                       cwd=os.path.join(ROOT, "apps", "api"))
    tail = [ln for ln in (p.stdout or "").split("\n") if "test result" in ln]
    ok("T2 纯函数单测通过（%s）" % (tail or "无输出"),
       p.returncode == 0 and "0 failed" in " ".join(tail),
       "「绑定态不得被说成已使用/已过期」这条断言掉了")
    # ---------- T3 面板 ----------
    panel = strip(read(PN))
    i_bound = panel.find("v.used_torrent_id")
    i_kind = panel.find('v.kind === "rename_card"')
    ok("T3 面板对绑定生效中的券先判 v.used_torrent_id（位置 %s < %s）并显示"
       "生效文案，不再给「使用」按钮" % (i_bound, i_kind),
       0 <= i_bound < i_kind and "dict.vouchers.used.replace" in panel,
       "顺序反了或分支没了＝会员仍会看到能点、点了被假指控的券")
    # ---------- T4 撤券口 ----------
    admin = strip(read(AV))
    ok("T4 撤券口空结果不再回 NotFound，改为说明本口范围的校验错",
       "return Err(DomainError::NotFound(body.user_id));" not in admin
       and "本口只撤" in admin,
       "回 404「资源不存在」＝管理员看不出是券的问题")
    # ---------- T5 账面 ----------
    n = int(psql_stdin("SELECT count(*) FROM user_vouchers WHERE "
                       "used_torrent_id IS NOT NULL AND used_at IS NULL;"))
    print("   [T5] 现网「生效中（已绑定未核销）」的券=%d 张；"
          "0 张时 T0b 会红，需自造夹具" % n)
    if "--selftest" in sys.argv:
        selftest()
    summary()


def selftest():
    """把一处 why_unusable 翻回谎话字面量 ⇒ T1 必须红；按字节还原。"""
    p = os.path.join(ROOT, VU)
    snap = io.open(p, "rb").read()
    txt = snap.decode("utf-8")
    a = ("        return Err(DomainError::Validation(\n"
         "            super::vouchers::why_unusable(\n"
         "                &state.repo.db,\n"
         "                body.voucher_id,\n"
         "                auth.id,\n"
         "            )\n"
         "            .await,\n"
         "        ));")
    b = ('        return Err(DomainError::Validation("%s".into()));' % LIE)
    if txt.count(a) != 1:
        ok("S0 负控锚点唯一（命中 %d）" % txt.count(a), False,
           "锚点搬家，先去看三处拒绝口现在长什么样")
        return
    try:
        io.open(p, "wb").write(txt.replace(a, b, 1).encode("utf-8"))
        now = strip(read(VU))
        ok("S1 负控：翻回一处字面量后 T1 抓到（调用数=%d 字面量数=%d）"
           % (now.count("vouchers::why_unusable("), now.count(LIE)),
           now.count("vouchers::why_unusable(") == 2,
           "翻红了没抓到＝判据不承重")
    finally:
        io.open(p, "wb").write(snap)
    back = io.open(p, "rb").read() == snap
    now = strip(read(VU))
    ok("S2 负控还原：按字节回到翻转前（%s）且正向复跑三处接线齐全"
       % back, back and now.count("vouchers::why_unusable(") == 3,
       "还原不干净＝把翻转态留在了工作树")


if __name__ == "__main__":
    main()
