/// Status returned by `pipebuf_run!`
///
/// See the `pipebuf_run` crate.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum RunStatus {
    /// Some data was processed
    Okay,

    /// No new input data available right now, or no components are
    /// able to make any progress with the data already in the buffers
    Wait,

    /// Blocked on output
    Blocked,

    /// Execution is complete: EOF has been consumed on all the inputs
    /// of all the sink components
    Done,

    /// The chain/network is hung due to a misbehaving component.
    /// Probably you'll want to abort the processing since running the
    /// network further will not progress.
    ///
    /// It's not possible to reliably detect all hangs.  For example,
    /// a component may be waiting for space for a very large output
    /// packet, for which there will never be enough space, but from
    /// the outside it just looks like it's waiting for more input.
    /// Detection of hangs depends on checks being implemented in
    /// `pipebuf_run!` and selected by the coder.
    Hung,
}
