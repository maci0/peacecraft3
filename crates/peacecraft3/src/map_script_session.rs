//! Persistent execution of one admitted map with an explicitly supplied Host.
use bevy::prelude::Resource;
use pcx_play::map_admission::ParsedMapContext;
use pcx_play::scripts::{AssetIdentity, OwnedLocalizedHost};
use pcx_script::{Error, Host, OwnedVm, Value, value::FunctionId};
use std::sync::{Arc, Weak};

#[derive(Debug, thiserror::Error)]
#[error("{diagnostic}")]
pub struct ScriptExecutionFailure {
    pub identity: AssetIdentity,
    /// None denotes global initialization before the explicit entrypoint list.
    pub function: Option<FunctionId>,
    pub source_label: Option<String>,
    #[source]
    pub error: Error,
    pub diagnostic: String,
}
#[derive(Debug, thiserror::Error)]
pub enum ScriptSessionError {
    #[error("no script session has been initialized")]
    NoSession,
    #[error("{identity:?}: function id {id:?} is not declared in the admitted program")]
    UnknownFunction {
        identity: AssetIdentity,
        id: FunctionId,
    },
    #[error("{identity:?}: candidate Host construction failed: {source}")]
    Factory {
        identity: AssetIdentity,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("callback function id {id:?} belongs to another or replaced session")]
    StaleCallback { id: FunctionId },
    #[error(transparent)]
    Execution(#[from] Box<ScriptExecutionFailure>),
}
/// A callback selected for one published session, including its initialization.
/// Holding a ticket retains no discarded map, VM or Host.
#[derive(Clone, Debug)]
pub struct CallbackTicket {
    session: Weak<()>,
    function: FunctionId,
}
struct ActiveSession<H: Host> {
    witness: Arc<()>,
    context: Arc<ParsedMapContext>,
    vm: OwnedVm<OwnedLocalizedHost<H>>,
}
#[derive(Resource)]
pub struct MapScriptSession<H: Host + Send + Sync + 'static> {
    active: Option<ActiveSession<H>>,
}
impl<H: Host + Send + Sync + 'static> Default for MapScriptSession<H> {
    fn default() -> Self {
        Self { active: None }
    }
}
impl<H: Host + Send + Sync + 'static> MapScriptSession<H> {
    pub fn context(&self) -> Option<&Arc<ParsedMapContext>> {
        self.active.as_ref().map(|active| &active.context)
    }
    /// Published-session witness. [`CallbackTicket`] stores this same `Arc`.
    /// Successful `replace` allocates the next witness before the previous one drops.
    /// `clear` drops it.
    pub fn session_witness(&self) -> Option<Weak<()>> {
        self.active
            .as_ref()
            .map(|active| Arc::downgrade(&active.witness))
    }
    pub fn host(&mut self) -> Option<&mut H> {
        self.active
            .as_mut()
            .map(|active| &mut active.vm.host().inner)
    }
    pub fn global(&self, name: &str) -> Option<Value> {
        self.active.as_ref().map(|active| active.vm.global(name))
    }
    /// The caller must isolate candidate Host effects from the active session.
    /// Failure preserves this resource, but cannot undo arbitrary external effects.
    /// An empty explicit entrypoint list initializes only globals.
    pub fn replace<F, E>(
        &mut self,
        context: Arc<ParsedMapContext>,
        create_host: F,
        entrypoints: &[FunctionId],
    ) -> Result<(), ScriptSessionError>
    where
        F: FnOnce(Arc<ParsedMapContext>) -> Result<H, E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        self.replace_sequenced(context, create_host, entrypoints, |_, _| {
            Ok::<(), std::convert::Infallible>(())
        })
    }
    /// `between(vm, completed)` runs after globals (`completed == 0`) and after
    /// each entrypoint that returned. A between-callback error is a factory
    /// failure: the previous session stays published.
    pub fn replace_sequenced<F, E, B, BE>(
        &mut self,
        context: Arc<ParsedMapContext>,
        create_host: F,
        entrypoints: &[FunctionId],
        mut between: B,
    ) -> Result<(), ScriptSessionError>
    where
        F: FnOnce(Arc<ParsedMapContext>) -> Result<H, E>,
        E: std::error::Error + Send + Sync + 'static,
        B: FnMut(&mut OwnedVm<OwnedLocalizedHost<H>>, usize) -> Result<(), BE>,
        BE: std::error::Error + Send + Sync + 'static,
    {
        for &id in entrypoints {
            function_name(&context, id)?;
        }
        let host =
            create_host(Arc::clone(&context)).map_err(|source| ScriptSessionError::Factory {
                identity: context.sources.map.clone(),
                source: Box::new(source),
            })?;
        let mut vm = Arc::clone(&context.scripts).owned_vm(host);
        if let Err(error) = vm.init_globals() {
            return Err(execution_failure(&context, &mut vm, None, error));
        }
        if let Err(source) = between(&mut vm, 0) {
            return Err(ScriptSessionError::Factory {
                identity: context.sources.map.clone(),
                source: Box::new(source),
            });
        }
        for (index, &id) in entrypoints.iter().enumerate() {
            let name = function_name(&context, id)?;
            if let Err(error) = vm.call(&name, &[]) {
                return Err(execution_failure(&context, &mut vm, Some(id), error));
            }
            if let Err(source) = between(&mut vm, index + 1) {
                return Err(ScriptSessionError::Factory {
                    identity: context.sources.map.clone(),
                    source: Box::new(source),
                });
            }
        }
        self.active = Some(ActiveSession {
            witness: Arc::new(()),
            context,
            vm,
        });
        Ok(())
    }
    /// Mint immediately when binding a function from this active context.
    /// Do not rebind a queued bare ID after replacement; its origin is not encoded.
    pub fn callback_ticket(&self, id: FunctionId) -> Result<CallbackTicket, ScriptSessionError> {
        let active = self.active.as_ref().ok_or(ScriptSessionError::NoSession)?;
        function_name(&active.context, id)?;
        Ok(CallbackTicket {
            session: Arc::downgrade(&active.witness),
            function: id,
        })
    }
    /// Reject other/replaced session tickets before any VM or Host execution.
    pub fn call_callback(
        &mut self,
        ticket: &CallbackTicket,
        args: &[Value],
    ) -> Result<Value, ScriptSessionError> {
        let Some(active) = &self.active else {
            return Err(ScriptSessionError::StaleCallback {
                id: ticket.function,
            });
        };
        if !Weak::ptr_eq(&ticket.session, &Arc::downgrade(&active.witness)) {
            return Err(ScriptSessionError::StaleCallback {
                id: ticket.function,
            });
        }
        self.call_function(ticket.function, args)
    }
    /// Explicit direct calls must select IDs and arguments for the active context.
    /// Queued callbacks use call_callback and retain their original ticket.
    pub fn call_function(
        &mut self,
        id: FunctionId,
        args: &[Value],
    ) -> Result<Value, ScriptSessionError> {
        let active = self.active.as_mut().ok_or(ScriptSessionError::NoSession)?;
        let name = function_name(&active.context, id)?;
        active
            .vm
            .call(&name, args)
            .map_err(|error| execution_failure(&active.context, &mut active.vm, Some(id), error))
    }
    pub fn clear(&mut self) {
        self.active = None;
    }
}
fn function_name(context: &ParsedMapContext, id: FunctionId) -> Result<String, ScriptSessionError> {
    context
        .scripts
        .program()
        .functions()
        .get(id.index())
        .map(|function| function.name.clone())
        .ok_or_else(|| ScriptSessionError::UnknownFunction {
            identity: context.sources.map.clone(),
            id,
        })
}
fn execution_failure<H: Host>(
    context: &ParsedMapContext,
    vm: &mut OwnedVm<OwnedLocalizedHost<H>>,
    function: Option<FunctionId>,
    error: Error,
) -> ScriptSessionError {
    let source_label = vm.error_source().map(str::to_owned);
    let diagnostic = vm.host().diagnostic(source_label.as_deref(), &error);
    ScriptSessionError::Execution(Box::new(ScriptExecutionFailure {
        identity: context.sources.map.clone(),
        function,
        source_label,
        error,
        diagnostic,
    }))
}
