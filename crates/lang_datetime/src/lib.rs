//! `lang_datetime`: calendar dates, wall-clock times, and time zones for
//! `std.datetime` (jiff, with the system's time zone database).
//!
//! Values cross as records: a date `{year, month, day}`, a time `{hour,
//! minute, second, nanosecond}`, a date-time `{date, time}`, a span of
//! calendar and clock units, and a zoned date-time `{datetime,
//! offset_seconds, zone, timestamp_nanos}`. Every function is pure but
//! `system_zone`.

use jiff::civil::{Date, DateTime, Time};
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp, Zoned};
use lang_native::{Args, Error, Handler, ModuleDef, Outcome, Value};

fn bad(e: impl std::fmt::Display) -> Error {
    Error::new("datetime/invalid", e.to_string())
}

fn field(v: &Value, name: &str) -> Result<i64, Error> {
    match v.field(name) {
        Some(Value::Int(n)) => Ok(*n),
        _ => Err(Error::new("datetime/argument", format!("missing `{name}`"))),
    }
}

fn narrow<T: TryFrom<i64>>(n: i64, what: &str) -> Result<T, Error> {
    T::try_from(n).map_err(|_| Error::new("datetime/invalid", format!("{what} {n} is out of range")))
}

fn date_of(v: &Value) -> Result<Date, Error> {
    Date::new(
        narrow(field(v, "year")?, "year")?,
        narrow(field(v, "month")?, "month")?,
        narrow(field(v, "day")?, "day")?,
    )
    .map_err(bad)
}

fn time_of(v: &Value) -> Result<Time, Error> {
    Time::new(
        narrow(field(v, "hour")?, "hour")?,
        narrow(field(v, "minute")?, "minute")?,
        narrow(field(v, "second")?, "second")?,
        narrow(field(v, "nanosecond")?, "nanosecond")?,
    )
    .map_err(bad)
}

fn datetime_of(v: &Value) -> Result<DateTime, Error> {
    let d = v.field("date").ok_or_else(|| Error::new("datetime/argument", "missing `date`"))?;
    let t = v.field("time").ok_or_else(|| Error::new("datetime/argument", "missing `time`"))?;
    Ok(DateTime::from_parts(date_of(d)?, time_of(t)?))
}

fn span_of(v: &Value) -> Result<Span, Error> {
    let mut s = Span::new();
    type SetUnit = fn(Span, i64) -> Result<Span, jiff::Error>;
    let units: [(&str, SetUnit); 8] = [
        ("years", |s, n| s.try_years(n)),
        ("months", |s, n| s.try_months(n)),
        ("weeks", |s, n| s.try_weeks(n)),
        ("days", |s, n| s.try_days(n)),
        ("hours", |s, n| s.try_hours(n)),
        ("minutes", |s, n| s.try_minutes(n)),
        ("seconds", |s, n| s.try_seconds(n)),
        ("nanoseconds", |s, n| s.try_nanoseconds(n)),
    ];
    for (name, set) in units {
        let n = field(v, name)?;
        if n != 0 {
            s = set(s, n).map_err(bad)?;
        }
    }
    Ok(s)
}

fn date_value(d: Date) -> Value {
    Value::Map(vec![
        ("year".into(), Value::Int(d.year().into())),
        ("month".into(), Value::Int(d.month().into())),
        ("day".into(), Value::Int(d.day().into())),
    ])
}

fn time_value(t: Time) -> Value {
    Value::Map(vec![
        ("hour".into(), Value::Int(t.hour().into())),
        ("minute".into(), Value::Int(t.minute().into())),
        ("second".into(), Value::Int(t.second().into())),
        ("nanosecond".into(), Value::Int(t.subsec_nanosecond().into())),
    ])
}

fn datetime_value(dt: DateTime) -> Value {
    Value::Map(vec![("date".into(), date_value(dt.date())), ("time".into(), time_value(dt.time()))])
}

fn zone_name(tz: &TimeZone) -> String {
    tz.iana_name().map(str::to_string).unwrap_or_else(|| {
        let off = tz.to_offset(Timestamp::now());
        off.to_string()
    })
}

fn zoned_value(z: &Zoned) -> Result<Value, Error> {
    let nanos = i64::try_from(z.timestamp().as_nanosecond())
        .map_err(|_| Error::new("datetime/invalid", "the time is outside the timestamp range"))?;
    Ok(Value::Map(vec![
        ("datetime".into(), datetime_value(z.datetime())),
        ("offset_seconds".into(), Value::Int(z.offset().seconds().into())),
        ("zone".into(), Value::Str(zone_name(z.time_zone()))),
        ("timestamp_nanos".into(), Value::Int(nanos)),
    ]))
}

fn zone(name: &str) -> Result<TimeZone, Error> {
    if name.eq_ignore_ascii_case("utc") || name == "Z" {
        return Ok(TimeZone::UTC);
    }
    TimeZone::get(name).map_err(|e| Error::new("datetime/zone", e.to_string()))
}

fn timestamp(nanos: i64) -> Result<Timestamp, Error> {
    Timestamp::from_nanosecond(nanos.into()).map_err(bad)
}

fn zoned_of(v: &Value) -> Result<Zoned, Error> {
    let nanos = field(v, "timestamp_nanos")?;
    let name = match v.field("zone") {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(Error::new("datetime/argument", "missing `zone`")),
    };
    let tz = match zone(&name) {
        Ok(tz) => tz,
        // a fixed offset (`+02:00`) from a value parsed without a zone name
        Err(_) => {
            let secs = narrow::<i32>(field(v, "offset_seconds")?, "offset")?;
            TimeZone::fixed(jiff::tz::Offset::from_seconds(secs).map_err(bad)?)
        }
    };
    Ok(timestamp(nanos)?.to_zoned(tz))
}

// ---------------------------------------------------------------- functions

fn date(a: &Args) -> Outcome {
    Ok(date_value(
        Date::new(narrow(a.int(0)?, "year")?, narrow(a.int(1)?, "month")?, narrow(a.int(2)?, "day")?)
            .map_err(bad)?,
    ))
}

fn time(a: &Args) -> Outcome {
    Ok(time_value(
        Time::new(
            narrow(a.int(0)?, "hour")?,
            narrow(a.int(1)?, "minute")?,
            narrow(a.int(2)?, "second")?,
            narrow(a.int(3)?, "nanosecond")?,
        )
        .map_err(bad)?,
    ))
}

fn arg(a: &Args, i: usize) -> Result<&Value, Error> {
    a.get(i).ok_or_else(|| Error::new("datetime/argument", format!("argument {}", i + 1)))
}

fn date_add(a: &Args) -> Outcome {
    let d = date_of(arg(a, 0)?)?;
    Ok(date_value(d.checked_add(span_of(arg(a, 1)?)?).map_err(bad)?))
}

fn date_info(a: &Args) -> Outcome {
    let d = date_of(arg(a, 0)?)?;
    Ok(Value::Map(vec![
        ("weekday".into(), Value::Int(d.weekday().to_monday_one_offset().into())),
        ("day_of_year".into(), Value::Int(d.day_of_year().into())),
        ("days_in_month".into(), Value::Int(d.days_in_month().into())),
        ("leap_year".into(), Value::Bool(d.in_leap_year())),
    ]))
}

/// Days from the first date to the second.
fn days_between(a: &Args) -> Outcome {
    let (x, y) = (date_of(arg(a, 0)?)?, date_of(arg(a, 1)?)?);
    let span = x.until((jiff::Unit::Day, y)).map_err(bad)?;
    Ok(Value::Int(span.get_days().into()))
}

fn datetime_add(a: &Args) -> Outcome {
    let dt = datetime_of(arg(a, 0)?)?;
    Ok(datetime_value(dt.checked_add(span_of(arg(a, 1)?)?).map_err(bad)?))
}

/// Add in a zone: calendar units by the wall clock (a day across a DST
/// change is 23 or 25 hours), clock units as elapsed time.
fn zoned_add(a: &Args) -> Outcome {
    let z = zoned_of(arg(a, 0)?)?;
    zoned_value(&z.checked_add(span_of(arg(a, 1)?)?).map_err(bad)?)
}

fn in_zone(a: &Args) -> Outcome {
    zoned_value(&timestamp(a.int(0)?)?.to_zoned(zone(a.str(1)?)?))
}

/// A wall-clock time in a zone; a time skipped by a DST change moves
/// forward, and a repeated one takes the earlier instant.
fn at_zone(a: &Args) -> Outcome {
    let dt = datetime_of(arg(a, 0)?)?;
    zoned_value(&zone(a.str(1)?)?.to_ambiguous_zoned(dt).compatible().map_err(bad)?)
}

fn parse_date(a: &Args) -> Outcome {
    Ok(date_value(a.str(0)?.trim().parse::<Date>().map_err(bad)?))
}

fn parse_time(a: &Args) -> Outcome {
    Ok(time_value(a.str(0)?.trim().parse::<Time>().map_err(bad)?))
}

fn parse_datetime(a: &Args) -> Outcome {
    Ok(datetime_value(a.str(0)?.trim().parse::<DateTime>().map_err(bad)?))
}

/// RFC 9557 (`2026-10-16T12:30:00+02:00[Europe/Berlin]`), or RFC 3339
/// with an offset.
fn parse_zoned(a: &Args) -> Outcome {
    let s = a.str(0)?.trim();
    match s.parse::<Zoned>() {
        Ok(z) => zoned_value(&z),
        Err(e) => match s.parse::<Timestamp>() {
            Ok(ts) => {
                // keep the written offset
                let off =
                    jiff::fmt::temporal::Pieces::parse(s).ok().and_then(|p| p.offset()).and_then(
                        |o| match o {
                            jiff::fmt::temporal::PiecesOffset::Numeric(n) => Some(n.offset()),
                            jiff::fmt::temporal::PiecesOffset::Zulu => Some(jiff::tz::Offset::UTC),
                            _ => None,
                        },
                    );
                zoned_value(&ts.to_zoned(TimeZone::fixed(off.unwrap_or(jiff::tz::Offset::UTC))))
            }
            Err(_) => Err(bad(e)),
        },
    }
}

/// An instant written with an offset (RFC 3339), as nanoseconds since the
/// Unix epoch.
fn parse_timestamp(a: &Args) -> Outcome {
    let ts = a.str(0)?.trim().parse::<Timestamp>().map_err(bad)?;
    i64::try_from(ts.as_nanosecond()).map(Value::Int).map_err(bad)
}

fn format_date(a: &Args) -> Outcome {
    let d = date_of(arg(a, 1)?)?;
    jiff::fmt::strtime::format(a.str(0)?, d).map(Value::Str).map_err(bad)
}

fn format_datetime(a: &Args) -> Outcome {
    let dt = datetime_of(arg(a, 1)?)?;
    jiff::fmt::strtime::format(a.str(0)?, dt).map(Value::Str).map_err(bad)
}

fn format_zoned(a: &Args) -> Outcome {
    let z = zoned_of(arg(a, 1)?)?;
    jiff::fmt::strtime::format(a.str(0)?, &z).map(Value::Str).map_err(bad)
}

/// `strptime`: a date-time by a format (`%Y-%m-%d %H:%M`).
fn parse_datetime_with(a: &Args) -> Outcome {
    let tm = jiff::fmt::strtime::parse(a.str(0)?, a.str(1)?).map_err(bad)?;
    Ok(datetime_value(tm.to_datetime().map_err(bad)?))
}

/// `strptime` for a format with an offset (`%z`) or zone (`%Q`).
fn parse_zoned_with(a: &Args) -> Outcome {
    let tm = jiff::fmt::strtime::parse(a.str(0)?, a.str(1)?).map_err(bad)?;
    zoned_value(&tm.to_zoned().map_err(bad)?)
}

fn zoned_text(a: &Args) -> Outcome {
    let z = zoned_of(arg(a, 0)?)?;
    Ok(Value::Str(if z.time_zone().iana_name().is_some() {
        z.to_string()
    } else {
        z.timestamp().display_with_offset(z.offset()).to_string()
    }))
}

fn timestamp_text(a: &Args) -> Outcome {
    Ok(Value::Str(timestamp(a.int(0)?)?.to_string()))
}

fn check_zone(a: &Args) -> Outcome {
    let tz = zone(a.str(0)?)?;
    Ok(Value::Str(zone_name(&tz)))
}

/// The system's time zone (`TZ`, else `/etc/localtime`); UTC if unknown.
fn system_zone(_: &Args) -> Outcome {
    Ok(Value::Str(TimeZone::try_system().map(|tz| zone_name(&tz)).unwrap_or_else(|_| "UTC".into())))
}

pub static DEF: ModuleDef = ModuleDef {
    name: "lang_datetime",
    functions: &[
        ("date", Handler::Inline(date)),
        ("time", Handler::Inline(time)),
        ("date_add", Handler::Inline(date_add)),
        ("date_info", Handler::Inline(date_info)),
        ("days_between", Handler::Inline(days_between)),
        ("datetime_add", Handler::Inline(datetime_add)),
        ("zoned_add", Handler::Inline(zoned_add)),
        ("in_zone", Handler::Inline(in_zone)),
        ("at_zone", Handler::Inline(at_zone)),
        ("parse_date", Handler::Inline(parse_date)),
        ("parse_time", Handler::Inline(parse_time)),
        ("parse_datetime", Handler::Inline(parse_datetime)),
        ("parse_zoned", Handler::Inline(parse_zoned)),
        ("parse_timestamp", Handler::Inline(parse_timestamp)),
        ("format_date", Handler::Inline(format_date)),
        ("format_datetime", Handler::Inline(format_datetime)),
        ("format_zoned", Handler::Inline(format_zoned)),
        ("parse_datetime_with", Handler::Inline(parse_datetime_with)),
        ("parse_zoned_with", Handler::Inline(parse_zoned_with)),
        ("zoned_text", Handler::Inline(zoned_text)),
        ("timestamp_text", Handler::Inline(timestamp_text)),
        ("check_zone", Handler::Inline(check_zone)),
        ("system_zone", Handler::Inline(system_zone)),
    ],
    drop_handle: None,
    cancel: None,
    recorded: &[("system_zone", "datetime system zone", 0)],
    pure: &[
        "date",
        "time",
        "date_add",
        "date_info",
        "days_between",
        "datetime_add",
        "zoned_add",
        "in_zone",
        "at_zone",
        "parse_date",
        "parse_time",
        "parse_datetime",
        "parse_zoned",
        "parse_timestamp",
        "format_date",
        "format_datetime",
        "format_zoned",
        "parse_datetime_with",
        "parse_zoned_with",
        "zoned_text",
        "timestamp_text",
        "check_zone",
    ],
};

lang_native::export!("lang_datetime", DEF);
