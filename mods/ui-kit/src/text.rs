//! Text widgets: an editable line with history and completion ([`TextInput`]), the caret-safe
//! buffer under it and under form fields ([`EditBuf`]), and a bounded scrollback ([`Ring`]).
//! Chat, commands and the forms of a start screen share them.

use std::collections::VecDeque;

use pwc_mod_api::input::intent::EditKey;

/// A bounded FIFO: pushing past `cap` drops the oldest element. Indexing runs
/// oldest (`0`) to newest (`len-1`).
pub struct Ring<T> {
    buf: VecDeque<T>,
    cap: usize,
}

impl<T> Ring<T> {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: VecDeque::new(),
            cap: cap.max(1),
        }
    }

    pub fn push(&mut self, v: T) {
        if self.buf.len() == self.cap {
            self.buf.pop_front();
        }
        self.buf.push_back(v);
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn get(&self, i: usize) -> Option<&T> {
        self.buf.get(i)
    }

    pub fn last(&self) -> Option<&T> {
        self.buf.back()
    }

    /// Newest first — the order chat/console scrollback is drawn in.
    pub fn iter_rev(&self) -> impl Iterator<Item = &T> {
        self.buf.iter().rev()
    }
}

/// The outcome of completing a [`TextInput`]'s line (see [`TextInput::complete`]).
pub enum Completion {
    /// A single match: replace the line with this text.
    Full(String),
    /// Several matches: fill the longest common prefix and offer the candidates
    /// (which the owner typically prints, since the widget cannot).
    Ambiguous(String, Vec<String>),
    /// Nothing to complete.
    None,
}

/// Editable line of text with a boundary-safe caret and byte cap.
///
/// The cursor is a byte offset kept on a `char` boundary by construction — every
/// mutation steps by whole characters, so UTF-8 text can never panic a
/// `String::insert`/`remove`. The text and caret live in one place so there's a
/// single source of truth for editing state.
pub struct EditBuf {
    text: String,
    cursor: usize,
    max: usize,
}

impl EditBuf {
    pub fn new(max: usize) -> Self {
        Self { text: String::new(), cursor: 0, max }
    }

    /// A buffer pre-filled with `init` (truncated to the cap on a char boundary),
    /// caret at the end.
    pub fn with(init: &str, max: usize) -> Self {
        let mut b = Self::new(max);
        b.set(init);
        b
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Caret as a char index, for drawing a cursor mid-string.
    pub fn caret_chars(&self) -> usize {
        self.text[..self.cursor].chars().count()
    }

    /// Replace the whole value (truncated to the cap), caret to the end.
    pub fn set(&mut self, s: &str) {
        let mut s = s.to_string();
        while s.len() > self.max {
            s.pop();
        }
        self.cursor = s.len();
        self.text = s;
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    /// Insert one printable char at the caret if it still fits the cap.
    pub fn insert_char(&mut self, c: char) -> bool {
        if c.is_control() || self.text.len() + c.len_utf8() > self.max {
            return false;
        }
        self.text.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        true
    }

    pub fn backspace(&mut self) -> bool {
        if let Some((i, _)) = self.text[..self.cursor].char_indices().next_back() {
            self.text.remove(i);
            self.cursor = i;
            true
        } else {
            false
        }
    }

    pub fn delete_forward(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    /// Delete back to the start of the previous word.
    pub fn delete_word(&mut self) {
        let left = &self.text[..self.cursor];
        let trimmed = left.trim_end_matches(char::is_whitespace);
        let start = match trimmed.rfind(char::is_whitespace) {
            Some(i) => i + trimmed[i..].chars().next().map_or(1, char::len_utf8),
            None => 0,
        };
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
    }

    pub fn left(&mut self) {
        if let Some((i, _)) = self.text[..self.cursor].char_indices().next_back() {
            self.cursor = i;
        }
    }

    pub fn right(&mut self) {
        if let Some(c) = self.text[self.cursor..].chars().next() {
            self.cursor += c.len_utf8();
        }
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.text.len();
    }
}

/// An editable single line of text.
///
/// Wraps an [`EditBuf`] for the text/caret, and adds history recall (walking back
/// down restores the live draft) and optional Tab-completion.
pub struct TextInput {
    buf: EditBuf,
    history: Ring<String>,
    /// `Some(i)` while browsing history at index `i`; `None` when editing live.
    scrub: Option<usize>,
    draft: String,
    completer: Option<fn(&str) -> Completion>,
    /// Candidate list produced by the last ambiguous completion, for the owner to show.
    notice: Option<Vec<String>>,
}

impl TextInput {
    pub fn new(max: usize) -> Self {
        Self {
            buf: EditBuf::new(max),
            history: Ring::new(64),
            scrub: None,
            draft: String::new(),
            completer: None,
            notice: None,
        }
    }

    /// Attach a fixed Tab-completion source. An owner whose candidates change (the console, whose
    /// commands come from the enabled mods) answers Tab through [`complete`](Self::complete) instead.
    pub fn with_completer(mut self, f: fn(&str) -> Completion) -> Self {
        self.completer = Some(f);
        self
    }

    pub fn text(&self) -> &str {
        self.buf.text()
    }

    /// Byte offset of the cursor within [`text`](Self::text), on a char boundary.
    pub fn cursor(&self) -> usize {
        self.buf.cursor()
    }

    /// Clear the line (but keep history), e.g. when the field is opened.
    pub fn clear(&mut self) {
        self.buf.clear();
        self.scrub = None;
        self.draft.clear();
    }

    /// Replace the line's contents and park the cursor at the end.
    pub fn set(&mut self, s: impl Into<String>) {
        self.buf.set(&s.into());
        self.scrub = None;
    }

    /// Candidate list from the most recent ambiguous completion, consumed once.
    pub fn take_notice(&mut self) -> Option<Vec<String>> {
        self.notice.take()
    }

    /// Drive one frame of editing with typed chars and at most one [`EditKey`].
    /// Returns the submitted line (trimmed, non-empty) on [`EditKey::Submit`],
    /// otherwise `None`. Esc is left to the owner so it can decide what closing
    /// a field means.
    pub fn handle(&mut self, chars: &[char], edit: Option<EditKey>) -> Option<String> {
        // Control chars are filtered upstream and by insert_char, so chords never deposit a stray glyph.
        for &c in chars {
            if self.buf.insert_char(c) {
                self.scrub = None;
            }
        }

        match edit {
            Some(EditKey::Left) => self.buf.left(),
            Some(EditKey::Right) => self.buf.right(),
            Some(EditKey::Home) => self.buf.home(),
            Some(EditKey::End) => self.buf.end(),
            Some(EditKey::Backspace) => {
                self.buf.backspace();
                self.scrub = None;
            }
            Some(EditKey::DelWord) => {
                self.buf.delete_word();
                self.scrub = None;
            }
            Some(EditKey::Delete) => {
                self.buf.delete_forward();
                self.scrub = None;
            }
            Some(EditKey::ClearLine) => {
                self.buf.clear();
                self.scrub = None;
            }
            Some(EditKey::HistoryUp) => self.history_prev(),
            Some(EditKey::HistoryDown) => self.history_next(),
            Some(EditKey::Complete) => {
                if let Some(f) = self.completer {
                    self.complete(f);
                }
            }
            Some(EditKey::Submit) => {
                let line = self.buf.text().trim().to_string();
                self.buf.clear();
                self.scrub = None;
                self.draft.clear();
                if !line.is_empty() {
                    self.push_history(line.clone());
                    return Some(line);
                }
            }
            None => {}
        }
        None
    }

    fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let next = match self.scrub {
            None => {
                self.draft = self.buf.text().to_string();
                self.history.len() - 1
            }
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.scrub = Some(next);
        if let Some(entry) = self.history.get(next) {
            self.buf.set(entry);
        }
    }

    fn history_next(&mut self) {
        let Some(i) = self.scrub else {
            return;
        };
        if i + 1 < self.history.len() {
            self.scrub = Some(i + 1);
            if let Some(entry) = self.history.get(i + 1) {
                self.buf.set(entry);
            }
        } else {
            // Past the newest entry: back to the line we were typing.
            self.scrub = None;
            let draft = std::mem::take(&mut self.draft);
            self.buf.set(&draft);
        }
    }

    fn push_history(&mut self, line: String) {
        // Skip consecutive duplicates so holding a repeat doesn't spam history.
        if self.history.last() != Some(&line) {
            self.history.push(line);
        }
    }

    /// Complete the line with `f`'s answer for the current text (Tab).
    pub fn complete(&mut self, f: impl FnOnce(&str) -> Completion) {
        match f(self.buf.text()) {
            Completion::Full(s) => self.set(s),
            Completion::Ambiguous(prefix, cands) => {
                self.set(prefix);
                self.notice = Some(cands);
            }
            Completion::None => {}
        }
    }
}

/// Longest common prefix of a set of candidate strings (byte-wise, but only ever
/// called on ASCII command names so it stays on char boundaries).
pub fn common_prefix(items: &[&str]) -> String {
    let Some(first) = items.first() else {
        return String::new();
    };
    let mut end = first.len();
    for s in &items[1..] {
        end = first
            .bytes()
            .zip(s.bytes())
            .take(end)
            .take_while(|(a, b)| a == b)
            .count();
    }
    first[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_drops_oldest_past_cap() {
        let mut r = Ring::new(2);
        r.push(1);
        r.push(2);
        r.push(3);
        assert_eq!(r.len(), 2);
        assert_eq!(r.get(0), Some(&2));
        assert_eq!(r.last(), Some(&3));
        assert_eq!(r.iter_rev().copied().collect::<Vec<_>>(), [3, 2]);
    }

    #[test]
    fn common_prefix_of_candidates() {
        assert_eq!(common_prefix(&["tp", "teleport"]), "t");
        assert_eq!(common_prefix(&["gfx"]), "gfx");
        assert_eq!(common_prefix(&["pos", "gfx"]), "");
    }

    #[test]
    fn the_edit_buffer_keeps_the_caret_on_char_boundaries_and_the_cap() {
        let mut b = EditBuf::with("héllo", 7);
        assert_eq!((b.text(), b.caret_chars()), ("héllo", 5));
        b.left();
        b.left();
        b.backspace();
        assert_eq!((b.text(), b.caret_chars()), ("hélo", 2));
        assert!(b.insert_char('x'));
        assert!(!b.insert_char('é'), "the byte cap holds");
        b.delete_word();
        assert_eq!(b.text(), "lo");
        b.end();
        b.delete_forward();
        assert_eq!(b.cursor(), 2);
    }

    #[test]
    fn a_text_input_submits_trimmed_lines_and_walks_history() {
        let mut t = TextInput::new(32);
        assert_eq!(t.handle(&[' ', 'h', 'i', ' '], Some(EditKey::Submit)), Some("hi".to_string()));
        t.handle(&['y', 'o'], Some(EditKey::Submit));
        t.handle(&['d', 'r'], None);
        t.handle(&[], Some(EditKey::HistoryUp));
        assert_eq!(t.text(), "yo");
        t.handle(&[], Some(EditKey::HistoryUp));
        assert_eq!(t.text(), "hi");
        t.handle(&[], Some(EditKey::HistoryDown));
        t.handle(&[], Some(EditKey::HistoryDown));
        assert_eq!(t.text(), "dr", "past the newest entry: the draft comes back");
        t.complete(|_| Completion::Ambiguous("dr".into(), vec!["drop".into(), "draw".into()]));
        assert_eq!(t.take_notice(), Some(vec!["drop".to_string(), "draw".to_string()]));
        assert_eq!(t.take_notice(), None);
        let mut done = TextInput::new(32).with_completer(|_| Completion::Full("/help".into()));
        done.handle(&['/'], Some(EditKey::Complete));
        assert_eq!(done.text(), "/help");
    }
}
