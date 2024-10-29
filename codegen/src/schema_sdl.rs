use std::collections::{BTreeMap, BTreeSet};
use graphql_parser::schema::ParseError;
use graphql_parser::schema::{Document, TypeDefinition, Type, InputObjectType, ObjectType};
use graphql_parser::schema::Definition;
use crate::schema::{ GqlDocument, Argument, GqlType, Enum, Field, Object };
use graphql_parser::schema::parse_schema;
use crate::schema;

pub fn from_sdl_string(sdl: &str) -> Result<GqlDocument, ParseError> {
    let schema = parse_schema(sdl)?;
    Ok(from_parser_document(schema))
}

fn from_parser_document(document: Document<'_, String>) -> GqlDocument {
    let mut builder = GqlDocumentBuilder::new();
    for scalar in schema::BUILT_IN_SCALARS {
        builder.add_scalar(scalar);
    }
    builder.add_document(document);
    builder.build()
}

struct GqlDocumentBuilder<'a> {
    input_definitions: BTreeMap<String, InputObjectType<'a, String>>,
    output_definitions: BTreeMap<String, ObjectType<'a, String>>,
    enums: BTreeMap<String, Enum>,
    scalars: BTreeSet<String>
}

impl<'a> GqlDocumentBuilder<'a> {
    fn new() -> GqlDocumentBuilder<'a> {
        GqlDocumentBuilder {
            input_definitions: BTreeMap::new(),
            output_definitions: BTreeMap::new(),
            enums: BTreeMap::new(),
            scalars: BTreeSet::new()
        }
    }

    fn add_scalar(&mut self, name: &str) {
        self.scalars.insert(name.to_string());
    }

    fn add_document(&mut self, schema: Document<'a, String>) {
        for definition in schema.definitions {
            match definition {
                Definition::TypeDefinition(definition) => {
                    match definition {
                        TypeDefinition::Scalar(definition) => {
                            self.scalars.insert(definition.name);
                        }
                        TypeDefinition::Object(definition) => {
                            self.output_definitions.insert(definition.name.clone(), definition);
                        }
                        TypeDefinition::InputObject(definition) => {
                            self.input_definitions.insert(definition.name.clone(), definition);
                        }
                        TypeDefinition::Enum(definition) => {
                            let name = definition.name;
                            let enum_members: Vec<String> = definition.values.iter().map(|it| it.name.clone()).collect();
                            let enum_def = Enum { name: name.clone(), values: enum_members };
                            self.enums.insert(name, enum_def);
                        }
                        TypeDefinition::Union(_) => (),
                        TypeDefinition::Interface(_) => (),
                    }
                }
                Definition::SchemaDefinition(_) => (),
                Definition::TypeExtension(_) => (),
                Definition::DirectiveDefinition(_) => ()
            }
        }
    }

    fn build(self) -> GqlDocument {
        let inputs = self.input_definitions
            .iter()
            .map(|(_, object)| self.to_input_object(object))
            .collect();
        let outputs = self.output_definitions
            .iter()
            .map(|(_, object)| self.to_output_object(object))
            .collect();
        GqlDocument {
            inputs,
            outputs,
            scalars: self.scalars,
            enums: self.enums.into_values().collect()
        }
    }

    fn to_output_object(&self, definition: &ObjectType<'_, String>) -> Object {
        let fields: Vec<Field> = definition.fields
            .iter()
            .map(|field| {
                let field_name = &field.name;
                let field_type = &field.field_type;
                let field_arguments = &field.arguments;
                
                if field_arguments.is_empty() {
                    Field { name: field_name.clone(), field_type: self.to_gql_type(field_type, true) }
                } else {
                    let func_output = self.to_gql_type(field_type, true);
                    let args = field_arguments
                        .iter()
                        .map(|arg| {
                            Argument {
                                name: arg.name.clone(),
                                argument_type: self.to_gql_type(&arg.value_type, true),
                                type_name: arg.value_type.to_string()
                            }
                        })
                        .collect();
                    Field {
                        name: field_name.clone(),
                        field_type: GqlType::Function {
                            inputs: args,
                            output: Box::new(func_output)
                        }
                    }
                }
            })
            .collect();
        Object {
            name: definition.name.clone(),
            fields
        }
    }

    fn to_input_object(&self, definition: &InputObjectType<'_, String>) -> Object {
        let fields = definition.fields.iter()
            .map(|field| {
                let name = &field.name;
                let field_type = &field.value_type;
                Field { name: name.clone(), field_type: self.to_gql_type(&field_type, true) }
            })
            .collect();
        Object {
            name: definition.name.clone(),
            fields
        }
    }

    fn to_gql_type(&self, field_type: &Type<'_, String>, is_nullable: bool) -> GqlType {
        match field_type {
            Type::NonNullType(inner) => {
                self.to_gql_type(&inner, false)
            }
            Type::ListType(inner) => {
                let inner_type = self.to_gql_type(&inner, true);
                if is_nullable {
                    GqlType::Nullable(Box::new(GqlType::List(Box::new(inner_type))))
                } else {
                    GqlType::List(Box::new(inner_type))
                }
            }
            Type::NamedType(name) => {
                let inner = if self.scalars.contains(&name.to_string()) {
                    GqlType::Scalar(name.clone())
                } else if self.enums.contains_key(name) {
                    GqlType::Enum(name.clone())
                } else if self.input_definitions.contains_key(name) {
                    GqlType::Object(name.clone())
                } else if self.output_definitions.contains_key(name) {
                    GqlType::Object(name.clone())
                } else {
                    panic!("Unknown type {}", name);
                };
                if is_nullable {
                    GqlType::Nullable(Box::new(inner))
                } else {
                    inner
                }
            }
        }
    }
}
