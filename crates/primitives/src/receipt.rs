use crate::{Bloom, Log, TxType};
use bytes::{Buf, BufMut, BytesMut};
use reth_codecs::{main_codec, Compact};
use reth_rlp::{length_of_length, Decodable, Encodable};
use std::cmp::Ordering;

/// Receipt containing result of transaction execution.
#[main_codec]
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Receipt {
    /// Receipt type.
    pub tx_type: TxType,
    /// If transaction is executed successfully.
    ///
    /// This is the `statusCode`
    pub success: bool,
    /// Gas used
    pub cumulative_gas_used: u64,
    /// Bloom filter.
    pub bloom: Bloom,
    /// Log send from contracts.
    pub logs: Vec<Log>,
}

impl Receipt {
    /// Returns the rlp header for the receipt payload.
    fn receipt_rlp_header(&self) -> reth_rlp::Header {
        let mut rlp_head = reth_rlp::Header { list: true, payload_length: 0 };

        rlp_head.payload_length += self.success.length();
        rlp_head.payload_length += self.cumulative_gas_used.length();
        rlp_head.payload_length += self.bloom.length();
        rlp_head.payload_length += self.logs.length();

        rlp_head
    }

    /// Encodes the receipt data.
    fn encode_fields(&self, out: &mut dyn BufMut) {
        self.receipt_rlp_header().encode(out);
        self.success.encode(out);
        self.cumulative_gas_used.encode(out);
        self.bloom.encode(out);
        self.logs.encode(out);
    }

    /// Encode receipt with or without the header data.
    pub fn encode_inner(&self, out: &mut dyn BufMut, with_header: bool) {
        if matches!(self.tx_type, TxType::Legacy) {
            self.encode_fields(out);
            return
        }

        let mut payload = BytesMut::new();
        self.encode_fields(&mut payload);

        if with_header {
            let payload_length = payload.len() + 1;
            let header = reth_rlp::Header { list: false, payload_length };
            header.encode(out);
        }

        match self.tx_type {
            TxType::EIP2930 => {
                out.put_u8(0x01);
            }
            TxType::EIP1559 => {
                out.put_u8(0x02);
            }
            _ => unreachable!("legacy handled; qed."),
        }
        out.put_slice(payload.as_ref());
    }

    /// Returns the length of the receipt data.
    fn receipt_length(&self) -> usize {
        let rlp_head = self.receipt_rlp_header();
        length_of_length(rlp_head.payload_length) + rlp_head.payload_length
    }

    /// Decodes the receipt payload
    fn decode_receipt(buf: &mut &[u8], tx_type: TxType) -> Result<Self, reth_rlp::DecodeError> {
        let b = &mut &**buf;
        let rlp_head = reth_rlp::Header::decode(b)?;
        if !rlp_head.list {
            return Err(reth_rlp::DecodeError::UnexpectedString)
        }
        let started_len = b.len();
        let this = Self {
            tx_type,
            success: reth_rlp::Decodable::decode(b)?,
            cumulative_gas_used: reth_rlp::Decodable::decode(b)?,
            bloom: reth_rlp::Decodable::decode(b)?,
            logs: reth_rlp::Decodable::decode(b)?,
        };
        let consumed = started_len - b.len();
        if consumed != rlp_head.payload_length {
            return Err(reth_rlp::DecodeError::ListLengthMismatch {
                expected: rlp_head.payload_length,
                got: consumed,
            })
        }
        *buf = *b;
        Ok(this)
    }
}

impl Encodable for Receipt {
    fn length(&self) -> usize {
        let mut payload_len = self.receipt_length();
        // account for eip-2718 type prefix and set the list
        if matches!(self.tx_type, TxType::EIP1559 | TxType::EIP2930) {
            payload_len += 1;
            // we include a string header for typed receipts, so include the length here
            payload_len = length_of_length(payload_len);
        }

        payload_len
    }
    fn encode(&self, out: &mut dyn BufMut) {
        self.encode_inner(out, true)
    }
}

impl Decodable for Receipt {
    fn decode(buf: &mut &[u8]) -> Result<Self, reth_rlp::DecodeError> {
        // a receipt is either encoded as a string (non legacy) or a list (legacy).
        // We should not consume the buffer if we are decoding a legacy receipt, so let's
        // check if the first byte is between 0x80 and 0xbf.
        let rlp_type = *buf
            .first()
            .ok_or(reth_rlp::DecodeError::Custom("cannot decode a receipt from empty bytes"))?;

        match rlp_type.cmp(&reth_rlp::EMPTY_LIST_CODE) {
            Ordering::Less => {
                // strip out the string header
                let _header = reth_rlp::Header::decode(buf)?;
                let receipt_type = *buf.first().ok_or(reth_rlp::DecodeError::Custom(
                    "typed receipt cannot be decoded from an empty slice",
                ))?;
                if receipt_type == 0x01 {
                    buf.advance(1);
                    Self::decode_receipt(buf, TxType::EIP2930)
                } else if receipt_type == 0x02 {
                    buf.advance(1);
                    Self::decode_receipt(buf, TxType::EIP1559)
                } else {
                    Err(reth_rlp::DecodeError::Custom("invalid receipt type"))
                }
            }
            Ordering::Equal => {
                Err(reth_rlp::DecodeError::Custom("an empty list is not a valid receipt encoding"))
            }
            Ordering::Greater => Self::decode_receipt(buf, TxType::Legacy),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hex_literal::hex, Address, H256};
    use ethers_core::types::Bytes;
    use reth_rlp::{Decodable, Encodable};
    use std::str::FromStr;

    #[test]
    // Test vector from: https://eips.ethereum.org/EIPS/eip-2481
    fn encode_legacy_receipt() {
        let expected = hex!("f901668001b9010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000f85ff85d940000000000000000000000000000000000000011f842a0000000000000000000000000000000000000000000000000000000000000deada0000000000000000000000000000000000000000000000000000000000000beef830100ff");

        let mut data = vec![];
        let receipt = Receipt {
            tx_type: TxType::Legacy,
            bloom: [0; 256].into(),
            cumulative_gas_used: 0x1u64,
            logs: vec![Log {
                address: Address::from_str("0000000000000000000000000000000000000011").unwrap(),
                topics: vec![
                    H256::from_str(
                        "000000000000000000000000000000000000000000000000000000000000dead",
                    )
                    .unwrap(),
                    H256::from_str(
                        "000000000000000000000000000000000000000000000000000000000000beef",
                    )
                    .unwrap(),
                ],
                data: Bytes::from_str("0100ff").unwrap().0,
            }],
            success: false,
        };

        receipt.encode(&mut data);

        // check that the rlp length equals the length of the expected rlp
        assert_eq!(receipt.length(), expected.len());
        assert_eq!(data, expected);
    }

    #[test]
    // Test vector from: https://eips.ethereum.org/EIPS/eip-2481
    fn decode_legacy_receipt() {
        let data = hex!("f901668001b9010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000f85ff85d940000000000000000000000000000000000000011f842a0000000000000000000000000000000000000000000000000000000000000deada0000000000000000000000000000000000000000000000000000000000000beef830100ff");

        // EIP658Receipt
        let expected = Receipt {
            tx_type: TxType::Legacy,
            bloom: [0; 256].into(),
            cumulative_gas_used: 0x1u64,
            logs: vec![Log {
                address: Address::from_str("0000000000000000000000000000000000000011").unwrap(),
                topics: vec![
                    H256::from_str(
                        "000000000000000000000000000000000000000000000000000000000000dead",
                    )
                    .unwrap(),
                    H256::from_str(
                        "000000000000000000000000000000000000000000000000000000000000beef",
                    )
                    .unwrap(),
                ],
                data: Bytes::from_str("0100ff").unwrap().0,
            }],
            success: false,
        };

        let receipt = Receipt::decode(&mut &data[..]).unwrap();
        assert_eq!(receipt, expected);
    }
}
