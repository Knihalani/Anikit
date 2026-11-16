//! Expressions: `"=960 + 300 * sin(t * 2)"`, compiled once to a small stack
//! program and evaluated without allocating.

/// Variables available to expressions.
#[derive(Clone, Copy, Debug, Default)]
pub struct Vars {
    /// Seconds since the layer's start.
    pub t: f64,
    /// Seconds since the scene's start.
    pub scene_t: f64,
    /// Global seconds.
    pub gt: f64,
    /// Layer progress 0..1.
    pub p: f64,
    /// Index within the parent and sibling count.
    pub i: f64,
    pub n: f64,
    /// Frame size.
    pub w: f64,
    pub h: f64,
    pub fps: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Ins {
    Const(f64),
    Var(u8),
    Neg,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Lt,
    Gt,
    Call(u8, u8),
}

/// A compiled expression.
#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    code: Vec<Ins>,
    /// Whether it depends on time (`t`, `T`, `gt`, `p`, `frame`).
    pub dynamic: bool,
}

const VARS: &[&str] = &["t", "T", "gt", "p", "i", "n", "w", "h", "fps", "frame", "pi", "e", "tau"];
const FUNCS: &[(&str, u8)] = &[
    ("sin", 1),
    ("cos", 1),
    ("tan", 1),
    ("asin", 1),
    ("acos", 1),
    ("atan", 1),
    ("atan2", 2),
    ("abs", 1),
    ("min", 2),
    ("max", 2),
    ("floor", 1),
    ("ceil", 1),
    ("round", 1),
    ("sqrt", 1),
    ("pow", 2),
    ("exp", 1),
    ("log", 1),
    ("clamp", 3),
    ("mix", 3),
    ("smoothstep", 3),
    ("noise", 1),
    ("noise2", 2),
    ("rand", 1),
    ("mod", 2),
    ("sign", 1),
    ("fract", 1),
    ("step", 2),
    ("pingpong", 2),
];

struct P<'a> {
    s: &'a [u8],
    i: usize,
    out: Vec<Ins>,
    /// Reference length for `%` (and vw/vh via frame size).
    pct_ref: f64,
    w: f64,
    h: f64,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.s.get(self.i).copied()
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expr(&mut self) -> Result<(), String> {
        self.additive()?;
        loop {
            if self.eat(b'<') {
                self.additive()?;
                self.out.push(Ins::Lt);
            } else if self.eat(b'>') {
                self.additive()?;
                self.out.push(Ins::Gt);
            } else {
                return Ok(());
            }
        }
    }
    fn additive(&mut self) -> Result<(), String> {
        self.term()?;
        loop {
            if self.eat(b'+') {
                self.term()?;
                self.out.push(Ins::Add);
            } else if self.eat(b'-') {
                self.term()?;
                self.out.push(Ins::Sub);
            } else {
                return Ok(());
            }
        }
    }
    fn term(&mut self) -> Result<(), String> {
        self.unary()?;
        loop {
            if self.eat(b'*') {
                self.unary()?;
                self.out.push(Ins::Mul);
            } else if self.eat(b'/') {
                self.unary()?;
                self.out.push(Ins::Div);
            } else if self.eat(b'%') {
                self.unary()?;
                self.out.push(Ins::Mod);
            } else {
                return Ok(());
            }
        }
    }
    fn unary(&mut self) -> Result<(), String> {
        if self.eat(b'-') {
            self.unary()?;
            self.out.push(Ins::Neg);
            return Ok(());
        }
        if self.eat(b'+') {
            return self.unary();
        }
        self.power()
    }
    fn power(&mut self) -> Result<(), String> {
        self.atom()?;
        if self.eat(b'^') {
            self.unary()?;
            self.out.push(Ins::Pow);
        }
        Ok(())
    }
    fn atom(&mut self) -> Result<(), String> {
        let c = self.peek().ok_or("unexpected end of expression")?;
        if c == b'(' {
            self.i += 1;
            self.expr()?;
            if !self.eat(b')') {
                return Err("missing ')'".into());
            }
            return Ok(());
        }
        if c.is_ascii_digit() || c == b'.' {
            let st = self.i;
            while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.') {
                self.i += 1;
            }
            if self.i < self.s.len() && (self.s[self.i] == b'e' || self.s[self.i] == b'E') {
                let save = self.i;
                self.i += 1;
                if self.i < self.s.len() && (self.s[self.i] == b'-' || self.s[self.i] == b'+') {
                    self.i += 1;
                }
                if self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                    while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                        self.i += 1;
                    }
                } else {
                    self.i = save;
                }
            }
            let v: f64 = std::str::from_utf8(&self.s[st..self.i])
                .unwrap()
                .parse()
                .map_err(|_| "bad number".to_string())?;
            // Units directly after the number.
            let rest = &self.s[self.i..];
            let unit = |u: &str| rest.starts_with(u.as_bytes()) && !rest.get(u.len()).is_some_and(|c| c.is_ascii_alphanumeric());
            let (mul, len) = if rest.first() == Some(&b'%') && !self.after_pct_is_operand() {
                (self.pct_ref / 100.0, 1)
            } else if unit("vw") {
                (self.w / 100.0, 2)
            } else if unit("vh") {
                (self.h / 100.0, 2)
            } else if unit("vmin") {
                (self.w.min(self.h) / 100.0, 4)
            } else if unit("vmax") {
                (self.w.max(self.h) / 100.0, 4)
            } else if unit("px") {
                (1.0, 2)
            } else if unit("deg") {
                (1.0, 3)
            } else if unit("ms") {
                (0.001, 2)
            } else if unit("s") {
                (1.0, 1)
            } else {
                (1.0, 0)
            };
            self.i += len;
            self.out.push(Ins::Const(v * mul));
            return Ok(());
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let st = self.i;
            while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_') {
                self.i += 1;
            }
            let name = std::str::from_utf8(&self.s[st..self.i]).unwrap();
            if self.eat(b'(') {
                let mut argc = 0u8;
                if !self.eat(b')') {
                    loop {
                        self.expr()?;
                        argc += 1;
                        if self.eat(b',') {
                            continue;
                        }
                        if self.eat(b')') {
                            break;
                        }
                        return Err(format!("expected ',' or ')' in call to {name}"));
                    }
                }
                let name = if name == "noise" && argc == 2 { "noise2" } else { name };
                let idx = FUNCS.iter().position(|(f, _)| *f == name).ok_or_else(|| {
                    let names: Vec<&str> = FUNCS.iter().map(|f| f.0).collect();
                    match crate::spec::validate::suggest(name, &names) {
                        Some(s) => format!("unknown function '{name}', did you mean '{s}'?"),
                        None => format!("unknown function '{name}'"),
                    }
                })?;
                let want = FUNCS[idx].1;
                if argc != want {
                    return Err(format!("{name}() takes {want} argument(s), got {argc}"));
                }
                self.out.push(Ins::Call(idx as u8, argc));
                return Ok(());
            }
            let idx = VARS.iter().position(|v| *v == name).ok_or_else(|| match crate::spec::validate::suggest(name, VARS) {
                Some(s) => format!("unknown variable '{name}', did you mean '{s}'?"),
                None => format!("unknown variable '{name}' (use t, T, gt, p, i, n, w, h, fps, frame)"),
            })?;
            match name {
                "pi" => self.out.push(Ins::Const(std::f64::consts::PI)),
                "e" => self.out.push(Ins::Const(std::f64::consts::E)),
                "tau" => self.out.push(Ins::Const(std::f64::consts::TAU)),
                _ => self.out.push(Ins::Var(idx as u8)),
            }
            return Ok(());
        }
        Err(format!("unexpected '{}'", c as char))
    }
    /// After `5%`: is the `%` a modulo (an operand follows) or a unit?
    fn after_pct_is_operand(&self) -> bool {
        let mut j = self.i + 1;
        while j < self.s.len() && self.s[j] == b' ' {
            j += 1;
        }
        // `50% + 3` → unit; `t % 2` never reaches here (no number before %).
        // `5 %2` → modulo only when separated by a space before '%'.
        j > self.i + 1 && self.s.get(j).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'(')
            && self.i > 0
            && self.s[self.i - 1] == b' '
    }
}

impl Expr {
    /// Compile `src` (without the leading `=`). `pct_ref` is what `100%` means.
    pub fn compile(src: &str, pct_ref: f64, w: f64, h: f64) -> Result<Expr, String> {
        let mut p = P { s: src.as_bytes(), i: 0, out: Vec::new(), pct_ref, w, h };
        p.expr()?;
        p.ws();
        if p.i != p.s.len() {
            return Err(format!("unexpected '{}' at column {}", &src[p.i..], p.i + 1));
        }
        // Stack depth check (bounded so eval needs no allocation).
        let mut d: i32 = 0;
        let mut maxd = 0;
        for ins in &p.out {
            d += match ins {
                Ins::Const(_) | Ins::Var(_) => 1,
                Ins::Neg => 0,
                Ins::Call(_, n) => 1 - *n as i32,
                _ => -1,
            };
            maxd = maxd.max(d);
        }
        if maxd > 32 {
            return Err("expression too deeply nested".into());
        }
        let dynamic = p.out.iter().any(|i| matches!(i, Ins::Var(0..=3) | Ins::Var(9)));
        let mut e = Expr { code: p.out, dynamic };
        e.fold();
        Ok(e)
    }

    /// Constant value, if the expression has no variables.
    pub fn constant(&self) -> Option<f64> {
        match self.code.as_slice() {
            [Ins::Const(v)] => Some(*v),
            _ => None,
        }
    }

    fn fold(&mut self) {
        if self.code.iter().all(|i| !matches!(i, Ins::Var(_))) {
            let v = self.eval(&Vars::default());
            self.code = vec![Ins::Const(v)];
        }
    }

    /// Evaluate.
    pub fn eval(&self, v: &Vars) -> f64 {
        let mut st = [0.0f64; 32];
        let mut sp = 0usize;
        for ins in &self.code {
            match *ins {
                Ins::Const(c) => {
                    st[sp] = c;
                    sp += 1;
                }
                Ins::Var(i) => {
                    st[sp] = match i {
                        0 => v.t,
                        1 => v.scene_t,
                        2 => v.gt,
                        3 => v.p,
                        4 => v.i,
                        5 => v.n,
                        6 => v.w,
                        7 => v.h,
                        8 => v.fps,
                        _ => (v.gt * v.fps).floor(),
                    };
                    sp += 1;
                }
                Ins::Neg => st[sp - 1] = -st[sp - 1],
                Ins::Call(f, n) => {
                    let n = n as usize;
                    let a = &st[sp - n..sp];
                    let r = call(f, a);
                    sp -= n;
                    st[sp] = r;
                    sp += 1;
                }
                op => {
                    let b = st[sp - 1];
                    let a = st[sp - 2];
                    sp -= 1;
                    st[sp - 1] = match op {
                        Ins::Add => a + b,
                        Ins::Sub => a - b,
                        Ins::Mul => a * b,
                        Ins::Div => a / b,
                        Ins::Mod => a.rem_euclid(b),
                        Ins::Pow => a.powf(b),
                        Ins::Lt => (a < b) as u8 as f64,
                        Ins::Gt => (a > b) as u8 as f64,
                        _ => unreachable!(),
                    };
                }
            }
        }
        let r = st[0];
        if r.is_finite() {
            r
        } else {
            0.0
        }
    }
}

fn hash(x: f64) -> f64 {
    let mut h = (x.to_bits() ^ 0x9E37_79B9_7F4A_7C15).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 31;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 29;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

fn vnoise(x: f64) -> f64 {
    let i = x.floor();
    let f = x - i;
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash(i) * 2.0 - 1.0;
    let b = hash(i + 1.0) * 2.0 - 1.0;
    a + (b - a) * u
}

fn call(f: u8, a: &[f64]) -> f64 {
    match FUNCS[f as usize].0 {
        "sin" => a[0].sin(),
        "cos" => a[0].cos(),
        "tan" => a[0].tan(),
        "asin" => a[0].asin(),
        "acos" => a[0].acos(),
        "atan" => a[0].atan(),
        "atan2" => a[0].atan2(a[1]),
        "abs" => a[0].abs(),
        "min" => a[0].min(a[1]),
        "max" => a[0].max(a[1]),
        "floor" => a[0].floor(),
        "ceil" => a[0].ceil(),
        "round" => a[0].round(),
        "sqrt" => a[0].sqrt(),
        "pow" => a[0].powf(a[1]),
        "exp" => a[0].exp(),
        "log" => a[0].ln(),
        "clamp" => a[0].clamp(a[1].min(a[2]), a[2].max(a[1])),
        "mix" => a[0] + (a[1] - a[0]) * a[2],
        "smoothstep" => {
            let x = ((a[2] - a[0]) / (a[1] - a[0])).clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        }
        "noise" => vnoise(a[0]),
        "noise2" => vnoise(a[0] + vnoise(a[1] * 1.37) * 4.1 + a[1] * 0.71),
        "rand" => hash(a[0]),
        "mod" => a[0].rem_euclid(a[1]),
        "sign" => a[0].signum(),
        "fract" => a[0] - a[0].floor(),
        "step" => (a[1] >= a[0]) as u8 as f64,
        "pingpong" => {
            let l = a[1].abs().max(1e-9);
            let m = a[0].rem_euclid(2.0 * l);
            if m > l {
                2.0 * l - m
            } else {
                m
            }
        }
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str, t: f64) -> f64 {
        Expr::compile(s, 1920.0, 1920.0, 1080.0).unwrap().eval(&Vars { t, w: 1920.0, h: 1080.0, ..Default::default() })
    }

    #[test]
    fn basics() {
        assert_eq!(ev("1 + 2 * 3", 0.0), 7.0);
        assert_eq!(ev("-2^2", 0.0), -4.0);
        assert_eq!(ev("(1+2)*3", 0.0), 9.0);
        assert_eq!(ev("50%", 0.0), 960.0);
        assert_eq!(ev("50% + 10", 0.0), 970.0);
        assert_eq!(ev("10vh", 0.0), 108.0);
        assert_eq!(ev("t * 2", 3.0), 6.0);
        assert_eq!(ev("7 % 3", 0.0), 1.0);
        assert_eq!(ev("clamp(5, 0, 2)", 0.0), 2.0);
        assert!((ev("sin(pi/2)", 0.0) - 1.0).abs() < 1e-12);
        assert_eq!(ev("w/2", 0.0), 960.0);
        assert!(Expr::compile("sinn(t)", 1.0, 1.0, 1.0).unwrap_err().contains("did you mean 'sin'"));
        assert!(Expr::compile("1 +", 1.0, 1.0, 1.0).is_err());
        assert!(!Expr::compile("w/2", 1.0, 1.0, 1.0).unwrap().dynamic);
        assert!(Expr::compile("t", 1.0, 1.0, 1.0).unwrap().dynamic);
    }
}
