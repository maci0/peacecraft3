# Project LAN discovery collection

The project's advertisement payload contains a name, map and player counts;
its parser supplies a placeholder address. Collection previously compared that
placeholder with the datagram source and dropped valid replies. It also applied
a read timeout to a nonblocking socket, then returned on `WouldBlock` before a
delayed response arrived.

Collection now binds each parsed advertisement to its received UDP source and
temporarily blocks for the existing discovery deadline. It attempts to restore
nonblocking game polling after success and collection errors. A failed restoration
returns `DiscoveryRestore`, preserving the restoration error and any collection
error. This does not guarantee a usable mode after an OS restoration failure.

The fresh ephemeral loopback test executes request, host response, collection,
peer registration and subsequent game-packet polling. Private deterministic
socket/clock substitutions exercise delayed responses, malformed input, timeout
configuration, receive failures and simultaneous restoration/collection failures
without depending on elapsed-time performance thresholds. The receive/send span
fix and shared datagram codec retain their existing paths.

This is the project protocol, not classic Warcraft III LAN compatibility or
source authentication. Broadcast routing and application lobby wiring remain
unverified. Advertisement parsing defaults and sorting/deduplication retain their
existing policy. Browser builds exclude UDP transport; their shared codec checks
do not establish browser UDP execution.
