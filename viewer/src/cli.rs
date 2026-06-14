//! Hand-rolled argument dispatch — a prax-style `take()` cursor over the
//! argument list, with explicit per-subcommand flag handling. No argument-
//! parsing dependency, no `HashMap`: flags are matched by name and an unknown
//! flag, an unknown command, or a missing required argument is a loud error
//! (the caller appends the usage block), never a silent default.

use crate::{
    help_text, run_colonist, run_dashboard, run_price, scenarios_text, usage, DEFAULT_RUN_TICKS,
    DEFAULT_SEED,
};

/// A cursor over the argument list. `take()` pops the next token (advancing the
/// position) — the lab `prax` CLI's dispatch idiom.
struct Cursor {
    args: Vec<String>,
    pos: usize,
}

impl Cursor {
    fn new(args: &[String]) -> Self {
        Self {
            args: args.to_vec(),
            pos: 0,
        }
    }

    fn take(&mut self) -> Option<String> {
        let item = self.args.get(self.pos).cloned();
        if item.is_some() {
            self.pos += 1;
        }
        item
    }
}

/// Dispatch the program arguments to a subcommand. On error the message is
/// returned with the usage block appended; on success the text to print.
pub fn dispatch(args: &[String]) -> Result<String, String> {
    inner(args).map_err(with_usage)
}

fn inner(args: &[String]) -> Result<String, String> {
    let mut cursor = Cursor::new(args);
    let command = match cursor.take() {
        Some(command) => command,
        // No arguments at all: show help rather than error — help is not a
        // command default being silently chosen, it is the empty invocation.
        None => return Ok(help_text()),
    };
    match command.as_str() {
        "help" | "-h" | "--help" => {
            reject_remaining(&mut cursor, "help")?;
            Ok(help_text())
        }
        "scenarios" => {
            reject_remaining(&mut cursor, "scenarios")?;
            Ok(scenarios_text())
        }
        "run" => parse_run(&mut cursor),
        "inspect" => parse_inspect(&mut cursor),
        other => Err(format!(
            "unknown command: {other:?} (expected one of: run, inspect, scenarios, help)"
        )),
    }
}

fn with_usage(message: String) -> String {
    format!("{message}\n\n{}", usage())
}

fn parse_run(cursor: &mut Cursor) -> Result<String, String> {
    let mut scenario: Option<String> = None;
    let mut ticks = DEFAULT_RUN_TICKS;
    let mut seed = DEFAULT_SEED;

    while let Some(arg) = cursor.take() {
        match arg.as_str() {
            "--ticks" => ticks = take_u64(cursor, "--ticks")?,
            "--seed" => seed = take_u64(cursor, "--seed")?,
            flag if flag.starts_with("--") => {
                return Err(format!("unknown flag for `run`: {flag:?}"));
            }
            positional => set_scenario(&mut scenario, positional, "run")?,
        }
    }

    let scenario = scenario.ok_or_else(|| "missing required <scenario> for `run`".to_string())?;
    run_dashboard(&scenario, ticks, seed)
}

fn parse_inspect(cursor: &mut Cursor) -> Result<String, String> {
    let kind = cursor
        .take()
        .ok_or_else(|| "missing inspector kind: expected `price` or `colonist`".to_string())?;
    match kind.as_str() {
        "price" => parse_inspect_price(cursor),
        "colonist" => parse_inspect_colonist(cursor),
        other => Err(format!(
            "unknown inspector: {other:?} (expected `price` or `colonist`)"
        )),
    }
}

fn parse_inspect_price(cursor: &mut Cursor) -> Result<String, String> {
    let mut scenario: Option<String> = None;
    let mut good: Option<String> = None;
    let mut at_tick: Option<u64> = None;
    let mut ticks: Option<u64> = None;
    let mut seed = DEFAULT_SEED;

    while let Some(arg) = cursor.take() {
        match arg.as_str() {
            "--good" => good = Some(take_value(cursor, "--good")?),
            "--at-tick" => at_tick = Some(take_u64(cursor, "--at-tick")?),
            "--ticks" => ticks = Some(take_u64(cursor, "--ticks")?),
            "--seed" => seed = take_u64(cursor, "--seed")?,
            flag if flag.starts_with("--") => {
                return Err(format!("unknown flag for `inspect price`: {flag:?}"));
            }
            positional => set_scenario(&mut scenario, positional, "inspect price")?,
        }
    }

    let scenario =
        scenario.ok_or_else(|| "missing required <scenario> for `inspect price`".to_string())?;
    let good =
        good.ok_or_else(|| "missing required --good NAME for `inspect price`".to_string())?;
    run_price(&scenario, &good, at_tick, ticks, seed)
}

fn parse_inspect_colonist(cursor: &mut Cursor) -> Result<String, String> {
    let mut scenario: Option<String> = None;
    let mut id: Option<usize> = None;
    let mut at_tick: Option<u64> = None;
    let mut ticks: Option<u64> = None;
    let mut seed = DEFAULT_SEED;

    while let Some(arg) = cursor.take() {
        match arg.as_str() {
            "--id" => id = Some(take_usize(cursor, "--id")?),
            "--at-tick" => at_tick = Some(take_u64(cursor, "--at-tick")?),
            "--ticks" => ticks = Some(take_u64(cursor, "--ticks")?),
            "--seed" => seed = take_u64(cursor, "--seed")?,
            flag if flag.starts_with("--") => {
                return Err(format!("unknown flag for `inspect colonist`: {flag:?}"));
            }
            positional => set_scenario(&mut scenario, positional, "inspect colonist")?,
        }
    }

    let scenario =
        scenario.ok_or_else(|| "missing required <scenario> for `inspect colonist`".to_string())?;
    let id = id.ok_or_else(|| "missing required --id N for `inspect colonist`".to_string())?;
    run_colonist(&scenario, id, at_tick, ticks, seed)
}

/// Record the (single) positional scenario, erroring on a second one.
fn set_scenario(scenario: &mut Option<String>, value: &str, command: &str) -> Result<(), String> {
    if scenario.is_some() {
        return Err(format!(
            "unexpected argument: {value:?} (`{command}` takes a single <scenario>)"
        ));
    }
    *scenario = Some(value.to_string());
    Ok(())
}

/// No-argument commands still reject stray tokens: typos must be loud.
fn reject_remaining(cursor: &mut Cursor, command: &str) -> Result<(), String> {
    let Some(arg) = cursor.take() else {
        return Ok(());
    };
    if arg.starts_with("--") {
        Err(format!("unknown flag for `{command}`: {arg:?}"))
    } else {
        Err(format!("unexpected argument for `{command}`: {arg:?}"))
    }
}

/// Take the next token as a flag's string value, erroring if it is missing.
fn take_value(cursor: &mut Cursor, flag: &str) -> Result<String, String> {
    let value = cursor
        .take()
        .ok_or_else(|| format!("{flag} requires a value"))?;
    if value.starts_with("--") {
        return Err(format!("{flag} requires a value, got flag {value:?}"));
    }
    Ok(value)
}

/// Take the next token as a `u64` flag value.
fn take_u64(cursor: &mut Cursor, flag: &str) -> Result<u64, String> {
    let value = take_value(cursor, flag)?;
    value.parse::<u64>().map_err(|_| {
        format!("invalid value for {flag}: {value:?} (expected a non-negative integer)")
    })
}

/// Take the next token as a `usize` flag value.
fn take_usize(cursor: &mut Cursor, flag: &str) -> Result<usize, String> {
    let value = take_value(cursor, flag)?;
    value.parse::<usize>().map_err(|_| {
        format!("invalid value for {flag}: {value:?} (expected a non-negative integer)")
    })
}
