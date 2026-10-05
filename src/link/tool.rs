//! A linked agent used like a tool: `Weather.ask city: "Lisbon"`.

use super::{cycle, interface, resolve::resolve};
use crate::lang::{AgentDef, LinkDecl};
use crate::runtime::Runtime;
use crate::runtime::interpreter::Agent;
use crate::runtime::task::CallContext;
use crate::runtime::value::Value;
use crate::tools::{ActionInfo, Args, ParamInfo, Tool, ToolError};
use async_trait::async_trait;
use std::sync::Arc;

pub struct LinkTool {
    decl: LinkDecl,
    rt: Arc<Runtime>,
    caller: Arc<AgentDef>,
}

impl LinkTool {
    pub fn new(rt: Arc<Runtime>, caller: Arc<AgentDef>, decl: LinkDecl) -> LinkTool {
        LinkTool { decl, rt, caller }
    }

    fn target(&self) -> Result<Arc<AgentDef>, ToolError> {
        resolve(
            &self.rt.linker,
            &self.rt.config.root,
            &self.caller,
            &self.decl.name,
            self.decl.path.as_deref(),
        )
        .map_err(ToolError::from)
    }
}

#[async_trait]
impl Tool for LinkTool {
    fn name(&self) -> &str {
        &self.decl.name
    }

    async fn actions(&self) -> Result<Vec<ActionInfo>, ToolError> {
        let target = self.target()?;
        Ok(target
            .accepts
            .iter()
            .map(|a| ActionInfo {
                name: a.message.clone(),
                description: a.description.clone().unwrap_or_default(),
                params: a
                    .params
                    .iter()
                    .map(|p| ParamInfo {
                        name: p.clone(),
                        required: true,
                    })
                    .collect(),
                schema: None,
            })
            .collect())
    }

    async fn call(&self, action: &str, args: Args, ctx: &CallContext) -> Result<Value, ToolError> {
        let target = self.target()?;
        cycle::check(&ctx.chain, &target.name)?;
        interface::check(&target, action, &args)?;
        let agent = Agent::new(self.rt.clone(), target.clone())?;
        agent.handle(action, args, ctx).await.map_err(|d| {
            let mut related = vec![format!("{} could not answer:", target.name)];
            related.extend(d.render().lines().map(|l| format!("  {}", l)));
            ToolError::new(format!("{} could not answer `{}`", target.name, action))
                .related(related)
        })
    }
}
