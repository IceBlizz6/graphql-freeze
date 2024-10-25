use serde::Deserialize;
use serde_json::Deserializer;
use serde_path_to_error::deserialize;
use crate::schema;
use crate::schema::{ GqlDocument, Argument, Object, GqlType, Enum};

pub fn from_response_body(response_body: &str) -> GqlDocument {
    let deserializer = &mut Deserializer::from_str(response_body);
    let response: IntrospectionQueryResponse = deserialize(deserializer).unwrap();
    let types = response.data.schema.types;

    let mut enums: Vec<Enum> = Vec::new();
    let mut scalars: Vec<String> = Vec::new();
    let mut inputs: Vec<Object> = Vec::new();
    let mut outputs: Vec<Object> = Vec::new();

    for gql_type in types {
        match gql_type {
            FullType::Enum { name, enum_values, .. } => {
                enums.push(Enum { name, values: enum_values.iter().map(|it| it.name.clone()).collect() });
            }
            FullType::Object { name, fields, .. } => {
                let object_fields = fields
                    .iter()
                    .map(|field| {
                        let field_name = field.name.clone();
                        let field_type = to_gql_type(&field.field_type, true);
                        if field.args.is_empty() {
                            schema::Field { name: field_name, field_type }
                        } else {

                            let args = field.args
                                .iter()
                                .map(|arg| {
                                    let arg_name = arg.name.clone();
                                    let arg_type = to_gql_type(&arg.input_type, true);
                                    let type_name = gql_type_name(&arg.input_type);
                                    Argument { name: arg_name, argument_type: arg_type, type_name }
                                })
                                .collect();

                            let fn_type = GqlType::Function {
                                inputs: args,
                                output: Box::new(field_type)
                            };

                             schema::Field { name: field_name, field_type: fn_type }
                        }
                    })
                    .collect();

                outputs.push(Object { name, fields: object_fields });
            }
            FullType::Scalar { name, .. } => {
                scalars.push(name);
            }
            FullType::InputObject { name, input_fields, .. } => {
                let fields = input_fields
                    .iter()
                    .map(|field| {
                        let field_name = &field.name;
                        let field_type = to_gql_type(&field.input_type, true);
                        schema::Field { name: field_name.clone(), field_type }
                    })
                    .collect();
                inputs.push(Object { name, fields });
            }
            FullType::Interface { .. } => (),
            FullType::Union { .. } => ()
        }
    }
    GqlDocument { inputs, outputs, enums, scalars }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntrospectionQueryResponse {
    data: SchemaData,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaData {
    #[serde(rename = "__schema")]
    schema: Schema,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Schema {
    types: Vec<FullType>
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
enum FullType {
    #[serde(rename = "OBJECT")]
    Object {
        name: String,
        fields: Vec<Field>
    },
    #[serde(rename = "INTERFACE")]
    Interface,
    #[serde(rename = "ENUM")]
    Enum {
        name: String,
        #[serde(rename = "enumValues")]
        enum_values: Vec<EnumValue>
    },
    #[serde(rename = "INPUT_OBJECT")]
    InputObject {
        name: String,
        #[serde(rename = "inputFields")]
        input_fields: Vec<InputValue>
    },
    #[serde(rename = "SCALAR")]
    Scalar {
        name: String
    },
    #[serde(rename = "UNION")]
    Union,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Field {
    name: String,
    args: Vec<InputValue>,
    #[serde(rename = "type")]
    field_type: TypeRef,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputValue {
    name: String,
    #[serde(rename = "type")]
    input_type: TypeRef
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
enum TypeRef {
    #[serde(rename = "NON_NULL")]
    NonNull {
        #[serde(rename = "ofType")]
        of_type: Box<TypeRef>
    },
    #[serde(rename = "SCALAR")]
    Scalar {
        name: String
    },
    #[serde(rename = "INPUT_OBJECT")]
    InputObject {
        name: String
    },
    #[serde(rename = "LIST")]
    List {
        #[serde(rename = "ofType")]
        of_type: Box<TypeRef>,
    },
    #[serde(rename = "OBJECT")]
    Object {
        name: String
    },
    #[serde(rename = "ENUM")]
    Enum {
        name: String
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnumValue {
    name: String,
}

fn to_gql_type(type_ref: &TypeRef, nullable: bool) -> GqlType {
    match type_ref {
        TypeRef::Enum { name, .. } => {
            let inner = GqlType::Enum(name.clone());
            if nullable {
                GqlType::Nullable(Box::new(inner))
            } else {
                inner
            }
        }
        TypeRef::List { of_type, .. } => {
            let inner = to_gql_type(of_type, true);
            if nullable {
                GqlType::Nullable(Box::new(GqlType::List(Box::new(inner))))
            } else {
                GqlType::List(Box::new(inner))
            }
        }
        TypeRef::InputObject { name, .. } => {
            let inner = GqlType::Object(name.clone());
            if nullable {
                GqlType::Nullable(Box::new(inner))
            } else {
                inner
            }
        }
        TypeRef::NonNull { of_type, .. } => to_gql_type(of_type, false),
        TypeRef::Scalar { name, .. } => {
            let inner = GqlType::Scalar(name.clone());
            if nullable {
                GqlType::Nullable(Box::new(inner))
            } else {
                inner
            }
        },
        TypeRef::Object { name, .. } => {
            let inner = GqlType::Object(name.clone());
            if nullable {
                GqlType::Nullable(Box::new(inner))
            } else {
                inner
            }
        }
    }
}

fn gql_type_name(type_ref: &TypeRef) -> String {
    match type_ref {
        TypeRef::Scalar { name, .. } => name.clone(),
        TypeRef::Object { name, .. } => name.clone(),
        TypeRef::Enum { name, .. } => name.clone(),
        TypeRef::InputObject { name, .. } => name.clone(),
        TypeRef::List { of_type, .. } => format!("[{}]", gql_type_name(of_type)),
        TypeRef::NonNull { of_type, .. } => format!("{}!", gql_type_name(of_type)),
    }
}
