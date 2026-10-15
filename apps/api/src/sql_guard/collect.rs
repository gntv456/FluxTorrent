//! sql_guard 收集层：白名单常量 + AST 收集器（sqlparser Visitor）。
//!
//! sqlparser 0.52 的 `Visitor` 只提供 query / relation / table_factor /
//! expr / statement 五组钩子。钩子**覆盖不到**的对象名（INSERT 列名、
//! UPDATE SET 目标、DELETE 多表名、`SELECT *` 投影、Cast 自定义类型），
//! 由 mod.rs 的 `collect_manual` 手动结构匹配补齐——fail-closed 方向：
//! 宁可漏收后误拒，不可放过。

use std::collections::{BTreeMap, BTreeSet};
use std::ops::ControlFlow;

use sqlparser::ast::{
    DataType, Expr, ObjectName, Query, SetExpr, TableFactor, Visit, Visitor,
};

use crate::plugin_ctx::{DbCapability, DbMode};

/// 允许的函数（小写）。插件需要更多时在此扩展并补测试。
pub(super) const FUNC_ALLOWLIST: &[&str] = &[
    "min", "max", "count", "sum", "avg", "coalesce", "nullif", "greatest",
    "least", "abs", "round", "floor", "ceil", "ceiling", "lower", "upper",
    "length", "char_length", "now", "current_timestamp", "date_trunc",
    "extract", "to_char",
];

/// 类型白名单（DataType::Custom 的用户自定义类型名，如 `$1::inet`
/// 的标准类型是具名变体不经对象名，Custom 才是兜底目标）。
pub(super) const TYPE_ALLOWLIST: &[&str] = &[
    "inet", "text", "varchar", "char", "bpchar", "citext", "int", "int2",
    "int4", "int8", "integer", "bigint", "smallint", "boolean", "bool",
    "numeric", "decimal", "real", "float4", "float8", "timestamptz",
    "timestamp", "date", "time", "jsonb", "json", "uuid", "bytea",
];

/// 粗粒度危险记号兜底（结构校验之外的额外一层；大写比较）。
pub(super) const FORBIDDEN_TOKENS: &[&str] = &[
    "FOR UPDATE", "FOR SHARE", "SET_CONFIG", "CURRENT_SETTING", "PG_SLEEP",
    "PG_READ_FILE", "DBLINK", "LO_IMPORT", "LO_EXPORT",
];

/// AST 收集器：遍历语句，收集表/标识符/对象名/星号/函数。
pub(super) struct Collector {
    /// 声明表（小写）
    pub(super) tables: BTreeSet<String>,
    /// 声明表 → 声明列（均小写）
    pub(super) cols: BTreeMap<String, BTreeSet<String>>,
    pub(super) modes: BTreeMap<String, DbMode>,
    pub(super) unbounded: BTreeMap<String, bool>,
    /// AST 收集到的表引用（小写）
    pub(super) relations: BTreeSet<String>,
    /// (限定表或 None, 标识符)
    pub(super) idents: Vec<(Option<String>, String)>,
    /// ObjectName 兜底收集（自定义类型名等）
    pub(super) objs: BTreeSet<String>,
    pub(super) funcs: BTreeSet<String>,
    pub(super) star: bool,
    /// 表别名 / 派生表 / 子查询 WITH / INSERT 表别名（命中即拒）
    pub(super) bad_factor: Option<String>,
}

impl Collector {
    pub(super) fn new(caps: &[DbCapability]) -> Self {
        let mut c = Collector {
            tables: BTreeSet::new(),
            cols: BTreeMap::new(),
            modes: BTreeMap::new(),
            unbounded: BTreeMap::new(),
            relations: BTreeSet::new(),
            idents: Vec::new(),
            objs: BTreeSet::new(),
            funcs: BTreeSet::new(),
            star: false,
            bad_factor: None,
        };
        for cap in caps {
            let t = cap.table.to_lowercase();
            c.tables.insert(t.clone());
            c.cols.insert(
                t.clone(),
                cap.columns.iter().map(|s| s.to_lowercase()).collect(),
            );
            c.modes.insert(t.clone(), cap.mode);
            c.unbounded.insert(t, cap.allow_unbounded);
        }
        c
    }

    /// 遍历 + 手动补收（钩子盲区），返回收集结果。
    pub(super) fn collect(
        stmt: &sqlparser::ast::Statement,
        caps: &[DbCapability],
    ) -> Self {
        let mut c = Self::new(caps);
        let _ = stmt.visit(&mut c);
        c
    }

    /// 手动补收：限定/裸标识符（INSERT 列名、UPDATE SET 目标）。
    pub(super) fn push_ident(&mut self, head: Option<&str>, col: &str) {
        self.idents.push((head.map(str::to_string), col.to_string()));
    }

    /// 手动补收：表引用（DELETE 多表名）。
    pub(super) fn push_relation(&mut self, table: &str) {
        self.relations.insert(table.to_lowercase());
    }
}

impl Visitor for Collector {
    type Break = ();

    fn pre_visit_relation(&mut self, r: &ObjectName) -> ControlFlow<()> {
        if let Some(i) = r.0.last() {
            self.relations.insert(i.value.to_lowercase());
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_expr(&mut self, e: &Expr) -> ControlFlow<()> {
        match e {
            Expr::Identifier(i) => {
                self.idents.push((None, i.value.clone()));
            }
            Expr::CompoundIdentifier(ids) if ids.len() >= 2 => {
                let head = ids[0].value.clone();
                for c in &ids[1..] {
                    self.idents.push((Some(head.clone()), c.value.clone()));
                }
            }
            Expr::Function(f) => {
                if let Some(i) = f.name.0.last() {
                    self.funcs.insert(i.value.to_lowercase());
                }
            }
            // `$1::inet` / CAST(x AS mytype)：标准类型是具名 DataType
            // 变体不经对象名；Custom 的用户类型名在此兜底收集
            Expr::Cast { data_type, .. }
            | Expr::TypedString { data_type, .. } => {
                if let DataType::Custom(name, _) = data_type {
                    if let Some(i) = name.0.last() {
                        self.objs.insert(i.value.to_lowercase());
                    }
                }
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_table_factor(
        &mut self,
        tf: &TableFactor,
    ) -> ControlFlow<()> {
        // 表别名 / 派生表：其标识符无法匹配声明表（v1 直接拒）
        let why = match tf {
            TableFactor::Table { alias: Some(a), .. } => {
                Some(format!("表别名 {}", a.name))
            }
            TableFactor::Derived { alias, .. } => Some(match alias {
                Some(a) => format!("派生表 {}", a.name),
                None => "派生表".to_string(),
            }),
            TableFactor::TableFunction { .. }
            | TableFactor::Function { .. }
            | TableFactor::UNNEST { .. }
            | TableFactor::JsonTable { .. }
            | TableFactor::NestedJoin { .. }
            | TableFactor::Pivot { .. }
            | TableFactor::Unpivot { .. }
            | TableFactor::MatchRecognize { .. } => {
                Some("复杂表因子".to_string())
            }
            TableFactor::Table { alias: None, .. } => None,
        };
        if let Some(w) = why {
            if self.bad_factor.is_none() {
                self.bad_factor = Some(w);
            }
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_query(&mut self, q: &Query) -> ControlFlow<()> {
        // 子查询 WITH；SELECT * 投影（钩子无 select_item，手动查 projection）
        if q.with.is_some() && self.bad_factor.is_none() {
            self.bad_factor = Some("子查询 WITH".to_string());
        }
        if let SetExpr::Select(s) = q.body.as_ref() {
            for item in &s.projection {
                if matches!(
                    item,
                    sqlparser::ast::SelectItem::Wildcard(_)
                        | sqlparser::ast::SelectItem::QualifiedWildcard(_, _)
                ) {
                    self.star = true;
                }
            }
        }
        ControlFlow::Continue(())
    }
}
