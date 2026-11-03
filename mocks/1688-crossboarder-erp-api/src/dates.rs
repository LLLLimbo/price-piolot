use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, Offset, TimeZone};
use chrono_tz::Tz;
use serde_json::Value;

pub const DEFAULT_PATTERN: &str = "yyyyMMddHHmmssSSSZ";

pub struct Options {
    format: String,
    zone: Zone,
}
enum Zone {
    Named(Tz),
    Offset(FixedOffset),
}
impl Options {
    pub fn new(pattern: Option<&str>, zone: Option<&str>) -> Result<Self, String> {
        let name = zone.unwrap_or("UTC");
        let zone = if let Ok(zone) = name.parse::<Tz>() {
            Zone::Named(zone)
        } else {
            let offset = name
                .strip_prefix("GMT")
                .or_else(|| name.strip_prefix("UTC"))
                .unwrap_or(name);
            let sign = match offset.as_bytes().first() {
                Some(b'+') => 1,
                Some(b'-') => -1,
                _ => return Err("invalid _aop_timeZone".into()),
            };
            let digits = offset[1..].replace(':', "");
            if ![2, 4].contains(&digits.len()) || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err("invalid _aop_timeZone".into());
            }
            let hours = digits[..2].parse::<i32>().unwrap();
            let minutes = if digits.len() == 4 {
                digits[2..].parse::<i32>().unwrap()
            } else {
                0
            };
            if hours > 23 || minutes > 59 {
                return Err("invalid _aop_timeZone".into());
            }
            Zone::Offset(
                FixedOffset::east_opt(sign * (hours * 3600 + minutes * 60))
                    .ok_or("invalid _aop_timeZone")?,
            )
        };
        Ok(Self {
            format: translate(pattern.unwrap_or(DEFAULT_PATTERN))?,
            zone,
        })
    }
    pub fn from_value(input: &Value) -> Result<Self, String> {
        Self::new(
            input.get("_aop_datePattern").and_then(Value::as_str),
            input.get("_aop_timeZone").and_then(Value::as_str),
        )
    }
    pub fn parse(&self, value: &str) -> Result<i64, String> {
        let parse_format = self
            .format
            .replace("__ISO1__", "%#z")
            .replace("__ISO2__", "%#z")
            .replace("__ISO3__", "%#z");
        if let Ok(date) = DateTime::parse_from_str(value, &parse_format) {
            return Ok(date.timestamp_millis());
        }
        let local = NaiveDateTime::parse_from_str(value, &self.format)
            .or_else(|_| {
                NaiveDate::parse_from_str(value, &self.format)
                    .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
            })
            .map_err(|_| "invalid date".to_owned())?;
        // Ambiguous and nonexistent wall times are rejected instead of silently moving an instant.
        match self.zone {
            Zone::Named(zone) => zone
                .from_local_datetime(&local)
                .single()
                .map(|d| d.timestamp_millis()),
            Zone::Offset(zone) => zone
                .from_local_datetime(&local)
                .single()
                .map(|d| d.timestamp_millis()),
        }
        .ok_or_else(|| "ambiguous or nonexistent local date".into())
    }
    pub fn format(&self, millis: i64) -> Result<String, String> {
        let date = DateTime::from_timestamp_millis(millis).ok_or("date outside supported range")?;
        let offset = match self.zone {
            Zone::Named(zone) => date.with_timezone(&zone).offset().fix(),
            Zone::Offset(zone) => zone,
        };
        let mut output = match self.zone {
            Zone::Named(zone) => date.with_timezone(&zone).format(&self.format).to_string(),
            Zone::Offset(zone) => date.with_timezone(&zone).format(&self.format).to_string(),
        };
        let date = date.with_timezone(&offset);
        for (marker, format) in [("__ISO1__", "%z"), ("__ISO2__", "%z"), ("__ISO3__", "%:z")] {
            let mut value = if offset.local_minus_utc() == 0 {
                "Z".into()
            } else {
                date.format(format).to_string()
            };
            if marker == "__ISO1__" && value != "Z" {
                value.truncate(3);
            }
            output = output.replace(marker, &value);
        }
        Ok(output)
    }
}

fn translate(pattern: &str) -> Result<String, String> {
    if pattern.is_empty() || pattern.len() > 256 {
        return Err("invalid _aop_datePattern".into());
    }
    let chars: Vec<char> = pattern.chars().collect();
    let mut output = String::new();
    let mut quoted = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            if chars.get(i + 1) == Some(&'\'') {
                output.push('\'');
                i += 2;
                continue;
            }
            quoted = !quoted;
            i += 1;
            continue;
        }
        if quoted || !c.is_ascii_alphabetic() {
            if c == '%' {
                output.push('%');
            }
            output.push(c);
            i += 1;
            continue;
        }
        let start = i;
        while chars.get(i) == Some(&c) {
            i += 1;
        }
        let count = i - start;
        let token = match c {
            'y' if count == 2 => "%y",
            'y' => "%Y",
            'Y' if count == 2 => "%g",
            'Y' => "%G",
            'M' | 'L' if count >= 4 => "%B",
            'M' | 'L' if count == 3 => "%b",
            'M' | 'L' if count == 1 => "%-m",
            'M' | 'L' => "%m",
            'd' if count == 1 => "%-d",
            'd' => "%d",
            'D' => "%j",
            'H' if count == 1 => "%-H",
            'H' => "%H",
            'h' if count == 1 => "%-I",
            'h' => "%I",
            'm' if count == 1 => "%-M",
            'm' => "%M",
            's' if count == 1 => "%-S",
            's' => "%S",
            'S' if count == 3 => "%3f",
            'a' => "%p",
            'E' if count >= 4 => "%A",
            'E' => "%a",
            'u' => "%u",
            'w' => "%V",
            'z' => "%Z",
            'Z' => "%z",
            'X' if count == 1 => "__ISO1__",
            'X' if count == 2 => "__ISO2__",
            'X' if count == 3 => "__ISO3__",
            'G' => "AD",
            _ => {
                return Err(format!(
                    "unsupported date pattern token {}",
                    chars[start..i].iter().collect::<String>()
                ))
            }
        };
        output.push_str(token);
    }
    if quoted {
        return Err("unterminated date pattern quote".into());
    }
    Ok(output)
}

pub fn internal_millis(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value.as_str().and_then(|value| {
            Options::new(None, None)
                .ok()?
                .parse(value)
                .ok()
                .or_else(|| {
                    DateTime::parse_from_rfc3339(value)
                        .ok()
                        .map(|d| d.timestamp_millis())
                })
                .or_else(|| {
                    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                        .ok()
                        .map(|d| d.and_utc().timestamp_millis())
                })
        })
    })
}
pub fn task_time(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .unwrap()
        .with_timezone(&chrono_tz::Asia::Shanghai)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timezone_roundtrip_and_dst() {
        let cn = Options::new(Some("yyyy-MM-dd HH:mm:ss"), Some("Asia/Shanghai")).unwrap();
        let utc = Options::new(Some("yyyy-MM-dd'T'HH:mm:ss.SSSXXX"), Some("UTC")).unwrap();
        let time = cn.parse("2026-10-26 08:00:00").unwrap();
        assert_eq!(utc.format(time).unwrap(), "2026-10-26T00:00:00.000Z");
        assert_eq!(utc.parse("2026-10-26T08:00:00.000+08:00").unwrap(), time);
        assert_eq!(utc.parse("2026-10-26T00:00:00.000Z").unwrap(), time);
        let ny = Options::new(Some("yyyy-MM-dd HH:mm:ss"), Some("America/New_York")).unwrap();
        assert!(ny.parse("2026-03-08 02:30:00").is_err());
        assert!(ny.parse("2026-11-01 01:30:00").is_err());
        assert_eq!(ny.format(time).unwrap(), "2026-10-25 20:00:00");
        assert_eq!(
            Options::new(Some("yyyy-MM-dd HH:mm:ss z"), Some("America/New_York"))
                .unwrap()
                .format(time)
                .unwrap(),
            "2026-10-25 20:00:00 EDT"
        );
        assert_eq!(
            Options::new(Some("yyyy/MM/dd"), Some("GMT+08:00"))
                .unwrap()
                .parse("2026/10/26")
                .unwrap(),
            time - 8 * 3600000
        );
    }
    #[test]
    fn rejects_invalid_options_and_dates() {
        assert!(Options::new(None, Some("Not/AZone")).is_err());
        assert!(Options::new(None, Some("GMT+25:00")).is_err());
        assert!(Options::new(Some("yyyy'"), None).is_err());
        assert!(Options::new(None, None)
            .unwrap()
            .parse("20260230000000000+0000")
            .is_err());
    }
}
