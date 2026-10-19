use super::{Error, Failure, ObjectHost};
use pcx_script::{RealOps, Value};

impl ObjectHost {
    /// Admit source-qualified factor limits on this session's existing player rows.
    /// Initial factors and experience awards are not inferred from a setter.
    pub fn with_player_experience_handicap(
        mut self,
        rules: crate::player_handicap::Rules,
    ) -> Result<Self, Error> {
        let name = "SetPlayerHandicapXP";
        let players = self.players.as_ref().ok_or_else(|| {
            self.inner
                .error(name, Failure::PlayerState("players required"))
        })?;
        if players.experience_handicap.is_some() {
            return Err(self.inner.error(
                name,
                Failure::PlayerState("experience handicap already admitted"),
            ));
        }
        if self
            .numeric
            .as_ref()
            .is_none_or(|n| n.image_sha256() != rules.image_sha256())
        {
            return Err(self.inner.error(
                name,
                Failure::PlayerState("matching selected numeric policy required"),
            ));
        }
        let source = &self
            .inner
            .program
            .natives()
            .get("Player")
            .expect("players admitted")
            .source;
        for (native, args, result) in [
            (name, &["player", "real"][..], "nothing"),
            ("GetPlayerHandicapXP", &["player"][..], "real"),
        ] {
            Self::contract(&self.inner, native, args, result, true, Some("player"))?;
            if &self
                .inner
                .program
                .natives()
                .get(native)
                .expect("checked contract")
                .source
                != source
            {
                return Err(self.inner.error(native, Failure::Source));
            }
        }
        self.players
            .as_mut()
            .expect("checked players")
            .experience_handicap = Some(rules);
        Ok(self)
    }

    pub(super) fn experience_handicap_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        if name == "GetPlayerHandicapXP" {
            let [player] = args else {
                return Err(self.inner.error(name, Failure::PlayerArguments));
            };
            if *player == Value::Null {
                return Ok(Value::Real(
                    self.players
                        .as_ref()
                        .expect("players admitted")
                        .experience_handicap
                        .as_ref()
                        .expect("rules admitted")
                        .null_result(),
                ));
            }
            let at = self.player_index(player, name)?;
            return self.players.as_ref().expect("players admitted").rows[at]
                .experience_handicap
                .map(Value::Real)
                .ok_or_else(|| {
                    self.inner.error(
                        name,
                        Failure::PlayerState("initial experience handicap is not derived"),
                    )
                });
        }
        let [player, value] = args else {
            return Err(self.inner.error(name, Failure::PlayerArguments));
        };
        let value = match value {
            Value::Real(v) => *v,
            Value::Int(v) => self
                .numeric
                .as_ref()
                .expect("numeric admission")
                .integer(*v)
                .map_err(|e| self.inner.error(name, Failure::Expression(e)))?,
            _ => return Err(self.inner.error(name, Failure::PlayerArguments)),
        };
        if *player == Value::Null {
            return Ok(Value::Null);
        }
        let at = self.player_index(player, name)?;
        let players = self.players.as_mut().expect("players admitted");
        players.rows[at].experience_handicap = Some(
            players
                .experience_handicap
                .as_ref()
                .expect("rules admitted")
                .clamp(value),
        );
        Ok(Value::Null)
    }
}
