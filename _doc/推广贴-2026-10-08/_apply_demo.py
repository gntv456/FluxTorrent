# -*- coding: utf-8 -*-
"""把 0018_demo_data.sql 适配到非空库：演示用户显式占 id 2..13。

0018 的种子/做种/评论块按 owner_id 1..12 硬编码，只在空库上成立；
本机 users 从 283 起跳，直接重放会撞 torrents_owner_id_fkey。
id 2..15 在本库空闲，故给演示用户补上设计时的 id。
"""
import re
import sys

SRC = "apps/api/migrations/0018_demo_data.sql"
OUT = "_demo_patched.sql"

text = open(SRC, encoding="utf-8").read()

COLS_OLD = ("INSERT INTO users (username, email, pass_hash, passkey, class_id, "
            "status, spark_balance, uploaded, downloaded)")
COLS_NEW = ("INSERT INTO users (id, username, email, pass_hash, passkey, class_id, "
            "status, spark_balance, uploaded, downloaded)")
assert COLS_OLD in text, "users 插入段未找到，0018 口径已变"
text = text.replace(COLS_OLD, COLS_NEW, 1)

# 演示用户顺序即 id 2..13
names = ["xuehai", "gaozhong", "chuzhong", "xiaoxue", "laoshi", "banshou",
         "yinyi", "jilupian", "kejian", "biji", "ruanjian", "xueqian"]
for i, n in enumerate(names, start=2):
    text = text.replace("  ('%s'," % n, "  (%d, '%s'," % (i, n), 1)

open(OUT, "w", encoding="utf-8").write(text)
print("wrote", OUT)
