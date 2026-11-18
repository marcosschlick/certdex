import test from "node:test";
import assert from "node:assert/strict";
import { matchesDocumentFilters } from "../src/documentForm.ts";
import { buildReportData, calculateTotalWorkload, generateReportPdf, selectReportDocuments } from "../src/report.ts";

const sampleProfile = {
  name: "Ada Lovelace",
  email: "ada@example.com",
  phone: "+55 53 99999-0000",
  city: "Pelotas",
  state: "RS",
  country: "Brazil",
  github: "https://github.com/ada",
  linkedin: "",
  orcid: "0000-0001-2345-6789",
  website: "",
  professional_summary: "Software developer",
  academic_summary: "",
};

test("calculates total workload across documents including nulls and decimals", () => {
  assert.equal(calculateTotalWorkload([]), 0);
  assert.equal(
    calculateTotalWorkload([
      { workload_hours: 10 },
      { workload_hours: null },
      { workload_hours: 4.5 },
    ]),
    14.5,
  );
});

test("selects report document fields with fallbacks", () => {
  const rows = selectReportDocuments([
    {
      title: "  Rust Workshop  ",
      organization: "IFSul",
      issue_date: "2025-05-10",
      start_date: "2025-05-01",
      end_date: "2025-05-10",
      workload_hours: 12,
    },
    {
      title: null,
      organization: "   ",
      issue_date: null,
      start_date: "2024-03-01",
      end_date: "2024-03-05",
      workload_hours: null,
    },
    {
      title: "Seminar",
      organization: null,
      issue_date: null,
      start_date: null,
      end_date: "2023-11-20",
      workload_hours: 2,
    },
    {
      title: "",
      organization: null,
      issue_date: null,
      start_date: null,
      end_date: null,
      workload_hours: 0,
    },
  ]);

  assert.deepEqual(rows, [
    { title: "Rust Workshop", institution: "IFSul", date: "2025-05-10", hours: 12 },
    { title: "-", institution: "-", date: "2024-03-01", hours: null },
    { title: "Seminar", institution: "-", date: "2023-11-20", hours: 2 },
    { title: "-", institution: "-", date: "-", hours: 0 },
  ]);
});

test("builds report data from filtered documents and calculates matching total workload", () => {
  const documents = [
    {
      type: "course",
      title: "TypeScript Fundamentals",
      role: "Student",
      city: "Pelotas",
      country: "Brazil",
      issue_date: "2025-04-01",
      start_date: null,
      end_date: null,
      organization: "IFSul",
      event: null,
      people: [],
      tags: ["Web"],
      workload_hours: 20,
    },
    {
      type: "article",
      title: "Distributed Systems",
      role: "Author",
      city: "Porto Alegre",
      country: "Brazil",
      issue_date: "2024-08-15",
      start_date: null,
      end_date: null,
      organization: "UFRGS",
      event: "SBC",
      people: [],
      tags: ["Research"],
      workload_hours: 8,
    },
    {
      type: "course",
      title: "Database Design",
      role: "Student",
      city: "Pelotas",
      country: "Brazil",
      issue_date: "2025-06-10",
      start_date: null,
      end_date: null,
      organization: "IFSul",
      event: null,
      people: [],
      tags: ["Data"],
      workload_hours: 15,
    },
  ];

  const filters = { search: "", type: "course", year: "2025", tag: "" };
  const filtered = documents.filter((doc) =>
    matchesDocumentFilters(doc, filters.search, filters.type, filters.year, filters.tag),
  );
  const report = buildReportData(sampleProfile, filtered, filters);

  assert.equal(report.profile.name, "Ada Lovelace");
  assert.deepEqual(report.filters, filters);
  assert.equal(report.documents.length, 2);
  assert.deepEqual(report.documents.map((item) => item.title), ["TypeScript Fundamentals", "Database Design"]);
  assert.equal(report.totalHours, 35);
});

test("generates a valid PDF byte stream including multi-page documents", () => {
  const manyDocuments = Array.from({ length: 45 }, (_, index) => ({
    title: `Certificate ${index + 1} with a descriptive title`,
    organization: "Federal Institute of Sul-rio-grandense",
    issue_date: "2025-01-15",
    start_date: null,
    end_date: null,
    workload_hours: 4,
  }));
  const report = buildReportData(sampleProfile, manyDocuments, { search: "", type: "", year: "", tag: "" });
  const bytes = generateReportPdf(report, "2026-11-18");

  assert.ok(bytes instanceof Uint8Array);
  assert.ok(bytes.length > 500);
  const header = new TextDecoder().decode(bytes.slice(0, 5));
  assert.equal(header, "%PDF-");
});
