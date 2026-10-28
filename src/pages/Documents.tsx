import { useRef, useState, type ChangeEvent } from "react";

const documentTypes = [
  "certificate", "course", "article", "proceedings", "presentation", "event",
  "award", "project", "internship", "monitoring", "other",
];

function Documents() {
  const [fileName, setFileName] = useState("");
  const [fileStatus, setFileStatus] = useState("");
  const selection = useRef(0);

  async function handleFileChange(event: ChangeEvent<HTMLInputElement>) {
    const selected = ++selection.current;
    const file = event.target.files?.[0];
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
      if (selected === selection.current) setFileStatus(`SHA-256: ${hash}`);
    } catch {
      if (selected === selection.current) setFileStatus("Could not process the PDF. Check the file and try again.");
    }
  }

  return (
    <section className="profile">
      <h1>Documents</h1>
      <form onSubmit={(event) => event.preventDefault()}>
        <fieldset className="profile-fields">
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
      </form>
    </section>
  );
}

export default Documents;
