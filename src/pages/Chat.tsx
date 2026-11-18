import { useState, type FormEvent } from "react";

type ChatDocumentResult = {
  id: number;
  title: string | null;
  document_type: string;
  organization: string | null;
  issue_date: string | null;
  file_path: string;
  score: number;
  matched_by: string;
};

type ChatSearchResponse = {
  reply: string;
  results: ChatDocumentResult[];
  used_embeddings: boolean;
};

type ChatMessage =
  | { id: number; role: "user"; text: string }
  | {
      id: number;
      role: "assistant";
      text: string;
      results: ChatDocumentResult[];
      usedEmbeddings: boolean;
    };

function Chat() {
  const [query, setQuery] = useState("");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState("");
  const [actionError, setActionError] = useState("");

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const trimmed = query.trim();
    if (!trimmed || searching) return;
    setError("");
    setActionError("");
    const userId = Date.now();
    setMessages((previous) => [...previous, { id: userId, role: "user", text: trimmed }]);
    setQuery("");
    setSearching(true);
    try {
      const response = await window.__TAURI__.core.invoke<ChatSearchResponse>(
        "search_documents_chat",
        { query: trimmed },
      );
      setMessages((previous) => [
        ...previous,
        {
          id: userId + 1,
          role: "assistant",
          text: response.reply,
          results: response.results,
          usedEmbeddings: response.used_embeddings,
        },
      ]);
    } catch (reason) {
      setError(typeof reason === "string" ? reason : "Could not search documents.");
    } finally {
      setSearching(false);
    }
  }

  async function openDocument(id: number) {
    setActionError("");
    try {
      await window.__TAURI__.core.invoke("open_document", { id });
    } catch (reason) {
      setActionError(typeof reason === "string" ? reason : "Could not open document.");
    }
  }

  async function revealDocument(id: number) {
    setActionError("");
    try {
      await window.__TAURI__.core.invoke("reveal_document", { id });
    } catch (reason) {
      setActionError(typeof reason === "string" ? reason : "Could not show document in folder.");
    }
  }

  return (
    <section className="chat">
      <h1>Chat</h1>
      <div className="chat-messages" role="log" aria-live="polite">
        {messages.length === 0 && (
          <p className="chat-empty">
            Search your documents by title, topic, organization, or extracted content.
          </p>
        )}
        {messages.map((message) =>
          message.role === "user" ? (
            <article key={message.id} className="chat-message chat-message-user">
              <p>{message.text}</p>
            </article>
          ) : (
            <article key={message.id} className="chat-message chat-message-assistant">
              <p className="chat-reply">{message.text}</p>
              {!message.usedEmbeddings && message.results.length > 0 && (
                <p className="chat-mode">Keyword search (FTS5)</p>
              )}
              {message.results.length > 0 && (
                <ul className="chat-results">
                  {message.results.map((result, index) => (
                    <li
                      key={result.id}
                      className="chat-result-card"
                      style={{ animationDelay: `${Math.min(index, 4) * 35}ms` }}
                    >
                      <div className="chat-result-header">
                        <strong>{result.title || "Untitled document"}</strong>
                        <span className="chat-result-type">{result.document_type}</span>
                      </div>
                      {(result.organization || result.issue_date) && (
                        <p className="chat-result-meta">
                          {[result.organization, result.issue_date].filter(Boolean).join(" · ")}
                        </p>
                      )}
                      <p className="chat-result-path">{result.file_path}</p>
                      <div className="chat-result-actions">
                        <button
                          type="button"
                          className="chat-action-open"
                          onClick={() => openDocument(result.id)}
                        >
                          Open
                        </button>
                        <button
                          type="button"
                          className="chat-action-reveal"
                          onClick={() => revealDocument(result.id)}
                        >
                          Show in folder
                        </button>
                      </div>
                    </li>
                  ))}
                </ul>
              )}
            </article>
          ),
        )}
      </div>
      {error && (
        <p className="chat-error" role="alert">
          {error}
        </p>
      )}
      {actionError && (
        <p className="chat-error" role="alert">
          {actionError}
        </p>
      )}
      <form className="chat-form" onSubmit={handleSubmit}>
        <label className="chat-input-label">
          <span>Search documents</span>
          <input
            type="text"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="e.g. Rust certificate IFSul"
          />
        </label>
        <button type="submit" disabled={searching || !query.trim()}>
          {searching ? "Searching…" : "Send"}
        </button>
      </form>
    </section>
  );
}

export default Chat;
