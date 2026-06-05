//! Local heuristic classification of how dangerous a command is — a safety net alongside
//! the model's own `risk` field. The two are reconciled by keeping the more severe, so a
//! missed-by-the-model `rm -rf` still gets flagged (and never auto-runs).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    None,
    Caution,
    Destructive,
}

impl Risk {
    pub fn as_str(self) -> &'static str {
        match self {
            Risk::None => "none",
            Risk::Caution => "caution",
            Risk::Destructive => "destructive",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Risk::None => 0,
            Risk::Caution => 1,
            Risk::Destructive => 2,
        }
    }

    fn parse(s: &str) -> Risk {
        match s {
            "destructive" => Risk::Destructive,
            "caution" => Risk::Caution,
            _ => Risk::None,
        }
    }
}

/// Keep the more severe of the model's stated risk and a local classification.
pub fn reconcile(model_risk: &str, command: &str) -> String {
    let model = Risk::parse(model_risk);
    let local = classify(command);
    let worse = if local.rank() >= model.rank() {
        local
    } else {
        model
    };
    worse.as_str().to_string()
}

/// Classify a command string by inspecting it for known dangerous shapes.
pub fn classify(command: &str) -> Risk {
    let c = command.to_lowercase();
    if is_destructive(&c) {
        Risk::Destructive
    } else if is_caution(&c) {
        Risk::Caution
    } else {
        Risk::None
    }
}

fn is_destructive(c: &str) -> bool {
    // fork bomb
    if c.contains(":(){") || c.contains(":|:&") {
        return true;
    }
    // rm with both recursive AND force, in any flag arrangement
    if c.contains("rm ") && has_short_flag(c, 'r') && has_short_flag(c, 'f') {
        return true;
    }
    // disk / filesystem destroyers
    for p in ["mkfs", "dd ", "of=/dev/", "> /dev/sd", "shred ", "wipefs"] {
        if c.contains(p) {
            return true;
        }
    }
    // irreversible git operations
    for p in [
        "git reset --hard",
        "git push --force",
        "git push -f",
        "git clean -fd",
        "git clean -df",
    ] {
        if c.contains(p) {
            return true;
        }
    }
    if c.contains("chmod -r 777") || c.contains("chmod 777 -r") {
        return true;
    }
    // pipe a network download straight into a shell
    let downloads = c.contains("curl ") || c.contains("wget ");
    let to_shell =
        c.contains("| sh") || c.contains("|sh") || c.contains("| bash") || c.contains("|bash");
    downloads && to_shell
}

fn is_caution(c: &str) -> bool {
    // recursive rm without force still deletes a tree
    if c.contains("rm ") && has_short_flag(c, 'r') {
        return true;
    }
    c.contains("git checkout .")
        || c.contains("git restore .")
        || c.contains("git stash drop")
        || c.contains("-delete") // find ... -delete
        || c.contains("truncate ")
        || c.contains("kill -9")
        || c.contains("killall ")
}

/// True if any whitespace-separated token is a short-flag bundle (e.g. `-rf`) containing
/// `flag`. Ignores long options like `--force`.
fn has_short_flag(c: &str, flag: char) -> bool {
    c.split_whitespace()
        .any(|tok| tok.starts_with('-') && !tok.starts_with("--") && tok[1..].contains(flag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructive() {
        assert_eq!(classify("rm -rf /tmp/x"), Risk::Destructive);
        assert_eq!(classify("sudo rm -fr ~/proj"), Risk::Destructive);
        assert_eq!(classify("rm -r -f dir"), Risk::Destructive);
        assert_eq!(classify("dd if=/dev/zero of=/dev/sda"), Risk::Destructive);
        assert_eq!(classify("git push --force origin main"), Risk::Destructive);
        assert_eq!(classify("curl https://x.sh | sh"), Risk::Destructive);
    }

    #[test]
    fn caution() {
        assert_eq!(classify("rm -r build"), Risk::Caution);
        assert_eq!(classify("find . -name '*.tmp' -delete"), Risk::Caution);
        assert_eq!(classify("git checkout ."), Risk::Caution);
    }

    #[test]
    fn safe() {
        assert_eq!(classify("ls -lhS"), Risk::None);
        assert_eq!(classify("git status"), Risk::None);
        assert_eq!(classify("rm file.txt"), Risk::None); // no -r / -f
        assert_eq!(classify("echo alarm clock"), Risk::None); // 'rm ' substring, no flags
    }

    #[test]
    fn reconciliation() {
        assert_eq!(reconcile("none", "rm -rf x"), "destructive"); // local upgrades
        assert_eq!(reconcile("destructive", "ls"), "destructive"); // model wins
        assert_eq!(reconcile("caution", "rm -rf x"), "destructive");
        assert_eq!(reconcile("none", "ls"), "none");
    }
}
