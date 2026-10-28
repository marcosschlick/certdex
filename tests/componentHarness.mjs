import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

export function createComponent(name, invoke, runEffects = false, props = {}) {
  const hooks = [];
  const cleanups = [];
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
    useEffect(effect) {
      if (runEffects && !effectsStarted) {
        const cleanup = effect();
        if (typeof cleanup === "function") cleanups.push(cleanup);
      }
    },
  };
  const jsx = (type, props) => ({ type, props: props ?? {} });
  const formModule = { exports: {} };
  const source = (path) => ts.transpileModule(readFileSync(new URL(path, import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX },
  }).outputText;
  vm.runInNewContext(source("../src/documentForm.ts"), { exports: formModule.exports });
  const profileModule = { exports: {} };
  vm.runInNewContext(source("../src/profile.ts"), { exports: profileModule.exports });
  const componentModule = { exports: {} };
  const context = {
    exports: componentModule.exports,
    require(name) {
      if (name === "react") return react;
      if (name === "react/jsx-runtime") return { jsx, jsxs: jsx, Fragment: "fragment" };
      if (name === "../documentForm") return formModule.exports;
      if (name === "../profile") return profileModule.exports;
      throw new Error(`Unexpected import: ${name}`);
    },
    window: { __TAURI__: { core: { invoke } } },
    FormData: class { get(name) { return name === "type" ? "course" : null; } },
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
  return { find, unmount() { cleanups.forEach((cleanup) => cleanup()); } };
}

