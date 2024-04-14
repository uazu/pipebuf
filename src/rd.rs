use super::{PBufState, PBufTrip, PBufWr, PipeBuf};

#[cfg(feature = "std")]
use std::io::{ErrorKind, Write};

/// Consumer reference to a [`PipeBuf`]
///
/// Obtain this reference using [`PipeBuf::rd`].  This is a mutable
/// reference to a [`PipeBuf`] that exposes the calls that a consumer
/// is allowed to use.  It acts just like a `&mut PipeBuf`, and has
/// the same size and efficiency.  However unlike a `&mut` reference,
/// reborrowing doesn't happen automatically, but it can still be done
/// just as efficiently using [`PBufRd::reborrow`].
pub struct PBufRd<'a, T: 'static = u8> {
    pub(crate) pb: &'a mut PipeBuf<T>,
}

impl<'a, T: Copy + Default + 'static> PBufRd<'a, T> {
    /// Create a new reference from this one, reborrowing it.  Thanks
    /// to the borrow checker, the original reference will be
    /// inaccessible until the returned reference's lifetime ends.
    /// The cost is just a pointer copy, just as for automatic `&mut`
    /// reborrowing.
    #[inline(always)]
    pub fn reborrow<'b, 'r>(&'r mut self) -> PBufRd<'b, T>
    where
        'a: 'b,
        'r: 'b,
    {
        PBufRd { pb: &mut *self.pb }
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

    /// Get a reference to a slice of bytes representing the current
    /// contents of the buffer.  If the consuming code is able to
    /// process any data, it should do so, and then indicate how many
    /// bytes have been consumed using [`PBufRd::consume`].
    #[inline(always)]
    pub fn data(&self) -> &[T] {
        &self.pb.data[self.pb.rd..self.pb.wr]
    }

    /// Get a mutable reference to a slice of bytes representing the
    /// current contents of the buffer.  A mutable slice may be useful
    /// if the consuming code needs to modify the data in place during
    /// its processing.  If the consuming code is able to process any
    /// data, it should do so, and then indicate how many bytes have
    /// been consumed using [`PBufRd::consume`].
    #[inline(always)]
    pub fn data_mut(&mut self) -> &mut [T] {
        &mut self.pb.data[self.pb.rd..self.pb.wr]
    }

    /// Indicate that `len` bytes should be marked as consumed from
    /// the start of the buffer.  They will be discarded and will no
    /// longer be visible through this interface.
    ///
    /// # Panics
    ///
    /// Panics if `len` is greater than the number bytes in the buffer
    #[inline]
    #[track_caller]
    pub fn consume(&mut self, len: usize) {
        let rd = self.pb.rd + len;
        if rd > self.pb.wr {
            panic_consume_overflow();
        }
        self.pb.rd = rd;
    }

    /// Get the number of bytes held in the buffer
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.pb.wr - self.pb.rd
    }

    /// Test whether the buffer is empty
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.pb.rd == self.pb.wr
    }

    /// Try to consume a "push" indication from the stream.  Returns
    /// `true` if a "push" was present and was consumed, and `false`
    /// if there was no "push" present.
    #[inline]
    pub fn consume_push(&mut self) -> bool {
        if self.pb.state == PBufState::Push {
            self.pb.state = PBufState::Open;
            true
        } else {
            false
        }
    }

    /// Try to consume an EOF indication from the stream.  This
    /// converts state `Closing` to `Closed` and `Aborting` to
    /// `Aborted`.  Returns `true` if there was an EOF present waiting
    /// to be consumed and it was consumed, or `false` if there was no
    /// EOF indicated or if EOF was indicated but it was already
    /// consumed.  The nature of the EOF consumed (close or abort) can
    /// be checked afterwards using [`PBufRd::is_aborted`].
    #[inline]
    pub fn consume_eof(&mut self) -> bool {
        match self.pb.state {
            PBufState::Closing => {
                self.pb.state = PBufState::Closed;
                true
            }
            PBufState::Aborting => {
                self.pb.state = PBufState::Aborted;
                true
            }
            _ => false,
        }
    }

    /// Test whether there is an end-of-file waiting to be consumed.
    /// This means a state of `Closing` or `Aborting`.
    #[inline]
    pub fn has_pending_eof(&self) -> bool {
        matches!(self.pb.state, PBufState::Closing | PBufState::Aborting)
    }

    /// Test whether end-of-file has been indicated by the producer.
    /// This means any of the states: `Closing`, `Closed`, `Aborting`
    /// or `Aborted`.  In the case of EOF, then whatever unconsumed
    /// data is left in the buffer is the final data of the stream.
    #[inline]
    pub fn is_eof(&self) -> bool {
        !matches!(self.pb.state, PBufState::Open | PBufState::Push)
    }

    /// Test whether this stream has been aborted by the producer
    /// (states `Aborting` or `Aborted`)
    #[inline]
    pub fn is_aborted(&self) -> bool {
        matches!(self.pb.state, PBufState::Aborting | PBufState::Aborted)
    }

    /// Test whether an EOF has been indicated and consumed, and for
    /// the case of a `Closed` EOF also that the buffer is empty.
    /// This means that processing on this [`PipeBuf`] is complete
    #[inline]
    pub fn is_done(&self) -> bool {
        self.pb.is_done()
    }

    /// Get the current EOF/push state
    #[inline(always)]
    pub fn state(&self) -> PBufState {
        self.pb.state
    }

    /// Forward all the data found in this pipe to another pipe.  Also
    /// forwards "push" and EOF indications.
    pub fn forward(&mut self, mut dest: PBufWr<'_, T>) {
        if dest.is_eof() {
            return;
        }

        let data = self.data();
        let len = data.len();
        dest.space(len).copy_from_slice(data);
        dest.commit(len);
        self.consume(len);

        if self.consume_push() {
            dest.push();
        }
        if self.consume_eof() {
            if self.is_aborted() {
                dest.abort();
            } else {
                dest.close();
            }
        }
    }
}

impl<'a> PBufRd<'a, u8> {
    /// Output as much data as possible to the given `Write`
    /// implementation.  The "push" state is converted into a `flush`
    /// call if the pipe buffer is emptied.  Also a flush can be
    /// forced if `force_flush` is set to `true`.  End-of-file is not
    /// handled here as the `Write` trait does not support that.  The
    /// calls are retried if `ErrorKind::Interrupted` is returned, but
    /// all other errors are returned directly.
    ///
    /// You can use a tripwire (see [`PBufRd::tripwire`]) if you need
    /// to determine whether or not data was written.  This is
    /// necessary because a call may both write data and return an
    /// error (for example `WouldBlock`).
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    #[track_caller]
    pub fn output_to(&mut self, sink: &mut impl Write, force_flush: bool) -> std::io::Result<()> {
        while !self.is_empty() {
            match sink.write(self.data()) {
                Err(ref e) if e.kind() == ErrorKind::Interrupted => (),
                Err(e) => return Err(e),
                Ok(0) => break, // Should never happen, but deal with it
                Ok(len) => {
                    if len > self.len() {
                        panic!("Faulty Write implementation consumed more data than it was given");
                    }
                    self.consume(len);
                }
            }
        }
        if self.consume_push() || force_flush {
            loop {
                match sink.flush() {
                    Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                    Ok(()) => break,
                }
            }
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<'a> std::io::Read for PBufRd<'a, u8> {
    /// Read data from the pipe-buffer, as much as is available.  The
    /// following returns are possible:
    ///
    /// - `Ok(len)`: Some data was read
    /// - `Ok(0)`: Successful end-of-file was reached
    /// - `Err(e)` with `e.kind() == ErrorKind::WouldBlock`: No data available right now
    /// - `Err(e)` with `e.kind() == ErrorKind::ConnectionAborted`: Aborted end-of-file was reached
    fn read(&mut self, data: &mut [u8]) -> Result<usize, std::io::Error> {
        self.pb.read(data)
    }
}

#[inline(never)]
#[cold]
#[track_caller]
fn panic_consume_overflow() -> ! {
    panic!("Illegal to consume more PipeBuf bytes than are available");
}
