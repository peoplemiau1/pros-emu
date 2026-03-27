use crate::utils::{rd_str, set_cf};
use crate::vga::Vga;
use std::collections::VecDeque;
use std::ffi::{CStr, CString};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use unicorn_engine::{RegisterX86, Unicorn};

pub fn handle_10(uc: &mut Unicorn<'_, ()>, v: &Arc<Mutex<Vga>>) {
    let ax = uc.reg_read(RegisterX86::AX).unwrap();
    let ah = (ax >> 8) as u8;
    let al = (ax & 0xFF) as u8;
    let mut s = v.lock().unwrap();
    match ah {
        0x00 => {
            s.mode13 = al == 0x13;
            s.txt.fill(0x0720);
            s.cx = 0;
            s.cy = 0;
        }
        0x02 => {
            let dx = uc.reg_read(RegisterX86::DX).unwrap();
            s.cy = ((dx >> 8) as usize).min(24);
            s.cx = ((dx & 0xFF) as usize).min(79);
        }
        0x03 => {
            uc.reg_write(RegisterX86::DX, ((s.cy as u64) << 8) | (s.cx as u64)).unwrap();
            uc.reg_write(RegisterX86::CX, 0x0607).unwrap();
        }
        0x0E => {
            let bx = uc.reg_read(RegisterX86::BX).unwrap();
            s.putch(al, (bx & 0xFF) as u8);
        }
        _ => {}
    }
}

pub fn handle_15(uc: &mut Unicorn<'_, ()>) {
    let ax = uc.reg_read(RegisterX86::AX).unwrap();
    if (ax >> 8) as u8 == 0x86 {
        let cx = uc.reg_read(RegisterX86::CX).unwrap();
        let dx = uc.reg_read(RegisterX86::DX).unwrap();
        unsafe {
            libc::usleep(((cx << 16) | dx) as u32);
        }
        set_cf(uc, false);
    }
}

pub fn handle_16(uc: &mut Unicorn<'_, ()>, k: &Arc<Mutex<VecDeque<u16>>>) {
    let ax = uc.reg_read(RegisterX86::AX).unwrap();
    let ah = (ax >> 8) as u8;
    if ah == 0x00 || ah == 0x10 {
        loop {
            {
                let mut kb = k.lock().unwrap();
                if let Some(code) = kb.pop_front() {
                    uc.reg_write(RegisterX86::AX, code as u64).unwrap();
                    return;
                }
            }
            unsafe {
                libc::usleep(5000);
            }
        }
    } else if ah == 0x01 || ah == 0x11 {
        let kb = k.lock().unwrap();
        let mut ef = uc.reg_read(RegisterX86::EFLAGS).unwrap();
        if kb.is_empty() {
            ef |= 0x40;
        } else {
            ef &= !0x40;
            uc.reg_write(RegisterX86::AX, kb[0] as u64).unwrap();
        }
        uc.reg_write(RegisterX86::EFLAGS, ef).unwrap();
    }
}

pub fn handle_1a(uc: &mut Unicorn<'_, ()>) {
    let ax = uc.reg_read(RegisterX86::AX).unwrap();
    if (ax >> 8) as u8 == 0x00 {
        let t = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            / 55) as u64;
        uc.reg_write(RegisterX86::CX, t >> 16).unwrap();
        uc.reg_write(RegisterX86::DX, t & 0xFFFF).unwrap();
        uc.reg_write(RegisterX86::AX, 0).unwrap();
    }
}

pub fn handle_21(uc: &mut Unicorn<'_, ()>, v: &Arc<Mutex<Vga>>) {
    let ax = uc.reg_read(RegisterX86::AX).unwrap();
    let ah = (ax >> 8) as u8;
    let mut s = v.lock().unwrap();
    match ah {
        0x01 | 0x02 | 0x03 | 0x04 | 0x08 => {
            let ds = uc.reg_read(RegisterX86::DS).unwrap();
            let si = uc.reg_read(RegisterX86::SI).unwrap();
            let co = match ah {
                0x01 => 0x0F,
                0x02 => 0x0A,
                0x03 => 0x0B,
                0x04 => 0x0C,
                0x08 => s.color,
                _ => 0x0F,
            };
            let mut a = ds * 16 + si;
            let mut b = [0u8; 1];
            loop {
                uc.mem_read(a, &mut b).unwrap();
                if b[0] == 0 {
                    break;
                }
                s.putch(b[0], co);
                a += 1;
            }
        }
        0x05 => {
            s.putch(0x0D, 0);
            s.putch(0x0A, 0);
        }
        0x06 => {
            s.txt.fill(0x0720);
            s.cx = 0;
            s.cy = 0;
        }
        0x07 => {
            let bx = uc.reg_read(RegisterX86::BX).unwrap();
            s.color = (bx & 0xFF) as u8;
        }
        _ => {}
    }
}

pub fn handle_22(uc: &mut Unicorn<'_, ()>) {
    let ax = uc.reg_read(RegisterX86::AX).unwrap();
    let ah = (ax >> 8) as u8;
    let ds = uc.reg_read(RegisterX86::DS).unwrap();
    let si = uc.reg_read(RegisterX86::SI).unwrap();
    let cx = uc.reg_read(RegisterX86::CX).unwrap();
    let fn_str = rd_str(uc, ds, si);
    let c_fn = CString::new(fn_str).unwrap_or_default();

    match ah {
        0x01 => unsafe {
            let dir = libc::opendir(b".\0".as_ptr() as *const _);
            if !dir.is_null() {
                let (mut cnt, mut sz_total, mut ptr) = (0u64, 0u64, (ds * 16) + si);
                loop {
                    let ent = libc::readdir(dir);
                    if ent.is_null() {
                        break;
                    }
                    let d_name = CStr::from_ptr((*ent).d_name.as_ptr()).to_bytes();
                    if d_name == b"." || d_name == b".." {
                        continue;
                    }
                    let mut st: libc::stat = std::mem::zeroed();
                    libc::stat((*ent).d_name.as_ptr(), &mut st);
                    let mut buf18 = [0u8; 18];
                    let clen = d_name.len().min(13);
                    buf18[..clen].copy_from_slice(&d_name[..clen]);
                    buf18[14..18].copy_from_slice(&(st.st_size as u32).to_le_bytes());
                    if (st.st_mode & libc::S_IFMT) == libc::S_IFDIR {
                        buf18[16] |= 0x10;
                    }
                    uc.mem_write(ptr, &buf18).unwrap();
                    ptr += 18;
                    cnt += 1;
                    sz_total += st.st_size as u64;
                }
                libc::closedir(dir);
                uc.reg_write(RegisterX86::DX, cnt).unwrap();
                uc.reg_write(RegisterX86::BX, sz_total & 0xFFFF).unwrap();
                uc.reg_write(RegisterX86::CX, sz_total >> 16).unwrap();
                set_cf(uc, false);
            } else {
                set_cf(uc, true);
            }
        },
        0x02 | 0x10 => unsafe {
            let fd = libc::open(c_fn.as_ptr(), libc::O_RDONLY);
            if fd >= 0 {
                let sz = libc::lseek(fd, 0, libc::SEEK_END) as usize;
                libc::lseek(fd, 0, libc::SEEK_SET);
                let mut buf = vec![0u8; sz];
                libc::read(fd, buf.as_mut_ptr() as *mut _, sz);
                libc::close(fd);
                let addr = if ah == 0x10 {
                    uc.reg_read(RegisterX86::DX).unwrap() * 16 + cx
                } else {
                    ds * 16 + cx
                };
                uc.mem_write(addr, &buf).unwrap();
                uc.reg_write(RegisterX86::BX, sz as u64).unwrap();
                set_cf(uc, false);
            } else {
                set_cf(uc, true);
            }
        },
        0x03 => unsafe {
            let bx = uc.reg_read(RegisterX86::BX).unwrap();
            let mut buf = vec![0u8; cx as usize];
            uc.mem_read(ds * 16 + bx, &mut buf).unwrap();
            let fd = libc::open(
                c_fn.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
                0o666,
            );
            if fd >= 0 {
                libc::write(fd, buf.as_ptr() as *const _, cx as usize);
                libc::close(fd);
                set_cf(uc, false);
            } else {
                set_cf(uc, true);
            }
        },
        0x04 => unsafe {
            let mut st: libc::stat = std::mem::zeroed();
            set_cf(uc, libc::stat(c_fn.as_ptr(), &mut st) != 0);
        },
        0x06 => unsafe {
            set_cf(uc, libc::unlink(c_fn.as_ptr()) != 0);
        },
        0x07 => unsafe {
            let di = uc.reg_read(RegisterX86::DI).unwrap();
            let c_nn = CString::new(rd_str(uc, ds, di)).unwrap_or_default();
            set_cf(uc, libc::rename(c_fn.as_ptr(), c_nn.as_ptr()) != 0);
        },
        0x08 => unsafe {
            let mut st: libc::stat = std::mem::zeroed();
            if libc::stat(c_fn.as_ptr(), &mut st) == 0 {
                uc.reg_write(RegisterX86::BX, (st.st_size & 0xFFFF) as u64)
                    .unwrap();
                set_cf(uc, false);
            } else {
                set_cf(uc, true);
            }
        },
        0x09 => unsafe {
            set_cf(uc, libc::chdir(c_fn.as_ptr()) != 0);
        },
        0x0B => unsafe {
            set_cf(uc, libc::mkdir(c_fn.as_ptr(), 0o777) != 0);
        },
        0x0C => unsafe {
            set_cf(uc, libc::rmdir(c_fn.as_ptr()) != 0);
        },
        _ => set_cf(uc, false),
    }
}
