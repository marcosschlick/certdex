use crate::managed_file::{commit_file_change, discard_file, resolve_managed_file};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tauri::{Manager, State};

#[derive(Debug, Default, Deserialize, Serialize, FromRow, PartialEq)]
pub(crate) struct Profile {
    name: String,
    email: String,
    phone: String,
    city: String,
    state: String,
    country: String,
    github: String,
    linkedin: String,
    orcid: String,
    website: String,
    professional_summary: String,
    academic_summary: String,
    #[serde(default)]
    documents_dir: String,
}

pub(crate) fn resolve_documents_dir(home: &Path, configured: &str) -> Result<PathBuf, String> {
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        return Ok(home.join("Certdex"));
    }
    let path = if trimmed == "~" {
        home.to_path_buf()
    } else if let Some(rest) = trimmed.strip_prefix("~/").or_else(|| trimmed.strip_prefix("~\\")) {
        home.join(rest)
    } else {
        PathBuf::from(trimmed)
    };
    if !path.is_absolute() {
        return Err("Documents folder must be an absolute path or start with ~/.".into());
    }
    Ok(path)
}

pub(crate) async fn load_documents_dir(pool: &SqlitePool, home: &Path) -> Result<PathBuf, String> {
    let configured: Option<String> =
        sqlx::query_scalar("SELECT documents_dir FROM profile WHERE id = 1")
            .fetch_optional(pool)
            .await
            .map_err(|error| error.to_string())?
            .flatten();
    resolve_documents_dir(home, configured.as_deref().unwrap_or(""))
}

async fn load(pool: &SqlitePool) -> Result<Profile, sqlx::Error> {
    Ok(sqlx::query_as::<_, Profile>(
        "SELECT COALESCE(name, '') AS name, COALESCE(email, '') AS email,
            COALESCE(phone, '') AS phone, COALESCE(city, '') AS city,
            COALESCE(state, '') AS state, COALESCE(country, '') AS country,
            COALESCE(github, '') AS github, COALESCE(linkedin, '') AS linkedin,
            COALESCE(orcid, '') AS orcid, COALESCE(website, '') AS website,
            COALESCE(professional_summary, '') AS professional_summary,
            COALESCE(academic_summary, '') AS academic_summary,
            COALESCE(documents_dir, '') AS documents_dir
        FROM profile WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or_default())
}

async fn save(pool: &SqlitePool, home: &Path, profile: &Profile) -> Result<(), String> {
    if [
        &profile.name,
        &profile.email,
        &profile.phone,
        &profile.city,
        &profile.state,
        &profile.country,
    ]
    .iter()
    .any(|value| value.trim().is_empty())
    {
        return Err("Please complete all required profile fields.".into());
    }
    resolve_documents_dir(home, &profile.documents_dir)?;
    sqlx::query(
        "INSERT INTO profile (id, name, email, phone, city, state, country, github, linkedin, orcid, website, professional_summary, academic_summary, documents_dir)
        VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET name = excluded.name, email = excluded.email,
            phone = excluded.phone, city = excluded.city, state = excluded.state,
            country = excluded.country, github = excluded.github, linkedin = excluded.linkedin,
            orcid = excluded.orcid, website = excluded.website,
            professional_summary = excluded.professional_summary, academic_summary = excluded.academic_summary,
            documents_dir = excluded.documents_dir",
    )
    .bind(&profile.name)
    .bind(&profile.email)
    .bind(&profile.phone)
    .bind(&profile.city)
    .bind(&profile.state)
    .bind(&profile.country)
    .bind(&profile.github)
    .bind(&profile.linkedin)
    .bind(&profile.orcid)
    .bind(&profile.website)
    .bind(&profile.professional_summary)
    .bind(&profile.academic_summary)
    .bind(profile.documents_dir.trim())
    .execute(pool)
    .await.map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn get_profile(pool: State<'_, SqlitePool>) -> Result<Profile, String> {
    load(&pool).await.map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn save_profile(
    app: tauri::AppHandle,
    pool: State<'_, SqlitePool>,
    profile: Profile,
) -> Result<(), String> {
    let home = app.path().home_dir().map_err(|error| error.to_string())?;
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let previous_dir = load_documents_dir(&pool, &home).await?;
    save(&pool, &home, &profile).await?;
    let next_dir = resolve_documents_dir(&home, &profile.documents_dir)?;
    if next_dir != previous_dir {
        let legacy_dir = data_dir.join("documents");
        crate::document::reorganize_documents(&pool, &next_dir, &[&previous_dir, &legacy_dir])
            .await?;
    }
    Ok(())
}

fn image_type(data: &[u8]) -> Option<(&'static str, &'static str)> {
    let format = image::guess_format(data).ok()?;
    let kind = match format {
        image::ImageFormat::Png => ("png", "image/png"),
        image::ImageFormat::Jpeg => ("jpg", "image/jpeg"),
        image::ImageFormat::Gif => ("gif", "image/gif"),
        image::ImageFormat::WebP => ("webp", "image/webp"),
        _ => return None,
    };
    image::ImageReader::with_format(std::io::Cursor::new(data), format)
        .decode()
        .ok()?;
    Some(kind)
}

#[derive(Serialize)]
pub(crate) struct ProfilePhoto {
    data: Vec<u8>,
    mime_type: &'static str,
}

async fn save_photo(root: &Path, pool: &SqlitePool, data: &[u8]) -> Result<(), String> {
    let (extension, _) = image_type(data).ok_or("Please select a PNG, JPEG, GIF or WebP image.")?;
    let directory = root.join("profile");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create profile folder: {error}"))?;
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;
    let stored_path: Option<String> =
        sqlx::query_scalar("SELECT photo_path FROM profile WHERE id = 1")
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|error| error.to_string())?
            .flatten();
    let previous = resolve_managed_file(&[&directory], stored_path.as_deref())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let mut number = 0u64;
    let (path, mut file) = loop {
        let path = directory.join(format!("photo-{stamp}-{number}.{extension}"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => break (path, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => number += 1,
            Err(error) => return Err(format!("Could not save profile photo: {error}")),
        }
    };
    if let Err(error) = file.write_all(data) {
        drop(file);
        return Err(discard_file(
            &path,
            format!("Could not save profile photo: {error}"),
        ));
    }
    drop(file);
    let result = sqlx::query("INSERT INTO profile (id, photo_path) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET photo_path = excluded.photo_path")
        .bind(path.to_string_lossy().as_ref())
        .execute(&mut *transaction).await;
    if let Err(error) = result {
        return Err(discard_file(
            &path,
            format!("Could not save profile photo path: {error}"),
        ));
    }
    commit_file_change(transaction, &path, previous.as_deref()).await
}

async fn load_photo(root: &Path, pool: &SqlitePool) -> Result<Option<ProfilePhoto>, String> {
    let path: Option<String> = sqlx::query_scalar("SELECT photo_path FROM profile WHERE id = 1")
        .fetch_optional(pool)
        .await
        .map_err(|error| error.to_string())?
        .flatten();
    let Some(path) = path else { return Ok(None) };
    let directory = root
        .join("profile")
        .canonicalize()
        .map_err(|error| format!("Could not access profile folder: {error}"))?;
    let path = Path::new(&path)
        .canonicalize()
        .map_err(|error| format!("Could not access profile photo: {error}"))?;
    if !path.starts_with(directory) || !path.is_file() {
        return Err("Invalid profile photo path.".into());
    }
    let data = fs::read(path).map_err(|error| format!("Could not read profile photo: {error}"))?;
    let (_, mime_type) = image_type(&data).ok_or("Invalid profile photo format.")?;
    Ok(Some(ProfilePhoto { data, mime_type }))
}

#[tauri::command]
pub(crate) async fn save_profile_photo(
    app: tauri::AppHandle,
    pool: State<'_, SqlitePool>,
    data: Vec<u8>,
) -> Result<(), String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    save_photo(&root, &pool, &data).await
}

#[tauri::command]
pub(crate) async fn get_profile_photo(
    app: tauri::AppHandle,
    pool: State<'_, SqlitePool>,
) -> Result<Option<ProfilePhoto>, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    load_photo(&root, &pool).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_profile() -> Profile {
        Profile {
            name: "Existing".into(),
            email: "name@example.com".into(),
            phone: "123".into(),
            city: "City".into(),
            state: "State".into(),
            country: "Country".into(),
            ..Profile::default()
        }
    }

    fn encoded_image(format: image::ImageFormat) -> Vec<u8> {
        let mut output = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut output, format)
            .unwrap();
        output.into_inner()
    }

    #[test]
    fn rejects_missing_required_profile_fields_without_overwriting_data() {
        tauri::async_runtime::block_on(async {
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            let home = std::env::temp_dir();
            let profile = complete_profile();
            save(&pool, &home, &profile).await.unwrap();
            for field in ["name", "email", "phone", "city", "state", "country"] {
                for missing in ["", " \t\n "] {
                    let mut data = serde_json::to_value(&profile).unwrap();
                    data[field] = serde_json::json!(missing);
                    let invalid = serde_json::from_value(data).unwrap();
                    assert!(save(&pool, &home, &invalid).await.is_err());
                    assert_eq!(load(&pool).await.unwrap(), profile);
                }
            }
        });
    }

    #[test]
    fn decodes_supported_images_and_rejects_truncated_or_fake_images() {
        for (format, extension) in [
            (image::ImageFormat::Png, "png"),
            (image::ImageFormat::Jpeg, "jpg"),
            (image::ImageFormat::Gif, "gif"),
            (image::ImageFormat::WebP, "webp"),
        ] {
            let bytes = encoded_image(format);
            assert_eq!(image_type(&bytes).unwrap().0, extension);
            assert!(image_type(&bytes[..bytes.len() / 2]).is_none());
        }
        for fake in [
            b"\x89PNG\r\n\x1a\nfake".as_slice(),
            b"\xff\xd8\xfffake",
            b"GIF89afake",
            b"RIFF0000WEBPfake",
        ] {
            assert!(image_type(fake).is_none());
        }
    }

    #[test]
    fn profile_can_be_created_and_updated() {
        tauri::async_runtime::block_on(async {
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            assert_eq!(load(&pool).await.unwrap(), Profile::default());
            let home = std::env::temp_dir();
            let mut profile = Profile {
                name: "Name".into(),
                email: "Email".into(),
                phone: "Phone".into(),
                city: "City".into(),
                state: "State".into(),
                country: "Country".into(),
                github: "GitHub".into(),
                linkedin: "LinkedIn".into(),
                orcid: "ORCID".into(),
                website: "Website".into(),
                professional_summary: "Professional".into(),
                academic_summary: "Academic".into(),
                documents_dir: "~/CustomCerts".into(),
            };
            save(&pool, &home, &profile).await.unwrap();
            assert_eq!(load(&pool).await.unwrap(), profile);
            profile.name = "Updated".into();
            save(&pool, &home, &profile).await.unwrap();
            assert_eq!(load(&pool).await.unwrap(), profile);
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM profile")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(count, 1);
        });
    }

    #[test]
    fn saves_and_replaces_profile_photo_without_changing_profile() {
        tauri::async_runtime::block_on(async {
            let root = std::env::temp_dir().join(format!(
                "profile-photo-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            assert!(load_photo(&root, &pool).await.unwrap().is_none());
            let home = std::env::temp_dir();
            let profile = complete_profile();
            save(&pool, &home, &profile).await.unwrap();
            let first = encoded_image(image::ImageFormat::Png);
            save_photo(&root, &pool, &first).await.unwrap();
            let photo = load_photo(&root, &pool).await.unwrap().unwrap();
            assert_eq!(photo.data, first);
            assert_eq!(photo.mime_type, "image/png");
            let first_path: String =
                sqlx::query_scalar("SELECT photo_path FROM profile WHERE id = 1")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            let second = encoded_image(image::ImageFormat::Jpeg);
            save_photo(&root, &pool, &second).await.unwrap();
            let photo = load_photo(&root, &pool).await.unwrap().unwrap();
            assert_eq!(photo.data, second);
            assert_eq!(photo.mime_type, "image/jpeg");
            assert!(!Path::new(&first_path).exists());
            assert_eq!(fs::read_dir(root.join("profile")).unwrap().count(), 1);
            assert_eq!(load(&pool).await.unwrap(), profile);
            save(&pool, &home, &profile).await.unwrap();
            assert_eq!(
                load_photo(&root, &pool).await.unwrap().unwrap().data,
                second
            );
            assert!(save_photo(&root, &pool, b"not an image").await.is_err());
            assert_eq!(
                load_photo(&root, &pool).await.unwrap().unwrap().data,
                second
            );
            let photo_path: String =
                sqlx::query_scalar("SELECT photo_path FROM profile WHERE id = 1")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            sqlx::query("CREATE TRIGGER reject_photo BEFORE UPDATE OF photo_path ON profile BEGIN SELECT RAISE(ABORT, 'rejected'); END").execute(&pool).await.unwrap();
            assert!(save_photo(&root, &pool, &first).await.is_err());
            assert_eq!(
                load_photo(&root, &pool).await.unwrap().unwrap().data,
                second
            );
            assert_eq!(fs::read_dir(root.join("profile")).unwrap().count(), 1);
            sqlx::query("DROP TRIGGER reject_photo")
                .execute(&pool)
                .await
                .unwrap();
            fs::remove_file(photo_path).unwrap();
            save_photo(&root, &pool, &first).await.unwrap();
            assert_eq!(fs::read_dir(root.join("profile")).unwrap().count(), 1);
            fs::remove_dir_all(root).unwrap();
        });
    }

    #[test]
    fn resolves_default_tilde_and_absolute_documents_dir_and_rejects_relative() {
        tauri::async_runtime::block_on(async {
            let home = std::env::temp_dir();
            assert_eq!(resolve_documents_dir(&home, "").unwrap(), home.join("Certdex"));
            assert_eq!(resolve_documents_dir(&home, "  ").unwrap(), home.join("Certdex"));
            assert_eq!(resolve_documents_dir(&home, "~").unwrap(), home);
            assert_eq!(
                resolve_documents_dir(&home, "~/MyCertificates").unwrap(),
                home.join("MyCertificates")
            );
            let absolute = home.join("AbsoluteCerts");
            assert_eq!(
                resolve_documents_dir(&home, absolute.to_str().unwrap()).unwrap(),
                absolute
            );
            assert!(resolve_documents_dir(&home, "relative/folder").is_err());

            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            assert_eq!(load_documents_dir(&pool, &home).await.unwrap(), home.join("Certdex"));

            let mut profile = complete_profile();
            profile.documents_dir = "relative/folder".into();
            assert!(save(&pool, &home, &profile).await.is_err());
            profile.documents_dir = "~/CustomCerts".into();
            save(&pool, &home, &profile).await.unwrap();
            assert_eq!(
                load_documents_dir(&pool, &home).await.unwrap(),
                home.join("CustomCerts")
            );
        });
    }
}
