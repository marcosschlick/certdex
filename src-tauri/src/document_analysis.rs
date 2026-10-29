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
pub struct DocumentAnalysis {
    pub r#type: Option<DocumentType>,
    pub title: Option<String>,
    pub role: Option<String>,
    pub issue_date: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub workload_hours: Option<f64>,
    pub city: Option<String>,
    pub country: Option<String>,
    pub validation_code: Option<String>,
    pub validation_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn schema_matches_structure_and_document_types() {
        let schema: Value = serde_json::from_str(DOCUMENT_ANALYSIS_SCHEMA).unwrap();
        let properties = schema["properties"].as_object().unwrap();
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), properties.len());
        for name in properties.keys() {
            assert!(required.contains(&json!(name)));
        }
        let empty: DocumentAnalysis = serde_json::from_value(json!({})).unwrap();
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
            assert!(serde_json::from_value::<DocumentAnalysis>(json!({"type": name})).is_ok());
        }
        assert!(serde_json::from_value::<DocumentAnalysis>(json!({"type": "invalid"})).is_err());
        assert!(serde_json::from_value::<DocumentAnalysis>(json!({"extra": "value"})).is_err());
        assert!(serde_json::from_value::<DocumentAnalysis>(json!({"workload_hours": 1.5})).is_ok());
        assert!(
            serde_json::from_value::<DocumentAnalysis>(json!({"workload_hours": "1.5"})).is_err()
        );
    }
}
