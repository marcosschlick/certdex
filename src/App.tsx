import { useState } from "react";
import "./App.css";

function App() {
  const [page, setPage] = useState<"documents" | "profile">("documents");

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
        <h1>{page === "documents" ? "Documents" : "Profile"}</h1>
      </main>
    </div>
  );
}

export default App;
