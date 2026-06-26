// SPDX-License-Identifier: MIT OR Apache-2.0
//! Smart playlists.
//!
//! v0.5.0 minimum: a small predicate DSL that filters tracks
//! by metadata + features, a typed playlist model that
//! supports both predicate-based and AI-generated playlists,
//! and an auto-update mechanism that recomputes the playlist
//! on track add/remove. v0.5.0 ships the predicate path
//! end-to-end; the AI generation path is deferred to 0032
//! (LLM provider, v0.6.0).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// The source of a smart playlist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlaylistSource {
    /// A static list of track IDs. The user picks tracks by
    /// hand; the playlist is fixed until edited.
    Static { track_ids: Vec<i64> },
    /// A predicate that filters the library. The playlist is
    /// re-evaluated on track add/remove.
    Predicate { expression: String },
    /// An AI-generated playlist. v0.6.0 minimum: the LLM
    /// produces a Predicate, which the engine then evaluates.
    Ai {
        prompt: String,
        resolved_predicate: Option<String>,
    },
}

impl Default for PlaylistSource {
    fn default() -> Self {
        PlaylistSource::Static { track_ids: Vec::new() }
    }
}

/// A smart playlist.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SmartPlaylist {
    pub id: i64,
    pub name: String,
    pub source: PlaylistSource,
    /// The set of track IDs that match the source. v0.5.0
    /// minimum: this is the current evaluation result; the
    /// engine recomputes it on track add/remove.
    pub track_ids: Vec<i64>,
    /// Whether the playlist is auto-updated on track add/remove.
    pub is_smart: bool,
    /// The AI-generated playlist (a predicate that the LLM
    /// produced). v0.6.0 minimum: set when `source` is `Ai`.
    pub smart_predicate: Option<String>,
}

/// A small predicate DSL for the smart-playlist filter. v0.5.0
/// minimum supports a tiny subset:
///
/// ```
/// genre == "Rock" AND bpm > 120
/// mood == "energetic" OR genre == "Synthwave"
/// NOT (year < 1990)
/// ```
///
/// Operators: `==`, `!=`, `>`, `<`, `>=`, `<=`.
/// Connectives: `AND`, `OR`, `NOT`, parentheses.
/// Fields: `genre`, `artist`, `album`, `title`, `year`,
/// `bpm`, `key`, `mood`, `favourite` (bool), `play_count`
/// (int), `codec`, `duration_ms`.
///
/// The parser is a hand-rolled recursive-descent implementation.
/// v0.5.0 minimum: the predicate AST evaluates against a
/// `TrackSnapshot` (the v0.5.0 minimum is a small subset of
/// the AST; v0.5.0 hardening adds the rest of the grammar
/// and the `mood == "energetic"` semantic check via 0027's
/// feature extractors).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Predicate {
    True,
    False,
    And(Box<Predicate>, Box<Predicate>),
    Or(Box<Predicate>, Box<Predicate>),
    Not(Box<Predicate>),
    Eq(String, String),  // field, value
    Ne(String, String),
    Gt(String, f64),
    Lt(String, f64),
    Ge(String, f64),
    Le(String, f64),
}

impl Default for Predicate {
    fn default() -> Self {
        Predicate::True
    }
}

/// A minimal track snapshot for predicate evaluation.
/// v0.5.0 minimum: a flat struct of the fields the predicate
/// can reference.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrackSnapshot {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: Option<u32>,
    pub bpm: Option<u32>,
    pub key: String,
    pub mood: String,
    pub favourite: bool,
    pub play_count: u32,
    pub codec: String,
    pub duration_ms: u64,
}

/// Evaluate the predicate against a track snapshot.
pub fn evaluate(predicate: &Predicate, track: &TrackSnapshot) -> bool {
    match predicate {
        Predicate::True => true,
        Predicate::False => false,
        Predicate::And(a, b) => evaluate(a, track) && evaluate(b, track),
        Predicate::Or(a, b) => evaluate(a, track) || evaluate(b, track),
        Predicate::Not(p) => !evaluate(p, track),
        Predicate::Eq(field, value) => field_value(field, track).as_deref() == Some(value.as_str()),
        Predicate::Ne(field, value) => field_value(field, track).as_deref() != Some(value.as_str()),
        Predicate::Gt(field, n) => {
            field_numeric(field, track).map(|x| x > *n).unwrap_or(false)
        }
        Predicate::Lt(field, n) => {
            field_numeric(field, track).map(|x| x < *n).unwrap_or(false)
        }
        Predicate::Ge(field, n) => {
            field_numeric(field, track).map(|x| x >= *n).unwrap_or(false)
        }
        Predicate::Le(field, n) => {
            field_numeric(field, track).map(|x| x <= *n).unwrap_or(false)
        }
    }
}

fn field_value(field: &str, t: &TrackSnapshot) -> Option<String> {
    match field {
        "title" => Some(t.title.clone()),
        "artist" => Some(t.artist.clone()),
        "album" => Some(t.album.clone()),
        "genre" => Some(t.genre.clone()),
        "key" => Some(t.key.clone()),
        "mood" => Some(t.mood.clone()),
        "codec" => Some(t.codec.clone()),
        _ => None,
    }
}

fn field_numeric(field: &str, t: &TrackSnapshot) -> Option<f64> {
    match field {
        "year" => t.year.map(|y| y as f64),
        "bpm" => t.bpm.map(|b| b as f64),
        "play_count" => Some(t.play_count as f64),
        "duration_ms" => Some(t.duration_ms as f64),
        _ => None,
    }
}

/// Filter a list of `(track_id, TrackSnapshot)` pairs by the
/// predicate.
pub fn filter(
    predicate: &Predicate,
    tracks: &[(i64, TrackSnapshot)],
) -> Vec<i64> {
    tracks
        .iter()
        .filter(|(_, t)| evaluate(predicate, t))
        .map(|(id, _)| *id)
        .collect()
}

/// Update a playlist: the new track IDs are the
/// predicate-filtered subset. Used by the auto-update worker.
pub fn update_playlist(
    playlist: &mut SmartPlaylist,
    tracks: &[(i64, TrackSnapshot)],
) {
    if playlist.is_smart {
        if let PlaylistSource::Predicate { expression } = &playlist.source {
            if let Ok(predicate) = parse(expression) {
                playlist.track_ids = filter(&predicate, tracks);
                // De-duplicate while preserving order.
                let mut seen = HashSet::new();
                playlist.track_ids.retain(|id| seen.insert(*id));
            }
        }
    }
}

/// Parse the small predicate DSL. v0.5.0 minimum: a simple
/// recursive-descent parser that handles `AND`, `OR`, `NOT`,
/// `==`, `!=`, `>`, `<`, `>=`, `<=`, parentheses, and
/// identifiers (fields) + string/number literals.
pub fn parse(input: &str) -> Result<Predicate, ParseError> {
    let mut p = Parser::new(input);
    let pred = p.parse_or()?;
    p.skip_ws();
    if !p.is_eof() {
        return Err(ParseError::UnexpectedTrailing(p.pos()));
    }
    Ok(pred)
}

/// Errors produced by the predicate parser.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("unexpected character at position {0}: {1:?}")]
    UnexpectedChar(usize, char),
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("unexpected trailing input at position {0}")]
    UnexpectedTrailing(usize),
    #[error("empty parentheses")]
    EmptyParens,
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.input[self.pos..].chars().next() {
            if c.is_whitespace() {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn pos(&self) -> usize {
        self.pos
    }

    fn parse_or(&mut self) -> Result<Predicate, ParseError> {
        let mut left = self.parse_and()?;
        loop {
            self.skip_ws();
            if self.peek() == Some('O') && self.input[self.pos..].starts_with("OR") {
                self.pos += 2;
                let right = self.parse_and()?;
                left = Predicate::Or(Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Predicate, ParseError> {
        let mut left = self.parse_not()?;
        loop {
            self.skip_ws();
            if self.peek() == Some('A') && self.input[self.pos..].starts_with("AND") {
                self.pos += 3;
                let right = self.parse_not()?;
                left = Predicate::And(Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Predicate, ParseError> {
        self.skip_ws();
        if self.peek() == Some('N') && self.input[self.pos..].starts_with("NOT") {
            self.pos += 3;
            let inner = self.parse_not()?;
            return Ok(Predicate::Not(Box::new(inner)));
        }
        self.parse_atom()
    }

    fn parse_atom(&mut self) -> Result<Predicate, ParseError> {
        self.skip_ws();
        if self.peek() == Some('(') {
            self.pos += 1;
            let inner = self.parse_or()?;
            self.skip_ws();
            if self.peek() != Some(')') {
                return Err(ParseError::EmptyParens);
            }
            self.pos += 1;
            return Ok(inner);
        }
        // Identifier + operator + literal.
        let field = self.parse_identifier()?;
        self.skip_ws();
        let op = self.parse_operator()?;
        self.skip_ws();
        let value = self.parse_literal()?;
        Ok(match op {
            "==" => Predicate::Eq(field, value),
            "!=" => Predicate::Ne(field, value),
            ">" => Predicate::Gt(field, value.parse().unwrap_or(0.0)),
            "<" => Predicate::Lt(field, value.parse().unwrap_or(0.0)),
            ">=" => Predicate::Ge(field, value.parse().unwrap_or(0.0)),
            "<=" => Predicate::Le(field, value.parse().unwrap_or(0.0)),
            _ => return Err(ParseError::UnexpectedChar(self.pos(), '?')),
        })
    }

    fn parse_identifier(&mut self) -> Result<String, ParseError> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
        if start == self.pos {
            return Err(ParseError::UnexpectedChar(self.pos(), self.peek().unwrap_or(' ')));
        }
        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_operator(&mut self) -> Result<&'static str, ParseError> {
        let rest = &self.input[self.pos..];
        for op in &[">=", "<=", "==", "!=", ">", "<"] {
            if rest.starts_with(op) {
                self.pos += op.len();
                return Ok(op);
            }
        }
        Err(ParseError::UnexpectedChar(self.pos(), self.peek().unwrap_or(' ')))
    }

    fn parse_literal(&mut self) -> Result<String, ParseError> {
        if self.peek() == Some('"') {
            self.pos += 1;
            let start = self.pos;
            while let Some(c) = self.peek() {
                if c == '"' {
                    break;
                }
                self.pos += c.len_utf8();
            }
            if self.peek() != Some('"') {
                return Err(ParseError::UnexpectedEof);
            }
            let s = self.input[start..self.pos].to_string();
            self.pos += 1;
            Ok(s)
        } else {
            let start = self.pos;
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() || c == '.' {
                    self.pos += c.len_utf8();
                } else {
                    break;
                }
            }
            if start == self.pos {
                return Err(ParseError::UnexpectedChar(self.pos(), self.peek().unwrap_or(' ')));
            }
            Ok(self.input[start..self.pos].to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rock_synth() -> TrackSnapshot {
        TrackSnapshot {
            title: "X".into(),
            artist: "Y".into(),
            album: "Z".into(),
            genre: "Rock".into(),
            year: Some(2020),
            bpm: Some(130),
            key: "A minor".into(),
            mood: "energetic".into(),
            favourite: true,
            play_count: 10,
            codec: "flac".into(),
            duration_ms: 240_000,
        }
    }

    #[test]
    fn evaluate_eq() {
        let t = rock_synth();
        let p = Predicate::Eq("genre".into(), "Rock".into());
        assert!(evaluate(&p, &t));
        let p = Predicate::Eq("genre".into(), "Pop".into());
        assert!(!evaluate(&p, &t));
    }

    #[test]
    fn evaluate_gt() {
        let t = rock_synth();
        let p = Predicate::Gt("bpm".into(), 100.0);
        assert!(evaluate(&p, &t));
        let p = Predicate::Gt("bpm".into(), 200.0);
        assert!(!evaluate(&p, &t));
    }

    #[test]
    fn evaluate_and_or_not() {
        let t = rock_synth();
        let p = Predicate::And(
            Box::new(Predicate::Eq("genre".into(), "Rock".into())),
            Box::new(Predicate::Gt("bpm".into(), 100.0)),
        );
        assert!(evaluate(&p, &t));
        let p = Predicate::Or(
            Box::new(Predicate::Eq("genre".into(), "Pop".into())),
            Box::new(Predicate::Gt("bpm".into(), 100.0)),
        );
        assert!(evaluate(&p, &t));
        let p = Predicate::Not(Box::new(Predicate::Eq("genre".into(), "Rock".into())));
        assert!(!evaluate(&p, &t));
    }

    #[test]
    fn parse_simple() {
        let p = parse("genre == \"Rock\"").expect("ok");
        assert!(matches!(p, Predicate::Eq(_, _)));
    }

    #[test]
    fn parse_and_or() {
        let p = parse("genre == \"Rock\" AND bpm > 120").expect("ok");
        assert!(matches!(p, Predicate::And(_, _)));
        let p = parse("genre == \"Rock\" OR genre == \"Pop\"").expect("ok");
        assert!(matches!(p, Predicate::Or(_, _)));
    }

    #[test]
    fn parse_with_parens() {
        let p = parse("(genre == \"Rock\" OR genre == \"Pop\") AND bpm > 100").expect("ok");
        assert!(matches!(p, Predicate::And(_, _)));
    }

    #[test]
    fn filter_returns_matching_track_ids() {
        let t1 = rock_synth();
        let mut t2 = rock_synth();
        t2.genre = "Pop".into();
        let p = Predicate::Eq("genre".into(), "Rock".into());
        let ids = filter(&p, &[(1, t1), (2, t2)]);
        assert_eq!(ids, vec![1]);
    }

    #[test]
    fn update_playlist_evaluates_predicate() {
        let mut pl = SmartPlaylist {
            id: 1,
            name: "Rock".into(),
            source: PlaylistSource::Predicate {
                expression: "genre == \"Rock\"".into(),
            },
            track_ids: Vec::new(),
            is_smart: true,
            smart_predicate: None,
        };
        let t1 = rock_synth();
        let mut t2 = rock_synth();
        t2.genre = "Pop".into();
        update_playlist(&mut pl, &[(1, t1), (2, t2)]);
        assert_eq!(pl.track_ids, vec![1]);
    }
}
