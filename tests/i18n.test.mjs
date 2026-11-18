import test from "node:test";
import assert from "node:assert/strict";
import {
  dictionaries,
  formatDate,
  formatNumber,
  getInitialLocale,
  LOCALE_STORAGE_KEY,
  resolveSystemLocale,
  saveLocale,
  supportedLocales,
  translate,
  translateDocumentType,
  translateError,
  translateWarning,
} from "../src/i18n.ts";

function placeholders(text) {
  return [...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort();
}

test("all translation dictionaries contain the exact same keys, non-empty values, and matching placeholders", () => {
  const baseKeys = Object.keys(dictionaries.en).sort();
  assert.ok(baseKeys.length > 0);

  for (const locale of supportedLocales) {
    const dictionary = dictionaries[locale];
    const keys = Object.keys(dictionary).sort();
    assert.deepEqual(keys, baseKeys, `Key mismatch for locale ${locale}`);

    for (const key of baseKeys) {
      const value = dictionary[key];
      assert.equal(typeof value, "string", `Expected string for ${locale}.${key}`);
      assert.ok(value.trim().length > 0, `Empty translation for ${locale}.${key}`);
      assert.deepEqual(
        placeholders(value),
        placeholders(dictionaries.en[key]),
        `Placeholder mismatch for ${locale}.${key}`,
      );
    }
  }
});

test("translates all document types and interpolates parameters", () => {
  const documentTypes = [
    "certificate", "course", "article", "proceedings", "presentation", "event",
    "award", "project", "internship", "monitoring", "other",
  ];
  for (const locale of supportedLocales) {
    for (const type of documentTypes) {
      const label = translateDocumentType(locale, type);
      assert.ok(label.length > 0);
      assert.notEqual(label, type);
    }
  }
  assert.equal(translateDocumentType("pt-BR", "internship"), "Estágio");
  assert.equal(translateDocumentType("es-UY", "internship"), "Pasantía");
  assert.equal(translate("es-UY", "chat.replyMany", { count: 2, query: "rust" }), 'Se encontraron 2 documentos para "rust":');
});

test("resolves system locale with English fallback and persists selection locally", () => {
  assert.equal(resolveSystemLocale(["pt-BR"]), "pt-BR");
  assert.equal(resolveSystemLocale(["pt-PT"]), "pt-BR");
  assert.equal(resolveSystemLocale(["pt"]), "pt-BR");
  assert.equal(resolveSystemLocale(["es-UY"]), "es-UY");
  assert.equal(resolveSystemLocale(["es-AR"]), "es-UY");
  assert.equal(resolveSystemLocale(["es"]), "es-UY");
  assert.equal(resolveSystemLocale(["en-US"]), "en");
  assert.equal(resolveSystemLocale(["fr-FR", "es-UY"]), "es-UY");
  assert.equal(resolveSystemLocale(["fr-FR", "de-DE"]), "en");
  assert.equal(resolveSystemLocale([]), "en");

  const store = new Map();
  const storage = {
    getItem: (key) => store.get(key) ?? null,
    setItem: (key, value) => store.set(key, value),
  };

  assert.equal(getInitialLocale(storage, ["pt-BR"]), "pt-BR");
  saveLocale("es-UY", storage);
  assert.equal(store.get(LOCALE_STORAGE_KEY), "es-UY");
  assert.equal(getInitialLocale(storage, ["pt-BR"]), "es-UY");

  store.set(LOCALE_STORAGE_KEY, "invalid-locale");
  assert.equal(getInitialLocale(storage, ["fr-FR"]), "en");
});

test("translates backend error codes and falls back cleanly for unknown errors", () => {
  assert.equal(
    translateError("en", "profile.required_fields", "profile.saveError"),
    "Please complete all required profile fields.",
  );
  assert.equal(
    translateError("pt-BR", "profile.required_fields", "profile.saveError"),
    "Preencha todos os campos obrigatórios do perfil.",
  );
  assert.equal(
    translateError("es-UY", "profile.required_fields", "profile.saveError"),
    "Completá todos los campos obligatorios del perfil.",
  );
  assert.equal(
    translateError("es-UY", new Error("unexpected"), "chat.searchError"),
    "No se pudieron buscar los documentos.",
  );
  assert.equal(
    translateWarning("pt-BR", "ocr_failed_no_pdf_text"),
    "O OCR falhou e nenhum texto foi extraído do PDF.",
  );
});

test("formats dates and numbers using Intl according to active locale", () => {
  const expectedDate = (locale) =>
    new Intl.DateTimeFormat(locale, { timeZone: "UTC" }).format(new Date(Date.UTC(2025, 4, 10)));
  const expectedNumber = (locale) =>
    new Intl.NumberFormat(locale, { maximumFractionDigits: 2 }).format(1234.5);

  for (const locale of supportedLocales) {
    assert.equal(formatDate(locale, "2025-05-10"), expectedDate(locale));
    assert.equal(formatDate(locale, null), "—");
    assert.equal(formatNumber(locale, 1234.5), expectedNumber(locale));
  }
});
