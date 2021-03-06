use crate::serialization::op_code::OpCode;
use crate::types::smethod::SMethod;
use crate::types::stype::SType;

use super::expr::Expr;

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct MethodCall {
    pub obj: Box<Expr>,
    pub method: SMethod,
    pub args: Vec<Expr>,
}

impl MethodCall {
    pub fn tpe(&self) -> &SType {
        self.method.tpe()
    }

    pub fn op_code(&self) -> OpCode {
        OpCode::METHOD_CALL
    }
}
