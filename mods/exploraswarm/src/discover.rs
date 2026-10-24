// SPDX-FileCopyrightText: 2026 Project Watt Cubed contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Swarm-wide memory of behaviour signatures, and the discovery log.
//!
//! `note` returns true the first time a signature is seen and queues a log line
//! `episode<TAB>agent<TAB>label`. The line is buffered until the next checkpoint. The map is a
//! `HashMap` at runtime; anything persisted or printed is sorted by key first, so the file does
//! not depend on hash order. A resumed run loads the map and does not announce those signatures again.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::rules::Discoveries as DiscoveriesTrait;

thread_local! {
    /// Which agent is evaluating on this thread. `note` has no agent argument, and a shared
    /// field would race when two episodes finish at once.
    static CTX: RefCell<(String, u64)> = const { RefCell::new((String::new(), 0)) };
}

#[derive(Clone, Debug)]
struct Found {
    count: u64,
    agent: String,
    episode: u64,
}

#[derive(Clone, Debug)]
struct Inner {
    map: HashMap<u64, Found>,
    log: Vec<String>,
}

/// Shared across agents. The mutex is the whole critical section of `count` / `note`.
#[derive(Debug)]
pub struct Discoveries {
    inner: Mutex<Inner>,
}

/// One persisted signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub key: u64,
    pub count: u64,
    pub episode: u64,
    pub agent: String,
}

impl Discoveries {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                map: HashMap::new(),
                log: Vec::new(),
            }),
        }
    }

    /// Who is about to be credited if this thread's episode notes something new.
    pub fn set_context(&self, agent: &str, episode: u64) {
        CTX.with(|ctx| *ctx.borrow_mut() = (agent.to_string(), episode));
    }

    pub fn len(&self) -> usize {
        lock(&self.inner).map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Lines queued since the last take, in the order they were first seen.
    pub fn take_log(&self) -> Vec<String> {
        std::mem::take(&mut lock(&self.inner).log)
    }

    /// Sorted by signature, for the checkpoint.
    pub fn export(&self) -> Vec<Record> {
        let g = lock(&self.inner);
        let mut keys: Vec<u64> = g.map.keys().copied().collect();
        keys.sort_unstable();
        keys.into_iter()
            .map(|key| {
                let f = &g.map[&key];
                Record {
                    key,
                    count: f.count,
                    episode: f.episode,
                    agent: f.agent.clone(),
                }
            })
            .collect()
    }

    pub fn import(&self, records: Vec<Record>) {
        let mut g = lock(&self.inner);
        g.map.clear();
        for r in records {
            g.map.insert(
                r.key,
                Found {
                    count: r.count,
                    agent: r.agent,
                    episode: r.episode,
                },
            );
        }
    }
}

impl Default for Discoveries {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscoveriesTrait for Discoveries {
    fn note(&self, signature: u64, label: &dyn Fn() -> String) -> bool {
        let (agent, episode) = CTX.with(|ctx| ctx.borrow().clone());
        let mut g = lock(&self.inner);
        if let Some(found) = g.map.get_mut(&signature) {
            found.count = found.count.saturating_add(1);
            return false;
        }
        let text = label();
        g.log.push(format!("{episode}\t{agent}\t{text}"));
        g.map.insert(
            signature,
            Found {
                count: 1,
                agent,
                episode,
            },
        );
        true
    }

    fn count(&self, signature: u64) -> u64 {
        lock(&self.inner)
            .map
            .get(&signature)
            .map(|f| f.count)
            .unwrap_or(0)
    }
}

fn lock(m: &Mutex<Inner>) -> std::sync::MutexGuard<'_, Inner> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_sight_is_logged_once_and_export_is_sorted() {
        let d = Discoveries::new();
        d.set_context("swarm01", 4);
        assert_eq!(d.count(9), 0);
        assert!(d.note(9, &|| "material c:9".to_string()));
        assert!(!d.note(9, &|| "again".to_string()));
        assert_eq!(d.count(9), 2);
        d.set_context("swarm02", 1);
        assert!(d.note(3, &|| "material c:3".to_string()));
        let log = d.take_log();
        assert_eq!(
            log,
            vec![
                "4\tswarm01\tmaterial c:9".to_string(),
                "1\tswarm02\tmaterial c:3".to_string()
            ]
        );
        assert!(d.take_log().is_empty());
        let exp = d.export();
        assert_eq!(exp[0].key, 3);
        assert_eq!(exp[1].key, 9);
        let again = Discoveries::new();
        again.import(exp);
        assert_eq!(again.count(9), 2);
        assert!(!again.note(9, &|| "nope".to_string()));
        assert!(
            again.take_log().is_empty(),
            "a resumed signature is not announced again"
        );
    }
}
