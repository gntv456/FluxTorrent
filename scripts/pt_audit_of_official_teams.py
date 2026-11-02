# -*- coding: utf-8 -*-
"""闸门 OF：官种派生的唯一口径（0449 名单 + 0450 team 维 + 0451 时机补全
+ 0453 按标题派生制作组）。

为什么要有这一支：官种是「一个物化列被四处当事实尺子读」的典型承重面
（促销 scope、发布自动置顶、首页统计、NexusPHP 兼容口），2026-11-01~02 新长
了两条派生腿（站长可配的名单 + 种子挂的制作组），却一条常驻判据都没有。
「列与关联表漂移」在本仓有前科（0044），派生收进 SQL 函数之后必须有人盯着
三件事：函数还在、口径只有一份、写表时机都触发得到。

判据分四层：
  S1 在册：两个函数 + site_settings 行 + settings_meta 的 csv_ids 规则 +
           触发器名册（torrent_sections 两条、site_settings 一到两条）。
  S2 单源：Rust 侧不许再出现就地派生（`SET official_tag = EXISTS…`），
           只许调 SQL 函数；并带一条正控——同一把尺在迁移目录里必须抓得到，
           否则「零命中」是尺坏了不是代码干净。
  S3 行为：BEGIN…ROLLBACK 内跑完整流转，每步要读数（挂组/改出/改回/撤组/
           脏名单/清空名单），并核对全站官种数随动。
  S4 账面：回滚后官种数、名单值、夹具行必须回到开局。

0451 / 0453 未应用时（api 还没重启）S3i「插行/删行也要重算」与
S6b-d「新种按标题自动挂组」记 PENDING，不当绿也不当红——那是待部署的本体。
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
os.chdir(os.path.dirname(HERE))          # 判据按仓库根路径扫源码
sys.path.insert(0, HERE)
from pt_audit_lib import grep_count, ok, psql_stdin, summary  # noqa: E402

RS = ["apps/api/src"]
SQLDIR = "apps/api/migrations"


def q(sql):
    return psql_stdin(sql)


def eq(label, got, want, extra=""):
    ok(label + "（实际=%s 期望=%s%s）" % (got, want, extra),
       str(got) == str(want))


print("== S1 机制在册 ==")
eq("S1a 派生函数两个都在",
   q("SELECT count(*) FROM pg_proc WHERE proname IN "
     "('refresh_official_tag','official_team_ids_arr')"), "2")
eq("S1b site_settings.official_team_ids 行在册",
   q("SELECT count(*) FROM site_settings WHERE name='official_team_ids'"), "1")
eq("S1c settings_meta 登记 text + csv_ids 规则（保存口才校验得住）",
   q("SELECT count(*) FROM settings_meta WHERE name='official_team_ids' "
     "AND type='text' AND options->>'rule'='csv_ids'"), "1")
eq("S1d torrent_sections 上 0450 的两条触发器（INS/UPD 与 DEL 分开）",
   q("SELECT count(*) FROM pg_trigger WHERE NOT tgisinternal "
     "AND tgrelid='torrent_sections'::regclass"), "2")
tri_set = q("SELECT coalesce(string_agg(tgname, ',' ORDER BY tgname),'(空)') "
            "FROM pg_trigger WHERE NOT tgisinternal "
            "AND tgrelid='site_settings'::regclass")
HAS_451 = ("insupd" in tri_set and "_del" in tri_set)
print("   site_settings 触发器名册：%s" % tri_set)
print("   0451（增/删也重算）：%s" % ("已应用" if HAS_451 else "未应用，待 api 重启"))

print("== S2 派生口径只有一份 ==")
inline = grep_count(r"SET\s+official_tag\s*=\s*EXISTS", RS)
eq("S2a Rust 侧没有第二份就地派生", inline, 0)
# 正控：同一把尺必须能在迁移目录抓到那句——否则「零命中」是尺坏不是干净
in_sql = grep_count(r"SET official_tag = EXISTS", [SQLDIR])
eq("S2b 正控：尺子抓得到 SQL 函数里那份派生", in_sql >= 1, True)
calls = grep_count(r"SELECT refresh_official_tag", RS)
eq("S2c Rust 侧确实在调那个函数（不是「函数在册但没人用」的假开关）",
   calls >= 1, True)
eq("S2d 0451 迁移文件在册",
   os.path.exists(os.path.join(SQLDIR,
                               "0451_official_teams_trigger_coverage.sql")),
   True)
# 读侧四处都拿这一列当尺子（口径漂移的受害面），钉一下没被人改成平行谓词
readers = grep_count(r"official_tag", RS)
eq("S2e 读侧仍在读这一列（promo/home/nexusphp/列表筛选都在）", readers >= 8, True)

print("== S3 行为流转（同一事务，末尾 ROLLBACK 不留痕）==")
BASE = q("SELECT count(*) FROM torrents WHERE official_tag")
LIST0 = q("SELECT coalesce((SELECT value FROM site_settings "
          "WHERE name='official_team_ids'),'(无行)')")
print("   基线：官种=%s 名单=[%s]" % (BASE, LIST0))

SCRIPT = """
BEGIN;
CREATE TEMP TABLE res (k text, v text);
CREATE TEMP TABLE fx AS
SELECT (SELECT t.id FROM torrents t
         WHERE NOT t.official_tag AND t.approval_status = 1
           AND NOT EXISTS (SELECT 1 FROM torrent_sections s
                           WHERE s.torrent_id = t.id AND s.kind='team')
         ORDER BY t.id LIMIT 1) AS tor,
       (SELECT min(id) FROM section_dict WHERE kind='team') AS team_in,
       (SELECT max(id) FROM section_dict WHERE kind='team') AS team_out,
       (SELECT count(*) FROM torrents WHERE official_tag) AS base;
UPDATE site_settings SET value = (SELECT team_in::text FROM fx)
 WHERE name = 'official_team_ids';
INSERT INTO res SELECT 's1_名单有值但未挂组', official_tag::text
  FROM torrents WHERE id = (SELECT tor FROM fx);
INSERT INTO torrent_sections (torrent_id, kind, dict_id, value, ordinal)
  SELECT tor, 'team', team_in, NULL, 99 FROM fx;
INSERT INTO res SELECT 's2_挂进名单组', official_tag::text
  FROM torrents WHERE id = (SELECT tor FROM fx);
INSERT INTO res SELECT 's2b_全站官种数', count(*)::text
  FROM torrents WHERE official_tag;
UPDATE torrent_sections SET dict_id = (SELECT team_out FROM fx)
  WHERE torrent_id = (SELECT tor FROM fx) AND kind='team' AND ordinal=99;
INSERT INTO res SELECT 's3_把组改出名单', official_tag::text
  FROM torrents WHERE id = (SELECT tor FROM fx);
UPDATE torrent_sections SET dict_id = (SELECT team_in FROM fx)
  WHERE torrent_id = (SELECT tor FROM fx) AND kind='team' AND ordinal=99;
INSERT INTO res SELECT 's4_再改回名单组', official_tag::text
  FROM torrents WHERE id = (SELECT tor FROM fx);
UPDATE site_settings SET value = 'abc, ,999999,' || (SELECT team_in FROM fx)
 WHERE name = 'official_team_ids';
INSERT INTO res SELECT 's5_脏名单仍认合法项', official_tag::text
  FROM torrents WHERE id = (SELECT tor FROM fx);
DELETE FROM torrent_sections
  WHERE torrent_id = (SELECT tor FROM fx) AND kind='team' AND ordinal=99;
INSERT INTO res SELECT 's6_撤组后', official_tag::text
  FROM torrents WHERE id = (SELECT tor FROM fx);
UPDATE site_settings SET value = '' WHERE name = 'official_team_ids';
INSERT INTO res SELECT 's7_清空名单后全站官种', count(*)::text
  FROM torrents WHERE official_tag;
SELECT k || '=' || v FROM res ORDER BY k;
ROLLBACK;
"""
rows = {}
for ln in psql_stdin(SCRIPT).splitlines():
    if "=" in ln:
        k, v = ln.split("=", 1)
        rows[k] = v
print("   读数：%s" % rows)
eq("S3a 名单有值但种子没挂组 → 不是官种",
   rows.get("s1_名单有值但未挂组"), "false")
eq("S3b 挂进名单内的制作组 → 官种（0450 的 INSERT 触发）",
   rows.get("s2_挂进名单组"), "true")
eq("S3c 全站官种数随之 +1（列与账面一起动）", rows.get("s2b_全站官种数"),
   int(BASE) + 1)
eq("S3d 把组改出名单 → 掉回非官种（0450 的 UPDATE 触发）",
   rows.get("s3_把组改出名单"), "false")
eq("S3e 再改回名单内 → 又是官种", rows.get("s4_再改回名单组"), "true")
eq("S3f 脏名单（非数字/越界 token）不报错且仍认合法项",
   rows.get("s5_脏名单仍认合法项"), "true")
eq("S3g 撤掉制作组 → 掉回非官种（0450 的 DELETE 触发）",
   rows.get("s6_撤组后"), "false")
eq("S3h 名单清空 → 全站官种回到基线（只认标签）",
   rows.get("s7_清空名单后全站官种"), BASE)

print("== S3i 三种写表时机（0451 补的两条）==")
if HAS_451:
    r2 = {}
    for ln in psql_stdin("""
BEGIN;
CREATE TEMP TABLE res2 (k text, v text);
CREATE TEMP TABLE fx2 AS
SELECT (SELECT t.id FROM torrents t
         WHERE NOT t.official_tag AND t.approval_status = 1
           AND NOT EXISTS (SELECT 1 FROM torrent_sections s
                           WHERE s.torrent_id = t.id AND s.kind='team')
         ORDER BY t.id LIMIT 1) AS tor,
       (SELECT min(id) FROM section_dict WHERE kind='team') AS team;
INSERT INTO torrent_sections (torrent_id, kind, dict_id, value, ordinal)
  SELECT tor, 'team', team, NULL, 99 FROM fx2;
DELETE FROM site_settings WHERE name = 'official_team_ids';
INSERT INTO site_settings (name, value, grp)
  SELECT 'official_team_ids', team::text, 'torrent' FROM fx2;
INSERT INTO res2 SELECT 'ins', official_tag::text FROM torrents
  WHERE id = (SELECT tor FROM fx2);
DELETE FROM site_settings WHERE name = 'official_team_ids';
INSERT INTO res2 SELECT 'del', official_tag::text FROM torrents
  WHERE id = (SELECT tor FROM fx2);
SELECT k || '=' || v FROM res2 ORDER BY k;
ROLLBACK;
""").splitlines():
        if "=" in ln:
            k, v = ln.split("=", 1)
            r2[k] = v
    eq("S3i-1 INSERT 名单行也重算（0449 的 AFTER UPDATE 抓不到）",
       r2.get("ins"), "true")
    eq("S3i-2 DELETE 名单行也重算（否则删行后列值永远挂着官种）",
       r2.get("del"), "false")
else:
    print("   PENDING：0451 未应用，S3i 两条不计入"
          "（api 重启跑完迁移后自动变成判据）")

print("== S4 回滚不留痕 ==")
eq("S4a 官种数回到开局", q("SELECT count(*) FROM torrents WHERE official_tag"),
   BASE)
eq("S4b 名单值回到开局（空串=只认标签）",
   "[" + q("SELECT coalesce((SELECT value FROM site_settings "
            "WHERE name='official_team_ids'),'(无行)')") + "]",
   "[" + LIST0 + "]")
eq("S4c 夹具行（ordinal=99）一条不留",
   q("SELECT count(*) FROM torrent_sections WHERE ordinal=99"), "0")

print("== S6 按标题派生制作组（0453，名单「有货」的前提）==")
HAS_453 = os.path.exists(os.path.join(
    SQLDIR, "0453_team_from_title.sql"))
eq("S6a 0453 迁移文件在册", HAS_453, True)
fn453 = q("SELECT count(*) FROM pg_proc WHERE proname='attach_team_from_name'")
tri_tor = q("SELECT count(*) FROM pg_trigger WHERE NOT tgisinternal "
            "AND tgrelid='torrents'::regclass "
            "AND tgname='torrents_team_derive'")
if fn453 == "1" and tri_tor == "1":
    r3 = {}
    for ln in psql_stdin("""
BEGIN;
CREATE TEMP TABLE res3 (k text, v text);
DO $t$ DECLARE nid bigint; hl int; nm text;
BEGIN
  SELECT coalesce(max(char_length(info_hash)), 40) INTO hl FROM torrents;
  nid := (SELECT max(id) + 1 FROM torrents);
  -- 真插一条最小行：触发器只能在 INSERT 事件上被验，克隆/模拟都不算
  INSERT INTO torrents (id, name, info_hash, category_id, size)
       VALUES (nid, 'Gate.Of.2026.1080p.WEB-DL.x264-HXWEB',
               substr(replace(md5(random()::text),'-','')
                 || replace(md5(clock_timestamp()::text),'-',''), 1, hl),
               (SELECT min(id) FROM categories), 1024);
  SELECT coalesce((SELECT d.name FROM torrent_sections s
                     JOIN section_dict d ON d.id = s.dict_id
                    WHERE s.torrent_id = nid AND s.kind='team'),'(没挂上)')
    INTO nm;
  INSERT INTO res3 SELECT 'derive', nm;
  -- 会员显式选了别的组：write_one 先 DELETE 该维再 INSERT，派生行必须让位
  DELETE FROM torrent_sections WHERE torrent_id=nid AND kind='team';
  INSERT INTO torrent_sections (torrent_id, kind, dict_id, ordinal)
       VALUES (nid,'team',
               (SELECT min(id) FROM section_dict WHERE kind='team'), 1);
  INSERT INTO res3 SELECT 'override_rows', count(*)::text
    FROM torrent_sections WHERE torrent_id=nid AND kind='team';
  -- 技术串不许硬认（-GROUP / x264 都不在字典）
  nid := nid + 1;
  INSERT INTO torrents (id, name, info_hash, category_id, size)
       VALUES (nid, 'Gate.Of.2026.1080p.WEB-DL.x264-GROUP',
               substr(replace(md5(random()::text),'-','')
                 || replace(md5(clock_timestamp()::text),'-',''), 1, hl),
               (SELECT min(id) FROM categories), 1024);
  INSERT INTO res3 SELECT 'junk_rows', count(*)::text
    FROM torrent_sections WHERE torrent_id=nid AND kind='team';
END $t$;
SELECT k || '=' || v FROM res3 ORDER BY k;
ROLLBACK;
""").splitlines():
        if "=" in ln:
            k, v = ln.split("=", 1)
            r3[k] = v
    eq("S6b 新种标题 -HXWEB ⇒ 触发器自动挂上 HXWEB（名单才有东西可判）",
       r3.get("derive"), "HXWEB")
    eq("S6c 会员显式选组后只剩 1 行（派生让位于人选，不双挂）",
       r3.get("override_rows"), "1")
    eq("S6d 结尾是技术串（-GROUP）时不硬认 ⇒ 0 行", r3.get("junk_rows"), "0")
    mounted = q("SELECT count(DISTINCT torrent_id) FROM torrent_sections "
                "WHERE kind='team'")
    print("   team 维现网挂载 %s 颗（0=站长勾完名单仍看不到效果，需真实发布语料）"
          % mounted)
else:
    print("   PENDING：0453 未应用（函数=%s 触发器=%s），S6b-d 不计入"
          % (fn453, tri_tor))

print("== S5 现网提醒（不计红）==")
mounted = q("SELECT count(DISTINCT torrent_id) FROM torrent_sections "
            "WHERE kind='team'")
modes = q("SELECT count(*) FROM mode_kinds WHERE kind='team' AND visible")
if mounted == "0":
    print("   NOTE：现网 team 维挂载 0 行 ⇒ 站长勾完名单看不到效果。")
    print("         0453（按标题派生 + 存量回填）未应用时这就是预期；发布表单"
          "在 %s 个模式里露出该维度但非必填。" % modes)
else:
    print("   team 维已挂载 %s 颗种子（名单对它们直接生效）。" % mounted)

summary()
