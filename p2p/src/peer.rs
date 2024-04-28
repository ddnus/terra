use std::sync::Arc;
use std::future::Future;

use tokio::net::{TcpListener, TcpStream};
use crate::Command;
use tokio::sync::{broadcast, mpsc, Semaphore};
use tokio::time::{self, Duration};
use tracing::{error, info, instrument};


#[derive(Debug)]
struct Listener {
    /// Shared database handle.
    ///
    /// Contains the key / value store as well as the broadcast channels for
    /// pub/sub.
    ///
    /// This holds a wrapper around an `Arc`. The internal `Db` can be
    /// retrieved and passed into the per connection state (`Handler`).
    // db_holder: DbDropGuard,

    listener: TcpListener,

    limit_connections: Arc<Semaphore>,

    notify_shutdown: broadcast::Sender<()>,

    shutdown_complete_tx: mpsc::Sender<()>,
}

#[derive(Debug)]
struct Handler {

    db: Db,

    connection: Connection,

    shutdown: Shutdown,

    _shutdown_complete: mpsc::Sender<()>,
}

const MAX_CONNECTIONS: usize = 250;


pub async fn run(listener: TcpListener, shutdown: impl Future) {
    let (notify_shutdown, _) = broadcast::channel(1);
    let (shutdown_complete_tx, mut shutdown_complete_rx) = mpsc::channel(1);

    // Initialize the listener state
    let mut server = Listener {
        listener,
        db_holder: DbDropGuard::new(),
        limit_connections: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
        notify_shutdown,
        shutdown_complete_tx,
    };

    tokio::select! {
        res = server.run() => {
            // If an error is received here, accepting connections from the TCP
            // listener failed multiple times and the server is giving up and
            // shutting down.
            //
            // Errors encountered when handling individual connections do not
            // bubble up to this point.
            if let Err(err) = res {
                error!(cause = %err, "failed to accept");
            }
        }
        _ = shutdown => {
            // The shutdown signal has been received.
            info!("shutting down");
        }
    }

    // Extract the `shutdown_complete` receiver and transmitter
    // explicitly drop `shutdown_transmitter`. This is important, as the
    // `.await` below would otherwise never complete.
    let Listener {
        shutdown_complete_tx,
        notify_shutdown,
        ..
    } = server;

    // When `notify_shutdown` is dropped, all tasks which have `subscribe`d will
    // receive the shutdown signal and can exit
    drop(notify_shutdown);
    // Drop final `Sender` so the `Receiver` below can complete
    drop(shutdown_complete_tx);

    // Wait for all active connections to finish processing. As the `Sender`
    // handle held by the listener has been dropped above, the only remaining
    // `Sender` instances are held by connection handler tasks. When those drop,
    // the `mpsc` channel will close and `recv()` will return `None`.
    let _ = shutdown_complete_rx.recv().await;
}


impl Listener {

    async fn run(&mut self) -> crate::Result<()> {
        info!("accepting inbound connections");

        loop {
            // Wait for a permit to become available
            //
            // `acquire_owned` returns a permit that is bound to the semaphore.
            // When the permit value is dropped, it is automatically returned
            // to the semaphore.
            //
            // `acquire_owned()` returns `Err` when the semaphore has been
            // closed. We don't ever close the semaphore, so `unwrap()` is safe.
            let permit = self
                .limit_connections
                .clone()
                .acquire_owned()
                .await
                .unwrap();

            // Accept a new socket. This will attempt to perform error handling.
            // The `accept` method internally attempts to recover errors, so an
            // error here is non-recoverable.
            let socket = self.accept().await?;

            // Create the necessary per-connection handler state.
            let mut handler = Handler {
                // Get a handle to the shared database.
                db: self.db_holder.db(),

                // Initialize the connection state. This allocates read/write
                // buffers to perform redis protocol frame parsing.
                connection: Connection::new(socket),

                // Receive shutdown notifications.
                shutdown: Shutdown::new(self.notify_shutdown.subscribe()),

                // Notifies the receiver half once all clones are
                // dropped.
                _shutdown_complete: self.shutdown_complete_tx.clone(),
            };

            // Spawn a new task to process the connections. Tokio tasks are like
            // asynchronous green threads and are executed concurrently.
            tokio::spawn(async move {
                // Process the connection. If an error is encountered, log it.
                if let Err(err) = handler.run().await {
                    error!(cause = ?err, "connection error");
                }
                // Move the permit into the task and drop it after completion.
                // This returns the permit back to the semaphore.
                drop(permit);
            });
        }
    }

    /// Accept an inbound connection.
    ///
    /// Errors are handled by backing off and retrying. An exponential backoff
    /// strategy is used. After the first failure, the task waits for 1 second.
    /// After the second failure, the task waits for 2 seconds. Each subsequent
    /// failure doubles the wait time. If accepting fails on the 6th try after
    /// waiting for 64 seconds, then this function returns with an error.
    async fn accept(&mut self) -> crate::Result<TcpStream> {
        let mut backoff = 1;

        // Try to accept a few times
        loop {
            // Perform the accept operation. If a socket is successfully
            // accepted, return it. Otherwise, save the error.
            match self.listener.accept().await {
                Ok((socket, _)) => return Ok(socket),
                Err(err) => {
                    if backoff > 64 {
                        // Accept has failed too many times. Return the error.
                        return Err(err.into());
                    }
                }
            }

            // Pause execution until the back off period elapses.
            time::sleep(Duration::from_secs(backoff)).await;

            // Double the back off
            backoff *= 2;
        }
    }
}

impl Handler {
    /// Process a single connection.
    ///
    /// Request frames are read from the socket and processed. Responses are
    /// written back to the socket.
    ///
    /// Currently, pipelining is not implemented. Pipelining is the ability to
    /// process more than one request concurrently per connection without
    /// interleaving frames. See for more details:
    /// https://redis.io/topics/pipelining
    ///
    /// When the shutdown signal is received, the connection is processed until
    /// it reaches a safe state, at which point it is terminated.
    #[instrument(skip(self))]
    async fn run(&mut self) -> crate::Result<()> {
        // As long as the shutdown signal has not been received, try to read a
        // new request frame.
        while !self.shutdown.is_shutdown() {
            // While reading a request frame, also listen for the shutdown
            // signal.
            let maybe_frame = tokio::select! {
                res = self.connection.read_frame() => res?,
                _ = self.shutdown.recv() => {
                    // If a shutdown signal is received, return from `run`.
                    // This will result in the task terminating.
                    return Ok(());
                }
            };

            // If `None` is returned from `read_frame()` then the peer closed
            // the socket. There is no further work to do and the task can be
            // terminated.
            let frame = match maybe_frame {
                Some(frame) => frame,
                None => return Ok(()),
            };

            // Convert the redis frame into a command struct. This returns an
            // error if the frame is not a valid redis command or it is an
            // unsupported command.
            let cmd = Command::from_frame(frame)?;

            // Logs the `cmd` object. The syntax here is a shorthand provided by
            // the `tracing` crate. It can be thought of as similar to:
            //
            // ```
            // debug!(cmd = format!("{:?}", cmd));
            // ```
            //
            // `tracing` provides structured logging, so information is "logged"
            // as key-value pairs.
            debug!(?cmd);

            // Perform the work needed to apply the command. This may mutate the
            // database state as a result.
            //
            // The connection is passed into the apply function which allows the
            // command to write response frames directly to the connection. In
            // the case of pub/sub, multiple frames may be send back to the
            // peer.
            cmd.apply(&self.db, &mut self.connection, &mut self.shutdown)
                .await?;
        }

        Ok(())
    }
}
