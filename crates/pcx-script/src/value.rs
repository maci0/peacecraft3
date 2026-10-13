//! The JASS value model and its arithmetic.
//!
//! # What a value is
//!
//! `null`, `boolean`, `integer`, `real`, `string` and a `code` reference.
//! There is no handle variant: only a native can mint a handle, every native
//! this stage can run returns one of the five above, and every host that
//! implements `Host::native` ([`crate::eval`]) decides how its own handle
//! values are identified. A null handle is [`Value::Null`], and handle
//! identity therefore reduces to null-versus-null here, which is the only
//! comparison this stage can honestly make.
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
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    Null,
    Bool,
    Int,
    Real,
    Str,
    Code,
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
        }
    }
}

/// Why an operator could not be applied. Each variant names both operand
/// types so the caller can put the message against the map's own span.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum OpError {
    /// `>` and friends on a value that is not a number.
    Ordered(Class, Class),
    /// `==` / `!=` between two different classes. JASS has no cross-class
    /// equality, not even against `null`.
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
            OpError::EqualityMismatch(a, b) => format!(
                "`==` and `!=` need two values of the same type, got {} and {}",
                a.name(),
                b.name()
            ),
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

/// Same-class equality, with the integer/real pair promoted the way
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
