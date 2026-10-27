# TsClientlib [![docs.rs](https://docs.rs/tsclientlib/badge.svg)](https://docs.rs/tsclientlib)
tsclientlib is a library which makes it simple to create TeamSpeak clients
and bots.

For a full client application, you might want to have a look at [Qint].

If more power over the internals of a connection is needed, the `unstable` feature can be
enabled. Beware that functionality behind this feature may change on any minor release.

The base class of this library is the [`Connection`]. One instance of this
struct manages a single connection to a server.

[Qint]: https://github.com/ReSpeak/Qint

## License
Licensed under either of

 * [Apache License, Version 2.0](../LICENSE-APACHE)
 * [MIT license](../LICENSE-MIT)

at your option.

### Audio and cryptography in this fork

Audio uses `opus` with the bundled `opusic-sys` native library. CMake and a C compiler
are required; a system libopus installation is not required. The `audio` feature
remains enabled by default. `AudioQueue::get_decoder` and audio errors now expose
`opus` types. The old `audiopus-unstable` feature is removed; experimental decoder
controls from the audiopus fork are not supported.

P-256 uses `p256 0.14` and `elliptic-curve >=0.14.3` within the 0.14 series.
This selects the patched `crypto-bigint` 0.7 series instead of the vulnerable
0.5 series. TeamSpeak identity formats, UIDs, ECDH output, and DER signature
encoding are preserved.
