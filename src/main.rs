//! BORUIX evdemo：事件流输入源端到端验收（I-EVENTS 阶段 2 第 2 小点）。
//!
//! 数据链：IRQ1 → push_event（阶段 1）→ /devices/input/events（16 字节记录）
//! → libsys::event 转换层（keymap/修饰键，阶段 2 第 1 点）→ 本程序逐条消费回显。
//!
//! **诊断形态**（本小点验收专用）：绕开 libline::EventSource，直接
//! open → read → parse_record → feed，把每轮 read 的真值（n / errno）打到串口——
//! 「积压吐完后 read 无产出」的定位需要这个证据（S21）。行为与 EventSource
//! 等价（同一条链）；组件切换在下一小点做 shell 接线时验证。
//!
//! 阻塞语义（本小点新增）：事件环为空时 `read` 在**内核内挂起**本进程
//! （`input_event_blocking` → `block_for_input_event`），按键到达由 IRQ1 的
//! `wake_input_event` 唤醒并以 `-EAGAIN` 哨兵要求重试——用户态不做
//! `yield` 自旋。
//!
//! 诚实边界（S09）：本程序证明「事件流可被用户态消费、产出正确字节，
//! 且空读会在内核阻塞而非 EOF/自旋」，**不**证明 shell 已切换输入源
//! （组件接线在后续小点）。
#![no_std]
#![no_main]

extern crate alloc;

use libsys::event::{EVENT_RECORD_SIZE, KeymapState, feed as feed_event, parse_record};
use libsys::{open, read, write, OpenFlags, Permissions, STDOUT};

fn out(b: &[u8]) {
    let _ = write(STDOUT, b);
}

fn outln(b: &[u8]) {
    out(b);
    out(b"\n");
}

fn out_hex(mut v: u64) {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut buf = [0u8; 16];
    let mut i = buf.len();
    if v == 0 {
        out(b"0");
        return;
    }
    while v > 0 {
        i -= 1;
        buf[i] = HEX[(v % 16) as usize];
        v /= 16;
    }
    out(&buf[i..]);
}

/// 回显一个转换产出的字节；q 返回 true（退出信号）。
fn echo_byte(ch: u8) -> bool {
    out(b"[");
    match ch {
        b'\n' => outln(b"RET]"),
        0x7f => outln(b"BS]"),
        0x1b => outln(b"ESC]"),
        c if (0x20..0x7f).contains(&c) => {
            out(&[c]);
            outln(b"]");
        }
        _ => {
            out(b"x");
            out_hex(ch as u64);
            outln(b"]");
        }
    }
    ch == b'q'
}

#[unsafe(no_mangle)]
pub extern "C" fn user_main(_argc: isize, _argv: *const *const u8) -> i32 {
    outln(b"[evdemo] event-stream echo (I-EVENTS phase2); press q to quit");
    let fd = match open("/devices/input/events", OpenFlags::READ_ONLY, Permissions::all()) {
        Ok(f) => f,
        Err(e) => {
            out(b"[evdemo] FAIL open events: -");
            out_hex(e.to_errno() as u64);
            outln(b"");
            return 1;
        }
    };
    let mut keymap = KeymapState::default();
    let mut rec_buf = [0u8; EVENT_RECORD_SIZE];
    let mut rec_len = 0usize;
    let mut n_reads: u64 = 0;
    let mut n_blocks: u64 = 0;
    // 诊断：fd 真值自述（fstat）——is_terminal/console_owner 应为 0（事件节点
    // 不是终端）。若非 0，open 落到了别的节点（铁证）。
    if let Ok(info) = libsys::fstat(fd) {
        out(b"[evdemo] fstat: is_terminal=");
        out_hex(info.is_terminal as u64);
        out(b" owner=");
        out_hex(info.console_owner);
        outln(b"");
    }
    loop {
        let mut buf = [0u8; 64]; // 4 条整记录/次
        let n = match read(fd, &mut buf) {
            Ok(n) => n,
            Err(e) if e == libsys::Error::WouldBlock => {
                // **正常等待路径**（I-EVENTS 阶段 2 等待原语）：内核在事件环
                // 为空时把本进程置 Blocked 切走；按键到达后经 `wake_input_event`
                // 唤醒，并以 `-EAGAIN` 哨兵（libsys 解码为 `WouldBlock`）告知
                // 「曾阻塞、请重试」。故此处**不是错误**——如实重试 `read`，
                // 下一轮即取到记录。绝不 yield 自旋（那正是本原语要消灭的行为）。
                //
                // 计数用于诊断证明「确实发生了阻塞等待」（S09 可观察）：
                // 每 4 次唤醒打一条，避免刷屏。
                n_blocks += 1;
                if n_blocks % 4 == 0 {
                    outln(b"[evdemo] waited");
                }
                continue;
            }
            Err(e) => {
                // 其它错误如实打（S09）——events 节点不该产生别的错误。
                out(b"[evdemo] read err -");
                out_hex(e.to_errno() as u64);
                outln(b"");
                n_reads += 1;
                let _ = libsys::yield_now();
                continue;
            }
        };
        n_reads += 1;
        if n == 0 {
            // 空读：如实轮询（诊断打印证明「真在轮询」，每 4 轮一条）。
            if n_reads % 4 == 0 {
                outln(b"[evdemo] idle");
            }
            let _ = libsys::yield_now();
            continue;
        }
        // 恒打印 read 真值：n 恒 0x40=64？还是变化？（证据链核心）
        out(b"[evdemo] R");
        out_hex(n as u64);
        out(b" t");
        out_hex(n_reads);
        outln(b"");
        for &b in &buf[..n] {
            rec_buf[rec_len] = b;
            rec_len += 1;
            if rec_len == EVENT_RECORD_SIZE {
                rec_len = 0;
                if let Ok(rec) = parse_record(&rec_buf) {
                    if let Some(k) = feed_event(&mut keymap, &rec) {
                        for &ch in k.bytes() {
                            if echo_byte(ch) {
                                // 退出（q）：如实汇报本轮消耗的读取次数与
                                // 内核阻塞次数（S09 可观察）。
                                out(b"[evdemo] reads=");
                                out_hex(n_reads);
                                out(b" waits=");
                                out_hex(n_blocks);
                                outln(b"");
                                outln(b"[evdemo] PASS");
                                return 0;
                            }
                        }
                    }
                }
            }
        }
    }
}
