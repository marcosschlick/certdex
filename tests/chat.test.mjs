import assert from "node:assert/strict";
import test from "node:test";
import { createComponent } from "./componentHarness.mjs";

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

function collectText(node) {
  if (node == null || typeof node === "boolean") return "";
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(collectText).join(" ");
  return collectText(node.props?.children);
}

test("chat searches documents, shows real file path, and triggers open and reveal", async () => {
  const calls = [];
  const invoke = async (command, args) => {
    calls.push([command, args]);
    if (command === "search_documents_chat") {
      return {
        query: "certificacao",
        used_embeddings: true,
        results: [
          {
            id: 42,
            title: "Certificação Rust",
            document_type: "certificate",
            organization: "IFSul",
            issue_date: "2025-05-10",
            file_path: "/home/user/Certificates/certificate/2025/ifsul/certificacao-rust.pdf",
            score: 0.92,
            matched_by: "hybrid",
          },
        ],
      };
    }
    return null;
  };

  const chat = createComponent("Chat", invoke);
  const input = chat.find((node) => node.type === "input");
  input.props.onChange({ target: { value: "certificacao" } });

  const form = chat.find((node) => node.type === "form");
  await form.props.onSubmit({ preventDefault() {} });
  await flush();

  const reply = chat.find((node) => node.props?.className === "chat-reply");
  assert.equal(collectText(reply), 'Found 1 document matching "certificacao":');

  const path = chat.find((node) => node.props?.className === "chat-result-path");
  assert.equal(
    collectText(path),
    "/home/user/Certificates/certificate/2025/ifsul/certificacao-rust.pdf",
  );

  const openButton = chat.find((node) => node.props?.className === "chat-action-open");
  await openButton.props.onClick();

  const revealButton = chat.find((node) => node.props?.className === "chat-action-reveal");
  await revealButton.props.onClick();

  assert.deepEqual(JSON.parse(JSON.stringify(calls)), [
    ["search_documents_chat", { query: "certificacao" }],
    ["open_document", { id: 42 }],
    ["reveal_document", { id: 42 }],
  ]);
});

test("chat renders localized empty reply and error feedback", async () => {
  let fail = false;
  const invoke = async (command) => {
    if (command === "search_documents_chat") {
      if (fail) throw "database.error";
      return {
        query: "missing",
        used_embeddings: false,
        results: [],
      };
    }
    return null;
  };

  const chat = createComponent("Chat", invoke, false, {}, { navigator: { languages: ["es-UY"] } });
  chat.find((node) => node.type === "input").props.onChange({ target: { value: "missing" } });
  await chat.find((node) => node.type === "form").props.onSubmit({ preventDefault() {} });
  await flush();

  const reply = chat.find((node) => node.props?.className === "chat-reply");
  assert.equal(collectText(reply), 'No se encontraron documentos para "missing".');
  assert.equal(chat.find((node) => node.props?.className === "chat-result-card"), undefined);

  fail = true;
  chat.find((node) => node.type === "input").props.onChange({ target: { value: "retry" } });
  await chat.find((node) => node.type === "form").props.onSubmit({ preventDefault() {} });
  await flush();

  const alert = chat.find((node) => node.props?.role === "alert");
  assert.equal(collectText(alert), "Ocurrió un error en la base de datos. Intentá de nuevo.");
});

test("chat shows FTS5 mode indicator, translated document type, and action errors", async () => {
  const invoke = async (command) => {
    if (command === "search_documents_chat") {
      return {
        query: "rust",
        used_embeddings: false,
        results: [
          {
            id: 1,
            title: "Rust Workshop",
            document_type: "course",
            organization: "IFSul",
            issue_date: "2025-05-10",
            file_path: "/tmp/rust.pdf",
            score: 1,
            matched_by: "keyword",
          },
        ],
      };
    }
    if (command === "open_document") throw "document.file_not_found";
    return null;
  };

  const chat = createComponent("Chat", invoke, false, {}, { navigator: { languages: ["pt-BR"] } });
  chat.find((node) => node.type === "input").props.onChange({ target: { value: "rust" } });
  await chat.find((node) => node.type === "form").props.onSubmit({ preventDefault() {} });
  await flush();

  const mode = chat.find((node) => node.props?.className === "chat-mode");
  assert.equal(collectText(mode), "Busca por palavras-chave (FTS5)");
  const docType = chat.find((node) => node.props?.className === "chat-result-type");
  assert.equal(collectText(docType), "Curso");

  await chat.find((node) => node.props?.className === "chat-action-open").props.onClick();
  const alert = chat.find((node) => node.props?.role === "alert");
  assert.equal(collectText(alert), "Arquivo do documento não encontrado.");
});
