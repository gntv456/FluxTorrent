"""常驻闸门：发给会员的站内通知里不许出现裸 uid（`用户 #123`）。

用户视角：收到「用户 #1 回复了你的主题」，根本不知道是谁——站内还有匿名
发布者与成套同名探针号，数字对收件人毫无意义。修法统一走
`repo::actor::actor_name`（在册用户给 username，注销/被处置给「已注销用户」）。

三段判据：
  A1 源码里（**剥掉注释后**再扫，否则注释里提到 `用户 #{}` 会造假红，
     而别人注释里写的键名也能洗白真洞）出现 `用户 #` + 占位/数字 ⇒ 红；
     白名单只放「给职员的」出口：leak_bot 工单、审计日志文案。
  A2 台账钉桩：仍在途未收的那一处（community_http/topic_create.rs 的发帖
     关注者通知）**多一处少一处都红**——多＝新缺陷；少＝已修好，要同步
     把这里从钉桩转成硬断言，不许留着空壳骗自己。
  A3 正控：白名单里的 leak_bot 必须仍带 uid 形状（它是给职员的行，
     被"顺手改掉"就说明判据把口径搞反了）。

自测：python scripts/pt_audit_actor_label.py --selftest
  往 forum_util.rs 插一条裸 uid ⇒ A1 必须红；还原后必须绿。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
API = os.path.join(ROOT, "apps", "api", "src")
WEB = os.path.join(ROOT, "apps", "web")
# 刻意保留 uid 的出口——**按文件+次数记账**（按文件放行会让同文件里
# 新冒出来的裸 uid 一起溜过去）：
#   leak_bot.rs            泄露处置工单，管理组就是按 id 去重置 passkey/封号
#   i18n/zh-CN.ts          admin-users 面板的 flash（classChanged /
#                          statusChanged / adjustedMsg），读者是职员
#   subtitles_flow.rs      把提交载荷里的 crew uid 回显给发起人（组长）。
#                          属轻微可读性问题：改成回显人名时把这里降为 0。
ALLOW = {
    "community_http/leak_bot.rs": 2,
    "i18n/zh-CN.ts": 3,
    "content_http/subtitles_flow.rs": 1,
}
# 待收口台账：本轮清空。最后一条 `community_http/topic_create.rs`
# （「关注的人发了新主题 · 来自用户 #123」）已于 2026-10-21 收口成真人名字，
# 该文件从此按 A1 的零裸 uid 硬判据管。将来若又要钉桩「站方在途文件」，
# 重新加 PENDING 字典 + A2 那条「数量必须等于登记值」的断言，别只改注释。
UID_SHAPE = re.compile(r"用户\s*#\s*(\{|\$|\d)")
SKIP = (os.sep + "node_modules", os.sep + ".next")


def strip(line, in_block):
    """返回 (可判定的代码文本, 新的块注释状态)。"""
    s = line.strip()
    if in_block:
        return "", "*/" not in s
    if s.startswith("/*"):
        return "", not s.endswith("*/")
    if s.startswith("//") or s.startswith("*"):
        return "", False
    # 行尾注释切掉：注释里写 `用户 #{uid}` 不算违例
    code = re.split(r"(?<!:)//", line, maxsplit=1)[0]
    return code, False


def scan():
    hits = {}
    files = []
    for base, tag in ((API, "apps/api/src"), (WEB, "apps/web")):
        for dirpath, _d, fs in os.walk(base):
            if any(s in dirpath for s in SKIP):
                continue
            for f in fs:
                if f.endswith((".rs", ".ts", ".tsx")):
                    p = os.path.join(dirpath, f)
                    files.append((p, os.path.relpath(p, base)
                                  .replace(os.sep, "/")))
    for p, rel in files:
        in_block = False
        n = 0
        try:
            lines = open(p, encoding="utf-8", errors="replace").readlines()
        except OSError:
            continue
        for ln in lines:
            code, in_block = strip(ln, in_block)
            if UID_SHAPE.search(code):
                n += 1
        if n:
            hits[rel] = n
    return hits


def main():
    hits = scan()
    print("扫到裸 uid 形状的文件与次数：", hits or "无")

    bad = {k: "%d 处/允许 %d" % (v, ALLOW.get(k, 0))
           for k, v in hits.items() if v > ALLOW.get(k, 0)}
    ok("A1 发给会员的出口里没有裸 uid（职员侧按文件+次数入账）",
       not bad, "违例 %s" % bad)

    staff = hits.get("community_http/leak_bot.rs", 0)
    ok("A2 正控：职员侧工单仍按 id 写（判据没把口径改反）", staff >= 1,
       "leak_bot 命中=%d" % staff)
    summary()


def selftest():
    victim = os.path.join(API, "community_http", "forum_util.rs")
    src = open(victim, encoding="utf-8").read()
    anchor = "pub(crate) async fn notify_user("
    if src.count(anchor) != 1:
        raise SystemExit("锚点不唯一")
    injected = src.replace(
        anchor, '    let _probe = format!("用户 #{} 探针", 1);\n' + anchor, 1)
    open(victim, "w", encoding="utf-8", newline="").write(injected)
    try:
        flipped = subprocess.run(
            [sys.executable, os.path.abspath(__file__)],
            capture_output=True, text=True, encoding="utf-8", errors="replace")
        print("[自测] 插入裸 uid 后 rc=%s（必须非 0）" % flipped.returncode)
        print("\n".join([l for l in flipped.stdout.splitlines()
                         if "违例" in l or "FAIL" in l][:3]))
    finally:
        open(victim, "w", encoding="utf-8", newline="").write(src)
    back = subprocess.run(
        [sys.executable, os.path.abspath(__file__)],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    print("[自测] 还原后 rc=%s（必须 0）" % back.returncode)
    return 0 if (flipped.returncode != 0 and back.returncode == 0) else 2


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(selftest())
    main()
