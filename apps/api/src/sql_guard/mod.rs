//! 插件 SQL 语句校验器（安全闸门，fail-closed）。
//!
//! 插件经 `PluginCtx::db_exec` 执行任何 SQL 前必须先过本校验：
//! 1. **解析**：sqlparser Postgres 方言解析；**解析失败一律拒绝**（插件必须写
//!    本校验器能解析的 SQL，宁可误拒不可放过）；
//! 2. **单语句**：拒绝 `;` 分隔的多语句；
//! 3. **语句类型白名单**：仅 SELECT / INSERT / UPDATE / DELETE（Display 反序列化
//!    取首词判定，跨 sqlparser 版本稳定；WITH/CTE、DDL、SET、GRANT 全部拒绝）；
//! 4. **拒表别名/派生表**：别名标识符无法匹配声明表（见 collect.rs）；
//! 5. **表白名单**：AST 收集的所有表引用（含子查询/JOIN）必须 ∈ 插件声明的表；
//! 6. **写保护**：写语句每个表须 `DbMode::Write`；UPDATE/DELETE 必须带 WHERE
//!    （无界写需该表 `allow_unbounded`）；
//! 7. **列白名单**：限定标识符 `t.c` 的 `t` 必须是声明表、`c` 必须是该表声明列；
//!    裸标识符（含 INSERT 列名 / UPDATE SET 目标）必须是任一引用表的声明列；
//! 8. **函数白名单**：仅 `FUNC_ALLOWLIST`；
//! 9. **拒 SELECT ***：插件必须显式列列名（含 `t.*`）；
//! 10. **ObjectName 兜底**：自定义类型名等对象名必须落在
//!     「声明表 ∪ 声明列 ∪ 函数白名单 ∪ 类型白名单」内；
//! 11. **危险记号**：粗粒度字符串兜底（FOR UPDATE / set_config / dblink 等）。
//!
//! 已知限制（v1，fail-closed 方向）：插件写不带别名的平铺 SQL 即可；
//! 需要子查询的场景用嵌套 Query（其表/列同样过白名单）。

mod collect;
#[cfg(test)]
mod tests;

use sqlparser::ast::{AssignmentTarget, Statement};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use crate::errors::{DomainError, DomainResult};
use crate::plugin_ctx::{DbCapability, DbMode};

use collect::{Collector, FORBIDDEN_TOKENS, FUNC_ALLOWLIST, TYPE_ALLOWLIST};

/// 校验插件 SQL。`Err` = 拒绝（调用方应记审计并中止执行）。
pub fn validate(sql: &str, caps: &[DbCapability]) -> DomainResult<()> {
    // 0. 粗粒度危险记号（额外一层，结构校验之外的兜底）
    let up = sql.to_uppercase();
    for bad in FORBIDDEN_TOKENS {
        if up.contains(bad) {
            return Err(DomainError::Validation(format!(
                "插件 SQL 含危险记号：{bad}"
            )));
        }
    }
    // 1. 解析（fail-closed）
    let ast = Parser::parse_sql(&PostgreSqlDialect {}, sql).map_err(|e| {
        DomainError::Validation(format!(
            "插件 SQL 解析失败（fail-closed）: {e}"
        ))
    })?;
    // 2. 单语句
    if ast.len() != 1 {
        return Err(DomainError::Validation(
            "插件 SQL 只允许单语句（拒绝多语句注入）".into(),
        ));
    }
    let stmt = &ast[0];
    // 3. 语句类型白名单（Display 首词，跨版本稳定）
    let head = stmt
        .to_string()
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    if !matches!(head.as_str(), "SELECT" | "INSERT" | "UPDATE" | "DELETE") {
        return Err(DomainError::Validation(format!(
            "插件 SQL 语句类型不允许：{head}（仅 SELECT/INSERT/UPDATE/DELETE）"
        )));
    }
    let is_write = matches!(head.as_str(), "INSERT" | "UPDATE" | "DELETE");
    // 4. 收集（Visitor 钩子 + 手动补收钩子盲区）
    let mut col = Collector::collect(stmt, caps);
    collect_manual(stmt, &mut col);
    // 5. 表别名 / 派生表 / 子查询 WITH
    if let Some(w) = &col.bad_factor {
        return Err(DomainError::Validation(format!(
            "插件 SQL 不支持{w}（写平铺 SQL）"
        )));
    }
    // 6. 表白名单
    for t in &col.relations {
        if !col.tables.contains(t) {
            return Err(DomainError::Validation(format!(
                "插件 SQL 越表：{t}（声明表：{:?}）",
                col.tables.iter().collect::<Vec<_>>()
            )));
        }
    }
    // 7. 写语句的每个表必须是 Write 能力
    if is_write {
        for t in &col.relations {
            if col.modes.get(t).copied() != Some(DbMode::Write) {
                return Err(DomainError::Validation(format!(
                    "插件 SQL 写表 {t} 但未声明 Write 能力"
                )));
            }
        }
    }
    // 8. UPDATE/DELETE 必须带 WHERE（按表判无界写豁免）
    let no_where = match stmt {
        Statement::Update { selection, .. } => selection.is_none(),
        Statement::Delete(d) => d.selection.is_none(),
        _ => false,
    };
    if no_where {
        for t in &col.relations {
            if !col.unbounded.get(t).copied().unwrap_or(false) {
                return Err(DomainError::Validation(
                    "UPDATE/DELETE 必须带 WHERE（无界写需该表 allow_unbounded）"
                        .into(),
                ));
            }
        }
    }
    // 9. 函数白名单
    for f in &col.funcs {
        if !FUNC_ALLOWLIST.contains(&f.as_str()) {
            return Err(DomainError::Validation(format!(
                "插件 SQL 函数不在白名单：{f}"
            )));
        }
    }
    // 10. 拒 SELECT *
    if col.star {
        return Err(DomainError::Validation(
            "插件 SQL 不允许 SELECT *（显式列列名）".into(),
        ));
    }
    // 11. 标识符（列）白名单
    for (head_opt, name) in &col.idents {
        let name_l = name.to_lowercase();
        match head_opt {
            Some(t) => {
                let t = t.to_lowercase();
                if !col.tables.contains(&t) {
                    return Err(DomainError::Validation(format!(
                        "插件 SQL 限定表未声明：{t}（表别名/派生表 v1 不支持）"
                    )));
                }
                let ok =
                    col.cols.get(&t).is_some_and(|cs| cs.contains(&name_l));
                if !ok {
                    return Err(DomainError::Validation(format!(
                        "插件 SQL 越列：{t}.{name}"
                    )));
                }
            }
            None => {
                let any = col.cols.values().any(|cs| cs.contains(&name_l));
                if !any {
                    return Err(DomainError::Validation(format!(
                        "插件 SQL 未知列：{name}"
                    )));
                }
            }
        }
    }
    // 12. ObjectName 兜底（自定义类型名等）
    let all_cols: std::collections::BTreeSet<&String> =
        col.cols.values().flatten().collect();
    for o in &col.objs {
        let ok = col.tables.contains(o)
            || all_cols.contains(o)
            || FUNC_ALLOWLIST.contains(&o.as_str())
            || TYPE_ALLOWLIST.contains(&o.as_str());
        if !ok {
            return Err(DomainError::Validation(format!(
                "插件 SQL 未知对象名：{o}"
            )));
        }
    }
    Ok(())
}

/// Visitor 钩子覆盖不到的对象名，手动结构匹配补齐：
/// - INSERT 列名（`Vec<Ident>` 无钩子）；
/// - UPDATE SET 目标（`AssignmentTarget::ColumnName(ObjectName)`）；
/// - DELETE 多表名（MySQL 风格 `Vec<ObjectName>`）；
/// - INSERT 表别名（PG `INSERT INTO t AS x`）。
fn collect_manual(stmt: &Statement, col: &mut Collector) {
    match stmt {
        Statement::Insert(ins) => {
            for c in &ins.columns {
                col.push_ident(None, &c.value);
            }
            if ins.table_alias.is_some() && col.bad_factor.is_none() {
                col.bad_factor = Some("INSERT 表别名".to_string());
            }
        }
        Statement::Update { assignments, .. } => {
            for a in assignments {
                match &a.target {
                    AssignmentTarget::ColumnName(name) => {
                        push_object_name(col, name);
                    }
                    AssignmentTarget::Tuple(names) => {
                        for n in names {
                            push_object_name(col, n);
                        }
                    }
                }
            }
        }
        Statement::Delete(d) => {
            for t in &d.tables {
                if let Some(i) = t.0.last() {
                    col.push_relation(&i.value);
                }
            }
        }
        _ => {}
    }
}

/// ObjectName 作列目标收编：单段=裸列名，多段=限定标识符。
fn push_object_name(col: &mut Collector, name: &sqlparser::ast::ObjectName) {
    match name.0.as_slice() {
        [] => {}
        [one] => col.push_ident(None, &one.value),
        [head, .., last] => {
            col.push_ident(Some(&head.value), &last.value);
        }
    }
}
