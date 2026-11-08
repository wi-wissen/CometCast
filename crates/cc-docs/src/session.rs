// SPDX-License-Identifier: GPL-3.0-or-later
//! Receiver-side page state, driven by the remote-control commands of `cc-proto`.
//!
//! Pure logic: no rendering, no I/O. The page numbers are 1-based like `ControlCmd::GoToPage`.

use cc_proto::ControlCmd;

/// What a command did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    /// The shown page changed to this page number (1-based).
    PageChanged(u32),
    /// Valid command, but nothing changed (already at the first or last page, or the same
    /// page asked for again).
    Unchanged,
    /// `Stop` was applied: the content is no longer shown.
    Stopped,
    /// The session was stopped before; commands are ignored.
    Ignored,
}

/// Which page of a document is shown. Commands that go out of range are clamped, never rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentSession {
    page_count: u32,
    current: u32,
    stopped: bool,
}

impl DocumentSession {
    /// A session on page 1 of a document with `page_count` pages. `None` for 0 pages.
    pub fn new(page_count: u32) -> Option<Self> {
        (page_count > 0).then_some(Self {
            page_count,
            current: 1,
            stopped: false,
        })
    }

    /// The shown page number (1-based).
    pub fn current_page(&self) -> u32 {
        self.current
    }

    /// The shown page as a 0-based index for [`Document`](crate::Document).
    pub fn current_index(&self) -> usize {
        (self.current - 1) as usize
    }

    /// Pages in the document.
    pub fn page_count(&self) -> u32 {
        self.page_count
    }

    /// `Stop` was applied.
    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// Jump to `page` (1-based); 0 becomes 1 and anything past the end becomes the last page.
    pub fn go_to(&mut self, page: u32) -> SessionEvent {
        if self.stopped {
            return SessionEvent::Ignored;
        }
        let target = page.clamp(1, self.page_count);
        self.set(target)
    }

    /// Next page; stays on the last page.
    pub fn next_page(&mut self) -> SessionEvent {
        if self.stopped {
            return SessionEvent::Ignored;
        }
        self.set(self.current.saturating_add(1).min(self.page_count))
    }

    /// Previous page; stays on the first page.
    pub fn prev_page(&mut self) -> SessionEvent {
        if self.stopped {
            return SessionEvent::Ignored;
        }
        self.set(self.current.saturating_sub(1).max(1))
    }

    /// Apply a remote-control command.
    pub fn apply(&mut self, cmd: &ControlCmd) -> SessionEvent {
        match cmd {
            ControlCmd::NextPage => self.next_page(),
            ControlCmd::PrevPage => self.prev_page(),
            ControlCmd::GoToPage { page } => self.go_to(*page),
            // Pause and resume concern the live screen picture, not a document.
            ControlCmd::Pause | ControlCmd::Resume => SessionEvent::Ignored,
            ControlCmd::Stop => {
                if self.stopped {
                    SessionEvent::Ignored
                } else {
                    self.stopped = true;
                    SessionEvent::Stopped
                }
            }
        }
    }

    fn set(&mut self, target: u32) -> SessionEvent {
        if target == self.current {
            SessionEvent::Unchanged
        } else {
            self.current = target;
            SessionEvent::PageChanged(target)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(n: u32) -> DocumentSession {
        DocumentSession::new(n).unwrap()
    }

    #[test]
    fn zero_pages_is_refused() {
        assert!(DocumentSession::new(0).is_none());
    }

    #[test]
    fn next_and_prev_clamp() {
        let mut x = s(3);
        assert_eq!(x.prev_page(), SessionEvent::Unchanged);
        assert_eq!(x.next_page(), SessionEvent::PageChanged(2));
        assert_eq!(x.next_page(), SessionEvent::PageChanged(3));
        assert_eq!(x.next_page(), SessionEvent::Unchanged);
        assert_eq!(x.current_page(), 3);
        assert_eq!(x.prev_page(), SessionEvent::PageChanged(2));
    }

    #[test]
    fn go_to_clamps_both_ends() {
        let mut x = s(5);
        assert_eq!(x.go_to(0), SessionEvent::Unchanged);
        assert_eq!(x.go_to(4), SessionEvent::PageChanged(4));
        assert_eq!(x.go_to(4), SessionEvent::Unchanged);
        assert_eq!(x.go_to(u32::MAX), SessionEvent::PageChanged(5));
        assert_eq!(x.go_to(0), SessionEvent::PageChanged(1));
        assert_eq!(x.current_index(), 0);
    }

    #[test]
    fn single_page_never_moves() {
        let mut x = s(1);
        assert_eq!(x.next_page(), SessionEvent::Unchanged);
        assert_eq!(x.prev_page(), SessionEvent::Unchanged);
        assert_eq!(x.go_to(9), SessionEvent::Unchanged);
    }

    #[test]
    fn control_commands_drive_the_session() {
        let mut x = s(10);
        assert_eq!(x.apply(&ControlCmd::NextPage), SessionEvent::PageChanged(2));
        assert_eq!(
            x.apply(&ControlCmd::GoToPage { page: 7 }),
            SessionEvent::PageChanged(7)
        );
        assert_eq!(x.apply(&ControlCmd::PrevPage), SessionEvent::PageChanged(6));
        assert_eq!(x.apply(&ControlCmd::Stop), SessionEvent::Stopped);
        assert!(x.is_stopped());
        // After Stop nothing moves and the page stays where it was.
        assert_eq!(x.apply(&ControlCmd::NextPage), SessionEvent::Ignored);
        assert_eq!(x.apply(&ControlCmd::Stop), SessionEvent::Ignored);
        assert_eq!(x.current_page(), 6);
    }

    #[test]
    fn commands_from_json_work() {
        // The web client sends serde's default form (docs/web-m2.md).
        let cmd: ControlCmd = serde_json::from_str(r#"{"go_to_page":{"page":3}}"#).unwrap();
        let mut x = s(4);
        assert_eq!(x.apply(&cmd), SessionEvent::PageChanged(3));
    }
}
