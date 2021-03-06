#![deny(rust_2018_idioms)]
#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]
#![allow(unused_variables)]
pub(crate) mod chunk;
pub mod column;
pub mod row_group;
pub(crate) mod table;

use std::collections::BTreeMap;

use arrow_deps::arrow::record_batch::RecordBatch;

use chunk::Chunk;
use column::AggregateType;
use row_group::{ColumnName, Predicate};

/// The `Store` is responsible for providing an execution engine for reading
/// `Chunk` data.
#[derive(Default)]
pub struct Store {
    // A mapping from database name (tenant id, bucket id etc) to a database.
    databases: BTreeMap<String, Database>,

    // The current total size of the store, in bytes
    size: u64,
}

impl Store {
    // TODO(edd): accept a configuration of some sort.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new database to the store
    pub fn add_database(&mut self, id: String, database: Database) {
        self.size += database.size();
        self.databases.insert(id, database);
    }

    /// Remove an entire database from the store.
    pub fn remove_database(&mut self, id: String) {
        todo!()
    }

    /// This method adds a `Chunk` to the Read Buffer. It is probably what
    /// the `MutableBuffer` will call.
    ///
    /// The chunk should comprise a single record batch for each table it
    /// contains.
    pub fn add_chunk(&mut self, database_id: String, chunk: BTreeMap<String, RecordBatch>) {
        todo!()
    }

    /// Executes selections against matching chunks, returning a single
    /// record batch with all chunk results appended.
    ///
    /// Results may be filtered by (currently only) equality predicates, but can
    /// be ranged by time, which should be represented as nanoseconds since the
    /// epoch. Results are included if they satisfy the predicate and fall
    /// with the [min, max) time range domain.
    pub fn select(
        &self,
        database_name: &str,
        table_name: &str,
        time_range: (i64, i64),
        predicates: &[Predicate<'_>],
        select_columns: Vec<String>,
    ) -> Option<RecordBatch> {
        // Execute against matching database.
        //
        // TODO(edd): error handling on everything...................
        //
        if let Some(db) = self.databases.get(database_name) {
            return db.select(table_name, time_range, predicates, select_columns);
        }
        None
    }

    /// Returns aggregates segmented by grouping keys for the specified
    /// measurement as record batches, with one record batch per matching
    /// chunk.
    ///
    /// The set of data to be aggregated may be filtered by (currently only)
    /// equality predicates, but can be ranged by time, which should be
    /// represented as nanoseconds since the epoch. Results are included if they
    /// satisfy the predicate and fall with the [min, max) time range domain.
    ///
    /// Group keys are determined according to the provided group column names.
    /// Currently only grouping by string (tag key) columns is supported.
    ///
    /// Required aggregates are specified via a tuple comprising a column name
    /// and the type of aggregation required. Multiple aggregations can be
    /// applied to the same column.
    pub fn aggregate(
        &self,
        database_name: &str,
        table_name: &str,
        time_range: (i64, i64),
        predicates: &[Predicate<'_>],
        group_columns: Vec<String>,
        aggregates: Vec<(ColumnName<'_>, AggregateType)>,
    ) -> Option<RecordBatch> {
        if let Some(db) = self.databases.get(database_name) {
            return db.aggregate(
                table_name,
                time_range,
                predicates,
                group_columns,
                aggregates,
            );
        }
        None
    }

    /// Returns aggregates segmented by grouping keys and windowed by time.
    ///
    /// The set of data to be aggregated may be filtered by (currently only)
    /// equality predicates, but can be ranged by time, which should be
    /// represented as nanoseconds since the epoch. Results are included if they
    /// satisfy the predicate and fall with the [min, max) time range domain.
    ///
    /// Group keys are determined according to the provided group column names
    /// (`group_columns`). Currently only grouping by string (tag key) columns
    /// is supported.
    ///
    /// Required aggregates are specified via a tuple comprising a column name
    /// and the type of aggregation required. Multiple aggregations can be
    /// applied to the same column.
    ///
    /// Results are grouped and windowed according to the `window` parameter,
    /// which represents an interval in nanoseconds. For example, to window
    /// results by one minute, window should be set to 600_000_000_000.
    pub fn aggregate_window(
        &self,
        database_name: &str,
        table_name: &str,
        time_range: (i64, i64),
        predicates: &[Predicate<'_>],
        group_columns: Vec<String>,
        aggregates: Vec<(ColumnName<'_>, AggregateType)>,
        window: i64,
    ) -> Option<RecordBatch> {
        if let Some(db) = self.databases.get(database_name) {
            return db.aggregate_window(
                table_name,
                time_range,
                predicates,
                group_columns,
                aggregates,
                window,
            );
        }
        None
    }

    //
    // ---- Schema API queries
    //

    /// Returns the distinct set of table names that contain data that satisfies
    /// the time range and predicates.
    pub fn table_names(
        &self,
        database_name: &str,
        time_range: (i64, i64),
        predicates: &[Predicate<'_>],
    ) -> Option<RecordBatch> {
        if let Some(db) = self.databases.get(database_name) {
            return db.table_names(database_name, time_range, predicates);
        }
        None
    }

    /// Returns the distinct set of tag keys (column names) matching the
    /// provided optional predicates and time range.
    pub fn tag_keys(
        &self,
        database_name: &str,
        table_name: &str,
        time_range: (i64, i64),
        predicates: &[Predicate<'_>],
    ) -> Option<RecordBatch> {
        if let Some(db) = self.databases.get(database_name) {
            return db.tag_keys(table_name, time_range, predicates);
        }
        None
    }

    /// Returns the distinct set of tag values (column values) for each provided
    /// tag key, where each returned value lives in a row matching the provided
    /// optional predicates and time range.
    ///
    /// As a special case, if `tag_keys` is empty then all distinct values for
    /// all columns (tag keys) are returned for the chunks.
    pub fn tag_values(
        &self,
        database_name: &str,
        table_name: &str,
        time_range: (i64, i64),
        predicates: &[Predicate<'_>],
        tag_keys: &[String],
    ) -> Option<RecordBatch> {
        if let Some(db) = self.databases.get(database_name) {
            return db.tag_values(table_name, time_range, predicates, tag_keys);
        }
        None
    }
}

/// Generate a predicate for the time range [from, to).
pub fn time_range_predicate<'a>(from: i64, to: i64) -> Vec<row_group::Predicate<'a>> {
    vec![
        (
            row_group::TIME_COLUMN_NAME,
            (
                column::cmp::Operator::GTE,
                column::Value::Scalar(column::Scalar::I64(from)),
            ),
        ),
        (
            row_group::TIME_COLUMN_NAME,
            (
                column::cmp::Operator::LT,
                column::Value::Scalar(column::Scalar::I64(to)),
            ),
        ),
    ]
}

// A database is scoped to a single tenant. Within a database there exists
// tables for measurements. There is a 1:1 mapping between a table and a
// measurement name.
#[derive(Default)]
pub struct Database {
    // The collection of chunks in the database. Each chunk is uniquely
    // identified by a chunk key.
    chunks: BTreeMap<String, Chunk>,

    // The current total size of the database.
    size: u64,
}

impl Database {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_chunk(&mut self, chunk: Chunk) {
        todo!()
    }

    pub fn remove_chunk(&mut self, chunk: Chunk) {
        todo!()
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    /// Executes selections against matching chunks, returning a single
    /// record batch with all chunk results appended.
    ///
    /// Results may be filtered by (currently only) equality predicates, but can
    /// be ranged by time, which should be represented as nanoseconds since the
    /// epoch. Results are included if they satisfy the predicate and fall
    /// with the [min, max) time range domain.
    pub fn select(
        &self,
        table_name: &str,
        time_range: (i64, i64),
        predicates: &[Predicate<'_>],
        select_columns: Vec<String>,
    ) -> Option<RecordBatch> {
        // Find all matching chunks using:
        //   - time range
        //   - measurement name.
        //
        // Execute against each chunk and append each result set into a
        // single record batch.
        todo!();
    }

    /// Returns aggregates segmented by grouping keys for the specified
    /// measurement as record batches, with one record batch per matching
    /// chunk.
    ///
    /// The set of data to be aggregated may be filtered by (currently only)
    /// equality predicates, but can be ranged by time, which should be
    /// represented as nanoseconds since the epoch. Results are included if they
    /// satisfy the predicate and fall with the [min, max) time range domain.
    ///
    /// Group keys are determined according to the provided group column names.
    /// Currently only grouping by string (tag key) columns is supported.
    ///
    /// Required aggregates are specified via a tuple comprising a column name
    /// and the type of aggregation required. Multiple aggregations can be
    /// applied to the same column.
    pub fn aggregate(
        &self,
        table_name: &str,
        time_range: (i64, i64),
        predicates: &[Predicate<'_>],
        group_columns: Vec<String>,
        aggregates: Vec<(ColumnName<'_>, AggregateType)>,
    ) -> Option<RecordBatch> {
        // Find all matching chunks using:
        //   - time range
        //   - measurement name.
        //
        // Execute query against each matching chunk and get result set.
        // For each result set it may be possible for there to be duplicate
        // group keys, e.g., due to back-filling. So chunk results may need
        // to be merged together with the aggregates from identical group keys
        // being resolved.
        //
        // Finally a record batch is returned.
        todo!()
    }

    /// Returns aggregates segmented by grouping keys and windowed by time.
    ///
    /// The set of data to be aggregated may be filtered by (currently only)
    /// equality predicates, but can be ranged by time, which should be
    /// represented as nanoseconds since the epoch. Results are included if they
    /// satisfy the predicate and fall with the [min, max) time range domain.
    ///
    /// Group keys are determined according to the provided group column names
    /// (`group_columns`). Currently only grouping by string (tag key) columns
    /// is supported.
    ///
    /// Required aggregates are specified via a tuple comprising a column name
    /// and the type of aggregation required. Multiple aggregations can be
    /// applied to the same column.
    ///
    /// Results are grouped and windowed according to the `window` parameter,
    /// which represents an interval in nanoseconds. For example, to window
    /// results by one minute, window should be set to 600_000_000_000.
    pub fn aggregate_window(
        &self,
        table_name: &str,
        time_range: (i64, i64),
        predicates: &[Predicate<'_>],
        group_columns: Vec<String>,
        aggregates: Vec<(ColumnName<'_>, AggregateType)>,
        window: i64,
    ) -> Option<RecordBatch> {
        // Find all matching chunks using:
        //   - time range
        //   - measurement name.
        //
        // Execute query against each matching chunk and get result set.
        // For each result set it may be possible for there to be duplicate
        // group keys, e.g., due to back-filling. So chunk results may need
        // to be merged together with the aggregates from identical group keys
        // being resolved.
        //
        // Finally a record batch is returned.
        todo!()
    }

    //
    // ---- Schema API queries
    //

    /// Returns the distinct set of table names that contain data that satisfies
    /// the time range and predicates.
    pub fn table_names(
        &self,
        database_name: &str,
        time_range: (i64, i64),
        predicates: &[Predicate<'_>],
    ) -> Option<RecordBatch> {
        //
        // TODO(edd): do we want to add the ability to apply a predicate to the
        // table names? For example, a regex where you only want table names
        // beginning with /cpu.+/ or something?
        todo!()
    }

    /// Returns the distinct set of tag keys (column names) matching the
    /// provided optional predicates and time range.
    pub fn tag_keys(
        &self,
        table_name: &str,
        time_range: (i64, i64),
        predicates: &[Predicate<'_>],
    ) -> Option<RecordBatch> {
        // Find all matching chunks using:
        //   - time range
        //   - measurement name.
        //
        // Execute query against matching chunks. The `tag_keys` method for
        // a chunk allows the caller to provide already found tag keys
        // (column names). This allows the execution to skip entire chunks,
        // tables or segments if there are no new columns to be found there...
        todo!();
    }

    /// Returns the distinct set of tag values (column values) for each provided
    /// tag key, where each returned value lives in a row matching the provided
    /// optional predicates and time range.
    ///
    /// As a special case, if `tag_keys` is empty then all distinct values for
    /// all columns (tag keys) are returned for the chunk.
    pub fn tag_values(
        &self,
        table_name: &str,
        time_range: (i64, i64),
        predicates: &[Predicate<'_>],
        tag_keys: &[String],
    ) -> Option<RecordBatch> {
        // Find the measurement name on the chunk and dispatch query to the
        // table for that measurement if the chunk's time range overlaps the
        // requested time range.
        todo!();
    }
}
