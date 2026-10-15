//! Bounded shape admission for the supplied script's sound-volume operand.
use pcx_script::{BinOp, Expr, FunctionDecl, Item, Literal, Span, Stmt, TypeKind};

/// The selected script supplies the numeric range. Mapping table Volume linearly
/// into this range is a recreation convention; native gain parity is unverified.
#[derive(Debug, Clone, Copy)]
pub struct VolumeScale {
    pub maximum: f32,
    pub helper_span: Span,
    pub operand_span: Span,
    pub conversion_span: Span,
}

fn ident(e: &Expr, expected: &str) -> bool {
    matches!(ungroup(e), Expr::Ident(i, _) if i.name == expected)
}
fn ungroup(mut e: &Expr) -> &Expr {
    for _ in 0..16 {
        if let Expr::Group(inner, _) = e {
            e = inner;
        } else {
            break;
        }
    }
    e
}
fn call<'a>(e: &'a Expr, name: &str) -> Option<&'a [Expr]> {
    match ungroup(e) {
        Expr::Call { callee, args, .. } if ident(callee, name) => Some(args),
        _ => None,
    }
}
fn unique<'a>(items: &'a [Item], name: &str) -> Result<&'a FunctionDecl, String> {
    let mut matches = items.iter().filter_map(|i| match i {
        Item::Function(f) if f.name.name == name => Some(f),
        _ => None,
    });
    let f = matches.next().ok_or_else(|| format!("missing {name}"))?;
    if matches.next().is_some()
        || f.is_constant
        || f.is_static
        || f.ret.array
        || f.params.iter().any(|p| p.ty.array)
    {
        return Err(format!("ambiguous or unsupported {name}"));
    }
    Ok(f)
}
// Lexing is iterative. Only the two selected, bounded declarations reach the
// recursive parser; unrelated script functions are not parsed or executed.
fn selected_function(
    text: &str,
    tokens: &[pcx_script::lexer::Token],
    name: &str,
) -> Result<(FunctionDecl, u32), String> {
    use pcx_script::lexer::{Kw, Tok};
    let mut selected = None;
    let mut in_function = false;
    for (i, token) in tokens.iter().enumerate() {
        if token.kind == Tok::Kw(Kw::EndFunction) {
            in_function = false;
            continue;
        }
        if token.kind != Tok::Kw(Kw::Function)
            || !tokens
                .get(i + 2)
                .is_some_and(|t| t.kind == Tok::Kw(Kw::Takes))
        {
            continue;
        }
        let target = tokens
            .get(i + 1)
            .is_some_and(|t| matches!(&t.kind,Tok::Ident(id) if id.name==name));
        if target {
            if in_function || selected.is_some() {
                return Err(format!("ambiguous or nested {name}"));
            }
            let end = tokens[i + 1..]
                .iter()
                .position(|t| t.kind == Tok::Kw(Kw::EndFunction))
                .map(|n| i + 1 + n)
                .ok_or_else(|| format!("unterminated {name}"))?;
            let part = &tokens[i..=end];
            if part.len() > 256 {
                return Err(format!("{name}: helper token work bound exceeded"));
            }
            let mut operators = 0usize;
            let mut groups = 0usize;
            let mut blocks = 0usize;
            for t in part {
                match t.kind {
                    Tok::Plus
                    | Tok::Minus
                    | Tok::Star
                    | Tok::Slash
                    | Tok::Percent
                    | Tok::EqEq
                    | Tok::BangEq
                    | Tok::Lt
                    | Tok::Le
                    | Tok::Gt
                    | Tok::Ge
                    | Tok::Kw(Kw::Not | Kw::And | Kw::Or | Kw::Debug) => operators += 1,
                    Tok::LParen | Tok::LBracket => groups += 1,
                    Tok::RParen | Tok::RBracket => {
                        groups = groups
                            .checked_sub(1)
                            .ok_or("unbalanced helper delimiters")?
                    }
                    Tok::Kw(Kw::If | Kw::Loop) => blocks += 1,
                    Tok::Kw(Kw::EndIf | Kw::EndLoop) => {
                        blocks = blocks.checked_sub(1).ok_or("unbalanced helper blocks")?
                    }
                    _ => {}
                }
                if operators > 32 || groups > 16 || blocks > 16 {
                    return Err(format!(
                        "{name}: helper expression/statement depth bound exceeded"
                    ));
                }
            }
            let start = token.span.start as usize;
            let end = tokens[end].span.end as usize;
            let line = text[..start].rfind(['\n', '\r']).map_or(0, |n| n + 1);
            if !text[line..start].trim().is_empty() || end - start > 16 * 1024 {
                return Err(format!("unsupported {name} declaration or byte bound"));
            }
            let mut tree = pcx_script::parse(&text[start..end]).map_err(|e| e.to_string())?;
            let _ = unique(&tree.items, name)?;
            let Item::Function(f) = tree.items.remove(0) else {
                return Err(format!("unsupported {name}"));
            };
            selected = Some((f, start as u32));
        }
        in_function = true;
    }
    selected.ok_or_else(|| format!("missing {name}"))
}

fn lower_clamp(branch: &pcx_script::IfBranch, result: &str) -> bool {
    let Expr::Binary {
        op: BinOp::Lt,
        lhs,
        rhs,
        ..
    } = ungroup(&branch.cond)
    else {
        return false;
    };
    let Expr::Literal(Literal::Int(zero), _) = ungroup(rhs) else {
        return false;
    };
    let [Stmt::Set { target, value, .. }] = branch.body.as_slice() else {
        return false;
    };
    ident(lhs, result)
        && zero.value == 0
        && ident(target, result)
        && matches!(ungroup(value),Expr::Literal(Literal::Int(v),_) if v.value==zero.value)
}
fn upper_clamp(branch: &pcx_script::IfBranch, result: &str, maximum: &str) -> bool {
    let Expr::Binary {
        op: BinOp::Gt,
        lhs,
        rhs,
        ..
    } = ungroup(&branch.cond)
    else {
        return false;
    };
    let [Stmt::Set { target, value, .. }] = branch.body.as_slice() else {
        return false;
    };
    ident(lhs, result) && ident(rhs, maximum) && ident(target, result) && ident(value, maximum)
}
fn conversion(f: &FunctionDecl) -> bool {
    let [percent, maximum] = f.params.as_slice() else {
        return false;
    };
    if percent.ty.kind != TypeKind::Real
        || maximum.ty.kind != TypeKind::Integer
        || f.ret.kind != TypeKind::Integer
    {
        return false;
    }
    let Some(body) = f.body.as_deref() else {
        return false;
    };
    let (local, lower, upper, ret) = match body {
        [
            Stmt::Local(local, _),
            Stmt::If {
                branches,
                else_body: None,
                ..
            },
            Stmt::Return(Some(ret), _),
        ] if branches.len() == 2 => (local, &branches[0], &branches[1], ret),
        [
            Stmt::Local(local, _),
            Stmt::If {
                branches: first,
                else_body: None,
                ..
            },
            Stmt::If {
                branches: second,
                else_body: None,
                ..
            },
            Stmt::Return(Some(ret), _),
        ] if first.len() == 1 && second.len() == 1 => (local, &first[0], &second[0], ret),
        _ => return false,
    };
    if local.ty.array || local.ty.kind != TypeKind::Integer {
        return false;
    }
    let Some(init) = local.init.as_ref() else {
        return false;
    };
    let Some([value]) = call(init, "R2I") else {
        return false;
    };
    let Expr::Binary {
        op: BinOp::Mul,
        lhs,
        rhs,
        ..
    } = ungroup(value)
    else {
        return false;
    };
    let Expr::Literal(Literal::Real(factor), _) = ungroup(rhs) else {
        return false;
    };
    if !factor.value.is_finite() || factor.value <= 0.0 {
        return false;
    }
    let Expr::Binary {
        op: BinOp::Mul,
        lhs: percent_arg,
        rhs: maximum_arg,
        ..
    } = ungroup(lhs)
    else {
        return false;
    };
    let Some([maximum_arg]) = call(maximum_arg, "I2R") else {
        return false;
    };
    ident(percent_arg, &percent.name.name)
        && ident(maximum_arg, &maximum.name.name)
        && lower_clamp(lower, &local.name.name)
        && upper_clamp(upper, &local.name.name, &maximum.name.name)
        && ident(ret, &local.name.name)
}

impl VolumeScale {
    pub fn checked(bytes: &[u8]) -> Result<Self, String> {
        const MAX_SCRIPT_BYTES: usize = 2 * 1024 * 1024;
        if bytes.len() > MAX_SCRIPT_BYTES {
            return Err("sound script exceeds project admission byte bound".into());
        }
        let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
        let lexed = pcx_script::lex(text).map_err(|e| e.to_string())?;
        if lexed.tokens.len() > 512 * 1024 {
            return Err("sound script token work bound exceeded".into());
        }
        let (f, base) = selected_function(text, &lexed.tokens, "SetSoundVolumeBJ")?;
        let [sound, percent] = f.params.as_slice() else {
            return Err("unsupported SetSoundVolumeBJ parameters".into());
        };
        let Some([Stmt::Call { callee, args, .. }]) = f.body.as_deref() else {
            return Err("unsupported SetSoundVolumeBJ body".into());
        };
        let [handle, value] = args.as_slice() else {
            return Err("unsupported SetSoundVolume arguments".into());
        };
        if f.ret.kind != TypeKind::Nothing
            || sound.ty.kind != TypeKind::Named("sound".into())
            || percent.ty.kind != TypeKind::Real
            || callee.name != "SetSoundVolume"
            || !ident(handle, &sound.name.name)
        {
            return Err("unsupported SetSoundVolumeBJ binding".into());
        }
        let Some([actual, Expr::Literal(Literal::Int(max), span)]) = call(value, "PercentToInt")
        else {
            return Err("unsupported sound maximum operand".into());
        };
        if !ident(actual, &percent.name.name) || max.value <= 0 || max.value > i64::from(i32::MAX) {
            return Err("invalid authored sound maximum".into());
        }
        let (convert, convert_base) = selected_function(text, &lexed.tokens, "PercentToInt")?;
        if !conversion(&convert) {
            return Err("unsupported PercentToInt arithmetic/clamp shape".into());
        }
        Ok(Self {
            maximum: max.value as f32,
            helper_span: Span {
                start: f.span.start + base,
                end: f.span.end + base,
            },
            operand_span: Span {
                start: span.start + base,
                end: span.end + base,
            },
            conversion_span: Span {
                start: convert.span.start + convert_base,
                end: convert.span.end + convert_base,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(max: i32) -> String {
        format!(
            "function SetSoundVolumeBJ takes sound h, real p returns nothing\ncall SetSoundVolume(h, PercentToInt(p, {max}))\nendfunction\nfunction PercentToInt takes real p, integer m returns integer\nlocal integer r=R2I(p*I2R(m)*0.025)\nif (r<0) then\nset r=0\nendif\nif (r>m) then\nset r=m\nendif\nreturn r\nendfunction\n"
        )
    }
    #[test]
    fn range_follows_source_and_rejects_unsupported_or_ambiguous_bodies() {
        for max in [37, 511] {
            let s = source(max);
            let scale = VolumeScale::checked(s.as_bytes()).unwrap();
            assert_eq!(scale.maximum, max as f32);
            assert!(scale.operand_span.start < scale.operand_span.end);
            assert!(scale.helper_span.start < scale.helper_span.end);
            assert!(scale.conversion_span.start < scale.conversion_span.end);
        }
        let valid = source(37);
        let alternate = valid.replace("endif\nif (r>m)", "elseif (r>m)");
        assert_eq!(
            VolumeScale::checked(alternate.as_bytes()).unwrap().maximum,
            37.0
        );
        for bad in [
            source(0),
            source(-3),
            valid.replace("r>m", "r<m"),
            valid.replace("set r=m", "set r=1"),
            valid.replace("0.025", "-0.025"),
            valid.replace("I2R(m)", "I2R(other)"),
            valid.replace("return r", "call Other()\nreturn r"),
            format!("{valid}{valid}"),
        ] {
            assert!(VolumeScale::checked(bad.as_bytes()).is_err(), "{bad}");
        }
        for expression in [
            format!("{}p{}", "(".repeat(40), ")".repeat(40)),
            format!("{}p", "-".repeat(40)),
            format!("p{}", "+p".repeat(40)),
        ] {
            let bad = valid.replace("R2I(p*I2R(m)*0.025)", &format!("R2I({expression})"));
            assert!(VolumeScale::checked(bad.as_bytes()).is_err());
        }
        let nested = format!(
            "{}set r=1\n{}",
            "if true then\n".repeat(20),
            "endif\n".repeat(20)
        );
        assert!(
            VolumeScale::checked(
                valid
                    .replace("return r", &format!("{nested}return r"))
                    .as_bytes()
            )
            .is_err()
        );
        assert!(VolumeScale::checked(&vec![b' '; 2 * 1024 * 1024 + 1]).is_err());
    }
}
