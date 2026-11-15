# Development Guidelines

- Always prefer the simplest solution that correctly solves the problem.
- Follow KISS, DRY and SRP.
- Prioritize readability over cleverness.
- Avoid premature abstractions and over-engineering.
- Keep functions, modules and components small and focused.
- Prefer idiomatic and straightforward code.
- Prefer official libraries. Otherwise, use mature, widely adopted and well-maintained libraries.
- Avoid unnecessary dependencies.
- Preserve the existing project architecture and conventions.
- Make the smallest change necessary to solve the current task.
- Do not refactor unrelated code.
- Write tests for important business logic and bug fixes.
- Do not write comments in implementation code. Code should be self-explanatory through clear naming and structure.
- Follow the colors and UI rules defined in `color-palette.md`.
- Code, identifiers, commits and technical artifacts must be written in English.

## Project Context

- Certdex is an offline desktop app that catalogs certificates and documents in PDF. It extracts the PDF text and uses a local LLM (Ollama, qwen3:1.7b) to fill in type, title, role, dates, workload, city, country, code and validation URL.
- Name: "cert" (certificate) + "dex" (from Pokedex), a Pokedex of certificates. Write it as Certdex, without hyphen.
- Stack: Tauri 2 (Rust), React 19, TypeScript, Vite, local database.
- Structure: src/ (frontend), src-tauri/src/ (Rust backend: database, document, pdf, ollama, profile), tests/ (.mjs tests).
- Context: course project for Advanced Topics at IFSul (Analysis and Systems Development).
