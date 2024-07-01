# Efficient byte-stream pipe buffer

[![license:MIT/Apache-2.0][1]](https://github.com/uazu/pipebuf)&nbsp;
[![github:uazu/pipebuf][2]](https://github.com/uazu/pipebuf)&nbsp;
[![crates.io:pipebuf][3]](https://crates.io/crates/pipebuf)&nbsp;
[![docs.rs:pipebuf][4]](https://docs.rs/pipebuf)&nbsp;
[![Coverage:99%][5]](https://docs.rs/pipebuf)

[1]: https://img.shields.io/badge/license-MIT%2FApache--2.0-blue
[2]: https://img.shields.io/badge/github-uazu%2Fpipebuf-brightgreen
[3]: https://img.shields.io/badge/crates.io-pipebuf-red
[4]: https://img.shields.io/badge/docs.rs-pipebuf-purple
[5]: https://img.shields.io/badge/Coverage-99%25-blue

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
`no_std`.  See ["Dependents" on
crates.io](https://crates.io/crates/pipebuf/reverse_dependencies) for
`PipeBuf` wrappers for other crates.

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
