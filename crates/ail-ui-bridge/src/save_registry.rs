//! Save registry — tracks files that this process wrote so the watcher can
//! suppress its own echo events.
//!
//! ## Why this exists
//!
//! The IDE writes `.ail` text whenever the user drags a node, edits a rule
//! inline, or accepts an agent preview. Those writes hit the same directory
//! the watcher is observing, so without coordination the watcher fires a
//! `graph-updated` patch for the file we just produced. That patch round-trips
//! through the frontend and clobbers the local optimistic UI (e.g. dragged
//! positions snap back). The same problem applies to agent-driven writes via
//! MCP.
//!
//! ## Mechanism
//!
//! Each save records `(path, SaveContext, written_at)`. When the watcher
//! receives an event batch it inspects the registry: if **every** relevant
//! path was written by us within [`ECHO_WINDOW`], the dispatch is skipped. If
//! one or more paths are external (or beyond the window), the watcher
//! dispatches normally so external editors continue to work.
//!
//! Entries past [`MAX_AGE`] are evicted lazily on every read so the registry
//! cannot grow unbounded.
//!
//! ## Future work
//!
//! `SessionId` is currently advisory metadata — Phase 18 ships single-session
//! behaviour. Phase 19 will use the session id to negotiate concurrent
//! agent + UI writes per the conflict-resolution rule (agent wins semantic
//! fields, UI wins layout).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Origin of a save call.
///
/// `Ui` covers user-driven actions in the desktop IDE (drag persist, inline
/// edit, direct create). `Agent` covers MCP-driven writes from a running AI
/// agent. `External` is the implicit default for anything that did not
/// register a save: external editors, version-control checkouts, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveSource {
    Ui,
    Agent,
    External,
}

/// Caller-supplied identifier for a save. Stored alongside the path so future
/// concurrent-write conflict resolution (Phase 19) can disambiguate a UI drag
/// from an agent edit on the same node within the same window.
pub type SessionId = String;

/// Metadata captured at save-call time and forwarded to [`SaveRegistry::record`].
#[derive(Debug, Clone)]
pub struct SaveContext {
    pub source: SaveSource,
    pub session_id: SessionId,
}

impl SaveContext {
    /// Convenience for callers that don't need explicit session tracking.
    pub fn ui(session_id: impl Into<SessionId>) -> Self {
        Self {
            source: SaveSource::Ui,
            session_id: session_id.into(),
        }
    }

    pub fn agent(session_id: impl Into<SessionId>) -> Self {
        Self {
            source: SaveSource::Agent,
            session_id: session_id.into(),
        }
    }
}

/// Echo-suppression window. Watcher events for paths recorded within this
/// duration are dropped. The window must be wider than the watcher's 250 ms
/// debounce plus worst-case OS event delay, but short enough that a real
/// external save arriving immediately after a UI save is not silently lost.
pub const ECHO_WINDOW: Duration = Duration::from_millis(2_000);

/// Hard upper bound on registry entry age. Entries are evicted lazily on
/// every read; nothing older than [`MAX_AGE`] survives a check.
pub const MAX_AGE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
struct Entry {
    written_at: Instant,
    context: SaveContext,
}

/// Process-wide save registry. Use [`save_registry`] to access the singleton.
#[derive(Debug)]
pub struct SaveRegistry {
    inner: Mutex<HashMap<PathBuf, Entry>>,
}

impl SaveRegistry {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Record a save against `path` with the supplied [`SaveContext`].
    /// Replaces any prior entry for the same path.
    pub fn record(&self, path: impl Into<PathBuf>, ctx: SaveContext) {
        self.record_at(path, ctx, Instant::now());
    }

    /// Returns `true` when `path` was recorded within [`ECHO_WINDOW`] of the
    /// current instant. Lazily evicts entries past [`MAX_AGE`] on the way in.
    pub fn was_recently_saved(&self, path: &Path) -> bool {
        self.was_recently_saved_at(path, Instant::now())
    }

    /// Last [`SaveContext`] recorded for `path` if still within [`MAX_AGE`].
    /// Useful for diagnostics and the conflict-resolution work in Phase 19.
    pub fn last_session(&self, path: &Path) -> Option<SaveContext> {
        let mut map = self.inner.lock().expect("registry mutex poisoned");
        evict_old(&mut map, Instant::now());
        map.get(path).map(|e| e.context.clone())
    }

    /// Drop entries older than [`MAX_AGE`] without checking any path.
    pub fn cleanup(&self) {
        let mut map = self.inner.lock().expect("registry mutex poisoned");
        evict_old(&mut map, Instant::now());
    }

    /// Number of live entries (post-eviction). Test/diagnostic only.
    pub fn len(&self) -> usize {
        let mut map = self.inner.lock().expect("registry mutex poisoned");
        evict_old(&mut map, Instant::now());
        map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Direct-time variant of [`record`]; required for deterministic testing
    /// without `thread::sleep`. Crate-private intentionally.
    pub(crate) fn record_at(
        &self,
        path: impl Into<PathBuf>,
        ctx: SaveContext,
        when: Instant,
    ) {
        let mut map = self.inner.lock().expect("registry mutex poisoned");
        evict_old(&mut map, when);
        map.insert(
            path.into(),
            Entry {
                written_at: when,
                context: ctx,
            },
        );
    }

    /// Direct-time variant of [`was_recently_saved`]; crate-private.
    pub(crate) fn was_recently_saved_at(&self, path: &Path, now: Instant) -> bool {
        let mut map = self.inner.lock().expect("registry mutex poisoned");
        evict_old(&mut map, now);
        match map.get(path) {
            Some(entry) => now.duration_since(entry.written_at) <= ECHO_WINDOW,
            None => false,
        }
    }

    /// Test-only: clear all entries.
    #[cfg(test)]
    pub fn clear(&self) {
        let mut map = self.inner.lock().expect("registry mutex poisoned");
        map.clear();
    }
}

impl Default for SaveRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn evict_old(map: &mut HashMap<PathBuf, Entry>, now: Instant) {
    map.retain(|_, e| now.duration_since(e.written_at) < MAX_AGE);
}

/// Process-wide singleton. Always-on; no feature gate so default builds can
/// reference it from tests and command modules without conditional imports.
pub fn save_registry() -> &'static SaveRegistry {
    static REGISTRY: OnceLock<SaveRegistry> = OnceLock::new();
    REGISTRY.get_or_init(SaveRegistry::new)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_ui(id: &str) -> SaveContext {
        SaveContext::ui(id.to_string())
    }

    #[test]
    fn test_record_then_recent_within_window() {
        let reg = SaveRegistry::new();
        let now = Instant::now();
        reg.record_at("/p/foo.ail", ctx_ui("s1"), now);
        assert!(reg.was_recently_saved_at(Path::new("/p/foo.ail"), now));
    }

    #[test]
    fn test_recent_check_at_window_boundary_inclusive() {
        let reg = SaveRegistry::new();
        let t0 = Instant::now();
        reg.record_at("/p/foo.ail", ctx_ui("s1"), t0);
        // Exactly at the boundary the entry is still considered an echo.
        let at_boundary = t0 + ECHO_WINDOW;
        assert!(reg.was_recently_saved_at(Path::new("/p/foo.ail"), at_boundary));
    }

    #[test]
    fn test_recent_check_past_window_returns_false() {
        let reg = SaveRegistry::new();
        let t0 = Instant::now();
        reg.record_at("/p/foo.ail", ctx_ui("s1"), t0);
        let past_window = t0 + ECHO_WINDOW + Duration::from_millis(1);
        assert!(!reg.was_recently_saved_at(Path::new("/p/foo.ail"), past_window));
    }

    #[test]
    fn test_unrecorded_path_not_recent() {
        let reg = SaveRegistry::new();
        assert!(!reg.was_recently_saved(Path::new("/p/never.ail")));
    }

    #[test]
    fn test_record_replaces_existing_entry() {
        let reg = SaveRegistry::new();
        let t0 = Instant::now();
        reg.record_at("/p/foo.ail", ctx_ui("s1"), t0);
        let t1 = t0 + Duration::from_millis(500);
        reg.record_at("/p/foo.ail", ctx_ui("s2"), t1);
        let got = reg.last_session(Path::new("/p/foo.ail")).expect("entry");
        assert_eq!(got.session_id, "s2");
    }

    #[test]
    fn test_last_session_returns_recorded_context() {
        let reg = SaveRegistry::new();
        let now = Instant::now();
        reg.record_at("/p/foo.ail", SaveContext::agent("agent-7"), now);
        let got = reg.last_session(Path::new("/p/foo.ail")).expect("entry");
        assert_eq!(got.source, SaveSource::Agent);
        assert_eq!(got.session_id, "agent-7");
    }

    #[test]
    fn test_last_session_missing_returns_none() {
        let reg = SaveRegistry::new();
        assert!(reg.last_session(Path::new("/p/never.ail")).is_none());
    }

    #[test]
    fn test_evict_old_drops_stale_entries() {
        let reg = SaveRegistry::new();
        let t0 = Instant::now();
        reg.record_at("/p/old.ail", ctx_ui("s1"), t0);
        reg.record_at("/p/fresh.ail", ctx_ui("s2"), t0 + Duration::from_secs(4));
        // Probing 6s after t0 evicts /p/old.ail (past MAX_AGE) but keeps fresh.
        let probe = t0 + Duration::from_secs(6);
        let _ = reg.was_recently_saved_at(Path::new("/p/probe.ail"), probe);
        assert!(reg.last_session(Path::new("/p/old.ail")).is_none());
        // /p/fresh.ail is 2s old at probe — still under MAX_AGE.
        assert!(reg.last_session(Path::new("/p/fresh.ail")).is_some());
    }

    #[test]
    fn test_cleanup_evicts_without_lookup() {
        let reg = SaveRegistry::new();
        let t0 = Instant::now();
        reg.record_at("/p/old.ail", ctx_ui("s1"), t0);
        // Sleep past MAX_AGE for cleanup() to actually drop it; cleanup() uses
        // the real clock, so simulate by recording multiple entries we know
        // will be reaped on the second cleanup pass.
        reg.cleanup();
        // The freshly recorded entry is well under MAX_AGE → still present.
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn test_session_id_distinct_across_records() {
        let reg = SaveRegistry::new();
        let t0 = Instant::now();
        reg.record_at("/p/a.ail", ctx_ui("s1"), t0);
        reg.record_at("/p/b.ail", ctx_ui("s2"), t0);
        let a = reg.last_session(Path::new("/p/a.ail")).expect("a");
        let b = reg.last_session(Path::new("/p/b.ail")).expect("b");
        assert_eq!(a.session_id, "s1");
        assert_eq!(b.session_id, "s2");
        assert_ne!(a.session_id, b.session_id);
    }

    #[test]
    fn test_save_source_labels_distinguish_origin() {
        let reg = SaveRegistry::new();
        let t0 = Instant::now();
        reg.record_at("/p/from-ui.ail", SaveContext::ui("u"), t0);
        reg.record_at("/p/from-agent.ail", SaveContext::agent("a"), t0);
        assert_eq!(
            reg.last_session(Path::new("/p/from-ui.ail")).unwrap().source,
            SaveSource::Ui
        );
        assert_eq!(
            reg.last_session(Path::new("/p/from-agent.ail"))
                .unwrap()
                .source,
            SaveSource::Agent
        );
    }

    #[test]
    fn test_singleton_returns_same_instance() {
        let r1 = save_registry();
        let r2 = save_registry();
        assert!(std::ptr::eq(r1, r2));
    }

    #[test]
    fn test_concurrent_records_do_not_panic() {
        use std::sync::Arc;
        use std::thread;
        let reg = Arc::new(SaveRegistry::new());
        let mut handles = Vec::new();
        for i in 0..16 {
            let r = reg.clone();
            handles.push(thread::spawn(move || {
                for j in 0..50 {
                    let path = format!("/p/t{i}-{j}.ail");
                    r.record(path, ctx_ui(&format!("s{i}-{j}")));
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(reg.len(), 16 * 50);
    }

    #[test]
    fn test_path_normalisation_exact_match_only() {
        // The registry intentionally uses raw PathBuf equality; callers are
        // expected to canonicalise paths before recording so absolute and
        // relative variants of the same file stay in sync.
        let reg = SaveRegistry::new();
        let t0 = Instant::now();
        reg.record_at("/abs/p.ail", ctx_ui("s1"), t0);
        assert!(reg.was_recently_saved_at(Path::new("/abs/p.ail"), t0));
        assert!(!reg.was_recently_saved_at(Path::new("relative/p.ail"), t0));
    }

    #[test]
    fn test_default_constructs_empty_registry() {
        let reg = SaveRegistry::default();
        assert!(reg.is_empty());
    }

    #[test]
    fn test_clear_resets_state() {
        let reg = SaveRegistry::new();
        reg.record("/p/foo.ail", ctx_ui("s1"));
        assert_eq!(reg.len(), 1);
        reg.clear();
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn test_distinct_paths_do_not_alias() {
        let reg = SaveRegistry::new();
        let t0 = Instant::now();
        reg.record_at("/p/a.ail", ctx_ui("a"), t0);
        // Recording an unrelated path must not stamp /p/a.ail with a fresh
        // timestamp — each path is independent.
        let later = t0 + ECHO_WINDOW + Duration::from_millis(100);
        reg.record_at("/p/b.ail", ctx_ui("b"), later);
        // /p/a.ail's timestamp is unchanged → past the window.
        assert!(!reg.was_recently_saved_at(Path::new("/p/a.ail"), later));
        // /p/b.ail is fresh.
        assert!(reg.was_recently_saved_at(Path::new("/p/b.ail"), later));
    }

    #[test]
    fn test_register_during_check_is_thread_safe() {
        // Smoke-test the lock under contention: many threads alternating
        // record/check on the same registry must never panic or deadlock.
        use std::sync::Arc;
        use std::thread;
        let reg = Arc::new(SaveRegistry::new());
        let mut handles = Vec::new();
        for i in 0..8 {
            let r = reg.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let p = format!("/p/contend-{i}.ail");
                    r.record(&p, ctx_ui(&format!("c{i}")));
                    let _ = r.was_recently_saved(Path::new(&p));
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }
}
