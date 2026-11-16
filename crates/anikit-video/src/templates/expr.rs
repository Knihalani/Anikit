//! A tiny, safe expression language for templates: `{{ product }}`,
//! `{{ i * 0.4 + 1 }}`, `{{ fmt.portrait ? 96 : 120 }}`, `{{ len(features) }}`.
//!
//! Grammar (lowest precedence first): `a ? b : c`, `a ?? b` (null-coalesce),
//! `||`, `&&`, comparisons, `+ -`, `* / %`, unary `- !`, then member access
//! `a.b`, indexing `a[0]`, calls `f(x, y)`, literals (numbers, `'str'`,
//! `"str"`, `true`, `false`, `null`, `[a, b]`).
//!
//! Values are JSON values. Truthiness: `null`, `false`, `0`, `""` and `[]`
//! are false. `+` concatenates when either side is a string.

use serde_json::{Map, Value};

use super::color;

/// Variables visible to an expression: a stack of JSON objects searched innermost first.
pub struct Scope<'a> {
    frames: Vec<&'a Map<String, Value>>,
    owned: Vec<Map<String, Value>>,
}

impl<'a> Scope<'a> {
    pub fn new() -> Self {
        Scope { frames: Vec::new(), owned: Vec::new() }
    }
    pub fn push(&mut self, m: &'a Map<String, Value>) {
        self.frames.push(m);
    }
    /// Push a frame the scope owns (loop variables, `$let` bindings).
    pub fn push_owned(&mut self, m: Map<String, Value>) {
        self.owned.push(m);
    }
    pub fn pop_owned(&mut self) -> Option<Map<String, Value>> {
        self.owned.pop()
    }
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.owned.iter().rev().find_map(|m| m.get(name)).or_else(|| self.frames.iter().rev().find_map(|m| m.get(name)))
    }
    /// Every visible name, for "did you mean" suggestions.
    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.frames.iter().copied().chain(self.owned.iter()).flat_map(|m| m.keys().cloned()).collect();
        v.sort();
        v.dedup();
        v
    }
}

impl Default for Scope<'_> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Op(&'static str),
}

const OPS: [&str; 23] = [
    "??", "||", "&&", "==", "!=", "<=", ">=", "<", ">", "+", "-", "*", "/", "%", "!", "?", ":", ".", ",", "(", ")",
    "[", "]",
];

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == b'.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()) {
            let s = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            let n: f64 = src[s..i].parse().map_err(|_| format!("bad number '{}'", &src[s..i]))?;
            out.push(Tok::Num(n));
        } else if c == b'\'' || c == b'"' {
            let q = c;
            i += 1;
            let mut s = String::new();
            loop {
                if i >= b.len() {
                    return Err("unterminated string".into());
                }
                if b[i] == q {
                    i += 1;
                    break;
                }
                if b[i] == b'\\' && i + 1 < b.len() {
                    i += 1;
                }
                let ch = src[i..].chars().next().unwrap();
                s.push(ch);
                i += ch.len_utf8();
            }
            out.push(Tok::Str(s));
        } else if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                i += 1;
            }
            out.push(Tok::Ident(src[s..i].to_string()));
        } else {
            let op = OPS.iter().find(|o| src[i..].starts_with(**o)).ok_or_else(|| format!("unexpected character '{}'", c as char))?;
            out.push(Tok::Op(op));
            i += op.len();
        }
    }
    Ok(out)
}

/// Parsed expression tree.
#[derive(Clone, Debug)]
enum Ex {
    Lit(Value),
    Var(String),
    Member(Box<Ex>, String),
    Index(Box<Ex>, Box<Ex>),
    Call(String, Vec<Ex>),
    List(Vec<Ex>),
    Un(&'static str, Box<Ex>),
    Bin(&'static str, Box<Ex>, Box<Ex>),
    Cond(Box<Ex>, Box<Ex>, Box<Ex>),
}

struct Parser {
    t: Vec<Tok>,
    p: usize,
}

impl Parser {
    fn peek_op(&self, op: &str) -> bool {
        matches!(self.t.get(self.p), Some(Tok::Op(o)) if *o == op)
    }
    fn eat(&mut self, op: &str) -> bool {
        if self.peek_op(op) {
            self.p += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, op: &str) -> Result<(), String> {
        if self.eat(op) {
            Ok(())
        } else {
            Err(format!("expected '{op}'"))
        }
    }
    fn expr(&mut self) -> Result<Ex, String> {
        let c = self.coalesce()?;
        if self.eat("?") {
            let a = self.expr()?;
            self.expect(":")?;
            let b = self.expr()?;
            return Ok(Ex::Cond(Box::new(c), Box::new(a), Box::new(b)));
        }
        Ok(c)
    }
    fn binary(&mut self, level: usize) -> Result<Ex, String> {
        const LEVELS: [&[&str]; 6] =
            [&["??"], &["||"], &["&&"], &["==", "!=", "<=", ">=", "<", ">"], &["+", "-"], &["*", "/", "%"]];
        if level == LEVELS.len() {
            return self.unary();
        }
        let mut l = self.binary(level + 1)?;
        'outer: loop {
            for op in LEVELS[level] {
                if self.eat(op) {
                    let r = self.binary(level + 1)?;
                    l = Ex::Bin(op, Box::new(l), Box::new(r));
                    continue 'outer;
                }
            }
            return Ok(l);
        }
    }
    fn coalesce(&mut self) -> Result<Ex, String> {
        self.binary(0)
    }
    fn unary(&mut self) -> Result<Ex, String> {
        if self.eat("-") {
            return Ok(Ex::Un("-", Box::new(self.unary()?)));
        }
        if self.eat("!") {
            return Ok(Ex::Un("!", Box::new(self.unary()?)));
        }
        self.postfix()
    }
    fn postfix(&mut self) -> Result<Ex, String> {
        let mut e = self.primary()?;
        loop {
            if self.eat(".") {
                match self.t.get(self.p).cloned() {
                    Some(Tok::Ident(n)) => {
                        self.p += 1;
                        e = Ex::Member(Box::new(e), n);
                    }
                    Some(Tok::Num(n)) if n.fract() == 0.0 => {
                        self.p += 1;
                        e = Ex::Index(Box::new(e), Box::new(Ex::Lit(Value::from(n))));
                    }
                    _ => return Err("expected a name after '.'".into()),
                }
            } else if self.eat("[") {
                let i = self.expr()?;
                self.expect("]")?;
                e = Ex::Index(Box::new(e), Box::new(i));
            } else {
                return Ok(e);
            }
        }
    }
    fn primary(&mut self) -> Result<Ex, String> {
        let t = self.t.get(self.p).cloned().ok_or("unexpected end of expression")?;
        self.p += 1;
        match t {
            Tok::Num(n) => Ok(Ex::Lit(num(n))),
            Tok::Str(s) => Ok(Ex::Lit(Value::String(s))),
            Tok::Ident(n) => match n.as_str() {
                "true" => Ok(Ex::Lit(Value::Bool(true))),
                "false" => Ok(Ex::Lit(Value::Bool(false))),
                "null" => Ok(Ex::Lit(Value::Null)),
                _ if self.peek_op("(") => {
                    self.p += 1;
                    let mut args = Vec::new();
                    if !self.eat(")") {
                        loop {
                            args.push(self.expr()?);
                            if self.eat(")") {
                                break;
                            }
                            self.expect(",")?;
                        }
                    }
                    Ok(Ex::Call(n, args))
                }
                _ => Ok(Ex::Var(n)),
            },
            Tok::Op("(") => {
                let e = self.expr()?;
                self.expect(")")?;
                Ok(e)
            }
            Tok::Op("[") => {
                let mut items = Vec::new();
                if !self.eat("]") {
                    loop {
                        items.push(self.expr()?);
                        if self.eat("]") {
                            break;
                        }
                        self.expect(",")?;
                    }
                }
                Ok(Ex::List(items))
            }
            Tok::Op(o) => Err(format!("unexpected '{o}'")),
        }
    }
}

/// A JSON number, integral when it can be (so `3` prints as `3`, not `3.0`).
pub fn num(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.0e15 {
        Value::from(n as i64)
    } else if n.is_finite() {
        // Round away float noise (0.1 + 0.2) so specs stay readable.
        let r = (n * 1e6).round() / 1e6;
        Value::from(r)
    } else {
        Value::Null
    }
}

pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(_) => true,
    }
}

/// Render a value inside an interpolated string.
pub fn to_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            let f = n.as_f64().unwrap_or(0.0);
            match num(f) {
                Value::Number(n) => n.to_string(),
                _ => String::new(),
            }
        }
        other => other.to_string(),
    }
}

fn f(v: &Value, what: &str) -> Result<f64, String> {
    match v {
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0)),
        Value::Bool(b) => Ok(*b as u8 as f64),
        Value::Null => Ok(0.0),
        Value::String(s) => s.trim().parse().map_err(|_| format!("{what}: expected a number, got \"{s}\"")),
        _ => Err(format!("{what}: expected a number, got {v}")),
    }
}

fn s(v: &Value) -> String {
    to_text(v)
}

/// Number of words in a piece of copy (what a viewer has to read).
pub fn word_count(text: &str) -> usize {
    text.split_whitespace().filter(|w| w.chars().any(|c| c.is_alphanumeric())).count()
}

/// Seconds a viewer needs to read `text`: about 3 words per second plus 1 s
/// to notice it, never less than 1.5 s.
pub fn read_time(text: &str) -> f64 {
    let w = word_count(text) as f64;
    (w / 3.0 + 1.0).max(1.5)
}

/// Evaluate an expression string in `scope`.
pub fn eval(src: &str, scope: &Scope) -> Result<Value, String> {
    let toks = lex(src)?;
    let mut p = Parser { t: toks, p: 0 };
    let e = p.expr()?;
    if p.p != p.t.len() {
        return Err(format!("unexpected {:?} after a complete expression", p.t[p.p]));
    }
    run(&e, scope)
}

fn run(e: &Ex, sc: &Scope) -> Result<Value, String> {
    Ok(match e {
        Ex::Lit(v) => v.clone(),
        Ex::Var(n) => match sc.get(n) {
            Some(v) => v.clone(),
            None => {
                let names = sc.names();
                let hint = crate::templates::suggest(n, names.iter().map(|s| s.as_str()))
                    .map(|s| format!(" (did you mean '{s}'?)"))
                    .unwrap_or_default();
                return Err(format!("unknown name '{n}'{hint}"));
            }
        },
        Ex::Member(o, k) => match run(o, sc)? {
            Value::Object(m) => m.get(k).cloned().unwrap_or(Value::Null),
            Value::Array(a) if k == "length" => Value::from(a.len()),
            Value::String(s) if k == "length" => Value::from(s.chars().count()),
            _ => Value::Null,
        },
        Ex::Index(o, i) => {
            let o = run(o, sc)?;
            let i = run(i, sc)?;
            match (&o, &i) {
                (Value::Array(a), _) => {
                    let mut k = f(&i, "index")? as i64;
                    if k < 0 {
                        k += a.len() as i64;
                    }
                    a.get(k as usize).cloned().unwrap_or(Value::Null)
                }
                (Value::Object(m), Value::String(k)) => m.get(k).cloned().unwrap_or(Value::Null),
                _ => Value::Null,
            }
        }
        Ex::List(items) => Value::Array(items.iter().map(|x| run(x, sc)).collect::<Result<_, _>>()?),
        Ex::Un(op, x) => {
            let v = run(x, sc)?;
            if *op == "!" {
                Value::Bool(!truthy(&v))
            } else {
                num(-f(&v, "-")?)
            }
        }
        Ex::Cond(c, a, b) => {
            if truthy(&run(c, sc)?) {
                run(a, sc)?
            } else {
                run(b, sc)?
            }
        }
        Ex::Bin(op, a, b) => {
            match *op {
                "&&" => {
                    let l = run(a, sc)?;
                    return if truthy(&l) { run(b, sc) } else { Ok(l) };
                }
                "||" => {
                    let l = run(a, sc)?;
                    return if truthy(&l) { Ok(l) } else { run(b, sc) };
                }
                "??" => {
                    // An undefined name on the left counts as null, so `x ?? 1` works for optional args.
                    let l = match run(a, sc) {
                        Err(e) if e.starts_with("unknown name") => Value::Null,
                        r => r?,
                    };
                    return if l.is_null() || l == Value::String(String::new()) { run(b, sc) } else { Ok(l) };
                }
                _ => {}
            }
            let l = run(a, sc)?;
            let r = run(b, sc)?;
            match *op {
                "==" => Value::Bool(loose_eq(&l, &r)),
                "!=" => Value::Bool(!loose_eq(&l, &r)),
                "+" if l.is_string() || r.is_string() => Value::String(s(&l) + &s(&r)),
                "+" if l.is_array() && r.is_array() => {
                    let mut v = l.as_array().unwrap().clone();
                    v.extend(r.as_array().unwrap().iter().cloned());
                    Value::Array(v)
                }
                _ => {
                    let (x, y) = (f(&l, op)?, f(&r, op)?);
                    match *op {
                        "+" => num(x + y),
                        "-" => num(x - y),
                        "*" => num(x * y),
                        "/" => num(if y == 0.0 { 0.0 } else { x / y }),
                        "%" => num(if y == 0.0 { 0.0 } else { x.rem_euclid(y) }),
                        "<" => Value::Bool(x < y),
                        "<=" => Value::Bool(x <= y),
                        ">" => Value::Bool(x > y),
                        ">=" => Value::Bool(x >= y),
                        _ => unreachable!(),
                    }
                }
            }
        }
        Ex::Call(name, args) => {
            let a: Vec<Value> = args.iter().map(|x| run(x, sc)).collect::<Result<_, _>>()?;
            call(name, &a)?
        }
    })
}

fn loose_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        _ => a == b,
    }
}

/// Built-in functions.
pub const FUNCTIONS: &[(&str, &str)] = &[
    ("len(x)", "length of a list, string (characters) or object"),
    ("min(a, b, …) / max(a, b, …)", "smallest / largest number (also accepts one list)"),
    ("clamp(x, lo, hi)", "x limited to [lo, hi]"),
    ("round(x, digits?) / floor(x) / ceil(x) / abs(x)", "rounding"),
    ("sum(list)", "sum of numbers"),
    ("argmax(list)", "index of the largest number"),
    ("range(n) / range(a, b)", "list of integers"),
    ("words(text)", "number of words"),
    ("readtime(text)", "seconds needed to read text: words/3 + 1, at least 1.5"),
    ("upper(s) / lower(s) / trim(s)", "case and whitespace"),
    ("split(s, sep) / join(list, sep)", "strings to lists and back"),
    ("slice(x, start, end?)", "part of a list or string"),
    ("truncate(s, n)", "at most n characters, ending with … when cut"),
    ("default(x, fallback)", "fallback when x is null or empty"),
    ("pick(list, key)", "list of one field from a list of objects"),
    ("contrast(bg)", "#000000 or #FFFFFF, whichever reads best on bg"),
    ("mix(a, b, t)", "colour between a and b (t = 0..1)"),
    ("alpha(c, a)", "colour c with opacity a (0..1), as #RRGGBBAA"),
    ("lighten(c, t) / darken(c, t)", "mix towards white / black"),
    ("str(x) / num(x)", "conversions"),
];

fn arg(a: &[Value], i: usize) -> &Value {
    a.get(i).unwrap_or(&Value::Null)
}

fn nums(a: &[Value], name: &str) -> Result<Vec<f64>, String> {
    let list: Vec<&Value> = if a.len() == 1 {
        match &a[0] {
            Value::Array(v) => v.iter().collect(),
            v => vec![v],
        }
    } else {
        a.iter().collect()
    };
    list.into_iter().map(|v| f(v, name)).collect()
}

fn call(name: &str, a: &[Value]) -> Result<Value, String> {
    Ok(match name {
        "len" => Value::from(match arg(a, 0) {
            Value::Array(v) => v.len(),
            Value::String(s) => s.chars().count(),
            Value::Object(m) => m.len(),
            Value::Null => 0,
            _ => 1,
        }),
        "min" => num(nums(a, name)?.into_iter().fold(f64::INFINITY, f64::min)),
        "max" => num(nums(a, name)?.into_iter().fold(f64::NEG_INFINITY, f64::max)),
        "sum" => num(nums(a, name)?.into_iter().sum()),
        "argmax" => {
            let v = nums(a, name)?;
            Value::from(v.iter().enumerate().fold((0usize, f64::NEG_INFINITY), |b, (i, &x)| if x > b.1 { (i, x) } else { b }).0)
        }
        "clamp" => num(f(arg(a, 0), name)?.max(f(arg(a, 1), name)?).min(f(arg(a, 2), name)?)),
        "round" => {
            let p = 10f64.powi(f(arg(a, 1), name)? as i32);
            num((f(arg(a, 0), name)? * p).round() / p)
        }
        "floor" => num(f(arg(a, 0), name)?.floor()),
        "ceil" => num(f(arg(a, 0), name)?.ceil()),
        "abs" => num(f(arg(a, 0), name)?.abs()),
        "range" => {
            let (lo, hi) = if a.len() >= 2 { (f(&a[0], name)?, f(&a[1], name)?) } else { (0.0, f(arg(a, 0), name)?) };
            Value::Array((lo as i64..hi as i64).map(Value::from).collect())
        }
        "words" => Value::from(word_count(&s(arg(a, 0)))),
        "readtime" => {
            let text = match arg(a, 0) {
                Value::Array(v) => v.iter().map(s).collect::<Vec<_>>().join(" "),
                v => s(v),
            };
            num(read_time(&text))
        }
        "upper" => Value::String(s(arg(a, 0)).to_uppercase()),
        "lower" => Value::String(s(arg(a, 0)).to_lowercase()),
        "trim" => Value::String(s(arg(a, 0)).trim().to_string()),
        "str" => Value::String(s(arg(a, 0))),
        "num" => num(f(arg(a, 0), name)?),
        "split" => {
            let sep = s(arg(a, 1));
            let text = s(arg(a, 0));
            Value::Array(
                if sep.is_empty() { text.split_whitespace().collect::<Vec<_>>() } else { text.split(sep.as_str()).collect() }
                    .into_iter()
                    .map(|x| Value::String(x.trim().to_string()))
                    .collect(),
            )
        }
        "join" => Value::String(match arg(a, 0) {
            Value::Array(v) => v.iter().map(s).collect::<Vec<_>>().join(&s(arg(a, 1))),
            v => s(v),
        }),
        "slice" => {
            let start = f(arg(a, 1), name)? as i64;
            let end = if a.len() > 2 { Some(f(&a[2], name)? as i64) } else { None };
            let norm = |i: i64, n: usize| -> usize { if i < 0 { (n as i64 + i).max(0) as usize } else { (i as usize).min(n) } };
            match arg(a, 0) {
                Value::Array(v) => {
                    let (s0, e0) = (norm(start, v.len()), norm(end.unwrap_or(v.len() as i64), v.len()));
                    Value::Array(v[s0..e0.max(s0)].to_vec())
                }
                v => {
                    let c: Vec<char> = s(v).chars().collect();
                    let (s0, e0) = (norm(start, c.len()), norm(end.unwrap_or(c.len() as i64), c.len()));
                    Value::String(c[s0..e0.max(s0)].iter().collect())
                }
            }
        }
        "truncate" => {
            let n = f(arg(a, 1), name)? as usize;
            let t = s(arg(a, 0));
            if t.chars().count() <= n {
                Value::String(t)
            } else {
                let cut: String = t.chars().take(n.saturating_sub(1)).collect();
                Value::String(format!("{}…", cut.trim_end()))
            }
        }
        "default" => {
            let v = arg(a, 0);
            if v.is_null() || v.as_str() == Some("") || v.as_array().is_some_and(|x| x.is_empty()) {
                arg(a, 1).clone()
            } else {
                v.clone()
            }
        }
        "pick" => {
            let k = s(arg(a, 1));
            Value::Array(arg(a, 0).as_array().map(|v| v.iter().map(|o| o.get(&k).cloned().unwrap_or(Value::Null)).collect()).unwrap_or_default())
        }
        "contrast" => Value::String(color::to_hex(color::contrast_text(color::parse(&s(arg(a, 0)))?))),
        "mix" => {
            let t = f(arg(a, 2), name)?;
            Value::String(color::to_hex(color::mix(color::parse(&s(arg(a, 0)))?, color::parse(&s(arg(a, 1)))?, t)))
        }
        "alpha" => {
            let mut c = color::parse(&s(arg(a, 0)))?;
            c[3] *= f(arg(a, 1), name)?.clamp(0.0, 1.0);
            Value::String(color::to_hex(c))
        }
        "lighten" | "darken" => {
            let c = color::parse(&s(arg(a, 0)))?;
            let to = if name == "lighten" { [1.0, 1.0, 1.0, c[3]] } else { [0.0, 0.0, 0.0, c[3]] };
            Value::String(color::to_hex(color::mix(c, to, f(arg(a, 1), name)?)))
        }
        _ => {
            let names = FUNCTIONS.iter().map(|(sig, _)| sig.split('(').next().unwrap());
            let hint = crate::templates::suggest(name, names).map(|s| format!(" (did you mean '{s}'?)")).unwrap_or_default();
            return Err(format!("unknown function '{name}'{hint}"));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(src: &str, vars: Value) -> Value {
        let m = vars.as_object().unwrap().clone();
        let mut sc = Scope::new();
        sc.push(&m);
        eval(src, &sc).unwrap()
    }

    #[test]
    fn arithmetic_and_precedence() {
        assert_eq!(ev("1 + 2 * 3", json!({})), json!(7));
        assert_eq!(ev("(1 + 2) * 3", json!({})), json!(9));
        assert_eq!(ev("0.1 + 0.2", json!({})), json!(0.3));
        assert_eq!(ev("-i * 0.5 + 2", json!({"i": 2})), json!(1));
        assert_eq!(ev("7 % 3", json!({})), json!(1));
    }

    #[test]
    fn members_conditionals_and_functions() {
        let v = json!({"fmt": {"portrait": true, "w": 1080}, "list": [{"t": "a b c"}, {"t": "d"}], "x": null, "name": "Ada"});
        assert_eq!(ev("fmt.portrait ? 96 : 120", v.clone()), json!(96));
        assert_eq!(ev("fmt.w / 2", v.clone()), json!(540));
        assert_eq!(ev("len(list)", v.clone()), json!(2));
        assert_eq!(ev("list[0].t", v.clone()), json!("a b c"));
        assert_eq!(ev("list[-1].t", v.clone()), json!("d"));
        assert_eq!(ev("x ?? 'none'", v.clone()), json!("none"));
        assert_eq!(ev("'Hi ' + name", v.clone()), json!("Hi Ada"));
        assert_eq!(ev("readtime(list[0].t)", v.clone()), json!(2));
        assert_eq!(ev("upper(name)", v.clone()), json!("ADA"));
        assert_eq!(ev("join(pick(list, 't'), ' / ')", v.clone()), json!("a b c / d"));
        assert_eq!(ev("max(1, 5, 3)", v.clone()), json!(5));
        assert_eq!(ev("argmax([1, 5, 3])", v.clone()), json!(1));
        assert_eq!(ev("truncate('abcdefgh', 5)", v.clone()), json!("abcd…"));
        assert_eq!(ev("contrast('#111111')", v.clone()), json!("#FFFFFF"));
        assert_eq!(ev("alpha('#FF0000', 0.5)", v), json!("#FF000080"));
    }

    #[test]
    fn unknown_names_suggest() {
        let m = json!({"product": "X"}).as_object().unwrap().clone();
        let mut sc = Scope::new();
        sc.push(&m);
        let e = eval("prodcut", &sc).unwrap_err();
        assert!(e.contains("did you mean 'product'"), "{e}");
        let e = eval("lenght(product)", &sc).unwrap_err();
        assert!(e.contains("did you mean 'len'"), "{e}");
    }
}
