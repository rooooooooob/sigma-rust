//! Operators in ErgoTree

use super::expr::Expr;
use crate::serialization::op_code::OpCode;
use crate::serialization::sigma_byte_reader::SigmaByteRead;
use crate::serialization::sigma_byte_writer::SigmaByteWrite;
use crate::serialization::SerializationError;
use crate::serialization::SigmaSerializable;
use crate::types::stype::SType;

use std::io::Error;

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Upcast {
    pub input: Box<Expr>,
    pub tpe: SType,
}

impl Upcast {
    pub const OP_CODE: OpCode = OpCode::UPCAST;

    pub fn op_code(&self) -> OpCode {
        Self::OP_CODE
    }

    pub fn tpe(&self) -> SType {
        self.tpe.clone()
    }
}

impl SigmaSerializable for Upcast {
    fn sigma_serialize<W: SigmaByteWrite>(&self, w: &mut W) -> Result<(), Error> {
        self.input.sigma_serialize(w)?;
        self.tpe.sigma_serialize(w)
    }

    fn sigma_parse<R: SigmaByteRead>(r: &mut R) -> Result<Self, SerializationError> {
        let input = Expr::sigma_parse(r)?.into();
        let tpe = SType::sigma_parse(r)?;
        Ok(Upcast { input, tpe })
    }
}

#[cfg(feature = "arbitrary")]
pub mod arbitrary {
    use crate::mir::expr::arbitrary::ArbExprParams;

    use super::*;
    use proptest::prelude::*;

    impl Arbitrary for Upcast {
        type Parameters = ();
        type Strategy = BoxedStrategy<Self>;

        fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
            any_with::<Expr>(ArbExprParams {
                tpe: SType::SInt,
                depth: 2,
            })
            .prop_map(|input| Upcast {
                input: Box::new(input),
                tpe: SType::SLong,
            })
            .boxed()
        }
    }
}

#[cfg(test)]
#[cfg(feature = "arbitrary")]
pub mod proptests {

    use super::*;
    use crate::serialization::sigma_serialize_roundtrip;
    use proptest::prelude::*;

    proptest! {

        #[test]
        fn ser_roundtrip(v in any::<Upcast>()) {
            let expr: Expr = v.into();
            prop_assert_eq![sigma_serialize_roundtrip(&expr), expr];
        }
    }
}