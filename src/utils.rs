use unicorn_engine::{RegisterX86, Unicorn};

pub fn guess_load_address(bd: &[u8], f: &str) -> u64 {
    let n = f.to_uppercase();
    if n.contains("KERNEL") || (!bd.is_empty() && bd[0] == 0xFA) {
        0
    } else if n.contains("BOOT") {
        0x7C00
    } else {
        0x8000
    }
}

pub fn set_cf(uc: &mut Unicorn<'_, ()>, v: bool) {
    let mut f = uc.reg_read(RegisterX86::EFLAGS).unwrap();
    if v {
        f |= 1
    } else {
        f &= !1
    }
    uc.reg_write(RegisterX86::EFLAGS, f).unwrap();
}

pub fn rd_str(uc: &Unicorn<'_, ()>, seg: u64, off: u64) -> String {
    let (mut a, mut r, mut b) = (seg * 16 + off, Vec::new(), [0u8; 1]);
    loop {
        uc.mem_read(a, &mut b).unwrap();
        if b[0] == 0 {
            break;
        }
        r.push(b[0]);
        a += 1;
    }
    String::from_utf8_lossy(&r).into_owned()
}

pub fn key_to_bios(k: minifb::Key, sh: bool) -> Option<u16> {
    use minifb::Key::*;
    let (sc, n, s) = match k {
        A => (0x1E, b'a', b'A'),
        B => (0x30, b'b', b'B'),
        C => (0x2E, b'c', b'C'),
        D => (0x20, b'd', b'D'),
        Enter => return Some(0x1C0D),
        Backspace => return Some(0x0E08),
        Space => return Some(0x3920),
        _ => return None,
    };
    Some((sc as u16) << 8 | (if sh { s } else { n }) as u16)
}
