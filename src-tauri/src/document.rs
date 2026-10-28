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
    for chunk in padded.chunks_exact(64) {
        let mut words = [0u32; 64];
        for (word, bytes) in words.iter_mut().zip(chunk.chunks_exact(4)) {
            *word = u32::from_be_bytes(bytes.try_into().unwrap());
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

#[tauri::command]
pub(crate) async fn document_hash_exists(
    pool: State<'_, SqlitePool>,
    hash: String,
) -> Result<bool, String> {
    contains_hash(&pool, &hash)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn stores_pdfs_with_safe_unique_names() {
        let root = std::env::temp_dir().join(format!(
            "documents-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = store_pdf(&root, b"%PDF-first", "course", "2026", "../My: Course").unwrap();
        let second = store_pdf(&root, b"%PDF-second", "course", "2026", "../My: Course").unwrap();
        assert_eq!(
            first,
            root.join("documents/2026/course/2026-course-my--course.pdf")
        );
        assert_eq!(
            second,
            root.join("documents/2026/course/2026-course-my--course-1.pdf")
        );
        assert_eq!(fs::read(first).unwrap(), b"%PDF-first");
        assert_eq!(fs::read(second).unwrap(), b"%PDF-second");
        assert!(store_pdf(&root, b"%PDF-", "../course", "2026", "Title").is_err());
        assert!(store_pdf(&root, b"%PDF-", "course", "../x", "Title").is_err());
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
            };
            let path = register_document(&root, &pool, b"%PDF-first", &document)
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
            assert_eq!(row.get::<String, _>("file_hash"), sha256(b"%PDF-first"));
            assert_eq!(row.get::<f64, _>("workload_hours"), 12.5);
            assert_eq!(row.get::<String, _>("role"), "Student");
            assert_eq!(row.get::<Option<String>, _>("start_date"), None);
            assert!(
                register_document(&root, &pool, b"%PDF-first", &document)
                    .await
                    .is_err()
            );
            sqlx::query("CREATE TRIGGER reject_document BEFORE INSERT ON documents BEGIN SELECT RAISE(ABORT, 'rejected'); END")
                .execute(&pool).await.unwrap();
            assert!(
                register_document(&root, &pool, b"%PDF-second", &document)
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
    let directory = root.join("documents").join(year).join(document_type);
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let base = format!("{year}-{document_type}-{title}");
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
                    let _ = fs::remove_file(&path);
                    return Err(error.to_string());
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("Could not create a unique filename.".into())
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
}

async fn register_document(
    root: &Path,
    pool: &SqlitePool,
    data: &[u8],
    document: &DocumentInput,
) -> Result<PathBuf, String> {
    if !data.starts_with(b"%PDF-") {
        return Err("The selected file is not a PDF.".into());
    }
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
    let date = document
        .issue_date
        .as_deref()
        .or(document.start_date.as_deref())
        .or(document.end_date.as_deref());
    let year = date.and_then(|value| value.get(..4)).unwrap_or("unknown");
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
    let result = sqlx::query("INSERT INTO documents (type, title, role, issue_date, start_date, end_date, workload_hours, city, country, validation_code, validation_url, original_filename, stored_filename, file_path, file_hash) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
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
        .execute(&mut *transaction)
        .await;
    if let Err(error) = result {
        if let Err(cleanup_error) = fs::remove_file(&path) {
            return Err(format!(
                "{error}. Could not remove {}: {cleanup_error}",
                path.display()
            ));
        }
        return Err(error.to_string());
    }
    if let Err(error) = transaction.commit().await {
        if let Err(cleanup_error) = fs::remove_file(&path) {
            return Err(format!(
                "{error}. Could not remove {}: {cleanup_error}",
                path.display()
            ));
        }
        return Err(error.to_string());
    }
    Ok(path)
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
