# Headless Mode

`muzon-core` can run as a standalone process so the UI can be
closed while music keeps playing, and so external tools (the
future mobile client, scripts) can talk to the same instance.
The cross-process variant uses a Unix-domain socket inside
`XDG_RUNTIME_DIR` (with a `/tmp/muzon-$UID.sock` fallback) and
length-prefixed `postcard` framing. The same wire contract as
the in-process variant from issue 0011 is used; only the
transport changes.

## Start the core

```sh
$ muzon --headless
muzon: v0.1.0 starting (config=/home/alice/.config/muzon/config.toml, log=/home/alice/.local/share/muzon/logs/muzon.log, format=Human)
muzon-audio: pipeline built; playbin3 attached
muzon --headless: socket=/run/user/1000/muzon.sock
```

The core runs in the foreground. On `SIGINT` or `SIGTERM`, it
stops the audio engine cleanly, removes the socket file,
and exits 0.

A systemd user unit is the recommended way to keep the core
running across UI sessions; the unit file itself is a
v1.0.0 deliverable (the v0.2.0 docs link to a minimal unit).

## Control it with `muzonctl`

`muzonctl` is a thin CLI client that talks to the running
core over the same socket. If the core is not running,
`muzonctl` exits non-zero with a clear error:

```sh
$ muzonctl status
muzonctl: no muzon-core running at /run/user/1000/muzon.sock; start one with `muzon --headless` (error: Io { ... })
$ echo $?
3
```

Subcommands:

```text
muzonctl ping                                  # liveness probe
muzonctl status                                # current playback status
muzonctl play <file>                           # start playback
muzonctl pause | resume | stop                 # transport controls
muzonctl seek <ms>                             # seek to position
muzonctl volume <0-100>                        # set output volume
muzonctl queue-add <file>                      # append to queue
muzonctl queue-list                            # print the queue
muzonctl queue-clear                           # clear the queue
muzonctl repeat <off|one|all>                  # repeat mode
muzonctl shuffle <on|off>                      # toggle shuffle
muzonctl list-skins                             # list installed skins
muzonctl set-skin <id>                         # set the active skin
muzonctl list-tracks                            # library track count
```

The `--socket <PATH>` flag overrides the default socket
path. The `$MUZON_SOCKET_PATH` env var on the headless side
does the same for the server.

## Socket security

- The socket is created with mode `0600` and owned by the
  current user. Only the same user can talk to the core.
- No network exposure. The IPC is over a Unix-domain
  socket; the kernel enforces the perms.
- No telemetry: the headless core has no outbound network
  calls. The `network.explicit_opt_in` validator in
  `muzon-core::config` rejects any non-explicit network use
  (decision 0001-N4).

## Tauri shell integration

When the Tauri shell from issue 0011 starts up, it tries to
connect to a running headless core via the socket. If the
connect succeeds, the shell attaches to the running core
instead of spawning an in-process one. If the connect fails
(the core is not running), the shell spawns an in-process
core. If the running core has a different config or library
DB path, the shell warns the user and asks whether to take
over or attach.

## Implementation

- The IPC server is `muzon-ipc::server::serve` in
  `crates/muzon-ipc/src/server.rs`. It binds the Unix socket,
  spawns a tokio task per connection, and dispatches each
  request to the supplied async handler closure. The wire
  format is 4-byte little-endian length + `postcard` payload.
- The client is `muzon-ipc::client::IpcClient` in
  `crates/muzon-ipc/src/client.rs`. It connects, sends a
  request, reads the response. 5-second read/write timeout
  prevents a stuck socket from blocking the caller.
- The headless binary is `muzon --headless` in
  `crates/muzon-cli/src/headless.rs`. It builds the in-process
  `CoreHandle` from `muzon-ui`, binds the socket via
  `muzon-ipc::serve`, and waits for SIGINT/SIGTERM.
- The CLI client is `muzonctl <subcommand>` in
  `crates/muzon-cli/src/ctl.rs`. It connects to the socket
  and forwards each subcommand.
- D-Bus / MPRIS2 is a v1.0.0 deliverable; the Unix socket is
  the v0.2.0+ IPC surface.
