//! Best-effort secret redaction applied to the terminal context before it is sent to a
//! model. Heuristic — known token shapes plus `key = value` style assignments. Not a
//! guarantee; it favors a few false positives over leaking credentials.

use regex::Regex;

const PLACEHOLDER: &str = "[REDACTED]";

pub fn redact(input: &str) -> String {
    let mut out = input.to_string();

    // key/secret/token/password assignments: keep the name, redact the value.
    let assign = Regex::new(
        r"(?i)\b(api[_-]?key|secret|token|password|passwd|pwd|access[_-]?key|auth)\b(\s*[:=]\s*)(\S+)",
    )
    .unwrap();
    let replacement = format!("${{1}}${{2}}{PLACEHOLDER}");
    out = assign.replace_all(&out, replacement.as_str()).into_owned();

    for pattern in [
        r"sk-[A-Za-z0-9_-]{16,}",        // OpenAI / Anthropic style
        r"gh[posru]_[A-Za-z0-9]{20,}",   // GitHub tokens (ghp_/gho_/ghu_/ghs_/ghr_)
        r"github_pat_[A-Za-z0-9_]{20,}", // GitHub fine-grained PAT
        r"glpat-[A-Za-z0-9_-]{16,}",     // GitLab
        r"xox[baprs]-[A-Za-z0-9-]{10,}", // Slack
        r"AKIA[0-9A-Z]{16}",             // AWS access key id
        r"AIza[0-9A-Za-z_\-]{35}",       // Google API key
        r"eyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}", // JWT
        r"(?i)bearer\s+[A-Za-z0-9._\-]{12,}", // bearer tokens
    ] {
        out = Regex::new(pattern)
            .unwrap()
            .replace_all(&out, PLACEHOLDER)
            .into_owned();
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_assignments() {
        assert!(redact("export API_KEY=sk-abcdef0123456789xyz").contains("API_KEY=[REDACTED]"));
        assert!(redact("password: hunter2horse").contains("password: [REDACTED]"));
        assert!(!redact("token=ghp_0123456789abcdefghij0123").contains("ghp_"));
    }

    #[test]
    fn redacts_token_shapes() {
        assert_eq!(
            redact("key is sk-ABCDEFGHIJKLMNOPqrstuvwx here"),
            "key is [REDACTED] here"
        );
        assert!(redact("AKIAIOSFODNN7EXAMPLE").contains(PLACEHOLDER));
    }

    #[test]
    fn leaves_normal_text_alone() {
        let s = "git log shows commit a1b2c3d4 and file Cargo.toml";
        assert_eq!(redact(s), s);
    }
}
