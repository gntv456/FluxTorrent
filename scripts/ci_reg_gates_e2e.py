# -*- coding: utf-8 -*-
"""CI 用：M4 注册治理四道闸变更型端到端（0370/0371）。

覆盖：一次性邮箱（含子域后缀匹配）→ SMTP 配了但连不上时不建号/不假发
→ 同 IP 限额（摘要留痕、无明文 IP）→ 注册窗口（已结束/未开始/名额满）
→ P-689 邀请码不受窗口/同 IP 限额约束、预约窗口到点对外投影 open、
slots=0 不下发剩余额度 → 剩 1 个名额时并发注册不超发 → maxusers 上限
（事务内 advisory 锁）→ 域名库 admin API（导入去重/导出/删除）→
枚举设定对象形选项校验 → 普通用户 403 → reg_cleanup cron。

副作用：临时改 5 个 settings 键、注册 1~5 个测试账号，结束全部还原/墓碑化
（audit_log 0266 哈希链禁改删 + FK 无 ON DELETE，硬删必炸——见 ci_reg_common）。
退出码非 0 = 失败。前提：api + worker 已起（worker 供 reg_cleanup 触发）。
"""
import hashlib
import json
import sys
import threading
import time

from ci_reg_common import (
    call, captcha, check, clear_throttle, data_of, jwt, leftover, msg,
    nowmark, psql, register, report, restore_settings, set_setting,
    tombstone,
)

MARK = "m4gate"
NEW_DOMAIN = "m4smoke-test.example"
# 与 api auth_http/register_gates.rs::ip_hash 同算法（sha3-256 hex）
H_LOCAL = hashlib.sha3_256(b"127.0.0.1").hexdigest()
SETTINGS = ("registration_mode", "reg_ip_limit", "reg_ip_window_hours",
            "reg_window", "maxusers", "captcha_provider",
            "application_signup", "smtp_host", "smtp_port", "smtp_from",
            "encryption", "accountname", "accountpassword")


def make_invite(uid: int, code: str) -> None:
    psql(f"INSERT INTO invites (inviter_id, code, expires_at) VALUES "
         f"({uid}, '{code}', now() + interval '1 day')")


def drop_invites(uid: int, mark: str) -> None:
    psql(f"DELETE FROM invites WHERE inviter_id = {uid} AND "
         f"btrim(code) LIKE '{mark}%'")


def burst_register(names: list) -> list:
    """同一瞬间并发注册（各自带独立验证码），返回 [(status, body)]。"""
    caps = [captcha() for _ in names]
    barrier = threading.Barrier(len(names))
    out: list = []

    def one(i: int, name: str) -> None:
        cid, ans = caps[i]
        payload = {"username": name, "email": f"{name}@example.com",
                   "password": "Passw0rd!ci", "invite_code": "",
                   "captcha_id": cid, "captcha_answer": ans,
                   "fields": {}}
        barrier.wait()
        out.append(call("POST", "/api/v1/auth/register", None, payload))

    ths = [threading.Thread(target=one, args=(i, n))
           for i, n in enumerate(names)]
    for t in ths:
        t.start()
    for t in ths:
        t.join()
    return out


def run_cron_once(tok: str) -> bool:
    """触发 reg_cleanup 并轮询 job_triggers 至 done（约 1 分钟上限）。"""
    for _ in range(2):
        st, r = call("POST", "/api/v1/admin/jobs/run", tok,
                     {"job": "reg_cleanup"})
        tid = data_of(r).get("id") if st == 200 else None
        if not tid:
            time.sleep(3)
            continue
        for _ in range(15):
            time.sleep(4)
            s = psql(f"SELECT status FROM job_triggers WHERE id = {tid}")
            if s == "done":
                return True
            if s == "failed":
                return False
    return False


def main() -> int:
    print("== 0. 备份设置 + 清理残留 ==")
    saved = {n: psql(f"SELECT value FROM site_settings WHERE name='{n}'")
             for n in SETTINGS}
    tombstone("zzdel_" + MARK)
    psql(f"DELETE FROM disposable_email_domains WHERE domain = "
         f"'{NEW_DOMAIN}' OR domain LIKE '%.{NEW_DOMAIN}'; "
         f"DELETE FROM reg_ip_signups WHERE ip_hash = '{H_LOCAL}'")
    set_setting("registration_mode", "open")
    set_setting("reg_window", "")
    uid = int(psql("SELECT id FROM users WHERE class_id >= 90 "
                   "ORDER BY id LIMIT 1") or 0)
    if uid:
        drop_invites(uid, MARK)
    t_root = jwt(uid) if uid else "jwt-缺失"
    check("解析 staff uid", uid > 0, f"uid={uid}")

    ts = nowmark()
    print("== 1. 一次性邮箱闸（含子域后缀匹配）==")
    psql(f"INSERT INTO disposable_email_domains (domain, source) VALUES "
         f"('{NEW_DOMAIN}', 'import') ON CONFLICT DO NOTHING")
    clear_throttle()
    st, r = register(f"{MARK}disp{ts}", f"abc@{NEW_DOMAIN}")
    check("一次性邮箱被拒", st == 400 and "一次性" in msg(r),
          f"HTTP {st} {msg(r)}")
    clear_throttle()
    st, r = register(f"{MARK}disp2{ts}", f"abc@mail.{NEW_DOMAIN}")
    check("子域命中拒绝", st == 400 and "一次性" in msg(r),
          f"HTTP {st} {msg(r)}")

    print("== 1b. SMTP 配置了但连不上：不建号、不假发 ==")
    set_setting("registration_mode", "email_verify")
    set_setting("smtp_host", "127.0.0.1")
    set_setting("smtp_port", "1")
    set_setting("smtp_from", "noreply@example.test")
    set_setting("encryption", "none")
    set_setting("accountname", "")
    set_setting("accountpassword", "")
    clear_throttle()
    before_users = psql("SELECT count(*) FROM users")
    st, r = register(f"{MARK}mail{ts}", f"mail{ts}@example.com")
    after_users = psql("SELECT count(*) FROM users")
    check("邮件不可达时拒绝建号（503/4103）",
          st == 503 and (r or {}).get("code") == 4103,
          f"HTTP {st} {msg(r)}")
    check("失败不留进不去的账号", before_users == after_users,
          f"{before_users} -> {after_users}")
    clear_throttle()
    st, r = call("POST", "/api/v1/auth/email/resend", None,
                 {"email": f"mail{ts}@example.com"})
    check("重发不假报 sent:true（503/4103）",
          st == 503 and (r or {}).get("code") == 4103,
          f"HTTP {st} {msg(r)}")
    set_setting("registration_mode", "open")
    set_setting("smtp_host", "")
    set_setting("smtp_from", "")

    print("== 2. 同 IP 限额闸 ==")
    set_setting("reg_ip_limit", "1")
    clear_throttle()
    st, r = register(f"{MARK}one{ts}", f"one{ts}@example.com")
    uid1 = data_of(r).get("user_id") if st == 200 else None
    check("首个注册成功（限额 1）", st == 200, f"HTTP {st} {msg(r)}")
    if uid1:
        row = psql("SELECT length(ip_hash) || '|' || "
                   "(ip_hash ~ '^[0-9a-f]{64}$')::text "
                   f"FROM reg_ip_signups WHERE user_id = {uid1}")
        check("摘要留痕（64 位 hex，无明文 IP）", row == "64|true", row)
    # 容器端口映射下 client_ip 是网桥网关，不是宿主的 127.0.0.1；
    # 后续按本次真实留痕取哈希，闸门才能同时跑在宿主机/容器/CI。
    ip_hash = (psql("SELECT ip_hash FROM reg_ip_signups WHERE "
                    f"user_id = {uid1}") if uid1 else "") or H_LOCAL
    clear_throttle()
    st, r = register(f"{MARK}two{ts}", f"two{ts}@example.com")
    check("同 IP 第 2 个被拒", st == 400 and "网络环境" in msg(r),
          f"HTTP {st} {msg(r)}")

    print("== 3. 注册窗口闸 ==")
    set_setting("reg_ip_limit", "0")
    set_setting("reg_window", '{"open":"2020-01-01T00:00:00Z",'
                              '"close":"2020-01-02T00:00:00Z","slots":0,'
                              '"label":"ci 旧窗口"}')
    clear_throttle()
    st, r = register(f"{MARK}w1{ts}", f"w1{ts}@example.com")
    check("窗口已结束被拒", st == 400 and "结束" in msg(r),
          f"HTTP {st} {msg(r)}")
    set_setting("reg_window", '{"open":"2099-01-01T00:00:00Z",'
                              '"close":"2099-01-02T00:00:00Z","slots":0,'
                              '"label":"ci 未来窗口"}')
    clear_throttle()
    st, r = register(f"{MARK}w2{ts}", f"w2{ts}@example.com")
    check("窗口未开始被拒", st == 400 and "尚未开始" in msg(r),
          f"HTTP {st} {msg(r)}")
    set_setting("reg_window", '{"open":"2020-01-01T00:00:00Z",'
                              '"close":"2099-01-01T00:00:00Z","slots":1,'
                              '"label":"ci 名额窗口"}')
    clear_throttle()
    st, r = register(f"{MARK}w3{ts}", f"w3{ts}@example.com")
    check("名额已满被拒", st == 400 and "名额已满" in msg(r),
          f"HTTP {st} {msg(r)}")

    print("== 3b. P-689 邀请码不受窗口/同 IP 限额约束 ==")
    set_setting("registration_mode", "invite_only")
    set_setting("reg_window", '{"open":"2099-01-01T00:00:00Z",'
                              '"close":"2099-01-02T00:00:00Z","slots":0,'
                              '"label":"ci 预约窗口"}')
    c_a = f"{MARK}ia{ts}"
    make_invite(uid, c_a)
    clear_throttle()
    st, r = register(f"{MARK}i1{ts}", f"i1{ts}@example.com",
                     {"invite_code": c_a})
    check("窗口未开始时邀请注册放行", st == 200, f"HTTP {st} {msg(r)}")
    set_setting("reg_window", '{"open":"2020-01-01T00:00:00Z",'
                              '"close":"2099-01-01T00:00:00Z","slots":1,'
                              '"label":"ci 满额窗口"}')
    c_b = f"{MARK}ib{ts}"
    make_invite(uid, c_b)
    clear_throttle()
    st, r = register(f"{MARK}i2{ts}", f"i2{ts}@example.com",
                     {"invite_code": c_b})
    check("名额已满时邀请注册放行", st == 200, f"HTTP {st} {msg(r)}")
    set_setting("reg_window", '{"open":"2020-01-01T00:00:00Z",'
                              '"close":"2099-01-01T00:00:00Z","slots":0,'
                              '"label":"ci 无限额窗口"}')
    st, r = call("GET", "/api/v1/register-mode", None)
    d = data_of(r)
    win = d.get("reg_window") or {}
    check("预约窗口到点：invite_only 对外投影 open",
          d.get("mode") == "open", f"mode={d.get('mode')}")
    check("slots=0 不下发 slots_left（不泄漏 i64::MAX）",
          "slots_left" not in win, f"win={win}")
    set_setting("registration_mode", "open")
    set_setting("reg_window", "")
    used = int(psql(
        "SELECT count(*) FROM reg_ip_signups s JOIN users u "
        "ON u.id = s.user_id WHERE u.invited_by IS NULL AND "
        f"s.ip_hash = '{ip_hash}' AND "
        "s.created_at > now() - interval '24 hours'"))
    set_setting("reg_ip_limit", str(used + 1))
    clear_throttle()
    st, r = register(f"{MARK}ip1{ts}", f"ip1{ts}@example.com")
    check("限额内直接注册成功", st == 200, f"HTTP {st} {msg(r)}")
    clear_throttle()
    st, r = register(f"{MARK}ip2{ts}", f"ip2{ts}@example.com")
    check("超限直接注册被拒（正控）",
          st == 400 and "网络环境" in msg(r), f"HTTP {st} {msg(r)}")
    c_c = f"{MARK}ic{ts}"
    make_invite(uid, c_c)
    clear_throttle()
    st, r = register(f"{MARK}ip3{ts}", f"ip3{ts}@example.com",
                     {"invite_code": c_c})
    check("超限时邀请注册放行（文案承诺的出路）",
          st == 200, f"HTTP {st} {msg(r)}")
    drop_invites(uid, MARK)
    set_setting("reg_ip_limit", "0")

    print("== 3c. 剩 1 个名额并发注册不超发 ==")
    open_ts = time.strftime("%Y-%m-%dT%H:%M:%SZ",
                            time.gmtime(time.time() - 600))
    used = int(psql(
        "SELECT count(*) FROM users WHERE created_at >= "
        f"'{open_ts}' AND invited_by IS NULL"))
    window = json.dumps({"open": open_ts,
                         "close": "2099-01-01T00:00:00Z",
                         "slots": used + 1, "label": "ci 并发名额"})
    set_setting("reg_window", window)
    clear_throttle()
    names = [f"{MARK}cc{i}{ts}" for i in range(3)]
    out = burst_register(names)
    okn = sum(1 for st, _ in out if st == 200)
    made = psql("SELECT count(*) FROM users WHERE username LIKE "
                f"'{MARK}cc%'")
    check("剩 1 个名额 3 并发只放过 1 个", okn == 1 and made == "1",
          f"成功 {okn} 落库 {made} "
          f"{[st for st, _ in out]}")

    print("== 4. 用户总量闸（maxusers）==")
    set_setting("reg_window", "")
    used = psql("SELECT count(*) FROM users WHERE status < 2")
    set_setting("maxusers", used)
    clear_throttle()
    st, r = register(f"{MARK}cap{ts}", f"cap{ts}@example.com")
    check("达上限被拒", st == 400 and "上限" in msg(r),
          f"HTTP {st} {msg(r)}（现役 {used}）")

    print("== 5. 域名库 admin API ==")
    st, r = call("GET", "/api/v1/admin/email-domains?q=mailinator", t_root)
    items = data_of(r).get("items") or []
    check("列表可查（含内置库）", st == 200 and len(items) > 0,
          f"HTTP {st} n={len(items)}")
    st, r = call("POST", "/api/v1/admin/email-domains/import", t_root,
                 {"domains": [NEW_DOMAIN, "不是域名", "x"],
                  "text": f"y.{NEW_DOMAIN}"})
    d = data_of(r)
    check("导入：合法入库 / 非法点名",
          st == 200 and d.get("invalid", 0) >= 2,
          f"added={d.get('added')} invalid={d.get('invalid')}")
    st, r = call("GET", f"/api/v1/admin/email-domains?q={NEW_DOMAIN}",
                 t_root)
    check("导入后可查（含子域行）",
          len(data_of(r).get("items") or []) >= 2)
    st, r = call("GET", "/api/v1/admin/email-domains/export", t_root)
    doms = data_of(r).get("domains") or []
    check("导出含内置+导入",
          NEW_DOMAIN in doms and "mailinator.com" in doms,
          f"count={len(doms)}")
    st, r = call("POST", "/api/v1/admin/email-domains/delete", t_root,
                 {"domains": [NEW_DOMAIN, f"y.{NEW_DOMAIN}"]})
    check("删除 2 行", st == 200 and data_of(r).get("deleted") == 2,
          f"HTTP {st}")

    print("== 5b. 枚举设定保存校验（对象形选项）==")
    st, r = call("PUT", "/api/v1/admin/settings/groups", t_root,
                 {"values": {"captcha_provider": "bogus"}})
    check("captcha_provider 非法值被拒", st == 400, f"HTTP {st} {msg(r)}")
    set_setting("captcha_provider", saved["captcha_provider"] or "none")
    st, r = call("PUT", "/api/v1/admin/settings/groups", t_root,
                 {"values": {"application_signup": "bogus"}})
    check("application_signup 非法值被拒", st == 400,
          f"HTTP {st} {msg(r)}")
    set_setting("application_signup", saved["application_signup"] or "off")

    print("== 6. 无权限拒绝 ==")
    if uid1:
        st, r = call("GET", "/api/v1/admin/email-domains", jwt(uid1, 1))
        check("普通用户 403", st == 403, f"HTTP {st}")
    else:
        check("普通用户 403", False, "第 2 步未拿到 uid1")

    print("== 7. 注册摘要清理 cron ==")
    psql("INSERT INTO reg_ip_signups (ip_hash, user_id, created_at) "
         f"VALUES ('{'0' * 64}', NULL, now() - interval '40 days')")
    done = run_cron_once(t_root) if uid else False
    left = psql("SELECT count(*) FROM reg_ip_signups WHERE ip_hash = "
                f"'{'0' * 64}'")
    check("40 天前摘要被清理", done and left == "0",
          f"trigger={done} 残留={left}")

    print("== 8. 还原设置 + 墓碑化测试账号 ==")
    drop_invites(uid, MARK)
    restore_settings(saved)
    tombstone(MARK)
    psql(f"DELETE FROM disposable_email_domains WHERE domain = "
         f"'{NEW_DOMAIN}' OR domain LIKE '%.{NEW_DOMAIN}'; "
         f"DELETE FROM reg_ip_signups WHERE ip_hash = '{H_LOCAL}'")
    check("测试账号清理归零", leftover(MARK) == "0",
          f"残留 {leftover(MARK)}")
    back = psql("SELECT value FROM site_settings WHERE name = "
                "'registration_mode'")
    check("registration_mode 已还原", back == saved["registration_mode"],
          back)
    return report()


if __name__ == "__main__":
    sys.exit(main())
