import { en, type Dictionary, type TranslationKey } from "./i18n/en.ts";
import { esUY } from "./i18n/es-UY.ts";
import { ptBR } from "./i18n/pt-BR.ts";

export type { Dictionary, TranslationKey };

export const supportedLocales = ["en", "pt-BR", "es-UY"] as const;
export type Locale = (typeof supportedLocales)[number];

export const LOCALE_STORAGE_KEY = "certdex.locale";

export const localeLabels: Record<Locale, string> = {
  en: "English",
  "pt-BR": "Português (Brasil)",
  "es-UY": "Español (Uruguay)",
};

export const dictionaries: Record<Locale, Dictionary> = {
  en,
  "pt-BR": ptBR,
  "es-UY": esUY,
};

export function isSupportedLocale(value: unknown): value is Locale {
  return typeof value === "string" && (supportedLocales as readonly string[]).includes(value);
}

export function resolveSystemLocale(languages?: readonly string[]): Locale {
  const candidates = languages
    ?? (typeof navigator !== "undefined"
      ? (navigator.languages?.length ? navigator.languages : [navigator.language])
      : []);

  for (const raw of candidates) {
    if (!raw) continue;
    const normalized = raw.trim().toLowerCase();
    if (normalized === "pt-br" || normalized === "pt" || normalized.startsWith("pt-")) {
      return "pt-BR";
    }
    if (normalized === "es-uy" || normalized === "es" || normalized.startsWith("es-")) {
      return "es-UY";
    }
    if (normalized === "en" || normalized.startsWith("en-")) {
      return "en";
    }
  }
  return "en";
}

export function getInitialLocale(
  storage: Pick<Storage, "getItem"> | undefined = typeof localStorage !== "undefined" ? localStorage : undefined,
  languages?: readonly string[],
): Locale {
  try {
    const stored = storage?.getItem(LOCALE_STORAGE_KEY);
    if (isSupportedLocale(stored)) return stored;
  } catch {
  }
  return resolveSystemLocale(languages);
}

export function saveLocale(
  locale: Locale,
  storage: Pick<Storage, "setItem"> | undefined = typeof localStorage !== "undefined" ? localStorage : undefined,
): void {
  try {
    storage?.setItem(LOCALE_STORAGE_KEY, locale);
  } catch {
  }
}

export function translate(
  locale: Locale,
  key: TranslationKey,
  params?: Record<string, string | number>,
): string {
  const template = dictionaries[locale][key];
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in params ? String(params[name]) : match,
  );
}

export function translateDocumentType(locale: Locale, type: string): string {
  const key = `docType.${type}` as TranslationKey;
  return key in dictionaries[locale] ? dictionaries[locale][key] : type;
}

export function translateError(
  locale: Locale,
  error: unknown,
  fallbackKey: TranslationKey,
): string {
  if (typeof error === "string") {
    const key = `error.${error}` as TranslationKey;
    if (key in dictionaries[locale]) {
      return dictionaries[locale][key];
    }
  }
  return dictionaries[locale][fallbackKey];
}

export function translateWarning(locale: Locale, code: string): string {
  const key = `warning.${code}` as TranslationKey;
  return key in dictionaries[locale] ? dictionaries[locale][key] : code;
}

export function formatDate(locale: Locale, value: string | null | undefined, fallback = "—"): string {
  if (!value) return fallback;
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value.trim());
  if (!match) return value;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const utcDate = new Date(Date.UTC(year, month - 1, day));
  if (
    utcDate.getUTCFullYear() !== year
    || utcDate.getUTCMonth() !== month - 1
    || utcDate.getUTCDate() !== day
  ) {
    return value;
  }
  return new Intl.DateTimeFormat(locale, { timeZone: "UTC" }).format(utcDate);
}

export function formatNumber(locale: Locale, value: number): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: 2 }).format(value);
}
