use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(serde::Serialize)]
pub struct ExtractedText {
    pub text: String,
    pub has_sufficient_text: bool,
    pub source: &'static str,
    pub used_ocr: bool,
}

pub fn extract_text_with_quality(path: &Path) -> Result<ExtractedText, String> {
    let text = extract_text(path)?;
    let used_ocr = !has_sufficient_text(&text);
    let text = if used_ocr { extract_ocr(path)? } else { text };
    let has_sufficient_text = has_sufficient_text(&text);
    Ok(ExtractedText {
        text,
        has_sufficient_text,
        source: if used_ocr { "ocr" } else { "pdf_text" },
        used_ocr,
    })
}

fn extract_ocr(path: &Path) -> Result<String, String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("Could not access PDF: {error}"))?;
    let directory = create_temp_directory()?;
    let result = run_ocr(
        &path,
        &directory,
        Path::new("pdftoppm"),
        Path::new("tesseract"),
    );
    match fs::remove_dir_all(&directory) {
        Ok(()) => result,
        Err(error) => Err(format!(
            "Could not remove OCR temporary directory {}: {error}{}",
            directory.display(),
            result
                .err()
                .map(|original| format!("; {original}"))
                .unwrap_or_default()
        )),
    }
}

fn create_temp_directory() -> Result<PathBuf, String> {
    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
    loop {
        let candidate = std::env::temp_dir().join(format!(
            "pdf-ocr-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Could not create OCR temporary directory: {error}")),
        }
    }
}

fn read_pdf(data: &[u8]) -> Result<ExtractedText, String> {
    if !data.starts_with(b"%PDF-") {
        return Err("The selected file is not a PDF.".into());
    }
    let directory = create_temp_directory()?;
    let path = directory.join("input.pdf");
    let result = fs::write(&path, data)
        .map_err(|error| format!("Could not prepare PDF for reading: {error}"))
        .and_then(|()| extract_text_with_quality(&path));
    match fs::remove_dir_all(&directory) {
        Ok(()) => result,
        Err(error) => Err(format!(
            "Could not remove temporary PDF directory {}: {error}{}",
            directory.display(),
            result
                .err()
                .map(|original| format!("; {original}"))
                .unwrap_or_default()
        )),
    }
}

#[tauri::command]
pub(crate) async fn read_selected_pdf(data: Vec<u8>) -> Result<ExtractedText, String> {
    tauri::async_runtime::spawn_blocking(move || read_pdf(&data))
        .await
        .map_err(|error| format!("PDF reading failed: {error}"))?
}

fn run_ocr(
    path: &Path,
    directory: &Path,
    renderer: &Path,
    tesseract: &Path,
) -> Result<String, String> {
    let output = Command::new(renderer)
        .arg("-png")
        .arg(path)
        .arg(directory.join("page"))
        .output()
        .map_err(|error| format!("Could not run pdftoppm: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "PDF page rendering failed (pdftoppm): {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut pages: Vec<(u32, PathBuf)> = Vec::new();
    for entry in
        fs::read_dir(directory).map_err(|error| format!("Could not read OCR pages: {error}"))?
    {
        let entry = entry.map_err(|error| format!("Could not read OCR page: {error}"))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(number) = name
            .strip_prefix("page-")
            .and_then(|name| name.strip_suffix(".png"))
            .and_then(|number| number.parse::<u32>().ok())
        {
            pages.push((number, entry.path()));
        }
    }
    pages.sort_unstable_by_key(|(number, _)| *number);
    if pages.is_empty() {
        return Err("PDF page rendering produced no images.".into());
    }
    let mut text = String::new();
    for (number, page) in pages {
        let output = Command::new(tesseract)
            .arg(page)
            .arg("stdout")
            .arg("-l")
            .arg("por+eng+spa")
            .output()
            .map_err(|error| format!("Could not run tesseract for page {number}: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "OCR failed on page {number} (tesseract): {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let page_text = String::from_utf8(output.stdout)
            .map_err(|error| format!("OCR returned invalid UTF-8 on page {number}: {error}"))?;
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&page_text);
    }
    Ok(text)
}

fn has_sufficient_text(text: &str) -> bool {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .take(20)
        .count()
        >= 20
}

pub fn extract_text(path: &Path) -> Result<String, String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("Could not access PDF: {error}"))?;
    let output = Command::new("pdftotext")
        .arg("-enc")
        .arg("UTF-8")
        .arg(&path)
        .arg("-")
        .output()
        .map_err(|error| format!("Could not run pdftotext: {error}"))?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PDF text extraction failed: {}", message.trim()));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("PDF text extraction returned invalid UTF-8: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_text_without_changing_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.pdf");
        let original = std::fs::read(&path).unwrap();
        assert!(extract_text(&path).unwrap().contains("Hello PDF"));
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    #[test]
    fn skips_ocr_for_searchable_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/searchable.pdf");
        let extracted = extract_text_with_quality(&path).unwrap();
        assert!(extracted.has_sufficient_text);
        assert_eq!(extracted.source, "pdf_text");
        assert!(!extracted.used_ocr);
        assert!(
            extracted
                .text
                .contains("This PDF has enough searchable text.")
        );
    }

    #[test]
    fn reads_pdf_bytes_with_source_and_rejects_non_pdf() {
        let data = include_bytes!("../tests/fixtures/searchable.pdf");
        let result = read_pdf(data).unwrap();
        assert_eq!(result.source, "pdf_text");
        assert!(!result.used_ocr);
        assert!(result.text.contains("This PDF has enough searchable text."));
        assert!(read_pdf(b"not a PDF").is_err());
    }

    #[test]
    fn reports_errors_for_missing_or_invalid_files() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(
            extract_text(&root.join("nonexistent.pdf"))
                .unwrap_err()
                .contains("Could not access PDF")
        );
        assert!(
            extract_text(&root.join("Cargo.toml"))
                .unwrap_err()
                .contains("PDF text extraction failed")
        );
    }

    #[test]
    fn checks_useful_characters_without_counting_whitespace() {
        assert!(!has_sufficient_text(" \n\t"));
        assert!(!has_sufficient_text("123456789 1234567890 \n"));
        assert!(has_sufficient_text("1234567890 \n 1234567890"));
        assert!(!has_sufficient_text("éééééééééé"));
        assert!(has_sufficient_text("éééééééééééééééééééé"));
    }

    #[cfg(unix)]
    #[test]
    fn ocr_orders_pages_and_reports_tool_failures() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "ocr-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let renderer = root.join("renderer");
        let tesseract = root.join("tesseract");
        fs::write(
            &renderer,
            "#!/bin/sh\ntouch \"$3-10.png\" \"$3-2.png\" \"$3-1.png\"\n",
        )
        .unwrap();
        fs::write(&tesseract, "#!/bin/sh\n[ \"$2\" = stdout ] && [ \"$3\" = -l ] && [ \"$4\" = por+eng+spa ] || exit 1\nprintf '%s' \"${1##*/}\"\n").unwrap();
        for program in [&renderer, &tesseract] {
            fs::set_permissions(program, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let text = run_ocr(Path::new("input.pdf"), &root, &renderer, &tesseract).unwrap();
        assert_eq!(text, "page-1.png\npage-2.png\npage-10.png");
        let error = run_ocr(
            Path::new("input.pdf"),
            &root,
            Path::new("/bin/false"),
            &tesseract,
        )
        .unwrap_err();
        assert!(error.contains("pdftoppm"));
        let error = run_ocr(
            Path::new("input.pdf"),
            &root,
            &renderer,
            Path::new("/bin/false"),
        )
        .unwrap_err();
        assert!(error.contains("tesseract"));
        fs::remove_dir_all(root).unwrap();
    }
}
