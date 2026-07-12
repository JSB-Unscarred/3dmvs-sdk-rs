#[derive(Clone, Debug, PartialEq)]
pub enum ParameterRecord {
    Bool(bool),
    Integer {
        value: i64,
        minimum: i64,
        maximum: i64,
        increment: i64,
    },
    Float {
        value: f32,
        minimum: f32,
        maximum: f32,
    },
    Enumeration {
        value: u32,
        supported: Vec<u32>,
    },
    String {
        value: Vec<u8>,
        maximum_length: u32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParameterValueRecord {
    Bool(bool),
    Integer(i64),
    Float(f32),
    Enumeration(u32),
    String(Vec<u8>),
}
