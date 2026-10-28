use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::path::Path;

pub async fn initialize(path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    create_tables(&pool).await?;
    Ok(pool)
}

pub(crate) async fn create_tables(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS profile (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            name TEXT,
            email TEXT,
            phone TEXT,
            city TEXT,
            state TEXT,
            country TEXT,
            github TEXT,
            linkedin TEXT,
            orcid TEXT,
            website TEXT,
            professional_summary TEXT,
            academic_summary TEXT
        )",
    )
    .execute(pool)
    .await?;
    let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('profile')")
        .fetch_all(pool)
        .await?;
    if !columns.iter().any(|column| column == "photo_path") {
        sqlx::query("ALTER TABLE profile ADD COLUMN photo_path TEXT")
            .execute(pool)
            .await?;
    }
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS documents (
            id INTEGER PRIMARY KEY,
            type TEXT NOT NULL CHECK (type IN (
                'certificate', 'course', 'article', 'proceedings', 'presentation',
                'event', 'award', 'project', 'internship', 'monitoring', 'other'
            )),
            title TEXT,
            role TEXT,
            issue_date TEXT,
            start_date TEXT,
            end_date TEXT,
            workload_hours REAL,
            city TEXT,
            country TEXT,
            validation_code TEXT,
            validation_url TEXT,
            original_filename TEXT,
            stored_filename TEXT,
            file_path TEXT,
            file_hash TEXT,
            extracted_text TEXT,
            extra_data TEXT,
            organization_id INTEGER,
            event_id INTEGER,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(pool)
    .await?;
    for statement in [
        "CREATE TABLE IF NOT EXISTS organizations (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE)",
        "CREATE TABLE IF NOT EXISTS events (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE)",
        "CREATE TABLE IF NOT EXISTS people (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE)",
        "CREATE TABLE IF NOT EXISTS tags (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE)",
        "CREATE TABLE IF NOT EXISTS document_people (document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE, person_id INTEGER NOT NULL REFERENCES people(id), role TEXT NOT NULL, PRIMARY KEY (document_id, person_id))",
        "CREATE TABLE IF NOT EXISTS document_tags (document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE, tag_id INTEGER NOT NULL REFERENCES tags(id), PRIMARY KEY (document_id, tag_id))",
    ] {
        sqlx::query(statement).execute(pool).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_profile_table_and_limits_it_to_one_profile() {
        tauri::async_runtime::block_on(async {
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            create_tables(&pool).await.unwrap();
            create_tables(&pool).await.unwrap();

            sqlx::query("INSERT INTO profile (id, name) VALUES (1, 'Test')")
                .execute(&pool)
                .await
                .unwrap();
            assert!(
                sqlx::query("INSERT INTO profile (id) VALUES (2)")
                    .execute(&pool)
                    .await
                    .is_err()
            );
            let name: String = sqlx::query_scalar("SELECT name FROM profile WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(name, "Test");
        });
    }

    #[test]
    fn adds_photo_path_to_existing_profile_without_losing_data() {
        tauri::async_runtime::block_on(async {
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            sqlx::query("CREATE TABLE profile (id INTEGER PRIMARY KEY, name TEXT)")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO profile (id, name) VALUES (1, 'Existing')")
                .execute(&pool)
                .await
                .unwrap();
            create_tables(&pool).await.unwrap();
            create_tables(&pool).await.unwrap();
            let (name, photo): (String, Option<String>) =
                sqlx::query_as("SELECT name, photo_path FROM profile WHERE id = 1")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(name, "Existing");
            assert_eq!(photo, None);
        });
    }

    #[test]
    fn documents_accept_known_types_and_optional_fields() {
        tauri::async_runtime::block_on(async {
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            create_tables(&pool).await.unwrap();
            create_tables(&pool).await.unwrap();

            for document_type in [
                "certificate",
                "course",
                "article",
                "proceedings",
                "presentation",
                "event",
                "award",
                "project",
                "internship",
                "monitoring",
                "other",
            ] {
                sqlx::query("INSERT INTO documents (type) VALUES (?)")
                    .bind(document_type)
                    .execute(&pool)
                    .await
                    .unwrap();
            }

            assert!(
                sqlx::query("INSERT INTO documents (type) VALUES ('invalid')")
                    .execute(&pool)
                    .await
                    .is_err()
            );
            assert!(
                sqlx::query("INSERT INTO documents DEFAULT VALUES")
                    .execute(&pool)
                    .await
                    .is_err()
            );
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM documents WHERE title IS NULL AND created_at IS NOT NULL AND updated_at IS NOT NULL",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(count, 11);
        });
    }
}
