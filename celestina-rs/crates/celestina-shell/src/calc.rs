//! Whether a launcher query is a math expression, and its result as shown
//! (launcher design §5.7).
//!
//! The query is hostile free text typed one key at a time, so everything here
//! is bounded: at most [`MAX_INPUT_CHARS`] characters are looked at, parentheses
//! nest at most [`MAX_DEPTH`] deep, and nothing panics. Past either bound, or on
//! any text that does not parse whole, the answer is "not an expression".
//!
//! ```text
//! expr     = term (("+" | "-") term)*
//! term     = unary (("*" | "/" | "×" | "÷") unary | implicit)*
//! implicit = unary that starts with "(" or a function name, with no operator
//! unary    = ("-" | "+") unary | power
//! power    = postfix ("^" unary)?        right-associative; -2^2 = -(2^2)
//! postfix  = atom "%"*                   each % divides by 100
//! atom     = number | constant | function atom | "(" expr ")"
//! ```
//!
//! Only unary and function chains recurse without a parenthesis; the input
//! bound keeps those far below any stack limit.

/// The longest query, in characters, that is parsed at all.
pub const MAX_INPUT_CHARS: usize = 256;

/// The deepest parenthesis nesting that is parsed.
pub const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Func {
    Sqrt,
    Sin,
    Cos,
    Tan,
    Ln,
    Log,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Token {
    Num(f64),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Percent,
    LParen,
    RParen,
    Const(f64),
    Func(Func),
}

/// The formatted result of `query`, or `None` when it is not an expression:
/// it must parse whole, hold a binary operator, a `%` or a function, and give
/// a finite value.
#[must_use]
pub fn evaluate(query: &str) -> Option<String> {
    if query.chars().count() > MAX_INPUT_CHARS {
        return None;
    }
    let tokens = tokenize(query)?;
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
        depth: 0,
        counts: false,
    };
    let value = parser.expr()?;
    if parser.pos != tokens.len() || !parser.counts {
        return None;
    }
    format(value)
}

/// `value` with up to 12 significant digits, a comma decimal separator, no
/// grouping and no trailing zeros; exponent style at 1e15 or more and below
/// 1e-9 (nonzero). `None` when `value` is not finite.
#[must_use]
pub fn format(value: f64) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    if value == 0.0 {
        return Some("0".to_owned());
    }
    let sci = std::format!("{:.*e}", 11, value.abs());
    let (mantissa, exp) = sci.split_once('e')?;
    let exp: i32 = exp.parse().ok()?;
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let sign = if value < 0.0 { "-" } else { "" };
    if !(-9..15).contains(&exp) {
        let (head, tail) = digits.split_at(1);
        let fraction = if tail.is_empty() {
            String::new()
        } else {
            std::format!(",{tail}")
        };
        return Some(std::format!("{sign}{head}{fraction}e{exp}"));
    }
    let plain = if exp < 0 {
        let zeros = "0".repeat(usize::try_from(-exp - 1).ok()?);
        std::format!("0,{zeros}{digits}")
    } else {
        let int_len = usize::try_from(exp).ok()? + 1;
        if digits.len() <= int_len {
            std::format!("{digits}{}", "0".repeat(int_len - digits.len()))
        } else {
            let (int, frac) = digits.split_at(int_len);
            std::format!("{int},{frac}")
        }
    };
    Some(std::format!("{sign}{plain}"))
}

fn tokenize(query: &str) -> Option<Vec<Token>> {
    let chars: Vec<char> = query.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' || c == ',' {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i < chars.len() && (chars[i] == '.' || chars[i] == ',') {
                i += 1;
                let frac = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                if i == frac && frac - 1 == start {
                    return None;
                }
            }
            let text: String = chars[start..i]
                .iter()
                .map(|&d| if d == ',' { '.' } else { d })
                .collect();
            tokens.push(Token::Num(text.parse().ok()?));
        } else if c.is_alphabetic() && c != 'π' && c != '√' {
            let start = i;
            while i < chars.len() && chars[i].is_alphabetic() && chars[i] != 'π' && chars[i] != '√'
            {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            tokens.push(match word.as_str() {
                "pi" => Token::Const(std::f64::consts::PI),
                "e" => Token::Const(std::f64::consts::E),
                "sqrt" => Token::Func(Func::Sqrt),
                "sin" => Token::Func(Func::Sin),
                "cos" => Token::Func(Func::Cos),
                "tan" => Token::Func(Func::Tan),
                "ln" => Token::Func(Func::Ln),
                "log" => Token::Func(Func::Log),
                _ => return None,
            });
        } else {
            tokens.push(match c {
                '+' => Token::Plus,
                '-' => Token::Minus,
                '*' | '×' => Token::Star,
                '/' | '÷' => Token::Slash,
                '^' => Token::Caret,
                '%' => Token::Percent,
                '(' => Token::LParen,
                ')' => Token::RParen,
                'π' => Token::Const(std::f64::consts::PI),
                '√' => Token::Func(Func::Sqrt),
                _ => return None,
            });
            i += 1;
        }
    }
    Some(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    depth: usize,
    /// Whether a binary operator, an implicit product, a `%` or a function was
    /// seen: the expression rule.
    counts: bool,
}

/// `Some(v)` only for a finite `v`, so an infinite step cannot be cancelled
/// out by a later one.
fn finite(v: f64) -> Option<f64> {
    v.is_finite().then_some(v)
}

impl Parser<'_> {
    fn peek(&self) -> Option<Token> {
        self.tokens.get(self.pos).copied()
    }

    fn expr(&mut self) -> Option<f64> {
        let mut acc = self.term()?;
        while let Some(op @ (Token::Plus | Token::Minus)) = self.peek() {
            self.pos += 1;
            self.counts = true;
            let rhs = self.term()?;
            acc = finite(if op == Token::Plus {
                acc + rhs
            } else {
                acc - rhs
            })?;
        }
        Some(acc)
    }

    fn term(&mut self) -> Option<f64> {
        let mut acc = self.unary()?;
        loop {
            match self.peek() {
                Some(op @ (Token::Star | Token::Slash)) => {
                    self.pos += 1;
                    self.counts = true;
                    let rhs = self.unary()?;
                    acc = finite(if op == Token::Star {
                        acc * rhs
                    } else {
                        acc / rhs
                    })?;
                }
                Some(Token::LParen | Token::Func(_)) => {
                    self.counts = true;
                    acc = finite(acc * self.unary()?)?;
                }
                _ => return Some(acc),
            }
        }
    }

    fn unary(&mut self) -> Option<f64> {
        match self.peek() {
            Some(Token::Minus) => {
                self.pos += 1;
                Some(-self.unary()?)
            }
            Some(Token::Plus) => {
                self.pos += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> Option<f64> {
        let base = self.postfix()?;
        if self.peek() == Some(Token::Caret) {
            self.pos += 1;
            self.counts = true;
            let exponent = self.unary()?;
            return finite(base.powf(exponent));
        }
        Some(base)
    }

    fn postfix(&mut self) -> Option<f64> {
        let mut value = self.atom()?;
        while self.peek() == Some(Token::Percent) {
            self.pos += 1;
            self.counts = true;
            value /= 100.0;
        }
        Some(value)
    }

    fn atom(&mut self) -> Option<f64> {
        let token = self.peek()?;
        self.pos += 1;
        match token {
            Token::Num(v) | Token::Const(v) => Some(v),
            Token::Func(f) => {
                self.counts = true;
                let arg = self.atom()?;
                finite(match f {
                    Func::Sqrt => arg.sqrt(),
                    Func::Sin => arg.sin(),
                    Func::Cos => arg.cos(),
                    Func::Tan => arg.tan(),
                    Func::Ln => arg.ln(),
                    Func::Log => arg.log10(),
                })
            }
            Token::LParen => {
                if self.depth >= MAX_DEPTH {
                    return None;
                }
                self.depth += 1;
                let inner = self.expr()?;
                self.depth -= 1;
                if self.peek() != Some(Token::RParen) {
                    return None;
                }
                self.pos += 1;
                Some(inner)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(q: &str) -> Option<String> {
        evaluate(q)
    }

    #[test]
    fn shows_the_plan_values() {
        let cases = [
            ("2*(3+4)^2", "98"),
            ("80*15%", "12"),
            ("sqrt(2)/2", "0,707106781187"),
            ("1,5+1.5", "3"),
            ("2(3)", "6"),
            ("2sqrt(4)", "4"),
            ("-2^2", "-4"),
            ("2^3^2", "512"),
            ("10/4", "2,5"),
            ("1/3", "0,333333333333"),
            ("2^60", "1,15292150461e18"),
            ("1/2^40", "9,09494701773e-13"),
        ];
        for (query, shown) in cases {
            assert_eq!(ev(query).as_deref(), Some(shown), "{query}");
        }
    }

    #[test]
    fn accepts_the_other_spellings() {
        let cases = [
            ("3×4", "12"),
            ("10÷4", "2,5"),
            ("2 * ( 3 + 4 )", "14"),
            ("√(16)", "4"),
            ("√16", "4"),
            ("2*pi", "6,28318530718"),
            ("2*π", "6,28318530718"),
            ("ln(e)", "1"),
            ("log(1000)", "3"),
            ("sin(0)", "0"),
            ("(2)(3)", "6"),
            ("2^-1", "0,5"),
            ("-3+5", "2"),
            ("1+.5", "1,5"),
            ("-0*1", "0"),
        ];
        for (query, shown) in cases {
            assert_eq!(ev(query).as_deref(), Some(shown), "{query}");
        }
    }

    #[test]
    fn exponent_literals_are_not_numbers() {
        assert_eq!(ev("1e3"), None);
        assert_eq!(ev("2e"), None);
    }

    #[test]
    fn a_partial_or_trivial_query_is_not_an_expression() {
        for query in [
            "",
            " ",
            "-5",
            "pi",
            "2",
            "e",
            "7-zip",
            "2*",
            "(3+",
            "3+)",
            "()",
            "2 3",
            "2pi",
            "2e3",
            "1.2.3",
            "+",
            "*2",
            "2**3",
            "abc",
            "2+x",
            "sqrt",
            "1+2)",
            ".",
            ",",
            "2^",
            "2(",
            "\u{0663}+1",
        ] {
            assert_eq!(ev(query), None, "{query:?}");
        }
    }

    #[test]
    fn a_non_finite_result_is_not_an_expression() {
        for query in [
            "1/0",
            "0/0",
            "sqrt(-1)",
            "10^400",
            "ln(0)",
            "ln(-1)",
            "log(0)",
            "10^400/10^400",
            "1/(10^400)",
            "tan(pi/2)*10^400",
        ] {
            assert_eq!(ev(query), None, "{query}");
        }
    }

    #[test]
    fn nesting_is_bounded() {
        let nest = |n: usize| format!("{}1+1{}", "(".repeat(n), ")".repeat(n));
        assert_eq!(ev(&nest(MAX_DEPTH)).as_deref(), Some("2"));
        assert_eq!(ev(&nest(MAX_DEPTH + 1)), None);
        assert_eq!(ev(&nest(1000)), None);
        let unclosed = format!("{}1+1", "(".repeat(1000));
        assert_eq!(ev(&unclosed), None);
    }

    #[test]
    fn input_length_is_bounded() {
        let at_bound = format!("1{}", "+0".repeat(127)) + " ";
        assert_eq!(at_bound.chars().count(), MAX_INPUT_CHARS);
        assert_eq!(ev(&at_bound).as_deref(), Some("1"));
        assert_eq!(ev(&format!("{at_bound} ")), None);
        assert_eq!(ev(&"1+".repeat(5000)), None);
        // Characters, not bytes: 256 two-byte characters are under the bound
        // but are not an expression either way.
        assert_eq!(ev(&"×".repeat(MAX_INPUT_CHARS)), None);
    }

    #[test]
    fn long_unary_chains_return_at_once() {
        assert_eq!(ev(&format!("{}1+1", "-".repeat(250))).as_deref(), Some("2"));
        assert_eq!(
            ev(&format!("{}1", "sqrt ".repeat(50))).as_deref(),
            Some("1")
        );
    }

    #[test]
    fn format_rules() {
        assert_eq!(format(-0.0).as_deref(), Some("0"));
        assert_eq!(format(0.0).as_deref(), Some("0"));
        assert_eq!(format(f64::NAN), None);
        assert_eq!(format(f64::INFINITY), None);
        assert_eq!(format(f64::NEG_INFINITY), None);
        assert_eq!(format(1.5e20).as_deref(), Some("1,5e20"));
        assert_eq!(format(-1.5e20).as_deref(), Some("-1,5e20"));
        assert_eq!(format(1e15).as_deref(), Some("1e15"));
        assert_eq!(format(1e-9).as_deref(), Some("0,000000001"));
        assert_eq!(format(1e-10).as_deref(), Some("1e-10"));
        assert_eq!(format(-2.5).as_deref(), Some("-2,5"));
        assert_eq!(format(1234567.0).as_deref(), Some("1234567"));
        assert_eq!(
            format(123_456_789_012_345.0).as_deref(),
            Some("123456789012000")
        );
        assert_eq!(format(0.1 + 0.2).as_deref(), Some("0,3"));
        assert_eq!(format(f64::MAX).as_deref(), Some("1,79769313486e308"));
        assert_eq!(format(5e-324).as_deref(), Some("4,94065645841e-324"));
    }
}
