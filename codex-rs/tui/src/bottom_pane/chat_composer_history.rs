use std::collections::HashMap;
use std::path::PathBuf;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::MentionBinding;
use crate::mention_codec::decode_history_mentions;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::TextElement;

/// A composer history entry that can rehydrate draft state.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HistoryEntry {
    /// Raw text stored in history (may include placeholder strings).
    pub(crate) text: String,
    /// Text element ranges for placeholders inside `text`.
    pub(crate) text_elements: Vec<TextElement>,
    /// Local image paths captured alongside `text_elements`.
    pub(crate) local_image_paths: Vec<PathBuf>,
    /// Remote image URLs restored with this draft.
    pub(crate) remote_image_urls: Vec<String>,
    /// Mention bindings for tool/app/skill references inside `text`.
    pub(crate) mention_bindings: Vec<MentionBinding>,
    /// Placeholder-to-payload pairs used to restore large paste content.
    pub(crate) pending_pastes: Vec<(String, String)>,
    /// Working directory when this entry was submitted.
    pub(crate) cwd: Option<String>,
}

impl HistoryEntry {
    pub(crate) fn new(text: String) -> Self {
        let decoded = decode_history_mentions(&text);
        Self {
            text: decoded.text,
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
            mention_bindings: decoded
                .mentions
                .into_iter()
                .map(|mention| MentionBinding {
                    mention: mention.mention,
                    path: mention.path,
                })
                .collect(),
            pending_pastes: Vec::new(),
            cwd: None,
        }
    }

    pub(crate) fn new_with_cwd(text: String, cwd: Option<String>) -> Self {
        let mut entry = Self::new(text);
        entry.cwd = cwd;
        entry
    }

    #[cfg(test)]
    pub(crate) fn with_pending(
        text: String,
        text_elements: Vec<TextElement>,
        local_image_paths: Vec<PathBuf>,
        pending_pastes: Vec<(String, String)>,
    ) -> Self {
        Self {
            text,
            text_elements,
            local_image_paths,
            remote_image_urls: Vec::new(),
            mention_bindings: Vec::new(),
            pending_pastes,
            cwd: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_pending_and_remote(
        text: String,
        text_elements: Vec<TextElement>,
        local_image_paths: Vec<PathBuf>,
        pending_pastes: Vec<(String, String)>,
        remote_image_urls: Vec<String>,
    ) -> Self {
        Self {
            text,
            text_elements,
            local_image_paths,
            remote_image_urls,
            mention_bindings: Vec::new(),
            pending_pastes,
            cwd: None,
        }
    }
}

/// Navigation phase: CWD-local entries first, then all entries.
#[derive(Clone, Copy, PartialEq)]
enum NavPhase {
    CwdLocal,
    Global,
}

/// State machine that manages shell-style history navigation (Up/Down) inside
/// the chat composer, with CWD-aware priority and Ctrl+R reverse search.
pub(crate) struct ChatComposerHistory {
    /// Identifier of the history log as reported by `SessionConfiguredEvent`.
    history_log_id: Option<u64>,
    /// Number of entries already present in the persistent cross-session
    /// history file when the session started.
    history_entry_count: usize,

    /// Messages submitted by the user *during this UI session* (newest at END).
    local_history: Vec<HistoryEntry>,

    /// Cache of persistent history entries fetched on-demand (text-only).
    fetched_history: HashMap<usize, HistoryEntry>,

    /// Current cursor within the combined (persistent + local) history.
    history_cursor: Option<isize>,

    /// The text that was last inserted into the composer as a result of
    /// history navigation.
    last_history_text: Option<String>,

    /// Current working directory for CWD-local filtering.
    current_cwd: Option<String>,

    /// Persistent history offsets that match the current CWD (populated on session start).
    cwd_persistent_offsets: Vec<usize>,

    /// Navigation phase tracking.
    nav_phase: NavPhase,
    /// Cursor within the CWD-local phase (indexes into cwd_combined_indices).
    cwd_cursor: Option<usize>,

    /// Reverse search state.
    pub(crate) search_active: bool,
    pub(crate) search_query: String,
    search_matches: Vec<SearchMatch>,
    search_match_cursor: Option<usize>,
}

#[derive(Clone)]
struct SearchMatch {
    entry: HistoryEntry,
}

impl ChatComposerHistory {
    pub fn new() -> Self {
        Self {
            history_log_id: None,
            history_entry_count: 0,
            local_history: Vec::new(),
            fetched_history: HashMap::new(),
            history_cursor: None,
            last_history_text: None,
            current_cwd: None,
            cwd_persistent_offsets: Vec::new(),
            nav_phase: NavPhase::CwdLocal,
            cwd_cursor: None,
            search_active: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_match_cursor: None,
        }
    }

    /// Update metadata when a new session is configured.
    pub fn set_metadata(&mut self, log_id: u64, entry_count: usize) {
        self.history_log_id = Some(log_id);
        self.history_entry_count = entry_count;
        self.fetched_history.clear();
        self.local_history.clear();
        self.history_cursor = None;
        self.last_history_text = None;
        self.nav_phase = NavPhase::CwdLocal;
        self.cwd_cursor = None;
    }

    /// Set the current CWD for directory-aware history cycling.
    pub fn set_cwd(&mut self, cwd: String) {
        self.current_cwd = Some(cwd);
    }

    /// Set persistent offsets that match the current CWD.
    pub fn set_cwd_persistent_offsets(&mut self, offsets: Vec<usize>) {
        self.cwd_persistent_offsets = offsets;
    }

    /// Record a message submitted by the user in the current session.
    pub fn record_local_submission(&mut self, mut entry: HistoryEntry) {
        if entry.text.is_empty()
            && entry.text_elements.is_empty()
            && entry.local_image_paths.is_empty()
            && entry.remote_image_urls.is_empty()
            && entry.mention_bindings.is_empty()
            && entry.pending_pastes.is_empty()
        {
            return;
        }
        self.history_cursor = None;
        self.last_history_text = None;
        self.nav_phase = NavPhase::CwdLocal;
        self.cwd_cursor = None;

        // Tag with current CWD if not already set
        if entry.cwd.is_none() {
            entry.cwd = self.current_cwd.clone();
        }

        if self.local_history.last().is_some_and(|prev| prev == &entry) {
            return;
        }

        self.local_history.push(entry);
    }

    /// Reset navigation tracking so the next Up key resumes from the latest entry.
    pub fn reset_navigation(&mut self) {
        self.history_cursor = None;
        self.last_history_text = None;
        self.nav_phase = NavPhase::CwdLocal;
        self.cwd_cursor = None;
    }

    /// Returns whether Up/Down should navigate history for the current textarea state.
    pub fn should_handle_navigation(&self, text: &str, cursor: usize) -> bool {
        if self.history_entry_count == 0 && self.local_history.is_empty() {
            return false;
        }

        if text.is_empty() {
            return true;
        }

        if cursor != 0 && cursor != text.len() {
            return false;
        }

        matches!(&self.last_history_text, Some(prev) if prev == text)
    }

    /// CWD-local entries: local entries matching current cwd + persistent cwd offsets.
    fn cwd_local_entries_count(&self) -> usize {
        let local_cwd_count = self.local_cwd_entries().len();
        local_cwd_count + self.cwd_persistent_offsets.len()
    }

    fn local_cwd_entries(&self) -> Vec<HistoryEntry> {
        let cwd = match &self.current_cwd {
            Some(c) => c,
            None => return self.local_history.clone(),
        };
        self.local_history
            .iter()
            .filter(|e| e.cwd.as_deref() == Some(cwd.as_str()))
            .cloned()
            .collect()
    }

    /// Handle <Up> with CWD-first priority.
    pub fn navigate_up(&mut self, app_event_tx: &AppEventSender) -> Option<HistoryEntry> {
        if self.nav_phase == NavPhase::CwdLocal {
            if let Some(entry) = self.navigate_cwd_up(app_event_tx) {
                return Some(entry);
            }
            // Transition to global phase
            self.nav_phase = NavPhase::Global;
            self.history_cursor = None;
        }
        self.navigate_global_up(app_event_tx)
    }

    /// Handle <Down> with CWD-first priority.
    pub fn navigate_down(&mut self, app_event_tx: &AppEventSender) -> Option<HistoryEntry> {
        if self.nav_phase == NavPhase::Global {
            if let Some(entry) = self.navigate_global_down(app_event_tx) {
                return Some(entry);
            }
            // Transition back to cwd phase at bottom
            self.nav_phase = NavPhase::CwdLocal;
            self.cwd_cursor = None;
        }
        self.navigate_cwd_down(app_event_tx)
    }

    fn navigate_cwd_up(&mut self, app_event_tx: &AppEventSender) -> Option<HistoryEntry> {
        let local_cwd_count = self.local_cwd_entries().len();
        let persistent_count = self.cwd_persistent_offsets.len();
        let total_cwd = local_cwd_count + persistent_count;
        if total_cwd == 0 {
            return None;
        }

        let next = match self.cwd_cursor {
            None => total_cwd - 1,
            Some(0) => return None,
            Some(idx) => idx - 1,
        };
        self.cwd_cursor = Some(next);

        if next < persistent_count {
            let global_offset = self.cwd_persistent_offsets[next];
            if let Some(entry) = self.fetched_history.get(&global_offset).cloned() {
                self.last_history_text = Some(entry.text.clone());
                return Some(entry);
            }
            if let Some(log_id) = self.history_log_id {
                app_event_tx.send(AppEvent::CodexOp(Op::GetHistoryEntryRequest {
                    offset: global_offset,
                    log_id,
                }));
            }
            None
        } else {
            let local_idx = next - persistent_count;
            let local_cwd = self.local_cwd_entries();
            if let Some(entry) = local_cwd.get(local_idx).cloned() {
                self.last_history_text = Some(entry.text.clone());
                Some(entry)
            } else {
                None
            }
        }
    }

    fn navigate_cwd_down(&mut self, _app_event_tx: &AppEventSender) -> Option<HistoryEntry> {
        let local_cwd_count = self.local_cwd_entries().len();
        let persistent_count = self.cwd_persistent_offsets.len();
        let total_cwd = local_cwd_count + persistent_count;

        let next_opt = match self.cwd_cursor {
            None => return None,
            Some(idx) if idx + 1 >= total_cwd => None,
            Some(idx) => Some(idx + 1),
        };

        match next_opt {
            Some(idx) => {
                self.cwd_cursor = Some(idx);
                if idx < persistent_count {
                    let global_offset = self.cwd_persistent_offsets[idx];
                    if let Some(entry) = self.fetched_history.get(&global_offset).cloned() {
                        self.last_history_text = Some(entry.text.clone());
                        return Some(entry);
                    }
                    None
                } else {
                    let local_idx = idx - persistent_count;
                    let local_cwd = self.local_cwd_entries();
                    if let Some(entry) = local_cwd.get(local_idx).cloned() {
                        self.last_history_text = Some(entry.text.clone());
                        Some(entry)
                    } else {
                        None
                    }
                }
            }
            None => {
                self.cwd_cursor = None;
                self.last_history_text = None;
                Some(HistoryEntry::new(String::new()))
            }
        }
    }

    fn navigate_global_up(&mut self, app_event_tx: &AppEventSender) -> Option<HistoryEntry> {
        let total_entries = self.history_entry_count + self.local_history.len();
        if total_entries == 0 {
            return None;
        }

        let next_idx = match self.history_cursor {
            None => (total_entries as isize) - 1,
            Some(0) => return None,
            Some(idx) => idx - 1,
        };

        self.history_cursor = Some(next_idx);
        self.populate_history_at_index(next_idx as usize, app_event_tx)
    }

    fn navigate_global_down(&mut self, app_event_tx: &AppEventSender) -> Option<HistoryEntry> {
        let total_entries = self.history_entry_count + self.local_history.len();
        if total_entries == 0 {
            return None;
        }

        let next_idx_opt = match self.history_cursor {
            None => return None,
            Some(idx) if (idx as usize) + 1 >= total_entries => None,
            Some(idx) => Some(idx + 1),
        };

        match next_idx_opt {
            Some(idx) => {
                self.history_cursor = Some(idx);
                self.populate_history_at_index(idx as usize, app_event_tx)
            }
            None => {
                self.history_cursor = None;
                self.last_history_text = None;
                Some(HistoryEntry::new(String::new()))
            }
        }
    }

    /// Integrate a GetHistoryEntryResponse event.
    pub fn on_entry_response(
        &mut self,
        log_id: u64,
        offset: usize,
        entry: Option<String>,
    ) -> Option<HistoryEntry> {
        self.on_entry_response_with_cwd(log_id, offset, entry, None)
    }

    pub fn on_entry_response_with_cwd(
        &mut self,
        log_id: u64,
        offset: usize,
        entry: Option<String>,
        cwd: Option<String>,
    ) -> Option<HistoryEntry> {
        if self.history_log_id != Some(log_id) {
            return None;
        }
        let entry = HistoryEntry::new_with_cwd(entry?, cwd);
        self.fetched_history.insert(offset, entry.clone());

        let matches_cursor = self.history_cursor == Some(offset as isize);
        let matches_cwd_cursor = self.nav_phase == NavPhase::CwdLocal
            && self.cwd_cursor.is_some()
            && self
                .cwd_persistent_offsets
                .get(self.cwd_cursor.unwrap_or(usize::MAX))
                .copied()
                == Some(offset);

        if matches_cursor || matches_cwd_cursor {
            self.last_history_text = Some(entry.text.clone());
            return Some(entry);
        }
        None
    }

    // -----------------------------------------------------------------
    // Reverse search (Ctrl+R)
    // -----------------------------------------------------------------

    pub fn start_search(&mut self) {
        self.search_active = true;
        self.search_query.clear();
        self.search_matches.clear();
        self.search_match_cursor = None;
    }

    pub fn cancel_search(&mut self) {
        self.search_active = false;
        self.search_query.clear();
        self.search_matches.clear();
        self.search_match_cursor = None;
    }

    pub fn search_query_push(&mut self, ch: char) {
        self.search_query.push(ch);
        self.rebuild_search_matches();
    }

    pub fn search_query_pop(&mut self) {
        self.search_query.pop();
        self.rebuild_search_matches();
    }

    /// Move to the next (older) match. Returns entry if found.
    pub fn search_next(&mut self) -> Option<HistoryEntry> {
        if self.search_matches.is_empty() {
            return None;
        }
        let next = match self.search_match_cursor {
            None => 0,
            Some(idx) if idx + 1 >= self.search_matches.len() => return None,
            Some(idx) => idx + 1,
        };
        self.search_match_cursor = Some(next);
        Some(self.search_matches[next].entry.clone())
    }

    /// Get the current search match.
    pub fn current_search_match(&self) -> Option<HistoryEntry> {
        self.search_match_cursor
            .and_then(|idx| self.search_matches.get(idx))
            .map(|m| m.entry.clone())
    }

    /// Accept the current search match and exit search mode.
    pub fn accept_search(&mut self) -> Option<HistoryEntry> {
        let result = self.current_search_match();
        self.search_active = false;
        self.search_query.clear();
        self.search_matches.clear();
        self.search_match_cursor = None;
        result
    }

    /// Format the search prompt for display.
    pub fn search_prompt(&self) -> String {
        let match_info = match (self.search_match_cursor, self.search_matches.len()) {
            (Some(idx), total) if total > 0 => format!(" [{}/{}]", idx + 1, total),
            _ if !self.search_query.is_empty() && self.search_matches.is_empty() => {
                " [no match]".to_string()
            }
            _ => String::new(),
        };
        format!("(reverse-i-search){match_info}`{}'", self.search_query)
    }

    fn rebuild_search_matches(&mut self) {
        self.search_matches.clear();
        self.search_match_cursor = None;
        if self.search_query.is_empty() {
            return;
        }
        let query_lower = self.search_query.to_lowercase();

        // Search local entries (newest first)
        for entry in self.local_history.iter().rev() {
            if entry.text.to_lowercase().contains(&query_lower) {
                self.search_matches.push(SearchMatch {
                    entry: entry.clone(),
                });
            }
        }

        // Search fetched persistent entries (newest offset first)
        let mut persistent_offsets: Vec<usize> = self.fetched_history.keys().copied().collect();
        persistent_offsets.sort_unstable_by(|a, b| b.cmp(a));
        for offset in persistent_offsets {
            if let Some(entry) = self.fetched_history.get(&offset) {
                if entry.text.to_lowercase().contains(&query_lower) {
                    self.search_matches.push(SearchMatch {
                        entry: entry.clone(),
                    });
                }
            }
        }

        if !self.search_matches.is_empty() {
            self.search_match_cursor = Some(0);
        }
    }

    // -----------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------

    fn populate_history_at_index(
        &mut self,
        global_idx: usize,
        app_event_tx: &AppEventSender,
    ) -> Option<HistoryEntry> {
        if global_idx >= self.history_entry_count {
            if let Some(entry) = self
                .local_history
                .get(global_idx - self.history_entry_count)
                .cloned()
            {
                self.last_history_text = Some(entry.text.clone());
                return Some(entry);
            }
        } else if let Some(entry) = self.fetched_history.get(&global_idx).cloned() {
            self.last_history_text = Some(entry.text.clone());
            return Some(entry);
        } else if let Some(log_id) = self.history_log_id {
            app_event_tx.send(AppEvent::CodexOp(Op::GetHistoryEntryRequest {
                offset: global_idx,
                log_id,
            }));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::unbounded_channel;

    #[test]
    fn duplicate_submissions_are_not_recorded() {
        let mut history = ChatComposerHistory::new();

        history.record_local_submission(HistoryEntry::new(String::new()));
        assert_eq!(history.local_history.len(), 0);

        history.record_local_submission(HistoryEntry::new("hello".to_string()));
        assert_eq!(history.local_history.len(), 1);
        assert_eq!(
            history.local_history.last().unwrap(),
            &HistoryEntry::new("hello".to_string())
        );

        history.record_local_submission(HistoryEntry::new("hello".to_string()));
        assert_eq!(history.local_history.len(), 1);

        history.record_local_submission(HistoryEntry::new("world".to_string()));
        assert_eq!(history.local_history.len(), 2);
        assert_eq!(
            history.local_history.last().unwrap(),
            &HistoryEntry::new("world".to_string())
        );
    }

    #[test]
    fn navigation_with_async_fetch() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        history.set_metadata(/*log_id*/ 1, /*entry_count*/ 3);

        assert!(history.should_handle_navigation("", /*cursor*/ 0));
        // First up enters CWD phase, which has 0 CWD entries, so transitions to global
        assert!(history.navigate_up(&tx).is_none());

        let event = rx.try_recv().expect("expected AppEvent to be sent");
        let AppEvent::CodexOp(op) = event else {
            panic!("unexpected event variant");
        };
        assert_eq!(
            Op::GetHistoryEntryRequest {
                log_id: 1,
                offset: 2,
            },
            op
        );

        assert_eq!(
            Some(HistoryEntry::new("latest".to_string())),
            history.on_entry_response(/*log_id*/ 1, /*offset*/ 2, Some("latest".into()))
        );

        assert!(history.navigate_up(&tx).is_none());

        let event2 = rx.try_recv().expect("expected second event");
        let AppEvent::CodexOp(op) = event2 else {
            panic!("unexpected event variant");
        };
        assert_eq!(
            Op::GetHistoryEntryRequest {
                log_id: 1,
                offset: 1,
            },
            op
        );

        assert_eq!(
            Some(HistoryEntry::new("older".to_string())),
            history.on_entry_response(/*log_id*/ 1, /*offset*/ 1, Some("older".into()))
        );
    }

    #[test]
    fn reset_navigation_resets_cursor() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        history.set_metadata(/*log_id*/ 1, /*entry_count*/ 3);
        history
            .fetched_history
            .insert(1, HistoryEntry::new("command2".to_string()));
        history
            .fetched_history
            .insert(2, HistoryEntry::new("command3".to_string()));

        assert_eq!(
            Some(HistoryEntry::new("command3".to_string())),
            history.navigate_up(&tx)
        );
        assert_eq!(
            Some(HistoryEntry::new("command2".to_string())),
            history.navigate_up(&tx)
        );

        history.reset_navigation();
        assert!(history.history_cursor.is_none());
        assert!(history.last_history_text.is_none());

        assert_eq!(
            Some(HistoryEntry::new("command3".to_string())),
            history.navigate_up(&tx)
        );
    }

    #[test]
    fn should_handle_navigation_when_cursor_is_at_line_boundaries() {
        let mut history = ChatComposerHistory::new();
        history.record_local_submission(HistoryEntry::new("hello".to_string()));
        history.last_history_text = Some("hello".to_string());

        assert!(history.should_handle_navigation("hello", /*cursor*/ 0));
        assert!(history.should_handle_navigation("hello", "hello".len()));
        assert!(!history.should_handle_navigation("hello", /*cursor*/ 1));
        assert!(!history.should_handle_navigation("other", /*cursor*/ 0));
    }

    #[test]
    fn cwd_local_entries_cycle_first() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);

        let mut history = ChatComposerHistory::new();
        history.set_cwd("/project/a".to_string());

        let mut e1 = HistoryEntry::new("cmd in a".to_string());
        e1.cwd = Some("/project/a".to_string());
        let mut e2 = HistoryEntry::new("cmd in b".to_string());
        e2.cwd = Some("/project/b".to_string());
        let mut e3 = HistoryEntry::new("another in a".to_string());
        e3.cwd = Some("/project/a".to_string());

        history.record_local_submission(e1.clone());
        history.record_local_submission(e2);
        history.record_local_submission(e3.clone());

        // First Up: should get "another in a" (newest CWD match)
        let result = history.navigate_up(&tx);
        assert_eq!(
            result.as_ref().map(|e| e.text.as_str()),
            Some("another in a")
        );

        // Second Up: should get "cmd in a" (older CWD match)
        let result = history.navigate_up(&tx);
        assert_eq!(result.as_ref().map(|e| e.text.as_str()), Some("cmd in a"));

        // Third Up: CWD exhausted, transitions to global, gets "another in a" (newest global)
        let result = history.navigate_up(&tx);
        assert_eq!(
            result.as_ref().map(|e| e.text.as_str()),
            Some("another in a")
        );
    }

    #[test]
    fn reverse_search_finds_matching_entries() {
        let mut history = ChatComposerHistory::new();
        history.record_local_submission(HistoryEntry::new("fix the bug".to_string()));
        history.record_local_submission(HistoryEntry::new("run the tests".to_string()));
        history.record_local_submission(HistoryEntry::new("fix another bug".to_string()));

        history.start_search();
        history.search_query_push('f');
        history.search_query_push('i');
        history.search_query_push('x');

        assert_eq!(history.search_matches.len(), 2);
        let first = history.current_search_match().unwrap();
        assert_eq!(first.text, "fix another bug");

        let next = history.search_next().unwrap();
        assert_eq!(next.text, "fix the bug");
    }
}
