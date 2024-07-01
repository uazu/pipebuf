use super::{PBufRd, PBufWr};

#[cfg(all(not(feature = "std"), feature = "alloc"))]
extern crate alloc;
#[cfg(all(not(feature = "std"), feature = "alloc"))]
use {alloc::vec, alloc::vec::Vec};

#[cfg(feature = "std")]
use std::io::{ErrorKind, Read, Write};

/// Efficient byte-pipe buffer
///
/// This is the interface that is intended for use by the glue code.
/// Use [`PipeBuf::wr`] to get a [`PBufWr`] reference to write to the
/// buffer, and [`PipeBuf::rd`] get a [`PBufRd`] reference to read
/// from the buffer.  These are the references that should be passed
/// to component code.  See this crate's top-level documentation for
/// further discussion of how this works.
pub struct PipeBuf<T: 'static = u8> {
    // Invariants:
    // assert!(.rd <= .wr)
    // assert!(.rd < .wr || .wr == 0)  // Both set back to 0 if empty
    #[cfg(any(feature = "alloc", feature = "std"))]
    pub(crate) data: Vec<T>,
    #[cfg(not(any(feature = "alloc", feature = "std")))]
    pub(crate) data: &'static mut [T],
    pub(crate) rd: usize, // Read offset, or 0 if empty
    pub(crate) wr: usize, // Write offset, or 0 if empty
    pub(crate) state: PBufState,
    #[cfg(any(feature = "alloc", feature = "std"))]
    pub(crate) max_capacity: usize, // Live logical capacity
}

impl<T: Copy + Default + 'static> PipeBuf<T> {
    /// Create a new empty pipe buffer with the given minimum and
    /// maximum capacities.  Both capacities may be rounded up
    /// according to the allocation strategy of `Vec`, since `PipeBuf`
    /// uses all the capacity that `Vec` allocates.
    ///
    /// - The minimum capacity should be set according to the normal
    /// expected traffic or load-level, depending on the buffer sizing
    /// recommendations of the components on either end of the pipe.
    ///
    /// - The maximum capacity should be an upper limit that is not
    /// expected to be exceeded in normal operation.  This is the
    /// limit that avoids memory-exhaustion denial-of-service attacks
    /// if someone finds an exploit in the protocols being
    /// transferred.
    ///
    /// Note that some components will always make use of the maximum
    /// capacity of an output buffer, so in that case the maximum
    /// capacity represents the approximate unit of work for the
    /// downstream chain of components.  In that case the minimum
    /// capacity is not important and [`PipeBuf::fixed`] could be used
    /// instead.
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    #[inline]
    pub fn new(min_capacity: usize, max_capacity: usize) -> Self {
        let data = vec![T::default(); min_capacity];
        let max_capacity = max_capacity.max(data.capacity());
        Self {
            data,
            rd: 0,
            wr: 0,
            state: PBufState::Open,
            max_capacity,
        }
    }

    /// Create a new pipe buffer with the given fixed capacity.  The
    /// buffer will never be reallocated.  The actual capacity may be
    /// greater than the capacity requested, because we make full use
    /// of the memory that `Vec` allocates, and `Vec` may round up.
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    #[inline]
    pub fn fixed(capacity: usize) -> Self {
        Self::new(capacity, capacity)
    }

    /// Create a new pipe buffer backed by the given static memory.
    /// This is useful for `no_std` without an allocator.  This is a
    /// safe call, but requires use of `unsafe` in caller code because
    /// the caller must guarantee that no other code is using this
    /// static memory.
    ///
    /// ```
    ///# use pipebuf::PipeBuf;
    ///# use core::ptr::addr_of_mut;
    /// // Safety: No other `&mut` references exist to the global `BUF`
    /// static mut BUF: [u8; 1024] = [0; 1024];
    /// let mut pbuf = PipeBuf::new_static(unsafe { &mut *addr_of_mut!(BUF) });
    ///# let _ = pbuf;
    /// ```
    #[cfg(feature = "static")]
    #[cfg_attr(docsrs, doc(cfg(feature = "static")))]
    #[inline]
    pub fn new_static(buffer: &'static mut [T]) -> Self {
        Self {
            data: buffer,
            rd: 0,
            wr: 0,
            state: PBufState::Open,
        }
    }

    /// Reset the buffer to its initial state, i.e. in the `Open`
    /// state and empty.  The buffer backing memory is not zeroed, so
    /// malicious code may observe old data in the slice returned by
    /// [`PBufWr::space`].  If sensitive data would be exposed in this
    /// case, use [`PipeBuf::reset_and_zero`] instead.
    #[inline]
    pub fn reset(&mut self) {
        self.rd = 0;
        self.wr = 0;
        self.state = PBufState::Open;
    }

    /// Zero the buffer, and reset it to its initial state.  If a
    /// `PipeBuf` is going to be kept in a pool and reused, it may be
    /// best to zero it after use so that no sensitive data can leak
    /// between different parts of the codebase.
    #[inline]
    pub fn reset_and_zero(&mut self) {
        self.data[..].fill(T::default());
        self.rd = 0;
        self.wr = 0;
        self.state = PBufState::Open;
    }

    /// Get a consumer reference to the buffer
    #[inline(always)]
    pub fn rd(&mut self) -> PBufRd<'_, T> {
        PBufRd { pb: self }
    }

    /// Get a producer reference to the buffer
    #[inline(always)]
    pub fn wr(&mut self) -> PBufWr<'_, T> {
        PBufWr { pb: self }
    }

    /// Obtain a tripwire value to detect buffer changes.  See the
    /// [`PBufTrip`] type for further explanation.
    #[inline]
    pub fn tripwire(&self) -> PBufTrip {
        // The priority here is that a tripwire value can be generated
        // efficiently without adding any overhead to all the
        // different operation methods.
        //
        // All consumer operations must decrease the value, and all
        // producer operations must increase the value (in a
        // wrapping-integer sense).  Otherwise there is a risk that
        // consuming or producing a few bytes along with another
        // change may result in the same value, meaning that the
        // change would be missed.
        PBufTrip((self.wr - self.rd).wrapping_add(self.state as usize))
    }

    /// Test whether there has been a change to the buffer since the
    /// tripwire value provided was obtained.  See [`PBufTrip`].
    #[inline]
    pub fn is_tripped(&self, trip: PBufTrip) -> bool {
        self.tripwire() != trip
    }

    /// Get the current EOF/push state of the buffer
    #[inline(always)]
    pub fn state(&self) -> PBufState {
        self.state
    }

    /// Test whether the "push" state is set on the buffer without
    /// changing the state.
    #[inline(always)]
    pub fn is_push(&self) -> bool {
        self.state == PBufState::Push
    }

    /// Change the "push" state.  It may be necessary for the glue
    /// code to override the "push" status being set by a producer if
    /// the producer is flushing its output too frequently for optimal
    /// operation of a downstream component.
    #[inline]
    pub fn set_push(&mut self, push: bool) {
        if matches!(self.state, PBufState::Open | PBufState::Push) {
            self.state = if push {
                PBufState::Push
            } else {
                PBufState::Open
            };
        }
    }

    /// Test whether an EOF has been indicated and consumed, and for
    /// the case of a `Closed` EOF also that the buffer is empty.
    /// This means that processing on this [`PipeBuf`] is complete
    #[inline]
    pub fn is_done(&self) -> bool {
        match self.state {
            PBufState::Aborted => true,
            PBufState::Closed => self.rd == self.wr,
            _ => false,
        }
    }

    /// Get the logical maximum capacity of the buffer, i.e. the
    /// maximum amount of data which this pipe-buffer can hold.  For
    /// variable-capacity, this may be the `max_capacity` provided
    /// when the pipe-buffer was created, or might be a larger number
    /// if `Vec` has rounded up the capacity.
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        #[cfg(any(feature = "std", feature = "alloc"))]
        return self.max_capacity;
        #[cfg(not(any(feature = "std", feature = "alloc")))]
        return self.data.len();
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl Read for PipeBuf<u8> {
    /// Read data from the pipe-buffer, as much as is available.  The
    /// following returns are possible:
    ///
    /// - `Ok(len)`: Some data was read
    /// - `Ok(0)`: Successful end-of-file was reached
    /// - `Err(e)` with `e.kind() == ErrorKind::WouldBlock`: No data available right now
    /// - `Err(e)` with `e.kind() == ErrorKind::ConnectionAborted`: Aborted end-of-file was reached
    fn read(&mut self, data: &mut [u8]) -> Result<usize, std::io::Error> {
        let mut rd = self.rd();
        if !rd.is_empty() {
            let slice = rd.data();
            let len = slice.len().min(data.len());
            data[..len].copy_from_slice(&slice[..len]);
            rd.consume(len);
            Ok(len)
        } else if rd.consume_eof() {
            if rd.is_aborted() {
                Err(ErrorKind::ConnectionAborted.into())
            } else {
                Ok(0)
            }
        } else {
            Err(ErrorKind::WouldBlock.into())
        }
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl Write for PipeBuf<u8> {
    /// Write data to the pipe-buffer.  The following returns are
    /// possible:
    ///
    /// - `Ok(len)` with `len > 0`: Some data was written, but not
    /// necessarily all data
    ///
    /// - `Err(e)` with `e.kind() == ErrorKind::WouldBlock`: No space
    /// available to write right now
    fn write(&mut self, data: &[u8]) -> Result<usize, std::io::Error> {
        let mut wr = self.wr();
        let space = wr.space_upto(data.len());
        let len = space.len();
        if len > 0 {
            space.copy_from_slice(&data[..len]);
            wr.commit(len);
            return Ok(len);
        }
        Err(ErrorKind::WouldBlock.into())
    }

    /// Flush sets the "push" state on the [`PipeBuf`]
    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.wr().push();
        Ok(())
    }
}

impl<T: Copy + Default + 'static> core::fmt::Debug for PipeBuf<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let len = self.wr - self.rd;
        write!(
            f,
            "PipeBuf({:?} {}+{})",
            self.state,
            len,
            self.capacity() - len
        )
    }
}

/// End-of-file and "push" state of the buffer
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PBufState {
    // Note that the values here are selected so that producer
    // operations increase the value, and consumer operations decrease
    // the value.  This is necessary for `tripwire` to function
    // correctly.
    //
    // Also values are selected so that typical `is_*` operations can
    // optimise down to a single comparison.
    //
    /// End-of-file has not been reached yet.  More data may follow.
    Open = 0,
    /// End-of-file has not been reached yet.  More data may follow.
    /// Producer has suggested that current data be flushed.
    Push = 1,
    /// The producer has reported a successful end-of-file.  Any data
    /// left in the buffer is the final data of the stream.  The
    /// consumer has not yet processed the EOF.
    Closing = 3,
    /// Successful end-of-file has been reported by the producer and
    /// processed by the consumer.
    Closed = 2,
    /// The producer has reported end-of-file due to some error
    /// condition.  The data in the stream might be in an inconsistent
    /// or incomplete state (e.g. a partial record, protocol not
    /// terminated normally, etc).  The consumer has not yet processed
    /// the EOF.
    Aborting = 5,
    /// Abort end-of-file has been reported by the producer and
    /// processed by the consumer.
    Aborted = 4,
}

/// Tripwire value used to detect changes
///
/// This value is obtained using [`PipeBuf::tripwire`],
/// [`PBufRd::tripwire`] or [`PBufWr::tripwire`], which all calculate
/// the same value.  See also the [`tripwire!`] macro which allows
/// tuples of tripwire values to be created and compared.  This is a
/// very cheap value to generate: just a couple of integer operations.
///
/// The value will change in these cases:
///
/// - Data is written to the pipe
/// - Data is read from the pipe
/// - A "push" state is set or consumed
/// - An EOF state is set or consumed
///
/// This value can be compared before and after some operation to
/// detect whether a change has occurred.  However that operation must
/// be purely a consumer operation or purely a producer operation.  If
/// data is both produced and consumed, then the tripwire value may
/// return to the same value and the change wouldn't be detected.
///
/// These scenarios are supported:
///
/// - In a consumer, avoiding re-parsing an input buffer when there
/// have been no changes made by the producer.  Save a `PBufTrip`
/// value before returning, and when called the next time, compare it
/// to the current value.
///
/// - In the glue code, detect whether a component call has caused
/// changes to a buffer.
///
/// - In consumer code, check whether some sub-part of the consumer
/// processing has done something.
///
/// - In producer code, check whether some sub-part of the producer
/// processing has done something.
///
/// [`tripwire!`]: macro.tripwire.html
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub struct PBufTrip(usize);

impl Default for PBufTrip {
    fn default() -> Self {
        Self(PBufState::Open as usize)
    }
}

impl From<usize> for PBufTrip {
    /// May be used to generate `PBufTrip` values for other types
    fn from(v: usize) -> Self {
        Self(v)
    }
}

#[cfg(test)]
mod test {
    // This test is here so that it can directly check inc/dec of
    // tripwire values, which is not possible from outside the crate
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[test]
    fn tripwire() {
        let mut p;
        let mut t;

        macro_rules! assert_inc {
            () => {{
                let n = p.tripwire();
                assert!(t.0 < n.0, "Expecting increase: {} -> {}", t.0, n.0);
                t = n;
            }};
        }
        macro_rules! assert_dec {
            () => {{
                let n = p.tripwire();
                assert!(t.0 > n.0, "Expecting decrease: {} -> {}", t.0, n.0);
                t = n;
            }};
        }

        p = super::PipeBuf::new(0, 1024);
        t = p.tripwire();
        assert!(p.wr().append(b"X"));
        assert_inc!();
        p.rd().consume(1);
        assert_dec!();
        p.wr().push();
        assert_inc!();
        assert!(p.rd().consume_push());
        assert_dec!();
        p.wr().close();
        assert_inc!();
        assert!(p.rd().consume_eof());
        assert_dec!();
        let _ = t;

        p = super::PipeBuf::new(0, 1024);
        t = p.tripwire();
        assert!(p.wr().append(b"X"));
        assert_inc!();
        p.wr().push();
        assert_inc!();
        assert!(p.rd().consume_push());
        assert_dec!();
        p.rd().consume(1);
        assert_dec!();
        p.wr().abort();
        assert_inc!();
        assert!(p.rd().consume_eof());
        assert_dec!();

        let _ = t;
    }
}
