use crate::document_analysis::{DocumentAnalysis, LlmDocumentAnalysis, combine_analysis};
use crate::regex_extraction::extract_with_regex;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, metadata::Orientation};
use std::{
    fs,
    io::Cursor,
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
    pub warnings: Vec<String>,
    pub regex_analysis: Option<DocumentAnalysis>,
}

fn build_extracted_text(
    text: String,
    source: &'static str,
    used_ocr: bool,
    warnings: Vec<String>,
) -> ExtractedText {
    let regex_analysis = (!text.trim().is_empty())
        .then(|| combine_analysis(extract_with_regex(&text), LlmDocumentAnalysis::default()));
    ExtractedText {
        has_sufficient_text: has_sufficient_text(&text),
        text,
        source,
        used_ocr,
        warnings,
        regex_analysis,
    }
}

pub fn extract_text_with_quality(path: &Path) -> Result<ExtractedText, String> {
    let (text, warning) = extract_pdf_text(path, Path::new("pdftotext"))
        .unwrap_or_else(|error| (String::new(), Some(error)));
    extract_with_ocr(text, warning, || extract_ocr(path))
}

fn extract_with_ocr(
    text: String,
    pdf_warning: Option<String>,
    ocr: impl FnOnce() -> Result<String, String>,
) -> Result<ExtractedText, String> {
    let pdf_recognized = pdf_warning.is_none();
    let mut warnings: Vec<String> = pdf_warning.into_iter().collect();
    let (text, used_ocr) = if has_sufficient_text(&text) && warnings.is_empty() {
        (text, false)
    } else {
        match ocr() {
            Ok(ocr_text) if !ocr_text.trim().is_empty() || text.trim().is_empty() => {
                (ocr_text, true)
            }
            Ok(_) => {
                warnings.push("ocr_empty_using_pdf_text".into());
                (text, false)
            }
            Err(_) if !text.trim().is_empty() => {
                warnings.push("ocr_failed_using_pdf_text".into());
                (text, false)
            }
            Err(_) => {
                if !pdf_recognized {
                    return Err("document.ocr_and_pdf_text_failed".into());
                }
                warnings.push("ocr_failed_no_pdf_text".into());
                (String::new(), false)
            }
        }
    };
    Ok(build_extracted_text(
        text,
        if used_ocr { "ocr" } else { "pdf_text" },
        used_ocr,
        warnings,
    ))
}

fn extract_ocr(path: &Path) -> Result<String, String> {
    let path = path
        .canonicalize()
        .map_err(|_| "document.pdf_read_failed".to_string())?;
    let directory = create_temp_directory()?;
    let result = run_ocr(
        &path,
        &directory,
        Path::new("pdftoppm"),
        Path::new("tesseract"),
    );
    match fs::remove_dir_all(&directory) {
        Ok(()) => result,
        Err(_) => Err(result.err().unwrap_or_else(|| "document.temp_dir_failed".into())),
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
                .map_err(|_| "document.temp_dir_failed".to_string())?
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
            Err(_) => return Err("document.temp_dir_failed".into()),
        }
    }
}

fn read_pdf(data: &[u8]) -> Result<ExtractedText, String> {
    with_pdf_file(data, extract_text_with_quality)
}

async fn validate_pdf(data: &[u8]) -> Result<(), String> {
    let data = data.to_vec();
    tauri::async_runtime::spawn_blocking(move || {
        with_pdf_file(&data, |path| extract_text(path).map(|_| ()))
    })
    .await
    .map_err(|_| "document.pdf_read_failed".to_string())?
}

fn with_pdf_file<T>(
    data: &[u8],
    operation: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    if !data.starts_with(b"%PDF-") {
        return Err("document.not_a_pdf".into());
    }
    let directory = create_temp_directory()?;
    let path = directory.join("input.pdf");
    let result = fs::write(&path, data)
        .map_err(|_| "document.pdf_read_failed".to_string())
        .and_then(|()| operation(&path));
    match fs::remove_dir_all(&directory) {
        Ok(()) => result,
        Err(_) => Err(result.err().unwrap_or_else(|| "document.temp_dir_failed".into())),
    }
}

pub(crate) async fn validate_document_file(data: &[u8]) -> Result<(), String> {
    if data.starts_with(b"%PDF-") {
        return validate_pdf(data).await;
    }
    let data = data.to_vec();
    tauri::async_runtime::spawn_blocking(move || {
        decode_supported_image(&data).map(|_| ())
    })
    .await
    .map_err(|_| "document.invalid_image".to_string())?
}

fn decode_supported_image(data: &[u8]) -> Result<DynamicImage, String> {
    let format = image::guess_format(data)
        .map_err(|_| "document.unsupported_file_type".to_string())?;
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
        return Err("document.unsupported_file_type".into());
    }
    let mut decoder = ImageReader::with_format(Cursor::new(data), format)
        .into_decoder()
        .map_err(|_| "document.invalid_image".to_string())?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder)
        .map_err(|_| "document.invalid_image".to_string())?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn read_image(data: &[u8], tesseract: &Path) -> Result<ExtractedText, String> {
    let image = decode_supported_image(data)?;
    let directory = create_temp_directory()?;
    let path = directory.join("input.png");
    let ocr_result = image
        .save_with_format(&path, ImageFormat::Png)
        .map_err(|_| "document.ocr_prepare_failed".to_string())
        .and_then(|()| run_tesseract_ocr(&path, tesseract));
    if fs::remove_dir_all(&directory).is_err() {
        return Err(ocr_result.err().unwrap_or_else(|| "document.temp_dir_failed".into()));
    }
    Ok(match ocr_result {
        Ok(text) => build_extracted_text(text, "ocr", true, Vec::new()),
        Err(_) => build_extracted_text(
            String::new(),
            "ocr",
            false,
            vec!["ocr_failed".into()],
        ),
    })
}

fn read_document(data: &[u8]) -> Result<ExtractedText, String> {
    if data.starts_with(b"%PDF-") {
        read_pdf(data)
    } else {
        read_image(data, Path::new("tesseract"))
    }
}

#[tauri::command]
pub(crate) async fn read_selected_document(data: Vec<u8>) -> Result<ExtractedText, String> {
    tauri::async_runtime::spawn_blocking(move || read_document(&data))
        .await
        .map_err(|_| "document.read_failed".to_string())?
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
        .map_err(|_| "document.pdf_render_failed".to_string())?;
    if !output.status.success() {
        return Err("document.pdf_render_failed".into());
    }
    let mut pages: Vec<(u32, PathBuf)> = Vec::new();
    for entry in
        fs::read_dir(directory).map_err(|_| "document.ocr_pages_read_failed".to_string())?
    {
        let entry = entry.map_err(|_| "document.ocr_pages_read_failed".to_string())?;
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
        return Err("document.pdf_render_failed".into());
    }
    let mut text = String::new();
    for (_, page) in pages {
        let page_text = run_tesseract_ocr(&page, tesseract)?;
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&page_text);
    }
    Ok(text)
}

fn run_tesseract_ocr(image: &Path, tesseract: &Path) -> Result<String, String> {
    let output = Command::new(tesseract)
        .arg(image)
        .arg("stdout")
        .arg("-l")
        .arg("por+eng+spa")
        .arg("--psm")
        .arg("1")
        .output()
        .map_err(|_| "document.ocr_failed".to_string())?;
    if !output.status.success() {
        return Err("document.ocr_failed".into());
    }
    String::from_utf8(output.stdout).map_err(|_| "document.ocr_failed".to_string())
}

fn has_sufficient_text(text: &str) -> bool {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .take(20)
        .count()
        >= 20
}

pub fn extract_text(path: &Path) -> Result<String, String> {
    let (text, warning) = extract_pdf_text(path, Path::new("pdftotext"))?;
    match warning {
        Some(error) => Err(error),
        None => Ok(text),
    }
}

fn extract_pdf_text(path: &Path, program: &Path) -> Result<(String, Option<String>), String> {
    let path = path
        .canonicalize()
        .map_err(|_| "document.pdf_read_failed".to_string())?;
    let output = Command::new(program)
        .arg("-enc")
        .arg("UTF-8")
        .arg(&path)
        .arg("-")
        .output()
        .map_err(|_| "document.pdf_text_extraction_failed".to_string())?;
    let text = String::from_utf8(output.stdout)
        .map_err(|_| "document.pdf_text_extraction_failed".to_string())?;
    let warning = (!output.status.success()).then(|| "pdf_text_extraction_failed".to_string());
    Ok((text, warning))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_readable_pdfs_without_requiring_extractable_text() {
        tauri::async_runtime::block_on(async {
            for data in [
                include_bytes!("../tests/fixtures/sample.pdf").as_slice(),
                include_bytes!("../tests/fixtures/searchable.pdf").as_slice(),
            ] {
                validate_pdf(data).await.unwrap();
            }
            let blank = String::from_utf8(include_bytes!("../tests/fixtures/sample.pdf").to_vec())
                .unwrap()
                .replace("(Hello PDF)", "(         )");
            assert!(
                with_pdf_file(blank.as_bytes(), extract_text)
                    .unwrap()
                    .trim()
                    .is_empty()
            );
            validate_pdf(blank.as_bytes()).await.unwrap();
        });
    }

    #[test]
    fn recovers_from_pdf_text_failures_and_preserves_valid_partial_text() {
        let recovered =
            extract_with_ocr(String::new(), Some("pdf_text_extraction_failed".into()), || {
                Ok("Recognized text".into())
            })
            .unwrap();
        assert_eq!(recovered.source, "ocr");
        assert!(recovered.used_ocr);
        assert_eq!(recovered.text, "Recognized text");
        assert_eq!(recovered.warnings, vec!["pdf_text_extraction_failed"]);
        let partial = "Valid partial text that is already sufficient";
        for (ocr, expected_warning) in [
            (Err("document.ocr_failed".into()), "ocr_failed_using_pdf_text"),
            (Ok(String::new()), "ocr_empty_using_pdf_text"),
        ] {
            let result = extract_with_ocr(
                partial.into(),
                Some("pdf_text_extraction_failed".into()),
                || ocr,
            )
            .unwrap();
            assert_eq!(result.text, partial);
            assert_eq!(result.source, "pdf_text");
            assert!(!result.used_ocr);
            assert_eq!(
                result.warnings,
                vec!["pdf_text_extraction_failed", expected_warning]
            );
        }
        let error = extract_with_ocr(
            String::new(),
            Some("pdf_text_extraction_failed".into()),
            || Err("document.ocr_failed".into()),
        )
        .err()
        .unwrap();
        assert_eq!(error, "document.ocr_and_pdf_text_failed");
    }

    #[cfg(unix)]
    #[test]
    fn preserves_stdout_from_failed_pdftotext_process() {
        use std::os::unix::fs::PermissionsExt;
        let root = create_temp_directory().unwrap();
        let program = root.join("pdftotext");
        fs::write(
            &program,
            "#!/bin/sh\nprintf 'Valid partial text'\nprintf 'extraction failed' >&2\nexit 1\n",
        )
        .unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        let input = root.join("input.pdf");
        fs::write(&input, b"%PDF-test").unwrap();
        let (text, warning) = extract_pdf_text(&input, &program).unwrap();
        assert_eq!(text, "Valid partial text");
        assert_eq!(warning.as_deref(), Some("pdf_text_extraction_failed"));
        let result = extract_with_ocr(text, warning, || Err("document.ocr_failed".into())).unwrap();
        assert_eq!(result.text, "Valid partial text");
        fs::remove_dir_all(root).unwrap();
    }

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
        assert!(read_pdf(b"%PDF-1.4\nnot a document\n%%EOF").is_err());
    }

    #[test]
    fn reports_errors_for_missing_or_invalid_files() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(
            extract_text(&root.join("nonexistent.pdf")).unwrap_err(),
            "document.pdf_read_failed"
        );
        assert_eq!(
            extract_text(&root.join("Cargo.toml")).unwrap_err(),
            "pdf_text_extraction_failed"
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

    #[test]
    fn preserves_pdf_text_when_ocr_fails_or_returns_no_text() {
        let result = extract_with_ocr("Short text".into(), None, || {
            Err("document.ocr_failed".into())
        })
        .unwrap();
        assert_eq!(result.text, "Short text");
        assert_eq!(result.source, "pdf_text");
        assert!(!result.used_ocr);
        assert!(!result.has_sufficient_text);
        assert_eq!(result.warnings, vec!["ocr_failed_using_pdf_text"]);

        let result = extract_with_ocr("Short text".into(), None, || Ok(" \n".into())).unwrap();
        assert_eq!(result.text, "Short text");
        assert_eq!(result.warnings, vec!["ocr_empty_using_pdf_text"]);
    }

    #[test]
    fn allows_empty_text_for_recognized_pdfs_and_preserves_successful_extraction() {
        let result =
            extract_with_ocr(" \n".into(), None, || Err("document.ocr_failed".into())).unwrap();
        assert!(result.text.is_empty());
        assert!(!result.has_sufficient_text);
        assert!(!result.used_ocr);
        assert_eq!(result.source, "pdf_text");
        assert_eq!(result.warnings, vec!["ocr_failed_no_pdf_text"]);

        let result = extract_with_ocr("Short text".into(), None, || {
            Ok("Recognized document text".into())
        })
        .unwrap();
        assert_eq!(result.text, "Recognized document text");
        assert_eq!(result.source, "ocr");
        assert!(result.used_ocr);
        assert!(result.has_sufficient_text);
        assert!(result.warnings.is_empty());

        let result = extract_with_ocr("This text is already sufficient".into(), None, || {
            panic!("OCR should not run")
        })
        .unwrap();
        assert_eq!(result.source, "pdf_text");
        assert!(result.warnings.is_empty());
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
        fs::write(&tesseract, "#!/bin/sh\n[ \"$2\" = stdout ] && [ \"$3\" = -l ] && [ \"$4\" = por+eng+spa ] && [ \"$5\" = --psm ] && [ \"$6\" = 1 ] || exit 1\nprintf '%s' \"${1##*/}\"\n").unwrap();
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
        assert_eq!(error, "document.pdf_render_failed");
        let error = run_ocr(
            Path::new("input.pdf"),
            &root,
            &renderer,
            Path::new("/bin/false"),
        )
        .unwrap_err();
        assert_eq!(error, "document.ocr_failed");
        fs::remove_dir_all(root).unwrap();
    }

    fn jpeg_with_exif_orientation(width: u32, height: u32, orientation: u16) -> Vec<u8> {
        let image = DynamicImage::new_rgb8(width, height);
        let mut raw_jpeg = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut raw_jpeg), ImageFormat::Jpeg)
            .unwrap();
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II*\0");
        tiff.extend_from_slice(&8u32.to_le_bytes());
        tiff.extend_from_slice(&1u16.to_le_bytes());
        tiff.extend_from_slice(&0x0112u16.to_le_bytes());
        tiff.extend_from_slice(&3u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&orientation.to_le_bytes());
        tiff.extend_from_slice(&0u16.to_le_bytes());
        tiff.extend_from_slice(&0u32.to_le_bytes());
        let mut app1 = Vec::new();
        app1.extend_from_slice(&[0xFF, 0xE1]);
        let length = (2 + 6 + tiff.len()) as u16;
        app1.extend_from_slice(&length.to_be_bytes());
        app1.extend_from_slice(b"Exif\0\0");
        app1.extend_from_slice(&tiff);
        let mut result = Vec::with_capacity(raw_jpeg.len() + app1.len());
        result.extend_from_slice(&raw_jpeg[..2]);
        result.extend_from_slice(&app1);
        result.extend_from_slice(&raw_jpeg[2..]);
        result
    }

    #[test]
    fn applies_exif_orientation_and_rejects_unsupported_image_formats() {
        let oriented_jpeg = jpeg_with_exif_orientation(4, 2, 6);
        let decoded = decode_supported_image(&oriented_jpeg).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (2, 4));

        let mut png = Vec::new();
        DynamicImage::new_rgba8(3, 5)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        let decoded_png = decode_supported_image(&png).unwrap();
        assert_eq!((decoded_png.width(), decoded_png.height()), (3, 5));

        let mut gif = Vec::new();
        DynamicImage::new_rgba8(2, 2)
            .write_to(&mut Cursor::new(&mut gif), ImageFormat::Gif)
            .unwrap();
        assert!(decode_supported_image(&gif).is_err());
        assert!(decode_supported_image(b"not an image").is_err());
        assert!(decode_supported_image(&oriented_jpeg[..10]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn image_ocr_uses_osd_and_handles_tesseract_failures() {
        use std::os::unix::fs::PermissionsExt;
        let root = create_temp_directory().unwrap();
        let tesseract = root.join("tesseract");
        fs::write(
            &tesseract,
            "#!/bin/sh\n[ \"$2\" = stdout ] && [ \"$3\" = -l ] && [ \"$4\" = por+eng+spa ] && [ \"$5\" = --psm ] && [ \"$6\" = 1 ] || exit 1\nprintf 'Recognized certificate text'\n",
        )
        .unwrap();
        fs::set_permissions(&tesseract, fs::Permissions::from_mode(0o700)).unwrap();

        let jpeg = jpeg_with_exif_orientation(4, 2, 6);
        let extracted = read_image(&jpeg, &tesseract).unwrap();
        assert_eq!(extracted.source, "ocr");
        assert!(extracted.used_ocr);
        assert!(extracted.has_sufficient_text);
        assert_eq!(extracted.text, "Recognized certificate text");
        assert!(extracted.warnings.is_empty());

        let failed = read_image(&jpeg, Path::new("/bin/false")).unwrap();
        assert_eq!(failed.source, "ocr");
        assert!(!failed.used_ocr);
        assert!(!failed.has_sufficient_text);
        assert!(failed.text.is_empty());
        assert!(failed.regex_analysis.is_none());
        assert_eq!(failed.warnings, vec!["ocr_failed"]);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn populates_regex_analysis_only_when_extracted_text_is_non_empty() {
        let with_fields = extract_with_ocr(
            "Emitido em 10/05/2025. Carga horária: 16h.".into(),
            None,
            || Ok(String::new()),
        )
        .unwrap();
        let regex = with_fields.regex_analysis.unwrap();
        assert_eq!(regex.issue_date.as_deref(), Some("2025-05-10"));
        assert_eq!(regex.workload_hours, Some(16.0));
        assert_eq!(regex.title, None);

        let empty = extract_with_ocr("   ".into(), None, || Ok(String::new())).unwrap();
        assert!(empty.regex_analysis.is_none());
    }
}
