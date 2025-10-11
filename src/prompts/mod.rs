use include_dir::{include_dir, Dir};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

static PROMPTS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/prompts");

#[derive(Debug, Deserialize, Clone)]
pub struct PromptConfig {
    pub name: String,
    pub description: String,
    pub model: String,
    pub system_context: String,
    pub prompt: PromptText,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PromptText {
    pub text: String,
}

static PROMPTS: OnceLock<HashMap<String, PromptConfig>> = OnceLock::new();

/// Initialize and return the prompts HashMap
pub fn prompts() -> &'static HashMap<String, PromptConfig> {
    PROMPTS.get_or_init(|| {
        let mut map = HashMap::new();

        for file in PROMPTS_DIR.files() {
            if let Some(extension) = file.path().extension() {
                if extension == "toml" {
                    if let Some(contents) = file.contents_utf8() {
                        match toml::from_str::<PromptConfig>(contents) {
                            Ok(config) => {
                                // Get filename without extension as key
                                let key = file
                                    .path()
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("unknown")
                                    .to_string();

                                map.insert(key, config);
                            }
                            Err(e) => {
                                eprintln!(
                                    "Failed to parse prompt file {:?}: {}",
                                    file.path(),
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }

        map
    })
}

/// Get a specific prompt by name
pub fn get_prompt(name: &str) -> Option<&'static PromptConfig> {
    prompts().get(name)
}

/// List all available prompt names
pub fn list_prompt_names() -> Vec<String> {
    prompts().keys().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompts_load() {
        let prompts = prompts();
        assert!(!prompts.is_empty(), "Should load at least one prompt");
    }

    #[test]
    fn test_get_prompt() {
        // This will only pass if the example prompts exist
        if let Some(prompt) = get_prompt("reading_comprehension") {
            assert_eq!(prompt.name, "reading_comprehension");
            assert!(!prompt.prompt.text.is_empty());
        }
    }

    #[test]
    fn test_list_prompt_names() {
        let names = list_prompt_names();
        assert!(!names.is_empty(), "Should have at least one prompt name");
    }
}
