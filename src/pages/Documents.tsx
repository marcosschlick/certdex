import { useEffect, useRef, useState, type ChangeEvent, type FormEvent } from "react";
import { applyDocumentAnalysis, emptyDocumentForm, matchesDocumentFilters, type DocumentAnalysis, type DocumentForm } from "../documentForm";
import type { TranslationKey } from "../i18n.ts";
import { useI18n } from "../i18nContext";
import { buildReportData, calculateTotalWorkload, generateReportPdf, type ReportProfile } from "../report";

const documentTypes = [
  "certificate", "course", "article", "proceedings", "presentation", "event",
  "award", "project", "internship", "monitoring", "other",
];

type DocumentItem = {
  id: number;
  title: string | null;
  type: string;
  issue_date: string | null;
  workload_hours: number | null;
  role: string | null;
  start_date: string | null;
  end_date: string | null;
  city: string | null;
  country: string | null;
  validation_code: string | null;
  validation_url: string | null;
  organization: string | null;
  event: string | null;
  people: { name: string; role: string }[];
  tags: string[];
};

type AcademicOptions = { organizations: string[]; events: string[]; people: string[]; tags: string[] };

type PdfReading = {
  text: string;
  source: "pdf_text" | "ocr";
  used_ocr: boolean;
  has_sufficient_text: boolean;
  warnings: string[];
  regex_analysis?: DocumentAnalysis | null;
};

type UiMessage =
  | { key: TranslationKey; params?: Record<string, string | number> }
  | { error: unknown; fallback: TranslationKey }
  | null;

function Documents() {
  const { locale, t, tDocType, tError, tWarning, formatDate, formatNumber } = useI18n();
  const [formValues, setFormValues] = useState<DocumentForm>(emptyDocumentForm);
  const editedFields = useRef(new Set<string>());
  const [fileName, setFileName] = useState("");
  const [fileStatus, setFileStatus] = useState<UiMessage>(null);
  const [file, setFile] = useState<File | null>(null);
  const [ready, setReady] = useState(false);
  const [saving, setSaving] = useState(false);
  const [reading, setReading] = useState<PdfReading | null>(null);
  const [readingStatus, setReadingStatus] = useState<UiMessage>(null);
  const [analysisStatus, setAnalysisStatus] = useState<UiMessage>(null);
  const [analysisPending, setAnalysisPending] = useState(false);
  const [documents, setDocuments] = useState<DocumentItem[] | null>(null);
  const [listError, setListError] = useState(false);
  const [optionsError, setOptionsError] = useState(false);
  const [openError, setOpenError] = useState<UiMessage>(null);
  const [deleteError, setDeleteError] = useState<UiMessage>(null);
  const [generatingReport, setGeneratingReport] = useState(false);
  const [reportStatus, setReportStatus] = useState<UiMessage>(null);
  const [reportError, setReportError] = useState<UiMessage>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<number | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [filter, setFilter] = useState("");
  const [search, setSearch] = useState("");
  const [year, setYear] = useState("");
  const [tag, setTag] = useState("");
  const [editing, setEditing] = useState<number | null>(null);
  const [organization, setOrganization] = useState("");
  const [academicEvent, setAcademicEvent] = useState("");
  const [people, setPeople] = useState<{ name: string; role: string }[]>([]);
  const [tags, setTags] = useState("");
  const [options, setOptions] = useState<AcademicOptions>({ organizations: [], events: [], people: [], tags: [] });
  const [revision, setRevision] = useState(0);
  const selection = useRef(0);
  const fileInput = useRef<HTMLInputElement>(null);
  const editingHeading = useRef<HTMLHeadingElement>(null);
  const [editRevision, setEditRevision] = useState(0);

  function renderMessage(message: UiMessage): string {
    if (!message) return "";
    return "key" in message
      ? t(message.key, message.params)
      : tError(message.error, message.fallback);
  }

  useEffect(() => () => { selection.current += 1; }, []);

  useEffect(() => {
    if (editing === null) return;
    editingHeading.current?.focus({ preventScroll: true });
    editingHeading.current?.scrollIntoView({ block: "start" });
  }, [editing, editRevision]);

  useEffect(() => {
    let active = true;
    window.__TAURI__.core.invoke<DocumentItem[]>("list_documents")
      .then((items) => { if (active) { setDocuments(items); setListError(false); } })
      .catch(() => { if (active) setListError(true); });
    window.__TAURI__.core.invoke<AcademicOptions>("list_academic_options")
      .then((items) => { if (active) { setOptions(items); setOptionsError(false); } })
      .catch(() => { if (active) setOptionsError(true); });
    return () => { active = false; };
  }, [revision]);

  function handleFieldChange(event: ChangeEvent<HTMLInputElement | HTMLSelectElement>) {
    const name = event.target.name as keyof DocumentForm;
    const value = event.target.value;
    editedFields.current.add(name);
    setFormValues((current) => ({ ...current, [name]: value }));
  }

  async function handleFileChange(event: ChangeEvent<HTMLInputElement>) {
    const selected = ++selection.current;
    const selectedFile = event.target.files?.[0];
    setFile(null);
    setReady(false);
    setFileName("");
    setFileStatus(null);
    setReading(null);
    setReadingStatus(null);
    setAnalysisStatus(null);
    setAnalysisPending(false);
    if (editing === null) {
      editedFields.current.clear();
      setFormValues(emptyDocumentForm);
      setOrganization("");
      setAcademicEvent("");
      setPeople([]);
      setTags("");
    }
    if (!selectedFile) return;
    if (![".pdf", ".png", ".jpg", ".jpeg"].some((extension) => selectedFile.name.toLowerCase().endsWith(extension))) {
      event.target.value = "";
      setFileStatus({ key: "documents.invalidFileExtension" });
      return;
    }
    setFileName(selectedFile.name);
    setFileStatus({ key: "documents.hashing" });
    try {
      const data = Array.from(new Uint8Array(await selectedFile.arrayBuffer()));
      const hash = await window.__TAURI__.core.invoke<string>("hash_document", { data });
      if (selected !== selection.current) return;
      setFileStatus({ key: "documents.checkingDuplicate" });
      const exists = await window.__TAURI__.core.invoke<boolean>("document_hash_exists", { hash, excludeId: editing });
      if (selected === selection.current) {
        setFileStatus(
          exists
            ? { key: "documents.duplicateFile", params: { hash } }
            : { key: "documents.fileHash", params: { hash } },
        );
      }
      if (selected !== selection.current || exists) return;
      setReadingStatus({ key: "documents.readingFile" });
      try {
        const result = await window.__TAURI__.core.invoke<PdfReading>("read_selected_document", { data });
        if (selected === selection.current) {
          setReading(result.text.trim() ? result : { ...result, text: "" });
          setReadingStatus(null);
          setFile(selectedFile);
          setReady(true);
          if (result.text.trim() && result.regex_analysis) {
            setFormValues((current) => applyDocumentAnalysis(current, editedFields.current as Set<keyof DocumentForm>, result.regex_analysis ?? null));
          }
        }
        if (selected !== selection.current) return;
        if (!result.text.trim()) {
          setAnalysisStatus({ key: "documents.noExtractedText" });
          return;
        }
        setAnalysisPending(true);
        setAnalysisStatus({ key: "documents.checkingOllama" });
        try {
          const status = await window.__TAURI__.core.invoke<"model_available" | "model_missing" | "unavailable">("check_ollama");
          if (selected !== selection.current) return;
          if (status !== "model_available") {
            setAnalysisPending(false);
            setAnalysisStatus({
              key: status === "unavailable" ? "documents.ollamaUnavailable" : "documents.ollamaModelMissing",
            });
            return;
          }
          setAnalysisStatus({ key: "documents.analyzing" });
          const analysisResult = await window.__TAURI__.core.invoke<DocumentAnalysis>("analyze_pdf_text", { text: result.text });
          if (selected === selection.current) {
            setFormValues((current) => applyDocumentAnalysis(current, editedFields.current as Set<keyof DocumentForm>, analysisResult));
            if (!editedFields.current.has("organization")) setOrganization(analysisResult.organization ?? "");
            if (!editedFields.current.has("event")) setAcademicEvent(analysisResult.event ?? "");
            setAnalysisPending(false);
            setAnalysisStatus({ key: "documents.analysisComplete" });
          }
        } catch (error) {
          if (selected === selection.current) {
            setAnalysisPending(false);
            setAnalysisStatus({ error, fallback: "documents.analysisError" });
          }
        }
      } catch (error) {
        if (selected === selection.current) setReadingStatus({ error, fallback: "documents.readError" });
      }
    } catch {
      if (selected === selection.current) setFileStatus({ key: "documents.processError" });
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (saving || analysisPending || (editing === null && (!file || !reading || !ready)) || (editing !== null && fileName && (!file || !reading || !ready))) return;
    selection.current += 1;
    setReadingStatus(null);
    setAnalysisStatus(null);
    setAnalysisPending(false);
    const value = (field: keyof DocumentForm) => formValues[field].trim() || null;
    const workload = value("workload_hours");
    setSaving(true);
    setFileStatus({ key: "documents.savingFile" });
    try {
      const document = {
        document_type: formValues.type,
        title: value("title"),
        role: value("role"),
        issue_date: value("issue_date"),
        start_date: value("start_date"),
        end_date: value("end_date"),
        workload_hours: workload === null ? null : Number(workload),
        city: value("city"),
        country: value("country"),
        validation_code: value("validation_code"),
        validation_url: value("validation_url"),
        original_filename: file?.name ?? "",
        extracted_text: reading?.text ?? "",
        organization: organization.trim() || null,
        event: academicEvent.trim() || null,
        people,
        tags: tags.split(",").map((entry) => entry.trim()).filter(Boolean),
      };
      let nextMessage: UiMessage;
      if (editing !== null) {
        const data = file ? Array.from(new Uint8Array(await file.arrayBuffer())) : null;
        await window.__TAURI__.core.invoke("update_document", { id: editing, document, data });
        nextMessage = { key: "documents.updated" };
      } else {
        const data = Array.from(new Uint8Array(await file!.arrayBuffer()));
        const path = await window.__TAURI__.core.invoke<string>("store_document", { data, document });
        nextMessage = { key: "documents.registered", params: { path } };
      }
      setReady(false);
      setFile(null);
      setFileName("");
      setFormValues(emptyDocumentForm);
      editedFields.current.clear();
      setReading(null);
      setEditing(null);
      setOrganization("");
      setAcademicEvent("");
      setPeople([]);
      setTags("");
      if (fileInput.current) fileInput.current.value = "";
      setFileStatus(nextMessage);
      setRevision((current) => current + 1);
    } catch (error) {
      setFileStatus({ error, fallback: "documents.saveError" });
    } finally {
      setSaving(false);
    }
  }

  const visibleDocuments = documents?.filter((document) => matchesDocumentFilters(document, search, filter, year, tag));
  const years = [...new Set((documents ?? []).map((document) => (document.issue_date || document.start_date || document.end_date || "").slice(0, 4)).filter(Boolean))].sort().reverse();
  const totalHours = calculateTotalWorkload(visibleDocuments ?? []);

  function editDocument(document: DocumentItem) {
    selection.current += 1;
    setEditing(document.id);
    setEditRevision((value) => value + 1);
    setFile(null);
    setReady(false);
    setFileName("");
    setFileStatus(null);
    setReading(null);
    setReadingStatus(null);
    setAnalysisStatus(null);
    setAnalysisPending(false);
    editedFields.current = new Set([...Object.keys(emptyDocumentForm), "organization", "event"]);
    setFormValues({ type: document.type, title: document.title ?? "", role: document.role ?? "", issue_date: document.issue_date ?? "", start_date: document.start_date ?? "", end_date: document.end_date ?? "", workload_hours: document.workload_hours?.toString() ?? "", city: document.city ?? "", country: document.country ?? "", validation_code: document.validation_code ?? "", validation_url: document.validation_url ?? "" });
    setOrganization(document.organization ?? "");
    setAcademicEvent(document.event ?? "");
    setPeople(document.people);
    setTags(document.tags.join(", "));
    if (fileInput.current) fileInput.current.value = "";
  }

  function cancelEdit() {
    setEditing(null);
    selection.current += 1;
    setFile(null);
    setReady(false);
    setFileName("");
    setReading(null);
    setReadingStatus(null);
    setAnalysisStatus(null);
    setAnalysisPending(false);
    setFormValues(emptyDocumentForm);
    setOrganization("");
    setAcademicEvent("");
    setPeople([]);
    setTags("");
    setFileStatus(null);
    if (fileInput.current) fileInput.current.value = "";
  }

  async function openDocument(id: number) {
    setOpenError(null);
    try {
      await window.__TAURI__.core.invoke("open_document", { id });
    } catch (error) {
      setOpenError({ error, fallback: "documents.openError" });
    }
  }

  async function deleteDocument(id: number) {
    if (deleting) return;
    setDeleting(true);
    setDeleteError(null);
    try {
      await window.__TAURI__.core.invoke("delete_document", { id });
      setConfirmDeleteId(null);
      if (editing === id) cancelEdit();
      setRevision((current) => current + 1);
    } catch (error) {
      setDeleteError({ error, fallback: "documents.deleteError" });
    } finally {
      setDeleting(false);
    }
  }

  async function generateReport() {
    if (generatingReport || !visibleDocuments || visibleDocuments.length === 0) return;
    setGeneratingReport(true);
    setReportStatus(null);
    setReportError(null);
    try {
      const profile = await window.__TAURI__.core.invoke<ReportProfile>("get_profile");
      const report = buildReportData(profile, visibleDocuments, { search, type: filter, year, tag });
      const generatedDate = new Date().toISOString().slice(0, 10);
      const pdfBytes = generateReportPdf(report, generatedDate, locale);
      const filename = `certdex-report-${generatedDate}.pdf`;
      const path = await window.__TAURI__.core.invoke<string>("save_report_pdf", {
        data: Array.from(pdfBytes),
        filename,
      });
      setReportStatus({ key: "documents.reportSaved", params: { path } });
    } catch (error) {
      setReportError({ error, fallback: "documents.reportError" });
    } finally {
      setGeneratingReport(false);
    }
  }

  const documentDisplayTitle = (document: DocumentItem) =>
    document.title || t("documents.fallbackDocumentTitle", { id: document.id });

  const emptyValue = t("documents.emptyValue");

  return (
    <section className="profile">
      <h1>{t("documents.title")}</h1>
      <section className="documents-list" aria-labelledby="documents-list-heading">
        <h2 id="documents-list-heading">{t("documents.registeredTitle")}</h2>
        {optionsError && <p role="alert">{t("documents.optionsLoadError")}</p>}
        <div className="document-filters">
          <label className="document-filter"><span>{t("documents.searchLabel")}</span><input type="search" value={search} onChange={(event) => setSearch(event.target.value)} /></label>
          <label className="document-filter">
            <span>{t("documents.filterTypeLabel")}</span>
            <select value={filter} onChange={(event) => setFilter(event.target.value)}>
              <option value="">{t("documents.allTypes")}</option>
              {documentTypes.map((type) => <option key={type} value={type}>{tDocType(type)}</option>)}
            </select>
          </label>
          <label className="document-filter"><span>{t("documents.filterYearLabel")}</span><select value={year} onChange={(event) => setYear(event.target.value)}><option value="">{t("documents.allYears")}</option>{years.map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
          <label className="document-filter"><span>{t("documents.filterTagLabel")}</span><select value={tag} onChange={(event) => setTag(event.target.value)}><option value="">{t("documents.allTags")}</option>{options.tags.map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
        </div>
        {documents && !listError && <p>{t("documents.totalWorkload", { hours: formatNumber(totalHours) })}</p>}
        {documents && !listError && (
          <div className="profile-actions">
            <button
              type="button"
              disabled={generatingReport || !visibleDocuments?.length}
              onClick={generateReport}
            >
              {generatingReport ? t("documents.generatingReport") : t("documents.generateReport")}
            </button>
            {reportStatus && <p role="status">{renderMessage(reportStatus)}</p>}
            {reportError && <p role="alert">{renderMessage(reportError)}</p>}
          </div>
        )}
        {listError ? <p role="alert">{t("documents.loadError")}</p> : documents === null ? <p role="status">{t("documents.loading")}</p> : (
          visibleDocuments?.length === 0 ? (
            <p>{t("documents.empty")}</p>
          ) : (
            <div className="documents-table">
              <table>
                <thead><tr><th>{t("documents.tableTitle")}</th><th>{t("documents.tableType")}</th><th>{t("documents.tableIssueDate")}</th><th>{t("documents.tableWorkload")}</th><th>{t("documents.tableFile")}</th><th>{t("documents.tableEdit")}</th><th>{t("documents.tableDelete")}</th></tr></thead>
                <tbody>
                  {visibleDocuments?.map((document) => (
                    <tr key={document.id}>
                      <td>{document.title || emptyValue}</td>
                      <td>{tDocType(document.type)}</td>
                      <td>{formatDate(document.issue_date, emptyValue)}</td>
                      <td>{document.workload_hours !== null ? formatNumber(document.workload_hours) : emptyValue}</td>
                      <td><button type="button" className="open-pdf" onClick={() => openDocument(document.id)} aria-label={t("documents.openFileAria", { title: documentDisplayTitle(document) })}>{t("documents.openFile")}</button></td>
                      <td><button type="button" className="open-pdf" disabled={saving || deleting} onClick={() => editDocument(document)} aria-label={t("documents.editAria", { title: documentDisplayTitle(document) })}>{t("documents.edit")}</button></td>
                      <td>
                        {confirmDeleteId === document.id ? (
                          <span className="delete-actions">
                            <button type="button" className="delete-confirm" disabled={deleting} onClick={() => deleteDocument(document.id)} aria-label={t("documents.confirmDeleteAria", { title: documentDisplayTitle(document) })}>{t("documents.confirmDelete")}</button>
                            <button type="button" className="open-pdf" disabled={deleting} onClick={() => setConfirmDeleteId(null)}>{t("documents.cancel")}</button>
                          </span>
                        ) : (
                          <button type="button" className="open-pdf" disabled={saving || deleting} onClick={() => { setDeleteError(null); setConfirmDeleteId(document.id); }} aria-label={t("documents.deleteAria", { title: documentDisplayTitle(document) })}>{t("documents.delete")}</button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )
        )}
        {openError && <p role="alert">{renderMessage(openError)}</p>}
        {deleteError && <p role="alert">{renderMessage(deleteError)}</p>}
      </section>
      <h2 ref={editingHeading} tabIndex={-1} className="document-form-heading">{editing === null ? t("documents.formRegisterTitle") : t("documents.formEditTitle")}</h2>
      <form onSubmit={handleSubmit}>
        <fieldset className="profile-fields" disabled={saving}>
          <label>
            <span>{t("documents.fieldType")}</span>
            <select name="type" value={formValues.type} required onChange={handleFieldChange}>
              <option value="">{t("documents.selectType")}</option>
              {documentTypes.map((type) => <option key={type} value={type}>{tDocType(type)}</option>)}
            </select>
          </label>
          <label><span>{t("documents.fieldTitle")}</span><input name="title" value={formValues.title} onChange={handleFieldChange} /></label>
          <label><span>{t("documents.fieldRole")}</span><input name="role" value={formValues.role} onChange={handleFieldChange} /></label>
          <label><span>{t("documents.fieldIssueDate")}</span><input name="issue_date" value={formValues.issue_date} onChange={handleFieldChange} type="date" /></label>
          <label><span>{t("documents.fieldStartDate")}</span><input name="start_date" value={formValues.start_date} onChange={handleFieldChange} type="date" /></label>
          <label><span>{t("documents.fieldEndDate")}</span><input name="end_date" value={formValues.end_date} onChange={handleFieldChange} type="date" /></label>
          <label><span>{t("documents.fieldWorkload")}</span><input name="workload_hours" value={formValues.workload_hours} onChange={handleFieldChange} type="number" min="0" step="any" /></label>
          <label><span>{t("documents.fieldCity")}</span><input name="city" value={formValues.city} onChange={handleFieldChange} /></label>
          <label><span>{t("documents.fieldCountry")}</span><input name="country" value={formValues.country} onChange={handleFieldChange} /></label>
          <label><span>{t("documents.fieldValidationCode")}</span><input name="validation_code" value={formValues.validation_code} onChange={handleFieldChange} /></label>
          <label className="profile-field-wide"><span>{t("documents.fieldValidationUrl")}</span><input name="validation_url" value={formValues.validation_url} onChange={handleFieldChange} type="url" /></label>
          <label><span>{t("documents.fieldOrganization")}</span><input list="organization-options" value={organization} onChange={(event) => { editedFields.current.add("organization"); setOrganization(event.target.value); }} /></label>
          <label><span>{t("documents.fieldEvent")}</span><input list="event-options" value={academicEvent} onChange={(event) => { editedFields.current.add("event"); setAcademicEvent(event.target.value); }} /></label>
          <datalist id="organization-options">{options.organizations.map((name) => <option key={name} value={name} />)}</datalist>
          <datalist id="event-options">{options.events.map((name) => <option key={name} value={name} />)}</datalist>
          <datalist id="people-options">{options.people.map((name) => <option key={name} value={name} />)}</datalist>
          <label className="profile-field-wide"><span>{t("documents.fieldTags")}</span><input value={tags} onChange={(event) => setTags(event.target.value)} /></label>
          {options.tags.length > 0 && <div className="profile-field-wide existing-tags">{t("documents.existingTags")} {options.tags.map((name) => <button type="button" className="open-pdf" key={name} onClick={() => setTags((current) => current.split(",").some((value) => value.trim().toLocaleLowerCase() === name.toLocaleLowerCase()) ? current : [current.trim(), name].filter(Boolean).join(", "))}>{name}</button>)}</div>}
          <div className="profile-field-wide">
            <p>{t("documents.peopleHeading")}</p>
            {people.map((person, index) => <div className="person-row" key={index}>
              <label><span>{t("documents.personName")}</span><input list="people-options" value={person.name} required onChange={(event) => setPeople((current) => current.map((item, i) => i === index ? { ...item, name: event.target.value } : item))} /></label>
              <label><span>{t("documents.personRole")}</span><input value={person.role} required onChange={(event) => setPeople((current) => current.map((item, i) => i === index ? { ...item, role: event.target.value } : item))} /></label>
              <button type="button" className="open-pdf" onClick={() => setPeople((current) => current.filter((_, i) => i !== index))}>{t("documents.removePerson")}</button>
            </div>)}
            <button type="button" className="open-pdf" onClick={() => setPeople((current) => [...current, { name: "", role: "" }])}>{t("documents.addPerson")}</button>
          </div>
          <label className="profile-field-wide"><span>{editing === null ? t("documents.fileLabel") : t("documents.replaceFileLabel")}</span><input ref={fileInput} type="file" accept=".pdf,.png,.jpg,.jpeg,application/pdf,image/png,image/jpeg" onChange={handleFileChange} /></label>
        </fieldset>
        {readingStatus && <p role="status">{renderMessage(readingStatus)}</p>}
        {reading?.warnings.map((warning) => <p role="alert" key={warning}>{tWarning(warning)}</p>)}
        {analysisStatus && <p role="status">{renderMessage(analysisStatus)}</p>}
        <div className="document-file" aria-live="polite">
          {fileName && <p>{t("documents.selectedFile", { name: fileName })}</p>}
          {fileStatus && <p>{renderMessage(fileStatus)}</p>}
        </div>
        <div className="profile-actions">
          <button type="submit" disabled={saving || analysisPending || ((editing === null || !!fileName) && !ready)}>{saving ? t("documents.saving") : editing === null ? t("documents.registerButton") : t("documents.saveChanges")}</button>
          {editing !== null && <button type="button" disabled={saving} onClick={cancelEdit}>{t("documents.cancel")}</button>}
        </div>
      </form>
    </section>
  );
}

export default Documents;
