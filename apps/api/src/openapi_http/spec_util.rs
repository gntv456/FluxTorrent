//! OpenAPI 组装器（0268）：spec.rs 与 spec_np.rs 共用的响应/参数/错误模板。
//! 放这里是为了「一份口径」—— 两处各自实现必然漂移。

use serde_json::{json, Map, Value};

/// 200 响应（Envelope<T>；schema 传 "-" 表示纯文本/二进制）
pub(super) fn ok_resp(schema: &str, desc: &str) -> Value {
    if schema == "-" {
        return json!({ "description": desc });
    }
    json!({
        "description": desc,
        "content": { "application/json": { "schema": { "$ref":
            format!("#/components/schemas/{schema}") } } }
    })
}

/// 标准 4xx/429 集合（Map，便于与 200 合并）
pub(super) fn errs() -> Map<String, Value> {
    json!({
        "401": { "description": "Token 无效/已撤销/未传（code 2001）" },
        "403": { "description": "缺所需 scope（code 2003）" },
        "404": { "description": "资源不存在（code 1004）" },
        "429": { "description": "限流（code 1015），带 Retry-After: 60" },
    })
    .as_object()
    .cloned()
    .expect("errs 是对象字面量")
}

/// query 参数速记
pub(super) fn qp(name: &str, desc: &str, ty: &str) -> Value {
    json!({ "name": name, "in": "query", "description": desc,
            "schema": { "type": ty } })
}

/// 组装一个 operation：summary + tag + 参数 + 200 + 标准错误集
pub(super) fn op(
    summary: &str,
    tag: &str,
    desc: Option<&str>,
    params: Vec<Value>,
    ok_schema: &str,
    ok_desc: &str,
) -> Value {
    let mut res = errs();
    res.insert("200".into(), ok_resp(ok_schema, ok_desc));
    let mut o = json!({
        "summary": summary,
        "tags": [tag],
        "parameters": params,
        "responses": Value::Object(res),
    });
    if let (Some(d), Some(obj)) = (desc, o.as_object_mut()) {
        obj.insert("description".into(), Value::String(d.into()));
    }
    o
}
