import { useEffect, useRef, useState, type ChangeEvent, type FormEvent } from "react";
import { applyDocumentAnalysis, emptyDocumentForm, type DocumentAnalysis, type DocumentForm } from "../documentForm";

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
};

type PdfReading = {
  text: string;
  source: "pdf_text" | "ocr";
  used_ocr: boolean;
  has_sufficient_text: boolean;
};


function Documents() {
  const [formValues, setFormValues] = useState<DocumentForm>(emptyDocumentForm);
  const editedFields = useRef(new Set<keyof DocumentForm>());
  const [fileName, setFileName] = useState("");
  const [fileStatus, setFileStatus] = useState("");
  const [file, setFile] = useState<File | null>(null);
  const [ready, setReady] = useState(false);
  const [saving, setSaving] = useState(false);
  const [reading, setReading] = useState<PdfReading | null>(null);
  const [readingStatus, setReadingStatus] = useState("");
  const [analysis, setAnalysis] = useState<DocumentAnalysis | null>(null);
  const [analysisStatus, setAnalysisStatus] = useState("");
  const [documents, setDocuments] = useState<DocumentItem[] | null>(null);
  const [listError, setListError] = useState("");
  const [openError, setOpenError] = useState("");
  const [filter, setFilter] = useState("");
  const [revision, setRevision] = useState(0);
  const selection = useRef(0);
  const fileInput = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let active = true;
    window.__TAURI__.core.invoke<DocumentItem[]>("list_documents")
      .then((items) => { if (active) { setDocuments(items); setListError(""); } })
      .catch(() => { if (active) setListError("Could not load documents."); });
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
    const file = event.target.files?.[0];
    setFile(null);
    setReady(false);
    setFileName("");
    setFileStatus("");
    setReading(null);
    setReadingStatus("");
    setAnalysis(null);
    setAnalysisStatus("");
    editedFields.current.clear();
    setFormValues(emptyDocumentForm);
    if (!file) return;
    if (!file.name.toLowerCase().endsWith(".pdf")) {
      event.target.value = "";
      setFileStatus("Please select a PDF file.");
      return;
    }
    setFileName(file.name);
    setFileStatus("Calculating SHA-256…");
    try {
      const data = Array.from(new Uint8Array(await file.arrayBuffer()));
      const hash = await window.__TAURI__.core.invoke<string>("hash_pdf", { data });
      if (selected !== selection.current) return;
      setFileStatus("Checking for duplicates…");
      const exists = await window.__TAURI__.core.invoke<boolean>("document_hash_exists", { hash });
      if (selected === selection.current) {
        setFileStatus(exists ? `This file has already been registered.\nSHA-256: ${hash}` : `SHA-256: ${hash}`);
      }
      if (selected !== selection.current) return;
      setReadingStatus("Reading PDF…");
      try {
        const result = await window.__TAURI__.core.invoke<PdfReading>("read_selected_pdf", { data });
        if (selected === selection.current) {
          setReading(result);
          setReadingStatus("");
        }
        if (selected !== selection.current) return;
        setAnalysisStatus("Checking Ollama…");
        try {
          const status = await window.__TAURI__.core.invoke<"model_available" | "model_missing" | "unavailable">("check_ollama");
          if (selected !== selection.current) return;
          if (status !== "model_available") {
            setAnalysisStatus(status === "unavailable" ? "Ollama is unavailable. You can continue filling out the form manually." : "The qwen3:1.7b model is not installed. You can continue filling out the form manually.");
            return;
          }
          setAnalysisStatus("Analyzing document…");
          const analysisResult = await window.__TAURI__.core.invoke<DocumentAnalysis>("analyze_pdf_text", { text: result.text });
          if (selected === selection.current) {
            setAnalysis(analysisResult);
            setFormValues((current) => applyDocumentAnalysis(current, editedFields.current, analysisResult));
            setAnalysisStatus("Analysis complete. Review the fields before registering the document.");
          }
        } catch (error) {
          if (selected === selection.current) setAnalysisStatus(typeof error === "string" ? error : "Could not analyze the document.");
        } finally {
          if (selected === selection.current && !exists) {
            setFile(file);
            setReady(true);
          }
        }
      } catch (error) {
        if (selected === selection.current) setReadingStatus(typeof error === "string" ? error : "Could not read the PDF.");
      }
    } catch {
      if (selected === selection.current) setFileStatus("Could not process the PDF. Check the file and try again.");
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!file || !ready || saving) return;
    selection.current += 1;
    setReadingStatus("");
    setAnalysisStatus("");
    const form = new FormData(event.currentTarget);
    const value = (name: string) => String(form.get(name) || "") || null;
    setSaving(true);
    setFileStatus("Saving PDF…");
    try {
      const data = Array.from(new Uint8Array(await file.arrayBuffer()));
      const path = await window.__TAURI__.core.invoke<string>("store_document_pdf", {
        data,
        document: {
          document_type: form.get("type"),
          title: value("title"),
          role: value("role"),
          issue_date: value("issue_date"),
          start_date: value("start_date"),
          end_date: value("end_date"),
          workload_hours: value("workload_hours") === null ? null : Number(value("workload_hours")),
          city: value("city"),
          country: value("country"),
          validation_code: value("validation_code"),
          validation_url: value("validation_url"),
          original_filename: file.name,
        },
      });
      setReady(false);
      setFile(null);
      setFileName("");
      setFormValues(emptyDocumentForm);
      editedFields.current.clear();
      setReading(null);
      setAnalysis(null);
      if (fileInput.current) fileInput.current.value = "";
      setFileStatus(`Document registered: ${path}`);
      setRevision((current) => current + 1);
    } catch (error) {
      setFileStatus(typeof error === "string" ? error : "Could not save the PDF. Please try again.");
    } finally {
      setSaving(false);
    }
  }

  const visibleDocuments = documents?.filter((document) => !filter || document.type === filter);

  async function openPdf(id: number) {
    setOpenError("");
    try {
      await window.__TAURI__.core.invoke("open_document_pdf", { id });
    } catch {
      setOpenError("Could not open the PDF. Check that the file is available.");
    }
  }

  return (
    <section className="profile">
      <h1>Documents</h1>
      <section className="documents-list" aria-labelledby="documents-list-heading">
        <h2 id="documents-list-heading">Registered documents</h2>
        <label className="document-filter">
          <span>Filter by type</span>
          <select value={filter} onChange={(event) => setFilter(event.target.value)}>
            <option value="">All types</option>
            {documentTypes.map((type) => <option key={type} value={type}>{type[0].toUpperCase() + type.slice(1)}</option>)}
          </select>
        </label>
        {listError ? <p role="alert">{listError}</p> : documents === null ? <p role="status">Loading documents…</p> : (
          visibleDocuments?.length === 0 ? (
            <p>No documents found.</p>
          ) : (
            <div className="documents-table">
              <table>
                <thead><tr><th>Title</th><th>Type</th><th>Issue date</th><th>Workload (hours)</th><th>PDF</th></tr></thead>
                <tbody>
                  {visibleDocuments?.map((document) => (
                    <tr key={document.id}>
                      <td>{document.title || "—"}</td>
                      <td>{document.type}</td>
                      <td>{document.issue_date || "—"}</td>
                      <td>{document.workload_hours ?? "—"}</td>
                      <td><button type="button" className="open-pdf" onClick={() => openPdf(document.id)} aria-label={`Open PDF for ${document.title || `document ${document.id}`}`}>Open PDF</button></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )
        )}
        {openError && <p role="alert">{openError}</p>}
      </section>
      <h2 className="document-form-heading">Register a document</h2>
      <form onSubmit={handleSubmit}>
        <fieldset className="profile-fields" disabled={saving}>
          <label>
            <span>Document type</span>
            <select name="type" value={formValues.type} required onChange={handleFieldChange}>
              <option value="">Select a type</option>
              {documentTypes.map((type) => <option key={type} value={type}>{type[0].toUpperCase() + type.slice(1)}</option>)}
            </select>
          </label>
          <label><span>Title</span><input name="title" value={formValues.title} onChange={handleFieldChange} /></label>
          <label><span>Role</span><input name="role" value={formValues.role} onChange={handleFieldChange} /></label>
          <label><span>Issue date</span><input name="issue_date" value={formValues.issue_date} onChange={handleFieldChange} type="date" /></label>
          <label><span>Start date</span><input name="start_date" value={formValues.start_date} onChange={handleFieldChange} type="date" /></label>
          <label><span>End date</span><input name="end_date" value={formValues.end_date} onChange={handleFieldChange} type="date" /></label>
          <label><span>Workload (hours)</span><input name="workload_hours" value={formValues.workload_hours} onChange={handleFieldChange} type="number" min="0" step="any" /></label>
          <label><span>City</span><input name="city" value={formValues.city} onChange={handleFieldChange} /></label>
          <label><span>Country</span><input name="country" value={formValues.country} onChange={handleFieldChange} /></label>
          <label><span>Validation code</span><input name="validation_code" value={formValues.validation_code} onChange={handleFieldChange} /></label>
          <label className="profile-field-wide"><span>Validation URL</span><input name="validation_url" value={formValues.validation_url} onChange={handleFieldChange} type="url" /></label>
          <label className="profile-field-wide"><span>PDF file</span><input ref={fileInput} type="file" accept=".pdf,application/pdf" onChange={handleFileChange} /></label>
        </fieldset>
        {analysisStatus && <p role="status">{analysisStatus}</p>}
        <div className="document-file" aria-live="polite">
          {fileName && <p>Selected file: {fileName}</p>}
          {fileStatus && <p>{fileStatus}</p>}
        </div>
        <div className="profile-actions">
          <button type="submit" disabled={!ready || saving}>{saving ? "Saving…" : "Register document"}</button>
        </div>
      </form>
      <section className="pdf-debug" aria-labelledby="pdf-debug-heading">
        <h2 id="pdf-debug-heading">PDF text (debug)</h2>
        {readingStatus && <p role="status">{readingStatus}</p>}
        {reading && <>
          <p>Source: {reading.source} · OCR used: {reading.used_ocr ? "yes" : "no"}</p>
          {!reading.has_sufficient_text && <p>The extracted text contains fewer than 20 non-whitespace characters.</p>}
          <pre>{reading.text || "No text extracted."}</pre>
        </>}
        {!reading && !readingStatus && <p>Select a PDF to inspect its extracted text.</p>}
        {reading && <>
          <h2>Document analysis (debug)</h2>
          {analysis && <pre>{JSON.stringify(analysis, null, 2)}</pre>}
        </>}
      </section>
    </section>
  );
}

export default Documents;
