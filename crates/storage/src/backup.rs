//! 起動時の DB バックアップと、開けなかったときの復元。
//!
//! このアプリは後方互換を持たない(AGENTS.md)ので、**新しい版で開いた DB は古い版では開けない**。
//! ロールバックの手段がバックアップしかないため、**マイグレーションを当てる前**に版ごとの
//! コピーを取る。
//!
//! 開けなかった場合も**起動不能にしない**。原因で対応を分ける:
//!
//! - **ファイルが壊れている**(`PRAGMA quick_check` が通らない)→ 退避してバックアップから戻す
//! - **マイグレーションに失敗した** → 同じバックアップを戻しても同じ所で失敗する。
//!   **ユーザーのファイルには一切触らず**、インメモリで起動して事情を伝える

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::{CharacterRepository, Result};

/// 残すバックアップの世代数。
const KEEP_BACKUPS: usize = 3;

const BACKUP_INFIX: &str = ".bak.";
const BROKEN_INFIX: &str = ".broken.";

/// 起動時に DB を開いた結果。
pub struct OpenOutcome {
    pub repo: CharacterRepository,
    /// 通常どおり開けたときは `None`。何か起きたときだけ入る。
    pub notice: Option<StartupNotice>,
}

/// 通常どおり開けなかったときに、何が起きて何をしたか。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupNotice {
    /// ファイルが壊れていたので、バックアップから復元して起動した。
    RestoredFromBackup {
        backup: PathBuf,
        moved_broken_to: PathBuf,
    },
    /// ファイルが壊れていて、戻せるバックアップも無かった。空の DB で起動した。
    StartedEmpty { moved_broken_to: PathBuf },
    /// マイグレーションに失敗した。**ファイルはそのまま残し**、保存されない状態で起動した。
    MigrationFailed { path: PathBuf, error: String },
}

impl StartupNotice {
    /// 画面に出す 1 行。
    pub fn message(&self) -> String {
        match self {
            Self::RestoredFromBackup { backup, .. } => format!(
                "データベースを読み込めなかったため、バックアップ「{}」から復元しました。\
                 バックアップを取ったあとの変更は失われています。",
                file_name(backup)
            ),
            Self::StartedEmpty { moved_broken_to } => format!(
                "データベースを読み込めず、戻せるバックアップもありませんでした。\
                 空の状態で起動しています。読み込めなかったファイルは「{}」として残してあります。",
                file_name(moved_broken_to)
            ),
            Self::MigrationFailed { path, error } => format!(
                "データベースの更新に失敗しました({error})。\
                 データは「{}」にそのまま残っていますが、この状態で登録しても保存されません。\
                 前のバージョンに戻すか、問い合わせから報告してください。",
                path.display()
            ),
        }
    }

    /// この状態で加えた変更が保存されるか。`false` なら画面で操作を止めたい。
    pub fn persists_changes(&self) -> bool {
        !matches!(self, Self::MigrationFailed { .. })
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// バックアップを取ってから開く。開けなければ復元を試みる。
///
/// `app_version` はバックアップ名に入る(版ごとに 1 つ。同じ版の 2 回目以降は上書きしない)。
pub fn open_with_backup(path: &Path, app_version: &str) -> Result<OpenOutcome> {
    if !path.exists() {
        // 初回起動。守るものがまだ無い。
        return Ok(OpenOutcome {
            repo: CharacterRepository::open(path)?,
            notice: None,
        });
    }

    if !is_readable(path) {
        return recover_broken_file(path);
    }

    // 読めることを確かめてから、**マイグレーションを当てる前**の状態を残す。
    back_up(path, app_version)?;

    match CharacterRepository::open(path) {
        Ok(repo) => Ok(OpenOutcome { repo, notice: None }),
        // 戻しても同じ所で失敗する。ユーザーのファイルには触らない。
        Err(error) => Ok(OpenOutcome {
            repo: CharacterRepository::open_in_memory()?,
            notice: Some(StartupNotice::MigrationFailed {
                path: path.to_path_buf(),
                error: error.to_string(),
            }),
        }),
    }
}

/// SQLite として読めるか。マイグレーションは当てない。
fn is_readable(path: &Path) -> bool {
    let Ok(conn) = Connection::open(path) else {
        return false;
    };
    conn.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))
        .map(|result| result == "ok")
        .unwrap_or(false)
}

/// 版ごとのコピーを作り、古い世代を落とす。
fn back_up(path: &Path, app_version: &str) -> Result<()> {
    let destination = suffixed(path, BACKUP_INFIX, &sanitize(app_version));
    // 同じ版で既に取ってあるなら、それが「この版が初めて触る前」の状態。上書きしない。
    if !destination.exists() {
        fs::copy(path, &destination)?;
    }
    prune_backups(path)?;
    Ok(())
}

/// 新しい順に `KEEP_BACKUPS` 世代だけ残す。
fn prune_backups(path: &Path) -> Result<()> {
    let mut backups = list_backups(path)?;
    if backups.len() <= KEEP_BACKUPS {
        return Ok(());
    }
    backups.sort_by(|a, b| b.1.cmp(&a.1));
    for (old, _) in backups.into_iter().skip(KEEP_BACKUPS) {
        fs::remove_file(old)?;
    }
    Ok(())
}

/// `<DB名>.bak.*` を更新時刻つきで集める。
fn list_backups(path: &Path) -> Result<Vec<(PathBuf, SystemTime)>> {
    let Some(directory) = path.parent() else {
        return Ok(Vec::new());
    };
    let prefix = format!("{}{}", file_name(path), BACKUP_INFIX);

    let mut found = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(&prefix) {
            continue;
        }
        found.push((entry.path(), entry.metadata()?.modified()?));
    }
    Ok(found)
}

/// 壊れたファイルを退避し、最新のバックアップから戻す。戻せなければ空で起動する。
fn recover_broken_file(path: &Path) -> Result<OpenOutcome> {
    let moved_broken_to = suffixed(path, BROKEN_INFIX, &unix_seconds());
    fs::rename(path, &moved_broken_to)?;

    let newest = list_backups(path)?
        .into_iter()
        .max_by_key(|(_, modified)| *modified)
        .map(|(backup, _)| backup);

    if let Some(backup) = newest {
        fs::copy(&backup, path)?;
        if let Ok(repo) = CharacterRepository::open(path) {
            return Ok(OpenOutcome {
                repo,
                notice: Some(StartupNotice::RestoredFromBackup {
                    backup,
                    moved_broken_to,
                }),
            });
        }
        // バックアップも開けなかった。空から作り直す。
        fs::remove_file(path)?;
    }

    Ok(OpenOutcome {
        repo: CharacterRepository::open(path)?,
        notice: Some(StartupNotice::StartedEmpty { moved_broken_to }),
    })
}

/// `foo.sqlite` + `.bak.` + `0.1.0` → `foo.sqlite.bak.0.1.0`
fn suffixed(path: &Path, infix: &str, tail: &str) -> PathBuf {
    let mut name = OsString::from(path.file_name().unwrap_or(path.as_os_str()));
    name.push(infix);
    name.push(tail);
    match path.parent() {
        Some(directory) => directory.join(name),
        None => PathBuf::from(name),
    }
}

/// ファイル名に入れられない文字を落とす。
fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn unix_seconds() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir(label: &str) -> PathBuf {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("tw-backup-{label}-{}-{unique}", unix_seconds()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn db_path(dir: &Path) -> PathBuf {
        dir.join("talesweaver-toolkit.sqlite")
    }

    #[test]
    fn 初回起動はバックアップを作らずに開ける() {
        let dir = temp_dir("first");
        let path = db_path(&dir);

        let outcome = open_with_backup(&path, "0.1.0").unwrap();

        assert_eq!(outcome.notice, None);
        assert!(path.exists());
        assert!(
            list_backups(&path).unwrap().is_empty(),
            "守るものが無いのでバックアップは作らない"
        );
    }

    #[test]
    fn 二回目の起動で版ごとのバックアップができる() {
        let dir = temp_dir("second");
        let path = db_path(&dir);

        open_with_backup(&path, "0.1.0").unwrap();
        let outcome = open_with_backup(&path, "0.1.0").unwrap();

        assert_eq!(outcome.notice, None);
        assert!(dir.join("talesweaver-toolkit.sqlite.bak.0.1.0").exists());
    }

    #[test]
    fn 同じ版では上書きせず別の版では増える() {
        let dir = temp_dir("versions");
        let path = db_path(&dir);

        for _ in 0..3 {
            open_with_backup(&path, "0.1.0").unwrap();
        }
        assert_eq!(list_backups(&path).unwrap().len(), 1, "同じ版は 1 つだけ");

        open_with_backup(&path, "0.2.0").unwrap();
        assert_eq!(list_backups(&path).unwrap().len(), 2);
    }

    #[test]
    fn バックアップは三世代だけ残る() {
        let dir = temp_dir("prune");
        let path = db_path(&dir);

        for minor in 1..=5u64 {
            open_with_backup(&path, &format!("0.{minor}.0")).unwrap();
            // 新旧は更新時刻で決めるので、同一秒に固まらないようずらす。
            let backup = dir.join(format!("talesweaver-toolkit.sqlite.bak.0.{minor}.0"));
            if backup.exists() {
                let file = fs::OpenOptions::new().write(true).open(&backup).unwrap();
                file.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000 + minor))
                    .unwrap();
            }
        }

        assert_eq!(list_backups(&path).unwrap().len(), KEEP_BACKUPS);
    }

    #[test]
    fn 壊れたdbはバックアップから復元して起動する() {
        let dir = temp_dir("restore");
        let path = db_path(&dir);

        open_with_backup(&path, "0.1.0").unwrap();
        open_with_backup(&path, "0.1.0").unwrap();
        assert_eq!(list_backups(&path).unwrap().len(), 1);

        fs::write(&path, b"this is not a sqlite database").unwrap();

        let outcome = open_with_backup(&path, "0.2.0").unwrap();

        match &outcome.notice {
            Some(StartupNotice::RestoredFromBackup {
                moved_broken_to, ..
            }) => assert!(moved_broken_to.exists(), "壊れた DB は退避して残す"),
            other => panic!("復元されるはず: {other:?}"),
        }
        assert!(outcome.repo.list().is_ok(), "復元後の DB は普通に使える");
        assert!(outcome.notice.as_ref().unwrap().persists_changes());
    }

    #[test]
    fn 戻せるバックアップが無ければ空で起動する() {
        let dir = temp_dir("empty");
        let path = db_path(&dir);

        fs::write(&path, b"this is not a sqlite database").unwrap();

        let outcome = open_with_backup(&path, "0.1.0").unwrap();

        match &outcome.notice {
            Some(StartupNotice::StartedEmpty { moved_broken_to }) => {
                assert!(moved_broken_to.exists())
            }
            other => panic!("空で起動するはず: {other:?}"),
        }
        assert!(outcome.repo.list().unwrap().is_empty());
    }

    #[test]
    fn マイグレーション失敗ではユーザーのファイルに触らない() {
        let dir = temp_dir("migration");
        let path = db_path(&dir);

        // SQLite としては健全だが `characters` がビューなので `CREATE TABLE` が通らない。
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("CREATE VIEW characters AS SELECT 1 AS id;")
                .unwrap();
        }
        let before = fs::metadata(&path).unwrap().len();

        let outcome = open_with_backup(&path, "0.1.0").unwrap();

        match &outcome.notice {
            Some(StartupNotice::MigrationFailed { .. }) => {}
            other => panic!("マイグレーション失敗として扱うはず: {other:?}"),
        }
        assert!(!outcome.notice.as_ref().unwrap().persists_changes());
        assert!(path.exists(), "ユーザーのファイルは消さない");
        assert_eq!(fs::metadata(&path).unwrap().len(), before, "書き換えもしない");
        // マイグレーション前の状態はバックアップとして残っている。
        assert_eq!(list_backups(&path).unwrap().len(), 1);
    }
}
