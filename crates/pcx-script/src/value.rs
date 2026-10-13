//! The JASS value model and its arithmetic.
//!
//! # What a value is
//!
//! `null`, `boolean`, `integer`, `real`, `string`, `code` and opaque host handles.
//! [`HandleId`] is an identity token supplied by the host, not an engine pointer
//! or a guessed `GetHandleId` result. The host owns allocation, lifetime and
//! recycling semantics. No native in this crate mints a handle.
//!
//! # Where each rule comes from
//!
//! Every rule below is either proved from the scripts that ship on this
//! machine or refused. Nothing is inferred from how a name sounds.
//!
//! | rule | basis |
//! |---|---|
//! | integers are 32-bit two's complement and wrap | `$FFFFFFFF` style bit literals and the 32-bit id space of every rawcode comparison; the VM's own opcodes operate on 32-bit stack slots |
//! | `%` follows the dividend's sign, `/` truncates toward zero | `Blizzard.j`'s `ModuloInteger`, whose comment states the remainder lands in `(-divisor..0]` for a negative dividend. That holds only for truncated division |
//! | `R2I` truncates toward zero | `Blizzard.j`'s `ModuloReal`, which computes `dividend - I2R(R2I(dividend / divisor)) * divisor` and documents the same range |
//! | `I2R` is the int-to-real conversion | same expression; it only reproduces the ring remainder if `I2R` is exact |
//! | a mixed `real` and `integer` operand promotes the integer | `Blizzard.j` writes `R2I(duration * 1000)` where `duration` is a `real` and `1000` an `integer` literal |
//! | `+` concatenates only two strings | all 4,109 string comparisons and every concatenation in the corpus spell the conversion explicitly (`"Win = " + I2S(...)`), so vanilla has no implicit operand conversion to `string` |
//! | `==` on strings compares contents | `udg_RedVote == "expert"` and 4,108 more such comparisons compare a variable against a literal |
//! | real division follows IEEE 754 | the engine is a C program; `1.0 / 0.0` is the one case the corpus cannot pin and it is documented as unverified |
//!
//! Integer division by zero is refused rather than given a value: C integer
//! division by zero is undefined behaviour, so any answer would be a guess.

use crate::ast::BinOp;

/// The id of a callable function in a resolved [`crate::Program`]. A `code`
/// value carries this, and two `code` values are equal when they name the same
/// function.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct FunctionId(pub u32);

impl FunctionId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// An opaque non-null host identity. This token has no prescribed mapping to
/// native engine handle numbers; absent handles are represented by `Value::Null`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct HandleId(std::num::NonZeroU64);
impl HandleId {
    pub fn new(token: std::num::NonZeroU64) -> Self {
        Self(token)
    }
    pub fn token(self) -> u64 {
        self.0.get()
    }
}

/// A JASS value. `String` is owned rather than interned: the engine's string
/// table is a handle table owned by the host, and copying bytes loses nothing
/// a handle comparison would decide.
#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    /// The null handle, and the absent value.
    Null,
    Bool(bool),
    Int(i32),
    Real(f32),
    Str(String),
    Code(FunctionId),
    Handle(HandleId),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    Null,
    Bool,
    Int,
    Real,
    Str,
    Code,
    Handle,
}

impl Class {
    /// The type name the map's own declaration would use.
    pub fn name(self) -> &'static str {
        match self {
            Class::Null => "null",
            Class::Bool => "boolean",
            Class::Int => "integer",
            Class::Real => "real",
            Class::Str => "string",
            Class::Code => "code",
            Class::Handle => "handle",
        }
    }
}

impl Value {
    pub fn class(&self) -> Class {
        match self {
            Value::Null => Class::Null,
            Value::Bool(_) => Class::Bool,
            Value::Int(_) => Class::Int,
            Value::Real(_) => Class::Real,
            Value::Str(_) => Class::Str,
            Value::Code(_) => Class::Code,
            Value::Handle(_) => Class::Handle,
        }
    }

    pub fn type_name(&self) -> &'static str {
        self.class().name()
    }

    /// Two's-complement 32-bit wrap of a wider literal. The lexer keeps an
    /// `i64`, and a hex literal such as `$FFFFFFFF` is the map's own spelling
    /// of the 32-bit id space, not an out-of-range value.
    pub fn wrap32(v: i64) -> i32 {
        v as i32
    }

    pub fn as_int(&self) -> Option<i32> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_real(&self) -> Option<f32> {
        match self {
            Value::Real(r) => Some(*r),
            Value::Int(i) => Some(*i as f32),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

impl std::fmt::Display for Value {
    /// Rendering is for diagnostics and for a caller that wants to show a
    /// value. It is deliberately not the engine's `I2S`/`R2S`, whose
    /// formatting is unverified and which therefore stay unimplemented in
    /// [`crate::natives`].
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => f.write_str("null"),
            Value::Bool(b) => f.write_str(if *b { "true" } else { "false" }),
            Value::Int(i) => write!(f, "{i}"),
            Value::Real(r) => write!(f, "{r}"),
            Value::Str(s) => f.write_str(s),
            Value::Code(id) => write!(f, "function#{}", id.0),
            Value::Handle(id) => write!(f, "host-handle#{}", id.token()),
        }
    }
}

/// Why an operator could not be applied. Each variant names both operand
/// types so the caller can put the message against the map's own span.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum OpError {
    /// `>` and friends on a value that is not a number.
    Ordered(Class, Class),
    /// `==` / `!=` between incompatible classes after supported numeric
    /// promotion and nullable comparisons have been handled.
    EqualityMismatch(Class, Class),
    /// One operand is a string and the other is not.
    Concat(Class, Class),
    /// `-`, `*`, `%` on a value that is not a number.
    Arithmetic(Class, Class),
    /// `%` on reals. Vanilla's modulus is the integer one (`NodeMod` in the
    /// engine's own parser), so a real operand has no defined result.
    ModuloNotInteger(Class, Class),
    /// `and` / `or` / `not` on a non-boolean.
    Logic(Class),
    IntDivByZero,
    IntModByZero,
}

impl OpError {
    pub fn message(&self) -> String {
        match self {
            OpError::Ordered(a, b) => format!(
                "`<`, `<=`, `>` and `>=` need numbers, got {} and {}",
                a.name(),
                b.name()
            ),
            OpError::EqualityMismatch(a, b) => {
                format!("`==` and `!=` cannot compare {} and {}", a.name(), b.name())
            }
            OpError::Concat(a, b) => format!(
                "`+` concatenates two strings; got {} and {}. Vanilla has no \
                 implicit conversion to string, so the operand needs its own \
                 I2S/R2S call",
                a.name(),
                b.name()
            ),
            OpError::Arithmetic(a, b) => {
                format!(
                    "`-`, `*` and `%` need numbers, got {} and {}",
                    a.name(),
                    b.name()
                )
            }
            OpError::ModuloNotInteger(a, b) => {
                format!("`%` is integer modulus, got {} and {}", a.name(), b.name())
            }
            OpError::Logic(a) => format!("`and`, `or` and `not` need a boolean, got {}", a.name()),
            OpError::IntDivByZero => "integer division by zero".to_string(),
            OpError::IntModByZero => "integer modulus by zero".to_string(),
        }
    }
}

/// `and` / `or` are evaluated by the caller so they can short-circuit; this
/// is the operand check alone.
pub fn logic_operand(v: &Value) -> Result<bool, OpError> {
    v.as_bool().ok_or(OpError::Logic(v.class()))
}

/// Unary operators. `not` rejects a non-boolean because vanilla does.
pub fn unary(op: crate::ast::UnOp, v: &Value) -> Result<Value, OpError> {
    use crate::ast::UnOp;
    match op {
        UnOp::Not => Ok(Value::Bool(!logic_operand(v)?)),
        UnOp::Neg => match v {
            Value::Int(i) => Ok(Value::Int(i.wrapping_neg())),
            Value::Real(r) => Ok(Value::Real(-r)),
            _ => Err(OpError::Arithmetic(v.class(), v.class())),
        },
        UnOp::Pos => match v {
            Value::Int(i) => Ok(Value::Int(*i)),
            Value::Real(r) => Ok(Value::Real(*r)),
            _ => Err(OpError::Arithmetic(v.class(), v.class())),
        },
    }
}

/// Binary operator application. `and` and `or` are **not** handled here: they
/// need short-circuiting, which belongs to evaluation (`crate::eval`).
pub fn binary(op: BinOp, lhs: &Value, rhs: &Value) -> Result<Value, OpError> {
    match op {
        BinOp::Add => add(lhs, rhs),
        BinOp::Sub | BinOp::Mul => match pair(lhs, rhs) {
            Pair::Ints(a, b) => {
                let v = if op == BinOp::Sub {
                    a.wrapping_sub(b)
                } else {
                    a.wrapping_mul(b)
                };
                Ok(Value::Int(v))
            }
            Pair::Reals(a, b) => {
                let v = if op == BinOp::Sub { a - b } else { a * b };
                Ok(Value::Real(v))
            }
            Pair::Bad(e) => Err(e),
        },
        BinOp::Div => match pair(lhs, rhs) {
            Pair::Ints(a, b) => {
                if b == 0 {
                    Err(OpError::IntDivByZero)
                } else {
                    // Truncating toward zero, which is what `Blizzard.j`'s
                    // ModuloInteger documents. `wrapping_div` only avoids the
                    // i32::MIN / -1 overflow, not division by zero.
                    Ok(Value::Int(a.wrapping_div(b)))
                }
            }
            Pair::Reals(a, b) => Ok(Value::Real(a / b)),
            Pair::Bad(e) => Err(e),
        },
        BinOp::Modulo => match pair(lhs, rhs) {
            Pair::Ints(a, b) => {
                if b == 0 {
                    Err(OpError::IntModByZero)
                } else {
                    // C remainder: the sign follows the dividend, so that
                    // `(a / b) * b + a % b == a` holds for negative `a`.
                    Ok(Value::Int(a.wrapping_rem(b)))
                }
            }
            Pair::Reals(_, _) => Err(OpError::ModuloNotInteger(lhs.class(), rhs.class())),
            Pair::Bad(e) => Err(e),
        },
        BinOp::Eq | BinOp::Ne => {
            let same = equal_classes(lhs, rhs)?;
            let eq = match op {
                BinOp::Eq => same,
                _ => !same,
            };
            Ok(Value::Bool(eq))
        }
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
            let ord = order(lhs, rhs)?;
            let r = match op {
                BinOp::Lt => ord.is_lt(),
                BinOp::Le => ord.is_le(),
                BinOp::Gt => ord.is_gt(),
                _ => ord.is_ge(),
            };
            Ok(Value::Bool(r))
        }
        BinOp::And | BinOp::Or => Err(OpError::Logic(lhs.class())),
    }
}

/// The two operand shapes a numeric operator was given.
enum Pair {
    Ints(i32, i32),
    Reals(f32, f32),
    Bad(OpError),
}

/// Classify a numeric operand pair, promoting an integer to a real exactly as
/// the engine's mixed-operand arithmetic does.
fn pair(lhs: &Value, rhs: &Value) -> Pair {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Pair::Ints(*a, *b),
        (Value::Real(a), Value::Real(b)) => Pair::Reals(*a, *b),
        (Value::Real(a), Value::Int(b)) => Pair::Reals(*a, *b as f32),
        (Value::Int(a), Value::Real(b)) => Pair::Reals(*a as f32, *b),
        _ => Pair::Bad(OpError::Arithmetic(lhs.class(), rhs.class())),
    }
}

/// Classify an ordered-operand pair. Two integers compare exactly; a mixed
/// pair goes through the same promotion as arithmetic, which loses precision
/// above 2^24 and does so in the engine too.
fn order_pair(lhs: &Value, rhs: &Value) -> Result<Pair, OpError> {
    match pair(lhs, rhs) {
        Pair::Bad(_) => Err(OpError::Ordered(lhs.class(), rhs.class())),
        p => Ok(p),
    }
}

fn order(lhs: &Value, rhs: &Value) -> Result<std::cmp::Ordering, OpError> {
    match order_pair(lhs, rhs)? {
        Pair::Ints(a, b) => Ok(a.cmp(&b)),
        Pair::Reals(a, b) => a
            .partial_cmp(&b)
            .ok_or_else(|| OpError::Ordered(lhs.class(), rhs.class())),
        Pair::Bad(e) => Err(e),
    }
}

/// Same-class equality and nullable string/handle checks, with the integer/real pair promoted the way
/// arithmetic promotes it. Two integers compare exactly.
fn equal_classes(lhs: &Value, rhs: &Value) -> Result<bool, OpError> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(a == b),
        (Value::Real(a), Value::Real(b)) => Ok(a == b),
        (Value::Real(a), Value::Int(b)) => Ok(a == &(*b as f32)),
        (Value::Int(a), Value::Real(b)) => Ok(&(*a as f32) == b),
        (Value::Bool(a), Value::Bool(b)) => Ok(a == b),
        (Value::Str(a), Value::Str(b)) => Ok(a == b),
        (Value::Code(a), Value::Code(b)) => Ok(a == b),
        (Value::Handle(a), Value::Handle(b)) => Ok(a == b),
        (Value::Handle(_), Value::Null)
        | (Value::Null, Value::Handle(_))
        | (Value::Str(_), Value::Null)
        | (Value::Null, Value::Str(_)) => Ok(false),
        (Value::Null, Value::Null) => Ok(true),
        _ => Err(OpError::EqualityMismatch(lhs.class(), rhs.class())),
    }
}

/// `+`: concatenation when a string is involved, otherwise arithmetic.
fn add(lhs: &Value, rhs: &Value) -> Result<Value, OpError> {
    match (lhs, rhs) {
        (Value::Str(a), Value::Str(b)) => {
            let mut s = String::with_capacity(a.len() + b.len());
            s.push_str(a);
            s.push_str(b);
            Ok(Value::Str(s))
        }
        (Value::Str(_), _) | (_, Value::Str(_)) => Err(OpError::Concat(lhs.class(), rhs.class())),
        _ => match pair(lhs, rhs) {
            Pair::Ints(a, b) => Ok(Value::Int(a.wrapping_add(b))),
            Pair::Reals(a, b) => Ok(Value::Real(a + b)),
            Pair::Bad(e) => Err(e),
        },
    }
}

#[cfg(test)]
mod handle_tests {
    use super::*;
    use crate::{Host, Vm, parse, resolve};
    fn handle(token: u64) -> Value {
        Value::Handle(HandleId::new(std::num::NonZeroU64::new(token).unwrap()))
    }
    #[test]
    fn opaque_handles_have_identity_and_null_equality_without_integer_conversion() {
        for (lhs, rhs, same) in [
            (handle(1), handle(1), true),
            (handle(1), handle(2), false),
            (handle(1), Value::Null, false),
            (Value::Null, handle(1), false),
        ] {
            assert_eq!(binary(BinOp::Eq, &lhs, &rhs).unwrap(), Value::Bool(same));
            assert_eq!(binary(BinOp::Ne, &lhs, &rhs).unwrap(), Value::Bool(!same));
        }
        assert_eq!(handle(1).as_int(), None);
        assert!(binary(BinOp::Eq, &handle(1), &Value::Int(1)).is_err());
        assert!(binary(BinOp::Add, &handle(1), &handle(1)).is_err());
    }
    #[test]
    fn native_supplied_handle_survives_globals_arrays_arguments_and_null_checks() {
        let source = "type timer extends handle\nnative Make takes nothing returns timer\nnative Inspect takes timer t returns boolean\nglobals\ntimer retained = null\ntimer array timers\nendglobals\nfunction main takes nothing returns boolean\nlocal timer t = Make()\nset retained = t\nset timers[3] = retained\nreturn timers[3] == t and t != null and Inspect(t)\nendfunction\n";
        let program = resolve(&parse(source).unwrap()).unwrap();
        struct Fixture;
        impl Host for Fixture {
            fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
                match (name, args) {
                    ("Make", []) => Ok(handle(31)),
                    ("Inspect", [value]) => Ok(Value::Bool(*value == handle(31))),
                    _ => Err(format!("unsupported {name}")),
                }
            }
        }
        let mut vm = Vm::new(&program, Fixture);
        assert_eq!(vm.call("main", &[]).unwrap(), Value::Bool(true));
        assert_eq!(vm.global("retained"), handle(31));
        assert_eq!(vm.global_at("timers", 3), handle(31));
    }
}

#[cfg(test)]
mod nullable_string_tests {
    use super::*;
    use crate::{NullHost, Vm, parse, resolve};
    #[test]
    fn equality_and_inequality_cover_both_orders_and_null_null() {
        for text in ["present", ""] {
            let present = Value::Str(text.into());
            for (left, right) in [(&present, &Value::Null), (&Value::Null, &present)] {
                assert_eq!(binary(BinOp::Eq, left, right).unwrap(), Value::Bool(false));
                assert_eq!(binary(BinOp::Ne, left, right).unwrap(), Value::Bool(true));
            }
        }
        assert_eq!(
            binary(BinOp::Eq, &Value::Null, &Value::Null).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            binary(BinOp::Ne, &Value::Null, &Value::Null).unwrap(),
            Value::Bool(false)
        );
        for numeric in [Value::Int(0), Value::Real(0.0)] {
            for other in [Value::Null, Value::Str(String::new())] {
                for (left, right) in [(&numeric, &other), (&other, &numeric)] {
                    for op in [BinOp::Eq, BinOp::Ne] {
                        assert_eq!(
                            binary(op, left, right),
                            Err(OpError::EqualityMismatch(left.class(), right.class()))
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn nullable_string_checks_accept_present_and_absent_values_without_numeric_coercion() {
        let source = "function Checked takes string text returns string\nif text == null then\nreturn \"absent\"\nendif\nreturn text\nendfunction\n";
        let program = resolve(&parse(source).unwrap()).unwrap();
        let mut vm = Vm::new(&program, NullHost);
        assert_eq!(
            vm.call("Checked", &[Value::Null]).unwrap(),
            Value::Str("absent".into())
        );
        assert_eq!(
            vm.call("Checked", &[Value::Str("present".into())]).unwrap(),
            Value::Str("present".into())
        );
        assert_eq!(
            binary(BinOp::Ne, &Value::Null, &Value::Str(String::new())).unwrap(),
            Value::Bool(true)
        );
        assert!(binary(BinOp::Eq, &Value::Null, &Value::Int(0)).is_err());
    }
}
