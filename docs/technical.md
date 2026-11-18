# Technical documentation

## Architecture

Certdex is a Tauri 2 desktop application with a React 19, TypeScript, and Vite frontend in `src/` and a Rust backend in `src-tauri/src/`.

The frontend communicates with the Rust backend through Tauri `invoke` commands defined in `src-tauri/src/lib.rs`. On startup, the backend initializes a SQLite database (`app.db`) in the app data directory and manages stored document files in the configured documents directory.

## Extraction flow

When you select a file, `src-tauri/src/pdf.rs`, `src-tauri/src/regex_extraction.rs`, `src-tauri/src/ollama.rs`, and `src-tauri/src/document_analysis.rs` process it in stages:

```text
Selected file (PDF or image)
  -> validate format and compute file hash
  -> if PDF: run `pdftotext -enc UTF-8`
       -> if fewer than 20 non-whitespace characters or extraction warning:
            render pages with `pdftoppm -png`
            run `tesseract <page> stdout -l por+eng+spa --psm 1`
  -> if image: normalize orientation and run `tesseract`
  -> extracted text
       -> regex extractor (`regex_extraction.rs`):
            issue_date, start_date, end_date, workload_hours,
            validation_code, validation_url
       -> Ollama `qwen3:1.7b` (`ollama.rs`):
            type, title, role, organization, event
  -> `combine_analysis` merges regex and LLM outputs into `DocumentAnalysis`
```

When a document is saved, `src-tauri/src/search.rs` indexes its fields in SQLite FTS5 (`documents_fts`) and requests a vector embedding from Ollama (`nomic-embed-text`) to store in `document_embeddings`.

## Where and why AI is used

Certdex uses two local Ollama models on `http://localhost:11434` so personal documents stay on the user's machine:

- `qwen3:1.7b` (`/api/chat`, `temperature: 0`, enforced JSON schema) extracts `type`, `title`, `role`, `organization`, and `event`. Certificates use varied wording and layouts across institutions, so deterministic patterns cannot reliably separate an issuing organization from a recipient or an event name from a course title. Deterministic fields such as dates, workload hours, validation codes, and URLs are handled by regular expressions instead.
- `nomic-embed-text` (`/api/embed`) generates embeddings for documents and Chat queries. In `src-tauri/src/search.rs`, Chat search combines cosine similarity (`0.55` weight) with SQLite FTS5 BM25 scores (`0.45` weight). If Ollama is unavailable or a document has no stored embedding, search falls back to FTS5 keyword matching.

## Database model

SQLite (`app.db`) stores all application data across these tables (defined in `src-tauri/src/database.rs`, `src-tauri/src/entities.rs`, and `src-tauri/src/search.rs`):

- `profile` holds a single row (`id = 1`) with personal info, links, summaries, `photo_path`, and `documents_dir`.
- `documents` stores document metadata (`type`, `title`, `role`, dates, `workload_hours`, `city`, `country`, `validation_code`, `validation_url`), file metadata (`original_filename`, `stored_filename`, `file_path`, `file_hash`), `extracted_text`, `extra_data`, and foreign keys `organization_id` and `event_id`.
- `organizations`, `events`, `people`, and `tags` store normalized entities deduplicated by `normalized_name` (accent-stripped lowercase).
- `document_people` links documents to `people` with a `role` column, and `document_tags` links documents to `tags`.
- `documents_fts` is an FTS5 virtual table (`unicode61 remove_diacritics 2`) indexing document metadata, related entities, and `extracted_text`.
- `document_embeddings` stores the `model` name and raw float embedding `BLOB` per `document_id`.
