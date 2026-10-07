//! The local copy of an upstream catalog, for searching without asking it.
//!
//! Two tables. `mcp_registry_index` holds one row per server: the upstream's
//! own record, plus a lowercased label and description to match a query
//! against. `mcp_registry_index_state` holds one row per catalog: which base URL
//! the rows came from, where an unfinished sync stopped, and when the last sync
//! finished.
//!
//! # A sync is resumable
//!
//! Each page is written together with the cursor for the next one, in one
//! transaction. A sync that stops part way — a timeout, a restart — leaves the
//! rows it already wrote and a cursor to resume from. Rows from an earlier sync
//! stay until a sync finishes, at which point any server it did not see again
//! is removed.

use std::fmt::Write as _;

use rusqlite::{OptionalExtension as _, params, params_from_iter};

use super::types::{Store, now_ms};
use crate::error::{Error, Result};

/// One server, ready to be written to the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexRow {
    /// The registry's qualified name.
    pub(crate) qualified_name: String,
    /// The name and title, lowercased, for matching.
    pub(crate) label: String,
    /// The description, lowercased, for matching.
    pub(crate) description: String,
    /// The upstream's record for the server, as JSON.
    pub(crate) record_json: String,
}

/// One server an index search matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexHit {
    /// The registry's qualified name.
    pub(crate) qualified_name: String,
    /// Whether every word matched the name or title.
    pub(crate) in_label: bool,
    /// The upstream's record for the server, as JSON.
    pub(crate) record_json: String,
}

/// Where one catalog's index stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexState {
    /// The base URL the rows came from.
    pub(crate) base_url: String,
    /// The cursor an unfinished sync resumes from; `None` for its first page.
    pub(crate) cursor: Option<String>,
    /// How many pages the unfinished sync has written.
    pub(crate) pages: u32,
    /// When the unfinished sync started, in Unix epoch milliseconds; `None`
    /// when no sync is unfinished.
    pub(crate) started_at: Option<i64>,
    /// When the last sync finished, in Unix epoch milliseconds; `None` when
    /// none has.
    pub(crate) synced_at: Option<i64>,
}

impl IndexState {
    /// Whether a sync started and has not finished.
    pub(crate) const fn in_progress(&self) -> bool {
        self.started_at.is_some()
    }
}

impl Store {
    /// Where `source`'s index stands, or `None` when no sync has begun.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Store`] when the query fails.
    pub(crate) fn index_state(&self, source: &str) -> Result<Option<IndexState>> {
        self.connection
            .lock()
            .query_row(
                "SELECT base_url, cursor, pages, started_at, synced_at
                 FROM mcp_registry_index_state WHERE source = ?1",
                params![source],
                |row| {
                    Ok(IndexState {
                        base_url: row.get(0)?,
                        cursor: row.get(1)?,
                        pages: row.get(2)?,
                        started_at: row.get(3)?,
                        synced_at: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(|source| Error::store("reading the index state", source))
    }

    /// Starts a sync of `source` from its first page.
    ///
    /// Rows from a different base URL are dropped, since they describe a
    /// different catalog; rows from the same one stay searchable until the
    /// sync finishes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Store`] when a statement fails.
    pub(crate) fn begin_index_sync(&self, source: &str, base_url: &str) -> Result<()> {
        let mut connection = self.connection.lock();
        let transaction = connection
            .transaction()
            .map_err(|source| Error::store("beginning an index sync", source))?;

        let previous_base: Option<String> = transaction
            .query_row(
                "SELECT base_url FROM mcp_registry_index_state WHERE source = ?1",
                params![source],
                |row| row.get(0),
            )
            .optional()
            .map_err(|source| Error::store("reading the index state", source))?;
        let same_catalog = previous_base.as_deref() == Some(base_url);

        if !same_catalog {
            transaction
                .execute(
                    "DELETE FROM mcp_registry_index WHERE source = ?1",
                    params![source],
                )
                .map_err(|source| Error::store("clearing the index", source))?;
        }

        transaction
            .execute(
                "INSERT INTO mcp_registry_index_state
                     (source, base_url, cursor, pages, started_at, synced_at)
                 VALUES (?1, ?2, NULL, 0, ?3, NULL)
                 ON CONFLICT(source) DO UPDATE SET
                     base_url = excluded.base_url,
                     cursor = NULL,
                     pages = 0,
                     started_at = excluded.started_at,
                     synced_at = CASE WHEN ?4 THEN synced_at ELSE NULL END",
                params![source, base_url, now_ms(), same_catalog],
            )
            .map_err(|source| Error::store("recording an index sync", source))?;

        transaction
            .commit()
            .map_err(|source| Error::store("committing an index sync start", source))
    }

    /// Writes one synced page of `source` and the cursor for the next.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Store`] when a statement fails; nothing from the page
    /// is written then.
    pub(crate) fn store_index_page(
        &self,
        source: &str,
        rows: &[IndexRow],
        next_cursor: Option<&str>,
    ) -> Result<()> {
        let now = now_ms();
        let mut connection = self.connection.lock();
        let transaction = connection
            .transaction()
            .map_err(|source| Error::store("beginning an index page", source))?;

        for row in rows {
            transaction
                .execute(
                    "INSERT OR REPLACE INTO mcp_registry_index
                         (source, qualified_name, label, description, record_json, synced_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        source,
                        row.qualified_name,
                        row.label,
                        row.description,
                        row.record_json,
                        now
                    ],
                )
                .map_err(|source| Error::store("writing an index row", source))?;
        }

        transaction
            .execute(
                "UPDATE mcp_registry_index_state SET cursor = ?2, pages = pages + 1
                 WHERE source = ?1",
                params![source, next_cursor],
            )
            .map_err(|source| Error::store("recording an index page", source))?;

        transaction
            .commit()
            .map_err(|source| Error::store("committing an index page", source))
    }

    /// Finishes the sync of `source`, dropping every server it did not see.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Store`] when a statement fails.
    pub(crate) fn finish_index_sync(&self, source: &str) -> Result<()> {
        let mut connection = self.connection.lock();
        let transaction = connection
            .transaction()
            .map_err(|source| Error::store("finishing an index sync", source))?;

        transaction
            .execute(
                "DELETE FROM mcp_registry_index WHERE source = ?1 AND synced_at < (
                     SELECT started_at FROM mcp_registry_index_state WHERE source = ?1
                 )",
                params![source],
            )
            .map_err(|source| Error::store("pruning the index", source))?;
        transaction
            .execute(
                "UPDATE mcp_registry_index_state
                 SET cursor = NULL, pages = 0, started_at = NULL, synced_at = ?2
                 WHERE source = ?1",
                params![source, now_ms()],
            )
            .map_err(|source| Error::store("recording a finished index sync", source))?;

        transaction
            .commit()
            .map_err(|source| Error::store("committing a finished index sync", source))
    }

    /// Every indexed server of `source` matching each of `terms` in its label
    /// or description.
    ///
    /// The terms are matched as given, so a caller lowercases them to match
    /// the stored columns.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Store`] when the query fails.
    pub(crate) fn search_index(&self, source: &str, terms: &[String]) -> Result<Vec<IndexHit>> {
        let mut sql = String::from(
            "SELECT qualified_name, record_json, label FROM mcp_registry_index WHERE source = ?1",
        );
        for position in 2..terms.len() + 2 {
            // Writing into the buffer cannot fail.
            let _ = write!(
                sql,
                " AND (instr(label, ?{position}) > 0 OR instr(description, ?{position}) > 0)"
            );
        }

        let connection = self.connection.lock();
        let mut statement = connection
            .prepare(&sql)
            .map_err(|source| Error::store("preparing an index search", source))?;
        let values = std::iter::once(source).chain(terms.iter().map(String::as_str));

        statement
            .query_map(params_from_iter(values), |row| {
                let label: String = row.get(2)?;
                Ok(IndexHit {
                    qualified_name: row.get(0)?,
                    in_label: terms.iter().all(|term| label.contains(term.as_str())),
                    record_json: row.get(1)?,
                })
            })
            .and_then(Iterator::collect)
            .map_err(|source| Error::store("searching the index", source))
    }
}

#[cfg(test)]
#[path = "index_tests.rs"]
mod tests;
