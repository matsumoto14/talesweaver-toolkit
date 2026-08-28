//! ユーザーデータ(SQLite)。登録キャラのみを持ち、静的データは入れない。
//! domain の型との変換はここで行う(domain は SQLite を知らない)。

mod backup;
mod character_repository;

pub use backup::{open_with_backup, OpenOutcome, StartupNotice};
pub use character_repository::validate as validate_new_character;
pub use character_repository::{CharacterRepository, NewCharacter, RegisteredCharacter};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("データベースエラー: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("キャラクター(id={0})が見つかりません")]
    CharacterNotFound(i64),
    #[error("不正な値: {0}")]
    InvalidValue(String),
    #[error("シリアライズに失敗しました: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("ファイル操作に失敗しました: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, StorageError>;
