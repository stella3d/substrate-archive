// Copyright 2017-2019 Parity Technologies (UK) Ltd.
// This file is part of substrate-archive.

// substrate-archive is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// substrate-archive is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with substrate-archive.  If not, see <http://www.gnu.org/licenses/>.

use crate::database::{models::StorageModel, Database, DbConn};
use crate::error::ArchiveResult;
use crate::queries;
use crate::types::*;
use sp_runtime::traits::{Block as BlockT, NumberFor};
use std::marker::PhantomData;
use xtra::prelude::*;

#[derive(Clone)]
pub struct DatabaseActor<B: BlockT> {
    db: Database,
    _marker: PhantomData<B>,
}

impl<B: BlockT> DatabaseActor<B> {
    pub async fn new(url: String) -> ArchiveResult<Self> {
        Ok(Self {
            db: Database::new(url).await?,
            _marker: PhantomData,
        })
    }

    #[allow(unused)]
    pub fn with_db(db: Database) -> Self {
        Self {
            db,
            _marker: PhantomData,
        }
    }

    async fn block_handler(&self, blk: Block<B>) -> ArchiveResult<()>
    where
        NumberFor<B>: Into<u32>,
    {
        let mut conn = self.db.conn().await?;
        while !queries::check_if_meta_exists(blk.spec, &mut conn).await? {
            timer::Delay::new(std::time::Duration::from_millis(20)).await;
        }
        std::mem::drop(conn);
        self.db.insert(blk).await?;
        Ok(())
    }

    // Returns true if all versions are in database
    // false if versions are missing
    async fn db_contains_metadata(blocks: &[Block<B>], conn: &mut DbConn) -> ArchiveResult<bool> {
        let specs: hashbrown::HashSet<u32> = blocks.iter().map(|b| b.spec).collect();
        let versions: hashbrown::HashSet<u32> =
            queries::get_versions(conn).await?.into_iter().collect();
        Ok(specs.is_subset(&versions))
    }

    async fn batch_block_handler(&self, blks: BatchBlock<B>) -> ArchiveResult<()>
    where
        NumberFor<B>: Into<u32>,
    {
        let mut conn = self.db.conn().await?;
        while !Self::db_contains_metadata(blks.inner(), &mut conn).await? {
            log::warn!("DB NOT CONTAIN META");
            timer::Delay::new(std::time::Duration::from_millis(50)).await;
        }
        std::mem::drop(conn);
        log::info!("Awaiting insert...");
        let len = blks.inner().len();
        self.db.insert(blks).await?;
        log::info!("Inserted {} blocks", len);
        Ok(())
    }

    async fn storage_handler(&self, storage: Storage<B>) -> ArchiveResult<()> {
        let mut conn = self.db.conn().await?;
        while !queries::contains_block::<B>(*storage.hash(), &mut conn).await? {
            timer::Delay::new(std::time::Duration::from_millis(10)).await;
        }
        let storage = Vec::<StorageModel<B>>::from(storage);
        std::mem::drop(conn);
        self.db.insert(storage).await?;
        Ok(())
    }

    async fn batch_storage_handler(&self, storage: Vec<Storage<B>>) -> ArchiveResult<()> {
        let mut conn = self.db.conn().await?;
        let block_nums: Vec<u32> = storage.iter().map(|s| s.block_num()).collect();
        log::trace!("Inserting: {:#?}", block_nums);
        let len = block_nums.len();
        while queries::contains_blocks::<B>(block_nums.as_slice(), &mut conn)
            .await?
            .len()
            != len
        {
            timer::Delay::new(std::time::Duration::from_millis(50)).await;
        }
        // we drop the connection early so that the insert() has the use of all db connections
        std::mem::drop(conn);
        let storage = Vec::<StorageModel<B>>::from(VecStorageWrap(storage));
        self.db.insert(storage).await?;
        Ok(())
    }
}

impl<B: BlockT> Actor for DatabaseActor<B> {}

#[async_trait::async_trait]
impl<B> Handler<Block<B>> for DatabaseActor<B>
where
    B: BlockT,
    NumberFor<B>: Into<u32>,
{
    async fn handle(&mut self, blk: Block<B>, _: &mut Context<Self>) {
        if let Err(e) = self.block_handler(blk).await {
            log::error!("{}", e.to_string())
        }
    }
}

#[async_trait::async_trait]
impl<B> Handler<BatchBlock<B>> for DatabaseActor<B>
where
    B: BlockT,
    NumberFor<B>: Into<u32>,
{
    async fn handle(&mut self, blks: BatchBlock<B>, _: &mut Context<Self>) {
        let len = blks.inner.len();
        let now = std::time::Instant::now();
        if let Err(e) = self.batch_block_handler(blks).await {
            log::error!("{}", e.to_string());
        }
        log::debug!("TOOK {:?} to insert {} blocks", now.elapsed(), len);
    }
}

#[async_trait::async_trait]
impl<B: BlockT> Handler<Metadata> for DatabaseActor<B> {
    async fn handle(&mut self, meta: Metadata, _ctx: &mut Context<Self>) {
        if let Err(e) = self.db.insert(meta).await {
            log::error!("{}", e.to_string());
        }
    }
}

#[async_trait::async_trait]
impl<B: BlockT> Handler<Storage<B>> for DatabaseActor<B> {
    async fn handle(&mut self, storage: Storage<B>, _ctx: &mut Context<Self>) {
        if let Err(e) = self.storage_handler(storage).await {
            log::error!("{}", e.to_string())
        }
    }
}
pub struct VecStorageWrap<B: BlockT>(pub Vec<Storage<B>>);

impl<B: BlockT> Message for VecStorageWrap<B> {
    type Result = ();
}

#[async_trait::async_trait]
impl<B: BlockT> Handler<VecStorageWrap<B>> for DatabaseActor<B> {
    async fn handle(&mut self, storage: VecStorageWrap<B>, _ctx: &mut Context<Self>) {
        let now = std::time::Instant::now();
        if let Err(e) = self.batch_storage_handler(storage.0).await {
            log::error!("{}", e.to_string());
        }
        log::debug!("took {:?} to insert storage", now.elapsed());
    }
}

// this is an enum in case there is some more state
// that might be needed in the future
/// Get Some State from the Database Actor
pub enum GetState {
    Conn,
}

/// A resposne to `GetState`
/// it is callers responsiblity to make sure to call the
/// correct method on the implement after receiving the message
pub enum StateResponse {
    Conn(DbConn),
}

impl StateResponse {
    /// Pull a connection out of the enum
    ///
    /// # Panics
    /// panics if the enum is not actually of the `Conn` type
    pub fn conn(self) -> DbConn {
        match self {
            StateResponse::Conn(v) => v,
        }
    }
}

impl Message for GetState {
    type Result = ArchiveResult<StateResponse>;
}

#[async_trait::async_trait]
impl<B: BlockT> Handler<GetState> for DatabaseActor<B> {
    async fn handle(
        &mut self,
        msg: GetState,
        _: &mut Context<Self>,
    ) -> ArchiveResult<StateResponse> {
        match msg {
            GetState::Conn => {
                let conn = self.db.conn().await?;
                Ok(StateResponse::Conn(conn))
            }
        }
    }
}
