//! ユーザーデータ(SQLite)。登録キャラのみを持ち、静的データは入れない。
//! domain の型との変換はここで行う(domain は SQLite を知らない)。

mod character_repository;

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
}

pub type Result<T> = std::result::Result<T, StorageError>;
