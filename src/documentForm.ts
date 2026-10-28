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

export type DocumentAnalysis = { [Field in keyof DocumentForm]: Field extends "workload_hours" ? number | null : string | null };

export const emptyDocumentForm: DocumentForm = {
  type: "", title: "", role: "", issue_date: "", start_date: "", end_date: "",
  workload_hours: "", city: "", country: "", validation_code: "", validation_url: "",
};

export function applyDocumentAnalysis(current: DocumentForm, edited: ReadonlySet<keyof DocumentForm>, analysis: DocumentAnalysis | null): DocumentForm {
  const result = { ...current };
  for (const field of Object.keys(emptyDocumentForm) as (keyof DocumentForm)[]) {
    if (!edited.has(field)) result[field] = analysis?.[field] == null ? "" : String(analysis[field]);
  }
  return result;
}
