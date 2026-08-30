//! 登録キャラごとの表示画像。ゲームのドメインデータとは分け、正規化済み PNG だけを保存する。

use std::io::Cursor;

use image::imageops::FilterType;
use image::{ImageFormat, ImageReader};
use rusqlite::params;

use crate::{CharacterRepository, Result, StorageError};

const MAX_SOURCE_BYTES: usize = 5 * 1024 * 1024;
const MAX_SOURCE_PIXELS: u64 = 16_000_000;
const ICON_SIZE: u32 = 128;

#[derive(Debug, Clone, PartialEq)]
pub struct CharacterIcon {
    pub character_id: i64,
    pub png: Vec<u8>,
}

pub(crate) fn migrate_character_icons(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS character_icons (
            character_id INTEGER PRIMARY KEY REFERENCES characters(id) ON DELETE CASCADE,
            png          BLOB    NOT NULL
        );",
    )?;
    Ok(())
}

fn normalize_icon(source: &[u8]) -> Result<Vec<u8>> {
    if source.is_empty() || source.len() > MAX_SOURCE_BYTES {
        return Err(StorageError::InvalidCharacterIcon(
            "画像は5 MiB以下にしてください".into(),
        ));
    }
    let reader = ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(|_| StorageError::InvalidCharacterIcon("画像形式を判定できません".into()))?;
    let format = reader.format().ok_or_else(|| {
        StorageError::InvalidCharacterIcon("PNG、JPEG、WebPを選んでください".into())
    })?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err(StorageError::InvalidCharacterIcon(
            "PNG、JPEG、WebPを選んでください".into(),
        ));
    }
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| StorageError::InvalidCharacterIcon("画像を読み取れません".into()))?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS {
        return Err(StorageError::InvalidCharacterIcon(
            "画像は合計1600万画素以下にしてください".into(),
        ));
    }
    let image = image::load_from_memory_with_format(source, format)
        .map_err(|_| StorageError::InvalidCharacterIcon("画像を読み取れません".into()))?;
    let side = width.min(height);
    let square = image.crop_imm((width - side) / 2, (height - side) / 2, side, side);
    let normalized = square.resize_exact(ICON_SIZE, ICON_SIZE, FilterType::Lanczos3);
    let mut png = Cursor::new(Vec::new());
    normalized
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|_| StorageError::InvalidCharacterIcon("画像を保存用に変換できません".into()))?;
    Ok(png.into_inner())
}

impl CharacterRepository {
    pub fn list_character_icons(&self) -> Result<Vec<CharacterIcon>> {
        let mut stmt = self
            .conn
            .prepare("SELECT character_id, png FROM character_icons ORDER BY character_id")?;
        let rows = stmt.query_map([], |row| {
            Ok(CharacterIcon {
                character_id: row.get(0)?,
                png: row.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn set_character_icon(&self, character_id: i64, source: &[u8]) -> Result<CharacterIcon> {
        self.get(character_id)?;
        let png = normalize_icon(source)?;
        self.conn.execute(
            "INSERT INTO character_icons (character_id, png) VALUES (?1, ?2)
             ON CONFLICT(character_id) DO UPDATE SET png = excluded.png",
            params![character_id, png],
        )?;
        Ok(CharacterIcon { character_id, png })
    }

    pub fn reset_character_icon(&self, character_id: i64) -> Result<()> {
        self.get(character_id)?;
        self.conn.execute(
            "DELETE FROM character_icons WHERE character_id = ?1",
            [character_id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};

    fn source_png(width: u32, height: u32) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_fn(width, height, |x, _| {
            if x < width / 2 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            }
        }));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn 画像を中央クロップして128px_pngへ保存する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let character_id = repo.conn.execute(
            "INSERT INTO characters (name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level, stat_sources, equipment, common_skills)
             VALUES ('x', 'lucian', 0,0,0,0,0,0,0,0,0,'{}','{}','{}')", [],
        ).map(|_| repo.conn.last_insert_rowid()).unwrap();
        let saved = repo
            .set_character_icon(character_id, &source_png(300, 200))
            .unwrap();
        let decoded = image::load_from_memory(&saved.png).unwrap();
        assert_eq!(decoded.dimensions(), (128, 128));
        assert_eq!(repo.list_character_icons().unwrap(), vec![saved]);
    }

    #[test]
    fn キャラ削除で画像も削除される() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let id = repo.conn.execute(
            "INSERT INTO characters (name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level, stat_sources, equipment, common_skills)
             VALUES ('x', 'lucian', 0,0,0,0,0,0,0,0,0,'{}','{}','{}')", [],
        ).map(|_| repo.conn.last_insert_rowid()).unwrap();
        repo.set_character_icon(id, &source_png(10, 10)).unwrap();
        repo.delete(id).unwrap();
        assert!(repo.list_character_icons().unwrap().is_empty());
    }

    #[test]
    fn 対応外形式と上限超過を拒否する() {
        let repo = CharacterRepository::open_in_memory().unwrap();
        let id = repo.conn.execute(
            "INSERT INTO characters (name, game_character_id, stab, hack, int, def, mr, dex, agi, awakening_stage, eternal_level, stat_sources, equipment, common_skills)
             VALUES ('x', 'lucian', 0,0,0,0,0,0,0,0,0,'{}','{}','{}')", [],
        ).map(|_| repo.conn.last_insert_rowid()).unwrap();
        assert!(matches!(
            repo.set_character_icon(id, b"not an image"),
            Err(StorageError::InvalidCharacterIcon(_))
        ));
        assert!(matches!(
            repo.set_character_icon(id, &vec![0; MAX_SOURCE_BYTES + 1]),
            Err(StorageError::InvalidCharacterIcon(_))
        ));
    }
}
