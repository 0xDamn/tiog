use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use crate::config::Config;
use crate::output::{AssistantResponse, Suggestion};

#[derive(Parser, Debug)]
#[command(
    name = "tiog",
    version,
    about = "Terminal-native command agent: ask in plain language, get the command."
)]
pub struct Cli {
    /// Your request, in plain language. Args are joined, e.g.
    /// `tiog replace foo with bar in all .txt files`.
    #[arg(trailing_var_arg = true, value_name = "QUERY")]
    pub query: Vec<String>,

    /// Emit the suggestion as JSON (machine-readable; used by shell integration).
    #[arg(long)]
    pub json: bool,

    /// Override the model name from config.
    #[arg(long, value_name = "NAME")]
    pub model: Option<String>,

    /// Use an alternate config file.
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Print the command-plugin terminal context, then exit (no API call).
    #[arg(long)]
    pub show_context: bool,

    /// Do not include terminal context for this request.
    #[arg(long)]
    pub no_context: bool,

    /// List available plugins, then exit (no API call).
    #[arg(long)]
    pub list_plugins: bool,

    /// Choose a plugin explicitly, bypassing model routing.
    #[arg(long, value_name = "NAME")]
    pub plugin: Option<String>,

    /// Shell-integration mode: exit code tells the shell whether to paste, auto-run, or print text.
    #[arg(long)]
    pub shell: bool,
}

impl Cli {
    pub async fn parse_and_run() -> Result<i32> {
        Self::parse().run().await
    }

    async fn run(self) -> Result<i32> {
        let mut cfg = Config::load(self.config.as_deref())?;
        if let Some(name) = self.model {
            cfg.model.name = name;
        }

        if self.list_plugins {
            print_plugins(&cfg);
            return Ok(0);
        }

        if self.show_context {
            let include_context = !self.no_context;
            println!(
                "{}",
                crate::query::gather_context_with(&cfg, include_context, true).trim_end()
            );
            return Ok(0);
        }

        if self.query.is_empty() {
            anyhow::bail!("no request given. Try: tiog how do I list files by size");
        }

        let question = self.query.join(" ");
        let options = crate::query::RunOptions {
            include_context: !self.no_context,
            plugin: self.plugin,
        };
        let response = crate::query::run_with_options(&cfg, &question, options).await?;
        crate::output::render(&response, self.json);
        if !self.json {
            crate::selflog::record(&crate::output::response_lines(&response));
        }

        Ok(if self.shell {
            shell_exit_code(&cfg, &response)
        } else {
            0
        })
    }
}

fn print_plugins(cfg: &Config) {
    for (name, plugin) in cfg.available_plugins() {
        let context = if plugin.include_context {
            "context"
        } else {
            "no-context"
        };
        let conversation = if plugin.include_conversation {
            "conversation"
        } else {
            "no-conversation"
        };
        println!(
            "{name}\toutput={}\t{context}\t{conversation}\t{}",
            plugin.output, plugin.description
        );
    }
}

/// Exit code for command `--shell` mode: 10 = paste and auto-run, 0 = paste only.
/// Auto-run only fires when the user opted in (`behavior.auto_run = safe`) and the reconciled
/// risk is `none`, so anything caution/destructive always falls back to paste-only.
fn command_shell_exit_code(cfg: &Config, s: &Suggestion) -> i32 {
    let auto_run =
        cfg.behavior.auto_run == "safe" && s.risk == "none" && !s.command.trim().is_empty();
    if auto_run {
        10
    } else {
        0
    }
}

fn shell_exit_code(cfg: &Config, response: &AssistantResponse) -> i32 {
    match response {
        AssistantResponse::Command(s) => command_shell_exit_code(cfg, s),
        AssistantResponse::Text(_) => 20,
    }
}
