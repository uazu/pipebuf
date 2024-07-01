# Significant feature changes and additions

This project follows Rust semantic versioning.

<!-- see keepachangelog.com for format ideas -->

## 0.3.2 (2024-07-01)

### Changed

- `PBufWr::close` and `PBufWr::abort` no longer panic if the stream is
  already closed


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
