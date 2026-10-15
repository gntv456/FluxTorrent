//! 服务层（FRAME-PLAN-01 §2/§4）：一个写用例一个函数的唯一居所。
//! 路由层（`*_http/`）只做参数适配与循环调用，SQL 与副作用编排收在这里。
//! 分层宪法（门禁 `scripts/layer_guard.mjs` 强制）：本目录禁 `actix_web`——
//! HTTP 词汇（请求/响应/路由）不得漏进用例层。

pub(crate) mod hr_pardon;
pub(crate) mod review_decide;
