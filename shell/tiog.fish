# tiog — fish integration
#
# Install:
#   1. Put the `tiog` binary on your PATH:   cargo install --path .
#   2. Link this file into fish's autoload dir, then open a new shell:
#        ln -s (path resolve shell/tiog.fish) ~/.config/fish/conf.d/tiog.fish
#
# Use:
#   • Hotkey (in-place): type your request on the command line, press Ctrl-G.
#       tiog replaces the line with the suggested command — review, then Enter.
#   • Command form (print / pipe):
#       tiog list files by size            # explanation -> stderr, command -> stdout
#       tiog list files by size | pbcopy   # copy just the command to the clipboard
#
# Rebind the hotkey: change the `bind \cg ...` lines near the bottom.
# Disable command logging for a session:  set -gx TIOG_NO_LOG 1

status is-interactive; or return

# --- session command log -----------------------------------------------------
# Producer for the `hooks` context source (the reader lands in M3). Records one line
# per command — exit code, cwd, and the command — with zero external forks per command.
set -l __tiog_state_dir "$HOME/.local/state/tiog/sessions"
if set -q XDG_STATE_HOME; and test -n "$XDG_STATE_HOME"
    set __tiog_state_dir "$XDG_STATE_HOME/tiog/sessions"
end
mkdir -p $__tiog_state_dir 2>/dev/null
set -gx TIOG_SESSION_LOG "$__tiog_state_dir/session-$fish_pid.log"
# best-effort retention: drop session logs older than 7 days (one fork, at shell start)
find $__tiog_state_dir -name 'session-*.log' -mtime +7 -delete 2>/dev/null

function __tiog_log_cmd --on-event fish_postexec --description 'tiog: record command + exit code'
    set -l code $status # capture exit status FIRST — anything else clobbers $status
    set -q TIOG_NO_LOG; and return
    test -n "$TIOG_SESSION_LOG"; or return
    test -n "$argv[1]"; or return
    set -l line (string replace -a \n ' ' -- $argv[1])
    printf '%d\t%s\t%s\n' $code "$PWD" "$line" >>$TIOG_SESSION_LOG
end

# --- Ctrl-G: turn the current command line into a shell command --------------
function __tiog_hotkey --description 'tiog: suggest a command from the current line'
    set -l query (commandline)
    test -z "$query"; and return

    if not command -q tiog
        echo
        echo "tiog: binary not found on PATH (try: cargo install --path .)"
        commandline -f repaint
        return
    end

    set -l outfile (mktemp)
    set -l errfile (mktemp)
    command tiog --shell -- $query >$outfile 2>$errfile
    set -l rc $status # tiog's own exit code, captured before anything else
    set -l cmd (string collect <$outfile)
    set -l info (cat $errfile)
    command rm -f $outfile $errfile

    echo # drop below the input line
    if test -n "$info"
        set_color brblack
        printf '%s\n' $info
        set_color normal
    end

    # rc: 0 = paste only, 10 = paste and auto-run, anything else = error (keep the query)
    if test $rc -eq 0 -o $rc -eq 10
        if test -n "$cmd"
            commandline -r -- $cmd # paste the command; user reviews, then Enter
            if test $rc -eq 10
                commandline -f execute # auto_run=safe and risk=none: run it
                return
            end
        end
    end
    commandline -f repaint
end

# Install the binding once, at the first prompt (after fish's defaults are in place).
function __tiog_install_binding --on-event fish_prompt --description 'tiog: bind hotkey once'
    functions -e __tiog_install_binding
    bind \cg __tiog_hotkey
    bind -M insert \cg __tiog_hotkey 2>/dev/null # vi insert mode too, if present
end
