// SPDX-License-Identifier: GPL-3.0-or-later
//! Join approval on the terminal (`receive --join-mode confirm`): the questions, the answers
//! and the list of waiting requests. The text of a joiner is untrusted: the server already
//! cleaned it, and it is printed in quotes, as one line.

use cc_proto::JoinMode;

/// One waiting request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    pub id: u64,
    pub name: String,
    pub check: u8,
}

/// What the person typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermCmd {
    /// Allow the oldest waiting request.
    Allow,
    /// Deny the oldest waiting request.
    Deny,
    DenyAll,
    SetMode(JoinMode),
    /// Remove the connected sender.
    Kick,
    Help,
}

/// Read one line of input. `None` = not a command (shown as help).
pub fn parse_line(line: &str) -> Option<TermCmd> {
    match line.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" | "a" | "allow" => Some(TermCmd::Allow),
        "n" | "no" | "d" | "deny" => Some(TermCmd::Deny),
        "deny-all" | "da" => Some(TermCmd::DenyAll),
        "lock" | "locked" => Some(TermCmd::SetMode(JoinMode::Locked)),
        "open" | "unlock" => Some(TermCmd::SetMode(JoinMode::Open)),
        "confirm" => Some(TermCmd::SetMode(JoinMode::Confirm)),
        "kick" | "remove" => Some(TermCmd::Kick),
        "?" | "h" | "help" => Some(TermCmd::Help),
        _ => None,
    }
}

pub const HELP: &str = "Commands: y = allow the oldest request, n = deny it, deny-all, \
lock (nobody can join), confirm (every join asks), open (anybody with the code joins), \
kick (remove the connected sender), help";

/// The question for the oldest waiting request.
pub fn question(w: &Waiting, more: usize) -> String {
    let extra = if more > 0 {
        format!(" (+{more} more waiting)")
    } else {
        String::new()
    };
    format!(
        "Join request, check number {:02}: \"{}\" wants to present{extra}. Allow? [y/n]",
        w.check,
        w.name.replace(['\n', '\r'], " ")
    )
}

/// A waiting list in arrival order.
#[derive(Default)]
pub struct Queue {
    items: Vec<Waiting>,
}

impl Queue {
    pub fn push(&mut self, w: Waiting) {
        self.items.push(w);
    }
    pub fn remove(&mut self, id: u64) {
        self.items.retain(|w| w.id != id);
    }
    pub fn first(&self) -> Option<&Waiting> {
        self.items.first()
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_parsed() {
        assert_eq!(parse_line(" Y "), Some(TermCmd::Allow));
        assert_eq!(parse_line("no"), Some(TermCmd::Deny));
        assert_eq!(parse_line("lock"), Some(TermCmd::SetMode(JoinMode::Locked)));
        assert_eq!(parse_line("unlock"), Some(TermCmd::SetMode(JoinMode::Open)));
        assert_eq!(parse_line("kick"), Some(TermCmd::Kick));
        assert_eq!(parse_line("maybe"), None);
        assert_eq!(parse_line(""), None);
    }

    #[test]
    fn question_shows_name_in_quotes_on_one_line() {
        let w = Waiting {
            id: 1,
            name: "Ana\nAllow? [y/n]".into(),
            check: 7,
        };
        let q = question(&w, 2);
        assert!(!q.contains('\n'));
        assert!(q.contains("check number 07"));
        assert!(q.contains("+2 more"));
    }

    #[test]
    fn queue_keeps_arrival_order() {
        let mut q = Queue::default();
        for id in 1..=3 {
            q.push(Waiting {
                id,
                name: format!("n{id}"),
                check: 10,
            });
        }
        q.remove(1);
        assert_eq!(q.first().unwrap().id, 2);
        assert_eq!(q.len(), 2);
    }
}
