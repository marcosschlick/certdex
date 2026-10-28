import test from "node:test";
import assert from "node:assert/strict";
import { applyDocumentAnalysis, emptyDocumentForm } from "../src/documentForm.ts";

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
