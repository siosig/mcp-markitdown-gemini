//! 変換器が共有するユーティリティ (テキストデコード・Markdown 表・XML 名処理・ZIP 読込)。

use std::io::{Cursor, Read};

/// ZIP バイト列から指定名のエントリを読み出す。
pub fn read_zip_entry(zip_bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes)).ok()?;
    let mut file = archive.by_name(name).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// ZIP バイト列から、指定プレフィックスで始まるエントリ名を列挙する。
pub fn list_zip_entries(zip_bytes: &[u8], prefix: &str) -> Vec<String> {
    let Ok(archive) = zip::ZipArchive::new(Cursor::new(zip_bytes)) else {
        return Vec::new();
    };
    archive
        .file_names()
        .filter(|n| n.starts_with(prefix))
        .map(|n| n.to_string())
        .collect()
}

/// バイト列をテキストへデコードする。
///
/// charset ヒントがあればそれを優先し、なければ `chardetng` で推定する (FR-015)。
pub fn decode_text(bytes: &[u8], charset: Option<&str>) -> String {
    if let Some(label) = charset {
        if let Some(enc) = encoding_rs::Encoding::for_label(label.as_bytes()) {
            let (cow, _, _) = enc.decode(bytes);
            return cow.into_owned();
        }
    }
    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Allow);
    detector.feed(bytes, true);
    let enc = detector.guess(None, chardetng::Utf8Detection::Allow);
    let (cow, _, _) = enc.decode(bytes);
    cow.into_owned()
}

/// quick-xml の Text イベントをデコードし、XML エンティティを展開した文字列を返す。
pub fn decode_xml_text(t: &quick_xml::events::BytesText) -> String {
    match t.decode() {
        Ok(cow) => quick_xml::escape::unescape(&cow)
            .map(|u| u.into_owned())
            .unwrap_or_else(|_| cow.into_owned()),
        Err(_) => String::new(),
    }
}

/// 先頭サンプルがテキストらしいか (NUL 無し・印字可能比率が高い) を判定する。
pub fn looks_like_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    let sample = &bytes[..bytes.len().min(4096)];
    if sample.contains(&0) {
        return false;
    }
    if std::str::from_utf8(sample).is_ok() {
        return true;
    }
    // UTF-8 でなくても制御文字が少なければテキスト扱い (レガシーエンコーディング)。
    let printable = sample
        .iter()
        .filter(|&&b| b >= 0x20 || b == b'\n' || b == b'\r' || b == b'\t')
        .count();
    (printable as f64) / (sample.len() as f64) > 0.85
}

/// XML の修飾名 (`w:p` など) からローカル名 (`p`) を取り出す。
pub fn local_name(qname: &[u8]) -> &[u8] {
    match qname.iter().position(|&b| b == b':') {
        Some(i) => &qname[i + 1..],
        None => qname,
    }
}

/// セル内の改行・パイプを Markdown 表で安全な表現へエスケープする。
fn escape_cell(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(['\n', '\r'], " ")
        .trim()
        .to_string()
}

/// 行列データを Markdown のテーブルへ整形する。先頭行をヘッダとして扱う。
///
/// 行が空の場合は空文字列を返す。列数は最大列に合わせてパディングする。
pub fn markdown_table(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return String::new();
    }

    let mut out = String::new();
    let render_row = |out: &mut String, row: &[String]| {
        out.push('|');
        for c in 0..cols {
            let cell = row.get(c).map(|s| escape_cell(s)).unwrap_or_default();
            out.push(' ');
            out.push_str(&cell);
            out.push_str(" |");
        }
        out.push('\n');
    };

    render_row(&mut out, &rows[0]);
    out.push('|');
    for _ in 0..cols {
        out.push_str(" --- |");
    }
    out.push('\n');
    for row in &rows[1..] {
        render_row(&mut out, row);
    }
    out
}
