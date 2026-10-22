# -*- coding: utf-8 -*-
"""常驻闸门 JI：可解释的业务结局不许演成 500（P-434 与它的台账）。

会员/运维双视角：商城扣款成功但订单行没建立时，代码把退款这件事写成
`DomainError::Internal(anyhow!("订单建立失败，已原路退回 N 火花；请稍后重试"))`
⇒ HTTP 500 + code 1000。那句话是**业务结局**（钱回来了、请重试），不是
服务故障：本仓 errors.rs 开头（P-102）就写着这条原则——「此前用 Internal
表达 ⇒ 500『内部错误』，用户与运维都看不出是配置状态」。后果是监控为一个
正常兜底翻页警、值班被叫错方向（与 GC3「把数据缺席演成服务故障」同族）。

判据三层：
  JI1  档位：落空退款那一句必须装在 Validation（400/1002）里，
       判法只看中文文案前面的那段上下文（同 P-478 一族「档位与文案分层」）
  JI2  正控：把修前的旧形状喂同一把尺，必须判为违例——否则这条判据是空的
  JI3  台账：全仓「以 Internal 携带中文文案」的位置必须**不增不减**
       （增＝又有人把业务结局塞进 500；减＝有人修好了却不同步这条尺，
        与 IZ4/EY5 同一双向规矩）。现册 10 个文件 13 处，逐条都是真故障
        （存储/限流/PoW/余额溢出/系统审核员配置错乱/批量发放中断）。

在册待裁（本闸门不擅自改口）：admin_p3_http/increment_bulk.rs 两处
（「已发放 N 人后中断」「账已落地无需重发」）与 admin_p3_http/ops.rs 一处
——它们既是业务结局也需要告警，档位该是 409 还是 500 归站长定，
改谁的都要同步这条台账。
"""
import io
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRCDIR = os.path.join(ROOT, "apps", "api", "src")
SHOP = os.path.join("economy_http", "shop.rs")
MSG = "订单建立失败"
# 修前形状（裁 B 落地后残留的尾巴，P-434）
OLD_SHAPE = ('return Err(DomainError::Internal(anyhow::anyhow!(\n'
             '            "订单建立失败，已原路退回 {total} 火花；请稍后重试"\n'
             '        )));')
LEDGER = {
    "admin_http/review_auto_decide.rs": 1,
    "admin_p3_http/increment_bulk.rs": 2,
    "admin_p3_http/ops.rs": 1,
    "economy_http/clawback.rs": 1,
    "economy_http/spend.rs": 1,
    "games_http/helpers.rs": 1,
    "gaps_http/pow.rs": 2,
    "gaps_http/reg_quiz.rs": 2,
    "http/throttle.rs": 1,
    "repo/lottery_fair.rs": 1,
}
PAT_INT = re.compile(
    r'DomainError::Internal\(\s*anyhow::anyhow!\(\s*\n?\s*"([^"]{0,160})"',
    re.S)
HAN = re.compile(r"[一-鿿]")


def body(rel):
    """读源码并剥整行注释：站方注释里常复述旧形状，不剥就是「注释洗白真洞」。"""
    raw = io.open(os.path.join(SRCDIR, rel), encoding="utf-8",
                  errors="replace").read()
    return "\n".join(l for l in raw.splitlines()
                     if not l.strip().startswith("//"))


def tier_of(text, msg=MSG):
    """只看装这句文案的那个 `return Err(...)` 里写的是哪个档位——
    不能拿上下文窗口比对：紧邻的上一句 `map_err(Internal)` 会把「Internal」
    蹭进窗口，判据就假红（本批实测）。"""
    i = text.find(msg)
    if i < 0:
        return ""
    j = text.rfind("return Err(", 0, i)
    return " ".join(text[j:i].split()) if j >= 0 else ""


def ledger_now():
    got = {}
    for dirpath, _d, names in os.walk(SRCDIR):
        for n in names:
            if not n.endswith(".rs"):
                continue
            rel = os.path.relpath(os.path.join(dirpath, n), SRCDIR)
            rel = rel.replace("\\", "/")
            for m in PAT_INT.finditer(body(rel)):
                if HAN.search(m.group(1)):
                    got[rel] = got.get(rel, 0) + 1
    return got


def main():
    tier = tier_of(body(SHOP))
    ok("JI1 落空退款走 Validation(400/1002)，不再用 Internal(500/1000)",
       "DomainError::Validation" in tier and "Internal" not in tier,
       "档位=%s" % tier[:88])
    ok("JI1b 文案一字未改（会员读到的还是那句『已原路退回…请稍后重试』）",
       MSG in body(SHOP) and "已原路退回" in body(SHOP))
    ok("JI2 正控：修前那段旧形状必须被同一把尺判为违例",
       "DomainError::Validation" not in tier_of(OLD_SHAPE),
       "抓不到旧形状＝JI1 的绿不算数")
    got = ledger_now()
    ok("JI3 台账：Internal 携带中文文案的位置不增不减（现 %d 处 / %d 文件）"
       % (sum(got.values()), len(got)), got == LEDGER,
       "新增=%s 消失=%s" % (sorted(set(got) - set(LEDGER)),
                            sorted(set(LEDGER) - set(got))))
    print("[INFO] 在册 %d 处分布：%s"
          % (sum(got.values()), ", ".join("%s×%d" % (k, v)
                                          for k, v in sorted(got.items()))))


if __name__ == "__main__":
    main()
    summary()
