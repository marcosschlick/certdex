import { useEffect, useState } from "react";
import Chat from "./pages/Chat";
import Documents from "./pages/Documents";
import Profile from "./pages/Profile";
import { isProfileComplete } from "./profile";
import "./App.css";

function App() {
  const [setup, setSetup] = useState<"loading" | "required" | "complete" | "error">("loading");
  const [attempt, setAttempt] = useState(0);
  const [page, setPage] = useState<"documents" | "chat" | "profile">("documents");
  const [photoUrl, setPhotoUrl] = useState<string | null>(null);
  const [photoRevision, setPhotoRevision] = useState(0);

  useEffect(() => {
    let active = true;
    window.__TAURI__.core.invoke<Record<"name" | "email" | "phone" | "city" | "state" | "country", string>>("get_profile")
      .then((profile) => { if (active) setSetup(isProfileComplete(profile) ? "complete" : "required"); })
      .catch(() => { if (active) setSetup("error"); });
    return () => { active = false; };
  }, [attempt]);

  useEffect(() => {
    if (setup !== "complete") return;
    let active = true;
    let url: string | null = null;
    window.__TAURI__.core.invoke<{ data: number[]; mime_type: string } | null>("get_profile_photo")
      .then((photo) => {
        if (!active) return;
        if (photo) url = URL.createObjectURL(new Blob([new Uint8Array(photo.data)], { type: photo.mime_type }));
        setPhotoUrl(url);
      })
      .catch(() => { if (active) setPhotoUrl(null); });
    return () => { active = false; if (url) URL.revokeObjectURL(url); };
  }, [setup, photoRevision]);

  if (setup !== "complete") {
    return (
      <main className="content">
        {setup === "required" ? <Profile setup onComplete={() => setSetup("complete")} /> :
          setup === "error" ? <div className="profile-actions"><p role="alert">Could not load the profile.</p><button type="button" onClick={() => { setSetup("loading"); setAttempt((value) => value + 1); }}>Try again</button></div> :
          <p role="status">Loading profile…</p>}
      </main>
    );
  }

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="sidebar-photo">
          {photoUrl ? <img src={photoUrl} alt="Profile" /> : <span aria-label="No profile photo">●</span>}
        </div>
        <div className="brand">Certificates</div>
        <nav aria-label="Main navigation">
          <button
            type="button"
            aria-current={page === "documents" ? "page" : undefined}
            onClick={() => setPage("documents")}
          >
            Documents
          </button>
          <button
            type="button"
            aria-current={page === "chat" ? "page" : undefined}
            onClick={() => setPage("chat")}
          >
            Chat
          </button>
          <button
            type="button"
            aria-current={page === "profile" ? "page" : undefined}
            onClick={() => setPage("profile")}
          >
            Profile
          </button>
        </nav>
      </aside>
      <main className="content">
        {page === "documents" ? (
          <Documents />
        ) : page === "chat" ? (
          <Chat />
        ) : (
          <Profile onPhotoChange={() => setPhotoRevision((value) => value + 1)} />
        )}
      </main>
    </div>
  );
}

export default App;
