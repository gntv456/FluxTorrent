# 运维手册（ops）

- [监控与告警](monitoring.md)：Prometheus + Grafana 栈与五条预置告警
- [任务面板](jobs.md)：43 个后台任务、手动触发、DLQ
- [性能基线](performance.md)：已测数字与复测方法
- [备份恢复](backup.md)：脚本、演练、恢复纪律
- [升级演练](upgrade-drill.md)：升级窗口四步实证脚本与版本支持政策
- [工具生态收录](ecosystem.md)：PT-Plugin-Plus/Jackett/cross-seed 适配指南
- [兼容矩阵](compat-matrix.md)：生态工具协议级冒烟实证（PTPP/cross-seed/NP 系客户端全绿）
- [运维事件订阅](events.md)：生命周期事件 webhook 矩阵（注册/封禁/H&R/捐赠）
- [搜索方案评估](search-eval.md)：pg_trgm 继续用/外置引擎触发条件与升级路径
- [质量基建](quality-gates.md)：覆盖率基线（vitest/cargo-llvm-cov）与 Playwright 冒烟
- [安全姿态](security.md)：认证/响应头/CSP 基线/限流/审计与开站自查清单
