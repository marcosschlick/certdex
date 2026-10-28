use sqlx::{AssertSqlSafe, SqliteConnection, SqlitePool};
use std::collections::HashMap;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

pub(crate) fn normalize_name(name: &str) -> String {
    name.trim()
        .nfd()
        .filter(|character| !is_combining_mark(*character))
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    for table in ["organizations", "events", "people", "tags"] {
        let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?)")
            .bind(table)
            .fetch_all(&mut *transaction)
            .await?;
        if columns.iter().any(|column| column == "normalized_name") {
            continue;
        }
        sqlx::query(AssertSqlSafe(format!(
            "ALTER TABLE {table} ADD COLUMN normalized_name TEXT NOT NULL DEFAULT ''"
        )))
        .execute(&mut *transaction)
        .await?;
        let rows: Vec<(i64, String)> = sqlx::query_as(AssertSqlSafe(format!(
            "SELECT id, name FROM {table} ORDER BY id"
        )))
        .fetch_all(&mut *transaction)
        .await?;
        let mut canonical = HashMap::new();
        let mut merged_roles = HashMap::new();
        for (id, name) in rows {
            let key = normalize_name(&name);
            if let Some(&target) = canonical.get(&key) {
                merge(&mut transaction, table, id, target, &mut merged_roles).await?;
                sqlx::query(AssertSqlSafe(format!("DELETE FROM {table} WHERE id = ?")))
                    .bind(id)
                    .execute(&mut *transaction)
                    .await?;
            } else {
                sqlx::query(AssertSqlSafe(format!(
                    "UPDATE {table} SET normalized_name = ? WHERE id = ?"
                )))
                .bind(&key)
                .bind(id)
                .execute(&mut *transaction)
                .await?;
                canonical.insert(key, id);
            }
        }
        sqlx::query(AssertSqlSafe(format!(
            "CREATE UNIQUE INDEX {table}_normalized_name ON {table}(normalized_name)"
        )))
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await
}

async fn merge(
    connection: &mut SqliteConnection,
    table: &str,
    source: i64,
    target: i64,
    merged_roles: &mut HashMap<(i64, i64), Vec<String>>,
) -> Result<(), sqlx::Error> {
    match table {
        "organizations" | "events" => {
            let column = if table == "organizations" {
                "organization_id"
            } else {
                "event_id"
            };
            sqlx::query(AssertSqlSafe(format!(
                "UPDATE documents SET {column} = ? WHERE {column} = ?"
            )))
            .bind(target)
            .bind(source)
            .execute(connection)
            .await?;
        }
        "tags" => {
            sqlx::query("INSERT OR IGNORE INTO document_tags (document_id, tag_id) SELECT document_id, ? FROM document_tags WHERE tag_id = ?")
                .bind(target).bind(source).execute(&mut *connection).await?;
            sqlx::query("DELETE FROM document_tags WHERE tag_id = ?")
                .bind(source)
                .execute(connection)
                .await?;
        }
        "people" => {
            let rows: Vec<(i64, String)> = sqlx::query_as(
                "SELECT document_id, role FROM document_people WHERE person_id = ? ORDER BY document_id",
            ).bind(source).fetch_all(&mut *connection).await?;
            for (document, role) in rows {
                let current: Option<String> = sqlx::query_scalar(
                    "SELECT role FROM document_people WHERE document_id = ? AND person_id = ?",
                )
                .bind(document)
                .bind(target)
                .fetch_optional(&mut *connection)
                .await?;
                let roles = merged_roles
                    .entry((document, target))
                    .or_insert_with(|| current.into_iter().collect());
                if !roles.contains(&role) {
                    roles.push(role);
                }
                let role = roles.join("; ");
                sqlx::query("INSERT INTO document_people (document_id, person_id, role) VALUES (?, ?, ?) ON CONFLICT(document_id, person_id) DO UPDATE SET role = excluded.role")
                    .bind(document).bind(target).bind(role).execute(&mut *connection).await?;
            }
            sqlx::query("DELETE FROM document_people WHERE person_id = ?")
                .bind(source)
                .execute(connection)
                .await?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

pub(crate) async fn named_id(
    connection: &mut SqliteConnection,
    table: &str,
    name: &str,
) -> Result<i64, String> {
    if !["organizations", "events", "people", "tags"].contains(&table) {
        return Err("Invalid association type.".into());
    }
    let key = normalize_name(name);
    sqlx::query(AssertSqlSafe(format!(
        "INSERT OR IGNORE INTO {table} (name, normalized_name) VALUES (?, ?)"
    )))
    .bind(name.trim())
    .bind(&key)
    .execute(&mut *connection)
    .await
    .map_err(|error| error.to_string())?;
    sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT id FROM {table} WHERE normalized_name = ?"
    )))
    .bind(key)
    .fetch_one(connection)
    .await
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn legacy_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        crate::database::create_tables(&pool).await.unwrap();
        for table in ["organizations", "events", "people", "tags"] {
            sqlx::query(AssertSqlSafe(format!("DROP INDEX {table}_normalized_name")))
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query(AssertSqlSafe(format!(
                "ALTER TABLE {table} DROP COLUMN normalized_name"
            )))
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(AssertSqlSafe(format!("INSERT INTO {table} (id, name) VALUES (1, 'São Paulo'), (2, 'SAO PAULO'), (3, 'SÃO PAULO')"))).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO documents (id, type, title, organization_id, event_id, file_hash, extracted_text) VALUES (1, 'course', 'Original', 2, 3, 'hash', 'Preserved text'), (2, 'article', 'Second', 3, 2, 'other-hash', 'Second text')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO document_people VALUES (1, 1, 'Author'), (1, 2, 'Speaker'), (1, 3, 'Author'), (2, 2, 'Advisor'), (2, 3, 'Editor')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO document_tags VALUES (1, 1), (1, 2), (1, 3), (2, 3)")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    #[test]
    fn normalizes_case_and_accents_without_changing_punctuation_or_internal_spaces() {
        assert_eq!(normalize_name(" SÃO Paulo "), "sao paulo");
        assert_eq!(normalize_name("Sa\u{303}o Paulo"), "sao paulo");
        assert_eq!(normalize_name("A-B  C"), "a-b  c");
    }

    #[test]
    fn migrates_existing_duplicates_and_preserves_all_document_links_and_roles() {
        tauri::async_runtime::block_on(async {
            let pool = legacy_pool().await;
            crate::database::create_tables(&pool).await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            for table in ["organizations", "events", "people", "tags"] {
                let rows: Vec<(i64, String, String)> = sqlx::query_as(AssertSqlSafe(format!(
                    "SELECT id, name, normalized_name FROM {table}"
                )))
                .fetch_all(&pool)
                .await
                .unwrap();
                assert_eq!(rows, [(1, "São Paulo".into(), "sao paulo".into())]);
                let mut transaction = pool.begin().await.unwrap();
                assert_eq!(
                    named_id(&mut transaction, table, " sao paulo ")
                        .await
                        .unwrap(),
                    1
                );
                assert_eq!(
                    named_id(&mut transaction, table, "Sa\u{303}o PAULO")
                        .await
                        .unwrap(),
                    1
                );
                transaction.commit().await.unwrap();
                assert!(sqlx::query(AssertSqlSafe(format!("INSERT INTO {table} (name, normalized_name) VALUES ('Other spelling', 'sao paulo')"))).execute(&pool).await.is_err());
            }
            let documents: Vec<(i64, i64, i64, String, String)> = sqlx::query_as("SELECT id, organization_id, event_id, file_hash, extracted_text FROM documents ORDER BY id").fetch_all(&pool).await.unwrap();
            assert_eq!(
                documents,
                [
                    (1, 1, 1, "hash".into(), "Preserved text".into()),
                    (2, 1, 1, "other-hash".into(), "Second text".into())
                ]
            );
            let people: Vec<(i64, i64, String)> = sqlx::query_as(
                "SELECT document_id, person_id, role FROM document_people ORDER BY document_id",
            )
            .fetch_all(&pool)
            .await
            .unwrap();
            assert_eq!(
                people,
                [
                    (1, 1, "Author; Speaker".into()),
                    (2, 1, "Advisor; Editor".into())
                ]
            );
            let tags: Vec<(i64, i64)> = sqlx::query_as(
                "SELECT document_id, tag_id FROM document_tags ORDER BY document_id",
            )
            .fetch_all(&pool)
            .await
            .unwrap();
            assert_eq!(tags, [(1, 1), (2, 1)]);
            let violations: Vec<(String, i64, String, i64)> =
                sqlx::query_as("PRAGMA foreign_key_check")
                    .fetch_all(&pool)
                    .await
                    .unwrap();
            assert!(violations.is_empty());
        });
    }

    #[test]
    fn failed_migration_rolls_back_schema_and_existing_data() {
        tauri::async_runtime::block_on(async {
            let pool = legacy_pool().await;
            sqlx::query("CREATE TRIGGER reject_merge BEFORE UPDATE OF event_id ON documents BEGIN SELECT RAISE(ABORT, 'rejected'); END").execute(&pool).await.unwrap();
            assert!(migrate(&pool).await.is_err());
            let columns: Vec<String> =
                sqlx::query_scalar("SELECT name FROM pragma_table_info('organizations')")
                    .fetch_all(&pool)
                    .await
                    .unwrap();
            assert!(!columns.iter().any(|name| name == "normalized_name"));
            assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM organizations")
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                3
            );
            assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT organization_id FROM documents WHERE id = 1")
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                2
            );
            sqlx::query("DROP TRIGGER reject_merge")
                .execute(&pool)
                .await
                .unwrap();
            migrate(&pool).await.unwrap();
        });
    }

    #[test]
    fn preserves_original_role_values_containing_the_separator() {
        tauri::async_runtime::block_on(async {
            let pool = legacy_pool().await;
            sqlx::query("UPDATE document_people SET role = 'Author; Speaker' WHERE document_id = 1 AND person_id = 1").execute(&pool).await.unwrap();
            migrate(&pool).await.unwrap();
            let role: String = sqlx::query_scalar(
                "SELECT role FROM document_people WHERE document_id = 1 AND person_id = 1",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(role, "Author; Speaker; Speaker; Author");
        });
    }
}
