use nu_engine::CallExt;
use nu_protocol::{
    ast::{Call, CellPath},
    engine::{Command, EngineState, Stack},
    Category, Example, PipelineData, ShellError, Signature, Span, SyntaxShape, Value,
};

struct Arguments {
    radix: Option<Value>,
    column_paths: Vec<CellPath>,
}

#[derive(Clone)]
pub struct SubCommand;

impl Command for SubCommand {
    fn name(&self) -> &str {
        "into int"
    }

    fn signature(&self) -> Signature {
        Signature::build("into int")
            .named("radix", SyntaxShape::Number, "radix of integer", Some('r'))
            .rest(
                "rest",
                SyntaxShape::CellPath,
                "column paths to convert to int (for table input)",
            )
            .category(Category::Conversions)
    }

    fn usage(&self) -> &str {
        "Convert value to integer"
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        input: PipelineData,
    ) -> Result<nu_protocol::PipelineData, nu_protocol::ShellError> {
        into_int(engine_state, stack, call, input)
    }

    fn examples(&self) -> Vec<Example> {
        vec![
            Example {
                description: "Convert string to integer in table",
                example: "echo [[num]; ['-5'] [4] [1.5]] | into int num",
                result: None,
            },
            Example {
                description: "Convert string to integer",
                example: "'2' | into int",
                result: Some(Value::test_int(2)),
            },
            Example {
                description: "Convert decimal to integer",
                example: "5.9 | into int",
                result: Some(Value::test_int(5)),
            },
            Example {
                description: "Convert decimal string to integer",
                example: "'5.9' | into int",
                result: Some(Value::test_int(5)),
            },
            Example {
                description: "Convert file size to integer",
                example: "4KB | into int",
                result: Some(Value::Int {
                    val: 4000,
                    span: Span::test_data(),
                }),
            },
            Example {
                description: "Convert bool to integer",
                example: "[false, true] | into int",
                result: Some(Value::List {
                    vals: vec![Value::test_int(0), Value::test_int(1)],
                    span: Span::test_data(),
                }),
            },
            Example {
                description: "Convert to integer from binary",
                example: "'1101' | into int -r 2",
                result: Some(Value::test_int(13)),
            },
            Example {
                description: "Convert to integer from hex",
                example: "'FF' |  into int -r 16",
                result: Some(Value::test_int(255)),
            },
        ]
    }
}

fn into_int(
    engine_state: &EngineState,
    stack: &mut Stack,
    call: &Call,
    input: PipelineData,
) -> Result<nu_protocol::PipelineData, nu_protocol::ShellError> {
    let head = call.head;

    let options = Arguments {
        radix: call.get_flag(engine_state, stack, "radix")?,
        column_paths: call.rest(engine_state, stack, 0)?,
    };

    let radix: u32 = match options.radix {
        Some(Value::Int { val, .. }) => val as u32,
        Some(_) => 10,
        None => 10,
    };

    if let Some(val) = &options.radix {
        if !(2..=36).contains(&radix) {
            return Err(ShellError::UnsupportedInput(
                "Radix must lie in the range [2, 36]".to_string(),
                val.span()?,
            ));
        }
    }

    input.map(
        move |v| {
            if options.column_paths.is_empty() {
                action(&v, head, radix)
            } else {
                let mut ret = v;
                for path in &options.column_paths {
                    let r = ret.update_cell_path(
                        &path.members,
                        Box::new(move |old| action(old, head, radix)),
                    );
                    if let Err(error) = r {
                        return Value::Error { error };
                    }
                }

                ret
            }
        },
        engine_state.ctrlc.clone(),
    )
}

pub fn action(input: &Value, span: Span, radix: u32) -> Value {
    match input {
        Value::Int { val: _, .. } => {
            if radix == 10 {
                input.clone()
            } else {
                convert_int(input, span, radix)
            }
        }
        Value::Filesize { val, .. } => Value::Int { val: *val, span },
        Value::Float { val, .. } => Value::Int {
            val: *val as i64,
            span,
        },
        Value::String { val, .. } => {
            if radix == 10 {
                match int_from_string(val, span) {
                    Ok(val) => Value::Int { val, span },
                    Err(error) => Value::Error { error },
                }
            } else {
                convert_int(input, span, radix)
            }
        }
        Value::Bool { val, .. } => {
            if *val {
                Value::Int { val: 1, span }
            } else {
                Value::Int { val: 0, span }
            }
        }
        _ => Value::Error {
            error: ShellError::UnsupportedInput("'into int' for unsupported type".into(), span),
        },
    }
}

fn convert_int(input: &Value, head: Span, radix: u32) -> Value {
    let i = match input {
        Value::Int { val, .. } => val.to_string(),
        Value::String { val, .. } => {
            if val.starts_with("0x") || val.starts_with("0b") {
                match int_from_string(val, head) {
                    Ok(x) => return Value::Int { val: x, span: head },
                    Err(e) => return Value::Error { error: e },
                }
            }
            val.to_string()
        }
        _ => {
            return Value::Error {
                error: ShellError::UnsupportedInput(
                    "only strings or integers are supported".to_string(),
                    head,
                ),
            }
        }
    };
    match i64::from_str_radix(&i, radix) {
        Ok(n) => Value::Int { val: n, span: head },
        Err(reason) => Value::Error {
            error: ShellError::CantConvert("".to_string(), reason.to_string(), head),
        },
    }
}

fn int_from_string(a_string: &str, span: Span) -> Result<i64, ShellError> {
    let trimmed = a_string.trim();
    match trimmed {
        b if b.starts_with("0b") => {
            let num = match i64::from_str_radix(b.trim_start_matches("0b"), 2) {
                Ok(n) => n,
                Err(reason) => {
                    return Err(ShellError::CantConvert(
                        "could not parse as integer".to_string(),
                        reason.to_string(),
                        span,
                    ))
                }
            };
            Ok(num)
        }
        h if h.starts_with("0x") => {
            let num = match i64::from_str_radix(h.trim_start_matches("0x"), 16) {
                Ok(n) => n,
                Err(reason) => {
                    return Err(ShellError::CantConvert(
                        "could not parse as int".to_string(),
                        reason.to_string(),
                        span,
                    ))
                }
            };
            Ok(num)
        }
        _ => match a_string.parse::<i64>() {
            Ok(n) => Ok(n),
            Err(_) => match a_string.parse::<f64>() {
                Ok(f) => Ok(f as i64),
                _ => Err(ShellError::CantConvert(
                    "into int".to_string(),
                    "string".to_string(),
                    span,
                )),
            },
        },
    }
}

#[cfg(test)]
mod test {
    use super::Value;
    use super::*;
    use nu_protocol::Type::Error;

    #[test]
    fn test_examples() {
        use crate::test_examples;

        test_examples(SubCommand {})
    }

    #[test]
    fn turns_to_integer() {
        let word = Value::test_string("10");
        let expected = Value::test_int(10);

        let actual = action(&word, Span::test_data(), 10);
        assert_eq!(actual, expected);
    }

    #[test]
    fn turns_binary_to_integer() {
        let s = Value::test_string("0b101");
        let actual = action(&s, Span::test_data(), 10);
        assert_eq!(actual, Value::test_int(5));
    }

    #[test]
    fn turns_hex_to_integer() {
        let s = Value::test_string("0xFF");
        let actual = action(&s, Span::test_data(), 16);
        assert_eq!(actual, Value::test_int(255));
    }

    #[test]
    fn communicates_parsing_error_given_an_invalid_integerlike_string() {
        let integer_str = Value::test_string("36anra");

        let actual = action(&integer_str, Span::test_data(), 10);

        assert_eq!(actual.get_type(), Error)
    }
}
