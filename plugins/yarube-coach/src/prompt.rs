use crate::{
    expand_tilde, is_english_lang, DEFAULT_EN_PROMPT_TOML, DEFAULT_JA_PROMPT_TOML,
    DEFAULT_USER_PROMPT_EN, DEFAULT_USER_PROMPT_JA,
};
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
    pub fn render(&self, added_text: &str) -> String {
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
            added_text.trim()
        ));
        out
    }
}

pub fn resolve_external_prompt_path(
    configured_prompt_path: Option<&str>,
    is_en: bool,
) -> Option<PathBuf> {
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

    if is_en {
        let en_path = expand_tilde(DEFAULT_USER_PROMPT_EN);
        if en_path.is_file() {
            return Some(en_path);
        }
        let ja_path = expand_tilde(DEFAULT_USER_PROMPT_JA);
        if ja_path.is_file() {
            return Some(ja_path);
        }
    } else {
        let ja_path = expand_tilde(DEFAULT_USER_PROMPT_JA);
        if ja_path.is_file() {
            return Some(ja_path);
        }
    }

    None
}

pub fn parse_prompt_or_fallback(toml_str: &str, is_en: bool) -> PromptTemplate {
    if let Ok(template) = toml::from_str::<PromptTemplate>(toml_str) {
        template
    } else {
        let default_str = if is_en {
            DEFAULT_EN_PROMPT_TOML
        } else {
            DEFAULT_JA_PROMPT_TOML
        };
        toml::from_str::<PromptTemplate>(default_str)
            .expect("embedded default prompt toml must be valid")
    }
}

pub fn load_prompt_template(configured_prompt_path: Option<&str>, is_en: bool) -> PromptTemplate {
    if let Some(path) = resolve_external_prompt_path(configured_prompt_path, is_en) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(template) = toml::from_str::<PromptTemplate>(&content) {
                return template;
            }
        }
    }
    parse_prompt_or_fallback("", is_en)
}

pub fn build_prompt(added_text: &str, is_en: bool, configured_prompt_path: Option<&str>) -> String {
    let template = load_prompt_template(configured_prompt_path, is_en);
    template.render(added_text)
}

pub fn build_prompt_with_env(added_text: &str, configured_prompt_path: Option<&str>) -> String {
    let lang_env = std::env::var("YARUBE_LANG").ok();
    let is_en = is_english_lang(lang_env.as_deref());
    build_prompt(added_text, is_en, configured_prompt_path)
}
