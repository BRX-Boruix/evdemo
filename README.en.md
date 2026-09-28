# evdemo

An **end-to-end acceptance program** for BORUIX, verifying that the keyboard event stream can be consumed from user space and yields the correct bytes.

[简体中文](README.md)

## The data path

```
keyboard interrupt → event node (16-byte records) → translation layer (keymap, modifiers) → this program consuming and echoing each record
```

## What it tests

1. **Event records are readable from user space** — the program opens the event node and reads records through real syscalls;
2. **Records parse correctly** — each 16-byte record is decoded into a key event;
3. **The bytes produced are correct** — after keymap and modifier handling, the emitted character matches the key pressed;
4. **An empty read blocks in the kernel**, rather than returning "end of file" or spinning — when the event ring is empty the read suspends the process, which is woken when a key arrives. **No spinning in user space.**

## Diagnostic form

This program **deliberately bypasses** the regular event source component and takes the "open → read → parse" path directly, printing the true result of each read (byte count and error code). That is so that problems like "read yields nothing after a backlog drains" can be pinned down with **direct evidence** — the regular component would hide the intermediate detail.

Behaviour is equivalent to the regular component (the same chain). Acceptance for the component path itself is in [`evsrcdemo`](https://github.com/BRX-Boruix/evsrcdemo).

## Honest boundary

This program proves "the event stream can be consumed from user space, yields correct bytes, and an empty read blocks in the kernel".

It does **not** prove that the `shell` has switched to this input source — wiring the component up is a separate matter.

## Building

```bash
cargo build --release
```

## Layout

```
evdemo/
├── Cargo.toml    # package definition
├── build.rs      # injects the linker script
├── linker.ld     # user-space section layout
└── src/
    └── main.rs   # event reading, parsing, and echoing
```

## Related projects

- [`evsrcdemo`](https://github.com/BRX-Boruix/evsrcdemo) — acceptance for the regular event source component path
- [`blkdemo`](https://github.com/BRX-Boruix/blkdemo) — the control for the legacy byte path
- [`libline`](https://github.com/BRX-Boruix/libline) — provides the event source component

## License

MIT License, copyright Yang Borui. See [LICENSE](LICENSE).
