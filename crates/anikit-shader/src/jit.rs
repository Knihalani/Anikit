//! naga IR → native code with Cranelift.
//!
//! Execution model (SPMD, like a GPU quad): one kernel call processes a row
//! of 2×2 pixel quads; every shader value is a 4-lane vector (`f32x4` for
//! floats, `i32x4` for ints and bools, bools as 0/−1 lane masks), one lane
//! per pixel of the quad, so `dFdx`/`dFdy`/`fwidth` are lane shuffles.
//!
//! * Every naga type is flattened into scalar *slots* (a `mat3` is 9 slots,
//!   a struct its members' slots in order); each slot is one vector value.
//! * Variables (locals, private globals, out-parameters) are Cranelift
//!   `Variable`s per slot; pointers are (root variable, slot range, optional
//!   dynamic indices resolved with per-lane select chains).
//! * Divergent control flow runs under an execution mask: stores blend with
//!   the mask, `if` runs a branch only when some lane takes it, loops run
//!   while any lane is active, `break`/`continue`/`return`/`discard` clear
//!   lanes. Uniform control flow therefore costs only a mask test.
//! * Every function call is inlined (GLSL has no recursion).

use crate::front::{Kind, ShaderError};
use crate::texture::{self, Env};
use crate::vmath::{self as vm, any, as_f, as_i, fsplat, isplat, sel};
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::types::{F32, F32X4, I32, I32X4, I64};
use cranelift_codegen::ir::{AbiParam, Block, FuncRef, InstBuilder, MemFlagsData, StackSlot, StackSlotData, StackSlotKind, Type, Value};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, Linkage, Module};
use naga::valid::{FunctionInfo, ModuleInfo};
use naga::{Expression as E, Handle, ScalarKind as SK, Statement as S, TypeInner as T};
use std::collections::HashMap;

/// `kernel(env, uniforms, x0, y0, quads, out)`: shades `quads` quads starting
/// at fragCoord `(x0, y0)` (lane 0), writing r,g,b,a planes (16 floats) per quad.
pub type KernelFn = unsafe extern "C" fn(*const Env, *const u32, f32, f32, i32, *mut f32);

/// Most iterations one loop may run per quad before it is cut off.
const LOOP_CAP: i64 = 1 << 20;

pub struct Kernel {
    module: Option<JITModule>,
    pub func: KernelFn,
    /// Number of Cranelift IR instructions (for stats).
    pub insts: usize,
}

// SAFETY: the compiled code is immutable once finalised; the module is only
// touched again in `Drop`.
unsafe impl Send for Kernel {}
unsafe impl Sync for Kernel {}

impl Drop for Kernel {
    fn drop(&mut self) {
        if let Some(m) = self.module.take() {
            // SAFETY: nothing can call `func` once the kernel is dropped.
            unsafe { m.free_memory() };
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum K {
    F,
    I,
    U,
    B,
}

impl K {
    fn ty(self) -> Type {
        if self == K::F {
            F32X4
        } else {
            I32X4
        }
    }
}

#[derive(Clone)]
struct Ptr {
    root: usize,
    off: usize,
    len: usize,
    /// Dynamic indices: (lane index vector, slot stride, element count).
    dynx: Vec<(Value, usize, usize)>,
}

#[derive(Clone)]
enum Val {
    V(Vec<Value>),
    P(Ptr),
    Img(u32),
    Smp,
}

enum Ctl {
    Loop { mask: Variable, cnt: Variable },
    Switch { brk: Variable },
}

struct Frame<'m> {
    f: &'m naga::Function,
    fi: &'m FunctionInfo,
    args: Vec<Val>,
    ex: Vec<Option<Val>>,
    locals: Vec<usize>,
    ret: Variable,
    ret_vars: Vec<Variable>,
    ret_kinds: Vec<K>,
    ctl: Vec<Ctl>,
    /// Whether the caller's mask was its own entry mask at the call.
    entry_full: bool,
    /// Loops (real or unrolled) currently open in this frame.
    loops: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sc {
    Global,
    Top,
}

struct Lower<'m, 'b> {
    m: &'m naga::Module,
    info: &'m ModuleInfo,
    b: FunctionBuilder<'b>,
    roots: Vec<Vec<Variable>>,
    /// Frame depth that owns each root (0 = kernel: globals, output).
    root_owner: Vec<usize>,
    root_kinds: Vec<Vec<K>>,
    gexpr: Vec<Option<Val>>,
    globals: HashMap<Handle<naga::GlobalVariable>, Val>,
    frames: Vec<Frame<'m>>,
    exec: Variable,
    killed: Variable,
    has_kill: bool,
    full: bool,
    /// Expressions emitted by loop unrolling so far (bounded code growth).
    unrolled: usize,
    env: Value,
    sample_ref: FuncRef,
    fetch_ref: FuncRef,
    scratch: StackSlot,
    ptr_ty: Type,
    depth: usize,
}

fn err(msg: impl Into<String>) -> ShaderError {
    ShaderError::new(msg)
}

type R<T> = Result<T, ShaderError>;

fn skind(s: naga::Scalar) -> R<K> {
    Ok(match s.kind {
        SK::Float | SK::AbstractFloat => K::F,
        SK::Sint | SK::AbstractInt => K::I,
        SK::Uint => K::U,
        SK::Bool => K::B,
    })
}

fn vsize(s: naga::VectorSize) -> usize {
    s as usize
}

impl<'m, 'b> Lower<'m, 'b> {
    // ---------- types ----------

    fn kinds(&self, ty: &T) -> R<Vec<K>> {
        Ok(match ty {
            T::Scalar(s) => vec![skind(*s)?],
            T::Vector { size, scalar } => vec![skind(*scalar)?; vsize(*size)],
            T::Matrix { columns, rows, .. } => vec![K::F; vsize(*columns) * vsize(*rows)],
            T::Array { base, size: naga::ArraySize::Constant(n), .. } => {
                let e = self.kinds(&self.m.types[*base].inner)?;
                let mut v = Vec::with_capacity(e.len() * n.get() as usize);
                for _ in 0..n.get() {
                    v.extend_from_slice(&e);
                }
                v
            }
            T::Struct { members, .. } => {
                let mut v = Vec::new();
                for mem in members {
                    v.extend(self.kinds(&self.m.types[mem.ty].inner)?);
                }
                v
            }
            T::Atomic(s) => vec![skind(*s)?],
            _ => return Err(err(format!("unsupported type {ty:?} (images, samplers and runtime-sized arrays can't be stored in variables)"))),
        })
    }

    fn nslots(&self, ty: &T) -> R<usize> {
        Ok(self.kinds(ty)?.len())
    }

    fn ety(&self, sc: Sc, h: Handle<E>) -> &'m T {
        match sc {
            Sc::Global => self.info[h].inner_with(&self.m.types),
            Sc::Top => {
                let fr = self.frames.last().unwrap();
                fr.fi[h].ty.inner_with(&self.m.types)
            }
        }
    }

    fn ekind(&self, sc: Sc, h: Handle<E>) -> R<K> {
        match self.ety(sc, h) {
            T::Scalar(s) | T::Vector { scalar: s, .. } | T::Matrix { scalar: s, .. } | T::ValuePointer { scalar: s, .. } => skind(*s),
            T::Pointer { base, .. } => match &self.m.types[*base].inner {
                T::Scalar(s) | T::Vector { scalar: s, .. } | T::Matrix { scalar: s, .. } => skind(*s),
                _ => Ok(K::F),
            },
            _ => Ok(K::F),
        }
    }

    /// The type a pointer expression points at.
    fn pointee(&self, sc: Sc, h: Handle<E>) -> R<T> {
        Ok(match self.ety(sc, h) {
            T::Pointer { base, .. } => self.m.types[*base].inner.clone(),
            T::ValuePointer { size: Some(n), scalar, .. } => T::Vector { size: *n, scalar: *scalar },
            T::ValuePointer { size: None, scalar, .. } => T::Scalar(*scalar),
            t => t.clone(),
        })
    }

    fn span_err(&self, sc: Sc, h: Handle<E>, msg: String) -> ShaderError {
        let _ = (sc, h);
        err(msg)
    }

    // ---------- small helpers ----------

    fn zero(&mut self, k: K) -> Value {
        if k == K::F {
            fsplat(&mut self.b, 0.0)
        } else {
            isplat(&mut self.b, 0)
        }
    }

    fn mask_all(&mut self) -> Value {
        isplat(&mut self.b, -1)
    }

    fn new_root(&mut self, kinds: Vec<K>) -> usize {
        let vars: Vec<Variable> = kinds.iter().map(|k| self.b.declare_var(k.ty())).collect();
        self.roots.push(vars);
        self.root_kinds.push(kinds);
        self.root_owner.push(self.frames.len());
        self.roots.len() - 1
    }

    fn exec(&mut self) -> Value {
        self.b.use_var(self.exec)
    }

    fn set_exec(&mut self, v: Value) {
        self.b.def_var(self.exec, v);
    }

    /// Masked write of one slot.
    fn write(&mut self, var: Variable, val: Value, mask: Option<Value>) {
        match mask {
            None => self.b.def_var(var, val),
            Some(m) => {
                let old = self.b.use_var(var);
                let v = sel(&mut self.b, m, val, old);
                self.b.def_var(var, v);
            }
        }
    }

    /// Mask for a store into a variable owned by frame depth `owner`: none
    /// when the current lanes are exactly the lanes that variable's frame
    /// was entered with (then inactive lanes can't observe the write).
    fn mask_for(&mut self, owner: usize) -> Option<Value> {
        if self.full && self.frames[owner.min(self.frames.len())..].iter().all(|f| f.entry_full) {
            None
        } else {
            Some(self.exec())
        }
    }

    fn get(&mut self, sc: Sc, h: Handle<E>) -> R<Val> {
        match sc {
            Sc::Global => self.gexpr[h.index()].clone().ok_or_else(|| err("unsupported constant expression")),
            Sc::Top => {
                if let Some(v) = &self.frames.last().unwrap().ex[h.index()] {
                    return Ok(v.clone());
                }
                let v = self.lower(sc, h)?;
                self.frames.last_mut().unwrap().ex[h.index()] = Some(v.clone());
                Ok(v)
            }
        }
    }

    fn vals(&mut self, sc: Sc, h: Handle<E>) -> R<Vec<Value>> {
        match self.get(sc, h)? {
            Val::V(v) => Ok(v),
            Val::P(p) => self.load(&p),
            _ => Err(self.span_err(sc, h, "a texture or sampler can't be used as a value".into())),
        }
    }

    fn load(&mut self, p: &Ptr) -> R<Vec<Value>> {
        let vars = self.roots[p.root].clone();
        if p.dynx.is_empty() {
            return Ok((p.off..p.off + p.len).map(|i| self.b.use_var(vars[i.min(vars.len() - 1)])).collect());
        }
        // Select over every combination of dynamic indices (index 0 also catches out-of-range).
        let combos = combos(&p.dynx);
        let mut out: Vec<Value> = Vec::new();
        for (ci, (offset, conds)) in combos.iter().enumerate() {
            let base = p.off + offset;
            let vals: Vec<Value> = (0..p.len).map(|i| self.b.use_var(vars[(base + i).min(vars.len() - 1)])).collect();
            if ci == 0 {
                out = vals;
                continue;
            }
            let c = self.combo_mask(&p.dynx, conds);
            for i in 0..p.len {
                out[i] = sel(&mut self.b, c, vals[i], out[i]);
            }
        }
        Ok(out)
    }

    fn combo_mask(&mut self, dynx: &[(Value, usize, usize)], conds: &[usize]) -> Value {
        let mut m: Option<Value> = None;
        for (j, &k) in conds.iter().enumerate() {
            let kv = isplat(&mut self.b, k as i32);
            let c = self.b.ins().icmp(IntCC::Equal, dynx[j].0, kv);
            m = Some(match m {
                None => c,
                Some(p) => self.b.ins().band(p, c),
            });
        }
        m.unwrap()
    }

    fn store(&mut self, p: &Ptr, vals: &[Value]) -> R<()> {
        let vars = self.roots[p.root].clone();
        let mask = self.mask_for(self.root_owner[p.root]);
        if p.dynx.is_empty() {
            for i in 0..p.len.min(vals.len()) {
                if let Some(&v) = vars.get(p.off + i) {
                    self.write(v, vals[i], mask);
                }
            }
            return Ok(());
        }
        let combos = combos(&p.dynx);
        for (offset, conds) in combos.iter() {
            let c = self.combo_mask(&p.dynx, conds);
            let m = match mask {
                Some(e) => self.b.ins().band(e, c),
                None => c,
            };
            for i in 0..p.len.min(vals.len()) {
                if let Some(&v) = vars.get(p.off + offset + i) {
                    self.write(v, vals[i], Some(m));
                }
            }
        }
        Ok(())
    }

    // ---------- expressions ----------

    fn lit(&mut self, l: &naga::Literal) -> R<Value> {
        use naga::Literal as L;
        Ok(match *l {
            L::F32(v) => fsplat(&mut self.b, v),
            L::F64(v) | L::AbstractFloat(v) => fsplat(&mut self.b, v as f32),
            L::I32(v) => isplat(&mut self.b, v),
            L::U32(v) => isplat(&mut self.b, v as i32),
            L::I64(v) | L::AbstractInt(v) => isplat(&mut self.b, v as i32),
            L::U64(v) => isplat(&mut self.b, v as i32),
            L::Bool(v) => isplat(&mut self.b, if v { -1 } else { 0 }),
            _ => return Err(err(format!("unsupported literal {l:?}"))),
        })
    }

    fn expr_of(&self, sc: Sc, h: Handle<E>) -> &'m E {
        match sc {
            Sc::Global => &self.m.global_expressions[h],
            Sc::Top => &self.frames.last().unwrap().f.expressions[h],
        }
    }

    fn lower(&mut self, sc: Sc, h: Handle<E>) -> R<Val> {
        let e = self.expr_of(sc, h);
        Ok(match e {
            E::Literal(l) => Val::V(vec![self.lit(l)?]),
            E::Constant(c) => {
                let init = self.m.constants[*c].init;
                self.gexpr[init.index()].clone().ok_or_else(|| err("unsupported constant"))?
            }
            E::ZeroValue(ty) => {
                let ks = self.kinds(&self.m.types[*ty].inner)?;
                Val::V(ks.into_iter().map(|k| self.zero(k)).collect())
            }
            E::Compose { components, .. } => {
                let mut v = Vec::new();
                for c in components {
                    v.extend(self.vals(sc, *c)?);
                }
                Val::V(v)
            }
            E::Splat { size, value } => {
                let x = self.vals(sc, *value)?[0];
                Val::V(vec![x; vsize(*size)])
            }
            E::Swizzle { size, vector, pattern } => {
                let v = self.vals(sc, *vector)?;
                Val::V((0..vsize(*size)).map(|i| v[(pattern[i] as usize).min(v.len() - 1)]).collect())
            }
            E::AccessIndex { base, index } => self.access_index(sc, *base, *index as usize)?,
            E::Access { base, index } => self.access(sc, *base, *index)?,
            E::FunctionArgument(i) => self.frames.last().unwrap().args[*i as usize].clone(),
            E::GlobalVariable(g) => {
                self.globals.get(g).cloned().ok_or_else(|| err(format!("unsupported global variable `{}`", self.m.global_variables[*g].name.clone().unwrap_or_default())))?
            }
            E::LocalVariable(l) => {
                let root = self.frames.last().unwrap().locals[l.index()];
                let len = self.roots[root].len();
                Val::P(Ptr { root, off: 0, len, dynx: Vec::new() })
            }
            E::Load { pointer } => match self.get(sc, *pointer)? {
                Val::P(p) => Val::V(self.load(&p)?),
                v => v,
            },
            E::Unary { op, expr } => {
                let k = self.ekind(sc, *expr)?;
                let v = self.vals(sc, *expr)?;
                Val::V(
                    v.into_iter()
                        .map(|x| match (op, k) {
                            (naga::UnaryOperator::Negate, K::F) => self.b.ins().fneg(x),
                            (naga::UnaryOperator::Negate, _) => self.b.ins().ineg(x),
                            (_, _) => self.b.ins().bnot(x),
                        })
                        .collect(),
                )
            }
            E::Binary { op, left, right } => Val::V(self.binary(sc, *op, *left, *right)?),
            E::Select { condition, accept, reject } => {
                let c = self.vals(sc, *condition)?;
                let a = self.vals(sc, *accept)?;
                let r = self.vals(sc, *reject)?;
                Val::V((0..a.len()).map(|i| sel(&mut self.b, c[i.min(c.len() - 1)], a[i], r[i])).collect())
            }
            E::Derivative { axis, expr, .. } => {
                let v = self.vals(sc, *expr)?;
                Val::V(v.into_iter().map(|x| self.deriv(*axis, x)).collect())
            }
            E::Relational { fun, argument } => {
                let v = self.vals(sc, *argument)?;
                use naga::RelationalFunction as RF;
                match fun {
                    RF::All | RF::Any => {
                        let mut r = v[0];
                        for &x in &v[1..] {
                            r = if *fun == RF::All { self.b.ins().band(r, x) } else { self.b.ins().bor(r, x) };
                        }
                        Val::V(vec![r])
                    }
                    RF::IsNan => Val::V(v.into_iter().map(|x| self.b.ins().fcmp(FloatCC::Unordered, x, x)).collect()),
                    RF::IsInf => Val::V(
                        v.into_iter()
                            .map(|x| {
                                let a = self.b.ins().fabs(x);
                                let inf = fsplat(&mut self.b, f32::INFINITY);
                                self.b.ins().fcmp(FloatCC::Equal, a, inf)
                            })
                            .collect(),
                    ),
                }
            }
            E::Math { fun, arg, arg1, arg2, arg3 } => Val::V(self.math(sc, *fun, *arg, *arg1, *arg2, *arg3)?),
            E::As { expr, kind, convert } => Val::V(self.cast(sc, *expr, *kind, *convert)?),
            E::ImageSample { image, coordinate, level, offset, gather, depth_ref, .. } => {
                if offset.is_some() || gather.is_some() || depth_ref.is_some() {
                    return Err(err("textureOffset, textureGather and shadow samplers are not supported"));
                }
                Val::V(self.sample(sc, *image, *coordinate, level)?)
            }
            E::ImageLoad { image, coordinate, level, .. } => Val::V(self.fetch(sc, *image, *coordinate, *level)?),
            E::ImageQuery { image, query } => {
                let Val::Img(ch) = self.get(sc, *image)? else { return Err(err("textureSize needs an iChannel")) };
                match query {
                    naga::ImageQuery::Size { .. } => {
                        let mut out = Vec::new();
                        for k in 0..2 {
                            let s = self.b.ins().load(I32, MemFlagsData::trusted(), self.env, texture::ENV_SIZES + (ch as i32 * 8) + k * 4);
                            out.push(self.b.ins().splat(I32X4, s));
                        }
                        Val::V(out)
                    }
                    _ => Val::V(vec![isplat(&mut self.b, 1)]),
                }
            }
            E::CallResult(_) => return Err(err("internal: call result used before the call")),
            other => return Err(self.span_err(sc, h, format!("unsupported expression {:?}", std::mem::discriminant(other)))),
        })
    }

    fn access_index(&mut self, sc: Sc, base: Handle<E>, index: usize) -> R<Val> {
        let bv = self.get(sc, base)?;
        let ty = match &bv {
            Val::P(_) => self.pointee(sc, base)?,
            _ => self.ety(sc, base).clone(),
        };
        let (off, len) = match &ty {
            T::Vector { .. } => (index, 1),
            T::Matrix { rows, .. } => (index * vsize(*rows), vsize(*rows)),
            T::Array { base: eb, .. } => {
                let n = self.nslots(&self.m.types[*eb].inner)?;
                (index * n, n)
            }
            T::Struct { members, .. } => {
                let mut o = 0;
                for mem in &members[..index] {
                    o += self.nslots(&self.m.types[mem.ty].inner)?;
                }
                (o, self.nslots(&self.m.types[members[index].ty].inner)?)
            }
            _ => (index, 1),
        };
        Ok(match bv {
            Val::P(mut p) => {
                p.off += off;
                p.len = len;
                Val::P(p)
            }
            Val::V(v) => Val::V(v[off.min(v.len() - 1)..(off + len).min(v.len())].to_vec()),
            other => other,
        })
    }

    fn access(&mut self, sc: Sc, base: Handle<E>, index: Handle<E>) -> R<Val> {
        let bv = self.get(sc, base)?;
        let ty = match &bv {
            Val::P(_) => self.pointee(sc, base)?,
            _ => self.ety(sc, base).clone(),
        };
        let (stride, count) = match &ty {
            T::Vector { size, .. } => (1, vsize(*size)),
            T::Matrix { rows, columns, .. } => (vsize(*rows), vsize(*columns)),
            T::Array { base: eb, size: naga::ArraySize::Constant(n), .. } => (self.nslots(&self.m.types[*eb].inner)?, n.get() as usize),
            _ => return Err(err("dynamic indexing is only supported on vectors, matrices and fixed-size arrays")),
        };
        // A constant index folds to a static access.
        if let E::Literal(l) = self.expr_of(sc, index) {
            let i = match *l {
                naga::Literal::I32(v) => v as i64,
                naga::Literal::U32(v) => v as i64,
                naga::Literal::AbstractInt(v) => v,
                _ => -1,
            };
            if i >= 0 && (i as usize) < count {
                return self.access_index(sc, base, i as usize);
            }
        }
        let idx = self.vals(sc, index)?[0];
        Ok(match bv {
            Val::P(mut p) => {
                p.len = stride;
                p.dynx.push((idx, stride, count));
                Val::P(p)
            }
            Val::V(v) => {
                let mut out: Vec<Value> = v[0..stride].to_vec();
                for k in 1..count {
                    let kv = isplat(&mut self.b, k as i32);
                    let c = self.b.ins().icmp(IntCC::Equal, idx, kv);
                    for i in 0..stride {
                        out[i] = sel(&mut self.b, c, v[k * stride + i], out[i]);
                    }
                }
                Val::V(out)
            }
            other => other,
        })
    }

    fn deriv(&mut self, axis: naga::DerivativeAxis, x: Value) -> Value {
        // Lanes: 0 (x, top) 1 (x+1, top) 2 (x, bottom) 3 (x+1, bottom); shader y grows upward.
        let dx = {
            let a = vm::shuffle4(&mut self.b, x, [1, 1, 3, 3]);
            let c = vm::shuffle4(&mut self.b, x, [0, 0, 2, 2]);
            self.b.ins().fsub(a, c)
        };
        let dy = || -> Option<()> { None };
        let _ = dy;
        let ddy = |s: &mut Self| {
            let a = vm::shuffle4(&mut s.b, x, [0, 1, 0, 1]);
            let c = vm::shuffle4(&mut s.b, x, [2, 3, 2, 3]);
            s.b.ins().fsub(a, c)
        };
        match axis {
            naga::DerivativeAxis::X => dx,
            naga::DerivativeAxis::Y => ddy(self),
            naga::DerivativeAxis::Width => {
                let y = ddy(self);
                let ax = self.b.ins().fabs(dx);
                let ay = self.b.ins().fabs(y);
                self.b.ins().fadd(ax, ay)
            }
        }
    }

    fn binary(&mut self, sc: Sc, op: naga::BinaryOperator, l: Handle<E>, r: Handle<E>) -> R<Vec<Value>> {
        use naga::BinaryOperator as O;
        let lt = self.ety(sc, l).clone();
        let rt = self.ety(sc, r).clone();
        let a = self.vals(sc, l)?;
        let c = self.vals(sc, r)?;
        if op == O::Multiply {
            match (&lt, &rt) {
                (T::Matrix { columns, rows, .. }, T::Vector { .. }) => {
                    let (nc, nr) = (vsize(*columns), vsize(*rows));
                    return Ok((0..nr)
                        .map(|i| {
                            let mut s = self.b.ins().fmul(a[i], c[0]);
                            for k in 1..nc {
                                let t = self.b.ins().fmul(a[k * nr + i], c[k]);
                                s = self.b.ins().fadd(s, t);
                            }
                            s
                        })
                        .collect());
                }
                (T::Vector { .. }, T::Matrix { columns, rows, .. }) => {
                    let (nc, nr) = (vsize(*columns), vsize(*rows));
                    return Ok((0..nc)
                        .map(|j| {
                            let mut s = self.b.ins().fmul(a[0], c[j * nr]);
                            for k in 1..nr {
                                let t = self.b.ins().fmul(a[k], c[j * nr + k]);
                                s = self.b.ins().fadd(s, t);
                            }
                            s
                        })
                        .collect());
                }
                (T::Matrix { columns: c1, rows: r1, .. }, T::Matrix { columns: c2, rows: r2, .. }) => {
                    let (r1, k1, c2, r2) = (vsize(*r1), vsize(*c1), vsize(*c2), vsize(*r2));
                    let mut out = Vec::with_capacity(c2 * r1);
                    for j in 0..c2 {
                        for i in 0..r1 {
                            let mut s = self.b.ins().fmul(a[i], c[j * r2]);
                            for k in 1..k1 {
                                let t = self.b.ins().fmul(a[k * r1 + i], c[j * r2 + k]);
                                s = self.b.ins().fadd(s, t);
                            }
                            out.push(s);
                        }
                    }
                    return Ok(out);
                }
                _ => {}
            }
        }
        let k = self.ekind(sc, l)?;
        let n = a.len().max(c.len());
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let x = a[if a.len() == 1 { 0 } else { i }];
            let y = c[if c.len() == 1 { 0 } else { i }];
            out.push(self.binop(op, k, x, y)?);
        }
        Ok(out)
    }

    fn binop(&mut self, op: naga::BinaryOperator, k: K, x: Value, y: Value) -> R<Value> {
        use naga::BinaryOperator as O;
        let b = &mut self.b;
        Ok(match (k, op) {
            (K::F, O::Add) => b.ins().fadd(x, y),
            (K::F, O::Subtract) => b.ins().fsub(x, y),
            (K::F, O::Multiply) => b.ins().fmul(x, y),
            (K::F, O::Divide) => match const_splat_f32(b, y) {
                // Dividing by a constant: multiply by its reciprocal. Exact for powers
                // of two, within 1.5 ulp otherwise (GLSL allows 2.5), and a multiply
                // has a third of a divide's latency on the march loops' critical path.
                Some(k) if k != 0.0 && k.is_finite() && (1.0 / k).is_normal() => {
                    let r = fsplat(b, 1.0 / k);
                    b.ins().fmul(x, r)
                }
                _ => b.ins().fdiv(x, y),
            },
            (K::F, O::Modulo) => {
                let q = b.ins().fdiv(x, y);
                let t = b.ins().trunc(q);
                let m = b.ins().fmul(t, y);
                b.ins().fsub(x, m)
            }
            (K::F, O::Equal) => b.ins().fcmp(FloatCC::Equal, x, y),
            (K::F, O::NotEqual) => b.ins().fcmp(FloatCC::NotEqual, x, y),
            (K::F, O::Less) => b.ins().fcmp(FloatCC::LessThan, x, y),
            (K::F, O::LessEqual) => b.ins().fcmp(FloatCC::LessThanOrEqual, x, y),
            (K::F, O::Greater) => b.ins().fcmp(FloatCC::GreaterThan, x, y),
            (K::F, O::GreaterEqual) => b.ins().fcmp(FloatCC::GreaterThanOrEqual, x, y),
            (_, O::Add) => b.ins().iadd(x, y),
            (_, O::Subtract) => b.ins().isub(x, y),
            (_, O::Multiply) => b.ins().imul(x, y),
            (_, O::Divide) | (_, O::Modulo) => self.idiv(op == O::Modulo, k == K::U, x, y),
            (_, O::Equal) => b.ins().icmp(IntCC::Equal, x, y),
            (_, O::NotEqual) => b.ins().icmp(IntCC::NotEqual, x, y),
            (K::U, O::Less) => b.ins().icmp(IntCC::UnsignedLessThan, x, y),
            (K::U, O::LessEqual) => b.ins().icmp(IntCC::UnsignedLessThanOrEqual, x, y),
            (K::U, O::Greater) => b.ins().icmp(IntCC::UnsignedGreaterThan, x, y),
            (K::U, O::GreaterEqual) => b.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, x, y),
            (_, O::Less) => b.ins().icmp(IntCC::SignedLessThan, x, y),
            (_, O::LessEqual) => b.ins().icmp(IntCC::SignedLessThanOrEqual, x, y),
            (_, O::Greater) => b.ins().icmp(IntCC::SignedGreaterThan, x, y),
            (_, O::GreaterEqual) => b.ins().icmp(IntCC::SignedGreaterThanOrEqual, x, y),
            (_, O::And) | (_, O::LogicalAnd) => b.ins().band(x, y),
            (_, O::InclusiveOr) | (_, O::LogicalOr) => b.ins().bor(x, y),
            (_, O::ExclusiveOr) => b.ins().bxor(x, y),
            (_, O::ShiftLeft) | (_, O::ShiftRight) => self.shift(op == O::ShiftLeft, k == K::U, x, y),
        })
    }

    /// A splat's scalar operand, if `v` is a splat.
    fn splat_scalar(&self, v: Value) -> Option<Value> {
        let inst = self.b.func.dfg.value_def(v).inst()?;
        if self.b.func.dfg.insts[inst].opcode() == cranelift_codegen::ir::Opcode::Splat {
            return Some(self.b.func.dfg.inst_args(inst)[0]);
        }
        None
    }

    fn shift(&mut self, left: bool, unsigned: bool, x: Value, y: Value) -> Value {
        if let Some(s) = self.splat_scalar(y) {
            return if left {
                self.b.ins().ishl(x, s)
            } else if unsigned {
                self.b.ins().ushr(x, s)
            } else {
                self.b.ins().sshr(x, s)
            };
        }
        self.per_lane(x, y, |b, a, c| {
            if left {
                b.ins().ishl(a, c)
            } else if unsigned {
                b.ins().ushr(a, c)
            } else {
                b.ins().sshr(a, c)
            }
        })
    }

    fn per_lane(&mut self, x: Value, y: Value, f: impl Fn(&mut FunctionBuilder<'b>, Value, Value) -> Value) -> Value {
        let mut out = isplat(&mut self.b, 0);
        for l in 0..4u8 {
            let a = self.b.ins().extractlane(x, l);
            let c = self.b.ins().extractlane(y, l);
            let r = f(&mut self.b, a, c);
            out = self.b.ins().insertlane(out, r, l);
        }
        out
    }

    fn idiv(&mut self, rem: bool, unsigned: bool, x: Value, y: Value) -> Value {
        self.per_lane(x, y, move |b, a, d| {
            // Guard against traps: d == 0 → 0; INT_MIN / −1 wraps.
            let zero = b.ins().iconst(I32, 0);
            let one = b.ins().iconst(I32, 1);
            let is0 = b.ins().icmp(IntCC::Equal, d, zero);
            let safe = b.ins().select(is0, one, d);
            if unsigned {
                let r = if rem { b.ins().urem(a, safe) } else { b.ins().udiv(a, safe) };
                b.ins().select(is0, zero, r)
            } else {
                let m1 = b.ins().iconst(I32, -1);
                let isn = b.ins().icmp(IntCC::Equal, safe, m1);
                let safe2 = b.ins().select(isn, one, safe);
                let r = if rem { b.ins().srem(a, safe2) } else { b.ins().sdiv(a, safe2) };
                let neg = b.ins().ineg(a);
                let alt = if rem { zero } else { neg };
                let r = b.ins().select(isn, alt, r);
                b.ins().select(is0, zero, r)
            }
        })
    }

    fn cast(&mut self, sc: Sc, h: Handle<E>, kind: SK, convert: Option<u8>) -> R<Vec<Value>> {
        let from = self.ekind(sc, h)?;
        let to = skind(naga::Scalar { kind, width: 4 })?;
        let v = self.vals(sc, h)?;
        let mut out = Vec::with_capacity(v.len());
        for x in v {
            let b = &mut self.b;
            out.push(match (convert.is_some(), from, to) {
                (_, a, c) if a == c => x,
                (false, K::F, _) => as_i(b, x),
                (false, _, K::F) => as_f(b, x),
                (false, _, _) => x,
                (true, K::F, K::I) => b.ins().fcvt_to_sint_sat(I32X4, x),
                (true, K::F, K::U) => {
                    // Values ≥ 2³¹ need the high bit.
                    let big = fsplat(b, 2_147_483_648.0);
                    let hi = b.ins().fcmp(FloatCC::GreaterThanOrEqual, x, big);
                    let lo_v = b.ins().fcvt_to_sint_sat(I32X4, x);
                    let xs = b.ins().fsub(x, big);
                    let hv = b.ins().fcvt_to_sint_sat(I32X4, xs);
                    let sb = isplat(b, i32::MIN);
                    let hv = b.ins().bxor(hv, sb);
                    sel(b, hi, hv, lo_v)
                }
                (true, K::I, K::F) => b.ins().fcvt_from_sint(F32X4, x),
                (true, K::U, K::F) => b.ins().fcvt_from_uint(F32X4, x),
                (true, K::B, K::F) => {
                    let one = fsplat(b, 1.0);
                    let zero = fsplat(b, 0.0);
                    sel(b, x, one, zero)
                }
                (true, K::B, _) => {
                    let one = isplat(b, 1);
                    b.ins().band(x, one)
                }
                (true, K::F, K::B) => {
                    let zero = fsplat(b, 0.0);
                    b.ins().fcmp(FloatCC::NotEqual, x, zero)
                }
                (true, _, K::B) => {
                    let zero = isplat(b, 0);
                    b.ins().icmp(IntCC::NotEqual, x, zero)
                }
                (true, _, _) => x,
            });
        }
        Ok(out)
    }

    fn math(&mut self, sc: Sc, fun: naga::MathFunction, a0: Handle<E>, a1: Option<Handle<E>>, a2: Option<Handle<E>>, _a3: Option<Handle<E>>) -> R<Vec<Value>> {
        use naga::MathFunction as M;
        let k = self.ekind(sc, a0)?;
        let x = self.vals(sc, a0)?;
        let y = match a1 {
            Some(h) => self.vals(sc, h)?,
            None => Vec::new(),
        };
        let z = match a2 {
            Some(h) => self.vals(sc, h)?,
            None => Vec::new(),
        };
        let n = x.len().max(y.len()).max(z.len());
        let at = |v: &Vec<Value>, i: usize| v[if v.len() == 1 { 0 } else { i }];
        let b = &mut self.b;
        macro_rules! map1 {
            ($f:expr) => {{
                let f = $f;
                x.iter().map(|&v| f(&mut *b, v)).collect::<Vec<Value>>()
            }};
        }
        macro_rules! map2 {
            ($f:expr) => {{
                let f = $f;
                (0..n).map(|i| f(&mut *b, at(&x, i), at(&y, i))).collect::<Vec<Value>>()
            }};
        }
        macro_rules! map3 {
            ($f:expr) => {{
                let f = $f;
                (0..n).map(|i| f(&mut *b, at(&x, i), at(&y, i), at(&z, i))).collect::<Vec<Value>>()
            }};
        }
        fn dot(b: &mut FunctionBuilder, x: &[Value], y: &[Value]) -> Value {
            let mut s = b.ins().fmul(x[0], y[0]);
            for i in 1..x.len() {
                let t = b.ins().fmul(x[i], y[i]);
                s = b.ins().fadd(s, t);
            }
            s
        }
        let int = k != K::F;
        Ok(match fun {
            M::Abs if int => map1!(|b: &mut FunctionBuilder, v| {
                let n = b.ins().ineg(v);
                let z = isplat(b, 0);
                let neg = b.ins().icmp(IntCC::SignedLessThan, v, z);
                sel(b, neg, n, v)
            }),
            M::Abs => map1!(|b: &mut FunctionBuilder, v| b.ins().fabs(v)),
            M::Min if k == K::U => map2!(|b: &mut FunctionBuilder, p, q| b.ins().umin(p, q)),
            M::Min if int => map2!(|b: &mut FunctionBuilder, p, q| b.ins().smin(p, q)),
            M::Min => map2!(|b: &mut FunctionBuilder, p, q| vm::fmin(b, p, q)),
            M::Max if k == K::U => map2!(|b: &mut FunctionBuilder, p, q| b.ins().umax(p, q)),
            M::Max if int => map2!(|b: &mut FunctionBuilder, p, q| b.ins().smax(p, q)),
            M::Max => map2!(|b: &mut FunctionBuilder, p, q| vm::fmax(b, p, q)),
            M::Clamp if k == K::U => map3!(|b: &mut FunctionBuilder, v, lo, hi| {
                let t = b.ins().umax(v, lo);
                b.ins().umin(t, hi)
            }),
            M::Clamp if int => map3!(|b: &mut FunctionBuilder, v, lo, hi| {
                let t = b.ins().smax(v, lo);
                b.ins().smin(t, hi)
            }),
            M::Clamp => map3!(|b: &mut FunctionBuilder, v, lo, hi| vm::clamp(b, v, lo, hi)),
            M::Saturate => map1!(|b: &mut FunctionBuilder, v| {
                let lo = fsplat(b, 0.0);
                let hi = fsplat(b, 1.0);
                vm::clamp(b, v, lo, hi)
            }),
            M::Cos => map1!(vm::cos),
            M::Cosh => map1!(vm::cosh),
            M::Sin => map1!(vm::sin),
            M::Sinh => map1!(vm::sinh),
            M::Tan => map1!(vm::tan),
            M::Tanh => map1!(vm::tanh),
            M::Acos => map1!(vm::acos),
            M::Asin => map1!(vm::asin),
            M::Atan => map1!(vm::atan),
            M::Atan2 => map2!(vm::atan2),
            M::Asinh => map1!(vm::asinh),
            M::Acosh => map1!(vm::acosh),
            M::Atanh => map1!(vm::atanh),
            M::Radians => map1!(|b: &mut FunctionBuilder, v| {
                let c = fsplat(b, std::f32::consts::PI / 180.0);
                b.ins().fmul(v, c)
            }),
            M::Degrees => map1!(|b: &mut FunctionBuilder, v| {
                let c = fsplat(b, 180.0 / std::f32::consts::PI);
                b.ins().fmul(v, c)
            }),
            M::Ceil => map1!(|b: &mut FunctionBuilder, v| b.ins().ceil(v)),
            M::Floor => map1!(|b: &mut FunctionBuilder, v| b.ins().floor(v)),
            M::Round => map1!(|b: &mut FunctionBuilder, v| b.ins().nearest(v)),
            M::Trunc => map1!(|b: &mut FunctionBuilder, v| b.ins().trunc(v)),
            M::Fract => map1!(vm::fract),
            M::Modf => {
                let w: Vec<Value> = x.iter().map(|&v| b.ins().trunc(v)).collect();
                let mut f: Vec<Value> = x.iter().zip(&w).map(|(&v, &t)| b.ins().fsub(v, t)).collect();
                f.extend(w);
                f
            }
            M::Ldexp => map2!(|b: &mut FunctionBuilder, v, e| {
                let lo = isplat(b, -252);
                let hi = isplat(b, 254);
                let e = b.ins().smax(e, lo);
                let e = b.ins().smin(e, hi);
                let n1 = b.ins().sshr_imm_s(e, 1);
                let n2 = b.ins().isub(e, n1);
                let bias = isplat(b, 127);
                let p1 = b.ins().iadd(n1, bias);
                let p1 = b.ins().ishl_imm_s(p1, 23);
                let p2 = b.ins().iadd(n2, bias);
                let p2 = b.ins().ishl_imm_s(p2, 23);
                let s1 = as_f(b, p1);
                let s2 = as_f(b, p2);
                let t = b.ins().fmul(v, s1);
                b.ins().fmul(t, s2)
            }),
            M::Exp => map1!(vm::exp),
            M::Exp2 => map1!(vm::exp2),
            M::Log => map1!(vm::log),
            M::Log2 => map1!(vm::log2),
            M::Pow => map2!(vm::pow),
            M::Dot if int => {
                let mut s = b.ins().imul(x[0], y[0]);
                for i in 1..x.len() {
                    let t = b.ins().imul(x[i], y[i]);
                    s = b.ins().iadd(s, t);
                }
                vec![s]
            }
            M::Dot => vec![dot(b, &x, &y)],
            M::Outer => {
                let mut out = Vec::new();
                for j in 0..y.len() {
                    for i in 0..x.len() {
                        out.push(b.ins().fmul(x[i], y[j]));
                    }
                }
                out
            }
            M::Cross => {
                let c = |b: &mut FunctionBuilder, i: usize, j: usize| {
                    let p = b.ins().fmul(x[i], y[j]);
                    let q = b.ins().fmul(x[j], y[i]);
                    b.ins().fsub(p, q)
                };
                vec![c(b, 1, 2), c(b, 2, 0), c(b, 0, 1)]
            }
            M::Distance => {
                let d: Vec<Value> = (0..n).map(|i| b.ins().fsub(at(&x, i), at(&y, i))).collect();
                if d.len() == 1 {
                    vec![b.ins().fabs(d[0])]
                } else {
                    let s = dot(b, &d, &d);
                    vec![b.ins().sqrt(s)]
                }
            }
            M::Length => {
                if x.len() == 1 {
                    vec![b.ins().fabs(x[0])]
                } else {
                    let s = dot(b, &x, &x);
                    vec![b.ins().sqrt(s)]
                }
            }
            M::Normalize => {
                if x.len() == 1 {
                    vec![vm::sign(b, x[0])]
                } else {
                    // One divide and n multiplies (≤ 1 ulp from v / l): divides are the slowest SIMD op.
                    let s = dot(b, &x, &x);
                    let l = b.ins().sqrt(s);
                    let one = fsplat(b, 1.0);
                    let r = b.ins().fdiv(one, l);
                    x.iter().map(|&v| b.ins().fmul(v, r)).collect()
                }
            }
            M::FaceForward => {
                // faceforward(N, I, Nref) = dot(Nref, I) < 0 ? N : -N
                let d = dot(b, &z, &y);
                let zero = fsplat(b, 0.0);
                let neg = vm::flt(b, d, zero);
                x.iter()
                    .map(|&v| {
                        let nv = b.ins().fneg(v);
                        sel(b, neg, v, nv)
                    })
                    .collect()
            }
            M::Reflect => {
                // I - 2·dot(N, I)·N
                let d = dot(b, &y, &x);
                let two = fsplat(b, 2.0);
                let d2 = b.ins().fmul(d, two);
                (0..x.len())
                    .map(|i| {
                        let t = b.ins().fmul(d2, y[i]);
                        b.ins().fsub(x[i], t)
                    })
                    .collect()
            }
            M::Refract => {
                let eta = z[0];
                let d = dot(b, &y, &x);
                let one = fsplat(b, 1.0);
                let dd = b.ins().fmul(d, d);
                let omd = b.ins().fsub(one, dd);
                let ee = b.ins().fmul(eta, eta);
                let t = b.ins().fmul(ee, omd);
                let kk = b.ins().fsub(one, t);
                let zero = fsplat(b, 0.0);
                let tir = vm::flt(b, kk, zero);
                let kc = vm::fmax(b, kk, zero);
                let sk = b.ins().sqrt(kc);
                let ed = b.ins().fmul(eta, d);
                let f = b.ins().fadd(ed, sk);
                (0..x.len())
                    .map(|i| {
                        let p = b.ins().fmul(eta, x[i]);
                        let q = b.ins().fmul(f, y[i]);
                        let r = b.ins().fsub(p, q);
                        sel(b, tir, zero, r)
                    })
                    .collect()
            }
            M::Sign if int => map1!(|b: &mut FunctionBuilder, v| {
                let z = isplat(b, 0);
                let one = isplat(b, 1);
                let m1 = isplat(b, -1);
                let pos = b.ins().icmp(IntCC::SignedGreaterThan, v, z);
                let neg = b.ins().icmp(IntCC::SignedLessThan, v, z);
                let r = sel(b, pos, one, z);
                sel(b, neg, m1, r)
            }),
            M::Sign => map1!(vm::sign),
            M::Fma => map3!(|b: &mut FunctionBuilder, p, q, r| {
                let t = b.ins().fmul(p, q);
                b.ins().fadd(t, r)
            }),
            M::Mix => map3!(vm::mix),
            M::Step => map2!(vm::step),
            M::SmoothStep => map3!(vm::smoothstep),
            M::Sqrt => map1!(|b: &mut FunctionBuilder, v| b.ins().sqrt(v)),
            M::InverseSqrt => map1!(|b: &mut FunctionBuilder, v| {
                let s = b.ins().sqrt(v);
                let one = fsplat(b, 1.0);
                b.ins().fdiv(one, s)
            }),
            M::Transpose => {
                let (c, r) = match self.ety(sc, a0) {
                    T::Matrix { columns, rows, .. } => (vsize(*columns), vsize(*rows)),
                    _ => return Err(err("transpose needs a matrix")),
                };
                let mut out = Vec::with_capacity(c * r);
                for i in 0..r {
                    for j in 0..c {
                        out.push(x[j * r + i]);
                    }
                }
                out
            }
            M::Determinant => vec![det(b, &x)],
            M::Inverse => inverse(b, &x),
            M::CountOneBits => map1!(|b: &mut FunctionBuilder, v| {
                // SWAR popcount.
                let m1 = isplat(b, 0x5555_5555);
                let m2 = isplat(b, 0x3333_3333);
                let m4 = isplat(b, 0x0f0f_0f0f);
                let h = isplat(b, 0x0101_0101);
                let t = b.ins().ushr_imm_s(v, 1);
                let t = b.ins().band(t, m1);
                let v = b.ins().isub(v, t);
                let a = b.ins().band(v, m2);
                let c = b.ins().ushr_imm_s(v, 2);
                let c = b.ins().band(c, m2);
                let v = b.ins().iadd(a, c);
                let t = b.ins().ushr_imm_s(v, 4);
                let v = b.ins().iadd(v, t);
                let v = b.ins().band(v, m4);
                let v = b.ins().imul(v, h);
                b.ins().ushr_imm_s(v, 24)
            }),
            other => return Err(err(format!("unsupported built-in function {other:?}"))),
        })
    }

    fn sample(&mut self, sc: Sc, image: Handle<E>, coord: Handle<E>, level: &naga::SampleLevel) -> R<Vec<Value>> {
        let Val::Img(ch) = self.get(sc, image)? else { return Err(err("texture() needs an iChannel")) };
        let uv = self.vals(sc, coord)?;
        let (u, v) = (uv[0], uv[1]);
        let zero = fsplat(&mut self.b, 0.0);
        let (mode, lod, grads) = match level {
            naga::SampleLevel::Zero => (texture::MODE_LOD0, zero, None),
            naga::SampleLevel::Exact(e) => (texture::MODE_LOD, self.vals(sc, *e)?[0], None),
            naga::SampleLevel::Auto => (texture::MODE_AUTO, zero, None),
            naga::SampleLevel::Bias(e) => (texture::MODE_AUTO, self.vals(sc, *e)?[0], None),
            naga::SampleLevel::Gradient { x, y } => {
                let gx = self.vals(sc, *x)?;
                let gy = self.vals(sc, *y)?;
                (texture::MODE_AUTO, zero, Some([gx[0], gx[1], gy[0], gy[1]]))
            }
        };
        // Plain level-0 bilinear/nearest with clamp or repeat runs inline (no
        // call, so no register spills); mipmaps and mirror go through the sampler.
        let fl = MemFlagsData::trusted();
        let base = texture::ENV_CHAN + ch as i32 * texture::CHAN_SIZE;
        let simple = self.b.ins().load(I32, fl, self.env, base + 32);
        let out: Vec<Variable> = (0..4).map(|_| self.b.declare_var(F32X4)).collect();
        let inl = self.b.create_block();
        let slow = self.b.create_block();
        let merge = self.b.create_block();
        self.b.ins().brif(simple, inl, &[], slow, &[]);
        self.b.switch_to_block(inl);
        let r = self.sample_inline(base, u, v);
        for (var, val) in out.iter().zip(r) {
            self.b.def_var(*var, val);
        }
        self.b.ins().jump(merge, &[]);
        self.b.switch_to_block(slow);
        let grads = match grads {
            Some(g) => g,
            None if mode == texture::MODE_AUTO => {
                let ax = naga::DerivativeAxis::X;
                let ay = naga::DerivativeAxis::Y;
                [self.deriv(ax, u), self.deriv(ax, v), self.deriv(ay, u), self.deriv(ay, v)]
            }
            None => [zero; 4],
        };
        let addr = self.b.ins().stack_addr(self.ptr_ty, self.scratch, 0);
        for (i, val) in [u, v, lod, grads[0], grads[1], grads[2], grads[3]].into_iter().enumerate() {
            self.b.ins().store(fl, val, addr, (i * 16) as i32);
        }
        let chv = self.b.ins().iconst(I32, ch as i64);
        let mv = self.b.ins().iconst(I32, mode as i64);
        self.b.ins().call(self.sample_ref, &[self.env, chv, addr, mv]);
        for (i, var) in out.iter().enumerate() {
            let x = self.b.ins().load(F32X4, fl, addr, i as i32 * 16);
            self.b.def_var(*var, x);
        }
        self.b.ins().jump(merge, &[]);
        self.b.switch_to_block(merge);
        Ok(out.iter().map(|v| self.b.use_var(*v)).collect())
    }

    /// Texel index (0..n-1) along one axis: clamp, or repeat when `rep` is set.
    fn wrap_axis(&mut self, f: Value, nf: Value, n: Value, rep: Value) -> Value {
        // Repeat in float (exact below 2^24), then clamp as a backstop for NaN/huge.
        let q = self.b.ins().fdiv(f, nf);
        let q = self.b.ins().floor(q);
        let m = self.b.ins().fmul(q, nf);
        let r = self.b.ins().fsub(f, m);
        let f = sel(&mut self.b, rep, r, f);
        let i = self.b.ins().fcvt_to_sint_sat(I32X4, f);
        let z = isplat(&mut self.b, 0);
        let one = isplat(&mut self.b, 1);
        let hi = self.b.ins().isub(n, one);
        let i = self.b.ins().smax(i, z);
        self.b.ins().smin(i, hi)
    }

    /// Four RGBA8 texels at byte offsets `off` from `data`, as packed lanes.
    fn gather(&mut self, data: Value, off: Value) -> Value {
        let fl = MemFlagsData::trusted();
        let mut out = isplat(&mut self.b, 0);
        for l in 0..4u8 {
            let o = self.b.ins().extractlane(off, l);
            let o = self.b.ins().uextend(I64, o);
            let a = self.b.ins().iadd(data, o);
            let x = self.b.ins().load(I32, fl, a, 0);
            out = self.b.ins().insertlane(out, x, l);
        }
        out
    }

    fn unpack(&mut self, px: Value) -> [Value; 4] {
        let k = fsplat(&mut self.b, 1.0 / 255.0);
        let m = isplat(&mut self.b, 0xff);
        std::array::from_fn(|c| {
            let s = if c == 0 { px } else { self.b.ins().ushr_imm_s(px, 8 * c as i64) };
            let s = if c == 3 { s } else { self.b.ins().band(s, m) };
            let f = self.b.ins().fcvt_from_sint(F32X4, s);
            self.b.ins().fmul(f, k)
        })
    }

    fn sample_inline(&mut self, base: i32, u: Value, v: Value) -> Vec<Value> {
        let fl = MemFlagsData::trusted();
        let env = self.env;
        let data = self.b.ins().load(self.ptr_ty, fl, env, base);
        let ld_i = |s: &mut Self, o: i32| {
            let x = s.b.ins().load(I32, fl, env, base + o);
            s.b.ins().splat(I32X4, x)
        };
        let ld_f = |s: &mut Self, o: i32| {
            let x = s.b.ins().load(F32, fl, env, base + o);
            s.b.ins().splat(F32X4, x)
        };
        let (w, h, wf, hf) = (ld_i(self, 8), ld_i(self, 12), ld_f(self, 16), ld_f(self, 20));
        let zero_i = isplat(&mut self.b, 0);
        let rep = ld_i(self, 24);
        let rep = self.b.ins().icmp(IntCC::NotEqual, rep, zero_i);
        let near = ld_i(self, 28);
        let near = self.b.ins().icmp(IntCC::NotEqual, near, zero_i);
        // Same arithmetic as `Texture::sample_level`.
        let one = fsplat(&mut self.b, 1.0);
        let half = fsplat(&mut self.b, 0.5);
        let zero = fsplat(&mut self.b, 0.0);
        let x = self.b.ins().fmul(u, wf);
        let iv = self.b.ins().fsub(one, v);
        let y = self.b.ins().fmul(iv, hf);
        let hoff = sel(&mut self.b, near, zero, half);
        let x = self.b.ins().fsub(x, hoff);
        let y = self.b.ins().fsub(y, hoff);
        let fx = self.b.ins().floor(x);
        let fy = self.b.ins().floor(y);
        let tx = self.b.ins().fsub(x, fx);
        let ty = self.b.ins().fsub(y, fy);
        let tx = sel(&mut self.b, near, zero, tx);
        let ty = sel(&mut self.b, near, zero, ty);
        let fx1 = self.b.ins().fadd(fx, one);
        let fy1 = self.b.ins().fadd(fy, one);
        let xa = self.wrap_axis(fx, wf, w, rep);
        let xb = self.wrap_axis(fx1, wf, w, rep);
        let ya = self.wrap_axis(fy, hf, h, rep);
        let yb = self.wrap_axis(fy1, hf, h, rep);
        let ra = self.b.ins().imul(ya, w);
        let rb = self.b.ins().imul(yb, w);
        let mut corners = Vec::with_capacity(4);
        for (r, c) in [(ra, xa), (ra, xb), (rb, xa), (rb, xb)] {
            let i = self.b.ins().iadd(r, c);
            let off = self.b.ins().ishl_imm_s(i, 2);
            let g = self.gather(data, off);
            corners.push(self.unpack(g));
        }
        (0..4)
            .map(|k| {
                let (p00, p10, p01, p11) = (corners[0][k], corners[1][k], corners[2][k], corners[3][k]);
                let d = self.b.ins().fsub(p10, p00);
                let t = self.b.ins().fmul(d, tx);
                let top = self.b.ins().fadd(p00, t);
                let d = self.b.ins().fsub(p11, p01);
                let t = self.b.ins().fmul(d, tx);
                let bot = self.b.ins().fadd(p01, t);
                let d = self.b.ins().fsub(bot, top);
                let t = self.b.ins().fmul(d, ty);
                self.b.ins().fadd(top, t)
            })
            .collect()
    }

    fn fetch(&mut self, sc: Sc, image: Handle<E>, coord: Handle<E>, level: Option<Handle<E>>) -> R<Vec<Value>> {
        let Val::Img(ch) = self.get(sc, image)? else { return Err(err("texelFetch() needs an iChannel")) };
        let xy = self.vals(sc, coord)?;
        let lod = match level {
            Some(l) => self.vals(sc, l)?[0],
            None => isplat(&mut self.b, 0),
        };
        let addr = self.b.ins().stack_addr(self.ptr_ty, self.scratch, 0);
        let fl = MemFlagsData::trusted();
        for (i, val) in [xy[0], xy[1], lod].into_iter().enumerate() {
            self.b.ins().store(fl, val, addr, (i * 16) as i32);
        }
        let chv = self.b.ins().iconst(I32, ch as i64);
        self.b.ins().call(self.fetch_ref, &[self.env, chv, addr]);
        Ok((0..4).map(|i| self.b.ins().load(F32X4, fl, addr, i * 16)).collect())
    }

    // ---------- statements ----------

    /// Lanes still alive after a structured statement that started with `before`.
    fn after_mask(&mut self, before: Value) -> Value {
        let fr = self.frames.last().unwrap();
        let ret = fr.ret;
        let mut parts: Vec<(Variable, bool)> = Vec::new(); // (var, negate)
        for c in fr.ctl.iter().rev() {
            match c {
                Ctl::Switch { brk } => parts.push((*brk, true)),
                Ctl::Loop { mask, cnt } => {
                    parts.push((*mask, false));
                    parts.push((*cnt, true));
                    break;
                }
            }
        }
        let r = self.b.use_var(ret);
        let mut m = self.b.ins().band_not(before, r);
        for (v, neg) in parts {
            let x = self.b.use_var(v);
            m = if neg { self.b.ins().band_not(m, x) } else { self.b.ins().band(m, x) };
        }
        m
    }

    fn lower_block(&mut self, stmts: &'m [S]) -> R<()> {
        for (i, st) in stmts.iter().enumerate() {
            match self.unroll_plan(stmts, i) {
                Some(plan) => self.unroll(plan)?,
                None => self.stmt(st)?,
            }
            if matches!(st, S::Break | S::Continue | S::Return { .. } | S::Kill) {
                return Ok(()); // the rest of the block is unreachable
            }
            if i + 1 < stmts.len() && may_exit(st) {
                // Skip the rest of the block when no lane is left.
                let rest = self.b.create_block();
                let end = self.b.create_block();
                let e = self.exec();
                let a = any(&mut self.b, e);
                self.b.ins().brif(a, rest, &[], end, &[]);
                self.b.switch_to_block(rest);
                self.lower_block(&stmts[i + 1..])?;
                self.b.ins().jump(end, &[]);
                self.b.switch_to_block(end);
                return Ok(());
            }
        }
        Ok(())
    }

    /// A counted loop `for (int i = c0; i OP n; i += k)` with a short constant
    /// trip count and no way out but its guard can be emitted as straight-line
    /// copies with `i` a constant in each. Then everything that depends only on
    /// uniforms and `i` (the `sin(iTime + float(i))` of a Shadertoy scene) is
    /// loop-invariant and Cranelift hoists it out of the pixel loop. Same
    /// arithmetic, so the output is bit-identical.
    fn unroll_plan(&self, stmts: &'m [S], at: usize) -> Option<Unroll<'m>> {
        const MAX_TRIPS: usize = 16;
        const MAX_COPY: usize = 4000; // expressions per unrolled loop
        const MAX_TOTAL: usize = 60000; // per kernel
        let S::Loop { body, continuing, break_if: None } = &stmts[at] else { return None };
        let fr = self.frames.last()?;
        let f = fr.f;
        let local = |h: Handle<E>| match f.expressions[h] {
            E::LocalVariable(l) => Some(l),
            _ => None,
        };
        let int_lit = |h: Handle<E>| -> Option<i64> {
            match f.expressions[h] {
                E::Literal(naga::Literal::I32(v)) => Some(v as i64),
                E::Literal(naga::Literal::U32(v)) => Some(v as i64),
                E::Literal(naga::Literal::AbstractInt(v)) | E::Literal(naga::Literal::I64(v)) => Some(v),
                E::Constant(c) => match self.m.global_expressions[self.m.constants[c].init] {
                    naga::Expression::Literal(naga::Literal::I32(v)) => Some(v as i64),
                    naga::Expression::Literal(naga::Literal::U32(v)) => Some(v as i64),
                    _ => None,
                },
                _ => None,
            }
        };
        // Guard: Emits, then `if (!(i OP n)) break;` (or `if (i OP n) {} else break;`).
        let g = body.iter().position(|st| !matches!(st, S::Emit(_)))?;
        let S::If { condition, accept, reject } = &body[g] else { return None };
        let is_break = |b: &naga::Block| b.len() == 1 && matches!(b[0], S::Break);
        let cmp = if is_break(accept) && reject.is_empty() {
            match f.expressions[*condition] {
                E::Unary { op: naga::UnaryOperator::LogicalNot, expr } => expr,
                _ => return None,
            }
        } else if accept.is_empty() && is_break(reject) {
            *condition
        } else {
            return None;
        };
        let E::Binary { op, left, right } = f.expressions[cmp] else { return None };
        let E::Load { pointer } = f.expressions[left] else { return None };
        let lv = local(pointer)?;
        if !matches!(f.local_variables[lv].ty, ty if matches!(self.m.types[ty].inner, naga::TypeInner::Scalar(naga::Scalar { kind: naga::ScalarKind::Sint | naga::ScalarKind::Uint, width: 4 }))) {
            return None;
        }
        let bound = int_lit(right)?;
        // Continuing: only `i = i ± k`.
        let mut step = None;
        for st in continuing.iter() {
            match st {
                S::Emit(_) => {}
                S::Store { pointer, value } if local(*pointer) == Some(lv) && step.is_none() => {
                    let E::Binary { op: sop, left: l2, right: r2 } = f.expressions[*value] else { return None };
                    let E::Load { pointer: p2 } = f.expressions[l2] else { return None };
                    if local(p2) != Some(lv) {
                        return None;
                    }
                    let k = int_lit(r2)?;
                    step = Some(match sop {
                        naga::BinaryOperator::Add => k,
                        naga::BinaryOperator::Subtract => -k,
                        _ => return None,
                    });
                }
                _ => return None,
            }
        }
        let step = step.filter(|k| *k != 0)?;
        // The counter's value on entry: a constant store just before the loop, or
        // the declaration's initialiser for a loop that runs once per call.
        let stores_lv = |st: &S| matches!(st, S::Store { pointer, .. } if local(*pointer) == Some(lv))
            || matches!(st, S::Call { arguments, .. } if arguments.iter().any(|a| local(*a) == Some(lv)));
        let prev = stmts[..at].iter().rev().find(|st| !matches!(st, S::Emit(_)));
        let start = match prev {
            Some(S::Store { pointer, value }) if local(*pointer) == Some(lv) => int_lit(*value)?,
            _ => {
                if fr.loops > 0 || any_stmt(&f.body, &|st| stores_lv(st) && !continuing.iter().any(|c| std::ptr::eq(c, st))) {
                    return None;
                }
                int_lit(f.local_variables[lv].init?)?
            }
        };
        // The body (after the guard) may not leave early or touch the counter.
        let rest = &body[g + 1..];
        if contains_ret(rest) || any_stmt(rest, &|st| matches!(st, S::Kill) || stores_lv(st)) || leaves_loop(rest) {
            return None;
        }
        // Trip count.
        let test = |v: i64| match op {
            naga::BinaryOperator::Less => v < bound,
            naga::BinaryOperator::LessEqual => v <= bound,
            naga::BinaryOperator::Greater => v > bound,
            naga::BinaryOperator::GreaterEqual => v >= bound,
            naga::BinaryOperator::NotEqual => v != bound,
            _ => false,
        };
        if !matches!(op, naga::BinaryOperator::Less | naga::BinaryOperator::LessEqual | naga::BinaryOperator::Greater | naga::BinaryOperator::GreaterEqual | naga::BinaryOperator::NotEqual) {
            return None;
        }
        let mut vals = Vec::new();
        let mut v = start;
        while test(v) {
            if vals.len() >= MAX_TRIPS {
                return None;
            }
            vals.push(v);
            v += step;
        }
        let weight = block_weight(self.m, rest) + block_weight(self.m, &body[..g]);
        if weight * vals.len() > MAX_COPY || self.unrolled + weight * vals.len() > MAX_TOTAL {
            return None;
        }
        Some(Unroll { lv, guard: &body[..g], rest, vals, end: v, weight })
    }

    fn unroll(&mut self, u: Unroll<'m>) -> R<()> {
        let root = self.frames.last().unwrap().locals[u.lv.index()];
        let var = self.roots[root][0];
        let entry = self.mask_for(self.root_owner[root]);
        let old = self.b.use_var(var);
        self.unrolled += u.weight * u.vals.len();
        self.frames.last_mut().unwrap().loops += 1;
        for &v in &u.vals {
            // Every lane sees the constant; lanes not running here don't read it.
            let c = isplat(&mut self.b, v as i32);
            self.b.def_var(var, c);
            self.lower_block(u.guard)?;
            self.lower_block(u.rest)?;
        }
        self.frames.last_mut().unwrap().loops -= 1;
        // After the loop the counter holds its exit value, in the lanes that ran it.
        let e = isplat(&mut self.b, u.end as i32);
        let fin = match entry {
            None => e,
            Some(m) => sel(&mut self.b, m, e, old),
        };
        self.b.def_var(var, fin);
        Ok(())
    }

    fn stmt(&mut self, st: &'m S) -> R<()> {
        match st {
            S::Emit(range) => {
                for h in range.clone() {
                    let v = self.lower(Sc::Top, h)?;
                    self.frames.last_mut().unwrap().ex[h.index()] = Some(v);
                }
            }
            S::Block(b) => self.lower_block(b)?,
            S::If { condition, accept, reject } => {
                let c = self.vals(Sc::Top, *condition)?[0];
                let e = self.exec();
                let full = self.full;
                let mt = self.b.ins().band(e, c);
                let me = self.b.ins().band_not(e, c);
                let merge = self.b.create_block();
                if !accept.is_empty() {
                    let tb = self.b.create_block();
                    let next = self.b.create_block();
                    let a = any(&mut self.b, mt);
                    self.b.ins().brif(a, tb, &[], next, &[]);
                    self.b.switch_to_block(tb);
                    self.set_exec(mt);
                    self.full = false;
                    self.lower_block(accept)?;
                    self.b.ins().jump(next, &[]);
                    self.b.switch_to_block(next);
                }
                if !reject.is_empty() {
                    let eb = self.b.create_block();
                    let a = any(&mut self.b, me);
                    self.b.ins().brif(a, eb, &[], merge, &[]);
                    self.b.switch_to_block(eb);
                    self.set_exec(me);
                    self.full = false;
                    self.lower_block(reject)?;
                }
                self.b.ins().jump(merge, &[]);
                self.b.switch_to_block(merge);
                if may_exit(st) {
                    let m = self.after_mask(e);
                    self.set_exec(m);
                    self.full = false;
                } else {
                    self.set_exec(e);
                    self.full = full;
                }
            }
            S::Loop { body, continuing, break_if } => {
                let e = self.exec();
                let full = self.full;
                let mask = self.b.declare_var(I32X4);
                let cnt = self.b.declare_var(I32X4);
                let iters = self.b.declare_var(I32);
                self.b.def_var(mask, e);
                let z = isplat(&mut self.b, 0);
                self.b.def_var(cnt, z);
                let z0 = self.b.ins().iconst(I32, 0);
                self.b.def_var(iters, z0);
                let hdr = self.b.create_block();
                let body_b = self.b.create_block();
                let capped = self.b.create_block();
                let exit = self.b.create_block();
                self.b.ins().jump(hdr, &[]);
                self.b.switch_to_block(hdr);
                let z = isplat(&mut self.b, 0);
                self.b.def_var(cnt, z);
                let lm = self.b.use_var(mask);
                let ret = self.frames.last().unwrap().ret;
                let r = self.b.use_var(ret);
                let ex = self.b.ins().band_not(lm, r);
                self.set_exec(ex);
                self.full = false;
                let a = any(&mut self.b, ex);
                self.b.ins().brif(a, capped, &[], exit, &[]);
                self.b.switch_to_block(capped);
                let it = self.b.use_var(iters);
                let it1 = self.b.ins().iadd_imm_s(it, 1);
                self.b.def_var(iters, it1);
                let ok = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, it1, LOOP_CAP);
                self.b.ins().brif(ok, body_b, &[], exit, &[]);
                self.b.switch_to_block(body_b);
                self.frames.last_mut().unwrap().ctl.push(Ctl::Loop { mask, cnt });
                self.frames.last_mut().unwrap().loops += 1;
                self.lower_block(body)?;
                // Continuing: every lane that didn't break or return.
                let lm = self.b.use_var(mask);
                let r = self.b.use_var(ret);
                let ex = self.b.ins().band_not(lm, r);
                self.set_exec(ex);
                self.lower_block(continuing)?;
                if let Some(c) = break_if {
                    let c = self.vals(Sc::Top, *c)?[0];
                    let ex = self.exec();
                    let brk = self.b.ins().band(ex, c);
                    let lm = self.b.use_var(mask);
                    let lm = self.b.ins().band_not(lm, brk);
                    self.b.def_var(mask, lm);
                }
                self.frames.last_mut().unwrap().ctl.pop();
                self.frames.last_mut().unwrap().loops -= 1;
                self.b.ins().jump(hdr, &[]);
                self.b.switch_to_block(exit);
                let r = self.b.use_var(ret);
                let m = self.b.ins().band_not(e, r);
                if self.has_kill {
                    let k = self.b.use_var(self.killed);
                    let m2 = self.b.ins().band_not(m, k);
                    self.set_exec(m2);
                } else {
                    self.set_exec(m);
                }
                self.full = full && !contains_ret(body) && !contains_ret(continuing);
                if !self.full && full {
                    // Return inside the loop: lanes may be gone.
                }
            }
            S::Switch { selector, cases } => {
                let s = self.vals(Sc::Top, *selector)?[0];
                let e = self.exec();
                let full = self.full;
                let brk = self.b.declare_var(I32X4);
                let z = isplat(&mut self.b, 0);
                self.b.def_var(brk, z);
                let fall = self.b.declare_var(I32X4);
                self.b.def_var(fall, z);
                let mut matched = z;
                let mut masks = Vec::new();
                for c in cases {
                    let m = match c.value {
                        naga::SwitchValue::I32(v) => Some(v),
                        naga::SwitchValue::U32(v) => Some(v as i32),
                        naga::SwitchValue::Default => None,
                    };
                    let mv = m.map(|v| {
                        let kv = isplat(&mut self.b, v);
                        let mm = self.b.ins().icmp(IntCC::Equal, s, kv);
                        matched = self.b.ins().bor(matched, mm);
                        mm
                    });
                    masks.push(mv);
                }
                let not_matched = self.b.ins().bnot(matched);
                self.frames.last_mut().unwrap().ctl.push(Ctl::Switch { brk });
                for (c, m) in cases.iter().zip(masks) {
                    let m = m.unwrap_or(not_matched);
                    let em = self.b.ins().band(e, m);
                    let f = self.b.use_var(fall);
                    let entry = self.b.ins().bor(em, f);
                    let body = self.b.create_block();
                    let next = self.b.create_block();
                    let zero = isplat(&mut self.b, 0);
                    self.b.def_var(fall, zero);
                    let a = any(&mut self.b, entry);
                    self.b.ins().brif(a, body, &[], next, &[]);
                    self.b.switch_to_block(body);
                    self.set_exec(entry);
                    self.full = false;
                    self.lower_block(&c.body)?;
                    if c.fall_through {
                        let m = self.after_mask(entry);
                        self.b.def_var(fall, m);
                    }
                    self.b.ins().jump(next, &[]);
                    self.b.switch_to_block(next);
                }
                self.frames.last_mut().unwrap().ctl.pop();
                let m = self.after_mask(e);
                self.set_exec(m);
                self.full = full && !may_exit(st);
                if full && !self.full {
                    // keep masked
                }
            }
            S::Break => {
                let e = self.exec();
                let fr = self.frames.last().unwrap();
                match fr.ctl.last() {
                    Some(Ctl::Loop { mask, .. }) => {
                        let mask = *mask;
                        let lm = self.b.use_var(mask);
                        let lm = self.b.ins().band_not(lm, e);
                        self.b.def_var(mask, lm);
                    }
                    Some(Ctl::Switch { brk }) => {
                        let brk = *brk;
                        let bm = self.b.use_var(brk);
                        let bm = self.b.ins().bor(bm, e);
                        self.b.def_var(brk, bm);
                    }
                    None => return Err(err("`break` outside a loop or switch")),
                }
                let z = isplat(&mut self.b, 0);
                self.set_exec(z);
            }
            S::Continue => {
                let e = self.exec();
                let cnt = self.frames.last().unwrap().ctl.iter().rev().find_map(|c| match c {
                    Ctl::Loop { cnt, .. } => Some(*cnt),
                    _ => None,
                });
                let Some(cnt) = cnt else { return Err(err("`continue` outside a loop")) };
                let c = self.b.use_var(cnt);
                let c = self.b.ins().bor(c, e);
                self.b.def_var(cnt, c);
                let z = isplat(&mut self.b, 0);
                self.set_exec(z);
            }
            S::Return { value } => {
                if let Some(v) = value {
                    let vals = self.vals(Sc::Top, *v)?;
                    let mask = self.mask_for(self.frames.len());
                    let rv = self.frames.last().unwrap().ret_vars.clone();
                    for (var, val) in rv.into_iter().zip(vals) {
                        self.write(var, val, mask);
                    }
                }
                let e = self.exec();
                let ret = self.frames.last().unwrap().ret;
                let r = self.b.use_var(ret);
                let r = self.b.ins().bor(r, e);
                self.b.def_var(ret, r);
                let z = isplat(&mut self.b, 0);
                self.set_exec(z);
            }
            S::Kill => {
                let e = self.exec();
                let k = self.b.use_var(self.killed);
                let k = self.b.ins().bor(k, e);
                self.b.def_var(self.killed, k);
                let rets: Vec<Variable> = self.frames.iter().map(|f| f.ret).collect();
                for ret in rets {
                    let r = self.b.use_var(ret);
                    let r = self.b.ins().bor(r, e);
                    self.b.def_var(ret, r);
                }
                let z = isplat(&mut self.b, 0);
                self.set_exec(z);
            }
            S::Store { pointer, value } => {
                let p = match self.get(Sc::Top, *pointer)? {
                    Val::P(p) => p,
                    _ => return Err(err("store to a non-variable")),
                };
                let v = self.vals(Sc::Top, *value)?;
                self.store(&p, &v)?;
            }
            S::Call { function, arguments, result } => {
                let mut args = Vec::with_capacity(arguments.len());
                for a in arguments {
                    args.push(self.get(Sc::Top, *a)?);
                }
                let e = self.exec();
                let full = self.full;
                let r = self.inline(*function, args)?;
                if self.has_kill {
                    let k = self.b.use_var(self.killed);
                    let m = self.b.ins().band_not(e, k);
                    self.set_exec(m);
                    self.full = false;
                } else {
                    self.set_exec(e);
                    self.full = full;
                }
                if let Some(h) = result {
                    self.frames.last_mut().unwrap().ex[h.index()] = Some(Val::V(r));
                }
            }
            S::ControlBarrier(_) | S::MemoryBarrier(_) => {}
            other => return Err(err(format!("unsupported statement {:?}", std::mem::discriminant(other)))),
        }
        Ok(())
    }

    /// Inline `fh` with `args`; returns the result slots.
    fn inline(&mut self, fh: Handle<naga::Function>, args: Vec<Val>) -> R<Vec<Value>> {
        let f = &self.m.functions[fh];
        let fi = &self.info[fh];
        self.depth += 1;
        if self.depth > 64 {
            return Err(err("functions nest too deeply (recursion is not allowed in GLSL)"));
        }
        let ret = self.b.declare_var(I32X4);
        let z = isplat(&mut self.b, 0);
        self.b.def_var(ret, z);
        let ret_kinds = match &f.result {
            Some(r) => self.kinds(&self.m.types[r.ty].inner)?,
            None => Vec::new(),
        };
        let mut ret_vars = Vec::new();
        for k in &ret_kinds {
            let v = self.b.declare_var(k.ty());
            let z = self.zero(*k);
            self.b.def_var(v, z);
            ret_vars.push(v);
        }
        let entry_full = self.full;
        self.frames.push(Frame { f, fi, args, ex: vec![None; f.expressions.len()], locals: Vec::new(), ret, ret_vars, ret_kinds, ctl: Vec::new(), entry_full, loops: 0 });
        // Inside the callee, "full" means "the lanes it was called with".
        self.full = true;
        // Expressions that need no Emit (literals, constants, arguments,
        // variables) are lowered here, where they dominate the whole body.
        let emitted = emitted_set(&f.body, f.expressions.len());
        for (h, e) in f.expressions.iter() {
            if emitted[h.index()] || matches!(e, E::CallResult(_) | E::LocalVariable(_)) {
                continue;
            }
            if let Ok(v) = self.lower(Sc::Top, h) {
                self.frames.last_mut().unwrap().ex[h.index()] = Some(v);
            }
        }
        let mut locals = Vec::with_capacity(f.local_variables.len());
        for (_, lv) in f.local_variables.iter() {
            let ks = self.kinds(&self.m.types[lv.ty].inner)?;
            let root = self.new_root(ks.clone());
            let init = match lv.init {
                Some(h) => Some(self.vals(Sc::Top, h)?),
                None => None,
            };
            for (i, k) in ks.iter().enumerate() {
                let v = match &init {
                    Some(iv) => iv[i.min(iv.len() - 1)],
                    None => self.zero(*k),
                };
                let var = self.roots[root][i];
                self.b.def_var(var, v);
            }
            locals.push(root);
        }
        self.frames.last_mut().unwrap().locals = locals;
        for (h, e) in f.expressions.iter() {
            if !emitted[h.index()] && matches!(e, E::LocalVariable(_)) {
                let v = self.lower(Sc::Top, h)?;
                self.frames.last_mut().unwrap().ex[h.index()] = Some(v);
            }
        }
        self.lower_block(&f.body)?;
        let fr = self.frames.pop().unwrap();
        let _ = &fr.ret_kinds;
        self.depth -= 1;
        Ok(fr.ret_vars.iter().map(|v| self.b.use_var(*v)).collect())
    }
}

/// Every (static slot offset, per-index value) combination of dynamic indices.
fn combos(dynx: &[(Value, usize, usize)]) -> Vec<(usize, Vec<usize>)> {
    let mut out = vec![(0usize, Vec::new())];
    for &(_, stride, count) in dynx {
        let mut next = Vec::with_capacity(out.len() * count);
        for (off, ks) in &out {
            for k in 0..count {
                let mut ks2 = ks.clone();
                ks2.push(k);
                next.push((off + k * stride, ks2));
            }
        }
        out = next;
    }
    out
}

fn emitted_set(body: &naga::Block, n: usize) -> Vec<bool> {
    fn walk(b: &naga::Block, set: &mut Vec<bool>) {
        for st in b.iter() {
            match st {
                S::Emit(r) => {
                    for h in r.clone() {
                        set[h.index()] = true;
                    }
                }
                S::Block(b) => walk(b, set),
                S::If { accept, reject, .. } => {
                    walk(accept, set);
                    walk(reject, set);
                }
                S::Switch { cases, .. } => {
                    for c in cases {
                        walk(&c.body, set);
                    }
                }
                S::Loop { body, continuing, .. } => {
                    walk(body, set);
                    walk(continuing, set);
                }
                _ => {}
            }
        }
    }
    let mut set = vec![false; n];
    walk(body, &mut set);
    set
}

fn any_stmt(b: &[S], f: &dyn Fn(&S) -> bool) -> bool {
    b.iter().any(|st| {
        f(st)
            || match st {
                S::Block(b) => any_stmt(b, f),
                S::If { accept, reject, .. } => any_stmt(accept, f) || any_stmt(reject, f),
                S::Switch { cases, .. } => cases.iter().any(|c| any_stmt(&c.body, f)),
                S::Loop { body, continuing, .. } => any_stmt(body, f) || any_stmt(continuing, f),
                _ => false,
            }
    })
}

/// `v` is `splat(f32const k)`: k.
fn const_splat_f32(b: &FunctionBuilder, v: Value) -> Option<f32> {
    use cranelift_codegen::ir::{InstructionData, Opcode, ValueDef};
    let dfg = &b.func.dfg;
    let ValueDef::Result(inst, _) = dfg.value_def(v) else { return None };
    let InstructionData::Unary { opcode: Opcode::Splat, arg } = dfg.insts[inst] else { return None };
    let ValueDef::Result(ci, _) = dfg.value_def(arg) else { return None };
    match dfg.insts[ci] {
        InstructionData::UnaryIeee32 { opcode: Opcode::F32const, imm } => Some(f32::from_bits(imm.bits())),
        _ => None,
    }
}

/// A loop body to emit as straight-line copies (see `Lower::unroll_plan`).
struct Unroll<'m> {
    lv: Handle<naga::LocalVariable>,
    guard: &'m [S],
    rest: &'m [S],
    vals: Vec<i64>,
    end: i64,
    weight: usize,
}

/// Does a loop body `break` or `continue` its own loop (not a nested one)?
fn leaves_loop(b: &[S]) -> bool {
    b.iter().any(|st| match st {
        S::Break | S::Continue => true,
        S::Block(b) => leaves_loop(b),
        S::If { accept, reject, .. } => leaves_loop(accept) || leaves_loop(reject),
        // `break` in a switch is the switch's; `continue` is the loop's.
        S::Switch { cases, .. } => cases.iter().any(|c| continues(&c.body)),
        _ => false,
    })
}

/// A `continue` outside any nested loop.
fn continues(b: &[S]) -> bool {
    b.iter().any(|st| match st {
        S::Continue => true,
        S::Block(b) => continues(b),
        S::If { accept, reject, .. } => continues(accept) || continues(reject),
        S::Switch { cases, .. } => cases.iter().any(|c| continues(&c.body)),
        _ => false,
    })
}

/// Rough size of a block once lowered: emitted expressions, with calls
/// counted as their callee's (they are inlined).
fn block_weight(m: &naga::Module, b: &[S]) -> usize {
    b.iter()
        .map(|st| match st {
            S::Emit(r) => r.clone().count(),
            S::Block(b) => block_weight(m, b),
            S::If { accept, reject, .. } => 1 + block_weight(m, accept) + block_weight(m, reject),
            S::Switch { cases, .. } => cases.iter().map(|c| block_weight(m, &c.body)).sum(),
            S::Loop { body, continuing, .. } => 8 + block_weight(m, body) + block_weight(m, continuing),
            S::Call { function, .. } => 4 + m.functions[*function].expressions.len(),
            _ => 1,
        })
        .sum()
}

/// Can this statement turn lanes off (break, continue, return, discard)?
fn may_exit(st: &S) -> bool {
    let f = |s: &S| matches!(s, S::Break | S::Continue | S::Return { .. } | S::Kill);
    match st {
        S::Loop { body, continuing, .. } => contains_ret(body) || contains_ret(continuing),
        S::Block(b) => any_stmt(b, &f),
        S::If { accept, reject, .. } => any_stmt(accept, &f) || any_stmt(reject, &f),
        S::Switch { cases, .. } => cases.iter().any(|c| any_stmt(&c.body, &f)),
        S::Call { .. } => false,
        other => f(other),
    }
}

fn contains_ret(b: &[S]) -> bool {
    any_stmt(b, &|s| matches!(s, S::Return { .. } | S::Kill))
}

fn det(b: &mut FunctionBuilder, m: &[Value]) -> Value {
    let n = (m.len() as f64).sqrt() as usize;
    let at = |c: usize, r: usize| m[c * n + r];
    match n {
        2 => {
            let p = b.ins().fmul(at(0, 0), at(1, 1));
            let q = b.ins().fmul(at(1, 0), at(0, 1));
            b.ins().fsub(p, q)
        }
        _ => {
            // Laplace expansion along the first column.
            let mut acc: Option<Value> = None;
            for r in 0..n {
                let mut sub = Vec::with_capacity((n - 1) * (n - 1));
                for c in 1..n {
                    for rr in 0..n {
                        if rr != r {
                            sub.push(at(c, rr));
                        }
                    }
                }
                let d = det(b, &sub);
                let t = b.ins().fmul(at(0, r), d);
                acc = Some(match acc {
                    None => t,
                    Some(a) if r % 2 == 1 => b.ins().fsub(a, t),
                    Some(a) => b.ins().fadd(a, t),
                });
            }
            acc.unwrap()
        }
    }
}

fn inverse(b: &mut FunctionBuilder, m: &[Value]) -> Vec<Value> {
    let n = (m.len() as f64).sqrt() as usize;
    let d = det(b, m);
    let one = fsplat(b, 1.0);
    let id = b.ins().fdiv(one, d);
    let mut out = vec![one; n * n];
    for c in 0..n {
        for r in 0..n {
            // inverse[c][r] = cofactor(r, c) / det  (adjugate = transposed cofactors)
            let mut sub = Vec::with_capacity((n - 1) * (n - 1));
            for cc in 0..n {
                if cc == r {
                    continue;
                }
                for rr in 0..n {
                    if rr != c {
                        sub.push(m[cc * n + rr]);
                    }
                }
            }
            let cof = if n == 2 { sub[0] } else { det(b, &sub) };
            let cof = if (r + c) % 2 == 1 { b.ins().fneg(cof) } else { cof };
            out[c * n + r] = b.ins().fmul(cof, id);
        }
    }
    out
}

/// Where each named uniform lives in the uniform buffer (in 4-byte slots).
pub type Layout = HashMap<String, u32>;

/// Compile the shader's entry into a kernel.
pub fn compile(m: &naga::Module, info: &ModuleInfo, entry: Handle<naga::Function>, kind: Kind, layout: &Layout) -> R<Kernel> {
    let mut flags = settings::builder();
    flags.set("opt_level", "speed").unwrap();
    flags.set("enable_verifier", if cfg!(debug_assertions) { "true" } else { "false" }).unwrap();
    let isa_builder = cranelift_native::builder().map_err(|e| err(format!("no JIT for this CPU: {e}")))?;
    let isa = isa_builder.finish(settings::Flags::new(flags)).map_err(|e| err(format!("JIT setup: {e}")))?;
    let mut jb = JITBuilder::with_isa(isa, default_libcall_names());
    jb.symbol("ak_sample", texture::ak_sample as *const u8);
    jb.symbol("ak_fetch", texture::ak_fetch as *const u8);
    let mut module = JITModule::new(jb);
    let ptr_ty = module.target_config().pointer_type();

    let mut sig = module.make_signature();
    for t in [ptr_ty, ptr_ty, F32, F32, I32, ptr_ty] {
        sig.params.push(AbiParam::new(t));
    }
    let fid = module.declare_function("ak_kernel", Linkage::Export, &sig).map_err(|e| err(e.to_string()))?;
    let mut ssig = module.make_signature();
    for t in [ptr_ty, I32, ptr_ty, I32] {
        ssig.params.push(AbiParam::new(t));
    }
    let sample_id = module.declare_function("ak_sample", Linkage::Import, &ssig).map_err(|e| err(e.to_string()))?;
    let mut fsig = module.make_signature();
    for t in [ptr_ty, I32, ptr_ty] {
        fsig.params.push(AbiParam::new(t));
    }
    let fetch_id = module.declare_function("ak_fetch", Linkage::Import, &fsig).map_err(|e| err(e.to_string()))?;

    let tcfg = module.target_config();
    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    let mut fctx = FunctionBuilderContext::new();
    {
        let mut b = FunctionBuilder::new(&mut ctx.func, &mut fctx);
        let sample_ref = module.declare_func_in_func(sample_id, b.func);
        let fetch_ref = module.declare_func_in_func(fetch_id, b.func);
        let scratch = b.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 128, 4));
        let entry_b = b.create_block();
        b.append_block_params_for_function_params(entry_b);
        b.switch_to_block(entry_b);
        let p = b.block_params(entry_b).to_vec();
        let (env, uni, x0, y0, nq, out) = (p[0], p[1], p[2], p[3], p[4], p[5]);
        let exec = b.declare_var(I32X4);
        let killed = b.declare_var(I32X4);
        let has_kill = m.functions.iter().any(|(_, f)| any_stmt(&f.body, &|s| matches!(s, S::Kill)));
        let mut lw = Lower {
            m,
            info,
            b,
            roots: Vec::new(),
            root_owner: Vec::new(),
            root_kinds: Vec::new(),
            gexpr: vec![None; m.global_expressions.len()],
            globals: HashMap::new(),
            frames: Vec::new(),
            exec,
            killed,
            has_kill,
            full: true,
            unrolled: 0,
            env,
            sample_ref,
            fetch_ref,
            scratch,
            ptr_ty,
            depth: 0,
        };
        // Constants, once, in the entry block.
        for (h, _) in m.global_expressions.iter() {
            if let Ok(v) = lw.lower(Sc::Global, h) {
                lw.gexpr[h.index()] = Some(v);
            }
        }
        // Globals: private ones become per-quad variables; textures are channels.
        let mut priv_globals = Vec::new();
        for (h, g) in m.global_variables.iter() {
            match g.space {
                naga::AddressSpace::Private | naga::AddressSpace::Function => {
                    let ks = lw.kinds(&m.types[g.ty].inner)?;
                    let root = lw.new_root(ks.clone());
                    lw.globals.insert(h, Val::P(Ptr { root, off: 0, len: ks.len(), dynx: Vec::new() }));
                    priv_globals.push((h, root, ks));
                }
                naga::AddressSpace::Handle => {
                    let bnd = g.binding.as_ref().map(|r| r.binding).unwrap_or(0);
                    match m.types[g.ty].inner {
                        T::Image { .. } => {
                            lw.globals.insert(h, Val::Img(bnd % 4));
                        }
                        _ => {
                            lw.globals.insert(h, Val::Smp);
                        }
                    }
                }
                _ => {}
            }
        }
        let fl = MemFlagsData::trusted();
        let ldx = lw.b.ins().load(F32X4, fl, env, texture::ENV_LANE_DX);
        let ldy = lw.b.ins().load(F32X4, fl, env, texture::ENV_LANE_DY);
        let qstep = lw.b.ins().load(F32, fl, env, texture::ENV_QSTEP);
        // Uniforms (and private-global initialisers) are loaded once per call,
        // so maths that only depends on them is loop-invariant and hoisted.
        let mut pre: Vec<(usize, Vec<Value>)> = Vec::new();
        for (h, root, ks) in &priv_globals {
            let g = &m.global_variables[*h];
            let name = g.name.as_deref().unwrap_or("");
            let mut vals = Vec::with_capacity(ks.len());
            if let Some(&off) = layout.get(name) {
                for (s, k) in ks.iter().enumerate() {
                    let ty = if *k == K::F { F32 } else { I32 };
                    let v = lw.b.ins().load(ty, fl, uni, ((off as usize + s) * 4) as i32);
                    vals.push(lw.b.ins().splat(k.ty(), v));
                }
            } else {
                let init = g.init.and_then(|i| match lw.gexpr[i.index()].clone() {
                    Some(Val::V(v)) => Some(v),
                    _ => None,
                });
                for (s, k) in ks.iter().enumerate() {
                    vals.push(match &init {
                        Some(iv) => iv[s.min(iv.len() - 1)],
                        None => lw.zero(*k),
                    });
                }
            }
            pre.push((*root, vals));
        }
        let xi = lw.b.declare_var(F32);
        let iv = lw.b.declare_var(I32);
        let op = lw.b.declare_var(ptr_ty);
        lw.b.def_var(xi, x0);
        let z = lw.b.ins().iconst(I32, 0);
        lw.b.def_var(iv, z);
        lw.b.def_var(op, out);
        let hdr = lw.b.create_block();
        let body = lw.b.create_block();
        let done = lw.b.create_block();
        lw.b.ins().jump(hdr, &[]);
        lw.b.switch_to_block(hdr);
        let i = lw.b.use_var(iv);
        let c = lw.b.ins().icmp(IntCC::SignedLessThan, i, nq);
        lw.b.ins().brif(c, body, &[], done, &[]);
        lw.b.switch_to_block(body);
        let x = lw.b.use_var(xi);
        let xs = lw.b.ins().splat(F32X4, x);
        let fx = lw.b.ins().fadd(xs, ldx);
        let ys = lw.b.ins().splat(F32X4, y0);
        let fy = lw.b.ins().fadd(ys, ldy);
        let all = lw.mask_all();
        lw.set_exec(all);
        let zero = isplat(&mut lw.b, 0);
        lw.b.def_var(killed, zero);
        lw.full = true;
        for (root, vals) in &pre {
            let vars = lw.roots[*root].clone();
            for (s, v) in vals.iter().enumerate() {
                lw.b.def_var(vars[s], *v);
            }
        }
        let f = &m.functions[entry];
        let color: Vec<Value> = match kind {
            Kind::Image if f.arguments.len() == 2 => {
                let root = lw.new_root(vec![K::F; 4]);
                let vars = lw.roots[root].clone();
                for (s, v) in [0.0, 0.0, 0.0, 1.0].iter().enumerate() {
                    let c = fsplat(&mut lw.b, *v);
                    lw.b.def_var(vars[s], c);
                }
                lw.inline(entry, vec![Val::P(Ptr { root, off: 0, len: 4, dynx: Vec::new() }), Val::V(vec![fx, fy])])?;
                vars.iter().map(|v| lw.b.use_var(*v)).collect()
            }
            Kind::Image if f.arguments.len() == 1 => lw.inline(entry, vec![Val::V(vec![fx, fy])])?,
            Kind::Image => return Err(err("mainImage must take (out vec4 fragColor, in vec2 fragCoord)")),
            Kind::Transition => {
                let off = *layout.get("iResolution").ok_or_else(|| err("internal: no iResolution"))?;
                let rx = lw.b.ins().load(F32, fl, uni, (off * 4) as i32);
                let ry = lw.b.ins().load(F32, fl, uni, (off * 4 + 4) as i32);
                let rx = lw.b.ins().splat(F32X4, rx);
                let ry = lw.b.ins().splat(F32X4, ry);
                let u = lw.b.ins().fdiv(fx, rx);
                let v = lw.b.ins().fdiv(fy, ry);
                lw.inline(entry, vec![Val::V(vec![u, v])])?
            }
        };
        if color.len() < 4 {
            return Err(err("the entry function must produce a vec4"));
        }
        let mut color = color;
        if has_kill {
            let k = lw.b.use_var(killed);
            for c in color.iter_mut() {
                let z = fsplat(&mut lw.b, 0.0);
                *c = sel(&mut lw.b, k, z, *c);
            }
        }
        let o = lw.b.use_var(op);
        for (i, c) in color.iter().take(4).enumerate() {
            lw.b.ins().store(fl, *c, o, (i * 16) as i32);
        }
        let x1 = lw.b.ins().fadd(x, qstep);
        lw.b.def_var(xi, x1);
        let i1 = lw.b.ins().iadd_imm_s(i, 1);
        lw.b.def_var(iv, i1);
        let o1 = lw.b.ins().iadd_imm_s(o, 64);
        lw.b.def_var(op, o1);
        lw.b.ins().jump(hdr, &[]);
        lw.b.switch_to_block(done);
        lw.b.ins().return_(&[]);
        lw.b.seal_all_blocks();
        let mut b = lw.b;
        let _ = &mut b;
        b.finalize(tcfg);
    }
    let insts = ctx.func.dfg.num_insts();
    let dump = std::env::var_os("ANIKIT_SHADER_DUMP").is_some();
    if dump {
        ctx.set_disasm(true);
        eprintln!("{}", ctx.func.display());
    }
    module.define_function(fid, &mut ctx).map_err(|e| err(format!("JIT: {e:?}")))?;
    if dump {
        if let Some(c) = ctx.compiled_code() {
            eprintln!("{}", c.vcode.as_deref().unwrap_or(""));
        }
    }
    module.clear_context(&mut ctx);
    module.finalize_definitions().map_err(|e| err(format!("JIT: {e}")))?;
    let code = module.get_finalized_function(fid);
    // SAFETY: the function was declared with exactly this signature.
    let func: KernelFn = unsafe { std::mem::transmute::<*const u8, KernelFn>(code) };
    let _ = I64;
    let _ = Block::from_u32;
    Ok(Kernel { module: Some(module), func, insts })
}
