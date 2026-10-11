# -*- coding: utf-8 -*-
"""批二十七闸门 AQ 公共层：psql 直连 / 探针账号（含回退注册日）/
worker job 双通道判据 / 附件（SHA3-256 口径）/ 悬赏池辅助。

单列成模块是行数门禁的既有做法（各闸门 helper 与断言分文件，
subtitles_* 在 api 侧同规矩按域拆）。
"""
import hashlib
import subprocess

from pt_audit_lib import call, login  # noqa: F401


def make_exec(CLEAN):
    """返回一组绑定同一 CLEAN 列表的辅助函数（收尾顺序=倒序执行）。"""

    def q(sql_text):
        p = subprocess.run(
            ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
             "-d", "fluxtorrent", "-tAc", sql_text],
            capture_output=True, text=True, encoding="utf-8")
        return p.stdout.strip()

    def exec_sql(sql_text):
        p = subprocess.run(
            ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
             "-d", "fluxtorrent", "-c", sql_text],
            capture_output=True, text=True, encoding="utf-8")
        if p.returncode:
            raise RuntimeError("psql 失败: %s" % (p.stderr or "")[:220])

    def setting(name, value):
        exec_sql("INSERT INTO site_settings (name, value) VALUES ('%s', "
                 "'%s') ON CONFLICT (name) DO UPDATE SET value = "
                 "EXCLUDED.value" % (name, value))

    def uid_of(name):
        return int(q("SELECT id::text FROM users WHERE username = '%s'"
                     % name))

    def mkuser(name, pw):
        """走 API 建号（含改密，口令最终为 pw+"x"）。"""
        call("POST", "/admin/adduser", {"username": name, "password": pw,
                                        "email": "%s@aq.local" % name},
             token=login())
        u = uid_of(name)
        tok = login(name, pw)
        call("POST", "/me/password/change",
             {"old_password": pw, "new_password": pw + "x"}, token=tok)
        _register_probe(u, q, exec_sql, CLEAN)
        return u, login(name, pw + "x")

    def mkbackdated(name, pw, days=30, cls=1, src_uid=None):
        """直插 + 回退注册日（API 建号做不到）。密码必须拷「本闸门已知
        口令的号」的 hash——拷 root 的原罪：root 口令与探针口令不同源，
        登录必 400（首轮先红就挂在这）。"""
        ph = q("SELECT pass_hash FROM users WHERE id = %d" % (src_uid or 0))
        if len(ph) < 20:
            raise RuntimeError("src_uid=%s 无可用 pass_hash" % src_uid)
        u = int(q("INSERT INTO users (username, email, pass_hash, class_id, "
                  "status, created_at, passkey) VALUES ('%s', '%s@aq.local',"
                  " '%s', %d, 0, now() - interval '%d days', "
                  "md5(random()::text)) RETURNING id"
                  % (name, name, ph, cls, days)))
        _register_probe(u, q, exec_sql, CLEAN)
        return u, login(name, pw)

    def add_attachment(u, content, fname):
        """服务端按 SHA3-256 记账；attachments.kind CHECK 只允许 file/video。"""
        sha = hashlib.sha3_256(content).hexdigest()
        exec_sql("INSERT INTO attachments (user_id, sha256, filename, mime,"
                 " size, kind) VALUES (%d, '%s', '%s', 'text/plain', %d, "
                 "'file')" % (u, sha, fname, len(content)))
        return sha

    def probe_torrent(owner_id, name):
        tid = int(q("INSERT INTO torrents (info_hash, name, size, owner_id,"
                    " category_id, approval_status) VALUES "
                    "(md5(random()::text), '%s', 1024, %d, 1, 1) "
                    "RETURNING id" % (name, owner_id)))
        # 拆序有讲究：subtitle_requests.fulfilled_subtitle_id 引用 subtitles、
        # subtitles.torrent_id 引用 torrents —— 先删求单再删字幕最后删种；
        # 且必须单语句（collect_milestones 会随时补 seed_milestones 行）。
        CLEAN.append("DELETE FROM seed_milestones WHERE torrent_id = %d; "
                     "DELETE FROM subtitle_requests WHERE torrent_id = %d "
                     "OR fulfilled_subtitle_id IN (SELECT id FROM subtitles "
                     "WHERE torrent_id = %d); "
                     "DELETE FROM snatches WHERE torrent_id = %d; "
                     "DELETE FROM subtitles WHERE torrent_id = %d; "
                     "DELETE FROM torrents WHERE id = %d"
                     % (tid, tid, tid, tid, tid, tid))
        return tid

    def list_row(path, tok, want_id):
        """读列表接口找某一行（search 参数需已 URL 编码）。返回
        (http_status, row_or_None)。"""
        s, r = call("GET", path, token=tok)
        body = r.get("data") or {}
        items = body if isinstance(body, list) else (body.get("items") or [])
        return s, next((x for x in items if x.get("id") == want_id), None)

    def pre_clean():
        """两道闸门（AQ 读写面 / AR 悬赏池面）共用的开跑自愈：
        上一轮中断留下的探针号与 Aq 前缀行全部按 FK 顺序拆净。"""
        stale = q("SELECT COALESCE(string_agg(id::text, ','), '0') FROM "
                  "users WHERE username ~ "
                  "'^aq(Vet|New|Me|C1|Ref|Ban)[0-9]+$'")
        if stale != "0":
            # 用 @U 占位再整体替换：一条语句里十几处同一个 id 列表，
            # % 元组数错一次就是整道闸门起不来（本批踩过）
            exec_sql((
                "DELETE FROM subtitle_votes WHERE user_id IN (@U); "
                "DELETE FROM reports WHERE reporter_id IN (@U); "
                "DELETE FROM messages WHERE receiver_id IN (@U) OR "
                "sender_id IN (@U); DELETE FROM user_subtitle_certs "
                "WHERE user_id IN (@U); DELETE FROM user_medals WHERE "
                "user_id IN (@U); DELETE FROM attachments WHERE user_id "
                "IN (@U); DELETE FROM spark_ledger WHERE user_id IN (@U); "
                "DELETE FROM subtitle_requests WHERE user_id IN (@U); "
                "DELETE FROM subtitles WHERE user_id IN (@U); "
                "DELETE FROM audit_log WHERE actor_id IN (@U) AND action "
                "LIKE 'sub%'; DELETE FROM shoutbox WHERE user_id IN (@U); "
                "UPDATE users SET status = 2, spark_balance = 0 "
                "WHERE id IN (@U)").replace("@U", stale))
        exec_sql("DELETE FROM subtitle_votes WHERE subtitle_id IN (SELECT id"
                 " FROM subtitles WHERE title LIKE 'Aq%'); "
                 "DELETE FROM subtitle_awards WHERE subtitle_id IN (SELECT "
                 "id FROM subtitles WHERE title LIKE 'Aq%'); "
                 "DELETE FROM subtitle_requests WHERE descr LIKE 'AqGate %' "
                 "OR fulfilled_subtitle_id IN (SELECT id FROM subtitles "
                 "WHERE title LIKE 'Aq%'); "
                 "DELETE FROM reports WHERE ref_type = 'subtitle'; "
                 "DELETE FROM seed_milestones WHERE torrent_id IN (SELECT id"
                 " FROM torrents WHERE name LIKE 'Aq%'); "
                 "DELETE FROM subtitles WHERE title LIKE 'Aq%'; "
                 "DELETE FROM torrents WHERE name LIKE 'Aq%'")
        return q("SELECT count(*) FROM subtitles")

    def login_refused(name, pw, wait_s=90):
        """断言「这个账号登不进来」。429 是登录限流，什么也没证明，
        必须重试到拿到确定答复为止（闸门连跑时 /auth/login 很容易吃 429，
        直接把它当成「被拒」是假绿，当成「失败」是假红）。"""
        import time
        end = time.time() + wait_s
        last = None
        while time.time() < end:
            last = call("POST", "/auth/login", {"username": name,
                                                "password": pw})
            if last[0] != 429:
                return last[0] != 200
            time.sleep(5)
        return False

    def queue_final_sweep():
        """把 Aq* 残料挂到 CLEAN 上收尾自清：只靠「下轮开跑自愈」会让
        本轮的『探针清零』断言必红（本批实测 2 行字幕残留）。"""
        CLEAN.append(
            "DELETE FROM subtitle_votes WHERE subtitle_id IN (SELECT id "
            "FROM subtitles WHERE title LIKE 'Aq%'); "
            "DELETE FROM subtitle_awards WHERE subtitle_id IN (SELECT id "
            "FROM subtitles WHERE title LIKE 'Aq%'); "
            "DELETE FROM subtitle_requests WHERE descr LIKE 'AqGate %' OR "
            "fulfilled_subtitle_id IN (SELECT id FROM subtitles WHERE "
            "title LIKE 'Aq%'); DELETE FROM reports WHERE ref_type = "
            "'subtitle'; DELETE FROM seed_milestones WHERE torrent_id IN "
            "(SELECT id FROM torrents WHERE name LIKE 'Aq%'); "
            "DELETE FROM subtitles WHERE title LIKE 'Aq%'; "
            "DELETE FROM torrents WHERE name LIKE 'Aq%'")

    def wait_row(sql_text, want, wait_s=150):
        """轮询到期望值为止：worker 全部 job 都是 60s tick，run_job 的
        回执可能对应「本轮动作之前」开始的那一拍（S9 gold 实测就撞上），
        一次性断言会把竞态写成假红。"""
        import time
        end = time.time() + wait_s
        got = None
        while time.time() < end:
            got = q(sql_text)
            if got == want:
                return True
            time.sleep(5)
        return False

    def wait_http(method, path, tok, want, body=None, wait_s=150):
        """轮询 HTTP 到期望状态：api 侧模块开关有缓存，关掉立刻生效、
        打开要等一拍（S10 实测）。"""
        import time
        end = time.time() + wait_s
        last = None
        while time.time() < end:
            last = call(method, path, body, token=tok)
            if last[0] == want:
                return True
            time.sleep(4)
        return False

    def run_job(job, wait_s=170):
        """双通道判据：模块拦下时 job_status 不动，只看 trigger 终态。"""
        t0 = q("SELECT to_char(now(), 'YYYY-MM-DD\"T\"HH24:MI:SS.US')")
        call("POST", "/admin/jobs/run", {"job": job}, token=login())
        import time
        deadline = time.time() + wait_s
        while time.time() < deadline:
            row = q("SELECT to_char(last_started_at, "
                    "'YYYY-MM-DD\"T\"HH24:MI:SS.US')||'|'||"
                    "COALESCE(last_ok::text,'-')||'|'||"
                    "LEFT(COALESCE(last_result,''),60) FROM job_status "
                    "WHERE job = '%s'" % job)
            if "|" in row:
                started, okv, res = row.split("|", 2)
                if started > t0:
                    return okv == "true", "started=%s %s" % (started, res)
            trow = q("SELECT status||'|'||COALESCE(ok::text,'-')||'|'||"
                     "LEFT(COALESCE(result,''),70) FROM job_triggers "
                     "WHERE job = '%s' AND requested_at >= "
                     "'%s'::timestamptz AND status IN ('done','failed') "
                     "ORDER BY id DESC LIMIT 1" % (job, t0))
            if "|" in trow:
                st, okv, res = trow.split("|", 2)
                return okv == "true", "trigger=%s %s" % (st, res)
            time.sleep(4)
        return None, "等 worker 本轮执行超时（%ss）" % wait_s

    def spark_of(u):
        return int(q("SELECT spark_balance::text FROM users WHERE id = %d"
                     % u))

    def grant_spark(u, amount, tag):
        """测试前置余额（演示号常为 0，冻结/入池需要本金）；收尾双撤。"""
        exec_sql("UPDATE users SET spark_balance = spark_balance + %d "
                 "WHERE id = %d" % (amount, u))
        CLEAN.append("UPDATE users SET spark_balance = spark_balance - %d "
                     "WHERE id = %d" % (amount, u))
        CLEAN.append("DELETE FROM spark_ledger WHERE user_id = %d AND "
                     "idempotency_key = 'aq-grant:%s'" % (u, tag))

    def ledger_rows(u, kind):
        return int(q("SELECT count(*) FROM spark_ledger WHERE user_id = %d "
                     "AND kind = '%s'" % (u, kind)))

    def cleanup():
        for stmt in reversed(CLEAN):
            try:
                exec_sql(stmt)
            except Exception as e:  # noqa: BLE001
                print("  [cleanup-skip] %s: %s"
                      % (stmt[:60], str(e)[:120]))

    return {"q": q, "exec_sql": exec_sql, "setting": setting,
            "uid_of": uid_of, "mkuser": mkuser, "mkbackdated": mkbackdated,
            "add_attachment": add_attachment, "probe_torrent": probe_torrent,
            "run_job": run_job, "spark_of": spark_of,
            "grant_spark": grant_spark, "ledger_rows": ledger_rows,
            "wait_row": wait_row, "wait_http": wait_http,
            "list_row": list_row, "pre_clean": pre_clean,
            "login_refused": login_refused,
            "queue_final_sweep": queue_final_sweep,
            "cleanup": cleanup}


def _register_probe(u, q, exec_sql, CLEAN):
    """探针号的副作用面：投票/流水/附件删净 + 余额回原值 + 停用。"""
    bal = q("SELECT spark_balance::text FROM users WHERE id = %d" % u)
    CLEAN.append("DELETE FROM subtitle_votes WHERE user_id = %d" % u)
    CLEAN.append("DELETE FROM spark_ledger WHERE user_id = %d" % u)
    CLEAN.append("DELETE FROM attachments WHERE user_id = %d" % u)
    CLEAN.append("DELETE FROM reports WHERE reporter_id = %d" % u)
    CLEAN.append("DELETE FROM messages WHERE receiver_id = %d OR "
                 "sender_id = %d" % (u, u))
    CLEAN.append("DELETE FROM user_subtitle_certs WHERE user_id = %d" % u)
    CLEAN.append("DELETE FROM user_medals WHERE user_id = %d" % u)
    CLEAN.append("UPDATE users SET spark_balance = %s, status = 2 "
                 "WHERE id = %d" % (bal, u))
