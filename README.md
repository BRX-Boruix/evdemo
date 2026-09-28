# evdemo

**简体中文** | [English](#english)

BORUIX 上的**事件流输入回显程序**——直接读取内核事件节点，把每个按键回显出来。

```
/devices/input/events → 键码转换（含修饰键）→ 回显到标准输出
```

按 `q` 退出。

---

## 它验证什么

键盘事件在内核里有一条完整的数据链：中断 → 事件推送 → 事件节（16 字节记录）→ 用户态转换层。
`evdemo` 是这条链的**端到端验收程序**：它逐条消费事件记录，把转换后的字节回显出来，证明
"事件流可以被用户态正确消费"。

它同时验证**阻塞语义**：当事件环为空时，`read` 会在**内核内挂起本进程**，而不是返回 EOF 或
让用户态自旋等待。按键到达时由中断唤醒，系统调用以"曾阻塞、请重试"返回，用户态据此重新发起
读取——这是正常的等待路径，不是错误。

## 设计：诊断形态

这个程序采用**诊断形态**——它绕开了库里的 `EventSource` 抽象，直接执行
`open → read → 解析记录 → 送入键码转换` 这四步，并把每一轮 `read` 的真实返回值打到串口。

之所以这样设计：定位"积压事件吐完之后 `read` 为什么没有产出"这类问题时，需要看到每轮读取的
原始真值（读回多少字节、错误码是什么）。行为上与 `EventSource` 等价，走的是同一条链。

它还会打印一次文件状态自述，确认打开的确是事件节点而不是终端——如果落到了别的节点上，这两个
字段会立刻暴露问题。

## 输出说明

| 输出 | 含义 |
| --- | --- |
| `[a]` `[RET]` `[BS]` `[ESC]` | 普通字符、回车、退格、Esc |
| `[x1b]` 之类 | 无法直接显示的字节，以十六进制给出 |
| `R<十六进制> t<十进制>` | 本轮读回的字节数与累计读取次数 |
| `waited` | 发生过内核阻塞等待（每 4 次打印一条，避免刷屏） |

最后的 `reads=` / `waits=` 如实汇报总读取次数与总阻塞次数。

## 它的边界

`evdemo` 证明的是"事件流可被用户态消费、产出正确字节，且空读会在内核阻塞而非自旋或 EOF"。

它**不**证明 shell 已经切换到事件输入源——那是组件接线阶段的工作。这个程序自己手搓了整条链，
走的不是产品代码路径；产品路径由另一个程序验收。

## 与 blkdemo 的关系

[`blkdemo`](https://github.com/BRX-Boruix/blkdemo) 是本程序的**对照组**：两者除了等待源之外
完全一样（`evdemo` 等事件节点，`blkdemo` 等标准输入），用来判定一次 CPU 占用缺陷的归属。

## 构建

```bash
cargo build --release
```

编译产物部署为 BORUIX 系统中的 `/programs/evdemo.elf`，然后在 shell 中执行。

## 文件结构

```
evdemo/
├── Cargo.toml    # 包定义
├── build.rs      # 注入链接脚本
├── linker.ld     # 用户态段布局
└── src/
    └── main.rs   # 程序本体
```

## 相关项目

- [`blkdemo`](https://github.com/BRX-Boruix/blkdemo) —— 对照组，走传统字节路径
- [`libsys`](https://github.com/BRX-Boruix/libsys) —— 用户态系统调用封装，含事件转换层

## 许可

MIT License，版权归 Yang Borui 所有。详见 [LICENSE](LICENSE)。

---

# English

[简体中文](#evdemo) | **English**

An **event-stream input echo program** for BORUIX — it reads the kernel's event node directly and
echoes every keystroke back.

```
/devices/input/events → keycode translation (incl. modifiers) → echoed to standard output
```

Press `q` to quit.

---

## What it verifies

Keyboard events travel a complete chain inside the kernel: interrupt → event push → event node
(16-byte records) → the user-space translation layer. `evdemo` is the **end-to-end acceptance
program** for that chain: it consumes event records one by one and echoes the translated bytes,
proving that the event stream can be consumed correctly from user space.

It also verifies the **blocking semantics**: when the event ring is empty, `read` **suspends the
process inside the kernel** rather than returning EOF or making user space spin. A keystroke wakes
it via interrupt, and the system call returns "you blocked, please retry", on which user space
re-issues the read — a normal wait path, not an error.

## Design: diagnostic form

The program takes a **diagnostic form** — it bypasses the library's `EventSource` abstraction and
performs `open → read → parse record → feed the keycode translator` directly, printing the true
return value of every `read` round to the serial port.

The reason is that diagnosing something like "why does `read` produce nothing once the backlog has
drained" requires seeing the raw per-round truth: how many bytes came back, and what the error code
was. Behaviourally it is equivalent to `EventSource`, following the same chain.

It also prints a one-time file status line confirming it really opened the event node and not a
terminal — had it landed on a different node, those two fields would expose it immediately.

## Reading the output

| Output | Meaning |
| --- | --- |
| `[a]` `[RET]` `[BS]` `[ESC]` | A printable character, Enter, Backspace, Escape |
| `[x1b]` and similar | A byte not directly displayable, given in hex |
| `R<hex> t<decimal>` | Bytes read this round, and cumulative read count |
| `waited` | A kernel block occurred (printed every 4th time to avoid flooding) |

The final `reads=` / `waits=` line reports the total read and block counts honestly.

## Its boundary

`evdemo` proves that the event stream can be consumed from user space, that it produces correct
bytes, and that an empty read blocks inside the kernel rather than spinning or returning EOF.

It does **not** prove that the shell has switched to the event input source — that is the wiring
stage's job. This program hand-rolls the whole chain rather than going through product code; the
product path is accepted by a different program.

## Relationship to blkdemo

[`blkdemo`](https://github.com/BRX-Boruix/blkdemo) is this program's **control**: the two are
identical except for what they wait on (`evdemo` waits on the event node, `blkdemo` on standard
input), which is what allowed a CPU-usage defect to be attributed.

## Building

```bash
cargo build --release
```

The artifact is deployed as `/programs/evdemo.elf` in a BORUIX system, then run from the shell.

## Layout

```
evdemo/
├── Cargo.toml    # package definition
├── build.rs      # injects the linker script
├── linker.ld     # user-space section layout
└── src/
    └── main.rs   # the program itself
```

## Related projects

- [`blkdemo`](https://github.com/BRX-Boruix/blkdemo) — the control, using the legacy byte path
- [`libsys`](https://github.com/BRX-Boruix/libsys) — the user-space syscall wrapper, including the event translation layer

## License

MIT License, copyright Yang Borui. See [LICENSE](LICENSE).
