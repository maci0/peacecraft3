//! `CreateBlightedGoldmine`: the Haunted Gold Mine of the selected scripts.
//!
//! The native creates a unit of one fixed type. The type is read from the selected
//! `Blizzard.j`: `CreateUnitAtLocSaveLast` routes exactly one rawcode to this native
//! (`if (unitid == 'ugol') then set bj_lastCreatedUnit = CreateBlightedGoldmine(...)`), and
//! `BlightGoldMineForPlayerBJ` calls it to replace a gold mine ("Create a Haunted Gold Mine").
//! The unit is then created through `CreateUnit` with that rawcode, so its model, resource
//! ability, collision and footprint come from the selected tables like any unit, and the
//! caller's `SetResourceAmount` sets the gold.
//!
//! Not established: whether the native itself paints blight under or around the mine. The
//! shipped melee start code paints a 768-radius patch with a separate `SetBlightLoc`, which
//! suggests it does not. No blight is painted here, and the `SetBlight*` natives remain
//! unimplemented (see `docs/re-notes/terrain-blight.md`).
use super::{Error, Failure, ObjectHost};
use pcx_script::ast::{BinOp, Expr, Literal, Stmt};
use pcx_script::{Host, Value};

const NAME: &str = "CreateBlightedGoldmine";
/// The `Blizzard.j` function that maps a rawcode to this native.
const MAPPER: &str = "CreateUnitAtLocSaveLast";

fn ungroup(mut expr: &Expr) -> &Expr {
    while let Expr::Group(inner, _) = expr {
        expr = inner;
    }
    expr
}

fn calls_native(statements: &[Stmt]) -> bool {
    statements.iter().any(|statement| {
        matches!(
            statement,
            Stmt::Set { value, .. }
                if matches!(
                    ungroup(value),
                    Expr::Call { callee, .. }
                        if matches!(callee.as_ref(), Expr::Ident(name, _) if name.name == NAME)
                )
        )
    })
}

/// The rawcode compared in the branch of `body` that calls this native.
fn mapped_rawcode(body: &[Stmt]) -> Option<i32> {
    body.iter().find_map(|statement| {
        let Stmt::If { branches, .. } = statement else {
            return None;
        };
        branches.iter().find_map(|branch| {
            if !calls_native(&branch.body) {
                return None;
            }
            let Expr::Binary {
                op: BinOp::Eq, rhs, ..
            } = ungroup(&branch.cond)
            else {
                return None;
            };
            let Expr::Literal(Literal::Raw(raw), _) = ungroup(rhs) else {
                return None;
            };
            let bytes: [u8; 4] = raw.text.as_bytes().try_into().ok()?;
            Some(i32::from_be_bytes(bytes))
        })
    })
}

impl ObjectHost {
    pub(super) fn create_blighted_goldmine(&mut self, args: &[Value]) -> Result<Value, Error> {
        Self::contract(
            &self.inner,
            NAME,
            &["player", "real", "real", "real"],
            "unit",
            false,
            Some("unit"),
        )?;
        let natives = self.inner.program.natives();
        if natives.get(NAME).map(|n| &n.source) != natives.get("CreateUnit").map(|n| &n.source) {
            return Err(self.inner.error(NAME, Failure::Source));
        }
        let [player, x, y, face] = args else {
            return Err(self.inner.error(NAME, Failure::Signature));
        };
        let rawcode = self
            .inner
            .program
            .function(MAPPER)
            .and_then(|function| mapped_rawcode(function.body.as_deref()?))
            .ok_or_else(|| {
                self.inner.error(
                    NAME,
                    Failure::UnitActivity(format!(
                        "the selected script's {MAPPER} does not name the haunted gold mine type"
                    )),
                )
            })?;
        let forwarded = [
            player.clone(),
            Value::Int(rawcode),
            x.clone(),
            y.clone(),
            face.clone(),
        ];
        Host::native(self, "CreateUnit", &forwarded)
            .map_err(|why| self.inner.error(NAME, Failure::UnitActivity(why)))
    }
}
