import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

function createDocuments(invoke) {
  const hooks = [];
  let index = 0;
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
    useEffect() {},
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
    return componentModule.exports.default();
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
