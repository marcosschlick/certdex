import { jsPDF } from "jspdf";
import {
  formatDate,
  formatNumber,
  translate,
  translateDocumentType,
  type Locale,
} from "./i18n.ts";

export type ReportProfile = {
  name: string;
  email: string;
  phone: string;
  city: string;
  state: string;
  country: string;
  github?: string;
  linkedin?: string;
  orcid?: string;
  website?: string;
  professional_summary?: string;
  academic_summary?: string;
};

export type ReportDocument = {
  title: string | null;
  organization: string | null;
  issue_date: string | null;
  start_date: string | null;
  end_date: string | null;
  workload_hours: number | null;
};

export type ReportFilters = {
  search: string;
  type: string;
  year: string;
  tag: string;
};

export type ReportRow = {
  title: string;
  institution: string;
  date: string;
  hours: number | null;
};

export type ReportData = {
  profile: ReportProfile;
  filters: ReportFilters;
  documents: ReportRow[];
  totalHours: number;
};

export function calculateTotalWorkload(documents: readonly Pick<ReportDocument, "workload_hours">[]): number {
  const total = documents.reduce((sum, document) => sum + (document.workload_hours ?? 0), 0);
  return Math.round(total * 100) / 100;
}

export function selectReportDocuments(documents: readonly ReportDocument[]): ReportRow[] {
  return documents.map((document) => ({
    title: document.title?.trim() || "-",
    institution: document.organization?.trim() || "-",
    date: document.issue_date || document.start_date || document.end_date || "-",
    hours: document.workload_hours,
  }));
}

export function buildReportData(
  profile: ReportProfile,
  documents: readonly ReportDocument[],
  filters: ReportFilters,
): ReportData {
  return {
    profile,
    filters,
    documents: selectReportDocuments(documents),
    totalHours: calculateTotalWorkload(documents),
  };
}

function formatActiveFilters(filters: ReportFilters, locale: Locale): string {
  const parts = [
    filters.search.trim() ? translate(locale, "report.filterSearch", { value: filters.search.trim() }) : "",
    filters.type ? translate(locale, "report.filterType", { value: translateDocumentType(locale, filters.type) }) : "",
    filters.year ? translate(locale, "report.filterYear", { value: filters.year }) : "",
    filters.tag ? translate(locale, "report.filterTag", { value: filters.tag }) : "",
  ].filter(Boolean);
  return parts.length > 0 ? parts.join(" | ") : translate(locale, "report.filterNone");
}

export function generateReportPdf(
  report: ReportData,
  generatedDate = new Date().toISOString().slice(0, 10),
  locale: Locale = "en",
): Uint8Array {
  const doc = new jsPDF({ unit: "mm", format: "a4" });
  const margin = 15;
  const pageWidth = doc.internal.pageSize.getWidth();
  const pageHeight = doc.internal.pageSize.getHeight();
  const contentWidth = pageWidth - margin * 2;
  let y = 18;

  const ensureSpace = (height: number) => {
    if (y + height <= pageHeight - margin) return;
    doc.addPage();
    y = margin;
  };

  const writeWrapped = (text: string, fontSize: number, style: "normal" | "bold" = "normal", gap = 5) => {
    doc.setFont("helvetica", style);
    doc.setFontSize(fontSize);
    const lines = doc.splitTextToSize(text, contentWidth) as string[];
    const lineHeight = fontSize * 0.45;
    ensureSpace(lines.length * lineHeight + gap);
    doc.text(lines, margin, y);
    y += lines.length * lineHeight + gap;
  };

  writeWrapped(translate(locale, "report.title"), 16, "bold", 3);
  writeWrapped(
    translate(locale, "report.generatedOn", { date: formatDate(locale, generatedDate, generatedDate) }),
    9,
    "normal",
    6,
  );

  writeWrapped(translate(locale, "report.profileHeading"), 12, "bold", 2);
  const location = [report.profile.city, report.profile.state, report.profile.country].filter(Boolean).join(", ");
  const profileLines = [
    translate(locale, "report.name", { value: report.profile.name }),
    translate(locale, "report.contact", { email: report.profile.email, phone: report.profile.phone }),
    location ? translate(locale, "report.location", { value: location }) : "",
    report.profile.github ? translate(locale, "report.github", { value: report.profile.github }) : "",
    report.profile.linkedin ? translate(locale, "report.linkedin", { value: report.profile.linkedin }) : "",
    report.profile.orcid ? translate(locale, "report.orcid", { value: report.profile.orcid }) : "",
    report.profile.website ? translate(locale, "report.website", { value: report.profile.website }) : "",
    report.profile.professional_summary
      ? translate(locale, "report.professionalSummary", { value: report.profile.professional_summary })
      : "",
    report.profile.academic_summary
      ? translate(locale, "report.academicSummary", { value: report.profile.academic_summary })
      : "",
  ].filter(Boolean);

  for (const line of profileLines) {
    writeWrapped(line, 10, "normal", 2);
  }
  y += 3;

  writeWrapped(
    translate(locale, "report.activeFilters", { value: formatActiveFilters(report.filters, locale) }),
    9,
    "normal",
    6,
  );
  writeWrapped(translate(locale, "report.documentsHeading"), 12, "bold", 3);

  const columns = {
    title: { x: margin, width: 72 },
    institution: { x: margin + 74, width: 54 },
    date: { x: margin + 130, width: 28 },
    hours: { x: margin + 160, width: 20 },
  };

  const drawTableHeader = () => {
    ensureSpace(10);
    doc.setFont("helvetica", "bold");
    doc.setFontSize(9);
    doc.text(translate(locale, "report.columnTitle"), columns.title.x, y);
    doc.text(translate(locale, "report.columnInstitution"), columns.institution.x, y);
    doc.text(translate(locale, "report.columnDate"), columns.date.x, y);
    doc.text(translate(locale, "report.columnHours"), columns.hours.x + columns.hours.width, y, { align: "right" });
    y += 2;
    doc.line(margin, y, pageWidth - margin, y);
    y += 5;
  };

  drawTableHeader();

  for (const item of report.documents) {
    doc.setFont("helvetica", "normal");
    doc.setFontSize(9);
    const titleLines = doc.splitTextToSize(item.title, columns.title.width) as string[];
    const institutionLines = doc.splitTextToSize(item.institution, columns.institution.width) as string[];
    const rowLines = Math.max(titleLines.length, institutionLines.length, 1);
    const rowHeight = rowLines * 4.2 + 2.5;
    if (y + rowHeight > pageHeight - margin) {
      doc.addPage();
      y = margin;
      drawTableHeader();
      doc.setFont("helvetica", "normal");
      doc.setFontSize(9);
    }
    const formattedDate = item.date === "-" ? "-" : formatDate(locale, item.date, "-");
    const formattedHours = item.hours === null ? "-" : formatNumber(locale, item.hours);
    doc.text(titleLines, columns.title.x, y);
    doc.text(institutionLines, columns.institution.x, y);
    doc.text(formattedDate, columns.date.x, y);
    doc.text(formattedHours, columns.hours.x + columns.hours.width, y, { align: "right" });
    y += rowHeight;
  }

  ensureSpace(10);
  doc.line(margin, y, pageWidth - margin, y);
  y += 6;
  const totalKey = report.documents.length === 1 ? "report.totalWorkloadOne" : "report.totalWorkloadMany";
  writeWrapped(
    translate(locale, totalKey, {
      hours: formatNumber(locale, report.totalHours),
      count: formatNumber(locale, report.documents.length),
    }),
    11,
    "bold",
    0,
  );

  return new Uint8Array(doc.output("arraybuffer"));
}
