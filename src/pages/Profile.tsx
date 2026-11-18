import { useEffect, useState, type ChangeEvent, type FormEvent } from "react";
import { isSupportedLocale, localeLabels, supportedLocales, type TranslationKey } from "../i18n.ts";
import { useI18n } from "../i18nContext";
import { isProfileComplete, requiredProfileFields } from "../profile";

type ProfileData = {
  name: string;
  email: string;
  phone: string;
  city: string;
  state: string;
  country: string;
  github: string;
  linkedin: string;
  orcid: string;
  website: string;
  professional_summary: string;
  academic_summary: string;
  documents_dir: string;
};

type ProfileMessage =
  | { key: TranslationKey }
  | { error: unknown; fallback: TranslationKey }
  | null;

declare global {
  interface Window {
    __TAURI__: {
      core: {
        invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
      };
    };
  }
}

const fields: { name: keyof ProfileData; labelKey: TranslationKey; multiline?: boolean; placeholder?: string }[] = [
  { name: "name", labelKey: "profile.name" },
  { name: "email", labelKey: "profile.email" },
  { name: "phone", labelKey: "profile.phone" },
  { name: "city", labelKey: "profile.city" },
  { name: "state", labelKey: "profile.state" },
  { name: "country", labelKey: "profile.country" },
  { name: "github", labelKey: "profile.github" },
  { name: "linkedin", labelKey: "profile.linkedin" },
  { name: "orcid", labelKey: "profile.orcid" },
  { name: "website", labelKey: "profile.website" },
  { name: "documents_dir", labelKey: "profile.documentsDir", placeholder: "~/Certdex" },
  { name: "professional_summary", labelKey: "profile.professionalSummary", multiline: true },
  { name: "academic_summary", labelKey: "profile.academicSummary", multiline: true },
];

function Profile({ setup = false, onComplete, onPhotoChange }: { setup?: boolean; onComplete?: () => void; onPhotoChange?: () => void }) {
  const { locale, setLocale, t, tError } = useI18n();
  const [profile, setProfile] = useState<ProfileData | null>(null);
  const [message, setMessage] = useState<ProfileMessage>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let active = true;
    window.__TAURI__.core.invoke<ProfileData>("get_profile")
      .then((data) => { if (active) setProfile(data); })
      .catch(() => { if (active) setMessage({ key: "profile.loadError" }); });
    return () => { active = false; };
  }, []);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!profile || saving) return;
    if (!isProfileComplete(profile)) {
      setMessage({ key: "error.profile.required_fields" });
      return;
    }
    setSaving(true);
    setMessage(null);
    try {
      await window.__TAURI__.core.invoke("save_profile", {
        profile: { ...profile, documents_dir: profile.documents_dir ?? "" },
      });
      if (setup) onComplete?.();
      else setMessage({ key: "profile.saved" });
    } catch (error) {
      setMessage({ error, fallback: "profile.saveError" });
    } finally {
      setSaving(false);
    }
  }

  async function handlePhotoChange(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    setSaving(true);
    setMessage(null);
    try {
      const data = Array.from(new Uint8Array(await file.arrayBuffer()));
      await window.__TAURI__.core.invoke("save_profile_photo", { data });
      onPhotoChange?.();
      setMessage({ key: "profile.photoSaved" });
    } catch (error) {
      setMessage({ error, fallback: "profile.photoSaveError" });
    } finally {
      setSaving(false);
    }
  }

  const renderedMessage = !message
    ? ""
    : "key" in message
      ? t(message.key)
      : tError(message.error, message.fallback);

  return (
    <section className="profile">
      <h1>{setup ? t("profile.setupTitle") : t("profile.title")}</h1>
      {setup && <p>{t("profile.setupPrompt")}</p>}
      {profile ? (
        <form onSubmit={handleSubmit}>
          {!setup && <label className="photo-field">
            <span>{t("profile.photoLabel")}</span>
            <input type="file" accept="image/png,image/jpeg,image/gif,image/webp" disabled={saving} onChange={handlePhotoChange} />
            <span className="photo-hint">{t("profile.photoHint")}</span>
          </label>}
          <fieldset disabled={saving} className="profile-fields">
            <label>
              <span>{t("profile.language")}</span>
              <select
                value={locale}
                onChange={(event) => {
                  const next = event.target.value;
                  if (isSupportedLocale(next)) setLocale(next);
                }}
              >
                {supportedLocales.map((code) => (
                  <option key={code} value={code}>
                    {localeLabels[code]}
                  </option>
                ))}
              </select>
            </label>
            {fields.filter((field) => !setup || requiredProfileFields.some((name) => name === field.name)).map((field) => (
              <label key={field.name} className={field.multiline ? "profile-field-wide" : undefined}>
                <span>{t(field.labelKey)}</span>
                {field.multiline ? (
                  <textarea value={profile[field.name] ?? ""} rows={4} onChange={(event) => {
                    setProfile({ ...profile, [field.name]: event.target.value });
                    setMessage(null);
                  }} />
                ) : (
                  <input required={requiredProfileFields.some((name) => name === field.name)} type={field.name === "email" ? "email" : "text"} placeholder={field.placeholder} value={profile[field.name] ?? ""} onChange={(event) => {
                    setProfile({ ...profile, [field.name]: event.target.value });
                    setMessage(null);
                  }} />
                )}
              </label>
            ))}
          </fieldset>
          <div className="profile-actions">
            <button type="submit" disabled={saving}>{saving ? t("profile.saving") : setup ? t("profile.saveAndContinue") : t("profile.saveChanges")}</button>
            <p role="status">{renderedMessage}</p>
          </div>
        </form>
      ) : <p role="status">{renderedMessage || t("profile.loading")}</p>}
    </section>
  );
}

export default Profile;
