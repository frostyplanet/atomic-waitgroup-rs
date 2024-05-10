# atomic-waitgroup

A waitgroup support async with advanced features,
implemented with atomic operations to reduce locking in mind.

## Features

* wait_to() is supported to wait for a value larger than zero.

* wait() & wait_to() can be canceled by tokio::time::timeout or futures::select!.

* Assumes only one thread calls wait(). If multiple concurrent wait() is detected,
will panic for this invalid usage.

* done() can be called by multiple coroutines other than the one calls wait().
