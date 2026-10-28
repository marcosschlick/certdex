import test from "node:test";
import assert from "node:assert/strict";
import { applyDocumentAnalysis, emptyDocumentForm, matchesDocumentFilters } from "../src/documentForm.ts";

test("fills identified fields without replacing manual edits", () => {
  const current = { ...emptyDocumentForm, title: "My title", city: "" };
  const edited = new Set(["title", "city"]);
  const result = applyDocumentAnalysis(current, edited, {
    type: "course", title: "Suggested", role: null, issue_date: null, start_date: null,
    end_date: null, workload_hours: 0, city: "Suggested city", country: "Brazil",
    validation_code: null, validation_url: null,
  });
  assert.equal(result.title, "My title");
  assert.equal(result.city, "");
  assert.equal(result.type, "course");
  assert.equal(result.workload_hours, "0");
  assert.equal(result.country, "Brazil");
  assert.equal(result.role, "");
  const reset = applyDocumentAnalysis(result, edited, null);
  assert.equal(reset.title, "My title");
  assert.equal(reset.city, "");
  assert.equal(reset.type, "");
  assert.equal(reset.country, "");
});

test("filters documents by text, type, year and tag together", () => {
  const document = { type: "course", title: "Rust Course", role: "Speaker", city: "Recife", country: "Brazil", issue_date: null, start_date: "2025-02-01", end_date: null, tags: ["Programming"] };
  for (const query of ["rust", "SPEAKER", "recife", "brazil"]) {
    assert.equal(matchesDocumentFilters(document, query, "course", "2025", "Programming"), true);
  }
  assert.equal(matchesDocumentFilters(document, "missing", "", "", ""), false);
  assert.equal(matchesDocumentFilters(document, "", "article", "", ""), false);
  assert.equal(matchesDocumentFilters(document, "", "", "2024", ""), false);
  assert.equal(matchesDocumentFilters(document, "", "", "", "Other"), false);
  assert.equal(matchesDocumentFilters({ ...document, title: null, role: null, city: null, country: null }, "x", "", "", ""), false);
});
