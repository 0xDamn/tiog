//! Best-effort cache for model routing decisions.
//!
//! The cache stores a deterministic key and the chosen plugin name, not the raw prompt.

use serde::{Deserialize, Serialize};
use std::io::Write as _;
use std::path::PathBuf;

use crate::config::Config;

const KEEP: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedRoute {
    pub plugin: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    key: String,
    plugin: String,
}

pub fn key(cfg: &Config, question: &str, catalog: &str, default_plugin: &str) -> String {
    let mut scope = String::new();
    scope.push_str("route-cache-v1\n");
    scope.push_str("provider=");
    scope.push_str(&cfg.model.provider);
    scope.push('\n');
    scope.push_str("model=");
    scope.push_str(&cfg.model.name);
    scope.push('\n');
    scope.push_str("default=");
    scope.push_str(default_plugin);
    scope.push('\n');
    scope.push_str("min_confidence=");
    scope.push_str(&cfg.router.min_confidence.to_string());
    scope.push('\n');
    scope.push_str("catalog=\n");
    scope.push_str(catalog);
    scope.push_str("plugins=\n");
    for (name, plugin) in cfg.available_plugins() {
        scope.push_str(&name);
        scope.push('\t');
        scope.push_str(&plugin.description);
        scope.push('\t');
        scope.push_str(&plugin.output);
        scope.push('\t');
        scope.push_str(if plugin.include_context {
            "ctx=1"
        } else {
            "ctx=0"
        });
        scope.push('\t');
        scope.push_str(if plugin.include_conversation {
            "conv=1"
        } else {
            "conv=0"
        });
        scope.push('\n');
    }
    scope.push_str("question=\n");
    scope.push_str(question);
    stable_hash(&scope)
}

pub fn get(key: &str) -> Option<CachedRoute> {
    entries()
        .into_iter()
        .rev()
        .find(|entry| entry.key == key)
        .map(|entry| CachedRoute {
            plugin: entry.plugin,
        })
}

pub fn put(key: &str, route: &CachedRoute) {
    if route.plugin.trim().is_empty() {
        return;
    }
    let Some(p) = path() else {
        return;
    };
    let mut entries: Vec<Entry> = entries()
        .into_iter()
        .filter(|entry| entry.key != key)
        .collect();
    entries.push(Entry {
        key: key.to_string(),
        plugin: route.plugin.clone(),
    });
    let start = entries.len().saturating_sub(KEEP);
    if let Some(dir) = p.parent() {
        let _ = crate::statefile::create_private_dir(dir);
    }
    if let Ok(mut f) = crate::statefile::create_private(&p) {
        for entry in &entries[start..] {
            if let Ok(line) = serde_json::to_string(entry) {
                let _ = writeln!(f, "{line}");
            }
        }
    }
}

fn entries() -> Vec<Entry> {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| {
            s.lines()
                .filter_map(|line| serde_json::from_str::<Entry>(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn path() -> Option<PathBuf> {
    crate::statefile::state_dir().map(|base| base.join("tiog/route-cache.jsonl"))
}

fn stable_hash(input: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Plugin};

    #[test]
    fn key_changes_with_question() {
        let cfg = Config::default();
        let catalog = "- command: command (output: command)\n";

        assert_ne!(
            key(&cfg, "list files", catalog, "command"),
            key(&cfg, "show disk space", catalog, "command")
        );
    }

    #[test]
    fn key_changes_with_plugin_policy() {
        let mut cfg = Config::default();
        let catalog = "- formatter: Format text. (output: text)\n";
        let before = key(&cfg, "format this", catalog, "command");
        cfg.plugins.insert(
            "formatter".into(),
            Plugin {
                description: "Format text.".into(),
                output: "text".into(),
                include_context: false,
                include_conversation: false,
                system_prompt: "Format.".into(),
                builtin: false,
            },
        );

        let after = key(&cfg, "format this", catalog, "command");

        assert_ne!(before, after);
    }

    #[test]
    fn stable_hash_is_deterministic() {
        assert_eq!(stable_hash("same input"), stable_hash("same input"));
        assert_ne!(stable_hash("same input"), stable_hash("different input"));
    }
}
