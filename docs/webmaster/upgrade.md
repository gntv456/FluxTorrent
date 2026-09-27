# 升级与回滚

## 零魔改升级（为什么可以直接升）

本系统的全部 schema 变更走 sqlx 迁移（编号见 `apps/api/migrations/`，0001 起）。api 容器启动时自动执行未跑过的迁移并带 checksum 对账——**只要你没有改过本仓库代码**，升级就是：

```bash
git fetch --tags && git checkout <新版本tag>
docker compose -f docker/docker-compose.yml up -d --build   # 或镜像方式：pull && up -d
```

- 升级只做「前进」：新迁移执行、数据前滚；升级中途失败可重跑（幂等，e2e 有双跑断言）。
- 配置不受升级影响：站点设定存在库里，代码升级不动数据。
- 每个版本的升级注意项写在 [CHANGELOG.md](../../CHANGELOG.md) 对应版本段（如删除废弃设置键、需要手工介入的迁移），升级前读一眼 Unreleased/新版本段。

## 回滚

- **应用层回滚**：`git checkout <旧tag> && docker compose up -d --build`。注意：若新版本已执行了不兼容的 schema 迁移，旧代码可能跑在新 schema 上——所以**重大升级前先备份**，回滚 = 旧代码 + 恢复备份。
- **备份/恢复**：`scripts/backup.sh`（pg_dump -Fc + 附件卷 tar + 14 天轮转）与 `scripts/restore.sh`（先 `--drill` 演练确认归档完整，再 `--force` 实际恢复；恢复动作故意不提供网页按钮，防误触）。
- **站型包回滚**：后台「站型包应用台账」可查看每次 apply 的 diff 快照并回滚（0223 起），与 schema 回滚相互独立。

## 从 NexusPHP 迁移过来

`apps/worker` 自带 NexusPHP 导入器（`np_import`，MySQL 只读、四阶段幂等：users → torrents → stats → report）。用法与字段映射见 `apps/worker/src/bin/np_import.rs` 文件头注释；导入后旧用户首次登录走强制改密。建议先在备份上演练一次全量导入再上生产。
