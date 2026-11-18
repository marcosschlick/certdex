import { useRef, useState, type FormEvent } from "react";
import type { TranslationKey } from "../i18n.ts";
import { useI18n } from "../i18nContext";

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
  query: string;
  results: ChatDocumentResult[];
  used_embeddings: boolean;
};

type ChatMessage =
  | { id: number; role: "user"; text: string }
  | {
      id: number;
      role: "assistant";
      query: string;
      results: ChatDocumentResult[];
      usedEmbeddings: boolean;
    };

type ErrorState = { error: unknown; fallback: TranslationKey } | null;

function Chat() {
  const { t, tDocType, tError, formatDate, formatNumber } = useI18n();
  const [query, setQuery] = useState("");
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<ErrorState>(null);
  const [actionError, setActionError] = useState<ErrorState>(null);
  const nextMessageId = useRef(1);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const trimmed = query.trim();
    if (!trimmed || searching) return;
    setError(null);
    setActionError(null);
    const userMessageId = nextMessageId.current++;
    setMessages((previous) => [...previous, { id: userMessageId, role: "user", text: trimmed }]);
    setQuery("");
    setSearching(true);
    try {
      const response = await window.__TAURI__.core.invoke<ChatSearchResponse>(
        "search_documents_chat",
        { query: trimmed },
      );
      const assistantMessageId = nextMessageId.current++;
      setMessages((previous) => [
        ...previous,
        {
          id: assistantMessageId,
          role: "assistant",
          query: response.query,
          results: response.results,
          usedEmbeddings: response.used_embeddings,
        },
      ]);
    } catch (reason) {
      setError({ error: reason, fallback: "chat.searchError" });
    } finally {
      setSearching(false);
    }
  }

  async function openDocument(id: number) {
    setActionError(null);
    try {
      await window.__TAURI__.core.invoke("open_document", { id });
    } catch (reason) {
      setActionError({ error: reason, fallback: "chat.openError" });
    }
  }

  async function revealDocument(id: number) {
    setActionError(null);
    try {
      await window.__TAURI__.core.invoke("reveal_document", { id });
    } catch (reason) {
      setActionError({ error: reason, fallback: "chat.revealError" });
    }
  }

  function formatReply(searchQuery: string, count: number): string {
    if (count === 0) return t("chat.replyNone", { query: searchQuery });
    const key = count === 1 ? "chat.replyOne" : "chat.replyMany";
    return t(key, { count: formatNumber(count), query: searchQuery });
  }

  return (
    <section className="chat">
      <h1>{t("chat.title")}</h1>
      <div className="chat-messages" role="log" aria-live="polite">
        {messages.length === 0 && (
          <p className="chat-empty">
            {t("chat.empty")}
          </p>
        )}
        {messages.map((message) =>
          message.role === "user" ? (
            <article key={message.id} className="chat-message chat-message-user">
              <p>{message.text}</p>
            </article>
          ) : (
            <article key={message.id} className="chat-message chat-message-assistant">
              <p className="chat-reply">{formatReply(message.query, message.results.length)}</p>
              {!message.usedEmbeddings && message.results.length > 0 && (
                <p className="chat-mode">{t("chat.keywordMode")}</p>
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
                        <strong>{result.title || t("chat.untitled")}</strong>
                        <span className="chat-result-type">{tDocType(result.document_type)}</span>
                      </div>
                      {(result.organization || result.issue_date) && (
                        <p className="chat-result-meta">
                          {[result.organization, result.issue_date ? formatDate(result.issue_date, "") : ""].filter(Boolean).join(" · ")}
                        </p>
                      )}
                      <p className="chat-result-path">{result.file_path}</p>
                      <div className="chat-result-actions">
                        <button
                          type="button"
                          className="chat-action-open"
                          onClick={() => openDocument(result.id)}
                        >
                          {t("chat.open")}
                        </button>
                        <button
                          type="button"
                          className="chat-action-reveal"
                          onClick={() => revealDocument(result.id)}
                        >
                          {t("chat.showInFolder")}
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
          {tError(error.error, error.fallback)}
        </p>
      )}
      {actionError && (
        <p className="chat-error" role="alert">
          {tError(actionError.error, actionError.fallback)}
        </p>
      )}
      <form className="chat-form" onSubmit={handleSubmit}>
        <label className="chat-input-label">
          <span>{t("chat.searchLabel")}</span>
          <input
            type="text"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("chat.searchPlaceholder")}
          />
        </label>
        <button type="submit" disabled={searching || !query.trim()}>
          {searching ? t("chat.searching") : t("chat.send")}
        </button>
      </form>
    </section>
  );
}

export default Chat;
