# Working on issues

When you pick up a GitHub issue, mark it as in progress before you start:

```sh
gh issue edit <number> --add-label "in progress"
```

Put `Closes #<number>` in the pull request description. When the pull request
is merged the issue closes and the
[In progress label](.github/workflows/in-progress-label.yml) workflow removes
the label. The same workflow also adds the label when a pull request that
closes the issue is opened, in case it was missed.

# Keep the server and client in sync

The browser client in `crates/server/assets/index.html` has its own copy of the
protocol version (`const PROTOCOL_VERSION`). It must always equal
`PROTOCOL_VERSION` in `crates/protocol/src/lib.rs`, otherwise the client
refuses to talk to the server.

Whenever you change what the server sends (messages, fields, event log text,
catalog contents), bump `PROTOCOL_VERSION` in the protocol crate and update the
client in the same pull request, including any client code that reads the
changed data.
