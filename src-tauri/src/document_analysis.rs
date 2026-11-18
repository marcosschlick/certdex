use serde::{Deserialize, Serialize};

pub const DOCUMENT_ANALYSIS_SCHEMA: &str = include_str!("document_analysis.schema.json");

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    Certificate,
    Course,
    Article,
    Proceedings,
    Presentation,
    Event,
    Award,
    Project,
    Internship,
    Monitoring,
    Other,
}

#[derive(Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LlmDocumentAnalysis {
    pub r#type: Option<DocumentType>,
    pub title: Option<String>,
    pub role: Option<String>,
    pub organization: Option<String>,
    pub event: Option<String>,
}

#[derive(Debug, Default, PartialEq)]
pub struct RegexDocumentAnalysis {
    pub issue_date: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub workload_hours: Option<f64>,
    pub validation_code: Option<String>,
    pub validation_url: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DocumentAnalysis {
    pub r#type: Option<DocumentType>,
    pub title: Option<String>,
    pub role: Option<String>,
    pub organization: Option<String>,
    pub event: Option<String>,
    pub issue_date: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub workload_hours: Option<f64>,
    pub city: Option<String>,
    pub country: Option<String>,
    pub validation_code: Option<String>,
    pub validation_url: Option<String>,
}

pub fn combine_analysis(
    regex: RegexDocumentAnalysis,
    llm: LlmDocumentAnalysis,
) -> DocumentAnalysis {
    DocumentAnalysis {
        r#type: llm.r#type,
        title: llm.title,
        role: llm.role,
        organization: llm.organization,
        event: llm.event,
        issue_date: regex.issue_date,
        start_date: regex.start_date,
        end_date: regex.end_date,
        workload_hours: regex.workload_hours,
        city: None,
        country: None,
        validation_code: regex.validation_code,
        validation_url: regex.validation_url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn schema_matches_llm_structure_and_document_types() {
        let schema: Value = serde_json::from_str(DOCUMENT_ANALYSIS_SCHEMA).unwrap();
        let properties = schema["properties"].as_object().unwrap();
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), properties.len());
        assert_eq!(properties.len(), 5);
        for name in properties.keys() {
            assert!(required.contains(&json!(name)));
        }
        let empty: LlmDocumentAnalysis = serde_json::from_value(json!({})).unwrap();
        let serialized = serde_json::to_value(empty).unwrap();
        let fields = serialized.as_object().unwrap();
        assert_eq!(properties.len(), fields.len());
        for (name, value) in fields {
            assert!(value.is_null());
            assert!(
                properties[name]["type"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("null"))
            );
        }
        let types = [
            "certificate",
            "course",
            "article",
            "proceedings",
            "presentation",
            "event",
            "award",
            "project",
            "internship",
            "monitoring",
            "other",
        ];
        let allowed = properties["type"]["enum"].as_array().unwrap();
        assert_eq!(allowed.len(), types.len() + 1);
        assert!(allowed.contains(&Value::Null));
        for name in types {
            assert!(allowed.contains(&json!(name)));
            assert!(serde_json::from_value::<LlmDocumentAnalysis>(json!({"type": name})).is_ok());
        }
        assert!(serde_json::from_value::<LlmDocumentAnalysis>(json!({"type": "invalid"})).is_err());
        assert!(serde_json::from_value::<LlmDocumentAnalysis>(json!({"extra": "value"})).is_err());
        assert!(
            serde_json::from_value::<LlmDocumentAnalysis>(json!({"issue_date": "2025-01-01"}))
                .is_err()
        );
    }

    #[test]
    fn combine_analysis_prioritizes_regex_fields_and_keeps_llm_semantic_fields() {
        let regex = RegexDocumentAnalysis {
            issue_date: Some("2025-09-29".into()),
            start_date: Some("2025-09-20".into()),
            end_date: Some("2025-09-25".into()),
            workload_hours: Some(20.0),
            validation_code: Some("ABC-1234".into()),
            validation_url: Some("https://valida.ifsul.edu.br/check".into()),
        };
        let llm = LlmDocumentAnalysis {
            r#type: Some(DocumentType::Certificate),
            title: Some("Rust Workshop".into()),
            role: Some("Participant".into()),
            organization: Some("IFSul".into()),
            event: Some("Tech Week".into()),
        };
        let combined = combine_analysis(regex, llm);
        assert_eq!(combined.r#type, Some(DocumentType::Certificate));
        assert_eq!(combined.title.as_deref(), Some("Rust Workshop"));
        assert_eq!(combined.role.as_deref(), Some("Participant"));
        assert_eq!(combined.organization.as_deref(), Some("IFSul"));
        assert_eq!(combined.event.as_deref(), Some("Tech Week"));
        assert_eq!(combined.issue_date.as_deref(), Some("2025-09-29"));
        assert_eq!(combined.start_date.as_deref(), Some("2025-09-20"));
        assert_eq!(combined.end_date.as_deref(), Some("2025-09-25"));
        assert_eq!(combined.workload_hours, Some(20.0));
        assert_eq!(combined.validation_code.as_deref(), Some("ABC-1234"));
        assert_eq!(
            combined.validation_url.as_deref(),
            Some("https://valida.ifsul.edu.br/check")
        );
        assert_eq!(combined.city, None);
        assert_eq!(combined.country, None);
    }
}
