use std::sync::Arc;

use async_trait::async_trait;
use datafusion::execution::context::{ExecutionProps, SessionState};
use datafusion::logical_expr::{LogicalPlan, UserDefinedLogicalNode};
use datafusion::physical_expr::create_physical_expr;
use datafusion::physical_plan::ExecutionPlan;
use datafusion::physical_planner::{ExtensionPlanner, PhysicalPlanner};
use models::arrow::Schema;
use spi::DFResult;

use crate::extension::logical::plan_node::ts_gen_func::TSGenFuncNode;
use crate::extension::physical::plan_node::ts_gen_func::TSGenFuncExec;
use crate::extension::utils::downcast_plan_node;

pub struct TsGenFuncPlanner;

#[async_trait]
impl ExtensionPlanner for TsGenFuncPlanner {
    /// Create a physical plan for an extension node
    async fn plan_extension(
        &self,
        _planner: &dyn PhysicalPlanner,
        node: &dyn UserDefinedLogicalNode,
        logical_inputs: &[&LogicalPlan],
        physical_inputs: &[Arc<dyn ExecutionPlan>],
        session_state: &SessionState,
    ) -> DFResult<Option<Arc<dyn ExecutionPlan>>> {
        Ok(match downcast_plan_node::<TSGenFuncNode>(node) {
            Some(ts_gen_func) => {
                if physical_inputs.len() != 1 || logical_inputs.len() != 1 {
                    return Err(datafusion::error::DataFusionError::Internal(format!(
                        "TsGenFunc node must have exactly one input, got {}",
                        physical_inputs.len()
                    )));
                }

                let ts_gen_func_exec = plan_ts_gen_func(
                    session_state.execution_props(),
                    ts_gen_func,
                    logical_inputs[0],
                    &physical_inputs[0],
                )?;

                Some(Arc::new(ts_gen_func_exec))
            }
            _ => None,
        })
    }
}

fn plan_ts_gen_func(
    execution_props: &ExecutionProps,
    ts_gen_func: &TSGenFuncNode,
    logical_inputs: &LogicalPlan,
    physical_inputs: &Arc<dyn ExecutionPlan>,
) -> DFResult<TSGenFuncExec> {
    let time_expr = create_physical_expr(
        &ts_gen_func.time_expr,
        logical_inputs.schema(),
        &physical_inputs.schema(),
        execution_props,
    )?;

    let field_exprs = ts_gen_func
        .field_exprs
        .iter()
        .map(|expr| {
            create_physical_expr(
                expr,
                logical_inputs.schema(),
                &physical_inputs.schema(),
                execution_props,
            )
        })
        .collect::<DFResult<Vec<_>>>()?;

    let arg_expr = if let Some(expr) = &ts_gen_func.arg_expr {
        Some(create_physical_expr(
            expr,
            logical_inputs.schema(),
            &physical_inputs.schema(),
            execution_props,
        )?)
    } else {
        None
    };

    let schema = Arc::new(Schema::from(ts_gen_func.schema().as_ref()));

    Ok(TSGenFuncExec::new(
        Arc::clone(physical_inputs),
        time_expr,
        field_exprs,
        arg_expr,
        ts_gen_func.symbol,
        schema,
    ))
}
