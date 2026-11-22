use scraper::{Html, Selector};
use std::path::Path;
use thiserror::Error;

pub const REQUIRED_SUBTABS: [(&str, &[&str]); 5] = [
    ("summary", &["summary"]),
    ("architecture", &["architecture"]),
    (
        "ui-before-after",
        &["ui-before-after", "ui before/after", "before/after"],
    ),
    ("user-stories", &["user-stories", "user stories"]),
    (
        "test-verification",
        &["test-verification", "test verification"],
    ),
];

#[derive(Debug, Error)]
pub enum ChangelogValidationError {
    #[error("Failed to read HTML changelog at {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("Changelog HTML is empty")]
    Empty,
    #[error("Changelog HTML is missing required 5-subtab sections: {missing:?}. Each subtab must declare data-tab=\"<name>\" (summary, architecture, ui-before-after, user-stories, test-verification).")]
    MissingSubtabs { missing: Vec<String> },
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub found_subtabs: Vec<String>,
    pub byte_len: usize,
}

pub fn validate_changelog_file(path: &Path) -> Result<ValidationResult, ChangelogValidationError> {
    let html = std::fs::read_to_string(path).map_err(|source| ChangelogValidationError::Io {
        path: path.display().to_string(),
        source,
    })?;
    validate_changelog_html(&html)
}

pub fn validate_changelog_html(html: &str) -> Result<ValidationResult, ChangelogValidationError> {
    if html.trim().is_empty() {
        return Err(ChangelogValidationError::Empty);
    }

    let document = Html::parse_document(html);

    let data_tab_selector = Selector::parse("[data-tab], [data-tab-btn], [id]").unwrap();
    let heading_selector = Selector::parse("h1, h2, h3, button, [role='tab']").unwrap();

    let mut discovered_tokens: Vec<String> = Vec::new();

    for el in document.select(&data_tab_selector) {
        if let Some(val) = el.value().attr("data-tab") {
            discovered_tokens.push(val.trim().to_lowercase());
        }
        if let Some(val) = el.value().attr("data-tab-btn") {
            discovered_tokens.push(val.trim().to_lowercase());
        }
        if let Some(val) = el.value().attr("id") {
            discovered_tokens.push(val.trim().to_lowercase());
        }
    }

    for el in document.select(&heading_selector) {
        let text = el.text().collect::<String>().trim().to_lowercase();
        if !text.is_empty() {
            discovered_tokens.push(text);
        }
    }

    let mut found = Vec::new();
    let mut missing = Vec::new();

    for (canonical, aliases) in REQUIRED_SUBTABS {
        let matched = aliases.iter().any(|alias| {
            discovered_tokens
                .iter()
                .any(|tok| tok == alias || tok.contains(alias))
        });
        if matched {
            found.push(canonical.to_string());
        } else {
            missing.push(canonical.to_string());
        }
    }

    if !missing.is_empty() {
        return Err(ChangelogValidationError::MissingSubtabs { missing });
    }

    Ok(ValidationResult {
        found_subtabs: found,
        byte_len: html.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_changelog_passes_validation() {
        let html = include_str!("../../templates/sample-changelog.html");
        let res = validate_changelog_html(html).expect("sample changelog should validate");
        assert_eq!(res.found_subtabs.len(), 5);
    }

    #[test]
    fn missing_subtabs_fails_with_clear_error() {
        let html = r#"<html><body><section data-tab="summary">Hi</section></body></html>"#;
        let err = validate_changelog_html(html).unwrap_err();
        match err {
            ChangelogValidationError::MissingSubtabs { missing } => {
                assert_eq!(missing.len(), 4);
                assert!(missing.contains(&"architecture".to_string()));
                assert!(missing.contains(&"test-verification".to_string()));
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}
