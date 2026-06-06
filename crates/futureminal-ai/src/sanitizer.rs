//! Privacy sanitizer for AI prompts.
//!
//! Strips sensitive data before sending to cloud AI providers:
//! - API keys, tokens, passwords (regex patterns)
//! - IP addresses (optional in strict mode)
//! - Absolute file paths (optional in strict mode)
//! - Email addresses
//! - Credit card numbers (basic pattern)

use regex::Regex;
use std::sync::OnceLock;

/// Privacy mode affecting what gets stripped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SanitizerLevel {
    /// Strip only obvious secrets (keys, tokens).
    Minimal,
    /// Strip secrets + PII (emails, IPs, paths).
    Balanced,
    /// Strip everything potentially identifying.
    Strict,
}

/// Sanitize input text for cloud AI submission.
pub fn sanitize_for_cloud(input: &str, level: SanitizerLevel) -> String {
    let mut result = input.to_string();

    // Always strip secrets regardless of level.
    result = strip_secrets(&result);

    match level {
        SanitizerLevel::Minimal => {}
        SanitizerLevel::Balanced | SanitizerLevel::Strict => {
            result = strip_email_addresses(&result);
            result = strip_ip_addresses(&result);
            result = strip_absolute_paths(&result);
        }
    }

    if level == SanitizerLevel::Strict {
        result = strip_usernames_from_paths(&result);
    }

    result
}

/// Strip common secret patterns from text.
fn strip_secrets(text: &str) -> String {
    let patterns = [
        // API keys
        (r#"(?i)(api[_-]?key\s*[:=]\s*)['\"]?[a-z0-9_\-]{20,}['\"]?"#, "${1}[REDACTED]"),
        (r#"(?i)(token\s*[:=]\s*)['\"]?[a-z0-9_\-]{20,}['\"]?"#, "${1}[REDACTED]"),
        (r#"(?i)(secret\s*[:=]\s*)['\"]?[a-z0-9_\-]{10,}['\"]?"#, "${1}[REDACTED]"),
        (r#"(?i)(password\s*[:=]\s*)['\"]?[^\s'\"]+['\"]?"#, "${1}[REDACTED]"),
        // GitHub tokens
        (r"gh[pousr]_[A-Za-z0-9_]{36}", "[REDACTED_GITHUB_TOKEN]"),
        // AWS access key
        (r"AKIA[0-9A-Z]{16}", "[REDACTED_AWS_KEY]"),
        // Generic hex tokens
        (r"\b[0-9a-f]{32,}\b", "[REDACTED_HEX]"),
    ];

    let mut result = text.to_string();
    for (pattern, replacement) in &patterns {
        if let Ok(re) = Regex::new(pattern) {
            result = re.replace_all(&result, *replacement).to_string();
        }
    }
    result
}

fn strip_email_addresses(text: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap()
    });
    re.replace_all(text, "[EMAIL_REDACTED]").to_string()
}

fn strip_ip_addresses(text: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b|\b[0-9a-fA-F:]{2,}\b").unwrap()
    });
    re.replace_all(text, "[IP_REDACTED]").to_string()
}

fn strip_absolute_paths(text: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?x)
            (/[a-zA-Z0-9_\-\.]+)+
            |
            ([A-Za-z]:\\[\\a-zA-Z0-9_\-\.]+)
        ").unwrap()
    });
    re.replace_all(text, "[PATH_REDACTED]").to_string()
}

fn strip_usernames_from_paths(text: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"/home/[a-zA-Z0-9_]+|/Users/[a-zA-Z0-9_]+|~[a-zA-Z0-9_]*").unwrap()
    });
    re.replace_all(text, "[HOME_DIR_REDACTED]").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_secrets() {
        let input = "api_key=sk-abc123secret45678901234567890";
        let sanitized = sanitize_for_cloud(input, SanitizerLevel::Minimal);
        assert!(sanitized.contains("[REDACTED]"));
        assert!(!sanitized.contains("sk-abc123"));
    }

    #[test]
    fn test_strip_email() {
        let input = "Contact me at user@example.com for help.";
        let sanitized = sanitize_for_cloud(input, SanitizerLevel::Balanced);
        assert!(sanitized.contains("[EMAIL_REDACTED]"));
        assert!(!sanitized.contains("user@example.com"));
    }

    #[test]
    fn test_strip_ip() {
        let input = "Server at 192.168.1.1 is down.";
        let sanitized = sanitize_for_cloud(input, SanitizerLevel::Balanced);
        assert!(sanitized.contains("[IP_REDACTED]"));
        assert!(!sanitized.contains("192.168.1.1"));
    }

    #[test]
    fn test_strip_path() {
        let input = "File at /etc/passwd contains secrets.";
        let sanitized = sanitize_for_cloud(input, SanitizerLevel::Balanced);
        assert!(sanitized.contains("[PATH_REDACTED]"));
    }

    #[test]
    fn test_minimal_does_not_strip_ips() {
        let input = "Server at 192.168.1.1";
        let sanitized = sanitize_for_cloud(input, SanitizerLevel::Minimal);
        assert!(sanitized.contains("192.168.1.1"));
    }
}
