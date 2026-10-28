use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::{fs, io::Write, path::Path};
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
}

async fn load(pool: &SqlitePool) -> Result<Profile, sqlx::Error> {
    Ok(sqlx::query_as::<_, Profile>(
        "SELECT COALESCE(name, '') AS name, COALESCE(email, '') AS email,
            COALESCE(phone, '') AS phone, COALESCE(city, '') AS city,
            COALESCE(state, '') AS state, COALESCE(country, '') AS country,
            COALESCE(github, '') AS github, COALESCE(linkedin, '') AS linkedin,
            COALESCE(orcid, '') AS orcid, COALESCE(website, '') AS website,
            COALESCE(professional_summary, '') AS professional_summary,
            COALESCE(academic_summary, '') AS academic_summary
        FROM profile WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or_default())
}

async fn save(pool: &SqlitePool, profile: &Profile) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO profile (id, name, email, phone, city, state, country, github, linkedin, orcid, website, professional_summary, academic_summary)
        VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET name = excluded.name, email = excluded.email,
            phone = excluded.phone, city = excluded.city, state = excluded.state,
            country = excluded.country, github = excluded.github, linkedin = excluded.linkedin,
            orcid = excluded.orcid, website = excluded.website,
            professional_summary = excluded.professional_summary, academic_summary = excluded.academic_summary",
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
    .execute(pool)
    .await?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn get_profile(pool: State<'_, SqlitePool>) -> Result<Profile, String> {
    load(&pool).await.map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn save_profile(
    pool: State<'_, SqlitePool>,
    profile: Profile,
) -> Result<(), String> {
    save(&pool, &profile)
        .await
        .map_err(|error| error.to_string())
}

fn image_type(data: &[u8]) -> Option<(&'static str, &'static str)> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("png", "image/png"))
    } else if data.starts_with(b"\xff\xd8\xff") {
        Some(("jpg", "image/jpeg"))
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some(("gif", "image/gif"))
    } else if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        Some(("webp", "image/webp"))
    } else {
        None
    }
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
        let _ = fs::remove_file(&path);
        return Err(format!("Could not save profile photo: {error}"));
    }
    drop(file);
    let result = sqlx::query("INSERT INTO profile (id, photo_path) VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET photo_path = excluded.photo_path")
        .bind(path.to_string_lossy().as_ref())
        .execute(pool).await;
    if let Err(error) = result {
        let _ = fs::remove_file(&path);
        return Err(format!("Could not save profile photo path: {error}"));
    }
    Ok(())
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

    #[test]
    fn profile_can_be_created_and_updated() {
        tauri::async_runtime::block_on(async {
            let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
            crate::database::create_tables(&pool).await.unwrap();
            assert_eq!(load(&pool).await.unwrap(), Profile::default());
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
            };
            save(&pool, &profile).await.unwrap();
            assert_eq!(load(&pool).await.unwrap(), profile);
            profile.name = "Updated".into();
            save(&pool, &profile).await.unwrap();
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
            let profile = Profile {
                name: "Existing".into(),
                ..Profile::default()
            };
            save(&pool, &profile).await.unwrap();
            let first = b"\x89PNG\r\n\x1a\nfirst";
            save_photo(&root, &pool, first).await.unwrap();
            let photo = load_photo(&root, &pool).await.unwrap().unwrap();
            assert_eq!(photo.data, first);
            assert_eq!(photo.mime_type, "image/png");
            let second = b"\xff\xd8\xffsecond";
            save_photo(&root, &pool, second).await.unwrap();
            let photo = load_photo(&root, &pool).await.unwrap().unwrap();
            assert_eq!(photo.data, second);
            assert_eq!(photo.mime_type, "image/jpeg");
            assert_eq!(load(&pool).await.unwrap(), profile);
            save(&pool, &profile).await.unwrap();
            assert_eq!(
                load_photo(&root, &pool).await.unwrap().unwrap().data,
                second
            );
            assert!(save_photo(&root, &pool, b"not an image").await.is_err());
            assert_eq!(
                load_photo(&root, &pool).await.unwrap().unwrap().data,
                second
            );
            fs::remove_dir_all(root).unwrap();
        });
    }
}
