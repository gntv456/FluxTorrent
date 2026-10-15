//! 考核多阶段状态机（**真身在 worker**：`worker/src/task_jobs/exam_fair`）。
//!
//! 结算循环跑在 worker，而 api 侧的运营/查询端也要用同一套判定
//! （如「我现在在第几阶段」「补救期还剩几天」）。两边若各写一份，
//! 迟早漂移——worker 判失败、api 显示进行中，就是最典型的症状。
//!
//! 故此处 `include!` worker 的源文件：单点真相、逐位一致。
//! 代价是本 crate 不能给它加 `pub(crate)` 之外的路径依赖——
//! 故 `InvalidConfig` 自带 Display/Error，由调用方包成
//! `crate::errors::DomainError::Validation`。

#[path = "../../../../worker/src/task_jobs/exam_fair/mod.rs"]
mod inner;

pub use inner::*;

#[cfg(test)]
#[path = "../../../../worker/src/task_jobs/exam_fair/tests.rs"]
mod tests;
