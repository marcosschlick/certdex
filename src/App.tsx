import { useEffect, useState } from "react";
import Documents from "./pages/Documents";
import Profile from "./pages/Profile";
import { isProfileComplete } from "./profile";
import "./App.css";

function App() {
  const [setup, setSetup] = useState<"loading" | "required" | "complete" | "error">("loading");
  const [attempt, setAttempt] = useState(0);
  const [page, setPage] = useState<"documents" | "profile">("documents");

  useEffect(() => {
    let active = true;
    window.__TAURI__.core.invoke<Record<"name" | "email" | "phone" | "city" | "state" | "country", string>>("get_profile")
      .then((profile) => { if (active) setSetup(isProfileComplete(profile) ? "complete" : "required"); })
      .catch(() => { if (active) setSetup("error"); });
    return () => { active = false; };
  }, [attempt]);

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
            aria-current={page === "profile" ? "page" : undefined}
            onClick={() => setPage("profile")}
          >
            Profile
          </button>
        </nav>
      </aside>
      <main className="content">
        {page === "documents" ? <Documents /> : <Profile />}
      </main>
    </div>
  );
}

export default App;
