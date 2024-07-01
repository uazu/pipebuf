//! [![license:MIT/Apache-2.0][1]](https://github.com/uazu/pipebuf)&nbsp;
//! [![github:uazu/pipebuf][2]](https://github.com/uazu/pipebuf)&nbsp;
//! [![crates.io:pipebuf][3]](https://crates.io/crates/pipebuf)&nbsp;
//! [![docs.rs:pipebuf][4]](https://docs.rs/pipebuf)&nbsp;
//! [![Coverage:99%][5]](https://docs.rs/pipebuf)
//!
//! [1]: https://img.shields.io/badge/license-MIT%2FApache--2.0-blue
//! [2]: https://img.shields.io/badge/github-uazu%2Fpipebuf-brightgreen
//! [3]: https://img.shields.io/badge/crates.io-pipebuf-red
//! [4]: https://img.shields.io/badge/docs.rs-pipebuf-purple
//! [5]: https://img.shields.io/badge/Coverage-99%25-blue
//!
//! Efficient byte-stream pipe buffer
//!
//! [`PipeBuf`] is a buffer intended to be accessed by both the
//! producer of data and the consumer of data.  It acts as both the
//! output buffer of the producer and the input buffer of the
//! consumer.  Neither should own it.  It will be owned by the glue
//! code managing the two components.  A reference to the buffer
//! (either [`PBufWr`] or [`PBufRd`]) should be passed to one or the
//! other component in turn to give each an opportunity to produce or
//! consume data.  This reduces copying because neither the producer
//! nor the consumer needs to keep their own input/output buffers.
//!
//! From the producer side, it's possible to reserve space, then write
//! data directly into the buffer using a system call or from the
//! output of some other code.  Either normal EOF (closed) or abnormal
//! EOF (aborted) may be indicated if the stream should be terminated.
//! In addition, there is a "push" state, which indicates that data
//! should be flushed immediately if the consumer supports that.
//!
//! From the consumer side, it's possible to view all of the available
//! data as a slice, and to consume all or just a part of it.  For
//! example, the consumer may need to wait for the end of a line or
//! the end of a record before consuming the next chunk of data from
//! the pipe.  The unread data should be left in the pipe buffer until
//! it can be consumed.
//!
//! This has similar characteristics to one half of a TCP stream
//! (except for the network transmission), i.e. it is a stream of
//! bytes that will likely arrive in chunks not aligned with any
//! protocol boundaries, and with its EOF occurring independently from
//! any other byte-stream.  Also, like TCP, there is a distinction
//! between "closed" and "aborted" to terminate the stream.
//!
//! Copying is minimised.  Firstly because the buffer is shared
//! between the producer and consumer, so there is no copying when
//! data is moved between producer and consumer.  And secondly,
//! because it is only required to copy data within the buffer when
//! there is some unconsumed data and there isn't enough capacity
//! available in the buffer to write in the next chunk of data.  So
//! typically this will be small, e.g. a partial line (for a
//! line-based protocols) or a partial record (for record-based
//! protocols) or a partial block (if the data is being consumed in
//! blocks or chunks).
//!
//! Note that this crate is generic on the contained "byte" data type,
//! which defaults to `u8` and so doesn't normally need to be
//! specified.  So read `T` as `u8` in this documentation.  See [notes
//! below](#using-this-crate-with-a-type-other-than-u8) about using
//! this crate with a different contained data type.
//!
//! If it is not clear how to apply this crate to a particular
//! situation, please open a [Discussion
//! item](https://github.com/uazu/pipebuf/discussions).
//!
//!
//! # Bidirectional streams
//!
//! [`PipeBufPair`] puts two pipe-buffers together to make a
//! bidirectional pipe, similar in characteristics to a TCP stream.
//! Each direction is fully independent.  Consumer and producer
//! references [`PBufRdWr`] can be obtained for either end of the
//! stream to pass to the component which handles that end of the
//! stream.
//!
//!
//! # Separation of concerns
//!
//! Typically there are components (protocol implementations or
//! data-processing modules) that could work over any byte-stream
//! medium (TCP, UNIX sockets, pipes or whatever).  They don't need to
//! know exactly what medium they are using.  They just want to be
//! able to process the data and do their job.
//!
//! On the other hand, there are many environments that might want to
//! make use of these components.  They might be running in a blocking
//! or a non-blocking environment, maybe with futures or async/await
//! or actors, or even embedded, or bare-metal in a kernel.  They
//! could be running from TCP or serial buffers or who knows what
//! medium exactly.  There could be different approaches to
//! backpressure or different priorities for different data within a
//! processing network.  All of this is independent to the protocol
//! handling or data processing.
//!
//! Using pipe-buffers as the interface would allow these two sides to
//! each concentrate on what they do best.  In addition, sharing
//! input/output buffers through a [`PipeBuf`] means that less copying
//! is required, especially compared to the situation when using the
//! `Read` trait.
//!
//!
//! # Capacity limit
//!
//! [`PipeBuf`] instances must always specify a maximum capacity.
//! This has several advantages:
//!
//! - There is built-in protection from denial-of-service through
//! memory exhaustion.  A malicious actor can't exploit a bug to cause
//! the internal generation of a huge amount of data and consume all
//! memory.
//!
//! - Some components that can produce large or unlimited amounts of
//! data (such as a file reader or a decompressor) have a limit to
//! fill up to.
//!
//! - It provides backpressure if backpressure is not implemented at a
//! higher level
//!
//! However this means that components always have to check that there
//! is enough space downstream to write to before processing data.
//!
//!
//! # Writing interoperable components using [`PipeBuf`]
//!
//! Your component code (e.g. a protocol or data-processing
//! implementation) should provide a `struct` which the glue code will
//! create and own.  This `struct` should have one or more methods
//! which accept whatever combination of [`PBufRd`], [`PBufWr`] and/or
//! [`PBufRdWr`] references are required for their operation.  For
//! example there may be a "process" call that attempts to process as
//! much data as possible given what is available in the input
//! buffers, generating output and altering the structure's internal
//! state.  Other methods may be required if output may be generated
//! in response to events other than input, depending on the
//! application.
//!
//! If convenient, a "process" method should return an "activity"
//! status.  This should be `true` if the call was able to consume or
//! produce any data at all (including changing the input or output
//! state in any way, including EOF), or `false` if it was not able to
//! make any progress.  So the return type could be `bool`, or maybe
//! `Result<bool, ...>` if it also needs to return an error, or some
//! variation on that.  Expect that your "process" call may be called
//! repeatedly until it returns an activity status of `false`, so it
//! must return `false` eventually.
//!
//! To avoid the inefficiency of repeatedly re-parsing incomplete
//! data, you can use tripwires (see [`PBufTrip`]) to see whether
//! there has been any change on an input buffer before re-parsing.
//! In addition, [`PBufRd::consume_push`] may be used to detect a
//! "push" request and perform an immediate flush in cases where data
//! will normally be held back until there is enough to fill a block.
//!
//! For example a compressor would require one [`PBufRd`] for input
//! and one [`PBufWr`] for output.  It may choose to react to the
//! "push" indication by flushing its output and creating a
//! sync-point.  Its interface may look like:
//!
//! ```ignore
//! fn process(&mut self, inp: PBufRd, out: PBufWr) -> bool {...}
//! ```
//!
//! However a TLS implementation would require one [`PBufRdWr`] for
//! the encrypted side and one [`PBufRdWr`] for the plain-text side,
//! since both sides must be bidirectional and input on the encrypted
//! side may cause output in both directions.  It may also want to
//! indicate failure.  So its interface may look like:
//!
//! ```ignore
//! fn process(&mut self, encrypted: PBufRdWr, plain: PBufRdWr)
//!     -> Result<bool, TlsError> {...}
//! ```
//!
//! Since [`PipeBuf`] supports backpressure, it's necessary for a
//! "process" call to both identify some work to do (e.g. identify a
//! unit of input data) and also check that there is enough space to
//! output the result.  Only when both conditions are met can the work
//! be done, consuming input and committing output.
//!
//! The component writer needs to handle conditions that may lead to a
//! stuck or hung network.  For example, waiting to output a huge
//! packet whose size is larger than the output buffer capacity will
//! never succeed and will hang the network.  This needs handling as
//! an individual packet abort, or else a component failure.
//!
//! If the "process" call also needs to input/output other types than
//! just bytes, then there are a few options:
//!
//! - Take a `&mut VecDeque` of whatever type needs to be
//! input/output, and process as much as possible in one go
//!
//! - Input or output the type one at a time, either inputting through
//! a method argument or outputting through a method return value
//!
//! - Accept one or more closures to handle parsed items.  This would
//! allow referencing data still in the [`PipeBuf`], avoiding some
//! copying
//!
//! - Maybe even handle in two stages, as a "parse" operation (which
//! may return references into the [`PipeBuf`]'s data) and then a
//! "consume" operation to consume that previously-parsed object from
//! the [`PipeBuf`].
//!
//! So there are many ways that you could approach your API design,
//! and [`PipeBuf`] doesn't limit you much in this regard.
//!
//!
//! # Writing the glue code to host [`PipeBuf`]-based components
//!
//! Firstly you'll need to create all the [`PipeBuf`] instances and
//! then all of the component structures which will connect them
//! together (for example, compressor, TLS implementation, etc) or
//! that will act as sources or sinks of data (for example, file
//! writer, file reader, TCP forwarder, etc).  This forms a chain or
//! network of components which the glue code will connect together by
//! making "process" or other method calls on the components to move
//! data into and out of the buffers.
//!
//! Then you need to introduce data into the network and run the
//! components to completion.  The glue code can be written manually
//! for maximum control, or alternatively in most cases the
//! `pipebuf_run!` macro in the **pipebuf_run** crate should simplify
//! the logic.
//!
//! A simple way of running the network to exhaustion would be to just
//! keep calling all the "process"/etc calls on all of the structures
//! until all of them indicate "no activity" (i.e. where a pass
//! through all of them returns `false` from each), which indicates
//! that no more progress can be made at this point.  If you want
//! greater control, then you might choose to use some knowledge of
//! where data is flowing (e.g. by using tripwires, see [`PBufTrip`])
//! and to try to call the specific "process" calls required to
//! advance that.
//!
//! If any component call does not provide an activity status, then
//! you'll have to synthesize an activity status for it
//! (`pipebuf_run!` does this automatically).  You'll need to check
//! all of the input and output buffers used by the call for changes,
//! for example by using tripwires (see [`PBufTrip`] and `tripwire!`)
//! and comparing the trip values before/after the call.
//!
//! Then you have to consider how you are going to regulate
//! introducing data into your network.  Here are some examples:
//!
//! - If you have a single source and you are running in a blocking
//! environment, then you can block on your source, and run the
//! "process" calls on the rest of your network whenever you get new
//! data
//!
//! - If you have multiple sources or you are running non-blocking,
//! then you will typically get a notification or event call when
//! there is new data available (for example a `mio` Ready
//! indication).  At this point you can read new data, and run
//! whatever "process" calls are necessary on the rest of your network
//! as a result.
//!
//! - If you are integrating with futures or async/await, then
//! whenever you are polled, you'll need to poll upstream sources to
//! obtain more data to process in your network until you have some
//! new data to return or have established that no progress can be
//! made right now.
//!
//! Typically you'll want to read input byte-data into your network
//! with a limited maximum chunk size, perhaps implemented by having a
//! fixed-size [`PipeBuf`] as the first buffer in the chain.  Then
//! after each fill of the first buffer, you'd run the rest of the
//! network to exhaustion to avoid having excess data build up within
//! your network's [`PipeBuf`] instances.  (`pipebuf_run!` supports
//! this model.)
//!
//! Output from the network may also be limited if any of the sinks
//! implement backpressure.  For example TCP won't let you write more
//! data if the kernel can't forward anything else right now.  There
//! are two ways to handle backpressure:
//!
//! - Let the backpressure propagate backwards through the network,
//! filling each of the buffers to capacity, until eventually it
//! reaches the input components, which with nowhere to write data
//! will stop reading more data.  This unfortunately means that all
//! the buffers will have been reallocated to their maximum capacity.
//!
//! - Detect backpressure on outputs and stop reading input data until
//! it passes.  This is the most efficient way and avoids building up
//! too much data in the component chain or network.  In a blocking
//! environment this may happen automatically: For example, writing to
//! the TCP output will block and that stops everything proceeding
//! (including input) until data starts to flow again.  For a
//! non-blocking environment this has to be implemented in the glue
//! code.  (`pipebuf_run!` supports this.)
//!
//! Finally you need to decide when the network has finished in order
//! to stop running it.  Typically this means checking
//! [`PipeBuf::is_done`] on the externally-visible outputs.
//! (`pipebuf_run!` supports this.)
//!
//!
//! # Safety and efficiency
//!
//! This crate is compiled with `#[forbid(unsafe_code)]` so it is
//! sound in a Rust sense, and it has 99% test coverage.  The use of
//! [`PBufRd`] and [`PBufWr`] references means that the consumer can
//! only do consumer operations, and the producer can only do producer
//! operations.  These reference types cost no more than a `&mut
//! PipeBuf`, so this protection is for free.  In addition most
//! operations on the [`PipeBuf`] generate very little code and can be
//! inlined by the compiler.
//!
//! However, this is a low-level buffer.  It is optimised for speed
//! rather than to exclude all possible foot-guns.  Here are some ways
//! you can shoot yourself in the foot:
//!
//! - By making any assumptions about the existing contents of the
//! mutable slice returned by [`PBufWr::space`].  For efficiency this
//! is not zeroed, so will contain some jumble of bytes previously
//! written to the buffer, but which bytes exactly are not defined.
//! In particular, calling [`PBufWr::space`] twice might not return
//! the same slice both times in the case that the buffer needed to be
//! compacted, so any data previously written but not committed would
//! be lost.
//!
//! - By doing something else to the buffer between calling
//! [`PBufWr::space`] and [`PBufWr::commit`].  If that something else
//! causes the buffer to be compacted then the data you wish to commit
//! may not be there any more.  The scoped call [`PBufWr::write_with`]
//! avoids this problem, but this may not be flexible enough for some
//! uses of the pipe-buffer.
//!
//! - By writing more data to a closed pipe.  This will cause a panic.
//!
//! - By passing different pipe-buffers to the same component on
//! different method calls.  That is guaranteed to confuse things.
//! But it is hard to do by mistake as the pipe-buffer argument will
//! typically be hardcoded in the glue code.
//!
//!
//! # `Read` and `Write` traits
//!
//! `Read` and `Write` traits are implemented on [`PipeBuf`] by
//! default (unless the "std" feature is disabled).  In addition there
//! are
#![cfg_attr(
    feature = "std",
    doc = "[`PBufRd::output_to`] and [`PBufWr::input_from`]"
)]
#![cfg_attr(
    not(feature = "std"),
    doc = "`PBufRd::output_to` and `PBufWr::input_from`"
)]
//! to send/receive data from external `Write` and `Read`
//! implementations.  These traits may make it easier to interface to
//! code that you don't control.
//!
//! For your own code, typically `Read` is much less useful than
//! examining the buffer directly, because it doesn't let you see
//! what's there before reading it in, which means probably you're
//! going to have to buffer it again somewhere else until you have
//! enough data, which is a duplication of buffering.
//!
//!
//! # `no_std` support
//!
//! For a `no_std` environment, if `alloc` is supported then you can
//! use **pipebuf** normally.  In your **Cargo.toml** add `pipebuf = {
//! version = "...", default-features = false, features = ["alloc"]
//! }`.  If you wish to use fixed-capacity buffers (to avoid
//! reallocations) this is supported using
#![cfg_attr(any(feature = "std", feature = "alloc"), doc = "[`PipeBuf::fixed`]")]
#![cfg_attr(not(any(feature = "std", feature = "alloc")), doc = "`PipeBuf::fixed`")]
//! .
//!
//! If no heap is available, a [`PipeBuf`] may be backed by a fixed
//! capacity static buffer, i.e. anything you can get a `&'static mut
//! [u8]` reference to, typically memory from a `static mut`.  In your
//! **Cargo.toml** add `pipebuf = { version = "...", default-features
//! = false, features = ["static"] }`.  Then use
#![cfg_attr(feature = "static", doc = "[`PipeBuf::new_static`]")]
#![cfg_attr(not(feature = "static"), doc = "`PipeBuf::new_static`")]
//! .
//!
//! If you wish to reuse [`PipeBuf`] instances (e.g. in a buffer
//! pool), use [`PipeBuf::reset_and_zero`] or [`PipeBuf::reset`] to
//! prepare the buffer before re-use.
//!
//!
//! # `no_std` support in components
//!
//! If the component supports `no_std` use, it should build using
//! `#![no_std]` and bring in **pipebuf** into its **Cargo.toml**
//! using `pipebuf = { version = "...", default-features = false }`.
//! Building this way will make it compatible with both `static` and
//! `alloc` features, according to the crate user's requirements.  The
//! component crate should not select `alloc` or `static` features
//! unless it really needs them, as that would limit the options for
//! the crate user.
//!
//!
//! # Using this as a dependency ... or not
//!
//! This crate currently depends on no other crates, and aims to
//! remain minimal.  This crate is not expected to grow.  It will not
//! be expanded to contain interfaces to other types -- those will go
//! into other `pipebuf_*` crates.  So it is a safe choice to use as a
//! dependency.
//!
//! However if you prefer not to depend on [`PipeBuf`] yet remain
//! compatible with [`PipeBuf`] and many other low-level scenarios,
//! this may be possible if you have a "process" call that accepts
//! byte slices, `&[u8]` for inputs and `&mut [u8]` for outputs, and
//! that returns how many bytes have been consumed or that are ready
//! to be committed in each.  The only downside here is that the
//! "process" call has no control over how much space it is given to
//! write in.  So there could be cases where it wants to process input
//! but can't because there isn't enough space to output the resulting
//! data.  So this requires a little more planning and thinking
//! through.  Either document the minimum free space required, or
//! provide a method for the caller to find out how much output space
//! is required.  Of course you could buffer the output internally,
//! but then that loses all the benefits of shared input/output
//! buffers.
//!
//! Ideally this would be in `std` so that it would be an obvious
//! choice to standardize around, like `Vec` and `HashMap`.
//!
//!
//! # Structure of the storage
//!
//! The internal `Vec` has the following segments:
//!
//! - Data already consumed and waiting to be discarded
//!
//! - Data waiting to be consumed, as viewed by [`PBufRd::data`]
//!
//! - Area free for adding more data, as returned by [`PBufWr::space`]
//!
//! When there is not enough space in the third section for a
//! [`PBufWr::space`] call, the first section is discarded and the
//! unconsumed data moved down.  If there is still not enough space
//! then the buffer is extended.
//!
//! Unlike a circular buffer (e.g. `VecDeque`), both data segments are
//! contiguous slices, which is important for efficient parsing and
//! filling.  Also, trying to do the same thing with a `VecDeque`
//! would be inefficient because when doing I/O you'd have to reserve
//! space by writing zeros and then afterwards rewind to the actual
//! length read, since I/O calls require a mutable slice to write to.
//!
//!
//! # Using this crate with a type other than `u8`
//!
//! This crate is optimised for handling byte-streams.  However it's
//! possible that small types other than `u8` may be of use, so it
//! accepts an optional generic argument for the contained type.  For
//! example `u16` may be useful if the data being passed is UTF-16, or
//! `char` for Unicode codepoints.  In general this crate is useful
//! where parsing code needs to see all the available data and may
//! consume a variable quantity of it depending on what it sees.  If
//! you only need to consume one item at a time then `VecDeque` from
//! `std` is preferable because it is a ring buffer so doesn't need to
//! move data down from time to time to compact the buffer.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]

// We don't mind if they enable both 'std' and 'alloc' together since
// they have the same API, and 'std' can take precedence, but the
// 'static' API and implementation cannot coexist with the others.
// However for building docs.rs docs, we need all features despite it
// not compiling, but doc-generation can handle that it seems.  For
// component crates, we have to support building with no features at
// all, but in that case there is no way to create a `PipeBuf`.  So
// glue code will be forced to choose either `static` or `alloc`.
#[cfg(all(not(docsrs), feature = "std", feature = "static"))]
compile_error!("Both feature 'std' and feature 'static' cannot be enabled at the same time");
#[cfg(all(not(docsrs), feature = "alloc", feature = "static"))]
compile_error!("Both feature 'alloc' and feature 'static' cannot be enabled at the same time");

mod buf;
pub use buf::{PBufState, PBufTrip, PipeBuf};

mod wr;
pub use wr::PBufWr;

mod rd;
pub use rd::PBufRd;

mod pair;
pub use pair::{PBufRdWr, PipeBufPair};

mod run;
pub use run::RunStatus;

/// Form a tuple of tripwire values
///
/// This is intended to be used to create a tuple of [`PBufTrip`]
/// values both before and after an operation.  The tuples can then be
/// compared to see whether there was any change.
///
#[cfg_attr(
    any(feature = "std", feature = "alloc"),
    doc = "
```
# use pipebuf::{tripwire, PipeBuf};
# let p1 = PipeBuf::<u8>::fixed(10);
# let p2 = PipeBuf::<u8>::fixed(10);
# let p3 = PipeBuf::<u8>::fixed(10);
# let p4 = PipeBuf::<u8>::fixed(10);
let before = tripwire!(p1, p2, p3, p4);
// some operation on p1/p2/p3/p4 ...
let after = tripwire!(p1, p2, p3, p4);
let activity = before != after;
```
"
)]
#[macro_export]
macro_rules! tripwire {
    ($($x:expr),+) => {{
        (
            $($x.tripwire()),+
        )
    }}
}

//@@@ TODO: Add a full example or two
