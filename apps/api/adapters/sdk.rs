// FluxTorrent 适配器 SDK（M4 收尾）：宿主 ABI 的 no_std 绑定层。
// 第三方适配器作者 include 本文件后，只需实现 `run(url) -> String`
// （返回 JSON 字符串），不再手写线性内存与导入声明。
//
// 宿主 API（见 apps/api/src/adapter_runtime.rs，冻结面 v1）：
//   host.http_fetch(url) -> Result<String>     出网（白名单强制）
//   host.cache_get(key) -> Option<String>      适配器命名空间 KV
//   host.cache_set(key, val, ttl_s)            KV 写
//   host.secret(name) -> Option<String>        manifest 声明的 secret
//   host.log(level, msg)                       日志透传
//
// 用法（适配器 crate）：
//   #[path = "../sdk.rs"] mod sdk;
//   sdk::entry!(|url: &str| -> String { ... });

#![no_std]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}

#[link(wasm_import_module = "host")]
extern "C" {
    #[link_name = "http_fetch"]
    fn ffi_http_fetch(
        url_ptr: *const u8,
        url_len: usize,
        out_ptr: *mut u8,
        out_cap: usize,
    ) -> isize;
    #[link_name = "cache_get"]
    fn ffi_cache_get(
        key_ptr: *const u8,
        key_len: usize,
        out_ptr: *mut u8,
        out_cap: usize,
    ) -> isize;
    #[link_name = "cache_set"]
    fn ffi_cache_set(
        key_ptr: *const u8,
        key_len: usize,
        val_ptr: *const u8,
        val_len: usize,
        ttl: isize,
    );
    #[link_name = "secret"]
    fn ffi_secret(
        name_ptr: *const u8,
        name_len: usize,
        out_ptr: *mut u8,
        out_cap: usize,
    ) -> isize;
    #[link_name = "host_log"]
    fn ffi_log(level: isize, msg_ptr: *const u8, msg_len: usize);
}

// ---- bump 堆（单次调用生命周期） ----
static mut HEAP: [u8; 128 * 1024] = [0; 128 * 1024];
static mut HEAP_POS: usize = 64;

/// 分配（不回收；单次调用口径足够）
pub fn alloc(size: usize) -> *mut u8 {
    unsafe {
        let p = HEAP.as_mut_ptr().add(HEAP_POS);
        HEAP_POS += size;
        p
    }
}

/// 字符串驻留（bump 堆）
pub fn leak(s: &str) -> &'static str {
    unsafe {
        let p = alloc(s.len()) as *mut u8;
        for (i, b) in s.bytes().enumerate() {
            *p.add(i) = b;
        }
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(
            p,
            s.len(),
        ))
    }
}

const FETCH_BUF: usize = 256 * 1024;

/// 出网（白名单强制）。Err 为宿主错误码描述。
pub fn http_fetch(url: &str) -> Result<&'static str, &'static str> {
    unsafe {
        let buf = alloc(FETCH_BUF) as *mut u8;
        let n = ffi_http_fetch(url.as_ptr(), url.len(), buf, FETCH_BUF);
        match n {
            -2 => Err("出网越权（白名单外 URL）"),
            -3 => Err("限流"),
            -4 => Err("上游请求失败"),
            -1 => Err("宿主调用失败"),
            n if n < 0 => Err("宿主错误"),
            n => Ok(core::str::from_utf8_unchecked(
                core::slice::from_raw_parts(buf, n as usize),
            )),
        }
    }
}

pub fn cache_get(key: &str) -> Option<&'static str> {
    unsafe {
        let buf = alloc(FETCH_BUF) as *mut u8;
        let n = ffi_cache_get(key.as_ptr(), key.len(), buf, FETCH_BUF);
        if n <= 0 {
            return None;
        }
        Some(core::str::from_utf8_unchecked(core::slice::from_raw_parts(
            buf,
            n as usize,
        )))
    }
}

pub fn cache_set(key: &str, val: &str, ttl_s: u64) {
    unsafe {
        ffi_cache_set(
            key.as_ptr(),
            key.len(),
            val.as_ptr(),
            val.len(),
            ttl_s as isize,
        );
    }
}

/// manifest.secrets_read 声明的 secret（越权键返回 None，不暴露是否存在）
pub fn secret(name: &str) -> Option<&'static str> {
    unsafe {
        let buf = alloc(4096) as *mut u8;
        let n = ffi_secret(name.as_ptr(), name.len(), buf, 4096);
        if n <= 0 {
            return None;
        }
        Some(core::str::from_utf8_unchecked(core::slice::from_raw_parts(
            buf,
            n as usize,
        )))
    }
}

pub fn log_info(msg: &str) {
    unsafe { ffi_log(1, msg.as_ptr(), msg.len()) }
}
pub fn log_warn(msg: &str) {
    unsafe { ffi_log(2, msg.as_ptr(), msg.len()) }
}

/// 适配器入口宏：生成 `fetch` 导出（宿主 ABI：in_ptr/in_len/out_ptr → len）。
/// 闭包返回 JSON 字符串；Err(码) 透传宿主错误。
macro_rules! entry_macro {
    ($f:expr) => {
        #[no_mangle]
        pub extern "C" fn fetch(
            in_ptr: *const u8,
            in_len: usize,
            out_ptr: *mut u8,
        ) -> isize {
            unsafe {
                let url = core::str::from_utf8_unchecked(
                    core::slice::from_raw_parts(in_ptr, in_len),
                );
                let out: &str = $f(url);
                for (i, b) in out.as_bytes().iter().enumerate() {
                    *out_ptr.add(i) = *b;
                }
                out.len() as isize
            }
        }
    };
}

// 宏文本顺序 re-export（供 use sdk::entry）
pub(crate) use entry_macro as entry;
