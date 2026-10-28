use std::{path::Path, process::Command};

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
}
