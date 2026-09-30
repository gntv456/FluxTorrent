# -*- coding: utf-8 -*-
"""娱乐屋「确定侧」核验（0246 用途写侧闸 / 0247 奖励行表）。

四条命题：
1. 奖励行配得动，但**配坏必须在保存时就拒**：一行什么都不发、归属玩法写错、
   引用停用或不存在的物品 —— 这三种都让奖励「看着在、领不到」；
2. 领取真的按行表发出魔力与物品，且同一期不能领第二次；
3. 确定侧不进 EV 闸，所以每一笔都要被**发放预算**看见：物品按 anchor 折算，
   与魔力一起进 det_cost 读数；
4. 门禁要能红：绕过面板直接改库把被引用的物品停用，自检必须点名。

用法：FLUX_API_BASE=http://127.0.0.1:8080/api/v1 \
      python scripts/e2e_arcade_rewards.py
本脚本**不写奖池**（只读它），所以不与 e2e_arcade_items.py 抢同一张表。
"""
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

BASE = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")
fails = []
n = [0]


def call(method, path, body=None, token=None):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body is not None else None
    try:
        with urllib.request.urlopen(req, data, timeout=25) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:
            return e.code, {}


def login(user="root", pw="password123"):
    for i in range(10):
        st, r = call("POST", "/auth/login",
                     {"username": user, "password": pw})
        tok = (r.get("data") or {}).get("token")
        if tok:
            return tok
        time.sleep(min(2 + i * 3, 20))  # 限流窗口比 1.5s 长得多
    raise SystemExit("登录失败")


def check(name, cond, detail=""):
    n[0] += 1
    msg = ("  PASS  " if cond else "  FAIL  ") + name
    if not cond:
        msg += "   " + str(detail)[:260]
        fails.append(name)
    print(msg)


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True,
        # 显式 UTF-8：Windows 默认 codecs 是 cp936，读回中文（label_zh /
        # hint / 报错文案）会让 reader 线程抛 UnicodeDecodeError，主线程
        # 只看到一个莫名的 IndexError: list index out of range。
        encoding="utf-8", errors="replace",
    )
    return r.stdout.strip()


DET = {"key": "det_probe", "name": "确定侧探针券", "kind": "voucher",
       "anchor": 300, "anchor_src": "declared", "unlimited": True,
       "stock": 0, "per_user": 5, "icon": "🧾", "enabled": True}
QCODE = "e2e_det"


def _cleanup():
    """自清范围 = 本脚本写过的每一张表（顺序：先解引用再删物品）。"""
    psql("DELETE FROM arcade_quests WHERE code = '%s'" % QCODE)
    psql("DELETE FROM arcade_claims WHERE ref_code = '%s'" % QCODE)
    psql("DELETE FROM arcade_item_grants WHERE item_key = 'det_probe'")
    psql("DELETE FROM arcade_item_uses WHERE item_key = 'det_probe'")
    psql("DELETE FROM arcade_items WHERE key LIKE '%_probe'")
    print("收尾：奖励行残留 %s 条 / 探针物品残留 %s 件" % (
        psql("SELECT count(*) FROM arcade_quests"
             " WHERE code LIKE 'e2e_%'"),
        psql("SELECT count(*) FROM arcade_items WHERE key LIKE '%_probe'")))


def main():
    tok = login()
    # 目录体检：一件可用途的奖品都没有 = 这半个功能是零，
    # 「端点存在」不能代替「配置真的落地」。
    usable = psql("SELECT count(*)::text FROM arcade_items"
                  " WHERE use_kind <> 'collect' AND enabled")
    check("目录里确实有带用途的奖品（不是只有收藏件）",
          int(usable) >= 1, usable)
    bad_bind = psql(
        "SELECT count(*)::text FROM arcade_items i"
        " WHERE i.use_kind = 'sku' AND NOT EXISTS ("
        "  SELECT 1 FROM shop_items s WHERE s.id::text = i.use_ref"
        "    AND s.active AND i.anchor >= s.price)")
    check("已绑 SKU 的奖品都能真兑出等价东西", int(bad_bind) == 0, bad_bind)

    st, r = call("POST", "/admin/arcade/items", DET, tok)
    check("确定侧探针物品可存", st == 200, (st, r))

    # 探针号：本周要有局数才领得到周常
    uname = "e2edet%d" % int(time.time() % 100000)
    st, r = call("POST", "/admin/adduser",
                 {"username": uname, "email": uname + "@e2e-probe.invalid",
                  "password": "E2eProbe!123"}, tok)
    uid = (r.get("data") or {}).get("user_id")
    check("前置·探针账号已建出 uid", uid is not None, (st, r))
    st, r = call("POST", "/admin/users/adjust",
                 {"user_id": uid, "spark_delta": 5000}, tok)
    check("前置·探针账号已注魔力", st == 200, (st, r))
    st, r = call("POST", "/me/password/change",
                 {"old_password": "E2eProbe!123",
                  "new_password": "E2eProbe!456"},
                 login(uname, "E2eProbe!123"))
    check("前置·临时密码已改掉（不改则一律 400）", st == 200, (st, r))
    ptok = login(uname, "E2eProbe!456")
    drew = 0
    for i in range(4):
        st, _ = call("POST", "/games/jgg",
                     {"idempotency_key": "e2e-det-%d" % i}, ptok)
        drew += 1 if st == 200 else 0
    check("前置·探针号本周有局数可计进度", drew >= 1, drew)

    q = {"kind": "quest", "code": QCODE, "scope": "*", "target": 1,
         "reward_spark": 50, "item_key": "det_probe", "item_qty": 1,
         "enabled": True, "sort": 900}
    st, r = call("POST", "/admin/arcade/rewards",
                 dict(q, reward_spark=0, item_key=""), tok)
    check("一行奖励什么都不发 -> 400", st == 400, (st, r))
    st, r = call("POST", "/admin/arcade/rewards",
                 dict(q, scope="scratches"), tok)
    txt = json.dumps(r, ensure_ascii=False)
    check("周常归属玩法写错 -> 400 并点名可选值",
          st == 400 and "ref_type" in txt, (st, txt[:120]))
    st, r = call("POST", "/admin/arcade/rewards",
                 dict(q, item_key="nope_probe"), tok)
    check("奖励引用不存在/停用的物品 -> 400", st == 400, (st, r))
    st, r = call("POST", "/admin/arcade/rewards", q, tok)
    check("周常行可配（既发魔力也发物品）", st == 200, (st, r))
    st, r = call("POST", "/admin/arcade/items",
                 dict(DET, enabled=False), tok)
    txt = json.dumps(r, ensure_ascii=False)
    check("被启用中奖励引用的物品不许停用（写侧先拒）",
          st == 400 and "奖励" in txt, (st, txt[:120]))

    st, ov = call("GET", "/admin/arcade/overview", None, tok)
    qs = (((ov.get("data") or {}).get("defs") or {}).get("quests")) or []
    check("奖励行读得回来且带物品字段",
          any(x.get("code") == QCODE
              and x.get("item_key") == "det_probe" for x in qs),
          [x.get("code") for x in qs])

    st, cl = call("POST", "/games/arcade/quest/%s/claim" % QCODE,
                  {"idempotency_key": "e2e-claim-1"}, ptok)
    d = cl.get("data") or {}
    check("领取按奖励行发出魔力与物品",
          st == 200 and d.get("reward") == 50
          and d.get("item") == "det_probe" and not d.get("fell_back"),
          (st, d))
    st, cl2 = call("POST", "/games/arcade/quest/%s/claim" % QCODE,
                   {"idempotency_key": "e2e-claim-2"}, ptok)
    check("同一期不能领第二次", st == 400, (st, cl2))

    st, meta = call("GET", "/games/arcade-meta", None, ptok)
    L = ((meta.get("data") or {}).get("ledger")) or {}
    check("预算环读数含物品折算价（det_cost >= 50+300）",
          st == 200 and (L.get("det_cost") or 0) >= 350, L)

    # 读侧兜底：绕过面板直接改库把被引用的物品停用，门禁必须红
    psql("UPDATE arcade_items SET enabled = false WHERE key = 'det_probe'")
    st, ov2 = call("GET", "/admin/arcade/overview", None, tok)
    gates = [c for c in (((ov2.get("data") or {}).get("checks")) or [])
             if "确定侧奖励" in str(c.get("name", ""))]
    check("直接停用被引用物品后门禁红并点名",
          len(gates) == 1 and gates[0].get("pass") is False, gates)
    psql("UPDATE arcade_items SET enabled = true WHERE key = 'det_probe'")

    # 物品用途的写侧闸（0246）：配错要在保存时就拒，别等玩家点「使用」
    shape = dict(DET, key="shape_probe", anchor=900, use_kind="spark")
    st, r = call("POST", "/admin/arcade/items", shape, tok)
    check("形状探针物品可存", st == 200, (st, r))
    st, r = call("POST", "/admin/arcade/items",
                 dict(shape, use_kind="sparkk"), tok)
    check("use_kind 写错拼法直接拒，不静默当成收藏件",
          st == 400 and "use_kind" in json.dumps(r, ensure_ascii=False),
          (st, json.dumps(r, ensure_ascii=False)[:110]))
    st, r = call("POST", "/admin/arcade/items",
                 dict(shape, use_kind="sku", use_ref="999999"), tok)
    check("绑定不存在的商店 SKU 时保存被拒", st == 400, (st, r))
    row = psql("SELECT id::text || ':' || price::text FROM shop_items"
               " WHERE kind='avatar_frame' AND active"
               " ORDER BY price DESC, id LIMIT 1")
    sku_id, sku_price = row.split(":")
    st, r = call("POST", "/admin/arcade/items",
                 dict(shape, use_kind="sku", use_ref=sku_id,
                      anchor=int(sku_price) - 1), tok)
    check("anchor 低于所绑 SKU 售价时保存被拒（少计负债）",
          st == 400, (st, json.dumps(r, ensure_ascii=False)[:120]))
    st, r = call("POST", "/admin/arcade/items",
                 {"key": "shape_probe", "name": "形状探针（改名）",
                  "kind": "voucher", "anchor": 900,
                  "anchor_src": "declared"}, tok)
    keep = psql("SELECT use_kind || '/' || unlimited::text || '/'"
                " || per_user::text FROM arcade_items"
                " WHERE key='shape_probe'")
    check("只改名字的保存不把用途与限购打回缺省",
          st == 200 and keep == "spark/true/5", keep)

    # 探针号自清（删除接口设计为仅封禁态可删，故先封再删）
    st, _ = call("POST", "/admin/users/status",
                 {"user_id": uid, "status": 2, "reason": "e2e 探针清理"}, tok)
    st2, _ = call("DELETE", "/admin/users/%s" % uid, None, tok)
    check("探针账号已清理（封禁→删除）", st == 200 and st2 == 200,
          {"uid": uid, "ban": st, "del": st2})

    tail = "" if not fails else " -> " + "; ".join(fails[:5])
    print("\n结果：%d 项断言，失败 %d%s" % (n[0], len(fails), tail))
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    try:
        main()
    finally:
        try:
            _cleanup()
        except BaseException as e:  # 兜底失败要喊出来，不能静默
            print("确定侧探针清理失败，请手工核对 arcade_quests/arcade_items"
                  "：%r" % e)
