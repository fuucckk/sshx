use std::convert::Infallible;
use std::env;
use std::ffi::CString;
use std::os::unix::io::{FromRawFd, RawFd};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use nix::fcntl::{fcntl, FcntlArg, OFlag};
use nix::pty;
use nix::unistd::{execv, ForkResult};
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::time;

/// Returns the default shell on this system.
fn get_default_shell() -> String {
    env::var("SHELL").unwrap_or_else(|_| String::from("/bin/bash"))
}

/// Entry point for the child process, which spawns a shell.
fn child_task(shell: &str) -> Result<Infallible> {
    let command = CString::new(shell)?;
    execv(&command, &[&command]).map_err(|e| e.into())
}

/// Entry point for the asynchronous controller.
#[tokio::main]
async fn controller_task(master_port: RawFd) -> Result<()> {
    fcntl(master_port, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;

    // Safety: The master file descriptor was created by forkpty() and has its
    // ownership transferred here. It is closed at the end of the function.
    let mut master = unsafe { File::from_raw_fd(master_port) };

    // Input to communicate with the terminal.
    let (tx, mut rx) = mpsc::channel::<Arc<[u8]>>(64);

    tokio::spawn(async move {
        // This task takes ownership of `master`, so there are no issues with
        // concurrent reads and writes to the same file.
        let mut buf = [0_u8; 2048];
        loop {
            tokio::select! {
                biased;

                message = rx.recv() => {
                    if let Some(buf) = message {
                        master.write_all(&buf[..]).await.expect("Failed to write to master");
                    } else {
                        break;
                    }
                }
                result = master.read(&mut buf) => {
                    match result {
                        Ok(n) => io::stdout().write_all(&buf[..n]).await.unwrap(),
                        Err(e) => match e.kind() {
                            io::ErrorKind::WouldBlock => {
                                // On EAGAIN (non-blocking read), wait for a little bit.
                                time::sleep(Duration::from_millis(10)).await;
                            }
                            _ => panic!("Failed to read from PTY master: {e}"),
                        },
                    }
                }
            };
        }
    });

    loop {
        let mut buf = [0_u8; 256];
        let n = io::stdin().read(&mut buf).await?;
        tx.send(buf[0..n].into()).await?;
    }
}

fn main() -> Result<()> {
    let shell = get_default_shell();
    println!("Using default shell: {shell}");

    // Safety: Child process spawned by forkpty() does no memory allocation and must
    // use only "async-signal-safe" functions.
    let result = unsafe { pty::forkpty(None, None) }?;
    match result.fork_result {
        ForkResult::Child => {
            child_task(&shell).expect("Child failed");
        }
        ForkResult::Parent { child } => {
            println!("Child has pid {child}");
            controller_task(result.master)?;
        }
    }

    Ok(())
}
