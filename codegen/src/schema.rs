pub struct Object {
    pub name: String,
    pub fields: Vec<Field>
}

pub struct Field {
    pub name: String,
    pub field_type: GqlType
}

pub struct Argument {
    pub name: String,
    pub argument_type: GqlType,
    pub type_name: String
}

pub struct Enum {
    pub name: String,
    pub values: Vec<String>
}

pub enum GqlType {
    List(Box<GqlType>),
    Object(String),
    Scalar(String),
    Enum(String),
    Nullable(Box<GqlType>),
    Function {
        inputs: Vec<Argument>,
        output: Box<GqlType>
    }
}

pub struct GqlDocument {
    pub inputs: Vec<Object>,
    pub outputs: Vec<Object>,
    pub enums: Vec<Enum>,
    pub scalars: Vec<String>
}

pub const BUILT_IN_SCALARS: &'static [&'static str] = &[ "Int", "String", "Float", "Boolean", "ID" ];
