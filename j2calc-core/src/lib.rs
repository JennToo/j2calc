#![no_std]

extern crate alloc;

use alloc::vec::Vec;

#[derive(PartialEq, PartialOrd, Debug, Clone)]
pub enum Value {
    Real(f64),
    Complex(f64, f64),
    // TODO: BigNum for ints
}

#[derive(Debug, Clone)]
pub enum Token {
    Value(Value),
    Operator(char),
}

#[derive(Debug, PartialEq)]
pub struct Error {
    // TODO
}

pub fn algebraic_evaluate(expression: &str) -> Result<Value, Error> {
    let mut rpn_stack: Vec<Value> = Vec::new();
    let mut operator_stack: Vec<char> = Vec::new();

    for c in expression.chars() {
        if let Some(n) = c.to_digit(10) {
            rpn_evaluate(&mut rpn_stack, &Token::Value(Value::Real(n as f64)))?;
        } else if c == '+' {
            operator_stack.push(c);
        }
    }

    while let Some(op) = operator_stack.pop() {
        rpn_evaluate(&mut rpn_stack, &Token::Operator(op))?;
    }

    if let Some(v) = rpn_stack.pop() {
        if rpn_stack.is_empty() {
            Ok(v)
        } else {
            Err(Error {})
        }
    } else {
        Err(Error {})
    }
}

pub fn rpn_evaluate(rpn_stack: &mut Vec<Value>, token: &Token) -> Result<(), Error> {
    match token {
        Token::Value(v) => rpn_stack.push(v.clone()),
        Token::Operator(c) => {
            if rpn_stack.len() < 2 {
                return Err(Error {});
            }
            if *c == '+' {
                let v0 = rpn_stack.pop().unwrap();
                let v1 = rpn_stack.pop().unwrap();
                if let (Value::Real(v0), Value::Real(v1)) = (v0, v1) {
                    rpn_stack.push(Value::Real(v0 + v1));
                } else {
                    return Err(Error {});
                }
            } else {
                return Err(Error {});
            }
        }
    }
    Ok(())
}

macro_rules! expression_tests {
    ($($name:ident: $value:expr,)*) => {
    $(
        #[test]
        fn $name() {
            let (input, expected) = $value;
            assert_eq!(expected, algebraic_evaluate(input));
        }
    )*
    }
}

expression_tests!(
    basic1: ("2+2", Ok(Value::Real(4.0))),
    basic2: ("2+2+2", Ok(Value::Real(6.0))),
);
