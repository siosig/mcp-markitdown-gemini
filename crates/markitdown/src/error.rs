//! 型付きエラー (`thiserror`)。ライブラリ crate のため `anyhow` は使用しない。

use thiserror::Error;

/// 変換パイプライン全体で使われるエラー型。
///
/// いずれのバリアントもプロセスを停止させず、原因が判別できるメッセージを持つ
/// (spec FR-009/FR-010, SC-004)。
#[derive(Error, Debug)]
pub enum MarkItDownError {
    /// 対応外の URI スキーム (http/https/file/data 以外)。
    #[error("unsupported URI scheme: '{0}' (supported: http, https, file, data)")]
    UnsupportedScheme(String),

    /// URI のパースに失敗。
    #[error("invalid URI: {0}")]
    InvalidUri(String),

    /// コンテンツ取得失敗 (権限/不在/ネットワーク/404/timeout など)。
    #[error("failed to acquire content from '{uri}': {message}")]
    Acquire { uri: String, message: String },

    /// 判定できない・未対応のフォーマット。
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    /// 破損・パース失敗。
    #[error("failed to decode {format}: {message}")]
    Decode { format: String, message: String },

    /// I/O エラー。
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl MarkItDownError {
    /// 取得エラーを簡潔に生成するヘルパ。
    pub fn acquire(uri: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Acquire {
            uri: uri.into(),
            message: message.into(),
        }
    }

    /// デコードエラーを簡潔に生成するヘルパ。
    pub fn decode(format: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Decode {
            format: format.into(),
            message: message.into(),
        }
    }
}
