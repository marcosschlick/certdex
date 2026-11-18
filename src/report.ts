import { jsPDF } from "jspdf";

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

function formatActiveFilters(filters: ReportFilters): string {
  const parts = [
    filters.search.trim() ? `Search: ${filters.search.trim()}` : "",
    filters.type ? `Type: ${filters.type}` : "",
    filters.year ? `Year: ${filters.year}` : "",
    filters.tag ? `Tag: ${filters.tag}` : "",
  ].filter(Boolean);
  return parts.length > 0 ? parts.join(" | ") : "None (all documents)";
}

export function generateReportPdf(report: ReportData, generatedDate = new Date().toISOString().slice(0, 10)): Uint8Array {
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

  writeWrapped("Certdex - Certificate & Document Report", 16, "bold", 3);
  writeWrapped(`Generated on ${generatedDate}`, 9, "normal", 6);

  writeWrapped("Profile", 12, "bold", 2);
  const location = [report.profile.city, report.profile.state, report.profile.country].filter(Boolean).join(", ");
  const profileLines = [
    `Name: ${report.profile.name}`,
    `Email: ${report.profile.email} | Phone: ${report.profile.phone}`,
    location ? `Location: ${location}` : "",
    report.profile.github ? `GitHub: ${report.profile.github}` : "",
    report.profile.linkedin ? `LinkedIn: ${report.profile.linkedin}` : "",
    report.profile.orcid ? `ORCID: ${report.profile.orcid}` : "",
    report.profile.website ? `Website: ${report.profile.website}` : "",
    report.profile.professional_summary ? `Professional summary: ${report.profile.professional_summary}` : "",
    report.profile.academic_summary ? `Academic summary: ${report.profile.academic_summary}` : "",
  ].filter(Boolean);

  for (const line of profileLines) {
    writeWrapped(line, 10, "normal", 2);
  }
  y += 3;

  writeWrapped(`Active filters: ${formatActiveFilters(report.filters)}`, 9, "normal", 6);
  writeWrapped("Documents", 12, "bold", 3);

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
    doc.text("Title", columns.title.x, y);
    doc.text("Institution", columns.institution.x, y);
    doc.text("Date", columns.date.x, y);
    doc.text("Hours", columns.hours.x + columns.hours.width, y, { align: "right" });
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
    doc.text(titleLines, columns.title.x, y);
    doc.text(institutionLines, columns.institution.x, y);
    doc.text(item.date, columns.date.x, y);
    doc.text(item.hours === null ? "-" : String(item.hours), columns.hours.x + columns.hours.width, y, { align: "right" });
    y += rowHeight;
  }

  ensureSpace(10);
  doc.line(margin, y, pageWidth - margin, y);
  y += 6;
  const noun = report.documents.length === 1 ? "document" : "documents";
  writeWrapped(`Total workload: ${report.totalHours} hours (${report.documents.length} ${noun})`, 11, "bold", 0);

  return new Uint8Array(doc.output("arraybuffer"));
}
