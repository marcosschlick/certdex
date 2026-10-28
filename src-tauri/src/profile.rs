use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use tauri::State;

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
}
