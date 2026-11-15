//! Data carried by the `sql` interface: an app's own schemas in the
//! Capsule's database, queried and changed with typed rows, and live
//! queries that push a fresh result whenever it changes.

use serde::{Deserialize, Serialize};

use crate::Bytes;

/// One SQL value. On the wire: `null`, `true`, `42`, `4.2`, `"text"`, or
/// `{"bytes":"<base64>"}`. A float is always written with a decimal point,
/// so it reads back as a float.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SqlValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Bytes { bytes: Bytes },
}

impl From<i64> for SqlValue {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}
impl From<f64> for SqlValue {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}
impl From<bool> for SqlValue {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}
impl From<&str> for SqlValue {
    fn from(v: &str) -> Self {
        Self::Text(v.to_string())
    }
}
impl From<String> for SqlValue {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}
impl From<Vec<u8>> for SqlValue {
    fn from(v: Vec<u8>) -> Self {
        Self::Bytes { bytes: Bytes(v) }
    }
}
impl<T: Into<SqlValue>> From<Option<T>> for SqlValue {
    fn from(v: Option<T>) -> Self {
        v.map_or(Self::Null, Into::into)
    }
}

/// A result column. `type` is the database's type name (`integer`, `text`,
/// `timestamp`, …), a string so a newer engine's types never fail to decode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

/// A result set: columns, then rows of values in column order.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Rows {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<SqlValue>>,
}

impl Rows {
    /// The value in `row` under the column named `column`.
    pub fn get(&self, row: usize, column: &str) -> Option<&SqlValue> {
        let index = self.columns.iter().position(|c| c.name == column)?;
        self.rows.get(row)?.get(index)
    }
}

/// One statement of a `sql.batch`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Statement {
    pub sql: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<SqlValue>,
}

/// The same statement whatever its spacing: runs of whitespace become one
/// space. The testkit keys its scripted results by this.
pub fn normalize(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_keep_their_kind() {
        let values: Vec<SqlValue> = vec![
            SqlValue::Null,
            true.into(),
            7i64.into(),
            1.0f64.into(),
            "x".into(),
            vec![1u8, 2].into(),
        ];
        let text = serde_json::to_string(&values).unwrap();
        assert_eq!(text, r#"[null,true,7,1.0,"x",{"bytes":"AQI="}]"#);
        let back: Vec<SqlValue> = serde_json::from_str(&text).unwrap();
        assert_eq!(back, values);
        assert_eq!(SqlValue::from(None::<i64>), SqlValue::Null);
    }

    #[test]
    fn rows_by_name_and_normalized_sql() {
        let rows = Rows {
            columns: vec![Column {
                name: "id".into(),
                kind: "integer".into(),
            }],
            rows: vec![vec![SqlValue::Int(3)]],
        };
        assert_eq!(rows.get(0, "id"), Some(&SqlValue::Int(3)));
        assert_eq!(rows.get(0, "nope"), None);
        assert_eq!(normalize(" select  *\n from t "), "select * from t");
    }
}
