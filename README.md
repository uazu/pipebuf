# Efficient byte-stream pipe buffer

`PipeBuf` is a byte-stream buffer intended to be accessed by both the
producer of data and the consumer of data.  It acts as both the output
buffer of the producer and the input buffer of the consumer.  Neither
should own it.  It will be owned by the glue code managing the two
components.  It offers a more efficient but compatible alternative to
`Read` and `Write` traits, and reduces copying because neither the
producer nor the consumer needs to keep their own input/output
buffers.  It provides a way to efficiently connect together low-level
protocol or data-processing components along with sources and sinks of
data.  It may be used to create low-level protocol handlers or
processing chains for: futures, async/await, actors, embedded and/or
bare-metal, in both blocking and non-blocking environments, `std` and
`no_std`.  See "Dependents" on crates.io for `PipeBuf` wrappers for
other crates.

### Documentation

See the [crate documentation](http://docs.rs/pipebuf).

# License

This project is licensed under either the Apache License version 2 or
the MIT license, at your option.  (See
[LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT)).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this crate by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
