import { useRef, useState, type ChangeEvent, type FormEvent } from "react";

const documentTypes = [
  "certificate", "course", "article", "proceedings", "presentation", "event",
  "award", "project", "internship", "monitoring", "other",
];

function Documents() {
  const [fileName, setFileName] = useState("");
  const [fileStatus, setFileStatus] = useState("");
  const [file, setFile] = useState<File | null>(null);
  const [ready, setReady] = useState(false);
  const [saving, setSaving] = useState(false);
  const selection = useRef(0);

  async function handleFileChange(event: ChangeEvent<HTMLInputElement>) {
    const selected = ++selection.current;
    const file = event.target.files?.[0];
    setFile(null);
    setReady(false);
    setFileName("");
    setFileStatus("");
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
        if (!exists) {
          setFile(file);
          setReady(true);
        }
      }
    } catch {
      if (selected === selection.current) setFileStatus("Could not process the PDF. Check the file and try again.");
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!file || !ready || saving) return;
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
      setFileStatus(`Document registered: ${path}`);
    } catch (error) {
      setFileStatus(typeof error === "string" ? error : "Could not save the PDF. Please try again.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="profile">
      <h1>Documents</h1>
      <form onSubmit={handleSubmit}>
        <fieldset className="profile-fields" disabled={saving}>
          <label>
            <span>Document type</span>
            <select name="type" defaultValue="certificate">
              {documentTypes.map((type) => <option key={type} value={type}>{type[0].toUpperCase() + type.slice(1)}</option>)}
            </select>
          </label>
          <label><span>Title</span><input name="title" /></label>
          <label><span>Role</span><input name="role" /></label>
          <label><span>Issue date</span><input name="issue_date" type="date" /></label>
          <label><span>Start date</span><input name="start_date" type="date" /></label>
          <label><span>End date</span><input name="end_date" type="date" /></label>
          <label><span>Workload (hours)</span><input name="workload_hours" type="number" min="0" step="any" /></label>
          <label><span>City</span><input name="city" /></label>
          <label><span>Country</span><input name="country" /></label>
          <label><span>Validation code</span><input name="validation_code" /></label>
          <label className="profile-field-wide"><span>Validation URL</span><input name="validation_url" type="url" /></label>
          <label className="profile-field-wide"><span>PDF file</span><input type="file" accept=".pdf,application/pdf" onChange={handleFileChange} /></label>
        </fieldset>
        <div className="document-file" aria-live="polite">
          {fileName && <p>Selected file: {fileName}</p>}
          {fileStatus && <p>{fileStatus}</p>}
        </div>
        <div className="profile-actions">
          <button type="submit" disabled={!ready || saving}>{saving ? "Saving…" : "Register document"}</button>
        </div>
      </form>
    </section>
  );
}

export default Documents;
