pub struct Vga {
    pub txt: [u16; 2000],
    pub gfx: [u8; 64000],
    pub pal: [[u8; 3]; 256],
    pub pi: usize,
    pub ps: u8,
    pub mode13: bool,
    pub cx: usize,
    pub cy: usize,
    pub color: u8,
}

impl Vga {
    pub fn new() -> Self {
        let mut p = [[0u8; 3]; 256];
        let d: [[u8; 3]; 16] = [
            [0, 0, 0], [0, 0, 170], [0, 170, 0], [0, 170, 170],
            [170, 0, 0], [170, 0, 170], [170, 85, 0], [170, 170, 170],
            [85, 85, 85], [85, 85, 255], [85, 255, 85], [85, 255, 255],
            [255, 85, 85], [255, 85, 255], [255, 255, 85], [255, 255, 255],
        ];
        for i in 0..16 {
            p[i] = d[i];
        }
        Self {
            txt: [0x0720; 2000],
            gfx: [0; 64000],
            pal: p,
            pi: 0,
            ps: 0,
            mode13: false,
            cx: 0,
            cy: 0,
            color: 0x0F,
        }
    }

    pub fn set(&mut self, r: usize, c: usize, ch: u8, co: u8) {
        if r < 25 && c < 80 {
            self.txt[r * 80 + c] = (co as u16) << 8 | ch as u16;
        }
    }

    pub fn putch(&mut self, ch: u8, co: u8) {
        if ch == 0x0A {
            self.cy += 1;
            if self.cy >= 25 {
                for i in 0..1920 {
                    self.txt[i] = self.txt[i + 80]
                }
                for i in 1920..2000 {
                    self.txt[i] = 0x0720
                }
                self.cy = 24;
            }
        } else if ch == 0x0D {
            self.cx = 0;
        } else {
            self.set(self.cy, self.cx, ch, co);
            self.cx += 1;
            if self.cx >= 80 {
                self.cx = 0;
                self.putch(0x0A, co);
            }
        }
    }
}

pub fn build_font() -> [[u8; 16]; 256] {
    let mut f = [[0u8; 16]; 256];
    let g: &[(u8, [u8; 8])] = &[
        (0x20, [0, 0, 0, 0, 0, 0, 0, 0]),
        (0x41, [24, 60, 102, 102, 126, 102, 102, 0]),
    ];
    for &(c, b) in g {
        for i in 0..8 {
            f[c as usize][i * 2] = b[i];
            f[c as usize][i * 2 + 1] = b[i];
        }
    }
    f
}
