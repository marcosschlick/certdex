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
}
