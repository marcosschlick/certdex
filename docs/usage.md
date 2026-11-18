# Usage guide

## Initial profile setup

On the first launch, Certdex asks you to complete your profile before opening the main workspace. Fill in your required contact and location details, then save. After setup, you can switch between Documents, Chat, and Profile from the sidebar, and update your profile, documents folder, or profile photo (PNG, JPEG, GIF, or WebP) at any time.

## Registering a document

Open the Documents page and use the "Register a document" form.

1. Select a PDF, PNG, or JPG file.
2. Certdex hashes the file to check for duplicates, extracts its text (using OCR when needed), prefills deterministic fields with regular expressions, and sends the text to Ollama (`qwen3:1.7b`) when available.
3. Review the prefilled fields: type, title, role, dates, workload hours, city, country, validation code, and validation URL.
4. Add or edit the issuing organization, event, people (with their roles), and tags.
5. Click "Register document" to save the record and store a managed copy of the file.

You can edit or delete any saved document later from the Documents list.

## Searching and filtering

The Documents page provides a text search input and three dropdown filters:

- Search documents matches text across title, role, city, country, organization, event, people, and tags.
- Filter by type narrows the list to one document type (`certificate`, `course`, `article`, `proceedings`, `presentation`, `event`, `award`, `project`, `internship`, `monitoring`, or `other`).
- Filter by year and Filter by tag narrow the list by date year or assigned tag.

## Chat search

Open the Chat page to search your catalog with natural-language queries or keywords.

Type a question or topic (for example, a subject, organization, or event name) and submit. Certdex searches across titles, roles, organizations, events, people, tags, and extracted text. Each result includes buttons to open the file in your default viewer or reveal it in its folder.

## Generating a PDF report

On the Documents page, apply any search or filters you want in the report, then click "Generate PDF report".

Certdex generates a PDF using your profile information, lists any active filters, includes a table of the visible documents (title, institution, date, and workload hours), and calculates the total workload hours.
