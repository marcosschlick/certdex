import { useEffect, useState, type FormEvent } from "react";

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

function Profile() {
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
    setSaving(true);
    setMessage("");
    try {
      await window.__TAURI__.core.invoke("save_profile", { profile });
      setMessage("Profile saved.");
    } catch {
      setMessage("Could not save the profile. Please try again.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="profile">
      <h1>Profile</h1>
      {profile ? (
        <form onSubmit={handleSubmit}>
          <fieldset disabled={saving} className="profile-fields">
            {fields.map((field) => (
              <label key={field.name} className={field.multiline ? "profile-field-wide" : undefined}>
                <span>{field.label}</span>
                {field.multiline ? (
                  <textarea value={profile[field.name]} rows={4} onChange={(event) => {
                    setProfile({ ...profile, [field.name]: event.target.value });
                    setMessage("");
                  }} />
                ) : (
                  <input value={profile[field.name]} onChange={(event) => {
                    setProfile({ ...profile, [field.name]: event.target.value });
                    setMessage("");
                  }} />
                )}
              </label>
            ))}
          </fieldset>
          <div className="profile-actions">
            <button type="submit" disabled={saving}>{saving ? "Saving…" : "Save changes"}</button>
            <p role="status">{message}</p>
          </div>
        </form>
      ) : <p role="status">{message || "Loading profile…"}</p>}
    </section>
  );
}

export default Profile;
