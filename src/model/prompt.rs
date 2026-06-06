//! System prompt and message assembly, shared across providers.

pub const SYSTEM: &str = "\
You are tiog, a command-line assistant that lives in the user's terminal.

The user is working in a shell and wants a single shell command to do something — often \
because they forgot the exact flags, or want to act on something they just ran or saw. You \
are given terminal context (OS, shell, current directory, git status, files, and — when \
available — recent commands and their output). Use it to return the best command FOR THIS \
environment.

Guidelines:
- Return ONE primary command. Chain with && or pipes if a task genuinely needs steps.
- Use flags correct for the user's OS — BSD/macOS and GNU tools differ (e.g. `sed -i ''` \
on macOS vs `sed -i` on GNU). Honor the shell shown in the context.
- When the request points at something the user just did (\"that error\", \"the last command\", \"what failed\"), ground the answer in the recent commands and their output shown in the context.
- A \"Recent tiog conversation\" section, if present, lists your recent exchanges this session. Use it for follow-ups: \"teach me more\", \"with examples\", \"again\" refer to the most recent topic — build on the previous answer with NEW detail, and never repeat it verbatim. Only return an empty `command` with `needs` when there is no relevant prior exchange and no target in the request.
- Keep `explanation` to one or two short lines, focused on the key flags the user likely \
forgot. No preamble, no markdown, no backticks.
- Set `risk`: \"destructive\" for irreversible or dangerous actions (rm -rf, dd, force \
push, overwriting files, dropping data); \"caution\" for recoverable state changes; \
otherwise \"none\".
- If the request is too vague to produce a command, return an empty `command` and set \
`needs` to the specific information you require.
- Put only the command itself in `command` — no leading `$`, prompt, or surrounding quotes.";

/// Appended to the system prompt for providers without tool/schema enforcement
/// (OpenAI JSON mode and OpenAI-compatible servers).
pub const JSON_FORMAT: &str = "Respond with ONLY a single JSON object (no prose, no code \
fences) using these keys: \"command\" (string), \"explanation\" (string), \"alternatives\" \
(array of strings, optional), \"risk\" (one of \"none\", \"caution\", \"destructive\"), \
\"needs\" (string, optional — set when no command is possible).";

pub const ROUTER_SYSTEM: &str = "\
You are tiog's request router. Choose the best plugin for the user's request.

Use only the plugin names listed in the plugin catalog. Prefer the command plugin for ambiguous
requests because tiog is primarily a terminal command assistant. Return a confidence from 0.0 to
1.0, a brief reason, and a normalized request for the selected plugin.";

pub const ROUTER_JSON_FORMAT: &str = "Respond with ONLY a single JSON object using these keys: \
\"plugin\" (string), \"confidence\" (number from 0.0 to 1.0), \"reason\" (string), \
\"normalized_request\" (string).";

pub const TEXT_JSON_FORMAT: &str = "Respond with ONLY a single JSON object using these keys: \
\"text\" (string), \"explanation\" (one short line), \"needs\" (string, optional — set when no \
answer is possible).";

pub fn command_system(output_language: &str) -> String {
    append_output_language(SYSTEM, output_language, command_language_instruction)
}

pub fn user_message(question: &str, context: &str) -> String {
    format!("# Terminal context\n{context}\n# Request\n{question}")
}

pub fn router_message(question: &str, catalog: &str) -> String {
    format!("# Plugin catalog\n{catalog}\n# Request\n{question}")
}

pub fn text_plugin_system(plugin_prompt: &str, output_language: &str) -> String {
    let base = format!(
        "{}\n\nReturn the user-facing answer in `text`. Keep `explanation` as brief internal metadata; it is not shown in normal output.",
        plugin_prompt.trim()
    );
    format!(
        "{}\n\n{}",
        append_output_language(&base, output_language, text_language_instruction),
        TEXT_JSON_FORMAT
    )
}

pub fn text_plugin_user_message(plugin_name: &str, question: &str, context: &str) -> String {
    let request = if plugin_name == "interesting" {
        format!(
            "# Freshness\nnonce: {}\nUse this nonce only to vary topic selection; do not mention it.\n# Request\n{question}",
            freshness_nonce()
        )
    } else {
        format!("# Request\n{question}")
    };

    if context.trim().is_empty() {
        request
    } else {
        format!("# Context\n{context}\n{request}")
    }
}

fn append_output_language(
    base: &str,
    output_language: &str,
    instruction: fn(&str) -> String,
) -> String {
    match normalized_output_language(output_language) {
        Some(language) => format!("{}\n\n{}", base.trim(), instruction(language)),
        None => base.trim().to_string(),
    }
}

fn normalized_output_language(output_language: &str) -> Option<&str> {
    let language = output_language.trim();
    if language.is_empty()
        || language.eq_ignore_ascii_case("auto")
        || language.eq_ignore_ascii_case("default")
    {
        None
    } else {
        Some(language)
    }
}

fn command_language_instruction(language: &str) -> String {
    format!(
        "Output language preference: write user-visible prose such as `explanation` and `needs` \
in {language}, unless the user's request explicitly asks for another language. Do not translate, \
localize, or rewrite shell commands, alternative commands, flags, paths, code, JSON keys, or \
quoted literals."
    )
}

fn text_language_instruction(language: &str) -> String {
    format!(
        "Output language preference: write user-visible prose such as `text`, `explanation`, \
and `needs` in {language}, unless the user's request explicitly asks for another language. Do \
not translate code, shell commands, flags, paths, JSON keys, or quoted literals unless the user \
asks for translation."
    )
}

fn freshness_nonce() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos().to_string())
        .unwrap_or_else(|_| "clock-unavailable".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_language_keeps_command_system_unchanged() {
        assert_eq!(command_system("auto"), SYSTEM);
    }

    #[test]
    fn command_system_adds_language_without_translating_commands() {
        let system = command_system("Chinese");

        assert!(system.contains("in Chinese"));
        assert!(system.contains("Do not translate"));
        assert!(system.contains("shell commands"));
    }

    #[test]
    fn text_plugin_system_adds_language_before_json_contract() {
        let system = text_plugin_system("Answer normally.", "Japanese");

        assert!(system.contains("in Japanese"));
        assert!(system.contains(TEXT_JSON_FORMAT));
    }

    #[test]
    fn interesting_user_message_includes_freshness_nonce() {
        let message = text_plugin_user_message("interesting", "I'm bored", "");

        assert!(message.contains("# Freshness"));
        assert!(message.contains("nonce:"));
        assert!(message.contains("# Request\nI'm bored"));
    }

    #[test]
    fn non_interesting_user_message_has_no_freshness_nonce() {
        let message = text_plugin_user_message("translator", "translate hello", "");

        assert!(!message.contains("# Freshness"));
        assert_eq!(message, "# Request\ntranslate hello");
    }
}
