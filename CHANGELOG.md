# Significant feature changes and additions

This project follows Rust semantic versioning.

<!-- see keepachangelog.com for format ideas -->

## 0.4.0

### Breaking

All pipe-buffers now have a maximum capacity, even for variable-sized
buffers.  Implications for existing code:

- In component code, you are no longer guaranteed to obtain space on
  request from an output buffer.  So now you must both find input that
  can be processed and also obtain enough space for the output before
  proceeding with an operation.

- Component code for handling variable-buffers, fixed-buffers and
  static-buffers can now be the same.

Some panics have been eliminated:

- No more panic on repeated close.  This makes some component code
  less error-prone.  The first `close` or `abort` takes precedence,
  and later `close` or `abort` calls have no effect.

- No more panic on requesting too much space from a static
  pipe-buffer.  This now returns `None` or a short slice according to
  the call, just as with other kinds of pipe-buffers.

These calls have changed:
- `PipeBuf::new()` -> `PipeBuf::new(min, max)`
- `PipeBuf::with_fixed_capacity(cap)` -> `PipeBuf::fixed(cap)`
- `PipeBuf::with_capacity(cap) -> `PipeBuf::new(cap, max)`
- `PipeBufPair::new()` -> `PipeBufPair::new(down_min, down_max, up_min, up_max)`
- `PipeBufPair::with_fixed_capacities(down, up)` -> `PipeBufPair::fixed(down, up)`
- `PipeBufPair::with_capacities(down, up)` -> `PipeBufPair::new(down, down_max, up, up_max)`
- `PBufWr::space` now returns `None` if requested space isn't available
- `PBufWr::free_space` -> `PBufWr::free`, and is no longer an `Option`
- `PBufWr::append` may now fail, so returns a `#[must_use]` boolean
- `PBufWr::close` no longer panics on repeated close, but returns a boolean
- `PBufWr::abort` no longer panics on repeated close, but returns a boolean
- `PBufWr::write_with` and `PBufWr::write_with_noerr` now may give the
  closure less than the requested amount of bytes, or even an empty slice
- `PBufWr::input_from` -> `PBufWr::input_from_upto`
- `PBufWr::input_from` now reads up to the maximum capacity of the buffer

### Added

Glue code generation can now be automated using the `pipebuf_run!`
macro in the separate `pipebuf_run` crate.  This automatically handles
both output backpressure and gradual input (regulated by input buffer
capacity) in order to run the intermediate pipe-buffers lean.

Added:
- `PipeBufPair::tripwire`
- `PipeBuf::capacity`
- `PBufRd::capacity`
- `PBufWr::capacity`
- `PBufRd::is_full`
- `PBufWr::is_full`
- `PBufRd::has_pending_push`
- `PBufWr::space_upto` if you can accept less space than requested
- `PBufWr::space_all` to get all the remaining available space, up to
  the maximum capacity
- `RunStatus`, for use by `pipebuf_run!` macro

### Removed

- `PBufWr::exceeds_limit`: Limits are now handled through having a
  maximum capacity


## 0.3.1 (2024-04-14)

### Added

- `PBufRd::data_mut` to support Rustls unbuffered interface

## 0.3.0 (2024-04-09)

### Added

- Generic support for contained element types other than `u8`.  So
  this allows `u16` streams or `char` streams, etc to be supported.
- `PBufWr::try_space` to get a writable slice to available space, but
  only if enough bytes are free
- `PBufWr::free_space` to report the number of bytes of space
  available in fixed-sized buffers

These changes were contributed by [Ken Hoover](https://github.com/khoover).

### Breaking

- Most code will compile with no changes.  However for some cases,
  especially small tests and examples, Rust may complain about not
  knowing the type for the `new` method, and `u8` will need to be
  specified, e.g. `PipeBuf::<u8>::new()`.  This is because Rust does
  not allow the default `T = u8` to be specified on methods, and
  relies on inference.  Normally the inference works fine, though.


## 0.2.1 (2023-07-19)

### Added

- Tests giving 99% coverage.  Missing 1% is due to KCOV weirdness.

### Fixed

- Compaction bug for "static" (no_std) feature


## 0.2.0 (2023-04-12)

First public release

<!-- Local Variables: -->
<!-- mode: markdown -->
<!-- End: -->
