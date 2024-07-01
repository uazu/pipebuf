use super::{PBufState, PBufTrip, PipeBuf};

#[cfg(feature = "std")]
use std::io::{ErrorKind, Read};

/// Producer reference to a [`PipeBuf`]
///
/// Obtain this reference using [`PipeBuf::wr`].  This is a mutable
/// reference to a [`PipeBuf`] that exposes the calls that a producer
/// is allowed to use.  It acts just like a `&mut PipeBuf`, and has
/// the same size and efficiency.  However unlike a `&mut` reference,
/// reborrowing doesn't happen automatically, but it can still be done
/// just as efficiently using [`PBufWr::reborrow`].
pub struct PBufWr<'a, T: 'static = u8> {
    pub(crate) pb: &'a mut PipeBuf<T>,
}

impl<'a, T: Copy + Default + 'static> PBufWr<'a, T> {
    /// Create a new reference from this one, reborrowing it.  Thanks
    /// to the borrow checker, the original reference will be
    /// inaccessible until the returned reference's lifetime ends.
    /// The cost is just a pointer copy, just as for automatic `&mut`
    /// reborrowing.
    #[inline(always)]
    pub fn reborrow<'b, 'r>(&'r mut self) -> PBufWr<'b, T>
    where
        'a: 'b,
        'r: 'b,
    {
        PBufWr { pb: &mut *self.pb }
    }

    /// Obtain a tripwire value to detect buffer changes.  See the
    /// [`PBufTrip`] type for further explanation.
    #[inline]
    pub fn tripwire(&self) -> PBufTrip {
        self.pb.tripwire()
    }

    /// Test whether there has been a change to the buffer since the
    /// tripwire value provided was obtained.  See [`PBufTrip`].
    #[inline]
    pub fn is_tripped(&self, trip: PBufTrip) -> bool {
        self.tripwire() != trip
    }

    /// Get a reference to a mutable slice of exactly `reserve` bytes
    /// of free space where new data may be written.  Compacts the
    /// buffer only if necessary.  Once written, the data must be
    /// committed immediately using [`PBufWr::commit`], before any
    /// other operation that might compact the buffer.
    ///
    /// Note that for efficiency the free space will not be
    /// initialised to zeros.  It will contain some jumble of bytes
    /// previously written to the pipe.  You must not make any
    /// assumptions about this data.
    ///
    /// Returns `None` if there is not enough free space in the output
    /// buffer, even if the buffer were to be compacted or
    /// reallocated.  There are two possibilities:
    ///
    /// - The buffer is too full of other data, and you need to return
    /// and wait to be called again later on after some data has been
    /// consumed by the next component in the chain
    ///
    /// - The amount of space you're asking for is larger than the
    /// maximum capacity of the buffer, in which case this request can
    /// never succeed.  You can check this by comparing to
    /// [`PBufWr::capacity`].  If the buffer maximum capacity is too
    /// low for writing a given packet or other unit of data, you
    /// might need to abort the operation, or abort this packet at the
    /// protocol level, or report an error somewhere.
    #[inline]
    #[track_caller]
    pub fn space(&mut self, reserve: usize) -> Option<&mut [T]> {
        if self.pb.wr + reserve > self.pb.data.len() && !self.try_make_space(reserve) {
            None
        } else {
            Some(&mut self.pb.data[self.pb.wr..self.pb.wr + reserve])
        }
    }

    /// Get a reference to up to `limit` bytes of free space where new
    /// data may be written, or less if the buffer is too full.
    /// Compacts the buffer if the space is available but not
    /// contiguous.  If there is no free space at all then returns an
    /// empty slice.  If you don't want an empty slice, then check
    /// [`PBufWr::is_full`] first.
    ///
    /// This may be useful if you have `limit` bytes to write, but you
    /// can just as easily write less, for example if it is just a
    /// byte-stream and isn't being handled internally as an
    /// indivisible unit (e.g. a packet or line).
    ///
    /// Note that for efficiency the free space will not be
    /// initialised to zeros.  It will contain some jumble of bytes
    /// previously written to the pipe.  You must not make any
    /// assumptions about this data.
    #[inline]
    #[track_caller]
    pub fn space_upto(&mut self, limit: usize) -> &mut [T] {
        let limit = limit.min(self.free());
        if self.pb.wr + limit > self.pb.data.len() {
            let _ = self.try_make_space(limit);
        }
        &mut self.pb.data[self.pb.wr..self.pb.wr + limit]
    }

    /// Get a reference to all the remaining free space in the buffer
    /// where new data may be written.  This forces the buffer to be
    /// fully allocated if it is not yet at its maximum capacity.  If
    /// there is already data in the buffer, then compacts the buffer
    /// first to maximize the free space.  If there is no free space
    /// at all then returns an empty slice.  If you don't want an
    /// empty slice, then check [`PBufWr::is_full`] first.
    ///
    /// Note that for efficiency the free space will not be
    /// initialised to zeros.  It will contain some jumble of bytes
    /// previously written to the pipe.  You must not make any
    /// assumptions about this data.
    #[inline]
    #[track_caller]
    pub fn space_all(&mut self) -> &mut [T] {
        let reserve = self.free();
        if self.pb.wr + reserve > self.pb.data.len() {
            let _ = self.try_make_space(reserve);
        }

        &mut self.pb.data[self.pb.wr..]
    }

    /// `try_make_space` is "cold" and not inlined into the caller's
    /// code as it is expected to be called less frequently.  This is
    /// done to keep the actual inlined code small and efficient.
    ///
    /// Returns `true`: successfully made space, `false`: not enough
    /// space available.
    #[inline(never)]
    #[cold]
    #[track_caller]
    fn try_make_space(&mut self, _reserve: usize) -> bool {
        // Guaranteed that if .rd == .wr, then now both .rd and .wr
        // will be zero, so if .rd > 0 then there is something to copy
        // down
        debug_assert!(self.pb.rd != self.pb.wr || self.pb.rd == 0);
        if self.pb.rd > 0 {
            self.pb.data.copy_within(self.pb.rd..self.pb.wr, 0);
            self.pb.wr -= self.pb.rd;
            self.pb.rd = 0;
        }

        #[cfg(any(feature = "std", feature = "alloc"))]
        if self.pb.wr + _reserve > self.pb.data.len() {
            if self.pb.data.len() >= self.pb.max_capacity {
                return false;
            }
            let req_len = (self.pb.wr + _reserve)
                .max(_reserve * 2)
                .min(self.pb.max_capacity);
            self.pb.data.reserve(req_len - self.pb.data.len());
            let cap = self.pb.data.capacity();
            self.pb.data.resize(cap, T::default());
            self.pb.max_capacity = self.pb.max_capacity.max(cap);
        }

        #[cfg(feature = "static")]
        if self.pb.wr + _reserve > self.pb.data.len() {
            return false;
        }
        true
    }

    /// Commit the given number of bytes to the pipe buffer.  This
    /// data should have been written to the start of the slice
    /// returned by one of the `PBufWr::space*` methods just before
    /// this call.
    ///
    /// # Panics
    ///
    /// Panics if data is written to the stream after it has been
    /// marked as closed or aborted.  May panic if more data is
    /// committed than the space that was reserved.
    #[inline]
    #[track_caller]
    pub fn commit(&mut self, len: usize) {
        if self.is_eof() {
            panic_closed_pipebuf();
        }

        let wr = self.pb.wr + len;
        if wr > self.pb.data.len() {
            panic_commit_overflow();
        }
        self.pb.wr = wr;
    }

    /// Return the amount of free space left for writing in the
    /// underlying [`PipeBuf`].  This is the amount of space available
    /// up to the logical capacity limit, not necessarily the current
    /// capacity for a variable-capacity pipe-buffer.
    #[inline]
    pub fn free(&self) -> usize {
        self.capacity() - (self.pb.wr - self.pb.rd)
    }

    /// Test whether the buffer is full, i.e. the stored data length
    /// equals the maximum capacity
    #[inline]
    pub fn is_full(&self) -> bool {
        self.free() == 0
    }

    /// Set the "push" state on the buffer, which the consumer may use
    /// to decide whether or not to flush data immediately.
    #[inline]
    pub fn push(&mut self) {
        if self.pb.state == PBufState::Open {
            self.pb.state = PBufState::Push;
        }
    }

    /// Append a slice of data to the buffer
    ///
    /// If it's possible to write the entire slice, then returns
    /// `true`.  If there is not enough space to write the whole
    /// slice, then does nothing and returns `false`.
    ///
    /// # Panics
    ///
    /// Panics if data is written to the pipe buffer after it has been
    /// marked as closed or aborted.
    #[inline]
    #[track_caller]
    #[must_use]
    pub fn append(&mut self, data: &[T]) -> bool {
        let len = data.len();
        if let Some(space) = self.space(len) {
            space.copy_from_slice(data);
            self.commit(len);
            true
        } else {
            false
        }
    }

    /// Test whether end-of-file has already been indicated, either
    /// using [`PBufWr::close`] or [`PBufWr::abort`].  No more data
    /// should be written after EOF.
    #[inline(always)]
    pub fn is_eof(&self) -> bool {
        !matches!(self.pb.state, PBufState::Open | PBufState::Push)
    }

    /// Indicate end-of-file with success, if not already closed.
    /// This is a normal EOF, where the data will be complete.  The
    /// pipe buffer is given the state [`PBufState::Closing`].  There
    /// may still be unread data in the buffer, but that is the final
    /// data before the EOF.
    ///
    /// Returns `true` if successfully marked as `Closing`, `false` if
    /// the buffer was already closed or aborted.  Note that this
    /// means that if the buffer was already aborted, then it remains
    /// in an aborted state, i.e. the failure indication is not lost.
    #[inline]
    pub fn close(&mut self) -> bool {
        if self.is_eof() {
            false
        } else {
            self.pb.state = PBufState::Closing;
            true
        }
    }

    /// Indicate end-of-file with abort, if not already closed.  This
    /// is an EOF after some kind of failure, where the data may be
    /// incomplete.  The pipe buffer is given the state
    /// [`PBufState::Aborting`].
    ///
    /// Returns `true` if successfully marked as `Aborting`, or
    /// `false` if the buffer was already closed.  Note that this
    /// means that if the buffer was previously closed successfully,
    /// then it remains in that successfully-closed state.  Logically
    /// at the point in time when it was closed, the code considered
    /// the stream to have completed successfully, and that cannot be
    /// overridden.  In addition the consumer may have already
    /// observed the state.
    #[inline]
    pub fn abort(&mut self) -> bool {
        if self.is_eof() {
            false
        } else {
            self.pb.state = PBufState::Aborting;
            true
        }
    }

    /// Write data to the buffer using a closure.  A mutable slice of
    /// maximum `limit` bytes of free space is passed to the closure,
    /// but possibly less or even an empty slice if there is not
    /// enough space in the buffer (check [`PBufWr::free`] or
    /// [`PBufWr::is_full`] first if you don't want to deal with those
    /// situations).  If the closure successfully writes data to the
    /// slice, it should return the length written as `Ok(len)` to
    /// commit that data to the buffer.  If it fails then it may
    /// return any type of its choosing as `Err(e)`.  The closure
    /// return value is directly returned.  If no error-handling is
    /// required see [`PBufWr::write_with_noerr`].
    ///
    /// Note that for efficiency the free space will not be
    /// initialised to zeros.  It will contain some jumble of bytes
    /// previously written to the pipe.  You must not make any
    /// assumptions about this data.
    ///
    /// # Panics
    ///
    /// Panics if data is written to the stream after it has been
    /// marked as closed or aborted.  May panic if more data is
    /// committed than the space that was reserved.
    #[inline]
    #[track_caller]
    pub fn write_with<E>(
        &mut self,
        limit: usize,
        mut cb: impl FnMut(&mut [T]) -> Result<usize, E>,
    ) -> Result<usize, E> {
        let len = cb(self.space_upto(limit))?;
        self.commit(len);
        Ok(len)
    }

    /// Write data to the buffer using a closure.  A mutable slice of
    /// maximum `limit` bytes of free space is passed to the closure,
    /// but possibly less or even an empty slice if there is not
    /// enough space in the buffer (check [`PBufWr::free`] or
    /// [`PBufWr::is_full`] first if you don't want to deal with those
    /// situations).  If the closure successfully writes data to the
    /// slice, it should return the length written to commit that data
    /// to the buffer.  The same length is returned from this method.
    /// To pass through errors see [`PBufWr::write_with`].
    ///
    /// Note that for efficiency the free space will not be
    /// initialised to zeros.  It will contain some jumble of bytes
    /// previously written to the pipe.  You must not make any
    /// assumptions about this data.
    ///
    /// # Panics
    ///
    /// Panics if data is written to the stream after it has been
    /// marked as closed or aborted.  May panic if more data is
    /// committed than the space that was reserved.
    #[inline]
    #[track_caller]
    pub fn write_with_noerr(
        &mut self,
        limit: usize,
        mut cb: impl FnMut(&mut [T]) -> usize,
    ) -> usize {
        let len = cb(self.space_upto(limit));
        self.commit(len);
        len
    }

    /// Get the logical capacity of the buffer, i.e. the maximum
    /// amount of data which this pipe-buffer can hold.
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.pb.capacity()
    }
}

impl<'a> PBufWr<'a, u8> {
    /// Input data from the given `Read` implementation as available,
    /// up to the capacity of the buffer.  If EOF is indicated by the
    /// `Read` source through an `Ok(0)` return, then a normal
    /// [`PBufState::Closing`] EOF is set on the pipe buffer, and no
    /// more data will be read in future calls.  The read call is
    /// retried in case of `ErrorKind::Interrupted` errors, but all
    /// other errors are returned directly.  So if the `Read`
    /// implementation supports an error return that indicates that
    /// the stream has aborted, that needs handling by the caller.
    ///
    /// Use a tripwire (see [`PBufWr::tripwire`]) if you need to
    /// determine whether or not new data was read.  This is necessary
    /// because a call may both read data and return an error (for
    /// example `WouldBlock`).
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn input_from(&mut self, source: &mut impl Read) -> std::io::Result<()> {
        self.input_from_upto(source, usize::MAX)
    }

    /// Input data from the given `Read` implementation as available,
    /// up to `limit` bytes.  If EOF is indicated by the `Read` source
    /// through an `Ok(0)` return, then a normal
    /// [`PBufState::Closing`] EOF is set on the pipe buffer, and no
    /// more data will be read in future calls.  The read call is
    /// retried in case of `ErrorKind::Interrupted` errors, but all
    /// other errors are returned directly.  So if the `Read`
    /// implementation supports an error return that indicates that
    /// the stream has aborted, that needs handling by the caller.
    ///
    /// Use a tripwire (see [`PBufWr::tripwire`]) if you need to
    /// determine whether or not new data was read.  This is necessary
    /// because a call may both read data and return an error (for
    /// example `WouldBlock`).
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn input_from_upto(&mut self, source: &mut impl Read, limit: usize) -> std::io::Result<()> {
        if self.is_eof() {
            return Ok(());
        }

        let mut total = 0;
        while total < limit && !self.is_full() {
            match self.write_with(limit - total, |buf| source.read(buf)) {
                Err(ref e) if e.kind() == ErrorKind::Interrupted => (),
                Err(e) => return Err(e),
                Ok(0) => {
                    self.close();
                    return Ok(());
                }
                Ok(count) => {
                    total += count;
                }
            }
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<'a> std::io::Write for PBufWr<'a, u8> {
    /// Write data to the pipe-buffer.  The following returns are
    /// possible:
    ///
    /// - `Ok(len)` with `len > 0`: Some data was written, but not
    /// necessarily all data
    ///
    /// - `Err(e)` with `e.kind() == ErrorKind::WouldBlock`: No space
    /// available to write right now
    fn write(&mut self, data: &[u8]) -> Result<usize, std::io::Error> {
        self.pb.write(data)
    }

    /// Flush sets the "push" state on the [`PipeBuf`]
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.pb.flush()
    }
}

// Panic code is pulled out into non-inlined functions to reduce
// overhead in inlined code
#[inline(never)]
#[cold]
#[track_caller]
fn panic_closed_pipebuf() -> ! {
    panic!("Illegal to commit data to a closed PipeBuf");
}
#[inline(never)]
#[cold]
#[track_caller]
fn panic_commit_overflow() -> ! {
    panic!("Illegal to commit more bytes to a PipeBuf than the reserved space");
}
