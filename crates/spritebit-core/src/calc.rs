//! Rechnen in Zahlenfeldern: „24 * 4“ statt im Kopf.
//!
//! Für die Felder mit Sprite-Größen (Neuer Sprite, Größe ändern, Leinwand).
//! Erlaubt sind Zahlen (auch mit Komma), + − * / und Klammern; x und ×
//! gelten als Mal ("24x4"), : als geteilt. Wie im Web (js/calc.js).

/// Ausdruck ausrechnen. `None` = kein gültiger Ausdruck (oder durch 0 geteilt).
pub fn eval(text: &str) -> Option<f64> {
    let src: Vec<char> = text
        .chars()
        .map(|c| match c {
            ',' => '.',
            'x' | 'X' | '×' => '*',
            ':' => '/',
            '−' => '-',
            c => c,
        })
        .filter(|c| !c.is_whitespace())
        .collect();
    if src.is_empty() {
        return None;
    }
    let mut p = Parser { s: &src, i: 0 };
    let v = p.expr()?;
    (p.i == src.len() && v.is_finite()).then_some(v)
}

struct Parser<'a> {
    s: &'a [char],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop {
            match self.peek() {
                Some('+') => {
                    self.i += 1;
                    v += self.term()?;
                }
                Some('-') => {
                    self.i += 1;
                    v -= self.term()?;
                }
                _ => return Some(v),
            }
        }
    }

    fn term(&mut self) -> Option<f64> {
        let mut v = self.factor()?;
        loop {
            match self.peek() {
                Some('*') => {
                    self.i += 1;
                    v *= self.factor()?;
                }
                Some('/') => {
                    self.i += 1;
                    v /= self.factor()?;
                }
                _ => return Some(v),
            }
        }
    }

    fn factor(&mut self) -> Option<f64> {
        match self.peek()? {
            '-' => {
                self.i += 1;
                Some(-self.factor()?)
            }
            '+' => {
                self.i += 1;
                self.factor()
            }
            '(' => {
                self.i += 1;
                let v = self.expr()?;
                if self.peek()? != ')' {
                    return None;
                }
                self.i += 1;
                Some(v)
            }
            _ => self.number(),
        }
    }

    fn number(&mut self) -> Option<f64> {
        let start = self.i;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit() || c == '.') {
            self.i += 1;
        }
        let t: String = self.s[start..self.i].iter().collect();
        if t.is_empty() || t.matches('.').count() > 1 || t == "." {
            return None;
        }
        t.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::eval;

    #[test]
    fn grundrechenarten() {
        assert_eq!(eval("24"), Some(24.0));
        assert_eq!(eval("24 * 4"), Some(96.0));
        assert_eq!(eval("64 / 2"), Some(32.0));
        assert_eq!(eval("16 + 8"), Some(24.0));
        assert_eq!(eval("100 - 36"), Some(64.0));
    }

    #[test]
    fn punkt_vor_strich_klammern_vorzeichen() {
        assert_eq!(eval("2 + 3 * 4"), Some(14.0));
        assert_eq!(eval("(2 + 3) * 4"), Some(20.0));
        assert_eq!(eval("-4 + 10"), Some(6.0));
        assert_eq!(eval("2 * -3"), Some(-6.0));
    }

    #[test]
    fn andere_schreibweisen() {
        assert_eq!(eval("24x4"), Some(96.0));
        assert_eq!(eval("24 × 4"), Some(96.0));
        assert_eq!(eval("96 : 4"), Some(24.0));
        assert_eq!(eval("1,5 * 16"), Some(24.0));
    }

    #[test]
    fn unsinn_ergibt_none() {
        for s in ["", "abc", "2 +", "(2 + 3", "4 / 0", "1.2.3", "24 * 4 ; 1"] {
            assert_eq!(eval(s), None, "{s}");
        }
    }
}
