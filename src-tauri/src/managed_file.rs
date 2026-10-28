use sqlx::{Sqlite, Transaction};
use std::{
    fs,
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
    if let Err(error) = transaction.commit().await {
        return Err(discard_file(new_path, error.to_string()));
    }
    if let Some((path, _)) = previous
        && let Err(error) = fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(format!(
            "Changes were saved, but the previous file could not be removed ({}): {error}. The new file was preserved.",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    #[test]
    fn preserves_previous_file_and_cleans_new_file_when_commit_fails() {
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
            fs::write(&old, b"original updated after reading").unwrap();
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
            assert_eq!(fs::read(&old).unwrap(), b"original updated after reading");
            assert!(!new.exists());
            assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM child")
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                0
            );
            assert!(read_managed_file(&root.join("other"), old.to_str()).is_err());
            fs::remove_dir_all(root).unwrap();
        });
    }

    #[test]
    fn preserves_committed_file_and_database_when_previous_file_cleanup_fails() {
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
            sqlx::query("CREATE TABLE changes (file_path TEXT)")
                .execute(&pool)
                .await
                .unwrap();
            let previous = (root.join("old"), b"old data".to_vec());
            fs::create_dir(&previous.0).unwrap();
            let next = root.join("new");
            fs::write(&next, b"new data").unwrap();
            sqlx::query("INSERT INTO changes VALUES (?)")
                .bind(previous.0.to_string_lossy().as_ref())
                .execute(&pool)
                .await
                .unwrap();
            let mut transaction = pool.begin().await.unwrap();
            sqlx::query("UPDATE changes SET file_path = ?")
                .bind(next.to_string_lossy().as_ref())
                .execute(&mut *transaction)
                .await
                .unwrap();
            let error = commit_file_change(transaction, &next, Some(&previous))
                .await
                .unwrap_err();
            assert!(error.contains("Changes were saved"));
            assert!(error.contains("previous file could not be removed"));
            assert_eq!(fs::read(&next).unwrap(), b"new data");
            assert!(previous.0.is_dir());
            assert_eq!(
                sqlx::query_scalar::<_, String>("SELECT file_path FROM changes")
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                next.to_string_lossy().as_ref()
            );
            fs::remove_dir_all(root).unwrap();
        });
    }
}
