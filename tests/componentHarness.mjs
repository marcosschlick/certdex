import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

export function createComponent(name, invoke, runEffects = false, props = {}, options = {}) {
  const hooks = [];
  const cleanups = [];
  let index = 0;
  let effectsStarted = false;
  const storageMap = new Map(Object.entries(options.storage ?? {}));
  const localStorage = {
    getItem: (key) => storageMap.get(key) ?? null,
    setItem: (key, value) => { storageMap.set(key, String(value)); },
  };
  const navigator = options.navigator ?? { languages: ["en"], language: "en" };

  const source = (path) => ts.transpileModule(readFileSync(new URL(path, import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX },
  }).outputText;

  const enModule = { exports: {} };
  vm.runInNewContext(source("../src/i18n/en.ts"), { exports: enModule.exports });
  const ptBRModule = { exports: {} };
  vm.runInNewContext(source("../src/i18n/pt-BR.ts"), { exports: ptBRModule.exports });
  const esUYModule = { exports: {} };
  vm.runInNewContext(source("../src/i18n/es-UY.ts"), { exports: esUYModule.exports });

  const i18nModule = { exports: {} };
  vm.runInNewContext(source("../src/i18n.ts"), {
    exports: i18nModule.exports,
    localStorage,
    navigator,
    Intl,
    require(id) {
      if (id === "./i18n/en.ts" || id === "./i18n/en") return enModule.exports;
      if (id === "./i18n/pt-BR.ts" || id === "./i18n/pt-BR") return ptBRModule.exports;
      if (id === "./i18n/es-UY.ts" || id === "./i18n/es-UY") return esUYModule.exports;
      throw new Error(`Unexpected import: ${id}`);
    },
  });

  let activeLocale = i18nModule.exports.getInitialLocale(localStorage, navigator.languages);
  const i18nContextModule = {
    exports: {
      useI18n() {
        return {
          locale: activeLocale,
          setLocale(next) {
            i18nModule.exports.saveLocale(next, localStorage);
            activeLocale = next;
          },
          t: (key, params) => i18nModule.exports.translate(activeLocale, key, params),
          tDocType: (type) => i18nModule.exports.translateDocumentType(activeLocale, type),
          tError: (error, fallbackKey) => i18nModule.exports.translateError(activeLocale, error, fallbackKey),
          tWarning: (code) => i18nModule.exports.translateWarning(activeLocale, code),
          formatDate: (value, fallback) => i18nModule.exports.formatDate(activeLocale, value, fallback),
          formatNumber: (value) => i18nModule.exports.formatNumber(activeLocale, value),
        };
      },
    },
  };

  const react = {
    useState(initial) {
      const position = index++;
      if (!(position in hooks)) {
        hooks[position] = typeof initial === "function" ? initial() : initial;
      }
      return [hooks[position], (value) => {
        hooks[position] = typeof value === "function" ? value(hooks[position]) : value;
      }];
    },
    useRef(initial) {
      const position = index++;
      return hooks[position] ??= { current: initial };
    },
    useEffect(effect) {
      if (runEffects && !effectsStarted) {
        const cleanup = effect();
        if (typeof cleanup === "function") cleanups.push(cleanup);
      }
    },
  };
  const jsx = (type, props) => ({ type, props: props ?? {} });
  const formModule = { exports: {} };
  vm.runInNewContext(source("../src/documentForm.ts"), { exports: formModule.exports });
  const profileModule = { exports: {} };
  vm.runInNewContext(source("../src/profile.ts"), { exports: profileModule.exports });
  const reportModule = { exports: {} };
  const jsPdfStub = {
    jsPDF: class {
      internal = { pageSize: { getWidth: () => 210, getHeight: () => 297 } };
      setFont() {}
      setFontSize() {}
      splitTextToSize(text) { return [text]; }
      text() {}
      line() {}
      addPage() {}
      output() { return new Uint8Array([37, 80, 68, 70]).buffer; }
    },
  };
  vm.runInNewContext(source("../src/report.ts"), {
    exports: reportModule.exports,
    require(id) {
      if (id === "jspdf") return jsPdfStub;
      if (id === "./i18n.ts" || id === "./i18n") return i18nModule.exports;
      throw new Error(`Unexpected import: ${id}`);
    },
  });
  const componentModule = { exports: {} };
  const context = {
    exports: componentModule.exports,
    require(id) {
      if (id === "react") return react;
      if (id === "react/jsx-runtime") return { jsx, jsxs: jsx, Fragment: "fragment" };
      if (id === "../documentForm") return formModule.exports;
      if (id === "../profile") return profileModule.exports;
      if (id === "../report") return reportModule.exports;
      if (id === "../i18n.ts" || id === "../i18n") return i18nModule.exports;
      if (id === "../i18nContext") return i18nContextModule.exports;
      throw new Error(`Unexpected import: ${id}`);
    },
    window: { __TAURI__: { core: { invoke } } },
    Uint8Array,
  };
  vm.runInNewContext(source(`../src/pages/${name}.tsx`), context);
  function render() {
    index = 0;
    const result = componentModule.exports.default(props);
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
  return {
    find,
    storage: localStorage,
    unmount() { cleanups.forEach((cleanup) => cleanup()); },
  };
}
