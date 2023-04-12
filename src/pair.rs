use super::{PBufRd, PBufTrip, PBufWr, PipeBuf};

/// A bidirectional pipe made up of two pipe buffers
///
/// Like a TCP stream, the two pipes are independent, and can be
/// closed independently.
///
/// There are two calls to get producer/consumer references to the
/// buffers, corresponding to the two ends of the bidirectional pipe,
/// which are arbitrarily referred to as the "upper" and "lower" ends,
/// or alternatively as the "left" and "right" ends, depending on how
/// you wish to conceptualize things.  Since pipes usually run between
/// layers and layer diagrams are stacked vertically, it is hoped that
/// upper/lower is the most helpful terminology, but left/right is
/// offered as an alternative.
///
pub struct PipeBufPair {
    /// Downwards-flowing pipe
    pub down: PipeBuf,
    /// Upwards-flowing pipe
    pub up: PipeBuf,
}

impl PipeBufPair {
    /// Create a new empty bidirectional pipe
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    #[inline]
    pub fn new() -> Self {
        Self {
            down: PipeBuf::new(),
            up: PipeBuf::new(),
        }
    }

    /// Create a new bidirectional pipe buffer with the given initial
    /// capacity in the two directions
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    #[inline]
    pub fn with_capacities(down_size: usize, up_size: usize) -> Self {
        Self {
            down: PipeBuf::with_capacity(down_size),
            up: PipeBuf::with_capacity(up_size),
        }
    }

    /// Create a new bidirectional pipe buffer with the given fixed
    /// capacity in the two directions.  The buffers will never be
    /// reallocated.  If a [`PBufWr::space`] call requests more space
    /// than is available, then the call will panic.
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    #[inline]
    pub fn with_fixed_capacities(down_size: usize, up_size: usize) -> Self {
        Self {
            down: PipeBuf::with_fixed_capacity(down_size),
            up: PipeBuf::with_fixed_capacity(up_size),
        }
    }

    /// Create a new bidirectional pipe buffer backed by two regions
    /// of static memory
    #[cfg(feature = "static")]
    #[cfg_attr(docsrs, doc(cfg(feature = "static")))]
    #[inline]
    pub fn new_static(down_buf: &'static mut [u8], up_buf: &'static mut [u8]) -> Self {
        Self {
            down: PipeBuf::new_static(down_buf),
            up: PipeBuf::new_static(up_buf),
        }
    }

    /// Get the references for reading and writing the stream from the
    /// "upper" end
    #[inline]
    pub fn upper(&mut self) -> PBufRdWr<'_> {
        PBufRdWr {
            rd: self.up.rd(),
            wr: self.down.wr(),
        }
    }

    /// Get the references for reading and writing the stream from the
    /// "lower" end
    #[inline]
    pub fn lower(&mut self) -> PBufRdWr<'_> {
        PBufRdWr {
            rd: self.down.rd(),
            wr: self.up.wr(),
        }
    }

    /// Get the references for reading and writing the stream from the
    /// "left" end.  This is just a convenience to make code more
    /// readable, and actually this is the same as
    /// [`PipeBufPair::upper`].
    #[inline]
    pub fn left(&mut self) -> PBufRdWr<'_> {
        self.upper()
    }

    /// Get the references for reading and writing the stream from the
    /// "right" end.  This is just a convenience to make code more
    /// readable, and actually this is the same as
    /// [`PipeBufPair::lower`].
    #[inline]
    pub fn right(&mut self) -> PBufRdWr<'_> {
        self.lower()
    }

    /// Reset the buffers to their initial state, i.e. in the `Open`
    /// state and empty.  The buffer backing memory is not zeroed.
    #[inline]
    pub fn reset(&mut self) {
        self.down.reset();
        self.up.reset();
    }

    /// Zero the buffers, and reset them to their initial state.  If a
    /// `PipeBufPair` is going to be kept in a pool and reused, it
    /// should be zeroed after use so that no data can leak between
    /// different parts of the codebase.
    #[inline]
    pub fn reset_and_zero(&mut self) {
        self.down.reset_and_zero();
        self.up.reset_and_zero();
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
impl Default for PipeBufPair {
    fn default() -> Self {
        Self::new()
    }
}

/// Pair of consumer and producer references
///
/// Create this using the [`PipeBufPair::upper`] or
/// [`PipeBufPair::lower`] calls, or equivalently
/// [`PipeBufPair::left`] and [`PipeBufPair::right`].  Reborrow it
/// using [`PBufRdWr::reborrow`], or by reborrowing the members
/// individually.
pub struct PBufRdWr<'a> {
    /// Consumer reference for the incoming pipe
    pub rd: PBufRd<'a>,
    /// Producer reference for the outgoing pipe
    pub wr: PBufWr<'a>,
}

impl<'a> PBufRdWr<'a> {
    /// Create new references from these, reborrowing them.  Thanks to
    /// the borrow checker, the original references will be
    /// inaccessible until the returned references' lifetimes end.
    /// The cost is just a couple of pointer copies, just as for
    /// `&mut` reborrowing.
    #[inline(always)]
    pub fn reborrow<'b, 'r>(&'r mut self) -> PBufRdWr<'b>
    where
        'a: 'b,
        'r: 'b,
    {
        PBufRdWr {
            rd: self.rd.reborrow(),
            wr: self.wr.reborrow(),
        }
    }

    /// Generate tripwire values for both `rd` and `wr` halves of the
    /// pipe.  See [`PBufTrip`] for more details.
    #[inline]
    pub fn tripwire(&self) -> (PBufTrip, PBufTrip) {
        (self.rd.tripwire(), self.wr.tripwire())
    }
}
