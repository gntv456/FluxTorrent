// 生态商店 M4 自营适配器样例：douban（元数据源）。
// 目标 wasm32-unknown-unknown、手写宿主 ABI（与 adapter_runtime.rs 的
// linker 导入对齐）：
//   import: host.http_fetch(in_ptr, in_len, out_ptr, out_cap) -> len | 负错误码
//   export: fetch(in_ptr, in_len, out_ptr) -> len
// guest 策略：读入 douban 条目 URL → 交宿主抓取（白名单强制）→ 从 HTML
// 里提标题/封面/简介（粗提，正式版由 PT-Gen 兼容层替代）→ JSON 输出。
// 无 std 依赖（core only）：no_std + 手写 strlen。
#![no_std]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}

#[link(wasm_import_module = "host")]
extern "C" {
    fn http_fetch(
        url_ptr: *const u8,
        url_len: usize,
        out_ptr: *mut u8,
        out_cap: usize,
    ) -> isize;
}

/// 线性内存分配（bump，无回收——单次调用生命周期足够）
static mut HEAP: [u8; 128 * 1024] = [0; 128 * 1024];
static mut HEAP_POS: usize = 64; // 0..64 留作调用约定区

fn bump(size: usize) -> *mut u8 {
    unsafe {
        let p = HEAP.as_mut_ptr().add(HEAP_POS);
        HEAP_POS += size;
        p
    }
}

fn extract_between<'a>(html: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let i = html.find(open)? + open.len();
    let j = html[i..].find(close)? + i;
    Some(&html[i..j])
}

/// 从 douban 页面 HTML 粗提元数据（title/封面/简介）
fn parse(html: &str) -> (&'static str, &'static str, &'static str) {
    // no_std 下无法便捷 String → 用 leak 进 bump 堆（单次调用口径）
    let leak = |s: &str| -> &'static str {
        let p = bump(s.len()) as *mut u8;
        unsafe {
            for (i, b) in s.bytes().enumerate() {
                *p.add(i) = b;
            }
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(p, s.len()))
        }
    };
    let title = extract_between(html, "<title>", "</title>")
        .map(|t| t.trim_end_matches(" (豆瓣)").trim())
        .filter(|t| !t.is_empty())
        .unwrap_or("");
    let poster = extract_between(html, "rel=\"v:image\" href=\"", "\"").unwrap_or("");
    let descr = extract_between(html, "property=\"og:description\" content=\"", "\"")
        .map(|d| d.trim())
        .filter(|d| !d.is_empty())
        .unwrap_or("");
    (leak(title), leak(poster), leak(descr))
}

fn json_escape(s: &str) -> &'static str {
    // 粗转义：双引号与反斜杠（粗提字段的剩余字符风险由宿主 from_str 兜底）
    let mut out = [0u8; 4096];
    let mut n = 0;
    for &b in s.as_bytes() {
        if b == b'"' || b == b'\\' {
            if n + 2 > out.len() { break; }
            out[n] = b'\\'; out[n + 1] = b; n += 2;
        } else if b >= 0x20 {
            if n + 1 > out.len() { break; }
            out[n] = b; n += 1;
        }
    }
    let p = bump(n) as *mut u8;
    unsafe {
        for i in 0..n { *p.add(i) = out[i]; }
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(p, n))
    }
}

/// 宿主调用入口：out_ptr 有 1MiB 缓冲（宿主侧保证）
#[no_mangle]
pub extern "C" fn fetch(in_ptr: *const u8, in_len: usize, out_ptr: *mut u8) -> isize {
    unsafe {
        let url = core::str::from_utf8_unchecked(core::slice::from_raw_parts(in_ptr, in_len));
        // 抓取缓冲（宿主上限 20s/2MB 文本；这里 256KB 足够提取）
        let buf = bump(256 * 1024) as *mut u8;
        let n = http_fetch(url.as_ptr(), url.len(), buf, 256 * 1024);
        if n < 0 {
            return n; // 宿主错误码原样透传（-2 越权 / -3 限流 / -4 上游失败）
        }
        let html = core::str::from_utf8_unchecked(core::slice::from_raw_parts(buf, n as usize));
        let (title, poster, descr) = parse(html);
        let json = concat_json([
            b"{\"success\":true,\"source\":\"douban\",\"name\":\"",
            json_escape(title).as_bytes(),
            b"\",\"poster\":\"",
            json_escape(poster).as_bytes(),
            b"\",\"descr\":\"",
            json_escape(descr).as_bytes(),
            b"\"}",
        ]);
        for (i, &b) in json.iter().enumerate() {
            *out_ptr.add(i) = b;
        }
        json.len() as isize
    }
}

fn concat_json(parts: [&[u8]; 7]) -> &'static [u8] {
    let total: usize = parts.iter().map(|p| p.len()).sum();
    let p = bump(total) as *mut u8;
    let mut n = 0;
    for part in parts {
        unsafe {
            for &b in part {
                *p.add(n) = b;
                n += 1;
            }
        }
    }
    unsafe { core::slice::from_raw_parts(p, total) }
}
