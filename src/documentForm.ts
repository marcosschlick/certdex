export type DocumentForm = {
  type: string;
  title: string;
  role: string;
  issue_date: string;
  start_date: string;
  end_date: string;
  workload_hours: string;
  city: string;
  country: string;
  validation_code: string;
  validation_url: string;
};

export type DocumentAnalysis = {
  [Field in keyof DocumentForm]: Field extends "workload_hours" ? number | null : string | null;
} & {
  organization?: string | null;
  event?: string | null;
};

export const emptyDocumentForm: DocumentForm = {
  type: "", title: "", role: "", issue_date: "", start_date: "", end_date: "",
  workload_hours: "", city: "", country: "", validation_code: "", validation_url: "",
};

export function applyDocumentAnalysis(current: DocumentForm, edited: ReadonlySet<keyof DocumentForm>, analysis: DocumentAnalysis | null): DocumentForm {
  const result = { ...current };
  for (const field of Object.keys(emptyDocumentForm) as (keyof DocumentForm)[]) {
    if (edited.has(field)) continue;
    const value = analysis?.[field];
    const isDate = field === "issue_date" || field === "start_date" || field === "end_date";
    const text = value == null ? "" : String(value);
    result[field] = isDate && text && !/^\d{4}-\d{2}-\d{2}$/.test(text) ? "" : text;
  }
  return result;
}

type FilterableDocument = {
  type: string;
  title: string | null;
  role: string | null;
  city: string | null;
  country: string | null;
  issue_date: string | null;
  start_date: string | null;
  end_date: string | null;
  organization: string | null;
  event: string | null;
  people: { name: string; role: string }[];
  tags: string[];
};

export function matchesDocumentFilters(document: FilterableDocument, search: string, type: string, year: string, tag: string): boolean {
  const query = search.trim().toLocaleLowerCase();
  return (!type || document.type === type)
    && (!year || (document.issue_date || document.start_date || document.end_date || "").startsWith(year))
    && (!tag || document.tags.includes(tag))
    && (!query || [
      document.title, document.role, document.city, document.country,
      document.organization, document.event,
      ...document.people.flatMap((person) => [person.name, person.role]),
      ...document.tags,
    ].some((value) => value?.toLocaleLowerCase().includes(query)));
}
