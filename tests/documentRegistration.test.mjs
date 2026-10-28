import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

function createDocuments(invoke, runEffects = false) {
  const hooks = [];
  let index = 0;
  let effectsStarted = false;
  const react = {
    useState(initial) {
      const position = index++;
      if (!(position in hooks)) hooks[position] = initial;
      return [hooks[position], (value) => {
        hooks[position] = typeof value === "function" ? value(hooks[position]) : value;
      }];
    },
    useRef(initial) {
      const position = index++;
      return hooks[position] ??= { current: initial };
    },
    useEffect(effect) { if (runEffects && !effectsStarted) effect(); },
  };
  const jsx = (type, props) => ({ type, props: props ?? {} });
  const formModule = { exports: {} };
  const source = (path) => ts.transpileModule(readFileSync(new URL(path, import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX },
  }).outputText;
  vm.runInNewContext(source("../src/documentForm.ts"), { exports: formModule.exports });
  const componentModule = { exports: {} };
  const context = {
    exports: componentModule.exports,
    require(name) {
      if (name === "react") return react;
      if (name === "react/jsx-runtime") return { jsx, jsxs: jsx, Fragment: "fragment" };
      if (name === "../documentForm") return formModule.exports;
      throw new Error(`Unexpected import: ${name}`);
    },
    window: { __TAURI__: { core: { invoke } } },
    FormData: class { get(name) { return name === "type" ? "course" : null; } },
    Uint8Array,
  };
  vm.runInNewContext(source("../src/pages/Documents.tsx"), context);
  function render() {
    index = 0;
    const result = componentModule.exports.default();
    effectsStarted = true;
    return result;
  }
  function find(predicate) {
    const visit = (node) => {
      if (Array.isArray(node)) return node.map(visit).find(Boolean);
      if (!node || typeof node !== "object") return undefined;
      return predicate(node) ? node : visit(node.props?.children);
    };
    return visit(render());
  }
  return { find };
}

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((success, failure) => { resolve = success; reject = failure; });
  return { promise, resolve, reject };
}

const pdf = (name) => ({ name, arrayBuffer: async () => new Uint8Array([1]).buffer });
const fileInput = (view) => view.find((node) => node.type === "input" && node.props.type === "file");
const registerButton = (view) => view.find((node) => node.type === "button" && node.props.type === "submit");
const titleInput = (view) => view.find((node) => node.type === "input" && node.props.name === "title");
const reading = { text: "Document text", source: "pdf_text", used_ocr: false, has_sufficient_text: true };

test("registration waits for processing and rejects failed PDF reads", async () => {
  const read = deferred();
  const analysis = deferred();
  const view = createDocuments((command) => {
    if (command === "hash_pdf") return Promise.resolve("hash");
    if (command === "document_hash_exists") return Promise.resolve(false);
    if (command === "read_selected_pdf") return read.promise;
    if (command === "check_ollama") return Promise.resolve("model_available");
    if (command === "analyze_pdf_text") return analysis.promise;
    throw new Error(command);
  });
  const processing = fileInput(view).props.onChange({ target: { files: [pdf("first.pdf")] } });
  await new Promise(setImmediate);
  assert.equal(registerButton(view).props.disabled, true);
  read.resolve(reading);
  await new Promise(setImmediate);
  assert.equal(registerButton(view).props.disabled, true);
  analysis.reject("Analysis failed");
  await processing;
  assert.equal(registerButton(view).props.disabled, false);

  const failure = deferred();
  const failedView = createDocuments((command) => command === "hash_pdf" ? Promise.resolve("hash") :
    command === "document_hash_exists" ? Promise.resolve(false) : failure.promise);
  const failedProcessing = fileInput(failedView).props.onChange({ target: { files: [pdf("bad.pdf")] } });
  await new Promise(setImmediate);
  failure.reject("Invalid PDF");
  await failedProcessing;
  assert.equal(registerButton(failedView).props.disabled, true);
});

test("edits an existing document with its associations", async () => {
  const existing = { id: 5, type: "course", title: "Existing", role: "Speaker", issue_date: "2025-01-01", start_date: null, end_date: null, workload_hours: 12, city: "Recife", country: "Brazil", validation_code: null, validation_url: null, organization: "University", event: "Congress", people: [{ name: "Ada", role: "Author" }], tags: ["Research"] };
  let updated;
  const view = createDocuments((command, args) => {
    if (command === "list_documents") return Promise.resolve([existing]);
    if (command === "list_academic_options") return Promise.resolve({ organizations: [], events: [], people: [], tags: ["Research"] });
    if (command === "update_document") { updated = args; return Promise.resolve(); }
    throw new Error(command);
  }, true);
  view.find(() => false);
  await new Promise(setImmediate);
  view.find((node) => node.type === "button" && node.props["aria-label"] === "Edit Existing").props.onClick();
  assert.equal(titleInput(view).props.value, "Existing");
  await view.find((node) => node.type === "form").props.onSubmit({ preventDefault() {}, currentTarget: {} });
  assert.equal(updated.id, 5);
  assert.equal(updated.data, null);
  assert.equal(updated.document.organization, "University");
  assert.equal(updated.document.event, "Congress");
  assert.equal(updated.document.people[0].role, "Author");
  assert.equal(updated.document.tags[0], "Research");
  assert.equal(titleInput(view).props.value, "");
});

test("filtered documents determine the displayed workload", async () => {
  const base = { type: "course", role: null, issue_date: "2025-01-01", start_date: null, end_date: null, city: null, country: null, tags: [], workload_hours: 3 };
  const view = createDocuments((command) => {
    if (command === "list_documents") return Promise.resolve([{ ...base, id: 1, title: "Rust" }, { ...base, id: 2, title: "Math", workload_hours: 5 }]);
    if (command === "list_academic_options") return Promise.resolve({ organizations: [], events: [], people: [], tags: [] });
    throw new Error(command);
  }, true);
  view.find(() => false);
  await new Promise(setImmediate);
  const total = () => view.find((node) => node.type === "p" && node.props.children?.[0] === "Total workload: ").props.children[1];
  assert.equal(total(), 8);
  view.find((node) => node.type === "input" && node.props.type === "search").props.onChange({ target: { value: "rust" } });
  assert.equal(total(), 3);
});

test("changing PDFs and successful registration clear document fields", async () => {
  const view = createDocuments((command) => {
    if (command === "hash_pdf") return Promise.resolve("hash");
    if (command === "document_hash_exists") return Promise.resolve(false);
    if (command === "read_selected_pdf") return Promise.resolve(reading);
    if (command === "check_ollama") return Promise.resolve("unavailable");
    if (command === "store_document_pdf") return Promise.resolve("saved.pdf");
    throw new Error(command);
  });
  await fileInput(view).props.onChange({ target: { files: [pdf("first.pdf")] } });
  titleInput(view).props.onChange({ target: { name: "title", value: "First document" } });
  await fileInput(view).props.onChange({ target: { files: [pdf("second.pdf")] } });
  assert.equal(titleInput(view).props.value, "");
  titleInput(view).props.onChange({ target: { name: "title", value: "Second document" } });
  const input = { value: "second.pdf" };
  fileInput(view).props.ref.current = input;
  const form = view.find((node) => node.type === "form");
  await form.props.onSubmit({ preventDefault() {}, currentTarget: {} });
  assert.equal(titleInput(view).props.value, "");
  assert.equal(input.value, "");
  assert.equal(registerButton(view).props.disabled, true);
});

test("stale processing and duplicate PDFs cannot enable registration", async () => {
  const firstRead = deferred();
  let count = 0;
  const view = createDocuments((command) => {
    if (command === "hash_pdf") return Promise.resolve("hash");
    if (command === "document_hash_exists") return Promise.resolve(++count === 2);
    if (command === "read_selected_pdf") return count === 1 ? firstRead.promise : Promise.resolve(reading);
    if (command === "check_ollama") return Promise.resolve("unavailable");
    throw new Error(command);
  });
  const first = fileInput(view).props.onChange({ target: { files: [pdf("first.pdf")] } });
  await new Promise(setImmediate);
  await fileInput(view).props.onChange({ target: { files: [pdf("duplicate.pdf")] } });
  firstRead.resolve(reading);
  await first;
  assert.equal(registerButton(view).props.disabled, true);
});

test("registration saves the current PDF reading without extracting it again", async () => {
  for (const result of [
    { ...reading, text: "PDF text\nwith accents: ação" },
    { ...reading, text: "OCR result", source: "ocr", used_ocr: true },
    { ...reading, text: "", source: "ocr", used_ocr: true, has_sufficient_text: false },
  ]) {
    let reads = 0;
    let savedText;
    const view = createDocuments((command, args) => {
      if (command === "hash_pdf") return Promise.resolve("hash");
      if (command === "document_hash_exists") return Promise.resolve(false);
      if (command === "read_selected_pdf") { reads += 1; return Promise.resolve(result); }
      if (command === "check_ollama") return Promise.resolve("unavailable");
      if (command === "store_document_pdf") {
        savedText = args.document.extracted_text;
        return Promise.resolve("saved.pdf");
      }
      throw new Error(command);
    });
    await fileInput(view).props.onChange({ target: { files: [pdf("document.pdf")] } });
    await view.find((node) => node.type === "form").props.onSubmit({ preventDefault() {}, currentTarget: {} });
    assert.equal(savedText, result.text);
    assert.equal(reads, 1);
  }
});
