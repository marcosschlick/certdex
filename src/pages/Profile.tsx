import { useEffect, useState, type ChangeEvent, type FormEvent } from "react";
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
};

declare global {
  interface Window {
    __TAURI__: {
      core: {
        invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
      };
    };
  }
}

const fields: { name: keyof ProfileData; label: string; multiline?: boolean }[] = [
  { name: "name", label: "Name" },
  { name: "email", label: "Email" },
  { name: "phone", label: "Phone" },
  { name: "city", label: "City" },
  { name: "state", label: "State" },
  { name: "country", label: "Country" },
  { name: "github", label: "GitHub" },
  { name: "linkedin", label: "LinkedIn" },
  { name: "orcid", label: "ORCID" },
  { name: "website", label: "Website" },
  { name: "professional_summary", label: "Professional summary", multiline: true },
  { name: "academic_summary", label: "Academic summary", multiline: true },
];

function Profile({ setup = false, onComplete, onPhotoChange }: { setup?: boolean; onComplete?: () => void; onPhotoChange?: () => void }) {
  const [profile, setProfile] = useState<ProfileData | null>(null);
  const [message, setMessage] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let active = true;
    window.__TAURI__.core.invoke<ProfileData>("get_profile")
      .then((data) => { if (active) setProfile(data); })
      .catch(() => { if (active) setMessage("Could not load the profile. Please reopen this page to try again."); });
    return () => { active = false; };
  }, []);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!profile || saving) return;
    if (!isProfileComplete(profile)) {
      setMessage("Please complete all required fields.");
      return;
    }
    setSaving(true);
    setMessage("");
    try {
      await window.__TAURI__.core.invoke("save_profile", { profile });
      if (setup) onComplete?.();
      else setMessage("Profile saved.");
    } catch (error) {
      setMessage(typeof error === "string" ? error : "Could not save the profile. Please try again.");
    } finally {
      setSaving(false);
    }
  }

  async function handlePhotoChange(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    setSaving(true);
    setMessage("");
    try {
      const data = Array.from(new Uint8Array(await file.arrayBuffer()));
      await window.__TAURI__.core.invoke("save_profile_photo", { data });
      onPhotoChange?.();
      setMessage("Profile photo saved.");
    } catch (error) {
      setMessage(typeof error === "string" ? error : "Could not save the profile photo.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="profile">
      <h1>{setup ? "Set up your profile" : "Profile"}</h1>
      {setup && <p>Complete these fields to continue.</p>}
      {profile ? (
        <form onSubmit={handleSubmit}>
          {!setup && <label className="photo-field">
            <span>Profile photo</span>
            <input type="file" accept="image/png,image/jpeg,image/gif,image/webp" disabled={saving} onChange={handlePhotoChange} />
            <span className="photo-hint">The photo is saved when selected.</span>
          </label>}
          <fieldset disabled={saving} className="profile-fields">
            {fields.filter((field) => !setup || requiredProfileFields.some((name) => name === field.name)).map((field) => (
              <label key={field.name} className={field.multiline ? "profile-field-wide" : undefined}>
                <span>{field.label}</span>
                {field.multiline ? (
                  <textarea value={profile[field.name]} rows={4} onChange={(event) => {
                    setProfile({ ...profile, [field.name]: event.target.value });
                    setMessage("");
                  }} />
                ) : (
                  <input required={requiredProfileFields.some((name) => name === field.name)} type={field.name === "email" ? "email" : "text"} value={profile[field.name]} onChange={(event) => {
                    setProfile({ ...profile, [field.name]: event.target.value });
                    setMessage("");
                  }} />
                )}
              </label>
            ))}
          </fieldset>
          <div className="profile-actions">
            <button type="submit" disabled={saving}>{saving ? "Saving…" : setup ? "Save and continue" : "Save changes"}</button>
            <p role="status">{message}</p>
          </div>
        </form>
      ) : <p role="status">{message || "Loading profile…"}</p>}
    </section>
  );
}

export default Profile;
