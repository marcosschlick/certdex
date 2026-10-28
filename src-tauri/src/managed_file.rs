use sqlx::{Sqlite, Transaction};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub(crate) fn read_managed_file(
    directory: &Path,
    stored_path: Option<&str>,
) -> Result<Option<(PathBuf, Vec<u8>)>, String> {
    let Some(stored_path) = stored_path else {
        return Ok(None);
    };
    let path = match Path::new(stored_path).canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let directory = directory
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !path.starts_with(directory) || !path.is_file() {
        return Err("Invalid managed file path.".into());
    }
    let data = fs::read(&path).map_err(|error| error.to_string())?;
    Ok(Some((path, data)))
}

pub(crate) fn discard_file(path: &Path, error: String) -> String {
    match fs::remove_file(path) {
        Ok(()) => error,
        Err(cleanup) if cleanup.kind() == std::io::ErrorKind::NotFound => error,
        Err(cleanup) => format!("{error}. Could not remove {}: {cleanup}", path.display()),
    }
}

pub(crate) async fn commit_file_change(
    transaction: Transaction<'_, Sqlite>,
    new_path: &Path,
    previous: Option<&(PathBuf, Vec<u8>)>,
) -> Result<(), String> {
    if let Some((path, _)) = previous
        && let Err(error) = fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(discard_file(new_path, error.to_string()));
    }
    if let Err(error) = transaction.commit().await {
        if let Some((path, data)) = previous {
            let restoration = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .and_then(|mut file| file.write_all(data));
            if let Err(restoration) = restoration {
                return Err(format!(
                    "{error}. Could not restore {}: {restoration}. Recovery file retained at {}.",
                    path.display(),
                    new_path.display()
                ));
            }
        }
        return Err(discard_file(new_path, error.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    #[test]
    fn restores_original_bytes_after_commit_failure_and_preserves_recovery_on_restore_failure() {
        tauri::async_runtime::block_on(async {
            let root = std::env::temp_dir().join(format!(
                "file-recovery-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&root).unwrap();
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            sqlx::query("CREATE TABLE parent (id INTEGER PRIMARY KEY)")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("CREATE TABLE child (parent_id INTEGER REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED)").execute(&pool).await.unwrap();
            let old = root.join("old.pdf");
            let new = root.join("new.pdf");
            fs::write(&old, b"original").unwrap();
            fs::write(&new, b"replacement").unwrap();
            let previous = read_managed_file(&root, old.to_str()).unwrap().unwrap();
            let mut transaction = pool.begin().await.unwrap();
            sqlx::query("INSERT INTO child VALUES (999)")
                .execute(&mut *transaction)
                .await
                .unwrap();
            assert!(
                commit_file_change(transaction, &new, Some(&previous))
                    .await
                    .is_err()
            );
            assert_eq!(fs::read(&old).unwrap(), b"original");
            assert!(!new.exists());
            assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM child")
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                0
            );
            fs::write(&new, b"only recovery copy").unwrap();
            let previous = (root.join("missing/old.pdf"), b"original".to_vec());
            let mut transaction = pool.begin().await.unwrap();
            sqlx::query("INSERT INTO child VALUES (999)")
                .execute(&mut *transaction)
                .await
                .unwrap();
            let error = commit_file_change(transaction, &new, Some(&previous))
                .await
                .unwrap_err();
            assert!(error.contains("Recovery file retained"));
            assert!(new.exists());
            assert!(read_managed_file(&root.join("other"), old.to_str()).is_err());
            fs::remove_dir_all(root).unwrap();
        });
    }

    #[test]
    fn aborts_and_cleans_new_file_when_the_previous_file_cannot_be_removed() {
        tauri::async_runtime::block_on(async {
            let root = std::env::temp_dir().join(format!(
                "file-cleanup-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&root).unwrap();
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            sqlx::query("CREATE TABLE changes (id INTEGER)")
                .execute(&pool)
                .await
                .unwrap();
            let previous = (root.join("old"), b"old data".to_vec());
            fs::create_dir(&previous.0).unwrap();
            let next = root.join("new");
            fs::write(&next, b"new data").unwrap();
            let mut transaction = pool.begin().await.unwrap();
            sqlx::query("INSERT INTO changes VALUES (1)")
                .execute(&mut *transaction)
                .await
                .unwrap();
            assert!(
                commit_file_change(transaction, &next, Some(&previous))
                    .await
                    .is_err()
            );
            assert!(!next.exists());
            assert!(previous.0.is_dir());
            assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM changes")
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                0
            );
            fs::remove_dir_all(root).unwrap();
        });
    }
}
