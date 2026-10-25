# -*- coding: utf-8 -*-
"""闸门 DX：趣味盒（/fun/*）的权限、入参上限与计票前提。

批一百七十九。会员视角：我发一段笑话，然后给自己打「很好笑」，再把刚发的标成
「无聊」立刻再发一条——这两个动作在旧 API 上都成立。

  P-368（本批已修，接口层已当场验）：
  ① 状态位权限放开错人。旧判据 `(status == "banned" || owner != auth.id) &&
     !is_staff` 只拦「禁别人」和「banned」，**作者改自己的状态一律放行**，
     于是作者可自判 veryfunny/funny/notfunny/dull。而界面
     `fun-box.tsx:125` 的 setStatus 类型是 `"banned" | "normal"` 且在 staff
     面板里——两份规则里被改漏的是 API 这侧（文件头注释其实写的是
     「staff 或作者对自己可 dull」，代码比注释宽了四个状态）。
  ② 标题不判长度：`fun_items.title` 是 varchar(255)，超长提交撞 22001 ⇒
     会员收到 500 而不是中文 422（按**字符**判，不按字节——P-331 反向陷阱）。
  ③ 计票前提缺失：投给不存在的 fun_id 撞外键变 500；已 banned 的内容照旧收票。

接口层实测（侧进程 :8099）：超长标题 1002、投不存在 1004、staff 改 veryfunny
仍 0（合法路径没打死）、给已禁内容投票 1002；夹具行与票行收尾删净并独立复核。

**仍未闭合的两条（点名，不冒充修好）**：
  ④ 非 staff 作者自判被拒——**没在 HTTP 层验过**：现网没有可用的非 staff 凭据，
     我不在验收里注册新会员（会留会员行与注册额度占用）。本闸门的 DX3 只钉代码
     形状，DX5 用真库数据反证「判决级状态的作者必须是 staff」。
  ⑥ 24h 冷却的口径要站方裁：`fun_item_create` 的冷却池排除 `banned`/`dull`，
     旧版作者自设 dull 就能把自己移出冷却池、连发多条。本批把 dull 收回给 staff
     之后，这条 bypass 需要 staff 权限才成立；如果将来又放开作者自设 dull，
     冷却必须改成「不按 status 排除」或按用户计，否则额度还是会被自己抹掉。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_dx_fun_box.py
"""
import io
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FUN = os.path.join(ROOTDIR, "apps", "api", "src", "http", "fun_links.rs")
TSX = os.path.join(ROOTDIR, "apps", "web", "components", "fun-box.tsx")
VERDICT = ("'dull'", "'notfunny'", "'funny'", "'veryfunny'")


def rd(p):
    return io.open(p, encoding="utf-8", errors="replace").read()


def cut(f):
    s = re.sub(r"/\*.*?\*/", "", rd(f), flags=re.S)
    return "\n".join(re.sub(r"//.*", "", ln) for ln in s.split("\n"))


def psql(q):
    r = subprocess.run(
        ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1", "flux-postgres",
         "psql", "-U", "flux", "-q", "-d", "fluxtorrent", "-tA", "-f", "-"],
        input=q, capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise RuntimeError("   psql 失败：%s" % (r.stderr or "").strip()[:240])
    return (r.stdout or "").strip()


def sval(q):
    return psql(q).split("\n")[0].strip()


body = cut(FUN)
tsx = rd(TSX)

# ---------- DX1 标题上限 ----------
ok("DX1 create/update 两处都调 check_fun_title（命中 %d 处）"
   % body.count("check_fun_title(&body.title)"),
   body.count("check_fun_title(&body.title)") == 2,
   "只在一处判＝另一处仍能撞 varchar(255) 变 500")
ok("DX1b 长度按字符判（chars().count()），不是字节 len()",
   "title.chars().count() > 255" in body
   and not re.search(r"title\.len\(\)\s*>\s*255", body),
   "按字节会把 85 个中文当成 255 以内（P-331 反向）")

# ---------- DX2 计票前提 ----------
i_vote = body.find("async fn fun_item_vote")
i_next = body.find("struct FunItemBody")
vote = body[i_vote:i_next] if 0 <= i_vote else body[i_vote:]
ok("DX2 投票前先判目标存在（NotFound）与已禁（Validation）",
   "DomainError::NotFound(body.fun_id as i64)" in vote
   and '"banned"' in vote,
   "不判就是 500 外键错；已禁内容还能涨票")
ok("DX2b 判据在 INSERT 之前（顺序反了会先写票再报错）",
   0 <= vote.find("fun_items WHERE id = $1") < vote.find(
       "INSERT INTO fun_item_votes"),
   "先写后判＝脏票行留在库里")

# ---------- DX3 状态权限 ----------
i_st = body.find("async fn fun_item_set_status")
i_end = body.find("async fn fun_item_delete")
st = body[i_st:i_end] if 0 <= i_st else body[i_st:]
ok("DX3 作者自判被拦（owner==auth.id 且非 staff 一律拒）",
   re.search(r"owner == auth\.id && !is_staff_of", st.replace("\n", " "))
   is not None,
   "放开作者自判＝会员能给自己发判决级标签（界面并不提供这个操作）")
ok("DX3b staff 的判决路径仍保留（FUN_MANAGE 或 staff 档任一）",
   "FUN_MANAGE" in st and "is_staff_of" in st,
   "收紧时把 staff 一起打死＝运营失去 ban/restore")
ok("DX3c 旧的那条形宽条件已消失（不得再有 owner != auth.id 的短路放行）",
   '(body.status == "banned" || owner != auth.id)' not in st,
   "留着就是「非 banned 且是作者」整条放开的原洞")

# ---------- DX4 与界面同向 ----------
ok("DX4 界面只提供 staff 的 ban/restore（API 收紧后两层才同形）",
   'status: "banned" | "normal"' in tsx,
   "界面这份若改了（例如给作者加自判按钮），本判据要与 DX3 一起翻")

# ---------- DX5 真库反证 + 正控 ----------
JUDGE = ("SELECT count(*) FROM fun_items f JOIN users u ON u.id = f.user_id "
         "WHERE f.status IN ('dull','notfunny','funny','veryfunny') "
         "AND u.class_id < 90")
viol = sval(JUDGE)
ok("DX5 现网「判决级状态由非 staff 作者持有」的行数 %s（应为 0）" % viol,
   viol == "0",
   "非 0 就是旧放开规则留下的既成事实，得逐条改回 normal 或收回 staff 名下")
PC = r"""
BEGIN;
INSERT INTO fun_items (id, user_id, title, status)
VALUES (999998, (SELECT min(id) FROM users WHERE class_id < 90),
        'dx-probe', 'veryfunny');
SELECT 'probe=' || count(*) FROM fun_items f JOIN users u ON u.id = f.user_id
 WHERE f.status IN ('dull','notfunny','funny','veryfunny')
   AND u.class_id < 90;
ROLLBACK;
SELECT 'after=' || count(*) FROM fun_items WHERE id = 999998;
"""
pc = {}
for ln in psql(PC).split("\n"):
    ln = ln.strip()
    if "=" in ln:
        k, v = ln.split("=", 1)
        pc[k.strip()] = v.strip()
ok("DX6 正控：植入一条非 staff 的 veryfunny 后判据数到 %s（须 1），"
   "回滚后残留 %s（须 0）" % (pc.get("probe"), pc.get("after")),
   pc.get("probe") == "1" and pc.get("after") == "0",
   "数不到 1＝DX5 是空判据（现网没人用这功能时最容易假绿）")

# ---------- DX7 待裁台账 ----------
# 源码里那句 SQL 是跨行拼接的（`status NOT IN \` + `'banned','dull')`），
# Rust 字符串续行的反斜杠会留在剥注释后的文本里 ⇒ 先去掉 `\` 再折空白，
# 否则探测不命中，会把「仍在」报成「已不在」（判据自己撒谎）。
flat = re.sub(r"\s+", " ", re.sub(r"\\\s*", " ", body))
cool = "status NOT IN ('banned','dull')" in flat
ok("DX7 冷却池仍排除 banned/dull（在册：%s）" % ("是" if cool else "否"),
   True, "")
print("   [待裁 P-368⑥] 24h 冷却池排除 banned/dull：%s——作者自判已被收走，"
      "故该 bypass 需 staff 才可触发；若将来放开作者自设 dull，冷却必须改成"
      "不按 status 排除或按用户计，否则额度仍会被自己抹掉"
      % ("仍在" if cool else "已不在（源码形状变了，请复核本判据）"))

ok("DX8 残料：本闸门未留夹具行 %s" % sval(
    "SELECT count(*) FROM fun_items WHERE title LIKE 'dx-probe%' OR "
    "title LIKE '闸门夹具%'"),
   sval("SELECT count(*) FROM fun_items WHERE title LIKE 'dx-probe%' OR "
        "title LIKE '闸门夹具%'") == "0",
   "夹具留在现网会被下一个会员看到")

summary()
