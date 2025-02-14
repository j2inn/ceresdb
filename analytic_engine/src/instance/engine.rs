// Copyright 2022-2023 CeresDB Project Authors. Licensed under Apache-2.0.

//! Table engine logic of instance

use std::sync::Arc;

use common_types::schema::Version;
use common_util::{define_result, error::GenericError};
use snafu::{Backtrace, OptionExt, Snafu};
use table_engine::{
    engine::{CloseTableRequest, CreateTableRequest, DropTableRequest, OpenTableRequest},
    table::TableId,
};
use wal::manager::WalLocation;

use crate::{
    instance::{close::Closer, drop::Dropper, Instance},
    space::{Space, SpaceAndTable, SpaceContext, SpaceId, SpaceRef},
};

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display(
        "The space of the table does not exist, space_id:{}, table:{}.\nBacktrace:\n{}",
        space_id,
        table,
        backtrace,
    ))]
    SpaceNotExist {
        space_id: SpaceId,
        table: String,
        backtrace: Backtrace,
    },

    #[snafu(display("Failed to read meta update, table_id:{}, err:{}", table_id, source))]
    ReadMetaUpdate {
        table_id: TableId,
        source: GenericError,
    },

    #[snafu(display(
        "Failed to recover table data, space_id:{}, table:{}, err:{}",
        space_id,
        table,
        source
    ))]
    RecoverTableData {
        space_id: SpaceId,
        table: String,
        source: crate::table::data::Error,
    },

    #[snafu(display("Failed to read wal, err:{}", source))]
    ReadWal { source: wal::manager::Error },

    #[snafu(display(
        "Failed to apply log entry to memtable, table:{}, table_id:{}, err:{}",
        table,
        table_id,
        source
    ))]
    ApplyMemTable {
        space_id: SpaceId,
        table: String,
        table_id: TableId,
        source: crate::instance::write::Error,
    },

    #[snafu(display(
        "Flush failed, space_id:{}, table:{}, table_id:{}, err:{}",
        space_id,
        table,
        table_id,
        source
    ))]
    FlushTable {
        space_id: SpaceId,
        table: String,
        table_id: TableId,
        source: crate::instance::flush_compaction::Error,
    },

    #[snafu(display(
        "Failed to persist meta update to manifest, space_id:{}, table:{}, table_id:{}, err:{}",
        space_id,
        table,
        table_id,
        source
    ))]
    WriteManifest {
        space_id: SpaceId,
        table: String,
        table_id: TableId,
        source: GenericError,
    },

    #[snafu(display(
        "Failed to persist meta update to WAL, space_id:{}, table:{}, table_id:{}, err:{}",
        space_id,
        table,
        table_id,
        source
    ))]
    WriteWal {
        space_id: SpaceId,
        table: String,
        table_id: TableId,
        source: GenericError,
    },

    #[snafu(display(
        "Invalid options, space_id:{}, table:{}, table_id:{}, err:{}",
        space_id,
        table,
        table_id,
        source
    ))]
    InvalidOptions {
        space_id: SpaceId,
        table: String,
        table_id: TableId,
        source: GenericError,
    },

    #[snafu(display(
        "Failed to create table data, space_id:{}, table:{}, table_id:{}, err:{}",
        space_id,
        table,
        table_id,
        source
    ))]
    CreateTableData {
        space_id: SpaceId,
        table: String,
        table_id: TableId,
        source: crate::table::data::Error,
    },

    #[snafu(display(
    "Try to update schema to elder version, table:{}, current_version:{}, given_version:{}.\nBacktrace:\n{}",
    table,
    current_version,
    given_version,
    backtrace,
    ))]
    InvalidSchemaVersion {
        table: String,
        current_version: Version,
        given_version: Version,
        backtrace: Backtrace,
    },

    #[snafu(display(
    "Invalid previous schema version, table:{}, current_version:{}, pre_version:{}.\nBacktrace:\n{}",
    table,
    current_version,
    pre_version,
    backtrace,
    ))]
    InvalidPreVersion {
        table: String,
        current_version: Version,
        pre_version: Version,
        backtrace: Backtrace,
    },

    #[snafu(display(
        "Alter schema of a dropped table:{}.\nBacktrace:\n{}",
        table,
        backtrace
    ))]
    AlterDroppedTable { table: String, backtrace: Backtrace },

    #[snafu(display("Failed to store version edit, err:{}", source))]
    StoreVersionEdit { source: GenericError },

    #[snafu(display(
        "Failed to encode payloads, table:{}, wal_location:{:?}, err:{}",
        table,
        wal_location,
        source
    ))]
    EncodePayloads {
        table: String,
        wal_location: WalLocation,
        source: wal::manager::Error,
    },

    #[snafu(display(
        "Failed to do manifest snapshot for table, space_id:{}, table:{}, err:{}",
        space_id,
        table,
        source
    ))]
    DoManifestSnapshot {
        space_id: SpaceId,
        table: String,
        source: GenericError,
    },

    #[snafu(display(
        "Table open failed and can not be created again, table:{}.\nBacktrace:\n{}",
        table,
        backtrace,
    ))]
    CreateOpenFailedTable { table: String, backtrace: Backtrace },

    #[snafu(display("Failed to open manifest, err:{}", source))]
    OpenManifest {
        source: crate::manifest::details::Error,
    },

    #[snafu(display("Failed to find table, msg:{}.\nBacktrace:\n{}", msg, backtrace))]
    TableNotExist { msg: String, backtrace: Backtrace },
}

define_result!(Error);

impl From<Error> for table_engine::engine::Error {
    fn from(err: Error) -> Self {
        match &err {
            Error::InvalidOptions { table, .. } | Error::SpaceNotExist { table, .. } => {
                Self::InvalidArguments {
                    table: table.clone(),
                    source: Box::new(err),
                }
            }
            Error::WriteManifest { .. } => Self::WriteMeta {
                source: Box::new(err),
            },
            Error::WriteWal { .. }
            | Error::InvalidSchemaVersion { .. }
            | Error::InvalidPreVersion { .. }
            | Error::CreateTableData { .. }
            | Error::AlterDroppedTable { .. }
            | Error::ReadMetaUpdate { .. }
            | Error::RecoverTableData { .. }
            | Error::ReadWal { .. }
            | Error::ApplyMemTable { .. }
            | Error::FlushTable { .. }
            | Error::StoreVersionEdit { .. }
            | Error::EncodePayloads { .. }
            | Error::CreateOpenFailedTable { .. }
            | Error::DoManifestSnapshot { .. }
            | Error::OpenManifest { .. }
            | Error::TableNotExist { .. } => Self::Unexpected {
                source: Box::new(err),
            },
        }
    }
}

impl Instance {
    /// Find space by name, create if the space is not exists
    pub async fn find_or_create_space(
        self: &Arc<Self>,
        space_id: SpaceId,
        context: SpaceContext,
    ) -> Result<SpaceRef> {
        // Find space first
        if let Some(space) = self.get_space_by_read_lock(space_id) {
            return Ok(space);
        }

        let mut spaces = self.space_store.spaces.write().unwrap();
        // The space may already been created by other thread
        if let Some(space) = spaces.get_by_id(space_id) {
            return Ok(space.clone());
        }

        // Now we are the one responsible to create and persist the space info into meta

        // Create space
        let space = Arc::new(Space::new(
            space_id,
            context,
            self.space_write_buffer_size,
            self.mem_usage_collector.clone(),
        ));

        spaces.insert(space.clone());

        Ok(space)
    }

    /// Find space by id
    pub fn find_space(&self, space_id: SpaceId) -> Option<SpaceRef> {
        let spaces = self.space_store.spaces.read().unwrap();
        spaces.get_by_id(space_id).cloned()
    }

    /// Create a table under given space
    pub async fn create_table(
        self: &Arc<Self>,
        space_id: SpaceId,
        request: CreateTableRequest,
    ) -> Result<SpaceAndTable> {
        let context = SpaceContext {
            catalog_name: request.catalog_name.clone(),
            schema_name: request.schema_name.clone(),
        };
        let space = self.find_or_create_space(space_id, context).await?;
        let table_data = self.do_create_table(space.clone(), request).await?;

        Ok(SpaceAndTable::new(space, table_data))
    }

    /// Drop a table under given space
    /// Find the table under given space by its table name
    ///
    /// Return None if space or table is not found
    pub async fn find_table(
        &self,
        space_id: SpaceId,
        table: &str,
    ) -> Result<Option<SpaceAndTable>> {
        let space = match self.find_space(space_id) {
            Some(s) => s,
            None => return Ok(None),
        };

        let space_table = space
            .find_table(table)
            .map(|table_data| SpaceAndTable::new(space, table_data));

        Ok(space_table)
    }

    /// Find the table under given space by its table name
    ///
    /// Return None if space or table is not found
    pub async fn open_table(
        self: &Arc<Self>,
        space_id: SpaceId,
        request: &OpenTableRequest,
    ) -> Result<Option<SpaceAndTable>> {
        let context = SpaceContext {
            catalog_name: request.catalog_name.clone(),
            schema_name: request.schema_name.clone(),
        };
        let space = self.find_or_create_space(space_id, context).await?;

        let table_data = self.do_open_table(space.clone(), request).await?;

        Ok(table_data.map(|v| SpaceAndTable::new(space, v)))
    }

    /// Drop a table under given space
    pub async fn drop_table(
        self: &Arc<Self>,
        space_id: SpaceId,
        request: DropTableRequest,
    ) -> Result<bool> {
        let space = self.find_space(space_id).context(SpaceNotExist {
            space_id,
            table: &request.table_name,
        })?;
        let dropper = Dropper {
            space,
            space_store: self.space_store.clone(),
            flusher: self.make_flusher(),
        };

        dropper.drop(request).await
    }

    /// Close the table under given space by its table name
    pub async fn close_table(
        self: &Arc<Self>,
        space_id: SpaceId,
        request: CloseTableRequest,
    ) -> Result<()> {
        let space = self.find_space(space_id).context(SpaceNotExist {
            space_id,
            table: &request.table_name,
        })?;

        let closer = Closer {
            space,
            manifest: self.space_store.manifest.clone(),
            flusher: self.make_flusher(),
        };

        closer.close(request).await
    }
}
