mod interrupts;
mod utils;
mod vga;

use minifb::{KeyRepeat, Window, WindowOptions};
use std::collections::VecDeque;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use unicorn_engine::unicorn_const::{Arch, Mode, Prot};
use unicorn_engine::{RegisterX86, Unicorn};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        std::process::exit(1);
    }
    
    let mut bd = Vec::new();
    if std::fs::File::open(&args[1])
        .and_then(|mut f| std::io::Read::read_to_end(&mut f, &mut bd))
        .is_err()
    {
        std::process::exit(1);
    }

    let sa = utils::guess_load_address(&bd, &args[1]);
    let font = vga::build_font();
    let vga_st = Arc::new(Mutex::new(vga::Vga::new()));
    let keys = Arc::new(Mutex::new(VecDeque::new()));
    let run = Arc::new(AtomicBool::new(true));

    let (cv, ck, cr) = (Arc::clone(&vga_st), Arc::clone(&keys), Arc::clone(&run));
    
    thread::spawn(move || {
        let mut emu = Unicorn::new(Arch::X86, Mode::MODE_16).unwrap();
        emu.mem_map(0, 64 * 1024 * 1024, Prot::ALL).unwrap();
        emu.mem_write(sa, &bd).unwrap();
        emu.mem_write(0x10, &[0xCD, 0x20]).unwrap();
        emu.reg_write(RegisterX86::SP, 0xFFFC).unwrap();
        emu.mem_write(0xFFFC, &[0x10, 0x00]).unwrap();

        let (ov, sv, iv, ik) = (
            Arc::clone(&cv),
            Arc::clone(&cv),
            Arc::clone(&cv),
            Arc::clone(&ck),
        );
        
        emu.add_insn_out_hook(move |_, port, _, val| {
            let mut s = ov.lock().unwrap();
            if port == 0x3C8 {
                s.pi = val as usize;
                s.ps = 0;
            } else if port == 0x3C9 {
                let c = ((val & 0x3F) as u8) << 2;
                let (pi, ps) = (s.pi, s.ps as usize);
                s.pal[pi][ps] = c;
                if s.ps == 2 {
                    s.ps = 0;
                    s.pi = (s.pi + 1) % 256;
                } else {
                    s.ps += 1;
                }
            }
        })
        .unwrap();

        emu.add_code_hook(0, !0, move |uc, _, _| {
            let mut s = sv.lock().unwrap();
            if s.mode13 {
                uc.mem_read(0xA0000, &mut s.gfx).unwrap();
            }
        })
        .unwrap();

        emu.add_intr_hook(move |uc, int| match int {
            0x10 => interrupts::handle_10(uc, &iv),
            0x15 => interrupts::handle_15(uc),
            0x16 => interrupts::handle_16(uc, &ik),
            0x1A => interrupts::handle_1a(uc),
            0x21 => interrupts::handle_21(uc, &iv),
            0x22 => interrupts::handle_22(uc),
            0x20 => {
                uc.emu_stop().unwrap();
            }
            _ => {}
        })
        .unwrap();
        
        emu.emu_start(sa, sa + bd.len() as u64, 0, 0).unwrap();
        cr.store(false, Ordering::Relaxed);
    });

    let mut win = Window::new("PRos", 640, 400, WindowOptions::default()).unwrap();
    let mut buf = vec![0u32; 640 * 400];
    
    while win.is_open() && run.load(Ordering::Relaxed) {
        let sh = win.is_key_down(minifb::Key::LeftShift) || win.is_key_down(minifb::Key::RightShift);
        if let Some(keys_pressed) = win.get_keys_pressed(KeyRepeat::No).first() {
            if let Some(code) = utils::key_to_bios(*keys_pressed, sh) {
                keys.lock().unwrap().push_back(code);
            }
        }
        
        let s = vga_st.lock().unwrap();
        if s.mode13 {
            for i in 0..64000 {
                let c = s.pal[s.gfx[i] as usize];
                let rgb = (c[0] as u32) << 16 | (c[1] as u32) << 8 | (c[2] as u32);
                let (x, y) = (i % 320, i / 320);
                let b = y * 2 * 640 + x * 2;
                buf[b] = rgb;
                buf[b + 1] = rgb;
                buf[b + 640] = rgb;
                buf[b + 641] = rgb;
            }
        } else {
            buf.fill(0);
            for i in 0..2000 {
                let (ch, attr) = ((s.txt[i] & 0xFF) as usize, (s.txt[i] >> 8) as u8);
                let (fg, bg) = (s.pal[(attr & 0xF) as usize], s.pal[(attr >> 4) as usize]);
                let (fgr, bgr) = (
                    (fg[0] as u32) << 16 | (fg[1] as u32) << 8 | (fg[2] as u32),
                    (bg[0] as u32) << 16 | (bg[1] as u32) << 8 | (bg[2] as u32),
                );
                for py in 0..16 {
                    for px in 0..8 {
                        let pix = (font[ch][py] >> (7 - px)) & 1;
                        buf[((i / 80) * 16 + py) * 640 + (i % 80) * 8 + px] =
                            if pix == 1 { fgr } else { bgr };
                    }
                }
            }
        }
        win.update_with_buffer(&buf, 640, 400).unwrap();
    }
}
