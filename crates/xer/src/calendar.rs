//! A calendar's `clndr_data`: nodes in parentheses, each `(0||name(attributes)(children))`.
//! `DaysOfWeek` holds a node for each day, 1 for Sunday to 7 for Saturday, whose children are its
//! work periods (`s|08:00|f|12:00`); a day with none isn't worked. `Exceptions` holds the days that
//! differ from the week, each a date (`d|46381`, days from 30 December 1899, as spreadsheets count
//! them) with its own periods; one with none is a day off. As public descriptions of the format
//! give it (Planning Planet's forum): Oracle publishes the column, not its form.

use mkperth_core::Date;
use mkperth_p6xml::tree::Element;

/// The deepest nesting and the most nodes one calendar may have.
const MAX_DEPTH: usize = 16;
const MAX_NODES: usize = 100_000;
/// 30 December 1899, day 0 of the dates in `clndr_data`, in days from 1 January 1970.
const DAY_ZERO: i32 = -25_569;
/// P6 XML's names for the days, Sunday (1) first.
const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

#[derive(Debug, Default)]
struct Node {
    name: String,
    attributes: Vec<String>,
    children: Vec<Node>,
}

impl Node {
    /// The value after `key` among the attributes (`s|08:00` gives `08:00` for `s`).
    fn attribute(&self, key: &str) -> Option<&str> {
        let k = self.attributes.iter().position(|a| a.eq_ignore_ascii_case(key))?;
        self.attributes.get(k + 1).map(String::as_str)
    }

    /// The first node named `name`, searched breadth first, without recursion.
    fn find(&self, name: &str) -> Option<&Node> {
        let mut queue: std::collections::VecDeque<&Node> = self.children.iter().collect();
        while let Some(n) = queue.pop_front() {
            if n.name.eq_ignore_ascii_case(name) {
                return Some(n);
            }
            queue.extend(n.children.iter());
        }
        None
    }
}

/// Parses `clndr_data` into its root node, without recursion; `None` if it isn't well formed.
fn parse(text: &str) -> Option<Node> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0usize;
    let skip = |i: &mut usize| {
        while chars.get(*i).is_some_and(|c| c.is_whitespace() || c.is_control()) {
            *i += 1;
        }
    };
    let until = |i: &mut usize, end: char| -> Option<String> {
        let start = *i;
        while chars.get(*i).is_some_and(|c| *c != end) {
            *i += 1;
        }
        let s: String = chars.get(start..*i)?.iter().collect();
        (chars.get(*i) == Some(&end)).then(|| {
            *i += 1;
            s
        })
    };
    // A node's opening: `(0||name(attributes)(`.
    let open = |i: &mut usize| -> Option<Node> {
        skip(i);
        if chars.get(*i) != Some(&'(') {
            return None;
        }
        *i += 1;
        let head = until(i, '(')?;
        let attributes = until(i, ')')?;
        skip(i);
        if chars.get(*i) != Some(&'(') {
            return None;
        }
        *i += 1;
        Some(Node {
            name: head.rsplit("||").next().unwrap_or_default().trim().to_string(),
            attributes: attributes.split('|').map(|a| a.trim().to_string()).collect(),
            children: Vec::new(),
        })
    };
    let mut stack = vec![open(&mut i)?];
    let mut count = 1usize;
    loop {
        skip(&mut i);
        match chars.get(i)? {
            '(' => {
                count += 1;
                if stack.len() >= MAX_DEPTH || count > MAX_NODES {
                    return None;
                }
                stack.push(open(&mut i)?);
            }
            ')' => {
                // The children's list closes, then the node.
                i += 1;
                skip(&mut i);
                if chars.get(i) != Some(&')') {
                    return None;
                }
                i += 1;
                let node = stack.pop()?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => return Some(node),
                }
            }
            _ => return None,
        }
    }
}

/// `08:00` as P6 XML writes a time, `08:00:00`; `None` unless it is one.
fn time(text: &str) -> Option<String> {
    let (h, m) = text.trim().split_once(':')?;
    let (h, m): (u16, u16) = (h.trim().parse().ok()?, m.trim().get(..2).unwrap_or(m).parse().ok()?);
    (h <= 24 && m < 60).then(|| format!("{h:02}:{m:02}:00"))
}

/// A node's work periods as P6 XML's `WorkTime` elements.
fn work_times(node: &Node) -> String {
    node.children
        .iter()
        .filter_map(|p| Some((time(p.attribute("s")?)?, time(p.attribute("f")?)?)))
        .map(|(s, f)| format!("<WorkTime><Start>{s}</Start><Finish>{f}</Finish></WorkTime>"))
        .collect()
}

/// `clndr_data` as P6 XML's work week and exceptions; nothing for what isn't there (P6 XML's
/// reader then works Monday to Friday).
pub fn to_xml(data: &str) -> String {
    let Some(root) = parse(data) else { return String::new() };
    let mut out = String::new();
    if let Some(week) = root.find("DaysOfWeek") {
        out.push_str("<StandardWorkWeek>");
        for day in &week.children {
            let Some(name) = day.name.parse::<usize>().ok().and_then(|n| n.checked_sub(1)).and_then(|k| DAYS.get(k)) else { continue };
            out.push_str(&format!("<StandardWorkHours><DayOfWeek>{name}</DayOfWeek>{}</StandardWorkHours>", work_times(day)));
        }
        out.push_str("</StandardWorkWeek>");
    }
    if let Some(exceptions) = root.find("Exceptions") {
        out.push_str("<HolidayOrExceptions>");
        for x in &exceptions.children {
            let Some(date) = x
                .attribute("d")
                .and_then(|d| d.trim().parse::<f64>().ok())
                .filter(|d| d.is_finite() && d.abs() < 1e7)
                .and_then(|d| Date::from_days(DAY_ZERO.saturating_add(d.floor() as i32)))
            else {
                continue;
            };
            out.push_str(&format!("<HolidayOrException><Date>{date}T00:00:00</Date>{}</HolidayOrException>", work_times(x)));
        }
        out.push_str("</HolidayOrExceptions>");
    }
    out
}

/// A `WorkTime` list as `clndr_data`'s period nodes; midnight at a period's end is `00:00`.
fn periods(e: &Element) -> String {
    let hhmm = |t: &str| {
        let t = t.get(..5).unwrap_or(t);
        if t == "24:00" { "00:00".to_string() } else { t.to_string() }
    };
    e.all("WorkTime")
        .filter_map(|w| Some((hhmm(w.text_of("Start")?), hhmm(w.text_of("Finish")?))))
        .enumerate()
        .map(|(k, (s, f))| format!("(0||{k}(s|{s}|f|{f})())"))
        .collect()
}

/// A P6 XML calendar's work week and exceptions as `clndr_data`.
pub fn from_xml(calendar: &Element) -> String {
    let mut out = String::from("(0||CalendarData()((0||DaysOfWeek()(");
    for (k, name) in DAYS.iter().enumerate() {
        let day = calendar.child("StandardWorkWeek").and_then(|w| w.all("StandardWorkHours").find(|h| h.text_of("DayOfWeek") == Some(name)));
        out.push_str(&format!("(0||{}()({}))", k + 1, day.map(periods).unwrap_or_default()));
    }
    out.push_str("))(0||Exceptions()(");
    let exceptions = calendar.child("HolidayOrExceptions").into_iter().flat_map(|x| x.all("HolidayOrException"));
    for (k, x) in exceptions.enumerate() {
        let Some(date) = x.text_of("Date").and_then(|d| Date::parse(d.get(..10).unwrap_or(d))) else { continue };
        out.push_str(&format!("(0||{k}(d|{})({}))", date.days().saturating_sub(DAY_ZERO), periods(x)));
    }
    out.push_str("))))");
    out
}
