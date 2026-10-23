//! An activity's calculation as a LaTeX document that `pdflatex` compiles as it is: plain ASCII,
//! accents as TeX's commands, the symbols the sentences use as mathematics.

use mkperth_core::Date;
use mkperth_i18n::tr;

use crate::Explanation;

/// Text for LaTeX: its special characters escaped, every letter outside ASCII written with TeX's
/// accents, and anything else pdflatex has no glyph for as a question mark.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let accent = |mark: &str, base: char| format!("\\{mark}{{{base}}}");
        let piece = match c {
            '\\' => "\\textbackslash{}".to_string(),
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => format!("\\{c}"),
            '~' => "\\textasciitilde{}".to_string(),
            '^' => "\\textasciicircum{}".to_string(),
            '<' => "\\textless{}".to_string(),
            '>' => "\\textgreater{}".to_string(),
            '|' => "\\textbar{}".to_string(),
            '−' => "$-$".to_string(),
            '≤' => "$\\le$".to_string(),
            '≥' => "$\\ge$".to_string(),
            '→' => "$\\to$".to_string(),
            '·' => "$\\cdot$".to_string(),
            '–' => "--".to_string(),
            '—' => "---".to_string(),
            '…' => "\\ldots{}".to_string(),
            '•' => "\\textbullet{}".to_string(),
            '“' => "``".to_string(),
            '”' => "''".to_string(),
            '‘' => "`".to_string(),
            '’' => "'".to_string(),
            '\u{a0}' => "~".to_string(),
            'ç' => accent("c", 'c'),
            'Ç' => accent("c", 'C'),
            'ñ' => accent("~", 'n'),
            'Ñ' => accent("~", 'N'),
            'ß' => "{\\ss}".to_string(),
            c if c.is_ascii() && !c.is_ascii_control() => c.to_string(),
            '\t' | '\n' => " ".to_string(),
            c => match decompose(c) {
                Some((mark, base)) => accent(mark, base),
                None => "?".to_string(),
            },
        };
        out.push_str(&piece);
    }
    out
}

/// A Latin letter with one accent: TeX's accent command and the bare letter.
fn decompose(c: char) -> Option<(&'static str, char)> {
    const TABLE: [(&str, &str, &str); 6] = [
        ("'", "áéíóúýÁÉÍÓÚÝ", "aeiouyAEIOUY"),
        ("`", "àèìòùÀÈÌÒÙ", "aeiouAEIOU"),
        ("^", "âêîôûÂÊÎÔÛ", "aeiouAEIOU"),
        ("~", "ãõÃÕ", "aoAO"),
        ("\"", "äëïöüÿÄËÏÖÜ", "aeiouyAEIOU"),
        ("r", "åÅ", "aA"),
    ];
    TABLE.iter().find_map(|(mark, accented, bare)| {
        let k = accented.chars().position(|x| x == c)?;
        Some((*mark, bare.chars().nth(k)?))
    })
}

impl Explanation {
    /// The calculation as a document: the activity's dates and floats in a table, then each pass
    /// with its bounds, as `lines` says it. `title` heads it (the project's name, say); `date`
    /// writes dates.
    pub fn to_latex(&self, title: &str, name: &str, date: &dyn Fn(Date) -> String) -> String {
        let s = &self.scheduled;
        let mut body = String::new();
        body.push_str(&format!("\\section*{{{} --- {}}}\n", escape(&self.id), escape(name)));
        body.push_str(&format!("\\noindent {}\\par\\medskip\n\n", escape(title)));
        // The row of the schedule this calculation arrives at.
        body.push_str("\\begin{tabular}{llll}\n");
        let row = |a: String, b: String, c: String, d: String| format!("{} & {} & {} & {} \\\\\n", escape(&a), escape(&b), escape(&c), escape(&d));
        body.push_str(&row(tr!("col-early-start"), date(s.early_start), tr!("col-early-finish"), date(s.early_finish)));
        body.push_str(&row(tr!("col-late-start"), date(s.late_start), tr!("col-late-finish"), date(s.late_finish)));
        let days = |m: i64| {
            let d = self.day_minutes.max(1);
            if m % d == 0 { (m / d).to_string() } else { mkperth_i18n::decimal(format!("{:.2}", m as f64 / d as f64).trim_end_matches('0')) }
        };
        body.push_str(&row(tr!("col-total-float"), days(s.total_float_minutes), tr!("col-free-float"), days(s.free_float_minutes)));
        body.push_str("\\end{tabular}\n\n\\medskip\n");
        // The sentences: a pass's heading, its bounds as a list, its results as paragraphs.
        let headings = [tr!("ex-forward"), tr!("ex-backward")];
        let mut listing = false;
        for line in self.lines(date) {
            match line.strip_prefix("  ") {
                Some(bound) => {
                    if !listing {
                        body.push_str("\\begin{itemize}\n");
                        listing = true;
                    }
                    body.push_str(&format!("  \\item {}\n", escape(bound)));
                }
                None => {
                    if listing {
                        body.push_str("\\end{itemize}\n");
                        listing = false;
                    }
                    if headings.contains(&line) {
                        body.push_str(&format!("\n\\subsection*{{{}}}\n", escape(line.trim_end_matches(':'))));
                    } else {
                        body.push_str(&format!("\\noindent {}\\par\n", escape(&line)));
                    }
                }
            }
        }
        if listing {
            body.push_str("\\end{itemize}\n");
        }
        format!(
            "% {}\n\\documentclass[11pt]{{article}}\n\\usepackage[T1]{{fontenc}}\n\\usepackage[margin=2.5cm]{{geometry}}\n\\begin{{document}}\n{body}\\end{{document}}\n",
            escape(&tr!("tex-written-by"))
        )
    }
}
