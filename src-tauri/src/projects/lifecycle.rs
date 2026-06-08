use std::time::Duration;

use serde::Serialize;
use specta::Type;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Starting,
    Running,
    RunningNoPort,
    Waiting,
    Exited,
    Crashed,
}

/// Exit signal derived from `SpawnedProcess::try_status`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitState {
    Alive,
    Exited(i32),
    /// Terminated by a signal (no exit code), for example our own kill-on-quit.
    Signaled,
}

/// Pure classifier. `elapsed` is time since spawn; `grace` is the no-port grace
/// window; `holds_port` is whether any listening port is owned by this group.
pub fn classify(
    elapsed: Duration,
    grace: Duration,
    exit: ExitState,
    holds_port: bool,
    idle: Duration,
    last_line_is_prompt: bool,
) -> Lifecycle {
    match exit {
        ExitState::Exited(0) | ExitState::Signaled => Lifecycle::Exited,
        ExitState::Exited(_) => Lifecycle::Crashed,
        ExitState::Alive => {
            if idle >= WAITING_IDLE && last_line_is_prompt {
                Lifecycle::Waiting
            } else if holds_port {
                Lifecycle::Running
            } else if elapsed >= grace {
                Lifecycle::RunningNoPort
            } else {
                Lifecycle::Starting
            }
        }
    }
}

pub const WAITING_IDLE: Duration = Duration::from_secs(3);

pub fn line_looks_like_prompt(line: &str) -> bool {
    let stripped = strip_ansi(line);
    let trimmed = stripped.trim();
    let lower = trimmed.to_ascii_lowercase();
    trimmed.ends_with('?')
        || trimmed.ends_with(':')
        || trimmed.ends_with('>')
        || lower.contains("(y/n)")
        || lower.contains("[y/n]")
}

/// Strip ANSI escape sequences (CSI + OSC) so prompt detection sees the plain
/// text. A colored prompt like `\x1b[36mPassword:\x1b[0m` would otherwise end in
/// `m` and never match. Cheap char scan; only SGR resets/colors and OSC matter
/// here, so we drop CSI (`ESC [ … final`) and OSC (`ESC ] … BEL/ST`) runs.
fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('[') => {
                chars.next();
                // Consume parameter/intermediate bytes up to and including the
                // final byte in the 0x40..=0x7e range.
                for nc in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&nc) {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                // Consume until BEL or the ST sequence (ESC \).
                while let Some(nc) = chars.next() {
                    if nc == '\u{07}' {
                        break;
                    }
                    if nc == '\u{1b}' {
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const GRACE: Duration = Duration::from_secs(10);

    #[test]
    fn alive_before_grace_without_port_is_starting() {
        let l = classify(
            Duration::from_secs(2),
            GRACE,
            ExitState::Alive,
            false,
            Duration::ZERO,
            false,
        );
        assert_eq!(l, Lifecycle::Starting);
    }

    #[test]
    fn alive_with_port_is_running() {
        let l = classify(
            Duration::from_secs(1),
            GRACE,
            ExitState::Alive,
            true,
            Duration::ZERO,
            false,
        );
        assert_eq!(l, Lifecycle::Running);
    }

    #[test]
    fn alive_after_grace_without_port_is_running_no_port() {
        let l = classify(
            Duration::from_secs(11),
            GRACE,
            ExitState::Alive,
            false,
            Duration::ZERO,
            false,
        );
        assert_eq!(l, Lifecycle::RunningNoPort);
    }

    #[test]
    fn clean_exit_is_exited() {
        let l = classify(
            Duration::from_secs(1),
            GRACE,
            ExitState::Exited(0),
            false,
            Duration::from_secs(5),
            true,
        );
        assert_eq!(l, Lifecycle::Exited);
    }

    #[test]
    fn nonzero_exit_is_crashed() {
        let l = classify(
            Duration::from_secs(1),
            GRACE,
            ExitState::Exited(7),
            false,
            Duration::from_secs(5),
            true,
        );
        assert_eq!(l, Lifecycle::Crashed);
    }

    #[test]
    fn signal_termination_is_exited_not_crashed() {
        let l = classify(
            Duration::from_secs(1),
            GRACE,
            ExitState::Signaled,
            false,
            Duration::from_secs(5),
            true,
        );
        assert_eq!(l, Lifecycle::Exited);
    }

    #[test]
    fn idle_prompt_with_port_is_waiting() {
        let l = classify(
            Duration::from_secs(20),
            GRACE,
            ExitState::Alive,
            true,
            Duration::from_secs(5),
            true,
        );
        assert_eq!(l, Lifecycle::Waiting);
    }

    #[test]
    fn idle_non_prompt_with_port_stays_running() {
        let l = classify(
            Duration::from_secs(20),
            GRACE,
            ExitState::Alive,
            true,
            Duration::from_secs(5),
            false,
        );
        assert_eq!(l, Lifecycle::Running);
    }

    #[test]
    fn active_prompt_is_never_waiting() {
        let l = classify(
            Duration::from_secs(20),
            GRACE,
            ExitState::Alive,
            true,
            Duration::from_secs(1),
            true,
        );
        assert_eq!(l, Lifecycle::Running);
    }

    #[test]
    fn idle_prompt_without_port_after_grace_is_waiting_not_error() {
        let l = classify(
            Duration::from_secs(20),
            GRACE,
            ExitState::Alive,
            false,
            Duration::from_secs(5),
            true,
        );
        assert_eq!(l, Lifecycle::Waiting);
    }

    #[test]
    fn prompt_detection_matches_question_colon_angle_and_yn() {
        assert!(line_looks_like_prompt("Continue? [y/N]"));
        assert!(line_looks_like_prompt("password:"));
        assert!(line_looks_like_prompt(">"));
        assert!(!line_looks_like_prompt("compiled in 200ms"));
    }

    #[test]
    fn prompt_detection_sees_through_ansi_color_codes() {
        // Colored prompts end with a reset (`…m`); stripping ANSI must restore
        // the real trailing character so detection still fires.
        assert!(line_looks_like_prompt("\x1b[36mPassword:\x1b[0m"));
        assert!(line_looks_like_prompt("\x1b[1mContinue?\x1b[0m"));
        assert!(line_looks_like_prompt("\x1b[33mProceed (y/n)\x1b[0m"));
        // A plain colored status line must still not be treated as a prompt.
        assert!(!line_looks_like_prompt("\x1b[32mcompiled in 200ms\x1b[0m"));
    }
}
