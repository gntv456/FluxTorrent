//! 受限表达式引擎（生态商店 M3，策划案 §4.2）：规则包公式的求值与 lint。
//!
//! 能力面刻意做小（沙箱能力悖论的窄品类解）：
//! - 字符/记号白名单：算术、比较、`min`/`max` 两个安全内建函数（分档公式的
//!   夹逼表达靠它们）；**无**赋值、链式、字符串、其它函数（AST 侧校验
//!   FunctionIdentifier 名单）；
//! - 变量必须显式绑定（每条规则声明变量集），未知变量 lint 即拒；
//! - 规模上限（≤200 字符 / ≤64 AST 节点）近似超时防线；运行期错误
//!   （除零/越域/未定义）一律回落默认值并告警（A4 熔断的表达式版）。
//!
//! 首个品类：银行利率公式（term_days → 年化利率 / 万分比日利率）。

use crate::errors::{DomainError, DomainResult};

/// 单条规则的静态描述：表达式 + 变量白名单 + 默认值 + 数值域
pub(crate) struct RuleSpec {
    pub key: &'static str,
    /// 允许出现在表达式中的变量（如 ["term_days"]）
    pub vars: &'static [&'static str],
    /// 表达式非法/求值失败时的回退值
    pub fallback: f64,
    /// 结果数值域（含端点）；NaN/Inf 恒拒
    pub min: f64,
    pub max: f64,
}

/// 银行利率规则：变量 term_days，值域 [0, 1]（年化利率）
pub(crate) const TERM_RATE: RuleSpec = RuleSpec {
    key: "bank.term_rate",
    vars: &["term_days"],
    fallback: 0.0,
    min: 0.0,
    max: 1.0,
};

/// 贷款日利率规则：变量 term_days，值域 [0, 0.01]（万分比/日换算为小数）
pub(crate) const LOAN_RATE: RuleSpec = RuleSpec {
    key: "bank.loan_daily_rate",
    vars: &["term_days"],
    fallback: 0.0,
    min: 0.0,
    max: 0.01,
};

/// lint：解析 + 节点白名单 + 变量白名单。返回人类可读错误（空 = 通过）。
pub(crate) fn lint(expr: &str, spec: &RuleSpec) -> DomainResult<()> {
    let expr = expr.trim();
    if expr.is_empty() {
        return Err(DomainError::Validation("表达式为空".into()));
    }
    if expr.len() > 200 {
        return Err(DomainError::Validation(
            "表达式过长（≤200 字符）".into(),
        ));
    }
    // 字符白名单：数字/变量字符/空白/算术/比较/括号/逗号（min/max 参数表）
    let ok = expr.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || c == '_'
            || c.is_ascii_whitespace()
            || matches!(
                c,
                '+' | '-' | '*' | '/' | '%' | '(' | ')' | '<' | '>'
                    | '.' | ','
            )
    });
    if !ok {
        return Err(DomainError::Validation(
            "表达式含非法字符（仅允许算术/比较/min/max）".into(),
        ));
    }
    for forbidden in
        ["==", "&&", "||", "!", "=", ";", "[", "]", "?", ":"]
    {
        if expr.contains(forbidden) {
            return Err(DomainError::Validation(format!(
                "表达式含禁止记号：{forbidden}"
            )));
        }
    }
    let ast =
        evalexpr::build_operator_tree::<evalexpr::DefaultNumericTypes>(
            expr,
        )
        .map_err(|e| {
            DomainError::Validation(format!("表达式解析失败：{e}"))
        })?;
    // 节点数上限（近似超时防线：表达式规模有界）
    let node_count = ast.iter().count();
    if node_count > 64 {
        return Err(DomainError::Validation(
            "表达式节点过多（≤64）".into(),
        ));
    }
    // 变量白名单 + 函数白名单（仅 min/max）：遍历 AST
    let mut unknown = Vec::new();
    let mut bad_fn = Vec::new();
    for node in ast.iter() {
        match node.operator() {
            evalexpr::Operator::VariableIdentifierRead {
                identifier,
            } => {
                if !spec.vars.contains(&identifier.as_str()) {
                    unknown.push(identifier.clone());
                }
            }
            evalexpr::Operator::FunctionIdentifier { identifier } => {
                if !["min", "max"].contains(&identifier.as_str()) {
                    bad_fn.push(identifier.clone());
                }
            }
            _ => {}
        }
    }
    if !bad_fn.is_empty() {
        return Err(DomainError::Validation(format!(
            "禁止的函数：{}（仅允许 min/max）",
            bad_fn.join(", ")
        )));
    }
    if !unknown.is_empty() {
        return Err(DomainError::Validation(format!(
            "未知变量：{}（允许：{}）",
            unknown.join(", "),
            spec.vars.join(", ")
        )));
    }
    Ok(())
}

fn is_number(_s: &str) -> bool {
    // evalexpr 13 的常量叶子是 Value 节点，不再以标识符形式出现；
    // 保留占位避免误删调用方（lint 只需白名单外的 VariableIdentifierRead）
    true
}

/// 求值：失败（未定义变量/除零/溢出）= 回落默认值并告警（A4 降级语义）
pub(crate) fn eval(
    expr: &str,
    spec: &RuleSpec,
    vars: &std::collections::HashMap<&str, f64>,
) -> f64 {
    let expr = expr.trim();
    if expr.is_empty() {
        return spec.fallback;
    }
    // 变量注入：绑定全部白名单变量（缺省 0）
    use evalexpr::ContextWithMutableVariables;
    let mut ctx = evalexpr::HashMapContext::new();
    for v in spec.vars {
        let val = vars.get(v).copied().unwrap_or(0.0);
        let _ = ctx.set_value(
            v.to_string(),
            evalexpr::Value::Float(val),
        );
    }
    match ast_eval(expr, &ctx) {
        Some(v) if v.is_finite() && v >= spec.min && v <= spec.max => v,
        Some(v) => {
            tracing::warn!(
                rule = spec.key,
                v,
                "规则求值越域，回落默认值"
            );
            spec.fallback
        }
        None => {
            tracing::warn!(rule = spec.key, "规则求值失败，回落默认值");
            spec.fallback
        }
    }
}

fn ast_eval(
    expr: &str,
    ctx: &evalexpr::HashMapContext<evalexpr::DefaultNumericTypes>,
) -> Option<f64> {
    let tree =
        evalexpr::build_operator_tree::<evalexpr::DefaultNumericTypes>(expr)
            .ok()?;
    let v = tree.eval_with_context(ctx).ok()?;
    match v {
        evalexpr::Value::Float(f) => Some(f),
        evalexpr::Value::Int(i) => Some(i as f64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lint_accepts_clamped_formula() {
        // 连续利率曲线：底 0.5%、顶 18%，随期限线性爬升
        lint(
            "min(0.18, max(0.005, term_days / 30 * 0.02))",
            &TERM_RATE,
        )
        .unwrap();
    }

    #[test]
    fn lint_rejects_unknown_var() {
        let e = lint("principal * 2", &TERM_RATE).unwrap_err();
        assert!(e.to_string().contains("未知变量"));
    }

    #[test]
    fn lint_rejects_functions_and_assign() {
        assert!(lint("sin(term_days)", &TERM_RATE).is_err());
        assert!(lint("a = 1", &TERM_RATE).is_err());
        assert!(
            lint("min(0.01, max(0.005, term_days / 30 * 0.02))", &TERM_RATE)
                .is_ok()
        );
    }

    #[test]
    fn eval_clamped_curve() {
        let expr = "min(0.18, max(0.005, term_days / 30 * 0.02))";
        let mk = |d: f64| -> std::collections::HashMap<&str, f64> {
            [("term_days", d)].into_iter().collect()
        };
        // 底 0.5% 生效
        assert!((eval(expr, &TERM_RATE, &mk(7.0)) - 0.005).abs() < 1e-9);
        // 线性段
        assert!((eval(expr, &TERM_RATE, &mk(90.0)) - 0.06).abs() < 1e-9);
        // 顶 18% 封顶
        assert!((eval(expr, &TERM_RATE, &mk(3650.0)) - 0.18).abs() < 1e-9);
    }

    #[test]
    fn eval_out_of_domain_falls_back() {
        let ctx = std::collections::HashMap::new();
        assert_eq!(eval("5", &TERM_RATE, &ctx), TERM_RATE.fallback);
    }

    #[test]
    fn eval_div_zero_falls_back() {
        let ctx = std::collections::HashMap::new();
        assert_eq!(eval("1 / 0", &TERM_RATE, &ctx), TERM_RATE.fallback);
    }
}
