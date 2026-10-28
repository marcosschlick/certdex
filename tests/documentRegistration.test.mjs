import test from "node:test";
import assert from "node:assert/strict";
import { createComponent } from "./componentHarness.mjs";

const createDocuments = (invoke, runEffects = false) => createComponent("Documents", invoke, runEffects);

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

test("registration waits for PDF reading but not optional analysis and rejects failed reads", async () => {
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
  assert.equal(registerButton(view).props.disabled, false);
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


test("saves manually while checking or analyzing Ollama and ignores late results", async () => {
  for (const pendingCommand of ["check_ollama", "analyze_pdf_text"]) {
    const pending = deferred();
    let saved = false;
    const view = createDocuments((command) => {
      if (command === "hash_pdf") return Promise.resolve("hash");
      if (command === "document_hash_exists") return Promise.resolve(false);
      if (command === "read_selected_pdf") return Promise.resolve(reading);
      if (command === pendingCommand) return pending.promise;
      if (command === "check_ollama") return Promise.resolve("model_available");
      if (command === "store_document_pdf") { saved = true; return Promise.resolve("saved.pdf"); }
      throw new Error(command);
    });
    const processing = fileInput(view).props.onChange({ target: { files: [pdf("manual.pdf")] } });
    await new Promise(setImmediate);
    assert.equal(registerButton(view).props.disabled, false);
    titleInput(view).props.onChange({ target: { name: "title", value: "Manual title" } });
    await view.find((node) => node.type === "form").props.onSubmit({ preventDefault() {}, currentTarget: {} });
    assert.equal(saved, true);
    pending.resolve(pendingCommand === "check_ollama" ? "model_available" : { title: "Late analysis" });
    await processing;
    assert.equal(titleInput(view).props.value, "");
    assert.equal(registerButton(view).props.disabled, true);
  }
});

test("late analysis preserves manually edited fields", async () => {
  const pending = deferred();
  const view = createDocuments((command) => {
    if (command === "hash_pdf") return Promise.resolve("hash");
    if (command === "document_hash_exists") return Promise.resolve(false);
    if (command === "read_selected_pdf") return Promise.resolve(reading);
    if (command === "check_ollama") return Promise.resolve("model_available");
    if (command === "analyze_pdf_text") return pending.promise;
    throw new Error(command);
  });
  const processing = fileInput(view).props.onChange({ target: { files: [pdf("manual.pdf")] } });
  await new Promise(setImmediate);
  titleInput(view).props.onChange({ target: { name: "title", value: "Manual title" } });
  pending.resolve({ title: "Suggested title", country: "Brazil" });
  await processing;
  assert.equal(titleInput(view).props.value, "Manual title");
  assert.equal(view.find((node) => node.type === "input" && node.props.name === "country").props.value, "Brazil");
});

test("changing PDF, editing, cancelling and unmounting ignore previous analysis", async () => {
  for (const transition of ["selection", "edit", "cancel", "unmount"]) {
    const pending = deferred();
    let analyses = 0;
    const existing = { id: 5, type: "course", title: "Existing", people: [], tags: [] };
    const view = createDocuments((command) => {
      if (command === "list_documents") return Promise.resolve([existing]);
      if (command === "list_academic_options") return Promise.resolve({ organizations: [], events: [], people: [], tags: [] });
      if (command === "hash_pdf") return Promise.resolve("hash");
      if (command === "document_hash_exists") return Promise.resolve(false);
      if (command === "read_selected_pdf") return Promise.resolve(reading);
      if (command === "check_ollama") return Promise.resolve("model_available");
      if (command === "analyze_pdf_text") return ++analyses === 1 ? pending.promise : Promise.resolve({ title: "Second PDF" });
      throw new Error(command);
    }, true);
    view.find(() => false);
    await new Promise(setImmediate);
    if (transition === "cancel") view.find((node) => node.props["aria-label"] === "Edit Existing").props.onClick();
    const processing = fileInput(view).props.onChange({ target: { files: [pdf("first.pdf")] } });
    await new Promise(setImmediate);
    if (transition === "selection") await fileInput(view).props.onChange({ target: { files: [pdf("second.pdf")] } });
    if (transition === "edit") view.find((node) => node.props["aria-label"] === "Edit Existing").props.onClick();
    if (transition === "cancel") view.find((node) => node.type === "button" && node.props.children === "Cancel").props.onClick();
    if (transition === "unmount") view.unmount();
    const before = titleInput(view).props.value;
    pending.resolve({ title: "Stale title" });
    await processing;
    assert.equal(titleInput(view).props.value, before);
  }
});
