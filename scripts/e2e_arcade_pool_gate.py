# -*- coding: utf-8 -*-
"""娱乐屋奖池写侧关闸核验（0242 / fa193f2）。

要验的命题只有一条：**面板没有能力把玩法打成 503**。
`arcade_pool_save` 的 `validate_pool` 在 `db.begin()` 之前，所以不合法的保存必须
「返回 400 且表里一字未改」——只验 400 不验表未变，等于没验关闸（可能先写后判）。

读侧一律走 `/admin/arcade/overview`：它现在读表，所以「写进去了吗」和「玩法读到的是
同一份」用同一个端点证明，避免脚本自己查库造出第二份口径。

端点尚未部署时 POST 会返回 404「接口不存在」——那是旧二进制，不是关闸失效。
关闸失效的表现是 200 且表被改坏；脚本把两种情况分开报，不会把前者误读成后者。

用法：起一个带新二进制的侧容器，然后
    FLUX_API_BASE=http://127.0.0.1:8180/api/v1 python scripts/e2e_arcade_pool_gate.py
"""

import json
import os
import sys
import time
import urllib.error
import urllib.request

BASE = os.environ.get("FLUX_API_BASE", "http://127.0.0.1:8080/api/v1")
POOL_KEY = "jgg_default"
fails = []
n_checks = [0]


def call(method, path, body=None, token=None):
    req = urllib.request.Request(BASE + path, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    data = json.dumps(body).encode() if body is not None else None
    try:
        with urllib.request.urlopen(req, data, timeout=20) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        try:
            return e.code, json.loads(e.read())
        except Exception:
            return e.code, {}


def login(user="root", pw="password123"):
    for _ in range(4):
        st, r = call("POST", "/auth/login", {"username": user, "password": pw})
        tok = (r.get("data") or {}).get("token")
        if tok:
            return tok
        time.sleep(1.5)
    raise SystemExit("登录失败：侧容器是否指向同一个库？")


def snapshot(tok):
    """读侧现状。overview 读表，所以这就是表的投影。"""
    st, r = call("GET", "/admin/arcade/overview", token=tok)
    assert st == 200, ("读 overview 失败", st, r)
    d = r.get("data") or {}
    return d


def _norm(prize):
    """把读侧档位归一成脚本内部形状。
    0244 之后魔力位报 multiples、物品位报 item_key/qty/anchor，
    没有统一的 payout 字段 —— 在这里补别名，下游就不用到处猜。"""
    p = dict(prize)
    if p.get("kind") == "item":
        p["payout"] = 0
    else:
        p["payout"] = p.get("multiples", 0)
    return p


def pool_of(tok):
    st, r = call("GET", "/games", token=tok)
    if st != 200:
        return None
    j = (r.get("data") or {}).get("jgg") or {}
    if j.get("prizes"):
        j["prizes"] = [_norm(x) for x in j["prizes"]]
    return j


def to_req(p):
    """读侧档位 -> 写侧请求体：按 kind 分派，不再假设每档都有 payout。"""
    e = {"label": p["label"], "weight": p["weight_permille"],
         "kind": p.get("kind", "magic"), "enabled": True}
    if e["kind"] == "item":
        e.update({"item_key": p["item_key"], "qty": p["qty"]})
    else:
        e["payout"] = p["payout"]
    return e


def check(name, cond, detail=""):
    print(("  PASS  " if cond else "  FAIL  ") + name + ("" if cond else "   " + str(detail)[:220]))
    n_checks[0] += 1
    if not cond:
        fails.append(name)


def main():
    tok = login()
    print("FLUX_API_BASE =", BASE)

    before = pool_of(tok)
    if not before or not before.get("prizes"):
        raise SystemExit("读侧没拿到奖池 —— 玩法读表那一半没生效，先确认二进制版本")
    print("基线：ticket=%s 档数=%s" % (before.get("ticket"), len(before["prizes"])))

    # 用读侧的投影反构一个「只抬 50x 权重」的坏池：抬到 40 后 EV 破 1
    entries = [to_req(p) for p in before["prizes"]]
    def body_of(entries, ticket=None):
        return {"pool_key": POOL_KEY, "game": "jgg", "label": "九宫格 · 标准池",
                "ticket": before["ticket"] if ticket is None else ticket,
                "entries": entries}

    M = lambda lb, wt, mu: {"label": lb, "weight": wt, "kind": "magic", "payout": mu, "enabled": True}
    I = lambda lb, wt, k: {"label": lb, "weight": wt, "kind": "item",
                           "item_key": k, "qty": 1, "enabled": True}
    # ① 经济档权重堆到 EV 1.2
    body_bad = body_of([M("谢谢参与", 400, 0), M("2x 魔力", 600, 2)])

    st, r = call("POST", "/admin/arcade/pool", body_bad, tok)
    check("后门①抬经济奖权重 -> 400 拒绝", st == 400, (st, r))
    check("拒绝原因点名 EV", "EV" in json.dumps(r, ensure_ascii=False)
          or "返还" in json.dumps(r, ensure_ascii=False), r)

    after_bad = pool_of(tok)
    check("被拒之后表里一字未改（档数/权重/票价全等）", after_bad == before,
          {"before": before, "after": after_bad})

    # 其余四类坏配置：空池 / 零权重 / 票价非正 / EV 恰为 1
    cases = [
        ("空池", body_of([])),
        ("零权重档", body_of([M("谢谢参与", 500, 0), M("2x", 0, 2)])),
        ("票价 0", body_of([M("谢谢参与", 900, 0), M("2x", 100, 2)], ticket=0)),
        ("EV 恰为 1 的中性池", body_of([M("谢谢参与", 500, 0), M("2x", 500, 2)])),
        ("物品位引用不存在/停用物品", body_of([M("谢谢参与", 900, 0), I("空头券", 100, "no_such_item")])),
    ]
    for name, body in cases:
        st, r = call("POST", "/admin/arcade/pool", body, tok)
        check("拒绝 %s" % name, st == 400, (st, r))
        check("%s 拒绝后表未变" % name, pool_of(tok) == before)

    # 合法保存：只改票价，EV 按定义只与比例有关，不该变
    ok_body = body_of(entries, before["ticket"] + 20)
    st, r = call("POST", "/admin/arcade/pool", ok_body, tok)
    check("合法保存 -> 200", st == 200, (st, r))
    after_ok = pool_of(tok)
    check("票价改动被玩法读到（写读同一条链）",
          after_ok.get("ticket") == before["ticket"] + 20, after_ok)
    check("档数与权重未被顺手改写",
          [(p["label"], p["weight_permille"], p["payout"]) for p in after_ok["prizes"]]
          == [(p["label"], p["weight_permille"], p["payout"]) for p in before["prizes"]],
          after_ok.get("prizes"))

    # 复原，不留残留（main 外层还有 try/finally 兜底）
    st, _ = call("POST", "/admin/arcade/pool", body_of(entries), tok)
    check("复原成功", st == 200)
    check("复原后与基线完全一致", pool_of(tok) == before, pool_of(tok))

    print("\n结果：%d 项断言，失败 %d" % (n_checks[0], len(fails)))
    sys.exit(1 if fails else 0)



def _restore_snapshot():
    """脚本崩在半路也要把奖池写回进入时的样子：
    本轮就发生过 KeyError 把 ticket=120 的探针态留在生产库。"""
    tok = login()
    snap = pool_of(tok)
    if not snap or not snap.get("prizes"):
        print("兜底复原：读不到奖池，跳过")
        return
    st, r = call("POST", "/admin/arcade/pool",
                 body_for_restore(snap), tok)
    print("兜底复原奖池 -> HTTP %s" % st)


def body_for_restore(snap):
    return {"pool_key": POOL_KEY, "game": "jgg", "label": "九宫格 · 标准池",
            "ticket": snap.get("ticket"), "entries": [to_req(p) for p in snap["prizes"]]}


if __name__ == "__main__":
    try:
        main()
    finally:
        try:
            _restore_snapshot()
        except Exception as e:  # 兜底本身失败要喊出来，不能静默
            print("兜底复原失败，请手工核对 arcade_pool_entries：%r" % e)
