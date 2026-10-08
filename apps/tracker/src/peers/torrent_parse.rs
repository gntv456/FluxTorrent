//! .torrent 的最小 bencode 解析：**只为 piece 级抽查服务**。
//!
//! 背景：piece 哈希（`info.pieces`，每 20 字节一个 SHA-1）与 `info.piece length`
//! 只存在于 `torrent_files.raw` 的 bencode 里——`torrents` 表**没有**对应列
//! （仅有整体的 `pieces_hash`）。要做「请求一个 piece 并比对 SHA-1」的抽查，
//! tracker 必须先把这两项取出来。
//!
//! 刻意不做通用 bencode 解析器：只认顶层 dict 的 `info` 子 dict，其余
//! （announce 列表、comment、创建者…）一律跳过。这样既省代码又避免解析器
//! 本身成为攻击面（tracker 处理的是站内已注册的种子文件，但解析仍按
//! 「遇到无法识别的结构立即返回 None、绝不 panic」写）。
//!
//! 取**全部** piece 哈希（2026-10-08 开源反作弊收口）：旧版只取第一个，
//! 抽查恒打 piece 0——作弊者持有一块真 piece 0 即可过检。全量哈希让
//! 抽查位置可随机（见 bt_probe::probe_piece_hash）。内存口径：4096 条
//! 缓存上限 × 每条 hashes（大种子数千 piece × 20B ≈ 百 KB 级），可接受。

/// 从 .torrent raw 字节里抽出 piece 抽查所需的最小信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PieceProbe {
    /// `info.piece length`（BEP3 要求是 2 的幂且 >0）
    pub(crate) piece_len: u32,
    /// `info.pieces` 的全部 20 字节 SHA-1（抽查随机选一）
    pub(crate) hashes: Vec<[u8; 20]>,
}

/// 从 .torrent raw 中解析 piece_length 与 piece 哈希列表。
/// 结构不符/截断/字段缺失一律 `None`（保守：拿不到就不做 piece 抽查，
/// 退回握手+bitfield 判定，绝不因解析失败把真做种者误杀）。
pub(crate) fn parse_piece_probe(raw: &[u8]) -> Option<PieceProbe> {
    let mut p = Parser { b: raw, i: 0 };
    p.skip_ws();
    if p.peek()? != b'd' {
        return None;
    }
    p.i += 1; // 吃掉 'd'
    let mut piece_len: Option<u32> = None;
    let mut hashes: Option<Vec<[u8; 20]>> = None;
    loop {
        p.skip_ws();
        if p.peek()? == b'e' {
            // 顶层 dict 正常结束。无需再前进游标——`p` 随即被丢弃。
            break;
        }
        let key = p.dict_key()?;
        if key.as_slice() == b"info" {
            // info 是嵌套 dict，递归找 piece length / pieces
            let (pl, hs) = p.parse_info_dict()?;
            piece_len = pl;
            hashes = hs;
            // 已拿到所需信息就不再解析外层剩余字段（announce/comment/…）：
            // 既省掉无谓的容错分支（那些字段的 skip 一旦遇到畸形结构会
            // 让整个函数假失败），也符合「只取两个字段」的最小解析目标。
            if piece_len.is_some() && hashes.is_some() {
                break;
            }
        } else {
            p.skip_value()?;
        }
    }
    Some(PieceProbe {
        piece_len: piece_len?,
        hashes: hashes?,
    })
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c == b' ' || c == b'\n' || c == b'\r' || c == b'\t' {
                self.i += 1;
            } else {
                break;
            }
        }
    }
    /// 读一个 dict 键（x: 前缀的字符串）
    fn dict_key(&mut self) -> Option<Vec<u8>> {
        self.skip_ws();
        let n = self.digits()?;
        self.skip_ws();
        if self.peek()? != b':' {
            return None;
        }
        self.i += 1;
        let end = self.i.checked_add(n)?;
        if end > self.b.len() {
            return None;
        }
        let s = self.b[self.i..end].to_vec();
        self.i = end;
        Some(s)
    }
    /// 读一个十进制数字
    fn digits(&mut self) -> Option<usize> {
        self.skip_ws();
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.i += 1;
            } else {
                break;
            }
        }
        if self.i == start {
            return None;
        }
        std::str::from_utf8(&self.b[start..self.i])
            .ok()?
            .parse()
            .ok()
    }
    /// 跳过一个完整值（string / int / list / dict），用于跳过无关字段。
    fn skip_value(&mut self) -> Option<()> {
        self.skip_ws();
        match self.peek()? {
            b'i' => {
                // int：i<数字>e
                self.i += 1;
                while self.peek()? != b'e' {
                    self.i += 1;
                }
                self.i += 1;
                Some(())
            }
            b'l' => {
                // list：纯 value 序列，遇到 e 结束
                self.i += 1;
                loop {
                    self.skip_ws();
                    match self.peek()? {
                        b'e' => {
                            self.i += 1;
                            return Some(());
                        }
                        _ => self.skip_value()?,
                    }
                }
            }
            b'd' => {
                // dict：key + value 成对，遇到 e 结束
                self.i += 1;
                loop {
                    self.skip_ws();
                    match self.peek()? {
                        b'e' => {
                            self.i += 1;
                            return Some(());
                        }
                        _ => {
                            self.dict_key()?;
                            self.skip_value()?;
                        }
                    }
                }
            }
            _ => {
                // string：<len>:<bytes>
                let n = self.digits()?;
                self.skip_ws();
                if self.peek()? != b':' {
                    return None;
                }
                self.i += 1;
                let end = self.i.checked_add(n)?;
                if end > self.b.len() {
                    return None;
                }
                self.i = end;
                Some(())
            }
        }
    }
    /// 解析 info 子 dict，找 piece length 与首个 pieces 哈希
    fn parse_info_dict(
        &mut self,
    ) -> Option<(Option<u32>, Option<Vec<[u8; 20]>>)> {
        self.skip_ws();
        if self.peek()? != b'd' {
            return None;
        }
        self.i += 1;
        let mut piece_len = None;
        let mut hashes: Option<Vec<[u8; 20]>> = None;
        loop {
            self.skip_ws();
            if self.peek()? == b'e' {
                self.i += 1;
                break;
            }
            let key = self.dict_key()?;
            match key.as_slice() {
                b"piece length" => {
                    self.skip_ws();
                    if self.peek()? != b'i' {
                        return None;
                    }
                    self.i += 1;
                    let start = self.i;
                    while self.peek()? != b'e' {
                        self.i += 1;
                    }
                    let num = std::str::from_utf8(&self.b[start..self.i])
                        .ok()?
                        .parse::<i64>()
                        .ok()?;
                    self.i += 1; // 吃掉 'e'
                    if num > 0 {
                        piece_len = Some(num as u32);
                    }
                }
                b"pieces" => {
                    // string：全部 20 字节 SHA-1 逐块收进 hashes
                    let n = self.digits()?;
                    self.skip_ws();
                    if self.peek()? != b':' {
                        return None;
                    }
                    self.i += 1;
                    if n < 20 || n % 20 != 0 {
                        return None;
                    }
                    let end = self.i.checked_add(n)?;
                    if end > self.b.len() {
                        return None;
                    }
                    // 上界防御（1 GiB 哈希串 = 5000 万 piece）已由 CAP 与
                    // 种子文件本身约束；这里直接切齐
                    let hs: Vec<[u8; 20]> = self.b[self.i..end]
                        .chunks_exact(20)
                        .map(|c| c.try_into().unwrap())
                        .collect();
                    hashes = Some(hs);
                    self.i = end; // 跳到整个 string 之后
                }
                _ => self.skip_value()?,
            }
        }
        Some((piece_len, hashes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个最小 .torrent。**顶层与 info 内均按 bencode 规范键名升序**，
    /// 且字符串长度前缀必须与实际字节数严格一致（写错长度是这类解析器
    /// 最常见的自伤方式——解析器读到错位后必然失败）。
    fn sample(piece_len: i64, hash: [u8; 20]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"d");
        // 无关字段（announce），验证解析器能跳过；长度 = 实际字节数
        let announce = b"http://tracker/announce";
        v.extend_from_slice(b"8:announce");
        v.extend_from_slice(announce.len().to_string().as_bytes());
        v.push(b':');
        v.extend_from_slice(announce);
        v.extend_from_slice(b"4:infod");
        // info 内键升序：length < name < piece length < pieces
        v.extend_from_slice(b"6:lengthi1024e");
        v.extend_from_slice(b"4:name5:hello");
        v.extend_from_slice(
            format!("12:piece lengthi{}e", piece_len).as_bytes(),
        );
        v.extend_from_slice(b"6:pieces20:");
        v.extend_from_slice(&hash);
        v.extend_from_slice(b"e");
        v.extend_from_slice(b"e");
        v
    }

    #[test]
    fn parses_piece_len_and_hashes() {
        let h = [7u8; 20];
        let raw = sample(16384, h);
        let got = parse_piece_probe(&raw).expect("应解析成功");
        assert_eq!(got.piece_len, 16384);
        assert_eq!(got.hashes, vec![h]);
    }

    #[test]
    fn skips_unrelated_fields() {
        // 顶层含嵌套 list（announce-list）与 string（comment），
        // 解析器应能跳过；info 放在最后，拿到 piece 信息即返回。
        let mut raw = Vec::new();
        raw.extend_from_slice(b"d");
        // announce-list = l l 11:url http://a e e
        //   注意 "urlhttp://a" 是 11 字节，长度前缀必须写 11——bencode 的
        //   长度前缀与实际字节数不符时，任何合规解析器都只能失败。
        raw.extend_from_slice(b"13:announce-listll11:urlhttp://aee");
        // comment = 5:hello
        raw.extend_from_slice(b"7:comment5:hello");
        // info dict（升序：length < piece length < pieces）
        raw.extend_from_slice(b"4:infod");
        raw.extend_from_slice(b"6:lengthi1024e");
        raw.extend_from_slice(b"12:piece lengthi32768e");
        raw.extend_from_slice(b"6:pieces20:");
        raw.extend_from_slice(&[9u8; 20]);
        raw.extend_from_slice(b"e");
        raw.extend_from_slice(b"e");
        let got = parse_piece_probe(&raw).expect("应跳过无关字段");
        assert_eq!(got.piece_len, 32768);
        assert_eq!(got.hashes, vec![[9u8; 20]]);
    }

    #[test]
    fn rejects_malformed_inputs() {
        assert!(parse_piece_probe(b"").is_none());
        assert!(parse_piece_probe(b"not-bencode").is_none());
        assert!(parse_piece_probe(b"di1e").is_none()); // int 而非 dict
                                                       // 缺 piece length
        let raw = b"d4:infod6:pieces20:01234567890123456789ee";
        assert!(parse_piece_probe(raw).is_none());
        // 缺 pieces
        let raw = b"d4:infod12:piece lengthi16384eee";
        assert!(parse_piece_probe(raw).is_none());
    }

    #[test]
    fn rejects_truncated_input() {
        // 截断的 pieces string：声明 20 字节但实际不足
        let mut raw = Vec::new();
        raw.extend_from_slice(
            b"d4:infod12:piece lengthi16384e6:pieces20:short",
        );
        raw.extend_from_slice(b"e");
        assert!(parse_piece_probe(&raw).is_none());
    }

    #[test]
    fn rejects_zero_piece_length() {
        let raw = sample(0, [1u8; 20]);
        // piece length 为 0 视为无效（不设 piece_len）→ 整体 None
        assert!(parse_piece_probe(&raw).is_none());
    }

    /// 真实种子回归（torrent_id=40079，站内 demo 种子原样字节）：
    /// 含 UTF-8 中文 name/comment 与 announce 字段。真实文件比人造样本
    /// 覆盖更全（键升序、多字节长度前缀），是最有价值的回归锚点。
    #[test]
    fn parses_real_seed_bytes() {
        let raw: &[u8] = b"d4:infod12:piece lengthi16384e4:name37:FluxTorrent \xe5\xaf\xb9\xe6\x8e\xa5\xe8\x87\xaa\xe5\x8a\xa8\xe7\xa7\x8d\xe5\xad\x90 (demo)6:lengthi96e6:pieces20:\xd6\xbfRp\xc9\x96\xdd;\xef\x9b\x9f#V\xdf~\xd76~m\xa5e7:comment36:FluxTorrent demo seed (\xe5\xaf\xb9\xe6\x8e\xa5\xe8\x87\xaa\xe5\x8a\xa8)8:announce22:http://127.0.0.1:7070/announcee";
        let got = parse_piece_probe(raw).expect("真实种子应解析成功");
        assert_eq!(got.piece_len, 16384);
        // 首个 piece 的 SHA-1 应逐字节对上
        let expect_first: [u8; 20] = [
            0xd6, 0xbf, 0x52, 0x70, 0xc9, 0x96, 0xdd, 0x3b, 0xef, 0x9b, 0x9f,
            0x23, 0x56, 0xdf, 0x7e, 0xd7, 0x36, 0x7e, 0x6d, 0xa5,
        ];
        assert_eq!(got.hashes[0], expect_first);
    }
}
