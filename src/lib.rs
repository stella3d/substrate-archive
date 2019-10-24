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

#[macro_use] extern crate diesel;
mod rpc;
mod types;
mod error;
mod archive;
mod database;
mod queries;
mod util;
#[cfg(test)]
mod tests;
mod srml_ext;

pub use archive::Archive;

pub use types::{System, Module, ExtractCall};
pub use srml_ext::{SrmlExt, NotHandled};

pub mod srml {
    pub use srml_system;
    pub use srml_timestamp::Call as TimestampCall;
    pub use srml_finality_tracker::Call as FinalityCall;
    pub use srml_im_online::Call as ImOnlineCall;
}
