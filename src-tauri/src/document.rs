use crate::entities::named_id;
use crate::managed_file::{commit_file_change, discard_file, read_managed_file};
const SHA256_CONSTANTS: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256(data: &[u8]) -> String {
    let mut hash: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut padded = data.to_vec();
    let bit_length = (data.len() as u64).wrapping_mul(8);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());
    for chunk in padded.as_chunks::<64>().0 {
        let mut words = [0u32; 64];
        for (word, bytes) in words.iter_mut().zip(chunk.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*bytes);
        }
        for i in 16..64 {
            let s0 = words[i - 15].rotate_right(7)
                ^ words[i - 15].rotate_right(18)
                ^ (words[i - 15] >> 3);
            let s1 = words[i - 2].rotate_right(17)
                ^ words[i - 2].rotate_right(19)
                ^ (words[i - 2] >> 10);
            words[i] = words[i - 16]
                .wrapping_add(s0)
                .wrapping_add(words[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = hash;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(SHA256_CONSTANTS[i])
                .wrapping_add(words[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (value, result) in hash.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *value = value.wrapping_add(result);
        }
    }
    hash.iter().map(|value| format!("{value:08x}")).collect()
}

#[tauri::command]
pub(crate) fn hash_pdf(data: Vec<u8>) -> Result<String, String> {
    if !data.starts_with(b"%PDF-") {
        return Err("The selected file is not a PDF.".into());
    }
    Ok(sha256(&data))
}

async fn contains_hash(pool: &SqlitePool, hash: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM documents WHERE file_hash = ?)")
        .bind(hash)
        .fetch_one(pool)
        .await
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub(crate) struct DocumentListItem {
    id: i64,
    title: Option<String>,
    r#type: String,
    issue_date: Option<String>,
    workload_hours: Option<f64>,
    role: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    city: Option<String>,
    country: Option<String>,
    validation_code: Option<String>,
    validation_url: Option<String>,
    organization: Option<String>,
    event: Option<String>,
    #[sqlx(skip)]
    people: Vec<PersonInput>,
    #[sqlx(skip)]
    tags: Vec<String>,
}

async fn load_documents(pool: &SqlitePool) -> Result<Vec<DocumentListItem>, sqlx::Error> {
    let mut documents: Vec<DocumentListItem> = sqlx::query_as(
        "SELECT d.id, d.title, d.type, d.issue_date, d.workload_hours, d.role, d.start_date, d.end_date, d.city, d.country, d.validation_code, d.validation_url, o.name AS organization, e.name AS event FROM documents d LEFT JOIN organizations o ON o.id = d.organization_id LEFT JOIN events e ON e.id = d.event_id ORDER BY d.id DESC",
    ).fetch_all(pool).await?;
    for document in &mut documents {
        document.people = sqlx::query_as("SELECT p.name, dp.role FROM document_people dp JOIN people p ON p.id = dp.person_id WHERE dp.document_id = ? ORDER BY p.name")
            .bind(document.id).fetch_all(pool).await?;
        document.tags = sqlx::query_scalar("SELECT t.name FROM document_tags dt JOIN tags t ON t.id = dt.tag_id WHERE dt.document_id = ? ORDER BY t.name")
            .bind(document.id).fetch_all(pool).await?;
    }
    Ok(documents)
}

#[derive(serde::Serialize)]
pub(crate) struct AcademicOptions {
    organizations: Vec<String>,
    events: Vec<String>,
    people: Vec<String>,
    tags: Vec<String>,
}

#[tauri::command]
pub(crate) async fn list_academic_options(
    pool: State<'_, SqlitePool>,
) -> Result<AcademicOptions, String> {
    async fn names(pool: &SqlitePool, query: &'static str) -> Result<Vec<String>, String> {
        sqlx::query_scalar(query)
            .fetch_all(pool)
            .await
            .map_err(|error| error.to_string())
    }
    Ok(AcademicOptions {
        organizations: names(&pool, "SELECT name FROM organizations ORDER BY name").await?,
        events: names(&pool, "SELECT name FROM events ORDER BY name").await?,
        people: names(&pool, "SELECT name FROM people ORDER BY name").await?,
        tags: names(&pool, "SELECT name FROM tags ORDER BY name").await?,
    })
}

#[tauri::command]
pub(crate) async fn list_documents(
    pool: State<'_, SqlitePool>,
) -> Result<Vec<DocumentListItem>, String> {
    load_documents(&pool)
        .await
        .map_err(|error| error.to_string())
}

async fn document_pdf_path(pool: &SqlitePool, root: &Path, id: i64) -> Result<PathBuf, String> {
    let stored_path: Option<String> =
        sqlx::query_scalar("SELECT file_path FROM documents WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|error| error.to_string())?
            .flatten();
    let path = stored_path.ok_or("PDF not found for this document.")?;
    let directory = root
        .join("documents")
        .canonicalize()
        .map_err(|_| "PDF folder not found.")?;
    let path = Path::new(&path)
        .canonicalize()
        .map_err(|_| "PDF file not found.")?;
    if !path.starts_with(directory)
        || !path.is_file()
        || !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
    {
        return Err("Invalid PDF file path.".into());
    }
    Ok(path)
}

#[tauri::command]
pub(crate) async fn open_document_pdf(
    app: tauri::AppHandle,
    pool: State<'_, SqlitePool>,
    id: i64,
) -> Result<(), String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let path = document_pdf_path(&pool, &root, id).await?;
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let program = "xdg-open";
    std::process::Command::new(program)
        .arg(path)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn document_hash_exists(
    pool: State<'_, SqlitePool>,
    hash: String,
    exclude_id: Option<i64>,
) -> Result<bool, String> {
    if let Some(id) = exclude_id {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM documents WHERE file_hash = ? AND id != ?)")
            .bind(hash)
            .bind(id)
            .fetch_one(&*pool)
            .await
            .map_err(|error| error.to_string())
    } else {
        contains_hash(&pool, &hash)
            .await
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGINAL_PDF: &[u8] = include_bytes!("../tests/fixtures/sample.pdf");
    const REPLACEMENT_PDF: &[u8] = include_bytes!("../tests/fixtures/searchable.pdf");

    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn document_input() -> DocumentInput {
        serde_json::from_value(serde_json::json!({
            "document_type": "course", "title": "Original", "issue_date": "2025-01-01",
            "original_filename": "source.pdf", "extracted_text": "Original text", "tags": ["São Paulo", "SAO PAULO"]
        })).unwrap()
    }

    #[test]
    fn validates_calendar_dates_in_every_document_date_field() {
        for date in [
            "0001-01-01",
            "1900-02-28",
            "2000-02-29",
            "2024-02-29",
            "2026-04-30",
            "2026-12-31",
            "9999-12-31",
        ] {
            let document = DocumentInput {
                issue_date: Some(date.into()),
                start_date: Some(date.into()),
                end_date: Some(date.into()),
                ..document_input()
            };
            document.validate().unwrap();
        }
        for date in [
            "",
            "2026-99-99",
            "2026-00-01",
            "2026-01-00",
            "2026-01-32",
            "2026-04-31",
            "2025-02-29",
            "1900-02-29",
            "2100-02-29",
            "2024-02-30",
            "0000-01-01",
            "2026-1-01",
            "26-01-01",
            "10000-01-01",
            " 2026-01-01",
            "2026-01-01 ",
            "2026-01-01T00:00:00Z",
            "2026/01/01",
            "２０２６-01-01",
        ] {
            for field in ["Issue date", "Start date", "End date"] {
                let mut document = document_input();
                let target = match field {
                    "Issue date" => &mut document.issue_date,
                    "Start date" => &mut document.start_date,
                    "End date" => &mut document.end_date,
                    _ => unreachable!(),
                };
                *target = Some(date.into());
                let error = document.validate().unwrap_err();
                assert!(error.contains(field), "{field}: {date:?}: {error}");
            }
        }
        let document = DocumentInput {
            issue_date: None,
            start_date: None,
            end_date: None,
            ..document_input()
        };
        document.validate().unwrap();
    }

    #[test]
    fn rejects_invalid_dates_before_registering_or_updating_documents() {
        tauri::async_runtime::block_on(async {
            let root = test_root("document-dates");
            let rejected_root = root.join("rejected");
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            let path = register_document(&root, &pool, ORIGINAL_PDF, &document_input())
                .await
                .unwrap();
            let documents = load_documents(&pool).await.unwrap();
            let id = documents[0].id;
            let original = serde_json::to_value(documents).unwrap();
            for field in ["issue_date", "start_date", "end_date"] {
                let mut document = document_input();
                document.title = Some("Rejected".into());
                let target = match field {
                    "issue_date" => &mut document.issue_date,
                    "start_date" => &mut document.start_date,
                    "end_date" => &mut document.end_date,
                    _ => unreachable!(),
                };
                *target = Some("2026-99-99".into());
                assert!(
                    register_document(&rejected_root, &pool, REPLACEMENT_PDF, &document)
                        .await
                        .unwrap_err()
                        .contains("valid date")
                );
                for data in [None, Some(REPLACEMENT_PDF)] {
                    assert!(
                        update_document_record(&root, &pool, id, &document, data)
                            .await
                            .unwrap_err()
                            .contains("valid date")
                    );
                }
            }
            assert_eq!(
                serde_json::to_value(load_documents(&pool).await.unwrap()).unwrap(),
                original
            );
            assert_eq!(fs::read(&path).unwrap(), ORIGINAL_PDF);
            assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
            assert!(!rejected_root.exists());
            fs::remove_dir_all(root).unwrap();
        });
    }

    #[test]
    fn persists_empty_text_on_manual_registration_and_pdf_replacement() {
        tauri::async_runtime::block_on(async {
            let root = test_root("document-manual-registration");
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            let mut document = document_input();
            document.extracted_text.clear();
            register_document(&root, &pool, ORIGINAL_PDF, &document)
                .await
                .unwrap();
            let id = load_documents(&pool).await.unwrap()[0].id;
            let text: String =
                sqlx::query_scalar("SELECT extracted_text FROM documents WHERE id = ?")
                    .bind(id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert!(text.is_empty());
            sqlx::query(
                "UPDATE documents SET extracted_text = 'Previously extracted text' WHERE id = ?",
            )
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
            update_document_record(&root, &pool, id, &document, Some(REPLACEMENT_PDF))
                .await
                .unwrap();
            let text: String =
                sqlx::query_scalar("SELECT extracted_text FROM documents WHERE id = ?")
                    .bind(id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert!(text.is_empty());
            assert_eq!(
                fs::read(document_pdf_path(&pool, &root, id).await.unwrap()).unwrap(),
                REPLACEMENT_PDF
            );
            fs::remove_dir_all(root).unwrap();
        });
    }

    #[test]
    fn rejects_invalid_pdfs_without_changing_existing_documents_or_files() {
        tauri::async_runtime::block_on(async {
            let root = test_root("document-pdf-validation");
            let rejected_root = root.join("rejected");
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            let path = register_document(&root, &pool, ORIGINAL_PDF, &document_input())
                .await
                .unwrap();
            let documents = load_documents(&pool).await.unwrap();
            let id = documents[0].id;
            let original = serde_json::to_value(documents).unwrap();
            let mut document = document_input();
            document.title = Some("Rejected".into());
            document.organization = Some("Rejected organization".into());
            document.tags = vec!["Rejected tag".into()];
            document.extracted_text = "Rejected text".into();
            for data in [
                b"not a PDF".as_slice(),
                b"%PDF-".as_slice(),
                b"%PDF-1.4\nnot a document\n%%EOF".as_slice(),
                &ORIGINAL_PDF[..80],
            ] {
                assert!(
                    register_document(&rejected_root, &pool, data, &document)
                        .await
                        .is_err()
                );
                assert!(
                    update_document_record(&root, &pool, id, &document, Some(data))
                        .await
                        .is_err()
                );
            }
            assert_eq!(
                serde_json::to_value(load_documents(&pool).await.unwrap()).unwrap(),
                original
            );
            let stored: (String, String, String) = sqlx::query_as(
                "SELECT file_path, file_hash, extracted_text FROM documents WHERE id = ?",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(
                stored,
                (
                    path.to_string_lossy().into_owned(),
                    sha256(ORIGINAL_PDF),
                    "Original text".into()
                )
            );
            assert_eq!(fs::read(&path).unwrap(), ORIGINAL_PDF);
            assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
            assert!(!rejected_root.exists());
            fs::remove_dir_all(root).unwrap();
        });
    }

    #[test]
    fn rejects_invalid_hours_and_periods_without_touching_documents_or_files() {
        tauri::async_runtime::block_on(async {
            let root = test_root("document-validation");
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            let mut document = document_input();
            for hours in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                document.workload_hours = Some(hours);
                assert!(
                    register_document(&root, &pool, ORIGINAL_PDF, &document)
                        .await
                        .is_err()
                );
            }
            assert!(!root.exists());
            document.workload_hours = Some(0.0);
            document.start_date = Some("2025-02-02".into());
            document.end_date = Some("2025-02-01".into());
            assert!(
                register_document(&root, &pool, ORIGINAL_PDF, &document)
                    .await
                    .is_err()
            );
            assert!(!root.exists());
            document.end_date = document.start_date.clone();
            let path = register_document(&root, &pool, ORIGINAL_PDF, &document)
                .await
                .unwrap();
            let id = load_documents(&pool).await.unwrap()[0].id;
            for hours in [-1.0, f64::NAN, f64::INFINITY] {
                document.workload_hours = Some(hours);
                assert!(
                    update_document_record(&root, &pool, id, &document, Some(REPLACEMENT_PDF))
                        .await
                        .is_err()
                );
            }
            document.workload_hours = None;
            document.end_date = Some("2025-02-01".into());
            assert!(
                update_document_record(&root, &pool, id, &document, None)
                    .await
                    .is_err()
            );
            assert_eq!(fs::read(&path).unwrap(), ORIGINAL_PDF);
            assert_eq!(
                load_documents(&pool).await.unwrap()[0].workload_hours,
                Some(0.0)
            );
            assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
            document.end_date = None;
            document.validate().unwrap();
            document.start_date = None;
            document.end_date = Some("2025-01-01".into());
            document.validate().unwrap();
            fs::remove_dir_all(root).unwrap();
        });
    }

    #[test]
    fn reorganizes_pdfs_and_rolls_back_failed_file_updates_and_commits() {
        tauri::async_runtime::block_on(async {
            let root = test_root("document-relocation");
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            let mut document = document_input();
            let original = register_document(&root, &pool, ORIGINAL_PDF, &document)
                .await
                .unwrap();
            let id = load_documents(&pool).await.unwrap()[0].id;
            document.document_type = "article".into();
            document.title = Some("Updated".into());
            document.issue_date = None;
            document.start_date = Some("2026-01-01".into());
            document.original_filename = "must not overwrite.pdf".into();
            document.extracted_text = "must not overwrite".into();
            let collision =
                store_pdf(&root, REPLACEMENT_PDF, "article", "2026", "Updated").unwrap();
            update_document_record(&root, &pool, id, &document, None)
                .await
                .unwrap();
            let relocated = document_pdf_path(&pool, &root, id).await.unwrap();
            assert!(relocated.ends_with("2026/article/2026-article-updated-1.pdf"));
            assert!(!original.exists());
            assert_eq!(fs::read(&collision).unwrap(), REPLACEMENT_PDF);
            let metadata: (String, String, String, String) = sqlx::query_as("SELECT original_filename, file_hash, extracted_text, stored_filename FROM documents WHERE id = ?").bind(id).fetch_one(&pool).await.unwrap();
            assert_eq!(
                metadata,
                (
                    "source.pdf".into(),
                    sha256(ORIGINAL_PDF),
                    "Original text".into(),
                    "2026-article-updated-1.pdf".into()
                )
            );
            let unchanged: String =
                sqlx::query_scalar("SELECT file_path FROM documents WHERE id = ?")
                    .bind(id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            document.role = Some("Speaker".into());
            update_document_record(&root, &pool, id, &document, None)
                .await
                .unwrap();
            assert_eq!(
                document_pdf_path(&pool, &root, id).await.unwrap(),
                PathBuf::from(unchanged)
            );
            document.title = Some("Rejected".into());
            sqlx::query("CREATE TRIGGER reject_path BEFORE UPDATE OF file_path ON documents BEGIN SELECT RAISE(ABORT, 'rejected'); END").execute(&pool).await.unwrap();
            assert!(
                update_document_record(&root, &pool, id, &document, None)
                    .await
                    .is_err()
            );
            assert!(relocated.exists());
            assert_eq!(
                load_documents(&pool).await.unwrap()[0].title.as_deref(),
                Some("Updated")
            );
            assert_eq!(
                fs::read_dir(relocated.parent().unwrap()).unwrap().count(),
                2
            );
            sqlx::query("DROP TRIGGER reject_path")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("CREATE TABLE commit_parent (id INTEGER PRIMARY KEY)")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("CREATE TABLE commit_child (id INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED)").execute(&pool).await.unwrap();
            sqlx::query("CREATE TRIGGER reject_commit AFTER UPDATE OF file_path ON documents BEGIN INSERT INTO commit_child VALUES (999); END").execute(&pool).await.unwrap();
            assert!(
                update_document_record(&root, &pool, id, &document, Some(REPLACEMENT_PDF))
                    .await
                    .is_err()
            );
            assert_eq!(fs::read(&relocated).unwrap(), ORIGINAL_PDF);
            assert_eq!(
                load_documents(&pool).await.unwrap()[0].title.as_deref(),
                Some("Updated")
            );
            assert_eq!(
                fs::read_dir(relocated.parent().unwrap()).unwrap().count(),
                2
            );
            sqlx::query("DROP TRIGGER reject_commit")
                .execute(&pool)
                .await
                .unwrap();
            fs::remove_file(&relocated).unwrap();
            assert!(
                update_document_record(&root, &pool, id, &document, None)
                    .await
                    .is_err()
            );
            update_document_record(&root, &pool, id, &document, Some(REPLACEMENT_PDF))
                .await
                .unwrap();
            assert_eq!(
                fs::read(document_pdf_path(&pool, &root, id).await.unwrap()).unwrap(),
                REPLACEMENT_PDF
            );
            fs::remove_dir_all(root).unwrap();
        });
    }

    #[test]
    fn hashes_known_vectors() {
        assert_eq!(
            sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        assert!(hash_pdf(b"not a pdf".to_vec()).is_err());
    }

    #[test]
    fn finds_duplicates_by_hash_only() {
        tauri::async_runtime::block_on(async {
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            sqlx::query("INSERT INTO documents (type, file_hash, original_filename) VALUES ('certificate', ?, ?)")
                .bind("existing-hash")
                .bind("document.pdf")
                .execute(&pool)
                .await
                .unwrap();

            assert!(contains_hash(&pool, "existing-hash").await.unwrap());
            assert!(!contains_hash(&pool, "different-hash").await.unwrap());
        });
    }

    #[test]
    fn lists_saved_documents_with_optional_fields() {
        tauri::async_runtime::block_on(async {
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            assert!(load_documents(&pool).await.unwrap().is_empty());
            sqlx::query("INSERT INTO documents (type, title, issue_date, workload_hours) VALUES ('course', 'Rust', '2026-01-01', 12.5), ('other', NULL, NULL, NULL)")
                .execute(&pool).await.unwrap();
            let documents = load_documents(&pool).await.unwrap();
            assert_eq!(documents.len(), 2);
            assert_eq!(documents[0].r#type, "other");
            assert_eq!(documents[0].title, None);
            assert_eq!(documents[1].title.as_deref(), Some("Rust"));
            assert_eq!(documents[1].issue_date.as_deref(), Some("2026-01-01"));
            assert_eq!(documents[1].workload_hours, Some(12.5));
        });
    }

    #[test]
    fn resolves_only_existing_managed_pdfs() {
        tauri::async_runtime::block_on(async {
            let root = std::env::temp_dir().join(format!(
                "open-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            let path = store_pdf(&root, ORIGINAL_PDF, "course", "2026", "test").unwrap();
            sqlx::query("INSERT INTO documents (id, type, file_path) VALUES (1, 'course', ?), (2, 'course', ?), (3, 'course', NULL)")
                .bind(path.to_string_lossy().as_ref())
                .bind(root.join("outside.pdf").to_string_lossy().as_ref())
                .execute(&pool).await.unwrap();
            fs::write(root.join("outside.pdf"), ORIGINAL_PDF).unwrap();
            assert_eq!(
                document_pdf_path(&pool, &root, 1).await.unwrap(),
                path.canonicalize().unwrap()
            );
            assert!(document_pdf_path(&pool, &root, 2).await.is_err());
            assert!(document_pdf_path(&pool, &root, 3).await.is_err());
            assert!(document_pdf_path(&pool, &root, 4).await.is_err());
            fs::remove_file(path).unwrap();
            assert!(document_pdf_path(&pool, &root, 1).await.is_err());
            fs::remove_dir_all(root).unwrap();
        });
    }

    #[test]
    fn stores_pdfs_with_safe_unique_names() {
        let root = std::env::temp_dir().join(format!(
            "documents-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = store_pdf(&root, ORIGINAL_PDF, "course", "2026", "../My: Course").unwrap();
        let second = store_pdf(&root, REPLACEMENT_PDF, "course", "2026", "../My: Course").unwrap();
        assert_eq!(
            first,
            root.join("documents/2026/course/2026-course-my--course.pdf")
        );
        assert_eq!(
            second,
            root.join("documents/2026/course/2026-course-my--course-1.pdf")
        );
        assert_eq!(fs::read(first).unwrap(), ORIGINAL_PDF);
        assert_eq!(fs::read(second).unwrap(), REPLACEMENT_PDF);
        assert!(store_pdf(&root, ORIGINAL_PDF, "../course", "2026", "Title").is_err());
        assert!(store_pdf(&root, ORIGINAL_PDF, "course", "../x", "Title").is_err());
        assert!(store_pdf(&root, b"invalid", "course", "2026", "Title").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registers_document_and_cleans_up_on_database_failure() {
        tauri::async_runtime::block_on(async {
            use sqlx::Row;
            let root = std::env::temp_dir().join(format!(
                "register-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            let document = DocumentInput {
                document_type: "course".into(),
                title: Some("Title".into()),
                role: Some("Student".into()),
                issue_date: Some("2026-01-01".into()),
                start_date: None,
                end_date: None,
                workload_hours: Some(12.5),
                city: Some("City".into()),
                country: Some("Country".into()),
                validation_code: Some("Code".into()),
                validation_url: Some("https://example.com".into()),
                original_filename: "original.pdf".into(),
                extracted_text: "Extracted text\nwith accents: ação".into(),
                organization: None,
                event: None,
                people: vec![],
                tags: vec![],
            };
            let path = register_document(&root, &pool, ORIGINAL_PDF, &document)
                .await
                .unwrap();
            let row = sqlx::query("SELECT * FROM documents")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(row.get::<String, _>("original_filename"), "original.pdf");
            assert_eq!(
                row.get::<String, _>("stored_filename"),
                "2026-course-title.pdf"
            );
            assert_eq!(row.get::<String, _>("file_path"), path.to_string_lossy());
            assert_eq!(row.get::<String, _>("file_hash"), sha256(ORIGINAL_PDF));
            assert_eq!(
                row.get::<String, _>("extracted_text"),
                "Extracted text\nwith accents: ação"
            );
            assert_eq!(row.get::<f64, _>("workload_hours"), 12.5);
            assert_eq!(row.get::<String, _>("role"), "Student");
            assert_eq!(row.get::<Option<String>, _>("start_date"), None);
            assert!(
                register_document(&root, &pool, ORIGINAL_PDF, &document)
                    .await
                    .is_err()
            );
            sqlx::query("CREATE TRIGGER reject_document BEFORE INSERT ON documents BEGIN SELECT RAISE(ABORT, 'rejected'); END")
                .execute(&pool).await.unwrap();
            assert!(
                register_document(&root, &pool, REPLACEMENT_PDF, &document)
                    .await
                    .is_err()
            );
            assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM documents")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(count, 1);
            fs::remove_dir_all(root).unwrap();
        });
    }

    #[test]
    fn saves_and_updates_associations_and_preserves_or_replaces_pdf() {
        tauri::async_runtime::block_on(async {
            let root = std::env::temp_dir().join(format!(
                "academic-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            let mut document: DocumentInput = serde_json::from_value(serde_json::json!({
                "document_type": "course", "title": "First", "original_filename": "first.pdf",
                "extracted_text": "original text", "organization": "University", "event": "Congress",
                "people": [{"name": "Ada", "role": "Author"}], "tags": ["Rust", "rust"]
            })).unwrap();
            let original = register_document(&root, &pool, ORIGINAL_PDF, &document)
                .await
                .unwrap();
            let list = load_documents(&pool).await.unwrap();
            assert_eq!(list[0].organization.as_deref(), Some("University"));
            assert_eq!(list[0].event.as_deref(), Some("Congress"));
            assert_eq!(list[0].people[0].role, "Author");
            assert_eq!(list[0].tags, ["Rust"]);

            document.title = Some("Updated".into());
            document.organization = Some("university".into());
            document.people[0].role = "Speaker".into();
            document.tags = vec!["Research".into()];
            document.extracted_text = "must not overwrite".into();
            update_document_record(&root, &pool, list[0].id, &document, None)
                .await
                .unwrap();
            let updated = load_documents(&pool).await.unwrap();
            assert_eq!(updated[0].title.as_deref(), Some("Updated"));
            assert_eq!(updated[0].people[0].role, "Speaker");
            assert_eq!(updated[0].tags, ["Research"]);
            assert_eq!(
                sqlx::query_scalar::<_, String>(
                    "SELECT extracted_text FROM documents WHERE id = ?"
                )
                .bind(list[0].id)
                .fetch_one(&pool)
                .await
                .unwrap(),
                "original text"
            );
            let reorganized = document_pdf_path(&pool, &root, list[0].id).await.unwrap();
            assert!(!original.exists());
            assert!(reorganized.ends_with("unknown-course-updated.pdf"));
            assert_eq!(fs::read(&reorganized).unwrap(), ORIGINAL_PDF);
            assert_eq!(
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM organizations")
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                1
            );

            document.people.push(PersonInput {
                name: "ada".into(),
                role: "Author".into(),
            });
            document.title = Some("Should roll back".into());
            assert!(
                update_document_record(&root, &pool, list[0].id, &document, None)
                    .await
                    .is_err()
            );
            let unchanged = load_documents(&pool).await.unwrap();
            assert_eq!(unchanged[0].title.as_deref(), Some("Updated"));
            assert_eq!(unchanged[0].people[0].role, "Speaker");
            document.people.pop();

            document.original_filename = "replacement.pdf".into();
            document.extracted_text = "replacement text".into();
            update_document_record(&root, &pool, list[0].id, &document, Some(REPLACEMENT_PDF))
                .await
                .unwrap();
            let replacement = document_pdf_path(&pool, &root, list[0].id).await.unwrap();
            assert!(!original.exists());
            assert!(!reorganized.exists());
            assert_eq!(fs::read(replacement).unwrap(), REPLACEMENT_PDF);
            assert_eq!(
                sqlx::query_scalar::<_, String>(
                    "SELECT extracted_text FROM documents WHERE id = ?"
                )
                .bind(list[0].id)
                .fetch_one(&pool)
                .await
                .unwrap(),
                "replacement text"
            );
            fs::remove_dir_all(root).unwrap();
        });
    }
}
use sqlx::SqlitePool;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tauri::{Manager, State};

fn store_pdf(
    root: &Path,
    data: &[u8],
    document_type: &str,
    year: &str,
    title: &str,
) -> Result<PathBuf, String> {
    if !data.starts_with(b"%PDF-") {
        return Err("The selected file is not a PDF.".into());
    }
    let (relative, base) = pdf_location(document_type, year, title)?;
    let directory = root.join("documents").join(relative);
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    for number in 0u32.. {
        let name = if number == 0 {
            format!("{base}.pdf")
        } else {
            format!("{base}-{number}.pdf")
        };
        let path = directory.join(name);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                if let Err(error) = file.write_all(data) {
                    drop(file);
                    return Err(discard_file(&path, error.to_string()));
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("Could not create a unique filename.".into())
}

fn pdf_location(document_type: &str, year: &str, title: &str) -> Result<(PathBuf, String), String> {
    if ![
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
    ]
    .contains(&document_type)
    {
        return Err("Invalid document type.".into());
    }
    if year != "unknown" && (year.len() != 4 || !year.bytes().all(|byte| byte.is_ascii_digit())) {
        return Err("Invalid year.".into());
    }
    let title: String = title
        .chars()
        .take(80)
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let title = title.trim_matches('-');
    let title = if title.is_empty() { "document" } else { title };
    Ok((
        PathBuf::from(year).join(document_type),
        format!("{year}-{document_type}-{title}"),
    ))
}

fn document_year(date: Option<&str>) -> &str {
    date.and_then(|value| value.get(..4)).unwrap_or("unknown")
}

fn is_valid_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        value[..4].parse::<u16>(),
        value[5..7].parse::<u16>(),
        value[8..].parse::<u16>(),
    ) else {
        return false;
    };
    let leap_year =
        year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100));
    let days = match month {
        2 if leap_year => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return false,
    };
    year > 0 && day > 0 && day <= days
}

#[derive(serde::Deserialize)]
pub(crate) struct DocumentInput {
    document_type: String,
    title: Option<String>,
    role: Option<String>,
    issue_date: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    workload_hours: Option<f64>,
    city: Option<String>,
    country: Option<String>,
    validation_code: Option<String>,
    validation_url: Option<String>,
    original_filename: String,
    extracted_text: String,
    #[serde(default)]
    organization: Option<String>,
    #[serde(default)]
    event: Option<String>,
    #[serde(default)]
    people: Vec<PersonInput>,
    #[serde(default)]
    tags: Vec<String>,
}

impl DocumentInput {
    fn year(&self) -> &str {
        document_year(
            self.issue_date
                .as_deref()
                .or(self.start_date.as_deref())
                .or(self.end_date.as_deref()),
        )
    }

    fn validate(&self) -> Result<(), String> {
        for (field, date) in [
            ("Issue date", &self.issue_date),
            ("Start date", &self.start_date),
            ("End date", &self.end_date),
        ] {
            if let Some(date) = date
                && !is_valid_date(date)
            {
                return Err(format!(
                    "{field} must be a valid date in YYYY-MM-DD format."
                ));
            }
        }
        if self
            .workload_hours
            .is_some_and(|hours| !hours.is_finite() || hours < 0.0)
        {
            return Err("Workload must be a finite, non-negative number.".into());
        }
        if let (Some(start), Some(end)) = (&self.start_date, &self.end_date)
            && start > end
        {
            return Err("The start date must not be after the end date.".into());
        }
        pdf_location(
            &self.document_type,
            self.year(),
            self.title.as_deref().unwrap_or(""),
        )?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct StoredDocument {
    r#type: String,
    title: Option<String>,
    date: Option<String>,
    file_path: Option<String>,
}

#[derive(serde::Deserialize, serde::Serialize, sqlx::FromRow)]
struct PersonInput {
    name: String,
    role: String,
}

async fn save_associations(
    connection: &mut sqlx::SqliteConnection,
    id: i64,
    document: &DocumentInput,
) -> Result<(), String> {
    let organization = match document
        .organization
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        Some(name) => Some(named_id(connection, "organizations", name).await?),
        None => None,
    };
    let event = match document
        .event
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        Some(name) => Some(named_id(connection, "events", name).await?),
        None => None,
    };
    sqlx::query("UPDATE documents SET organization_id = ?, event_id = ? WHERE id = ?")
        .bind(organization)
        .bind(event)
        .bind(id)
        .execute(&mut *connection)
        .await
        .map_err(|error| error.to_string())?;
    sqlx::query("DELETE FROM document_people WHERE document_id = ?")
        .bind(id)
        .execute(&mut *connection)
        .await
        .map_err(|error| error.to_string())?;
    sqlx::query("DELETE FROM document_tags WHERE document_id = ?")
        .bind(id)
        .execute(&mut *connection)
        .await
        .map_err(|error| error.to_string())?;
    for person in &document.people {
        let name = person.name.trim();
        let role = person.role.trim();
        if name.is_empty() || role.is_empty() {
            return Err("Each person must have a name and role.".into());
        }
        let person_id = named_id(connection, "people", name).await?;
        sqlx::query("INSERT INTO document_people (document_id, person_id, role) VALUES (?, ?, ?)")
            .bind(id)
            .bind(person_id)
            .bind(role)
            .execute(&mut *connection)
            .await
            .map_err(|error| {
                if error
                    .as_database_error()
                    .is_some_and(|error| error.is_unique_violation())
                {
                    "A person can only be added once per document.".to_string()
                } else {
                    error.to_string()
                }
            })?;
    }
    for tag in &document.tags {
        let name = tag.trim();
        if name.is_empty() {
            continue;
        }
        let tag_id = named_id(connection, "tags", name).await?;
        sqlx::query("INSERT OR IGNORE INTO document_tags (document_id, tag_id) VALUES (?, ?)")
            .bind(id)
            .bind(tag_id)
            .execute(&mut *connection)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

async fn register_document(
    root: &Path,
    pool: &SqlitePool,
    data: &[u8],
    document: &DocumentInput,
) -> Result<PathBuf, String> {
    document.validate()?;
    crate::pdf::validate_pdf(data).await?;
    let hash = sha256(data);
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM documents WHERE file_hash = ?)")
            .bind(&hash)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|error| error.to_string())?;
    if exists {
        return Err("This file has already been registered.".into());
    }
    let year = document.year();
    let path = store_pdf(
        root,
        data,
        &document.document_type,
        year,
        document.title.as_deref().unwrap_or(""),
    )?;
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let result = sqlx::query("INSERT INTO documents (type, title, role, issue_date, start_date, end_date, workload_hours, city, country, validation_code, validation_url, original_filename, stored_filename, file_path, file_hash, extracted_text) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&document.document_type)
        .bind(&document.title)
        .bind(&document.role)
        .bind(&document.issue_date)
        .bind(&document.start_date)
        .bind(&document.end_date)
        .bind(document.workload_hours)
        .bind(&document.city)
        .bind(&document.country)
        .bind(&document.validation_code)
        .bind(&document.validation_url)
        .bind(&document.original_filename)
        .bind(filename)
        .bind(path.to_string_lossy().as_ref())
        .bind(&hash)
        .bind(&document.extracted_text)
        .execute(&mut *transaction)
        .await;
    let result = match result {
        Ok(result) => {
            save_associations(&mut transaction, result.last_insert_rowid(), document).await
        }
        Err(error) => Err(error.to_string()),
    };
    if let Err(error) = result {
        return Err(discard_file(&path, error));
    }
    commit_file_change(transaction, &path, None).await?;
    Ok(path)
}

async fn update_document_record(
    root: &Path,
    pool: &SqlitePool,
    id: i64,
    document: &DocumentInput,
    data: Option<&[u8]>,
) -> Result<(), String> {
    document.validate()?;
    if let Some(data) = data {
        crate::pdf::validate_pdf(data).await?;
    }
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;
    let original: StoredDocument = sqlx::query_as("SELECT type, title, COALESCE(issue_date, start_date, end_date) AS date, file_path FROM documents WHERE id = ?")
        .bind(id).fetch_optional(&mut *transaction).await.map_err(|error| error.to_string())?
        .ok_or("Document not found.")?;
    let old_location = pdf_location(
        &original.r#type,
        document_year(original.date.as_deref()),
        original.title.as_deref().unwrap_or(""),
    )?;
    let new_location = pdf_location(
        &document.document_type,
        document.year(),
        document.title.as_deref().unwrap_or(""),
    )?;
    let relocate = old_location != new_location;
    let previous = if data.is_some() || relocate {
        read_managed_file(&root.join("documents"), original.file_path.as_deref())?
    } else {
        None
    };
    if relocate && data.is_none() && previous.is_none() {
        return Err("The managed PDF is required to reorganize this document.".into());
    }
    let result = sqlx::query("UPDATE documents SET type = ?, title = ?, role = ?, issue_date = ?, start_date = ?, end_date = ?, workload_hours = ?, city = ?, country = ?, validation_code = ?, validation_url = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(&document.document_type).bind(&document.title).bind(&document.role)
        .bind(&document.issue_date).bind(&document.start_date).bind(&document.end_date)
        .bind(document.workload_hours).bind(&document.city).bind(&document.country)
        .bind(&document.validation_code).bind(&document.validation_url).bind(id)
        .execute(&mut *transaction).await.map_err(|error| error.to_string())?;
    if result.rows_affected() == 0 {
        return Err("Document not found.".into());
    }
    save_associations(&mut transaction, id, document).await?;
    if data.is_none() && !relocate {
        return transaction
            .commit()
            .await
            .map_err(|error| error.to_string());
    }
    let pdf = data
        .or_else(|| previous.as_ref().map(|(_, bytes)| bytes.as_slice()))
        .ok_or("PDF not found for this document.")?;
    if !pdf.starts_with(b"%PDF-") {
        return Err("The selected file is not a PDF.".into());
    }
    let hash = sha256(pdf);
    if data.is_some() {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM documents WHERE file_hash = ? AND id != ?)",
        )
        .bind(&hash)
        .bind(id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| error.to_string())?;
        if exists {
            return Err("This file has already been registered.".into());
        }
    }
    let path = store_pdf(
        root,
        pdf,
        &document.document_type,
        document.year(),
        document.title.as_deref().unwrap_or(""),
    )?;
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let result = if data.is_some() {
        sqlx::query("UPDATE documents SET original_filename = ?, stored_filename = ?, file_path = ?, file_hash = ?, extracted_text = ? WHERE id = ?")
            .bind(&document.original_filename).bind(filename).bind(path.to_string_lossy().as_ref())
            .bind(hash).bind(&document.extracted_text).bind(id).execute(&mut *transaction).await
    } else {
        sqlx::query("UPDATE documents SET stored_filename = ?, file_path = ? WHERE id = ?")
            .bind(filename)
            .bind(path.to_string_lossy().as_ref())
            .bind(id)
            .execute(&mut *transaction)
            .await
    };
    if let Err(error) = result {
        return Err(discard_file(&path, error.to_string()));
    }
    commit_file_change(transaction, &path, previous.as_ref()).await
}

#[tauri::command]
pub(crate) async fn update_document(
    app: tauri::AppHandle,
    pool: State<'_, SqlitePool>,
    id: i64,
    document: DocumentInput,
    data: Option<Vec<u8>>,
) -> Result<(), String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    update_document_record(&root, &pool, id, &document, data.as_deref()).await
}

#[tauri::command]
pub(crate) async fn store_document_pdf(
    app: tauri::AppHandle,
    pool: State<'_, SqlitePool>,
    data: Vec<u8>,
    document: DocumentInput,
) -> Result<String, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let path = register_document(&root, &pool, &data, &document).await?;
    Ok(path.to_string_lossy().into_owned())
}
