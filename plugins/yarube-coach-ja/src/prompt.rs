use crate::{expand_tilde, DEFAULT_EN_PROMPT_TOML, DEFAULT_USER_PROMPT_EN};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct PromptTemplate {
    pub system: String,
    #[serde(default)]
    pub examples: Vec<ExamplePair>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ExamplePair {
    pub user: String,
    pub assistant: String,
}

impl PromptTemplate {
    pub fn render(&self, en_user_memo: &str) -> String {
        let sys_trimmed = self.system.trim();
        let sys_with_no_think = if sys_trimmed.ends_with("/no_think") {
            sys_trimmed.to_string()
        } else {
            format!("{} /no_think", sys_trimmed)
        };

        let mut out = format!("<|im_start|>system\n{}<|im_end|>\n", sys_with_no_think);
        for ex in &self.examples {
            out.push_str(&format!(
                "<|im_start|>user\n{}<|im_end|>\n\
                 <|im_start|>assistant\n<think>\n\n</think>\n\n{}<|im_end|>\n",
                ex.user.trim(),
                ex.assistant.trim()
            ));
        }
        out.push_str(&format!(
            "<|im_start|>user\n{}<|im_end|>\n\
             <|im_start|>assistant\n<think>\n\n</think>\n\n",
            en_user_memo.trim()
        ));
        out
    }
}

pub fn resolve_external_prompt_path(configured_prompt_path: Option<&str>) -> Option<PathBuf> {
    if let Some(cfg_path) = configured_prompt_path {
        let trimmed = cfg_path.trim();
        if !trimmed.is_empty() {
            let path = expand_tilde(trimmed);
            if path.is_file() {
                return Some(path);
            }
            return None;
        }
    }

    let default_path = expand_tilde(DEFAULT_USER_PROMPT_EN);
    if default_path.is_file() {
        return Some(default_path);
    }

    None
}

pub fn parse_prompt_or_fallback(toml_str: &str) -> PromptTemplate {
    if let Ok(template) = toml::from_str::<PromptTemplate>(toml_str) {
        template
    } else {
        toml::from_str::<PromptTemplate>(DEFAULT_EN_PROMPT_TOML)
            .expect("embedded default_en.toml must be valid")
    }
}

pub fn load_prompt_template(configured_prompt_path: Option<&str>) -> PromptTemplate {
    if let Some(path) = resolve_external_prompt_path(configured_prompt_path) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(template) = toml::from_str::<PromptTemplate>(&content) {
                return template;
            }
        }
    }
    parse_prompt_or_fallback("")
}

pub fn build_en_coaching_prompt(
    en_user_memo: &str,
    configured_prompt_path: Option<&str>,
) -> String {
    let template = load_prompt_template(configured_prompt_path);
    template.render(en_user_memo)
}
