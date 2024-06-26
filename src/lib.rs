//! Multi-dimensional database implemented in Rust.

#![warn(missing_docs)]

mod range;

/// Module containing in-memory IO handlers for testing.
pub mod mem_io_handle;

mod macros;
mod world;

#[cfg(test)]
mod tests;

use std::ops::Deref;

use futures_lite::{AsyncRead, Future};

pub use world::{iter::Iter, iter::Lazy, Chunk, Chunks, Dim, Select, World};

#[doc(hidden)]
pub use futures_lite::StreamExt;

/// Represents types stored directly in a dimensional world.
pub trait Data: Sized + Send + Sync + Unpin {
    /// Count of dimensions.
    const DIMS: usize;

    /// The current version of this data type.
    const VERSION: u32;

    /// Gets the value of required dimension.
    ///
    /// Dimension index starts from 0, which should be
    /// a unique data such as the only id.
    fn dim(&self, dim: usize) -> u64;

    /// Decode this type from given `Read` and dimensional values,
    /// in the given data version.
    fn decode<B: bytes::Buf>(version: u32, dims: &[u64], buf: B) -> std::io::Result<Self>;

    /// Encode this type into bytes buffer, in
    /// latest data version.
    ///
    /// _To implementors: You don't need to encode dimensional values.
    /// They will be encoded automatically._
    fn encode<B: bytes::BufMut>(&self, buf: B) -> std::io::Result<()>;
}

const ARRAY_VERSION: u32 = 0;

impl<const DIMS: usize> Data for [u64; DIMS] {
    const DIMS: usize = DIMS;
    const VERSION: u32 = ARRAY_VERSION;

    #[inline]
    fn dim(&self, dim: usize) -> u64 {
        self[dim]
    }

    #[inline]
    fn decode<B: bytes::Buf>(_version: u32, dims: &[u64], _buf: B) -> std::io::Result<Self> {
        let mut this = [0; DIMS];
        this.copy_from_slice(dims);
        Ok(this)
    }

    #[inline]
    fn encode<B: bytes::BufMut>(&self, _buf: B) -> std::io::Result<()> {
        Ok(())
    }
}

/// Trait representing IO handlers for dimensional worlds.
pub trait IoHandle: Send + Sync {
    /// Type of reader.
    type Read<'a>: AsyncRead + Unpin + Send + Sync + 'a
    where
        Self: 'a;

    /// Hints if the chunk with given position is valid.
    ///
    /// If the chunk is hinted by valid, the world will
    /// load it from this handler.
    #[inline]
    fn hint_is_valid(&self, pos: &[usize]) -> bool {
        let _ = pos;
        true
    }

    /// Gets reader and data version for given chunk position.
    fn read_chunk<const DIMS: usize>(
        &self,
        pos: [usize; DIMS],
    ) -> impl Future<Output = std::io::Result<(u32, Self::Read<'_>)>> + Send + Sync;
}

impl<P, T> IoHandle for P
where
    T: IoHandle + 'static,
    P: Deref<Target = T> + Send + Sync,
{
    type Read<'a> = T::Read<'a> where Self: 'a;

    #[inline]
    fn hint_is_valid(&self, pos: &[usize]) -> bool {
        self.deref().hint_is_valid(pos)
    }

    #[inline]
    fn read_chunk<const DIMS: usize>(
        &self,
        pos: [usize; DIMS],
    ) -> impl Future<Output = std::io::Result<(u32, Self::Read<'_>)>> {
        self.deref().read_chunk(pos)
    }
}

/// Represents error variants produced by this crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// IO Error.
    #[error("io err: {0}")]
    Io(std::io::Error),
    /// Requesting value not found.
    #[error("requesting value not found")]
    ValueNotFound,
    /// Requesting value was moved to another chunk buffer.
    ///
    /// This is usually due to the target data was not suitable
    /// in the chunk after its modification.
    #[error("requesting value was moved to another chunk buffer")]
    ValueMoved,
    /// Given value out of range.
    #[error("value {value} out of range [{}, {}]", range.0, range.1)]
    ValueOutOfRange {
        /// The expected range.
        range: (u64, u64),
        /// The value.
        value: u64,
    },
}

/// Type alias for result produced by this crate.
type Result<T> = std::result::Result<T, Error>;
