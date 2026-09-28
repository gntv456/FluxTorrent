# 升级演练（upgrade drill）

> [upgrade.md](../webmaster/upgrade.md) 讲「为什么可以直升」；本篇是**实证**：
> 用 `scripts/upgrade_drill.py` 在本机把升级窗口的全路径跑一遍，出数字。
> 建议大版本升级前必跑一次；平时可作为备份链路的周检（第 2 步会真实走
> pg_dump|pg_restore）。

## 演练覆盖的四步

| 步 | 动作 | 断言 |
|---|---|---|
| 0 基线 | 读库内 `_sqlx_migrations` 水位 | INFO 记录（版本号/条数/无 null checksum） |
| 1 备份 | 跑 `scripts/backup.sh`（或 `--skip-backup` 复用当日归档） | 退出码 0 |
| 2 前进 | pg_dump|pg_restore 复制源库 → 演练库上重放迁移（等价新版本启动动作） | 演练库迁移水位与源库一致 |
| 3 断言 | 演练库关键表/账号 | root 存在；KEY_TABLES 同 backup_drill 口径 |
| 4 回滚口径 | 本机旧镜像在位 + `restore.sh` 在位 | 镜像数 > 0 且脚本存在 |

第 2 步顺带验证了备份链路本身——dump 出不来或 restore 不进去，第一时间暴露。

## 实证记录

- 2026-09-28（v0.2.0 + master 0237 迁移水位，本机 docker 栈）：
  全绿，耗时 12s；演练库 234 条迁移与源库一致；root 断言 PASS；
  回滚口径：本机 22 个 flux 镜像 + restore.sh 在位。
  脚本自动清演练库；`--keep` 可保留排查。

## 用法

```bash
python scripts/upgrade_drill.py                # 全流程（含现场备份）
python scripts/upgrade_drill.py --skip-backup  # 已有当日备份
python scripts/upgrade_drill.py --keep         # 保留演练库调试
```

要求：本机 docker 栈在跑（flux-postgres 容器）；迁移重放需要本地 cargo
sqlx（容器内生产镜像不带 CLI，脚本自动回落）。

## 版本支持政策

- 0.x 阶段（当前）：安全修复只针对**最新 minor 及其前一版**（如 0.2.x 与 0.1.x）；
- 升级路径：相邻 minor 可直升（迁移自动前进）；跨多级升级建议先看
  [CHANGELOG](../../CHANGELOG.md) 各版本的「升级注意」段，逐级跳板；
- 破坏性变更（如迁移不可回滚的）会在 CHANGELOG 用 **BREAKING** 标出，
  升级前备份是唯一回滚保障（见 upgrade.md 回滚节）。
