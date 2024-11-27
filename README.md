# SkyFeed Indexer
ATProto/Bluesky Indexer, powered by [SurrealDB](https://github.com/surrealdb/surrealdb) and [Jetstream](https://github.com/bluesky-social/jetstream), written in [Rust](https://www.rust-lang.org/).

The indexer attaches a websocket to a Jetstream endpoint and converts all received events to SurrealDB queries. Temporary outtages are handled by the cursor system, which allows the indexer to resume indexing from the last known event.

The database can then be used to run powerful queries on the network data or build advanced custom feeds. All skyfeed.xyz feeds are powered by this service.

## Installation
1. Install the latest stable rust compiler from [rustup.rs](https://rustup.rs/).
2. Install either onto your system or into a docker container a [SurrealDB](https://surrealdb.com/docs/surrealdb/installation/running).
3. Generate a secure password, which may be generated using `openssl rand -base64 32` or `pwgen -s 32 1`.
4. Launch SurrealDB with the following flags: `surreal start --user root --pass <password here> --bind 127.0.0.1:8000 <dbtype>:<dbfile>`.
5. Clone the repository and run `cargo build --release`.
6. Launch the indexer with `./target/release/skyfeed-indexer [--help]`.
