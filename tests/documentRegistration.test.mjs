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
const typeSelect = (view) => view.find((node) => node.type === "select" && node.props.name === "type");
const titleInput = (view) => view.find((node) => node.type === "input" && node.props.name === "title");
const reading = { text: "Document text", source: "pdf_text", used_ocr: false, has_sufficient_text: true, warnings: [] };

test("registration waits for PDF reading and Ollama analysis and rejects failed reads", async () => {
  const read = deferred();
  const analysis = deferred();
  const view = createDocuments((command) => {
    if (command === "hash_document") return Promise.resolve("hash");
    if (command === "document_hash_exists") return Promise.resolve(false);
    if (command === "read_selected_document") return read.promise;
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
  const failedView = createDocuments((command) => command === "hash_document" ? Promise.resolve("hash") :
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
  const base = { type: "course", role: null, issue_date: "2025-01-01", start_date: null, end_date: null, city: null, country: null, organization: null, event: null, people: [], tags: [], workload_hours: 3 };
  const view = createDocuments((command) => {
    if (command === "list_documents") return Promise.resolve([{ ...base, id: 1, title: "Rust", organization: "University", event: "Congress", people: [{ name: "Ada", role: "Author" }], tags: ["Research"] }, { ...base, id: 2, title: "Math", workload_hours: 5 }]);
    if (command === "list_academic_options") return Promise.resolve({ organizations: [], events: [], people: [], tags: [] });
    throw new Error(command);
  }, true);
  view.find(() => false);
  await new Promise(setImmediate);
  const total = () => view.find((node) => node.type === "p" && typeof node.props.children === "string" && node.props.children.startsWith("Total workload:")).props.children;
  assert.equal(total(), "Total workload: 8 hours");
  for (const query of ["rust", "university", "congress", "ada", "author", "research"]) {
    view.find((node) => node.type === "input" && node.props.type === "search").props.onChange({ target: { value: query } });
    assert.equal(total(), "Total workload: 3 hours");
  }
});

test("changing PDFs and successful registration clear document fields", async () => {
  const view = createDocuments((command) => {
    if (command === "hash_document") return Promise.resolve("hash");
    if (command === "document_hash_exists") return Promise.resolve(false);
    if (command === "read_selected_document") return Promise.resolve(reading);
    if (command === "check_ollama") return Promise.resolve("unavailable");
    if (command === "store_document") return Promise.resolve("saved.pdf");
    throw new Error(command);
  });
  await fileInput(view).props.onChange({ target: { files: [pdf("first.pdf")] } });
  titleInput(view).props.onChange({ target: { name: "title", value: "First document" } });
  await fileInput(view).props.onChange({ target: { files: [pdf("second.pdf")] } });
  assert.equal(titleInput(view).props.value, "");
  typeSelect(view).props.onChange({ target: { name: "type", value: "course" } });
  titleInput(view).props.onChange({ target: { name: "title", value: "Second document" } });
  const input = { value: "second.pdf" };
  fileInput(view).props.ref.current = input;
  const form = view.find((node) => node.type === "form");
  await form.props.onSubmit({ preventDefault() {} });
  assert.equal(titleInput(view).props.value, "");
  assert.equal(input.value, "");
  assert.equal(registerButton(view).props.disabled, true);
});

test("stale processing and duplicate PDFs cannot enable registration", async () => {
  const firstRead = deferred();
  let count = 0;
  const view = createDocuments((command) => {
    if (command === "hash_document") return Promise.resolve("hash");
    if (command === "document_hash_exists") return Promise.resolve(++count === 2);
    if (command === "read_selected_document") return count === 1 ? firstRead.promise : Promise.resolve(reading);
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
      if (command === "hash_document") return Promise.resolve("hash");
      if (command === "document_hash_exists") return Promise.resolve(false);
      if (command === "read_selected_document") { reads += 1; return Promise.resolve(result); }
      if (command === "check_ollama") return Promise.resolve("unavailable");
      if (command === "store_document") {
        savedText = args.document.extracted_text;
        return Promise.resolve("saved.pdf");
      }
      throw new Error(command);
    });
    await fileInput(view).props.onChange({ target: { files: [pdf("document.pdf")] } });
    typeSelect(view).props.onChange({ target: { name: "type", value: "course" } });
    await view.find((node) => node.type === "form").props.onSubmit({ preventDefault() {} });
    assert.equal(savedText, result.text);
    assert.equal(reads, 1);
  }
});

test("allows manual registration and replacement without text or AI calls", async () => {
  for (const editing of [false, true]) {
    for (const text of ["", " \n\t"]) {
      const calls = [];
      let saved;
      const existing = { id: 5, type: "course", title: "Existing", people: [], tags: [] };
      const view = createDocuments((command, args) => {
        calls.push(command);
        if (command === "list_documents") return Promise.resolve([existing]);
        if (command === "list_academic_options") return Promise.resolve({ organizations: [], events: [], people: [], tags: [] });
        if (command === "hash_document") return Promise.resolve("hash");
        if (command === "document_hash_exists") return Promise.resolve(false);
        if (command === "read_selected_document") return Promise.resolve({ ...reading, text, has_sufficient_text: false, warnings: ["ocr_failed_no_pdf_text"] });
        if (command === "store_document" || command === "update_document") {
          saved = { command, args };
          return Promise.resolve("saved.pdf");
        }
        throw new Error(command);
      }, true);
      view.find(() => false);
      await new Promise(setImmediate);
      if (editing) view.find((node) => node.props["aria-label"] === "Edit Existing").props.onClick();
      await fileInput(view).props.onChange({ target: { files: [pdf("manual.pdf")] } });
      if (!editing) typeSelect(view).props.onChange({ target: { name: "type", value: "course" } });
      assert.equal(registerButton(view).props.disabled, false);
      assert.ok(view.find((node) => node.type === "p" && node.props.role === "status" && node.props.children === "Text could not be extracted. You can continue filling out the form manually."));
      assert.equal(calls.includes("check_ollama"), false);
      assert.equal(calls.includes("analyze_pdf_text"), false);
      await view.find((node) => node.type === "form").props.onSubmit({ preventDefault() {} });
      assert.equal(saved.command, editing ? "update_document" : "store_document");
      assert.equal(saved.args.document.extracted_text, "");
      assert.equal(saved.args.document.original_filename, "manual.pdf");
      if (editing) {
        assert.equal(saved.args.id, existing.id);
        assert.ok(saved.args.data);
      }
    }
  }
});


test("blocks registration while checking or analyzing and enables it after a result", async () => {
  const check = deferred();
  const analysis = deferred();
  let saves = 0;
  const view = createDocuments((command) => {
    if (command === "hash_document") return Promise.resolve("hash");
    if (command === "document_hash_exists") return Promise.resolve(false);
    if (command === "read_selected_document") return Promise.resolve(reading);
    if (command === "check_ollama") return check.promise;
    if (command === "analyze_pdf_text") return analysis.promise;
    if (command === "store_document") { saves += 1; return Promise.resolve("saved.pdf"); }
    throw new Error(command);
  });
  const processing = fileInput(view).props.onChange({ target: { files: [pdf("manual.pdf")] } });
  await new Promise(setImmediate);
  assert.equal(registerButton(view).props.disabled, true);
  await view.find((node) => node.type === "form").props.onSubmit({ preventDefault() {}, currentTarget: {} });
  assert.equal(saves, 0);
  check.resolve("model_available");
  await new Promise(setImmediate);
  assert.equal(registerButton(view).props.disabled, true);
  analysis.resolve({ title: "Suggested title" });
  await processing;
  assert.equal(titleInput(view).props.value, "Suggested title");
  assert.equal(registerButton(view).props.disabled, false);
});

test("enables manual registration when Ollama is unavailable", async () => {
  const view = createDocuments((command) => {
    if (command === "hash_document") return Promise.resolve("hash");
    if (command === "document_hash_exists") return Promise.resolve(false);
    if (command === "read_selected_document") return Promise.resolve(reading);
    if (command === "check_ollama") return Promise.resolve("unavailable");
    if (command === "store_document") return Promise.resolve("saved.pdf");
    throw new Error(command);
  });
  await fileInput(view).props.onChange({ target: { files: [pdf("manual.pdf")] } });
  assert.equal(registerButton(view).props.disabled, false);
});

test("late analysis preserves manually edited fields", async () => {
  const pending = deferred();
  const view = createDocuments((command) => {
    if (command === "hash_document") return Promise.resolve("hash");
    if (command === "document_hash_exists") return Promise.resolve(false);
    if (command === "read_selected_document") return Promise.resolve(reading);
    if (command === "check_ollama") return Promise.resolve("model_available");
    if (command === "analyze_pdf_text") return pending.promise;
    throw new Error(command);
  });
  const processing = fileInput(view).props.onChange({ target: { files: [pdf("manual.pdf")] } });
  await new Promise(setImmediate);
  titleInput(view).props.onChange({ target: { name: "title", value: "Manual title" } });
  const orgInput = view.find((node) => node.type === "input" && node.props.list === "organization-options");
  const eventInput = view.find((node) => node.type === "input" && node.props.list === "event-options");
  orgInput.props.onChange({ target: { value: "Manual Org" } });
  pending.resolve({ title: "Suggested title", country: "Brazil", organization: "IFSul", event: "Tech Week" });
  await processing;
  assert.equal(titleInput(view).props.value, "Manual title");
  assert.equal(view.find((node) => node.type === "input" && node.props.name === "country").props.value, "Brazil");
  assert.equal(view.find((node) => node.type === "input" && node.props.list === "organization-options").props.value, "Manual Org");
  assert.equal(view.find((node) => node.type === "input" && node.props.list === "event-options").props.value, "Tech Week");
});

test("changing PDF, editing, cancelling and unmounting ignore previous analysis", async () => {
  for (const transition of ["selection", "edit", "cancel", "unmount"]) {
    const pending = deferred();
    let analyses = 0;
    const existing = { id: 5, type: "course", title: "Existing", people: [], tags: [] };
    const view = createDocuments((command) => {
      if (command === "list_documents") return Promise.resolve([existing]);
      if (command === "list_academic_options") return Promise.resolve({ organizations: [], events: [], people: [], tags: [] });
      if (command === "hash_document") return Promise.resolve("hash");
      if (command === "document_hash_exists") return Promise.resolve(false);
      if (command === "read_selected_document") return Promise.resolve(reading);
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

test("accepts PNG and JPG certificates and rejects unsupported extensions", async () => {
  for (const name of ["certificate.png", "certificate.jpg", "CERTIFICATE.JPEG"]) {
    let stored;
    const view = createDocuments((command, args) => {
      if (command === "hash_document") return Promise.resolve("hash");
      if (command === "document_hash_exists") return Promise.resolve(false);
      if (command === "read_selected_document") return Promise.resolve({ ...reading, source: "ocr", used_ocr: true });
      if (command === "check_ollama") return Promise.resolve("unavailable");
      if (command === "store_document") {
        stored = args;
        return Promise.resolve(`documents/2025/certificate/${name}`);
      }
      throw new Error(command);
    });
    await fileInput(view).props.onChange({ target: { files: [pdf(name)] } });
    assert.equal(registerButton(view).props.disabled, false);
    typeSelect(view).props.onChange({ target: { name: "type", value: "certificate" } });
    titleInput(view).props.onChange({ target: { name: "title", value: "Image certificate" } });
    await view.find((node) => node.type === "form").props.onSubmit({ preventDefault() {} });
    assert.equal(stored.document.document_type, "certificate");
    assert.equal(stored.document.title, "Image certificate");
    assert.equal(stored.document.original_filename, name);
  }

  let invoked = false;
  const rejectedView = createDocuments(() => {
    invoked = true;
    throw new Error("should not invoke");
  });
  await fileInput(rejectedView).props.onChange({ target: { files: [pdf("notes.txt")], value: "notes.txt" } });
  assert.equal(invoked, false);
  assert.equal(registerButton(rejectedView).props.disabled, true);
});

test("deletes a document after confirmation and cancels active editing", async () => {
  const existing = {
    id: 7, type: "course", title: "To Delete", role: null, issue_date: "2025-01-01",
    start_date: null, end_date: null, workload_hours: 10, city: null, country: null,
    validation_code: null, validation_url: null, organization: "IFSul", event: null,
    people: [], tags: [],
  };
  let deletedId = null;
  const view = createDocuments((command, args) => {
    if (command === "list_documents") return Promise.resolve(deletedId ? [] : [existing]);
    if (command === "list_academic_options") return Promise.resolve({ organizations: [], events: [], people: [], tags: [] });
    if (command === "delete_document") {
      deletedId = args.id;
      return Promise.resolve();
    }
    throw new Error(command);
  }, true);
  view.find(() => false);
  await new Promise(setImmediate);

  view.find((node) => node.type === "button" && node.props["aria-label"] === "Edit To Delete").props.onClick();
  assert.equal(titleInput(view).props.value, "To Delete");

  view.find((node) => node.type === "button" && node.props["aria-label"] === "Delete To Delete").props.onClick();
  assert.equal(deletedId, null);

  await view.find((node) => node.type === "button" && node.props["aria-label"] === "Confirm delete To Delete").props.onClick();
  assert.equal(deletedId, 7);
  assert.equal(titleInput(view).props.value, "");
});

test("generates PDF report for filtered documents and disables when none match", async () => {
  const base = { type: "course", role: null, issue_date: "2025-01-01", start_date: null, end_date: null, city: null, country: null, organization: "IFSul", event: null, people: [], tags: [], workload_hours: 3 };
  let savedPayload = null;
  const view = createDocuments((command, args) => {
    if (command === "list_documents") return Promise.resolve([{ ...base, id: 1, title: "Rust" }, { ...base, id: 2, title: "Math", workload_hours: 5 }]);
    if (command === "list_academic_options") return Promise.resolve({ organizations: [], events: [], people: [], tags: [] });
    if (command === "get_profile") return Promise.resolve({ name: "Ada", email: "ada@example.com", phone: "123", city: "Pelotas", state: "RS", country: "Brazil" });
    if (command === "save_report_pdf") {
      savedPayload = args;
      return Promise.resolve("/home/user/Certdex/reports/certdex-report.pdf");
    }
    throw new Error(command);
  }, true);
  view.find(() => false);
  await new Promise(setImmediate);

  const reportButton = () => view.find((node) => node.type === "button" && node.props.children === "Generate PDF report");
  assert.equal(reportButton().props.disabled, false);

  view.find((node) => node.type === "input" && node.props.type === "search").props.onChange({ target: { value: "nonexistent" } });
  assert.equal(reportButton().props.disabled, true);

  view.find((node) => node.type === "input" && node.props.type === "search").props.onChange({ target: { value: "rust" } });
  assert.equal(reportButton().props.disabled, false);

  await reportButton().props.onClick();
  assert.ok(Array.isArray(savedPayload.data));
  assert.ok(savedPayload.data.length > 0);
  assert.match(savedPayload.filename, /^certdex-report-\d{4}-\d{2}-\d{2}\.pdf$/);
  assert.equal(
    view.find((node) => node.type === "p" && node.props.role === "status" && String(node.props.children).startsWith("Report saved:")).props.children,
    "Report saved: /home/user/Certdex/reports/certdex-report.pdf",
  );
});

test("applies regex analysis when Ollama is unavailable without overwriting manual edits", async () => {
  const view = createDocuments((command) => {
    if (command === "hash_document") return Promise.resolve("hash");
    if (command === "document_hash_exists") return Promise.resolve(false);
    if (command === "read_selected_document") {
      return Promise.resolve({
        ...reading,
        regex_analysis: {
          type: null,
          title: null,
          role: null,
          organization: null,
          event: null,
          issue_date: "2025-11-15",
          start_date: null,
          end_date: null,
          workload_hours: 20.5,
          city: null,
          country: null,
          validation_code: "CERT-123",
          validation_url: "https://valida.example.org/CERT-123",
        },
      });
    }
    if (command === "check_ollama") return Promise.resolve("unavailable");
    throw new Error(command);
  });

  await fileInput(view).props.onChange({ target: { files: [pdf("cert.pdf")] } });
  assert.equal(view.find((node) => node.type === "input" && node.props.name === "issue_date").props.value, "2025-11-15");
  assert.equal(view.find((node) => node.type === "input" && node.props.name === "workload_hours").props.value, "20.5");
  assert.equal(view.find((node) => node.type === "input" && node.props.name === "validation_code").props.value, "CERT-123");
  assert.equal(view.find((node) => node.type === "input" && node.props.name === "validation_url").props.value, "https://valida.example.org/CERT-123");
  assert.equal(registerButton(view).props.disabled, false);
});

test("cancels editing and supports adding and removing people", async () => {
  const existing = {
    id: 9, type: "course", title: "Original", role: null, issue_date: "2025-01-01",
    start_date: null, end_date: null, workload_hours: 8, city: null, country: null,
    validation_code: null, validation_url: null, organization: null, event: null,
    people: [{ name: "Ada", role: "Speaker" }], tags: [],
  };
  let updated;
  const view = createDocuments((command, args) => {
    if (command === "list_documents") return Promise.resolve([existing]);
    if (command === "list_academic_options") return Promise.resolve({ organizations: [], events: [], people: [], tags: [] });
    if (command === "update_document") { updated = args; return Promise.resolve(); }
    throw new Error(command);
  }, true);
  view.find(() => false);
  await new Promise(setImmediate);

  view.find((node) => node.type === "button" && node.props["aria-label"] === "Edit Original").props.onClick();
  assert.equal(titleInput(view).props.value, "Original");
  view.find((node) => node.type === "button" && node.props.children === "Cancel").props.onClick();
  assert.equal(titleInput(view).props.value, "");

  view.find((node) => node.type === "button" && node.props["aria-label"] === "Edit Original").props.onClick();
  view.find((node) => node.type === "button" && node.props.children === "Add person").props.onClick();
  view.find((node) => node.type === "button" && node.props.children === "Remove").props.onClick();
  const nameInput = view.find((node) => node.type === "input" && node.props.list === "people-options");
  nameInput.props.onChange({ target: { value: "Grace" } });
  const roleInput = view.find((node) => node.type === "div" && node.props.className === "person-row").props.children[1].props.children[1];
  roleInput.props.onChange({ target: { value: "Organizer" } });
  await view.find((node) => node.type === "form").props.onSubmit({ preventDefault() {} });
  assert.deepEqual(JSON.parse(JSON.stringify(updated.document.people)), [{ name: "Grace", role: "Organizer" }]);
});
