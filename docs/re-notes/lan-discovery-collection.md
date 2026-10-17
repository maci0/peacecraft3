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
peer registration and subsequent game-packet polling. Collection in that test
returns on the first valid advertisement. `Finder::find` still waits the full
production window so a later host is not dropped. A zero window returns without
reading. Loopback datagram and advertisement waits are hang guards: they yield
until the packet is queued and fail only if it never arrives. They are not
elapsed-time speed gates. The receive/send span fix and shared datagram codec
retain their existing paths.

This is the project protocol, not classic Warcraft III LAN compatibility or
source authentication. Broadcast routing and application lobby wiring remain
unverified. Advertisement parsing defaults and sorting/deduplication retain their
existing policy. Browser builds exclude UDP transport; their shared codec checks
do not establish browser UDP execution.
