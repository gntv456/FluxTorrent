# 性能基线

## 已测数字

| 场景 | 吞吐 | p50 | p95 | p99 | 备注 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| announce（HTTP，32 并发 20s） | 2138 req/s | 7.2ms | 10.0ms | 13.5ms | 压测端饱和值，tracker 未降级、零失败 |
| announce（128 并发，双 passkey） | ~2100 req/s | 7.1ms | 9.8ms | 12.8ms | 同量级 |
| torrents 列表（api） | 1158 req/s | 6.0ms | 12.5ms | 26.6ms | release 构建 |
| login 成功请求 | ~0.4 | 46ms | — | — | Argon2 校验成本；其余被限流拦截（防护生效） |

端到端延迟抽检与 PG 热点见 `_doc/性能基线-2026-09-27.md`（`python scripts/perf_baseline.py --rounds 5` 可复跑，dev 空库数字仅用于同环境对比）。

## 复测方法

```bash
python scripts/bench.py 10 8                                        # api 场景
python scripts/announce_bench.py --url http://127.0.0.1:7070 \
  --passkeys <pk> --concurrency 32 --duration 20                    # tracker
```

## 容量参考

- tracker 为内存 peer 表 + Redis Stream 异步计费，announce 路径零 DB 依赖；千级 peer 站点余量 >100 倍；
- 数据库层瓶颈先看 `?tool=dbstats`（pg_stat_statements 热点）与 300ms 慢日志；
- 多机扩展：计费流消费者组 + 任务分片 + tracker XFF 双档已就位（G30 批），三机配方见 `_doc/G30-多机部署三机配方.md`。

## 版本纪律

重大版本发布前跑一轮 `perf_baseline.py` 存档到 `_doc/性能基线-<日期>.md`，与上一版对比无回退再发。
