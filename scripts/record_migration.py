#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""手工登记一条已直接执行过的迁移进 _sqlx_migrations（checksum = 文件字节 sha384）。

用法：python scripts/record_migration.py 0319_music_format_axes.sql
"""
import hashlib
import os
import subprocess
import sys

MIG_DIR = os.path.join(os.path.dirname(__file__), "..", "apps", "api",
                       "migrations")

fname = sys.argv[1]
version = int(fname.split("_", 1)[0])
path = os.path.join(MIG_DIR, fname)
data = open(path, "rb").read()
digest = hashlib.sha384(data).hexdigest()
hexlit = chr(92) + "x" + digest  # '\x...' 避免 unicodeescape 语法坑

sql = ("INSERT INTO _sqlx_migrations "
       "(version, description, installed_on, success, checksum, "
       "execution_time) "
       "VALUES (%d, '%s', now(), true, '%s'::bytea, 0) "
       "ON CONFLICT (version) DO UPDATE SET checksum = EXCLUDED.checksum "
       "RETURNING version"
       % (version, fname[:-4].replace("_", " "), hexlit))
r = subprocess.run(
    ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
     "fluxtorrent", "-t", "-A", "-c", sql],
    capture_output=True, text=True)
if r.returncode != 0:
    raise SystemExit("psql failed: " + r.stderr[:300])
print("recorded:", r.stdout.strip(), "sha384:", digest[:16] + "…")
