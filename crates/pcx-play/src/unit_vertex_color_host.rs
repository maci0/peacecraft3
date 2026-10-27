//! Explicit script colour on the canonical unit; team colour is independent.
use super::*;

impl ObjectHost {
    pub(super) fn vertex_color_declaration(&self, source: &str) -> Result<Option<Span>, Error> {
        if self.inner.program.natives().get(SET_VERTEX_COLOR).is_none()
            && self
                .inner
                .program
                .natives()
                .ambiguous(SET_VERTEX_COLOR)
                .is_none()
        {
            return Ok(None);
        }
        Self::contract(
            &self.inner,
            SET_VERTEX_COLOR,
            &[KIND, "integer", "integer", "integer", "integer"],
            "nothing",
            false,
            Some(KIND),
        )?;
        let native = self
            .inner
            .program
            .natives()
            .get(SET_VERTEX_COLOR)
            .expect("checked contract");
        if native.source != source {
            return Err(self.inner.error(SET_VERTEX_COLOR, Failure::Source));
        }
        Ok(Some(native.span))
    }

    pub(super) fn set_unit_vertex_color(&mut self, args: &[Value]) -> Result<Value, Error> {
        let [
            unit,
            Value::Int(red),
            Value::Int(green),
            Value::Int(blue),
            Value::Int(alpha),
        ] = args
        else {
            return Err(self.inner.error(SET_VERTEX_COLOR, Failure::Signature));
        };
        let unit = match unit {
            Value::Null => return Ok(Value::Null),
            Value::Handle(unit) => *unit,
            _ => return Err(self.inner.error(SET_VERTEX_COLOR, Failure::WrongFamily)),
        };
        match self
            .inner
            .entry(unit)
            .map_err(|error| self.inner.error(SET_VERTEX_COLOR, error.failure))?
        {
            Entry::RemovedUnit { .. } => return Ok(Value::Null),
            Entry::Unit { .. } => {}
            _ => return Err(self.inner.error(SET_VERTEX_COLOR, Failure::WrongFamily)),
        }
        // The native's signed saturation is the independently qualified RGBA-byte ABI.
        let rgba = [*red, *green, *blue, *alpha].map(|channel| {
            u8::try_from(channel.clamp(0, i32::from(u8::MAX))).expect("saturated byte")
        });
        let Entry::Unit { record } = self.inner.entry_mut(unit)? else {
            unreachable!("checked unit")
        };
        record.vertex_color = Some(rgba);
        Ok(Value::Null)
    }
}
