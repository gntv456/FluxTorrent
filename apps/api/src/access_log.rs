//! 访问日志（P2 防凭据泄漏）。从 main.rs 按域拆出（300 行门禁）。
//!
//! actix 默认格式里的 `%r` 含完整 query——compat 下载走 `?passkey=`、
//! 凭证下载走 `?token=`、开放 API 走 `?apikey=`，那等价把长期/短期凭据
//! 写进 access log。故这里把 `%r` 换成 `%m %U`（method + **不含 query** 的
//! path），其余字段与 default 对齐。
//!
//! 二轮审计（中）：`/rss/{passkey}` 的 passkey 在**路径**里，`%U` 挡不住
//! 路径形态 ⇒ 整个 RSS 前缀不落 access log（RSS 自身有 30/min/passkey
//! 限流，不记日志不影响排障粒度）。
//!
//! P-66（批三十，两轮实测）：这一条原本写 `exclude("/api/v1/rss")`。actix 的
//! exclude 是 HashSet 精确匹配（logger.rs:322 `exclude.contains(req.path())`），
//! 不存在通配语义 ⇒ 带路径参数的 RSS 一条都没被排除，实测 access log 与
//! request_id 的「请求开始」行都落了整串 passkey（长期凭据：拿到即可下种、
//! 订阅 RSS、替该账号 announce；日志的读者=运维/日志聚合/任何能
//! `docker logs` 的人都持有全体订阅者凭据）。
//! 现在 exclude_regex 挡路径形态，request_id 侧由 `log_safe_path` 按形态脱敏
//! 兜第二条通道——两条都得堵，只堵一条等于没堵。

use actix_web::middleware::Logger;

pub fn build() -> Logger {
    Logger::new("%a \"%m %U\" %s %b \"%{Referer}i\" \"%{User-Agent}i\" %T")
        .exclude("/api/v1/health")
        // actix 的 exclude 是 **HashSet 精确匹配**（actix-web-4.15.0
        // src/middleware/logger.rs:322 `exclude.contains(req.path())`），
        // 文档里没有通配这回事：`/api/v1/rss` 与 `/api/v1/rss*` 都一条没挡
        // （后者本批先写上去，实测 logger 行仍带整串 passkey）。
        // 带路径参数的出口只能用 exclude_regex。
        .exclude_regex("^/api/v1/rss")
}
