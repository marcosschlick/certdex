use crate::ollama::{default_embed_url, embedding_model, generate_embedding_at};
use serde::Serialize;
use sqlx::{SqliteConnection, SqlitePool};
use std::collections::HashMap;

const MAX_RESULTS: usize = 10;
const MIN_SEMANTIC_ONLY_SCORE: f64 = 0.25;
const SEMANTIC_WEIGHT: f64 = 0.55;
const FTS_WEIGHT: f64 = 0.45;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct ChatDocumentResult {
    pub(crate) id: i64,
    pub(crate) title: Option<String>,
    pub(crate) document_type: String,
    pub(crate) organization: Option<String>,
    pub(crate) issue_date: Option<String>,
    pub(crate) file_path: String,
    pub(crate) score: f64,
    pub(crate) matched_by: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct ChatSearchResponse {
    pub(crate) query: String,
    pub(crate) results: Vec<ChatDocumentResult>,
    pub(crate) used_embeddings: bool,
}

#[derive(sqlx::FromRow)]
struct DocumentIndexRow {
    id: i64,
    title: Option<String>,
    r#type: String,
    role: Option<String>,
    organization: Option<String>,
    event: Option<String>,
    city: Option<String>,
    country: Option<String>,
    validation_code: Option<String>,
    extracted_text: Option<String>,
}

#[derive(sqlx::FromRow)]
struct DocumentMetaRow {
    id: i64,
    title: Option<String>,
    r#type: String,
    organization: Option<String>,
    issue_date: Option<String>,
    file_path: Option<String>,
}

pub(crate) async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS document_embeddings (
            document_id INTEGER PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
            model TEXT NOT NULL,
            embedding BLOB NOT NULL,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(
            document_id UNINDEXED,
            title,
            document_type,
            role,
            organization,
            event,
            people,
            tags,
            city,
            country,
            validation_code,
            extracted_text,
            tokenize='unicode61 remove_diacritics 2'
        )",
    )
    .execute(pool)
    .await?;

    let missing_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT d.id FROM documents d WHERE NOT EXISTS (SELECT 1 FROM documents_fts f WHERE f.document_id = d.id)",
    )
    .fetch_all(pool)
    .await?;
    if !missing_ids.is_empty() {
        let mut connection = pool.acquire().await?;
        for id in missing_ids {
            sync_document_fts(&mut connection, id)
                .await
                .map_err(sqlx::Error::Protocol)?;
        }
    }
    Ok(())
}

async fn load_index_fields(
    connection: &mut SqliteConnection,
    document_id: i64,
) -> Result<Option<(DocumentIndexRow, String, String)>, String> {
    let row: Option<DocumentIndexRow> = sqlx::query_as(
        "SELECT d.id, d.title, d.type, d.role, o.name AS organization, e.name AS event, d.city, d.country, d.validation_code, d.extracted_text
         FROM documents d
         LEFT JOIN organizations o ON o.id = d.organization_id
         LEFT JOIN events e ON e.id = d.event_id
         WHERE d.id = ?",
    )
    .bind(document_id)
    .fetch_optional(&mut *connection)
    .await
    .map_err(|_| "database.error".to_string())?;
    let Some(row) = row else {
        return Ok(None);
    };
    let people_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT p.name, dp.role FROM document_people dp JOIN people p ON p.id = dp.person_id WHERE dp.document_id = ? ORDER BY p.name",
    )
    .bind(document_id)
    .fetch_all(&mut *connection)
    .await
    .map_err(|_| "database.error".to_string())?;
    let people = people_rows
        .into_iter()
        .map(|(name, role)| format!("{name} {role}"))
        .collect::<Vec<_>>()
        .join(" ");
    let tag_rows: Vec<String> = sqlx::query_scalar(
        "SELECT t.name FROM document_tags dt JOIN tags t ON t.id = dt.tag_id WHERE dt.document_id = ? ORDER BY t.name",
    )
    .bind(document_id)
    .fetch_all(&mut *connection)
    .await
    .map_err(|_| "database.error".to_string())?;
    let tags = tag_rows.join(" ");
    Ok(Some((row, people, tags)))
}

pub(crate) async fn sync_document_fts(
    connection: &mut SqliteConnection,
    document_id: i64,
) -> Result<(), String> {
    delete_document_fts(connection, document_id).await?;
    let Some((row, people, tags)) = load_index_fields(connection, document_id).await? else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO documents_fts (
            document_id, title, document_type, role, organization, event, people, tags, city, country, validation_code, extracted_text
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(row.id)
    .bind(row.title.unwrap_or_default())
    .bind(row.r#type)
    .bind(row.role.unwrap_or_default())
    .bind(row.organization.unwrap_or_default())
    .bind(row.event.unwrap_or_default())
    .bind(people)
    .bind(tags)
    .bind(row.city.unwrap_or_default())
    .bind(row.country.unwrap_or_default())
    .bind(row.validation_code.unwrap_or_default())
    .bind(row.extracted_text.unwrap_or_default())
    .execute(&mut *connection)
    .await
    .map_err(|_| "database.error".to_string())?;
    Ok(())
}

pub(crate) async fn delete_document_fts(
    connection: &mut SqliteConnection,
    document_id: i64,
) -> Result<(), String> {
    sqlx::query("DELETE FROM documents_fts WHERE document_id = ?")
        .bind(document_id)
        .execute(&mut *connection)
        .await
        .map_err(|_| "database.error".to_string())?;
    Ok(())
}

pub(crate) async fn build_document_search_text(
    pool: &SqlitePool,
    document_id: i64,
) -> Result<Option<String>, String> {
    let mut connection = pool.acquire().await.map_err(|_| "database.error".to_string())?;
    let Some((row, people, tags)) = load_index_fields(&mut connection, document_id).await? else {
        return Ok(None);
    };
    let parts = [
        row.title.unwrap_or_default(),
        row.r#type,
        row.role.unwrap_or_default(),
        row.organization.unwrap_or_default(),
        row.event.unwrap_or_default(),
        people,
        tags,
        row.city.unwrap_or_default(),
        row.country.unwrap_or_default(),
        row.validation_code.unwrap_or_default(),
        row.extracted_text.unwrap_or_default(),
    ];
    let combined = parts
        .into_iter()
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Ok((!combined.is_empty()).then_some(combined))
}

fn encode_embedding(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn decode_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| f32::from_le_bytes(*chunk))
        .collect()
}

pub(crate) async fn save_document_embedding(
    pool: &SqlitePool,
    document_id: i64,
    model: &str,
    embedding: &[f32],
) -> Result<(), String> {
    let blob = encode_embedding(embedding);
    sqlx::query(
        "INSERT INTO document_embeddings (document_id, model, embedding, updated_at)
         VALUES (?, ?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(document_id) DO UPDATE SET
            model = excluded.model,
            embedding = excluded.embedding,
            updated_at = CURRENT_TIMESTAMP",
    )
    .bind(document_id)
    .bind(model)
    .bind(blob)
    .execute(pool)
    .await
    .map_err(|_| "database.error".to_string())?;
    Ok(())
}

pub(crate) async fn try_refresh_document_embedding(pool: &SqlitePool, document_id: i64) {
    let _ = try_refresh_document_embedding_at(pool, default_embed_url(), document_id).await;
}

pub(crate) async fn try_refresh_document_embedding_at(
    pool: &SqlitePool,
    embed_url: &str,
    document_id: i64,
) -> Result<bool, String> {
    let Some(text) = build_document_search_text(pool, document_id).await? else {
        return Ok(false);
    };
    let Ok(embedding) = generate_embedding_at(embed_url, &text).await else {
        return Ok(false);
    };
    save_document_embedding(pool, document_id, embedding_model(), &embedding).await?;
    Ok(true)
}

pub(crate) fn sanitize_fts_query(query: &str) -> String {
    let tokens: Vec<String> = query
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{token}\"*"))
        .collect();
    tokens.join(" OR ")
}

pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0_f64;
    let mut norm_a = 0.0_f64;
    let mut norm_b = 0.0_f64;
    for (&x, &y) in a.iter().zip(b.iter()) {
        let xf = f64::from(x);
        let yf = f64::from(y);
        dot += xf * yf;
        norm_a += xf * xf;
        norm_b += yf * yf;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        (dot / denom).clamp(0.0, 1.0)
    }
}

pub(crate) async fn search_documents(
    pool: &SqlitePool,
    embed_url: &str,
    query: &str,
) -> Result<ChatSearchResponse, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err("search.empty_query".into());
    }

    let mut fts_scores: HashMap<i64, f64> = HashMap::new();
    let fts_query = sanitize_fts_query(trimmed);
    if !fts_query.is_empty() {
        let fts_rows: Vec<(i64, f64)> = sqlx::query_as(
            "SELECT document_id, bm25(documents_fts) AS rank FROM documents_fts WHERE documents_fts MATCH ? ORDER BY rank LIMIT 30",
        )
        .bind(&fts_query)
        .fetch_all(pool)
        .await
        .map_err(|_| "database.error".to_string())?;
        for (doc_id, rank) in fts_rows {
            let normalized = 1.0 / (1.0 + (-rank).max(0.0).recip().min(100.0));
            fts_scores.insert(doc_id, normalized.max(0.1));
        }
        if !fts_scores.is_empty() {
            let max_fts = fts_scores.values().copied().fold(0.0_f64, f64::max);
            if max_fts > 0.0 {
                for score in fts_scores.values_mut() {
                    *score /= max_fts;
                }
            }
        }
    }

    let mut semantic_scores: HashMap<i64, f64> = HashMap::new();
    let used_embeddings = if let Ok(query_embedding) = generate_embedding_at(embed_url, trimmed).await {
        let rows: Vec<(i64, Vec<u8>)> = sqlx::query_as(
            "SELECT document_id, embedding FROM document_embeddings WHERE model = ?",
        )
        .bind(embedding_model())
        .fetch_all(pool)
        .await
        .map_err(|_| "database.error".to_string())?;
        for (doc_id, blob) in rows {
            let doc_embedding = decode_embedding(&blob);
            let similarity = cosine_similarity(&query_embedding, &doc_embedding);
            if similarity >= MIN_SEMANTIC_ONLY_SCORE || fts_scores.contains_key(&doc_id) {
                semantic_scores.insert(doc_id, similarity);
            }
        }
        true
    } else {
        false
    };

    let mut candidate_ids: Vec<i64> = fts_scores
        .keys()
        .chain(semantic_scores.keys())
        .copied()
        .collect();
    candidate_ids.sort_unstable();
    candidate_ids.dedup();

    let mut scored: Vec<(i64, f64, &'static str)> = Vec::with_capacity(candidate_ids.len());
    for doc_id in candidate_ids {
        let fts = fts_scores.get(&doc_id).copied();
        let sem = semantic_scores.get(&doc_id).copied();
        let (combined, matched_by) = match (fts, sem) {
            (Some(f), Some(s)) => (SEMANTIC_WEIGHT * s + FTS_WEIGHT * f + 0.15, "hybrid"),
            (Some(f), None) => (f, "keyword"),
            (None, Some(s)) => (s * SEMANTIC_WEIGHT, "semantic"),
            (None, None) => continue,
        };
        scored.push((doc_id, combined, matched_by));
    }

    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.0.cmp(&a.0))
    });
    scored.truncate(MAX_RESULTS);

    let mut results = Vec::with_capacity(scored.len());
    for (doc_id, score, matched_by) in scored {
        let meta: Option<DocumentMetaRow> = sqlx::query_as(
            "SELECT d.id, d.title, d.type, o.name AS organization, COALESCE(d.issue_date, d.start_date, d.end_date) AS issue_date, d.file_path
             FROM documents d
             LEFT JOIN organizations o ON o.id = d.organization_id
             WHERE d.id = ?",
        )
        .bind(doc_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| "database.error".to_string())?;
        if let Some(meta) = meta
            && let Some(file_path) = meta.file_path
        {
            results.push(ChatDocumentResult {
                id: meta.id,
                title: meta.title,
                document_type: meta.r#type,
                organization: meta.organization,
                issue_date: meta.issue_date,
                file_path,
                score,
                matched_by: matched_by.to_string(),
            });
        }
    }

    Ok(ChatSearchResponse {
        query: trimmed.to_string(),
        results,
        used_embeddings,
    })
}

#[tauri::command]
pub(crate) async fn search_documents_chat(
    pool: tauri::State<'_, SqlitePool>,
    query: String,
) -> Result<ChatSearchResponse, String> {
    search_documents(&pool, default_embed_url(), &query).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        crate::database::create_tables(&pool).await.unwrap();
        pool
    }

    async fn insert_doc(
        pool: &SqlitePool,
        doc_type: &str,
        title: &str,
        organization: Option<&str>,
        file_path: &str,
        extracted_text: &str,
    ) -> i64 {
        let org_id = if let Some(name) = organization {
            sqlx::query("INSERT OR IGNORE INTO organizations (name, normalized_name) VALUES (?, ?)")
                .bind(name)
                .bind(crate::entities::normalize_name(name))
                .execute(pool)
                .await
                .unwrap();
            Some(
                sqlx::query_scalar::<_, i64>("SELECT id FROM organizations WHERE normalized_name = ?")
                    .bind(crate::entities::normalize_name(name))
                    .fetch_one(pool)
                    .await
                    .unwrap(),
            )
        } else {
            None
        };
        let id = sqlx::query(
            "INSERT INTO documents (type, title, organization_id, file_path, extracted_text) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(doc_type)
        .bind(title)
        .bind(org_id)
        .bind(file_path)
        .bind(extracted_text)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid();
        let mut conn = pool.acquire().await.unwrap();
        sync_document_fts(&mut conn, id).await.unwrap();
        id
    }

    fn mock_embed_server(responses: Vec<Vec<f32>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for embedding in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0; 4096];
                let _ = stream.read(&mut buffer).unwrap();
                let body = serde_json::json!({ "embeddings": [embedding] }).to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        format!("http://{address}/api/embed")
    }

    fn unreachable_url() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        format!("http://{address}/api/embed")
    }

    #[test]
    fn fts5_matches_ignoring_accents_and_handles_punctuation() {
        tauri::async_runtime::block_on(async {
            let pool = setup_pool().await;
            let doc_id = insert_doc(
                &pool,
                "certificate",
                "Certificação em Programação Rust",
                Some("IFSul São Paulo"),
                "/tmp/certs/rust.pdf",
                "Participação em oficina de computação.",
            )
            .await;

            let response = search_documents(&pool, &unreachable_url(), "certificacao programacao sao paulo!")
                .await
                .unwrap();
            assert!(!response.used_embeddings);
            assert_eq!(response.results.len(), 1);
            assert_eq!(response.results[0].id, doc_id);
            assert_eq!(response.results[0].file_path, "/tmp/certs/rust.pdf");
            assert_eq!(response.results[0].matched_by, "keyword");
            assert_eq!(response.query, "certificacao programacao sao paulo!");
        });
    }

    #[test]
    fn hybrid_search_combines_nomic_embeddings_and_fts5() {
        tauri::async_runtime::block_on(async {
            let pool = setup_pool().await;
            let id1 = insert_doc(
                &pool,
                "course",
                "Machine Learning Foundations",
                Some("Coursera"),
                "/docs/ml.pdf",
                "Neural networks and gradient descent",
            )
            .await;
            let id2 = insert_doc(
                &pool,
                "certificate",
                "Web Accessibility Workshop",
                Some("W3C"),
                "/docs/a11y.pdf",
                "ARIA roles and keyboard navigation",
            )
            .await;

            let embed_url = mock_embed_server(vec![
                vec![1.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0],
                vec![0.98, 0.02, 0.0],
            ]);
            assert!(try_refresh_document_embedding_at(&pool, &embed_url, id1).await.unwrap());
            assert!(try_refresh_document_embedding_at(&pool, &embed_url, id2).await.unwrap());

            let response = search_documents(&pool, &embed_url, "deep learning")
                .await
                .unwrap();
            assert!(response.used_embeddings);
            assert_eq!(response.results.len(), 1);
            assert_eq!(response.results[0].id, id1);
            assert_eq!(response.results[0].matched_by, "hybrid");
            assert_eq!(response.results[0].file_path, "/docs/ml.pdf");
        });
    }

    #[test]
    fn backfills_existing_documents_into_fts_on_migrate() {
        tauri::async_runtime::block_on(async {
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            sqlx::query(
                "INSERT INTO documents (type, title, file_path, extracted_text) VALUES ('article', 'Ação Climática', '/docs/clima.pdf', 'Texto')",
            )
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query("DELETE FROM documents_fts").execute(&pool).await.unwrap();

            migrate(&pool).await.unwrap();

            let response = search_documents(&pool, &unreachable_url(), "acao climatica")
                .await
                .unwrap();
            assert_eq!(response.results.len(), 1);
            assert_eq!(response.results[0].file_path, "/docs/clima.pdf");
        });
    }

    #[test]
    fn returns_fixed_empty_reply_and_rejects_blank_queries() {
        tauri::async_runtime::block_on(async {
            let pool = setup_pool().await;
            assert_eq!(
                search_documents(&pool, &unreachable_url(), "   ").await.unwrap_err(),
                "search.empty_query"
            );

            let response = search_documents(&pool, &unreachable_url(), "quantum")
                .await
                .unwrap();
            assert!(response.results.is_empty());
            assert_eq!(response.query, "quantum");
        });
    }

    #[test]
    fn semantic_only_matches_and_respects_similarity_threshold_and_limit() {
        tauri::async_runtime::block_on(async {
            let pool = setup_pool().await;
            let mut ids = Vec::new();
            for index in 0..12 {
                let id = insert_doc(
                    &pool,
                    "course",
                    &format!("Topic {index}"),
                    None,
                    &format!("/docs/topic-{index}.pdf"),
                    "Completely unrelated keywords",
                )
                .await;
                ids.push(id);
            }
            let low_id = insert_doc(
                &pool,
                "course",
                "Low Similarity",
                None,
                "/docs/low.pdf",
                "Unrelated",
            )
            .await;

            for (index, id) in ids.iter().enumerate() {
                let weight = 1.0 - (index as f32) * 0.02;
                save_document_embedding(&pool, *id, embedding_model(), &[weight, 0.2, 0.0])
                    .await
                    .unwrap();
            }
            save_document_embedding(&pool, low_id, embedding_model(), &[0.0, 1.0, 0.0])
                .await
                .unwrap();

            let embed_url = mock_embed_server(vec![vec![1.0, 0.0, 0.0]]);
            let response = search_documents(&pool, &embed_url, "conceptual inquiry")
                .await
                .unwrap();
            assert!(response.used_embeddings);
            assert_eq!(response.results.len(), MAX_RESULTS);
            assert!(response.results.iter().all(|item| item.matched_by == "semantic"));
            assert!(!response.results.iter().any(|item| item.id == low_id));

            assert_eq!(cosine_similarity(&[], &[1.0]), 0.0);
            assert_eq!(cosine_similarity(&[1.0], &[1.0, 0.0]), 0.0);
            assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
        });
    }
}
