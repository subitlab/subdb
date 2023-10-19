use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    sync::{
        atomic::{self, AtomicUsize},
        Arc, OnceLock, Weak,
    },
    task::Poll,
};

use bytes::BufMut;
use futures_lite::{ready, stream::CountFuture, AsyncRead, Stream};
use pin_project_lite::pin_project;

use crate::{Data, IoHandle};

use super::{select::Shape, World};

enum ReadType<const DIMS: usize> {
    Mem([usize; DIMS]),
    Io(usize),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io err: {0}")]
    Io(futures_lite::io::Error),
    #[error("requiring value has been taken")]
    ValueTaken,
    #[error("requiring value not found")]
    ValueNotFound,
    #[error("depending stream updated.")]
    IterUpdated {
        expected: usize,
        current: Option<usize>,
    },
}

/// A type polls value lazily and immutably.
pub struct Lazy<'a, T: Data, const DIMS: usize, Io: IoHandle> {
    world: &'a World<T, DIMS, Io>,
    dims: [u64; DIMS],
    read_type: ReadType<DIMS>,
    value: OnceLock<Value<'a, T, DIMS>>,
    read: std::sync::Mutex<Option<Pin<&'a mut Io::Read>>>,

    state: LazyCheckState,
}

struct LazyCheckState {
    current: Weak<AtomicUsize>,
    expected: usize,
}

enum Value<'a, T: Data, const DIMS: usize> {
    Ref(super::Ref<'a, T, DIMS>),
    Direct(T),
}

impl<T: Data, const DIMS: usize, Io: IoHandle> Lazy<'_, T, DIMS, Io> {
    /// Gets info of dimensions of the value.
    #[inline]
    pub fn dims(&self) -> &[u64; DIMS] {
        &self.dims
    }

    /// Gets the value inside this initializer or initialize it
    /// if uninitialized.
    pub async fn get_or_init(&self) -> Result<&T, Error> {
        if let Some(value) = self.value.get() {
            return match value {
                Value::Ref(val) => Ok(&*val),
                Value::Direct(val) => Ok(val),
            };
        }

        match self.read_type {
            ReadType::Mem(chunk) => {
                let _ = self.value.set(Value::Ref(
                    self.world
                        .get(&chunk, self.dims[0])
                        .await
                        .ok_or(Error::ValueNotFound)?,
                ));

                Ok(if let Some(Value::Ref(val)) = self.value.get() {
                    &*val
                } else {
                    unreachable!()
                })
            }
            ReadType::Io(len) => {
                {
                    let current = self
                        .state
                        .current
                        .upgrade()
                        .map(|v| v.load(atomic::Ordering::Acquire));

                    if current != Some(self.state.expected) {
                        return Err(Error::IterUpdated {
                            expected: self.state.expected,
                            current,
                        });
                    }
                }

                let _ = self.value.set(Value::Direct(
                    FromBytes {
                        _world: self.world,
                        read: self.read.lock().unwrap().take().unwrap(),
                        dims: &self.dims,
                        len,
                        buf: None,
                    }
                    .await
                    .map_err(Error::Io)?,
                ));

                Ok(if let Some(Value::Direct(val)) = self.value.get() {
                    val
                } else {
                    unreachable!()
                })
            }
        }
    }
}

struct FromBytes<'a, T: Data, const DIMS: usize, Io: IoHandle> {
    _world: &'a World<T, DIMS, Io>,
    read: Pin<&'a mut Io::Read>,
    dims: &'a [u64; DIMS],
    len: usize,
    buf: Option<bytes::BytesMut>,
}

impl<T: Data, const DIMS: usize, Io: IoHandle> Future for FromBytes<'_, T, DIMS, Io> {
    type Output = futures_lite::io::Result<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;

        if let Some(ref mut buf) = this.buf {
            match ready!(this.read.as_mut().poll_read(cx, buf)) {
                Ok(act_len) => {
                    if act_len != this.len {
                        return Poll::Ready(Err(futures_lite::io::Error::new(
                            futures_lite::io::ErrorKind::UnexpectedEof,
                            format!("read {act_len} bytes, expected {} bytes", self.len),
                        )));
                    }
                }
                Err(err) => return Poll::Ready(Err(err)),
            }

            let Some(buf) = this.buf.take() else {
                unreachable!()
            };
            let buf = buf.freeze();
            Poll::Ready(T::decode(this.dims, buf))
        } else {
            let mut buf = bytes::BytesMut::with_capacity(this.len);
            buf.put_bytes(0, this.len);
            this.buf = Some(buf);
            Pin::new(this).poll(cx)
        }
    }
}

enum ChunkIter<'a, T: Data, const DIMS: usize, Io: IoHandle> {
    Pre(Pin<Box<dyn std::future::Future<Output = futures_lite::io::Result<Io::Read>> + Send + 'a>>),
    InProcess(Io::Read, &'a World<T, DIMS, Io>),
}

impl<T: Data, const DIMS: usize, Io: IoHandle> Stream for ChunkIter<'_, T, DIMS, Io> {
    type Item = ();

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.get_mut();
        todo!()
    }
}

pub struct Iter<'a, T: Data, const DIMS: usize, Io: IoHandle> {
    world: &'a World<T, DIMS, Io>,
    shape_iter: super::select::RawShapeIter<'a, DIMS>,
}

impl<T: Data, const DIMS: usize, Io: IoHandle> Stream for Iter<'_, T, DIMS, Io> {
    type Item = ();

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.get_mut();
        todo!()
    }
}
