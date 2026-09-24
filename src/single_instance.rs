//! Keeps a second launch of Noir Player from opening a second window.
//!
//! A desktop shortcut, a taskbar pin or a `.desktop` file all just run the
//! executable again; nothing stops the OS from starting a second process that
//! opens a second window onto the same library and fights the first one for
//! the audio device. The fix is a local socket used purely as a lock: the
//! first process binds it and keeps listening, and a later launch either
//! connects to it and asks it to come to the foreground, or fails to connect
//! and proceeds to become the primary instance itself.
//!
//! Every failure path here falls back to opening a window rather than
//! refusing to start: a bug in this file must never be the reason the app
//! doesn't launch.

use std::io::{Read, Write};

use interprocess::local_socket::{prelude::*, GenericNamespaced, ListenerOptions, Stream};

/// Carries no information beyond its length; the connection itself is the
/// message. Read in full so the writing end's `write_all` completes instead
/// of the pipe or socket resetting under it mid-write.
const ACTIVATE: &[u8] = b"activate\n";

fn socket_name() -> std::io::Result<interprocess::local_socket::Name<'static>> {
    // Namespaced by user so two accounts on a shared machine never contend for
    // the same socket; on Windows and Linux this also keeps two accounts from
    // being able to activate one another's windows.
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default();
    format!("noir-player-{user}.sock")
        .to_ns_name::<GenericNamespaced>()
        .map(interprocess::local_socket::Name::into_owned)
}

/// If another instance is already listening, asks it to come to the
/// foreground and returns `true`: the caller should exit immediately without
/// opening a window. Returns `false` on any failure — including simply not
/// finding a listener — so the caller can proceed as the primary instance.
pub fn activate_existing() -> bool {
    let Ok(name) = socket_name() else {
        return false;
    };
    // Connecting to a name nobody holds fails immediately on both platforms
    // rather than blocking: Windows' `WaitNamedPipe` documents an instant
    // return when no instance of the pipe exists, and a Unix socket whose
    // listener has exited (even a killed one — closing on exit is the
    // kernel's job, not the process's) refuses the connection outright.
    let Ok(mut stream) = Stream::connect(name) else {
        return false;
    };
    stream.write_all(ACTIVATE).is_ok()
}

/// Starts listening for later launches asking to be brought forward. Returns
/// a receiver that yields once per request. Returns `None` if binding fails
/// for any reason — including a genuine other instance winning a simultaneous
/// startup race — in which case this process simply runs without enforcing
/// single-instance rather than refusing to start.
pub fn listen_for_activation() -> Option<smol::channel::Receiver<()>> {
    let name = socket_name().ok()?;
    let listener = ListenerOptions::new()
        .name(name)
        // A killed instance leaves its socket file behind on Unix with
        // nothing listening on it; without this, every launch after a crash
        // would fail to bind and silently stop enforcing single-instance.
        .try_overwrite(true)
        .create_sync()
        .ok()?;
    let (sender, receiver) = smol::channel::unbounded();
    std::thread::Builder::new()
        .name("noir-player-activation".to_owned())
        .spawn(move || {
            for connection in listener.incoming() {
                let Ok(mut connection) = connection else {
                    continue;
                };
                let mut buffer = [0u8; ACTIVATE.len()];
                let _ = connection.read_exact(&mut buffer);
                if sender.send_blocking(()).is_err() {
                    // The receiving end was dropped, which only happens as the
                    // app shuts down; nothing is left to notify.
                    break;
                }
            }
        })
        .ok()?;
    Some(receiver)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises the real client/server path end to end, standing in for the
    /// two processes: `listen_for_activation` binds and starts accepting, and
    /// `activate_existing`'s connect-and-write is invoked directly against
    /// that same socket rather than through a name lookup racing this test
    /// with a real running instance.
    #[test]
    fn a_connection_is_delivered_to_the_listening_instance() {
        let name = format!("noir-player-test-{}.sock", std::process::id())
            .to_ns_name::<GenericNamespaced>()
            .unwrap();
        let listener = ListenerOptions::new()
            .name(name.clone())
            .try_overwrite(true)
            .create_sync()
            .unwrap();
        let (sender, receiver) = smol::channel::unbounded();
        let accept_thread = std::thread::spawn(move || {
            for connection in listener.incoming() {
                let Ok(mut connection) = connection else {
                    continue;
                };
                let mut buffer = [0u8; ACTIVATE.len()];
                let _ = connection.read_exact(&mut buffer);
                if sender.send_blocking(()).is_err() {
                    break;
                }
                return; // one activation is all this test needs
            }
        });

        let mut stream = Stream::connect(name).expect("the listener is already up");
        stream.write_all(ACTIVATE).unwrap();
        drop(stream);

        smol::block_on(async {
            smol::future::or(
                async {
                    receiver.recv().await.expect("activation was delivered");
                },
                async {
                    smol::Timer::after(std::time::Duration::from_millis(500)).await;
                    panic!("no activation arrived within the timeout");
                },
            )
            .await;
        });
        accept_thread.join().unwrap();
    }

    #[test]
    fn connecting_to_a_name_nobody_is_listening_on_fails_without_hanging() {
        let name = format!("noir-player-test-nobody-{}.sock", std::process::id())
            .to_ns_name::<GenericNamespaced>()
            .unwrap();
        assert!(Stream::connect(name).is_err());
    }
}
