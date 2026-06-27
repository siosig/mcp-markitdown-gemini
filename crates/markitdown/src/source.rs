//! 取得済みコンテンツとそのヒント (data-model.md: SourceContent / Origin)。

use std::path::PathBuf;
use url::Url;

/// 取得元の種別。
#[derive(Debug, Clone)]
pub enum Origin {
    /// ローカルファイル (`file:`)。
    File(PathBuf),
    /// リモート (`http:` / `https:`)。
    Remote(Url),
    /// インライン埋め込み (`data:`)。
    Inline,
}

/// acquire 層が URI から取得した生データと、フォーマット判定のためのヒント。
#[derive(Debug, Clone)]
pub struct SourceContent {
    /// 取得した生バイト列。
    pub bytes: Vec<u8>,
    /// content-type / data URI 由来の MIME タイプ (あれば)。
    pub mime: Option<String>,
    /// ファイル名 (拡張子判定用、あれば)。
    pub filename: Option<String>,
    /// charset ヒント (あれば)。
    pub charset: Option<String>,
    /// 取得元種別。
    pub origin: Origin,
}

impl SourceContent {
    /// `filename` から小文字の拡張子を取り出す。
    pub fn extension(&self) -> Option<String> {
        let name = self.filename.as_ref()?;
        let ext = std::path::Path::new(name).extension()?;
        Some(ext.to_string_lossy().to_lowercase())
    }
}
