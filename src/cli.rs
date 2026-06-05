use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use crate::config::Config;
use crate::output::Suggestion;

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

    /// Print the terminal context tiog would send to the model, then exit (no API call).
    #[arg(long)]
    pub show_context: bool,

    /// Shell-integration mode: same output, but the exit code tells the shell whether to
    /// auto-run (10) or only paste (0). Used by the fish hotkey.
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

        if self.show_context {
            println!("{}", crate::query::gather_context(&cfg).trim_end());
            return Ok(0);
        }

        if self.query.is_empty() {
            anyhow::bail!("no request given. Try: tiog how do I list files by size");
        }

        let question = self.query.join(" ");
        let suggestion = crate::query::run(&cfg, &question).await?;
        crate::output::render(&suggestion, self.json);
        if !self.json {
            crate::selflog::record(&crate::output::suggestion_lines(&suggestion));
        }

        Ok(if self.shell {
            shell_exit_code(&cfg, &suggestion)
        } else {
            0
        })
    }
}

/// Exit code for `--shell` mode: 10 = paste and auto-run, 0 = paste only. Auto-run only
/// fires when the user opted in (`behavior.auto_run = safe`) and the reconciled risk is
/// `none`, so anything caution/destructive always falls back to paste-only.
fn shell_exit_code(cfg: &Config, s: &Suggestion) -> i32 {
    let auto_run =
        cfg.behavior.auto_run == "safe" && s.risk == "none" && !s.command.trim().is_empty();
    if auto_run {
        10
    } else {
        0
    }
}
