use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetPlatform {
    Freebuff,
    Agy,
    Both,
}

impl std::fmt::Display for TargetPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetPlatform::Freebuff => write!(f, "Freebuff"),
            TargetPlatform::Agy => write!(f, "AGY (Antigravity)"),
            TargetPlatform::Both => write!(f, "Both (Freebuff + AGY)"),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OpenCodePart {
    pub part_id: String,
    pub message_id: String,
    pub part_type: String,
    pub text: String,
    pub raw_json: serde_json::Value,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OpenCodeMessage {
    pub message_id: String,
    pub session_id: String,
    pub role: String,
    pub time_created: i64,
    pub raw_json: serde_json::Value,
    pub parts: Vec<OpenCodePart>,
}

impl OpenCodeMessage {
    pub fn get_full_text(&self) -> String {
        let texts: Vec<String> = self
            .parts
            .iter()
            .filter(|p| !p.text.is_empty())
            .map(|p| p.text.clone())
            .collect();
        texts.join("\n\n")
    }

    pub fn get_reasoning_text(&self) -> String {
        let texts: Vec<String> = self
            .parts
            .iter()
            .filter(|p| p.part_type == "reasoning" && !p.text.is_empty())
            .map(|p| p.text.clone())
            .collect();
        texts.join("\n\n")
    }

    pub fn get_visible_text(&self) -> String {
        let mut texts = Vec::new();
        for p in &self.parts {
            if p.part_type == "text" && !p.text.is_empty() {
                texts.push(p.text.clone());
            } else if p.part_type == "tool" {
                let tool_name = p
                    .raw_json
                    .get("tool")
                    .and_then(|t| t.as_str())
                    .unwrap_or("tool");
                texts.push(format!("[Tool: {}]", tool_name));
            }
        }
        texts.join("\n\n")
    }
}

#[derive(Debug, Clone)]
pub struct OpenCodeSession {
    pub session_id: String,
    pub title: String,
    pub directory: String,
    pub time_created: i64,
    pub time_updated: i64,
    pub model: String,
    pub message_count: usize,
    pub messages: Vec<OpenCodeMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebuffPart {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub text: String,
}
