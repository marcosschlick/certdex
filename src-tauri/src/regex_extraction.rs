use crate::document::is_valid_date;
use crate::document_analysis::RegexDocumentAnalysis;
use regex::Regex;
use std::sync::LazyLock;

const MONTH_NAMES: &[(&str, u16)] = &[
    ("janeiro", 1),
    ("january", 1),
    ("jan", 1),
    ("fevereiro", 2),
    ("february", 2),
    ("feb", 2),
    ("fev", 2),
    ("março", 3),
    ("marco", 3),
    ("march", 3),
    ("mar", 3),
    ("abril", 4),
    ("april", 4),
    ("apr", 4),
    ("abr", 4),
    ("maio", 5),
    ("may", 5),
    ("mai", 5),
    ("junho", 6),
    ("june", 6),
    ("jun", 6),
    ("julho", 7),
    ("july", 7),
    ("jul", 7),
    ("agosto", 8),
    ("august", 8),
    ("aug", 8),
    ("ago", 8),
    ("setembro", 9),
    ("september", 9),
    ("sept", 9),
    ("sep", 9),
    ("set", 9),
    ("outubro", 10),
    ("october", 10),
    ("oct", 10),
    ("out", 10),
    ("novembro", 11),
    ("november", 11),
    ("nov", 11),
    ("dezembro", 12),
    ("december", 12),
    ("dec", 12),
    ("dez", 12),
];

const MONTHS_PATTERN: &str = "janeiro|january|jan|fevereiro|february|feb|fev|março|marco|march|mar|abril|april|apr|abr|maio|may|mai|junho|june|jun|julho|july|jul|agosto|august|aug|ago|setembro|september|sept|sep|set|outubro|october|oct|out|novembro|november|nov|dezembro|december|dec|dez";

static ISO_DATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d{4})[-/](\d{2})[-/](\d{2})\b").unwrap());

static DMY_DATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d{1,2})[/.\-](\d{1,2})[/.\-](\d{4})\b").unwrap());

static PT_WRITTEN_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b(\d{{1,2}})[º°]?\s+de\s+({MONTHS_PATTERN})\s+de\s+(\d{{4}})\b"
    ))
    .unwrap()
});

static EN_MDY_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b({MONTHS_PATTERN})\.?\s+(\d{{1,2}})(?:st|nd|rd|th)?,?\s+(\d{{4}})\b"
    ))
    .unwrap()
});

static EN_DMY_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b(\d{{1,2}})(?:st|nd|rd|th)?\s+({MONTHS_PATTERN})\.?,?\s+(\d{{4}})\b"
    ))
    .unwrap()
});

static PT_SAME_MONTH_RANGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)\b(?:de\s+)?(\d{{1,2}})[º°]?\s*(?:a|até|-|–)\s*(\d{{1,2}})[º°]?\s+de\s+({MONTHS_PATTERN})\s+de\s+(\d{{4}})\b"
    ))
    .unwrap()
});

static DATE_RANGE_CONNECTOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:a|até|e|to|until|-|–)\s*$").unwrap()
});

static ISSUE_CONTEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:emitid[oa]\s+em|emiss[ãa]o|expedid[oa]\s+em|expedi[çc][ãa]o|data\s+de\s+emiss[ãa]o|issued\s+on|date\s+of\s+issue|issued)\s*:?\s*$").unwrap()
});

static CITY_LINE_CONTEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|\n)[^\n]{2,50},[ \t]*$").unwrap()
});

static LABELED_WORKLOAD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:carga\s+hor[áa]ria(?:\s+total)?|workload|dura[çc][ãa]o|duration|totalizando|total\s+of|ch)\s*(?:de|:|=)?\s*(\d+(?:[.,]\d+)?)\s*(?:horas?|hours?|hrs?|hs?|h)\b").unwrap()
});

static GENERIC_WORKLOAD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(\d+(?:[.,]\d+)?)\s*(?:horas?|hours?|hrs?|hs?|h)\b").unwrap()
});

static CLOCK_PREFIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:às|as|at)\s+$").unwrap());

static URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://[^\s<>()\[\]{}]+").unwrap());

static VALIDATION_URL_KEYWORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:valid|verif|autentic|certificad|consulta|confer|check|c[óo]digo|code|hash|qr)").unwrap()
});

static VALIDATION_CODE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:c[óo]digo\s+(?:de\s+(?:valida[çc][ãa]o|verifica[çc][ãa]o|autenticidade|registro)|verificador)|chave\s+de\s+(?:valida[çc][ãa]o|autenticidade|acesso)|n[º°]?\s*(?:de\s+)?registro|validation\s+code|verification\s+code|authenticity\s+code|certificate\s+(?:code|id)|auth\s+code)\s*(?::|#|n[º°]?\.?|é|is)?\s*([A-Za-z0-9][A-Za-z0-9._/\-]{3,})").unwrap()
});

struct DateMatch {
    start: usize,
    end: usize,
    date: String,
}

fn month_number(name: &str) -> Option<u16> {
    let lower = name.to_lowercase();
    MONTH_NAMES
        .iter()
        .find(|(key, _)| *key == lower)
        .map(|(_, value)| *value)
}

fn format_valid_date(year: u16, month: u16, day: u16) -> Option<String> {
    let formatted = format!("{year:04}-{month:02}-{day:02}");
    is_valid_date(&formatted).then_some(formatted)
}

fn collect_dates(text: &str) -> Vec<DateMatch> {
    let mut matches = Vec::new();

    for cap in ISO_DATE_RE.captures_iter(text) {
        let full = cap.get(0).unwrap();
        if let (Ok(y), Ok(m), Ok(d)) = (
            cap[1].parse::<u16>(),
            cap[2].parse::<u16>(),
            cap[3].parse::<u16>(),
        ) && let Some(date) = format_valid_date(y, m, d)
        {
            matches.push(DateMatch {
                start: full.start(),
                end: full.end(),
                date,
            });
        }
    }

    for cap in DMY_DATE_RE.captures_iter(text) {
        let full = cap.get(0).unwrap();
        if let (Ok(d), Ok(m), Ok(y)) = (
            cap[1].parse::<u16>(),
            cap[2].parse::<u16>(),
            cap[3].parse::<u16>(),
        ) && let Some(date) = format_valid_date(y, m, d)
        {
            matches.push(DateMatch {
                start: full.start(),
                end: full.end(),
                date,
            });
        }
    }

    for cap in PT_WRITTEN_DATE_RE.captures_iter(text) {
        let full = cap.get(0).unwrap();
        if let (Ok(d), Some(m), Ok(y)) = (
            cap[1].parse::<u16>(),
            month_number(&cap[2]),
            cap[3].parse::<u16>(),
        ) && let Some(date) = format_valid_date(y, m, d)
        {
            matches.push(DateMatch {
                start: full.start(),
                end: full.end(),
                date,
            });
        }
    }

    for cap in EN_MDY_DATE_RE.captures_iter(text) {
        let full = cap.get(0).unwrap();
        if let (Some(m), Ok(d), Ok(y)) = (
            month_number(&cap[1]),
            cap[2].parse::<u16>(),
            cap[3].parse::<u16>(),
        ) && let Some(date) = format_valid_date(y, m, d)
        {
            matches.push(DateMatch {
                start: full.start(),
                end: full.end(),
                date,
            });
        }
    }

    for cap in EN_DMY_DATE_RE.captures_iter(text) {
        let full = cap.get(0).unwrap();
        if let (Ok(d), Some(m), Ok(y)) = (
            cap[1].parse::<u16>(),
            month_number(&cap[2]),
            cap[3].parse::<u16>(),
        ) && let Some(date) = format_valid_date(y, m, d)
        {
            matches.push(DateMatch {
                start: full.start(),
                end: full.end(),
                date,
            });
        }
    }

    matches.sort_by_key(|m| (m.start, m.end));
    let mut deduped: Vec<DateMatch> = Vec::new();
    for item in matches {
        if deduped
            .last()
            .is_some_and(|prev| item.start < prev.end)
        {
            continue;
        }
        deduped.push(item);
    }
    deduped
}

fn extract_dates(text: &str) -> (Option<String>, Option<String>, Option<String>) {
    let dates = collect_dates(text);
    let mut start_date = None;
    let mut end_date = None;
    let mut range_spans: Vec<(usize, usize)> = Vec::new();

    for cap in PT_SAME_MONTH_RANGE_RE.captures_iter(text) {
        let full = cap.get(0).unwrap();
        if let (Ok(d1), Ok(d2), Some(m), Ok(y)) = (
            cap[1].parse::<u16>(),
            cap[2].parse::<u16>(),
            month_number(&cap[3]),
            cap[4].parse::<u16>(),
        ) && let (Some(start), Some(end)) =
            (format_valid_date(y, m, d1), format_valid_date(y, m, d2))
            && start <= end
        {
            start_date = Some(start);
            end_date = Some(end);
            range_spans.push((full.start(), full.end()));
            break;
        }
    }

    if start_date.is_none() {
        for pair in dates.windows(2) {
            let between = &text[pair[0].end..pair[1].start];
            if DATE_RANGE_CONNECTOR_RE.is_match(between) && pair[0].date <= pair[1].date {
                start_date = Some(pair[0].date.clone());
                end_date = Some(pair[1].date.clone());
                range_spans.push((pair[0].start, pair[1].end));
                break;
            }
        }
    }

    let is_in_range = |item: &DateMatch| {
        range_spans
            .iter()
            .any(|(s, e)| item.start >= *s && item.end <= *e)
    };

    let contextual_issue = dates.iter().find(|item| {
        if is_in_range(item) {
            return false;
        }
        let prefix = &text[..item.start];
        ISSUE_CONTEXT_RE.is_match(prefix) || CITY_LINE_CONTEXT_RE.is_match(prefix)
    });

    let issue_date = if let Some(found) = contextual_issue {
        Some(found.date.clone())
    } else {
        dates
            .iter()
            .rev()
            .find(|item| !is_in_range(item))
            .map(|item| item.date.clone())
    };

    (issue_date, start_date, end_date)
}

fn parse_hours(raw: &str) -> Option<f64> {
    let normalized = raw.replace(',', ".");
    let value: f64 = normalized.parse().ok()?;
    (value.is_finite() && value > 0.0).then_some(value)
}

fn extract_workload_hours(text: &str) -> Option<f64> {
    for cap in LABELED_WORKLOAD_RE.captures_iter(text) {
        if let Some(hours) = parse_hours(&cap[1]) {
            return Some(hours);
        }
    }
    for cap in GENERIC_WORKLOAD_RE.captures_iter(text) {
        let full = cap.get(0).unwrap();
        let prefix = &text[..full.start()];
        let suffix = &text[full.end()..];
        if CLOCK_PREFIX_RE.is_match(prefix)
            || suffix.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            continue;
        }
        if let Some(hours) = parse_hours(&cap[1]) {
            return Some(hours);
        }
    }
    None
}

fn extract_validation_url(text: &str) -> Option<String> {
    let mut fallback = None;
    for found in URL_RE.find_iter(text) {
        let cleaned = found
            .as_str()
            .trim_end_matches(['.', ',', ';', ')', ']', '"', '\'']);
        if cleaned.is_empty() {
            continue;
        }
        if fallback.is_none() {
            fallback = Some(cleaned.to_string());
        }
        let context_start = found.start().saturating_sub(60);
        let context = &text[context_start..found.start()];
        if VALIDATION_URL_KEYWORD_RE.is_match(cleaned)
            || VALIDATION_URL_KEYWORD_RE.is_match(context)
        {
            return Some(cleaned.to_string());
        }
    }
    fallback
}

fn extract_validation_code(text: &str) -> Option<String> {
    for cap in VALIDATION_CODE_RE.captures_iter(text) {
        let candidate = cap[1]
            .trim_end_matches(['.', ',', ';', ')', ']', '"', '\'']);
        if candidate.len() >= 4
            && candidate
                .chars()
                .any(|c| c.is_ascii_digit() || c == '-')
        {
            return Some(candidate.to_string());
        }
    }
    None
}

pub(crate) fn extract_with_regex(text: &str) -> RegexDocumentAnalysis {
    let (issue_date, start_date, end_date) = extract_dates(text);
    RegexDocumentAnalysis {
        issue_date,
        start_date,
        end_date,
        workload_hours: extract_workload_hours(text),
        validation_code: extract_validation_code(text),
        validation_url: extract_validation_url(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_portuguese_certificate_fields() {
        let text = "Certificamos que Maria Silva participou do Congresso de Tecnologia, realizado no período de 10 a 12 de novembro de 2025, com carga horária total de 20,5 horas.\nPelotas, 15 de novembro de 2025.\nCódigo de validação: CERT-2025-9981.\nVerifique em https://valida.ifsul.edu.br/autenticar?codigo=CERT-2025-9981.";
        let extracted = extract_with_regex(text);
        assert_eq!(extracted.start_date.as_deref(), Some("2025-11-10"));
        assert_eq!(extracted.end_date.as_deref(), Some("2025-11-12"));
        assert_eq!(extracted.issue_date.as_deref(), Some("2025-11-15"));
        assert_eq!(extracted.workload_hours, Some(20.5));
        assert_eq!(extracted.validation_code.as_deref(), Some("CERT-2025-9981"));
        assert_eq!(
            extracted.validation_url.as_deref(),
            Some("https://valida.ifsul.edu.br/autenticar?codigo=CERT-2025-9981")
        );
    }

    #[test]
    fn extracts_english_and_numeric_dates_and_ranges() {
        let text = "Issued on September 29, 2025. Course held from 2025-09-01 to 2025-09-15. Workload: 40 hours. Verification code: A1B2C3D4.";
        let extracted = extract_with_regex(text);
        assert_eq!(extracted.issue_date.as_deref(), Some("2025-09-29"));
        assert_eq!(extracted.start_date.as_deref(), Some("2025-09-01"));
        assert_eq!(extracted.end_date.as_deref(), Some("2025-09-15"));
        assert_eq!(extracted.workload_hours, Some(40.0));
        assert_eq!(extracted.validation_code.as_deref(), Some("A1B2C3D4"));
    }

    #[test]
    fn extracts_slash_dates_and_single_issue_date() {
        let text = "Emitido em 05/03/2024. Duração: 8h.";
        let extracted = extract_with_regex(text);
        assert_eq!(extracted.issue_date.as_deref(), Some("2024-03-05"));
        assert_eq!(extracted.start_date, None);
        assert_eq!(extracted.end_date, None);
        assert_eq!(extracted.workload_hours, Some(8.0));
    }

    #[test]
    fn ignores_invalid_calendar_dates_and_clock_times() {
        let text = "O evento ocorrerá às 14h30 do dia 31/02/2025. Carga horária: 4 horas.";
        let extracted = extract_with_regex(text);
        assert_eq!(extracted.issue_date, None);
        assert_eq!(extracted.workload_hours, Some(4.0));

        let clock_only = "Palestra às 14h no auditório.";
        let clock_extracted = extract_with_regex(clock_only);
        assert_eq!(clock_extracted.workload_hours, None);
    }

    #[test]
    fn prioritizes_validation_url_over_generic_homepage() {
        let text = "Instituição: https://www.ifsul.edu.br\nAutenticidade em: https://sgc.ifsul.edu.br/validar/12345.";
        let extracted = extract_with_regex(text);
        assert_eq!(
            extracted.validation_url.as_deref(),
            Some("https://sgc.ifsul.edu.br/validar/12345")
        );
    }

    #[test]
    fn falls_back_to_first_url_when_no_validation_keyword_is_present() {
        let text = "More details at https://example.org/certificates/42, thank you.";
        let extracted = extract_with_regex(text);
        assert_eq!(
            extracted.validation_url.as_deref(),
            Some("https://example.org/certificates/42")
        );
    }

    #[test]
    fn ignores_earlier_comma_lines_for_issue_date_and_handles_cross_month_ranges() {
        let text = "Pelotas, RS,\nEvento realizado de 28/11/2025 a 02/12/2025.\nEmitido em 10/12/2025.\nDuração de 12,5h.";
        let extracted = extract_with_regex(text);
        assert_eq!(extracted.start_date.as_deref(), Some("2025-11-28"));
        assert_eq!(extracted.end_date.as_deref(), Some("2025-12-02"));
        assert_eq!(extracted.issue_date.as_deref(), Some("2025-12-10"));
        assert_eq!(extracted.workload_hours, Some(12.5));

        let non_city_comma = "Certificamos que Maria, aluna regular,\nparticipou em 01/03/2025.\nEmitido em 15/03/2025.";
        let extracted_non_city = extract_with_regex(non_city_comma);
        assert_eq!(extracted_non_city.issue_date.as_deref(), Some("2025-03-15"));
    }
}
