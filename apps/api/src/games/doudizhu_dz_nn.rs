//! 斗地主 DouZero ONNX 推理（DMC 价值网络，Apache-2.0 权重）。
//! 三个座位各一个模型（landlord/landlord_down/landlord_up），输入
//! x[N×dim] + z[N×5×162]，输出每个候选动作的期望价值，取 argmax。
//!
//! 降级纪律（与「AI 是氛围组」的经济定位一致）：
//! - 编译开关 `douzero` 缺省关闭（本地/CI 不带 onnxruntime 也过门禁）；
//! - 运行时模型文件缺失/加载失败/推理异常 → 一律回落启发式 ai_choose，
//!   只 warn 不 Err——模型是增益不是依赖。
//! 模型路径：`models/doudizhu/{landlord,landlord_down,landlord_up}.onnx`，
//! 相对进程 CWD（容器内 /app）。

#[cfg(feature = "douzero")]
use std::sync::OnceLock;

/// 模型目录（相对 CWD）。
pub(crate) const MODEL_DIR: &str = "models/doudizhu";

/// 三个模型文件名（与上游位置一一对应）。
const FILES: [&str; 3] =
    ["landlord.onnx", "landlord_down.onnx", "landlord_up.onnx"];

/// 选中的动作（DouZero rank 列表或空 = pass）转回本仓牌编码。
/// 手牌实体按 rank 匹配取实际编码（花色无关紧要，状态机只认编码）。
pub(crate) fn dz_action_to_cards(action: &[i16], hand: &[u8]) -> Vec<u8> {
    let mut rest = hand.to_vec();
    let mut out = Vec::with_capacity(action.len());
    for &r in action {
        let want = match r {
            20 => 13u8, // 小王
            30 => 14u8,
            17 => 12u8, // 2
            v @ 3..=14 => (v - 3) as u8,
            _ => continue,
        };
        if let Some(pos) = rest
            .iter()
            .position(|&c| super::doudizhu::Rank::of(c).0 == want)
        {
            out.push(rest.remove(pos));
        }
    }
    out
}

/// 载入失败原因缓存（避免每手重复 warn）。
#[cfg(feature = "douzero")]
struct Sessions {
    sess: [std::sync::Mutex<ort::session::Session>; 3],
}

#[cfg(feature = "douzero")]
static SESSIONS: OnceLock<Result<Sessions, String>> = OnceLock::new();

/// 惰性加载三个模型；失败缓存原因串。
#[cfg(feature = "douzero")]
fn sessions() -> Result<&'static Sessions, &'static str> {
    let init = || -> Result<Sessions, String> {
        // load-dynamic：按 ORT_DYLIB_PATH（缺省 /app/lib/libonnxruntime.so）
        // 运行时加载，绕开编译期 glibc/libstdc++ 版本要求
        let dylib = std::env::var("ORT_DYLIB_PATH")
            .unwrap_or_else(|_| "/app/lib/libonnxruntime.so".into());
        let env = ort::init_from(&dylib)
            .map_err(|e| format!("dylib load: {e}"))?
            .build()
            .map_err(|e| format!("ort env: {e}"))?;
        let mut sess = Vec::with_capacity(3);
        for f in FILES {
            let path = format!("{MODEL_DIR}/{f}");
            if !std::path::Path::new(&path).exists() {
                return Err(format!("model missing: {path}"));
            }
            let s = ort::session::Session::builder(&env)
                .and_then(|mut b| b.commit_from_file(&path))
                .map_err(|e| format!("load {path}: {e}"))?;
            sess.push(std::sync::Mutex::new(s));
        }
        let arr: [std::sync::Mutex<ort::session::Session>; 3] =
            sess.try_into().map_err(|_| "sessions")?;
        Ok(Sessions { sess: arr })
    };
    SESSIONS
        .get_or_init(init)
        .as_ref()
        .map_err(|e| Box::leak(e.clone().into_boxed_str()) as &'static str)
}

/// 给定观测与候选动作，返回最优动作下标（None = 推理不可用）。
#[cfg(feature = "douzero")]
pub(crate) fn best_action(
    g: &super::doudizhu_game::GameState,
    seat: u8,
    actions: &[Vec<i16>],
) -> Option<usize> {
    let pos = pos_of(g.landlord.unwrap_or(0), seat);
    let sessions = sessions().ok()?;
    let Ok(mut guard) = sessions.sess[pos].lock() else {
        return None;
    };
    let (x, z, dim) = super::doudizhu_dz_encode::encode(g, seat, actions)?;
    let n = actions.len();
    let xs = ort::value::Tensor::from_array(([n, dim], x)).ok()?;
    let zs = ort::value::Tensor::from_array(([n, 5, 162], z)).ok()?;
    let outs = guard.run(ort::inputs!["x" => xs, "z" => zs]).ok()?;
    let values = outs["values"].try_extract_tensor::<f32>().ok()?;
    // values 形状 [N,1]：取每行首列做 argmax
    let (shape, data) = values;
    let rows = *shape.iter().next().unwrap_or(&0) as usize;
    if data.len() < rows || rows == 0 || rows != n {
        return None;
    }
    let mut best = 0usize;
    let mut best_v = f32::NEG_INFINITY;
    for (i, &v) in data.iter().take(rows).enumerate() {
        if v > best_v {
            best_v = v;
            best = i;
        }
    }
    Some(best)
}

/// 视角座位 → 模型下标（landlord=0, down=1, up=2）。
#[cfg(feature = "douzero")]
fn pos_of(landlord: u8, seat: u8) -> usize {
    let diff = (seat + 3 - landlord) % 3;
    diff as usize // 0=地主 1=下家 2=上家
}

/// 编译开关关闭时恒 None（调用方回落启发式）。
#[cfg(not(feature = "douzero"))]
pub(crate) fn best_action(
    _g: &super::doudizhu_game::GameState,
    _seat: u8,
    _actions: &[Vec<i16>],
) -> Option<usize> {
    None
}

/// 模型是否可用（hint 侧展示「AI 建议」角标用）。
pub(crate) fn available() -> bool {
    #[cfg(feature = "douzero")]
    {
        sessions().is_ok()
    }
    #[cfg(not(feature = "douzero"))]
    {
        false
    }
}
