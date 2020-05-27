// pub mod parser;
// pub mod tokens;

use crate::parser::tokens::{Token, TokenError};
use std::iter::{Iterator, Peekable};
use std::result;
use std::str::Chars;

pub type Result<T> = result::Result<T, TokenError>;

#[derive(Debug)]
pub struct Tokenizer<'a> {
    input: Peekable<Chars<'a>>,
}

enum Sign {
    Pos,
    Neg,
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        Tokenizer {
            input: input.chars().peekable(),
        }
    }

    fn consume_whitespace(&mut self) {
        while let Some(&c) = self.input.peek() {
            if c.is_whitespace() {
                self.input.next();
            } else {
                break;
            }
        }
    }

    fn consume_whitespace_and_comments_until_next_input(&mut self) {
        while let Some(&c) = self.input.peek() {
            match c {
                c if c.is_whitespace() => {
                    self.input.next();
                }
                ';' => self.read_rest_of_line(),
                _ => break,
            };
        }

        self.consume_whitespace()
    }

    fn read_rest_of_line(&mut self) {
        while let Some(c) = self.input.next() {
            if c == '\n' {
                break;
            }
        }
    }

    fn read_word(&mut self) -> Token {
        let mut word = String::new();
        while let Some(&c) = self.input.peek() {
            match c {
                '(' | '[' | '{' | ')' | ']' | '}' => break,
                c if c.is_whitespace() => break,
                _ => {
                    self.input.next();
                    word.push(c);
                }
            };
        }

        Token::Identifier(word)
    }

    fn read_word_with_starting_hyphen(&mut self) -> Token {
        let mut word = "-".to_string();
        while let Some(&c) = self.input.peek() {
            match c {
                '(' | '[' | '{' | ')' | ']' | '}' => break,
                c if c.is_whitespace() => break,
                _ => {
                    self.input.next();
                    word.push(c);
                }
            };
        }

        Token::Identifier(word)
    }

    fn read_hash_value(&mut self) -> Result<Token> {
        let mut word = String::new();
        while let Some(&c) = self.input.peek() {
            match c {
                '(' | '[' | '{' | ')' | ']' | '}' => break,
                c if c.is_whitespace() => break,
                c if c == '#' => return Err(TokenError::UnexpectedChar('#')),
                _ => {
                    self.input.next();
                    word.push(c);
                }
            };
        }

        match word.as_ref() {
            "t" | "true" => Ok(Token::BooleanLiteral(true)),
            "f" | "false" => Ok(Token::BooleanLiteral(false)),
            character if character.starts_with('\\') => match word.len() {
                2 | 3 | 4 => {
                    let c = word.chars().last().ok_or(TokenError::InvalidCharacter)?;
                    Ok(Token::CharacterLiteral(c))
                }
                _ => Err(TokenError::InvalidCharacter),
            },
            _ => Ok(Token::Identifier(word)), // TODO
                                              // _ => Err(TokenError::UnexpectedChar(#))
        }
    }

    fn read_num_or_int(&mut self, sign: Sign) -> Result<Token> {
        // unimplemented!()
        let mut num = String::new();
        while let Some(&c) = self.input.peek() {
            if !c.is_numeric() {
                break;
            }

            self.input.next();
            num.push(c);
        }

        if let Some(&'.') = self.input.peek() {
            self.input.next();
            num.push('.');

            while let Some(&c) = self.input.peek() {
                if !c.is_numeric() {
                    break;
                }

                self.input.next();
                num.push(c);
            }
            let num: f64 = match sign {
                Sign::Pos => num.parse().unwrap(),
                Sign::Neg => num.parse::<f64>().unwrap() * -1.0,
            };
            Ok(Token::NumberLiteral(num))
        } else {
            // We've found an integer
            let num: isize = match sign {
                Sign::Pos => num.parse().unwrap(),
                Sign::Neg => num.parse::<isize>().unwrap() * -1,
            };
            Ok(Token::IntegerLiteral(num))
        }
    }

    fn read_number(&mut self) -> f64 {
        let mut num = String::new();
        while let Some(&c) = self.input.peek() {
            if !c.is_numeric() {
                break;
            }

            self.input.next();
            num.push(c);
        }

        if let Some(&'.') = self.input.peek() {
            self.input.next();
            num.push('.');

            while let Some(&c) = self.input.peek() {
                if !c.is_numeric() {
                    break;
                }

                self.input.next();
                num.push(c);
            }
        }

        num.parse().unwrap()
    }

    fn read_string(&mut self) -> Result<Token> {
        // Skip the opening quote.
        self.input.next();

        let mut buf = String::new();
        while let Some(&c) = self.input.peek() {
            self.input.next();
            match c {
                '"' => return Ok(Token::StringLiteral(buf)),
                '\\' => match self.input.peek() {
                    Some(&c) if c == '"' || c == '\\' => {
                        self.input.next();
                        buf.push(c);
                    }
                    _ => return Err(TokenError::InvalidEscape),
                },
                _ => buf.push(c),
            }
        }

        buf.insert(0, '"');
        Err(TokenError::IncompleteString)
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Result<Token>;

    fn next(&mut self) -> Option<Self::Item> {
        self.consume_whitespace_and_comments_until_next_input();

        match self.input.peek() {
            None => None,
            Some('(') | Some('[') | Some('{') => {
                self.input.next();
                Some(Ok(Token::OpenParen))
            }
            Some(')') | Some(']') | Some('}') => {
                self.input.next();
                Some(Ok(Token::CloseParen))
            }
            Some('\'') => {
                self.input.next();
                Some(Ok(Token::QuoteTick))
            }
            Some('+') => {
                self.input.next();
                match self.input.peek() {
                    Some(&c) if c.is_numeric() => {
                        Some(self.read_num_or_int(Sign::Pos))
                        // Some(Ok(Token::NumberLiteral(self.read_number())))
                    }
                    _ => Some(Ok(Token::Identifier("+".to_string()))),
                }
            }
            Some('-') => {
                self.input.next();
                match self.input.peek() {
                    Some(&c) if c.is_numeric() => {
                        Some(self.read_num_or_int(Sign::Neg))
                        // Some(Ok(Token::NumberLiteral(self.read_number() * -1.0)))
                    }
                    Some(&c) if c == ' ' => Some(Ok(Token::Identifier("-".to_string()))),
                    _ => Some(Ok(self.read_word_with_starting_hyphen())),
                }
            }
            Some('*') => {
                self.input.next();
                Some(Ok(Token::Identifier("*".to_string())))
            }
            Some('/') => {
                self.input.next();
                Some(Ok(Token::Identifier("/".to_string())))
            }
            Some('#') => {
                self.input.next();
                Some(self.read_hash_value())
            }
            Some('"') => Some(self.read_string()),
            Some(c)
                if !c.is_whitespace() && (c.is_alphabetic() && !c.is_numeric())
                    || *c == '_'
                    || *c == '.' =>
            {
                Some(Ok(self.read_word()))
            }
            Some('=') | Some('<') | Some('>') => Some(Ok(self.read_word())),
            Some(c) if c.is_numeric() => Some(self.read_num_or_int(Sign::Pos)),
            Some(_) => match self.input.next() {
                Some(e) => Some(Err(TokenError::UnexpectedChar(e))),
                _ => None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::tokens::Token::*;
    use crate::parser::tokens::TokenError;

    #[test]
    fn test_chars() {
        let mut s = Tokenizer::new("#\\a #\\b #\\λ");
        assert_eq!(s.next(), Some(Ok(CharacterLiteral('a'))));
        assert_eq!(s.next(), Some(Ok(CharacterLiteral('b'))));
        assert_eq!(s.next(), Some(Ok(CharacterLiteral('λ'))));
    }

    #[test]
    fn test_unexpected_char() {
        let mut s = Tokenizer::new("($)");
        assert_eq!(s.next(), Some(Ok(OpenParen)));
        assert_eq!(s.next(), Some(Err(TokenError::UnexpectedChar('$'))));
    }

    #[test]
    fn test_words() {
        let mut s = Tokenizer::new("foo FOO _123_ Nil #f #t");
        assert_eq!(s.next(), Some(Ok(Identifier("foo".to_owned()))));
        assert_eq!(s.next(), Some(Ok(Identifier("FOO".to_owned()))));
        assert_eq!(s.next(), Some(Ok(Identifier("_123_".to_owned()))));
        assert_eq!(s.next(), Some(Ok(Identifier("Nil".to_owned()))));
        assert_eq!(s.next(), Some(Ok(BooleanLiteral(false))));
        assert_eq!(s.next(), Some(Ok(BooleanLiteral(true))));
        assert_eq!(s.next(), None);
    }

    #[test]
    fn test_number() {
        let mut s = Tokenizer::new("0 -0 -1.2 +2.3 999 1.");
        assert_eq!(s.next(), Some(Ok(IntegerLiteral(0))));
        assert_eq!(s.next(), Some(Ok(IntegerLiteral(0))));
        assert_eq!(s.next(), Some(Ok(NumberLiteral(-1.2))));
        assert_eq!(s.next(), Some(Ok(NumberLiteral(2.3))));
        assert_eq!(s.next(), Some(Ok(IntegerLiteral(999))));
        assert_eq!(s.next(), Some(Ok(NumberLiteral(1.0))));
        assert_eq!(s.next(), None);
    }

    #[test]
    fn test_string() {
        let mut s = Tokenizer::new(r#" "" "Foo bar" "\"\\" "#);
        assert_eq!(s.next(), Some(Ok(StringLiteral("".to_owned()))));
        assert_eq!(s.next(), Some(Ok(StringLiteral("Foo bar".to_owned()))));
        assert_eq!(s.next(), Some(Ok(StringLiteral(r#""\"#.to_owned()))));
        assert_eq!(s.next(), None);
    }

    #[test]
    fn test_comment() {
        let mut s = Tokenizer::new(";!/usr/bin/gate\n   ; foo\n");
        assert_eq!(s.next(), None);
    }

    #[test]
    fn scheme_statement() {
        let s = Tokenizer::new("(apples (function a b) (+ a b))");
        let res: Result<Vec<Token>> = s.collect();

        let expected: Vec<Token> = vec![
            OpenParen,
            Identifier("apples".to_string()),
            OpenParen,
            Identifier("function".to_string()),
            Identifier("a".to_string()),
            Identifier("b".to_string()),
            CloseParen,
            OpenParen,
            Identifier("+".to_string()),
            Identifier("a".to_string()),
            Identifier("b".to_string()),
            CloseParen,
            CloseParen,
        ];

        assert_eq!(res.unwrap(), expected);
    }
}
