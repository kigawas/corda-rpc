use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryInto;
use std::fmt;
use std::marker::PhantomData;

use oasis_amqp_macros::amqp;
use serde::{self, ser::SerializeTuple, Deserialize, Serialize};
use serde_bytes::{ByteBuf, Bytes};

use crate::{de, Described};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Frame<'a> {
    pub channel: u16,
    #[serde(borrow)]
    pub extended_header: Option<&'a Bytes>,
    pub performative: Performative<'a>,
    pub message: Option<Message<'a>>,
}

impl<'a> Frame<'a> {
    pub(crate) fn decode(doff: u8, buf: &'a [u8]) -> Result<Self, crate::Error> {
        let (channel, buf) = buf.split_at(2);
        let channel =
            u16::from_be_bytes(channel.try_into().map_err(|_| crate::Error::InvalidData)?);

        let (extended, buf) = buf.split_at((doff - 2) as usize);
        let extended_header = if !extended.is_empty() {
            Some(Bytes::new(extended))
        } else {
            None
        };

        let (performative, buf) = de::deserialize(buf)?;
        if !buf.is_empty() {
            return Err(crate::Error::TrailingCharacters);
        }

        Ok(Self {
            channel,
            extended_header,
            performative,
            message: None,
        })
    }
}

#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Message<'a> {
    pub header: Option<Header>,
    #[serde(borrow)]
    pub delivery_annotations: Option<DeliveryAnnotations<'a>>,
    pub message_annotations: Option<MessageAnnotations<'a>>,
    pub properties: Option<Properties<'a>>,
    pub application_properties: Option<ApplicationProperties<'a>>,
    pub body: Option<Body>,
    pub footer: Option<Footer<'a>>,
}

#[amqp(descriptor("amqp:header:list", 0x00000000_00000070))]
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Header {
    pub durable: Option<bool>,
    pub priority: Option<u8>,
    pub ttl: Option<u32>, // ms
    pub first_acquirer: Option<bool>,
    pub delivery_count: Option<u32>,
}

#[amqp(descriptor("amqp:delivery-annotations:map", 0x00000000_00000071))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct DeliveryAnnotations<'a>(#[serde(borrow)] pub HashMap<&'a str, &'a str>);

#[amqp(descriptor("amqp:message-annotations:map", 0x00000000_00000072))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct MessageAnnotations<'a>(#[serde(borrow)] pub HashMap<&'a str, &'a str>);

#[amqp(descriptor("amqp:properties:list", 0x00000000_00000073))]
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Properties<'a> {
    pub message_id: Option<Cow<'a, str>>,
    pub user_id: Option<&'a Bytes>,
    pub to: Option<&'a str>,
    pub subject: Option<&'a str>,
    pub reply_to: Option<Cow<'a, str>>,
    pub correlation_id: Option<&'a str>,
    pub content_type: Option<&'a str>,
    pub content_encoding: Option<&'a str>,
    pub absolute_expiry_time: Option<u32>,
    pub creation_time: Option<u32>,
    pub group_id: Option<&'a str>,
    pub group_sequence: Option<u32>,
    pub reply_to_group_id: Option<&'a str>,
}

#[amqp(descriptor("amqp:application-properties:map", 0x00000000_00000074))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct ApplicationProperties<'a>(#[serde(borrow)] pub HashMap<&'a str, Any<'a>>);

#[amqp]
#[derive(Debug, PartialEq, Serialize)]
pub enum Body {
    Data(Data),
    Sequence(Sequence),
    Value(Value),
}

#[amqp(descriptor("amqp:data:binary", 0x00000000_00000075))]
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Data(pub ByteBuf);

#[amqp(descriptor("amqp:amqp-sequence:list", 0x00000000_00000076))]
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Sequence {}

#[amqp(descriptor("amqp:value:*", 0x00000000_00000077))]
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Value {}

#[amqp(descriptor("amqp:footer:map", 0x00000000_00000078))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Footer<'a>(#[serde(borrow)] pub HashMap<&'a str, &'a str>);

#[allow(clippy::large_enum_variant)]
#[amqp]
#[derive(Debug, PartialEq, Serialize)]
pub enum Performative<'a> {
    Open(Open<'a>),
    Begin(Begin<'a>),
    Attach(Attach<'a>),
    Flow(Flow<'a>),
    Transfer(Transfer),
    Disposition(Disposition),
    Detach(Detach<'a>),
    Close(Close<'a>),
}

#[amqp(descriptor("amqp:open:list", 0x00000000_00000010))]
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Open<'a> {
    pub container_id: &'a str,
    pub hostname: Option<&'a str>,
    pub max_frame_size: Option<u32>,
    pub channel_max: Option<u16>,
    pub idle_timeout: Option<u32>,
    pub outgoing_locales: Option<Vec<&'a str>>,
    pub incoming_locales: Option<Vec<&'a str>>,
    pub offered_capabilities: Option<Vec<&'a str>>,
    pub properties: Option<Vec<(&'a [u8], &'a [u8])>>,
}

#[amqp(descriptor("amqp:begin:list", 0x00000000_00000011))]
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Begin<'a> {
    pub remote_channel: Option<u16>,
    pub next_outgoing_id: u32,
    pub incoming_window: u32,
    pub outgoing_window: u32,
    pub handle_max: Option<u32>,
    #[serde(borrow)]
    pub offered_capabilities: Option<Vec<&'a str>>,
    pub desired_capabilities: Option<Vec<&'a str>>,
    pub properties: Option<Vec<(&'a [u8], &'a [u8])>>,
}

#[amqp(descriptor("amqp:attach:list", 0x00000000_00000012))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Attach<'a> {
    pub name: String,
    pub handle: u32,
    pub role: Role,
    pub snd_settle_mode: Option<SenderSettleMode>,
    pub rcv_settle_mode: Option<ReceiverSettleMode>,
    #[serde(borrow)]
    pub source: Option<Source<'a>>,
    pub target: Option<Target<'a>>,
    pub unsettled: Option<HashMap<&'a Bytes, u32>>,
    pub incomplete_unsettled: Option<bool>,
    pub initial_delivery_count: Option<u32>,
    pub max_message_size: Option<u64>,
    pub offered_capabilities: Option<Vec<&'a str>>,
    pub desired_capabilities: Option<Vec<&'a str>>,
    pub properties: Option<Vec<(&'a Bytes, &'a Bytes)>>,
}

#[amqp(descriptor("amqp:flow:list", 0x00000000_00000013))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Flow<'a> {
    pub next_incoming_id: Option<u32>,
    pub incoming_window: u32,
    pub next_outgoing_id: u32,
    pub outgoing_window: u32,
    pub handle: Option<u32>,
    pub delivery_count: Option<u32>,
    pub link_credit: Option<u32>,
    pub available: Option<u32>,
    pub drain: Option<bool>,
    pub echo: Option<bool>,
    #[serde(borrow)]
    pub properties: Option<Vec<(&'a Bytes, &'a Bytes)>>,
}

#[amqp(descriptor("amqp:transfer:list", 0x00000000_00000014))]
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Transfer {
    pub handle: u32,
    pub delivery_id: Option<u32>,
    pub delivery_tag: Option<ByteBuf>,
    pub message_format: Option<u32>,
    pub settled: Option<bool>,
    pub more: Option<bool>,
    pub rcv_settle_mode: Option<ReceiverSettleMode>,
    pub state: Option<DeliveryState>,
    pub resume: Option<bool>,
    pub aborted: Option<bool>,
    pub batchable: Option<bool>,
}

#[amqp(descriptor("amqp:disposition:list", 0x00000000_00000015))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Disposition {
    pub role: Role,
    pub first: u32,
    pub last: Option<u32>,
    pub settled: Option<bool>,
    pub state: Option<DeliveryState>,
    pub batchable: Option<bool>,
}

#[amqp(descriptor("amqp:detach:list", 0x00000000_00000016))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Detach<'a> {
    pub handle: u32,
    pub closed: Option<bool>,
    #[serde(borrow)]
    pub error: Option<Error<'a>>,
}

#[amqp(descriptor("amqp:close:list", 0x00000000_00000018))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Close<'a> {
    #[serde(borrow)]
    pub error: Option<Error<'a>>,
}

#[amqp(descriptor("amqp:error:list", 0x00000000_0000001d))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Error<'a> {
    #[serde(borrow)]
    condition: &'a str,
    description: &'a str,
    info: Option<Vec<(&'a Bytes, &'a Bytes)>>,
}

#[derive(Debug, PartialEq, Deserialize)]
pub enum Role {
    Sender,
    Receiver,
}

impl Serialize for Role {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bool(match self {
            Role::Sender => false,
            Role::Receiver => true,
        })
    }
}

#[amqp(descriptor("amqp:source:list", 0x00000000_00000028))]
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Source<'a> {
    pub address: Option<String>,
    pub durable: Option<TerminusDurability>,
    pub expiry_policy: Option<ExpiryPolicy>,
    pub timeout: Option<u32>,
    pub dynamic: Option<bool>,
    #[serde(borrow)]
    pub dynamic_node_properties: Option<Vec<(&'a str, &'a str)>>,
    pub distribution_mode: Option<DistributionMode>,
    pub filter: Option<Vec<(&'a str, &'a str)>>,
    pub default_outcome: Option<Outcome>,
    pub outcomes: Option<Vec<&'a str>>,
    pub capabilities: Option<Vec<&'a str>>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub enum TerminusDurability {
    None,
    Configuration,
    UnsettledState,
}

impl Default for TerminusDurability {
    fn default() -> Self {
        TerminusDurability::None
    }
}

impl Serialize for TerminusDurability {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u32(match self {
            TerminusDurability::None => 0,
            TerminusDurability::Configuration => 1,
            TerminusDurability::UnsettledState => 2,
        })
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DistributionMode {
    Move,
    Copy,
}

#[amqp]
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeliveryState {
    Received(Received),
    Accepted(Accepted),
    Rejected(Rejected),
    Released(Released),
    Modified(Modified),
    Declared(Declared),
    TransactionalState(TransactionalState),
}

#[amqp]
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Received(Received),
    Accepted(Accepted),
    Rejected(Rejected),
    Released(Released),
    Modified(Modified),
    Declared(Declared),
}

#[amqp(descriptor("amqp:received:list", 0x00000000_00000023))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Received {}

#[amqp(descriptor("amqp:accepted:list", 0x00000000_00000024))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Accepted {}

#[amqp(descriptor("amqp:rejected:list", 0x00000000_00000025))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Rejected {}

#[amqp(descriptor("amqp:released:list", 0x00000000_00000026))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Released {}

#[amqp(descriptor("amqp:modified:list", 0x00000000_00000027))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Modified {}

#[amqp(descriptor("amqp:declared:list", 0x00000000_00000033))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Declared {}

#[amqp(descriptor("amqp:transactional-state:list", 0x00000000_00000034))]
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct TransactionalState {}

#[amqp(descriptor("amqp:target:list", 0x00000000_00000029))]
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Target<'a> {
    pub address: Option<String>,
    pub durable: Option<u32>,
    pub expiry_policy: Option<ExpiryPolicy>,
    pub timeout: Option<u32>,
    pub dynamic: Option<bool>,
    #[serde(borrow)]
    pub dynamic_node_properties: Option<Vec<(&'a str, &'a str)>>,
    pub capabilities: Option<Vec<&'a str>>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExpiryPolicy {
    LinkDetach,
    SessionEnd,
    ConnectionClose,
    Never,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum SenderSettleMode {
    Unsettled,
    Settled,
    Mixed,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum ReceiverSettleMode {
    First,
    Second,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename = "amqp:symbol")]
pub struct Symbol<'a>(&'a str);

impl<'a> From<&'a str> for Symbol<'a> {
    fn from(s: &'a str) -> Self {
        Symbol(s)
    }
}

impl<'de: 'a, 'a> serde::de::Deserialize<'de> for Symbol<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(SymbolVisitor)
    }
}

struct SymbolVisitor;

impl<'de> serde::de::Visitor<'de> for SymbolVisitor {
    type Value = Symbol<'de>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a symbol")
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct List<T>(pub Vec<T>);

impl<T> Default for List<T> {
    fn default() -> Self {
        List(Vec::new())
    }
}

impl<T> From<Vec<T>> for List<T> {
    fn from(v: Vec<T>) -> Self {
        List(v)
    }
}

impl<T> Serialize for List<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_tuple(self.0.len())?;
        for elem in self.0.iter() {
            s.serialize_element(elem)?;
        }
        s.end()
    }
}

#[derive(Debug, PartialEq, Serialize)]
pub enum Any<'a> {
    None,
    Bool(bool),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Bytes(&'a [u8]),
    Symbol(Cow<'a, str>),
    Str(Cow<'a, str>),
}

impl<'a, 'de: 'a> Deserialize<'de> for Any<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        enum AnyType {
            I8,
        }

        struct FieldVisitor;

        impl<'de: 'a, 'a> serde::de::Visitor<'de> for FieldVisitor {
            type Value = AnyType;
            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt::Formatter::write_str(fmt, "variant identifier")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0x51 => Ok(AnyType::I8),
                    _ => Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"constructor code",
                    )),
                }
            }
        }

        impl<'de> serde::Deserialize<'de> for AnyType {
            #[inline]
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                serde::Deserializer::deserialize_identifier(deserializer, FieldVisitor)
            }
        }

        struct Visitor<'de, 'a> {
            marker: PhantomData<Any<'a>>,
            lifetime: PhantomData<&'de ()>,
        }

        impl<'de: 'a, 'a> serde::de::Visitor<'de> for Visitor<'de, 'a> {
            type Value = Any<'a>;

            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt::Formatter::write_str(fmt, "enum Any")
            }

            fn visit_enum<A>(self, data: A) -> serde::export::Result<Self::Value, A::Error>
            where
                A: serde::de::EnumAccess<'de>,
            {
                let val = match serde::de::EnumAccess::variant(data) {
                    Ok(val) => val,
                    Err(err) => return Err(err),
                };

                match val {
                    (AnyType::I8, variant) => Result::map(
                        serde::de::VariantAccess::newtype_variant::<i8>(variant),
                        Any::I8,
                    ),
                }
            }
        }

        const VARIANTS: &[&'static str] = &[
            "I8",
        ];
        serde::Deserializer::deserialize_enum(deserializer, "Any", VARIANTS, Visitor {
            marker: PhantomData::default(),
            lifetime: PhantomData::default(),
        })
    }
}
