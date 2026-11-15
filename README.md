# Certdex

Certdex is an offline desktop app that catalogs your certificates and documents in PDF. A local LLM reads each file and fills in the metadata for you.

## Problem

Certificates come from everywhere: courses, events, internships, awards. They end up scattered across folders and inboxes, and registering each one by hand is slow. Sending them to online AI services is not ideal either, since they contain personal data.

## How it works

1. You add a PDF.
2. Certdex extracts the text from it.
3. A local LLM (`qwen3:1.7b`, running on Ollama) analyzes the text.
4. The fields are filled in automatically: type, title, role, dates, workload, city, country, code and validation URL.
5. You review, edit and save. Data stays in a local database, so everything works offline and private.

You can also manage a profile, tags, organizations, events and people, and search or filter documents by year, tag and type.

## Name

Certdex is "cert" (short for certificate) plus "dex" (from Pokedex). It is a Pokedex of certificates: you collect, catalog and look up your achievements, while the AI reads each PDF for you.

## Tech stack

- Tauri 2 (Rust backend)
- React 19, TypeScript and Vite (frontend)
- SQLite (local database)
- Ollama with `qwen3:1.7b` (local LLM)

## Prerequisites

- [Node.js](https://nodejs.org/)
- [Rust](https://www.rust-lang.org/tools/install) and the [Tauri system dependencies](https://tauri.app/start/prerequisites/)
- [Ollama](https://ollama.com/) with the model installed:

```
ollama pull qwen3:1.7b
```

## Running

```
npm install
npm run tauri dev
```

Other commands:

```
npm run build
```
