//! JSON serialization

use serde::Serializer;

pub fn serialize_bytes<S, T>(bytes: T, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: AsRef<[u8]>,
{
    serializer.serialize_str(&base16::encode_lower(bytes.as_ref()))
}

pub mod ergo_tree {

    use super::*;
    use crate::ergo_tree::ErgoTree;
    use crate::serialization::SigmaSerializable;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(ergo_tree: &ErgoTree, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes = ergo_tree.sigma_serialize_bytes();
        serialize_bytes(&bytes[..], serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ErgoTree, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        String::deserialize(deserializer)
            .and_then(|str| base16::decode(&str).map_err(|err| Error::custom(err.to_string())))
            .and_then(|bytes| {
                ErgoTree::sigma_parse_bytes(bytes).map_err(|error| Error::custom(error.to_string()))
            })
    }
}

pub mod ergo_box {
    use core::fmt;
    use serde::de::{self, MapAccess, Visitor};
    use serde::Deserializer;
    use std::convert::TryFrom;
    use std::marker::PhantomData;
    use std::str::FromStr;

    extern crate derive_more;
    use derive_more::From;

    use crate::ast::constant::Constant;
    use crate::chain::Base16DecodedBytes;
    use crate::serialization::SerializationError;
    use crate::serialization::SigmaSerializable;
    use crate::{
        chain::{
            ergo_box::{BoxId, BoxValue, NonMandatoryRegisters},
            token::Token,
            transaction::TxId,
        },
        ergo_tree::ErgoTree,
    };
    use serde::Deserialize;

    #[derive(Deserialize, PartialEq, Eq, Debug, Clone)]
    pub struct ErgoBoxFromJson {
        #[serde(rename = "boxId", alias = "id")]
        pub box_id: BoxId,
        /// amount of money associated with the box
        #[serde(rename = "value")]
        pub value: BoxValue,
        /// guarding script, which should be evaluated to true in order to open this box
        #[serde(rename = "ergoTree", with = "super::ergo_tree")]
        pub ergo_tree: ErgoTree,
        /// secondary tokens the box contains
        #[serde(rename = "assets")]
        pub tokens: Vec<Token>,
        ///  additional registers the box can carry over
        #[serde(rename = "additionalRegisters")]
        pub additional_registers: NonMandatoryRegisters,
        /// height when a transaction containing the box was created.
        /// This height is declared by user and should not exceed height of the block,
        /// containing the transaction with this box.
        #[serde(rename = "creationHeight")]
        pub creation_height: u32,
        /// id of transaction which created the box
        #[serde(rename = "transactionId", alias = "txId")]
        pub transaction_id: TxId,
        /// number of box (from 0 to total number of boxes the transaction with transactionId created - 1)
        #[serde(rename = "index")]
        pub index: u16,
    }

    #[derive(Deserialize, PartialEq, Eq, Debug, Clone)]
    pub struct ConstantHolder(
        #[serde(deserialize_with = "constant_as_string_or_struct")] RichConstant,
    );

    impl From<ConstantHolder> for Constant {
        fn from(ch: ConstantHolder) -> Self {
            ch.0.raw_value
        }
    }

    #[derive(Deserialize, PartialEq, Eq, Debug, Clone)]
    struct RichConstant {
        #[serde(rename = "rawValue")]
        raw_value: Constant,
    }

    use thiserror::Error;

    #[derive(Error, PartialEq, Eq, Debug, Clone, From)]
    pub enum ConstantParsingError {
        #[error("Base16 decoding error: {0}")]
        DecodeError(base16::DecodeError),
        #[error("Deserialization error: {0}")]
        DeserializationError(SerializationError),
    }

    impl FromStr for RichConstant {
        type Err = ConstantParsingError;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let bytes = Base16DecodedBytes::try_from(s)?;
            let c = Constant::sigma_parse_bytes(bytes.into())?;
            Ok(RichConstant { raw_value: c })
        }
    }

    // via https://serde.rs/string-or-struct.html
    pub fn constant_as_string_or_struct<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: Deserialize<'de> + FromStr<Err = ConstantParsingError>,
        D: Deserializer<'de>,
    {
        // This is a Visitor that forwards string types to T's `FromStr` impl and
        // forwards map types to T's `Deserialize` impl. The `PhantomData` is to
        // keep the compiler from complaining about T being an unused generic type
        // parameter. We need T in order to know the Value type for the Visitor
        // impl.
        struct StringOrStruct<T>(PhantomData<fn() -> T>);

        impl<'de, T> Visitor<'de> for StringOrStruct<T>
        where
            T: Deserialize<'de> + FromStr<Err = ConstantParsingError>,
        {
            type Value = T;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("string or map")
            }

            fn visit_str<E>(self, value: &str) -> Result<T, E>
            where
                E: de::Error,
            {
                FromStr::from_str(value)
                    .map_err(|_| de::Error::custom("error parsing constant from string: {value}"))
            }

            fn visit_map<M>(self, map: M) -> Result<T, M::Error>
            where
                M: MapAccess<'de>,
            {
                // `MapAccessDeserializer` is a wrapper that turns a `MapAccess`
                // into a `Deserializer`, allowing it to be used as the input to T's
                // `Deserialize` implementation. T then deserializes itself using
                // the entries from the map visitor.
                Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))
            }
        }

        deserializer.deserialize_any(StringOrStruct(PhantomData))
    }
}

pub mod transaction {
    use crate::chain::transaction::{DataInput, Input, UnsignedInput};
    use crate::chain::{ergo_box::ErgoBox, transaction::TxId};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
    pub struct TransactionJson {
        #[cfg_attr(feature = "json", serde(rename = "id"))]
        pub tx_id: TxId,
        /// inputs, that will be spent by this transaction.
        #[cfg_attr(feature = "json", serde(rename = "inputs"))]
        pub inputs: Vec<Input>,
        /// inputs, that are not going to be spent by transaction, but will be reachable from inputs
        /// scripts. `dataInputs` scripts will not be executed, thus their scripts costs are not
        /// included in transaction cost and they do not contain spending proofs.
        #[cfg_attr(feature = "json", serde(rename = "dataInputs"))]
        pub data_inputs: Vec<DataInput>,
        #[cfg_attr(feature = "json", serde(rename = "outputs"))]
        pub outputs: Vec<ErgoBox>,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
    pub struct UnsignedTransactionJson {
        #[cfg_attr(feature = "json", serde(rename = "id"))]
        pub tx_id: TxId,
        /// inputs, that will be spent by this transaction.
        #[cfg_attr(feature = "json", serde(rename = "inputs"))]
        pub inputs: Vec<UnsignedInput>,
        /// inputs, that are not going to be spent by transaction, but will be reachable from inputs
        /// scripts. `dataInputs` scripts will not be executed, thus their scripts costs are not
        /// included in transaction cost and they do not contain spending proofs.
        #[cfg_attr(feature = "json", serde(rename = "dataInputs"))]
        pub data_inputs: Vec<DataInput>,
        // we're using ErgoBox here instead of ErgoBoxCandidate,
        // since ErgoBox is expected here in the node REST API
        #[cfg_attr(feature = "json", serde(rename = "outputs"))]
        pub outputs: Vec<ErgoBox>,
    }
}

#[cfg(test)]
mod tests {
    use crate::chain::transaction::unsigned::UnsignedTransaction;
    use crate::sigma_protocol::prover::ContextExtension;
    use std::convert::TryInto;

    use super::super::ergo_box::*;
    use super::super::transaction::*;
    use super::*;
    use proptest::prelude::*;

    proptest! {

        #[test]
        fn ergo_box_roundtrip(b in any::<ErgoBox>()) {
            let j = serde_json::to_string(&b)?;
            // eprintln!("{}", j);
            let b_parsed: ErgoBox = serde_json::from_str(&j)?;
            prop_assert_eq![b, b_parsed];
        }

        #[test]
        fn tx_roundtrip(t in any::<Transaction>()) {
            let j = serde_json::to_string(&t)?;
            // dbg!(j);
            eprintln!("{}", j);
            let t_parsed: Transaction = serde_json::from_str(&j)?;
            prop_assert_eq![t, t_parsed];
        }

        #[test]
        fn unsigned_tx_roundtrip(t in any::<UnsignedTransaction>()) {
            let j = serde_json::to_string(&t)?;
            // dbg!(j);
            eprintln!("{}", j);
            let t_parsed: UnsignedTransaction = serde_json::from_str(&j)?;
            prop_assert_eq![t, t_parsed];
        }

    }

    #[test]
    fn parse_registers() {
        let json = r#"
        {"R4":"05b0b5cad8e6dbaef44a","R5":"048ce5d4e505"}
        "#;
        let regs: NonMandatoryRegisters = serde_json::from_str(json).unwrap();
        assert_eq!(regs.get_ordered_values().len(), 2)
    }

    #[test]
    fn parse_registers_explorer_api_v2() {
        let json = r#"
            {
                "R4": {
                    "decodedValue": "Coll(-89,30,-127,32,-20,-100,-42,0,-25,-9,-25,107,-100,27,10,-97,127,127,-93,109,-48,70,51,-111,27,85,107,-116,97,102,87,45)",
                    "valueType": "Coll[Byte]",
                    "rawValue": "0e20a71e8120ec9cd600e7f7e76b9c1b0a9f7f7fa36dd04633911b556b8c6166572d"
                }
            }
        "#;
        let regs: NonMandatoryRegisters = serde_json::from_str(json).unwrap();
        assert!(regs.get(NonMandatoryRegisterId::R4).is_some());
    }

    #[test]
    fn parse_registers_error() {
        let json = r#"
        {"R4":"0"}
        "#;
        let regs: Result<NonMandatoryRegisters, _> = serde_json::from_str(json);
        assert!(regs.is_err());
    }

    #[test]
    fn parse_ergo_tree_with_constants() {
        let json = r#"
            {"boxId":"dd4e69ae683d7c2d1de2b3174182e6c443fd68abbcc24002ddc99adb599e0193","value":1000000,"ergoTree":"0008cd03f1102eb87a4166bf9fbd6247d087e92e1412b0e819dbb5fbc4e716091ec4e4ec","assets":[],"creationHeight":268539,"additionalRegisters":{},"transactionId":"8204d2bbaabf946f89a27b366d1356eb10241dc1619a70b4e4a4a38b520926ce","index":0}
        "#;
        let b: ergo_box::ErgoBoxFromJson = serde_json::from_str(json).unwrap();
        assert!(b.ergo_tree.proposition().is_ok())
    }

    #[test]
    fn parse_empty_context_extension() {
        let c: ContextExtension = serde_json::from_str("{}").unwrap();
        assert_eq!(c, ContextExtension::empty());
    }

    #[test]
    fn parse_context_extension() {
        let json = r#"
        {"1" :"05b0b5cad8e6dbaef44a", "3":"048ce5d4e505"}
        "#;
        let c: ContextExtension = serde_json::from_str(json).unwrap();
        assert_eq!(c.values.len(), 2);
        assert!(c.values.get(&1u8).is_some());
        assert!(c.values.get(&3u8).is_some());
    }

    #[test]
    fn parse_ergo_box() {
        let box_json = r#"{
          "boxId": "e56847ed19b3dc6b72828fcfb992fdf7310828cf291221269b7ffc72fd66706e",
          "value": 67500000000,
          "ergoTree": "100204a00b08cd021dde34603426402615658f1d970cfa7c7bd92ac81a8b16eeebff264d59ce4604ea02d192a39a8cc7a70173007301",
          "assets": [],
          "creationHeight": 284761,
          "additionalRegisters": {},
          "transactionId": "9148408c04c2e38a6402a7950d6157730fa7d49e9ab3b9cadec481d7769918e9",
          "index": 1
        }"#;
        let b: ErgoBox = serde_json::from_str(box_json).unwrap();
        assert_eq!(b.value, 67500000000u64.try_into().unwrap());
    }

    #[test]
    fn parse_ergo_box_alternative_box_id_field_name() {
        // check that using "id" field name instead of "boxId" also works
        // used in explorer API
        let box_json = r#"{
          "id": "e56847ed19b3dc6b72828fcfb992fdf7310828cf291221269b7ffc72fd66706e",
          "value": 67500000000,
          "ergoTree": "100204a00b08cd021dde34603426402615658f1d970cfa7c7bd92ac81a8b16eeebff264d59ce4604ea02d192a39a8cc7a70173007301",
          "assets": [],
          "creationHeight": 284761,
          "additionalRegisters": {},
          "transactionId": "9148408c04c2e38a6402a7950d6157730fa7d49e9ab3b9cadec481d7769918e9",
          "index": 1
        }"#;
        let b: ErgoBox = serde_json::from_str(box_json).unwrap();
        assert_eq!(b.value, 67500000000u64.try_into().unwrap());
    }

    #[test]
    fn parse_ergo_box_alternative_transaction_id_field_name() {
        // check that using "txId" field name instead of "transactionId" also works
        // used in explorer API
        let box_json = r#"{
          "id": "e56847ed19b3dc6b72828fcfb992fdf7310828cf291221269b7ffc72fd66706e",
          "value": 67500000000,
          "ergoTree": "100204a00b08cd021dde34603426402615658f1d970cfa7c7bd92ac81a8b16eeebff264d59ce4604ea02d192a39a8cc7a70173007301",
          "assets": [],
          "creationHeight": 284761,
          "additionalRegisters": {},
          "txId": "9148408c04c2e38a6402a7950d6157730fa7d49e9ab3b9cadec481d7769918e9",
          "index": 1
        }"#;
        let b: ErgoBox = serde_json::from_str(box_json).unwrap();
        assert_eq!(b.value, 67500000000u64.try_into().unwrap());
    }

    #[test]
    fn parse_ergo_box_from_explorer() {
        let box_json = r#"
        {
            "id": "3e762407d99b006d53b6583adcca08ef690b42fb0b2ed7abf63179eb6b9033b2",
            "txId": "93d344aa527e18e5a221db060ea1a868f46b61e4537e6e5f69ecc40334c15e38",
            "value": 2875858910,
            "index": 0,
            "creationHeight": 352126,
            "ergoTree": "101f0400040004020402040004000402050005000580dac4090580dac409050005c00c05c80104000e20b662db51cf2dc39f110a021c2a31c74f0a1a18ffffbf73e8a051a7b8c0f09ebc0580dac40904040404050005feffffffffffffffff01050005e807050005e807050005a0060101050005c00c05a006d81ed601b2db6501fe730000d602b2a5730100d603c17202d604db6308a7d605b27204730200d6068c720502d607db63087202d608b27207730300d6098c720802d60a9472067209d60bb27204730400d60c8c720b02d60db27207730500d60e8c720d02d60f94720c720ed610e4c6a70505d611e4c672020505d612e4c6a70405d613e4c672020405d614b2a5730600d615e4c672140405d61695720a73077215d61795720a72157308d61899c1a77309d619e4c672140505d61a997203730ad61be4c672010405d61ca172189c7212721bd61d9c7213721bd61e9593721d730b730c9d9c721a730d721dd1ededed938cb2db63087201730e0001730fedededed9272037310edec720a720fefed720a720fed939a720672109a72097211939a720c72129a720e7213eded939a721272167213939a721072177211939a72187219721aeded938c720d018c720b01938c7208018c720501938cb27207731100018cb272047312000193721995720f9ca1721b95937212731373149d721c72127216d801d61f997218721c9c9593721f7315731695937210731773189d721f7210721795720f95917216731992721e731a731b95917217731c90721e731d92721e731e",
            "address": "9aFbqNsmDwSxCdcLDKmSxVTL58ms2A39Rpn2zodVzkBN5MzB8zvW5PFX551W1A5vUdFJ3yxwvwgYTTS4JrPQcb5qxBbRDJkGNikuqHRXhnbniK4ajumEj7ot2o7DbcNFaM674fWufQzSGS1KtgMw95ZojyqhswUNbKpYDV1PhKw62bEMdJL9vAvzea4KwKXGUTdYYkcPdQKFWXfrdo2nTS3ucFNxqyTRB3VtZk7AWE3eeNHFcXZ1kLkfrX1ZBjpQ7qrBemHk4KZgS8fzmm6hPSZThiVVtBfQ2CZhJQdAZjRwGrw5TDcZ4BBDAZxg9h13vZ7tQSPsdAtjMFQT1DxbqAruKxX38ZwaQ3UfWmbBpbJEThAQaS4gsCBBSjswrv8BvupxaHZ4oQmA2LZiz4nYaPr8MJtR4fbM9LErwV4yDVMb873bRE5TBF59NipUyHAir7ysajPjbGc8aRLqsMVjntFSCFYx7822RBrj7RRX11CpiGK6vdfKHe3k14EH6YaNXvGSq8DrfNHEK4SgreknTqCgjL6i3EMZKPCW8Lao3Q5tbJFnFjEyntpUDf5zfGgFURxzobeEY4USqFaxyppHkgLjQuFQtDWbYVu3ztQL6hdWHjZXMK4VVvEDeLd1woebD1CyqS5kJHpGa78wQZ4iKygw4ijYrodZpqqEwTXdqwEB6xaLfkxZCBPrYPST3xz67GGTBUFy6zkXP5vwVVM5gWQJFdWCZniAAzBpzHeVq1yzaBp5GTJgr9bfrrAmuX8ra1m125yfeT9sTWroVu",
            "assets": [
                {
                    "tokenId": "2d554219a80c011cc51509e34fa4950965bb8e01de4d012536e766c9ca08bc2c",
                    "index": 0,
                    "amount": 99999999998
                },
                {
                    "tokenId": "bcd5db3a2872f279ef89edaa51a9344a6095ea1f03396874b695b5ba95ff602e",
                    "index": 1,
                    "amount": 99995619990
                },
                {
                    "tokenId": "9f90c012e03bf99397e363fb1571b7999941e0862a217307e3467ee80cf53af7",
                    "index": 2,
                    "amount": 1
                }
            ],
            "additionalRegisters": {
                "R4": "0504",
                "R5": "05d4d59604"
            },
            "spentTransactionId": null,
            "mainChain": true
        }
        "#;
        let b: ErgoBox = serde_json::from_str(box_json).unwrap();
        assert_eq!(b.value, 2875858910u64.try_into().unwrap());
    }
}
