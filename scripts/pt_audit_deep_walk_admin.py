# -*- coding: utf-8 -*-
"""全站深测探针：邀请注册链 / 后台处置面 / 论坛动态。

从 pt_audit_deep_walk.py 拆出（500 行门禁）。这一支碰的是站方动作：发码→
注册→绑定→配额→奖励→撤销，以及后台列表/封禁/发放/软删/举报/群发/日志。
写侧带 deep_ 前缀并自清理。
"""
from __future__ import annotations

from pt_audit_deep_walk_base import *  # noqa: F401,F403
import sys

# ---------------------------------------------------------------- 域：invite2

def register_user(uname: str, code: str) -> tuple[int, object]:
    """注册新用户：先取算术验证码（provider=none 时必填）再提交。"""
    st, js = call("GET", "/api/v1/auth/captcha", "")
    cap = data_of(js)
    q = cap.get("question") or ""
    m = re.match(r"(\d+)\s*\+\s*(\d+)", q)
    ans = (int(m.group(1)) + int(m.group(2))) if m else 0
    return call("POST", "/api/v1/auth/register", "",
                body={"username": uname, "email": f"{uname}@example.com",
                      "password": "DeepTest#12345", "invite_code": code,
                      "captcha_id": cap.get("captcha_id") or "",
                      "captcha_answer": ans})


def d_invite2(vip: str, root: str) -> None:
    D = "invite2"
    # 域内自愈：并行会话/定时 job 会把探针用户洗回 class 1（连跑时实测），
    # 邀请等级闸（LV3）会正确拦截——那不是站点缺陷，是探针被洗
    # quota_extra 优先于等级校准且不读 5s 用户状态缓存；自动降级 job
    # 与自愈有秒级竞驰，只靠 class=9 会偶发 LV3 拦截（批次实测）。
    psql("UPDATE users SET class_id = 9, "
         "quota_extra = GREATEST(quota_extra, 1) WHERE id = 7346")
    psql("DELETE FROM invite_quota WHERE user_id = 7346")
    # 状态
    st, js = call("GET", "/api/v1/invites/status", vip)
    if not ok(D, "邀请状态", st, js):
        return
    unused0 = data_of(js).get("unused") or 0
    quota_used0 = data_of(js).get("quota_used") or 0

    # 申请一枚码（紧挨申请前再自愈一次：class_auto_adjust 按 stats 降级，
    # 与自愈存在秒级竞驰——被 LV3 拦下时补一次自愈重试）
    st, js = call("POST", "/api/v1/invites", vip, body={})
    if st != 200 and "LV3" in str(js.get("message", "")):
        psql("UPDATE users SET class_id = 9 WHERE id = 7346")
        st, js = call("POST", "/api/v1/invites", vip, body={})
    if not ok(D, "申请邀请码", st, js):
        return
    code = data_of(js).get("code") or data_of(js).get("invite", {}).get("code")
    expect(D, "拿到邀请码", bool(code), f"code={code}")
    if not code:
        return
    st, js = call("GET", "/api/v1/invites", vip)
    ok(D, "邀请列表在册", st, js)
    st, js = call("GET", "/api/v1/invites/status", vip)
    expect(D, "未用数 +1", (data_of(js).get("unused") or 0) == unused0 + 1,
           f"{unused0}→{data_of(js).get('unused')}")

    # 用码注册新用户。同 IP 24h 最多 2 个账号是站点的真实护闸，
    # 连跑时必然触顶——如实记 WARN 跳过依赖新用户的断言，不判站点红。
    uname = f"deep_inv_{int(time.time())}"
    st, js = register_user(uname, code)
    if st == 400 and "24 小时内最多注册" in str(js.get("message", "")):
        rec(D, "WARN", "被邀注册成功｜环境：本 IP 24h 注册配额已满")
        return
    if not ok(D, "被邀注册成功", st, js):
        return
    new_uid = data_of(js).get("id") or data_of(js).get("user_id")
    expect(D, "注册返回 uid", bool(new_uid), f"uid={new_uid}")

    # 关系绑定（API 面 + 库面 invited_by 双查）
    st, js = call("GET", "/api/v1/invites/invitees", vip)
    rows = data_of(js).get("items") or data_of(js)
    expect(D, "被邀者在册", any((r.get("username") == uname) or
                                (r.get("invitee", {}).get("username") == uname)
                                for r in (rows if isinstance(rows,
                                    list) else [])),
           f"n={len(rows) if isinstance(rows, list) else '?'}")
    if new_uid:
        ib = psql(f"SELECT invited_by FROM users WHERE id={new_uid}")
        expect(D, "被邀用户 invited_by == 邀请人", ib == str(VIP_UID),
               f"invited_by={ib}")

    # 码状态：已使用
    st, js = call("GET", "/api/v1/invites/status", vip)
    expect(D, "配额已用 +1",
        (data_of(js).get("quota_used") or 0) == quota_used0 + 1,
           f"{quota_used0}→{data_of(js).get('quota_used')}")

    # 码二次使用拒
    uname2 = f"deep_inv2_{int(time.time())}"
    st, js = register_user(uname2, code)
    expect(D, "邀请码二次使用拒", st in (400, 403, 409), f"st={st} {str(js)[:60]}")

    # 新用户可登录
    st, js = call("POST", "/api/v1/auth/login", "",
                  body={"username": uname, "password": "DeepTest#12345"})
    expect(D, "被邀新用户可登录", st == 200, f"st={st}")

    # 撤销一枚未用码
    st, js = call("POST", "/api/v1/invites", vip, body={})
    code2 = data_of(js).get("code")
    if code2:
        st, js = call("GET", "/api/v1/invites", vip)
        rows = data_of(js).get("items") or []
        inv2 = next((r for r in rows if r.get("code") == code2), None)
        if inv2 and inv2.get("id"):
            st, js = call("DELETE", f"/api/v1/invites/{inv2['id']}", vip)
            ok(D, "撤销未用邀请码", st, js)

    # 清理：删掉注册的深测用户（root 权限）
    if new_uid:
        psql(f"DELETE FROM users WHERE id={new_uid} AND username='{uname}'")


# ---------------------------------------------------------------- 域：admin2

def d_admin2(vip: str, root: str) -> None:
    D = "admin2"
    # 用户列表/详情
    st, js = call("GET", "/api/v1/admin/users?q=audit_vip", root)
    if not ok(D, "后台用户列表", st, js):
        return
    rows = rows_of(js)
    tgt = next((r for r in rows if r.get("username") == "audit_vip"), None)
    expect(D, "搜到探针用户", tgt is not None, f"n={len(rows)}")

    # 非 staff 拒入（权限面）
    st, _ = call("GET", "/api/v1/admin/users", vip)
    expect(D, "VIP 不进后台(403)", st == 403, f"st={st}")

    # 发放上传量：到账 + 流水
    up0 = int(psql(f"SELECT uploaded FROM users WHERE id={VIP_UID}") or 0)
    st, js = call("POST", "/api/v1/admin/amountupload", root,
                  body={"user_id": VIP_UID, "gb": 1,
                        "idempotency_key": f"deep_au_{int(time.time())}"})
    if ok(D, "发放上传量 1GB", st, js):
        up1 = int(psql(f"SELECT uploaded FROM users WHERE id={VIP_UID}") or 0)
        expect(D, "上传量到账", up1 - up0 == 1024**3, f"Δ={up1-up0}")
        st, js = call("GET", "/api/v1/admin/spark-logs?limit=5", root)
        ok(D, "后台流水可查", st, js)

    # 发放魔力：到账 + ledger
    sp0 = int(psql(f"SELECT COALESCE(spark_balance,0) FROM users WHERE id={VIP_UID}") or 0)
    st, js = call("POST", "/api/v1/admin/amountbonus", root,
                  body={"user_id": VIP_UID, "amount": 777,
                        "idempotency_key": f"deep_ab_{int(time.time())}"})
    if ok(D, "发放魔力 777", st, js):
        sp1 = int(psql(f"SELECT COALESCE(spark_balance,0) FROM users WHERE id={VIP_UID}") or 0)
        expect(D, "魔力到账", sp1 - sp0 == 777, f"Δ={sp1-sp0}")

    # 禁下载开关 → 恢复
    st, js = call("PUT", "/api/v1/admin/users/flags", root,
                  body={"user_id": VIP_UID, "download_enabled": False})
    ok(D, "禁下载开关", st, js)
    st, js = call("PUT", "/api/v1/admin/users/flags", root,
                  body={"user_id": VIP_UID, "download_enabled": True})
    ok(D, "恢复下载开关", st, js)

    # 封禁状态 → 登录阻断 → 恢复（先清 PoW/账户锁定/限流三把 Redis 键：
    # 历史失败累积会触发 PoW 闸回 400，与封禁语义混在一起）
    st, js = call("POST", "/api/v1/admin/users/status", root,
                  body={"user_id": VIP_UID, "status": 2, "reason": "深测封禁验证"})
    ok(D, "封禁用户", st, js)
    subprocess.run(["docker", "exec", "flux-redis", "redis-cli", "-a",
                    subprocess.run(["docker", "exec", "flux-redis", "printenv",
                                    "REDIS_PASSWORD"], capture_output=True,
                                   text=True).stdout.strip(), "--no-auth-warning",
                    "DEL", "acctlock:audit_vip"], capture_output=True)
    st, js = call("POST", "/api/v1/auth/login", "",
                  body={"username": "audit_vip", "password": "whatever"})
    # 口径：InvalidCredentials→HTTP 400 +「凭证无效」是**防枚举设计**
    # （封禁与密码错同口径，见 errors.rs 映射与 login.rs 注释）——
    # 不断言 401/403，断言「拒登录 + 不泄露封禁事实」
    expect(D, "封禁后登录拒(统一凭证无效口径)", st == 400 and
           "凭证无效" in str(js.get("message", "")), f"st={st} {str(js)[:60]}")
    st, js = call("POST", "/api/v1/admin/users/status", root,
                  body={"user_id": VIP_UID, "status": 0, "reason": "深测解封"})
    ok(D, "解封用户", st, js)

    # 种子管理：软删 → 回收站视图 → 恢复
    st, js = call("GET", "/api/v1/torrents?limit=1", root)
    items = data_of(js).get("items", [])
    if items:
        tid = items[0]["id"]
        st, js = call("DELETE", f"/api/v1/torrents/{tid}", root)
        ok(D, "软删种子", st, js)
        st, js = call("GET", f"/api/v1/torrents?approval=3&limit=50", root)
        expect(D, "回收站视图可见", st == 200 and
               any(r["id"] == tid for r in data_of(js).get("items", [])))
        st, js = call("POST", f"/api/v1/torrents/{tid}/restore", root, body={})
        ok(D, "恢复种子", st, js)

    # 举报：用户创建 + 后台列表（目标须 approval=1；restore 回 pending=0
    # 是防「删了再恢复绕过审核」的设计，故取一枚新鲜过审种）
    st, js = call("GET", "/api/v1/torrents?alive=0&limit=1", root)
    fresh = rows_of(js)
    if fresh:
        st, js = call("POST", "/api/v1/reports", vip,
                      body={"ref_type": "torrent", "ref_id": fresh[0]["id"],
                            "reason": "深测举报"})
        if ok(D, "用户提交举报", st, js):
            st, js = call("GET", "/api/v1/admin/reports?limit=10", root)
            expect(D, "后台举报列表", st == 200 and len(rows_of(js)) > 0)
    else:
        rec(D, "WARN", "无过审种可举报，跳过")

    # 设定读写（PUT 是单项写 {name, value}，键须已存在——不能新建键）
    st, js = call("GET", "/api/v1/admin/settings", root)
    if ok(D, "后台设定读取", st, js):
        srows = data_of(js).get("settings") or []
        pick = next((r for r in srows
                     if str(r.get("value", "")).isdigit()), None)
        if pick:
            key = pick["name"]
            orig = str(pick["value"])
            bumped = str(int(orig) + 1)
            st, js = call("PUT", "/api/v1/admin/settings", root,
                          body={"name": key, "value": bumped})
            ok(D, f"后台设定写入({key})", st, js)
            got = psql(f"SELECT value FROM site_settings WHERE name='{key}'")
            expect(D, "设定写入落库", got == bumped, f"{orig}→{got}")
            call("PUT", "/api/v1/admin/settings", root,
                 body={"name": key, "value": orig})
            got2 = psql(f"SELECT value FROM site_settings WHERE name='{key}'")
            expect(D, "设定还原", got2 == orig, f"now={got2}")
        else:
            rec(D, "WARN", "无数值型设定键可挑，写入测试跳过")

    # 群发 + 日志
    st, js = call("POST", "/api/v1/admin/massmail", root,
                  body={"subject": "deep_massmail", "body": "深测群发",
                        "class_ids": [9]})
    ok(D, "群发(管理信)", st, js)
    st, js = call("GET", "/api/v1/admin/syslog?limit=5", root)
    ok(D, "系统日志可查", st, js)
    st, js = call("GET", "/api/v1/admin/overview", root)
    ok(D, "后台总览", st, js)


# ---------------------------------------------------------------- 域：dlforum

def d_dlforum(vip: str, root: str) -> None:
    D = "dlforum"
    # ① 真实下载：download.php 是 RSS enclosure / 客户端拉种同款直链
    st, js = call("GET", "/api/v1/torrents?limit=1", vip)
    rows = rows_of(js)
    if rows:
        tid = rows[0]["id"]
        st, body = call("GET",
                        f"/api/v1/compat/nexusphp/download.php?id={tid}"
                        f"&passkey={VIP_PASSKEY}", vip, raw=True)
        expect(D, "download.php 200", st == 200, f"st={st}")
        is_torrent = (isinstance(body, bytes) and body.startswith(b"d")
                      and b"announce" in body[:100])
        expect(D, "返回合法 .torrent(bencode)", is_torrent)
        expect(D, "announce 内嵌 passkey", isinstance(body, bytes)
               and VIP_PASSKEY.encode() in body[:250])
        st2, _ = call("GET",
                      f"/api/v1/compat/nexusphp/download.php?id={tid}"
                      f"&passkey={'0' * 32}", vip, raw=True)
        # 401 也是合法拒绝（未认证口径，compat 下载口按鉴权失败处理）
        expect(D, "坏 passkey 下载拒", st2 in (400, 401, 403, 404), f"st={st2}")
    else:
        rec(D, "WARN", "无活种，下载链跳过")

    # ② 论坛深交互：发主题（!torrent 引用）→ 回帖 → 楼中楼 → 编辑 → 读回 → 删
    # （删帖是管理权限，作者本人 DELETE 吃 403，用 root——NP 同口径）
    subj = f"deep_forum_{int(time.time())}"
    st, js = call("POST", "/api/v1/forums/topics", vip,
                  body={"forum_id": 2, "title": subj,
                        "body": "深测论坛主题：正文含 !torrent 引用与普通文字。"
                                "长度足够通过论坛最低内容标准检查，补充若干说明文字。"})
    if not ok(D, "发主题", st, js):
        return
    topic_id = data_of(js).get("id") or data_of(js).get("topic_id")
    st, js = call("GET", f"/api/v1/forums/topics/{topic_id}", vip)
    expect(D, "读回主题", st == 200 and
           any(subj in str(p.get("title") or "") or
               subj in str(p.get("subject") or "")
               for p in ([data_of(js)] if isinstance(data_of(js),
                   dict) else [])),
           f"st={st}")
    # 回帖（论坛 flood 同族 10s 闸，与上一条发主题隔开）
    time.sleep(11)
    st, js = call("POST", f"/api/v1/forums/topics/{topic_id}/reply", vip,
                  body={"body": "深测回帖：一楼。"})
    if ok(D, "回帖", st, js):
        post_id = data_of(js).get("id") or data_of(js).get("post_id")
        if post_id:
            # 楼中楼
            time.sleep(11)
            st, js = call("POST", f"/api/v1/forums/topics/{topic_id}/reply",
                vip,
                          body={"body": "深测楼中楼回复。",
                                "reply_to": post_id})
            ok(D, "楼中楼回复", st, js)
            # 编辑自己的回帖
            time.sleep(1)
            st, js = call("PUT", f"/api/v1/forums/posts/{post_id}", vip,
                          body={"body": "深测回帖：一楼（已编辑）。"})
            ok(D, "编辑回帖", st, js)
            st, js = call("GET", f"/api/v1/forums/topics/{topic_id}", vip)
            expect(D, "编辑后读回生效", st == 200 and "已编辑" in str(js))
    # 作者本人删主题 → 403（管理权限）
    st, _ = call("DELETE", f"/api/v1/forums/topics/{topic_id}", vip)
    expect(D, "作者删主题 403(管理权限)", st == 403, f"st={st}")
    # root 删
    st, _ = call("DELETE", f"/api/v1/forums/topics/{topic_id}", root)
    expect(D, "版主删主题", st in (200, 204), f"st={st}")

    # ③ 做种收益到账（端到端）：探针用户有「可达+做种中」的 snatch 且
    # 无未决作弊事件时，手动跑 seeding_reward job → spark_ledger 到账。
    # 反作弊拦截（connectable=0 / 未决 ghost 事件）是设计行为，先模拟
    # 「管理员已处置 + 客户端可达」再验收益链本身。
    psql("UPDATE snatches SET connectable = 1 WHERE user_id = 7346 "
         "AND seeding AND last_port > 0")
    psql("UPDATE cheat_events SET resolved_at = now() "
         "WHERE user_id = 7346 AND resolved_at IS NULL")
    st, js = call("POST", "/api/v1/admin/jobs/run", root,
                  body={"job": "seeding_reward"})
    ok(D, "手动触发做种结算 job", st, js)
    wait_worker_tick(rounds=10, secs=8)
    led = psql("SELECT COALESCE(sum(amount),0) FROM spark_ledger "
               "WHERE user_id=7346 AND kind='seeding_reward'")
    expect(D, "做种收益入账(ledger)", float(led or 0) > 0, f"Σ={led}")
    # 余额断言不用「增量」——spark_balance 同 uploaded 是**快照列**
    # （= base_spark + Σspark_ledger，做种 job 每小时重算）。断言重算后的
    # 恒等式：余额 == 基线 + 流水合计（快照机制本身自洽）。
    base_sp = int(float(psql("SELECT COALESCE(base_spark,0) FROM balance_baseline "
                             "WHERE user_id=7346") or 0))
    led_all = int(float(psql("SELECT COALESCE(sum(amount),0) FROM spark_ledger "
                             "WHERE user_id=7346") or 0))
    sp1 = int(psql("SELECT COALESCE(spark_balance,0) FROM users WHERE id=7346") or 0)
    expect(D, "余额快照 == base_spark+Σledger", sp1 == base_sp + led_all,
           f"bal={sp1} base={base_sp} Σ={led_all}")


