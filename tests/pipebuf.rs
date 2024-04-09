//! This file must be called `pipebuf.rs` for the KCOV testing script
//! to find it.  This uses `unsafe` to test the `static` feature, so
//! needs to be external to the crate.
//!
//! NOTE: This is not what typical crate-user code would look like, as
//! usually producer code, consumer code and glue code would be
//! separate.

use pipebuf::PBufState;

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
use pipebuf::{PBufRd, PBufWr, PipeBuf, PipeBufPair};

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
macro_rules! fixed_capacity_pipebuf {
    ($size:expr) => {{
        #[cfg(any(feature = "std", feature = "alloc"))]
        let p = PipeBuf::<u8>::with_fixed_capacity($size);
        #[cfg(feature = "static")]
        let p = {
            static mut BUF: [u8; $size] = [0; $size];
            PipeBuf::new_static(unsafe { &mut BUF })
        };
        p
    }};
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
macro_rules! fixed_capacity_pipebufpair {
    ($size:expr) => {{
        #[cfg(any(feature = "std", feature = "alloc"))]
        let p = PipeBufPair::with_fixed_capacities($size, $size);
        #[cfg(feature = "static")]
        let p = {
            static mut BUF0: [u8; $size] = [0; $size];
            static mut BUF1: [u8; $size] = [0; $size];
            PipeBufPair::new_static(unsafe { &mut BUF0 }, unsafe { &mut BUF1 })
        };
        p
    }};
}

/// For tripwires to work correctly all producer actions that cause a
/// change must increase the tripwire value, and all consumer actions
/// that cause a change must decrease the tripwire value.  See also
/// unit test in src/buf.rs
#[test]
fn pbufstate() {
    macro_rules! increasing {
        ($x:ident, $y:ident) => {{
            assert!((PBufState::$x as u32) < (PBufState::$y as u32));
        }};
    }
    macro_rules! decreasing {
        ($x:ident, $y:ident) => {{
            assert!((PBufState::$x as u32) > (PBufState::$y as u32));
        }};
    }

    // Producer actions
    increasing!(Open, Push); // push
    increasing!(Open, Closing); // close
    increasing!(Push, Closing); // close
    increasing!(Open, Aborting); // abort
    increasing!(Push, Aborting); // abort

    // Consumer actions
    decreasing!(Push, Open); // consume_push
    decreasing!(Closing, Closed); // consume_eof
    decreasing!(Aborting, Aborted); // consume_eof
}

/// Test handling of states
#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
fn states() {
    let mut p = fixed_capacity_pipebuf!(10);

    // Initial state
    assert_eq!(true, p.rd().is_empty());
    assert_eq!(0, p.rd().len());
    assert_eq!(false, p.rd().consume_push());
    assert_eq!(false, p.rd().consume_eof());
    assert_eq!(false, p.rd().has_pending_eof());
    assert_eq!(false, p.rd().is_eof());
    assert_eq!(false, p.wr().is_eof());
    assert_eq!(false, p.rd().is_aborted());
    assert_eq!(false, p.rd().is_done());
    assert_eq!(PBufState::Open, p.rd().state());

    // Set and clear "push" through set_push()
    p.set_push(true);
    assert_eq!(PBufState::Push, p.state());
    p.set_push(false);
    assert_eq!(PBufState::Open, p.state());

    // Add data
    p.wr().append(b"0");
    assert_eq!(false, p.rd().is_empty());
    assert_eq!(1, p.rd().len());
    assert_eq!(b'0', p.rd().data()[0]);
    assert_eq!(false, p.rd().consume_push());
    assert_eq!(false, p.rd().consume_eof());
    assert_eq!(false, p.rd().has_pending_eof());
    assert_eq!(false, p.rd().is_eof());
    assert_eq!(false, p.wr().is_eof());
    assert_eq!(false, p.rd().is_aborted());
    assert_eq!(false, p.rd().is_done());
    assert_eq!(PBufState::Open, p.rd().state());

    // Add "push" through push(), and consume it
    p.wr().push();
    assert_eq!(PBufState::Push, p.rd().state());
    assert_eq!(true, p.is_push());
    assert_eq!(true, p.rd().consume_push());
    assert_eq!(false, p.is_push());
    assert_eq!(PBufState::Open, p.rd().state());
    assert_eq!(false, p.rd().consume_push());

    // Add "push" through set_push(), and consume it
    p.set_push(true);
    assert_eq!(PBufState::Push, p.rd().state());
    assert_eq!(true, p.is_push());
    assert_eq!(true, p.rd().consume_push());
    assert_eq!(false, p.is_push());
    assert_eq!(PBufState::Open, p.rd().state());
    assert_eq!(false, p.rd().consume_push());

    // Consume data
    p.rd().consume(1);
    assert_eq!(true, p.rd().is_empty());
    assert_eq!(0, p.rd().len());
    assert_eq!(false, p.rd().consume_push());
    assert_eq!(false, p.rd().consume_eof());
    assert_eq!(false, p.rd().has_pending_eof());
    assert_eq!(false, p.rd().is_eof());
    assert_eq!(false, p.wr().is_eof());
    assert_eq!(false, p.rd().is_aborted());
    assert_eq!(false, p.rd().is_done());
    assert_eq!(PBufState::Open, p.rd().state());

    // Add data
    p.wr().append(b"12");
    assert_eq!(false, p.rd().is_empty());
    assert_eq!(2, p.rd().len());
    assert_eq!(b"12", p.rd().data());
    assert_eq!(false, p.rd().consume_push());
    assert_eq!(false, p.rd().consume_eof());
    assert_eq!(false, p.rd().has_pending_eof());
    assert_eq!(false, p.rd().is_eof());
    assert_eq!(false, p.wr().is_eof());
    assert_eq!(false, p.rd().is_aborted());
    assert_eq!(false, p.rd().is_done());
    assert_eq!(PBufState::Open, p.rd().state());

    // Add normal EOF
    p.wr().close();
    assert_eq!(false, p.rd().is_empty());
    assert_eq!(2, p.rd().len());
    assert_eq!(b"12", p.rd().data());
    assert_eq!(false, p.rd().consume_push());
    assert_eq!(true, p.rd().has_pending_eof());
    assert_eq!(true, p.rd().is_eof());
    assert_eq!(true, p.wr().is_eof());
    assert_eq!(false, p.rd().is_aborted());
    assert_eq!(false, p.rd().is_done());
    assert_eq!(PBufState::Closing, p.rd().state());

    // Consume EOF
    assert_eq!(true, p.rd().consume_eof());
    assert_eq!(false, p.rd().is_empty());
    assert_eq!(2, p.rd().len());
    assert_eq!(b"12", p.rd().data());
    assert_eq!(false, p.rd().consume_push());
    assert_eq!(false, p.rd().has_pending_eof());
    assert_eq!(true, p.rd().is_eof());
    assert_eq!(true, p.wr().is_eof());
    assert_eq!(false, p.rd().is_aborted());
    assert_eq!(false, p.rd().is_done());
    assert_eq!(PBufState::Closed, p.rd().state());
    assert_eq!(false, p.rd().consume_eof());

    // Consume data
    p.rd().consume(2);
    assert_eq!(true, p.rd().is_empty());
    assert_eq!(0, p.rd().len());
    assert_eq!(false, p.rd().consume_push());
    assert_eq!(false, p.rd().consume_eof());
    assert_eq!(false, p.rd().has_pending_eof());
    assert_eq!(true, p.rd().is_eof());
    assert_eq!(true, p.wr().is_eof());
    assert_eq!(false, p.rd().is_aborted());
    assert_eq!(true, p.rd().is_done());
    assert_eq!(PBufState::Closed, p.rd().state());

    // Try again, initial state, add data
    p.reset();
    p.wr().append(b"345");
    assert_eq!(false, p.rd().is_empty());
    assert_eq!(3, p.rd().len());
    assert_eq!(b"345", p.rd().data());
    assert_eq!(false, p.rd().consume_push());
    assert_eq!(false, p.rd().consume_eof());
    assert_eq!(false, p.rd().has_pending_eof());
    assert_eq!(false, p.rd().is_eof());
    assert_eq!(false, p.wr().is_eof());
    assert_eq!(false, p.rd().is_aborted());
    assert_eq!(false, p.rd().is_done());
    assert_eq!(PBufState::Open, p.rd().state());

    // Add failure EOF
    p.wr().abort();
    assert_eq!(false, p.rd().is_empty());
    assert_eq!(3, p.rd().len());
    assert_eq!(b"345", p.rd().data());
    assert_eq!(false, p.rd().consume_push());
    assert_eq!(true, p.rd().has_pending_eof());
    assert_eq!(true, p.rd().is_eof());
    assert_eq!(true, p.wr().is_eof());
    assert_eq!(true, p.rd().is_aborted());
    assert_eq!(false, p.rd().is_done());
    assert_eq!(PBufState::Aborting, p.rd().state());

    // This time consume data first
    p.rd().consume(3);
    assert_eq!(true, p.rd().is_empty());
    assert_eq!(0, p.rd().len());
    assert_eq!(false, p.rd().consume_push());
    assert_eq!(true, p.rd().has_pending_eof());
    assert_eq!(true, p.rd().is_eof());
    assert_eq!(true, p.wr().is_eof());
    assert_eq!(true, p.rd().is_aborted());
    assert_eq!(false, p.rd().is_done());
    assert_eq!(PBufState::Aborting, p.rd().state());

    // Consume EOF
    assert_eq!(true, p.rd().consume_eof());
    assert_eq!(true, p.rd().is_empty());
    assert_eq!(0, p.rd().len());
    assert_eq!(false, p.rd().consume_push());
    assert_eq!(false, p.rd().consume_eof());
    assert_eq!(false, p.rd().has_pending_eof());
    assert_eq!(true, p.rd().is_eof());
    assert_eq!(true, p.wr().is_eof());
    assert_eq!(true, p.rd().is_aborted());
    assert_eq!(true, p.rd().is_done());
    assert_eq!(PBufState::Aborted, p.rd().state());
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
#[should_panic]
fn no_space() {
    let mut p = fixed_capacity_pipebuf!(10);
    // Note that capacity won't be exactly 10 since `Vec` rounds up,
    // so testing 11 or so on won't work.
    p.wr().space(100);
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
fn no_space_try() {
    let mut p = fixed_capacity_pipebuf!(10);
    assert!(p.wr().free_space().unwrap() >= 10);
    // Note that capacity won't be exactly 10 since `Vec` rounds up,
    // so testing 11 or so on won't work.
    assert!(p.wr().try_space(100).is_none());
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
#[should_panic]
fn commit_overflow() {
    let mut p = fixed_capacity_pipebuf!(10);
    // Note that capacity won't be exactly 10 since `Vec` rounds up,
    // so testing 11 or so on won't work.
    p.wr().commit(100);
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
#[should_panic]
fn consume_overflow() {
    let mut p = fixed_capacity_pipebuf!(10);
    p.rd().consume(1);
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
#[should_panic]
fn commit_after_close() {
    let mut p = fixed_capacity_pipebuf!(10);
    p.wr().close();
    p.wr().commit(1);
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
#[should_panic]
fn commit_after_abort() {
    let mut p = fixed_capacity_pipebuf!(10);
    p.wr().abort();
    p.wr().commit(1);
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
#[should_panic]
fn close_after_close() {
    let mut p = fixed_capacity_pipebuf!(10);
    p.wr().close();
    p.wr().close();
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
#[should_panic]
fn close_after_abort() {
    let mut p = fixed_capacity_pipebuf!(10);
    p.wr().abort();
    p.wr().close();
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
#[should_panic]
fn abort_after_close() {
    let mut p = fixed_capacity_pipebuf!(10);
    p.wr().close();
    p.wr().abort();
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
#[should_panic]
fn abort_after_abort() {
    let mut p = fixed_capacity_pipebuf!(10);
    p.wr().abort();
    p.wr().abort();
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
fn reset_and_zero() {
    let mut p = fixed_capacity_pipebuf!(10);
    p.wr().append(b"0123456789");
    assert_eq!(b"0123456789", p.rd().data());
    p.reset_and_zero();
    assert_eq!([0; 10], p.wr().space(10));
    assert_eq!([0; 10], p.wr().try_space(10).unwrap());
    assert_eq!(0, p.rd().len());
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[test]
fn with_capacity() {
    let mut p = PipeBuf::with_capacity(10);
    assert!(p.wr().free_space().is_none());
    p.wr().append(b"0123456789");
    p.wr().append(b"ABCDEFGHIJ");
    assert_eq!(b"0123456789ABCDEFGHIJ", p.rd().data());
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[test]
fn create_with_new() {
    let mut p = PipeBuf::new();
    assert!(p.wr().free_space().is_none());
    p.wr().try_space(23).unwrap()[..10].copy_from_slice(b"0123456789");
    p.wr().commit(10);
    p.wr().space(17)[..10].copy_from_slice(b"ABCDEFGHIJ");
    p.wr().commit(10);
    assert_eq!(b"0123456789ABCDEFGHIJ", p.rd().data());
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[test]
fn create_with_new_u16() {
    let mut p = PipeBuf::<u16>::new();
    p.wr().try_space(13).unwrap()[..5].copy_from_slice(&[0, 1, 2, 3, 4]);
    p.wr().commit(5);
    p.wr().space(9)[..7].copy_from_slice(&[5, 6, 7, 8, 9, 10, 11]);
    p.wr().commit(7);
    assert_eq!([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], p.rd().data());
    p.rd().consume(6);
    assert_eq!([6, 7, 8, 9, 10, 11], p.rd().data());
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[test]
fn create_with_new_char() {
    let mut p = PipeBuf::<char>::new();
    p.wr().try_space(13).unwrap()[..5].copy_from_slice(&['0', '1', '2', '3', '4']);
    p.wr().commit(5);
    p.wr().space(9)[..7].copy_from_slice(&['a', 'b', 'c', 'd', 'e', 'f', 'g']);
    p.wr().commit(7);
    assert_eq!(
        ['0', '1', '2', '3', '4', 'a', 'b', 'c', 'd', 'e', 'f', 'g'],
        p.rd().data()
    );
    p.rd().consume(6);
    assert_eq!(['b', 'c', 'd', 'e', 'f', 'g'], p.rd().data());
}

/// Test that buffer shifts down properly when there is both unread
/// data and not enough space.  Test is slightly different on "alloc"
/// and "static" since Vec rounds up.
#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
fn compact_buffer() {
    let mut p = fixed_capacity_pipebuf!(13);
    p.wr().append(b"0123456789");
    assert_eq!(b"0123456789", p.rd().data());
    p.rd().consume(5);
    p.wr().append(b"ABCDE");
    assert_eq!(b"56789ABCDE", p.rd().data());
    p.rd().consume(5);
    p.wr().append(b"FGHIJ");
    assert_eq!(b"ABCDEFGHIJ", p.rd().data());
    p.rd().consume(5);
    p.wr().append(b"KLMNO");
    assert_eq!(b"FGHIJKLMNO", p.rd().data());
    p.rd().consume(5);
    p.wr().append(b"PQRST");
    assert_eq!(b"KLMNOPQRST", p.rd().data());
    p.rd().consume(5);
    p.wr().append(b"UVWXYZ");
    assert_eq!(b"PQRSTUVWXYZ", p.rd().data());
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
fn wr_tripwire() {
    let mut p = fixed_capacity_pipebuf!(10);

    macro_rules! test_trip {
        ($cb:expr) => {{
            p.reset();
            let cb = $cb;
            let t0 = p.tripwire();
            assert_eq!(false, p.is_tripped(t0));
            let mut wr = p.wr();
            assert!(t0 == wr.tripwire());
            assert_eq!(false, wr.is_tripped(t0));
            cb(wr.reborrow());
            assert_eq!(true, wr.is_tripped(t0));
            assert_eq!(true, p.is_tripped(t0));
        }};
    }

    test_trip!(|mut wr: PBufWr| wr.append(b"0"));
    test_trip!(|mut wr: PBufWr| wr.push());
    test_trip!(|mut wr: PBufWr| wr.close());
    test_trip!(|mut wr: PBufWr| wr.abort());
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
fn rd_tripwire() {
    let mut p = fixed_capacity_pipebuf!(10);

    macro_rules! test_trip {
        ($eof:expr, $cb:expr) => {{
            let cb = $cb;
            p.reset();
            p.wr().append(b"0123456789");
            if $eof {
                p.wr().close();
            } else {
                p.wr().push();
            }
            let t0 = p.tripwire();
            assert_eq!(false, p.is_tripped(t0));
            let mut rd = p.rd();
            assert!(t0 == rd.tripwire());
            assert_eq!(false, rd.is_tripped(t0));
            cb(rd.reborrow());
            assert_eq!(true, rd.is_tripped(t0));
            assert_eq!(true, p.is_tripped(t0));
        }};
    }

    test_trip!(false, |mut rd: PBufRd| rd.consume(1));
    test_trip!(true, |mut rd: PBufRd| rd.consume(1));
    test_trip!(false, |mut rd: PBufRd| assert!(rd.consume_push()));
    test_trip!(true, |mut rd: PBufRd| assert!(rd.consume_eof()));
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
fn forward() {
    let mut p = fixed_capacity_pipebuf!(10);
    let mut q = fixed_capacity_pipebuf!(10);

    macro_rules! forward {
        () => {
            p.rd().forward(q.wr());
        };
    }

    p.wr().append(b"01234");
    assert!(!p.rd().is_empty());
    forward!();
    assert!(p.rd().is_empty());
    assert_eq!(b"01234", q.rd().data());
    q.rd().consume(5);
    assert!(!q.rd().consume_push());
    assert!(!q.rd().consume_eof());

    p.wr().append(b"ABC");
    p.wr().push();
    forward!();
    assert_eq!(b"ABC", q.rd().data());
    q.rd().consume(3);
    assert!(q.rd().consume_push());
    assert!(!q.rd().consume_eof());

    p.wr().close();
    forward!();
    assert!(q.rd().data().is_empty());
    assert!(!q.rd().consume_push());
    assert!(q.rd().consume_eof());
    assert!(!q.rd().is_aborted());

    p.reset();
    q.reset();

    p.wr().abort();
    forward!();
    assert!(q.rd().data().is_empty());
    assert!(!q.rd().consume_push());
    assert!(q.rd().consume_eof());
    assert!(q.rd().is_aborted());
}

#[cfg(any(feature = "std"))]
#[test]
fn read_trait() {
    use std::io::{ErrorKind, Read};

    let mut p = fixed_capacity_pipebuf!(10);

    // Read with bytes available
    let mut buf = [0; 10];
    p.wr().append(b"01234");
    assert!(matches!(p.rd().read(buf.as_mut_slice()), Ok(5)));
    assert_eq!(*b"01234", buf[..5]);

    // Read with no bytes available
    match p.rd().read(buf.as_mut_slice()) {
        Err(e) if e.kind() == ErrorKind::WouldBlock => (),
        _ => panic!("Empty read didn't give WouldBlock"),
    }

    // Read at aborted EOF
    p.wr().abort();
    match p.rd().read(buf.as_mut_slice()) {
        Err(e) if e.kind() == ErrorKind::ConnectionAborted => (),
        _ => panic!("Aborted read didn't give ConnectionAborted"),
    }

    // Read at normal EOF with data
    p.reset();
    p.wr().append(b"ABCD");
    p.wr().close();
    assert!(matches!(p.rd().read(buf.as_mut_slice()), Ok(4)));
    assert_eq!(*b"ABCD", buf[..4]);
    assert!(matches!(p.rd().read(buf.as_mut_slice()), Ok(0)));
}

#[cfg(any(feature = "std"))]
#[test]
fn output_to() {
    use std::io::{ErrorKind, Result, Write};
    #[derive(Default)]
    struct Dest {
        buf: Vec<u8>,
        flushed: bool,
        write_err_wouldblock: bool,
        write_err_interrupted: bool,
        flush_err_wouldblock: bool,
        flush_err_interrupted: bool,
    }
    impl Write for Dest {
        fn write(&mut self, data: &[u8]) -> Result<usize> {
            if self.write_err_wouldblock {
                self.write_err_wouldblock = false;
                Err(ErrorKind::WouldBlock.into())
            } else if self.write_err_interrupted {
                self.write_err_interrupted = false;
                Err(ErrorKind::Interrupted.into())
            } else {
                self.buf.extend_from_slice(data);
                Ok(data.len())
            }
        }
        fn flush(&mut self) -> Result<()> {
            if self.flush_err_wouldblock {
                self.flush_err_wouldblock = false;
                Err(ErrorKind::WouldBlock.into())
            } else if self.flush_err_interrupted {
                self.flush_err_interrupted = false;
                Err(ErrorKind::Interrupted.into())
            } else {
                self.flushed = true;
                Ok(())
            }
        }
    }
    let mut dest = Dest::default();

    // Test data output (and ignoring Interrupted on write)
    let mut p = fixed_capacity_pipebuf!(10);
    dest.write_err_interrupted = true;
    p.wr().append(b"0123456");
    assert!(p.rd().output_to(&mut dest, false).is_ok());
    assert_eq!(b"0123456", dest.buf.as_slice());
    assert_eq!(false, dest.flushed);

    // Test "push" -> "flush"
    p.wr().append(b"789");
    p.wr().push();
    assert!(p.rd().output_to(&mut dest, false).is_ok());
    assert_eq!(b"0123456789", dest.buf.as_slice());
    assert_eq!(true, dest.flushed);

    // Test force_flush == true (and ignoring Interrupted on flush)
    dest.flushed = false;
    dest.flush_err_interrupted = true;
    p.wr().append(b"ABCD");
    assert!(p.rd().output_to(&mut dest, true).is_ok());
    assert_eq!(b"0123456789ABCD", dest.buf.as_slice());
    assert_eq!(true, dest.flushed);

    // Test write error passthrough
    dest.write_err_wouldblock = true;
    p.wr().append(b"EFG");
    match p.rd().output_to(&mut dest, false) {
        Err(e) if e.kind() == ErrorKind::WouldBlock => (),
        _ => panic!("Expecting WouldBlock"),
    }

    // Test flush error passthrough
    dest.flush_err_wouldblock = true;
    p.wr().push();
    match p.rd().output_to(&mut dest, false) {
        Err(e) if e.kind() == ErrorKind::WouldBlock => (),
        _ => panic!("Expecting WouldBlock"),
    }
    assert_eq!(b"0123456789ABCDEFG", dest.buf.as_slice());
}

#[cfg(any(feature = "std"))]
#[test]
#[should_panic]
fn output_to_panic() {
    use std::io::{Result, Write};
    struct Dest;
    impl Write for Dest {
        fn write(&mut self, data: &[u8]) -> Result<usize> {
            Ok(data.len() + 1) // Breaks contract of `Write`
        }
        fn flush(&mut self) -> Result<()> {
            Ok(())
        }
    }
    let mut dest = Dest;
    let mut p = fixed_capacity_pipebuf!(10);
    p.wr().append(b"01234");
    let _ = p.rd().output_to(&mut dest, false); // Should panic
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
fn write_with() {
    let mut p = fixed_capacity_pipebuf!(10);

    assert!(matches!(
        p.wr().write_with(10, |space| {
            if space.len() < 6 {
                // Never happens, but keeps the type-checker happy
                Err(())
            } else {
                space[..6].copy_from_slice(b"ABCDEF");
                Ok(6)
            }
        }),
        Ok(6)
    ));
    assert_eq!(b"ABCDEF", p.rd().data());
    p.rd().consume(6);

    assert!(matches!(
        p.wr().write_with(10, |_| { Err(999_u16) }),
        Err(999_u16)
    ));

    assert_eq!(
        3,
        p.wr().write_with_noerr(10, |space| {
            space[..3].copy_from_slice(b"987");
            3
        })
    );
    assert_eq!(b"987", p.rd().data());
    p.rd().consume(3);
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
fn exceeds_limit() {
    let mut p = fixed_capacity_pipebuf!(10);
    assert!(!p.wr().exceeds_limit(5));
    p.wr().append(b"01234");
    assert!(!p.wr().exceeds_limit(5));
    p.wr().append(b"5");
    assert!(p.wr().exceeds_limit(5));
}

#[cfg(any(feature = "std"))]
#[test]
fn input_from() {
    use std::io::{ErrorKind, Read, Result};
    #[derive(Default)]
    struct Source {
        data: Vec<u8>,
        eof: bool,
        err_interrupted: bool,
    }
    impl Read for Source {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            if self.err_interrupted {
                self.err_interrupted = false;
                Err(ErrorKind::Interrupted.into())
            } else if self.data.is_empty() {
                if self.eof {
                    Ok(0)
                } else {
                    Err(ErrorKind::WouldBlock.into())
                }
            } else {
                let len = buf.len().min(self.data.len());
                buf[..len].copy_from_slice(&self.data[..len]);
                self.data.drain(..len);
                Ok(len)
            }
        }
    }

    let mut p = fixed_capacity_pipebuf!(20);
    let mut input = Source::default();
    input.data.extend_from_slice(b"01234567");
    input.err_interrupted = true;
    assert!(p.wr().input_from(&mut input, 5).is_ok());
    assert_eq!(5, p.rd().len());
    match p.wr().input_from(&mut input, 5) {
        Err(e) if e.kind() == ErrorKind::WouldBlock => (),
        _ => panic!("Expecting WouldBlock"),
    }
    assert_eq!(8, p.rd().len());
    input.data.extend_from_slice(b"8");
    input.eof = true;
    assert!(p.wr().input_from(&mut input, 5).is_ok());
    assert_eq!(9, p.rd().len());
    assert_eq!(true, p.wr().is_eof());
    assert_eq!(b"012345678", p.rd().data());

    // Reading after EOF, does nothing
    input.data.extend_from_slice(b"9");
    assert!(p.wr().input_from(&mut input, 5).is_ok());
    assert_eq!(9, p.rd().len());
}

#[cfg(any(feature = "std"))]
#[test]
fn write_trait() {
    use std::io::Write;

    let mut p = fixed_capacity_pipebuf!(10);
    assert!(p.wr().write(b"012345").is_ok());
    assert_eq!(b"012345", p.rd().data());
    assert_eq!(false, p.rd().consume_push());

    assert!(p.wr().flush().is_ok());
    assert_eq!(true, p.rd().consume_push());
}

#[cfg(any(feature = "std", feature = "alloc", feature = "static"))]
#[test]
fn pipebufpair_fixed() {
    let mut p = fixed_capacity_pipebufpair!(10);

    p.upper().wr.append(b"01234");
    p.upper().wr.close();
    assert_eq!(b"01234", p.lower().rd.data());
    assert_eq!(b"01234", p.right().rd.data());
    p.lower().rd.consume(5);
    assert_eq!(true, p.lower().rd.consume_eof());
    p.reset();
    assert_eq!(false, p.lower().rd.consume_eof());

    p.lower().wr.append(b"56789");
    p.lower().wr.abort();
    assert_eq!(b"56789", p.upper().rd.data());
    assert_eq!(b"56789", p.left().rd.data());
    assert_eq!(true, p.upper().rd.consume_eof());
    p.upper().rd.consume(5);
    p.reset_and_zero();
    assert_eq!(false, p.upper().rd.consume_eof());
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[test]
fn pipebufpair_var() {
    let mut p = PipeBufPair::default();
    let ut = p.upper().tripwire();
    let lt = p.lower().tripwire();
    p.upper().wr.append(b"01234");
    assert!(ut != p.upper().tripwire());
    assert!(lt != p.lower().tripwire());

    let mut p = PipeBufPair::with_capacities(10, 10);
    let ut = p.upper().tripwire();
    let lt = p.lower().tripwire();
    p.lower().wr.append(b"01234");
    assert!(ut != p.upper().tripwire());
    assert!(lt != p.lower().tripwire());
}
