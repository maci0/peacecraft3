//! Bounded group orders reuse the canonical per-unit command consumers.
use super::{Error, Failure, ObjectHost};
use crate::group_order_rules::Rules;
use pcx_script::Value;

const NATIVES: [(&str, &str, &[&str]); 6] = [
    ("GroupImmediateOrder", "IssueImmediateOrder", &["group", "string"]),
    ("GroupImmediateOrderById", "IssueImmediateOrderById", &["group", "integer"]),
    ("GroupPointOrder", "IssuePointOrder", &["group", "string", "real", "real"]),
    ("GroupPointOrderById", "IssuePointOrderById", &["group", "integer", "real", "real"]),
    ("GroupPointOrderLoc", "IssuePointOrderLoc", &["group", "string", "location"]),
    ("GroupPointOrderByIdLoc", "IssuePointOrderByIdLoc", &["group", "integer", "location"]),
];
impl ObjectHost {
    pub fn with_group_orders(mut self, rules: Rules) -> Result<Self, Error> {
        if self.group_orders.is_some() {
            return Err(self.inner.error("group orders", Failure::GroupState("group orders already admitted")));
        }
        if self.group_limit.is_none() || self.unit_activity.is_none() {
            return Err(self.inner.error("group orders", Failure::GroupState("groups and unit activity must be admitted")));
        }
        let registry=self.order_registry.as_ref().ok_or_else(||self.inner.error("group orders",Failure::GroupState("order registry must be admitted")))?;
        if registry.binding().image_sha256!=rules.image_sha256 || registry.binding().origin!=rules.origin {
            return Err(self.inner.error("group orders",Failure::GroupState("selected image lineage differs from order registry")));
        }
        for (name,unit,args) in NATIVES {
            let Some(native)=self.inner.program.natives().get(name) else {continue};
            Self::contract(&self.inner,name,args,"boolean",false,Some("group"))?;
            let mut unit_args=args.to_vec(); unit_args[0]="unit";
            Self::contract(&self.inner,unit,&unit_args,"boolean",false,Some("unit"))?;
            if self.inner.program.natives().get(unit).is_none_or(|n|n.source!=native.source) {
                return Err(self.inner.error(name,Failure::Signature));
            }
        }
        self.group_orders=Some(rules);
        Ok(self)
    }
    pub(super) fn group_order_native(&mut self,name:&str,args:&[Value])->Option<Result<Value,Error>> {
        let rules=self.group_orders.as_ref()?;
        let (_,unit_name,signature)=NATIVES.iter().find(|(n,_,_)|*n==name)?;
        self.inner.program.natives().get(name)?;
        let limit=if signature.len()>2 {Some(rules.point_member_limit())}else{None};
        Some((|| {
            if args.len()!=signature.len() {return Err(self.inner.error(name,Failure::GroupArguments));}
            let by_id=name.contains("ById");
            let order=match (&args[1],by_id) {
                (Value::Str(s),false)=>self.order_registry.as_ref().and_then(|r|r.id(s)),
                (Value::Int(id),true)=>Some(*id),
                _=>return Err(self.inner.error(name,Failure::GroupArguments)),
            };
            let Some(_)=order else {return Ok(Value::Bool(false))};
            // Native location wrappers resolve the location before the group.
            if name.ends_with("Loc") {
                if args[2]==Value::Null {return Ok(Value::Bool(false));}
                self.location_point(&args[2],name)?;
            } else if signature.len()==4 && !matches!((&args[2],&args[3]),(Value::Real(x),Value::Real(y)) if x.is_finite()&&y.is_finite()) {
                return Err(self.inner.error(name,Failure::GroupArguments));
            }
            if args[0]==Value::Null {return Ok(Value::Bool(false));}
            let group=self.inner.id(self.group_index(&args[0],name)?,name)?;
            // A null-unit probe checks command support without changing canonical state.
            let mut command=[Value::Null,args[1].clone(),Value::Null,Value::Null];
            command[2..args.len()].clone_from_slice(&args[2..]);
            let command=&mut command[..args.len()];
            match self.activity_native(unit_name,command) {
                Some(Ok(Value::Bool(false)))=>{},
                Some(Err(e))=>return Err(self.inner.error(name,e.failure)),
                _=>return Err(self.inner.error(name,Failure::GroupState("per-unit order is not supported"))),
            }
            let mut any=false; let mut visited=0usize;
            // Per-unit order consumers only defer events; they cannot run group-mutating callbacks.
            // Read retained membership order directly, without minting handles or copying groups.
            for at in 0..self.members.len() {
                let (member_group,unit)=self.members[at];
                if member_group!=group {continue;}
                if limit.is_some_and(|max|visited>=max) {break;}
                visited+=1;
                command[0]=Value::Handle(unit);
                match self.activity_native(unit_name,command) {
                    Some(Ok(Value::Bool(accepted)))=>any|=accepted,
                    Some(Err(e))=>return Err(self.inner.error(name,e.failure)),
                    _=>return Err(self.inner.error(name,Failure::GroupState("per-unit order consumer did not return boolean"))),
                }
            }
            Ok(Value::Bool(any))
        })())
    }
}
