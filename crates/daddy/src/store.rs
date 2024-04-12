use crate::{
    niches::integer::{Integer, IntegerSerError},
    utilities::cursor::Cursor,
    values::{Value, ValueSerError},
    version::{Version, VersionSerError},
};
use alloc::{collections::BTreeMap, vec, vec::Vec};
use core::ops::{Index, IndexMut};

#[derive(Debug)]
pub struct Store {
    version: Version,
    kvs: BTreeMap<Value, Value>,
}

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub enum StoreFailure {
    ValueError(ValueSerError),
    IntegerError(IntegerSerError),
    VersionError(VersionSerError),
    CouldntFindKey,
    FileTooLong,
}

impl From<ValueSerError> for StoreFailure {
    fn from(value: ValueSerError) -> Self {
        Self::ValueError(value)
    }
}
impl From<IntegerSerError> for StoreFailure {
    fn from(value: IntegerSerError) -> Self {
        Self::IntegerError(value)
    }
}
impl From<VersionSerError> for StoreFailure {
    fn from(value: VersionSerError) -> Self {
        Self::VersionError(value)
    }
}

impl Default for Store {
    fn default() -> Self {
        Self {
            version: Version::V0_1_0,
            kvs: BTreeMap::new(),
        }
    }
}

impl Store {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, k: Value, v: Value) {
        self.kvs.insert(k, v);
    }

    ///format:
    ///
    /// 10 bytes: title
    /// 1 byte: \0
    /// 6 bytes: version
    /// 1 byte: \0
    /// 4 bytes: size text
    /// 1 byte: \0
    /// 8 bytes: size
    /// 1 byte: \0
    ///
    /// keys:
    ///     8 bytes: `key_length`
    ///     8 bytes: `value_length`
    ///     `key_length` bytes: content
    ///
    /// values:
    ///     see value serialisations lol
    ///     NB: same order as keys
    pub fn ser(self) -> Result<Vec<u8>, StoreFailure> {
        let mut res = vec![];
        res.extend(b"DADDYSTORE".iter());
        res.push(0);
        res.extend(self.version.to_bytes().iter());
        res.push(0);

        let length = self.kvs.len();
        res.extend(b"SIZE".iter());
        res.push(0);
        res.extend(Integer::usize(length).ser());
        res.push(0);

        let mut keys: Vec<u8> = vec![];
        let mut values: Vec<u8> = vec![];

        for (k, v) in self.kvs {
            let ser_key = k.serialise()?;
            let ser_value = v.serialise()?;

            keys.extend(Integer::usize(ser_key.len()).ser());
            keys.extend(Integer::usize(ser_value.len()).ser());
            keys.extend(ser_key.iter());

            values.extend(ser_value.iter());
        }

        res.extend(keys);
        res.extend(values);

        Ok(res)
    }

    pub fn deser(bytes: &[u8]) -> Result<Self, StoreFailure> {
        let mut bytes = Cursor::new(&bytes).ok_or(StoreFailure::FileTooLong)?;

        bytes.seek(10); //title
        bytes.seek(1); //\0

        let version = Version::from_bytes(&mut bytes)?;

        match version {
            Version::V0_1_0 => {
                struct Val {
                    value_length: usize,
                    key: Value,
                }

                bytes.seek(1); //\0
                bytes.seek(4); //size
                bytes.seek(1); //\0

                let length: usize = Integer::deser(&mut bytes)?.try_into()?;

                bytes.seek(1); //\0

                let mut keys = vec![];
                for _ in 0..length {
                    let key_length: usize = Integer::deser(&mut bytes)?.try_into()?;
                    let value_length: usize = Integer::deser(&mut bytes)?.try_into()?;

                    let key = Value::deserialise(&mut bytes, key_length)?;
                    keys.push(Val { value_length, key });
                }

                let mut kvs = BTreeMap::new();
                for Val { value_length, key } in keys {
                    let value = Value::deserialise(&mut bytes, value_length)?;
                    kvs.insert(key, value);
                }

                Ok(Self { version, kvs })
            }
        }
    }
}

impl Index<Value> for Store {
    type Output = Value;

    fn index(&self, index: Value) -> &Self::Output {
        &self.kvs[&index]
    }
}
impl IndexMut<Value> for Store {
    fn index_mut(&mut self, index: Value) -> &mut Self::Output {
        self.kvs
            .get_mut(&index)
            .unwrap_or_else(|| panic!("key not found"))
    }
}