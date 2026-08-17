use std::collections::HashSet;

use serde_json::Value;

#[derive(Debug, Default)]
pub(crate) struct SchemaIndex {
    object_keys: HashSet<String>,
    string_values: HashSet<String>,
}

impl SchemaIndex {
    pub(crate) fn from_documents(documents: &[Value]) -> Self {
        let mut index = Self::default();

        for document in documents {
            visit(document, &mut index);
        }

        index
    }

    pub(crate) fn contains_exact(&self, token: &str) -> bool {
        self.object_keys.contains(token) || self.string_values.contains(token)
    }
}

fn visit(value: &Value, index: &mut SchemaIndex) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                index.object_keys.insert(key.clone());
                visit(value, index);
            }
        }
        Value::Array(values) => {
            for value in values {
                visit(value, index);
            }
        }
        Value::String(value) => {
            index.string_values.insert(value.clone());
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::SchemaIndex;

    #[test]
    fn indexes_exact_methods_and_fields() {
        let index = SchemaIndex::from_documents(&[json!({
            "properties": { "usedPercent": {} },
            "enum": ["account/read"]
        })]);

        assert!(index.contains_exact("account/read"));
        assert!(index.contains_exact("usedPercent"));
    }

    #[test]
    fn does_not_match_substrings() {
        let index = SchemaIndex::from_documents(&[json!({
            "enum": ["account/read-old"],
            "properties": { "usedPercentExtra": {} }
        })]);

        assert!(!index.contains_exact("account/read"));
        assert!(!index.contains_exact("usedPercent"));
    }
}
