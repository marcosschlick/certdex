import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import {
  formatDate as formatLocalizedDate,
  formatNumber as formatLocalizedNumber,
  getInitialLocale,
  saveLocale,
  translate,
  translateDocumentType,
  translateError,
  translateWarning,
  type Locale,
  type TranslationKey,
} from "./i18n.ts";

type I18nContextValue = {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (key: TranslationKey, params?: Record<string, string | number>) => string;
  tDocType: (type: string) => string;
  tError: (error: unknown, fallbackKey: TranslationKey) => string;
  tWarning: (code: string) => string;
  formatDate: (value: string | null | undefined, fallback?: string) => string;
  formatNumber: (value: number) => string;
};

function createValue(locale: Locale, setLocale: (locale: Locale) => void): I18nContextValue {
  return {
    locale,
    setLocale,
    t: (key, params) => translate(locale, key, params),
    tDocType: (type) => translateDocumentType(locale, type),
    tError: (error, fallbackKey) => translateError(locale, error, fallbackKey),
    tWarning: (code) => translateWarning(locale, code),
    formatDate: (value, fallback) => formatLocalizedDate(locale, value, fallback),
    formatNumber: (value) => formatLocalizedNumber(locale, value),
  };
}

const I18nContext = createContext<I18nContextValue>(createValue("en", () => {}));

export function LocaleProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => getInitialLocale());

  useEffect(() => {
    if (typeof document !== "undefined") {
      document.documentElement.lang = locale;
    }
  }, [locale]);

  function setLocale(next: Locale) {
    saveLocale(next);
    setLocaleState(next);
  }

  return (
    <I18nContext.Provider value={createValue(locale, setLocale)}>
      {children}
    </I18nContext.Provider>
  );
}

export function useI18n(): I18nContextValue {
  return useContext(I18nContext);
}
