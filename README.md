# Quadrant

Quadrant is a local-first, cross-platform four-quadrant task manager being rewritten in Rust and Slint.

The rewrite is in **M0 (repository foundation)**. The legacy .NET application, when present locally under `legacy/dotnet-reference/`, is a read-only behavioral reference and is not part of the Rust application.

## Build

```console
cargo run -p quadrant-app
```

The project is licensed under [GPL-3.0-only](LICENSE). UI primitives are derived from [`owu/wsl-dashboard`](https://github.com/owu/wsl-dashboard), and bundled icons come from [Microsoft Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons). See [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
