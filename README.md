# atomic-waitgroup

A waitgroup support async with advanced features,
implemented with atomic operations to reduce locking in mind.

document: <https://docs.rs/atomic-waitgroup/latest/atomic_waitgroup/>

## Features

* wait_to() is supported to wait for a value larger than zero.

* wait() & wait_to() can be canceled by tokio::time::timeout or futures::select!.

* Assumes only one thread calls wait(). If multiple concurrent wait() is detected,
will panic for this invalid usage.

* done() & wait() is allowed to called concurrently.

* add() & done() is allowed to called concurrently.

* Assumes add() & wait() are in the same thread.
