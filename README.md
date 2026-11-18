# Certdex

Certdex is an offline desktop app for cataloging PDF and image certificates. It extracts text from each file, runs a local LLM through Ollama to fill in metadata, and stores everything in a local SQLite database.

The name combines "cert" (certificate) and "dex" (from Pokedex).

## Tech stack

- Tauri 2 (Rust backend)
- React 19, TypeScript, and Vite (frontend)
- SQLite via `sqlx` (local database and FTS5 index)
- Ollama with `qwen3:1.7b` and `nomic-embed-text` (local LLM and embeddings)
- `poppler-utils` and `tesseract` (PDF text extraction and OCR)

## Prerequisites

Install these tools before running Certdex:

- [Node.js](https://nodejs.org/)
- [Rust](https://www.rust-lang.org/tools/install) and the [Tauri 2 system dependencies](https://tauri.app/start/prerequisites/)
- `poppler-utils` (provides `pdftotext` and `pdftoppm`)
- `tesseract` with Portuguese (`por`), English (`eng`), and Spanish (`spa`) language data (Certdex runs OCR with `-l por+eng+spa`)
- [Ollama](https://ollama.com/)

On Debian or Ubuntu, you can install the PDF and OCR tools with:

```bash
sudo apt install poppler-utils tesseract-ocr tesseract-ocr-por tesseract-ocr-eng tesseract-ocr-spa
```

Pull the two Ollama models used for metadata extraction and semantic search:

```bash
ollama pull qwen3:1.7b
ollama pull nomic-embed-text
```

Make sure Ollama is running locally on `http://localhost:11434` before using AI extraction or semantic search.

## Installation and running

Install frontend dependencies and start the app in development mode:

```bash
npm install
npm run tauri dev
```

To build the frontend and Tauri bundle:

```bash
npm run build
npm run tauri build
```

## Documentation

- [Usage guide](docs/usage.md) covers initial profile setup, registering documents, searching, chat search, and PDF reports.
- [Technical documentation](docs/technical.md) covers architecture, the extraction pipeline, AI usage, and the SQLite schema.
