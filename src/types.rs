use alloy::sol_types::SolCall;
use revm::primitives::{Account, Address, ExecutionResult, ResultAndState};

#[derive(Debug)]
pub struct CallOutput<S: SolCall> {
    pub execution_result: ExecutionResult,
    pub changes: alloy::primitives::map::HashMap<Address, Account>,
    pub output: Option<S::Return>,
}

pub fn process_transact_result<S: SolCall>(result_and_state: ResultAndState) -> CallOutput<S> {
    let ResultAndState { result, state } = result_and_state;
    let mut call_output = CallOutput::<S> {
        execution_result: result,
        changes: state,
        output: None,
    };
    if let revm::primitives::ExecutionResult::Success { output, .. } = &call_output.execution_result
    {
        if let revm::primitives::Output::Call(data) = output {
            if let Ok(decoded) = S::abi_decode_returns(&data, true) {
                call_output.output = Some(decoded);
            };
        }
    }
    call_output
}
