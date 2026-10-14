//! Expression evaluation and number formatting, independent of the GUI.

use std::str::FromStr;

use rust_decimal::prelude::*;
use rust_decimal::MathematicalOps;

/// Decimal places kept in a displayed result.
const DISPLAY_DP: u32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notation {
    /// Decimal comma: `1.234,5`
    Comma,
    /// Decimal point: `1,234.5`
    Point,
}

impl Notation {
    pub fn decimal(self) -> char {
        match self {
            Notation::Comma => ',',
            Notation::Point => '.',
        }
    }

    pub fn group(self) -> char {
        match self {
            Notation::Comma => '.',
            Notation::Point => ',',
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Notation::Comma => "comma",
            Notation::Point => "point",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "comma" => Some(Notation::Comma),
            "point" => Some(Notation::Point),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalcError {
    Empty,
    Syntax,
    Grouping,
    DivisionByZero,
    NegativeRoot,
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tok {
    Num(Decimal),
    Plus,
    Minus,
    Mul,
    Div,
    Pow,
    Square,
    Sqrt,
    Percent,
    LParen,
    RParen,
}

fn tokenize(input: &str, notation: Notation) -> Result<Vec<Tok>, CalcError> {
    let dec = notation.decimal();
    let grp = notation.group();
    let mut toks = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c.is_ascii_digit() || c == dec {
            let mut raw = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == dec || c == grp {
                    raw.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            toks.push(Tok::Num(parse_number(&raw, notation)?));
            continue;
        }
        toks.push(match c {
            '+' => Tok::Plus,
            '-' | '−' => Tok::Minus,
            '*' | '×' => Tok::Mul,
            '/' | '÷' | ':' => Tok::Div,
            '^' => Tok::Pow,
            '²' => Tok::Square,
            '√' => Tok::Sqrt,
            '%' => Tok::Percent,
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            _ => return Err(CalcError::Syntax),
        });
        chars.next();
    }
    Ok(toks)
}

/// Group separators are accepted only in the integer part and only in
/// proper groups of three, so a mistyped separator is reported instead of
/// silently changing the value.
fn parse_number(raw: &str, notation: Notation) -> Result<Decimal, CalcError> {
    let mut parts = raw.split(notation.decimal());
    let int = parts.next().unwrap_or("");
    let frac = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return Err(CalcError::Syntax);
    }
    if frac.contains(notation.group()) {
        return Err(CalcError::Grouping);
    }
    if int.contains(notation.group()) {
        let mut groups = int.split(notation.group());
        let first = groups.next().unwrap_or("");
        if !(1..=3).contains(&first.len()) || groups.any(|g| g.len() != 3) {
            return Err(CalcError::Grouping);
        }
    }
    let digits: String = int.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() && frac.is_empty() {
        return Err(CalcError::Syntax);
    }
    let int = if digits.is_empty() { "0" } else { &digits };
    let canonical = if frac.is_empty() {
        int.to_string()
    } else {
        format!("{int}.{frac}")
    };
    Decimal::from_str(&canonical).map_err(|_| CalcError::Overflow)
}

/// A value plus whether it was written as a bare percentage (`10%`), which
/// `+` and `−` interpret relative to their left operand.
#[derive(Clone, Copy)]
struct Val {
    v: Decimal,
    pct: bool,
}

impl Val {
    fn plain(v: Decimal) -> Self {
        Val { v, pct: false }
    }
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<Tok> {
        self.toks.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.peek();
        self.pos += 1;
        t
    }

    // expr := term (('+' | '−') term)*
    fn expr(&mut self) -> Result<Decimal, CalcError> {
        let mut acc = self.term()?.v;
        while let Some(op @ (Tok::Plus | Tok::Minus)) = self.peek() {
            self.next();
            let r = self.term()?;
            let rhs = if r.pct {
                acc.checked_mul(r.v).ok_or(CalcError::Overflow)?
            } else {
                r.v
            };
            acc = match op {
                Tok::Plus => acc.checked_add(rhs),
                _ => acc.checked_sub(rhs),
            }
            .ok_or(CalcError::Overflow)?;
        }
        Ok(acc)
    }

    // term := unary (('×' | '÷') unary | implicit-× unary)*
    fn term(&mut self) -> Result<Val, CalcError> {
        let mut acc = self.unary()?;
        loop {
            match self.peek() {
                Some(Tok::Mul) => {
                    self.next();
                    let r = self.unary()?;
                    acc = Val::plain(acc.v.checked_mul(r.v).ok_or(CalcError::Overflow)?);
                }
                Some(Tok::Div) => {
                    self.next();
                    let r = self.unary()?;
                    if r.v.is_zero() {
                        return Err(CalcError::DivisionByZero);
                    }
                    acc = Val::plain(acc.v.checked_div(r.v).ok_or(CalcError::Overflow)?);
                }
                // `2(3+4)` and `2√9`
                Some(Tok::LParen | Tok::Sqrt) => {
                    let r = self.unary()?;
                    acc = Val::plain(acc.v.checked_mul(r.v).ok_or(CalcError::Overflow)?);
                }
                _ => return Ok(acc),
            }
        }
    }

    // unary := ('−' | '+') unary | power
    fn unary(&mut self) -> Result<Val, CalcError> {
        match self.peek() {
            Some(Tok::Minus) => {
                self.next();
                let r = self.unary()?;
                Ok(Val { v: -r.v, pct: r.pct })
            }
            Some(Tok::Plus) => {
                self.next();
                self.unary()
            }
            _ => self.power(),
        }
    }

    // power := postfix ('^' unary)?   — right-associative, binds tighter than unary minus
    fn power(&mut self) -> Result<Val, CalcError> {
        let base = self.postfix()?;
        if self.peek() != Some(Tok::Pow) {
            return Ok(base);
        }
        self.next();
        let exp = self.unary()?.v;
        Ok(Val::plain(pow(base.v, exp)?))
    }

    // postfix := primary ('²' | '%')*
    fn postfix(&mut self) -> Result<Val, CalcError> {
        let mut val = self.primary()?;
        loop {
            match self.peek() {
                Some(Tok::Square) => {
                    self.next();
                    val = Val::plain(val.v.checked_mul(val.v).ok_or(CalcError::Overflow)?);
                }
                Some(Tok::Percent) => {
                    self.next();
                    val = Val {
                        v: val.v / Decimal::ONE_HUNDRED,
                        pct: true,
                    };
                }
                _ => return Ok(val),
            }
        }
    }

    // primary := number | '(' expr ')' | '√' unary
    fn primary(&mut self) -> Result<Val, CalcError> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Val::plain(n)),
            Some(Tok::LParen) => {
                let v = self.expr()?;
                match self.next() {
                    // Unclosed parentheses at the very end are closed implicitly,
                    // so the live preview works while typing `2×(3+4`.
                    Some(Tok::RParen) | None => Ok(Val::plain(v)),
                    _ => Err(CalcError::Syntax),
                }
            }
            Some(Tok::Sqrt) => {
                let v = self.unary()?.v;
                if v.is_sign_negative() && !v.is_zero() {
                    return Err(CalcError::NegativeRoot);
                }
                Ok(Val::plain(v.sqrt().ok_or(CalcError::Overflow)?))
            }
            _ => Err(CalcError::Syntax),
        }
    }
}

fn pow(base: Decimal, exp: Decimal) -> Result<Decimal, CalcError> {
    if exp.fract().is_zero() {
        let e = exp.to_i64().ok_or(CalcError::Overflow)?;
        if base.is_zero() && e < 0 {
            return Err(CalcError::DivisionByZero);
        }
        base.checked_powi(e).ok_or(CalcError::Overflow)
    } else {
        if base.is_sign_negative() && !base.is_zero() {
            return Err(CalcError::NegativeRoot);
        }
        base.checked_powd(exp).ok_or(CalcError::Overflow)
    }
}

pub fn evaluate(input: &str, notation: Notation) -> Result<Decimal, CalcError> {
    let toks = tokenize(input, notation)?;
    if toks.is_empty() {
        return Err(CalcError::Empty);
    }
    let mut p = Parser { toks, pos: 0 };
    let v = p.expr()?;
    if p.pos < p.toks.len() {
        return Err(CalcError::Syntax);
    }
    Ok(v)
}

/// Formats `value` in `notation`, with group separators if `grouped`.
pub fn format(value: Decimal, notation: Notation, grouped: bool) -> String {
    let v = value
        .round_dp_with_strategy(DISPLAY_DP, RoundingStrategy::MidpointAwayFromZero)
        .normalize();
    let plain = v.abs().to_string();
    let (int, frac) = plain.split_once('.').unwrap_or((&plain, ""));

    let mut out = String::new();
    if v.is_sign_negative() && !v.is_zero() {
        out.push('-');
    }
    for (i, c) in int.chars().enumerate() {
        if grouped && i > 0 && (int.len() - i) % 3 == 0 {
            out.push(notation.group());
        }
        out.push(c);
    }
    if !frac.is_empty() {
        out.push(notation.decimal());
        out.push_str(frac);
    }
    out
}

/// Swaps decimal and group separators, for switching notation mid-expression.
pub fn convert(input: &str, from: Notation, to: Notation) -> String {
    if from == to {
        return input.to_string();
    }
    input
        .chars()
        .map(|c| match c {
            c if c == from.decimal() => to.decimal(),
            c if c == from.group() => to.group(),
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use Notation::*;

    fn eval(s: &str) -> String {
        format(evaluate(s, Comma).unwrap(), Comma, true)
    }

    #[test]
    fn decimal_is_exact() {
        assert_eq!(eval("0,1+0,2"), "0,3");
    }

    #[test]
    fn precedence_and_unary() {
        assert_eq!(eval("2+3×4"), "14");
        assert_eq!(eval("-2^2"), "-4");
        assert_eq!(eval("(-2)^2"), "4");
        assert_eq!(eval("2^3^2"), "512");
        assert_eq!(eval("10−4−3"), "3");
        assert_eq!(eval("12÷4÷3"), "1");
    }

    #[test]
    fn square_root_and_square() {
        assert_eq!(eval("√16"), "4");
        assert_eq!(eval("3²"), "9");
        assert_eq!(eval("2√9"), "6");
        assert_eq!(eval("√(9+16)"), "5");
    }

    #[test]
    fn percent() {
        assert_eq!(eval("200+10%"), "220");
        assert_eq!(eval("200−10%"), "180");
        assert_eq!(eval("200×10%"), "20");
        assert_eq!(eval("50%"), "0,5");
    }

    #[test]
    fn implicit_multiplication_and_auto_close() {
        assert_eq!(eval("2(3+4)"), "14");
        assert_eq!(eval("2×(3+4"), "14");
    }

    #[test]
    fn grouping() {
        assert_eq!(eval("1.234,5×2"), "2.469");
        assert_eq!(eval("1.000.000"), "1.000.000");
        assert_eq!(evaluate("1.23", Comma), Err(CalcError::Grouping));
        assert_eq!(evaluate("1234.567", Comma), Err(CalcError::Grouping));
        assert_eq!(evaluate("1,5.000", Comma), Err(CalcError::Grouping));
    }

    #[test]
    fn decimal_point() {
        let v = evaluate("1,234.5+0.5", Point).unwrap();
        assert_eq!(format(v, Point, true), "1,235");
        assert_eq!(format(v, Point, false), "1235");
        assert_eq!(evaluate("1,5", Point), Err(CalcError::Grouping));
    }

    #[test]
    fn errors() {
        assert_eq!(evaluate("1÷0", Comma), Err(CalcError::DivisionByZero));
        assert_eq!(evaluate("√-4", Comma), Err(CalcError::NegativeRoot));
        assert_eq!(evaluate("2+", Comma), Err(CalcError::Syntax));
        assert_eq!(evaluate("2)", Comma), Err(CalcError::Syntax));
        assert_eq!(evaluate("", Comma), Err(CalcError::Empty));
        assert_eq!(evaluate("1,2,3", Comma), Err(CalcError::Syntax));
    }

    #[test]
    fn formatting() {
        let v = Decimal::from_str("-1234567.891").unwrap();
        assert_eq!(format(v, Comma, true), "-1.234.567,891");
        assert_eq!(format(v, Comma, false), "-1234567,891");
        assert_eq!(eval("1÷3"), "0,333333333333");
    }

    #[test]
    fn notation_conversion() {
        assert_eq!(convert("1.234,5+2", Comma, Point), "1,234.5+2");
    }
}
