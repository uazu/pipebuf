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

    /// Get a reference to a mutable slice of `reserve` bytes of free
    /// space where new data may be written.  Once written, the data
    /// must be committed immediately using [`PBufWr::commit`], before
    /// any other operation that might compact the buffer.
    ///
    /// Note that for efficiency the free space will not be
    /// initialised to zeros.  It will contain some jumble of bytes
    /// previously written to the pipe.  You must not make any
    /// assumptions about this data.
    ///
    /// # Panics
    ///
    /// For a fixed-capacity buffer (created with
    #[cfg_attr(
        any(feature = "std", feature = "alloc"),
        doc = "[`PipeBuf::with_fixed_capacity`] or"
    )]
    #[cfg_attr(
        not(any(feature = "std", feature = "alloc")),
        doc = "`PipeBuf::with_fixed_capacity` or"
    )]
    #[cfg_attr(feature = "static", doc = "[`PipeBuf::new_static`]),")]
    #[cfg_attr(not(feature = "static"), doc = "`PipeBuf::new_static`),")]
    /// panics if there is not enough free space to reserve the given
    /// number of bytes.
    #[inline]
    #[track_caller]
    pub fn space(&mut self, reserve: usize) -> &mut [T] {
        self.maybe_space(reserve)
            .expect("Not enough space available in fixed-capacity PipeBuf")
    }

    #[inline]
    #[track_caller]
    pub fn maybe_space(&mut self, reserve: usize) -> Option<&mut [T]> {
        if self.pb.rd == self.pb.wr {
            self.pb.rd = 0;
            self.pb.wr = 0;
        }

        (self.pb.wr + reserve > self.pb.data.len() || self.make_space(reserve))
            .then_some(&mut self.pb.data[self.pb.wr..self.pb.wr + reserve])
    }

    #[inline(never)]
    #[cold]
    #[track_caller]
    fn make_space(&mut self, _reserve: usize) -> bool {
        // Caller guarantees that if .rd == .wr, then now both .rd and
        // .wr will be zero, so if .rd > 0 then there is something to
        // copy down
        if self.pb.rd > 0 {
            self.pb.data.copy_within(self.pb.rd..self.pb.wr, 0);
            self.pb.wr -= self.pb.rd;
            self.pb.rd = 0;
        }

        #[cfg(any(feature = "std", feature = "alloc"))]
        if self.pb.wr + _reserve > self.pb.data.len() {
            if self.pb.fixed_capacity {
                return false;
            }
            let cap = (self.pb.wr + _reserve).max(_reserve * 2);
            self.pb.data.reserve(cap - self.pb.data.len());
            self.pb.data.resize(self.pb.data.capacity(), T::default());
        }

        #[cfg(feature = "static")]
        if self.pb.wr + _reserve > self.pb.data.len() {
            return false;
        }
        true
    }

    /// Commit the given number of bytes to the pipe buffer.  This
    /// data should have been written to the slice returned by the
    /// [`PBufWr::space`] method just before this call.
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
    /// # Panics
    ///
    /// Panics if data is written to the pipe buffer after it has been
    /// marked as closed or aborted.  For fixed-capacity panics, see
    /// [`PBufWr::space`].
    #[inline]
    #[track_caller]
    pub fn append(&mut self, data: &[T]) {
        let len = data.len();
        self.space(len).copy_from_slice(data);
        self.commit(len);
    }

    /// Test whether end-of-file has already been indicated, either
    /// using [`PBufWr::close`] or [`PBufWr::abort`].  No more data
    /// should be written after EOF.
    #[inline(always)]
    pub fn is_eof(&self) -> bool {
        !matches!(self.pb.state, PBufState::Open | PBufState::Push)
    }

    /// Indicate end-of-file with success.  This is a normal EOF,
    /// where the data will be complete.  The pipe buffer is given the
    /// state [`PBufState::Closing`].  There may still be unread data
    /// in the buffer, but that is the final data before the EOF.
    ///
    /// # Panics
    ///
    /// Panics if end-of-file has already been indicated on this
    /// buffer
    #[inline]
    #[track_caller]
    pub fn close(&mut self) {
        if self.is_eof() {
            panic_already_closed();
        }
        self.pb.state = PBufState::Closing;
    }

    /// Indicate end-of-file with abort.  This is an EOF after some
    /// kind of failure, where the data may be incomplete.  The pipe
    /// buffer is given the state [`PBufState::Aborting`].
    ///
    /// # Panics
    ///
    /// Panics if end-of-file has already been indicated on this
    /// buffer
    #[inline]
    #[track_caller]
    pub fn abort(&mut self) {
        if self.is_eof() {
            panic_already_closed();
        }
        self.pb.state = PBufState::Aborting;
    }

    /// Write data to the buffer using a closure.  A mutable slice of
    /// `reserve` bytes of free space is passed to the closure.  If
    /// the closure successfully writes data to the slice, it should
    /// return the length written as `Ok(len)` to commit that data to
    /// the buffer.  If it fails then it may return any type of its
    /// choosing as `Err(e)`.  The closure return value is directly
    /// returned.  If no error-handling is required see
    /// [`PBufWr::write_with_noerr`].
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
    /// committed than the space that was reserved.  Also see
    /// [`PBufWr::space`] for handling of fixed-capacity buffers.
    #[inline]
    #[track_caller]
    pub fn write_with<E>(
        &mut self,
        reserve: usize,
        mut cb: impl FnMut(&mut [T]) -> Result<usize, E>,
    ) -> Result<usize, E> {
        let len = cb(self.space(reserve))?;
        self.commit(len);
        Ok(len)
    }

    /// Write data to the buffer using a closure.  A mutable slice of
    /// `reserve` bytes of free space is passed to the closure.  If
    /// the closure successfully writes data to the slice, it should
    /// return the length written to commit that data to the buffer.
    /// The same length is returned from this method.  To pass through
    /// errors see [`PBufWr::write_with`].
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
    /// committed than the space that was reserved.  Also see
    /// [`PBufWr::space`] for handling of fixed-capacity buffers.
    #[inline]
    #[track_caller]
    pub fn write_with_noerr(
        &mut self,
        reserve: usize,
        mut cb: impl FnMut(&mut [T]) -> usize,
    ) -> usize {
        let len = cb(self.space(reserve));
        self.commit(len);
        len
    }

    /// Test whether the amount of data stored in the pipe-buffer
    /// exceeds the given limit in bytes.  It is preferred to not
    /// expose any information about the consumer-side of the
    /// pipe-buffer to the producer to avoid the producer changing its
    /// behaviour depending on how the consumer behaves, which could
    /// lead to hard-to-find bugs.  However it may be the producer
    /// that is enforcing limits to protect against denial-of-service,
    /// so this call is provided.
    #[inline(always)]
    pub fn exceeds_limit(&self, limit: usize) -> bool {
        (self.pb.wr - self.pb.rd) > limit
    }
}

impl<'a> PBufWr<'a, u8> {
    /// Input data from the given `Read` implementation, up to the
    /// given length.  If EOF is indicated by the `Read` source
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
    pub fn input_from(&mut self, source: &mut impl Read, len: usize) -> std::io::Result<()> {
        if self.is_eof() {
            return Ok(());
        }

        let mut total = 0;
        while total < len {
            match self.write_with(len - total, |buf| source.read(buf)) {
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
    /// Write data to the pipe-buffer
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
#[inline(never)]
#[cold]
#[track_caller]
fn panic_already_closed() -> ! {
    panic!("Illegal to close or abort a PipeBuf more than once");
}
